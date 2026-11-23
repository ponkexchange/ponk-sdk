//! `ponk perpetuals ...`: Perpetuals agents on Hyperliquid.
//!
//! The markets board needs no key; everything about your own agents does.
//! Nothing here signs: an agent trades only once its owner authorizes it in
//! the app with their wallet and funds its Hyperliquid account.

use anyhow::{bail, Result};
use clap::Subcommand;
use ponk::perpetuals::PerpTrade;

use crate::table::{dash, pct, render, usd, usd_str, Align, DASH};
use crate::{client, print_json, Cli};

#[derive(Subcommand)]
pub enum PerpCmd {
    /// Every Hyperliquid Perpetuals market, by volume. No key needed.
    Markets {
        /// Keep coins starting with this text.
        #[arg(long)]
        search: Option<String>,
        /// volume, oi, change, funding or leverage.
        #[arg(long)]
        sort: Option<String>,
        #[arg(long, default_value_t = 25)]
        limit: u32,
    },
    /// Your Perpetuals agents.
    Agents,
    /// One agent: its trade, account, position, newest decision and results.
    Agent { id: String },
    /// Why an agent did or did not act, newest first.
    Decisions { id: String },
    /// An agent's orders, fills, funding, fee transfers and withdrawals.
    Ledger { id: String },
    /// The closing card of an agent that has closed.
    Card { id: String },
    /// Check a typed trade on live prices without creating anything.
    Preview(TradeArgs),
    /// Create a typed trade as an agent. Nothing is signed or funded here.
    Create(TradeArgs),
    /// One sentence: "long btc 3x 500 stop 3", "close my sol short",
    /// "tighten btc stop to 2%".
    Say {
        text: String,
        /// Required for a close, and to acknowledge the high leverage warning
        /// on an open above 10x.
        #[arg(long)]
        confirm: bool,
    },
    /// Close the position; the funds stay on the agent's account.
    Close {
        id: String,
        #[arg(long)]
        confirm: bool,
    },
    /// Move the stop closer, in percent from the price (never wider).
    Stop { id: String, pct: String },
    /// Close, settle and withdraw to the address proven in the authorization.
    Exit {
        id: String,
        /// Required: an exit closes the position and moves funds.
        #[arg(long)]
        confirm: bool,
    },
    /// Plan a fleet from one sentence. Creates nothing.
    Plan {
        text: String,
        /// The whole fleet in practice mode.
        #[arg(long)]
        practice: bool,
    },
    /// Start a plan as one fleet. Moves no money; prints the page where each
    /// agent is authorized and funded.
    StartPlan {
        id: String,
        /// Above 10x, a leg's agent name typed after the high leverage
        /// warning, as INDEX=NAME (e.g. 0="BTC long"). Once per leg above 10x.
        #[arg(long = "confirm-name", value_name = "INDEX=NAME")]
        confirm_names: Vec<String>,
    },
    /// Move every open agent's own stop in a fleet, percent from the price.
    FleetStop { fleet_id: String, pct: f64 },
    /// Pause every agent of a fleet (stops and Liquidation Guard keep running).
    FleetPause { fleet_id: String },
    /// Resume every agent of a fleet.
    FleetResume { fleet_id: String },
}

#[derive(clap::Args)]
pub struct TradeArgs {
    /// The market's coin, e.g. BTC.
    #[arg(long)]
    coin: String,
    /// long or short.
    #[arg(long)]
    side: String,
    /// Isolated leverage, 1 up to the market's own maximum. Above 10x the
    /// high leverage warning is acknowledged with --confirm-name.
    #[arg(long)]
    leverage: u32,
    /// USDC the agent trades with, at least 10.
    #[arg(long)]
    capital: String,
    /// The stop, percent from the entry (3 for 3%).
    #[arg(long)]
    stop: String,
    /// A take-profit, percent from the entry.
    #[arg(long)]
    tp: Option<String>,
    #[arg(long)]
    name: Option<String>,
    /// Live mainnet prices, a practice balance, nothing sent.
    #[arg(long)]
    practice: bool,
    /// Above 10x: the agent's name (--name, or the coin), typed after the
    /// high leverage warning, to acknowledge it.
    #[arg(long = "confirm-name", value_name = "NAME")]
    confirm_name: Option<String>,
}

