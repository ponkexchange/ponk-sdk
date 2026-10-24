"""Typed results for the ponk public API.

Each class mirrors one response shape served by `apps/api/src/routes`. Field
names are the JSON keys, so a value here is the value the server sent, never a
derived or rounded one.

Three things are deliberate:

* Every field defaults to `None`. A field the server omits reads as `None`,
  which is also how the API expresses "not computable" - an unpriceable pool
  sends `null` for a USD figure and it must never be read as zero.
* `raw_json` on every object is the exact decoded payload, so a field added to
  the API after this client was written is still reachable.
* USD amounts arrive as strings on the agent and position endpoints (decimal
  precision that a float would destroy). They are kept as strings. Feed them to
  `decimal.Decimal` before doing arithmetic on money.
"""

from __future__ import annotations

import dataclasses
from dataclasses import dataclass, field
from decimal import Decimal
from typing import Any, ClassVar, Dict, List, Mapping, Optional, Type, TypeVar

__all__ = [
    "ActionCosts",
    "ActionLog",
    "ActionReceipt",
    "ActivityEvent",
    "Agent",
    "AgentCopy",
    "AgentPerformance",
    "AgentPosition",
    "AgentProof",
    "AgentWallet",
    "AgentWalletCreated",
    "AgentsActivity",
    "AutonomousState",
    "BinShare",
    "CalendarDay",
    "ClaimPayout",
    "ClaimPayoutToken",
    "ComponentStatus",
    "CostLeg",
    "CreatorDashboard",
    "CreatorDay",
    "CreatorTotals",
    "CuratedWallet",
    "CuratedWallets",
    "ExitSnapshot",
    "FeeEvent",
    "FeeRates",
    "FeeSummary",
    "Fleet",
    "FleetBatch",
    "FleetBatchItem",
    "FleetRow",
    "Follow",
    "Follows",
    "FundItem",
    "FundPlan",
    "FundTransaction",
    "FundTransfer",
    "HandleCheck",
    "Health",
    "HealthChecks",
    "LastWeek",
    "Leaderboard",
    "LeaderboardRow",
    "LinkRevoked",
    "Mandate",
    "MyPoints",
    "Notification",
    "NotifySettings",
    "PointsEntry",
    "PointsLeaderboard",
    "PointsSeason",
    "PonkPerks",
    "PonkillionairesWeek",
    "PoolSnapshot",
    "Portfolio",
    "PortfolioHistory",
    "PortfolioHistoryPoint",
    "PortfolioPosition",
    "PortfolioSeriesPoint",
    "Position",
    "Preset",
    "ProofAction",
    "PublicAgent",
    "PublicLink",
    "PublicReferral",
    "RangeCost",
    "RangeHistory",
    "RangePoint",
    "ReadAll",
    "RebalanceReceipt",
    "Recommendation",
    "Referral",
    "ReferralCode",
    "ReferralProgramme",
    "ReferralTier",
    "ReferralTierStep",
    "RiskEvent",
    "TimeInRange",
    "TokenAmount",
    "TrackedWalletRef",
    "Transaction",
    "UnreadCount",
    "ValueHistory",
    "WalletActivity",
    "WalletEvent",
    "WalletFeed",
    "WalletLeg",
    "WalletSource",
    "WeekPrize",
    "WeekStanding",
    "WeekWinner",
    "WhoAmI",
    "Withdrawal",
    "XAccount",
    "XAuthor",
    "XCaller",
    "XCallers",
    "XDirectory",
    "XDirectoryEntry",
    "XEvent",
    "XEventPost",
    "XFeed",
    "XFeedItem",
    "XFirstCall",
    "XMetrics",
    "XPost",
    "XSource",
    "XToken",
    "XTokenMention",
    "XTokenRef",
    "XTrending",
    "XTrendingToken",
]

T = TypeVar("T", bound="Model")


def _camel(name: str) -> str:
    """serde's camelCase: `last_ok_at` is `lastOkAt`, `accounts_1h` is `accounts1h`."""
    head, *rest = name.split("_")
    return head + "".join(part[:1].upper() + part[1:] for part in rest)


class Model:
    """Base for every response object.

    `raw_json` is the decoded body exactly as it arrived. Reach into it for
    anything this client does not yet name.
    """

    raw_json: Mapping[str, Any] = {}

    # JSON key for a python field, when the two differ (the $PONK perks
    # endpoint is camelCase; everything else is snake_case).
    _ALIASES: ClassVar[Mapping[str, str]] = {}
    # Fields that hold another model. A one-tuple means "a list of these".
    _NESTED: ClassVar[Mapping[str, Any]] = {}
    # True for a response the server serializes camelCase (the X Tracker,
    # the Wallet Tracker and the weekly Ponkillionaires race). Python fields
    # stay snake_case and each one reads its camelCase key, exactly as serde's
    # `rename_all = "camelCase"` spells it (`accounts_1h` is `accounts1h`).
    _CAMEL: ClassVar[bool] = False

    @classmethod
    def from_dict(cls: Type[T], data: Any) -> T:
        if not isinstance(data, Mapping):
            raise TypeError(
                "{0} expected a JSON object, got {1}".format(cls.__name__, type(data).__name__)
            )
        kwargs: Dict[str, Any] = {}
        for f in dataclasses.fields(cls):  # type: ignore[arg-type]
            key = cls._ALIASES.get(f.name) or (_camel(f.name) if cls._CAMEL else f.name)
            if key not in data:
                continue
            value = data[key]
            nested = cls._NESTED.get(f.name)
            if nested is not None and value is not None:
                if isinstance(nested, tuple):
                    inner = nested[0]
                    value = [inner.from_dict(item) for item in value]
                else:
                    value = nested.from_dict(value)
            kwargs[f.name] = value
        obj = cls(**kwargs)
        obj.raw_json = data
        return obj

    @classmethod
    def from_list(cls: Type[T], data: Any) -> List[T]:
        if not isinstance(data, list):
            raise TypeError(
                "{0} expected a JSON array, got {1}".format(cls.__name__, type(data).__name__)
            )
        return [cls.from_dict(item) for item in data]


# --------------------------------------------------------------------------
# Health and pools (public, no key needed)
# --------------------------------------------------------------------------


@dataclass
class ComponentStatus(Model):
    healthy: Optional[bool] = None
    detail: Optional[str] = None
    latency_ms: Optional[int] = None


@dataclass
class HealthChecks(Model):
    database: Optional[ComponentStatus] = None
    helius: Optional[ComponentStatus] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "database": ComponentStatus,
        "helius": ComponentStatus,
    }


@dataclass
class Health(Model):
    status: Optional[str] = None
    time: Optional[str] = None
    checks: Optional[HealthChecks] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"checks": HealthChecks}


@dataclass
class PoolSnapshot(Model):
    """One pool, decoded from its on-chain account.

    `current_price` is RAW: token-Y base units per token-X base unit, NOT
    decimal adjusted. For a human quote-per-base price multiply by
    `10 ** (token_x_decimals - token_y_decimals)`.

    `tvl_usd`, `volume_24h` and `fees_24h` come from market data, not the
    account, and are `None` when that data is unavailable.
    """

    dex: Optional[str] = None
    pool_address: Optional[str] = None
    token_x_mint: Optional[str] = None
    token_y_mint: Optional[str] = None
    token_x_decimals: Optional[int] = None
    token_y_decimals: Optional[int] = None
    bin_step: Optional[int] = None
    active_bin_id: Optional[int] = None
    current_price: Optional[str] = None
    tvl_usd: Optional[str] = None
    volume_24h: Optional[str] = None
    fees_24h: Optional[str] = None
    base_fee_pct: Optional[float] = None
    fetched_at: Optional[str] = None

    def human_price(self) -> Optional[Decimal]:
        """`current_price` as a quote-per-base price, or `None` if unknown.

        Returns `None` rather than a guess when either decimals or the price
        is missing.
        """
        if self.current_price is None:
            return None
        if self.token_x_decimals is None or self.token_y_decimals is None:
            return None
        scale = Decimal(10) ** (int(self.token_x_decimals) - int(self.token_y_decimals))
        return Decimal(self.current_price) * scale


