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
    "CallersBoard",
    "CallsFeed",
    "EvmLegShape",
    "FeedPrefs",
    "RecapBestAgent",
    "WalletEvmSource",
    "WeeklyRecap",
    "XPostToken",
    "EvmBoard",
    "EvmBoardStatus",
    "EvmChain",
    "EvmChains",
    "EvmFarm",
    "EvmFleet",
    "EvmFleetAgent",
    "EvmHook",
    "EvmOpenCost",
    "EvmPlan",
    "EvmPlanAgent",
    "EvmPlanAlternative",
    "EvmPlanChain",
    "EvmPlanDropped",
    "EvmPlanLeg",
    "EvmPool",
    "EvmReceived",
    "EvmToken",
    "EvmWindows",
    "FarmRewards",
    "ActionCosts",
    "ActionLog",
    "ActionReceipt",
    "ActivityEvent",
    "ActivityRow",
    "Agent",
    "AgentCopy",
    "AgentPerformance",
    "AgentPosition",
    "AgentProof",
    "AgentVisibility",
    "AgentWallet",
    "AgentWalletCreated",
    "AgentsActivity",
    "AutonomousState",
    "BatchOutcome",
    "BatchResult",
    "BinShare",
    "CalendarDay",
    "Call",
    "CallPreview",
    "CardLook",
    "ClaimPayout",
    "ClaimPayoutToken",
    "ComponentStatus",
    "CopilotFleet",
    "CopilotFleetSummary",
    "CopilotParse",
    "CopilotPlan",
    "CopilotReplay",
    "CopilotStart",
    "CopyAction",
    "CopyFollow",
    "CopyFollows",
    "CopyLog",
    "CopyLogEntry",
    "CopyReferralAttach",
    "CopyReferralOffer",
    "CostLeg",
    "CreatorDashboard",
    "CreatorDay",
    "CreatorTotals",
    "CuratedWallet",
    "CuratedWallets",
    "DepositCraft",
    "ExitSnapshot",
    "FeeEvent",
    "FeeRates",
    "FeeSummary",
    "Fleet",
    "FleetBatch",
    "FleetBatchItem",
    "FleetRow",
    "Follow",
    "FollowLeader",
    "FollowOffer",
    "FollowState",
    "FollowStatus",
    "Follows",
    "FundItem",
    "FundPlan",
    "FundTransaction",
    "FundTransfer",
    "HandleCheck",
    "Health",
    "HealthChecks",
    "JupiterChallenge",
    "JupiterSession",
    "LastWeek",
    "Leaderboard",
    "LeaderboardPrivacy",
    "LeaderboardRow",
    "LinkRevoked",
    "LogsCleared",
    "Mandate",
    "MandateRow",
    "Mandates",
    "MarkedTx",
    "MyFollow",
    "MyFollowing",
    "MyPoints",
    "NotCopied",
    "Notification",
    "NotifySettings",
    "OpenPosition",
    "OrderBatch",
    "OrderBatchItem",
    "OrderCancel",
    "OrderCreated",
    "OrderPlan",
    "PeopleBoard",
    "PeopleList",
    "PeopleSearch",
    "Person",
    "PersonBoardRow",
    "PersonPositions",
    "RecommendedPerson",
    "Recommendations",
    "SearchHit",
    "PointsEntry",
    "PointsLeaderboard",
    "PointsSeason",
    "PonkPerks",
    "PonkillionairesWeek",
    "PoolHistory",
    "PoolHistoryCurrent",
    "PoolHistoryEdge",
    "PoolHistoryPoint",
    "PoolHistorySource",
    "PoolHistorySources",
    "PoolHistoryWindow",
    "PoolSnapshot",
    "Portfolio",
    "PortfolioHistory",
    "PortfolioHistoryPoint",
    "PortfolioPosition",
    "PortfolioSeriesPoint",
    "Position",
    "Prepared",
    "Preset",
    "PresetRank",
    "Privacy",
    "PrivacySettings",
    "Profile",
    "ProofAction",
    "PublicAgent",
    "PublicLink",
    "PublicReferral",
    "PushSettings",
    "PushTest",
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
    "SocialFeed",
    "StrandedPositions",
    "StreamEvent",
    "TelegramShared",
    "TelegramStatus",
    "TimeInRange",
    "TokenAmount",
    "TopCalls",
    "TrackedLp",
    "TrackedLps",
    "TrackedWalletRef",
    "Transaction",
    "UnreadCount",
    "ValueHistory",
    "WalletActivity",
    "WalletEvent",
    "WalletFeed",
    "WalletFollow",
    "WalletLeg",
    "WalletPnl",
    "WalletSource",
    "WeekPrize",
    "WeekStanding",
    "WeekWinner",
    "WhoAmI",
    "Withdrawal",
    "XAccount",
    "XAccountCard",
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

    On `meteora_damm_v2` a position is a share of the whole pool with no
    bins, so `bin_step` and `active_bin_id` arrive as `0` and carry no
    meaning there.

    `price_divergence` is the pool price against the pair's market price, a
    non-negative fraction (0.02 is 2%), `None` when either leg is unpriced.
    `mispricing_warning` and `sell_warning` are one plain sentence each, or
    `None`: the pool trading more than 3% off the market, and the honeypot
    sell check finding a reason a leg may not be sellable (a risky Token-2022
    extension, no route back into SOL, or a round trip worth a small fraction
    of its cost). Both are advice, never a block.
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
    token_price: Optional[float] = None
    token_price_quote_mint: Optional[str] = None
    token_price_base_mint: Optional[str] = None
    price_divergence: Optional[float] = None
    mispricing_warning: Optional[str] = None
    sell_warning: Optional[str] = None

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
class PoolHistoryPoint(Model):
    """One bucket at least one source has a figure for.

    `ts` is the bucket open in unix seconds (UTC aligned) and `date` the same
    instant in RFC 3339. A field no source has for this bucket is `None`,
    never zero. `fees_usd` is every swap fee paid, protocol share included;
    `lp_fees_usd` is the part that reached liquidity providers and
    `protocol_fees_usd` the rest. `fee_rate_pct` is percent of volume paid as
    fees in the bucket.
    """

    ts: Optional[int] = None
    date: Optional[str] = None
    volume_usd: Optional[float] = None
    fees_usd: Optional[float] = None
    lp_fees_usd: Optional[float] = None
    protocol_fees_usd: Optional[float] = None
    fee_rate_pct: Optional[float] = None
    tvl_usd: Optional[float] = None


@dataclass
class PoolHistoryWindow(Model):
    """The span a pool history covers: `[from_, to)`, unix seconds, bucket aligned.

    `to` is the open of the in-progress bucket, which is never served.
    `buckets_served` of `buckets_in_window` carry a point; the rest are
    absent. `omitted` counts buckets a source returned and the route
    deliberately dropped (`before_pool_created`, and
    `no_recorded_activity` for zero-filled Meteora rows that cannot be told
    apart from a gap). `pool_created_at` is `None` when the venue does not
    say.
    """

    _ALIASES: ClassVar[Mapping[str, str]] = {"from_": "from"}

    bucket_secs: Optional[int] = None
    from_: Optional[int] = None
    to: Optional[int] = None
    from_iso: Optional[str] = None
    to_iso: Optional[str] = None
    buckets_in_window: Optional[int] = None
    buckets_served: Optional[int] = None
    omitted: Optional[Dict[str, int]] = None
    pool_created_at: Optional[int] = None


@dataclass
class PoolHistorySource(Model):
    """How one field's figures were obtained.

    `status` is `ok` (read as published), `derived` (plain arithmetic on read
    figures; `detail` states it), `not_published` (no source has it for this
    venue at this resolution, so every point carries `None`), `unavailable`
    (the source failed this time) or `busy` (ponk's budget for the source is
    spent this minute; ask again shortly). `source` is `meteora_datapi`,
    `geckoterminal`, `orca_api` or `raydium_api`, `None` when nothing was read.
    """

    status: Optional[str] = None
    source: Optional[str] = None
    detail: Optional[str] = None


@dataclass
class PoolHistorySources(Model):
    """Where each series field of a pool history came from."""

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "volume_usd": PoolHistorySource,
        "fees_usd": PoolHistorySource,
        "lp_fees_usd": PoolHistorySource,
        "protocol_fees_usd": PoolHistorySource,
        "fee_rate_pct": PoolHistorySource,
        "tvl_usd": PoolHistorySource,
    }

    volume_usd: Optional[PoolHistorySource] = None
    fees_usd: Optional[PoolHistorySource] = None
    lp_fees_usd: Optional[PoolHistorySource] = None
    protocol_fees_usd: Optional[PoolHistorySource] = None
    fee_rate_pct: Optional[PoolHistorySource] = None
    tvl_usd: Optional[PoolHistorySource] = None


@dataclass
class PoolHistoryCurrent(Model):
    """The pool as its venue describes it now, next to the series.

    `base_fee_pct` is the fee tier (Orca, Raydium) or the base half of the
    fee (Meteora). `fee_is_dynamic` is `None` when the venue states nothing.
    `lp_share_pct` is the percent of each fee that reaches LPs.
    """

    tvl_usd: Optional[float] = None
    base_fee_pct: Optional[float] = None
    fee_is_dynamic: Optional[bool] = None
    lp_share_pct: Optional[float] = None
    source: Optional[str] = None


@dataclass
class PoolHistoryEdge(Model):
    """The LVR edge NOW, never a bucket's: ponk stores no past readings.

    `status` is `measured`, `unmeasured` (inputs too thin to read),
    `not_measured_recently` (nothing measured this pool in the last ten
    minutes; `GET /pools/{dex}/{address}/edge` measures it) or `unavailable`.
    `verdict` is `pays`, `thin` or `bleeds` when measured. `reading` is the
    engine's reading as sent (`r_min`, `binding_window_secs`, `windows`).
    """

    status: Optional[str] = None
    verdict: Optional[str] = None
    reading: Optional[Dict[str, Any]] = None
    admit_r: Optional[float] = None
    measured_at: Optional[int] = None