impl TradeArgs {
    fn trade(&self) -> Result<PerpTrade> {
        let mut t = PerpTrade::new(&self.coin, &self.side, self.leverage, &self.capital, &self.stop).practice(self.practice);
        if let Some(tp) = &self.tp {
            t = t.take_profit_pct(tp);
        }
        if let Some(n) = &self.name {
            t = t.name(n);
        }
        if let Some(c) = &self.confirm_name {
            t = t.confirmation_text(c);
        }
        // The shape only; every gate, the market's maximum leverage included,
        // is the server's.
        t.check()?;
        Ok(t)
    }
}

pub async fn run(cli: &Cli, cmd: &PerpCmd) -> Result<()> {
    match cmd {
        PerpCmd::Markets { search, sort, limit } => markets(cli, search.as_deref(), sort.as_deref(), *limit).await,
        PerpCmd::Agents => agents(cli).await,
        PerpCmd::Agent { id } => agent(cli, id).await,
        PerpCmd::Decisions { id } => decisions(cli, id).await,
        PerpCmd::Ledger { id } => raw(cli, &format!("/v1/perp-agents/{id}/ledger")).await,
        PerpCmd::Card { id } => card(cli, id).await,
        PerpCmd::Preview(args) => preview(cli, args).await,
        PerpCmd::Create(args) => create(cli, args).await,
        PerpCmd::Say { text, confirm } => say(cli, text, *confirm).await,
        PerpCmd::Close { id, confirm } => close(cli, id, *confirm).await,
        PerpCmd::Stop { id, pct } => stop(cli, id, pct).await,
        PerpCmd::Exit { id, confirm } => exit(cli, id, *confirm).await,
        PerpCmd::Plan { text, practice } => plan(cli, text, *practice).await,
        PerpCmd::StartPlan { id, confirm_names } => start_plan(cli, id, confirm_names).await,
        PerpCmd::FleetStop { fleet_id, pct } => fleet_stop(cli, fleet_id, *pct).await,
        PerpCmd::FleetPause { fleet_id } => fleet_batch(cli, fleet_id, true).await,
        PerpCmd::FleetResume { fleet_id } => fleet_batch(cli, fleet_id, false).await,
    }
}

async fn raw(cli: &Cli, path: &str) -> Result<()> {
    print_json(&client(cli, true)?.get_json(path).await?)
}

fn num(v: Option<f64>, dp: usize) -> String {
    match v {
        Some(v) if v.is_finite() => format!("{v:.dp$}"),
        _ => DASH.to_string(),
    }
}

fn opt<T: std::fmt::Display>(v: Option<T>) -> String {
    v.map(|v| v.to_string()).unwrap_or_else(|| DASH.to_string())
}

async fn markets(cli: &Cli, search: Option<&str>, sort: Option<&str>, limit: u32) -> Result<()> {
    let client = client(cli, false)?;
    if cli.json {
        let mut q = vec![format!("limit={limit}")];
        if let Some(s) = search {
            q.push(format!("q={}", crate::urlencode(s)));
        }
        if let Some(s) = sort {
            q.push(format!("sort={}", crate::urlencode(s)));
        }
        return print_json(&client.get_json(&format!("/public/perp/markets?{}", q.join("&"))).await?);
    }
    let board = client.perp_markets(search, sort, Some(limit)).await?;
    if board.stale == Some(true) {
        println!("Stale: {}", dash(board.last_error.as_deref()));
    }
    let rows: Vec<Vec<String>> = board
        .markets
        .iter()
        .map(|m| {
            vec![
                dash(m.coin.as_deref()),
                num(m.mark_px, 4),
                pct(m.change24h_pct, 2),
                usd(m.volume24h_usd),
                usd(m.open_interest_usd),
                // Hourly funding as a percent: the rate is a fraction.
                pct(m.funding_hourly.map(|f| f * 100.0), 4),
                opt(m.max_leverage).to_string() + "x",
            ]
        })
        .collect();
    println!(
        "{}",
        render(
            &["COIN", "MARK", "24H", "VOLUME 24H", "OPEN INTEREST", "FUNDING 1H", "MAX LEV"],
            &[Align::Left, Align::Right, Align::Right, Align::Right, Align::Right, Align::Right, Align::Right],
            &rows,
        )
    );
    println!(
        "Agents run up to a market's MAX LEV. Above {}x: {}",
        ponk::perpetuals::HIGH_LEVERAGE_ABOVE,
        ponk::perpetuals::HIGH_LEVERAGE_WARNING
    );
    Ok(())
}

