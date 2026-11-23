"""Constants and pure helpers for Perpetuals agents on Hyperliquid.

A Perpetuals agent is one typed trade: a market, a direction (long or short),
isolated leverage, the USDC it trades with and a stop that is always set. The
server builds the ticket from Hyperliquid's live prices and refuses one its
gates refuse; this module only holds the vocabulary, the shape checks this
client makes before sending and the high leverage warning. Nothing in it
sends a request.

Leverage is the user's choice, up to the market's own maximum on
Hyperliquid (the server checks that). Above 10x the server asks for the
warning to be acknowledged: `create_perp_agent` takes the agent's name,
typed by the user after `HIGH_LEVERAGE_WARNING`, as `confirmation_text`.

    from ponk import perpetuals

    perpetuals.market_id("BTC")              # 'perp:hyperliquid:BTC'
    perpetuals.check_leverage(20)            # 20
    perpetuals.needs_confirmation(20)        # True: type the agent's name
    perpetuals.HIGH_LEVERAGE_WARNING         # the sentence to show first

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
    "HIGH_LEVERAGE_ABOVE",
    "HIGH_LEVERAGE_WARNING",
    "MIN_CAPITAL_USD",
    "DIRECTIONS",
    "MARKET_SORTS",
    "CANDLE_INTERVALS",
    "MAX_CANDLE_BARS",
    "MAX_FUNDING_DAYS",
    "MAX_STOP_PCT",
    "market_id",
    "coin_of",
    "check_leverage",
    "needs_confirmation",
    "check_direction",
]

#: The one Perpetuals venue.
VENUE = "hyperliquid"

#: Above this leverage the server asks for the high leverage warning to be
#: acknowledged with the agent's typed name. It is not a cap: an agent can
#: run up to its market's own maximum on Hyperliquid.
HIGH_LEVERAGE_ABOVE = 10

#: The high leverage warning, word for word as the server and the app state
#: it on the confirm step above 10x. Show it before asking for the name.
HIGH_LEVERAGE_WARNING = (
    "Above 10x a sudden 4% price jump can pass the liquidation price before any stop fills."
)

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


def check_leverage(leverage: Any) -> int:
    """`leverage` as a positive int, or `ValueError`.

    Only the shape is checked here. How high a market goes is the server's to
    say: up to the market's own maximum on Hyperliquid, and above 10x with
    the warning acknowledged (`needs_confirmation`).
    """
    if isinstance(leverage, bool) or not isinstance(leverage, int):
        if isinstance(leverage, float) and leverage.is_integer():
            leverage = int(leverage)
        else:
            raise ValueError("leverage is a whole number, 1 or more, got {0!r}".format(leverage))
    if leverage < 1:
        raise ValueError("leverage is a whole number, 1 or more, got {0!r}".format(leverage))
    return leverage


def needs_confirmation(leverage: Any) -> bool:
    """True above 10x: the server creates the agent only with its name typed
    as `confirmation_text` after `HIGH_LEVERAGE_WARNING` was shown."""
    return check_leverage(leverage) > HIGH_LEVERAGE_ABOVE


def check_direction(direction: Any) -> str:
    """`long` or `short`, else `ValueError` with the server's sentence."""
    if direction not in DIRECTIONS:
        raise ValueError("Say long or short.")
    return direction
