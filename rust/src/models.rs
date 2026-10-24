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
    /// `meteora_dlmm`, `orca`, `ponk_clouds` or `raydium`.
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
    /// Native SOL a withdraw would actually send right now.
    pub withdrawable_lamports: Option<u64>,
    /// Token accounts holding a balance a sweep would move. `None` when the
    /// read failed: treat tokens as unknown then, never as zero.
    pub token_accounts_with_value: Option<u64>,
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

/// The custody mandate an autonomous agent is signing under.
///
/// Read from the stored, signed scope, so it is what the owner actually
/// signed, not a default. The mandate expires on a clock and nothing on chain
/// records the moment it lapses: an agent whose mandate expired keeps ticking
/// but every signature it asks for is denied, which looks exactly like an
/// agent with nothing to do. `status` and `seconds_remaining` are how you tell
/// the two apart.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Mandate {
    pub mandate_id: Option<String>,
    /// `active` while it can still sign, `expired` once the clock has passed
    /// `expires_at`. Computed by the server at request time.
    pub status: Option<String>,
    /// RFC 3339.
    pub expires_at: Option<String>,
    /// RFC 3339.
    pub granted_at: Option<String>,
    /// Signed: negative once the mandate has lapsed.
    pub seconds_remaining: Option<i64>,
    /// The signed scope, verbatim, for example `claim_fees`.
    pub actions: Vec<String>,
    /// `None` means no per-action USD ceiling, not a ceiling of zero.
    pub max_action_value_usd: Option<u64>,
    pub max_daily_transactions: Option<u32>,
    pub max_slippage_bps: Option<u16>,
    pub pool_allowlist: Vec<String>,
    /// The address every withdrawal is locked to.
    pub withdrawal_address: Option<String>,
}

/// The agent's custody state after a `set_autonomous` call, read back from
/// its row rather than assumed.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AutonomousView {
    pub agent_id: Option<String>,
    /// `managed` once autonomous, `external` once turned off.
    pub wallet_source: Option<String>,
    /// `auto` or `manual`.
    pub mode: Option<String>,
    pub managed_wallet: Option<String>,
    pub mandate_id: Option<String>,
    /// The address every withdrawal is locked to.
    pub withdrawal_locked_to: Option<String>,
    /// The agent's lifecycle status after the call. `managed` plus `auto` does
    /// not by itself mean a loop is running: an agent a previous exit left
    /// `stopped` is autonomous and still needs [`resume_agent`].
    ///
    /// [`resume_agent`]: crate::Client::resume_agent
    pub status: Option<String>,
    /// Whether the agent simulates instead of executing after the call.
    /// Enabling autonomy through `/v1` always sets this to `false`.
    pub dry_run: Option<bool>,
}

/// What opening a Meteora DLMM range of a given width costs up front.
///
/// Rent for the position account comes back when the position closes. Rent
/// for bin arrays this wallet would have to create does not, because closing
/// one requires every LP in it to have left. Bin arrays that already exist on
/// chain cost nothing, which is why an established pool quotes lower than a
/// cold one.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RangeCostQuote {
    pub pool_address: Option<String>,
    pub dex: Option<String>,
    pub bin_step: Option<u16>,
    pub active_bin_id: Option<i32>,
    /// What was asked for, before the venue clamp.
    pub requested_bins: Option<u32>,
    /// What one position will actually hold.
    pub total_bins: Option<u32>,
    /// The venue's ceiling on one position.
    pub max_total_bins: Option<u32>,
    /// True when `total_bins < requested_bins`.
    pub clamped: Option<bool>,
    pub lower_bin_id: Option<i32>,
    pub upper_bin_id: Option<i32>,
    /// Distance of the lower edge from the current price, in percent.
    /// Negative.
    pub down_pct: Option<f64>,
    /// Distance of the upper edge from the current price, in percent.
    pub up_pct: Option<f64>,
    /// Refundable on close.
    pub position_rent_lamports: Option<u64>,
    pub bin_arrays_total: Option<u32>,
    pub bin_arrays_existing: Option<u32>,
    pub bin_arrays_missing: Option<u32>,
    /// Treat as not refundable.
    pub bin_array_rent_lamports: Option<u64>,
    /// `probed` when every bin array was read on chain. `unavailable` when
    /// the read failed, in which case `bin_arrays_existing` is 0 by assumption
    /// and the figures are the cold-pool worst case.
    pub bin_array_probe: Option<String>,
    /// `increase_position_length` transactions needed to reach `total_bins`.
    pub resize_tx_count: Option<u32>,
    /// Signature fees for the open plus every resize.
    pub tx_fee_lamports: Option<u64>,
    /// Everything the wallet must hold before the open can start.
    pub total_upfront_lamports: Option<u64>,
    pub refundable_lamports: Option<u64>,
    pub non_refundable_lamports: Option<u64>,
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
    /// `meteora_dlmm`, `orca`, `ponk_clouds` or `raydium`. An `entry_agent`
    /// (Degen Mode in the app) discovers its pool by screening, which works
    /// only on `meteora_dlmm`, and its config may carry a bin step filter,
    /// `min_bin_step` and `max_bin_step` (each 1 to 1000).
    pub dex: String,
    pub strategy: String,
    /// Strategy-specific configuration, matching the declared `strategy`.
    pub config: serde_json::Value,
    /// Required by every managing strategy. An `entry_agent` starts poolless
    /// and binds a pool by screening, so for it this and `position_address`
    /// are ignored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position_address: Option<String>,
    /// Left unset, `/v1` creates a LIVE agent (`dry_run = false`). Set
    /// `Some(true)` to simulate: a dry-run agent runs its whole strategy loop
    /// and logs every decision without sending a transaction. This is the
    /// opposite of the session route `POST /agents`, which defaults to dry
    /// run; `/v1` defaults to live on purpose, and refuses a non-boolean
    /// `dry_run` with a 400.
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
    fn an_unlimited_mandate_ceiling_stays_none() {
        let m: Mandate = serde_json::from_str(
            r#"{"mandate_id":"m","status":"expired","expires_at":"2026-10-01T00:00:00Z",
                "granted_at":"2026-09-01T00:00:00Z","seconds_remaining":-3600,
                "actions":["claim_fees","compound"],"max_action_value_usd":null,
                "max_daily_transactions":50,"max_slippage_bps":100,
                "pool_allowlist":[],"withdrawal_address":"W"}"#,
        )
        .unwrap();
        assert_eq!(m.status.as_deref(), Some("expired"));
        assert_eq!(m.seconds_remaining, Some(-3600));
        assert!(m.max_action_value_usd.is_none());
        assert_eq!(m.actions, vec!["claim_fees", "compound"]);
    }

    #[test]
    fn an_autonomous_view_reports_status_and_dry_run() {
        let v: AutonomousView = serde_json::from_str(
            r#"{"agent_id":"a","wallet_source":"managed","mode":"auto","managed_wallet":"M",
                "mandate_id":"m","withdrawal_locked_to":"W","status":"stopped","dry_run":false}"#,
        )
        .unwrap();
        assert_eq!(v.status.as_deref(), Some("stopped"));
        assert_eq!(v.dry_run, Some(false));
    }

    #[test]
    fn a_range_cost_quote_decodes() {
        let q: RangeCostQuote = serde_json::from_str(
            r#"{"pool_address":"P","dex":"meteora_dlmm","bin_step":10,"active_bin_id":5,
                "requested_bins":2000,"total_bins":1400,"max_total_bins":1400,"clamped":true,
                "lower_bin_id":-695,"upper_bin_id":704,"down_pct":-50.1,"up_pct":101.2,
                "position_rent_lamports":1,"bin_arrays_total":21,"bin_arrays_existing":3,
                "bin_arrays_missing":18,"bin_array_rent_lamports":2,"bin_array_probe":"probed",
                "resize_tx_count":20,"tx_fee_lamports":3,"total_upfront_lamports":6,
                "refundable_lamports":1,"non_refundable_lamports":5}"#,
        )
        .unwrap();
        assert_eq!(q.clamped, Some(true));
        assert_eq!(q.bin_array_probe.as_deref(), Some("probed"));
        assert_eq!(q.total_bins, Some(1400));
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