@dataclass
class RangeCost(Model):
    """What a Meteora DLMM range of a given width costs to open, in lamports.

    `requested_bins` is what you asked for and `total_bins` what one position
    will actually hold; `clamped` is true when the venue ceiling
    (`max_total_bins`) cut the request. The range is centred on the active
    bin, so `lower_bin_id` / `upper_bin_id` are the exact edges and
    `down_pct` / `up_pct` the geometric distance of each edge from the
    current price (negative below, positive above).

    `refundable_lamports` is the position rent, which `close_position`
    returns. `non_refundable_lamports` is the rent for bin arrays this wallet
    would have to create plus signature fees. `bin_array_probe` is `probed`
    when every bin array was read on chain, or `unavailable` when that read
    failed, in which case the quote assumes none exist and is the cold-pool
    worst case.
    """

    pool_address: Optional[str] = None
    dex: Optional[str] = None
    bin_step: Optional[int] = None
    active_bin_id: Optional[int] = None
    requested_bins: Optional[int] = None
    total_bins: Optional[int] = None
    max_total_bins: Optional[int] = None
    clamped: Optional[bool] = None
    lower_bin_id: Optional[int] = None
    upper_bin_id: Optional[int] = None
    down_pct: Optional[float] = None
    up_pct: Optional[float] = None
    position_rent_lamports: Optional[int] = None
    bin_arrays_total: Optional[int] = None
    bin_arrays_existing: Optional[int] = None
    bin_arrays_missing: Optional[int] = None
    bin_array_rent_lamports: Optional[int] = None
    bin_array_probe: Optional[str] = None
    resize_tx_count: Optional[int] = None
    tx_fee_lamports: Optional[int] = None
    total_upfront_lamports: Optional[int] = None
    refundable_lamports: Optional[int] = None
    non_refundable_lamports: Optional[int] = None


@dataclass
class FeeRates(Model):
    """One fee rate, undiscounted and as actually charged to a wallet.

    `base_*` is the list rate, `effective_*` is what THIS wallet pays right
    now, and `holder_*` is what a qualifying $PONK holder pays whether or not
    this wallet qualifies.
    """

    base_bps: Optional[int] = None
    effective_bps: Optional[int] = None
    base_percent: Optional[float] = None
    effective_percent: Optional[float] = None
    holder_bps: Optional[int] = None
    holder_percent: Optional[float] = None

    _ALIASES: ClassVar[Mapping[str, str]] = {
        "base_bps": "baseBps",
        "effective_bps": "effectiveBps",
        "base_percent": "basePercent",
        "effective_percent": "effectivePercent",
        "holder_bps": "holderBps",
        "holder_percent": "holderPercent",
    }


@dataclass
class PonkPerks(Model):
    """$PONK holder benefits for one wallet.

    `api_agent_fee` is the rate an agent created through this API pays, which
    is higher than `managed_agent_fee` (an agent created in the app). The rate
    is fixed when the agent is created and follows it for life.
    """

    mint: Optional[str] = None
    threshold_ui: Optional[int] = None
    balance_ui: Optional[float] = None
    holds: Optional[bool] = None
    shortfall_ui: Optional[float] = None
    discount_percent: Optional[int] = None
    managed_agent_fee: Optional[FeeRates] = None
    api_agent_fee: Optional[FeeRates] = None
    non_custodial_agent_fee: Optional[FeeRates] = None

    _ALIASES: ClassVar[Mapping[str, str]] = {
        "threshold_ui": "thresholdUi",
        "balance_ui": "balanceUi",
        "shortfall_ui": "shortfallUi",
        "discount_percent": "discountPercent",
        "managed_agent_fee": "managedAgentFee",
        "api_agent_fee": "apiAgentFee",
        "non_custodial_agent_fee": "nonCustodialAgentFee",
    }
    _NESTED: ClassVar[Mapping[str, Any]] = {
        "managed_agent_fee": FeeRates,
        "api_agent_fee": FeeRates,
        "non_custodial_agent_fee": FeeRates,
    }


# --------------------------------------------------------------------------
# Identity
# --------------------------------------------------------------------------


@dataclass
class WhoAmI(Model):
    """What a key is and what it may do. `scope` is `read` or `trade`."""

    wallet_address: Optional[str] = None
    scope: Optional[str] = None
    key_id: Optional[str] = None


# --------------------------------------------------------------------------
# Agents
# --------------------------------------------------------------------------


@dataclass
class Agent(Model):
    """One agent.

    `wallet_source` is `external` (you sign every action) or `managed` (the
    agent signs autonomously under a custody mandate you signed once in the
    app). `origin` is `web` or `api`, and it prices the agent:
    `managed_fee_bps` is the performance fee this agent's managed skim
    actually charges, before any $PONK holder discount.
    """

    id: Optional[str] = None
    name: Optional[str] = None
    wallet_address: Optional[str] = None
    dex: Optional[str] = None
    pool_address: Optional[str] = None
    position_address: Optional[str] = None
    status: Optional[str] = None
    strategy: Optional[str] = None
    dry_run: Optional[bool] = None
    config: Optional[Any] = None
    created_at: Optional[str] = None
    updated_at: Optional[str] = None
    mode: Optional[str] = None
    wallet_source: Optional[str] = None
    origin: Optional[str] = None
    managed_fee_bps: Optional[int] = None


@dataclass
class AgentPerformance(Model):
    """Live value, PnL and fees for one agent.

    Every USD field is a string or `None`. `None` means the figure could not be
    computed honestly, most often because a leg of the pool has no price right
    now. It does not mean zero, and rendering it as 0 understates a position.

    The canonical decomposition is Total = Price + Fee: `pnl_usd` is the total,
    `price_pnl_usd` the principal leg, `fee_pnl_usd` the yield leg of the
    CURRENT position. `fees_earned_lifetime_usd` spans every position the agent
    has run on its pool, so it does not reset when the agent recenters.

    `pnl_source` is `meteora_datapi` (Meteora's own figures) or
    `oracle_estimate` (ponk's on-chain valuation).
    """

    current_value_usd: Optional[str] = None
    cost_basis_usd: Optional[str] = None
    pnl_usd: Optional[str] = None
    pnl_pct: Optional[float] = None
    peak_value_usd: Optional[str] = None
    drawdown_pct: Optional[float] = None
    fees_claimed_usd: Optional[str] = None
    platform_fees_paid_usd: Optional[str] = None
    platform_fees_paid_is_estimate: Optional[bool] = None
    fees_claim_count: Optional[int] = None
    agent_realized_fees_usd: Optional[str] = None
    realized_fees_usd: Optional[str] = None
    has_position: Optional[bool] = None
    wallet_source: Optional[str] = None
    status: Optional[str] = None
    updated_at: Optional[str] = None
    fees_earned_usd: Optional[str] = None
    fees_unclaimed_usd: Optional[str] = None
    deposits_usd: Optional[str] = None
    withdrawals_usd: Optional[str] = None
    price_pnl_usd: Optional[str] = None
    fee_pnl_usd: Optional[str] = None
    fees_earned_lifetime_usd: Optional[str] = None
    fees_claimed_lifetime_usd: Optional[str] = None
    fees_unclaimed_lifetime_usd: Optional[str] = None
    positions_count: Optional[int] = None
    il_usd: Optional[str] = None
    pnl_source: Optional[str] = None


@dataclass
class BinShare(Model):
    """One bin's slice of a position's liquidity.

    `weight` is that bin's liquidity as a fraction of the position's largest
    bin, so it plots directly as a bar height.
    """

    bin_id: Optional[int] = None
    weight: Optional[float] = None
    is_active: Optional[bool] = None
    in_target_range: Optional[bool] = None


@dataclass
class AgentPosition(Model):
    """The agent's live on-chain range.

    The `lower_bin_id` / `upper_bin_id` / `active_bin_id` triple is the ACTUAL
    on-chain state; the `target_*` fields are the strategy's intent and are
    `None` when no target has been recorded. Units are bins on Meteora and Ponk
    Clouds, ticks on Orca, which is what `dex` is for.

    `price_samples` is process-local: it is empty after an API restart or
    before the agent's first tick, and every entry in it is a real observed
    pool price.
    """

    position_address: Optional[str] = None
    lower_bin_id: Optional[int] = None
    upper_bin_id: Optional[int] = None
    active_bin_id: Optional[int] = None
    in_range: Optional[bool] = None
    out_of_range_since: Optional[str] = None
    distance_from_active_bin: Optional[int] = None
    lower_price: Optional[str] = None
    upper_price: Optional[str] = None
    current_price: Optional[str] = None
    band_pct: Optional[float] = None
    price_position_pct: Optional[float] = None
    target_lower_bin_id: Optional[int] = None
    target_upper_bin_id: Optional[int] = None
    target_lower_price: Optional[str] = None
    target_upper_price: Optional[str] = None
    target_source: Optional[str] = None
    recovery_direction: Optional[str] = None
    is_full_range: Optional[bool] = None
    dex: Optional[str] = None
    bins: List[BinShare] = field(default_factory=list)
    price_samples: List[float] = field(default_factory=list)
    fees_unclaimed_usd: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"bins": (BinShare,)}


