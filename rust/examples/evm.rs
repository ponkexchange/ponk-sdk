//! Agents on an EVM chain: list the pools on BNB Chain, quote the first open,
//! and (only with PONK_CREATE=1) create an agent on the pool with the most LP
//! fees, then print its deposit address and the fund-and-sign link.
//!
//!     PONK_API_KEY=ponk_live_... cargo run --example evm
//!     PONK_API_KEY=ponk_live_... PONK_CREATE=1 cargo run --example evm
//!
//! Nothing here moves money. An agent acts only after you send its chain's
//! coin to its address and sign its mandate with your own wallets on
//! ponk.exchange.

use ponk::{evm, Client, EvmPool, NewAgent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = std::env::var("PONK_API_KEY").map_err(|_| "set PONK_API_KEY")?;
    let ponk = Client::new(ponk::DEFAULT_BASE_URL, key)?;
    let chain = "bsc";
    let venue = "pancakeswap_v3";

    let board = ponk.evm_pools(chain, Some(venue)).await?;
    let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis() as i64;
    let fees = |p: &EvmPool| p.fees.as_ref().and_then(|f| f.h24).unwrap_or(0.0);
    // A flagged pool (an unverified token, a hook an agent cannot use, under
    // an hour old) is left out, as the copilot leaves it out.
    let pool = board
        .pools
        .iter()
        .filter(|p| p.flags(now_ms).is_empty() && fees(p) > 0.0)
        .max_by(|a, b| fees(a).total_cmp(&fees(b)))
        .ok_or("the board has no unflagged pool with fees right now")?;
    let pool_id = pool.address.clone().ok_or("the pool has no id")?;
    println!(
        "{} on {venue}: fee tier {:?}%, TVL {:?}, LP fees 24h {:?}, farm {:?}",
        pool.pair(),
        pool.fee_pct,
        pool.tvl,
        pool.fees.as_ref().and_then(|f| f.h24),
        pool.farm.as_ref().map(|f| (f.token.clone(), f.live, f.apr_pct)),
    );

    // The rates on this quote are the ones an agent this key creates pays.
    let cost = ponk.evm_open_cost(chain, venue, Some(&pool_id), None).await?;
    let coin = evm::native_symbol(chain).unwrap_or("");
    println!(
        "first open: at least {} {coin} in the agent wallet, plus the amount to deploy; \
         ponk fee {:?} bps of the first open, {:?} bps of fees earned",
        cost.min_open_wei.as_deref().and_then(evm::wei_to_native).unwrap_or_else(|| "-".into()),
        cost.activation_fee_bps,
        cost.performance_fee_bps,
    );

    if std::env::var("PONK_CREATE").as_deref() != Ok("1") {
        println!("set PONK_CREATE=1 to create the agent");
        return Ok(());
    }

    let me = ponk.whoami().await?;
    let wallet = me.wallet_address.ok_or("the key answered without a wallet")?;
    // 0.05 is 5% either side of the price. `stake` farms CAKE.
    let new = NewAgent::evm_range(format!("bnb {}", pool.pair()), wallet, chain, venue, pool_id, 0.05, 0.05)?
        .stake(true)
        .dry_run(false);
    let agent = ponk.create_evm_agent(&new).await?;
    let agent_id = agent.id.ok_or("the agent has no id")?;

    let agent_wallet = ponk.create_agent_wallet(&agent_id).await?;
    println!("send {coin} to {}", agent_wallet.address.as_deref().unwrap_or("-"));
    println!("then sign: {}", evm::fund_link(&agent_id, Some(chain)).unwrap_or_default());
    Ok(())
}
