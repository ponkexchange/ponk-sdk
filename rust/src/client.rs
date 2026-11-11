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

/// The header that carries the caller's Jupiter access token on the order
/// calls. The token is sent on the one request it is given to, never stored.
pub const JUPITER_SESSION_HEADER: &str = "x-jupiter-session";

type Result<T> = std::result::Result<T, Error>;

/// A client for the ponk public API.
///
/// Cheap to clone: it shares one connection pool.
#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::Client,
    base_url: String,
    /// The web app's origin, for the handful of boards the app serves itself
    /// rather than through the API (`/pools/board`, `/discover/feed`). Derived
    /// from `base_url` by default; see [`ClientBuilder::web_url`]. Requests to
    /// it never carry the API key: every board on it is public, and a custom
    /// pair of origins must not leak the key across them.
    web_url: String,
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
    /// [`pool`], [`range_cost`], [`pool_history`], [`ponk_perks`], the public
    /// boards and the public self-custody LP builders (for example
    /// [`damm_v2_open_prepare`]). Every key-authenticated method will get a
    /// 401.
    ///
    /// [`health`]: Client::health
    /// [`pool`]: Client::pool
    /// [`range_cost`]: Client::range_cost
    /// [`pool_history`]: Client::pool_history
    /// [`ponk_perks`]: Client::ponk_perks
    /// [`damm_v2_open_prepare`]: Client::damm_v2_open_prepare
    pub fn public(base_url: impl Into<String>) -> Result<Self> {
        Self::builder(base_url).build()
    }

    /// Set the key, the timeouts or the underlying `reqwest::Client`.
    pub fn builder(base_url: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            base_url: base_url.into(),
            web_url: None,
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

    /// The web app origin the pool boards are read from, without a trailing
    /// slash. By default the base URL with its `/api` suffix removed.
    pub fn web_url(&self) -> &str {
        &self.web_url
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
    /// `dex` is `meteora_dlmm`, `meteora_damm_v2`, `orca`, `ponk_clouds` or
    /// `raydium`. This route is rate limited per IP (a 429 past the limit): it
    /// spends the same RPC budget the live agents use. `sell_warning` and
    /// `mispricing_warning` on the result are advice to show beside the pool,
    /// never a refusal.
    pub async fn pool(&self, dex: &str, address: &str) -> Result<PoolSnapshot> {
        self.get(&format!("/pools/{}/{}", seg(dex), seg(address)))
            .await
    }

    /// `GET /pools/{dex}/{address}/range-cost?bins={total_bins}`. What a range
    /// of `total_bins` bins, centred on the active bin, costs to open.
    ///
    /// Meteora DLMM only: any other `dex` answers 422, because its account
    /// model would make this arithmetic a fabricated number. `total_bins` of 0
    /// answers 400. A width past the venue's ceiling is clamped rather than
    /// refused, and `clamped` says so. Rate limited per IP, like [`pool`].
    ///
    /// [`pool`]: Client::pool
    pub async fn range_cost(
        &self,
        dex: &str,
        address: &str,
        total_bins: u32,
    ) -> Result<RangeCostQuote> {
        self.get(&format!(
            "/pools/{}/{}/range-cost?bins={total_bins}",
            seg(dex),
            seg(address)
        ))
        .await
    }

    /// `GET /public/pools/{dex}/{address}/history?resolution=&buckets=`. One
    /// pool's volume, fees, fee rate and TVL per bucket, each field's source
    /// named in `sources`, plus the LVR edge where ponk measured it recently.
    ///
    /// `dex` is `meteora_dlmm`, `orca` or `raydium`; any other venue answers
    /// 422. `resolution` is `day` (default 30 buckets, at most 90) or `hour`
    /// (default 48, at most 96); a count outside that answers 400. The
    /// in-progress bucket is never served, and a bucket no source has a figure
    /// for is absent rather than zero. No key needed; rate limited per IP.
    /// [`Client::pool_history_keyed`] reads the same body on the key's budget.
    pub async fn pool_history(
        &self,
        dex: &str,
        address: &str,
        resolution: Option<&str>,
        buckets: Option<u32>,
    ) -> Result<PoolHistory> {
        self.get(&pool_history_path("/public/pools", dex, address, resolution, buckets))
            .await
    }

    /// `GET /v1/pools/{dex}/{address}/history`: [`Client::pool_history`] on
    /// the API key's budget instead of the per-IP limit. Needs a key.
    pub async fn pool_history_keyed(
        &self,
        dex: &str,
        address: &str,
        resolution: Option<&str>,
        buckets: Option<u32>,
    ) -> Result<PoolHistory> {
        self.get(&pool_history_path("/v1/pools", dex, address, resolution, buckets))
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

    /// `GET /public/agents-leaderboard?window=`. Top Ponkillionaires: every
    /// agent its owner chose to share, ranked against holding.
    ///
    /// `window` is `24h`, `7d`, `30d` or `all` (`None` takes the server
    /// default). No key needed; rate limited per IP. Each row carries `copy`,
    /// so a row can be copied with [`AgentCopy::to_new_agent`] and
    /// [`Client::create_agent`] without a second read.
    pub async fn leaderboard(&self, window: Option<&str>) -> Result<Leaderboard> {
        self.get(&with_query("/public/agents-leaderboard", &[("window", window.map(str::to_string))]))
            .await
    }

    /// `GET /public/agents/{slug}`. A shared agent's public profile. 404 for
    /// an unknown slug, 410 for one its owner stopped sharing.
    pub async fn public_agent(&self, slug: &str) -> Result<PublicAgent> {
        self.get(&format!("/public/agents/{}", seg(slug))).await
    }

    /// `GET /public/agents/{slug}/value-history?range=`. The profile's value
    /// chart. A link that hides USD answers 404.
    pub async fn public_agent_value_history(
        &self,
        slug: &str,
        range: Option<&str>,
    ) -> Result<ValueHistory> {
        self.get(&with_query(
            &format!("/public/agents/{}/value-history", seg(slug)),
            &[("range", range.map(str::to_string))],
        ))
        .await
    }

    /// `GET /public/points/leaderboard?limit&offset`. The points season board,
    /// signed out. Handles are referral codes, never wallets.
    pub async fn public_points_leaderboard(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<PointsLeaderboard> {
        self.get(&with_query(
            "/public/points/leaderboard",
            &[
                ("limit", limit.map(|n| n.to_string())),
                ("offset", offset.map(|n| n.to_string())),
            ],
        ))
        .await
    }

    /// `GET /public/referral/{code}`. Whether a referral code or handle
    /// exists (404 when it does not). Reveals no wallet and no figure.
    pub async fn resolve_referral(&self, code: &str) -> Result<PublicReferral> {
        self.get(&format!("/public/referral/{}", seg(code))).await
    }

    /// `GET /public/agents-activity`. What the board's agents are doing now:
    /// confirmed actions of the last 24 hours and every live hold, with the
    /// reason and the check count. Same agents and privacy as the
    /// leaderboard; no event carries an id, a slug, a name or a wallet.
    pub async fn agents_activity(&self) -> Result<AgentsActivity> {
        self.get("/public/agents-activity").await
    }

    /// `GET /public/ponkillionaires/week`. This week's race for the top three
    /// (a lower agent fee rate for the following week plus points) and last
    /// week's winners.
    pub async fn ponkillionaires_week(&self) -> Result<PonkillionairesWeek> {
        self.get("/public/ponkillionaires/week").await
    }

    // -- X Tracker, public ---------------------------------------------------

    /// `GET /public/x/feed`. The curated X accounts' posts and events, newest
    /// first. See [`XFeedQuery`] for the filters.
    pub async fn x_feed(&self, query: &XFeedQuery) -> Result<XFeed> {
        self.get(&with_query("/public/x/feed", &query.params())).await
    }

    /// `GET /public/x/trending?window=&limit=`. Tokens trending on X with
    /// their first caller. `window` is `1h` (default) or `24h`; `limit` 1 to 50.
    pub async fn x_trending(&self, window: Option<&str>, limit: Option<i64>) -> Result<XTrending> {
        self.get(&with_query(
            "/public/x/trending",
            &[
                ("window", window.map(str::to_string)),
                ("limit", limit.map(|n| n.to_string())),
            ],
        ))
        .await
    }

    /// `GET /public/x/callers?days=`. The caller leaderboard (1 to 90 days,
    /// default 30), ranked by median 24h multiple, then by calls.
    pub async fn x_callers(&self, days: Option<i64>) -> Result<XCallers> {
        self.get(&with_query("/public/x/callers", &[("days", days.map(|n| n.to_string()))]))
            .await
    }

    /// `GET /public/calls`. The Ponk Calls feed, newest first. `source` is
    /// `ponk` (default), `x` or `all`; `before` an RFC 3339 cursor. Public.
    pub async fn calls_feed(
        &self,
        source: Option<&str>,
        mint: Option<&str>,
        before: Option<&str>,
        limit: Option<i64>,
    ) -> Result<serde_json::Value> {
        self.get(&with_query(
            "/public/calls",
            &[
                ("source", source.map(str::to_string)),
                ("mint", mint.map(str::to_string)),
                ("before", before.map(str::to_string)),
                ("limit", limit.map(|n| n.to_string())),
            ],
        ))
        .await
    }

    /// `GET /public/calls/callers`. The callers board. Public.
    pub async fn callers_board(&self, days: Option<i64>, source: Option<&str>) -> Result<serde_json::Value> {
        self.get(&with_query(
            "/public/calls/callers",
            &[("days", days.map(|n| n.to_string())), ("source", source.map(str::to_string))],
        ))
        .await
    }

    /// `GET /public/calls/caller/{who}`. One caller's profile and calls. Public.
    pub async fn caller(&self, who: &str) -> Result<serde_json::Value> {
        self.get(&format!("/public/calls/caller/{}", seg(who))).await
    }

    /// `GET /public/x/token/{mint}`. Every tracked mention of one token. 400
    /// for a string that is not a Solana address.
    pub async fn x_token(&self, mint: &str) -> Result<XToken> {
        self.get(&format!("/public/x/token/{}", seg(mint))).await
    }

    /// `GET /public/x/accounts`. The curated account directory and its categories.
    pub async fn x_accounts(&self) -> Result<XDirectory> {
        self.get("/public/x/accounts").await
    }

    /// `GET /public/x/status`. The feed source's state.
    pub async fn x_status(&self) -> Result<XSource> {
        self.get("/public/x/status").await
    }

    // -- Wallet Tracker, public ----------------------------------------------

    /// `GET /public/wallets/feed`. The curated wallets' swaps and LP actions,
    /// merged. See [`WalletFeedQuery`] for the filters.
    pub async fn wallets_feed(&self, query: &WalletFeedQuery) -> Result<WalletFeed> {
        self.get(&with_query("/public/wallets/feed", &query.params())).await
    }

    /// `GET /public/wallets/defaults`. The curated wallet list.
    pub async fn wallets_defaults(&self) -> Result<CuratedWallets> {
        self.get("/public/wallets/defaults").await
    }

    /// `GET /public/wallets/status`. The chain reader's state.
    pub async fn wallets_status(&self) -> Result<WalletSource> {
        self.get("/public/wallets/status").await
    }

    /// `GET /public/wallets/{address}/activity`. Any wallet's recent swaps and
    /// LP actions: from the tracker when it stores the wallet (`mode`
    /// `tracked`), read from chain otherwise (`live`, one page). Rate limited
    /// per IP. `query.wallet` is ignored here.
    pub async fn wallet_activity(&self, address: &str, query: &WalletFeedQuery) -> Result<WalletActivity> {
        let q = WalletFeedQuery { wallet: None, ..query.clone() };
        self.get(&with_query(&format!("/public/wallets/{}/activity", seg(address)), &q.params()))
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
    ///
    /// For an agent on an EVM chain the answer is the EVM view
    /// ([`AgentWallet::is_evm`]): the deposit address, the gas coin and its
    /// balances in wei and USD, the one withdrawal address and
    /// `mandate_status`. A 404 until the wallet exists;
    /// [`Client::create_agent_wallet`] creates it.
    pub async fn agent_wallet(&self, agent_id: &str) -> Result<AgentWallet> {
        self.get(&format!("/v1/agents/{}/wallet", seg(agent_id)))
            .await
    }

    /// `GET /v1/agents/{id}/mandate`. The custody mandate the agent is
    /// signing under: what ponk may do for it, the caps, and when the
    /// authority runs out.
    ///
    /// The expiry is the thing to watch. A lapsed mandate produces no error
    /// anywhere: the agent keeps ticking and every signature it asks for is
    /// denied. `status == "expired"` or a negative `seconds_remaining` is how
    /// you find out. A 404 means the agent has no active grant, which reads
    /// as "not autonomous".
    ///
    /// For an older agent with no wallet of its own this reads the legacy
    /// per-user shared wallet's mandate. [`set_autonomous`] never uses that
    /// one: enabling needs a mandate on the agent's OWN wallet.
    ///
    /// [`set_autonomous`]: Client::set_autonomous
    pub async fn agent_mandate(&self, agent_id: &str) -> Result<Mandate> {
        self.get(&format!("/v1/agents/{}/mandate", seg(agent_id)))
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

    /// `GET /v1/agents/{id}/value-history?range=`. The position's value per
    /// bucket next to what its opening tokens would be worth held. `range` is
    /// `24h`, `7d` (server default), `30d` or `all`.
    pub async fn agent_value_history(
        &self,
        agent_id: &str,
        range: Option<&str>,
    ) -> Result<ValueHistory> {
        self.get(&with_query(
            &format!("/v1/agents/{}/value-history", seg(agent_id)),
            &[("range", range.map(str::to_string))],
        ))
        .await
    }

    /// `GET /v1/agents/{id}/decisions?hours&limit`. Why the agent acted or
    /// held: the server-computed state, what it evaluated and declined, and
    /// which economics basis ran. Raw JSON. `hours` defaults to 24 (1 to 720).
    pub async fn agent_decisions(
        &self,
        agent_id: &str,
        hours: Option<i64>,
        limit: Option<i64>,
    ) -> Result<serde_json::Value> {
        self.get(&with_query(
            &format!("/v1/agents/{}/decisions", seg(agent_id)),
            &[
                ("hours", hours.map(|n| n.to_string())),
                ("limit", limit.map(|n| n.to_string())),
            ],
        ))
        .await
    }

    /// `GET /v1/agents/{id}/public-link`. The agent's live leaderboard link;
    /// 404 when it is not shared.
    pub async fn get_public_link(&self, agent_id: &str) -> Result<PublicLink> {
        self.get(&format!("/v1/agents/{}/public-link", seg(agent_id)))
            .await
    }

    /// `GET /v1/follows`. Slugs of the shared agents this account follows.
    pub async fn list_follows(&self) -> Result<Follows> {
        self.get("/v1/follows").await
    }

    /// `GET /v1/points`. This account's standing in the active season.
    pub async fn points(&self) -> Result<MyPoints> {
        self.get("/v1/points").await
    }

    /// `GET /v1/points/leaderboard?limit&offset`. The season board with this
    /// account's row marked `is_me`.
    pub async fn points_leaderboard(
        &self,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<PointsLeaderboard> {
        self.get(&with_query(
            "/v1/points/leaderboard",
            &[
                ("limit", limit.map(|n| n.to_string())),
                ("offset", offset.map(|n| n.to_string())),
            ],
        ))
        .await
    }

    /// `GET /v1/referral`. Codes, handle, referred count and referral money
    /// (earned, owed and unproven, never summed).
    pub async fn referral(&self) -> Result<Referral> {
        self.get("/v1/referral").await
    }

    /// `GET /v1/referral/creator?days=`. The Creator dashboard by UTC day
    /// (`days` 7 to 90, default 30). 404 for an account without the tier;
    /// claiming a handle grants it.
    pub async fn creator_dashboard(&self, days: Option<i32>) -> Result<CreatorDashboard> {
        self.get(&with_query(
            "/v1/referral/creator",
            &[("days", days.map(|n| n.to_string()))],
        ))
        .await
    }

    /// `GET /v1/referral/handle/check?handle=`. Can this account claim
    /// `/r/<handle>`.
    pub async fn check_handle(&self, handle: &str) -> Result<HandleCheck> {
        self.get(&with_query(
            "/v1/referral/handle/check",
            &[("handle", Some(handle.to_string()))],
        ))
        .await
    }

    /// Any GET this crate does not name, decoded as raw JSON.
    ///
    /// The escape hatch for a field or a route added to the API after this
    /// version. `path` starts with a slash and is appended to the base URL.
    pub async fn get_json(&self, path: &str) -> Result<serde_json::Value> {
        self.get(path).await
    }

    /// `POST` any path on the API, raw JSON in and raw JSON out: the same
    /// escape hatch as [`Client::get_json`], for a route this crate has not
    /// typed yet. `body` of `None` sends `{}`. `timeout` of `None` uses the
    /// read timeout; pass [`Client::fund_timeout`] for a fund-moving route.
    pub async fn post_json(
        &self,
        path: &str,
        body: Option<&serde_json::Value>,
        timeout: Option<Duration>,
    ) -> Result<serde_json::Value> {
        self.post(path, body, timeout.unwrap_or(self.timeout)).await
    }

    /// `GET` any path on the WEB origin ([`Client::web_url`]), raw JSON out:
    /// the escape hatch for the app-served boards. Never carries the API key.
    pub async fn get_web_json(&self, path: &str) -> Result<serde_json::Value> {
        self.get_web(path).await
    }

    // -- agents, write (needs a `trade` key) -------------------------------

    /// `POST /v1/agents`. Create an agent.
    ///
    /// Left unset, [`NewAgent::dry_run`] makes a LIVE agent here; the `/v1`
    /// default is `false`. Set it to `true` to simulate first.
    ///
    /// An agent created here is stamped `origin='api'` and pays the API
    /// performance rate for its whole life, including after autonomous mode is
    /// enabled for it in the app. [`PonkPerks::api_agent_fee`] is that rate.
    pub async fn create_agent(&self, agent: &NewAgent) -> Result<Agent> {
        self.post("/v1/agents", Some(agent), self.timeout).await
    }

    /// `POST /v1/agents/{id}/autonomous`. Turn autonomous signing on or off,
    /// USING a custody mandate the owner already signed in the app.
    ///
    /// A key cannot mint a mandate, and this method sends no grant (the server
    /// answers 422 to a body that carries one). So:
    ///
    /// * `true` works only when the agent has its OWN agent wallet holding an
    ///   active mandate that has not expired and still pays out to this
    ///   account's wallet. Anything else is a 422 whose message says which.
    ///   On success the agent is `managed`, `auto` and live (`dry_run` false).
    ///   Check the returned `status`: an agent a previous exit left `stopped`
    ///   still needs [`resume_agent`].
    /// * `false` is always allowed, so a leaked key can never make the off
    ///   switch harder to reach than the on switch. It turns signing off; it
    ///   does NOT revoke the mandate, which stays on record until it expires
    ///   or is revoked in the app, so a later `true` can re-arm the agent
    ///   while it is unexpired. It closes nothing and moves nothing: funds
    ///   and any open position stay in the agent's wallet.
    ///
    /// The owner is notified either way, and an enable names the API key as
    /// the credential that did it.
    ///
    /// [`resume_agent`]: Client::resume_agent
    pub async fn set_autonomous(&self, agent_id: &str, enabled: bool) -> Result<AutonomousView> {
        self.post(
            &format!("/v1/agents/{}/autonomous", seg(agent_id)),
            Some(&serde_json::json!({ "enabled": enabled })),
            self.timeout,
        )
        .await
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

    /// `PATCH /v1/agents/{id}/config`. Change how an existing agent recenters
    /// without exiting it.
    ///
    /// `patch` is a flat JSON object: a key with a value sets it, a key set to
    /// `null` clears it back to its default, an absent key is unchanged. On a
    /// bin rebalancer, conservative or volatility-aware agent the keys are
    /// `recenter_mode` (`recenter`, `ping_pong` or `extend`),
    /// `rebalance_direction`, `liquidity_shape`, `ping_pong_bid_bins`,
    /// `ping_pong_ask_bins`, `ping_pong_spread_bins`, `shape_below`,
    /// `shape_above`, `ratio_below_pct`, `rebalance_buffer`, `schedule_secs`,
    /// `cooldown_sec` (60 to 86400) and `max_rebalances_per_day` (1 to 96).
    /// `recenter_mode: "keep_side"` with `keep_side` (`token_x` or `token_y`)
    /// keeps the position one-sided on that token. On Meteora, `reshape`
    /// (`{shape, trigger_bins, buffer_bins?, alternate?}`, trigger 1 to 51)
    /// re-deposits into the same position instead of closing and reopening;
    /// `null` turns it off. Adaptive range recovery takes
    /// `rebalance_cooldown_sec` and `max_rebalances_per_day`. Any other key,
    /// the range width included, is a 422
    /// naming it. Moves no funds; applies to the next recenter.
    ///
    /// ```no_run
    /// # async fn demo(ponk: &ponk::Client, id: &str) -> Result<(), ponk::Error> {
    /// let agent = ponk
    ///     .update_config(id, &serde_json::json!({
    ///         "recenter_mode": "extend",
    ///         "ping_pong_bid_bins": null,
    ///         "ping_pong_ask_bins": null
    ///     }))
    ///     .await?;
    /// # let _ = agent;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn update_config(&self, agent_id: &str, patch: &serde_json::Value) -> Result<Agent> {
        let path = format!("/v1/agents/{}/config", seg(agent_id));
        let request = self.http.patch(self.url(&path)).timeout(self.timeout).json(patch);
        self.send(request, &path, &[]).await
    }

    /// `POST /v1/agents/{id}/exits`. Change an existing agent's exits
    /// (`tp_sl`) and fee handling (`automation`), each a partial patch with
    /// the same null-clears rule as [`Client::update_config`]. A `None` block
    /// is left out of the body and so left unchanged. Moves no funds.
    ///
    /// Besides `stop_loss_pct` and `take_profit_pct`, `tp_sl` takes
    /// `take_profit_market_cap_usd`, `take_profit_position_value_usd` and
    /// `take_profit_value_with_fees_usd`, each a positive USD figure.
    pub async fn update_exits(
        &self,
        agent_id: &str,
        tp_sl: Option<&serde_json::Value>,
        automation: Option<&serde_json::Value>,
    ) -> Result<Agent> {
        self.post(
            &format!("/v1/agents/{}/exits", seg(agent_id)),
            Some(&exits_body(tp_sl, automation)),
            self.timeout,
        )
        .await
    }

    /// `POST /v1/agents/{id}/auto-compound`. Turn auto-compound on or off.
    ///
    /// `min_claimable_base_units` is the pending-fee floor in base units
    /// (`None` sends nothing, which the server reads as 0, the gas dust gate);
    /// `min_claimable_usd` is the dollar floor (`None` leaves it off). Turning
    /// it on while the agent accumulates fees is a 422. Moves no funds.
    pub async fn set_auto_compound(
        &self,
        agent_id: &str,
        auto_compound: bool,
        min_claimable_base_units: Option<u64>,
        min_claimable_usd: Option<f64>,
    ) -> Result<Agent> {
        self.post(
            &format!("/v1/agents/{}/auto-compound", seg(agent_id)),
            Some(&auto_compound_body(
                auto_compound,
                min_claimable_base_units,
                min_claimable_usd,
            )),
            self.timeout,
        )
        .await
    }

    /// `POST /v1/agents/optimize-draft`. Rank settings for a draft agent, the
    /// same answer the app's builder shows. Raw JSON. Needs a `trade` key;
    /// moves no funds and creates nothing.
    pub async fn optimize_draft(&self, draft: &serde_json::Value) -> Result<serde_json::Value> {
        self.post("/v1/agents/optimize-draft", Some(draft), self.timeout)
            .await
    }

    /// `POST /v1/agents/{id}/public-link`. Publish an agent to the
    /// leaderboard, or change the switches on its existing link (idempotent).
    ///
    /// `hide_usd` shares percentages only. `show_onchain` publishes the agent
    /// wallet, position and signatures; `None` keeps what the link has (a new
    /// link starts without it), and it is always off while `hide_usd` is set.
    /// Ponk Clouds agents answer 422 while Clouds is parked.
    pub async fn share_agent(
        &self,
        agent_id: &str,
        hide_usd: bool,
        show_onchain: Option<bool>,
    ) -> Result<PublicLink> {
        self.post(
            &format!("/v1/agents/{}/public-link", seg(agent_id)),
            Some(&share_body(hide_usd, show_onchain)),
            self.timeout,
        )
        .await
    }

    /// `DELETE /v1/agents/{id}/public-link`. Stop sharing. The slug stays dead.
    pub async fn unshare_agent(&self, agent_id: &str) -> Result<LinkRevoked> {
        self.delete(&format!("/v1/agents/{}/public-link", seg(agent_id)))
            .await
    }

    /// `POST /v1/follows/{slug}`. Be told when a shared agent opens,
    /// rebalances or exits. Idempotent; your own agent is a 422.
    pub async fn follow(&self, slug: &str) -> Result<Follow> {
        self.post(&format!("/v1/follows/{}", seg(slug)), None::<&()>, self.timeout)
            .await
    }

    /// `DELETE /v1/follows/{slug}`. Stop following. Idempotent.
    pub async fn unfollow(&self, slug: &str) -> Result<Follow> {
        self.delete(&format!("/v1/follows/{}", seg(slug))).await
    }

    /// `POST /v1/referral/handle`. Claim or change this account's
    /// `/r/<handle>`. The old handle keeps working as a code, and a handle
    /// makes the account a Creator. 409 when someone else owns it.
    pub async fn claim_handle(&self, handle: &str) -> Result<Referral> {
        self.post(
            "/v1/referral/handle",
            Some(&serde_json::json!({ "handle": handle })),
            self.timeout,
        )
        .await
    }

    // -- agents, fund moving (needs a `trade` key) -------------------------

    /// `POST /v1/agents/{id}/compound`. Claim fees and redeposit them now.
    ///
    /// Autonomous agents on Meteora DLMM, Meteora DAMM v2, Orca and Raydium
    /// CLMM; a Ponk Clouds agent is refused (that venue has no fee claim). The
    /// position is read from chain, so the re-deposit cannot be redirected: on
    /// DAMM v2 it goes back into the same pool share.
    ///
    /// # `Ok` is not success
    ///
    /// A refusal at signing time, most commonly an expired custody mandate but
    /// also a risk halt or an exhausted daily cap, is recorded on the action
    /// rather than returned as an error: the call answers 200 and this method
    /// returns `Ok` with a receipt whose `status` is failed, whose `signature`
    /// is `None`, and whose `error_message` says why. So read the receipt.
    ///
    /// ```no_run
    /// # async fn demo(ponk: &ponk::Client, id: &str) -> Result<(), ponk::Error> {
    /// let r = ponk.compound(id).await?;
    /// if r.signature.is_none() {
    ///     // Nothing reached the chain. error_message says why, and
    ///     // risk_failed_checks is populated when the risk engine refused.
    ///     eprintln!("compound did not land: {:?}", r.error_message);
    /// }
    /// # Ok(())
    /// # }
    /// ```
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
    ///
    /// ON AN EVM AGENT this sweeps the idle tokens and the gas coin to the
    /// mandate's proven withdrawal address on the same chain, and only with
    /// no position open: with one open it answers 422 with a sentence that
    /// says to exit. It stops the agent first ([`Client::resume_agent`]
    /// starts it again). `lamports` is ignored there. Read
    /// [`Withdrawal::status`], `sentence`, `amount_wei` and `received`.
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

    // -- the agent fleet (0.6.0) ---------------------------------------------

    /// `POST /v1/agents/batch`. Create many agents in one call.
    ///
    /// Each [`NewAgent`] is created on its own through the same handler as
    /// [`Client::create_agent`], so one bad pool refuses only that item. As
    /// there, an unset `dry_run` makes a LIVE agent. `create_wallets` also
    /// makes each agent's own managed wallet, ready for
    /// [`Client::fund_prepare`]; a wallet that could not be made is reported
    /// on its item and never undoes the create. Needs a `trade` key.
    pub async fn create_agents(&self, agents: &[NewAgent], create_wallets: bool) -> Result<FleetBatch> {
        self.post(
            "/v1/agents/batch",
            Some(&serde_json::json!({ "agents": agents, "create_wallets": create_wallets })),
            self.fund_timeout,
        )
        .await
    }

    /// `POST /v1/agents/fund/prepare`. UNSIGNED transfers from your wallet to
    /// each listed agent's own wallet, in as few transactions as fit.
    ///
    /// The request names agent ids, never addresses: the server resolves
    /// every destination under the owner scope. Neither this crate nor the
    /// server signs. Open each of [`FundPlan::sign_urls`] on ponk.exchange
    /// with the wallet connected, or sign `unsigned_tx_b64` in wallet software
    /// you control. A refused plan is `Ok` with `ok == Some(false)` and
    /// `refusal` set. Needs a `trade` key.
    pub async fn fund_prepare(&self, transfers: &[FundRequest]) -> Result<FundPlan> {
        self.post(
            "/v1/agents/fund/prepare",
            Some(&serde_json::json!({ "transfers": transfers })),
            self.timeout,
        )
        .await
    }

    /// `POST /v1/agents/batch/exit` for the listed agents: halt each one,
    /// close its position and sweep its wallet to the owner's own wallet. One
    /// failing does not stop the others. `running` items keep going on the
    /// server; collect them with [`Client::get_batch`]. Needs a `trade` key.
    pub async fn exit_agents(&self, agent_ids: &[&str]) -> Result<FleetBatch> {
        self.post(
            "/v1/agents/batch/exit",
            Some(&serde_json::json!({ "agent_ids": agent_ids })),
            self.fund_timeout,
        )
        .await
    }

    /// `POST /v1/agents/batch/exit` with `all: true`: every agent of this
    /// account that has a managed wallet.
    pub async fn exit_all_agents(&self) -> Result<FleetBatch> {
        self.post(
            "/v1/agents/batch/exit",
            Some(&serde_json::json!({ "all": true })),
            self.fund_timeout,
        )
        .await
    }

    /// `POST /v1/agents/batch/withdraw` for the listed agents: sweep each
    /// wallet's spendable SOL home. The agents keep running.
    pub async fn withdraw_agents(&self, agent_ids: &[&str]) -> Result<FleetBatch> {
        self.post(
            "/v1/agents/batch/withdraw",
            Some(&serde_json::json!({ "agent_ids": agent_ids })),
            self.fund_timeout,
        )
        .await
    }

    /// `POST /v1/agents/batch/withdraw` with `all: true`.
    pub async fn withdraw_all_agents(&self) -> Result<FleetBatch> {
        self.post(
            "/v1/agents/batch/withdraw",
            Some(&serde_json::json!({ "all": true })),
            self.fund_timeout,
        )
        .await
    }

    /// `GET /v1/agents/batch/{batch_id}`. The latest state of a batch this
    /// account started. Kept for 30 minutes after it finishes.
    pub async fn get_batch(&self, batch_id: &str) -> Result<FleetBatch> {
        self.get(&format!("/v1/agents/batch/{}", seg(batch_id))).await
    }

    /// `GET /v1/agents/fleet?offset&limit`. One page of every agent with its
    /// wallet, mandate and value. `limit` defaults to 25, at most 100.
    pub async fn fleet(&self, offset: Option<usize>, limit: Option<usize>) -> Result<Fleet> {
        self.get(&with_query(
            "/v1/agents/fleet",
            &[
                ("offset", offset.map(|n| n.to_string())),
                ("limit", limit.map(|n| n.to_string())),
            ],
        ))
        .await
    }

    /// Every fleet row, following `next_offset` page by page.
    pub async fn fleet_all(&self, limit: Option<usize>) -> Result<Vec<FleetRow>> {
        let mut rows = Vec::new();
        let mut offset = None;
        loop {
            let page = self.fleet(offset, limit).await?;
            rows.extend(page.rows);
            match page.next_offset {
                Some(next) if Some(next) != offset => offset = Some(next),
                _ => return Ok(rows),
            }
        }
    }

    /// `POST /v1/agents/{id}/wallet`. Create the agent's own managed wallet,
    /// or get the existing one back (`created == Some(false)`). An empty
    /// wallet grants no authority. Needs a `trade` key.
    pub async fn create_agent_wallet(&self, agent_id: &str) -> Result<AgentWalletCreated> {
        self.post(
            &format!("/v1/agents/{}/wallet", seg(agent_id)),
            None::<&()>,
            self.timeout,
        )
        .await
    }

    // -- agents, more actions (0.6.0) ----------------------------------------

    /// `POST /v1/agents/{id}/rebalance`. Close and reopen a managed agent's
    /// range now, signed by the agent's wallet under its mandate. Read the
    /// receipt: a refusal is `Ok` with `signature == None`. Needs a `trade`
    /// key.
    pub async fn rebalance(&self, agent_id: &str, request: &RebalanceRequest) -> Result<RebalanceReceipt> {
        self.post(
            &format!("/v1/agents/{}/rebalance", seg(agent_id)),
            Some(request),
            self.fund_timeout,
        )
        .await
    }

    /// `POST /v1/agents/{id}/collect-fees`. Claim a managed agent's fees.
    /// `destination` is `Some("agent")` (keep them in the agent wallet, the
    /// default) or `Some("connected")` (pay them to the owner's wallet). It
    /// is a choice, not an address. Needs a `trade` key.
    pub async fn collect_fees(&self, agent_id: &str, destination: Option<&str>) -> Result<ActionReceipt> {
        let path = format!("/v1/agents/{}/collect-fees", seg(agent_id));
        match destination {
            Some(d) => {
                self.post(&path, Some(&serde_json::json!({ "destination": d })), self.fund_timeout)
                    .await
            }
            None => self.post(&path, None::<&()>, self.fund_timeout).await,
        }
    }

    /// `PATCH /v1/agents/{id}`. Rename an agent (1 to 100 characters).
    pub async fn rename_agent(&self, agent_id: &str, name: &str) -> Result<Agent> {
        let path = format!("/v1/agents/{}", seg(agent_id));
        let request = self
            .http
            .patch(self.url(&path))
            .timeout(self.timeout)
            .json(&serde_json::json!({ "name": name }));
        self.send(request, &path, &[]).await
    }

    /// `DELETE /v1/agents/{id}`. Delete a finished agent. Refused (409) while
    /// it still holds a position or a balance. Moves no funds.
    pub async fn delete_agent(&self, agent_id: &str) -> Result<()> {
        self.delete::<Option<serde_json::Value>>(&format!("/v1/agents/{}", seg(agent_id)))
            .await
            .map(|_| ())
    }

    /// `GET /v1/agents/{id}/proof?hours=`. What each action cost and earned.
    pub async fn agent_proof(&self, agent_id: &str, hours: Option<i64>) -> Result<AgentProof> {
        self.get(&with_query(
            &format!("/v1/agents/{}/proof", seg(agent_id)),
            &[("hours", hours.map(|n| n.to_string()))],
        ))
        .await
    }

    /// `GET /v1/agents/{id}/range-history?range=`.
    pub async fn agent_range_history(&self, agent_id: &str, range: Option<&str>) -> Result<RangeHistory> {
        self.get(&with_query(
            &format!("/v1/agents/{}/range-history", seg(agent_id)),
            &[("range", range.map(str::to_string))],
        ))
        .await
    }

    // -- the legacy shared wallet (0.6.0) ------------------------------------

    /// `POST /v1/custody/withdraw`. SOL out of the legacy per-user shared
    /// wallet, to the owner's own wallet only. `None` sweeps what is
    /// spendable.
    pub async fn custody_withdraw(&self, lamports: Option<u64>) -> Result<Withdrawal> {
        let body = match lamports {
            Some(n) => serde_json::json!({ "lamports": n }),
            None => serde_json::json!({}),
        };
        self.post("/v1/custody/withdraw", Some(&body), self.fund_timeout).await
    }

    /// `POST /v1/custody/exit`. Exit every managed agent and sweep every
    /// managed wallet, the shared one included, to the owner's own wallet.
    pub async fn custody_exit(&self) -> Result<Withdrawal> {
        self.post("/v1/custody/exit", None::<&()>, self.fund_timeout).await
    }

    // -- account reads (0.6.0) -----------------------------------------------

    /// `GET /v1/portfolio`.
    pub async fn portfolio(&self) -> Result<Portfolio> {
        self.get("/v1/portfolio").await
    }

    /// `GET /v1/portfolio/history?range=` (`24h`, `7d`, `30d`, `90d`, `all`).
    pub async fn portfolio_history(&self, range: Option<&str>) -> Result<PortfolioHistory> {
        self.get(&with_query(
            "/v1/portfolio/history",
            &[("range", range.map(str::to_string))],
        ))
        .await
    }

    /// `GET /v1/transactions?limit=`.
    pub async fn list_transactions(&self, limit: Option<i64>) -> Result<Vec<Transaction>> {
        self.get(&with_query("/v1/transactions", &[("limit", limit.map(|n| n.to_string()))]))
            .await
    }

    /// `GET /v1/risk-events?limit=`.
    pub async fn list_risk_events(&self, limit: Option<i64>) -> Result<Vec<RiskEvent>> {
        self.get(&with_query("/v1/risk-events", &[("limit", limit.map(|n| n.to_string()))]))
            .await
    }

    /// `GET /v1/fees?limit=`.
    pub async fn list_fees(&self, limit: Option<i64>) -> Result<Vec<FeeEvent>> {
        self.get(&with_query("/v1/fees", &[("limit", limit.map(|n| n.to_string()))]))
            .await
    }

    /// `GET /v1/fees/summary`.
    pub async fn fee_summary(&self) -> Result<FeeSummary> {
        self.get("/v1/fees/summary").await
    }

    // -- notifications and recommendations (0.6.0) ---------------------------

    /// `GET /v1/notifications?status&limit`.
    pub async fn list_notifications(&self, status: Option<&str>, limit: Option<i64>) -> Result<Vec<Notification>> {
        self.get(&with_query(
            "/v1/notifications",
            &[
                ("status", status.map(str::to_string)),
                ("limit", limit.map(|n| n.to_string())),
            ],
        ))
        .await
    }

    /// `GET /v1/notifications/unread-count`.
    pub async fn unread_notifications(&self) -> Result<UnreadCount> {
        self.get("/v1/notifications/unread-count").await
    }

    /// `POST /v1/notifications/read-all`.
    pub async fn mark_all_notifications_read(&self) -> Result<ReadAll> {
        self.post("/v1/notifications/read-all", None::<&()>, self.timeout).await
    }

    /// `POST /v1/notifications/{id}/read`.
    pub async fn mark_notification_read(&self, notification_id: &str) -> Result<()> {
        self.post::<(), Option<serde_json::Value>>(
            &format!("/v1/notifications/{}/read", seg(notification_id)),
            None,
            self.timeout,
        )
        .await
        .map(|_| ())
    }

    /// `POST /v1/notifications/{id}/dismiss`.
    pub async fn dismiss_notification(&self, notification_id: &str) -> Result<()> {
        self.post::<(), Option<serde_json::Value>>(
            &format!("/v1/notifications/{}/dismiss", seg(notification_id)),
            None,
            self.timeout,
        )
        .await
        .map(|_| ())
    }

    /// `GET /v1/recommendations?status&limit`.
    pub async fn list_recommendations(&self, status: Option<&str>, limit: Option<i64>) -> Result<Vec<Recommendation>> {
        self.get(&with_query(
            "/v1/recommendations",
            &[
                ("status", status.map(str::to_string)),
                ("limit", limit.map(|n| n.to_string())),
            ],
        ))
        .await
    }

    /// `GET /v1/recommendations/{id}`.
    pub async fn get_recommendation(&self, recommendation_id: &str) -> Result<Recommendation> {
        self.get(&format!("/v1/recommendations/{}", seg(recommendation_id))).await
    }

    /// `POST /v1/recommendations/{id}/review`. Mark it seen.
    pub async fn review_recommendation(&self, recommendation_id: &str) -> Result<Recommendation> {
        self.recommendation_action(recommendation_id, "review").await
    }

    /// `POST /v1/recommendations/{id}/approve`. Flips the status only; it
    /// builds and signs no transaction.
    pub async fn approve_recommendation(&self, recommendation_id: &str) -> Result<Recommendation> {
        self.recommendation_action(recommendation_id, "approve").await
    }

    /// `POST /v1/recommendations/{id}/dismiss`.
    pub async fn dismiss_recommendation(&self, recommendation_id: &str) -> Result<Recommendation> {
        self.recommendation_action(recommendation_id, "dismiss").await
    }

    async fn recommendation_action(&self, recommendation_id: &str, action: &str) -> Result<Recommendation> {
        self.post(
            &format!("/v1/recommendations/{}/{}", seg(recommendation_id), seg(action)),
            None::<&()>,
            self.timeout,
        )
        .await
    }

    /// `POST /v1/recommendations/{id}/snooze`. `None` uses the server default.
    pub async fn snooze_recommendation(&self, recommendation_id: &str, minutes: Option<i64>) -> Result<Recommendation> {
        self.post(
            &format!("/v1/recommendations/{}/snooze", seg(recommendation_id)),
            Some(&serde_json::json!({ "minutes": minutes })),
            self.timeout,
        )
        .await
    }

    // -- settings: notifications and saved presets (0.6.0) -------------------

    /// `GET /v1/notify-settings`.
    pub async fn notify_settings(&self) -> Result<NotifySettings> {
        self.get("/v1/notify-settings").await
    }

    /// `PUT /v1/notify-settings`. Only the switches set to `Some` change.
    pub async fn update_notify_settings(&self, settings: &NotifySettings) -> Result<NotifySettings> {
        let path = "/v1/notify-settings";
        let request = self.http.put(self.url(path)).timeout(self.timeout).json(settings);
        self.send(request, path, &[]).await
    }

    /// `GET /v1/presets`.
    pub async fn list_presets(&self) -> Result<Vec<Preset>> {
        self.get("/v1/presets").await
    }

    /// `POST /v1/presets`. The stored, normalized preset comes back.
    pub async fn create_preset(&self, preset: &NewPreset) -> Result<Preset> {
        self.post("/v1/presets", Some(preset), self.timeout).await
    }

    /// `PUT /v1/presets/{id}`. Replace a preset in place, keeping its id.
    pub async fn update_preset(&self, preset_id: &str, preset: &NewPreset) -> Result<Preset> {
        let path = format!("/v1/presets/{}", seg(preset_id));
        let request = self.http.put(self.url(&path)).timeout(self.timeout).json(preset);
        self.send(request, &path, &[]).await
    }

    /// `DELETE /v1/presets/{id}`.
    pub async fn delete_preset(&self, preset_id: &str) -> Result<()> {
        self.delete::<Option<serde_json::Value>>(&format!("/v1/presets/{}", seg(preset_id)))
            .await
            .map(|_| ())
    }

    // -- swap orders: Jupiter Trigger V2 and Recurring (0.7.0) ---------------
    //
    // Two doors, and neither signs. `plan_orders` returns one link on
    // ponk.exchange where the owner's wallet signs every order (the site's
    // own builders run there). A program that holds its own wallet key uses
    // the self-sign steps: `orders_challenge` and `orders_verify` for a
    // Jupiter session, `craft_order_deposit` for an UNSIGNED deposit, then
    // `create_price_order` / `create_dca_order` with it signed; `cancel_order`
    // for an UNSIGNED withdrawal and `confirm_order_cancel` to land it.
    //
    // The Jupiter access token is an argument of each call, sent as the
    // `X-Jupiter-Session` header and never kept. Every wallet field is the
    // key owner's: the server fills it in and refuses another. Ponk adds no
    // fee to orders.
    //
    // OUTCOME UNKNOWN: create and confirm land a transaction during the
    // request. A 504 with code `outcome_unknown` means it may have landed.
    // Read `list_orders` first and never retry the same create blindly.

    /// `POST /v1/orders/plan`. Validate up to ten orders (and any cancels) and
    /// get the one link where the owner signs them. Nothing is created.
    pub async fn plan_orders(
        &self,
        orders: &[OrderIntent],
        cancel: &[OrderRef],
        cancel_all: bool,
    ) -> Result<OrderPlan> {
        let body = serde_json::json!({ "orders": orders, "cancel": cancel, "cancel_all": cancel_all });
        self.post("/v1/orders/plan", Some(&body), self.timeout).await
    }

    /// `POST /v1/orders/auth/challenge`. `kind` is `message` or `transaction`.
    pub async fn orders_challenge(&self, kind: &str) -> Result<JupiterChallenge> {
        self.post("/v1/orders/auth/challenge", Some(&serde_json::json!({ "type": kind })), self.timeout)
            .await
    }

    /// `POST /v1/orders/auth/verify` with the wallet's base58 signature of a
    /// message challenge.
    pub async fn orders_verify_message(&self, signature: &str) -> Result<JupiterSession> {
        let body = serde_json::json!({ "type": "message", "signature": signature });
        self.post("/v1/orders/auth/verify", Some(&body), self.timeout).await
    }

    /// `POST /v1/orders/auth/verify` with a signed transaction challenge (base64).
    pub async fn orders_verify_transaction(&self, signed_transaction: &str) -> Result<JupiterSession> {
        let body = serde_json::json!({ "type": "transaction", "signedTransaction": signed_transaction });
        self.post("/v1/orders/auth/verify", Some(&body), self.timeout).await
    }

    /// `POST /v1/orders/auth/refresh`.
    pub async fn orders_refresh(&self, refresh_token: &str) -> Result<JupiterSession> {
        let body = serde_json::json!({ "refreshToken": refresh_token });
        self.post("/v1/orders/auth/refresh", Some(&body), self.timeout).await
    }

    /// `POST /v1/orders/auth/logout`.
    pub async fn orders_logout(&self, refresh_token: &str) -> Result<serde_json::Value> {
        let body = serde_json::json!({ "refreshToken": refresh_token });
        self.post("/v1/orders/auth/logout", Some(&body), self.timeout).await
    }

    /// `GET /v1/orders/mint-support?mints=`. Per mint, null when usable in an
    /// order, else the reason Jupiter's deposit would refuse it.
    pub async fn order_mint_support(&self, mints: &[&str]) -> Result<serde_json::Value> {
        self.get(&with_query("/v1/orders/mint-support", &[("mints", Some(mints.join(",")))]))
            .await
    }

    /// `POST /v1/orders/deposit/craft`. The UNSIGNED deposit. `order_type` is
    /// `price` (with `order_sub_type` `single`, `oco` or `otoco`) or `dca`;
    /// `amount` is base units (for DCA, per order times the count).
    pub async fn craft_order_deposit(
        &self,
        jupiter_session: &str,
        input_mint: &str,
        output_mint: &str,
        amount: u64,
        order_type: &str,
        order_sub_type: Option<&str>,
    ) -> Result<DepositCraft> {
        let mut body = serde_json::json!({
            "inputMint": input_mint,
            "outputMint": output_mint,
            "amount": amount.to_string(),
            "orderType": order_type,
        });
        if let Some(sub) = order_sub_type {
            body["orderSubType"] = sub.into();
        }
        self.post_jupiter("/v1/orders/deposit/craft", jupiter_session, Some(&body), self.timeout)
            .await
    }

    /// `POST /v1/orders/price`. Create a limit, trailing, OCO or OTOCO order:
    /// Jupiter's create body with `depositRequestId` and `depositSignedTx`.
    /// Takes the fund timeout. Never retry an `outcome_unknown` blindly.
    pub async fn create_price_order(&self, jupiter_session: &str, body: &serde_json::Value) -> Result<OrderCreated> {
        self.post_jupiter("/v1/orders/price", jupiter_session, Some(body), self.fund_timeout)
            .await
    }

    /// `POST /v1/orders/dca`. Create a DCA plan (`time_based`, or
    /// `price_conditional` with `triggerMint` and a min and/or max USD
    /// price). Takes the fund timeout.
    pub async fn create_dca_order(&self, jupiter_session: &str, body: &serde_json::Value) -> Result<OrderCreated> {
        self.post_jupiter("/v1/orders/dca", jupiter_session, Some(body), self.fund_timeout)
            .await
    }

    /// `POST /v1/orders/{family}/{id}/cancel`. Stop an order and get its
    /// UNSIGNED withdrawal. `family` is `price` or `dca`.
    pub async fn cancel_order(&self, jupiter_session: &str, family: &str, order_id: &str) -> Result<OrderCancel> {
        let path = format!("/v1/orders/{}/{}/cancel", seg(family), seg(order_id));
        self.post_jupiter::<serde_json::Value, _>(&path, jupiter_session, None, self.timeout)
            .await
    }

    /// `POST /v1/orders/{family}/{id}/confirm-cancel`. Land the signed
    /// withdrawal; the funds return to the owner's wallet. Takes the fund timeout.
    pub async fn confirm_order_cancel(
        &self,
        jupiter_session: &str,
        family: &str,
        order_id: &str,
        signed_transaction: &str,
        cancel_request_id: &str,
    ) -> Result<OrderCancel> {
        let path = format!("/v1/orders/{}/{}/confirm-cancel", seg(family), seg(order_id));
        let body = serde_json::json!({ "signedTransaction": signed_transaction, "cancelRequestId": cancel_request_id });
        self.post_jupiter(&path, jupiter_session, Some(&body), self.fund_timeout)
            .await
    }

    /// `GET /v1/orders`. Jupiter's order history as it sent it: one family
    /// (`{"orders": [...]}`), or both as `{"price", "dca"}` when `family` is
    /// `None`. `state` is `active` (open orders) or `past`.
    pub async fn list_orders(
        &self,
        jupiter_session: &str,
        family: Option<&str>,
        state: Option<&str>,
        limit: Option<u32>,
    ) -> Result<serde_json::Value> {
        let path = with_query(
            "/v1/orders",
            &[
                ("family", family.map(str::to_string)),
                ("state", state.map(str::to_string)),
                ("limit", limit.map(|l| l.to_string())),
            ],
        );
        let request = self
            .http
            .get(self.url(&path))
            .timeout(self.timeout)
            .header(JUPITER_SESSION_HEADER, jupiter_session);
        self.send(request, &path, &[]).await
    }

    /// `POST /v1/orders/cancel`. Cancel step one for many orders, or every
    /// open one with `all`. Each item carries its UNSIGNED withdrawal.
    pub async fn cancel_orders(&self, jupiter_session: &str, orders: &[OrderRef], all: bool) -> Result<OrderBatch> {
        let body = serde_json::json!({ "orders": orders, "all": all });
        self.post_jupiter("/v1/orders/cancel", jupiter_session, Some(&body), self.timeout)
            .await
    }

    /// `POST /v1/orders/confirm-cancel`. Land up to ten signed withdrawals;
    /// each item is `{family, id, signedTransaction, cancelRequestId}`.
    /// Takes the fund timeout.
    pub async fn confirm_order_cancels(&self, jupiter_session: &str, items: &[serde_json::Value]) -> Result<OrderBatch> {
        let body = serde_json::json!({ "items": items });
        self.post_jupiter("/v1/orders/confirm-cancel", jupiter_session, Some(&body), self.fund_timeout)
            .await
    }

    // -- copy, live follow and Copy LP (0.7.0) --------------------------------

    /// `POST /v1/agents/{id}/copy-source`. Record that this agent copies a
    /// board agent (`source` is the row's key) and optionally follow it live.
    pub async fn copy_source(
        &self,
        agent_id: &str,
        source: &str,
        follow: bool,
        auto_follow_pool_moves: bool,
    ) -> Result<FollowStatus> {
        let body = serde_json::json!({ "source": source, "follow": follow, "auto_follow_pool_moves": auto_follow_pool_moves });
        self.post(&format!("/v1/agents/{}/copy-source", seg(agent_id)), Some(&body), self.timeout)
            .await
    }

    /// `GET /v1/agents/{id}/follow`.
    pub async fn follow_status(&self, agent_id: &str) -> Result<FollowStatus> {
        self.get(&format!("/v1/agents/{}/follow", seg(agent_id))).await
    }

    /// `POST /v1/agents/{id}/follow`. Follow, live, the agent this copy came
    /// from. Config only, never funds.
    pub async fn follow_leader(&self, agent_id: &str, auto_follow_pool_moves: bool) -> Result<FollowStatus> {
        let body = serde_json::json!({ "auto_follow_pool_moves": auto_follow_pool_moves });
        self.post(&format!("/v1/agents/{}/follow", seg(agent_id)), Some(&body), self.timeout)
            .await
    }

    /// `PATCH /v1/agents/{id}/follow`. `patch` takes `auto_follow_pool_moves`,
    /// `resume_keys` and `pause_keys`.
    pub async fn update_follow(&self, agent_id: &str, patch: &serde_json::Value) -> Result<FollowStatus> {
        let path = format!("/v1/agents/{}/follow", seg(agent_id));
        let request = self.http.patch(self.url(&path)).timeout(self.timeout).json(patch);
        self.send(request, &path, &[]).await
    }

    /// `DELETE /v1/agents/{id}/follow`.
    pub async fn unfollow_leader(&self, agent_id: &str) -> Result<FollowStatus> {
        self.delete(&format!("/v1/agents/{}/follow", seg(agent_id))).await
    }

    /// `POST /v1/agents/{id}/follow/offer`. Accept or decline a leader's pool
    /// move or exit.
    pub async fn answer_follow_offer(&self, agent_id: &str, accept: bool) -> Result<FollowStatus> {
        let body = serde_json::json!({ "accept": accept });
        self.post(&format!("/v1/agents/{}/follow/offer", seg(agent_id)), Some(&body), self.timeout)
            .await
    }

    /// `GET /v1/copy-follows`.
    pub async fn list_copy_follows(&self) -> Result<CopyFollows> {
        self.get("/v1/copy-follows").await
    }

    /// `POST /v1/agents/follow/batch`. Live follow (or unfollow) many copies.
    pub async fn follow_leaders(
        &self,
        agent_ids: &[&str],
        auto_follow_pool_moves: bool,
        unfollow: bool,
    ) -> Result<BatchResult> {
        let items: Vec<serde_json::Value> = agent_ids
            .iter()
            .map(|a| serde_json::json!({ "agent_id": a, "auto_follow_pool_moves": auto_follow_pool_moves }))
            .collect();
        let body = serde_json::json!({ "items": items, "unfollow": unfollow });
        self.post("/v1/agents/follow/batch", Some(&body), self.timeout).await
    }

    /// `POST /v1/follows/batch`. Follow (or unfollow) many shared agents by slug.
    pub async fn follow_many(&self, slugs: &[&str], unfollow: bool) -> Result<BatchResult> {
        let body = serde_json::json!({ "slugs": slugs, "unfollow": unfollow });
        self.post("/v1/follows/batch", Some(&body), self.timeout).await
    }

    /// `GET /v1/agents/{id}/copy-log`.
    pub async fn copy_log(&self, agent_id: &str) -> Result<CopyLog> {
        self.get(&format!("/v1/agents/{}/copy-log", seg(agent_id))).await
    }

    /// `POST /v1/agents/{id}/wallet-follow`. Mirror a wallet's LP moves.
    pub async fn wallet_follow(&self, agent_id: &str, request: &WalletFollowRequest) -> Result<CopyLog> {
        self.post(&format!("/v1/agents/{}/wallet-follow", seg(agent_id)), Some(request), self.timeout)
            .await
    }

    /// `DELETE /v1/agents/{id}/wallet-follow`.
    pub async fn unfollow_wallet(&self, agent_id: &str) -> Result<CopyLog> {
        self.delete(&format!("/v1/agents/{}/wallet-follow", seg(agent_id))).await
    }

    /// `POST /v1/agents/{id}/wallet-follow/offer`.
    pub async fn answer_wallet_offer(&self, agent_id: &str, accept: bool) -> Result<CopyLog> {
        let body = serde_json::json!({ "accept": accept });
        self.post(&format!("/v1/agents/{}/wallet-follow/offer", seg(agent_id)), Some(&body), self.timeout)
            .await
    }

    /// `GET /v1/copy-referral/{key}`.
    pub async fn copy_referral(&self, key: &str) -> Result<CopyReferralOffer> {
        self.get(&format!("/v1/copy-referral/{}", seg(key))).await
    }

    /// `POST /v1/copy-referral/{key}`.
    pub async fn attach_copy_referral(&self, key: &str) -> Result<CopyReferralAttach> {
        self.post::<serde_json::Value, _>(&format!("/v1/copy-referral/{}", seg(key)), None, self.timeout)
            .await
    }

    // -- the self-custody loop: prepare, sign in your wallet, mark (0.7.0) ----
    //
    // Every prepare returns UNSIGNED transactions for the key owner's own
    // wallet. Sign in wallet software you control, send, then report the
    // signature with the matching mark call. Bodies are the session routes'
    // own, passed through as JSON.

    /// `POST /v1/agents/{id}/rebalance/prepare`.
    pub async fn agent_rebalance_prepare(&self, agent_id: &str, body: &serde_json::Value) -> Result<Prepared> {
        self.post(&format!("/v1/agents/{}/rebalance/prepare", seg(agent_id)), Some(body), self.timeout)
            .await
    }

    /// `POST /v1/agents/{id}/rebalance/prepare-split[?simulate=true]`.
    pub async fn agent_rebalance_prepare_split(
        &self,
        agent_id: &str,
        body: &serde_json::Value,
        simulate: bool,
    ) -> Result<Prepared> {
        let mut path = format!("/v1/agents/{}/rebalance/prepare-split", seg(agent_id));
        if simulate {
            path.push_str("?simulate=true");
        }
        self.post(&path, Some(body), self.timeout).await
    }

    /// `POST /v1/agents/{id}/rebalance/prepare-open`. The open leg; sign `open_tx_b64`.
    pub async fn agent_rebalance_prepare_open(&self, agent_id: &str, body: &serde_json::Value) -> Result<Prepared> {
        self.post(&format!("/v1/agents/{}/rebalance/prepare-open", seg(agent_id)), Some(body), self.timeout)
            .await
    }

    /// `POST /v1/agents/{id}/rebalance/mark-signature`.
    pub async fn agent_rebalance_mark(&self, agent_id: &str, body: &serde_json::Value) -> Result<MarkedTx> {
        self.post(&format!("/v1/agents/{}/rebalance/mark-signature", seg(agent_id)), Some(body), self.timeout)
            .await
    }

    /// `POST /v1/agents/{id}/collect-fees/prepare`.
    pub async fn agent_fees_prepare(&self, agent_id: &str) -> Result<Prepared> {
        self.post::<serde_json::Value, _>(&format!("/v1/agents/{}/collect-fees/prepare", seg(agent_id)), None, self.timeout)
            .await
    }

    /// `POST /v1/agents/{id}/collect-fees/mark-signature`.
    pub async fn agent_fees_mark(&self, agent_id: &str, draft_event_ids: &[&str], signature: &str) -> Result<MarkedTx> {
        let body = serde_json::json!({ "draft_event_ids": draft_event_ids, "signature": signature });
        self.post(&format!("/v1/agents/{}/collect-fees/mark-signature", seg(agent_id)), Some(&body), self.timeout)
            .await
    }

    /// `POST /v1/agents/{id}/collect-fees/confirm`.
    pub async fn agent_fees_confirm(&self, agent_id: &str, signature: &str, fee_token_mints: &[&str]) -> Result<MarkedTx> {
        let body = serde_json::json!({ "signature": signature, "fee_token_mints": fee_token_mints });
        self.post(&format!("/v1/agents/{}/collect-fees/confirm", seg(agent_id)), Some(&body), self.timeout)
            .await
    }

    /// `POST /v1/positions/open/prepare`.
    pub async fn position_open_prepare(&self, body: &serde_json::Value) -> Result<Prepared> {
        self.post("/v1/positions/open/prepare", Some(body), self.timeout).await
    }

    /// `POST /v1/positions/mark-tx`.
    pub async fn position_mark_tx(&self, body: &serde_json::Value) -> Result<MarkedTx> {
        self.post("/v1/positions/mark-tx", Some(body), self.timeout).await
    }

    /// `POST /v1/positions/{position}/rebalance/prepare-split`.
    pub async fn position_rebalance_split(&self, position: &str, body: &serde_json::Value) -> Result<Prepared> {
        self.post(&format!("/v1/positions/{}/rebalance/prepare-split", seg(position)), Some(body), self.timeout)
            .await
    }

    /// `POST /v1/positions/{position}/rebalance/prepare-open`.
    pub async fn position_rebalance_open(&self, position: &str, body: &serde_json::Value) -> Result<Prepared> {
        self.post(&format!("/v1/positions/{}/rebalance/prepare-open", seg(position)), Some(body), self.timeout)
            .await
    }

    /// `POST /v1/positions/{position}/rebalance/prepare`. Orca or Raydium recenter.
    pub async fn position_rebalance_tick(&self, position: &str, body: &serde_json::Value) -> Result<Prepared> {
        self.post(&format!("/v1/positions/{}/rebalance/prepare", seg(position)), Some(body), self.timeout)
            .await
    }

    /// `POST /v1/positions/{position}/compound/prepare`.
    pub async fn position_compound_prepare(&self, position: &str) -> Result<Prepared> {
        self.post::<serde_json::Value, _>(&format!("/v1/positions/{}/compound/prepare", seg(position)), None, self.timeout)
            .await
    }

    /// `POST /v1/positions/{position}/claim-fees/prepare`.
    pub async fn position_claim_prepare(&self, position: &str) -> Result<Prepared> {
        self.post::<serde_json::Value, _>(&format!("/v1/positions/{}/claim-fees/prepare", seg(position)), None, self.timeout)
            .await
    }

    /// `POST /v1/lp/{dex}/withdraw/prepare`. `dex` is `meteora_dlmm`,
    /// `meteora_damm_v2`, `orca`, `raydium` or `ponk_clouds`; `bps_to_remove`
    /// 1 to 10000. The owner is the key's wallet.
    ///
    /// On `meteora_damm_v2` the closing 100% claims the fees first (and so
    /// carries Ponk's fee on them), a partial withdraw collects no fees, and
    /// [`Prepared::closes_position`] says whether the position is also closed.
    pub async fn lp_withdraw_prepare(
        &self,
        dex: &str,
        pool_address: &str,
        position: &str,
        bps_to_remove: u16,
    ) -> Result<Prepared> {
        let body = serde_json::json!({ "pool_address": pool_address, "position": position, "bps_to_remove": bps_to_remove });
        self.post(&format!("/v1/lp/{}/withdraw/prepare", seg(dex)), Some(&body), self.timeout)
            .await
    }

    // -- self-custody builders for any wallet, no key needed --------------
    //
    // The `/public/lp/{dex}/...` routes build an UNSIGNED transaction for the
    // `owner` named, simulated before it is returned. Nothing is signed or
    // sent. Echo any `fee_event_ids` to `position_mark_tx` with the signature
    // once it lands.

    /// `POST /public/lp/{dex}/withdraw/prepare`. Withdraw from `owner`'s
    /// position. `dex` is `meteora_damm_v2`, `orca`, `raydium` or
    /// `ponk_clouds` (Meteora DLMM withdraws through
    /// [`Client::lp_withdraw_prepare`] with a key); `bps_to_remove` 1 to 10000.
    pub async fn lp_withdraw_prepare_public(
        &self,
        dex: &str,
        pool_address: &str,
        position: &str,
        owner: &str,
        bps_to_remove: u16,
    ) -> Result<Prepared> {
        let body = serde_json::json!({
            "pool_address": pool_address,
            "position": position,
            "owner": owner,
            "bps_to_remove": bps_to_remove,
        });
        self.post(&format!("/public/lp/{}/withdraw/prepare", seg(dex)), Some(&body), self.timeout)
            .await
    }

    /// `POST /public/lp/{dex}/add-liquidity/prepare`. Add to `owner`'s
    /// existing position. `dex` is `meteora_damm_v2`, `orca` or `raydium`.
    /// Amounts are base units, sent as strings so a u64 survives JSON; on
    /// `meteora_damm_v2` they are ceilings the pool's curve takes in its own
    /// ratio.
    pub async fn lp_add_liquidity_prepare(
        &self,
        dex: &str,
        pool_address: &str,
        position: &str,
        owner: &str,
        amount_x: u64,
        amount_y: u64,
    ) -> Result<Prepared> {
        let body = serde_json::json!({
            "pool_address": pool_address,
            "position": position,
            "owner": owner,
            "amount_x": amount_x.to_string(),
            "amount_y": amount_y.to_string(),
        });
        self.post(&format!("/public/lp/{}/add-liquidity/prepare", seg(dex)), Some(&body), self.timeout)
            .await
    }

    /// `POST /public/lp/meteora_damm_v2/open/prepare`. Open a Meteora DAMM v2
    /// position for `owner`.
    ///
    /// A DAMM v2 position is a share of the pool with NO price range, so it
    /// opens with two amounts and nothing else. Amounts are base units (sent
    /// as strings), ceilings the pool's curve takes in its own ratio; both
    /// zero is refused. `max_slippage_bps` defaults to 100 on the server and
    /// is clamped to 1..1000. The transaction is already partially signed by
    /// the fresh position-NFT mint it creates; [`Prepared::position_address`]
    /// and [`Prepared::position_nft_mint`] name the new position.
    pub async fn damm_v2_open_prepare(
        &self,
        pool_address: &str,
        owner: &str,
        amount_x: u64,
        amount_y: u64,
        max_slippage_bps: Option<u16>,
    ) -> Result<Prepared> {
        let mut body = serde_json::json!({
            "pool_address": pool_address,
            "owner": owner,
            "amount_x": amount_x.to_string(),
            "amount_y": amount_y.to_string(),
        });
        if let Some(bps) = max_slippage_bps {
            body["max_slippage_bps"] = serde_json::json!(bps);
        }
        self.post("/public/lp/meteora_damm_v2/open/prepare", Some(&body), self.timeout)
            .await
    }

    /// `POST /public/lp/meteora_damm_v2/claim/prepare`. Claim a DAMM v2
    /// position's fees, with Ponk's fee on them in the same transaction:
    /// [`Prepared::fee`] is the per-leg review and `fee_event_ids` the drafts
    /// to echo to [`Client::position_mark_tx`]. Nothing to claim answers 400.
    pub async fn damm_v2_claim_prepare(&self, pool_address: &str, position: &str, owner: &str) -> Result<Prepared> {
        let body = serde_json::json!({ "pool_address": pool_address, "position": position, "owner": owner });
        self.post("/public/lp/meteora_damm_v2/claim/prepare", Some(&body), self.timeout)
            .await
    }

    /// `POST /v1/wallet/reclaim-wsol/prepare`.
    pub async fn reclaim_wsol_prepare(&self) -> Result<Prepared> {
        self.post::<serde_json::Value, _>("/v1/wallet/reclaim-wsol/prepare", None, self.timeout)
            .await
    }

    /// `GET /v1/wallet/stranded-positions`.
    pub async fn stranded_positions(&self) -> Result<StrandedPositions> {
        self.get("/v1/wallet/stranded-positions").await
    }

    /// `POST /v1/wallet/cleanup-position/prepare`.
    pub async fn cleanup_position_prepare(&self, position_address: &str) -> Result<Prepared> {
        let body = serde_json::json!({ "position_address": position_address });
        self.post("/v1/wallet/cleanup-position/prepare", Some(&body), self.timeout)
            .await
    }

    /// `POST /v1/positions/sync`. `filter` may name `pool_address`,
    /// `position_address` and `dex`.
    pub async fn positions_sync(&self, filter: &serde_json::Value) -> Result<Vec<Position>> {
        self.post("/v1/positions/sync", Some(filter), self.timeout).await
    }

    /// `POST /v1/positions/share-telegram`. Post a PnL card to the linked Telegram.
    pub async fn share_position_telegram(&self, card: &serde_json::Value) -> Result<TelegramShared> {
        self.post("/v1/positions/share-telegram", Some(card), self.timeout).await
    }

    /// `GET /v1/clouds/referrals`.
    pub async fn clouds_referrals(&self) -> Result<serde_json::Value> {
        self.get("/v1/clouds/referrals").await
    }

    /// `POST /v1/clouds/referrals/claim/prepare`. One pool's accrual, unsigned.
    pub async fn clouds_referral_claim_prepare(&self, pool: &str) -> Result<Prepared> {
        let body = serde_json::json!({ "pool": pool });
        self.post("/v1/clouds/referrals/claim/prepare", Some(&body), self.timeout)
            .await
    }

    // -- account: calls, push, cards, referral, privacy, logs (0.7.0) --------

    /// `GET /v1/calls/preview?mint=`.
    pub async fn call_preview(&self, mint: &str) -> Result<CallPreview> {
        self.get(&with_query("/v1/calls/preview", &[("mint", Some(mint.to_string()))]))
            .await
    }

    /// `POST /v1/calls`. The server reads your position from chain itself.
    pub async fn post_call(&self, mint: &str, text: &str) -> Result<Call> {
        let body = serde_json::json!({ "mint": mint, "text": text });
        self.post("/v1/calls", Some(&body), self.timeout).await
    }

    /// `DELETE /v1/calls/{id}`.
    pub async fn delete_call(&self, call_id: &str) -> Result<serde_json::Value> {
        self.delete(&format!("/v1/calls/{}", seg(call_id))).await
    }

    /// `GET /v1/push[?endpoint=]`.
    pub async fn push_settings(&self, endpoint: Option<&str>) -> Result<PushSettings> {
        self.get(&with_query("/v1/push", &[("endpoint", endpoint.map(str::to_string))]))
            .await
    }

    /// `PUT /v1/push/prefs`. Only the keys in `prefs` change.
    pub async fn set_push_prefs(&self, prefs: &serde_json::Value) -> Result<serde_json::Value> {
        self.put_json("/v1/push/prefs", prefs).await
    }

    /// `POST /v1/push/subscriptions`. A browser push service endpoint only.
    pub async fn push_subscribe(&self, endpoint: &str, p256dh: &str, auth: &str) -> Result<()> {
        let body = serde_json::json!({ "endpoint": endpoint, "keys": { "p256dh": p256dh, "auth": auth } });
        self.post::<_, Option<serde_json::Value>>("/v1/push/subscriptions", Some(&body), self.timeout)
            .await
            .map(|_| ())
    }

    /// `DELETE /v1/push/subscriptions`.
    pub async fn push_unsubscribe(&self, endpoint: &str) -> Result<()> {
        let path = "/v1/push/subscriptions";
        let body = serde_json::json!({ "endpoint": endpoint });
        let request = self.http.delete(self.url(path)).timeout(self.timeout).json(&body);
        self.send::<Option<serde_json::Value>>(request, path, &[]).await.map(|_| ())
    }

    /// `POST /v1/push/test`. Once a minute.
    pub async fn push_test(&self) -> Result<PushTest> {
        self.post::<serde_json::Value, _>("/v1/push/test", None, self.timeout).await
    }

    /// `GET /v1/pnl-card`.
    pub async fn pnl_card(&self) -> Result<CardLook> {
        self.get("/v1/pnl-card").await
    }

    /// `PUT /v1/pnl-card`.
    pub async fn set_pnl_card(&self, prefs: &serde_json::Value) -> Result<CardLook> {
        self.put_json("/v1/pnl-card", prefs).await
    }

    /// `GET /v1/referral-card`.
    pub async fn referral_card(&self) -> Result<CardLook> {
        self.get("/v1/referral-card").await
    }

    /// `PUT /v1/referral-card`.
    pub async fn set_referral_card(&self, prefs: &serde_json::Value) -> Result<CardLook> {
        self.put_json("/v1/referral-card", prefs).await
    }

    /// `POST /v1/referral/code`. Claim the code this account was referred by.
    pub async fn set_referral_code(&self, code: &str) -> Result<Referral> {
        self.post("/v1/referral/code", Some(&serde_json::json!({ "code": code })), self.timeout)
            .await
    }

    /// `POST /v1/referral/codes`.
    pub async fn add_referral_code(&self, code: &str, label: Option<&str>) -> Result<Referral> {
        let body = serde_json::json!({ "code": code, "label": label });
        self.post("/v1/referral/codes", Some(&body), self.timeout).await
    }

    /// `POST /v1/referral/creator/apply`.
    pub async fn apply_creator(&self, handle: Option<&str>, note: Option<&str>) -> Result<Referral> {
        let body = serde_json::json!({ "handle": handle, "note": note });
        self.post("/v1/referral/creator/apply", Some(&body), self.timeout).await
    }

    /// `GET /v1/leaderboard-privacy`.
    pub async fn leaderboard_privacy(&self) -> Result<LeaderboardPrivacy> {
        self.get("/v1/leaderboard-privacy").await
    }

    /// `PUT /v1/leaderboard-privacy`.
    pub async fn set_leaderboard_privacy(&self, request: &PrivacyRequest) -> Result<LeaderboardPrivacy> {
        self.put_json("/v1/leaderboard-privacy", request).await
    }

    /// `GET /v1/agents/{id}/leaderboard-visibility`.
    pub async fn agent_visibility(&self, agent_id: &str) -> Result<AgentVisibility> {
        self.get(&format!("/v1/agents/{}/leaderboard-visibility", seg(agent_id))).await
    }

    /// `PUT /v1/agents/{id}/leaderboard-visibility`.
    pub async fn set_agent_visibility(&self, agent_id: &str, hidden: bool) -> Result<AgentVisibility> {
        self.put_json(
            &format!("/v1/agents/{}/leaderboard-visibility", seg(agent_id)),
            &serde_json::json!({ "hidden": hidden }),
        )
        .await
    }

    /// `DELETE /v1/logs`. Hides the log up to now; the fund ledger is never deleted.
    pub async fn clear_logs(&self) -> Result<LogsCleared> {
        self.delete("/v1/logs").await
    }

    /// `GET /v1/recap/weekly`.
    pub async fn weekly_recap(&self) -> Result<serde_json::Value> {
        self.get("/v1/recap/weekly").await
    }

    /// `GET /v1/ponkillionaires`. Your standing in this week's race.
    pub async fn my_ponkillionaires(&self) -> Result<serde_json::Value> {
        self.get("/v1/ponkillionaires").await
    }

    /// `GET /v1/telegram`. Linking itself is done in the app.
    pub async fn telegram_status(&self) -> Result<TelegramStatus> {
        self.get("/v1/telegram").await
    }

    /// `POST /v1/presets/rank`. Every wizard preset and your saved models,
    /// replayed by the site's Plot engine on one pool's window and ranked.
    /// Six a minute per account; a repeat is cached.
    pub async fn rank_presets(&self, request: &RankRequest) -> Result<PresetRank> {
        self.post("/v1/presets/rank", Some(request), self.timeout).await
    }

    // -- people and the social graph (0.10.0) --------------------------------

    /// `GET /public/people/{key}`. A public profile.
    ///
    /// `key` is an @handle (with or without the @), any /r/ referral code, a
    /// shown wallet, or a `t-<owner_key>` tag. A private account answers with
    /// its handle, bio, X and picture only. 404 for a key that names nothing
    /// public. No key needed.
    pub async fn profile(&self, key: &str) -> Result<Profile> {
        self.get(&format!("/public/people/{}", seg(key.trim_start_matches('@'))))
            .await
    }

    /// `GET /public/people/{key}/followers`. Who follows this person. Private
    /// followers are counted, never named. Page with `next_cursor`.
    pub async fn followers(&self, key: &str, cursor: Option<&str>) -> Result<PeopleList> {
        self.get(&with_query(
            &format!("/public/people/{}/followers", seg(key.trim_start_matches('@'))),
            &[("cursor", cursor.map(str::to_string))],
        ))
        .await
    }

    /// `GET /public/people/{key}/following`. Who this person follows.
    pub async fn following(&self, key: &str, cursor: Option<&str>) -> Result<PeopleList> {
        self.get(&with_query(
            &format!("/public/people/{}/following", seg(key.trim_start_matches('@'))),
            &[("cursor", cursor.map(str::to_string))],
        ))
        .await
    }

    /// `GET /public/people/{key}/activity`. One person's agent, LP, call and
    /// (when they share them) swap activity, newest first.
    ///
    /// `kinds` is `all`, `agents`, `lp`, `swaps` or `calls`; `pnl_only` keeps
    /// closes and exits with a PnL.
    pub async fn person_activity(
        &self,
        key: &str,
        cursor: Option<&str>,
        kinds: Option<&str>,
        pnl_only: bool,
    ) -> Result<SocialFeed> {
        self.get(&with_query(
            &format!("/public/people/{}/activity", seg(key.trim_start_matches('@'))),
            &[
                ("cursor", cursor.map(str::to_string)),
                ("kinds", kinds.map(str::to_string)),
                ("pnl", pnl_only.then(|| "true".to_string())),
            ],
        ))
        .await
    }

    /// `GET /public/people/{key}/positions`. A person's open positions on
    /// Meteora, Orca and Raydium, each with its Copy action.
    pub async fn person_positions(&self, key: &str) -> Result<PersonPositions> {
        self.get(&format!("/public/people/{}/positions", seg(key.trim_start_matches('@'))))
            .await
    }

    /// `GET /public/people-search?q=&cursor=`. Ponk accounts by @handle (the @
    /// is optional), creator handle, /r/ code or display name, from the first
    /// letter, case-insensitive. Twenty a page: pass `next_cursor` back as
    /// `cursor`. No key needed.
    pub async fn people_search(&self, q: &str, cursor: Option<&str>) -> Result<PeopleSearch> {
        self.get(&with_query(
            "/public/people-search",
            &[("q", Some(q.to_string())), ("cursor", cursor.map(str::to_string))],
        ))
        .await
    }

    /// `GET /public/social/recommended?limit=`. Who to follow, with the reason
    /// for each, for a reader with no account. `limit` is 1 to 30 (default
    /// 10). [`my_recommended`] is the same for your own account.
    ///
    /// [`my_recommended`]: Client::my_recommended
    pub async fn recommended_people(&self, limit: Option<i64>) -> Result<Recommendations> {
        self.get(&with_query(
            "/public/social/recommended",
            &[("limit", limit.map(|n| n.to_string()))],
        ))
        .await
    }

    /// `GET /public/people/{key}/similar?limit=`. Traders like this one: the
    /// same pools in the last 30 days, and the people this person's followers
    /// also follow. Never the person themself.
    pub async fn similar_people(&self, key: &str, limit: Option<i64>) -> Result<Recommendations> {
        self.get(&with_query(
            &format!("/public/people/{}/similar", seg(key.trim_start_matches('@'))),
            &[("limit", limit.map(|n| n.to_string()))],
        ))
        .await
    }

    /// `GET /public/people-leaderboard`. Ponk accounts ranked over `window`
    /// (`24h`, `7d`, `30d`) by PnL, with ROI and vs HODL beside it.
    pub async fn people_board(
        &self,
        window: Option<&str>,
        sort: Option<&str>,
        limit: Option<i64>,
    ) -> Result<PeopleBoard> {
        self.get(&with_query(
            "/public/people-leaderboard",
            &[
                ("window", window.map(str::to_string)),
                ("sort", sort.map(str::to_string)),
                ("limit", limit.map(|n| n.to_string())),
            ],
        ))
        .await
    }

    /// `GET /public/social/feed`. Activity of every public account, newest
    /// first (the signed-out Following tab).
    pub async fn live_feed(
        &self,
        cursor: Option<&str>,
        kinds: Option<&str>,
        pnl_only: bool,
    ) -> Result<SocialFeed> {
        self.get(&with_query(
            "/public/social/feed",
            &[
                ("cursor", cursor.map(str::to_string)),
                ("kinds", kinds.map(str::to_string)),
                ("pnl", pnl_only.then(|| "true".to_string())),
            ],
        ))
        .await
    }

    /// `POST /v1/people/{key}/follow`. Follow a person: their agent, LP, call
    /// and shared swap activity lands in your Following feed. Idempotent. Needs
    /// a `trade` key.
    pub async fn follow_person(&self, key: &str) -> Result<FollowState> {
        self.post(
            &format!("/v1/people/{}/follow", seg(key.trim_start_matches('@'))),
            None::<&()>,
            self.timeout,
        )
        .await
    }

    /// `DELETE /v1/people/{key}/follow`. Stop following. Idempotent. Needs a
    /// `trade` key.
    pub async fn unfollow_person(&self, key: &str) -> Result<FollowState> {
        self.delete(&format!("/v1/people/{}/follow", seg(key.trim_start_matches('@'))))
            .await
    }

    /// `GET /v1/social/recommended?limit=`. Who to follow, for your account:
    /// also ranked by who the people you follow follow, leaving out who you
    /// already follow or dismissed.
    pub async fn my_recommended(&self, limit: Option<i64>) -> Result<Recommendations> {
        self.get(&with_query(
            "/v1/social/recommended",
            &[("limit", limit.map(|n| n.to_string()))],
        ))
        .await
    }

    /// `POST /v1/social/recommended/{key}/dismiss`. Never suggest this person
    /// again. Answers `{"key", "dismissed": true}`. Needs a `trade` key.
    pub async fn dismiss_recommended(&self, key: &str) -> Result<serde_json::Value> {
        self.post(
            &format!("/v1/social/recommended/{}/dismiss", seg(key.trim_start_matches('@'))),
            None::<&()>,
            self.timeout,
        )
        .await
    }

    /// `GET /v1/me/following`. Everyone you follow, private accounts included,
    /// plus the shared agents you follow one by one.
    pub async fn my_following(&self) -> Result<MyFollowing> {
        self.get("/v1/me/following").await
    }

    /// `GET /v1/social/feed`. Your Following feed, newest first. `kinds` is
    /// `all`, `agents`, `lp`, `swaps` or `calls`; `pnl_only` keeps closes and
    /// exits with a PnL. Page with `next_cursor`.
    pub async fn following_feed(
        &self,
        cursor: Option<&str>,
        kinds: Option<&str>,
        pnl_only: bool,
    ) -> Result<SocialFeed> {
        self.get(&with_query(
            "/v1/social/feed",
            &[
                ("cursor", cursor.map(str::to_string)),
                ("kinds", kinds.map(str::to_string)),
                ("pnl", pnl_only.then(|| "true".to_string())),
            ],
        ))
        .await
    }

    /// Stream social activity over Server-Sent Events, indefinitely.
    ///
    /// `GET /public/social/stream` (`public = true`, no key) or
    /// `GET /me/social/stream` (`public = false`, your following graph, needs
    /// a key). The returned [`SocialStream`] yields one [`StreamEvent`] per
    /// frame from [`SocialStream::next_event`]: one `ready` on connect, a `row`
    /// for each event as it happens, and a `resync` when the server wants the
    /// client to reload from [`live_feed`]. The 15s keep-alive comment is
    /// skipped.
    ///
    /// The API exposes no WebSocket: Server-Sent Events are the transport, and
    /// this is a one-way read. The request carries no timeout, so it stays open
    /// until the server closes it or the connection drops; reconnection is the
    /// caller's, and after a `resync` read [`live_feed`] to fill the gap before
    /// streaming again.
    ///
    /// [`live_feed`]: Client::live_feed
    pub async fn stream_social(&self, public: bool) -> Result<SocialStream> {
        let path = if public {
            "/public/social/stream"
        } else {
            "/me/social/stream"
        };
        let mut request = self
            .http
            .get(self.url(path))
            .header(reqwest::header::ACCEPT, "text/event-stream");
        if public {
            // The public stream is anonymous: no key is sent even when the
            // client holds one.
        } else {
            match &self.api_key {
                Some(key) => request = request.bearer_auth(key),
                None => {
                    return Err(Error::Api(ApiError {
                        status: 401,
                        code: "no_api_key".to_string(),
                        message: "stream_social(public = false) needs an API key: construct the \
                                  client with one. Nothing was sent."
                            .to_string(),
                        request_id: None,
                        body: None,
                    }))
                }
            }
        }
        let response = request.send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await?;
            return Err(Error::Api(ApiError::from_body(status.as_u16(), Some(body))));
        }
        Ok(SocialStream::new(response))
    }

    // -- calls, X account cards and wallet PnL (0.10.0) ----------------------

    /// `GET /public/calls/{id}`. One call with its outcome so far. Public.
    pub async fn call(&self, call_id: &str) -> Result<serde_json::Value> {
        self.get(&format!("/public/calls/{}", seg(call_id))).await
    }

    /// `GET /public/x/account/{handle}`. One X account's caller card: calls,
    /// multiples, hit rate, 2x rate, first calls, rank and recent calls over
    /// `days` (default 30). Public.
    pub async fn x_account(&self, handle: &str, days: Option<i64>) -> Result<XAccountCard> {
        self.get(&with_query(
            &format!("/public/x/account/{}", seg(handle.trim_start_matches('@'))),
            &[("days", days.map(|n| n.to_string()))],
        ))
        .await
    }

    /// `GET /public/calls/top`. The best calls of the last `days` (default 7,
    /// up to 90), ranked by their multiple since the call. `source` is `ponk`
    /// (default), `x` or `all`. Public.
    pub async fn top_calls(
        &self,
        days: Option<i64>,
        limit: Option<i64>,
        source: Option<&str>,
    ) -> Result<TopCalls> {
        self.get(&with_query(
            "/public/calls/top",
            &[
                ("days", days.map(|n| n.to_string())),
                ("limit", limit.map(|n| n.to_string())),
                ("source", source.map(str::to_string)),
            ],
        ))
        .await
    }

    /// `GET /public/wallets/{address}/pnl`. Trading PnL per token for any
    /// wallet, from its swaps. No key needed.
    pub async fn wallet_pnl(&self, address: &str) -> Result<WalletPnl> {
        self.get(&format!("/public/wallets/{}/pnl", seg(address))).await
    }

    /// `GET /public/wallets/positions`. Open LP positions of the curated
    /// wallets on Meteora, Orca and Raydium.
    pub async fn tracked_lps(&self) -> Result<TrackedLps> {
        self.get("/public/wallets/positions").await
    }

    // -- pool market lists (0.13.0): no key required ------------------------

    /// `GET <web>/pools/board?view=&q=`. The Meteora DLMM pool board the app's
    /// own list shows.
    ///
    /// `view` picks a different UPSTREAM QUERY, not a re-sort of one page:
    /// `hot` (most fees in 24h, TVL over $50K; the default), `new` (created in
    /// the last 24h, most fees first, TVL over $10K), `volume` and `tvl`.
    /// `search` filters by symbol, name or pool/mint address. Served from the
    /// web app origin ([`Client::web_url`]), no key needed.
    ///
    /// Meteora only: for one board across every live venue, use
    /// [`Client::opportunities`].
    pub async fn hot_pools(&self, view: Option<&str>, search: Option<&str>) -> Result<PoolBoard> {
        self.get_web(&with_query(
            "/pools/board",
            &[
                ("view", view.map(str::to_string)),
                ("q", search.map(str::to_string)),
            ],
        ))
        .await
    }

    /// `GET <web>/discover/feed`. The Discover board: LP opportunities across
    /// Meteora DLMM, Meteora DAMM v2, Orca Whirlpools and Raydium CLMM in one
    /// read, each row with both token legs' risk reads and per-window volume
    /// and fees.
    ///
    /// Check `ok`: a source that did not answer leaves its rows out and the
    /// flag false, and nothing is substituted. Served from the web app origin
    /// ([`Client::web_url`]), no key needed.
    pub async fn opportunities(&self) -> Result<OppBoard> {
        self.get_web("/discover/feed").await
    }

    /// `GET /dex/dlmm/pools?page&pageSize&sortBy&q`. The venue's paginated
    /// Meteora DLMM pool list, with live metrics per row.
    ///
    /// `sort_by` is one of `tvl`, `volume_24h`, `fees_24h`,
    /// `fee_tvl_ratio_24h`, `apr`, `apy`, `bin_step`, `pool_created_at`,
    /// descending unless `:asc` is appended (the server default is
    /// `tvl:desc`). `page` is 1-based; `page_size` 1 to 100, default 50.
    /// `search` matches symbol, name or address. No key needed.
    ///
    /// Mind the units on [`DlmmPool::apr`]: percent per DAY, not annual.
    pub async fn dlmm_pools(
        &self,
        page: Option<u32>,
        page_size: Option<u32>,
        sort_by: Option<&str>,
        search: Option<&str>,
    ) -> Result<DlmmPoolPage> {
        let path = with_query(
            "/dex/dlmm/pools",
            &[
                ("page", page.map(|n| n.to_string())),
                ("pageSize", page_size.map(|n| n.to_string())),
                ("sortBy", sort_by.map(str::to_string)),
                ("q", search.map(str::to_string)),
            ],
        );
        let envelope: DexData<DlmmPoolPage> = self.get(&path).await?;
        Ok(envelope.data)
    }

    // -- EVM chains: Ethereum, Arbitrum, Base, BNB Chain, Robinhood Chain (0.15.0)

    /// `GET /public/evm/chains`. Every chain, its gas coin and its venues.
    /// No key needed.
    pub async fn evm_chains(&self) -> Result<EvmChains> {
        self.get("/public/evm/chains").await
    }

    /// `GET /v1/evm/chains`. The same body as [`Client::evm_chains`], on the
    /// API key's budget instead of the per-IP limit. Every `_keyed` EVM read
    /// is that same trade.
    pub async fn evm_chains_keyed(&self) -> Result<EvmChains> {
        self.get("/v1/evm/chains").await
    }

    /// `GET /public/evm/{chain}/board?venue=`. One chain's pool board.
    ///
    /// `chain` is `ethereum`, `arbitrum`, `base`, `bsc` or `robinhood`.
    /// `venue` keeps one venue's pools (an id from
    /// [`crate::evm::EVM_VENUES`] that runs on that chain; any other answers
    /// 400). Each [`EvmPool`] carries its CAIP id, fee tier, whether the fee
    /// is dynamic, TVL, volume and LP fees per window, the farm and
    /// [`EvmPool::flags`]. The board is a snapshot the server refreshes by
    /// itself, so the call reads no chain; [`EvmBoard::ready`] is false, with
    /// no pools, until its first read has finished. The rows come unsorted.
    pub async fn evm_pools(&self, chain: &str, venue: Option<&str>) -> Result<EvmBoard> {
        self.get(&evm_board_path("/public/evm", chain, venue)).await
    }

    /// `GET /v1/evm/{chain}/board?venue=`. [`Client::evm_pools`] on the key.
    pub async fn evm_pools_keyed(&self, chain: &str, venue: Option<&str>) -> Result<EvmBoard> {
        self.get(&evm_board_path("/v1/evm", chain, venue)).await
    }

    /// `GET /public/evm/pools/{id}`. One pool by its CAIP id:
    /// `eip155:<chainId>:0x<address>`, or the four part form for a Uniswap v4
    /// or PancakeSwap Infinity CL pool ([`crate::evm::pool_id`],
    /// [`crate::evm::v4_pool_id`]). An id that is not one answers 400, an
    /// unknown pool 404.
    pub async fn evm_pool(&self, pool_id: &str) -> Result<EvmPool> {
        self.get(&format!("/public/evm/pools/{}", seg(pool_id))).await
    }

    /// `GET /v1/evm/pools/{id}`. [`Client::evm_pool`] on the key.
    pub async fn evm_pool_keyed(&self, pool_id: &str) -> Result<EvmPool> {
        self.get(&format!("/v1/evm/pools/{}", seg(pool_id))).await
    }

    /// `GET /public/evm/tokens/{id}`. One token by its CAIP id.
    ///
    /// The zero address is the chain's gas coin
    /// ([`crate::evm::native_token_id`]), whose `price_usd` is the ETH or BNB
    /// price. Use it to turn the base unit amounts of
    /// [`Withdrawal::received`] into whole tokens.
    pub async fn evm_token(&self, token_id: &str) -> Result<EvmToken> {
        self.get(&format!("/public/evm/tokens/{}", seg(token_id))).await
    }

    /// `GET /v1/evm/tokens/{id}`. [`Client::evm_token`] on the key.
    pub async fn evm_token_keyed(&self, token_id: &str) -> Result<EvmToken> {
        self.get(&format!("/v1/evm/tokens/{}", seg(token_id))).await
    }

    /// `GET /v1/evm/{chain}/open-cost?dex=&pool=&stake=`. What the first open
    /// costs, with the rates an agent YOU create pays.
    ///
    /// The measured gas of the first open at the live fee, the reserve kept
    /// back for the exit, the least the agent wallet must hold, and ponk's
    /// two fee rates as an API key pays them: 1.5% of the first open, once,
    /// and 15% of fees earned. `stake` defaults to true on the server where
    /// the venue has a farm, because a staked open costs more gas. A venue
    /// that does not run on `chain` answers 422. Needs a key; see
    /// [`Client::evm_open_cost_public`] for the keyless quote.
    pub async fn evm_open_cost(
        &self,
        chain: &str,
        dex: &str,
        pool: Option<&str>,
        stake: Option<bool>,
    ) -> Result<EvmOpenCost> {
        self.get(&evm_open_cost_path("/v1/evm", chain, dex, pool, stake)).await
    }

    /// `GET /public/evm/{chain}/open-cost`. The same quote with no key. Its
    /// two fee rates are those of an agent created on ponk.exchange (1% and
    /// 8%), not of one this crate creates.
    pub async fn evm_open_cost_public(
        &self,
        chain: &str,
        dex: &str,
        pool: Option<&str>,
        stake: Option<bool>,
    ) -> Result<EvmOpenCost> {
        self.get(&evm_open_cost_path("/public/evm", chain, dex, pool, stake)).await
    }

    /// `POST /v1/agents`. Create an agent on an EVM chain.
    ///
    /// Build the body with [`NewAgent::evm_range`] (a range venue: the range
    /// as `down_pct` and `up_pct`, positive fractions) or
    /// [`NewAgent::evm_full_range`] (`uniswap_v2`, `pancakeswap_v2`,
    /// `aerodrome_classic`: compound and hold), then
    /// [`NewAgent::stake`], [`NewAgent::withdrawal_address`] and
    /// [`NewAgent::dry_run`] as needed. The strategy ids are the Solana ones;
    /// Degen Mode (`entry_agent`) is not offered on an EVM venue, and the
    /// agent opens its own position, so `position_address` is refused here
    /// before anything is sent.
    ///
    /// An agent created here pays the API rates for its whole life: 1.5% of
    /// its first open, once, and 15% of the fees it earns
    /// ([`Agent::activation_fee_bps`], [`Agent::managed_fee_bps`]).
    ///
    /// NOTHING IS FUNDED OR SIGNED BY THIS CALL. The agent acts only after
    /// you sign its mandate with your own wallets on ponk.exchange (two
    /// signatures: the Solana login wallet, then the EVM wallet that will
    /// receive withdrawals; [`crate::evm::fund_link`] is the page) and send
    /// the chain's gas coin to its address
    /// ([`Client::create_agent_wallet`]). A refusal is a 422 with a plain
    /// sentence: the venue is not on that chain, the pool is not a pool of
    /// that venue, a v4 hook an agent cannot use.
    pub async fn create_evm_agent(&self, agent: &NewAgent) -> Result<Agent> {
        match agent.chain.as_deref() {
            Some(chain) if crate::evm::is_evm_chain(chain) => {}
            other => {
                return Err(Error::InvalidArgument(format!(
                    "chain must be one of ethereum, arbitrum, base, bsc, robinhood, got {}. \
                     For a Solana agent use create_agent.",
                    other.map(|c| format!("'{c}'")).unwrap_or_else(|| "none".into())
                )))
            }
        }
        if agent.position_address.is_some() {
            return Err(Error::InvalidArgument(
                "an agent on an EVM chain opens its own position, so leave position_address unset".into(),
            ));
        }
        if agent.pool_address.is_none() {
            return Err(Error::InvalidArgument(
                "an agent on an EVM chain needs pool_address, the pool's CAIP id".into(),
            ));
        }
        self.create_agent(agent).await
    }

    /// `PATCH /v1/agents/{id}/config` with `{"evm": {"stake": stake}}`.
    ///
    /// Turn staking for farm rewards on or off for an EVM agent on
    /// `pancakeswap_v3`, `aerodrome_slipstream` or `aerodrome_classic`.
    /// Applies from the next open or recenter and moves nothing now. Any
    /// other venue answers 422.
    pub async fn set_stake_rewards(&self, agent_id: &str, stake: bool) -> Result<Agent> {
        self.update_config(agent_id, &crate::evm::stake_config(stake)).await
    }

    /// `POST /copilot/evm/plans`. Plan one sentence that names an EVM chain,
    /// for example `"$200 in the top 5 bnb pools"`.
    ///
    /// The grammar is closed and deterministic, with no language model:
    /// dollars (`$200`, `200 usd`, `$1.5k`), a pool count (`top 5`,
    /// `3 pools`, 1 to 10 per chain), one or more chains (`bnb`, `arbitrum`,
    /// `base`, `ethereum`, `robinhood`, `solana`), optional venues, risk
    /// (`careful`, `balanced`, `aggressive`) and a range (`8%`). One amount
    /// is the whole budget, split evenly across the chains named, or one
    /// amount goes in front of each chain (`$60 in bnb and $40 in solana`).
    /// Amounts are DOLLARS ONLY.
    ///
    /// Answers at once and creates nothing. A sentence it cannot plan comes
    /// back with `state` `refused` and `refusal`, not as an `Err`. Plans last
    /// 15 minutes. Needs an API key; `fees` on the answer is the fee sentence
    /// for that caller.
    pub async fn copilot_evm_plan(&self, text: &str) -> Result<EvmPlan> {
        self.post("/copilot/evm/plans", Some(&serde_json::json!({ "text": text })), self.timeout)
            .await
    }

    /// `GET /copilot/evm/plans/{id}`. The same plan, readable only by its maker.
    pub async fn copilot_evm_get_plan(&self, plan_id: &str) -> Result<EvmPlan> {
        self.get(&format!("/copilot/evm/plans/{}", seg(plan_id))).await
    }

    /// `POST /copilot/evm/plans/{id}/start`. Create one agent per leg.
    ///
    /// MOVES NO MONEY. The plan comes back with `state` `started`, `agents`
    /// (each with its `agent_id` and the amount of the chain's coin to send
    /// it, or `created` false with the sentence that says why), `sign_urls`,
    /// the ponk.exchange pages where your own wallets sign each agent's
    /// mandate and fund it, and `fleet_id`: the agents form a copilot fleet,
    /// so [`Client::copilot_fleet`], [`Client::copilot_stop_fleet`] and
    /// [`Client::copilot_tighten_stop`] act on them. A second start returns
    /// the same answer and creates nothing. Needs a `trade` key. Takes the
    /// fund timeout.
    pub async fn copilot_evm_start(&self, plan_id: &str) -> Result<EvmPlan> {
        self.post(
            &format!("/copilot/evm/plans/{}/start", seg(plan_id)),
            None::<&()>,
            self.fund_timeout,
        )
        .await
    }

    /// `GET /agents/evm/fleet`. Every EVM agent you own and what each needs.
    ///
    /// Each row has the agent's own address, its mandate status and expiry,
    /// and `needs`. `sign_urls` are the pages that fund and authorize the
    /// ones still running. [`Client::fleet`] lists the same agents beside the
    /// Solana ones, with wallet and performance. Needs an API key.
    pub async fn evm_fleet(&self) -> Result<EvmFleet> {
        self.get("/agents/evm/fleet").await
    }

    // -- the copilot (0.8.0) -------------------------------------------------

    /// `POST /copilot/parse`. Read one sentence with ponk's closed grammar.
    ///
    /// Pure on the server: reads nothing, writes nothing. `ok` false carries
    /// `refusal`. No language model runs anywhere. Needs an API key.
    pub async fn copilot_parse(&self, text: &str) -> Result<CopilotParse> {
        self.post("/copilot/parse", Some(&serde_json::json!({ "text": text })), self.timeout)
            .await
    }

    /// `POST /copilot/plans`. Start building a plan from one sentence.
    ///
    /// Returns at once with `state` `building`; poll [`copilot_get_plan`] (or
    /// use [`copilot_wait`]). Creates nothing and moves nothing. Plans last 15
    /// minutes.
    ///
    /// [`copilot_get_plan`]: Client::copilot_get_plan
    /// [`copilot_wait`]: Client::copilot_wait
    pub async fn copilot_plan(&self, text: &str) -> Result<CopilotPlan> {
        self.post("/copilot/plans", Some(&serde_json::json!({ "text": text })), self.timeout)
            .await
    }

    /// `GET /copilot/plans/{id}`. The plan, its progress and its replay.
    pub async fn copilot_get_plan(&self, plan_id: &str) -> Result<CopilotPlan> {
        self.get(&format!("/copilot/plans/{}", seg(plan_id))).await
    }

    /// `POST /copilot/plans/{id}/replay`. Replay every leg over the last 30
    /// days. The replay runs on the server; poll the plan until `replay.state`
    /// is `done`. It is one past window, not a forecast.
    pub async fn copilot_replay(&self, plan_id: &str) -> Result<CopilotPlan> {
        self.post(
            &format!("/copilot/plans/{}/replay", seg(plan_id)),
            None::<&()>,
            self.timeout,
        )
        .await
    }

    /// Poll a plan until it is ready (and replayed, with `replay = true`) or
    /// refused.
    ///
    /// Starts the replay once the plan is ready when it has not run. Returns
    /// the last read when `timeout` passes, so check `state` and `replay.state`
    /// rather than assuming it finished. `sleep` is how this waits between
    /// polls: pass your runtime's timer, for example
    /// `|d| tokio::time::sleep(d)`, which keeps this crate free of a runtime
    /// of its own.
    pub async fn copilot_wait<S, Fut>(
        &self,
        plan_id: &str,
        replay: bool,
        timeout: Duration,
        mut sleep: S,
    ) -> Result<CopilotPlan>
    where
        S: FnMut(Duration) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let deadline = std::time::Instant::now() + timeout;
        let mut asked = false;
        loop {
            let plan = self.copilot_get_plan(plan_id).await?;
            let rstate = plan.replay.as_ref().and_then(|r| r.state.clone());
            if plan.state.as_deref() == Some("ready")
                && replay
                && rstate.as_deref() == Some("idle")
                && !asked
            {
                asked = true;
                self.copilot_replay(plan_id).await?;
                continue;
            }
            let settled = matches!(plan.state.as_deref(), Some("refused") | Some("expired"))
                || (plan.state.as_deref() == Some("ready")
                    && (!replay || rstate.as_deref() == Some("done")));
            if settled || std::time::Instant::now() >= deadline {
                return Ok(plan);
            }
            let ms = plan.poll_after_ms.unwrap_or(2000).max(1000) as u64;
            sleep(Duration::from_millis(ms)).await;
        }
    }

    /// `POST /copilot/plans/{id}/start`. Create the agents and prepare the
    /// funding.
    ///
    /// The agents are created in dry run, each with its own empty managed
    /// wallet, and the funding comes back as UNSIGNED transfers from your
    /// wallet plus `sign_url`, the ponk.exchange page where your wallet signs
    /// them and then one custody mandate per agent. NOTHING MOVES until your
    /// wallet signs; this client never signs. A plan starts at most once. Takes
    /// the fund timeout.
    pub async fn copilot_start(&self, plan_id: &str) -> Result<CopilotStart> {
        self.post(
            &format!("/copilot/plans/{}/start", seg(plan_id)),
            None::<&()>,
            self.fund_timeout,
        )
        .await
    }

    /// `GET /copilot/fleets`. Every fleet this wallet started from a plan.
    pub async fn copilot_fleets(&self) -> Result<Vec<CopilotFleetSummary>> {
        self.get("/copilot/fleets").await
    }

    /// `GET /copilot/fleets/{id}`. The fleet live: totals, the stop now, every
    /// agent. USD figures the server could not price are `None`, never zero.
    pub async fn copilot_fleet(&self, fleet_id: &str) -> Result<CopilotFleet> {
        self.get(&format!("/copilot/fleets/{}", seg(fleet_id))).await
    }

    /// `POST /copilot/fleets/{id}/stop`. Close every agent in this fleet.
    ///
    /// Each agent is stopped, its position closed and its own wallet swept to
    /// your linked wallet through the managed exit under the mandate you
    /// signed; the destination cannot be named. On a fleet that is `stopping`
    /// with failed exits, calling again retries ONLY those exits; on a stopped
    /// fleet it answers 409. Takes the fund timeout.
    pub async fn copilot_stop_fleet(&self, fleet_id: &str) -> Result<FleetBatch> {
        self.post(
            &format!("/copilot/fleets/{}/stop", seg(fleet_id)),
            None::<&()>,
            self.fund_timeout,
        )
        .await
    }

    /// `POST /copilot/fleets/{id}/stop-rule`. Make the fleet stop tighter.
    ///
    /// `drawdown_pct` is percent below what went in, 1 to 50. Only a tighter
    /// (smaller) stop than the current one is accepted, and only while the
    /// fleet is active: a looser one answers 422.
    pub async fn copilot_tighten_stop(
        &self,
        fleet_id: &str,
        drawdown_pct: f64,
    ) -> Result<CopilotFleetSummary> {
        self.post(
            &format!("/copilot/fleets/{}/stop-rule", seg(fleet_id)),
            Some(&serde_json::json!({ "drawdown_pct": drawdown_pct })),
            self.timeout,
        )
        .await
    }

    // -- agents: exit with to-SOL, mandates, copy (0.9.0/0.10.0) -------------

    /// `POST /v1/agents/{id}/exit`. The full stop, with the exit-to-SOL choice.
    ///
    /// Like [`exit`], but `to_sol` sets whether the closed position's leftover
    /// tokens are swapped to SOL before the sweep (ponk fee 0.2% of the swapped
    /// amount). `Some(false)` sends them as they are, no swap, no fee; `None`
    /// lets the agent's own `exit_to_sol` setting decide (default true). Read
    /// the result like [`exit`]: a mandate denial is a 200 with a failed status
    /// and no signature, not an `Err`. Takes the fund timeout.
    ///
    /// ON AN EVM AGENT the exit stops the agent, unstakes and removes the
    /// whole position, unwraps, and sweeps every token and the gas coin to
    /// the mandate's proven withdrawal address on the same chain; `to_sol` is
    /// ignored. The answer carries the real amounts this call sent
    /// (`amount_wei`, `received`, `tx_hashes`) and `status`. An exit that
    /// does not finish in the call answers 422 with the sentence, or `status`
    /// `incomplete`, and the agent keeps running the exit by itself until
    /// everything has left its wallet; calling again is safe and never pays
    /// twice. It works under an expired mandate too.
    ///
    /// [`exit`]: Client::exit
    pub async fn exit_agent(&self, agent_id: &str, to_sol: Option<bool>) -> Result<Withdrawal> {
        let body = match to_sol {
            Some(b) => serde_json::json!({ "to_sol": b }),
            None => serde_json::json!({}),
        };
        self.post(
            &format!("/v1/agents/{}/exit", seg(agent_id)),
            Some(&body),
            self.fund_timeout,
        )
        .await
    }

    /// `GET /v1/agents/mandates`. Every autonomous agent's mandate, the ones
    /// that need renewing first. Reads only: a mandate is never extended
    /// without your wallet's signature.
    pub async fn mandates(&self) -> Result<Mandates> {
        self.get("/v1/agents/mandates").await
    }

    /// `GET /v1/agents/{id}/exit-to-sol`. Whether this agent's exits swap
    /// leftover tokens to SOL (default true).
    pub async fn get_exit_to_sol(&self, agent_id: &str) -> Result<ExitToSol> {
        self.get(&format!("/v1/agents/{}/exit-to-sol", seg(agent_id))).await
    }

    /// `POST /v1/agents/{id}/exit-to-sol`. Turn the exit-to-SOL swap on or off.
    ///
    /// Automatic exits (take-profit, stop-loss, price stop, drawdown kill,
    /// bleed exit, custom kill rule, Degen Mode) honour it; a manual exit that
    /// does not pass `to_sol` uses it. Moves no funds. Needs a `trade` key.
    pub async fn set_exit_to_sol(&self, agent_id: &str, exit_to_sol: bool) -> Result<ExitToSol> {
        self.post(
            &format!("/v1/agents/{}/exit-to-sol", seg(agent_id)),
            Some(&serde_json::json!({ "exit_to_sol": exit_to_sol })),
            self.timeout,
        )
        .await
    }

    /// Copy a shared agent: [`create_agent`] from its public `copy` block.
    ///
    /// Reads the agent with [`public_agent`] first, then creates one that is
    /// yours, on your wallet, with the same venue, pool, strategy and config;
    /// nothing of the owner's is shared. `config_overrides`, when set, is
    /// merged over the copied config (for example a narrower range). `dry_run`
    /// is required, as in [`create_agent`]. 422 when the agent carries no copy
    /// block to start from.
    ///
    /// [`create_agent`]: Client::create_agent
    /// [`public_agent`]: Client::public_agent
    pub async fn copy_agent(
        &self,
        slug: &str,
        name: &str,
        wallet_address: &str,
        dry_run: bool,
        config_overrides: Option<&serde_json::Value>,
    ) -> Result<Agent> {
        let source = self.public_agent(slug).await?;
        let mut new_agent = source.copy.to_new_agent(name, wallet_address).ok_or_else(|| {
            Error::Api(ApiError {
                status: 422,
                code: "no_copy_block".to_string(),
                message: "that agent carries no copy block to start from".to_string(),
                request_id: None,
                body: None,
            })
        })?;
        if let Some(overrides) = config_overrides.and_then(|v| v.as_object()) {
            if let Some(config) = new_agent.config.as_object_mut() {
                for (k, v) in overrides {
                    config.insert(k.clone(), v.clone());
                }
            }
        }
        new_agent.dry_run = Some(dry_run);
        self.create_agent(&new_agent).await
    }

    // -- orders: sign-in and place in one call (0.7.0) -----------------------

    /// Challenge, sign and verify in one call.
    ///
    /// `sign_message` takes Jupiter's challenge text and returns the wallet's
    /// base58 ed25519 signature of its UTF-8 bytes. Your code signs; this
    /// client never sees a key. Check the text says it authenticates the wallet
    /// you expect before signing.
    pub async fn orders_sign_in<F>(&self, sign_message: F) -> Result<JupiterSession>
    where
        F: FnOnce(&str) -> String,
    {
        let challenge = self.orders_challenge("message").await?;
        let text = challenge.challenge.ok_or_else(|| {
            Error::Api(ApiError {
                status: 422,
                code: "no_challenge".to_string(),
                message: "Jupiter sent no challenge text".to_string(),
                request_id: None,
                body: None,
            })
        })?;
        self.orders_verify_message(&sign_message(&text)).await
    }

    /// Craft the deposit, have your code sign it, create the order.
    ///
    /// `family` is `price` or `dca`. `body` is the create body WITHOUT the
    /// deposit fields (see [`create_price_order`] / [`create_dca_order`]);
    /// `inputAmount`, `inputMint`, `outputMint` and, for a price order,
    /// `orderType` are read from it for the craft. `sign_transaction` takes the
    /// UNSIGNED base64 deposit and returns it signed, base64. The crafted
    /// amount and mint are checked against `body` before your signer is called,
    /// so a mismatch refuses with code `deposit_mismatch` and nothing is
    /// signed.
    ///
    /// [`create_price_order`]: Client::create_price_order
    /// [`create_dca_order`]: Client::create_dca_order
    pub async fn place_order<F>(
        &self,
        jupiter_session: &str,
        family: &str,
        body: &serde_json::Value,
        sign_transaction: F,
    ) -> Result<OrderCreated>
    where
        F: FnOnce(&str) -> String,
    {
        if family != "price" && family != "dca" {
            return Err(Error::Api(ApiError {
                status: 422,
                code: "bad_order_family".to_string(),
                message: "family must be 'price' or 'dca'".to_string(),
                request_id: None,
                body: None,
            }));
        }
        let input_mint = json_str(body, "inputMint");
        let output_mint = json_str(body, "outputMint");
        let input_amount = json_str(body, "inputAmount");
        let amount: u64 = input_amount.parse().map_err(|_| {
            Error::Api(ApiError {
                status: 422,
                code: "bad_order_amount".to_string(),
                message: "inputAmount must be base units as a string".to_string(),
                request_id: None,
                body: None,
            })
        })?;
        let sub = if family == "price" {
            match body.get("orderType") {
                Some(serde_json::Value::String(s)) => Some(s.clone()),
                _ => None,
            }
        } else {
            None
        };
        let craft = self
            .craft_order_deposit(
                jupiter_session,
                &input_mint,
                &output_mint,
                amount,
                family,
                sub.as_deref(),
            )
            .await?;
        if craft.amount.as_deref() != Some(input_amount.as_str())
            || craft.mint.as_deref() != Some(input_mint.as_str())
        {
            return Err(Error::Api(ApiError {
                status: 422,
                code: "deposit_mismatch".to_string(),
                message: "Jupiter's deposit does not match this order. Nothing was signed."
                    .to_string(),
                request_id: None,
                body: None,
            }));
        }
        let signed = sign_transaction(craft.transaction.as_deref().unwrap_or(""));
        let mut full = body.clone();
        if let Some(obj) = full.as_object_mut() {
            if let Some(id) = craft.request_id {
                obj.insert("depositRequestId".to_string(), serde_json::Value::String(id));
            }
            obj.insert("depositSignedTx".to_string(), serde_json::Value::String(signed));
        }
        if family == "price" {
            self.create_price_order(jupiter_session, &full).await
        } else {
            self.create_dca_order(jupiter_session, &full).await
        }
    }

    // -- privacy (0.10.0) ----------------------------------------------------

    /// `GET /v1/me/privacy`. Since 2026-10-29 every Ponk account is public, so
    /// this answers the public state. Kept for older code.
    pub async fn privacy(&self) -> Result<Privacy> {
        self.get("/v1/me/privacy").await
    }

    /// `PUT /v1/me/privacy`. Every account is public since 2026-10-29: the
    /// server accepts this body, ignores it and answers the public state. Kept
    /// so older code does not break. Needs a `trade` key.
    pub async fn set_privacy(
        &self,
        is_public: bool,
        show_wallet: bool,
        show_usd: bool,
        share_swaps: bool,
        rank_anon: bool,
    ) -> Result<Privacy> {
        let body = serde_json::json!({
            "is_public": is_public,
            "show_wallet": show_wallet,
            "show_usd": show_usd,
            "share_swaps": share_swaps,
            "rank_anon": rank_anon,
        });
        self.put_json("/v1/me/privacy", &body).await
    }

    /// `POST /v1/me/privacy/ack`. The prompt it answered no longer exists; the
    /// server accepts and ignores it and answers the public state. Needs a
    /// `trade` key.
    pub async fn answer_privacy_prompt(&self, keep_public: bool) -> Result<Privacy> {
        self.post(
            "/v1/me/privacy/ack",
            Some(&serde_json::json!({ "keep_public": keep_public })),
            self.timeout,
        )
        .await
    }

    // -- transport ---------------------------------------------------------

    async fn put_json<B: Serialize + ?Sized, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T> {
        let request = self.http.put(self.url(path)).timeout(self.timeout).json(body);
        self.send(request, path, &[]).await
    }

    /// A POST that carries the caller's Jupiter session in
    /// `X-Jupiter-Session`. The token is used for this one request only.
    async fn post_jupiter<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        jupiter_session: &str,
        body: Option<&B>,
        timeout: Duration,
    ) -> Result<T> {
        let request = self
            .http
            .post(self.url(path))
            .timeout(timeout)
            .header(JUPITER_SESSION_HEADER, jupiter_session);
        let request = match body {
            Some(b) => request.json(b),
            None => request.json(&serde_json::json!({})),
        };
        self.send(request, path, &[]).await
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let request = self.http.get(self.url(path)).timeout(self.timeout);
        self.send(request, path, &[]).await
    }

    /// A GET against the web app origin rather than the API. The key is never
    /// attached: these boards are public, and when the two origins differ the
    /// key must not cross to the second one.
    async fn get_web<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let request = self
            .http
            .get(format!("{}{}", self.web_url, path))
            .timeout(self.timeout);
        self.send_with_auth(request, path, &[], false).await
    }

    async fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let request = self.http.delete(self.url(path)).timeout(self.timeout);
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
        self.send_with_auth(request, path, body_statuses, true).await
    }

    async fn send_with_auth<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        path: &str,
        body_statuses: &[u16],
        with_key: bool,
    ) -> Result<T> {
        let mut request = request.header(reqwest::header::ACCEPT, "application/json");
        if with_key {
            if let Some(key) = &self.api_key {
                request = request.bearer_auth(key);
            }
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
        // A 204 has no body. Decode it as JSON null, so a `()` or `Option`
        // result reads it as "nothing", not as a decode failure.
        let body = if body.trim().is_empty() { "null".to_string() } else { body };
        serde_json::from_str(&body).map_err(|e| Error::Decode {
            path: path.to_string(),
            message: e.to_string(),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

/// The `/exits` body. An absent block is omitted rather than sent as null,
/// because the server reads `null` there as "no patch" but a reader of the
/// wire would not.
fn exits_body(
    tp_sl: Option<&serde_json::Value>,
    automation: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut body = serde_json::Map::new();
    if let Some(t) = tp_sl {
        body.insert("tp_sl".into(), t.clone());
    }
    if let Some(a) = automation {
        body.insert("automation".into(), a.clone());
    }
    serde_json::Value::Object(body)
}

/// The `/auto-compound` body. Unset floors are omitted, never sent as 0.
fn auto_compound_body(
    auto_compound: bool,
    min_claimable_base_units: Option<u64>,
    min_claimable_usd: Option<f64>,
) -> serde_json::Value {
    let mut body = serde_json::Map::new();
    body.insert("auto_compound".into(), auto_compound.into());
    if let Some(n) = min_claimable_base_units {
        body.insert("compound_min_claimable_base_units".into(), n.into());
    }
    if let Some(u) = min_claimable_usd {
        body.insert("compound_min_claimable_usd".into(), u.into());
    }
    serde_json::Value::Object(body)
}

/// The `/public-link` body. `show_onchain` is omitted when unset, so the link
/// keeps what it has.
fn share_body(hide_usd: bool, show_onchain: Option<bool>) -> serde_json::Value {
    let mut body = serde_json::Map::new();
    body.insert("hide_usd".into(), hide_usd.into());
    if let Some(on) = show_onchain {
        body.insert("show_onchain".into(), on.into());
    }
    serde_json::Value::Object(body)
}

/// `path` plus the set query parameters, each value percent-encoded.
fn pool_history_path(prefix: &str, dex: &str, address: &str, resolution: Option<&str>, buckets: Option<u32>) -> String {
    with_query(
        &format!("{prefix}/{}/{}/history", seg(dex), seg(address)),
        &[
            ("resolution", resolution.map(str::to_string)),
            ("buckets", buckets.map(|b| b.to_string())),
        ],
    )
}

fn evm_board_path(prefix: &str, chain: &str, venue: Option<&str>) -> String {
    with_query(
        &format!("{prefix}/{}/board", seg(chain)),
        &[("venue", venue.map(str::to_string))],
    )
}

fn evm_open_cost_path(prefix: &str, chain: &str, dex: &str, pool: Option<&str>, stake: Option<bool>) -> String {
    with_query(
        &format!("{prefix}/{}/open-cost", seg(chain)),
        &[
            ("dex", Some(dex.to_string())),
            ("pool", pool.map(str::to_string)),
            ("stake", stake.map(|s| s.to_string())),
        ],
    )
}

fn with_query(path: &str, params: &[(&str, Option<String>)]) -> String {
    let set: Vec<String> = params
        .iter()
        .filter_map(|(k, v)| v.as_ref().map(|v| format!("{k}={}", seg(v))))
        .collect();
    if set.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{}", set.join("&"))
    }
}

/// Filters for [`Client::x_feed`]. Every field is optional.
#[derive(Debug, Clone, Default)]
pub struct XFeedQuery {
    /// RFC 3339: the previous page's `next_before`.
    pub before: Option<String>,
    /// RFC 3339: only what was seen after this time.
    pub since: Option<String>,
    /// 1 to 100, default 50.
    pub limit: Option<i64>,
    /// Comma-separated directory categories.
    pub category: Option<String>,
    /// Only posts that name a token.
    pub ca_only: bool,
    pub min_followers: Option<i64>,
    /// One handle.
    pub account: Option<String>,
    /// Text search.
    pub q: Option<String>,
}

impl XFeedQuery {
    fn params(&self) -> Vec<(&'static str, Option<String>)> {
        vec![
            ("before", self.before.clone()),
            ("since", self.since.clone()),
            ("limit", self.limit.map(|n| n.to_string())),
            ("category", self.category.clone()),
            ("ca", self.ca_only.then(|| "1".to_string())),
            ("minFollowers", self.min_followers.map(|n| n.to_string())),
            ("account", self.account.clone()),
            ("q", self.q.clone()),
        ]
    }
}

/// Filters for [`Client::wallets_feed`] and [`Client::wallet_activity`].
#[derive(Debug, Clone, Default)]
pub struct WalletFeedQuery {
    /// The previous page's `next_before`.
    pub before: Option<String>,
    pub limit: Option<i64>,
    /// Comma separated: `buy`, `sell`, `swap`, `lp` or any single kind.
    pub kind: Option<String>,
    pub min_usd: Option<f64>,
    /// Only rows touching this mint.
    pub token: Option<String>,
    /// Only this wallet (feed only).
    pub wallet: Option<String>,
}

impl WalletFeedQuery {
    fn params(&self) -> Vec<(&'static str, Option<String>)> {
        vec![
            ("before", self.before.clone()),
            ("limit", self.limit.map(|n| n.to_string())),
            ("kind", self.kind.clone()),
            ("minUsd", self.min_usd.map(|n| n.to_string())),
            ("token", self.token.clone()),
            ("wallet", self.wallet.clone()),
        ]
    }
}

/// A live Server-Sent Events stream from [`Client::stream_social`].
///
/// Call [`SocialStream::next_event`] in a loop: it awaits the next frame and
/// returns one [`StreamEvent`], or `None` once the server closes the stream.
/// The stream is a one-way read and holds its connection open, so iterating it
/// blocks between events.
#[derive(Debug)]
pub struct SocialStream {
    response: reqwest::Response,
    buf: Vec<u8>,
    event: String,
    data: Vec<String>,
}

impl SocialStream {
    fn new(response: reqwest::Response) -> Self {
        SocialStream {
            response,
            buf: Vec::new(),
            event: "message".to_string(),
            data: Vec::new(),
        }
    }

    /// Await the next event, or `None` when the stream ends.
    ///
    /// Parses the event-stream grammar: a blank line dispatches the frame just
    /// read, a line beginning with a colon is a keep-alive comment, `event:`
    /// sets the name and every `data:` line appends to the payload. The payload
    /// is decoded as JSON, or wrapped as `{"raw": <text>}` when it is not JSON.
    pub async fn next_event(&mut self) -> Result<Option<StreamEvent>> {
        loop {
            while let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = self.buf.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&line_bytes);
                let line = line.trim_end_matches(['\r', '\n']);
                if line.is_empty() {
                    if !self.data.is_empty() {
                        let event = std::mem::replace(&mut self.event, "message".to_string());
                        let payload = self.data.join("\n");
                        self.data.clear();
                        return Ok(Some(StreamEvent {
                            event,
                            data: parse_sse_data(&payload),
                        }));
                    }
                    self.event = "message".to_string();
                    continue;
                }
                if line.starts_with(':') {
                    continue;
                }
                let (field, value) = match line.split_once(':') {
                    Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
                    None => (line, ""),
                };
                match field {
                    "event" => self.event = value.to_string(),
                    "data" => self.data.push(value.to_string()),
                    _ => {}
                }
            }
            match self.response.chunk().await? {
                Some(bytes) => self.buf.extend_from_slice(&bytes),
                None => return Ok(None),
            }
        }
    }
}

/// Decode one SSE data payload: JSON when it parses, else `{"raw": <text>}`.
fn parse_sse_data(payload: &str) -> serde_json::Value {
    if payload.is_empty() {
        return serde_json::Value::Object(serde_json::Map::new());
    }
    serde_json::from_str(payload)
        .unwrap_or_else(|_| serde_json::json!({ "raw": payload }))
}

/// Read a field of a JSON object as a string, stringifying a number. Absent or
/// null fields read as empty, which the caller rejects.
fn json_str(body: &serde_json::Value, key: &str) -> String {
    match body.get(key) {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// Builder for [`Client`].
#[derive(Debug, Clone)]
pub struct ClientBuilder {
    base_url: String,
    web_url: Option<String>,
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

    /// The web app origin, for [`Client::hot_pools`] and
    /// [`Client::opportunities`]. Only needed when the two origins are split,
    /// as in local development (API on one port, app on another): by default
    /// it is the base URL with its `/api` suffix removed, so the production
    /// base `https://ponk.exchange/api` reads the boards from
    /// `https://ponk.exchange`.
    pub fn web_url(mut self, web_url: impl Into<String>) -> Self {
        self.web_url = Some(web_url.into());
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
        let web_url = match self.web_url {
            Some(raw) => {
                let web = raw.trim_end_matches('/').to_string();
                if !(web.starts_with("http://") || web.starts_with("https://")) {
                    return Err(Error::InvalidBaseUrl(raw));
                }
                web
            }
            None => base_url
                .strip_suffix("/api")
                .map(str::to_string)
                .unwrap_or_else(|| base_url.clone()),
        };
        let http = match self.http {
            Some(http) => http,
            None => reqwest::Client::builder().build()?,
        };
        Ok(Client {
            http,
            base_url,
            web_url,
            api_key: self.api_key,
            timeout: self.timeout,
            fund_timeout: self.fund_timeout,
        })
    }
}

/// The DEX leg of the API wraps every success as `{"data": ...}`.
#[derive(serde::Deserialize)]
struct DexData<T> {
    data: T,
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
    fn the_web_origin_is_the_base_without_its_api_suffix() {
        // Production shape: the boards live one level above the API.
        let c = Client::public("https://ponk.exchange/api").unwrap();
        assert_eq!(c.web_url(), "https://ponk.exchange");
        // A base with no /api suffix is its own web origin.
        let c = Client::public("http://127.0.0.1:8080").unwrap();
        assert_eq!(c.web_url(), "http://127.0.0.1:8080");
        // Split origins, as in local development, are set explicitly.
        let c = Client::builder("http://127.0.0.1:8080/api")
            .web_url("http://127.0.0.1:3000/")
            .build()
            .unwrap();
        assert_eq!(c.web_url(), "http://127.0.0.1:3000");
        // And a relative web origin is refused like a relative base.
        assert!(matches!(
            Client::builder("https://ponk.exchange/api").web_url("ponk.exchange").build(),
            Err(Error::InvalidBaseUrl(_))
        ));
    }

    #[test]
    fn the_board_and_pool_list_queries_use_the_servers_names() {
        assert_eq!(
            with_query(
                "/pools/board",
                &[("view", Some("hot".into())), ("q", Some("SOL USDC".into()))]
            ),
            "/pools/board?view=hot&q=SOL%20USDC"
        );
        assert_eq!(
            with_query(
                "/dex/dlmm/pools",
                &[
                    ("page", Some("2".into())),
                    ("pageSize", Some("25".into())),
                    ("sortBy", Some("fees_24h".into())),
                    ("q", None),
                ]
            ),
            "/dex/dlmm/pools?page=2&pageSize=25&sortBy=fees_24h"
        );
    }

    #[test]
    fn the_dex_envelope_unwraps_to_the_page() {
        let d: DexData<DlmmPoolPage> =
            serde_json::from_str(r#"{"data":{"pools":[],"total":0,"page":1,"pageSize":50,"pages":0}}"#).unwrap();
        assert_eq!(d.data.page, Some(1));
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
    fn a_range_cost_path_keeps_the_query_outside_the_segments() {
        let c = Client::public("https://ponk.exchange/api").unwrap();
        let path = format!(
            "/pools/{}/{}/range-cost?bins={}",
            seg("meteora_dlmm"),
            seg("a?bins=1"),
            70
        );
        assert_eq!(
            c.url(&path),
            "https://ponk.exchange/api/pools/meteora_dlmm/a%3Fbins%3D1/range-cost?bins=70"
        );
    }

    #[test]
    fn fund_calls_get_the_longer_timeout() {
        let c = Client::new("https://ponk.exchange/api", "k").unwrap();
        assert!(c.fund_timeout() > DEFAULT_TIMEOUT);
    }

    #[test]
    fn the_exits_body_omits_an_absent_block() {
        let tp = serde_json::json!({ "stop_loss_pct": 10.0, "take_profit_pct": null });
        assert_eq!(
            exits_body(Some(&tp), None),
            serde_json::json!({ "tp_sl": { "stop_loss_pct": 10.0, "take_profit_pct": null } })
        );
        let auto = serde_json::json!({ "accumulate_fees": true });
        assert_eq!(
            exits_body(None, Some(&auto)),
            serde_json::json!({ "automation": { "accumulate_fees": true } })
        );
    }

    #[test]
    fn the_auto_compound_body_omits_unset_floors() {
        assert_eq!(
            auto_compound_body(false, None, None),
            serde_json::json!({ "auto_compound": false })
        );
        assert_eq!(
            auto_compound_body(true, Some(1000), Some(0.5)),
            serde_json::json!({
                "auto_compound": true,
                "compound_min_claimable_base_units": 1000,
                "compound_min_claimable_usd": 0.5
            })
        );
    }

    /// One request against a throwaway local server: what goes on the wire
    /// for a config edit is a PATCH to the agent's `/config` with the patch
    /// verbatim, nulls included, and the bearer key.
    #[tokio::test]
    async fn update_config_patches_the_config_route_with_the_patch_verbatim() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = sock.read(&mut chunk).unwrap();
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(h) = text.find("\r\n\r\n") {
                    let len = text[..h]
                        .lines()
                        .find_map(|l| {
                            let l = l.to_ascii_lowercase();
                            l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if buf.len() >= h + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let body = r#"{"id":"a1","recenter_mode":"extend"}"#;
            write!(
                sock,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
            String::from_utf8(buf).unwrap()
        });
        let c = Client::new(format!("http://{addr}"), "ponk_live_k").unwrap();
        let patch = serde_json::json!({ "recenter_mode": "extend", "ping_pong_bid_bins": null });
        let agent = c.update_config("a1", &patch).await.unwrap();
        assert_eq!(agent.id.as_deref(), Some("a1"));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("PATCH /v1/agents/a1/config HTTP/1.1\r\n"), "{raw}");
        assert!(raw.to_ascii_lowercase().contains("authorization: bearer ponk_live_k"), "{raw}");
        let body = &raw[raw.find("\r\n\r\n").unwrap() + 4..];
        let sent: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(sent, patch);
    }

    #[test]
    fn the_tracker_queries_use_the_servers_camel_case_names() {
        let q = XFeedQuery { limit: Some(2), ca_only: true, min_followers: Some(1000), category: Some("kol,news".into()), ..Default::default() };
        assert_eq!(
            with_query("/public/x/feed", &q.params()),
            "/public/x/feed?limit=2&category=kol%2Cnews&ca=1&minFollowers=1000"
        );
        assert_eq!(with_query("/public/x/feed", &XFeedQuery::default().params()), "/public/x/feed");
        let w = WalletFeedQuery { kind: Some("lp".into()), min_usd: Some(50.0), ..Default::default() };
        assert_eq!(with_query("/public/wallets/feed", &w.params()), "/public/wallets/feed?kind=lp&minUsd=50");
    }

    #[test]
    fn a_query_carries_only_the_set_parameters_encoded() {
        assert_eq!(with_query("/v1/points/leaderboard", &[("limit", None), ("offset", None)]), "/v1/points/leaderboard");
        assert_eq!(
            with_query("/v1/referral/handle/check", &[("handle", Some("a b&c".into()))]),
            "/v1/referral/handle/check?handle=a%20b%26c"
        );
        assert_eq!(
            with_query("/x", &[("limit", Some("5".into())), ("offset", None), ("hours", Some("48".into()))]),
            "/x?limit=5&hours=48"
        );
    }

    #[test]
    fn the_share_body_omits_an_unset_onchain_switch() {
        assert_eq!(share_body(true, None), serde_json::json!({ "hide_usd": true }));
        assert_eq!(
            share_body(false, Some(true)),
            serde_json::json!({ "hide_usd": false, "show_onchain": true })
        );
    }

    #[test]
    fn a_leaderboard_row_decodes_and_copies_into_a_create_body() {
        let row: LeaderboardRow = serde_json::from_value(serde_json::json!({
            "rank": 1, "slug": "abc123", "pnl_usd": "12.34", "fees_earned_usd": null,
            "spark": [1.0, null], "future_field": true,
            "copy": { "dex": "meteora_dlmm", "pool_address": "POOL", "strategy": "bin_rebalancer",
                      "config": { "kind": "bin_rebalancer", "target_bin_width": 11 } }
        }))
        .unwrap();
        assert_eq!(row.pnl_usd.as_deref(), Some("12.34"));
        assert!(row.fees_earned_usd.is_none());
        assert_eq!(row.spark, vec![Some(1.0), None]);
        let agent = row.copy.to_new_agent("copy", "GYdJ").expect("a copy block");
        assert_eq!(agent.pool_address.as_deref(), Some("POOL"));
        assert_eq!(agent.config["target_bin_width"], 11);
        assert!(AgentCopy::default().to_new_agent("x", "y").is_none());
    }

    /// A one-request local server answering `body`; returns the raw request.
    fn one_shot(body: &'static str) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = sock.read(&mut chunk).unwrap();
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(h) = text.find("\r\n\r\n") {
                    let len = text[..h]
                        .lines()
                        .find_map(|l| {
                            let l = l.to_ascii_lowercase();
                            l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if buf.len() >= h + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            write!(
                sock,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
            String::from_utf8(buf).unwrap()
        });
        (format!("http://{addr}"), handle)
    }

    #[tokio::test]
    async fn unfollow_sends_a_delete_with_the_key() {
        let (base, server) = one_shot(r#"{"slug":"abc123","following":false,"followers":2}"#);
        let c = Client::new(base, "ponk_live_k").unwrap();
        let f = c.unfollow("abc123").await.unwrap();
        assert_eq!(f.following, Some(false));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("DELETE /v1/follows/abc123 HTTP/1.1\r\n"), "{raw}");
        assert!(raw.to_ascii_lowercase().contains("authorization: bearer ponk_live_k"), "{raw}");
    }

    #[tokio::test]
    async fn the_public_board_needs_no_key_and_passes_the_window() {
        let (base, server) = one_shot(r#"{"ranked":[],"unranked":[],"window":"30d"}"#);
        let c = Client::public(base).unwrap();
        let board = c.leaderboard(Some("30d")).await.unwrap();
        assert_eq!(board.window.as_deref(), Some("30d"));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("GET /public/agents-leaderboard?window=30d HTTP/1.1\r\n"), "{raw}");
        assert!(!raw.to_ascii_lowercase().contains("authorization:"), "{raw}");
    }

    #[tokio::test]
    async fn claim_handle_posts_the_handle() {
        let (base, server) = one_shot(r#"{"code":"kader","handle":"kader","codes":[]}"#);
        let c = Client::new(base, "ponk_live_k").unwrap();
        let r = c.claim_handle("kader").await.unwrap();
        assert_eq!(r.handle.as_deref(), Some("kader"));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /v1/referral/handle HTTP/1.1\r\n"), "{raw}");
        let body = &raw[raw.find("\r\n\r\n").unwrap() + 4..];
        assert_eq!(serde_json::from_str::<serde_json::Value>(body).unwrap(), serde_json::json!({ "handle": "kader" }));
    }

    fn body_of(raw: &str) -> serde_json::Value {
        serde_json::from_str(&raw[raw.find("\r\n\r\n").unwrap() + 4..]).unwrap()
    }

    #[tokio::test]
    async fn create_agents_posts_the_batch() {
        let (base, server) = one_shot(
            r#"{"batch_id":"b","kind":"create","total":1,"done":1,"failed":0,"running":0,
            "items":[{"index":0,"agent_id":"a1","outcome":"done","ok":true,"status":200,
            "result":{"agent":{"id":"a1","name":"fleet 1"},"wallet":{"pubkey":"W"}}}]}"#,
        );
        let c = Client::new(base, "k").unwrap();
        let agent = NewAgent::new("fleet 1", "OWNER", "meteora_dlmm", "bin_rebalancer", serde_json::json!({}))
            .pool_address("P")
            .dry_run(true);
        let b = c.create_agents(&[agent], true).await.unwrap();
        assert_eq!((b.total, b.done, b.running), (1, 1, 0));
        assert_eq!(b.items[0].agent().unwrap().name.as_deref(), Some("fleet 1"));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /v1/agents/batch HTTP/1.1\r\n"), "{raw}");
        let body = body_of(&raw);
        assert_eq!(body["create_wallets"], true);
        assert_eq!(body["agents"][0]["dry_run"], true);
    }

    #[tokio::test]
    async fn fund_prepare_sends_agent_ids_and_reads_the_plan() {
        let (base, server) = one_shot(
            r#"{"owner":"OWNER","ok":true,"total_lamports":2,"network_fee_lamports":5000,
            "transactions":[{"index":0,"unsigned_tx_b64":"AQAB","transfers":[{"agent_id":"a1","to":"W1","lamports":1},
            {"agent_id":"a2","to":"W2","lamports":1}],"total_lamports":2,"simulated":true}],
            "items":[],"needs_mandate":["a1","a2"],"sign_urls":["https://ponk.exchange/agents/fund?a=a1:1,a2:1"]}"#,
        );
        let c = Client::new(base, "k").unwrap();
        let plan = c
            .fund_prepare(&[FundRequest::top_up("a1"), FundRequest::exact("a2", 1)])
            .await
            .unwrap();
        assert_eq!(plan.transactions[0].transfers.len(), 2);
        assert_eq!(plan.needs_mandate, vec!["a1", "a2"]);
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /v1/agents/fund/prepare HTTP/1.1\r\n"), "{raw}");
        assert_eq!(
            body_of(&raw),
            serde_json::json!({ "transfers": [{ "agent_id": "a1" }, { "agent_id": "a2", "lamports": 1 }] })
        );
    }

    #[tokio::test]
    async fn exit_all_agents_sends_all_true() {
        let (base, server) = one_shot(r#"{"batch_id":"b","kind":"exit","total":0,"items":[]}"#);
        let c = Client::new(base, "k").unwrap();
        c.exit_all_agents().await.unwrap();
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /v1/agents/batch/exit HTTP/1.1\r\n"), "{raw}");
        assert_eq!(body_of(&raw), serde_json::json!({ "all": true }));
    }

    #[tokio::test]
    async fn the_fleet_page_decodes_typed_parts() {
        let (base, server) = one_shot(
            r#"{"total":1,"offset":0,"limit":25,"next_offset":null,"rows":[{"agent":{"id":"a1"},
            "wallet":{"pubkey":"W","lamports":10,"withdrawable_lamports":5},"mandate":null,
            "performance":{"pnl_usd":null},"read_errors":[]}]}"#,
        );
        let c = Client::new(base, "k").unwrap();
        let rows = c.fleet_all(Some(25)).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].wallet.as_ref().unwrap().withdrawable_lamports, Some(5));
        assert!(rows[0].mandate.is_none());
        let raw = server.join().unwrap();
        assert!(raw.starts_with("GET /v1/agents/fleet?limit=25 HTTP/1.1\r\n"), "{raw}");
    }

    #[tokio::test]
    async fn an_empty_204_style_body_is_ok_for_delete() {
        let (base, server) = one_shot("");
        let c = Client::new(base, "k").unwrap();
        c.delete_agent("a1").await.unwrap();
        let raw = server.join().unwrap();
        assert!(raw.starts_with("DELETE /v1/agents/a1 HTTP/1.1\r\n"), "{raw}");
    }

    #[tokio::test]
    async fn update_notify_settings_puts_only_the_set_switches() {
        let (base, server) = one_shot(r#"{"rebalance_telegram":false,"mandate_telegram":true,"weekly_recap_telegram":true}"#);
        let c = Client::new(base, "k").unwrap();
        let s = c
            .update_notify_settings(&NotifySettings {
                rebalance_telegram: Some(false),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(s.mandate_telegram, Some(true));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("PUT /v1/notify-settings HTTP/1.1\r\n"), "{raw}");
        assert_eq!(body_of(&raw), serde_json::json!({ "rebalance_telegram": false }));
    }

    #[test]
    fn an_order_intent_serializes_to_the_servers_wire_shape() {
        let v = serde_json::to_value(OrderIntent::limit("USDC", "SOL", 25_000_000, 150.0)).unwrap();
        assert_eq!(
            v,
            serde_json::json!({ "kind": "limit", "input_mint": "USDC", "output_mint": "SOL", "amount": "25000000", "target_usd": 150.0 })
        );
        let d = serde_json::to_value(OrderIntent::dca("USDC", "SOL", 10, 5, 86_400)).unwrap();
        assert_eq!(d["kind"], "dca");
        assert_eq!(d["count"], 5);
        assert!(d.get("band").is_none());
        let t = serde_json::to_value(OrderIntent::trailing("SOL", "USDC", 1, 500)).unwrap();
        assert_eq!(t["trailing_bps"], 500);
        assert!(t.get("target_usd").is_none());
    }

    #[tokio::test]
    async fn plan_orders_posts_intents_and_reads_the_link() {
        let (base, server) = one_shot(
            r#"{"sign_url":"https://ponk.exchange/orders/sign?w=W&c=all","wallet":"W","orders":[],"cancel":[],"cancel_all":true,"signatures":"x","notes":["n"]}"#,
        );
        let c = Client::new(base, "k").unwrap();
        let plan = c
            .plan_orders(&[OrderIntent::limit("USDC", "SOL", 1, 1.0)], &[], true)
            .await
            .unwrap();
        assert!(plan.sign_url.unwrap().starts_with("https://ponk.exchange/orders/sign"));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /v1/orders/plan HTTP/1.1\r\n"), "{raw}");
        let body = body_of(&raw);
        assert_eq!(body["cancel_all"], true);
        assert_eq!(body["orders"][0]["kind"], "limit");
    }

    #[tokio::test]
    async fn an_order_call_sends_the_jupiter_session_header() {
        let (base, server) = one_shot(r#"{"transaction":"AQ","requestId":"rq","mint":"USDC","amount":"25"}"#);
        let c = Client::new(base, "ponk_live_k").unwrap();
        let craft = c
            .craft_order_deposit("eyJhbGciOiJIUzI1NiJ9.x.sig", "USDC", "SOL", 25, "price", Some("single"))
            .await
            .unwrap();
        assert_eq!(craft.request_id.as_deref(), Some("rq"));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /v1/orders/deposit/craft HTTP/1.1\r\n"), "{raw}");
        let lower = raw.to_ascii_lowercase();
        assert!(lower.contains("x-jupiter-session: eyjhbgcioijiuzi1nij9.x.sig"), "{raw}");
        assert!(lower.contains("authorization: bearer ponk_live_k"), "{raw}");
        let body = body_of(&raw);
        assert_eq!(body["amount"], "25");
        assert_eq!(body["orderSubType"], "single");
        assert!(body.get("userAddress").is_none());
    }

    #[tokio::test]
    async fn cancel_all_orders_sends_all_true() {
        let (base, server) = one_shot(
            r#"{"total":1,"prepared":1,"failed":0,"items":[{"family":"price","id":"ord-1","ok":true,"result":{"id":"ord-1","transaction":"AQ","requestId":"rq"}}]}"#,
        );
        let c = Client::new(base, "k").unwrap();
        let b = c.cancel_orders("tokentokentoken1", &[], true).await.unwrap();
        assert_eq!(b.items[0].cancel().unwrap().request_id.as_deref(), Some("rq"));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /v1/orders/cancel HTTP/1.1\r\n"), "{raw}");
        assert_eq!(body_of(&raw), serde_json::json!({ "orders": [], "all": true }));
    }

    #[tokio::test]
    async fn follow_leaders_batches_the_agents() {
        let (base, server) = one_shot(r#"{"total":1,"succeeded":1,"failed":0,"items":[{"key":"a1","ok":true}]}"#);
        let c = Client::new(base, "k").unwrap();
        let r = c.follow_leaders(&["a1"], false, false).await.unwrap();
        assert_eq!(r.succeeded, Some(1));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /v1/agents/follow/batch HTTP/1.1\r\n"), "{raw}");
        assert_eq!(
            body_of(&raw),
            serde_json::json!({ "items": [{ "agent_id": "a1", "auto_follow_pool_moves": false }], "unfollow": false })
        );
    }

    #[tokio::test]
    async fn the_clouds_withdraw_names_no_owner() {
        let (base, server) = one_shot(r#"{"unsigned_tx_b64":"AQAB","recent_blockhash":"H"}"#);
        let c = Client::new(base, "k").unwrap();
        let p = c.lp_withdraw_prepare("ponk_clouds", "P", "POS", 10_000).await.unwrap();
        assert_eq!(p.unsigned_tx_b64.as_deref(), Some("AQAB"));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /v1/lp/ponk_clouds/withdraw/prepare HTTP/1.1\r\n"), "{raw}");
        let body = body_of(&raw);
        assert!(body.get("owner").is_none());
        assert_eq!(body["bps_to_remove"], 10_000);
    }

    #[tokio::test]
    async fn clear_logs_is_a_delete() {
        let (base, server) = one_shot(r#"{"deleted":3,"hidden":3,"cleared_before":"2026-10-23T00:00:00Z"}"#);
        let c = Client::new(base, "k").unwrap();
        let r = c.clear_logs().await.unwrap();
        assert_eq!(r.hidden, Some(3));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("DELETE /v1/logs HTTP/1.1\r\n"), "{raw}");
    }

    #[tokio::test]
    async fn rank_presets_posts_only_set_fields() {
        let (base, server) = one_shot(r#"{"rows":[{"id":"t1"}],"cached":false,"window_days":7}"#);
        let c = Client::new(base, "k").unwrap();
        let r = c
            .rank_presets(&RankRequest {
                dex: "meteora_dlmm".into(),
                pool: "POOL".into(),
                window_days: Some(7),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(r.rows.len(), 1);
        assert_eq!(r.extra["window_days"], 7);
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /v1/presets/rank HTTP/1.1\r\n"), "{raw}");
        assert_eq!(body_of(&raw), serde_json::json!({ "dex": "meteora_dlmm", "pool": "POOL", "window_days": 7 }));
    }

    #[tokio::test]
    async fn the_damm_v2_open_sends_two_amounts_as_strings_and_no_range() {
        let (base, server) = one_shot(
            r#"{"unsigned_tx_b64":"AQAB","recent_blockhash":"H","position_address":"POS","position_nft_mint":"NFT"}"#,
        );
        let c = Client::public(base).unwrap();
        let p = c.damm_v2_open_prepare("POOL", "OWNER", u64::MAX, 0, None).await.unwrap();
        assert_eq!(p.position_address.as_deref(), Some("POS"));
        assert_eq!(p.position_nft_mint.as_deref(), Some("NFT"));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /public/lp/meteora_damm_v2/open/prepare HTTP/1.1\r\n"), "{raw}");
        assert_eq!(
            body_of(&raw),
            serde_json::json!({ "pool_address": "POOL", "owner": "OWNER", "amount_x": "18446744073709551615", "amount_y": "0" })
        );
    }

    #[tokio::test]
    async fn the_damm_v2_open_passes_slippage_when_set() {
        let (base, server) = one_shot(r#"{"unsigned_tx_b64":"AQAB"}"#);
        let c = Client::public(base).unwrap();
        c.damm_v2_open_prepare("POOL", "OWNER", 1, 2, Some(50)).await.unwrap();
        assert_eq!(body_of(&server.join().unwrap())["max_slippage_bps"], 50);
    }

    #[tokio::test]
    async fn the_damm_v2_claim_reads_the_fee_review() {
        let (base, server) = one_shot(
            r#"{"unsigned_tx_b64":"AQAB","recent_blockhash":"H","fee":{"bps":800},"fee_event_ids":["e1"]}"#,
        );
        let c = Client::public(base).unwrap();
        let p = c.damm_v2_claim_prepare("POOL", "POS", "OWNER").await.unwrap();
        assert_eq!(p.fee.as_ref().unwrap()["bps"], 800);
        assert_eq!(p.fee_event_ids, vec!["e1".to_string()]);
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /public/lp/meteora_damm_v2/claim/prepare HTTP/1.1\r\n"), "{raw}");
        assert_eq!(body_of(&raw), serde_json::json!({ "pool_address": "POOL", "position": "POS", "owner": "OWNER" }));
    }

    #[tokio::test]
    async fn the_damm_v2_withdraw_on_the_key_route_reads_closes_position() {
        let (base, server) = one_shot(r#"{"unsigned_tx_b64":"AQAB","fee_event_ids":[],"closes_position":false}"#);
        let c = Client::new(base, "k").unwrap();
        let p = c.lp_withdraw_prepare("meteora_damm_v2", "POOL", "POS", 10_000).await.unwrap();
        assert_eq!(p.closes_position, Some(false));
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /v1/lp/meteora_damm_v2/withdraw/prepare HTTP/1.1\r\n"), "{raw}");
        assert!(body_of(&raw).get("owner").is_none());
    }

    #[tokio::test]
    async fn the_public_withdraw_and_add_name_the_owner() {
        let (base, server) = one_shot(r#"{"unsigned_tx_b64":"AQAB"}"#);
        let c = Client::public(base).unwrap();
        c.lp_withdraw_prepare_public("meteora_damm_v2", "POOL", "POS", "OWNER", 2_500).await.unwrap();
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /public/lp/meteora_damm_v2/withdraw/prepare HTTP/1.1\r\n"), "{raw}");
        assert_eq!(
            body_of(&raw),
            serde_json::json!({ "pool_address": "POOL", "position": "POS", "owner": "OWNER", "bps_to_remove": 2500 })
        );

        let (base, server) = one_shot(r#"{"unsigned_tx_b64":"AQAB","recent_blockhash":"H"}"#);
        let c = Client::public(base).unwrap();
        c.lp_add_liquidity_prepare("meteora_damm_v2", "POOL", "POS", "OWNER", 5, 7).await.unwrap();
        let raw = server.join().unwrap();
        assert!(raw.starts_with("POST /public/lp/meteora_damm_v2/add-liquidity/prepare HTTP/1.1\r\n"), "{raw}");
        assert_eq!(
            body_of(&raw),
            serde_json::json!({ "pool_address": "POOL", "position": "POS", "owner": "OWNER", "amount_x": "5", "amount_y": "7" })
        );
    }

    #[tokio::test]
    async fn pool_history_paths_and_absent_stays_absent() {
        let (base, server) = one_shot(
            r#"{"dex":"meteora_dlmm","pool":"P","resolution":"day",
                "window":{"bucket_secs":86400,"from":10,"to":20,"buckets_in_window":3,"buckets_served":1,
                          "omitted":{"before_pool_created":0,"no_recorded_activity":2},"pool_created_at":null},
                "sources":{"tvl_usd":{"status":"not_published","source":null,"detail":"none"}},
                "points":[{"ts":10,"date":"d","volume_usd":100.0,"fees_usd":0.25,"tvl_usd":null}],
                "current":{"tvl_usd":5.0},"edge":{"status":"not_measured_recently"},"generated_at":30}"#,
        );
        let c = Client::public(base).unwrap();
        let h = c.pool_history("meteora_dlmm", "P", Some("day"), Some(3)).await.unwrap();
        let raw = server.join().unwrap();
        assert!(raw.starts_with("GET /public/pools/meteora_dlmm/P/history?resolution=day&buckets=3 HTTP/1.1\r\n"), "{raw}");
        assert_eq!(h.points.len(), 1);
        assert_eq!(h.points[0].fees_usd, Some(0.25));
        assert_eq!(h.points[0].tvl_usd, None);
        let w = h.window.unwrap();
        assert_eq!(w.omitted.unwrap().no_recorded_activity, Some(2));
        assert_eq!(w.pool_created_at, None);
        let tvl = h.sources.unwrap().tvl_usd.unwrap();
        assert_eq!(tvl.status.as_deref(), Some("not_published"));
        assert_eq!(tvl.source, None);
        assert_eq!(h.edge.unwrap().status.as_deref(), Some("not_measured_recently"));

        let (base, server) = one_shot(r#"{"points":[]}"#);
        let c = Client::new(base, "k").unwrap();
        c.pool_history_keyed("orca", "P", None, None).await.unwrap();
        let raw = server.join().unwrap();
        assert!(raw.starts_with("GET /v1/pools/orca/P/history HTTP/1.1\r\n"), "{raw}");
    }

    #[tokio::test]
    async fn the_pool_read_carries_the_sell_check() {
        let (base, server) = one_shot(
            r#"{"dex":"meteora_damm_v2","pool_address":"P","bin_step":0,"active_bin_id":0,
                "price_divergence":0.01,"mispricing_warning":null,"sell_warning":"may not be sellable"}"#,
        );
        let c = Client::public(base).unwrap();
        let p = c.pool("meteora_damm_v2", "P").await.unwrap();
        assert_eq!(p.sell_warning.as_deref(), Some("may not be sellable"));
        assert_eq!(p.mispricing_warning, None);
        assert_eq!(p.price_divergence, Some(0.01));
        assert!(server.join().unwrap().starts_with("GET /pools/meteora_damm_v2/P HTTP/1.1\r\n"));
    }

    #[tokio::test]
    async fn copy_alerts_is_a_settings_switch() {
        let (base, server) = one_shot(
            r#"{"rebalance_telegram":false,"mandate_telegram":true,"weekly_recap_telegram":false,
                "comments_telegram":true,"copy_alerts":false}"#,
        );
        let c = Client::new(base, "k").unwrap();
        let s = c
            .update_notify_settings(&NotifySettings { copy_alerts: Some(false), ..Default::default() })
            .await
            .unwrap();
        assert_eq!(s.copy_alerts, Some(false));
        assert_eq!(s.comments_telegram, Some(true));
        assert_eq!(body_of(&server.join().unwrap()), serde_json::json!({ "copy_alerts": false }));
    }
    // -- EVM chains (0.15.0) -------------------------------------------------
    //
    // The bodies are the files in `tests/fixtures/evm`. The chain list, the
    // three pool boards, the v4 pool and the gas coin token are answers the
    // public routes gave on 2026-11-10, cut down to a few rows. The agent,
    // wallet, position, performance, exit, mandate, positions, plan and fleet
    // bodies are written field for field from the routes document and the
    // server's own Serialize structs, with illustrative values.

    macro_rules! fx {
        ($name:literal) => {
            include_str!(concat!("../tests/fixtures/evm/", $name))
        };
    }

    const EVM_AGENT: &str = "0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b";
    const EVM_POOL: &str = "eip155:56:0x92b7807bf19b7dddf89b706143896d05228f3121";
    const EVM_POOL_SEG: &str = "eip155%3A56%3A0x92b7807bf19b7dddf89b706143896d05228f3121";
    const EVM_WITHDRAW: &str = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed";
    const SOL_WALLET: &str = "GYdJfRSHhDAVVEz8optL8aGSQKd5daSZ9Ao3sVLZRkEe";

    fn request_line(raw: &str) -> &str {
        raw.lines().next().unwrap_or("")
    }

    fn has_key(raw: &str) -> bool {
        raw.to_ascii_lowercase().contains("authorization: bearer ponk_live_k")
    }

    /// Like `one_shot`, with a status and a body of the caller's choosing.
    fn one_shot_status(status: u16, body: &'static str) -> (String, std::thread::JoinHandle<String>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            loop {
                let n = sock.read(&mut chunk).unwrap();
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).to_string();
                if let Some(h) = text.find("\r\n\r\n") {
                    let len = text[..h]
                        .lines()
                        .find_map(|l| {
                            let l = l.to_ascii_lowercase();
                            l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    if buf.len() >= h + 4 + len {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            write!(
                sock,
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
            String::from_utf8(buf).unwrap()
        });
        (format!("http://{addr}"), handle)
    }

    #[tokio::test]
    async fn evm_chains_lists_six_chains_with_their_venues() {
        let (base, server) = one_shot(fx!("chains.json"));
        let chains = Client::public(base).unwrap().evm_chains().await.unwrap();
        let raw = server.join().unwrap();
        assert_eq!(request_line(&raw), "GET /public/evm/chains HTTP/1.1");
        assert!(!raw.to_ascii_lowercase().contains("authorization:"), "{raw}");
        assert_eq!(chains.read_enabled, Some(true));
        assert_eq!(chains.chains.len(), 6);
        let bsc = chains.chain("bsc").unwrap();
        assert_eq!((bsc.evm_chain_id, bsc.native_symbol.as_deref()), (Some(56), Some("BNB")));
        assert!(bsc.venues.iter().any(|v| v == "pancakeswap_v3"));
        assert_eq!(chains.chain("solana").unwrap().evm_chain_id, None);
        assert!(chains.chain("polygon").is_none());
        // The crate's own tables agree with what the server states.
        for row in &chains.chains {
            let id = row.id.as_deref().unwrap();
            if let Some(cid) = row.evm_chain_id {
                assert_eq!(crate::evm::chain_id(id), Some(cid), "{id}");
                assert_eq!(crate::evm::native_symbol(id), row.native_symbol.as_deref(), "{id}");
                for venue in &row.venues {
                    assert!(crate::evm::venue_chains(venue).contains(&id), "{venue} on {id}");
                }
            }
        }
    }

    #[tokio::test]
    async fn the_keyed_evm_reads_use_the_v1_twins_with_the_key() {
        let (base, server) = one_shot(fx!("chains.json"));
        Client::new(base, "ponk_live_k").unwrap().evm_chains_keyed().await.unwrap();
        let raw = server.join().unwrap();
        assert_eq!(request_line(&raw), "GET /v1/evm/chains HTTP/1.1");
        assert!(has_key(&raw), "{raw}");

        let (base, server) = one_shot(fx!("board_bsc.json"));
        Client::new(base, "ponk_live_k").unwrap().evm_pools_keyed("bsc", None).await.unwrap();
        assert_eq!(request_line(&server.join().unwrap()), "GET /v1/evm/bsc/board HTTP/1.1");

        let (base, server) = one_shot(fx!("pool_base.json"));
        let pool = Client::new(base, "ponk_live_k")
            .unwrap()
            .evm_pool_keyed("eip155:8453:0x4a7af46bc8abc00048c6c7920a1488daf568366c")
            .await
            .unwrap();
        assert_eq!(
            request_line(&server.join().unwrap()),
            "GET /v1/evm/pools/eip155%3A8453%3A0x4a7af46bc8abc00048c6c7920a1488daf568366c HTTP/1.1"
        );
        assert_eq!(pool.aerodrome.unwrap()["gaugeLive"], true);

        let (base, server) = one_shot(fx!("token_native.json"));
        Client::new(base, "ponk_live_k")
            .unwrap()
            .evm_token_keyed(&crate::evm::native_token_id("bsc").unwrap())
            .await
            .unwrap();
        assert!(request_line(&server.join().unwrap()).starts_with("GET /v1/evm/tokens/eip155%3A56%3A0x0000"));
    }

    #[tokio::test]
    async fn the_bnb_board_reads_fee_tier_tvl_windows_farm_and_flags() {
        let (base, server) = one_shot(fx!("board_bsc.json"));
        let board = Client::public(base).unwrap().evm_pools("bsc", Some("pancakeswap_v3")).await.unwrap();
        assert_eq!(
            request_line(&server.join().unwrap()),
            "GET /public/evm/bsc/board?venue=pancakeswap_v3 HTTP/1.1"
        );
        assert!(board.ready());
        assert_eq!(board.chain.as_deref(), Some("bsc"));
        assert_eq!(board.status.as_ref().unwrap().stale, Some(false));
        let (v3, v2) = (&board.pools[0], &board.pools[1]);
        assert_eq!(v3.dex.as_deref(), Some("pancakeswap_v3"));
        assert_eq!(v3.address.as_deref(), Some(EVM_POOL));
        assert_eq!(v3.pair(), "USDT/USDC");
        assert_eq!((v3.fee_pct, v3.dynamic_fee, v3.bin_step), (Some(0.01), Some(false), Some(1)));
        assert!(v3.tvl.unwrap() > 1_000_000.0);
        assert_eq!(v3.tvl_source.as_deref(), Some("onchain"));
        let (volume, fees) = (v3.volume.as_ref().unwrap(), v3.fees.as_ref().unwrap());
        assert!(volume.h24.unwrap() > volume.h1.unwrap());
        assert!(fees.h24.unwrap() > 0.0);
        // A window with no reading is None, never 0.
        assert_eq!(fees.h4, None);
        let farm = v3.farm.as_ref().unwrap();
        assert_eq!((farm.token.as_deref(), farm.live), (Some("CAKE"), Some(true)));
        assert!(farm.apr_pct.is_some());
        let x = v3.x.as_ref().unwrap();
        assert_eq!(x.mint.as_deref(), Some("eip155:56:0x55d398326f99059ff775485246999027b3197955"));
        assert_eq!(x.decimals, Some(18));
        let now = v3.created_at.unwrap() + 86_400_000;
        assert!(v3.flags(now).is_empty());
        // Under an hour old is a flag.
        assert_eq!(v3.flags(v3.created_at.unwrap() + 60_000), vec!["under_1h_old"]);
        // PancakeSwap v2: full range, no farm, and a token off the list.
        assert!(v2.farm.is_none());
        assert_eq!(v2.bin_step, None);
        assert_eq!(v2.flags(v2.created_at.unwrap() + 86_400_000), vec!["unverified_token"]);
    }

    #[tokio::test]
    async fn a_dynamic_fee_v4_pool_has_no_fee_tier_and_a_hook_flag() {
        let (base, server) = one_shot(fx!("board_ethereum.json"));
        let board = Client::public(base).unwrap().evm_pools("ethereum", None).await.unwrap();
        assert_eq!(request_line(&server.join().unwrap()), "GET /public/evm/ethereum/board HTTP/1.1");
        let (hookless, dynamic) = (&board.pools[0], &board.pools[1]);
        let id = crate::evm::parse_evm_id(hookless.address.as_deref().unwrap()).unwrap();
        assert!(matches!(id.kind, crate::evm::EvmIdKind::V4Pool { .. }));
        let later = |p: &EvmPool| p.created_at.unwrap() + 86_400_000;
        // The zero address is a pool with no hook, so nothing is flagged.
        assert!(hookless.flags(later(hookless)).is_empty());
        assert_eq!(hookless.fee_pct, Some(0.0005));
        assert_eq!(dynamic.dynamic_fee, Some(true));
        assert_eq!(dynamic.fee_pct, None);
        assert_eq!(dynamic.fees.as_ref().unwrap().h24, None);
        let hook = dynamic.hooks.as_ref().unwrap();
        assert_eq!((hook.agent_eligible, hook.touches_lp_funds), (Some(false), Some(true)));
        assert_eq!(dynamic.flags(later(dynamic)), vec!["hook_not_agent_eligible"]);
        assert!(dynamic.data_insufficient[0].starts_with("fees:"));
    }

    #[tokio::test]
    async fn the_base_board_carries_aero_farms_and_gauge_facts() {
        let (base, _server) = one_shot(fx!("board_base.json"));
        let board = Client::public(base).unwrap().evm_pools("base", None).await.unwrap();
        assert_eq!(board.pools.len(), 2);
        for p in &board.pools {
            let farm = p.farm.as_ref().unwrap();
            assert_eq!((farm.token.as_deref(), farm.live), (Some("AERO"), Some(true)));
            assert!(board.aerodrome.iter().any(|f| f["key"].as_str() == p.key.as_deref()));
        }
    }

    #[tokio::test]
    async fn an_unread_chain_is_an_empty_board_not_an_error() {
        let (base, _server) = one_shot(
            r#"{"chain":"arbitrum","asOf":0,"servedAt":1794371669624,"pools":[],"ok":{},"status":{"lastError":"the first read of this chain has not completed yet"}}"#,
        );
        let board = Client::public(base).unwrap().evm_pools("arbitrum", None).await.unwrap();
        assert!(!board.ready());
        assert!(board.pools.is_empty());
        assert!(board.status.unwrap().last_error.unwrap().contains("first read"));
    }

    #[tokio::test]
    async fn pool_detail_and_the_gas_coin_token() {
        let (base, server) = one_shot(fx!("pool_v4.json"));
        let id = "eip155:1:0x000000000004444c5dc75cb358380d2e3de08a90:0x98316e8405502774a5d0a0761276b5d26505a8195c1905169864008d5e7a3eea";
        let pool = Client::public(base).unwrap().evm_pool(id).await.unwrap();
        assert_eq!(
            request_line(&server.join().unwrap()),
            format!("GET /public/evm/pools/{} HTTP/1.1", id.replace(':', "%3A"))
        );
        assert_eq!(pool.address.as_deref(), Some(id));
        assert!(pool.sqrt_price_x96.is_some());

        let (base, server) = one_shot(fx!("token_native.json"));
        let token = Client::public(base)
            .unwrap()
            .evm_token(&crate::evm::native_token_id("bsc").unwrap())
            .await
            .unwrap();
        assert!(request_line(&server.join().unwrap()).starts_with("GET /public/evm/tokens/eip155%3A56%3A0x0000"));
        assert_eq!((token.symbol.as_deref(), token.decimals), (Some("BNB"), Some(18)));
        assert_eq!(token.price_source.as_deref(), Some("chainlink"));
        assert!(token.price_usd.unwrap() > 0.0);
        assert!(crate::evm::parse_evm_id(token.mint.as_deref().unwrap()).unwrap().is_native());
    }

    #[tokio::test]
    async fn the_apis_sentence_is_the_error() {
        let (base, _server) = one_shot_status(
            422,
            r#"{"error":{"code":"unprocessable","message":"Solana is not read yet: EVM reads cover Arbitrum, Robinhood Chain, Ethereum, BNB Chain, Base","request_id":"r1"}}"#,
        );
        let err = Client::public(base).unwrap().evm_pools("solana", None).await.unwrap_err();
        let api = err.api().expect("an api error");
        assert_eq!(api.status, 422);
        assert!(api.message.starts_with("Solana is not read yet"), "{}", api.message);
        assert_eq!(api.request_id.as_deref(), Some("r1"));
        assert!(err.to_string().contains("Solana is not read yet"), "{err}");
    }

    #[tokio::test]
    async fn open_cost_reads_the_v1_quote_with_the_api_rates() {
        let (base, server) = one_shot(fx!("open_cost_v1.json"));
        let cost = Client::new(base, "ponk_live_k")
            .unwrap()
            .evm_open_cost("bsc", "pancakeswap_v3", Some(EVM_POOL), Some(true))
            .await
            .unwrap();
        let raw = server.join().unwrap();
        assert_eq!(
            request_line(&raw),
            format!("GET /v1/evm/bsc/open-cost?dex=pancakeswap_v3&pool={EVM_POOL_SEG}&stake=true HTTP/1.1")
        );
        assert!(has_key(&raw), "{raw}");
        assert_eq!(
            (cost.activation_fee_bps, cost.performance_fee_bps, cost.exit_fee_bps),
            (Some(150), Some(1500), Some(0))
        );
        assert_eq!(cost.native_symbol.as_deref(), Some("BNB"));
        // Wei stays a string; the minimum is the reserve plus the gas budget.
        assert_eq!(cost.min_open_wei.as_deref(), Some("452200000000000"));
        assert_eq!(
            crate::evm::wei_to_native(cost.min_open_wei.as_deref().unwrap()).as_deref(),
            Some("0.0004522")
        );
        assert_eq!((cost.gas_usd.as_deref(), cost.native_usd.as_deref()), (Some("0.0560"), Some("680.65")));
    }

    #[tokio::test]
    async fn the_public_open_cost_needs_no_key_and_omits_what_was_not_set() {
        let (base, server) = one_shot(
            r#"{"chain":"bsc","dex":"pancakeswap_v3","pool":null,"native_symbol":"BNB","gas_wei":null,"gas_usd":null,"reserve_wei":null,"min_open_wei":null,"native_usd":null,"activation_fee_bps":100,"performance_fee_bps":800,"exit_fee_bps":0}"#,
        );
        let cost = Client::public(base)
            .unwrap()
            .evm_open_cost_public("bsc", "pancakeswap_v3", None, Some(false))
            .await
            .unwrap();
        assert_eq!(
            request_line(&server.join().unwrap()),
            "GET /public/evm/bsc/open-cost?dex=pancakeswap_v3&stake=false HTTP/1.1"
        );
        assert_eq!((cost.activation_fee_bps, cost.performance_fee_bps), (Some(100), Some(800)));
        // An unread chain quotes nothing rather than guessing.
        assert_eq!((cost.gas_wei, cost.min_open_wei), (None, None));
    }

    #[tokio::test]
    async fn create_evm_agent_sends_the_chain_the_pool_and_the_range_as_fractions() {
        let (base, server) = one_shot(fx!("agent.json"));
        let new = NewAgent::evm_range("bnb usdt", SOL_WALLET, "bsc", "pancakeswap_v3", EVM_POOL, 0.05, 0.05)
            .unwrap()
            .stake(true)
            .withdrawal_address(EVM_WITHDRAW)
            .dry_run(false);
        let agent = Client::new(base, "ponk_live_k").unwrap().create_evm_agent(&new).await.unwrap();
        let raw = server.join().unwrap();
        assert_eq!(request_line(&raw), "POST /v1/agents HTTP/1.1");
        assert!(has_key(&raw), "{raw}");
        assert_eq!(
            body_of(&raw),
            serde_json::json!({
                "name": "bnb usdt",
                "wallet_address": SOL_WALLET,
                "dex": "pancakeswap_v3",
                "strategy": "bin_rebalancer",
                "config": {
                    "kind": "bin_rebalancer",
                    "target_bin_width": 977,
                    "auto_compound": true,
                    "knobs": { "range_pct": { "down_pct": 0.05, "up_pct": 0.05 } },
                    "evm": { "stake": true }
                },
                "pool_address": EVM_POOL,
                "dry_run": false,
                "chain": "bsc",
                "withdrawal_address": EVM_WITHDRAW
            })
        );
        assert_eq!(agent.chain.as_deref(), Some("bsc"));
        assert_eq!(agent.origin.as_deref(), Some("api"));
        assert_eq!((agent.wallet_source.as_deref(), agent.mode.as_deref()), (Some("managed"), Some("auto")));
        // The rates an agent a key creates pays: 1.5% once, 15% of fees.
        assert_eq!((agent.activation_fee_bps, agent.managed_fee_bps), (Some(150), Some(1500)));
        assert_eq!(crate::evm::chain_of(agent.pool_address.as_deref().unwrap()), "bsc");
    }

    #[test]
    fn a_full_range_evm_agent_is_compound_and_hold() {
        let new = NewAgent::evm_full_range(
            "aero",
            SOL_WALLET,
            "base",
            "aerodrome_classic",
            "eip155:8453:0xf634058dc94e6bf8e828e27621320c423bc8b18e",
        )
        .stake(true)
        .dry_run(true);
        assert_eq!(
            serde_json::to_value(&new).unwrap(),
            serde_json::json!({
                "name": "aero",
                "wallet_address": SOL_WALLET,
                "dex": "aerodrome_classic",
                "strategy": "fee_collector",
                "config": {
                    "kind": "fee_collector", "min_total_pending_base_units": 0,
                    "auto_compound": true, "evm": { "stake": true }
                },
                "pool_address": "eip155:8453:0xf634058dc94e6bf8e828e27621320c423bc8b18e",
                "dry_run": true,
                "chain": "base"
            })
        );
        assert!(NewAgent::evm_range("x", "w", "bsc", "pancakeswap_v3", EVM_POOL, 5.0, 0.05).is_err());
    }

    #[test]
    fn a_solana_create_body_is_what_it_was() {
        let body = NewAgent::new("a", SOL_WALLET, "orca", "monitor_only", serde_json::json!({}))
            .pool_address("P")
            .dry_run(true);
        assert_eq!(
            serde_json::to_value(&body).unwrap(),
            serde_json::json!({
                "name": "a", "wallet_address": SOL_WALLET, "dex": "orca", "strategy": "monitor_only",
                "config": {}, "pool_address": "P", "dry_run": true
            })
        );
    }

    #[tokio::test]
    async fn create_evm_agent_refuses_before_sending_what_cannot_be_right() {
        // Nothing listens on this port: a request would be a transport error.
        let c = Client::new("http://127.0.0.1:9", "ponk_live_k").unwrap();
        let solana = NewAgent::new("a", SOL_WALLET, "orca", "monitor_only", serde_json::json!({})).pool_address("P");
        let adopt = NewAgent::evm_full_range("a", SOL_WALLET, "bsc", "pancakeswap_v2", EVM_POOL).position_address("X");
        let mut no_pool = NewAgent::evm_full_range("a", SOL_WALLET, "bsc", "pancakeswap_v2", EVM_POOL);
        no_pool.pool_address = None;
        for (agent, word) in [(solana, "chain must be"), (adopt, "position_address"), (no_pool, "pool_address")] {
            match c.create_evm_agent(&agent).await {
                Err(Error::InvalidArgument(m)) => assert!(m.contains(word), "{m}"),
                other => panic!("expected a refusal naming {word}, got {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn every_agent_carries_a_chain() {
        let (base, server) = one_shot(fx!("agent.json"));
        let agent = Client::new(base, "ponk_live_k").unwrap().get_agent(EVM_AGENT).await.unwrap();
        assert_eq!(request_line(&server.join().unwrap()), format!("GET /v1/agents/{EVM_AGENT} HTTP/1.1"));
        assert_eq!((agent.chain.as_deref(), agent.dex.as_deref()), (Some("bsc"), Some("pancakeswap_v3")));
        let solana: Agent = serde_json::from_str(r#"{"id":"a","chain":"solana","dex":"orca"}"#).unwrap();
        assert_eq!(solana.chain.as_deref(), Some("solana"));
    }

    #[tokio::test]
    async fn the_evm_wallet_view_is_wei_usd_and_the_mandate_status() {
        let (base, server) = one_shot(fx!("wallet.json"));
        let w = Client::new(base, "ponk_live_k").unwrap().agent_wallet(EVM_AGENT).await.unwrap();
        assert_eq!(request_line(&server.join().unwrap()), format!("GET /v1/agents/{EVM_AGENT}/wallet HTTP/1.1"));
        assert!(w.is_evm());
        assert_eq!((w.chain.as_deref(), w.native_symbol.as_deref()), (Some("bsc"), Some("BNB")));
        assert_eq!(w.address.as_deref(), Some("0x52908400098527886E0F7030069857D2E4169EE7"));
        assert_eq!(w.balance_wei.as_deref(), Some("150000000000000000"));
        assert_eq!(crate::evm::wei_to_native(w.balance_wei.as_deref().unwrap()).as_deref(), Some("0.15"));
        assert_eq!((w.balance_usd.as_deref(), w.native_usd.as_deref()), (Some("102.10"), Some("680.65")));
        assert_eq!(
            (w.withdrawal_address.as_deref(), w.mandate_status.as_deref()),
            (Some(EVM_WITHDRAW), Some("active"))
        );
        // The Solana fields are absent, not zero.
        assert_eq!((w.lamports, w.pubkey.as_deref()), (None, None));
        let solana: AgentWallet = serde_json::from_str(r#"{"pubkey":"7Q7w","lamports":5,"scope":"per_agent"}"#).unwrap();
        assert!(!solana.is_evm());
        assert_eq!((solana.lamports, solana.balance_wei), (Some(5), None));
    }

    #[tokio::test]
    async fn a_new_evm_wallet_has_no_mandate_yet() {
        let (base, server) = one_shot(fx!("wallet_new.json"));
        let made = Client::new(base, "ponk_live_k").unwrap().create_agent_wallet(EVM_AGENT).await.unwrap();
        assert_eq!(request_line(&server.join().unwrap()), format!("POST /v1/agents/{EVM_AGENT}/wallet HTTP/1.1"));
        assert_eq!(made.address.as_deref(), Some("0x52908400098527886E0F7030069857D2E4169EE7"));
        assert_eq!((made.chain.as_deref(), made.native_symbol.as_deref()), (Some("bsc"), Some("BNB")));
        assert_eq!(made.mandate_status, None);
        assert_eq!(made.min_open_wei.as_deref(), Some("452200000000000"));
    }

    #[tokio::test]
    async fn the_mandate_names_its_chain() {
        let (base, server) = one_shot(fx!("mandate.json"));
        let m = Client::new(base, "ponk_live_k").unwrap().agent_mandate(EVM_AGENT).await.unwrap();
        assert_eq!(request_line(&server.join().unwrap()), format!("GET /v1/agents/{EVM_AGENT}/mandate HTTP/1.1"));
        assert_eq!((m.chain.as_deref(), m.status.as_deref()), (Some("bsc"), Some("active")));
        assert_eq!(m.withdrawal_address.as_deref(), Some(EVM_WITHDRAW));
        assert_eq!(m.max_action_value_usd, None);
    }

    #[tokio::test]
    async fn an_evm_position_is_in_ticks_and_a_full_range_one_has_none() {
        let (base, server) = one_shot(fx!("position.json"));
        let p = Client::new(base, "ponk_live_k").unwrap().agent_position(EVM_AGENT).await.unwrap();
        assert_eq!(request_line(&server.join().unwrap()), format!("GET /v1/agents/{EVM_AGENT}/position HTTP/1.1"));
        assert_eq!(
            (p.lower_bin_id, p.upper_bin_id, p.active_bin_id, p.bin_step),
            (Some(-498), Some(478), Some(-10), Some(1))
        );
        assert_eq!((p.chain.as_deref(), p.staked, p.is_full_range), (Some("bsc"), Some(true), Some(false)));
        assert_eq!(p.current_price.as_deref(), Some("0.999006"));
        let full: AgentPosition = serde_json::from_str(fx!("position_full_range.json")).unwrap();
        assert_eq!(full.is_full_range, Some(true));
        assert_eq!((full.lower_bin_id, full.bin_step, full.current_price), (None, None, None));
        assert_eq!(full.chain.as_deref(), Some("base"));
    }

    #[tokio::test]
    async fn evm_performance_is_usd_with_fractions_and_the_farm_block() {
        let (base, server) = one_shot(fx!("performance.json"));
        let perf = Client::new(base, "ponk_live_k").unwrap().agent_performance(EVM_AGENT).await.unwrap();
        assert_eq!(
            request_line(&server.join().unwrap()),
            format!("GET /v1/agents/{EVM_AGENT}/performance HTTP/1.1")
        );
        // Fractions: 0.0162 is +1.62%.
        assert_eq!((perf.pnl_pct, perf.drawdown_pct), (Some(0.0162), Some(0.0069)));
        assert_eq!((perf.pnl_usd.as_deref(), perf.cost_basis_usd.as_deref()), (Some("1.62"), Some("100.00")));
        let farm = perf.farm_rewards.as_ref().unwrap();
        assert_eq!((farm.token.as_deref(), farm.pending_amount.as_deref()), (Some("CAKE"), Some("0.412000")));
        assert_eq!((farm.pending_usd.as_deref(), farm.harvested_usd.as_deref()), (Some("0.74"), Some("1.10")));
        assert_eq!((farm.staked, farm.apr_pct, farm.apr_basis.as_deref()), (Some(true), Some(8.2), Some("position")));
        assert_eq!(perf.figures_unavailable, None);
        assert!(perf.pnl_note.as_deref().unwrap().starts_with("Position value"));

        let unread: AgentPerformance = serde_json::from_str(fx!("performance_unread.json")).unwrap();
        assert_eq!((unread.current_value_usd, unread.pnl_usd, unread.pnl_pct), (None, None, None));
        assert_eq!(unread.figures_unavailable.as_deref(), Some("The BNB Smart Chain RPC did not answer."));
        assert_eq!(unread.farm_rewards.unwrap().apr_basis, None);
        // A Solana agent has no farm block at all.
        let solana: AgentPerformance = serde_json::from_str(r#"{"pnl_usd":"1"}"#).unwrap();
        assert!(solana.farm_rewards.is_none());
    }

    #[tokio::test]
    async fn evm_decisions_and_value_history_use_the_same_routes() {
        let (base, server) = one_shot(r#"{"state":"healthy_no_action","decisions":[]}"#);
        let d = Client::new(base, "ponk_live_k").unwrap().agent_decisions(EVM_AGENT, Some(6), None).await.unwrap();
        assert_eq!(
            request_line(&server.join().unwrap()),
            format!("GET /v1/agents/{EVM_AGENT}/decisions?hours=6 HTTP/1.1")
        );
        assert_eq!(d["state"], "healthy_no_action");
        let (base, server) = one_shot(fx!("value_history.json"));
        let h = Client::new(base, "ponk_live_k").unwrap().agent_value_history(EVM_AGENT, Some("24h")).await.unwrap();
        assert_eq!(
            request_line(&server.join().unwrap()),
            format!("GET /v1/agents/{EVM_AGENT}/value-history?range=24h HTTP/1.1")
        );
        assert_eq!(h.bucket_secs, Some(900));
        // A bucket with no reading is null, never filled in.
        assert!(h.points[1]["value_usd"].is_null());
        assert_eq!(h.points[2]["value_usd"], 101.2);
    }

    #[tokio::test]
    async fn positions_list_solana_and_evm_rows_together() {
        let (base, server) = one_shot(fx!("positions.json"));
        let rows = Client::new(base, "ponk_live_k").unwrap().list_positions().await.unwrap();
        assert_eq!(request_line(&server.join().unwrap()), "GET /v1/positions HTTP/1.1");
        let (sol, evm) = (&rows[0], &rows[1]);
        assert_eq!((sol.chain.as_deref(), evm.chain.as_deref()), (Some("solana"), Some("bsc")));
        // A Solana row's raw is a number, an EVM row's a string: both decode.
        assert_eq!(sol.fees_claimable_x.as_ref().unwrap().raw, Some(12_345));
        assert_eq!(evm.fees_claimable_x.as_ref().unwrap().raw, Some(210_000_000_000_000_000));
        // Past what a JSON number or a u64 can hold.
        assert_eq!(evm.fees_claimable_y.as_ref().unwrap().raw, Some(u128::MAX));
        assert_eq!(evm.dex.as_deref(), Some("pancakeswap_v3"));
        assert_eq!(evm.agent_id.as_deref(), Some(EVM_AGENT));
        assert_eq!(evm.pnl_source.as_deref(), Some("evm_runtime"));
        assert_eq!(crate::evm::chain_of(evm.token_x.as_deref().unwrap()), "bsc");
        assert_eq!((evm.staked, evm.farm_rewards_usd), (Some(true), Some(1.84)));
        assert_eq!(evm.farm_rewards.as_ref().unwrap().token.as_deref(), Some("CAKE"));
        assert_eq!((evm.deposited_usd, evm.bin_step, evm.is_full_range), (Some(100.0), Some(1), Some(false)));
        assert_eq!(evm.fee_apr_pct, None);
    }

    #[test]
    fn a_base_unit_amount_that_is_not_a_whole_number_is_a_decode_error() {
        assert!(serde_json::from_str::<TokenAmount>(r#"{"raw":"1.5","decimals":18}"#).is_err());
        assert!(serde_json::from_str::<TokenAmount>(r#"{"raw":true,"decimals":18}"#).is_err());
        let absent: TokenAmount = serde_json::from_str(r#"{"decimals":18}"#).unwrap();
        assert_eq!(absent.raw, None);
        let null: TokenAmount = serde_json::from_str(r#"{"raw":null,"decimals":18}"#).unwrap();
        assert_eq!(null.raw, None);
    }

    #[tokio::test]
    async fn pause_resume_and_stake_on_an_evm_agent() {
        for (pause, word) in [(true, "pause"), (false, "resume")] {
            let (base, server) = one_shot(fx!("agent.json"));
            let c = Client::new(base, "ponk_live_k").unwrap();
            let agent = if pause { c.pause_agent(EVM_AGENT).await } else { c.resume_agent(EVM_AGENT).await }.unwrap();
            assert_eq!(
                request_line(&server.join().unwrap()),
                format!("POST /v1/agents/{EVM_AGENT}/{word} HTTP/1.1")
            );
            assert_eq!(agent.chain.as_deref(), Some("bsc"));
        }
        let (base, server) = one_shot(fx!("agent.json"));
        Client::new(base, "ponk_live_k").unwrap().set_stake_rewards(EVM_AGENT, false).await.unwrap();
        let raw = server.join().unwrap();
        assert_eq!(request_line(&raw), format!("PATCH /v1/agents/{EVM_AGENT}/config HTTP/1.1"));
        assert_eq!(body_of(&raw), serde_json::json!({ "evm": { "stake": false } }));
    }

    #[tokio::test]
    async fn an_evm_exit_returns_the_real_amounts() {
        let (base, server) = one_shot(fx!("exit.json"));
        let out = Client::new(base, "ponk_live_k").unwrap().exit_agent(EVM_AGENT, None).await.unwrap();
        let raw = server.join().unwrap();
        assert_eq!(request_line(&raw), format!("POST /v1/agents/{EVM_AGENT}/exit HTTP/1.1"));
        assert_eq!(body_of(&raw), serde_json::json!({}));
        assert_eq!((out.chain.as_deref(), out.status.as_deref(), out.lamports), (Some("bsc"), Some("completed"), Some(0)));
        assert_eq!(out.destination.as_deref(), Some(EVM_WITHDRAW));
        assert_eq!(out.amount_wei.as_deref(), Some("149000000000000000"));
        assert_eq!(crate::evm::wei_to_native(out.amount_wei.as_deref().unwrap()).as_deref(), Some("0.149"));
        assert_eq!(out.tx_hashes.len(), 2);
        assert_eq!(out.signature.as_ref(), out.tx_hashes.last());
        let (token, native) = (&out.received[0], &out.received[1]);
        assert_eq!(token.amount.as_deref(), Some("50120000000000000000"));
        assert_eq!(token.tx_hash.as_ref(), out.tx_hashes.first());
        assert!(!crate::evm::parse_evm_id(token.token.as_deref().unwrap()).unwrap().is_native());
        assert!(crate::evm::parse_evm_id(native.token.as_deref().unwrap()).unwrap().is_native());
        assert_eq!(native.amount, out.amount_wei);
        assert!(out.sentence.unwrap().contains("reached your withdrawal address"));

        let running: Withdrawal = serde_json::from_str(fx!("exit_incomplete.json")).unwrap();
        assert_eq!(running.status.as_deref(), Some("incomplete"));
        assert_eq!((running.signature, running.amount_wei), (None, None));
        assert!(running.tx_hashes.is_empty() && running.received.is_empty());
        assert!(running.sentence.unwrap().contains("keeps running the exit by itself"));
    }

    #[tokio::test]
    async fn an_evm_withdraw_uses_the_same_route_and_a_refusal_is_a_sentence() {
        let (base, server) = one_shot(fx!("exit.json"));
        let out = Client::new(base, "ponk_live_k").unwrap().withdraw(EVM_AGENT, None).await.unwrap();
        let raw = server.join().unwrap();
        assert_eq!(request_line(&raw), format!("POST /v1/agents/{EVM_AGENT}/withdraw HTTP/1.1"));
        assert_eq!(body_of(&raw), serde_json::json!({}));
        assert_eq!(out.chain.as_deref(), Some("bsc"));

        let (base, _server) = one_shot_status(
            422,
            r#"{"error":{"code":"unprocessable","message":"This agent has an open position. Exit it to take everything out."}}"#,
        );
        let err = Client::new(base, "ponk_live_k").unwrap().withdraw(EVM_AGENT, None).await.unwrap_err();
        assert!(err.api().unwrap().message.contains("Exit it"));

        // A Solana withdrawal reads as it did.
        let solana: Withdrawal = serde_json::from_str(r#"{"signature":"5vJz","lamports":1500000000,"pnl":null}"#).unwrap();
        assert_eq!((solana.lamports, solana.chain, solana.amount_wei), (Some(1_500_000_000), None, None));
        assert!(solana.received.is_empty() && solana.tx_hashes.is_empty());
    }

    #[tokio::test]
    async fn an_evm_plan_states_dollars_and_chains() {
        let (base, server) = one_shot(fx!("plan.json"));
        let plan = Client::new(base, "ponk_live_k")
            .unwrap()
            .copilot_evm_plan("$100 in bnb and $50 in ethereum")
            .await
            .unwrap();
        let raw = server.join().unwrap();
        assert_eq!(request_line(&raw), "POST /copilot/evm/plans HTTP/1.1");
        assert!(has_key(&raw), "{raw}");
        assert_eq!(body_of(&raw), serde_json::json!({ "text": "$100 in bnb and $50 in ethereum" }));
        assert_eq!((plan.state.as_deref(), plan.total_usd.as_deref()), (Some("ready"), Some("150.00")));
        assert_eq!(plan.chains(), vec!["bsc", "ethereum"]);
        let bsc = &plan.funding_by_chain[0];
        assert_eq!(
            (bsc.amount_usd.as_deref(), bsc.amount_native.as_deref(), bsc.native_symbol.as_deref()),
            (Some("100.00"), Some("0.146918"), Some("BNB"))
        );
        let legs = plan.legs();
        assert_eq!(legs.len(), 1);
        let leg = legs[0];
        assert_eq!((leg.dex.as_deref(), leg.pool_address.as_deref()), (Some("pancakeswap_v3"), Some(EVM_POOL)));
        assert_eq!(
            (leg.fund_usd.as_deref(), leg.fund_native.as_deref(), leg.fund_native_wei.as_deref()),
            (Some("100.00"), Some("0.146918"), Some("146918000000000000"))
        );
        assert_eq!((leg.deploy_usd.as_deref(), leg.range_pct), (Some("99.69"), Some(5.0)));
        assert_eq!(
            leg.config.as_ref().unwrap(),
            &crate::evm::range_config(0.05, 0.05, "bin_rebalancer", None).unwrap()
        );
        // The key's own rates, in the server's sentence.
        let fees = plan.fees.as_deref().unwrap();
        assert!(fees.contains("1.5%") && fees.contains("15%"), "{fees}");
        // A share under the minimum says what works instead.
        let eth = &plan.funding_by_chain[1];
        assert!(eth.legs.is_empty());
        assert!(eth.refusal.as_deref().unwrap().starts_with("$50 is not enough for one Ethereum pool"));
        assert_eq!(eth.minimum_usd.as_deref(), Some("320"));
        let alt = eth.alternative.as_ref().unwrap();
        assert_eq!((alt.kind.as_deref(), alt.chain.as_deref()), (Some("same_pair_on_chain"), Some("base")));
        assert!(alt.plan_id.is_some());
        assert_eq!(plan.dropped[0].chain.as_deref(), Some("ethereum"));
    }

    #[tokio::test]
    async fn a_refused_evm_plan_is_an_answer_not_an_error() {
        let (base, _server) = one_shot(
            r#"{"id":"p","state":"refused","text":"0.5 bnb in bnb pools","refusal":{"code":"coin_amount","message":"Say the amount in dollars."},"funding_by_chain":[],"fees":"f"}"#,
        );
        let plan = Client::new(base, "ponk_live_k").unwrap().copilot_evm_plan("0.5 bnb in bnb pools").await.unwrap();
        assert_eq!(plan.state.as_deref(), Some("refused"));
        assert_eq!(plan.refusal.unwrap()["code"], "coin_amount");
        assert!(plan.funding_by_chain.is_empty());
    }

    #[tokio::test]
    async fn an_evm_plan_is_read_and_started() {
        let pid = "0190a1b2-aaaa-7000-8000-0000000000aa";
        let (base, server) = one_shot(fx!("plan.json"));
        Client::new(base, "ponk_live_k").unwrap().copilot_evm_get_plan(pid).await.unwrap();
        assert_eq!(request_line(&server.join().unwrap()), format!("GET /copilot/evm/plans/{pid} HTTP/1.1"));

        let (base, server) = one_shot(fx!("plan_started.json"));
        let started = Client::new(base, "ponk_live_k").unwrap().copilot_evm_start(pid).await.unwrap();
        let raw = server.join().unwrap();
        assert_eq!(request_line(&raw), format!("POST /copilot/evm/plans/{pid}/start HTTP/1.1"));
        assert!(has_key(&raw), "{raw}");
        assert_eq!(started.state.as_deref(), Some("started"));
        assert_eq!(started.fleet_id.as_deref(), Some("0190a1b2-cccc-7000-8000-0000000000cc"));
        let (made, refused) = (&started.agents[0], &started.agents[1]);
        assert_eq!((made.agent_id.as_deref(), made.created), (Some(EVM_AGENT), Some(true)));
        assert_eq!((made.fund_native.as_deref(), made.native_symbol.as_deref()), (Some("0.146918"), Some("BNB")));
        assert_eq!((refused.agent_id.as_deref(), refused.created), (None, Some(false)));
        assert!(refused.refusal.as_deref().unwrap().contains("token list"));
        // The server's link is the one this crate builds.
        assert_eq!(started.sign_urls, vec![crate::evm::fund_link(EVM_AGENT, Some("bsc")).unwrap()]);
    }

    #[tokio::test]
    async fn the_evm_fleet_says_what_each_agent_needs() {
        let (base, server) = one_shot(fx!("fleet.json"));
        let fleet = Client::new(base, "ponk_live_k").unwrap().evm_fleet().await.unwrap();
        let raw = server.join().unwrap();
        assert_eq!(request_line(&raw), "GET /agents/evm/fleet HTTP/1.1");
        assert!(has_key(&raw), "{raw}");
        let (ready, new) = (&fleet.agents[0], &fleet.agents[1]);
        assert_eq!((ready.agent_id.as_deref(), ready.chain.as_deref()), (Some(EVM_AGENT), Some("bsc")));
        assert_eq!((ready.needs.as_deref(), ready.mandate_status.as_deref()), (Some("nothing"), Some("active")));
        assert_eq!((new.chain.as_deref(), new.address.as_deref(), new.mandate_status.as_deref()), (Some("base"), None, None));
        assert_eq!(new.needs.as_deref(), Some("authorize_and_fund"));
        let agents: Vec<crate::evm::FundLinkAgent> = fleet
            .agents
            .iter()
            .map(|a| crate::evm::FundLinkAgent::new(a.agent_id.clone().unwrap()).chain(a.chain.clone().unwrap()))
            .collect();
        assert_eq!(fleet.sign_urls, crate::evm::fund_links(&agents, false));
        assert_eq!(fleet.fund_page.as_deref(), Some("https://ponk.exchange/agents/fund"));
    }

    /// Opt in, read only, no key: `PONK_LIVE_TEST=1 cargo test live_evm -- --ignored --nocapture`
    /// lists pools on BNB Chain and Ethereum through the public route.
    #[tokio::test]
    #[ignore = "reads the live public pool boards; set PONK_LIVE_TEST=1"]
    async fn live_evm_boards_answer() {
        if std::env::var("PONK_LIVE_TEST").as_deref() != Ok("1") {
            eprintln!("skipped: set PONK_LIVE_TEST=1 to read the live boards");
            return;
        }
        let c = Client::public(DEFAULT_BASE_URL).unwrap();
        for chain in ["bsc", "ethereum"] {
            let board = c.evm_pools(chain, None).await.unwrap();
            assert_eq!(board.chain.as_deref(), Some(chain));
            assert!(board.ready(), "{:?}", board.status);
            assert!(!board.pools.is_empty());
            let mut venues = std::collections::BTreeMap::new();
            for p in &board.pools {
                let id = crate::evm::parse_evm_id(p.address.as_deref().unwrap()).unwrap();
                assert_eq!(id.chain, chain);
                let dex = p.dex.as_deref().unwrap();
                assert!(crate::evm::venue_chains(dex).contains(&chain), "{dex}");
                *venues.entry(dex.to_string()).or_insert(0u32) += 1;
            }
            let fees = |p: &EvmPool| p.fees.as_ref().and_then(|f| f.h24).unwrap_or(0.0);
            let top = board.pools.iter().max_by(|a, b| fees(a).total_cmp(&fees(b))).unwrap();
            println!(
                "{chain}: {} pools {venues:?}; most LP fees in 24h: {} on {}, fee tier {}, TVL ${:.0}, fees 24h ${:.0}, farm {}",
                board.pools.len(),
                top.pair(),
                top.dex.as_deref().unwrap_or("-"),
                match (top.dynamic_fee, top.fee_pct) {
                    (Some(true), _) => "dynamic".to_string(),
                    (_, Some(f)) => format!("{f}%"),
                    _ => "-".to_string(),
                },
                top.tvl.unwrap_or(0.0),
                fees(top),
                top.farm.as_ref().map(|f| format!("{:?} live={:?}", f.token, f.live)).unwrap_or_else(|| "none".into()),
            );
        }
    }
}
