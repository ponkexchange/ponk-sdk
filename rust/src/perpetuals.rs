//! Constants and pure helpers for Perpetuals agents on Hyperliquid.
//!
//! A Perpetuals agent is one typed trade: a market, a direction (long or
//! short), isolated leverage, the USDC it trades with and a stop that is
//! always set. The server builds the ticket from Hyperliquid's live prices and
//! refuses one its gates refuse; this module holds the vocabulary, the request
//! body ([`PerpTrade`]) and the one check this crate makes before sending, the
//! 10x agent cap, worded exactly as the server words it. Nothing in it sends a
//! request.
//!
//! ```
//! use ponk::perpetuals::{self, PerpTrade};
//!
//! assert_eq!(perpetuals::market_id("BTC"), "perp:hyperliquid:BTC");
//! let trade = PerpTrade::new("BTC", "long", 3, "500", "3").take_profit_pct("10");
//! assert!(trade.check().is_ok());
//! let err = PerpTrade::new("BTC", "long", 20, "500", "3").check().unwrap_err();
//! assert!(err.to_string().starts_with("20x is above the 10x most an agent can use."));
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

/// No agent runs above 10x leverage: above it a sudden 4% price jump can pass
/// the liquidation price before any stop fills. A market whose Hyperliquid
/// maximum is lower keeps its own lower maximum (the server says so).
pub const AGENT_MAX_LEVERAGE: u32 = 10;

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

/// The server's sentence for leverage above the agent cap, word for word.
pub fn leverage_refusal(leverage: u32) -> String {
    format!(
        "{leverage}x is above the {AGENT_MAX_LEVERAGE}x most an agent can use. Above {AGENT_MAX_LEVERAGE}x a sudden 4% price jump can pass the liquidation price before any stop fills, so pick {AGENT_MAX_LEVERAGE}x or less."
    )
}

/// `Ok` for 1 to 10. Above 10 is [`Error::InvalidArgument`] with the
/// server's own sentence ([`leverage_refusal`]), so a caller sees the same
/// words whether this crate or the server stopped it. The server stays
/// authoritative: a market whose Hyperliquid maximum is lower than 10 is
/// refused there with its own figure.
pub fn check_leverage(leverage: u32) -> Result<u32, Error> {
    if leverage > AGENT_MAX_LEVERAGE {
        return Err(Error::InvalidArgument(leverage_refusal(leverage)));
    }
    if leverage == 0 {
        return Err(Error::InvalidArgument(format!(
            "leverage is a whole number from 1 to {AGENT_MAX_LEVERAGE}, got 0"
        )));
    }
    Ok(leverage)
}

/// The body of `POST /v1/perp-agents` and its preview: one typed trade.
///
/// Money and percents are strings so they stay exact (`"500"`, `"3"`,
/// `"2.5"`); the server reads them as decimals. `stop_loss_pct` is percent
/// from the entry (3 for 3%) and is required: no position runs without a
/// stop. `name` defaults to the coin on the server. `practice` runs on live
/// mainnet prices with a practice balance and sends nothing.
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

    /// The checks this crate makes before sending: the 10x cap and the side,
    /// each in the server's own words. Every other gate is the server's,
    /// because it reads live prices.
    pub fn check(&self) -> Result<(), Error> {
        check_leverage(self.leverage)?;
        if !DIRECTIONS.contains(&self.direction.as_str()) {
            return Err(Error::InvalidArgument("Say long or short.".into()));
        }
        Ok(())
    }
}
