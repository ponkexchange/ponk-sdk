//! Typed results for the ponk public API.
//!
//! Each type mirrors one response shape served by `apps/api/src/routes`. Field
//! names are the JSON keys, so a value here is the value the server sent,
//! never a derived or rounded one.
//!
//! Two conventions run through all of it:
//!
//! * Every struct is `#[serde(default)]`, so a field the server stops sending
//!   decodes as `None` instead of failing the whole call.
//! * `Option<String>` on a USD field is not laziness. Those figures arrive as
//!   decimal strings because a float would round money, and `None` means the
//!   figure could not be computed honestly, most often an unpriceable pool.
//!   `None` is unknown, never zero. Parse with your decimal type of choice.

use serde::{Deserialize, Serialize};

/// Health of the API and the two things it cannot work without.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Health {
    pub status: Option<String>,
    /// RFC 3339.
    pub time: Option<String>,
    pub checks: Option<HealthChecks>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HealthChecks {
    pub database: Option<ComponentStatus>,
    pub helius: Option<ComponentStatus>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ComponentStatus {
    pub healthy: Option<bool>,
    pub detail: Option<String>,
    pub latency_ms: Option<u32>,
}

/// One pool, decoded from its on-chain account.
///
/// `current_price` is RAW: token-Y base units per token-X base unit, NOT
/// decimal adjusted. [`PoolSnapshot::human_price`] does the conversion.
///
/// `tvl_usd`, `volume_24h` and `fees_24h` come from market data rather than
/// the account, and are `None` when that data is unavailable.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PoolSnapshot {
    pub dex: Option<String>,
    pub pool_address: Option<String>,
    pub token_x_mint: Option<String>,
    pub token_y_mint: Option<String>,
    pub token_x_decimals: Option<u8>,
    pub token_y_decimals: Option<u8>,
    pub bin_step: Option<u16>,
    pub active_bin_id: Option<i32>,
    pub current_price: Option<String>,
    pub tvl_usd: Option<String>,
    pub volume_24h: Option<String>,
    pub fees_24h: Option<String>,
    /// The pool's BASE swap-fee tier as a percent, for example `0.20` is
    /// 0.20%. On Meteora this is the base half only: a swap also pays a
    /// variable half that moves with volatility, so do not build a slippage
    /// floor out of this number.
    pub base_fee_pct: Option<f64>,
    /// RFC 3339.
    pub fetched_at: Option<String>,
}

impl PoolSnapshot {
    /// `current_price` as a human quote-per-base price.
    ///
    /// Returns `None` rather than a guess when the price or either decimals
    /// field is missing. `f64` is fine for display; it is not a money type,
    /// so do not settle a trade with it.
    pub fn human_price(&self) -> Option<f64> {
        let raw: f64 = self.current_price.as_deref()?.parse().ok()?;
        let dx = i32::from(self.token_x_decimals?);
        let dy = i32::from(self.token_y_decimals?);
        Some(raw * 10f64.powi(dx - dy))
    }
}

/// One fee rate, undiscounted and as actually charged to a wallet.
///
/// `base_*` is the list rate, `effective_*` is what this wallet pays right
/// now, and `holder_*` is what a qualifying $PONK holder pays whether or not
/// this wallet qualifies today.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FeeRates {
    pub base_bps: Option<u16>,
    pub effective_bps: Option<u16>,
    pub base_percent: Option<f64>,
    pub effective_percent: Option<f64>,
    pub holder_bps: Option<u16>,
    pub holder_percent: Option<f64>,
}

/// $PONK holder benefits for one wallet.
///
/// `api_agent_fee` is the rate an agent created through this API pays, which
/// is higher than `managed_agent_fee`, the rate for an agent created in the
/// app. The rate is fixed when the agent is created and follows it for life.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PonkPerks {
    pub mint: Option<String>,
    /// Whole PONK required to qualify.
    pub threshold_ui: Option<u64>,
    /// This wallet's live PONK balance, in whole PONK.
    pub balance_ui: Option<f64>,
    pub holds: Option<bool>,
    /// Whole PONK still needed to qualify. Zero when already qualifying.
    pub shortfall_ui: Option<f64>,
    /// Percent OFF the rate for holders, so `10` means a tenth off.
    pub discount_percent: Option<u16>,
    pub managed_agent_fee: Option<FeeRates>,
    pub api_agent_fee: Option<FeeRates>,
    pub non_custodial_agent_fee: Option<FeeRates>,
}

/// What a key is and what it may do.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct WhoAmI {
    /// The wallet that minted this key. Everything it can reach belongs to it.
    pub wallet_address: Option<String>,
    /// `read` or `trade`.
    pub scope: Option<String>,
    /// Which key answered, so several integrations can be told apart.
    pub key_id: Option<String>,
}