@dataclass
class PoolHistory(Model):
    """One pool's volume, fees, fee rate and TVL per day or hour.

    Every per-bucket figure is proxied from the venue's own data API or
    GeckoTerminal, or derived from proxied figures by plain arithmetic, and
    `sources` says which per field. ABSENT IS ABSENT: a bucket no source has a
    figure for is not in `points` at all, and nothing is zero filled or
    interpolated. `points` are oldest first. `dex` is `meteora_dlmm`, `orca`
    or `raydium`.
    """

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "window": PoolHistoryWindow,
        "sources": PoolHistorySources,
        "points": (PoolHistoryPoint,),
        "current": PoolHistoryCurrent,
        "edge": PoolHistoryEdge,
    }

    dex: Optional[str] = None
    pool: Optional[str] = None
    resolution: Optional[str] = None
    window: Optional[PoolHistoryWindow] = None
    sources: Optional[PoolHistorySources] = None
    points: Optional[List[PoolHistoryPoint]] = None
    current: Optional[PoolHistoryCurrent] = None
    edge: Optional[PoolHistoryEdge] = None
    generated_at: Optional[int] = None


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
    #: The one-time activation fee on an agent's first open: 1%, or 0.5% for
    #: a wallet holding 1,000,000 $PONK.
    activation_fee: Optional[FeeRates] = None
    #: The exit fee on what an agent that has opened a position sends its
    #: owner, of the amount withdrawn: 1%, or 0.5% for a holder. An agent that
    #: never opened a position pays none.
    exit_fee: Optional[FeeRates] = None

    _ALIASES: ClassVar[Mapping[str, str]] = {
        "activation_fee": "activationFee",
        "exit_fee": "exitFee",
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
        "activation_fee": FeeRates,
        "exit_fee": FeeRates,
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
    actually charges, before any $PONK holder discount, and
    `activation_fee_bps` the one-time fee on its first open: 1500 and 150 for
    an agent a key created. `exit_fee_bps` is the exit fee on what the agent
    sends you once it has opened a position, in basis points of the amount
    withdrawn, before the $PONK holder rate: 100 (1%; 50 for a wallet holding
    1,000,000 PONK), on agents created in the app and through the API alike.
    It is 0 for a non-managed agent, and an agent that never opened a
    position pays nothing whatever this says.

    `chain` is always present: `solana`, `ethereum`, `arbitrum`, `base`,
    `bsc` or `robinhood`. On an EVM agent `dex` is an EVM venue id,
    `pool_address` and `position_address` are CAIP ids (`ponk.evm`),
    `wallet_address` is still the owner's Solana login wallet (the account;
    the agent's own EVM address is on `agent_wallet`), `wallet_source` is
    `managed` and `mode` is `auto`.
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
    activation_fee_bps: Optional[int] = None
    exit_fee_bps: Optional[int] = None
    chain: Optional[str] = None
    waiting: Optional[Any] = None
    pool_warning: Optional[str] = None


@dataclass
class FarmRewards(Model):
    """Farm rewards on an EVM agent or position: CAKE or AERO.

    `pending_amount` is whole tokens as a decimal string; `pending_usd` and
    `harvested_usd` are USD strings. `apr_pct` is a number in PERCENT (8.2 is
    8.2%), paid in `token` on top of swap fees, and `apr_basis` says what it
    is the APR of: `position` when the agent's position is staked on a
    concentrated venue, where the figure is the position's own rate at this
    instant and exactly 0 while it is out of range; `pool` otherwise, the
    pool's figure (`EvmFarm.apr_pct`). Both are `None` when the farm is not
    live or the figure cannot be computed.
    """

    token: Optional[str] = None
    pending_amount: Optional[str] = None
    pending_usd: Optional[str] = None
    harvested_usd: Optional[str] = None
    staked: Optional[bool] = None
    apr_pct: Optional[float] = None
    apr_basis: Optional[str] = None


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

    `pnl_pct` and `drawdown_pct` are FRACTIONS on every chain: 0.05 is 5%.

    On an EVM agent every dollar figure is USD from the agent's own priced
    chain read, and one the runtime does not record is `None`. `pnl_usd` is
    position value plus unclaimed fees minus the capital put in
    (`cost_basis_usd`, also `deposits_usd`): the value at the open plus every
    later top-up. `farm_rewards` is the CAKE or AERO block, `None` on a venue
    with no farm. `figures_unavailable` is the one sentence that says why the
    live figures are `None` when they are (no wallet yet, the chain could not
    be read, no position, a token with no price).
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
    chain: Optional[str] = None
    vs_hodl_usd: Optional[str] = None
    hodl_value_usd: Optional[str] = None
    pnl_note: Optional[str] = None
    farm_rewards: Optional[FarmRewards] = None
    figures_unavailable: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"farm_rewards": FarmRewards}


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

    On an EVM range venue the three ids are TICKS, `bin_step` is the tick
    spacing and the prices are token1 per token0 as decimal strings. On a
    full-range venue `is_full_range` is true and the ticks and prices are
    `None`. `staked` says whether the position is staked for farm rewards.

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
    bin_step: Optional[int] = None
    chain: Optional[str] = None
    staked: Optional[bool] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"bins": (BinShare,)}


@dataclass
class AgentWallet(Model):
    """The isolated wallet an autonomous agent owns.

    `min_open_lamports` is the native-SOL reserve the executor always holds
    back, so a wallet at or below it cannot deploy.
    `recommended_open_lamports` clears that floor and leaves enough to open a
    small position. `scope` is `per_agent` for the agent's own wallet, or
    `shared` for the legacy per-user wallet.

    AN EVM AGENT answers a different set of fields, and the Solana ones above
    are then `None`. `address` is the agent's own address on `chain`, EIP-55:
    the owner sends the gas coin (`native_symbol`, ETH or BNB) there from a
    wallet they already hold on that chain; nothing is bridged. `balance_wei`,
    `reserve_wei` (gas always kept back to exit), `min_open_wei` (what the
    first open needs: the reserve plus the measured gas of the open, before
    the amount to deploy) and `withdrawable_wei` are WEI as decimal strings;
    `balance_usd` and `native_usd` (the gas coin's price) are USD strings.
    `withdrawal_address` is the one address withdrawals and exits pay, set by
    the mandate, `None` until one is granted. `mandate_status` is `active`,
    `revoked`, `expired`, or `None` with no mandate yet: the mandate is signed
    by the owner's own wallets on ponk.exchange (`ponk.evm.fund_link`).
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
    address: Optional[str] = None
    chain: Optional[str] = None
    native_symbol: Optional[str] = None
    balance_wei: Optional[str] = None
    reserve_wei: Optional[str] = None
    min_open_wei: Optional[str] = None
    withdrawable_wei: Optional[str] = None
    balance_usd: Optional[str] = None
    native_usd: Optional[str] = None
    withdrawal_address: Optional[str] = None
    mandate_status: Optional[str] = None

    @property
    def is_evm(self) -> bool:
        """True for the wallet of an agent on an EVM chain."""
        return self.address is not None and self.pubkey is None


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
    chain: Optional[str] = None


@dataclass
class MandateRow(Model):
    """One autonomous agent's mandate on the `mandates()` list. `state` is
    `active`, `expiring`, `expired` or `none`; `renew_url` opens this agent's
    renewal with its previous terms preselected."""

    agent_id: Optional[str] = None
    name: Optional[str] = None
    state: Optional[str] = None
    expires_at: Optional[Any] = None
    seconds_remaining: Optional[int] = None
    renew_url: Optional[str] = None


@dataclass
class Mandates(Model):
    """Every autonomous agent's mandate, the most urgent first."""

    expiring_within_days: Optional[int] = None
    needs_renewal: Optional[int] = None
    renew_all_url: Optional[str] = None
    rows: Optional[List[MandateRow]] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"rows": (MandateRow,)}


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
    """A token amount in base units, with the mint's decimals.

    `raw` is a number on a Solana row and a decimal string on an EVM row,
    where wei amounts do not fit a float. `to_decimal` reads both exactly.
    """

    raw: Optional[Any] = None
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

    `chain` is on every row. An EVM agent's position is its own row: `id` and
    `position_address` are the position's CAIP id, `wallet_address` the
    agent's address, `token_x` / `token_y` token ids, the bin ids TICKS, and
    `fees_claimable_x.raw` a decimal STRING (wei does not fit a float).
    `pnl_pct` on this list is PERCENT on every chain (2.0 is 2%), unlike the
    fraction on `AgentPerformance`. `farm_rewards`, `farm_rewards_usd` and
    `staked` carry the CAKE or AERO side.
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
    chain: Optional[str] = None
    token_x: Optional[str] = None
    token_y: Optional[str] = None
    bin_step: Optional[int] = None
    is_full_range: Optional[bool] = None
    deposited_usd: Optional[float] = None
    fees_earned_usd: Optional[float] = None
    farm_rewards: Optional[FarmRewards] = None
    farm_rewards_usd: Optional[float] = None
    staked: Optional[bool] = None
    vs_hodl_usd: Optional[float] = None
    figures_unavailable: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "fees_claimable_x": TokenAmount,
        "fees_claimable_y": TokenAmount,
        "farm_rewards": FarmRewards,
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
class ExitSwapLeg(Model):
    """One leftover token of an exit-to-SOL liquidation.

    `outcome` is `swapped` (sold to SOL; `signature`, `sol_out_lamports` and
    `fee_lamports` are set, the fee being ponk's 0.2%), `kept` (sent as it is;
    `reason` says why) or `outcome_unknown` (sent but not confirmed in time;
    never retried, `signature` names it).
    """

    mint: Optional[str] = None
    amount: Optional[int] = None
    decimals: Optional[int] = None
    outcome: Optional[str] = None
    signature: Optional[str] = None
    sol_out_lamports: Optional[int] = None
    fee_lamports: Optional[int] = None
    reason: Optional[str] = None


@dataclass
class EvmReceived(Model):
    """One transfer that landed at the withdrawal address.

    `token` is a token id (`eip155:<chainId>:0x...`; the zero address is the
    gas coin) and `amount` is BASE UNITS as a decimal string. `evm_token`
    gives a token's decimals and symbol.
    """

    token: Optional[str] = None
    amount: Optional[str] = None
    tx_hash: Optional[str] = None


@dataclass
class Withdrawal(Model):
    """The result of a withdraw or an exit.

    `destination` is server-locked to the wallet that owns the agent: there is
    no destination parameter and there cannot be one. `pnl` is present only on
    an exit that closed a live managed position. `exit_swaps` and `exit_notes`
    are present only on an exit that swapped (or tried to swap) leftover tokens
    to SOL: one leg per token, and one plain sentence per outcome.

    ON AN EVM AGENT the funds go only to the mandate's proven
    `withdrawal_address` on the same chain, and the answer carries what THIS
    call sent, from the sweep's own receipts: `tx_hashes` (every hash, in
    order; `signature` is the last, `None` when nothing was sent),
    `amount_wei` (the gas coin that landed, WEI as a decimal string; `"0"`
    when only tokens were sent, `None` when nothing was), `received` (one
    entry per transfer that landed), `status` (`completed`, or `incomplete`
    while something has not landed and the agent keeps retrying it by itself:
    calling again is safe and never pays twice) and `sentence`, what happened
    in plain words. `lamports` is 0.

    THE EXIT FEE. A withdrawal or an exit from an agent that has opened a
    position pays 1% of what leaves (0.5% for a wallet holding 1,000,000
    PONK), taken in kind and sent to the treasury in the same run. On an EVM
    agent `exit_fees` lists what the fee took, one entry per asset in base
    units (what you did NOT receive; empty when none was taken), and
    `exit_fee_bps` the rate the call ran under, `None` when it took none.
    """

    signature: Optional[str] = None
    lamports: Optional[int] = None
    destination: Optional[str] = None
    source: Optional[str] = None
    pnl: Optional[ExitSnapshot] = None
    exit_swaps: List[ExitSwapLeg] = field(default_factory=list)
    exit_notes: List[str] = field(default_factory=list)
    tx_hashes: List[str] = field(default_factory=list)
    amount_wei: Optional[str] = None
    received: List[EvmReceived] = field(default_factory=list)
    chain: Optional[str] = None
    status: Optional[str] = None
    sentence: Optional[str] = None
    exit_fees: List[EvmReceived] = field(default_factory=list)
    exit_fee_bps: Optional[int] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "pnl": ExitSnapshot,
        "exit_swaps": (ExitSwapLeg,),
        "received": (EvmReceived,),
        "exit_fees": (EvmReceived,),
    }