// --------------------------------------------------------------------------
// Leaderboard, sharing and follows (0.4.0)
// --------------------------------------------------------------------------

/// What a copy of a shared agent starts from: venue, pool, strategy and the
/// strategy config with anything owner-specific removed.
/// [`AgentCopy::to_new_agent`] turns it into a create body.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AgentCopy {
    pub dex: Option<String>,
    pub pool_address: Option<String>,
    pub strategy: Option<String>,
    pub config: serde_json::Value,
}

impl AgentCopy {
    /// A [`NewAgent`] on YOUR wallet with this copy's venue, pool, strategy
    /// and config. `None` when the copy carries no strategy or venue.
    pub fn to_new_agent(
        &self,
        name: impl Into<String>,
        wallet_address: impl Into<String>,
    ) -> Option<NewAgent> {
        let dex = self.dex.clone().filter(|s| !s.is_empty())?;
        let strategy = self.strategy.clone().filter(|s| !s.is_empty())?;
        let config = if self.config.is_object() {
            self.config.clone()
        } else {
            serde_json::json!({ "kind": strategy })
        };
        let mut agent = NewAgent::new(name, wallet_address, dex, strategy, config);
        agent.pool_address = self.pool_address.clone();
        Some(agent)
    }
}

/// One row of the Top Ponkillionaires board.
///
/// `rank` is `None` on an unranked row and `unranked_reason` says why. USD
/// figures are decimal strings, `None` when the link hides USD or the figure
/// cannot be priced. Nested blocks the board may grow are kept as JSON.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LeaderboardRow {
    pub rank: Option<i64>,
    pub slug: Option<String>,
    pub dex: Option<String>,
    pub strategy: Option<String>,
    pub pool_address: Option<String>,
    pub token_x_mint: Option<String>,
    pub token_y_mint: Option<String>,
    pub running_since: Option<String>,
    pub vs_hodl_pct: Option<f64>,
    pub score_pct: Option<f64>,
    pub pnl_pct: Option<f64>,
    pub drawdown_pct: Option<f64>,
    pub unranked_reason: Option<String>,
    pub owner_key: Option<String>,
    pub handle: Option<String>,
    pub usd_hidden: Option<bool>,
    pub window: Option<String>,
    pub pnl_usd: Option<String>,
    pub roi_pct: Option<f64>,
    pub basis_usd: Option<String>,
    pub fees_realized_usd: Option<String>,
    pub fees_earned_usd: Option<String>,
    pub fees_unclaimed_usd: Option<String>,
    pub deposited_usd: Option<String>,
    pub spark: Vec<Option<f64>>,
    pub followers: Option<i64>,
    pub crew: Option<String>,
    pub win_rate: Option<serde_json::Value>,
    pub copy: AgentCopy,
}

/// `GET /public/agents-leaderboard`: ranked and unranked shared agents.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Leaderboard {
    pub ranked: Vec<LeaderboardRow>,
    pub unranked: Vec<LeaderboardRow>,
    /// `24h`, `7d`, `30d` or `all`.
    pub window: Option<String>,
    pub min_days: Option<i64>,
    pub min_hodl_usd: Option<f64>,
    pub min_win_samples: Option<i64>,
    /// The board header (shared, ranked, LPers, crews, verified, summed
    /// figures), as sent.
    pub totals: Option<serde_json::Value>,
    pub computed_at: Option<String>,
}

/// `GET /public/agents/{slug}`: a shared agent's public profile.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PublicAgent {
    pub slug: Option<String>,
    pub usd_hidden: Option<bool>,
    pub shared_at: Option<String>,
    pub dex: Option<String>,
    pub pool_address: Option<String>,
    pub token_x_mint: Option<String>,
    pub token_y_mint: Option<String>,
    pub bin_step: Option<u16>,
    pub strategy: Option<String>,
    pub status: Option<String>,
    pub running_since: Option<String>,
    /// `None` with `performance_unavailable` naming the reason.
    pub performance: Option<serde_json::Value>,
    pub performance_unavailable: Option<String>,
    pub proof: Option<serde_json::Value>,
    pub counts: Option<serde_json::Value>,
    pub copy: AgentCopy,
    pub owner_key: Option<String>,
    pub handle: Option<String>,
    pub followers: Option<i64>,
    pub win_rate: Option<serde_json::Value>,
    pub crew: Option<String>,
    /// Present only when the owner turned on-chain proof on, and never beside
    /// a percent-only link.
    pub onchain: Option<serde_json::Value>,
    pub computed_at: Option<String>,
}

/// An agent's position value over time against holding its opening tokens.
/// A bucket with no reading is null in `points`, never interpolated.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ValueHistory {
    pub range: Option<String>,
    pub bucket_secs: Option<i64>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub points: Vec<serde_json::Value>,
    pub hodl_since: Option<String>,
    pub positions: Option<u64>,
}