/// One agent.
///
/// `wallet_source` is `external` (you sign every action) or `managed` (the
/// agent signs autonomously under a custody mandate you signed once in the
/// app). `origin` is `web` or `api`, and it prices the agent.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Agent {
    pub id: Option<String>,
    pub name: Option<String>,
    pub wallet_address: Option<String>,
    /// `meteora_dlmm`, `orca` or `ponk_clouds`.
    pub dex: Option<String>,
    /// `None` for an entry agent that has not yet screened and bound a pool.
    pub pool_address: Option<String>,
    pub position_address: Option<String>,
    pub status: Option<String>,
    pub strategy: Option<String>,
    pub dry_run: Option<bool>,
    /// Strategy-specific configuration, exactly as stored.
    pub config: Option<serde_json::Value>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    /// `manual` or `auto`.
    pub mode: Option<String>,
    pub wallet_source: Option<String>,
    pub origin: Option<String>,
    /// The performance fee this agent's managed skim actually charges, in
    /// basis points, before any $PONK holder discount. Zero for a
    /// non-managed agent, which pays a per-action platform fee instead.
    pub managed_fee_bps: Option<u16>,
}

/// Live value, PnL and fees for one agent.
///
/// The canonical decomposition is Total = Price + Fee: `pnl_usd` is the total,
/// `price_pnl_usd` the principal leg and `fee_pnl_usd` the yield leg of the
/// CURRENT position. `fees_earned_lifetime_usd` spans every position the agent
/// has run on its pool, so it does not reset when the agent recenters.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AgentPerformance {
    pub current_value_usd: Option<String>,
    pub cost_basis_usd: Option<String>,
    pub pnl_usd: Option<String>,
    /// PnL as a fraction of cost basis, so `0.123` is +12.3%.
    pub pnl_pct: Option<f64>,
    pub peak_value_usd: Option<String>,
    /// Drawdown from peak as a fraction in 0..1, zero when at or above peak.
    pub drawdown_pct: Option<f64>,
    pub fees_claimed_usd: Option<String>,
    pub platform_fees_paid_usd: Option<String>,
    /// True when the platform-fee total includes a row priced later rather
    /// than at confirm time, so it should be labelled an estimate.
    pub platform_fees_paid_is_estimate: Option<bool>,
    /// Confirmed claim rows, so a null USD total is never read as zero claims.
    pub fees_claim_count: Option<i64>,
    pub agent_realized_fees_usd: Option<String>,
    /// Realized, already-claimed fee dollars. Deliberately NOT summed into
    /// `pnl_usd`: for a managed agent the live value already contains them.
    pub realized_fees_usd: Option<String>,
    pub has_position: Option<bool>,
    pub wallet_source: Option<String>,
    pub status: Option<String>,
    pub updated_at: Option<String>,
    pub fees_earned_usd: Option<String>,
    /// Unclaimed fees accruing in the open position. Fees accrue before a
    /// claim, so this is the honest answer to "nothing has been claimed yet".
    pub fees_unclaimed_usd: Option<String>,
    pub deposits_usd: Option<String>,
    pub withdrawals_usd: Option<String>,
    pub price_pnl_usd: Option<String>,
    pub fee_pnl_usd: Option<String>,
    pub fees_earned_lifetime_usd: Option<String>,
    pub fees_claimed_lifetime_usd: Option<String>,
    pub fees_unclaimed_lifetime_usd: Option<String>,
    /// How many positions the lifetime aggregation spans, zero when unknown.
    pub positions_count: Option<i64>,
    /// Impermanent loss against HODL. `None` when the deposited composition is
    /// not available, never fabricated.
    pub il_usd: Option<String>,
    /// `meteora_datapi` (Meteora's own figures) or `oracle_estimate` (ponk's
    /// on-chain valuation).
    pub pnl_source: Option<String>,
}

/// One bin's slice of a position's liquidity.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct BinShare {
    pub bin_id: Option<i32>,
    /// This bin's liquidity as a fraction of the position's largest bin, so it
    /// plots directly as a bar height.
    pub weight: Option<f64>,
    pub is_active: Option<bool>,
    pub in_target_range: Option<bool>,
}

