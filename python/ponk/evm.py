"""Chains, venues, ids and links for ponk agents on EVM chains.

Ponk agents run on Solana and on five EVM chains. This module holds the
constants and the pure helpers the client needs for the EVM side; nothing in
it sends a request.

    from ponk import evm

    evm.pool_id("bsc", "0x92b7807bF19b7DDdf89b706143896d05228f3121")
    # 'eip155:56:0x92b7807bf19b7dddf89b706143896d05228f3121'

Ids are CAIP strings, lowercase hex, exactly as the API writes them:

* pool or token: ``eip155:<chainId>:0x<40 hex>`` (the gas coin is the zero
  address);
* Uniswap v4 or PancakeSwap Infinity CL pool:
  ``eip155:<chainId>:0x<pool manager>:0x<64 hex pool id>``;
* position NFT: ``eip155:<chainId>:0x<position manager>:<decimal token id>``;
* v2 position: ``eip155:<chainId>:0x<pair>:0x<owner>``.

Units, everywhere on the EVM side: wei amounts are decimal strings (they do
not fit a float), USD amounts are decimal strings, and a figure the server
could not read is ``None``, never 0.
"""

from __future__ import annotations

import math
import re
from dataclasses import dataclass
from decimal import Decimal
from typing import Any, Dict, Iterable, List, Mapping, Optional, Tuple

__all__ = [
    "CHAINS",
    "EVM_CHAINS",
    "EVM_CHAIN_IDS",
    "NATIVE_SYMBOL",
    "EVM_VENUES",
    "FULL_RANGE_VENUES",
    "STAKE_VENUES",
    "RANGE_STRATEGIES",
    "SHAPES",
    "PLAN_SHAPES",
    "ONE_SIDED",
    "SHAPE_VENUES",
    "SITE",
    "IDS_PER_FUND_LINK",
    "EvmId",
    "is_evm_chain",
    "chain_id",
    "chain_of",
    "venue_chains",
    "is_full_range_venue",
    "stake_reward_token",
    "is_shape_venue",
    "with_shape",
    "pool_id",
    "v4_pool_id",
    "token_id",
    "native_token_id",
    "position_nft_id",
    "parse_evm_id",
    "wei_to_native",
    "range_config",
    "full_range_config",
    "stake_config",
    "fund_links",
    "fund_link",
    "agent_page_url",
]

#: Every chain word the API uses, Solana first.
CHAINS: Tuple[str, ...] = ("solana", "ethereum", "arbitrum", "base", "bsc", "robinhood")

#: The five EVM chains and their EIP-155 chain ids.
EVM_CHAIN_IDS: Dict[str, int] = {
    "ethereum": 1,
    "arbitrum": 42161,
    "base": 8453,
    "bsc": 56,
    "robinhood": 4663,
}

EVM_CHAINS: Tuple[str, ...] = tuple(EVM_CHAIN_IDS)

#: The gas coin an agent on each chain is funded in. ETH everywhere but BNB Chain.
NATIVE_SYMBOL: Dict[str, str] = {
    "solana": "SOL",
    "ethereum": "ETH",
    "arbitrum": "ETH",
    "base": "ETH",
    "bsc": "BNB",
    "robinhood": "ETH",
}

#: Each EVM venue id (the `dex` of an agent, a pool or a position) and the
#: chains it runs on.
EVM_VENUES: Dict[str, Tuple[str, ...]] = {
    "uniswap_v2": ("arbitrum", "robinhood", "ethereum"),
    "uniswap_v3": ("arbitrum", "robinhood", "ethereum"),
    "uniswap_v4": ("arbitrum", "robinhood", "ethereum"),
    "pancakeswap_v2": ("bsc",),
    "pancakeswap_v3": ("bsc",),
    "pancakeswap_infinity_cl": ("bsc",),
    "aerodrome_classic": ("base",),
    "aerodrome_slipstream": ("base",),
}

#: Venues where a position is a share of the whole pool: no range, and the
#: agent compounds and holds (`fee_collector`, or `monitor_only` to watch).
FULL_RANGE_VENUES: Tuple[str, ...] = ("uniswap_v2", "pancakeswap_v2", "aerodrome_classic")

#: Venues where a position can be staked for farm rewards by choice
#: (`config.evm.stake`), and the token the farm pays.
STAKE_VENUES: Dict[str, str] = {
    "pancakeswap_v3": "CAKE",
    "aerodrome_slipstream": "AERO",
    "aerodrome_classic": "AERO",
}

