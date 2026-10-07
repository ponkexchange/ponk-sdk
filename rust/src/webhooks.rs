//! Verify webhook deliveries from ponk.
//!
//! ponk POSTs a signed JSON body to a URL you register. This module checks
//! that a delivery really came from ponk and has not been replayed. It is the
//! whole receiver side of the feature.
//!
//! # Why there is no `Client::create_webhook`
//!
//! Registering an endpoint is an account-level act, and the API scopes it to
//! a signed-in session (`/me/webhooks`), not to an API key: a key cannot mint
//! keys and it cannot register a URL that will receive your account's
//! activity either. A key-holding client like this one therefore cannot manage
//! endpoints, and a method for it would answer 401 forever. Register an endpoint
//! from a signed-in session with `POST /me/webhooks`, keep the `whsec_...`
//! signing secret its response carries, then verify here.
//!
//! # The contract
//!
//! Every delivery carries three headers:
//!
//! * [`SIGNATURE_HEADER`]: `t=<unix seconds>,v1=<hex HMAC-SHA256>`, where the
//!   MAC is keyed with your endpoint's signing secret and covers the bytes
//!   `"<t>.<raw body>"`. The timestamp is inside the MAC, so a captured
//!   delivery cannot be aged forward without breaking `v1`.
//! * [`EVENT_HEADER`]: the event kind, also in the body as `event`.
//! * [`DELIVERY_HEADER`]: the delivery id. A retry of the same event reuses
//!   it, so it is the key to deduplicate on.
//!
//! Only a 2xx answer counts as delivered. Anything else is retried with
//! backoff, and an endpoint whose deliveries keep exhausting their retries is
//! disabled until it is re-enabled (`POST /me/webhooks/{id}/reenable`, also
//! session-authenticated).
//!
//! ```no_run
//! # fn handle(body: &[u8], signature: &str, secret: &str) -> u16 {
//! match ponk::webhooks::verify(body, signature, secret) {
//!     Ok(event) => {
//!         if event.event == "mandate_expiring" {
//!             // Renew the agent's mandate in the app before it lapses.
//!         }
//!         200
//!     }
//!     // Do not act on the body. Answer non-2xx.
//!     Err(_) => 400,
//! }
//! # }
//! ```
//!
//! # The one mistake that matters
//!
//! Verify the RAW BYTES you received. The MAC covers the exact body on the
//! wire, and re-serializing a parsed value is not guaranteed to reproduce it,
//! so verifying `serde_json::to_vec(&parsed)` fails for reasons that look like
//! a ponk bug and are not. Read the request body as bytes, verify, and only
//! then use the [`WebhookEvent`] this returns.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// The header carrying `t=<unix>,v1=<hex>`.
pub const SIGNATURE_HEADER: &str = "X-Ponk-Signature";
/// The header carrying the event kind.
pub const EVENT_HEADER: &str = "X-Ponk-Event";
/// The header carrying the delivery id, stable across retries.
pub const DELIVERY_HEADER: &str = "X-Ponk-Delivery";

/// Default replay window, in seconds, either side of the signed timestamp.
///
/// Five minutes covers clock skew between two hosts and is short enough that
/// a captured delivery is not replayable an hour later.
pub const DEFAULT_TOLERANCE_SECS: u64 = 300;

/// Every event kind ponk can send, as of this release.
///
/// For recognition, NOT for rejecting: the server may add a kind before this
/// crate is updated, and dropping an unknown event would be worse than handing
/// it to you, so [`verify`] accepts any `event`. `GET /me/webhooks/kinds` is
/// the live list.
pub const EVENT_KINDS: [&str; 18] = [
    "agent_recommendation",
    "agent_auto_executed",
    "agent_auto_exec_blocked",
    "agent_auto_exec_failed",
    "agent_quarantined",
    "agent_kill_switch",
    "agent_tp_sl_stop",
    "agent_rebalance_unverified",
    "agent_autonomous_enabled",
    "agent_autonomous_revoked",
    "agent_entry_opened",
    "agent_out_of_range",
    "agent_stalled",
    "agent_low_exit_gas",
    "agent_shared_wallet_hold",
    "mandate_expiring",
    "mandate_expired",
    "alert_rule",
];

/// One verified delivery.
///
/// Fields mirror the JSON body one for one. `data` is the event's own metadata
/// and its shape varies by `event`, so it stays a raw JSON value rather than
/// a type that would have to guess.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct WebhookEvent {
    /// The event kind, one of [`EVENT_KINDS`] or a newer one.
    pub event: String,
    /// The underlying event's id. A retried delivery carries the same one.
    pub event_id: Option<String>,
    /// RFC 3339.
    pub occurred_at: Option<String>,
    pub severity: Option<String>,
    pub title: Option<String>,
    pub message: Option<String>,
    pub user_id: Option<String>,
    /// `None` for an event that is not about one agent.
    pub agent_id: Option<String>,
    pub data: Option<serde_json::Value>,
}