/// The agent's live on-chain range.
///
/// The `lower_bin_id` / `upper_bin_id` / `active_bin_id` triple is the ACTUAL
/// on-chain state; the `target_*` fields are the strategy's intent and are
/// `None` when no target has been recorded. Units are bins on Meteora and Ponk
/// Clouds, ticks on Orca, which is what `dex` is for.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AgentPosition {
    pub position_address: Option<String>,
    pub lower_bin_id: Option<i32>,
    pub upper_bin_id: Option<i32>,
    pub active_bin_id: Option<i32>,
    pub in_range: Option<bool>,
    /// When the position first went out of range, RFC 3339. `None` while in
    /// range, so a timer can be rendered without inventing a start time.
    pub out_of_range_since: Option<String>,
    pub distance_from_active_bin: Option<i32>,
    pub lower_price: Option<String>,
    pub upper_price: Option<String>,
    pub current_price: Option<String>,
    /// Half-band as a fraction, so `0.007` is +-0.70%.
    pub band_pct: Option<f64>,
    /// Where the current price sits within the range, clamped 0..1.
    pub price_position_pct: Option<f64>,
    pub target_lower_bin_id: Option<i32>,
    pub target_upper_bin_id: Option<i32>,
    pub target_lower_price: Option<String>,
    pub target_upper_price: Option<String>,
    /// `decision` (a recorded rebalance or open target) or `config` (derived
    /// from the configured width around the active bin).
    pub target_source: Option<String>,
    /// `up` or `down` for an asymmetric one-sided recovery target.
    pub recovery_direction: Option<String>,
    pub is_full_range: Option<bool>,
    pub dex: Option<String>,
    /// Real per-bin liquidity of this position. Empty on venues that do not
    /// surface it, never fabricated.
    pub bins: Vec<BinShare>,
    /// Recently observed pool prices, oldest first, in the same units as
    /// `current_price`. Process-local: empty after an API restart or before
    /// the agent's first tick. Every entry is a real observed price.
    pub price_samples: Vec<f64>,
    pub fees_unclaimed_usd: Option<String>,
}

/// The isolated wallet an autonomous agent owns.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AgentWallet {
    pub pubkey: Option<String>,
    pub status: Option<String>,
    pub lamports: Option<u64>,
    /// `lamports` rendered as SOL, for display.
    pub sol: Option<f64>,
    /// The native-SOL reserve the executor always holds back. A wallet at or
    /// below this cannot deploy, which is why an underfunded agent sits idle.
    pub min_open_lamports: Option<u64>,
    /// Clears that floor and leaves enough to open a small position.
    pub recommended_open_lamports: Option<u64>,
    /// `per_agent` for the agent's own wallet, `shared` for the legacy
    /// per-user wallet.
    pub scope: Option<String>,
}

/// A token amount in base units, with the mint's decimals.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TokenAmount {
    pub raw: Option<u128>,
    pub decimals: Option<u8>,
}

impl TokenAmount {
    /// Whole-token amount for display. `None` when either part is missing.
    ///
    /// `f64` loses precision on large amounts. Use `raw` and `decimals` for
    /// anything that has to be exact.
    pub fn to_f64(&self) -> Option<f64> {
        let raw = self.raw?;
        let decimals = self.decimals?;
        Some(raw as f64 / 10f64.powi(i32::from(decimals)))
    }
}

/// One LP position held by the key's wallet, agent-managed or not.
///
/// `claimable_fees_usd` is only the currently unclaimed slice sitting on the
/// position. `fee_pnl_usd` is LIFETIME fees earned, claimed plus unclaimed.
/// They are different numbers and must not be added together.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Position {
    pub id: Option<String>,
    pub dex: Option<String>,
    pub wallet_address: Option<String>,
    pub pool_address: Option<String>,
    pub position_address: Option<String>,
    pub token_x_amount: Option<String>,
    pub token_y_amount: Option<String>,
    pub token_x_decimals: Option<u8>,
    pub token_y_decimals: Option<u8>,
    pub liquidity: Option<String>,
    pub lower_bin_id: Option<i32>,
    pub upper_bin_id: Option<i32>,
    pub active_bin_id: Option<i32>,
    pub in_range: Option<bool>,
    pub distance_from_active_bin: Option<i32>,
    pub fees_claimable_x: Option<TokenAmount>,
    pub fees_claimable_y: Option<TokenAmount>,
    /// Live-priced principal value. `None` when no leg could be priced.
    pub value_usd: Option<String>,
    pub updated_at: Option<String>,
    pub cost_basis_usd: Option<f64>,
    /// True when the cost basis is only a first-observation stamp, because
    /// ponk did not open this position. Lifetime PnL then stays `None` rather
    /// than asserting a gain.
    pub cost_basis_estimated: Option<bool>,
    pub claimed_fees_usd: Option<f64>,
    pub pnl_usd: Option<f64>,
    pub pnl_pct: Option<f64>,
    pub price_pnl_usd: Option<f64>,
    pub fee_pnl_usd: Option<f64>,
    pub claimable_fees_usd: Option<f64>,
    pub rewards_claimable_usd: Option<f64>,
    pub il_usd: Option<f64>,
    pub pnl_source: Option<String>,
    /// True when this row's PnL is a value-weighted share of a multi-position
    /// pool's lifetime figure rather than an independently measured one.
    pub pnl_pool_share: Option<bool>,
    pub fee_apr_pct: Option<f64>,
    pub fees_usd_per_day: Option<f64>,
    pub pool_fee_pct: Option<f64>,
    pub lower_price: Option<String>,
    pub upper_price: Option<String>,
    pub current_price: Option<String>,
    pub band_pct: Option<f64>,
    pub price_position_pct: Option<f64>,
    /// Set only when this position belongs to one of your own autonomous
    /// agents.
    pub agent_id: Option<String>,
    pub agent_name: Option<String>,
}

