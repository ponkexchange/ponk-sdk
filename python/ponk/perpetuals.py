"""Constants and pure helpers for Perpetuals agents on Hyperliquid.

A Perpetuals agent is one typed trade: a market, a direction (long or short),
isolated leverage, the USDC it trades with and a stop that is always set. The
server builds the ticket from Hyperliquid's live prices and refuses one its
gates refuse; this module only holds the vocabulary and the one check this
client makes before sending, the 10x agent cap, worded exactly as the server
words it. Nothing in it sends a request.

    from ponk import perpetuals

    perpetuals.market_id("BTC")          # 'perp:hyperliquid:BTC'
    perpetuals.check_leverage(3)         # 3
    perpetuals.check_leverage(20)        # ValueError, the server's sentence

Units, everywhere on the Perpetuals routes: decimal figures the server keeps
exactly (capital, realized net, balances, ledger amounts) arrive as strings;
figures computed from live prices (the ticket, marks, liquidation prices,
funding rates) arrive as numbers. A figure the server could not read is
``None``, never 0. Funding and fee rates are fractions; percents are named
``..._pct``.
"""

from __future__ import annotations

from typing import Any, Tuple

__all__ = [
    "VENUE",
    "AGENT_MAX_LEVERAGE",
    "MIN_CAPITAL_USD",
    "DIRECTIONS",
    "MARKET_SORTS",
    "CANDLE_INTERVALS",
    "MAX_CANDLE_BARS",
    "MAX_FUNDING_DAYS",
    "MAX_STOP_PCT",
    "market_id",
    "coin_of",
    "leverage_refusal",
    "check_leverage",
    "check_direction",
]

#: The one Perpetuals venue.
VENUE = "hyperliquid"

#: No agent runs above 10x leverage: above it a sudden 4% price jump can pass
#: the liquidation price before any stop fills. A market whose Hyperliquid
#: maximum is lower keeps its own lower maximum (the server says so).
AGENT_MAX_LEVERAGE = 10

#: Hyperliquid's smallest order, in USDC: the least an agent trades with.
MIN_CAPITAL_USD = 10

#: The two sides. The agent never reverses the one you chose.
DIRECTIONS: Tuple[str, ...] = ("long", "short")

#: The orders the markets board takes (`sort=`), largest first.
MARKET_SORTS: Tuple[str, ...] = ("volume", "oi", "change", "funding", "leverage")

#: The candle intervals Hyperliquid serves.
CANDLE_INTERVALS: Tuple[str, ...] = (
    "1m", "3m", "5m", "15m", "30m", "1h", "2h", "4h", "8h", "12h", "1d", "3d", "1w", "1M",
)

#: At most this many candles per read (fewer at 1w and 1M).
MAX_CANDLE_BARS = 5000

#: At most this many days of funding history per read.
MAX_FUNDING_DAYS = 30

#: A stop is a percent from the price, more than 0 and under this.
MAX_STOP_PCT = 90


def market_id(coin: str) -> str:
    """`perp:hyperliquid:<COIN>`, the id the market routes take.

    The coin is kept exactly as given: Hyperliquid's coins are case sensitive
    (`kPEPE`). The market routes also take a bare coin.
    """
    if not isinstance(coin, str) or not coin.strip():
        raise ValueError("Name a Hyperliquid market, like BTC.")
    coin = coin.strip()
    if coin.startswith("perp:"):
        return coin
    return "perp:{0}:{1}".format(VENUE, coin)


def coin_of(market: str) -> str:
    """The coin of a market id (`perp:hyperliquid:BTC` is `BTC`); a bare coin
    comes back as it is."""
    prefix = "perp:{0}:".format(VENUE)
    return market[len(prefix):] if market.startswith(prefix) else market


def leverage_refusal(leverage: Any) -> str:
    """The server's sentence for leverage above the agent cap, word for word."""
    return (
        "{0}x is above the {1}x most an agent can use. Above {1}x a sudden 4% price jump "
        "can pass the liquidation price before any stop fills, so pick {1}x or less."
    ).format(leverage, AGENT_MAX_LEVERAGE)


def check_leverage(leverage: Any) -> int:
    """`leverage` as an int, or `ValueError`.

    Above 10 raises with the server's own sentence (`leverage_refusal`), so a
    caller sees the same words whether this client or the server stopped it.
    The server stays authoritative: a market whose Hyperliquid maximum is
    lower than 10 is refused there with its own figure.
    """
    if isinstance(leverage, bool) or not isinstance(leverage, int):
        if isinstance(leverage, float) and leverage.is_integer():
            leverage = int(leverage)
        else:
            raise ValueError(
                "leverage is a whole number from 1 to {0}, got {1!r}".format(AGENT_MAX_LEVERAGE, leverage)
            )
    if leverage > AGENT_MAX_LEVERAGE:
        raise ValueError(leverage_refusal(leverage))
    if leverage < 1:
        raise ValueError(
            "leverage is a whole number from 1 to {0}, got {1!r}".format(AGENT_MAX_LEVERAGE, leverage)
        )
    return leverage


def check_direction(direction: Any) -> str:
    """`long` or `short`, else `ValueError` with the server's sentence."""
    if direction not in DIRECTIONS:
        raise ValueError("Say long or short.")
    return direction
