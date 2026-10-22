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
}