/// One thing an agent did, or decided not to do.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ActionLog {
    pub id: Option<String>,
    pub agent_id: Option<String>,
    pub agent_name: Option<String>,
    pub action_type: Option<String>,
    pub status: Option<String>,
    /// Present once a transaction landed, which is what makes a row verifiable
    /// on a block explorer. A dry-run agent logs decisions here with none.
    pub tx_signature: Option<String>,
    pub error_message: Option<String>,
    pub decision_reason: Option<String>,
    pub created_at: Option<String>,
    pub completed_at: Option<String>,
    pub pnl_delta_usd: Option<String>,
    pub pnl_cumulative_usd: Option<String>,
}

/// One mint's leg of a claim payout.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ClaimPayoutToken {
    pub mint: Option<String>,
    /// Base units, as a string: a u64 of lamports exceeds what a JSON number
    /// can carry safely, and rounding a fund figure is not acceptable.
    pub amount: Option<String>,
    pub decimals: Option<u8>,
}

/// What a claim actually paid out to the owner's wallet.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ClaimPayout {
    /// Resolved by the server, never named by the request.
    pub destination: Option<String>,
    /// One per landed payout transaction: the SOL leg and each token leg are
    /// sent independently, so a partial payout reports what it moved.
    pub signatures: Vec<String>,
    pub sol_lamports: Option<String>,
    /// A list, not one summed amount: a stable/stable pool pays two different
    /// mints and one figure would be a balance of nothing.
    pub tokens: Vec<ClaimPayoutToken>,
}

/// The receipt for one executed action, such as a compound.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ActionReceipt {
    pub action_id: Option<String>,
    pub signature: Option<String>,
    /// The transaction outcome.
    pub status: Option<String>,
    /// The recorded action's own state.
    pub action_status: Option<String>,
    pub compute_units_consumed: Option<u64>,
    pub error_message: Option<String>,
    pub risk_approved: Option<bool>,
    pub risk_failed_checks: Vec<String>,
    /// `None` whenever nothing moved, which means the funds are still in the
    /// agent wallet. It must never be read as a payout that happened.
    pub payout: Option<ClaimPayout>,
}

/// The frozen final figures for an agent that has just exited.
///
/// Captured at exit time, because once the position is closed the live view of
/// it is empty.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ExitSnapshot {
    pub agent_id: Option<String>,
    pub pnl_usd: Option<f64>,
    pub pnl_pct: Option<f64>,
    pub fees_lifetime_usd: Option<f64>,
    pub il_usd: Option<f64>,
    pub capital_usd: Option<f64>,
    pub swap_fee_bps: Option<i32>,
    pub bin_step: Option<i32>,
    pub venue: Option<String>,
    pub referral_code: Option<String>,
    pub opened_at: Option<String>,
    pub closed_at: Option<String>,
    pub exit_signature: Option<String>,
}

/// The result of a withdraw or an exit.
///
/// `destination` is server-locked to the wallet that owns the agent: there is
/// no destination parameter and there cannot be one.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Withdrawal {
    pub signature: Option<String>,
    pub lamports: Option<u64>,
    pub destination: Option<String>,
    pub source: Option<String>,
    /// Present only on an exit that closed a live managed position.
    pub pnl: Option<ExitSnapshot>,
}