@dataclass
class AgentWallet(Model):
    """The isolated wallet an autonomous agent owns.

    `min_open_lamports` is the native-SOL reserve the executor always holds
    back, so a wallet at or below it cannot deploy.
    `recommended_open_lamports` clears that floor and leaves enough to open a
    small position. `scope` is `per_agent` for the agent's own wallet, or
    `shared` for the legacy per-user wallet.
    """

    pubkey: Optional[str] = None
    status: Optional[str] = None
    lamports: Optional[int] = None
    sol: Optional[float] = None
    min_open_lamports: Optional[int] = None
    recommended_open_lamports: Optional[int] = None
    scope: Optional[str] = None
    withdrawable_lamports: Optional[int] = None
    token_accounts_with_value: Optional[int] = None


@dataclass
class Mandate(Model):
    """The custody mandate an autonomous agent is signing under.

    Read from the stored, signed scope, so it is what the owner actually
    signed rather than today's defaults.

    `status` is `active` while it can still sign and `expired` once the
    server's clock passed `expires_at`. `seconds_remaining` is signed: it goes
    negative once the mandate has lapsed, and it is computed by the server, so
    it does not depend on your clock agreeing with ponk's.

    `actions` is the signed list of what ponk may do (for example
    `claim_fees`). `max_action_value_usd` is `None` when there is no
    per-action USD ceiling, which means unlimited, never zero.
    `withdrawal_address` is the only address funds can be withdrawn to.
    """

    mandate_id: Optional[str] = None
    status: Optional[str] = None
    expires_at: Optional[str] = None
    granted_at: Optional[str] = None
    seconds_remaining: Optional[int] = None
    actions: List[str] = field(default_factory=list)
    max_action_value_usd: Optional[int] = None
    max_daily_transactions: Optional[int] = None
    max_slippage_bps: Optional[int] = None
    pool_allowlist: List[str] = field(default_factory=list)
    withdrawal_address: Optional[str] = None


@dataclass
class AutonomousState(Model):
    """An agent's custody state right after autonomous mode was switched.

    `wallet_source` is `managed` and `mode` is `auto` after enabling, or
    `external` and `manual` after disabling. `managed_wallet`, `mandate_id`
    and `withdrawal_locked_to` are set only when enabled.

    `status` and `dry_run` are read back from the agent, not assumed. Enabling
    does not start a stopped loop: an agent a previous exit left `stopped` is
    autonomous and live but idle until you call `resume_agent`. Check `status`
    rather than reading `mode == "auto"` as "running".
    """

    agent_id: Optional[str] = None
    wallet_source: Optional[str] = None
    mode: Optional[str] = None
    managed_wallet: Optional[str] = None
    mandate_id: Optional[str] = None
    withdrawal_locked_to: Optional[str] = None
    status: Optional[str] = None
    dry_run: Optional[bool] = None


# --------------------------------------------------------------------------
# Positions and logs
# --------------------------------------------------------------------------


@dataclass
class TokenAmount(Model):
    """A token amount in base units, with the mint's decimals."""

    raw: Optional[int] = None
    decimals: Optional[int] = None

    def to_decimal(self) -> Optional[Decimal]:
        """Whole-token amount, exactly. `None` when either part is missing."""
        if self.raw is None or self.decimals is None:
            return None
        return Decimal(int(self.raw)) / (Decimal(10) ** int(self.decimals))


@dataclass
class Position(Model):
    """One LP position held by the key's wallet, agent-managed or not.

    Read the USD fields the same way as `AgentPerformance`: string or `None`,
    and `None` is unknown, not zero.

    `claimable_fees_usd` is only the currently unclaimed slice sitting on the
    position. `fee_pnl_usd` is LIFETIME fees earned, claimed plus unclaimed.
    The two are different numbers and must not be added together.

    `cost_basis_estimated` marks a position ponk did not open: the basis is
    then a first-observation stamp rather than a true entry, so lifetime PnL
    fields stay `None` instead of asserting a gain.
    """

    id: Optional[str] = None
    dex: Optional[str] = None
    wallet_address: Optional[str] = None
    pool_address: Optional[str] = None
    position_address: Optional[str] = None
    token_x_amount: Optional[str] = None
    token_y_amount: Optional[str] = None
    token_x_decimals: Optional[int] = None
    token_y_decimals: Optional[int] = None
    liquidity: Optional[str] = None
    lower_bin_id: Optional[int] = None
    upper_bin_id: Optional[int] = None
    active_bin_id: Optional[int] = None
    in_range: Optional[bool] = None
    distance_from_active_bin: Optional[int] = None
    fees_claimable_x: Optional[TokenAmount] = None
    fees_claimable_y: Optional[TokenAmount] = None
    value_usd: Optional[str] = None
    updated_at: Optional[str] = None
    cost_basis_usd: Optional[float] = None
    cost_basis_estimated: Optional[bool] = None
    claimed_fees_usd: Optional[float] = None
    pnl_usd: Optional[float] = None
    pnl_pct: Optional[float] = None
    price_pnl_usd: Optional[float] = None
    fee_pnl_usd: Optional[float] = None
    claimable_fees_usd: Optional[float] = None
    rewards_claimable_usd: Optional[float] = None
    il_usd: Optional[float] = None
    pnl_source: Optional[str] = None
    pnl_pool_share: Optional[bool] = None
    fee_apr_pct: Optional[float] = None
    fees_usd_per_day: Optional[float] = None
    pool_fee_pct: Optional[float] = None
    lower_price: Optional[str] = None
    upper_price: Optional[str] = None
    current_price: Optional[str] = None
    band_pct: Optional[float] = None
    price_position_pct: Optional[float] = None
    agent_id: Optional[str] = None
    agent_name: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "fees_claimable_x": TokenAmount,
        "fees_claimable_y": TokenAmount,
    }


@dataclass
class ActionLog(Model):
    """One thing an agent did, or decided not to do.

    `tx_signature` is present once a transaction landed, which is what makes a
    row verifiable on a block explorer. A dry-run agent logs every decision
    here with no signature and no transaction.
    """

    id: Optional[str] = None
    agent_id: Optional[str] = None
    agent_name: Optional[str] = None
    action_type: Optional[str] = None
    status: Optional[str] = None
    tx_signature: Optional[str] = None
    error_message: Optional[str] = None
    decision_reason: Optional[str] = None
    created_at: Optional[str] = None
    completed_at: Optional[str] = None
    pnl_delta_usd: Optional[str] = None
    pnl_cumulative_usd: Optional[str] = None


# --------------------------------------------------------------------------
# Fund movement
# --------------------------------------------------------------------------


@dataclass
class ClaimPayoutToken(Model):
    mint: Optional[str] = None
    amount: Optional[str] = None
    decimals: Optional[int] = None


@dataclass
class ClaimPayout(Model):
    """What a claim actually paid out to the owner's wallet.

    `destination` is resolved by the server, never by the request. `tokens` is
    a list because a stable/stable pool pays two different mints and one summed
    figure would be a balance of nothing.
    """

    destination: Optional[str] = None
    signatures: List[str] = field(default_factory=list)
    sol_lamports: Optional[str] = None
    tokens: List[ClaimPayoutToken] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {"tokens": (ClaimPayoutToken,)}


@dataclass
class ActionReceipt(Model):
    """The receipt for one executed action, such as a compound.

    `status` is the transaction outcome and `action_status` the recorded
    action's own state. `payout` is `None` whenever nothing moved, so a `None`
    payout must be read as "the funds are still in the agent wallet", never as
    a payout that happened.
    """

    action_id: Optional[str] = None
    signature: Optional[str] = None
    status: Optional[str] = None
    action_status: Optional[str] = None
    compute_units_consumed: Optional[int] = None
    error_message: Optional[str] = None
    risk_approved: Optional[bool] = None
    risk_failed_checks: List[str] = field(default_factory=list)
    payout: Optional[ClaimPayout] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"payout": ClaimPayout}


@dataclass
class ExitSnapshot(Model):
    """The frozen final figures for an agent that has just exited.

    Captured at exit time, because once the position is closed the live view of
    it is empty. Money fields are `None` when a leg could not be priced.
    """

    agent_id: Optional[str] = None
    pnl_usd: Optional[float] = None
    pnl_pct: Optional[float] = None
    fees_lifetime_usd: Optional[float] = None
    il_usd: Optional[float] = None
    capital_usd: Optional[float] = None
    swap_fee_bps: Optional[int] = None
    bin_step: Optional[int] = None
    venue: Optional[str] = None
    referral_code: Optional[str] = None
    opened_at: Optional[str] = None
    closed_at: Optional[str] = None
    exit_signature: Optional[str] = None


@dataclass
class Withdrawal(Model):
    """The result of a withdraw or an exit.

    `destination` is server-locked to the wallet that owns the agent: there is
    no destination parameter and there cannot be one. `pnl` is present only on
    an exit that closed a live managed position.
    """

    signature: Optional[str] = None
    lamports: Optional[int] = None
    destination: Optional[str] = None
    source: Optional[str] = None
    pnl: Optional[ExitSnapshot] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"pnl": ExitSnapshot}