async fn agents(cli: &Cli) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        return print_json(&client.get_json("/v1/perp-agents").await?);
    }
    let list = client.perp_agents().await?;
    if list.is_empty() {
        println!("No Perpetuals agents on this account. ponk perpetuals create starts one.");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = list
        .iter()
        .map(|a| {
            vec![
                dash(a.name.as_deref()),
                dash(a.id.as_deref()),
                dash(a.coin.as_deref()),
                dash(a.direction.as_deref()),
                a.leverage.map(|l| format!("{l}x")).unwrap_or_else(|| DASH.into()),
                usd_str(a.capital_usd.as_deref()),
                dash(a.status.as_deref()),
                if a.booking_pending == Some(true) { DASH.into() } else { usd_str(a.realized_net_usd.as_deref()) },
                if a.practice == Some(true) { "practice".into() } else { dash(a.network.as_deref()) },
            ]
        })
        .collect();
    println!(
        "{}",
        render(
            &["NAME", "ID", "COIN", "SIDE", "LEV", "CAPITAL", "STATUS", "REALIZED NET", "NETWORK"],
            &[
                Align::Left,
                Align::Left,
                Align::Left,
                Align::Left,
                Align::Right,
                Align::Right,
                Align::Left,
                Align::Right,
                Align::Left,
            ],
            &rows,
        )
    );
    Ok(())
}

async fn agent(cli: &Cli, id: &str) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        return print_json(&client.get_json(&format!("/v1/perp-agents/{id}")).await?);
    }
    let a = client.perp_agent(id).await?;
    let line = |k: &str, v: String| println!("{k:<14} {v}");
    if let Some(c) = &a.config {
        line(
            "trade",
            format!(
                "{} {} {}x, ${}, stop {}%{}",
                dash(c.direction.as_deref()),
                dash(c.coin.as_deref()),
                opt(c.leverage),
                dash(c.capital_usd.as_deref()),
                dash(c.stop_loss_pct.as_deref()),
                if c.practice == Some(true) { ", practice" } else { "" }
            ),
        );
    }
    line("status", dash(a.status.as_deref()));
    line("account", dash(a.agent_account.as_deref()));
    match &a.account {
        Some(acct) if acct.unavailable.is_some() => line("balances", dash(acct.unavailable.as_deref())),
        Some(acct) => {
            if let Some(b) = &acct.balances {
                let why = acct.value_unavailable.as_deref();
                line("value", if why.is_some() { DASH.into() } else { usd_str(b.perps_value_usd.as_deref()) });
                line("withdrawable", if why.is_some() { DASH.into() } else { usd_str(b.withdrawable_usd.as_deref()) });
                if let Some(w) = why {
                    line("", w.to_string());
                }
            }
            match &acct.position {
                Some(p) => {
                    line("position", format!("{} {}", dash(p.side.as_deref()), dash(p.size.as_deref())));
                    line("entry", dash(p.entry_price.as_deref()));
                    line("mark", dash(p.mark_price.as_deref()));
                    line("unrealized", usd_str(p.unrealized_pnl_usd.as_deref()));
                    line("liquidation", num(p.liquidation_price, 4));
                    line("stop", dash(p.stop_price.as_deref()));
                    line("margin usage", pct(p.margin_usage.map(|u| u * 100.0), 1));
                }
                None if acct.booking_pending == Some(true) => line("position", "Closed, settling".into()),
                None => line("position", "none open".into()),
            }
            if acct.stale == Some(true) {
                line("", "This read is stale.".into());
            }
        }
        None => {}
    }
    match &a.realized {
        Some(r) => line("realized net", format!("{} (high-water {})", usd_str(r.net_usd.as_deref()), usd_str(r.high_water_usd.as_deref()))),
        None => line("realized net", format!("{DASH} (nothing has closed yet)")),
    }
    match &a.decision {
        Some(d) => line("now", d.sentences.join(" ")),
        None => line("now", "No decision yet: the agent has not ticked.".into()),
    }
    if let Some(e) = &a.exit {
        line("exit", format!("{} {}", dash(e.status.as_deref()), usd_str(e.amount_usd.as_deref())));
    }
    Ok(())
}

