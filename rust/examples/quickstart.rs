//! Read every agent this key owns, then show one in detail.
//!
//!     PONK_API_KEY=ponk_live_... cargo run --example quickstart
//!
//! Read-only: it creates nothing and moves no funds, so a `read` key is
//! enough.

use ponk::{Client, Error, ErrorKind, DEFAULT_BASE_URL};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let key = std::env::var("PONK_API_KEY").expect("set PONK_API_KEY");
    let base = std::env::var("PONK_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
    let ponk = Client::new(base, key)?;

    let me = match ponk.whoami().await {
        Ok(me) => me,
        Err(e) => {
            if let Some(api) = e.api() {
                if api.kind() == ErrorKind::Unauthorized {
                    eprintln!("that key is missing, revoked or not a ponk key");
                }
            }
            return Err(e);
        }
    };
    println!(
        "wallet {}  scope {}",
        me.wallet_address.as_deref().unwrap_or("-"),
        me.scope.as_deref().unwrap_or("-")
    );

    let agents = ponk.list_agents().await?;
    println!("{} agent(s)", agents.len());

    for agent in &agents {
        let Some(id) = agent.id.as_deref() else {
            continue;
        };
        println!(
            "\n{}  {}  {}  {}",
            agent.name.as_deref().unwrap_or("-"),
            agent.status.as_deref().unwrap_or("-"),
            agent.strategy.as_deref().unwrap_or("-"),
            if agent.dry_run.unwrap_or(false) {
                "dry run"
            } else {
                "live"
            }
        );

        // Every USD field is a string or None. None means the figure could not
        // be priced, never zero, so print a dash rather than a number that is
        // not there.
        let perf = ponk.agent_performance(id).await?;
        println!(
            "  value {}  pnl {}  fees(lifetime) {}  source {}",
            perf.current_value_usd.as_deref().unwrap_or("-"),
            perf.pnl_usd.as_deref().unwrap_or("-"),
            perf.fees_earned_lifetime_usd.as_deref().unwrap_or("-"),
            perf.pnl_source.as_deref().unwrap_or("-")
        );

        let pos = ponk.agent_position(id).await?;
        if pos.position_address.is_some() {
            println!(
                "  range {} to {}  current {}  {}",
                pos.lower_price.as_deref().unwrap_or("-"),
                pos.upper_price.as_deref().unwrap_or("-"),
                pos.current_price.as_deref().unwrap_or("-"),
                if pos.in_range.unwrap_or(false) {
                    "in range"
                } else {
                    "OUT OF RANGE"
                }
            );
        }
    }

    for log in ponk.list_logs(Some(10)).await? {
        println!(
            "{}  {}  {}  {}",
            log.created_at.as_deref().unwrap_or("-"),
            log.action_type.as_deref().unwrap_or("-"),
            log.status.as_deref().unwrap_or("-"),
            log.tx_signature.as_deref().unwrap_or("")
        );
    }

    Ok(())
}