#: The strategy ids a range venue takes. Degen Mode (`entry_agent`) is not
#: offered on an EVM venue.
RANGE_STRATEGIES: Tuple[str, ...] = (
    "adaptive_range_recovery",
    "bin_rebalancer",
    "conservative_bin_rebalancer",
    "volatility_aware_bin_rebalancer",
    "custom",
)

#: The liquidity shapes an EVM agent takes (`knobs.liquidity_shape`, or the
#: top-level `liquidity_shape` on `adaptive_range_recovery`): `spot` is one
#: even range (the default), `curve` is heaviest at the price, `bid_ask`
#: heaviest at the edges and `spot_bid_ask` half Spot and half Bid-Ask.
SHAPES: Tuple[str, ...] = ("spot", "curve", "bid_ask", "spot_bid_ask")

#: The shapes the EVM copilot's sentence grammar names.
PLAN_SHAPES: Tuple[str, ...] = ("spot", "curve", "bid_ask")

#: The two sides a one-sided range sits on: `below` the price (the band that
#: buys the base token as the price falls) or `above` it.
ONE_SIDED: Tuple[str, ...] = ("below", "above")

#: Where a shape other than Spot opens as several ranges managed as one
#: position, venue to chains. On every other range venue a shape is kept on
#: the agent and the position opens as one range.
SHAPE_VENUES: Dict[str, Tuple[str, ...]] = {
    "uniswap_v3": ("arbitrum", "ethereum", "robinhood"),
    "uniswap_v4": ("arbitrum", "ethereum"),
    "pancakeswap_v3": ("bsc",),
    "pancakeswap_infinity_cl": ("bsc",),
    "aerodrome_slipstream": ("base",),
}

SITE = "https://ponk.exchange"

#: Agents per fund-and-sign link, the API's own grouping.
IDS_PER_FUND_LINK = 40

_ZERO = "0x" + "0" * 40
_ADDR = re.compile(r"^0x[0-9a-fA-F]{40}$")
_HASH = re.compile(r"^0x[0-9a-fA-F]{64}$")
_UUID = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
_BY_ID = {v: k for k, v in EVM_CHAIN_IDS.items()}


def is_evm_chain(chain: Any) -> bool:
    """True for one of the five EVM chain words."""
    return isinstance(chain, str) and chain in EVM_CHAIN_IDS


def chain_id(chain: str) -> int:
    """The EIP-155 chain id of an EVM chain word. Raises `ValueError` otherwise."""
    try:
        return EVM_CHAIN_IDS[chain]
    except (KeyError, TypeError):
        raise ValueError(
            "{0!r} is not an EVM chain; the chains are {1}".format(chain, ", ".join(EVM_CHAINS))
        ) from None


def venue_chains(dex: str) -> Tuple[str, ...]:
    """The chains an EVM venue runs on. Empty for a venue this client does not know."""
    return EVM_VENUES.get(dex, ())


def is_full_range_venue(dex: str) -> bool:
    """True where a position has no range (v2 style pools)."""
    return dex in FULL_RANGE_VENUES


def stake_reward_token(dex: str) -> Optional[str]:
    """`CAKE` or `AERO` where an agent can stake its position, else `None`."""
    return STAKE_VENUES.get(dex)


def is_shape_venue(dex: str, chain: str) -> bool:
    """True when a shape other than Spot opens as several ranges on this venue
    and chain (`SHAPE_VENUES`)."""
    return chain in SHAPE_VENUES.get(dex, ())


def _addr(value: str, what: str) -> str:
    if not isinstance(value, str) or not _ADDR.match(value):
        raise ValueError("{0} must be 0x followed by 40 hex characters, got {1!r}".format(what, value))
    return value.lower()


def pool_id(chain: str, address: str) -> str:
    """`eip155:<chainId>:0x<address>`: a v2 or v3 style pool, lowercase."""
    return "eip155:{0}:{1}".format(chain_id(chain), _addr(address, "the pool address"))


def token_id(chain: str, address: str) -> str:
    """`eip155:<chainId>:0x<address>`: a token. The gas coin is the zero address."""
    return "eip155:{0}:{1}".format(chain_id(chain), _addr(address, "the token address"))


def native_token_id(chain: str) -> str:
    """The id of a chain's gas coin (ETH, or BNB on BNB Chain)."""
    return "eip155:{0}:{1}".format(chain_id(chain), _ZERO)