# --------------------------------------------------------------------------
# Leaderboard, sharing and follows (0.4.0)
# --------------------------------------------------------------------------


@dataclass
class AgentCopy(Model):
    """What a copy of a shared agent starts from.

    The same allow-listed slice the public profile serves: venue, pool,
    strategy and the strategy config with anything owner-specific removed.
    `PonkClient.copy_agent` turns it into a `create_agent` call.
    """

    dex: Optional[str] = None
    pool_address: Optional[str] = None
    strategy: Optional[str] = None
    config: Optional[Any] = None


@dataclass
class LeaderboardRow(Model):
    """One row of the Top Ponkillionaires board.

    `rank` is `None` on an unranked row and `unranked_reason` says why. USD
    figures arrive as decimal strings and are `None` when the link hides USD
    or the figure cannot be priced; never read `None` as zero. Every other
    field the server sends is in `raw_json`.
    """

    rank: Optional[int] = None
    slug: Optional[str] = None
    dex: Optional[str] = None
    strategy: Optional[str] = None
    pool_address: Optional[str] = None
    token_x_mint: Optional[str] = None
    token_y_mint: Optional[str] = None
    running_since: Optional[str] = None
    vs_hodl_pct: Optional[float] = None
    score_pct: Optional[float] = None
    pnl_pct: Optional[float] = None
    drawdown_pct: Optional[float] = None
    unranked_reason: Optional[str] = None
    owner_key: Optional[str] = None
    handle: Optional[str] = None
    usd_hidden: Optional[bool] = None
    window: Optional[str] = None
    pnl_usd: Optional[str] = None
    roi_pct: Optional[float] = None
    basis_usd: Optional[str] = None
    fees_realized_usd: Optional[str] = None
    fees_earned_usd: Optional[str] = None
    fees_unclaimed_usd: Optional[str] = None
    deposited_usd: Optional[str] = None
    spark: Optional[List[Optional[float]]] = None
    followers: Optional[int] = None
    crew: Optional[str] = None
    win_rate: Optional[Any] = None
    copy: Optional[AgentCopy] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"copy": AgentCopy}


@dataclass
class Leaderboard(Model):
    """`GET /public/agents-leaderboard`: ranked and unranked shared agents.

    `window` is `24h`, `7d`, `30d` or `all`. `totals` is the board header
    (shared, ranked, LPers, crews, verified, summed P&L and fees) as sent.
    """

    ranked: List[LeaderboardRow] = field(default_factory=list)
    unranked: List[LeaderboardRow] = field(default_factory=list)
    window: Optional[str] = None
    min_days: Optional[int] = None
    min_hodl_usd: Optional[float] = None
    min_win_samples: Optional[int] = None
    totals: Optional[Any] = None
    computed_at: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "ranked": (LeaderboardRow,),
        "unranked": (LeaderboardRow,),
    }


@dataclass
class PublicAgent(Model):
    """`GET /public/agents/{slug}`: a shared agent's public profile.

    `performance`, `proof`, `counts`, `win_rate` and `onchain` are kept as the
    server sent them. `performance` is `None` with `performance_unavailable`
    naming the reason; `onchain` is present only when the owner turned proof
    on (and never beside a percent-only link).
    """

    slug: Optional[str] = None
    usd_hidden: Optional[bool] = None
    shared_at: Optional[str] = None
    dex: Optional[str] = None
    pool_address: Optional[str] = None
    token_x_mint: Optional[str] = None
    token_y_mint: Optional[str] = None
    bin_step: Optional[int] = None
    strategy: Optional[str] = None
    status: Optional[str] = None
    running_since: Optional[str] = None
    performance: Optional[Any] = None
    performance_unavailable: Optional[str] = None
    proof: Optional[Any] = None
    counts: Optional[Any] = None
    copy: Optional[AgentCopy] = None
    owner_key: Optional[str] = None
    handle: Optional[str] = None
    followers: Optional[int] = None
    win_rate: Optional[Any] = None
    crew: Optional[str] = None
    onchain: Optional[Any] = None
    computed_at: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"copy": AgentCopy}


@dataclass
class ValueHistory(Model):
    """An agent's position value over time against holding its opening tokens.

    `points` is the bucketed series as sent; a bucket with no reading is
    `None`, never interpolated.
    """

    range: Optional[str] = None
    bucket_secs: Optional[int] = None
    since: Optional[str] = None
    until: Optional[str] = None
    points: Optional[List[Any]] = None
    hodl_since: Optional[str] = None
    positions: Optional[int] = None


@dataclass
class PublicLink(Model):
    """An agent's live public link. `path` is site-relative (`/ponkillionaires/<slug>`).

    `show_onchain` is always `False` while `hide_usd` is set.
    """

    slug: Optional[str] = None
    path: Optional[str] = None
    hide_usd: Optional[bool] = None
    show_onchain: Optional[bool] = None
    created_at: Optional[str] = None


@dataclass
class LinkRevoked(Model):
    revoked: Optional[bool] = None


@dataclass
class Follow(Model):
    """The result of a follow or unfollow, with the agent's follower count."""

    slug: Optional[str] = None
    following: Optional[bool] = None
    followers: Optional[int] = None


@dataclass
class Follows(Model):
    """Slugs this account follows, newest first."""

    slugs: List[str] = field(default_factory=list)


# --------------------------------------------------------------------------
# Points and referrals (0.4.0)
# --------------------------------------------------------------------------


@dataclass
class PointsSeason(Model):
    id: Optional[str] = None
    name: Optional[str] = None
    starts_at: Optional[str] = None
    ends_at: Optional[str] = None
    multiplier: Optional[str] = None


@dataclass
class MyPoints(Model):
    """This account's standing in the active season.

    `season` is `None` between seasons, and the totals are then zero. Points
    arrive as decimal strings. `rank` is `None` until the account has earned.
    """

    season: Optional[PointsSeason] = None
    total_points: Optional[str] = None
    tvl_time_points: Optional[str] = None
    fee_paid_points: Optional[str] = None
    referral_points: Optional[str] = None
    event_count: Optional[int] = None
    rank: Optional[int] = None
    participant_count: Optional[int] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"season": PointsSeason}


@dataclass
class PointsEntry(Model):
    """One leaderboard row. `display_code` is a referral code, never a wallet."""

    rank: Optional[int] = None
    display_code: Optional[str] = None
    total_points: Optional[str] = None
    tvl_time_points: Optional[str] = None
    fee_paid_points: Optional[str] = None
    referral_points: Optional[str] = None
    is_me: Optional[bool] = None


@dataclass
class PointsLeaderboard(Model):
    season: Optional[PointsSeason] = None
    participant_count: Optional[int] = None
    entries: List[PointsEntry] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {"season": PointsSeason, "entries": (PointsEntry,)}


@dataclass
class ReferralCode(Model):
    code: Optional[str] = None
    label: Optional[str] = None
    created_at: Optional[str] = None


@dataclass
class Referral(Model):
    """Referral codes, handle and money for this account.

    EARNED (`total_earned_usd`, reached your wallet), OWED (`owed_usd`, held by
    the treasury for you) and UNPROVEN (`unproven_usd`, booked against a
    signature that did not verify) are three different facts. Never add them.
    """

    code: Optional[str] = None
    codes: List[ReferralCode] = field(default_factory=list)
    referred_by_code: Optional[str] = None
    referred_count: Optional[int] = None
    total_payouts: Optional[int] = None
    total_earned_usd: Optional[float] = None
    back_paid_usd: Optional[float] = None
    back_paid_events: Optional[int] = None
    owed_usd: Optional[float] = None
    owed_events: Optional[int] = None
    owed_unpriced_events: Optional[int] = None
    unproven_usd: Optional[float] = None
    unproven_events: Optional[int] = None
    unpriced_payouts: Optional[int] = None
    creator_handle: Optional[str] = None
    referred_active_agents: Optional[int] = None
    referred_lp_fees_usd: Optional[float] = None
    handle: Optional[str] = None
    creator_application: Optional[Any] = None
    tier: Optional["ReferralTier"] = None
    programme: Optional["ReferralProgramme"] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {}


@dataclass
class CreatorTotals(Model):
    referred_wallets: Optional[int] = None
    referred_with_active_agent: Optional[int] = None
    active_agents: Optional[int] = None
    lp_fees_usd: Optional[float] = None
    ponk_fees_usd: Optional[float] = None
    earned_usd: Optional[float] = None
    fee_events: Optional[int] = None
    unpriced_events: Optional[int] = None


