//! Constants and pure helpers for Perpetuals agents on Hyperliquid.
//!
//! A Perpetuals agent is one typed trade: a market, a direction (long or
//! short), isolated leverage, the USDC it trades with and a stop that is
//! always set. The server builds the ticket from Hyperliquid's live prices and
//! refuses one its gates refuse; this module holds the vocabulary, the request
//! body ([`PerpTrade`]), the shape checks this crate makes before sending and
//! the high leverage warning. Nothing in it sends a request.
//!
//! Leverage is the user's choice, up to the market's own maximum on
//! Hyperliquid (the server checks that). Above 10x the server asks for the
//! warning to be acknowledged: the agent's name, typed by the user after
//! [`HIGH_LEVERAGE_WARNING`], goes in [`PerpTrade::confirmation_text`].
//!
//! ```
//! use ponk::perpetuals::{self, PerpTrade};
//!
//! assert_eq!(perpetuals::market_id("BTC"), "perp:hyperliquid:BTC");
//! let trade = PerpTrade::new("BTC", "long", 3, "500", "3").take_profit_pct("10");
//! assert!(trade.check().is_ok());
//! assert!(!perpetuals::needs_confirmation(3));
//! let high = PerpTrade::new("BTC", "long", 20, "500", "3").name("BTC 20x").confirmation_text("BTC 20x");
//! assert!(high.check().is_ok());
//! assert!(perpetuals::needs_confirmation(20));
//! ```
//!
//! Units, everywhere on the Perpetuals routes: decimal figures the server
//! keeps exactly (capital, realized net, balances, ledger amounts) are
//! strings; figures computed from live prices (the ticket, marks, liquidation
//! prices, funding rates) are numbers. A figure the server could not read is
//! `None`, never 0. Funding and fee rates are fractions; percents are named
//! `..._pct`.

use serde::Serialize;

use crate::Error;

/// The one Perpetuals venue.
pub const VENUE: &str = "hyperliquid";

/// Above this leverage the server asks for the high leverage warning to be
/// acknowledged with the agent's typed name. It is not a cap: an agent can run
/// up to its market's own maximum on Hyperliquid.
pub const HIGH_LEVERAGE_ABOVE: u32 = 10;

/// The high leverage warning, word for word as the server and the app state it
/// on the confirm step above 10x. Show it before asking for the name.
pub const HIGH_LEVERAGE_WARNING: &str =
    "Above 10x a sudden 4% price jump can pass the liquidation price before any stop fills.";

/// Hyperliquid's smallest order, in USDC: the least an agent trades with.
pub const MIN_CAPITAL_USD: u32 = 10;

/// The two sides. The agent never reverses the one you chose.
pub const DIRECTIONS: [&str; 2] = ["long", "short"];

/// The orders the markets board takes (`sort=`), largest first.
pub const MARKET_SORTS: [&str; 5] = ["volume", "oi", "change", "funding", "leverage"];

/// The candle intervals Hyperliquid serves.
pub const CANDLE_INTERVALS: [&str; 14] = [
    "1m", "3m", "5m", "15m", "30m", "1h", "2h", "4h", "8h", "12h", "1d", "3d", "1w", "1M",
];

/// At most this many candles per read (fewer at 1w and 1M).
pub const MAX_CANDLE_BARS: u32 = 5000;

/// At most this many days of funding history per read.
pub const MAX_FUNDING_DAYS: u32 = 30;

/// A stop is a percent from the price, more than 0 and under this.
pub const MAX_STOP_PCT: u32 = 90;

/// `perp:hyperliquid:<COIN>`, the id the market routes take. The coin is
/// kept exactly as given: Hyperliquid's coins are case sensitive (`kPEPE`).
/// An id that already starts with `perp:` comes back as it is.
pub fn market_id(coin: &str) -> String {
    let coin = coin.trim();
    if coin.starts_with("perp:") {
        coin.to_string()
    } else {
        format!("perp:{VENUE}:{coin}")
    }
}

/// The coin of a market id (`perp:hyperliquid:BTC` is `BTC`); a bare coin
/// comes back as it is.
pub fn coin_of(market: &str) -> &str {
    market.strip_prefix("perp:hyperliquid:").unwrap_or(market)
}

/// `Ok` for 1 or more. Only the shape is checked here: how high a market goes
/// is the server's to say, up to the market's own maximum on Hyperliquid and
/// above 10x with the warning acknowledged ([`needs_confirmation`]).
pub fn check_leverage(leverage: u32) -> Result<u32, Error> {
    if leverage == 0 {
        return Err(Error::InvalidArgument("leverage is a whole number, 1 or more, got 0".into()));
    }
    Ok(leverage)
}

/// True above 10x: the server creates the agent only with its name typed as
/// [`PerpTrade::confirmation_text`] after [`HIGH_LEVERAGE_WARNING`] was shown.
pub fn needs_confirmation(leverage: u32) -> bool {
    leverage > HIGH_LEVERAGE_ABOVE
}

/// The body of `POST /v1/perp-agents` and its preview: one typed trade.
///
/// Money and percents are strings so they stay exact (`"500"`, `"3"`,
/// `"2.5"`); the server reads them as decimals. `stop_loss_pct` is percent
/// from the entry (3 for 3%) and is required: no position runs without a
/// stop. `name` defaults to the coin on the server. `practice` runs on live
/// mainnet prices with a practice balance and sends nothing. Above 10x,
/// `confirmation_text` is the agent's name as the user typed it after the high
/// leverage warning.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PerpTrade {
    pub coin: String,
    pub direction: String,
    pub leverage: u32,
    pub capital_usd: String,
    pub stop_loss_pct: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub take_profit_pct: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub practice: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation_text: Option<String>,
}

impl PerpTrade {
    /// A live trade. `direction` is `long` or `short`.
    pub fn new(
        coin: impl Into<String>,
        direction: impl Into<String>,
        leverage: u32,
        capital_usd: impl Into<String>,
        stop_loss_pct: impl Into<String>,
    ) -> Self {
        Self {
            coin: coin.into(),
            direction: direction.into(),
            leverage,
            capital_usd: capital_usd.into(),
            stop_loss_pct: stop_loss_pct.into(),
            take_profit_pct: None,
            name: None,
            practice: false,
            confirmation_text: None,
        }
    }

    /// A take-profit, percent from the entry.
    pub fn take_profit_pct(mut self, pct: impl Into<String>) -> Self {
        self.take_profit_pct = Some(pct.into());
        self
    }

    /// The agent's name, 80 characters at most.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Practice mode: live mainnet prices, a practice balance, nothing sent.
    pub fn practice(mut self, practice: bool) -> Self {
        self.practice = practice;
        self
    }

    /// Above 10x: the agent's name as the user typed it after
    /// [`HIGH_LEVERAGE_WARNING`]. Without it the server answers 422 with the
    /// warning and the name to type.
    pub fn confirmation_text(mut self, typed: impl Into<String>) -> Self {
        self.confirmation_text = Some(typed.into());
        self
    }

    /// The shape checks this crate makes before sending: leverage 1 or more
    /// and the side, in the server's words. Every other gate is the server's,
    /// including the market's maximum leverage, because it reads live prices.
    pub fn check(&self) -> Result<(), Error> {
        check_leverage(self.leverage)?;
        if !DIRECTIONS.contains(&self.direction.as_str()) {
            return Err(Error::InvalidArgument("Say long or short.".into()));
        }
        Ok(())
    }
}