/// An agent's live public link. `path` is site-relative (`/ponkillionaires/<slug>`), and
/// `show_onchain` is always false while `hide_usd` is set.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PublicLink {
    pub slug: Option<String>,
    pub path: Option<String>,
    pub hide_usd: Option<bool>,
    pub show_onchain: Option<bool>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LinkRevoked {
    pub revoked: Option<bool>,
}

/// The result of a follow or unfollow, with the agent's follower count.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Follow {
    pub slug: Option<String>,
    pub following: Option<bool>,
    pub followers: Option<i64>,
}

/// Slugs this account follows, newest first.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Follows {
    pub slugs: Vec<String>,
}

// --------------------------------------------------------------------------
// Points and referrals (0.4.0)
// --------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PointsSeason {
    pub id: Option<String>,
    pub name: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub multiplier: Option<String>,
}

/// This account's standing in the active season. `season` is `None` between
/// seasons. Points are decimal strings; `rank` is `None` until earned.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MyPoints {
    pub season: Option<PointsSeason>,
    pub total_points: Option<String>,
    pub tvl_time_points: Option<String>,
    pub fee_paid_points: Option<String>,
    pub referral_points: Option<String>,
    pub event_count: Option<i64>,
    pub rank: Option<i64>,
    pub participant_count: Option<i64>,
}

/// One points row. `display_code` is a referral code, never a wallet.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PointsEntry {
    pub rank: Option<i64>,
    pub display_code: Option<String>,
    pub total_points: Option<String>,
    pub tvl_time_points: Option<String>,
    pub fee_paid_points: Option<String>,
    pub referral_points: Option<String>,
    pub is_me: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PointsLeaderboard {
    pub season: Option<PointsSeason>,
    pub participant_count: Option<i64>,
    pub entries: Vec<PointsEntry>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ReferralCode {
    pub code: Option<String>,
    pub label: Option<String>,
    pub created_at: Option<String>,
}

/// Referral codes, handle and money. EARNED (`total_earned_usd`), OWED
/// (`owed_usd`) and UNPROVEN (`unproven_usd`) are three different facts:
/// never add them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Referral {
    pub code: Option<String>,
    pub codes: Vec<ReferralCode>,
    pub referred_by_code: Option<String>,
    pub referred_count: Option<i64>,
    pub total_payouts: Option<i64>,
    pub total_earned_usd: Option<f64>,
    pub back_paid_usd: Option<f64>,
    pub back_paid_events: Option<i64>,
    pub owed_usd: Option<f64>,
    pub owed_events: Option<i64>,
    pub owed_unpriced_events: Option<i64>,
    pub unproven_usd: Option<f64>,
    pub unproven_events: Option<i64>,
    pub unpriced_payouts: Option<i64>,
    pub creator_handle: Option<String>,
    pub referred_active_agents: Option<i64>,
    pub referred_lp_fees_usd: Option<f64>,
    pub handle: Option<String>,
    pub creator_application: Option<serde_json::Value>,
    /// Your referral tier now and the next rung.
    pub tier: Option<ReferralTier>,
    /// The whole ladder, so no client hardcodes a rate.
    pub programme: Option<ReferralProgramme>,
}

/// One rung of the referral ladder. `share_bps` is bps of ponk's platform fee.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ReferralTierStep {
    pub tier: Option<u8>,
    pub min_fees_usd: Option<u64>,
    pub share_bps: Option<u16>,
}

/// Your referral tier, from the platform fees your referred users paid over
/// the trailing `window_days`. One level only: nobody above you earns on the
/// people you referred. `referred_fees_usd` is `None` when it could not be
/// read; the share is then the base rate and no progress should be shown.
/// `next` is `None` at the top. Every share comes out of ponk's fee and never
/// changes what a user pays.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ReferralTier {
    pub current: Option<u8>,
    pub share_bps: Option<u16>,
    pub referred_fees_usd: Option<f64>,
    pub window_days: Option<i32>,
    pub next: Option<ReferralTierStep>,
}

/// The programme's constants, lowest rung first.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ReferralProgramme {
    pub ladder: Vec<ReferralTierStep>,
    pub window_days: Option<i32>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CreatorTotals {
    pub referred_wallets: Option<i64>,
    pub referred_with_active_agent: Option<i64>,
    pub active_agents: Option<i64>,
    pub lp_fees_usd: Option<f64>,
    pub ponk_fees_usd: Option<f64>,
    pub earned_usd: Option<f64>,
    pub fee_events: Option<i64>,
    pub unpriced_events: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CreatorDay {
    /// UTC calendar day, `YYYY-MM-DD`.
    pub day: Option<String>,
    pub signups: Option<i64>,
    pub lp_fees_usd: Option<f64>,
    pub ponk_fees_usd: Option<f64>,
    pub earned_usd: Option<f64>,
    pub fee_events: Option<i64>,
}

/// The Creator dashboard: totals and one row per UTC day, oldest first.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CreatorDashboard {
    pub handle: Option<String>,
    pub granted_at: Option<String>,
    pub totals: CreatorTotals,
    pub days: Vec<CreatorDay>,
}

/// Whether a `/r/<handle>` can be claimed; `reason` when it cannot.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HandleCheck {
    pub handle: Option<String>,
    pub available: Option<bool>,
    pub reason: Option<String>,
}

/// `GET /public/referral/{code}`: the code exists, and `creator` when it is a
/// Creator's handle. Reveals no wallet and no figure.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PublicReferral {
    pub code: Option<String>,
    pub creator: Option<bool>,
}

// -- live agents activity ----------------------------------------------------

/// One event on the live agents strip. It carries no identity at all.
///
/// `kind` is `open`, `rebalance`, `compound`, `claim`, `exit` or `hold`.
/// `coarse` means `at` was truncated to the start of its hour. `usd` is a
/// decimal string; `usd_kind` is `fees` (realized net fees of a compound or
/// claim) or `margin` (the rebalance gate's benefit minus cost on a hold).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ActivityEvent {
    pub key: Option<String>,
    pub kind: Option<String>,
    pub at: Option<String>,
    pub coarse: Option<bool>,
    pub dex: Option<String>,
    pub token_x_mint: Option<String>,
    pub token_y_mint: Option<String>,
    pub usd: Option<String>,
    pub usd_kind: Option<String>,
    pub reason_key: Option<String>,
    pub evaluations: Option<i64>,
    pub last_seen_at: Option<String>,
}

/// `GET /public/agents-activity`: the board agents' actions of the last 24
/// hours and every live hold. `holding` counts agents holding right now.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AgentsActivity {
    pub events: Vec<ActivityEvent>,
    pub holding: Option<i64>,
    pub computed_at: Option<String>,
}

// -- weekly Ponkillionaires --------------------------------------------------