@dataclass
class ExitToSol(Model):
    """An agent's exit-to-SOL setting (`/v1/agents/{id}/exit-to-sol`).

    `exit_to_sol` true (the default): the agent's exits, the ones you start
    and the ones its own stops trigger, swap leftover tokens to SOL with
    ponk's 0.2% swap fee. False: tokens are kept as they are. `fee_line` is
    the sentence every exit confirm shows.
    """

    agent_id: Optional[str] = None
    exit_to_sol: Optional[bool] = None
    fee_line: Optional[str] = None


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

    `chain` is `solana` or the EVM chain the agent runs on. On an EVM row
    `dex` is an EVM venue id and `pool_address`, `token_x_mint` and
    `token_y_mint` are CAIP ids (`ponk.evm.parse_evm_id`).
    """

    rank: Optional[int] = None
    slug: Optional[str] = None
    chain: Optional[str] = None
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
    `chains` is the chain ids the answer covers: every chain unless the
    request named some, and then `rank` is numbered from 1 over the rows kept
    and `totals` are their totals.
    """

    ranked: List[LeaderboardRow] = field(default_factory=list)
    unranked: List[LeaderboardRow] = field(default_factory=list)
    window: Optional[str] = None
    chains: List[str] = field(default_factory=list)
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

    An EVM agent answers from its own recorded readings: 15 minute buckets
    for the last 26 hours, hourly to 8 days, four hourly to 31 days, and
    nothing older, so `range="all"` reaches back 31 days at most.
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
    """A token an event's post named. `chain` is `solana`, the EVM chain the
    token is on (then `mint` is its CAIP id), or `evm` for a bare address the
    post did not place on one chain."""

    mint: Optional[str] = None
    symbol: Optional[str] = None
    chain: Optional[str] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class XPostToken(Model):
    """A token a post names, with how it has done since that post.

    `chain` is `solana`, the EVM chain the token is on (then `mint` is its
    CAIP id, `eip155:<chain id>:0x...`), or `evm` for a bare `0x` address
    that is a token on several chains in a post that does not say which:
    then `mint` is the bare address, `chains` lists the candidate chains, and
    the price, first call and multiple are `None`. `chains` is empty
    otherwise. The call figures are the reads taken when the post was seen
    (`None` when it was seen too late), the others the read now; `multiple`
    is price now over the call price.
    """

    mint: Optional[str] = None
    symbol: Optional[str] = None
    chain: Optional[str] = None
    chains: List[str] = field(default_factory=list)
    call_price_usd: Optional[float] = None
    call_mcap_usd: Optional[float] = None
    price_usd: Optional[float] = None
    mcap_usd: Optional[float] = None
    liquidity_usd: Optional[float] = None
    multiple: Optional[float] = None
    first_call: Optional[bool] = None

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
    """A post on the feed. `tokens` are the tokens it names, each with its
    chain (`XPostToken`). `media`, `quote` and `card` are passed through as
    decoded JSON."""

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
    tokens: List[XPostToken] = field(default_factory=list)
    cashtags: List[str] = field(default_factory=list)
    deleted_at: Optional[str] = None

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {
        "author": XAuthor,
        "metrics": XMetrics,
        "tokens": (XPostToken,),
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
    for the next page; it is `None` at the end. `chains` is the chain ids the
    page covers."""

    items: List[XFeedItem] = field(default_factory=list)
    next_before: Optional[str] = None
    source: Optional[XSource] = None
    chains: List[str] = field(default_factory=list)

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
    price, `None` without both. `chain` is `solana`, the EVM chain the token
    is on, or `evm` with `chains` (the candidates) for an address the posts
    did not place on one chain."""

    mint: Optional[str] = None
    chain: Optional[str] = None
    chains: List[str] = field(default_factory=list)
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
    """Tokens trending on X. `chains` is the chain ids the ranking is
    computed over."""

    window: Optional[str] = None
    tokens: List[XTrendingToken] = field(default_factory=list)
    chains: List[str] = field(default_factory=list)
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
    """The X caller board. `chains` is the chain ids it is computed over: a
    caller's record under one chain is their calls on it."""

    days: Optional[int] = None
    callers: List[XCaller] = field(default_factory=list)
    chains: List[str] = field(default_factory=list)
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
    """Every tracked mention of one token, its first call and its multiple since.

    `chain` is `solana` or the EVM chain the token is on. `price_source` says
    where an EVM token's price came from (`chainlink`, `onchain_pool`,
    `coingecko`, `defillama` or `dexscreener`); `None` on Solana.
    """

    mint: Optional[str] = None
    chain: Optional[str] = None
    price_source: Optional[str] = None
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
    """A swap or liquidity action by a tracked wallet, with its signature.

    `chain` is `solana` or the EVM chain the transaction is on, and
    `explorer_url` the transaction on Solscan or that chain's explorer. On an
    EVM event `signature` is the transaction hash, `pool`, `position`,
    `token_mint` and each leg's `mint` are CAIP ids, and `venue` is an EVM
    dex id (`uniswap_v3`, `pancakeswap_infinity_cl`, `aerodrome_slipstream`,
    ...) or `other`.
    """

    key: Optional[str] = None
    chain: Optional[str] = None
    explorer_url: Optional[str] = None
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
class WalletEvmSource(Model):
    """The wallet reader's state on one EVM chain.

    `state` is `live`, `slowed` or `starting`. `wallets_tracked` is the
    wallets carried on the chain and `wallets_read` those read at least once.
    `cadence` is `webhook` when the provider pushes the chain's activity
    (events in seconds) or `poll` when the tracker reads on a timer, every
    `poll_minutes` for a wallet with alerts on.
    """

    chain: Optional[str] = None
    state: Optional[str] = None
    wallets_tracked: Optional[int] = None
    wallets_read: Optional[int] = None
    last_ok_at: Optional[str] = None
    cadence: Optional[str] = None
    poll_minutes: Optional[int] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class WalletSource(Model):
    """The chain reader's state: `live`, `slowed` or `starting`.

    `evm` is one entry per EVM chain the tracker reads, BNB Chain first. It
    is empty while EVM wallets are switched off, so offer a chain only when
    it is listed.
    """

    state: Optional[str] = None
    wallets_fresh: Optional[int] = None
    wallets_tracked: Optional[int] = None
    last_ok_at: Optional[str] = None
    evm: List[WalletEvmSource] = field(default_factory=list)

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"evm": (WalletEvmSource,)}


@dataclass
class WalletFeed(Model):
    """A page of wallet activity, newest first. `next_before` is the cursor.
    `chains` is the chain ids the page covers."""

    items: List[WalletEvent] = field(default_factory=list)
    next_before: Optional[str] = None
    source: Optional[WalletSource] = None
    chains: List[str] = field(default_factory=list)

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
    from what the tracker stored), `live` (a Solana wallet read from chain
    for this request, with no paging) or `untracked` (an EVM wallet nobody
    tracks yet: no items)."""

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

    For an agent on an EVM chain the answer is the wallet view instead:
    `address` (the deposit address on `chain`, EIP-55), `native_symbol`,
    `min_open_wei` and `mandate_status`, with the rest in `raw_json`. Read it
    in full with `agent_wallet`.
    """

    pubkey: Optional[str] = None
    status: Optional[str] = None
    created_at: Optional[str] = None
    created: Optional[bool] = None
    address: Optional[str] = None
    chain: Optional[str] = None
    native_symbol: Optional[str] = None
    min_open_wei: Optional[str] = None
    mandate_status: Optional[str] = None


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
# The copilot (wallet session only)
# --------------------------------------------------------------------------


@dataclass
class CopilotParse(Model):
    """What ponk's closed grammar read from one sentence.

    `ok` true carries `intent` (`kind` is `deploy`, `stop_fleet` or
    `show_fleet`; a deploy carries `verb`, `budget`, `per_agent`, `count`,
    `pools`, `risk`, `risk_stated`, `range_pct`, `fleet_stop_pct`) and `echo`,
    the intent restated for the user to confirm. `ok` false carries `refusal`:
    `code`, `message`, `unknown_words` and `examples`.
    """

    ok: Optional[bool] = None
    intent: Optional[Dict[str, Any]] = None
    echo: Optional[str] = None
    refusal: Optional[Dict[str, Any]] = None


@dataclass
class CopilotReplay(Model):
    """A plan's 30 day replay. `state` is `idle`, `running` or `done`.

    `result` (when done) holds per-leg `legs` and fleet `totals` with
    `net_pnl_usd`, `hold_pnl_usd` and `net_vs_hold_usd` side by side, plus
    `statement`. A leg the engine could not replay honestly is `refused` with
    its `reason` and is left out of the totals (`complete` false).
    """

    state: Optional[str] = None
    done: Optional[int] = None
    total: Optional[int] = None
    result: Optional[Dict[str, Any]] = None


@dataclass
class CopilotPlan(Model):
    """A plan: `state` is `building`, `ready`, `refused` or `expired`.

    `plan` (when ready) is the server's plan verbatim: `intent`, `echo`,
    `inputs`, `sizing`, `legs` (pool with both mints and their verification,
    funding and deposit lamports, range, strategy, config), `totals` (with
    `ponk_fee_bps`, the Ponk fee on fees earned for this wallet, and
    `ponk_fee_holder_bps`), `fleet_stop`, `notes`, `skipped` (each pool
    looked at and not used, with its reason) and `resolved_tokens` (each
    token you named and the mint it was resolved to). The copilot plans
    Meteora DLMM, Orca and Raydium pools. A leg on a pool your sentence named
    that is below the pool checks carries `warnings` (one sentence each);
    starting the plan accepts them. A `refusal` with code `unverified_token`
    or `ambiguous_token` carries `suggestions`: the tokens with that symbol
    (`typed`, `mint`, `symbol`, `name`, `liquidity_usd`, `verified`), most
    liquid first; put a mint into your sentence to plan it. `can_start` is true only for a ready start plan
    with at least one leg whose 30 day replay is done. An expired plan
    carries `refusal` with code `plan_expired`. `poll_after_ms` is set while
    something is still moving.
    """

    plan_id: Optional[str] = None
    state: Optional[str] = None
    progress: Optional[str] = None
    text: Optional[str] = None
    created_at: Optional[str] = None
    expires_in_secs: Optional[int] = None
    plan: Optional[Dict[str, Any]] = None
    refusal: Optional[Dict[str, Any]] = None
    replay: Optional[CopilotReplay] = None
    can_start: Optional[bool] = None
    poll_after_ms: Optional[int] = None
    signed_in: Optional[bool] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"replay": CopilotReplay}


