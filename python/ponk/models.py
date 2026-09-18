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
    "ActionLog",
    "ActionReceipt",
    "Agent",
    "AgentPerformance",
    "AgentPosition",
    "AgentWallet",
    "BinShare",
    "ClaimPayout",
    "ClaimPayoutToken",
    "ComponentStatus",
    "ExitSnapshot",
    "FeeRates",
    "Health",
    "HealthChecks",
    "PonkPerks",
    "PoolSnapshot",
    "Position",
    "TokenAmount",
    "WhoAmI",
    "Withdrawal",
]

T = TypeVar("T", bound="Model")


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

    @classmethod
    def from_dict(cls: Type[T], data: Any) -> T:
        if not isinstance(data, Mapping):
            raise TypeError(
                "{0} expected a JSON object, got {1}".format(cls.__name__, type(data).__name__)
            )
        kwargs: Dict[str, Any] = {}
        for f in dataclasses.fields(cls):  # type: ignore[arg-type]
            key = cls._ALIASES.get(f.name, f.name)
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
