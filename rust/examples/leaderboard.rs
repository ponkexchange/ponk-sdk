//! Read the Top Ponkillionaires board, show how its top agent would be
//! copied, and print this account's points and referral standing.
//!
//!     PONK_API_KEY=ponk_live_... cargo run --example leaderboard
//!
//! Creates nothing unless PONK_COPY=1 is set, and then creates a DRY-RUN copy
//! (a `trade` key is needed for that). The board itself needs no key.

use ponk::{Client, Error, ErrorKind, DEFAULT_BASE_URL};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let key = std::env::var("PONK_API_KEY").expect("set PONK_API_KEY");
    let base = std::env::var("PONK_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
    let ponk = Client::new(base, key)?;

    let board = ponk.leaderboard(Some("7d")).await?;
    for row in board.ranked.iter().take(5) {
        println!(
            "#{:<3} {:<12} {:<28} vs hodl {:>8}  pnl {}",
            row.rank.unwrap_or(0),
            row.slug.as_deref().unwrap_or("-"),
            row.strategy.as_deref().unwrap_or("-"),
            row.vs_hodl_pct.map(|p| format!("{p:.2}%")).unwrap_or_else(|| "-".into()),
            // None means hidden or unpriceable, never zero.
            row.pnl_usd.as_deref().unwrap_or("-"),
        );
    }

    let me = ponk.whoami().await?;
    let wallet = me.wallet_address.clone().unwrap_or_default();
    if let Some(top) = board.ranked.first() {
        if let Some(mut body) = top.copy.to_new_agent("copied from the board", &wallet) {
            body.dry_run = Some(true);
            println!("copy would create {} on {:?}", body.strategy, body.pool_address);
            if std::env::var("PONK_COPY").as_deref() == Ok("1") {
                let agent = ponk.create_agent(&body).await?;
                println!("created dry-run agent {}", agent.id.as_deref().unwrap_or("-"));
            }
        }
    }

    let points = ponk.points().await?;
    println!(
        "points {} rank {} of {} ({})",
        points.total_points.as_deref().unwrap_or("-"),
        points.rank.map(|r| r.to_string()).unwrap_or_else(|| "-".into()),
        points.participant_count.unwrap_or(0),
        points.season.as_ref().and_then(|s| s.name.as_deref()).unwrap_or("off-season"),
    );

    let r = ponk.referral().await?;
    // Three separate facts. Never add them.
    println!(
        "referrals {}: earned ${:.2}, owed ${:.2}, unproven ${:.2}",
        r.referred_count.unwrap_or(0),
        r.total_earned_usd.unwrap_or(0.0),
        r.owed_usd.unwrap_or(0.0),
        r.unproven_usd.unwrap_or(0.0),
    );
    match ponk.creator_dashboard(None).await {
        Ok(d) => println!(
            "creator /r/{}: earned ${:.2} over {} days",
            d.handle.as_deref().unwrap_or("-"),
            d.totals.earned_usd.unwrap_or(0.0),
            d.days.len()
        ),
        Err(e) if e.api().map(|a| a.kind()) == Some(ErrorKind::NotFound) => {
            println!("not a Creator yet: claim a handle with claim_handle")
        }
        Err(e) => return Err(e),
    }
    Ok(())
}