/// A weekly prize: the agent fee rate for the following week
/// (`fee_rate_pct`, 2.0 means 2%) plus points.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WeekPrize {
    pub rank: Option<i64>,
    pub fee_rate_pct: Option<f64>,
    pub points: Option<i64>,
}

/// A row of this week's race. `prize` is what the rank would win if the week
/// ended now, `None` for a row in loss. `handle`, `display_name` and `wallet`
/// appear only when the owner chose to show them on the board.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WeekStanding {
    pub rank: Option<i64>,
    pub pnl_usd: Option<f64>,
    pub agents: Option<i64>,
    pub deployed_usd: Option<f64>,
    pub prize: Option<WeekPrize>,
    pub owner_key: Option<String>,
    pub handle: Option<String>,
    pub display_name: Option<String>,
    pub wallet: Option<String>,
}

/// A settled winner; the prize rate runs from `rate_from` to `rate_until`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WeekWinner {
    pub rank: Option<i64>,
    pub pnl_usd: Option<f64>,
    pub agents: Option<i64>,
    pub fee_rate_pct: Option<f64>,
    pub points: Option<f64>,
    pub rate_from: Option<String>,
    pub rate_until: Option<String>,
    pub owner_key: Option<String>,
    pub handle: Option<String>,
    pub display_name: Option<String>,
    pub wallet: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LastWeek {
    pub week_start: Option<String>,
    pub settled: Option<bool>,
    pub winners: Vec<WeekWinner>,
}

/// `GET /public/ponkillionaires/week`: this week's race for the top three and
/// last week's winners. Resets Monday 00:00 UTC (`resets_at`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PonkillionairesWeek {
    pub week_start: Option<String>,
    pub resets_at: Option<String>,
    pub prizes: Vec<WeekPrize>,
    pub min_peak_usd: Option<i64>,
    pub min_running_days: Option<i64>,
    pub standings: Vec<WeekStanding>,
    pub last_week: Option<LastWeek>,
    pub computed_at: Option<String>,
}

// -- X Tracker ---------------------------------------------------------------

/// A tracked X account. `handle` is lowercase; `screen_name` keeps its case.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XAccount {
    pub handle: Option<String>,
    pub screen_name: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub followers: Option<i64>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XTokenRef {
    pub mint: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XAuthor {
    pub screen_name: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XMetrics {
    pub views: Option<i64>,
    pub likes: Option<i64>,
    pub replies: Option<i64>,
    pub reposts: Option<i64>,
}

/// A post on the feed. `tokens` are the Solana mints it names.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XPost {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub url: Option<String>,
    pub text: Option<String>,
    pub created_at: Option<String>,
    pub seen_at: Option<String>,
    pub latency_secs: Option<i64>,
    pub is_backfill: Option<bool>,
    pub reply_to: Option<String>,
    pub author: Option<XAuthor>,
    pub media: Vec<serde_json::Value>,
    pub quote: Option<serde_json::Value>,
    pub card: Option<serde_json::Value>,
    pub metrics: Option<XMetrics>,
    pub tokens: Vec<XTokenRef>,
    pub cashtags: Vec<String>,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XEventPost {
    pub id: Option<String>,
    pub url: Option<String>,
    pub text: Option<String>,
    pub created_at: Option<String>,
    pub tokens: Vec<XTokenRef>,
}

/// A change on a tracked account, with `before` and `after` where they apply.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XEvent {
    pub id: Option<i64>,
    pub kind: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
    pub detail: Option<serde_json::Value>,
    pub at: Option<String>,
    pub post: Option<XEventPost>,
}

/// One feed row. `item_type` (the JSON `type`) is `post` or `event`, and the
/// matching field is set.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XFeedItem {
    #[serde(rename = "type")]
    pub item_type: Option<String>,
    pub key: Option<String>,
    pub account: Option<XAccount>,
    pub post: Option<XPost>,
    pub event: Option<XEvent>,
}

/// The feed's data source. `state` is `live`, `degraded`, `down` or `starting`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XSource {
    pub name: Option<String>,
    pub url: Option<String>,
    pub state: Option<String>,
    pub last_ok_at: Option<String>,
    pub last_error_at: Option<String>,
    pub last_error: Option<String>,
    pub requests_1h: Option<i64>,
    pub rate_limited_1h: Option<i64>,
    pub median_latency_secs: Option<i64>,
    pub accounts_fresh: Option<i64>,
    pub accounts_active: Option<i64>,
    pub hot_secs: Option<u64>,
    pub normal_secs: Option<u64>,
    pub cold_secs: Option<u64>,
}

/// A page of the X feed, newest first. Pass `next_before` back as `before`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XFeed {
    pub items: Vec<XFeedItem>,
    pub next_before: Option<String>,
    pub source: Option<XSource>,
}

/// The first tracked account to post a token, with the price when seen.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XFirstCall {
    pub handle: Option<String>,
    pub screen_name: Option<String>,
    pub avatar_url: Option<String>,
    pub post_id: Option<String>,
    pub url: Option<String>,
    pub at: Option<String>,
    pub price_usd: Option<f64>,
    pub mcap_usd: Option<f64>,
}

/// A token trending on X. `multiple` is price now over the first call's price.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XTrendingToken {
    pub mint: Option<String>,
    pub symbol: Option<String>,
    pub name: Option<String>,
    pub accounts_1h: Option<i64>,
    pub accounts_24h: Option<i64>,
    pub mentions_24h: Option<i64>,
    pub last_at: Option<String>,
    pub first_call: Option<XFirstCall>,
    pub price_usd: Option<f64>,
    pub mcap_usd: Option<f64>,
    pub multiple: Option<f64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XTrending {
    pub window: Option<String>,
    pub tokens: Vec<XTrendingToken>,
    pub computed_at: Option<String>,
}

/// A caller's record. Medians and `best` are price multiples after the call;
/// `hit_rate` is the share of 24h-marked calls that were up, as a fraction.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XCaller {
    pub account: Option<XAccount>,
    pub calls: Option<i64>,
    pub priced_calls: Option<i64>,
    pub median_1h: Option<f64>,
    pub median_24h: Option<f64>,
    pub best: Option<f64>,
    pub hit_rate: Option<f64>,
    pub scored_24h: Option<i64>,
    pub last_call_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XCallers {
    pub days: Option<i64>,
    pub callers: Vec<XCaller>,
    pub computed_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XTokenMention {
    pub account: Option<XAccount>,
    pub post_id: Option<String>,
    pub url: Option<String>,
    pub text: Option<String>,
    pub kind: Option<String>,
    pub at: Option<String>,
    pub price_usd: Option<f64>,
    pub mcap_usd: Option<f64>,
    pub deleted: Option<bool>,
}

/// Every tracked mention of one token, its first call and its multiple since.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XToken {
    pub mint: Option<String>,
    pub symbol: Option<String>,
    pub accounts_1h: Option<i64>,
    pub accounts_24h: Option<i64>,
    pub first_call: Option<XFirstCall>,
    pub price_usd: Option<f64>,
    pub multiple: Option<f64>,
    pub mentions: Vec<XTokenMention>,
}