/// A delivery did not verify. Answer non-2xx and do not act on the body.
///
/// The message says which check failed, because a receiver that cannot tell
/// "wrong secret" from "too old" cannot debug its own integration. It never
/// contains the secret or the computed digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidSignature {
    reason: String,
}

impl InvalidSignature {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    /// Which check failed, in a sentence.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

impl fmt::Display for InvalidSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid ponk webhook signature: {}", self.reason)
    }
}

impl std::error::Error for InvalidSignature {}

/// Split `t=<unix>,v1=<hex>` into the timestamp and the hex digest.
///
/// Tolerates spaces and extra comma-separated fields, because a future
/// version may add one, and a receiver that breaks on it breaks on an upgrade
/// it did not make.
pub fn parse_signature_header(header: &str) -> Result<(i64, String), InvalidSignature> {
    let mut t = None;
    let mut v1 = None;
    for chunk in header.split(',') {
        let Some((key, value)) = chunk.split_once('=') else {
            continue;
        };
        match key.trim() {
            "t" => t = Some(value.trim()),
            "v1" => v1 = Some(value.trim()),
            _ => {}
        }
    }
    let (Some(t), Some(v1)) = (t, v1) else {
        let shown: String = header.chars().take(80).collect();
        return Err(InvalidSignature::new(format!(
            "signature header is missing t= or v1=; got {shown:?}"
        )));
    };
    let timestamp = t
        .parse::<i64>()
        .map_err(|_| InvalidSignature::new("signature timestamp is not an integer"))?;
    Ok((timestamp, v1.to_string()))
}

/// Check a delivery against the current clock and the default
/// [`DEFAULT_TOLERANCE_SECS`] window, and return it parsed.
///
/// `raw_body` is the request body exactly as it arrived. `signature_header` is
/// the value of [`SIGNATURE_HEADER`]. `secret` is the endpoint's `whsec_...`
/// signing secret.
pub fn verify(
    raw_body: &[u8],
    signature_header: &str,
    secret: &str,
) -> Result<WebhookEvent, InvalidSignature> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    verify_at(raw_body, signature_header, secret, DEFAULT_TOLERANCE_SECS, now)
}

/// [`verify`] with an explicit replay window and clock.
///
/// `tolerance_secs` bounds how far the signed timestamp may sit from `now`,
/// in either direction. Pass 0 to disable the age check, which you want only
/// when replaying a captured delivery in a test; in production it is the
/// replay window. `now_unix` is seconds since the Unix epoch.
pub fn verify_at(
    raw_body: &[u8],
    signature_header: &str,
    secret: &str,
    tolerance_secs: u64,
    now_unix: i64,
) -> Result<WebhookEvent, InvalidSignature> {
    let (timestamp, sent_hex) = parse_signature_header(signature_header)?;

    if tolerance_secs > 0 {
        let age = now_unix.abs_diff(timestamp);
        if age > tolerance_secs {
            return Err(InvalidSignature::new(format!(
                "delivery is {age} seconds old, outside the {tolerance_secs} second tolerance"
            )));
        }
    }

    // A digest that is not even hex cannot match. Decoding first lets the
    // comparison below run on bytes, in constant time.
    let sent = decode_hex(&sent_hex).ok_or_else(mismatch)?;

    // The MAC covers "<t>.<raw body>", with `t` exactly as the server printed
    // it, so the timestamp is part of what is authenticated.
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| InvalidSignature::new("the signing secret could not key HMAC-SHA256"))?;
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(raw_body);
    // `verify_slice` compares in constant time. A byte-at-a-time comparison
    // leaks how much of a forged digest was right, which is enough to build
    // the rest of it.
    mac.verify_slice(&sent).map_err(|_| mismatch())?;

    let value: serde_json::Value = serde_json::from_slice(raw_body).map_err(|e| {
        // Verified but unreadable: its own message, because the secret is
        // evidently right and the problem is upstream of this crate.
        InvalidSignature::new(format!("delivery verified but its body is not JSON: {e}"))
    })?;
    if !value.is_object() {
        return Err(InvalidSignature::new("delivery body is not a JSON object"));
    }
    serde_json::from_value(value).map_err(|e| {
        InvalidSignature::new(format!(
            "delivery verified but its body is not a ponk event: {e}"
        ))
    })
}

fn mismatch() -> InvalidSignature {
    InvalidSignature::new(
        "signature does not match; check you are verifying the RAW request body and using \
         the secret for this endpoint",
    )
}

