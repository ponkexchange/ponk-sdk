//! Errors from the ponk public API.
//!
//! Every error the API returns carries the same JSON envelope:
//!
//! ```json
//! {"error": {"code": "conflict", "message": "exit already running", "request_id": "..."}}
//! ```
//!
//! [`ApiError`] is that envelope plus the HTTP status. [`ApiError::kind`]
//! groups the statuses worth branching on, while `code` is kept verbatim: a
//! 422 can carry a risk code (a Token-2022 mint on Orca, a transfer-hook mint)
//! and a closed enum here would drop codes the server adds later.

use std::fmt;

use serde::Deserialize;

/// What this crate can return instead of a result.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The server answered with a non-2xx status.
    #[error("{0}")]
    Api(ApiError),

    /// The request never produced a usable response: DNS, connection, TLS or
    /// timeout.
    ///
    /// A fund-moving call that times out has NOT necessarily failed. The
    /// server keeps running the work, and `exit` is idempotent, so calling it
    /// again continues the same exit rather than starting a second one.
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),

    /// The base URL was not an absolute http(s) URL.
    #[error("invalid base url: {0}")]
    InvalidBaseUrl(String),

    /// A 2xx body did not match the shape this crate expects.
    #[error("could not decode the response from {path}: {message}")]
    Decode { path: String, message: String },
}

impl Error {
    /// True when the failure was a timeout rather than a refusal.
    pub fn is_timeout(&self) -> bool {
        match self {
            Error::Transport(e) => e.is_timeout(),
            _ => false,
        }
    }

    /// The API error inside, when this is one.
    pub fn api(&self) -> Option<&ApiError> {
        match self {
            Error::Api(e) => Some(e),
            _ => None,
        }
    }
}

/// One non-2xx response, decoded.
#[derive(Debug, Clone)]
pub struct ApiError {
    /// HTTP status.
    pub status: u16,
    /// Stable machine code, for example `forbidden`, `invalid_strategy_config`,
    /// or a risk code on a 422. `http_<status>` when the body was not the
    /// envelope at all, such as a proxy error page.
    pub code: String,
    /// Client-facing message. A 5xx message is deliberately generic; the
    /// detail is in the server's logs under `request_id`.
    pub message: String,
    /// Quote this to support. Present on anything the API itself produced.
    pub request_id: Option<String>,
    /// The body as it arrived, for whatever this struct does not name.
    pub body: Option<String>,
}

impl ApiError {
    /// The status group worth branching on.
    pub fn kind(&self) -> ErrorKind {
        match self.status {
            400 => ErrorKind::BadRequest,
            401 => ErrorKind::Unauthorized,
            403 => ErrorKind::Forbidden,
            404 => ErrorKind::NotFound,
            409 => ErrorKind::Conflict,
            422 => ErrorKind::Unprocessable,
            429 => ErrorKind::RateLimited,
            s if s >= 500 => ErrorKind::Server,
            _ => ErrorKind::Other,
        }
    }

    /// Whether retrying the identical request could plausibly succeed.
    ///
    /// A rejected key, a read-only key and a malformed body will be rejected
    /// again just as fast, so they are false.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self.kind(),
            ErrorKind::RateLimited | ErrorKind::Conflict | ErrorKind::Server
        )
    }

    pub(crate) fn from_body(status: u16, body: Option<String>) -> Self {
        let parsed = body
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Envelope>(raw).ok())
            .map(|e| e.error);
        match parsed {
            Some(payload) => ApiError {
                status,
                code: payload.code,
                message: payload.message,
                request_id: payload.request_id,
                body,
            },
            None => ApiError {
                status,
                code: format!("http_{status}"),
                message: format!("request failed with status {status}"),
                request_id: None,
                body,
            },
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ponk api {}: {} ({})",
            self.status, self.message, self.code
        )?;
        if let Some(id) = &self.request_id {
            write!(f, " request_id={id}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

/// The status groups a caller usually branches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Status 400: a malformed request, or a strategy config that did not
    /// validate against its declared strategy.
    BadRequest,
    /// Status 401: a missing, malformed, unknown or revoked key. The last
    /// two are deliberately indistinguishable. Do not retry.
    Unauthorized,
    /// Status 403: the key is valid but read-only. Mint a `trade` key. Do
    /// not retry.
    Forbidden,
    /// Status 404: no such object, or it belongs to another account. Those
    /// two are also deliberately indistinguishable.
    NotFound,
    /// Status 409: conflicts with in-flight work, for example an exit
    /// already running. Retry after a pause.
    Conflict,
    /// Status 422: understood but cannot apply, for example compounding an
    /// agent with no open position. `code` carries the risk code when there
    /// is one.
    Unprocessable,
    /// Status 429: back off and retry.
    RateLimited,
    /// Status 5xx.
    Server,
    /// Any other non-2xx status.
    Other,
}

/// True when a body is the API's own error envelope, as opposed to a normal
/// response that happens to arrive with a non-2xx status.
pub(crate) fn is_envelope(body: &str) -> bool {
    serde_json::from_str::<Envelope>(body).is_ok()
}

#[derive(Debug, Deserialize)]
struct Envelope {
    error: EnvelopePayload,
}

#[derive(Debug, Deserialize)]
struct EnvelopePayload {
    code: String,
    message: String,
    #[serde(default)]
    request_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_shared_envelope() {
        let body = r#"{"error":{"code":"conflict","message":"exit already running","request_id":"req-1"}}"#;
        let e = ApiError::from_body(409, Some(body.to_string()));
        assert_eq!(e.kind(), ErrorKind::Conflict);
        assert_eq!(e.code, "conflict");
        assert_eq!(e.request_id.as_deref(), Some("req-1"));
        assert!(e.is_retryable());
        assert!(e.to_string().contains("exit already running"));
    }

    #[test]
    fn keeps_a_risk_code_verbatim() {
        let body =
            r#"{"error":{"code":"token_2022_unsupported","message":"no","request_id":null}}"#;
        let e = ApiError::from_body(422, Some(body.to_string()));
        assert_eq!(e.kind(), ErrorKind::Unprocessable);
        assert_eq!(e.code, "token_2022_unsupported");
        assert!(!e.is_retryable());
    }

    #[test]
    fn a_proxy_error_page_is_not_mistaken_for_the_envelope() {
        let e = ApiError::from_body(502, Some("<html>502 Bad Gateway</html>".to_string()));
        assert_eq!(e.code, "http_502");
        assert_eq!(e.kind(), ErrorKind::Server);
        assert!(e.body.is_some());
    }

    #[test]
    fn a_degraded_health_body_is_not_an_envelope() {
        // /health answers 503 with its full report. The client returns that
        // report, so it must not be mistaken for an error body.
        let health = r#"{"status":"degraded","checks":{"database":{"healthy":true}}}"#;
        assert!(!is_envelope(health));
        let envelope = r#"{"error":{"code":"internal_error","message":"internal error"}}"#;
        assert!(is_envelope(envelope));
        assert!(!is_envelope("<html>502</html>"));
    }

    #[test]
    fn a_read_only_key_is_not_retryable() {
        let body = r#"{"error":{"code":"forbidden","message":"forbidden","request_id":"r"}}"#;
        let e = ApiError::from_body(403, Some(body.to_string()));
        assert_eq!(e.kind(), ErrorKind::Forbidden);
        assert!(!e.is_retryable());
    }
}