@dataclass
class CopilotStart(Model):
    """What a start created and what is left for your wallet to sign.

    `created` is the create batch (one item per leg). `funding` is the
    UNSIGNED funding plan, `None` when nothing could be funded (then `next`
    says why). `sign_url` is the ponk.exchange page that signs the funding
    and then each mandate in `mandate_queue`. `already_started` true means the
    plan was started before: `fleet_id`, `next` and `funding_state` are set.
    `funding_state` is what the fleet still needs from your wallet: `agents`
    (each with `agent_id`, `pair`, `planned_lamports`, `wallet_lamports`,
    `funded`, `mandate_needed`), `unfunded`, `mandates_needed`, `creating`
    and a fresh `sign_url` for what is left.
    """

    fleet_id: Optional[str] = None
    already_started: Optional[bool] = None
    created: Optional[FleetBatch] = None
    funding: Optional[FundPlan] = None
    mandate_queue: List[str] = field(default_factory=list)
    sign_url: Optional[str] = None
    next: Optional[str] = None
    funding_state: Optional[Dict[str, Any]] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"created": FleetBatch, "funding": FundPlan}


@dataclass
class CopilotFleetSummary(Model):
    """One fleet. `status` is `active`, `stopping` or `stopped`."""

    id: Optional[str] = None
    intent_text: Optional[str] = None
    status: Optional[str] = None
    agents: Optional[int] = None
    stop_drawdown_pct: Optional[float] = None
    stop_armed: Optional[bool] = None
    stop_reason: Optional[str] = None
    stop_batch_id: Optional[str] = None
    breach_count: Optional[int] = None
    last_eval: Optional[Dict[str, Any]] = None
    last_eval_at: Optional[str] = None
    stopped_at: Optional[str] = None
    created_at: Optional[str] = None


@dataclass
class CopilotFleet(Model):
    """A fleet live.

    `totals` has `value_usd`, `cost_basis_usd`, `pnl_usd` over the same
    in-market agents the fleet stop counts (each `None`, with `reason`, when
    any of them cannot be valued), `agents_counted`, `unvalued` and `exited`.
    `stop_now` is the fleet stop evaluated on this read: `state` `waiting`,
    `holding`, `breached` or `fire`, with `drawdown_pct` and `threshold_usd`.
    `exit` (once the fleet is stopping) has `done`, `failed`, `running` and
    `failed_agents` by name. `funding` is what the fleet still needs from
    your wallet, as on `CopilotStart.funding_state`. `missing_agents` lists
    agents whose row could not be read.
    """

    fleet: Optional[CopilotFleetSummary] = None
    stop_now: Optional[Dict[str, Any]] = None
    totals: Optional[Dict[str, Any]] = None
    rows: List[FleetRow] = field(default_factory=list)
    missing_agents: List[str] = field(default_factory=list)
    as_of: Optional[str] = None
    poll_after_ms: Optional[int] = None
    exit: Optional[Dict[str, Any]] = None
    funding: Optional[Dict[str, Any]] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"fleet": CopilotFleetSummary, "rows": (FleetRow,)}


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
    #: What the fee was charged on: `activation` (an agent's one-time fee on
    #: the capital of its first open), `manual_lp` or `telegram_lp` (a manual
    #: position's yield), or `None` for an agent's yield fee.
    source: Optional[str] = None


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
    """Which Telegram messages the account receives.

    `comments_telegram`: a reply to your comment, or a comment on your call,
    is DMed. `copy_alerts`: every action a Copy LP follow completed is pushed
    and DMed (the bell row is written either way).
    """

    rebalance_telegram: Optional[bool] = None
    mandate_telegram: Optional[bool] = None
    weekly_recap_telegram: Optional[bool] = None
    comments_telegram: Optional[bool] = None
    copy_alerts: Optional[bool] = None


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


# --------------------------------------------------------------------------
# 0.7.0: swap orders (Jupiter Trigger V2 and Recurring), routes/v1_orders.rs
# --------------------------------------------------------------------------


@dataclass
class OrderPlan(Model):
    """`POST /v1/orders/plan`: the orders, validated, and the one link where
    the owner's wallet signs them all. Nothing was created or signed.

    Open `sign_url` with the wallet named in `wallet` connected. Expiry counts
    from when the page is signed. Jupiter's 10 USD minimum per order and per
    DCA round is checked there at the live price, before any signature.
    """

    sign_url: Optional[str] = None
    wallet: Optional[str] = None
    orders: Optional[List[Any]] = None
    cancel: Optional[List[Any]] = None
    cancel_all: Optional[bool] = None
    signatures: Optional[str] = None
    notes: Optional[List[str]] = None


@dataclass
class JupiterChallenge(Model):
    """Jupiter's sign-in challenge. Sign `challenge` (a text message) with the
    wallet, or for `type == "transaction"` sign `transaction` (a memo-only
    transaction that is never sent). Signing it moves nothing."""

    type: Optional[str] = None
    challenge: Optional[str] = None
    transaction: Optional[str] = None


@dataclass
class JupiterSession(Model):
    """The wallet's Jupiter session. Pass `access_token` as `jupiter_session`
    to every order call. It lapses after about 15 minutes: `orders_refresh`
    with `refresh_token` gives a new one. This client never stores either."""

    _CAMEL: ClassVar[bool] = True

    access_token: Optional[str] = None
    refresh_token: Optional[str] = None
    expires_at: Optional[str] = None


@dataclass
class DepositCraft(Model):
    """An UNSIGNED deposit transaction (base64) for the owner's wallet, and the
    `request_id` to create the order with. Check `amount` and `mint` against
    what you asked for before signing."""

    _CAMEL: ClassVar[bool] = True

    transaction: Optional[str] = None
    request_id: Optional[str] = None
    receiver_address: Optional[str] = None
    mint: Optional[str] = None
    amount: Optional[str] = None
    token_decimals: Optional[int] = None


@dataclass
class OrderCreated(Model):
    """A created order. `tx_signature` is the deposit Jupiter landed."""

    _CAMEL: ClassVar[bool] = True

    id: Optional[str] = None
    tx_signature: Optional[str] = None
    deposit_confirmed: Optional[bool] = None


@dataclass
class OrderCancel(Model):
    """Cancel step one: the order is stopped and its withdrawal comes back as
    an UNSIGNED transaction plus `request_id`. Step two
    (`confirm_order_cancel`) lands it, and `tx_signature` is set then."""

    _CAMEL: ClassVar[bool] = True

    id: Optional[str] = None
    transaction: Optional[str] = None
    request_id: Optional[str] = None
    refund_amount: Optional[str] = None
    tx_signature: Optional[str] = None


@dataclass
class OrderBatchItem(Model):
    """One order in a batch cancel or confirm. `result` is Jupiter's answer
    (typed by `cancel()`); `error` carries `code`, `message` and the HTTP
    `status`. A code of `outcome_unknown` means the withdrawal may have
    landed: read the orders, never retry blindly."""

    family: Optional[str] = None
    id: Optional[str] = None
    ok: Optional[bool] = None
    result: Optional[Any] = None
    error: Optional[Any] = None

    def cancel(self) -> Optional["OrderCancel"]:
        return OrderCancel.from_dict(self.result) if isinstance(self.result, Mapping) else None


@dataclass
class OrderBatch(Model):
    """A batch cancel (`prepared`) or a batch confirm (`confirmed`)."""

    total: Optional[int] = None
    prepared: Optional[int] = None
    confirmed: Optional[int] = None
    failed: Optional[int] = None
    items: List[OrderBatchItem] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {"items": (OrderBatchItem,)}


# --------------------------------------------------------------------------
# 0.7.0: copy, live follow and Copy LP (routes/copy_follow.rs, copy_mirror.rs)
# --------------------------------------------------------------------------


@dataclass
class FollowLeader(Model):
    """The leader as the board already names it. Never its id or wallet."""

    name: Optional[str] = None
    slug: Optional[str] = None
    key: Optional[str] = None


@dataclass
class FollowOffer(Model):
    """A pending pool move or exit the follower may accept or decline."""

    kind: Optional[str] = None
    pool_address: Optional[str] = None
    at: Optional[str] = None


@dataclass
class NotCopied(Model):
    keys: Optional[List[str]] = None
    reason: Optional[str] = None


@dataclass
class FollowStatus(Model):
    """A copy's link to the agent it copied. It mirrors CONFIG, never funds."""

    copied: Optional[bool] = None
    following: Optional[bool] = None
    leader: Optional[FollowLeader] = None
    auto_follow_pool_moves: Optional[bool] = None
    pool_moves_enabled: Optional[bool] = None
    mirrored_keys: Optional[List[str]] = None
    paused_keys: Optional[List[str]] = None
    offer: Optional[FollowOffer] = None
    last_mirrored_at: Optional[str] = None
    last_error: Optional[str] = None
    not_copied: List[NotCopied] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "leader": FollowLeader,
        "offer": FollowOffer,
        "not_copied": (NotCopied,),
    }


@dataclass
class CopyFollow(Model):
    agent_id: Optional[str] = None
    agent_name: Optional[str] = None
    status: Optional[FollowStatus] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"status": FollowStatus}


@dataclass
class CopyFollows(Model):
    follows: List[CopyFollow] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {"follows": (CopyFollow,)}


@dataclass
class WalletFollow(Model):
    """A Copy LP mirror: which wallet, how big (`size_pct` of the leader,
    capped by `max_usd_per_action`), which pools, and auto or ask."""

    leader_address: Optional[str] = None
    leader_position: Optional[str] = None
    size_pct: Optional[float] = None
    max_usd_per_action: Optional[float] = None
    pools: Optional[List[str]] = None
    min_tvl_usd: Optional[float] = None
    auto_mirror: Optional[bool] = None
    pool_moves_enabled: Optional[bool] = None
    managed: Optional[bool] = None
    offer_log_id: Optional[int] = None
    last_mirrored_at: Optional[str] = None


@dataclass
class CopyLogEntry(Model):
    id: Optional[int] = None
    source: Optional[str] = None
    signature: Optional[str] = None
    leader_kind: Optional[str] = None
    pool: Optional[str] = None
    leader_position: Optional[str] = None
    leader_usd: Optional[float] = None
    target_usd: Optional[float] = None
    outcome: Optional[str] = None
    reason_code: Optional[str] = None
    reason: Optional[str] = None
    leader_at: Optional[str] = None
    at: Optional[str] = None


@dataclass
class CopyLog(Model):
    """The wallet follow (or `None`) and what the mirror did or declined."""

    follow: Optional[WalletFollow] = None
    entries: List[CopyLogEntry] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {"follow": WalletFollow, "entries": (CopyLogEntry,)}


@dataclass
class CopyReferralOffer(Model):
    offer: Optional[bool] = None
    name: Optional[str] = None


@dataclass
class CopyReferralAttach(Model):
    attached: Optional[bool] = None


@dataclass
class BatchOutcome(Model):
    """One item of an account batch. `key` is the agent id or the slug."""

    key: Optional[str] = None
    ok: Optional[bool] = None
    result: Optional[Any] = None
    error_code: Optional[str] = None
    error: Optional[str] = None