/// The body of `POST /v1/agents`.
///
/// `wallet_address` must be the key's own wallet; anything else is rejected as
/// a cross-account attempt. `config` is validated against `strategy`, so a
/// mismatched shape comes back as a field-precise 400 rather than a broken
/// agent.
#[derive(Debug, Clone, Default, Serialize)]
pub struct NewAgent {
    pub name: String,
    pub wallet_address: String,
    /// `meteora_dlmm`, `orca` or `ponk_clouds`.
    pub dex: String,
    pub strategy: String,
    /// Strategy-specific configuration, matching the declared `strategy`.
    pub config: serde_json::Value,
    /// Required by every managing strategy. An entry agent may start without
    /// one and bind a pool by screening.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_address: Option<String>,
    /// Left unset, the server's own default applies, which is `true`. A
    /// dry-run agent runs its whole strategy loop and logs every decision
    /// without sending a transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dry_run: Option<bool>,
}

impl NewAgent {
    /// The five fields every agent needs.
    pub fn new(
        name: impl Into<String>,
        wallet_address: impl Into<String>,
        dex: impl Into<String>,
        strategy: impl Into<String>,
        config: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            wallet_address: wallet_address.into(),
            dex: dex.into(),
            strategy: strategy.into(),
            config,
            pool_address: None,
            position_address: None,
            dry_run: None,
        }
    }

    pub fn pool_address(mut self, pool: impl Into<String>) -> Self {
        self.pool_address = Some(pool.into());
        self
    }

    pub fn position_address(mut self, position: impl Into<String>) -> Self {
        self.position_address = Some(position.into());
        self
    }

    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = Some(dry_run);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_null_usd_field_stays_none() {
        let perf: AgentPerformance =
            serde_json::from_str(r#"{"pnl_usd":null,"current_value_usd":"1234.56"}"#).unwrap();
        assert!(perf.pnl_usd.is_none());
        assert_eq!(perf.current_value_usd.as_deref(), Some("1234.56"));
    }

    #[test]
    fn an_unknown_field_does_not_break_decoding() {
        let agent: Agent =
            serde_json::from_str(r#"{"id":"a","name":"n","brand_new_field":7}"#).unwrap();
        assert_eq!(agent.id.as_deref(), Some("a"));
    }

    #[test]
    fn perks_are_camel_case() {
        let perks: PonkPerks = serde_json::from_str(
            r#"{"mint":"M","thresholdUi":1000000,"holds":false,
                "apiAgentFee":{"baseBps":1500,"holderPercent":13.5}}"#,
        )
        .unwrap();
        assert_eq!(perks.threshold_ui, Some(1_000_000));
        assert_eq!(perks.holds, Some(false));
        assert_eq!(perks.api_agent_fee.unwrap().holder_percent, Some(13.5));
    }

    #[test]
    fn a_pool_price_is_decimal_adjusted() {
        // A SOL(9)/USDC(6) pool at $150: raw is USDC base units per SOL base
        // unit, so the human price is raw * 10 ^ (9 - 6).
        let pool: PoolSnapshot = serde_json::from_str(
            r#"{"current_price":"0.15","token_x_decimals":9,"token_y_decimals":6}"#,
        )
        .unwrap();
        assert_eq!(pool.human_price(), Some(150.0));
    }

    #[test]
    fn a_pool_price_is_none_when_an_input_is_missing() {
        let pool: PoolSnapshot = serde_json::from_str(r#"{"current_price":"0.15"}"#).unwrap();
        assert_eq!(pool.human_price(), None);
    }

    #[test]
    fn a_new_agent_omits_what_was_not_set() {
        let body = NewAgent::new(
            "n",
            "W",
            "meteora_dlmm",
            "bin_rebalancer",
            serde_json::json!({}),
        )
        .pool_address("POOL");
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["pool_address"], "POOL");
        assert!(json.get("dry_run").is_none());
        assert!(json.get("position_address").is_none());
    }

    #[test]
    fn a_claimable_fee_decodes_with_its_decimals() {
        let pos: Position = serde_json::from_str(
            r#"{"id":"p","fees_claimable_x":{"raw":1234567,"decimals":6},"claimable_fees_usd":null}"#,
        )
        .unwrap();
        let x = pos.fees_claimable_x.unwrap();
        assert_eq!(x.raw, Some(1_234_567));
        assert_eq!(x.to_f64(), Some(1.234567));
        assert!(pos.claimable_fees_usd.is_none());
    }

    #[test]
    fn a_position_range_carries_its_bins() {
        let pos: AgentPosition = serde_json::from_str(
            r#"{"position_address":"P","in_range":false,"dex":"meteora_dlmm",
                "bins":[{"bin_id":1,"weight":0.5,"is_active":true,"in_target_range":true}],
                "price_samples":[1.0,1.1]}"#,
        )
        .unwrap();
        assert_eq!(pos.bins.len(), 1);
        assert_eq!(pos.bins[0].is_active, Some(true));
        assert_eq!(pos.price_samples, vec![1.0, 1.1]);
        assert_eq!(pos.in_range, Some(false));
    }
}