/// A directory account. `status` is `active` or `not_found`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XDirectoryEntry {
    pub handle: Option<String>,
    pub screen_name: Option<String>,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub followers: Option<i64>,
    pub category: Option<String>,
    pub is_default: Option<bool>,
    pub status: Option<String>,
    pub last_ok_at: Option<String>,
    pub followed: Option<bool>,
    pub muted: Option<bool>,
    pub alerting: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct XDirectory {
    pub accounts: Vec<XDirectoryEntry>,
    pub hidden_categories: Vec<String>,
    pub categories: Vec<String>,
}

// -- Wallet Tracker ----------------------------------------------------------

/// Who a feed row belongs to. `source` is `ponkillionaire`, `agent`,
/// `notable`, `mine`, or `None` for a wallet read on demand.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TrackedWalletRef {
    pub address: Option<String>,
    pub label: Option<String>,
    pub emoji: Option<String>,
    pub color: Option<String>,
    pub source: Option<String>,
    pub agent_slug: Option<String>,
}

/// One token movement. `amount` is a base-unit string, `dir` is `in` or `out`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WalletLeg {
    pub mint: Option<String>,
    pub symbol: Option<String>,
    pub amount: Option<String>,
    pub decimals: Option<u8>,
    pub dir: Option<String>,
    pub usd: Option<f64>,
}

/// A swap or liquidity action by a tracked wallet, with its signature.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WalletEvent {
    pub key: Option<String>,
    pub signature: Option<String>,
    pub kind: Option<String>,
    pub venue: Option<String>,
    pub pool: Option<String>,
    pub position: Option<String>,
    pub token_mint: Option<String>,
    pub legs: Vec<WalletLeg>,
    pub usd: Option<f64>,
    pub block_time: Option<String>,
    pub seen_at: Option<String>,
    pub is_backfill: Option<bool>,
    pub wallet: Option<TrackedWalletRef>,
}

/// The chain reader's state: `live`, `slowed` or `starting`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WalletSource {
    pub state: Option<String>,
    pub wallets_fresh: Option<i64>,
    pub wallets_tracked: Option<i64>,
    pub last_ok_at: Option<String>,
}

/// A page of wallet activity, newest first. `next_before` is the cursor.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WalletFeed {
    pub items: Vec<WalletEvent>,
    pub next_before: Option<String>,
    pub source: Option<WalletSource>,
}

/// A wallet on the curated list and where it comes from (`kind`).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CuratedWallet {
    pub address: Option<String>,
    pub label: Option<String>,
    pub kind: Option<String>,
    pub agent_slug: Option<String>,
    pub source: Option<String>,
    pub muted: Option<bool>,
    pub last_activity_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CuratedWallets {
    pub defaults: Vec<CuratedWallet>,
}

/// Any wallet's recent swaps and LP actions. `mode` is `tracked` (from what
/// the tracker stored) or `live` (read from chain for this request).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WalletActivity {
    pub address: Option<String>,
    pub mode: Option<String>,
    pub items: Vec<WalletEvent>,
    pub next_before: Option<String>,
    pub last_ok_at: Option<String>,
}

// ---------------------------------------------------------------------------
// The agent fleet: batches, funding and the fleet read (0.6.0)
// ---------------------------------------------------------------------------

/// The answer to [`crate::Client::create_agent_wallet`]: the agent's own
/// managed wallet.
///
/// `created` is true when this call made it and false when an existing active
/// one came back, so calling twice is safe. An empty wallet grants no
/// authority: nothing signs from it until the owner signs a custody mandate
/// in the app.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AgentWalletCreated {
    pub pubkey: Option<String>,
    pub status: Option<String>,
    /// RFC 3339.
    pub created_at: Option<String>,
    pub created: Option<bool>,
}

/// One agent's outcome inside a batch.
///
/// `outcome` is `done`, `failed` or `running`. `running` is not a failure:
/// the work is still going on the server, detached from the request, and
/// [`crate::Client::get_batch`] collects it. A `failed` item always carries
/// `error` (a plain sentence) plus `status` and `code` to branch on.
/// `result` is the single-agent call's own answer, verbatim: `{"agent",
/// "wallet"}` (plus `wallet_error`) for a create, the [`Withdrawal`] shape
/// for an exit or a withdraw.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FleetBatchItem {
    pub index: Option<usize>,
    /// `None` on a create that failed.
    pub agent_id: Option<String>,
    pub outcome: Option<String>,
    pub ok: Option<bool>,
    pub status: Option<u16>,
    pub code: Option<String>,
    pub error: Option<String>,
    pub result: Option<serde_json::Value>,
}

impl FleetBatchItem {
    /// The created agent on a create item.
    pub fn agent(&self) -> Option<Agent> {
        let a = self.result.as_ref()?.get("agent")?;
        serde_json::from_value(a.clone()).ok()
    }

    /// The receipt on a done exit or withdraw item.
    pub fn withdrawal(&self) -> Option<Withdrawal> {
        let r = self.result.as_ref()?;
        r.get("signature")?;
        serde_json::from_value(r.clone()).ok()
    }
}

/// A create, exit or withdraw over many agents.
///
/// `kind` is `create`, `exit` or `withdraw`, and `done + failed + running ==
/// total`. The server answers once its fund deadline passes; whatever is
/// still in flight then is `running`, and the batch keeps going after the
/// response. Poll [`crate::Client::get_batch`] until `running == 0`. A
/// finished batch is kept for 30 minutes.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FleetBatch {
    pub batch_id: Option<String>,
    pub kind: Option<String>,
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub running: usize,
    pub items: Vec<FleetBatchItem>,
}

/// One agent to fund in [`crate::Client::fund_prepare`]. The destination is
/// never part of the request: the server resolves the agent's own wallet.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FundRequest {
    pub agent_id: String,
    /// Exact lamports to send. `None` tops the wallet up to the venue's
    /// recommended opening balance.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lamports: Option<u64>,
}

impl FundRequest {
    /// Top this agent's wallet up to its recommended opening balance.
    pub fn top_up(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            lamports: None,
        }
    }

    /// Send exactly `lamports`.
    pub fn exact(agent_id: impl Into<String>, lamports: u64) -> Self {
        Self {
            agent_id: agent_id.into(),
            lamports: Some(lamports),
        }
    }
}