@dataclass
class CreatorDay(Model):
    day: Optional[str] = None
    signups: Optional[int] = None
    lp_fees_usd: Optional[float] = None
    ponk_fees_usd: Optional[float] = None
    earned_usd: Optional[float] = None
    fee_events: Optional[int] = None


@dataclass
class CreatorDashboard(Model):
    """The Creator dashboard: totals and one row per UTC day, oldest first."""

    handle: Optional[str] = None
    granted_at: Optional[str] = None
    totals: Optional[CreatorTotals] = None
    days: List[CreatorDay] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {"totals": CreatorTotals, "days": (CreatorDay,)}


@dataclass
class HandleCheck(Model):
    """Whether a /r/<handle> can be claimed. `reason` is set when it cannot."""

    handle: Optional[str] = None
    available: Optional[bool] = None
    reason: Optional[str] = None


@dataclass
class PublicReferral(Model):
    """`GET /public/referral/{code}`: the code exists; `creator` if it is a Creator's handle."""

    code: Optional[str] = None
    creator: Optional[bool] = None


@dataclass
class ReferralTierStep(Model):
    """One rung of the referral ladder. `share_bps` is bps of ponk's platform fee."""

    tier: Optional[int] = None
    min_fees_usd: Optional[int] = None
    share_bps: Optional[int] = None


@dataclass
class ReferralTier(Model):
    """Your referral tier, from the platform fees your referred users paid.

    One level only: you earn on the people you referred, nobody above you
    earns on them. `current` is 1, 2 or 3 and `share_bps` is your share of
    ponk's platform fee now. `referred_fees_usd` is the trailing
    `window_days` total, `None` when it could not be read; the share is then
    the base rate and no progress should be shown. `next` is the next rung,
    `None` at the top. Every share comes out of ponk's fee and never changes
    what a user pays.
    """

    current: Optional[int] = None
    share_bps: Optional[int] = None
    referred_fees_usd: Optional[float] = None
    window_days: Optional[int] = None
    next: Optional[ReferralTierStep] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"next": ReferralTierStep}


@dataclass
class ReferralProgramme(Model):
    """The programme's constants, lowest rung first, so no client hardcodes a rate."""

    ladder: List[ReferralTierStep] = field(default_factory=list)
    window_days: Optional[int] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"ladder": (ReferralTierStep,)}


Referral._NESTED = {
    "codes": (ReferralCode,),
    "tier": ReferralTier,
    "programme": ReferralProgramme,
}


# -- live agents activity --------------------------------------------------


@dataclass
class ActivityEvent(Model):
    """One event on the live agents strip. It carries no identity at all.

    `kind` is `open`, `rebalance`, `compound`, `claim`, `exit` or `hold`.
    `coarse` means `at` was truncated to the start of its hour (an agent with
    no live public link). `usd` is a decimal string or `None`; `usd_kind`
    says what it is: `fees` (realized net fees of a compound or claim) or
    `margin` (the rebalance gate's benefit minus cost on a hold). Holds also
    carry `reason_key`, `evaluations` and `last_seen_at`.
    """

    key: Optional[str] = None
    kind: Optional[str] = None
    at: Optional[str] = None
    coarse: Optional[bool] = None
    dex: Optional[str] = None
    token_x_mint: Optional[str] = None
    token_y_mint: Optional[str] = None
    usd: Optional[str] = None
    usd_kind: Optional[str] = None
    reason_key: Optional[str] = None
    evaluations: Optional[int] = None
    last_seen_at: Optional[str] = None


@dataclass
class AgentsActivity(Model):
    """`GET /public/agents-activity`: the last 24 hours of board agents' actions
    and every live hold, under the board's privacy. `holding` counts agents
    holding right now."""

    events: List[ActivityEvent] = field(default_factory=list)
    holding: Optional[int] = None
    computed_at: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"events": (ActivityEvent,)}


# -- weekly Ponkillionaires --------------------------------------------------


@dataclass
class WeekPrize(Model):
    """A weekly prize: the agent fee rate for the following week (`fee_rate_pct`,
    2.0 means 2%) plus points."""

    rank: Optional[int] = None
    fee_rate_pct: Optional[float] = None
    points: Optional[int] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class WeekStanding(Model):
    """A row of this week's race. `prize` is what the rank would win if the week
    ended now, `None` for a row in loss. Identity follows the owner's board
    privacy: `owner_key` always, `handle`, `display_name` and `wallet` only
    when they chose to show them."""

    rank: Optional[int] = None
    pnl_usd: Optional[float] = None
    agents: Optional[int] = None
    deployed_usd: Optional[float] = None
    prize: Optional[WeekPrize] = None
    owner_key: Optional[str] = None
    handle: Optional[str] = None
    display_name: Optional[str] = None
    wallet: Optional[str] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"prize": WeekPrize}


@dataclass
class WeekWinner(Model):
    """A settled winner. The prize rate applies from `rate_from` to `rate_until`."""

    rank: Optional[int] = None
    pnl_usd: Optional[float] = None
    agents: Optional[int] = None
    fee_rate_pct: Optional[float] = None
    points: Optional[float] = None
    rate_from: Optional[str] = None
    rate_until: Optional[str] = None
    owner_key: Optional[str] = None
    handle: Optional[str] = None
    display_name: Optional[str] = None
    wallet: Optional[str] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class LastWeek(Model):
    week_start: Optional[str] = None
    settled: Optional[bool] = None
    winners: List[WeekWinner] = field(default_factory=list)

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"winners": (WeekWinner,)}


@dataclass
class PonkillionairesWeek(Model):
    """`GET /public/ponkillionaires/week`: this week's race for the top three and
    last week's winners. The week resets Monday 00:00 UTC (`resets_at`).
    `min_peak_usd` and `min_running_days` are the eligibility floors."""

    week_start: Optional[str] = None
    resets_at: Optional[str] = None
    prizes: List[WeekPrize] = field(default_factory=list)
    min_peak_usd: Optional[int] = None
    min_running_days: Optional[int] = None
    standings: List[WeekStanding] = field(default_factory=list)
    last_week: Optional[LastWeek] = None
    computed_at: Optional[str] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {
        "prizes": (WeekPrize,),
        "standings": (WeekStanding,),
        "last_week": LastWeek,
    }


# -- X Tracker -----------------------------------------------------------------


@dataclass
class XAccount(Model):
    """A tracked X account. `handle` is lowercase; `screen_name` keeps its case."""

    handle: Optional[str] = None
    screen_name: Optional[str] = None
    name: Optional[str] = None
    avatar_url: Optional[str] = None
    followers: Optional[int] = None
    category: Optional[str] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class XTokenRef(Model):
    mint: Optional[str] = None
    symbol: Optional[str] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class XAuthor(Model):
    screen_name: Optional[str] = None
    name: Optional[str] = None
    avatar_url: Optional[str] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class XMetrics(Model):
    views: Optional[int] = None
    likes: Optional[int] = None
    replies: Optional[int] = None
    reposts: Optional[int] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class XPost(Model):
    """A post on the feed. `tokens` are the Solana mints it names. `media`,
    `quote` and `card` are passed through as decoded JSON."""

    id: Optional[str] = None
    kind: Optional[str] = None
    url: Optional[str] = None
    text: Optional[str] = None
    created_at: Optional[str] = None
    seen_at: Optional[str] = None
    latency_secs: Optional[int] = None
    is_backfill: Optional[bool] = None
    reply_to: Optional[str] = None
    author: Optional[XAuthor] = None
    media: List[Any] = field(default_factory=list)
    quote: Optional[Any] = None
    card: Optional[Any] = None
    metrics: Optional[XMetrics] = None
    tokens: List[XTokenRef] = field(default_factory=list)
    cashtags: List[str] = field(default_factory=list)
    deleted_at: Optional[str] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {
        "author": XAuthor,
        "metrics": XMetrics,
        "tokens": (XTokenRef,),
    }


@dataclass
class XEventPost(Model):
    id: Optional[str] = None
    url: Optional[str] = None
    text: Optional[str] = None
    created_at: Optional[str] = None
    tokens: List[XTokenRef] = field(default_factory=list)

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"tokens": (XTokenRef,)}


@dataclass
class XEvent(Model):
    """A change on a tracked account (a profile edit, a deleted post), with
    `before` and `after` where they apply."""

    id: Optional[int] = None
    kind: Optional[str] = None
    before: Optional[str] = None
    after: Optional[str] = None
    detail: Optional[Any] = None
    at: Optional[str] = None
    post: Optional[XEventPost] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"post": XEventPost}


@dataclass
class XFeedItem(Model):
    """One feed row. `type` is `post` (then `post` is set) or `event` (then `event`)."""

    type: Optional[str] = None
    key: Optional[str] = None
    account: Optional[XAccount] = None
    post: Optional[XPost] = None
    event: Optional[XEvent] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"account": XAccount, "post": XPost, "event": XEvent}