fn decode_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    fn nibble(c: u8) -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    }
    s.as_bytes()
        .chunks(2)
        .map(|pair| Some((nibble(pair[0])? << 4) | nibble(pair[1])?))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "whsec_test";
    const NOW: i64 = 1_700_000_000;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Signs exactly the way `apps/api/src/webhook_dispatcher.rs`
    /// `signature_header` does.
    fn sign(body: &[u8], t: i64, secret: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(t.to_string().as_bytes());
        mac.update(b".");
        mac.update(body);
        format!("t={t},v1={}", hex(&mac.finalize().into_bytes()))
    }

    fn body() -> Vec<u8> {
        br#"{"event":"agent_out_of_range","event_id":"e1","occurred_at":"2026-10-07T00:00:00Z","severity":"warning","title":"t","message":"m","user_id":"u1","agent_id":"a1","data":{"bin":7}}"#.to_vec()
    }

    #[test]
    fn a_good_delivery_verifies_and_parses() {
        let b = body();
        let ev = verify_at(&b, &sign(&b, NOW, SECRET), SECRET, 300, NOW).unwrap();
        assert_eq!(ev.event, "agent_out_of_range");
        assert_eq!(ev.agent_id.as_deref(), Some("a1"));
        assert_eq!(ev.event_id.as_deref(), Some("e1"));
        assert_eq!(ev.data.unwrap()["bin"], 7);
    }

    /// The same vector the dispatcher's own test pins:
    /// HMAC-SHA256(key "whsec_test", "1700000000.{}").
    #[test]
    fn matches_the_dispatcher_vector() {
        let header = sign(b"{}", NOW, SECRET);
        assert!(header.starts_with("t=1700000000,v1="));
        let ev = verify_at(b"{}", &header, SECRET, 300, NOW).unwrap();
        assert_eq!(ev.event, "");
        assert!(ev.agent_id.is_none());
    }

    #[test]
    fn a_tampered_body_is_rejected() {
        let b = body();
        let header = sign(&b, NOW, SECRET);
        let mut tampered = b.clone();
        tampered.push(b' ');
        assert!(verify_at(&tampered, &header, SECRET, 300, NOW).is_err());
    }

    #[test]
    fn the_wrong_secret_is_rejected() {
        let b = body();
        let e = verify_at(&b, &sign(&b, NOW, SECRET), "whsec_other", 300, NOW).unwrap_err();
        assert!(e.reason().contains("does not match"));
    }

    #[test]
    fn a_moved_timestamp_is_rejected() {
        let b = body();
        let header = sign(&b, NOW, SECRET).replace("t=1700000000", "t=1700000001");
        assert!(verify_at(&b, &header, SECRET, 300, NOW).is_err());
    }

    #[test]
    fn an_old_delivery_is_rejected_unless_the_window_is_off() {
        let b = body();
        let header = sign(&b, NOW, SECRET);
        let e = verify_at(&b, &header, SECRET, 300, NOW + 301).unwrap_err();
        assert!(e.reason().contains("outside the 300 second tolerance"));
        // A delivery from the future is outside the window too.
        assert!(verify_at(&b, &header, SECRET, 300, NOW - 301).is_err());
        assert!(verify_at(&b, &header, SECRET, 300, NOW + 300).is_ok());
        assert!(verify_at(&b, &header, SECRET, 0, NOW + 1_000_000).is_ok());
    }

    #[test]
    fn a_malformed_header_is_rejected() {
        let b = body();
        assert!(verify_at(&b, "v1=abcd", SECRET, 0, NOW).is_err());
        assert!(verify_at(&b, "t=1700000000", SECRET, 0, NOW).is_err());
        assert!(verify_at(&b, "t=soon,v1=abcd", SECRET, 0, NOW).is_err());
        assert!(verify_at(&b, "t=1700000000,v1=zz", SECRET, 0, NOW).is_err());
    }

    #[test]
    fn extra_fields_and_spaces_in_the_header_are_tolerated() {
        let b = body();
        let header = sign(&b, NOW, SECRET).replace(",", " , v2=ignored , ");
        assert!(verify_at(&b, &header, SECRET, 300, NOW).is_ok());
    }

    #[test]
    fn a_verified_non_object_body_is_refused() {
        let header = sign(b"[1]", NOW, SECRET);
        let e = verify_at(b"[1]", &header, SECRET, 300, NOW).unwrap_err();
        assert!(e.reason().contains("not a JSON object"));
    }

    #[test]
    fn eighteen_distinct_kinds() {
        let mut kinds = EVENT_KINDS.to_vec();
        kinds.sort_unstable();
        kinds.dedup();
        assert_eq!(kinds.len(), 18);
        assert!(EVENT_KINDS.contains(&"mandate_expiring"));
        assert!(EVENT_KINDS.contains(&"mandate_expired"));
    }
}