async fn decisions(cli: &Cli, id: &str) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        return print_json(&client.get_json(&format!("/v1/perp-agents/{id}/decisions")).await?);
    }
    let rows = client.perp_agent_decisions(id).await?;
    if rows.is_empty() {
        println!("No decisions yet: the agent has not ticked.");
    }
    for d in rows.iter().take(30) {
        println!("{}  {}", dash(d.at.as_deref()), d.sentences.join(" "));
    }
    Ok(())
}

async fn card(cli: &Cli, id: &str) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        return print_json(&client.get_json(&format!("/v1/perp-agents/{id}/exit-card")).await?);
    }
    let c = client.perp_agent_exit_card(id).await?;
    println!("{} {} {}x", dash(c.side.as_deref()), dash(c.market.as_deref()), opt(c.leverage));
    match (c.pnl_usd, c.pnl_unavailable.as_deref()) {
        (Some(p), _) => println!("net {} ({})", usd(Some(p)), pct(c.pnl_pct, 2)),
        (None, why) => println!("net {DASH} {}", dash(why)),
    }
    if c.liquidated == Some(true) {
        println!("liquidated");
    }
    if let Some(n) = &c.note {
        println!("{n}");
    }
    Ok(())
}

fn print_ticket(t: &ponk::PerpTicket) {
    println!("{}", dash(t.sentence.as_deref()));
    println!("size          {} {}", num(t.size, 6), dash(t.coin.as_deref()));
    println!("notional      {}", usd(t.notional_usd));
    println!("liquidation   {}", num(t.liquidation_price, 4));
    println!("stop          {}", num(t.stop_price, 4));
    println!("loss at stop  {}", usd(t.loss_at_stop_usd));
}

/// What the confirm step warns about, printed before the result.
fn print_warnings(warnings: &[String]) {
    for w in warnings {
        println!("Warning: {w}");
    }
    if !warnings.is_empty() {
        println!();
    }
}

/// Above 10x the leverage needs the typed name; say how, in one line.
fn confirm_hint(t: &PerpTrade) -> Option<String> {
    if !ponk::perpetuals::needs_confirmation(t.leverage) || t.confirmation_text.is_some() {
        return None;
    }
    let name = t.name.clone().unwrap_or_else(|| t.coin.trim().to_ascii_uppercase());
    Some(format!(
        "{} To go ahead at {}x, type the agent's name: --confirm-name \"{name}\"",
        ponk::perpetuals::HIGH_LEVERAGE_WARNING,
        t.leverage
    ))
}

fn print_fees(f: Option<&ponk::PerpFeeRates>) {
    if let Some(f) = f {
        let bps = |b: Option<i64>| b.map(|b| format!("{}%", b as f64 / 100.0)).unwrap_or_else(|| DASH.into());
        println!(
            "The authorization will state: {} when the first trade fills, {} of new realized profit, {} when you withdraw (only if it traded).",
            bps(f.activation_bps),
            bps(f.performance_bps),
            bps(f.exit_bps)
        );
    }
}

async fn preview(cli: &Cli, args: &TradeArgs) -> Result<()> {
    let t = args.trade()?;
    let client = client(cli, true)?;
    if cli.json {
        return print_json(&client.post_json("/v1/perp-agents/preview", Some(&serde_json::to_value(&t)?), None).await?);
    }
    let p = client.preview_perp_agent(&t).await?;
    if p.ok != Some(true) {
        bail!("{}", dash(p.sentence.as_deref()));
    }
    print_warnings(&p.warnings);
    if let Some(ticket) = &p.ticket {
        print_ticket(ticket);
    }
    print_fees(p.fees.as_ref());
    println!("\nNothing was created. Create it with ponk perpetuals create and the same flags.");
    if p.needs_confirmation == Some(true) {
        if let Some(hint) = confirm_hint(&t) {
            println!("{hint}");
        }
    }
    Ok(())
}