@dataclass
class BatchResult(Model):
    total: Optional[int] = None
    succeeded: Optional[int] = None
    failed: Optional[int] = None
    items: List[BatchOutcome] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {"items": (BatchOutcome,)}


# --------------------------------------------------------------------------
# 0.7.0: the self-custody loop (prepare, sign in your wallet, mark)
# --------------------------------------------------------------------------


@dataclass
class Prepared(Model):
    """An UNSIGNED build for the key owner's own wallet.

    Every prepare route has its own shape; the fields they share are named
    here and the rest is in `raw_json`. The transaction to sign is
    `unsigned_tx_b64` (or `open_tx_b64` for the reopen leg). Nothing was
    signed or sent: sign it in wallet software you control, send it, then
    report the signature with the matching mark call.

    `position_nft_mint` is set on a Meteora DAMM v2 open (whoever holds the
    NFT owns the position). `fee` is Ponk's fee on the LP fees a claim or
    withdraw collects, per leg, for the review; `None` when it collects none.
    `closes_position` is set on a DAMM v2 withdraw: whether it also closes
    the position and returns its rent.
    """

    unsigned_tx_b64: Optional[str] = None
    open_tx_b64: Optional[str] = None
    recent_blockhash: Optional[str] = None
    last_valid_block_height: Optional[int] = None
    dex: Optional[str] = None
    pool_address: Optional[str] = None
    position_address: Optional[str] = None
    current_position: Optional[str] = None
    new_position_address: Optional[str] = None
    new_lower_bin_id: Optional[int] = None
    new_upper_bin_id: Optional[int] = None
    fee_event_ids: Optional[List[str]] = None
    draft_event_ids: Optional[List[str]] = None
    position_nft_mint: Optional[str] = None
    fee: Optional[Dict[str, Any]] = None
    closes_position: Optional[bool] = None


@dataclass
class MarkedTx(Model):
    """What a mark call recorded. Which fields are set depends on the route:
    `tx_id` and `fee_events_confirmed` (position mark-tx), `updated` (fee
    signature), `status` and `confirmed` (fee confirm), `position_address`
    (rebalance mark)."""

    tx_id: Optional[str] = None
    pnl_delta_usd: Optional[str] = None
    fee_events_confirmed: Optional[int] = None
    agent_id: Optional[str] = None
    position_address: Optional[str] = None
    updated: Optional[int] = None
    status: Optional[str] = None
    confirmed: Optional[int] = None


@dataclass
class StrandedPositions(Model):
    positions: Optional[List[Any]] = None
    total_rent_lamports: Optional[int] = None


# --------------------------------------------------------------------------
# 0.7.0: calls, push, cards, privacy, logs, Telegram, ranking
# --------------------------------------------------------------------------


@dataclass
class CallPreview(Model):
    """What a call on a token would attach.

    `chain` is `solana` or the EVM chain the token is on (then `mint` is its
    CAIP id, also when the request named a pool). `price_source` says where
    an EVM token's price came from; `None` on Solana. `clears_floor` is false
    when the position is priced and under `min_position_usd`: posting would
    be refused. `next_allowed_at` is when the next call is allowed, `None`
    when one is allowed now.
    """

    chain: Optional[str] = None
    price_source: Optional[str] = None
    mint: Optional[str] = None
    symbol: Optional[str] = None
    price_usd: Optional[float] = None
    wallet: Optional[str] = None
    attached: Optional[Any] = None
    has_position: Optional[bool] = None
    position_usd: Optional[float] = None
    min_position_usd: Optional[float] = None
    clears_floor: Optional[bool] = None
    next_allowed_at: Optional[str] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class Call(Model):
    """A call. `attached` is the position the server read from chain.

    In the Calls feed `source` is `ponk` (posted on Ponk, with `caller`) or
    `x` (a tracked X account's first mention, with `account`, `post_id` and
    `url`). `chain` is `solana` or the EVM chain the token is on. On an EVM
    call `mint` is a CAIP token id, `mcap_usd` and `mcap_now_usd` are `None`
    (the EVM boards state no market cap) and `price_source` says where the
    price came from: `chainlink`, `onchain_pool`, `coingecko`, `defillama`
    or `dexscreener` (`None` on Solana). `auto` is true for a call created
    from a confirmed buy through Ponk's swap, which runs on Solana, so a call
    on an EVM token is always posted by hand. `status` is `open` while any
    position remains and `closed` once fully exited.

    The figures arrive camelCase from the API and are read as such from
    0.16.0 on (earlier versions left them `None`).
    """

    id: Optional[str] = None
    source: Optional[str] = None
    caller: Optional[Any] = None
    account: Optional[Any] = None
    post_id: Optional[str] = None
    url: Optional[str] = None
    chain: Optional[str] = None
    price_source: Optional[str] = None
    mint: Optional[str] = None
    symbol: Optional[str] = None
    text: Optional[str] = None
    at: Optional[str] = None
    price_usd: Optional[float] = None
    mcap_usd: Optional[float] = None
    price_now_usd: Optional[float] = None
    mcap_now_usd: Optional[float] = None
    multiple_now: Optional[float] = None
    multiple_1h: Optional[float] = None
    multiple_24h: Optional[float] = None
    peak_multiple: Optional[float] = None
    peak_at: Optional[str] = None
    auto: Optional[bool] = None
    status: Optional[str] = None
    realized_pnl_usd: Optional[float] = None
    closed_at: Optional[str] = None
    attached: Optional[Any] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class CallsFeed(Model):
    """`GET /public/calls`: a page of the Calls feed, newest first.

    Pass `next_before` back as `before` for the next page; it is `None` at
    the end. `chains` is the chain ids the page covers.
    """

    calls: List[Call] = field(default_factory=list)
    next_before: Optional[str] = None
    chains: List[str] = field(default_factory=list)

    _CAMEL: ClassVar[bool] = True
    _NESTED: ClassVar[Mapping[str, Any]] = {"calls": (Call,)}


@dataclass
class CallersBoard(Model):
    """`GET /public/calls/callers`: the callers board over `days`.

    `callers` are passed through as decoded JSON. `chains` is the chain ids
    the board is computed over: a caller's record under one chain is their
    calls on it.
    """

    days: Optional[int] = None
    callers: List[Any] = field(default_factory=list)
    chains: List[str] = field(default_factory=list)
    computed_at: Optional[str] = None

    _CAMEL: ClassVar[bool] = True


@dataclass
class PushSettings(Model):
    enabled: Optional[bool] = None
    prefs: Optional[Any] = None
    devices: Optional[int] = None
    this_device: Optional[bool] = None


@dataclass
class PushTest(Model):
    delivered: Optional[int] = None


@dataclass
class CardLook(Model):
    """A share card's saved look (the PnL card or the referral card)."""

    prefs: Optional[Any] = None
    saved: Optional[bool] = None


@dataclass
class LeaderboardPrivacy(Model):
    privacy: Optional[Any] = None
    saved: Optional[bool] = None
    creator_handle: Optional[str] = None
    wallet: Optional[str] = None
    owner_key: Optional[str] = None
    named_key: Optional[str] = None
    anon_key: Optional[str] = None
    agents: Optional[int] = None
    agents_hidden: Optional[int] = None


@dataclass
class AgentVisibility(Model):
    hidden: Optional[bool] = None


@dataclass
class LogsCleared(Model):
    """Rows hidden from the activity log. The fund ledger is never deleted."""

    deleted: Optional[int] = None
    hidden: Optional[int] = None
    cleared_before: Optional[str] = None


@dataclass
class TelegramStatus(Model):
    linked: Optional[bool] = None
    telegram_username: Optional[str] = None
    linked_at: Optional[str] = None


@dataclass
class TelegramShared(Model):
    sent: Optional[bool] = None


@dataclass
class PresetRank(Model):
    """Every wizard preset (and your saved models) replayed on one pool's
    window by the site's own Plot engine, ranked by net result. A row the
    engine could not answer carries its reason and no number. The row shape
    is the site ranker's; read `rows` and `raw_json`."""

    rows: Optional[List[Any]] = None
    reasons: Optional[Any] = None
    cached: Optional[bool] = None


# --------------------------------------------------------------------------
# People: profiles, follows, the Following feed and the people board
# --------------------------------------------------------------------------


@dataclass
class Person(Model):
    """A Ponk account as other people see it.

    `key` is its profile link (`/u/<key>`): the @handle, a shown wallet, or a
    `t-<owner_key>` tag for a public account with neither. `None` for a
    private account. `usd_hidden` is true when the owner shows sizes and P&L
    as percentages only.
    """

    key: Optional[str] = None
    owner_key: Optional[str] = None
    name: Optional[str] = None
    handle: Optional[str] = None
    display_name: Optional[str] = None
    wallet: Optional[str] = None
    avatar_version: Optional[int] = None
    is_public: Optional[bool] = None
    usd_hidden: Optional[bool] = None
    #: On a followers or following list: when the follow started.
    followed_at: Optional[str] = None


@dataclass
class Profile(Model):
    """A public profile (`/u/<key>`). Every claimed @handle and every /r/ code
    opens one. A private account's profile carries only its handle, bio, X and
    picture: no agents, no wallet, no figures. `social` holds the follower and
    following counts."""

    key: Optional[str] = None
    label: Optional[str] = None
    is_public: Optional[bool] = None
    owner_key: Optional[str] = None
    handle: Optional[str] = None
    display_name: Optional[str] = None
    wallet: Optional[str] = None
    wallet_verified: Optional[bool] = None
    bio: Optional[str] = None
    x_handle: Optional[str] = None
    avatar_version: Optional[int] = None
    joined: Optional[str] = None
    agents: Optional[List[Any]] = None
    agents_unavailable: Optional[str] = None
    min_rank_days: Optional[int] = None
    activity: Optional[List[Any]] = None
    social: Optional[Any] = None


@dataclass
class PeopleList(Model):
    """Followers or following of one person. Private accounts are counted in
    `private_count`, never listed. `hidden` is true when the list belongs to a
    private account, so only `total` is shown. Page with `next_cursor`."""

    people: Optional[List[Person]] = None
    private_count: Optional[int] = None
    total: Optional[int] = None
    next_cursor: Optional[str] = None
    hidden: Optional[bool] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"people": (Person,)}


@dataclass
class SearchHit(Person):
    """One person a search found, with their follower count."""

    followers: Optional[int] = None


@dataclass
class PeopleSearch(Model):
    """A page of a people search. `next_cursor` is `None` on the last page."""

    people: Optional[List[SearchHit]] = None
    next_cursor: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"people": (SearchHit,)}


@dataclass
class RecommendedPerson(Person):
    """Someone to follow. `reason` is the strongest reason as one line;
    `reasons` holds every one, strongest first, each `{"kind", "text"}` with
    kind `followed_by`, `same_pool`, `same_token`, `top`, `rising` or
    `caller`."""

    followers: Optional[int] = None
    reason: Optional[str] = None
    reasons: Optional[List[Any]] = None