/// One agent in a funding plan.
///
/// `ok` false carries `refusal`, a sentence saying why the agent was left
/// out. `needs_mandate` is true when the agent cannot act on these funds
/// until its owner signs a custody mandate in the app.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FundItem {
    pub agent_id: Option<String>,
    pub ok: Option<bool>,
    /// The agent's own managed wallet, resolved by the server.
    pub wallet: Option<String>,
    pub dex: Option<String>,
    pub pool_address: Option<String>,
    pub lamports: Option<u64>,
    pub current_lamports: Option<u64>,
    pub recommended_open_lamports: Option<u64>,
    pub needs_mandate: Option<bool>,
    pub refusal: Option<String>,
}

/// One SystemProgram transfer inside a funding transaction.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FundTransfer {
    pub agent_id: Option<String>,
    pub to: Option<String>,
    pub lamports: Option<u64>,
}

/// One UNSIGNED funding transaction: a legacy Solana transaction, base64 of
/// the bincode wire format, with the owner's wallet as fee payer and only
/// signer. Simulated against mainnet before it was returned. This crate
/// never signs it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FundTransaction {
    pub index: Option<usize>,
    pub unsigned_tx_b64: Option<String>,
    pub transfers: Vec<FundTransfer>,
    pub total_lamports: Option<u64>,
    pub simulated: Option<bool>,
    pub units_consumed: Option<u64>,
}

/// Unsigned transfers from your wallet to your agents' own wallets, packed
/// into as few transactions as fit (ten agents fit in one).
///
/// `ok` false carries `refusal` and no transaction. To sign, open each of
/// `sign_urls` on ponk.exchange with the wallet connected (the page also
/// queues each mandate in `needs_mandate`), or sign `unsigned_tx_b64` in
/// wallet software you control before `last_valid_block_height`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FundPlan {
    /// The fee payer and the only signer.
    pub owner: Option<String>,
    pub ok: Option<bool>,
    pub refusal: Option<String>,
    pub balance_lamports: Option<u64>,
    pub total_lamports: Option<u64>,
    pub network_fee_lamports: Option<u64>,
    pub recent_blockhash: Option<String>,
    pub last_valid_block_height: Option<u64>,
    pub transactions: Vec<FundTransaction>,
    pub items: Vec<FundItem>,
    pub needs_mandate: Vec<String>,
    pub sign_urls: Vec<String>,
}

/// One agent with its wallet, mandate and performance.
///
/// A part that is `None` because its read FAILED, rather than because there
/// is nothing, is named in `read_errors`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FleetRow {
    pub agent: Agent,
    /// `None` before the agent has its own wallet.
    pub wallet: Option<AgentWallet>,
    /// `None` when no mandate was ever signed.
    pub mandate: Option<Mandate>,
    pub performance: Option<AgentPerformance>,
    pub read_errors: Vec<String>,
}

/// One page of the fleet. `next_offset` is `None` on the last page.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Fleet {
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub next_offset: Option<usize>,
    pub rows: Vec<FleetRow>,
}

// ---------------------------------------------------------------------------
// /v1 account parity (0.6.0)
// ---------------------------------------------------------------------------

/// The body of [`crate::Client::rebalance`].
#[derive(Debug, Clone, Default, Serialize)]
pub struct RebalanceRequest {
    pub new_lower_bin_id: i32,
    pub new_upper_bin_id: i32,
    /// The liquidity shape, for example `spot`.
    pub strategy: String,
    /// Basis points; the server default is 50.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_slippage_bps: Option<u16>,
}

/// The receipt for a managed manual rebalance. Read it like
/// [`ActionReceipt`]: a refusal at signing time answers 200 with a failed
/// `status` and `signature == None`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RebalanceReceipt {
    pub action_id: Option<String>,
    pub signature: Option<String>,
    pub status: Option<String>,
    pub action_status: Option<String>,
    /// The new position once one opened.
    pub position_address: Option<String>,
    pub compute_units_consumed: Option<u64>,
    pub error_message: Option<String>,
    pub risk_approved: Option<bool>,
    pub risk_failed_checks: Vec<String>,
}

/// How long the position was in range. `pct` is `None` when nothing was
/// observed, never zero.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TimeInRange {
    pub in_range_secs: Option<i64>,
    pub observed_secs: Option<i64>,
    pub pct: Option<f64>,
    pub readings: Option<i64>,
}

/// One cost of an action. `usd` is `None`, with `reason`, when not measured.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CostLeg {
    pub usd: Option<String>,
    pub reason: Option<String>,
}

/// What one action cost. `total_usd` is `None` unless every leg is known.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ActionCosts {
    pub network: Option<CostLeg>,
    pub swap: Option<CostLeg>,
    pub rent: Option<CostLeg>,
    pub ponk_fee: Option<CostLeg>,
    pub total_usd: Option<String>,
    pub modelled_usd: Option<String>,
}

/// One action the agent took, what it cost, and what it earned while held.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProofAction {
    pub kind: Option<String>,
    pub at: Option<String>,
    pub tx_signature: Option<String>,
    pub trigger_key: Option<String>,
    pub costs: Option<ActionCosts>,
    pub earned_usd: Option<String>,
    pub earned_reason: Option<String>,
    pub held_secs: Option<i64>,
    pub open: Option<bool>,
    pub within_measured_range: Option<bool>,
}

/// Whether the agent's actions paid for themselves, from the ledger.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AgentProof {
    pub window_hours: Option<i64>,
    pub since: Option<String>,
    pub time_in_range: Option<TimeInRange>,
    pub gap_cap_secs: Option<i64>,
    pub actions: Vec<ProofAction>,
    pub earned_usd: Option<String>,
    pub earned_known: Option<i64>,
    pub earned_countable: Option<i64>,
    pub measured_cost_usd: Option<String>,
    pub cost_legs_known: Option<i64>,
    pub cost_legs_total: Option<i64>,
}

/// One bucket of the range history.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RangePoint {
    pub t: Option<String>,
    pub active: Option<i32>,
    pub lower: Option<i32>,
    pub upper: Option<i32>,
    pub in_range: Option<bool>,
    pub price: Option<f64>,
    pub lower_price: Option<f64>,
    pub upper_price: Option<f64>,
}

/// Where the price sat against the agent's range over time.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RangeHistory {
    pub range: Option<String>,
    pub bucket_secs: Option<i64>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub dex: Option<String>,
    pub pool_address: Option<String>,
    pub price_now: Option<f64>,
    pub active_now: Option<i32>,
    pub in_range_share: Option<f64>,
    pub points: Vec<RangePoint>,
}