def v4_pool_id(chain: str, pool_manager: str, pool_key_hash: str) -> str:
    """`eip155:<chainId>:0x<pool manager>:0x<64 hex>`.

    The form of a Uniswap v4 or PancakeSwap Infinity CL pool, which lives
    inside its venue's pool manager and is named by a 32 byte pool id.
    """
    if not isinstance(pool_key_hash, str) or not _HASH.match(pool_key_hash):
        raise ValueError(
            "the pool id must be 0x followed by 64 hex characters, got {0!r}".format(pool_key_hash)
        )
    return "eip155:{0}:{1}:{2}".format(
        chain_id(chain), _addr(pool_manager, "the pool manager"), pool_key_hash.lower()
    )


def position_nft_id(chain: str, position_manager: str, nft_token_id: int) -> str:
    """`eip155:<chainId>:0x<position manager>:<decimal token id>`."""
    n = int(nft_token_id)
    if n < 0:
        raise ValueError("the token id must not be negative")
    return "eip155:{0}:{1}:{2}".format(chain_id(chain), _addr(position_manager, "the position manager"), n)


@dataclass(frozen=True)
class EvmId:
    """One parsed CAIP id.

    `kind` is `address` (a pool or a token), `v4_pool`, `position_nft` or
    `v2_position`. `address` is the first address of the id: the pool or
    token, the pool manager, the position manager or the pair. Exactly one of
    `pool_key_hash`, `nft_token_id` and `owner` is set on the three longer
    forms.
    """

    chain: str
    chain_id: int
    kind: str
    address: str
    pool_key_hash: Optional[str] = None
    nft_token_id: Optional[int] = None
    owner: Optional[str] = None

    @property
    def is_native(self) -> bool:
        """True for the gas coin's token id (the zero address)."""
        return self.kind == "address" and self.address == _ZERO


def parse_evm_id(value: str) -> EvmId:
    """Read a CAIP id the API sent. Raises `ValueError` for anything else.

    A chain id ponk does not run on is refused, so a Solana address or an id
    for another network can never be mistaken for one of these.
    """
    if not isinstance(value, str):
        raise ValueError("an EVM id is a string, got {0}".format(type(value).__name__))
    parts = value.split(":")
    if len(parts) not in (3, 4) or parts[0] != "eip155" or not parts[1].isdigit():
        raise ValueError("{0!r} is not an eip155 id".format(value))
    cid = int(parts[1])
    chain = _BY_ID.get(cid)
    if chain is None:
        raise ValueError("{0!r} names chain id {1}, which ponk does not run on".format(value, cid))
    address = _addr(parts[2], "the address in the id")
    if len(parts) == 3:
        return EvmId(chain, cid, "address", address)
    tail = parts[3]
    if _HASH.match(tail):
        return EvmId(chain, cid, "v4_pool", address, pool_key_hash=tail.lower())
    if _ADDR.match(tail):
        return EvmId(chain, cid, "v2_position", address, owner=tail.lower())
    if tail.isdigit():
        return EvmId(chain, cid, "position_nft", address, nft_token_id=int(tail))
    raise ValueError("{0!r} is not an eip155 id".format(value))


def chain_of(value: Any) -> str:
    """The chain an id lives on: the EVM chain word for a CAIP id, else `solana`."""
    try:
        return parse_evm_id(value).chain
    except ValueError:
        return "solana"


def wei_to_native(wei: Optional[str]) -> Optional[Decimal]:
    """A wei string as whole coins (ETH or BNB), exactly. `None` stays `None`."""
    if wei is None:
        return None
    return Decimal(int(wei)) / (Decimal(10) ** 18)


# -- configs ------------------------------------------------------------------


def _ticks(fraction: float) -> int:
    # One tick is a price step of 1.0001, on every concentrated EVM venue.
    return max(int(round(math.log(1.0 + fraction) / math.log(1.0001))), 1)