@dataclass
class Recommendations(Model):
    """Who to follow, strongest first, and `basis`: the signals ranked."""

    people: Optional[List[RecommendedPerson]] = None
    basis: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"people": (RecommendedPerson,)}


@dataclass
class FollowState(Model):
    """Your follow of one person. `private` is true when they are private:
    nothing they do is shown until they go public."""

    key: Optional[str] = None
    following: Optional[bool] = None
    followers: Optional[int] = None
    notify_telegram: Optional[bool] = None
    notify_push: Optional[bool] = None
    muted: Optional[bool] = None
    private: Optional[bool] = None


@dataclass
class MyFollow(Person):
    notify_telegram: Optional[bool] = None
    notify_push: Optional[bool] = None
    muted: Optional[bool] = None
    last_active_at: Optional[str] = None


@dataclass
class MyFollowing(Model):
    """Everyone you follow (private accounts included) plus the shared agents
    you follow one by one, by slug."""

    people: Optional[List[MyFollow]] = None
    agents: Optional[List[str]] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"people": (MyFollow,)}


@dataclass
class CopyAction(Model):
    """What Copy does for a feed row or position: `agent` (copy the shared
    agent's config), `mirror` (Copy LP the wallet), `open_same` (one open of the
    same pool and range), `trade` (open the token) or `none`."""

    mode: Optional[str] = None
    slug: Optional[str] = None
    wallet: Optional[str] = None
    venue: Optional[str] = None
    pool: Optional[str] = None
    mint: Optional[str] = None
    range_lo: Optional[float] = None
    range_hi: Optional[float] = None
    edge_lo: Optional[int] = None
    edge_hi: Optional[int] = None


@dataclass
class ActivityRow(Model):
    """One row of the Following feed or a person's activity: an agent open,
    rebalance or exit, a manual LP move, a call, or a swap (only when the owner
    shares swaps). `size_usd` and `pnl_usd` are `None` when the owner hides
    dollars (`usd_hidden`); `size_rounded` and `time_rounded` say a figure was
    coarsened to protect a hidden wallet."""

    id: Optional[int] = None
    kind: Optional[str] = None
    #: Who did it, and the verb the site prints ("Opened", "Exited", ...).
    actor: Optional[Person] = None
    verb: Optional[str] = None
    agent_slug: Optional[str] = None
    call_id: Optional[str] = None
    venue: Optional[str] = None
    pool: Optional[str] = None
    token_x_mint: Optional[str] = None
    token_y_mint: Optional[str] = None
    range_lo: Optional[float] = None
    range_hi: Optional[float] = None
    range_width_pct: Optional[float] = None
    size_usd: Optional[float] = None
    size_rounded: Optional[bool] = None
    pnl_usd: Optional[float] = None
    pnl_pct: Optional[float] = None
    usd_hidden: Optional[bool] = None
    unavailable: Optional[str] = None
    position: Optional[str] = None
    signature: Optional[str] = None
    count: Optional[int] = None
    first_at: Optional[str] = None
    at: Optional[str] = None
    time_rounded: Optional[bool] = None
    copy: Optional[CopyAction] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"actor": Person, "copy": CopyAction}


@dataclass
class SocialFeed(Model):
    """A page of activity, newest first. For your Following feed `following`
    and `following_private` count the people you follow and how many of them
    are private (their rows never appear). Page with `next_cursor`."""

    rows: Optional[List[ActivityRow]] = None
    next_cursor: Optional[str] = None
    following: Optional[int] = None
    following_private: Optional[int] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"rows": (ActivityRow,)}


@dataclass
class OpenPosition(Model):
    """One open position on a person's profile, from an agent (`source`
    `agent`) or a manual LP (`lp`), on Meteora, Orca or Raydium."""

    source: Optional[str] = None
    agent_slug: Optional[str] = None
    venue: Optional[str] = None
    pool: Optional[str] = None
    token_x_mint: Optional[str] = None
    token_y_mint: Optional[str] = None
    value_usd: Optional[float] = None
    value_rounded: Optional[bool] = None
    pnl_usd: Optional[float] = None
    pnl_pct: Optional[float] = None
    in_range: Optional[bool] = None
    position: Optional[str] = None
    copy: Optional[CopyAction] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"copy": CopyAction}


@dataclass
class PersonPositions(Model):
    """A person's open positions. `scope` is `all`, `shared_agents` (agents
    shared by link only) or `private` (nothing listed)."""

    positions: Optional[List[OpenPosition]] = None
    scope: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"positions": (OpenPosition,)}


@dataclass
class PersonBoardRow(Model):
    """One person on the people board. `rank` is `None` with an
    `unranked_reason` (for example under $100 deployed in the window)."""

    rank: Optional[int] = None
    person: Optional[Person] = None
    pnl_usd: Optional[float] = None
    roi_pct: Optional[float] = None
    vs_hodl_pct: Optional[float] = None
    fees_earned_usd: Optional[float] = None
    drawdown_pct: Optional[float] = None
    positions: Optional[int] = None
    followers: Optional[int] = None
    usd_hidden: Optional[bool] = None
    unranked_reason: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"person": Person}


@dataclass
class PeopleBoard(Model):
    """The people board: Ponk accounts ranked over `window` (`24h`, `7d`,
    `30d`) by `sort` (`pnl`, `roi`, ...). `basis` says what the figures cover."""

    window: Optional[str] = None
    sort: Optional[str] = None
    rows: Optional[List[PersonBoardRow]] = None
    basis: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"rows": (PersonBoardRow,)}


@dataclass
class PrivacySettings(Model):
    """The privacy switches. `is_public` is the one that matters: a public
    account can be found and followed, and its positions, agents and results
    show. Under it, `show_wallet` prints the wallet, `show_usd` shows dollar
    amounts (off: percentages only) and `share_swaps` puts swaps in followers'
    feeds. A private account may still rank its agents anonymously
    (`rank_anon`)."""

    is_public: Optional[bool] = None
    show_wallet: Optional[bool] = None
    show_usd: Optional[bool] = None
    share_swaps: Optional[bool] = None
    rank_anon: Optional[bool] = None


@dataclass
class Privacy(Model):
    """Your privacy: the switches (`settings`), whether the one-time "Keep
    public or make private" prompt is due (`needs_ack`), and what a row of
    yours carries right now (`profile_key`, `owner_key`)."""

    settings: Optional[PrivacySettings] = None
    saved: Optional[bool] = None
    activity_ack: Optional[bool] = None
    needs_ack: Optional[bool] = None
    creator_handle: Optional[str] = None
    handle: Optional[str] = None
    wallet: Optional[str] = None
    owner_key: Optional[str] = None
    named_key: Optional[str] = None
    anon_key: Optional[str] = None
    profile_key: Optional[str] = None
    avatar_version: Optional[int] = None
    agents: Optional[int] = None
    agents_hidden: Optional[int] = None
    visible: Optional[bool] = None
    show_name: Optional[bool] = None
    show_wallet: Optional[bool] = None
    display_name: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"settings": PrivacySettings}


# --------------------------------------------------------------------------
# Wallet Tracker P&L and tracked LPs, X account cards, Top calls
# --------------------------------------------------------------------------


@dataclass
class WalletPnl(Model):
    """Trading P&L per token for any wallet, from its swaps. `tokens` rows
    carry buys, sells, cost, proceeds, realized and unrealized P&L;
    `basisComplete` is false when the history did not reach the first buy, so
    the cost basis is partial. Unpriced values are `None`, never zero."""

    _CAMEL: ClassVar[bool] = True

    address: Optional[str] = None
    tokens: Optional[List[Any]] = None
    summary: Optional[Any] = None
    sol_usd: Optional[float] = None
    balances_read: Optional[bool] = None
    transactions_read: Optional[int] = None
    read_at: Optional[str] = None


@dataclass
class TrackedLp(Model):
    """One open LP position of a tracked wallet: `wallet` (address, label,
    source) and `position` (venue, pool, range, value, fees and P&L)."""

    wallet: Optional[Any] = None
    position: Optional[Any] = None


@dataclass
class TrackedLps(Model):
    """Open LP positions of the curated wallets on Meteora, Orca and Raydium.
    Only wallets with an LP move in the last 30 days are read."""

    _CAMEL: ClassVar[bool] = True

    scope: Optional[str] = None
    rows: Optional[List[TrackedLp]] = None
    total: Optional[int] = None
    wallets_read: Optional[int] = None
    wallets_failed: Optional[int] = None
    wallets_lp_active: Optional[int] = None
    read_at: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"rows": (TrackedLp,)}


@dataclass
class XAccountCard(Model):
    """One X account's caller card over `days`: calls, priced and scored
    calls, median 1h and 24h multiples, best, hit rate, median peak, 2x rate,
    first calls, posting rate, median detection latency, rank and recent calls."""

    _CAMEL: ClassVar[bool] = True

    days: Optional[int] = None
    account: Optional[Any] = None
    calls: Optional[int] = None
    priced_calls: Optional[int] = None
    median1h: Optional[float] = None
    median24h: Optional[float] = None
    best: Optional[float] = None
    hit_rate: Optional[float] = None
    scored24h: Optional[int] = None
    median_peak: Optional[float] = None
    x2_rate: Optional[float] = None
    peaked: Optional[int] = None
    ranked: Optional[bool] = None
    first_calls: Optional[int] = None
    posts24h: Optional[int] = None
    median_latency_secs: Optional[int] = None
    rank: Optional[int] = None
    recent: Optional[List[Any]] = None
    computed_at: Optional[str] = None

    _ALIASES: ClassVar[Mapping[str, str]] = {"x2_rate": "x2Rate"}


@dataclass
class TopCalls(Model):
    """The best calls of the last `days`, Ponk and X, ranked by their
    multiple since the call, computed from real prices. `chains` is the
    chain ids the answer covers."""

    _CAMEL: ClassVar[bool] = True

    days: Optional[int] = None
    calls: Optional[List[Any]] = None
    chains: List[str] = field(default_factory=list)
    computed_at: Optional[str] = None


@dataclass
class StreamEvent(Model):
    """One Server-Sent Event from a streaming endpoint.

    ``event`` is the SSE event name and ``data`` the JSON payload the server
    sent with it. On the social stream: ``ready`` ({}) arrives once when the
    stream opens, ``row`` carries one social event as it happens, and
    ``resync`` ({}) asks the client to reload from ``live_feed`` because it may
    have missed rows. Built by the client as events arrive, not parsed from a
    single response body.
    """

    event: Optional[str] = None
    data: Optional[Mapping[str, Any]] = None


# --------------------------------------------------------------------------
# EVM chains: Ethereum, Arbitrum, Base, BNB Chain, Robinhood Chain (0.15.0)
# --------------------------------------------------------------------------