/// One position in the portfolio summary. `None` is unpriced, not zero.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PortfolioPosition {
    pub dex: Option<String>,
    pub pool_address: Option<String>,
    pub value_usd: Option<f64>,
    pub cost_basis_usd: Option<f64>,
    pub pnl_usd: Option<f64>,
    pub pnl_pct: Option<f64>,
    pub claimable_fees_usd: Option<f64>,
    pub price_pnl_usd: Option<f64>,
    pub fee_pnl_usd: Option<f64>,
    pub il_usd: Option<f64>,
    pub vs_hodl_usd: Option<f64>,
    pub fee_apr_pct: Option<f64>,
    pub fees_per_day_usd: Option<f64>,
    pub fees_earned_usd: Option<f64>,
}

/// `t` is unix seconds.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PortfolioSeriesPoint {
    pub t: Option<i64>,
    pub value_usd: Option<f64>,
}

/// The account's LP portfolio summary.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Portfolio {
    pub total_position_value_usd: Option<f64>,
    pub all_time_deposit_usd: Option<f64>,
    pub fees_claimed_usd: Option<f64>,
    pub claimable_fees_usd: Option<f64>,
    pub total_pnl_usd: Option<f64>,
    pub total_pnl_pct: Option<f64>,
    pub avg_invested_usd: Option<f64>,
    pub win_rate_pct: Option<f64>,
    pub biggest_win_usd: Option<f64>,
    pub position_count: Option<i64>,
    pub price_pnl_usd: Option<f64>,
    pub fee_pnl_usd: Option<f64>,
    pub il_usd: Option<f64>,
    pub vs_hodl_usd: Option<f64>,
    pub vs_hodl_measured: Option<i64>,
    pub vs_hodl_positions: Option<i64>,
    pub fee_apr_pct: Option<f64>,
    pub positions: Vec<PortfolioPosition>,
    pub series: Vec<PortfolioSeriesPoint>,
}

/// `value_usd` is `None` for a bucket holding an unpriced position.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PortfolioHistoryPoint {
    pub t: Option<String>,
    pub value_usd: Option<f64>,
    pub fees_usd: Option<f64>,
    pub realized_usd: Option<f64>,
    pub unpriced: Option<u32>,
}

/// One UTC day of fees and realized PnL.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CalendarDay {
    /// `YYYY-MM-DD`.
    pub date: Option<String>,
    pub fees_usd: Option<f64>,
    pub realized_usd: Option<f64>,
    pub pnl_usd: Option<f64>,
    pub events: Option<u32>,
    pub unpriced: Option<u32>,
}

/// The account's value over time, plus a daily calendar.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PortfolioHistory {
    pub range: Option<String>,
    pub bucket_secs: Option<i64>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub points: Vec<PortfolioHistoryPoint>,
    pub incomplete_buckets: Option<usize>,
    pub positions: Option<usize>,
    pub days: Vec<CalendarDay>,
    pub first_record_at: Option<String>,
}

/// One transaction an agent sent, with its on-chain outcome.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Transaction {
    pub id: Option<String>,
    pub agent_id: Option<String>,
    pub agent_name: Option<String>,
    pub signature: Option<String>,
    pub status: Option<String>,
    pub dex: Option<String>,
    pub action_type: Option<String>,
    pub priority_fee_lamports: Option<i64>,
    pub compute_units: Option<i32>,
    pub error_message: Option<String>,
    pub created_at: Option<String>,
    pub confirmed_at: Option<String>,
    pub pnl_delta_usd: Option<String>,
}

/// Something the risk engine flagged or refused.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RiskEvent {
    pub id: Option<String>,
    pub agent_id: Option<String>,
    pub agent_name: Option<String>,
    pub severity: Option<String>,
    pub code: Option<String>,
    pub message: Option<String>,
    pub created_at: Option<String>,
}

/// One platform fee taken on claimed LP fees. Amounts are base-unit strings
/// of `fee_token_mint`; USD is `None` when the token could not be priced.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FeeEvent {
    pub id: Option<String>,
    pub agent_id: Option<String>,
    pub wallet_address: Option<String>,
    pub dex: Option<String>,
    pub pool_address: Option<String>,
    pub position_address: Option<String>,
    pub fee_bps: Option<i32>,
    pub fee_token_mint: Option<String>,
    pub gross_fee_amount: Option<String>,
    pub platform_fee_amount: Option<String>,
    pub user_net_amount: Option<String>,
    pub gross_fee_usd: Option<String>,
    pub platform_fee_usd: Option<String>,
    pub user_net_usd: Option<String>,
    pub platform_fee_wallet: Option<String>,
    pub platform_fee_signature: Option<String>,
    pub status: Option<String>,
    pub error_message: Option<String>,
    pub created_at: Option<String>,
    pub confirmed_at: Option<String>,
}

/// Totals over every fee event of the account, as decimal strings.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FeeSummary {
    pub total_platform_fees_usd: Option<String>,
    pub total_claimed_fees_usd: Option<String>,
    pub total_net_fees_usd: Option<String>,
    pub event_count: Option<i32>,
}

/// One notification from the app's bell.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Notification {
    pub id: Option<String>,
    pub agent_id: Option<String>,
    pub kind: Option<String>,
    pub severity: Option<String>,
    pub title: Option<String>,
    pub message: Option<String>,
    pub status: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: Option<String>,
    pub read_at: Option<String>,
    pub dismissed_at: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UnreadCount {
    pub unread: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ReadAll {
    pub marked: Option<u64>,
}

/// A recommendation an agent raised for its owner. Approving one flips its
/// status only; no transaction is built or signed by the approval.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Recommendation {
    pub id: Option<String>,
    pub agent_id: Option<String>,
    pub recommendation_type: Option<String>,
    pub status: Option<String>,
    pub dex: Option<String>,
    pub pool_address: Option<String>,
    pub position_address: Option<String>,
    pub reason: Option<String>,
    pub strategy_snapshot: Option<serde_json::Value>,
    pub risk_result: Option<serde_json::Value>,
    pub transaction_preview: Option<serde_json::Value>,
    pub created_at: Option<String>,
    pub expires_at: Option<String>,
    pub reviewed_at: Option<String>,
    pub acted_at: Option<String>,
    pub dismissed_at: Option<String>,
}

/// Which Telegram messages the account receives. On a PUT, a `None` switch
/// is left out and so left unchanged.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NotifySettings {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rebalance_telegram: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mandate_telegram: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weekly_recap_telegram: Option<bool>,
}