@dataclass
class XSource(Model):
    """The feed's data source. `state` is `live`, `degraded`, `down` or `starting`."""

    name: Optional[str] = None
    url: Optional[str] = None
    state: Optional[str] = None
    last_ok_at: Optional[str] = None
    last_error_at: Optional[str] = None
    last_error: Optional[str] = None
    requests_1h: Optional[int] = None
    rate_limited_1h: Optional[int] = None
    median_latency_secs: Optional[int] = None
    accounts_fresh: Optional[int] = None
    accounts_active: Optional[int] = None
    hot_secs: Optional[int] = None
    normal_secs: Optional[int] = None
    cold_secs: Optional[int] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class XFeed(Model):
    """A page of the X feed, newest first. Pass `next_before` back as `before`
    for the next page; it is `None` at the end."""

    items: List[XFeedItem] = field(default_factory=list)
    next_before: Optional[str] = None
    source: Optional[XSource] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"items": (XFeedItem,), "source": XSource}


@dataclass
class XFirstCall(Model):
    """The first tracked account to post a token. `price_usd` and `mcap_usd` are
    the read when the call was seen, `None` when it was seen too late to know."""

    handle: Optional[str] = None
    screen_name: Optional[str] = None
    avatar_url: Optional[str] = None
    post_id: Optional[str] = None
    url: Optional[str] = None
    at: Optional[str] = None
    price_usd: Optional[float] = None
    mcap_usd: Optional[float] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class XTrendingToken(Model):
    """A token trending on X. `multiple` is price now over the first call's
    price, `None` without both."""

    mint: Optional[str] = None
    symbol: Optional[str] = None
    name: Optional[str] = None
    accounts_1h: Optional[int] = None
    accounts_24h: Optional[int] = None
    mentions_24h: Optional[int] = None
    last_at: Optional[str] = None
    first_call: Optional[XFirstCall] = None
    price_usd: Optional[float] = None
    mcap_usd: Optional[float] = None
    multiple: Optional[float] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"first_call": XFirstCall}


@dataclass
class XTrending(Model):
    window: Optional[str] = None
    tokens: List[XTrendingToken] = field(default_factory=list)
    computed_at: Optional[str] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"tokens": (XTrendingToken,)}


@dataclass
class XCaller(Model):
    """A caller's record over the window. Medians and `best` are price multiples
    after the call; `hit_rate` is the share of calls with a 24h mark that were
    up (a fraction), `None` until one has a 24h mark."""

    account: Optional[XAccount] = None
    calls: Optional[int] = None
    priced_calls: Optional[int] = None
    median_1h: Optional[float] = None
    median_24h: Optional[float] = None
    best: Optional[float] = None
    hit_rate: Optional[float] = None
    scored_24h: Optional[int] = None
    last_call_at: Optional[str] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"account": XAccount}


@dataclass
class XCallers(Model):
    days: Optional[int] = None
    callers: List[XCaller] = field(default_factory=list)
    computed_at: Optional[str] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"callers": (XCaller,)}


@dataclass
class XTokenMention(Model):
    account: Optional[XAccount] = None
    post_id: Optional[str] = None
    url: Optional[str] = None
    text: Optional[str] = None
    kind: Optional[str] = None
    at: Optional[str] = None
    price_usd: Optional[float] = None
    mcap_usd: Optional[float] = None
    deleted: Optional[bool] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"account": XAccount}


@dataclass
class XToken(Model):
    """Every tracked mention of one token, its first call and its multiple since."""

    mint: Optional[str] = None
    symbol: Optional[str] = None
    accounts_1h: Optional[int] = None
    accounts_24h: Optional[int] = None
    first_call: Optional[XFirstCall] = None
    price_usd: Optional[float] = None
    multiple: Optional[float] = None
    mentions: List[XTokenMention] = field(default_factory=list)

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"first_call": XFirstCall, "mentions": (XTokenMention,)}


@dataclass
class XDirectoryEntry(Model):
    """A directory account. `status` is `active` or `not_found`. `followed`,
    `muted` and `alerting` are always false signed out."""

    handle: Optional[str] = None
    screen_name: Optional[str] = None
    name: Optional[str] = None
    avatar_url: Optional[str] = None
    followers: Optional[int] = None
    category: Optional[str] = None
    is_default: Optional[bool] = None
    status: Optional[str] = None
    last_ok_at: Optional[str] = None
    followed: Optional[bool] = None
    muted: Optional[bool] = None
    alerting: Optional[bool] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class XDirectory(Model):
    accounts: List[XDirectoryEntry] = field(default_factory=list)
    hidden_categories: List[str] = field(default_factory=list)
    categories: List[str] = field(default_factory=list)

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"accounts": (XDirectoryEntry,)}


# -- Wallet Tracker ------------------------------------------------------------


@dataclass
class TrackedWalletRef(Model):
    """Who a feed row belongs to. `source` is `ponkillionaire`, `agent`,
    `notable`, `mine`, or `None` for a wallet read on demand. `agent_slug` is
    the public profile at `/ponkillionaires/<slug>` when there is one."""

    address: Optional[str] = None
    label: Optional[str] = None
    emoji: Optional[str] = None
    color: Optional[str] = None
    source: Optional[str] = None
    agent_slug: Optional[str] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class WalletLeg(Model):
    """One token movement. `amount` is a base-unit string, `dir` is `in` or `out`."""

    mint: Optional[str] = None
    symbol: Optional[str] = None
    amount: Optional[str] = None
    decimals: Optional[int] = None
    dir: Optional[str] = None
    usd: Optional[float] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class WalletEvent(Model):
    """A swap or liquidity action by a tracked wallet, with its signature."""

    key: Optional[str] = None
    signature: Optional[str] = None
    kind: Optional[str] = None
    venue: Optional[str] = None
    pool: Optional[str] = None
    position: Optional[str] = None
    token_mint: Optional[str] = None
    legs: List[WalletLeg] = field(default_factory=list)
    usd: Optional[float] = None
    block_time: Optional[str] = None
    seen_at: Optional[str] = None
    is_backfill: Optional[bool] = None
    wallet: Optional[TrackedWalletRef] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"legs": (WalletLeg,), "wallet": TrackedWalletRef}


@dataclass
class WalletSource(Model):
    """The chain reader's state: `live`, `slowed` or `starting`."""

    state: Optional[str] = None
    wallets_fresh: Optional[int] = None
    wallets_tracked: Optional[int] = None
    last_ok_at: Optional[str] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class WalletFeed(Model):
    """A page of wallet activity, newest first. `next_before` is the cursor."""

    items: List[WalletEvent] = field(default_factory=list)
    next_before: Optional[str] = None
    source: Optional[WalletSource] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"items": (WalletEvent,), "source": WalletSource}


@dataclass
class CuratedWallet(Model):
    """A wallet on the curated list and where it comes from (`kind`)."""

    address: Optional[str] = None
    label: Optional[str] = None
    kind: Optional[str] = None
    agent_slug: Optional[str] = None
    source: Optional[str] = None
    muted: Optional[bool] = None
    last_activity_at: Optional[str] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class CuratedWallets(Model):
    defaults: List[CuratedWallet] = field(default_factory=list)

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"defaults": (CuratedWallet,)}


@dataclass
class WalletActivity(Model):
    """Any wallet's recent swaps and LP actions. `mode` is `tracked` (served
    from what the tracker stored) or `live` (read from chain for this request,
    with no paging)."""

    address: Optional[str] = None
    mode: Optional[str] = None
    items: List[WalletEvent] = field(default_factory=list)
    next_before: Optional[str] = None
    last_ok_at: Optional[str] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"items": (WalletEvent,)}


# --------------------------------------------------------------------------
# The agent fleet: batches, funding and the fleet read (0.6.0)
# --------------------------------------------------------------------------


@dataclass
class AgentWalletCreated(Model):
    """The answer to `create_agent_wallet`: the agent's own managed wallet.

    `created` is true when this call made the wallet and false when an existing
    active one was returned, so calling it twice is safe. An empty wallet
    grants no authority: nothing signs from it until the owner signs a custody
    mandate in the app.
    """

    pubkey: Optional[str] = None
    status: Optional[str] = None
    created_at: Optional[str] = None
    created: Optional[bool] = None