def range_config(
    down_pct: float,
    up_pct: float,
    *,
    strategy: str = "bin_rebalancer",
    auto_compound: bool = True,
    stake: Optional[bool] = None,
    shape: Optional[str] = None,
    shape_ranges: Any = None,
) -> Dict[str, Any]:
    """The config of a range agent on a concentrated EVM venue.

    `down_pct` and `up_pct` are how far the range reaches below and above the
    price, as FRACTIONS: 0.05 is 5%. The keys are the ones the API reads,
    `range_pct: {down_pct, up_pct}`; `down` and `up` are refused. A ONE-SIDED
    range is one side at 0: `up_pct=0` puts the whole band below the price
    (it buys the base token as the price falls), `down_pct=0` puts it above.
    Both at 0 is refused.

    Built for the bin rebalancer family (`bin_rebalancer`, the default, and
    `conservative_bin_rebalancer`, which takes `min_out_of_range_secs` too:
    add it to the result). The range goes in as the edge percentages
    (`knobs.range_pct`), which is what the agent opens on, and as the same
    span in ticks (`target_bin_width`), which the engine reads only when the
    percentages are absent. For any other strategy write the config yourself;
    on `adaptive_range_recovery` the range is a top-level `range_pct`.

    `stake=True` stakes the position for CAKE or AERO (`config.evm.stake`),
    on the venues in `STAKE_VENUES` only. `shape` and `shape_ranges` are
    `with_shape`'s.
    """
    for name, value in (("down_pct", down_pct), ("up_pct", up_pct)):
        if not isinstance(value, (int, float)) or isinstance(value, bool) or not 0 <= value < 1:
            raise ValueError(
                "{0} is a fraction from 0 to below 1 (0.05 is 5%), got {1!r}".format(name, value)
            )
    if down_pct == 0 and up_pct == 0:
        raise ValueError(
            "down_pct and up_pct are both 0: give the range a width (one side at 0 is a "
            "one-sided range)"
        )
    width = (_ticks(down_pct) if down_pct > 0 else 0) + (_ticks(up_pct) if up_pct > 0 else 0)
    config: Dict[str, Any] = {
        "kind": strategy,
        "target_bin_width": width + 1,
        "auto_compound": auto_compound,
        "knobs": {"range_pct": {"down_pct": down_pct, "up_pct": up_pct}},
    }
    if stake is not None:
        config["evm"] = {"stake": bool(stake)}
    if shape is not None or shape_ranges is not None:
        config = with_shape(config, shape, shape_ranges=shape_ranges)
    return config


def _shape_ranges(value: Any) -> Any:
    if isinstance(value, str) and value.strip().lower() == "auto":
        return "auto"
    if isinstance(value, int) and not isinstance(value, bool) and 1 <= value <= 7:
        return value
    raise ValueError('shape_ranges is "auto" or a whole number from 1 to 7, got {0!r}'.format(value))


def with_shape(
    config: Mapping[str, Any],
    shape: Optional[str] = None,
    *,
    shape_ranges: Any = None,
    strategy: Optional[str] = None,
) -> Dict[str, Any]:
    """A copy of `config` with its liquidity shape set, as the API reads it.

    `shape` is one of `SHAPES`: `spot` (one even range, the default),
    `curve` (heaviest at the price), `bid_ask` (heaviest at the edges) or
    `spot_bid_ask` (half of each). It goes where the strategy keeps it:
    top level `liquidity_shape` on `adaptive_range_recovery`,
    `knobs.liquidity_shape` on every other strategy. The strategy is
    `strategy` when given, else the config's `kind`.

    On a venue in `SHAPE_VENUES` (Uniswap v3 on Arbitrum, Ethereum and
    Robinhood Chain, Uniswap v4 on Arbitrum and Ethereum, PancakeSwap v3 and
    Infinity CL on BNB Chain, Aerodrome Slipstream on Base) a shape other
    than Spot opens as several ranges managed as one position. The count is
    `shape_ranges`: `"auto"` (the default with a shape: chosen from live gas
    at the open, so that opening and later closing stays within 3% of the
    deposit) or 1 to 7, sent as `config.evm.shape_ranges`. The server lays
    the ranges out; a client never sends them. On every other range venue the
    agent keeps the shape and opens one range, and a `shape_ranges` there is
    refused. A full-range venue (`FULL_RANGE_VENUES`) takes no shape.

    Different shapes on the two sides of the price are the config keys
    `shape_below`, `shape_above` (each `spot`, `curve` or `bid_ask`) and
    `ratio_below_pct` (1 to 99, the share of the deposit placed below): set
    them in your own config beside `knobs`.
    """
    out: Dict[str, Any] = dict(config)
    if shape is not None:
        if shape not in SHAPES:
            raise ValueError("shape is one of {0}, got {1!r}".format(", ".join(SHAPES), shape))
        if (strategy or out.get("kind")) == "adaptive_range_recovery":
            out["liquidity_shape"] = shape
        else:
            knobs = dict(out.get("knobs") or {})
            knobs["liquidity_shape"] = shape
            out["knobs"] = knobs
    if shape_ranges is not None:
        block = dict(out.get("evm") or {})
        block["shape_ranges"] = _shape_ranges(shape_ranges)
        out["evm"] = block
    return out