/// A saved agent preset. `overrides` is the normalized document the server
/// stored; `dex` is `None` for any venue except Ponk Clouds.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Preset {
    pub id: Option<String>,
    pub name: Option<String>,
    pub overrides: Option<serde_json::Value>,
    pub dex: Option<String>,
    pub created_at: Option<String>,
}

/// The body of [`crate::Client::create_preset`] and
/// [`crate::Client::update_preset`].
#[derive(Debug, Clone, Default, Serialize)]
pub struct NewPreset {
    /// 1 to 60 characters.
    pub name: String,
    /// Validated and clamped by the server.
    pub overrides: serde_json::Value,
    pub dex: Option<String>,
}

#[cfg(test)]
mod live_shape_tests {
    use super::*;

    #[test]
    fn the_x_trackers_digit_suffixed_camel_case_keys_decode() {
        let t: XTrending = serde_json::from_str(
            r#"{"window":"1h","tokens":[{"mint":"M","symbol":"S","accounts1h":1,"accounts24h":2,
            "mentions24h":3,"lastAt":"2026-10-23T03:22:04Z","firstCall":{"handle":"h","screenName":"H",
            "postId":"9","url":"u","at":"2026-10-23T03:00:00Z","priceUsd":null,"mcapUsd":null},
            "priceUsd":1.5,"mcapUsd":null,"multiple":null}],"computedAt":"2026-10-23T04:00:00Z"}"#,
        )
        .unwrap();
        let tok = &t.tokens[0];
        assert_eq!((tok.accounts_1h, tok.accounts_24h, tok.mentions_24h), (Some(1), Some(2), Some(3)));
        assert_eq!(tok.first_call.as_ref().unwrap().post_id.as_deref(), Some("9"));
        assert!(tok.multiple.is_none());

        let s: XSource = serde_json::from_str(
            r#"{"name":"FxTwitter","state":"live","requests1h":5721,"rateLimited1h":0,"hotSecs":20}"#,
        )
        .unwrap();
        assert_eq!((s.requests_1h, s.rate_limited_1h, s.hot_secs), (Some(5721), Some(0), Some(20)));
    }

    #[test]
    fn a_feed_item_names_its_type() {
        let f: XFeed = serde_json::from_str(
            r#"{"items":[{"type":"post","key":"p:a:1","account":{"handle":"a","screenName":"A"},
            "post":{"id":"1","tokens":[{"mint":"M","symbol":null}],"metrics":{"views":3}}}],
            "nextBefore":null,"source":{"state":"live"}}"#,
        )
        .unwrap();
        let item = &f.items[0];
        assert_eq!(item.item_type.as_deref(), Some("post"));
        assert_eq!(item.post.as_ref().unwrap().tokens[0].mint.as_deref(), Some("M"));
        assert!(item.event.is_none());
    }

    #[test]
    fn the_week_flattens_identity_into_each_row() {
        let w: PonkillionairesWeek = serde_json::from_str(
            r#"{"weekStart":"2026-10-19T00:00:00Z","resetsAt":"2026-10-26T00:00:00Z",
            "prizes":[{"rank":1,"feeRatePct":2.0,"points":50000}],"minPeakUsd":250,"minRunningDays":3,
            "standings":[{"rank":1,"pnlUsd":41.5,"agents":2,"deployedUsd":900.0,"prize":null,
            "ownerKey":"k1","handle":null,"displayName":"kader","wallet":null}],
            "lastWeek":{"weekStart":"2026-10-12T00:00:00Z","settled":true,"winners":[]},
            "computedAt":"2026-10-23T04:00:00Z"}"#,
        )
        .unwrap();
        assert_eq!(w.min_peak_usd, Some(250));
        assert_eq!(w.prizes[0].fee_rate_pct, Some(2.0));
        assert_eq!(w.standings[0].display_name.as_deref(), Some("kader"));
        assert_eq!(w.last_week.unwrap().settled, Some(true));
    }

    #[test]
    fn activity_money_stays_a_string() {
        let a: AgentsActivity = serde_json::from_str(
            r#"{"events":[{"key":"k","kind":"compound","at":"2026-10-23T02:14:09Z","coarse":false,
            "dex":"meteora_dlmm","usd":"1.234567","usd_kind":"fees"}],"holding":7,
            "computed_at":"2026-10-23T04:10:05Z"}"#,
        )
        .unwrap();
        assert_eq!(a.holding, Some(7));
        assert_eq!(a.events[0].usd.as_deref(), Some("1.234567"));
    }

    #[test]
    fn the_referral_tier_and_ladder_decode() {
        let r: Referral = serde_json::from_str(
            r#"{"code":"abc","tier":{"current":2,"share_bps":2500,"referred_fees_usd":1500.0,
            "window_days":30,"next":{"tier":3,"min_fees_usd":5000,"share_bps":3000}},
            "programme":{"ladder":[{"tier":1,"min_fees_usd":0,"share_bps":2000},
            {"tier":2,"min_fees_usd":1000,"share_bps":2500},{"tier":3,"min_fees_usd":5000,"share_bps":3000}],
            "window_days":30}}"#,
        )
        .unwrap();
        let t = r.tier.unwrap();
        assert_eq!((t.current, t.share_bps), (Some(2), Some(2500)));
        assert_eq!(t.next.unwrap().min_fees_usd, Some(5000));
        let ladder: Vec<_> = r.programme.unwrap().ladder.iter().map(|s| s.share_bps).collect();
        assert_eq!(ladder, vec![Some(2000), Some(2500), Some(3000)]);
    }

    #[test]
    fn a_wallet_event_keeps_base_unit_amounts() {
        let f: WalletFeed = serde_json::from_str(
            r#"{"items":[{"key":"sig:a","signature":"a","kind":"buy","tokenMint":"M",
            "legs":[{"mint":"U","amount":"1000000","decimals":6,"dir":"out","usd":1.0}],
            "usd":1.0,"blockTime":"2026-10-23T04:00:00Z","seenAt":"2026-10-23T04:00:03Z","isBackfill":false,
            "wallet":{"address":"W","agentSlug":"abc123","source":"ponkillionaire"}}],
            "nextBefore":"1761192000000_42","source":{"state":"live","walletsFresh":1,"walletsTracked":2}}"#,
        )
        .unwrap();
        let e = &f.items[0];
        assert_eq!(e.token_mint.as_deref(), Some("M"));
        assert_eq!(e.legs[0].amount.as_deref(), Some("1000000"));
        assert_eq!(e.wallet.as_ref().unwrap().agent_slug.as_deref(), Some("abc123"));
        assert_eq!(f.source.unwrap().wallets_tracked, Some(2));
    }
}