@dataclass
class EvmChain(Model):
    """One chain ponk has fixed an id for.

    `id` is the chain word every route takes (`solana`, `ethereum`,
    `arbitrum`, `base`, `bsc`, `robinhood`), `evm_chain_id` the EIP-155 id
    (`None` on Solana), `native_symbol` the gas coin, `venues` the venue ids
    that run there, and `reads_served` whether the pool board is answering
    for it right now.
    """

    _CAMEL: ClassVar[bool] = True

    id: Optional[str] = None
    label: Optional[str] = None
    status: Optional[str] = None
    evm_chain_id: Optional[int] = None
    native_symbol: Optional[str] = None
    explorer: Optional[str] = None
    l2_kind: Optional[str] = None
    inclusion: Optional[str] = None
    venues: List[str] = field(default_factory=list)
    reads_served: Optional[bool] = None


@dataclass
class EvmChains(Model):
    """`evm_chains()`: the chain dimension. `read_enabled` false means the EVM
    pool boards are switched off on this server."""

    _CAMEL: ClassVar[bool] = True

    read_enabled: Optional[bool] = None
    market_sources: List[str] = field(default_factory=list)
    chains: List[EvmChain] = field(default_factory=list)
    capabilities: Optional[Dict[str, Any]] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"chains": (EvmChain,)}

    def chain(self, chain_id: str) -> Optional[EvmChain]:
        """The row for one chain word, or `None`."""
        for c in self.chains:
            if c.id == chain_id:
                return c
        return None


@dataclass
class EvmToken(Model):
    """A token on an EVM chain, as the pool board knows it.

    `mint` is the token id (`eip155:<chainId>:0x...`; the gas coin is the
    zero address), `address` the bare address. `price_usd` is a number in USD
    with `price_source` naming where it came from, `None` when no source ponk
    stands behind has one. `verified` is whether the token is on its venue's
    token list; `stock` marks a Robinhood Chain stock token.
    """

    _CAMEL: ClassVar[bool] = True

    mint: Optional[str] = None
    chain: Optional[str] = None
    address: Optional[str] = None
    symbol: Optional[str] = None
    name: Optional[str] = None
    decimals: Optional[int] = None
    logo_uri: Optional[str] = None
    price_usd: Optional[float] = None
    price_source: Optional[str] = None
    price_via: Optional[str] = None
    change_1h: Optional[float] = None
    change_24h: Optional[float] = None
    verified: Optional[bool] = None
    verification: Optional[Dict[str, Any]] = None
    stock: Optional[Any] = None
    created_at: Optional[int] = None


@dataclass
class EvmWindows(Model):
    """A USD figure over the board's four windows. A window with no reading is `None`."""

    m30: Optional[float] = None
    h1: Optional[float] = None
    h4: Optional[float] = None
    h24: Optional[float] = None

    _ALIASES: ClassVar[Mapping[str, str]] = {"m30": "30m", "h1": "1h", "h4": "4h", "h24": "24h"}


@dataclass
class EvmFarm(Model):
    """Farm rewards for staked liquidity on a pool.

    `token` is `CAKE` or `AERO`. `live` means staking there earns the token
    now. `apr_pct` is emissions per year in dollars over the dollars staked,
    a number in PERCENT, computed from chain reads only. ON A CONCENTRATED
    POOL (PancakeSwap v3, Aerodrome Slipstream) it is the rate of a staked
    position spread like the whole pool: a tighter range earns more per
    dollar while in range and nothing out of it, so it reads lower than the
    venue's own headline. On Aerodrome Classic it is the rate of any staked
    LP. `None` when the farm is not live or an input could not be read. An
    agent's own rate is `AgentPerformance.farm_rewards`.
    """

    _CAMEL: ClassVar[bool] = True

    token: Optional[str] = None
    live: Optional[bool] = None
    apr_pct: Optional[float] = None


@dataclass
class EvmHook(Model):
    """The hook of a Uniswap v4 or PancakeSwap Infinity pool.

    The zero address is a pool with no hook. `agent_eligible` false means an
    agent on this pool is refused at create; `touches_lp_funds` says the hook
    can change what an LP holds.
    """

    _CAMEL: ClassVar[bool] = True

    address: Optional[str] = None
    agent_eligible: Optional[bool] = None
    points: List[str] = field(default_factory=list)
    policy: Optional[Dict[str, Any]] = None
    touches_lp_funds: Optional[bool] = None


@dataclass
class EvmPool(Model):
    """One pool on an EVM chain's board.

    `address` is the CAIP pool id to pass as `pool_address` (`eip155:56:0x...`,
    or the four part form for Uniswap v4 and PancakeSwap Infinity CL). `x`
    and `y` are token0 and token1. `fee_pct` is the LP fee tier in PERCENT
    (0.05 is 0.05%), `None` on a `dynamic_fee` pool, where a hook sets the fee
    on each swap. `bin_step` is the tick spacing, `None` on a full-range (v2
    style) venue. `tvl` is USD with `tvl_source`; `volume` and `fees` (LP fees
    earned, net of the protocol's share) are USD per window. `price` is token1
    per token0 in whole units. `farm` is `None` when the pool has no farm.
    `created_at` is milliseconds. `data_insufficient` names every figure that
    is `None` and why. On Base, `aerodrome` carries the pool's gauge facts
    (`gauge`, `gaugeLive`, `stakedShare`, `curve`).
    """

    _CAMEL: ClassVar[bool] = True

    key: Optional[str] = None
    dex: Optional[str] = None
    chain: Optional[str] = None
    address: Optional[str] = None
    x: Optional[EvmToken] = None
    y: Optional[EvmToken] = None
    bin_step: Optional[int] = None
    fee_pct: Optional[float] = None
    dynamic_fee: Optional[bool] = None
    tvl: Optional[float] = None
    tvl_source: Optional[str] = None
    created_at: Optional[int] = None
    launchpad: Optional[str] = None
    volume: Optional[EvmWindows] = None
    fees: Optional[EvmWindows] = None
    volume_source: Optional[str] = None
    price: Optional[float] = None
    tick: Optional[int] = None
    liquidity: Optional[str] = None
    sqrt_price_x96: Optional[str] = None
    hooks: Optional[EvmHook] = None
    farm: Optional[EvmFarm] = None
    data_insufficient: List[str] = field(default_factory=list)
    aerodrome: Optional[Dict[str, Any]] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "x": EvmToken,
        "y": EvmToken,
        "volume": EvmWindows,
        "fees": EvmWindows,
        "hooks": EvmHook,
        "farm": EvmFarm,
    }

    @property
    def pair(self) -> str:
        """`USDT/WBNB`, a `?` standing in for a symbol the board does not have."""
        sym = lambda t: (t.symbol if t is not None and t.symbol else "?")  # noqa: E731
        return "{0}/{1}".format(sym(self.x), sym(self.y))

    def flags(self, now_ms: Optional[int] = None) -> List[str]:
        """The risk flags the ponk.exchange board shows for this row.

        `unverified_token`: a token is not on the venue's token list.
        `hook_not_agent_eligible`: the pool has a hook an agent cannot use.
        `under_1h_old`: the pool was created under an hour ago. Empty when
        none applies. These are read off fields the server sent; the copilot
        never picks a flagged pool. `now_ms` is for tests.
        """
        out: List[str] = []
        if any(t is not None and t.verified is False for t in (self.x, self.y)):
            out.append("unverified_token")
        hook = self.hooks
        if (
            hook is not None
            and hook.address
            and hook.address.lower().replace("0x", "", 1).strip("0") != ""
            and hook.agent_eligible is not True
        ):
            out.append("hook_not_agent_eligible")
        if self.created_at:
            import time

            now = int(time.time() * 1000) if now_ms is None else now_ms
            created = self.created_at * 1000 if self.created_at < 10**12 else self.created_at
            if now - created < 3_600_000:
                out.append("under_1h_old")
        return out


@dataclass
class EvmBoardStatus(Model):
    """How fresh a board is. `stale` true means the last live read failed and
    the pools are an older snapshot; `last_error` says why."""

    _CAMEL: ClassVar[bool] = True

    stale: Optional[bool] = None
    consecutive_failures: Optional[int] = None
    last_error: Optional[str] = None
    market_source: Optional[str] = None
    market_sources: List[str] = field(default_factory=list)
    block_timestamp: Optional[int] = None


@dataclass
class EvmBoard(Model):
    """One chain's pool board.

    `as_of` is when the pools' chain state was read, in milliseconds; 0 means
    the server has not finished its first read of the chain, and `pools` is
    then empty. On Base, `aerodrome` is the gauge facts array, joined to the
    pools by `key`.
    """

    _CAMEL: ClassVar[bool] = True

    chain: Optional[str] = None
    as_of: Optional[int] = None
    served_at: Optional[int] = None
    pools: List[EvmPool] = field(default_factory=list)
    ok: Optional[Dict[str, Any]] = None
    status: Optional[EvmBoardStatus] = None
    aerodrome: List[Dict[str, Any]] = field(default_factory=list)

    _NESTED: ClassVar[Mapping[str, Any]] = {"pools": (EvmPool,), "status": EvmBoardStatus}

    @property
    def ready(self) -> bool:
        """False until the server has finished its first read of the chain."""
        return bool(self.as_of)


@dataclass
class EvmOpenCost(Model):
    """What the first open of an agent costs on an EVM venue.

    `gas_wei` is the measured gas of the first open at the live fee, WEI as a
    decimal string, and `gas_usd` the same in USD. `reserve_wei` is always
    kept back for the exit, and `min_open_wei` is their sum: the least the
    agent wallet must hold before it can open, before the amount to deploy.
    `native_usd` is the gas coin's price. Each is `None` when the chain could
    not be read, never an estimate.

    The fee rates are in basis points and are the ones an agent THIS CALLER
    creates pays: with an API key 150 (1.5% of the first open, once) and 1500
    (15% of fees earned); the keyless public route states the rates of an
    agent created on ponk.exchange, 100 and 800. `exit_fee_bps` is the exit
    fee on what an agent that has opened a position sends its owner: 100
    (1% of the amount withdrawn, taken in kind), 50 for a wallet holding
    1,000,000 PONK when it withdraws, 0 while the fee is switched off.
    `exit_yield_fee_bps` is the rate an exit takes on unclaimed yield (the
    performance rate when that switch is on, else 0) and `exit_swap_fee_bps`
    the swap fee on an exit's swaps.

    `shape` is set when the quote asked for one (`evm_open_cost(shape=...)`):
    `kind`, `label`, `several_ranges`, `ranges` (the count the agent would
    open now), `ranges_asked`, `ceiling`, `entry_cost` (per count: `ranges`,
    `usd`, `pct`), `entry_cost_limit_pct`, `range_pct`, `side`,
    `width_ticks`, `amount_usd` and `note`, as decoded JSON.
    """

    chain: Optional[str] = None
    dex: Optional[str] = None
    pool: Optional[str] = None
    native_symbol: Optional[str] = None
    gas_wei: Optional[str] = None
    gas_usd: Optional[str] = None
    reserve_wei: Optional[str] = None
    min_open_wei: Optional[str] = None
    native_usd: Optional[str] = None
    activation_fee_bps: Optional[int] = None
    performance_fee_bps: Optional[int] = None
    exit_fee_bps: Optional[int] = None
    exit_yield_fee_bps: Optional[int] = None
    exit_swap_fee_bps: Optional[int] = None
    shape: Optional[Dict[str, Any]] = None