@dataclass
class FleetBatchItem(Model):
    """One agent's outcome inside a batch.

    `outcome` is `done`, `failed` or `running`. `running` is not a failure: the
    work is still going on the server, detached from the request, and
    `get_batch(batch_id)` collects it. A `failed` item always carries `error`
    (a plain sentence) plus `status` and `code` to branch on. `result` is the
    single-agent call's own answer, verbatim: for a create it is
    `{"agent": ..., "wallet": ...}` (plus `wallet_error` when the wallet could
    not be made, which never undoes the create), for an exit or a withdraw it
    is the `Withdrawal` shape. `agent_id` is `None` on a create that failed.
    """

    index: Optional[int] = None
    agent_id: Optional[str] = None
    outcome: Optional[str] = None
    ok: Optional[bool] = None
    status: Optional[int] = None
    code: Optional[str] = None
    error: Optional[str] = None
    result: Optional[Any] = None

    def agent(self) -> Optional[Agent]:
        """The created agent on a create item, or `None`."""
        if isinstance(self.result, Mapping) and isinstance(self.result.get("agent"), Mapping):
            return Agent.from_dict(self.result["agent"])
        return None

    def withdrawal(self) -> Optional[Withdrawal]:
        """The receipt on a done exit or withdraw item, or `None`."""
        if isinstance(self.result, Mapping) and "signature" in self.result:
            return Withdrawal.from_dict(self.result)
        return None


@dataclass
class FleetBatch(Model):
    """A create, exit or withdraw over many agents.

    `kind` is `create`, `exit` or `withdraw`, and `done + failed + running ==
    total`. The server answers once its fund deadline passes; whatever is
    still in flight then is `running`, and the batch keeps going after the
    response. Poll `get_batch(batch_id)` until `running == 0`. A finished
    batch is kept for 30 minutes.
    """

    batch_id: Optional[str] = None
    kind: Optional[str] = None
    total: Optional[int] = None
    done: Optional[int] = None
    failed: Optional[int] = None
    running: Optional[int] = None
    items: List[FleetBatchItem] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {"items": (FleetBatchItem,)}


@dataclass
class FundItem(Model):
    """One agent in a funding plan.

    `wallet` is the agent's own managed wallet, resolved by the server under
    the owner scope; it is never taken from the request. `ok` false carries
    `refusal`, a sentence saying why the agent was left out (not on this
    account, already funded, or an amount that would leave it below the
    `recommended_open_lamports` it needs to open). `needs_mandate` is true
    when the agent cannot act on these funds until its owner signs a custody
    mandate in the app.
    """

    agent_id: Optional[str] = None
    ok: Optional[bool] = None
    wallet: Optional[str] = None
    dex: Optional[str] = None
    pool_address: Optional[str] = None
    lamports: Optional[int] = None
    current_lamports: Optional[int] = None
    recommended_open_lamports: Optional[int] = None
    needs_mandate: Optional[bool] = None
    refusal: Optional[str] = None


@dataclass
class FundTransfer(Model):
    """One SystemProgram transfer inside a funding transaction."""

    agent_id: Optional[str] = None
    to: Optional[str] = None
    lamports: Optional[int] = None


@dataclass
class FundTransaction(Model):
    """One unsigned funding transaction.

    `unsigned_tx_b64` is a legacy Solana transaction, base64 of the bincode
    wire format, with the owner's wallet as fee payer and only signer. The
    server simulated it against mainnet before returning it (`simulated`).
    This client never signs it.
    """

    index: Optional[int] = None
    unsigned_tx_b64: Optional[str] = None
    transfers: List[FundTransfer] = field(default_factory=list)
    total_lamports: Optional[int] = None
    simulated: Optional[bool] = None
    units_consumed: Optional[int] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"transfers": (FundTransfer,)}


@dataclass
class FundPlan(Model):
    """Unsigned transfers from your wallet to your agents' own wallets.

    `owner` is the fee payer and the only signer: the wallet the key belongs
    to. The transfers are packed into as few transactions as the packet size
    allows (ten fit in one). `ok` false carries `refusal` and no transaction,
    for example when the balance cannot cover `total_lamports` plus
    `network_fee_lamports` plus the rent-exempt minimum a wallet must keep.

    Two ways to sign, neither of them this client:

    * open each of `sign_urls` on ponk.exchange with the wallet connected; the
      page rebuilds the same transfers fresh, asks the wallet to sign, then
      queues the custody mandate each agent in `needs_mandate` still needs;
    * or sign `transactions[i].unsigned_tx_b64` in wallet software you already
      control, before `last_valid_block_height`, and send it yourself.
    """

    owner: Optional[str] = None
    ok: Optional[bool] = None
    refusal: Optional[str] = None
    balance_lamports: Optional[int] = None
    total_lamports: Optional[int] = None
    network_fee_lamports: Optional[int] = None
    recent_blockhash: Optional[str] = None
    last_valid_block_height: Optional[int] = None
    transactions: List[FundTransaction] = field(default_factory=list)
    items: List[FundItem] = field(default_factory=list)
    needs_mandate: List[str] = field(default_factory=list)
    sign_urls: List[str] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "transactions": (FundTransaction,),
        "items": (FundItem,),
    }


@dataclass
class FleetRow(Model):
    """One agent with its wallet, mandate and performance.

    `wallet` is `None` before the agent has its own wallet and `mandate` is
    `None` when none was ever signed. A part that is `None` because its read
    FAILED, rather than because there is nothing, is named in `read_errors`.
    So `performance is None` with empty `read_errors` means "nothing to
    price", and with a non-empty one means "could not be read right now".
    """

    agent: Optional[Agent] = None
    wallet: Optional[AgentWallet] = None
    mandate: Optional[Mandate] = None
    performance: Optional[AgentPerformance] = None
    read_errors: List[str] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "agent": Agent,
        "wallet": AgentWallet,
        "mandate": Mandate,
        "performance": AgentPerformance,
    }


@dataclass
class Fleet(Model):
    """One page of the fleet. `next_offset` is `None` on the last page.

    `limit` defaults to 25 and the server clamps it to 1 to 100.
    `PonkClient.iter_fleet` follows `next_offset` for you.
    """

    total: Optional[int] = None
    offset: Optional[int] = None
    limit: Optional[int] = None
    next_offset: Optional[int] = None
    rows: List[FleetRow] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {"rows": (FleetRow,)}


# --------------------------------------------------------------------------
# /v1 account parity (0.6.0)
# --------------------------------------------------------------------------


@dataclass
class RebalanceReceipt(Model):
    """The receipt for a managed manual rebalance.

    Read it like `ActionReceipt`: a refusal at signing time (an expired
    mandate, a risk halt, the 60 second cooldown after a confirmed rebalance)
    answers 200 with a failed `status` and `signature is None`.
    `position_address` is the new position once one opened.
    """

    action_id: Optional[str] = None
    signature: Optional[str] = None
    status: Optional[str] = None
    action_status: Optional[str] = None
    position_address: Optional[str] = None
    compute_units_consumed: Optional[int] = None
    error_message: Optional[str] = None
    risk_approved: Optional[bool] = None
    risk_failed_checks: List[str] = field(default_factory=list)


@dataclass
class TimeInRange(Model):
    """How long the position was in range over the measured window.

    `pct` is `None` when nothing was observed, never zero.
    """

    in_range_secs: Optional[int] = None
    observed_secs: Optional[int] = None
    pct: Optional[float] = None
    readings: Optional[int] = None


@dataclass
class CostLeg(Model):
    """One cost of an action. `usd` is `None`, with `reason`, when it was not measured."""

    usd: Optional[str] = None
    reason: Optional[str] = None


@dataclass
class ActionCosts(Model):
    """What one action cost: network, swap, rent and the ponk fee.

    `total_usd` is `None` unless every leg is known. `modelled_usd` is what the
    engine's cost model predicted before acting.
    """

    network: Optional[CostLeg] = None
    swap: Optional[CostLeg] = None
    rent: Optional[CostLeg] = None
    ponk_fee: Optional[CostLeg] = None
    total_usd: Optional[str] = None
    modelled_usd: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "network": CostLeg,
        "swap": CostLeg,
        "rent": CostLeg,
        "ponk_fee": CostLeg,
    }


@dataclass
class ProofAction(Model):
    """One action the agent took, what it cost, and what it earned while held."""

    kind: Optional[str] = None
    at: Optional[str] = None
    tx_signature: Optional[str] = None
    trigger_key: Optional[str] = None
    costs: Optional[ActionCosts] = None
    earned_usd: Optional[str] = None
    earned_reason: Optional[str] = None
    held_secs: Optional[int] = None
    open: Optional[bool] = None
    within_measured_range: Optional[bool] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"costs": ActionCosts}


@dataclass
class AgentProof(Model):
    """Whether the agent's actions paid for themselves, read from the ledger.

    `earned_known` of `earned_countable` actions have a measured earning and
    `cost_legs_known` of `cost_legs_total` cost legs a measured cost. The USD
    totals cover only those, and are `None` when none is known.
    """

    window_hours: Optional[int] = None
    since: Optional[str] = None
    time_in_range: Optional[TimeInRange] = None
    gap_cap_secs: Optional[int] = None
    actions: List[ProofAction] = field(default_factory=list)
    earned_usd: Optional[str] = None
    earned_known: Optional[int] = None
    earned_countable: Optional[int] = None
    measured_cost_usd: Optional[str] = None
    cost_legs_known: Optional[int] = None
    cost_legs_total: Optional[int] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "time_in_range": TimeInRange,
        "actions": (ProofAction,),
    }


