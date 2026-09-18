//! The HTTP client: one method per endpoint, and nothing else.

use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{ApiError, Error};
use crate::models::*;

/// Production. Pass a different base URL to reach another environment.
pub const DEFAULT_BASE_URL: &str = "https://ponk.exchange/api";

/// Read and control calls. Anything slower than this is a bug on the server.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// `compound`, `withdraw` and `exit` send several transactions and wait for
/// each confirmation, so they get their own, longer timeout.
pub const DEFAULT_FUND_TIMEOUT: Duration = Duration::from_secs(180);

type Result<T> = std::result::Result<T, Error>;

/// A client for the ponk public API.
///
/// Cheap to clone: it shares one connection pool.
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    timeout: Duration,
    fund_timeout: Duration,
}

impl Client {
    /// Construct with the base URL and an API key.
    ///
    /// ```no_run
    /// # fn main() -> Result<(), ponk::Error> {
    /// let ponk = ponk::Client::new(ponk::DEFAULT_BASE_URL, std::env::var("PONK_API_KEY").unwrap())?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Result<Self> {
        Self::builder(base_url).api_key(api_key).build()
    }

    /// Construct without a key, for the endpoints that need none: [`health`],
    /// [`pool`] and [`ponk_perks`]. Every other method will get a 401.
    ///
    /// [`health`]: Client::health
    /// [`pool`]: Client::pool
    /// [`ponk_perks`]: Client::ponk_perks
    pub fn public(base_url: impl Into<String>) -> Result<Self> {
        Self::builder(base_url).build()
    }

    /// Set the key, the timeouts or the underlying `reqwest::Client`.
    pub fn builder(base_url: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            base_url: base_url.into(),
            api_key: None,
            timeout: DEFAULT_TIMEOUT,
            fund_timeout: DEFAULT_FUND_TIMEOUT,
            http: None,
        }
    }

    /// The base URL this client sends to, without a trailing slash.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The timeout used by [`Client::compound`], [`Client::withdraw`] and
    /// [`Client::exit`].
    pub fn fund_timeout(&self) -> Duration {
        self.fund_timeout
    }

    // -- public, no key required ------------------------------------------

    /// `GET /health`. Whether the API and its database and RPC are up.
    ///
    /// A degraded API answers 503 with the full report of which component is
    /// down, and that report is the point of the call, so this returns it
    /// rather than failing. Check `status` (`ok` or `degraded`) and
    /// `checks.database.healthy` / `checks.helius.healthy`.
    pub async fn health(&self) -> Result<Health> {
        let request = self.http.get(self.url("/health")).timeout(self.timeout);
        self.send(request, "/health", &[503]).await
    }

    /// `GET /pools/{dex}/{address}`. One pool, decoded from chain.
    ///
    /// `dex` is `meteora_dlmm`, `orca` or `ponk_clouds`. This route is rate
    /// limited per IP: it spends the same RPC budget the live agents use.
    pub async fn pool(&self, dex: &str, address: &str) -> Result<PoolSnapshot> {
        self.get(&format!("/pools/{}/{}", seg(dex), seg(address)))
            .await
    }

    /// `GET /public/ponk/perks/{address}`. What fees a wallet pays.
    ///
    /// Reads only the wallet's public $PONK balance. Works for any wallet,
    /// with no key and no signature.
    pub async fn ponk_perks(&self, wallet_address: &str) -> Result<PonkPerks> {
        self.get(&format!("/public/ponk/perks/{}", seg(wallet_address)))
            .await
    }

    // -- identity ----------------------------------------------------------

    /// `GET /v1/me`. The first call any integration should make.
    pub async fn whoami(&self) -> Result<WhoAmI> {
        self.get("/v1/me").await
    }

    // -- agents, read ------------------------------------------------------

    /// `GET /v1/agents`. Every agent this key's wallet owns.
    pub async fn list_agents(&self) -> Result<Vec<Agent>> {
        self.get("/v1/agents").await
    }

    /// `GET /v1/agents/{id}`. A foreign or missing id is 404 alike, so the API
    /// never confirms that another account's agent exists.
    pub async fn get_agent(&self, agent_id: &str) -> Result<Agent> {
        self.get(&format!("/v1/agents/{}", seg(agent_id))).await
    }

    /// `GET /v1/agents/{id}/performance`. Live value, PnL, fees and IL.
    pub async fn agent_performance(&self, agent_id: &str) -> Result<AgentPerformance> {
        self.get(&format!("/v1/agents/{}/performance", seg(agent_id)))
            .await
    }

    /// `GET /v1/agents/{id}/position`. The live on-chain range.
    pub async fn agent_position(&self, agent_id: &str) -> Result<AgentPosition> {
        self.get(&format!("/v1/agents/{}/position", seg(agent_id)))
            .await
    }

    /// `GET /v1/agents/{id}/wallet`. The agent's own managed wallet.
    /// Autonomous agents only.
    pub async fn agent_wallet(&self, agent_id: &str) -> Result<AgentWallet> {
        self.get(&format!("/v1/agents/{}/wallet", seg(agent_id)))
            .await
    }

    /// `GET /v1/positions`. Every LP position the wallet holds, agent-managed
    /// or not, re-read from chain on each call.
    pub async fn list_positions(&self) -> Result<Vec<Position>> {
        self.get("/v1/positions").await
    }

    /// `GET /v1/logs`. Recent agent activity, newest first.
    pub async fn list_logs(&self, limit: Option<i64>) -> Result<Vec<ActionLog>> {
        match limit {
            Some(n) => self.get(&format!("/v1/logs?limit={n}")).await,
            None => self.get("/v1/logs").await,
        }
    }

    /// Any GET this crate does not name, decoded as raw JSON.
    ///
    /// The escape hatch for a field or a route added to the API after this
    /// version. `path` starts with a slash and is appended to the base URL.
    pub async fn get_json(&self, path: &str) -> Result<serde_json::Value> {
        self.get(path).await
    }

    // -- agents, write (needs a `trade` key) -------------------------------

    /// `POST /v1/agents`. Create an agent.
    ///
    /// An agent created here is stamped `origin='api'` and pays the API
    /// performance rate for its whole life, including after autonomous mode is
    /// enabled for it in the app. [`PonkPerks::api_agent_fee`] is that rate.
    pub async fn create_agent(&self, agent: &NewAgent) -> Result<Agent> {
        self.post("/v1/agents", Some(agent), self.timeout).await
    }

    /// `POST /v1/agents/{id}/pause`. Stop the loop.
    ///
    /// The position stays open and keeps earning. Nothing is closed or swept.
    pub async fn pause_agent(&self, agent_id: &str) -> Result<Agent> {
        self.post(
            &format!("/v1/agents/{}/pause", seg(agent_id)),
            None::<&()>,
            self.timeout,
        )
        .await
    }

    /// `POST /v1/agents/{id}/resume`. Restart the loop.
    pub async fn resume_agent(&self, agent_id: &str) -> Result<Agent> {
        self.post(
            &format!("/v1/agents/{}/resume", seg(agent_id)),
            None::<&()>,
            self.timeout,
        )
        .await
    }

    /// `POST /v1/agents/{id}/dry-run`. Simulate instead of sending.
    pub async fn set_dry_run(&self, agent_id: &str, dry_run: bool) -> Result<Agent> {
        self.post(
            &format!("/v1/agents/{}/dry-run", seg(agent_id)),
            Some(&serde_json::json!({ "dry_run": dry_run })),
            self.timeout,
        )
        .await
    }

    /// `POST /v1/agents/{id}/mode`. `manual` or `auto`.
    pub async fn set_mode(&self, agent_id: &str, mode: &str) -> Result<Agent> {
        self.post(
            &format!("/v1/agents/{}/mode", seg(agent_id)),
            Some(&serde_json::json!({ "mode": mode })),
            self.timeout,
        )
        .await
    }

    // -- agents, fund moving (needs a `trade` key) -------------------------

    /// `POST /v1/agents/{id}/compound`. Claim fees and redeposit them now.
    ///
    /// Meteora DLMM autonomous agents only. The range is read from chain, so
    /// the re-deposit cannot be redirected.
    pub async fn compound(&self, agent_id: &str) -> Result<ActionReceipt> {
        self.post(
            &format!("/v1/agents/{}/compound", seg(agent_id)),
            None::<&()>,
            self.fund_timeout,
        )
        .await
    }

    /// `POST /v1/agents/{id}/withdraw`. Move SOL out of the agent's wallet.
    ///
    /// Pass `None` to sweep everything spendable. The destination is the
    /// wallet that owns the agent and cannot be named by the request.
    pub async fn withdraw(&self, agent_id: &str, lamports: Option<u64>) -> Result<Withdrawal> {
        let body = match lamports {
            Some(n) => serde_json::json!({ "lamports": n }),
            None => serde_json::json!({}),
        };
        self.post(
            &format!("/v1/agents/{}/withdraw", seg(agent_id)),
            Some(&body),
            self.fund_timeout,
        )
        .await
    }

    /// `POST /v1/agents/{id}/exit`. The full stop.
    ///
    /// Halts the agent, closes its position and sweeps every token plus native
    /// SOL back to the owner's wallet. Idempotent: if a call times out the
    /// work keeps running on the server, and calling again continues it rather
    /// than double-sending.
    pub async fn exit(&self, agent_id: &str) -> Result<Withdrawal> {
        self.post(
            &format!("/v1/agents/{}/exit", seg(agent_id)),
            None::<&()>,
            self.fund_timeout,
        )
        .await
    }

    // -- transport ---------------------------------------------------------

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let request = self.http.get(self.url(path)).timeout(self.timeout);
        self.send(request, path, &[]).await
    }

    async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: Option<&B>,
        timeout: Duration,
    ) -> Result<T> {
        let mut request = self.http.post(self.url(path)).timeout(timeout);
        // An empty POST still sends a JSON object, so the server's extractor
        // sees the content type it expects.
        request = match body {
            Some(b) => request.json(b),
            None => request.json(&serde_json::json!({})),
        };
        self.send(request, path, &[]).await
    }

    /// `body_statuses` lists non-2xx statuses whose body is a result rather
    /// than a failure, such as the 503 a degraded `/health` answers with. One
    /// of those still fails when the body IS the error envelope.
    async fn send<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        path: &str,
        body_statuses: &[u16],
    ) -> Result<T> {
        let mut request = request.header(reqwest::header::ACCEPT, "application/json");
        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }
        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            let body_is_the_result =
                body_statuses.contains(&status.as_u16()) && !crate::error::is_envelope(&body);
            if !body_is_the_result {
                return Err(Error::Api(ApiError::from_body(status.as_u16(), Some(body))));
            }
        }
        serde_json::from_str(&body).map_err(|e| Error::Decode {
            path: path.to_string(),
            message: e.to_string(),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

/// Builder for [`Client`].
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    base_url: String,
    api_key: Option<String>,
    timeout: Duration,
    fund_timeout: Duration,
    http: Option<reqwest::Client>,
}

impl ClientBuilder {
    /// The `ponk_live_...` secret from Settings, API keys.
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Timeout for read and control calls.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Timeout for `compound`, `withdraw` and `exit`.
    pub fn fund_timeout(mut self, timeout: Duration) -> Self {
        self.fund_timeout = timeout;
        self
    }

    /// Send through an existing `reqwest::Client`, to share its connection
    /// pool or carry a proxy setting.
    pub fn http_client(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }

    pub fn build(self) -> Result<Client> {
        let base_url = self.base_url.trim_end_matches('/').to_string();
        if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
            return Err(Error::InvalidBaseUrl(self.base_url));
        }
        let http = match self.http {
            Some(http) => http,
            None => reqwest::Client::builder().build()?,
        };
        Ok(Client {
            http,
            base_url,
            api_key: self.api_key,
            timeout: self.timeout,
            fund_timeout: self.fund_timeout,
        })
    }
}

/// Percent-encode one path segment, so an id can never change the route.
fn seg(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_base_url_must_be_absolute() {
        assert!(matches!(
            Client::new("ponk.exchange/api", "k"),
            Err(Error::InvalidBaseUrl(_))
        ));
        assert!(Client::new("https://ponk.exchange/api", "k").is_ok());
    }

    #[test]
    fn a_trailing_slash_does_not_double_up() {
        let c = Client::new("https://ponk.exchange/api/", "k").unwrap();
        assert_eq!(c.base_url(), "https://ponk.exchange/api");
        assert_eq!(c.url("/v1/me"), "https://ponk.exchange/api/v1/me");
    }

    #[test]
    fn a_path_segment_cannot_escape_its_route() {
        assert_eq!(seg("../../health"), "..%2F..%2Fhealth");
        assert_eq!(
            seg("018f0000-0000-7000-8000-000000000000"),
            "018f0000-0000-7000-8000-000000000000"
        );
    }

    #[test]
    fn fund_calls_get_the_longer_timeout() {
        let c = Client::new("https://ponk.exchange/api", "k").unwrap();
        assert!(c.fund_timeout() > DEFAULT_TIMEOUT);
    }
}