async fn create(cli: &Cli, args: &TradeArgs) -> Result<()> {
    let t = args.trade()?;
    // Above 10x without the typed name the server refuses with the warning;
    // say it here, with the flag, before sending anything.
    if let Some(hint) = confirm_hint(&t) {
        bail!("{hint}");
    }
    let client = client(cli, true)?;
    if cli.json {
        return print_json(&client.post_json("/v1/perp-agents", Some(&serde_json::to_value(&t)?), None).await?);
    }
    let a = client.create_perp_agent(&t).await?;
    print_warnings(&a.warnings);
    match &a.ticket_figures {
        Some(ticket) => print_ticket(ticket),
        None => println!("{}", dash(a.ticket.as_deref())),
    }
    print_fees(a.fees.as_ref());
    println!("agent         {}", dash(a.id.as_deref()));
    println!("{}", dash(a.next.as_deref()));
    Ok(())
}

async fn say(cli: &Cli, text: &str, confirm: bool) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        let body = serde_json::json!({ "text": text, "confirm": confirm });
        return print_json(&client.post_json("/v1/perp-agents/say", Some(&body), None).await?);
    }
    let s = client.perp_say(text, confirm).await?;
    if s.created() {
        print_warnings(&s.warnings);
        println!("{}", dash(s.ticket.as_deref()));
        print_fees(s.fees.as_ref());
        println!("agent         {}", dash(s.id.as_deref()));
        println!("{}", dash(s.next.as_deref()));
    } else {
        println!("{}", dash(s.sentence.as_deref()));
    }
    Ok(())
}

async fn close(cli: &Cli, id: &str, confirm: bool) -> Result<()> {
    if !confirm {
        bail!("a close closes the position. Run it again with --confirm: ponk perpetuals close {id} --confirm");
    }
    let client = client(cli, true)?;
    if cli.json {
        let body = serde_json::json!({ "confirm": true });
        return print_json(&client.post_json(&format!("/v1/perp-agents/{id}/close"), Some(&body), None).await?);
    }
    println!("{}", dash(client.close_perp_agent(id, true).await?.sentence.as_deref()));
    Ok(())
}

async fn stop(cli: &Cli, id: &str, pct: &str) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        let body = serde_json::json!({ "stopPct": pct });
        return print_json(&client.post_json(&format!("/v1/perp-agents/{id}/stop"), Some(&body), None).await?);
    }
    println!("{}", dash(client.tighten_perp_stop(id, pct).await?.sentence.as_deref()));
    Ok(())
}

async fn exit(cli: &Cli, id: &str, confirm: bool) -> Result<()> {
    if !confirm {
        bail!(
            "an exit closes the position, settles and withdraws to the address proven in the agent's \
             authorization. Run it again with --confirm: ponk perpetuals exit {id} --confirm"
        );
    }
    let client = client(cli, true)?;
    if cli.json {
        let body = serde_json::json!({ "confirm": true });
        return print_json(
            &client
                .post_json(&format!("/v1/perp-agents/{id}/exit"), Some(&body), Some(client.fund_timeout()))
                .await?,
        );
    }
    let e = client.exit_perp_agent(id, true).await?;
    println!("{}", dash(e.sentence.as_deref()));
    println!("status        {}", dash(e.status.as_deref()));
    println!("to            {}", dash(e.destination.as_deref()));
    Ok(())
}

fn print_plan(p: &ponk::PerpFleetPlan) {
    if let Some(r) = &p.refusal {
        println!("Refused: {}", r.get("message").and_then(|m| m.as_str()).unwrap_or(DASH));
        return;
    }
    println!("{}", dash(p.echo.as_deref()));
    // What each leg's confirm step warns about, before the plan itself.
    for l in p.legs.iter().filter(|l| l.refusal.is_none()) {
        if let Some(n) = l.note.as_deref().filter(|n| !n.is_empty()) {
            println!("Warning, {}: {n}", dash(l.name.as_deref()));
        }
    }
    let rows: Vec<Vec<String>> = p
        .legs
        .iter()
        .map(|l| {
            vec![
                dash(l.coin.as_deref()),
                dash(l.direction.as_deref()),
                l.leverage.map(|v| format!("{v}x")).unwrap_or_else(|| DASH.into()),
                usd_str(l.capital_usd.as_deref()),
                pct(l.stop_pct, 1),
                match (&l.refusal, l.needs_confirmation) {
                    (Some(r), _) => r.clone(),
                    (None, Some(true)) => "ready, type the name".into(),
                    (None, _) => "ready".into(),
                },
            ]
        })
        .collect();
    println!(
        "{}",
        render(
            &["COIN", "SIDE", "LEV", "CAPITAL", "STOP", "STATE"],
            &[Align::Left, Align::Left, Align::Right, Align::Right, Align::Right, Align::Left],
            &rows,
        )
    );
    if let Some(t) = &p.totals {
        println!("{} agents, loss at every stop together {}", opt(t.agents), usd(t.loss_at_stops_usd));
    }
    for n in &p.notes {
        println!("{n}");
    }
    if let Some(u) = &p.sign_url {
        println!("Authorize and fund: {u}");
    }
}