@dataclass
class RangePoint(Model):
    """One bucket of the range history: the active bin against the range edges."""

    t: Optional[str] = None
    active: Optional[int] = None
    lower: Optional[int] = None
    upper: Optional[int] = None
    in_range: Optional[bool] = None
    price: Optional[float] = None
    lower_price: Optional[float] = None
    upper_price: Optional[float] = None


@dataclass
class RangeHistory(Model):
    """Where the price sat against the agent's range over time."""

    range: Optional[str] = None
    bucket_secs: Optional[int] = None
    since: Optional[str] = None
    until: Optional[str] = None
    dex: Optional[str] = None
    pool_address: Optional[str] = None
    price_now: Optional[float] = None
    active_now: Optional[int] = None
    in_range_share: Optional[float] = None
    points: List[RangePoint] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {"points": (RangePoint,)}


@dataclass
class PortfolioPosition(Model):
    """One position in the portfolio summary. `None` is unpriced, not zero."""

    dex: Optional[str] = None
    pool_address: Optional[str] = None
    value_usd: Optional[float] = None
    cost_basis_usd: Optional[float] = None
    pnl_usd: Optional[float] = None
    pnl_pct: Optional[float] = None
    claimable_fees_usd: Optional[float] = None
    price_pnl_usd: Optional[float] = None
    fee_pnl_usd: Optional[float] = None
    il_usd: Optional[float] = None
    vs_hodl_usd: Optional[float] = None
    fee_apr_pct: Optional[float] = None
    fees_per_day_usd: Optional[float] = None
    fees_earned_usd: Optional[float] = None


@dataclass
class PortfolioSeriesPoint(Model):
    """`t` is unix seconds."""

    t: Optional[int] = None
    value_usd: Optional[float] = None


@dataclass
class Portfolio(Model):
    """The account's LP portfolio, the summary the app's Portfolio shows.

    `vs_hodl_usd` covers `vs_hodl_measured` of `vs_hodl_positions` positions.
    """

    total_position_value_usd: Optional[float] = None
    all_time_deposit_usd: Optional[float] = None
    fees_claimed_usd: Optional[float] = None
    claimable_fees_usd: Optional[float] = None
    total_pnl_usd: Optional[float] = None
    total_pnl_pct: Optional[float] = None
    avg_invested_usd: Optional[float] = None
    win_rate_pct: Optional[float] = None
    biggest_win_usd: Optional[float] = None
    position_count: Optional[int] = None
    price_pnl_usd: Optional[float] = None
    fee_pnl_usd: Optional[float] = None
    il_usd: Optional[float] = None
    vs_hodl_usd: Optional[float] = None
    vs_hodl_measured: Optional[int] = None
    vs_hodl_positions: Optional[int] = None
    fee_apr_pct: Optional[float] = None
    positions: List[PortfolioPosition] = field(default_factory=list)
    series: List[PortfolioSeriesPoint] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "positions": (PortfolioPosition,),
        "series": (PortfolioSeriesPoint,),
    }


@dataclass
class PortfolioHistoryPoint(Model):
    """`value_usd` is `None` for a bucket holding an unpriced position (`unpriced`)."""

    t: Optional[str] = None
    value_usd: Optional[float] = None
    fees_usd: Optional[float] = None
    realized_usd: Optional[float] = None
    unpriced: Optional[int] = None


@dataclass
class CalendarDay(Model):
    """One UTC day of fees and realized PnL. `date` is `YYYY-MM-DD`."""

    date: Optional[str] = None
    fees_usd: Optional[float] = None
    realized_usd: Optional[float] = None
    pnl_usd: Optional[float] = None
    events: Optional[int] = None
    unpriced: Optional[int] = None


@dataclass
class PortfolioHistory(Model):
    """The account's value over time, plus a daily calendar."""

    range: Optional[str] = None
    bucket_secs: Optional[int] = None
    since: Optional[str] = None
    until: Optional[str] = None
    points: List[PortfolioHistoryPoint] = field(default_factory=list)
    incomplete_buckets: Optional[int] = None
    positions: Optional[int] = None
    days: List[CalendarDay] = field(default_factory=list)
    first_record_at: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "points": (PortfolioHistoryPoint,),
        "days": (CalendarDay,),
    }


@dataclass
class Transaction(Model):
    """One transaction an agent sent, with its on-chain outcome."""

    id: Optional[str] = None
    agent_id: Optional[str] = None
    agent_name: Optional[str] = None
    signature: Optional[str] = None
    status: Optional[str] = None
    dex: Optional[str] = None
    action_type: Optional[str] = None
    priority_fee_lamports: Optional[int] = None
    compute_units: Optional[int] = None
    error_message: Optional[str] = None
    created_at: Optional[str] = None
    confirmed_at: Optional[str] = None
    pnl_delta_usd: Optional[str] = None


@dataclass
class RiskEvent(Model):
    """Something the risk engine flagged or refused."""

    id: Optional[str] = None
    agent_id: Optional[str] = None
    agent_name: Optional[str] = None
    severity: Optional[str] = None
    code: Optional[str] = None
    message: Optional[str] = None
    created_at: Optional[str] = None


@dataclass
class FeeEvent(Model):
    """One platform fee taken on claimed LP fees.

    Amounts are base-unit strings of `fee_token_mint`. The USD figures are
    `None` when the token could not be priced.
    """

    id: Optional[str] = None
    agent_id: Optional[str] = None
    wallet_address: Optional[str] = None
    dex: Optional[str] = None
    pool_address: Optional[str] = None
    position_address: Optional[str] = None
    fee_bps: Optional[int] = None
    fee_token_mint: Optional[str] = None
    gross_fee_amount: Optional[str] = None
    platform_fee_amount: Optional[str] = None
    user_net_amount: Optional[str] = None
    gross_fee_usd: Optional[str] = None
    platform_fee_usd: Optional[str] = None
    user_net_usd: Optional[str] = None
    platform_fee_wallet: Optional[str] = None
    platform_fee_signature: Optional[str] = None
    status: Optional[str] = None
    error_message: Optional[str] = None
    created_at: Optional[str] = None
    confirmed_at: Optional[str] = None


@dataclass
class FeeSummary(Model):
    """Totals over every fee event of the account, as decimal strings."""

    total_platform_fees_usd: Optional[str] = None
    total_claimed_fees_usd: Optional[str] = None
    total_net_fees_usd: Optional[str] = None
    event_count: Optional[int] = None


@dataclass
class Notification(Model):
    """One notification from the app's bell."""

    id: Optional[str] = None
    agent_id: Optional[str] = None
    kind: Optional[str] = None
    severity: Optional[str] = None
    title: Optional[str] = None
    message: Optional[str] = None
    status: Optional[str] = None
    metadata: Optional[Any] = None
    created_at: Optional[str] = None
    read_at: Optional[str] = None
    dismissed_at: Optional[str] = None


@dataclass
class UnreadCount(Model):
    unread: Optional[int] = None


@dataclass
class ReadAll(Model):
    """How many notifications `mark_all_notifications_read` marked."""

    marked: Optional[int] = None


@dataclass
class Recommendation(Model):
    """A recommendation an agent raised for its owner to act on.

    Approving one flips its status only. The approval builds and signs no
    transaction.
    """

    id: Optional[str] = None
    agent_id: Optional[str] = None
    recommendation_type: Optional[str] = None
    status: Optional[str] = None
    dex: Optional[str] = None
    pool_address: Optional[str] = None
    position_address: Optional[str] = None
    reason: Optional[str] = None
    strategy_snapshot: Optional[Any] = None
    risk_result: Optional[Any] = None
    transaction_preview: Optional[Any] = None
    created_at: Optional[str] = None
    expires_at: Optional[str] = None
    reviewed_at: Optional[str] = None
    acted_at: Optional[str] = None
    dismissed_at: Optional[str] = None


@dataclass
class NotifySettings(Model):
    """Which Telegram messages the account receives."""

    rebalance_telegram: Optional[bool] = None
    mandate_telegram: Optional[bool] = None
    weekly_recap_telegram: Optional[bool] = None


@dataclass
class Preset(Model):
    """A saved agent preset. `overrides` is the normalized document the server stored.

    `dex` is `None` for "any venue except Ponk Clouds".
    """

    id: Optional[str] = None
    name: Optional[str] = None
    overrides: Optional[Any] = None
    dex: Optional[str] = None
    created_at: Optional[str] = None