@dataclass
class EvmLegShape(Model):
    """The liquidity shape of one plan leg, set only when the sentence named a
    shape or a one-sided range.

    `kind` is `spot`, `curve` or `bid_ask` and `label` its name.
    `several_ranges` is true where the venue holds the shape as several
    ranges managed as one position; `ranges` is the count at the gas read
    just now (the agent chooses again from live gas when it opens) and
    `ranges_asked` what the shape asks for on this chain. `one_sided` is
    `below` or `above` for a band on one side of the price. `entry_cost_usd`
    (a decimal string) and `entry_cost_pct` are the gas of opening and later
    closing those ranges, `None` when gas could not be read, against
    `entry_cost_limit_pct`. `sentence` says it in one line.
    """

    kind: Optional[str] = None
    label: Optional[str] = None
    ranges: Optional[int] = None
    ranges_asked: Optional[int] = None
    several_ranges: Optional[bool] = None
    one_sided: Optional[str] = None
    entry_cost_usd: Optional[str] = None
    entry_cost_pct: Optional[float] = None
    entry_cost_limit_pct: Optional[float] = None
    sentence: Optional[str] = None


@dataclass
class EvmPlanLeg(Model):
    """One pool of an EVM plan: one agent once the plan is started.

    `fund_usd` is what the owner sends to this agent in USD, `fund_native`
    the same in whole coins of the chain and `fund_native_wei` in wei, all
    decimal strings. `deploy_usd` is what is left to deploy after the wallet
    keeps its exit reserve and gas (`wallet_floor_usd`, part of `fund_usd`).
    `range_pct` is the range either side of the price in PERCENT (5.0 is 5%).
    `ranked_apr_pct` is what the pool was ranked on, percent a year:
    `fee_apr_pct` plus `farm_apr_pct` where the agent stakes. It is income per
    dollar of pool liquidity, not a forecast. `fee_yield_24h_pct` is percent
    per DAY. `config` is the strategy config the agent is created with.
    `shape` is set only when the sentence named a shape or a one-sided range.
    """

    chain: Optional[str] = None
    dex: Optional[str] = None
    pool_address: Optional[str] = None
    pair: Optional[str] = None
    native_symbol: Optional[str] = None
    fund_usd: Optional[str] = None
    fund_native: Optional[str] = None
    fund_native_wei: Optional[str] = None
    deploy_usd: Optional[str] = None
    strategy: Optional[str] = None
    config: Optional[Any] = None
    range_pct: Optional[float] = None
    fee_yield_24h_pct: Optional[float] = None
    ranked_apr_pct: Optional[float] = None
    fee_apr_pct: Optional[float] = None
    farm_apr_pct: Optional[float] = None
    tvl_usd: Optional[float] = None
    name: Optional[str] = None
    open_gas_usd: Optional[str] = None
    open_gas_native: Optional[str] = None
    wallet_floor_usd: Optional[str] = None
    shape: Optional[EvmLegShape] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"shape": EvmLegShape}


@dataclass
class EvmPlanDropped(Model):
    """A pool the planner looked at and left out, with the sentence that says why."""

    chain: Optional[str] = None
    dex: Optional[str] = None
    pool_address: Optional[str] = None
    pair: Optional[str] = None
    reason: Optional[str] = None


@dataclass
class EvmPlanAlternative(Model):
    """What works instead when a chain's share is under its minimum.

    `kind` is `same_pair_on_chain` (the same pair on a cheaper chain;
    `plan_id` is a plan already made for it, started like any plan) or
    `minimum_on_chain` (`plan_sentence` plans it when sent to
    `copilot_evm_plan`). `amount_usd` is what it needs sent. Never applied by
    itself.
    """

    kind: Optional[str] = None
    chain: Optional[str] = None
    pair: Optional[str] = None
    dex: Optional[str] = None
    pool_address: Optional[str] = None
    amount_usd: Optional[str] = None
    sentence: Optional[str] = None
    plan_id: Optional[str] = None
    plan_sentence: Optional[str] = None


@dataclass
class EvmPlanChain(Model):
    """One chain of an EVM plan: one deposit, funded by the owner on that chain.

    `amount_usd` is the share of the budget on this chain and `amount_native`
    the same in whole coins of the chain (`native_symbol`). `refusal` is set,
    with no `legs`, when nothing on this chain could be planned; the other
    chains still plan. `minimum_usd` (whole dollars) is set with it when size
    is the only reason, and `alternative` is the cheapest thing that works
    instead.
    """

    chain: Optional[str] = None
    native_symbol: Optional[str] = None
    amount_usd: Optional[str] = None
    amount_native: Optional[str] = None
    legs: List[EvmPlanLeg] = field(default_factory=list)
    dropped: List[EvmPlanDropped] = field(default_factory=list)
    notes: List[str] = field(default_factory=list)
    refusal: Optional[str] = None
    minimum_usd: Optional[str] = None
    alternative: Optional[EvmPlanAlternative] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "legs": (EvmPlanLeg,),
        "dropped": (EvmPlanDropped,),
        "alternative": EvmPlanAlternative,
    }


@dataclass
class EvmPlanAgent(Model):
    """One agent a started plan created, in leg order.

    `created` false carries `refusal`, the sentence that says why that leg
    could not be created, and no `agent_id`. `fund_native` is the amount of
    the chain's coin (`native_symbol`) to send to the agent's address, which
    is on `agent_wallet(agent_id).address`.
    """

    agent_id: Optional[str] = None
    chain: Optional[str] = None
    dex: Optional[str] = None
    pool_address: Optional[str] = None
    pair: Optional[str] = None
    fund_usd: Optional[str] = None
    fund_native: Optional[str] = None
    fund_native_wei: Optional[str] = None
    native_symbol: Optional[str] = None
    created: Optional[bool] = None
    refusal: Optional[str] = None


@dataclass
class EvmPlan(Model):
    """A plan made from one sentence that names an EVM chain.

    `state` is `ready`, `solana_only` (no EVM leg could be planned but the
    Solana part was; start answers 422 and `notes[0]` says what to do),
    `refused` or `started`. `echo` is what was understood. `total_usd` is the
    whole budget in dollars and `funding_by_chain` its split, one entry per
    EVM chain. `refusal` is `{code, message}` when the plan as a whole is
    refused. `solana` is the Solana part of a plan across chains: an ordinary
    Solana plan with its own id under `solana["plan"]`, read and started
    through `copilot_get_plan` and `copilot_start`. `fees` is the fee
    sentence for this caller, from the server's fee model.

    `agents`, `sign_urls` and `fleet_id` are filled by `copilot_evm_start`.
    `sign_urls` is never empty after a start: the ponk.exchange pages where
    the owner signs each agent's mandate and funds it.
    """

    id: Optional[str] = None
    state: Optional[str] = None
    text: Optional[str] = None
    echo: Optional[str] = None
    refusal: Optional[Dict[str, Any]] = None
    total_usd: Optional[str] = None
    funding_by_chain: List[EvmPlanChain] = field(default_factory=list)
    dropped: List[EvmPlanDropped] = field(default_factory=list)
    solana: Optional[Dict[str, Any]] = None
    fees: Optional[str] = None
    notes: List[str] = field(default_factory=list)
    agents: List[EvmPlanAgent] = field(default_factory=list)
    sign_urls: List[str] = field(default_factory=list)
    fleet_id: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {
        "funding_by_chain": (EvmPlanChain,),
        "dropped": (EvmPlanDropped,),
        "agents": (EvmPlanAgent,),
    }

    @property
    def chains(self) -> List[str]:
        """The EVM chains this plan covers, in order."""
        return [c.chain for c in self.funding_by_chain if c.chain]

    @property
    def legs(self) -> List[EvmPlanLeg]:
        """Every leg across the chains, in order."""
        return [leg for c in self.funding_by_chain for leg in c.legs]


@dataclass
class EvmFleetAgent(Model):
    """One EVM agent the caller owns.

    `address` is the agent's own address on `chain`, `None` until its wallet
    exists. `mandate_status` is `active`, `expired` or `None` with none.
    `needs` is what the agent still needs from its owner: `authorize_and_fund`,
    `authorize` (the mandate lapsed: sign again), `funding_or_waiting_to_open`
    or `nothing`.
    """

    agent_id: Optional[str] = None
    name: Optional[str] = None
    chain: Optional[str] = None
    dex: Optional[str] = None
    pool_address: Optional[str] = None
    position_address: Optional[str] = None
    status: Optional[str] = None
    strategy: Optional[str] = None
    address: Optional[str] = None
    mandate_status: Optional[str] = None
    mandate_expires_at: Optional[str] = None
    needs: Optional[str] = None


@dataclass
class EvmFleet(Model):
    """`evm_fleet()`: every EVM agent the caller owns, with the links that
    fund and authorize the ones still running, 40 agents to a link."""

    agents: List[EvmFleetAgent] = field(default_factory=list)
    sign_urls: List[str] = field(default_factory=list)
    fund_page: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"agents": (EvmFleetAgent,)}


# --------------------------------------------------------------------------
# All chains on the boards and trackers (0.16.0)
# --------------------------------------------------------------------------


@dataclass
class RecapBestAgent(Model):
    """The week's best agent of the account. `chain` is `solana` or the EVM
    chain it runs on; on an EVM agent `pool_address` is a CAIP id."""

    id: Optional[str] = None
    name: Optional[str] = None
    chain: Optional[str] = None
    dex: Optional[str] = None
    pool_address: Optional[str] = None
    strategy: Optional[str] = None
    pnl_usd: Optional[float] = None


@dataclass
class WeeklyRecap(Model):
    """`GET /v1/recap/weekly`: the weekly recap card's figures.

    `complete` is true once the week has ended and its figures are final.
    `agents` is the agents with marks in the week; `pnl_usd` their summed
    week P&L (`None` with no agents) and `fees_usd` their summed fees
    (`None` when any agent's fees are unknown, because a partial sum would
    read as the whole). `rank` is your place among every ranked owner across
    every chain (`None` when not ranked, and `rank_reason` says why) out of
    `ranked`.
    """

    week_start: Optional[str] = None
    week_end: Optional[str] = None
    complete: Optional[bool] = None
    agents: Optional[int] = None
    pnl_usd: Optional[float] = None
    fees_usd: Optional[float] = None
    best_agent: Optional[RecapBestAgent] = None
    rank: Optional[int] = None
    ranked: Optional[int] = None
    rank_reason: Optional[str] = None
    computed_at: Optional[str] = None

    _NESTED: ClassVar[Mapping[str, Any]] = {"best_agent": RecapBestAgent}


@dataclass
class FeedPrefs(Model):
    """`GET` and `PATCH /v1/feed-prefs`: the account's saved chain filter.

    `chains` is the chain ids every signed-in board and tracker read that
    names no `chains` covers; `all` is true when that is every chain.
    """

    chains: List[str] = field(default_factory=list)
    all: Optional[bool] = None