async fn plan(cli: &Cli, text: &str, practice: bool) -> Result<()> {
    let client = client(cli, false)?;
    if cli.json {
        let body = serde_json::json!({ "text": text, "practice": practice });
        return print_json(&client.post_json("/copilot/perp/plans", Some(&body), None).await?);
    }
    let p = client.copilot_perp_plan(text, practice).await?;
    print_plan(&p);
    if p.can_start == Some(true) {
        if let Some(id) = &p.id {
            let typed: Vec<String> = p
                .legs
                .iter()
                .filter(|l| l.refusal.is_none() && l.needs_confirmation == Some(true))
                .map(|l| format!(" --confirm-name {}=\"{}\"", opt(l.index), dash(l.name.as_deref())))
                .collect();
            if !typed.is_empty() {
                println!("\n{}", ponk::perpetuals::HIGH_LEVERAGE_WARNING);
                println!("Above 10x, type each agent's name to confirm.");
            }
            println!("\nNothing was created. Start it with: ponk perpetuals start-plan {id}{}", typed.concat());
        }
    }
    Ok(())
}

/// `--confirm-name 0="BTC long"` pairs, leg index to the typed name.
fn confirmations(pairs: &[String]) -> Result<std::collections::BTreeMap<usize, String>> {
    let mut out = std::collections::BTreeMap::new();
    for p in pairs {
        let Some((i, name)) = p.split_once('=') else {
            bail!("--confirm-name takes INDEX=NAME, like 0=\"BTC long\", got {p:?}");
        };
        let Ok(i) = i.trim().parse::<usize>() else {
            bail!("--confirm-name takes a leg index before the =, like 0=\"BTC long\", got {p:?}");
        };
        out.insert(i, name.to_string());
    }
    Ok(out)
}

async fn start_plan(cli: &Cli, id: &str, confirm_names: &[String]) -> Result<()> {
    let names = confirmations(confirm_names)?;
    let client = client(cli, true)?;
    if cli.json {
        let body = (!names.is_empty()).then(|| {
            let by_leg: std::collections::BTreeMap<String, &String> = names.iter().map(|(i, n)| (i.to_string(), n)).collect();
            serde_json::json!({ "confirmations": by_leg })
        });
        return print_json(
            &client
                .post_json(&format!("/copilot/perp/plans/{id}/start"), body.as_ref(), Some(client.fund_timeout()))
                .await?,
        );
    }
    print_plan(&client.copilot_perp_start_confirmed(id, &names).await?);
    Ok(())
}

async fn fleet_stop(cli: &Cli, fleet_id: &str, pct: f64) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        let body = serde_json::json!({ "stop_pct": pct });
        return print_json(&client.post_json(&format!("/copilot/fleets/{fleet_id}/stop-rule"), Some(&body), None).await?);
    }
    let s = client.copilot_tighten_agent_stops(fleet_id, pct).await?;
    for a in &s.answers {
        println!("{:<20} {}", dash(a.name.as_deref()), dash(a.sentence.as_deref()));
    }
    Ok(())
}

async fn fleet_batch(cli: &Cli, fleet_id: &str, pause: bool) -> Result<()> {
    let client = client(cli, true)?;
    let action = if pause { "pause" } else { "resume" };
    if cli.json {
        return print_json(&client.post_json(&format!("/copilot/fleets/{fleet_id}/{action}"), None, None).await?);
    }
    let b = if pause { client.copilot_pause_fleet(fleet_id).await? } else { client.copilot_resume_fleet(fleet_id).await? };
    println!("{action}: {} done, {} failed of {}", b.done, b.failed, b.total);
    for i in b.items.iter().filter(|i| i.ok != Some(true)) {
        println!("  {} {}", dash(i.agent_id.as_deref()), dash(i.error.as_deref()));
    }
    Ok(())
}
