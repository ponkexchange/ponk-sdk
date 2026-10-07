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
//! * It cannot mint a custody mandate, and it cannot revoke one. The mandate
//!   is a wallet signature over an exact canonical text, and that signature
//!   can only be produced in the ponk app with your wallet connected. What a
//!   `trade` key CAN do is use a mandate that already exists:
//!   [`Client::set_autonomous`] with `true` turns autonomous signing on for an
//!   agent whose own agent wallet holds an active, unexpired mandate, and with
//!   `false` turns it off at any time. Turning it off does not revoke the
//!   mandate, so a later `true` can re-arm the agent until the mandate
//!   expires. Revoking the mandate itself happens in the app.
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
//! # The mandate expires, and that is the failure to design for
//!
//! A mandate carries a hard expiry, and the grant the app asks you to sign
//! runs 30 days by default. Lapsing is a pure clock fact: no transaction and no
//! on-chain state change. The agent keeps running and every signature it asks
//! for is denied, so what you see from here is an agent that quietly stopped
//! compounding, claiming and recentering with its funds still deployed. The
//! symptom is silence, not an error. Three consequences for your code:
//!
//! * A fund call can return `Ok` and still have done nothing.
//!   [`Client::compound`] returns `Err` only for an HTTP failure, and a
//!   mandate denial is not one: it arrives as a 200 carrying a FAILED
//!   [`ActionReceipt`]. Check `status`, `action_status` and `error_message`,
//!   and read `signature == None` as "nothing reached the chain". Never read
//!   `Ok` as success on a fund route.
//! * Watch the expiry yourself. [`Client::agent_mandate`] returns the live
//!   mandate with `expires_at`, `seconds_remaining` (negative once lapsed) and
//!   `status` (`active` or `expired`). For a push instead of a poll, register
//!   a webhook from a signed-in session (`POST /me/webhooks`) for
//!   `mandate_expiring` and `mandate_expired`, and check each delivery with
//!   [`webhooks::verify`]. The server warns from five
//!   days ahead and keeps reminding for fourteen days after expiry.
//! * A lapsed mandate can never trap your funds. [`Client::withdraw`] and
//!   [`Client::exit`] are user-initiated and deliberately bypass the mandate,
//!   so both keep working after it expires. Renewing it needs a fresh wallet
//!   signature in the app.
//!
//! # Scope
//!
//! This crate covers the key-authenticated `/v1` API plus the four public
//! reads that need no key at all ([`Client::health`], [`Client::pool`],
//! [`Client::range_cost`], [`Client::ponk_perks`]), and, behind the default
//! `webhooks` feature, receiver-side verification of webhook deliveries in
//! [`webhooks`]. The rest of the server's surface, the session-authenticated
//! app routes (including registering webhooks, minting API keys and an
//! agent's decision history) and the wallet-signed Meteora transaction
//! builders, needs a wallet session rather than a key, so it is out of scope
//! here.

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

mod client;
mod error;
mod models;
#[cfg(feature = "webhooks")]
pub mod webhooks;

pub use client::{Client, ClientBuilder, DEFAULT_BASE_URL, DEFAULT_FUND_TIMEOUT, DEFAULT_TIMEOUT};
pub use error::{ApiError, Error, ErrorKind};
pub use models::{
    ActionLog, ActionReceipt, Agent, AgentPerformance, AgentPosition, AgentWallet,
    AutonomousView, BinShare, ClaimPayout, ClaimPayoutToken, ComponentStatus, ExitSnapshot,
    FeeRates, Health, HealthChecks, Mandate, NewAgent, PonkPerks, PoolSnapshot, Position,
    RangeCostQuote, TokenAmount, WhoAmI, Withdrawal,
};