def full_range_config(*, auto_compound: bool = True, stake: Optional[bool] = None) -> Dict[str, Any]:
    """The config of a compound and hold agent on a full-range venue.

    For `uniswap_v2`, `pancakeswap_v2` and `aerodrome_classic`, with
    `strategy="fee_collector"`. `stake` is for `aerodrome_classic` only.
    """
    config: Dict[str, Any] = {
        "kind": "fee_collector",
        "min_total_pending_base_units": 0,
        "auto_compound": auto_compound,
    }
    if stake is not None:
        config["evm"] = {"stake": bool(stake)}
    return config


def stake_config(stake: bool) -> Dict[str, Any]:
    """`{"evm": {"stake": bool}}`, the patch `update_config` takes on a farm venue."""
    return {"evm": {"stake": bool(stake)}}


# -- the fund-and-sign link ---------------------------------------------------


def _link_fields(agent: Any) -> Tuple[str, Optional[int], Optional[str]]:
    if isinstance(agent, str):
        return agent, None, None
    if isinstance(agent, Mapping):
        return (
            agent.get("id") or agent.get("agent_id") or "",
            agent.get("lamports"),
            agent.get("chain"),
        )
    return (
        getattr(agent, "id", None) or getattr(agent, "agent_id", None) or "",
        getattr(agent, "lamports", None),
        getattr(agent, "chain", None),
    )


def _amount(lamports: Any, chain: Optional[str]) -> str:
    # The amount field is lamports. An EVM agent is funded in its chain's own
    # coin, sent by the owner to the agent's address there, so it is listed
    # with 0, which means "sign the approval only".
    if is_evm_chain(chain):
        return "0"
    if isinstance(lamports, bool) or not isinstance(lamports, int):
        return "0"
    if lamports <= 0 or len(str(lamports)) > 16:
        return "0"
    return str(lamports)


def fund_links(agents: Iterable[Any], *, top_up: bool = False) -> List[str]:
    """The ponk.exchange pages where the owner funds agents and signs for them.

        https://ponk.exchange/agents/fund?a=<agent id>:<lamports>,<agent id>:<lamports>

    Each item of `agents` is an agent id, an `Agent` (or anything with `id`
    and optionally `chain` and `lamports`), or a mapping with those keys.
    `lamports` is the SOL to send to a Solana agent, a whole number; absent or
    0 means sign the approval only. An agent on an EVM chain is always listed
    with 0: it is funded in its chain's own coin (BNB on BNB Chain, ETH on the
    others), which the owner sends to the agent's address on that chain, and
    its approval is two signatures on the page (the Solana login wallet, then
    the EVM wallet that will receive withdrawals).

    At most 40 agents go in one link, so a longer list returns several. An id
    that is not a UUID is left out and a repeated one is listed once; with
    nothing usable the result is empty. `top_up=True` adds `&mode=top_up`, so
    the page sends only what each wallet is still short of and opening the
    link twice never sends the same SOL twice.

    NOTHING IS SIGNED HERE. A link names agent ids only; the page resolves
    every destination for the signed-in account, so a link cannot name an
    address. This client holds no key and offers no signer.
    """
    seen = set()
    parts: List[Tuple[str, str]] = []
    for agent in agents:
        raw_id, lamports, chain = _link_fields(agent)
        agent_id = raw_id.strip().lower() if isinstance(raw_id, str) else ""
        if not _UUID.match(agent_id) or agent_id in seen:
            continue
        seen.add(agent_id)
        parts.append((agent_id, _amount(lamports, chain)))
    links: List[str] = []
    for start in range(0, len(parts), IDS_PER_FUND_LINK):
        chunk = parts[start : start + IDS_PER_FUND_LINK]
        funded = any(amount != "0" for _, amount in chunk)
        links.append(
            "{0}/agents/fund?a={1}{2}".format(
                SITE,
                ",".join("{0}:{1}".format(i, a) for i, a in chunk),
                "&mode=top_up" if top_up and funded else "",
            )
        )
    return links


def fund_link(agent_id: str, lamports: int = 0, *, chain: Optional[str] = None) -> str:
    """One agent's fund-and-sign link. See `fund_links`.

    Raises `ValueError` when `agent_id` is not an agent id, rather than
    returning a link that opens on nothing.
    """
    links = fund_links([{"id": agent_id, "lamports": lamports, "chain": chain}])
    if not links:
        raise ValueError("{0!r} is not an agent id".format(agent_id))
    return links[0]


def agent_page_url(agent_id: str) -> str:
    """An agent's own page. For an EVM agent it shows the deposit address and
    takes the two approval signatures."""
    return "{0}/agents/{1}".format(SITE, agent_id.strip().lower())
