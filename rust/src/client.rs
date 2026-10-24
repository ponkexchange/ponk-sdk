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
    /// [`pool`], [`range_cost`] and [`ponk_perks`]. Every other method will
    /// get a 401.
    ///
    /// [`health`]: Client::health
    /// [`pool`]: Client::pool
    /// [`range_cost`]: Client::range_cost
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
    /// `dex` is `meteora_dlmm`, `orca`, `ponk_clouds` or `raydium`. This
    /// route is rate limited per IP (a 429 past the limit): it spends the same
    /// RPC budget the live agents use.
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
    /// Meteora DLMM autonomous agents only. The range is read from chain, so
    /// the re-deposit cannot be redirected.
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

    // -- transport ---------------------------------------------------------

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let request = self.http.get(self.url(path)).timeout(self.timeout);
        self.send(request, path, &[]).await
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
}
