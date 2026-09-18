//! A thin Rust client for the ponk public API: create and drive Solana DLMM
//! liquidity agents, read their live positions, PnL and fees, and move funds
//! home.
//!
//! One method per endpoint, typed results, and no behaviour of its own. It
//! builds the request, sends it, maps the shared error envelope to
//! [`enum@Error`] and decodes the body. It never computes, rounds or fills in a
//! number the server did not send.
//!
//! ```no_run
//! #[tokio::main]
//! async fn main() -> Result<(), ponk::Error> {
//!     let key = std::env::var("PONK_API_KEY").expect("PONK_API_KEY");
//!     let ponk = ponk::Client::new(ponk::DEFAULT_BASE_URL, key)?;
//!
//!     let me = ponk.whoami().await?;
//!     println!("acting as {:?} with scope {:?}", me.wallet_address, me.scope);
//!
//!     for agent in ponk.list_agents().await? {
//!         let id = agent.id.clone().unwrap_or_default();
//!         let perf = ponk.agent_performance(&id).await?;
//!         // A null USD figure means "could not be priced", never zero, so
//!         // print a dash rather than a number you do not have.
//!         println!(
//!             "{:?} value {} pnl {}",
//!             agent.name,
//!             perf.current_value_usd.as_deref().unwrap_or("-"),
//!             perf.pnl_usd.as_deref().unwrap_or("-"),
//!         );
//!     }
//!     Ok(())
//! }
//! ```
//!
//! # What a key cannot do
//!
//! Three limits are structural. They are properties of how custody works in
//! ponk, not settings that can be turned off.
//!
//! * It cannot sign with your connected wallet. Non-custodial actions still
//!   need your signature in the app. This crate can read their state, not
//!   produce the signature.
//! * It cannot grant itself custody. Turning an agent autonomous requires a
//!   one-time custody mandate you sign with your wallet. Create the agent
//!   here, enable autonomous mode once in the app, and from then on this crate
//!   can drive it.
//! * It cannot act on an agent that holds no wallet of its own.
//!   [`Client::compound`], [`Client::withdraw`], [`Client::exit`] and
//!   [`Client::agent_wallet`] operate on the isolated wallet an autonomous
//!   agent owns. A self-custody agent has no such wallet, so those four do not
//!   apply to it. Reads, pause, resume, dry-run and mode do.
//!
//! [`Client::withdraw`] and [`Client::exit`] have no destination parameter and
//! cannot be given one. The server locks the destination to the wallet that
//! owns the agent. A stolen key can move your funds home. It cannot move them
//! anywhere else.
//!
//! # Scope
//!
//! This crate covers the key-authenticated `/v1` API plus the three public
//! reads that need no key at all ([`Client::health`], [`Client::pool`],
//! [`Client::ponk_perks`]). The rest of the server's surface, the
//! session-authenticated app routes and the wallet-signed Meteora transaction
//! builders, needs a wallet signature rather than a key, so it is out of scope
//! here.

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

mod client;
mod error;
mod models;

pub use client::{Client, ClientBuilder, DEFAULT_BASE_URL, DEFAULT_FUND_TIMEOUT, DEFAULT_TIMEOUT};
pub use error::{ApiError, Error, ErrorKind};
pub use models::{
    ActionLog, ActionReceipt, Agent, AgentPerformance, AgentPosition, AgentWallet, BinShare,
    ClaimPayout, ClaimPayoutToken, ComponentStatus, ExitSnapshot, FeeRates, Health, HealthChecks,
    NewAgent, PonkPerks, PoolSnapshot, Position, TokenAmount, WhoAmI, Withdrawal,
};
