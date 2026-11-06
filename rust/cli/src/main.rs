//! `ponk`: the command line for the ponk public API.
//!
//! Reads come straight off the public endpoints and the key-authenticated
//! `/v1` routes through the `ponk` crate; this binary only renders. Two rules
//! it inherits from the product and keeps:
//!
//! * A number the server did not send is a dash, never a zero and never a
//!   guess.
//! * `--json` prints the raw response for the same request, so a script can
//!   parse exactly what the server said rather than what a table dropped.
//!
//! Configuration is environment first, flags second:
//!
//! * `PONK_API_KEY`  the `ponk_live_...` key from Settings, API keys. Needed
//!   by `agents`, `agent <uuid>`, `create-agent`, `pause`, `resume`,
//!   `withdraw` and `whoami`. The pool and leaderboard reads need none.
//! * `PONK_API_URL`  the API base, default `https://ponk.exchange/api`.
//! * `PONK_WEB_URL`  the web origin for the two app-served boards, only when
//!   it is not the API base minus `/api` (local development).

mod table;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use table::{age, dash, pct, price, render, sol, usd, usd_str, Align, DASH};

#[derive(Parser)]
#[command(
    name = "ponk",
    version,
    about = "Ponk from the terminal: LP pools on Meteora DLMM, Meteora DAMM v2, Orca and Raydium, your agents, the leaderboard."
)]
struct Cli {
    /// API base URL.
    #[arg(long, env = "PONK_API_URL", default_value = ponk::DEFAULT_BASE_URL, global = true)]
    base_url: String,
    /// Web origin for the pool boards, when split from the API base.
    #[arg(long, env = "PONK_WEB_URL", global = true)]
    web_url: Option<String>,
    /// Print the raw JSON response instead of a table.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum BoardView {
    /// Most fees in 24h, TVL over $50K (Meteora board).
    Hot,
    /// Created in the last 24h, most fees first (Meteora board).
    New,
    /// Top volume (Meteora board).
    Volume,
    /// Top TVL (Meteora board).
    Tvl,
}

impl BoardView {
    fn as_str(self) -> &'static str {
        match self {
            Self::Hot => "hot",
            Self::New => "new",
            Self::Volume => "volume",
            Self::Tvl => "tvl",
        }
    }
}

#[derive(Subcommand)]
enum Cmd {
    /// LP pools. Default: the Discover board across Meteora DLMM, Meteora
    /// DAMM v2, Orca Whirlpools and Raydium CLMM, most 24h fees first. --view
    /// switches to one of the Meteora DLMM board's own queries.
    Pools {
        /// A Meteora board view instead of the all-venue Discover board.
        #[arg(long, value_enum)]
        view: Option<BoardView>,
        /// Filter by token symbol or address (upstream on a board view,
        /// locally on the Discover board).
        #[arg(long)]
        search: Option<String>,
        /// Rows to print.
        #[arg(long, default_value_t = 30)]
        limit: usize,
    },
    /// One pool, decoded from chain. Tries Meteora DLMM, Meteora DAMM v2,
    /// Orca, Raydium and Ponk Clouds in turn unless --dex names the venue.
    Pool {
        address: String,
        /// meteora_dlmm, meteora_damm_v2, orca, raydium or ponk_clouds.
        #[arg(long)]
        dex: Option<String>,
    },
    /// Every agent your key's wallet owns.
    Agents,
    /// One agent: your own by UUID, or any shared agent by its public slug.
    Agent { id_or_slug: String },
    /// Create an agent on your wallet. Live by default, exactly like the API:
    /// pass --dry-run for one that simulates and never sends a transaction.
    CreateAgent {
        #[arg(long)]
        name: String,
        /// meteora_dlmm, meteora_damm_v2, orca or raydium. On meteora_damm_v2
        /// (a share of the pool, no range) only monitor_only and
        /// fee_collector run.
        #[arg(long)]
        dex: String,
        /// bin_rebalancer, conservative_bin_rebalancer,
        /// volatility_aware_bin_rebalancer, adaptive_range_recovery,
        /// fee_collector, monitor_only, custom or entry_agent.
        #[arg(long)]
        strategy: String,
        /// Pool address. Required by every managing strategy; an entry_agent
        /// starts poolless and screens for one.
        #[arg(long)]
        pool: Option<String>,
        /// An existing position to adopt.
        #[arg(long)]
        position: Option<String>,
        /// Path to the strategy config JSON, or - for stdin.
        #[arg(long)]
        config: String,
        /// Simulate: evaluate and log every decision, send nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Stop the agent's loop. The position stays open and keeps earning.
    Pause { id: String },
    /// Restart the loop.
    Resume { id: String },
    /// Move SOL from the agent's wallet to your own wallet. The destination
    /// is server-locked to the wallet that owns the agent.
    Withdraw {
        id: String,
        /// Lamports to withdraw. Omit to sweep everything spendable.
        #[arg(long)]
        lamports: Option<u64>,
    },
    /// Top Ponkillionaires: every shared agent, ranked against holding.
    Leaderboard {
        /// 24h, 7d, 30d or all.
        #[arg(long)]
        window: Option<String>,
    },
    /// Confirm the key works and what it is allowed to do.
    Whoami,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("could not start the async runtime")?
        .block_on(run(cli))
}

/// Build the client. `need_key` commands refuse early with the fix, rather
/// than sending a request that can only 401.
fn client(cli: &Cli, need_key: bool) -> Result<ponk::Client> {
    let mut builder = ponk::Client::builder(&cli.base_url);
    if let Some(web) = &cli.web_url {
        builder = builder.web_url(web);
    }
    match std::env::var("PONK_API_KEY") {
        Ok(key) if !key.trim().is_empty() => builder = builder.api_key(key.trim().to_string()),
        _ if need_key => bail!(
            "PONK_API_KEY is not set. Mint a key on ponk.exchange under Settings, API keys, \
             then export PONK_API_KEY=ponk_live_..."
        ),
        _ => {}
    }
    Ok(builder.build()?)
}

async fn run(cli: Cli) -> Result<()> {
    match &cli.cmd {
        Cmd::Pools { view, search, limit } => pools(&cli, *view, search.as_deref(), *limit).await,
        Cmd::Pool { address, dex } => pool(&cli, address, dex.as_deref()).await,
        Cmd::Agents => agents(&cli).await,
        Cmd::Agent { id_or_slug } => agent(&cli, id_or_slug).await,
        Cmd::CreateAgent {
            name,
            dex,
            strategy,
            pool,
            position,
            config,
            dry_run,
        } => {
            create_agent(
                &cli,
                name,
                dex,
                strategy,
                pool.as_deref(),
                position.as_deref(),
                config,
                *dry_run,
            )
            .await
        }
        Cmd::Pause { id } => pause_resume(&cli, id, true).await,
        Cmd::Resume { id } => pause_resume(&cli, id, false).await,
        Cmd::Withdraw { id, lamports } => withdraw(&cli, id, *lamports).await,
        Cmd::Leaderboard { window } => leaderboard(&cli, window.as_deref()).await,
        Cmd::Whoami => whoami(&cli).await,
    }
}

fn print_json(value: &serde_json::Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/* ---------------------------------------------------------------------- */
/* pools                                                                   */
/* ---------------------------------------------------------------------- */

/// The board path `--json` fetches, identical to what the typed call sends.
fn board_path(view: BoardView, search: Option<&str>) -> String {
    let mut path = format!("/pools/board?view={}", view.as_str());
    if let Some(q) = search {
        path.push_str(&format!("&q={}", urlencode(q)));
    }
    path
}

fn urlencode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

fn opp_pair(p: &ponk::OppPool) -> String {
    let sym = |t: &Option<ponk::OppToken>| {
        t.as_ref()
            .and_then(|t| t.symbol.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "?".to_string())
    };
    format!("{}/{}", sym(&p.x), sym(&p.y))
}

fn opp_fees_24h(p: &ponk::OppPool) -> Option<f64> {
    p.fees.get("24h").copied().flatten()
}

fn opp_volume_24h(p: &ponk::OppPool) -> Option<f64> {
    p.volume.get("24h").copied().flatten()
}

/// Filter by symbol or address, then order by 24h fees, pools with a figure
/// first. A filter and an ordering of real rows, never a computed figure.
fn opp_select(mut pools: Vec<ponk::OppPool>, search: Option<&str>) -> Vec<ponk::OppPool> {
    if let Some(q) = search {
        let q = q.to_lowercase();
        pools.retain(|p| {
            opp_pair(p).to_lowercase().contains(&q)
                || p.address.as_deref().is_some_and(|a| a.to_lowercase().contains(&q))
        });
    }
    pools.sort_by(|a, b| match (opp_fees_24h(b), opp_fees_24h(a)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => std::cmp::Ordering::Equal,
    });
    pools
}

fn venue_label(dex: Option<&str>) -> &'static str {
    match dex {
        Some("meteora_dlmm") => "meteora",
        Some("meteora_damm_v2") => "damm v2",
        Some("orca") => "orca",
        Some("raydium") => "raydium",
        Some("ponk_clouds") => "clouds",
        _ => DASH,
    }
}

async fn pools(cli: &Cli, view: Option<BoardView>, search: Option<&str>, limit: usize) -> Result<()> {
    let client = client(cli, false)?;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    match view {
        Some(view) => {
            if cli.json {
                return print_json(&client.get_web_json(&board_path(view, search)).await?);
            }
            let board = client.hot_pools(Some(view.as_str()), search).await?;
            let rows: Vec<Vec<String>> = board
                .pools
                .iter()
                .take(limit)
                .map(|p| {
                    let sym = |t: &Option<ponk::BoardToken>| {
                        t.as_ref()
                            .and_then(|t| t.symbol.clone())
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| "?".to_string())
                    };
                    vec![
                        format!("{}/{}", sym(&p.token_x), sym(&p.token_y)),
                        dash(p.address.as_deref()),
                        p.bin_step.map(|b| b.to_string()).unwrap_or_else(|| DASH.into()),
                        pct(p.base_fee_pct, 2),
                        usd(p.tvl_usd),
                        usd(p.volume_24h_usd),
                        usd(p.fees_24h_usd),
                        age(p.created_at, now_ms),
                    ]
                })
                .collect();
            println!(
                "{}",
                render(
                    &["PAIR", "POOL", "STEP", "FEE", "TVL", "VOL 24H", "FEES 24H", "AGE"],
                    &[
                        Align::Left,
                        Align::Left,
                        Align::Right,
                        Align::Right,
                        Align::Right,
                        Align::Right,
                        Align::Right,
                        Align::Right,
                    ],
                    &rows,
                )
            );
            println!("\nMeteora DLMM board, view {}. All venues: ponk pools", view.as_str());
        }
        None => {
            if cli.json {
                return print_json(&client.get_web_json("/discover/feed").await?);
            }
            let feed = client.opportunities().await?;
            let ok = feed.ok.clone().unwrap_or_default();
            let rows: Vec<Vec<String>> = opp_select(feed.pools, search)
                .iter()
                .take(limit)
                .map(|p| {
                    vec![
                        venue_label(p.dex.as_deref()).to_string(),
                        opp_pair(p),
                        dash(p.address.as_deref()),
                        p.bin_step.map(|b| b.to_string()).unwrap_or_else(|| DASH.into()),
                        pct(p.fee_pct, 2),
                        usd(p.tvl),
                        usd(opp_volume_24h(p)),
                        usd(opp_fees_24h(p)),
                        age(p.created_at, now_ms),
                    ]
                })
                .collect();
            println!(
                "{}",
                render(
                    &["VENUE", "PAIR", "POOL", "STEP", "FEE", "TVL", "VOL 24H", "FEES 24H", "AGE"],
                    &[
                        Align::Left,
                        Align::Left,
                        Align::Left,
                        Align::Right,
                        Align::Right,
                        Align::Right,
                        Align::Right,
                        Align::Right,
                        Align::Right,
                    ],
                    &rows,
                )
            );
            // A missing source is said out loud, not padded over.
            let mut down: Vec<&str> = Vec::new();
            if ok.meteora == Some(false) {
                down.push("meteora");
            }
            if ok.orca == Some(false) {
                down.push("orca");
            }
            if ok.raydium == Some(false) {
                down.push("raydium");
            }
            if ok.damm_v2 == Some(false) {
                down.push("meteora damm v2");
            }
            if !down.is_empty() {
                println!("\n{} did not answer this read; those venues' rows are missing.", down.join(", "));
            }
        }
    }
    Ok(())
}

/* ---------------------------------------------------------------------- */
/* pool                                                                    */
/* ---------------------------------------------------------------------- */

const POOL_DEXES: [&str; 5] = ["meteora_dlmm", "meteora_damm_v2", "orca", "raydium", "ponk_clouds"];

/// A venue whose position is a share of the whole pool, with no bins: its
/// snapshot zero-fills `bin_step` and `active_bin_id`, which mean nothing
/// there and print as a dash rather than a fabricated 0.
fn has_bins(dex: &str) -> bool {
    dex != "meteora_damm_v2"
}

async fn pool(cli: &Cli, address: &str, dex: Option<&str>) -> Result<()> {
    let client = client(cli, false)?;
    let (dex, snapshot) = match dex {
        Some(d) => (d.to_string(), client.pool(d, address).await?),
        None => {
            let mut found = None;
            let mut last_err = None;
            for d in POOL_DEXES {
                match client.pool(d, address).await {
                    Ok(s) => {
                        found = Some((d.to_string(), s));
                        break;
                    }
                    Err(e) => last_err = Some(e),
                }
            }
            match found {
                Some(hit) => hit,
                None => bail!(
                    "no venue knows pool {address} (tried {}): {}",
                    POOL_DEXES.join(", "),
                    last_err.map(|e| e.to_string()).unwrap_or_else(|| "no response".into())
                ),
            }
        }
    };
    if cli.json {
        return print_json(&client.get_json(&format!("/pools/{dex}/{address}")).await?);
    }
    let line = |k: &str, v: String| println!("{k:<16} {v}");
    let bins = has_bins(&dex);
    line("dex", dex);
    line("pool", dash(snapshot.pool_address.as_deref()));
    line(
        "token x",
        format!(
            "{} (decimals {})",
            dash(snapshot.token_x_mint.as_deref()),
            snapshot.token_x_decimals.map(|d| d.to_string()).unwrap_or_else(|| DASH.into())
        ),
    );
    line(
        "token y",
        format!(
            "{} (decimals {})",
            dash(snapshot.token_y_mint.as_deref()),
            snapshot.token_y_decimals.map(|d| d.to_string()).unwrap_or_else(|| DASH.into())
        ),
    );
    line(
        "bin step",
        snapshot.bin_step.filter(|_| bins).map(|b| b.to_string()).unwrap_or_else(|| DASH.into()),
    );
    line(
        "active bin",
        snapshot.active_bin_id.filter(|_| bins).map(|b| b.to_string()).unwrap_or_else(|| DASH.into()),
    );
    line("price (y per x)", price(snapshot.human_price()));
    if snapshot.token_price.is_some() {
        line("token price", price(snapshot.token_price));
    }
    line("base fee", pct(snapshot.base_fee_pct, 2));
    line("tvl", usd_str(snapshot.tvl_usd.as_deref()));
    line("volume 24h", usd_str(snapshot.volume_24h.as_deref()));
    line("fees 24h", usd_str(snapshot.fees_24h.as_deref()));
    line("fetched", dash(snapshot.fetched_at.as_deref()));
    if let Some(w) = snapshot.mispricing_warning.as_deref() {
        line("price warning", w.to_string());
    }
    if let Some(w) = snapshot.sell_warning.as_deref() {
        line("sell warning", w.to_string());
    }
    Ok(())
}

/* ---------------------------------------------------------------------- */
/* agents                                                                  */
/* ---------------------------------------------------------------------- */

async fn agents(cli: &Cli) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        return print_json(&client.get_json("/v1/agents").await?);
    }
    let list = client.list_agents().await?;
    if list.is_empty() {
        println!("No agents on this wallet. ponk create-agent starts one.");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = list
        .iter()
        .map(|a| {
            vec![
                dash(a.name.as_deref()),
                dash(a.id.as_deref()),
                dash(a.dex.as_deref()),
                dash(a.strategy.as_deref()),
                dash(a.status.as_deref()),
                dash(a.mode.as_deref()),
                match a.dry_run {
                    Some(true) => "dry-run".to_string(),
                    Some(false) => "live".to_string(),
                    None => DASH.to_string(),
                },
                dash(a.pool_address.as_deref()),
            ]
        })
        .collect();
    println!(
        "{}",
        render(
            &["NAME", "ID", "DEX", "STRATEGY", "STATUS", "MODE", "RUN", "POOL"],
            &[
                Align::Left,
                Align::Left,
                Align::Left,
                Align::Left,
                Align::Left,
                Align::Left,
                Align::Left,
                Align::Left,
            ],
            &rows,
        )
    );
    Ok(())
}

/// A v4/v7 UUID shape: 36 characters with dashes at 8, 13, 18, 23.
fn is_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
}

async fn agent(cli: &Cli, id_or_slug: &str) -> Result<()> {
    if is_uuid(id_or_slug) {
        let client = client(cli, true)?;
        if cli.json {
            return print_json(&client.get_json(&format!("/v1/agents/{id_or_slug}")).await?);
        }
        let a = client.get_agent(id_or_slug).await?;
        let perf = client.agent_performance(id_or_slug).await?;
        let line = |k: &str, v: String| println!("{k:<18} {v}");
        line("name", dash(a.name.as_deref()));
        line("id", dash(a.id.as_deref()));
        line("dex", dash(a.dex.as_deref()));
        line("pool", dash(a.pool_address.as_deref()));
        line("strategy", dash(a.strategy.as_deref()));
        line("status", dash(a.status.as_deref()));
        line("mode", dash(a.mode.as_deref()));
        line("wallet source", dash(a.wallet_source.as_deref()));
        line(
            "run",
            match a.dry_run {
                Some(true) => "dry-run (simulating, sends nothing)".into(),
                Some(false) => "live".into(),
                None => DASH.into(),
            },
        );
        if let Some(waiting) = &a.waiting {
            let reason = waiting.get("reason").and_then(|v| v.as_str()).unwrap_or("waiting");
            line("waiting", reason.to_string());
        }
        println!();
        line("value", usd_str(perf.current_value_usd.as_deref()));
        line("cost basis", usd_str(perf.cost_basis_usd.as_deref()));
        line("pnl", usd_str(perf.pnl_usd.as_deref()));
        line("pnl %", pct(perf.pnl_pct.map(|f| f * 100.0), 2));
        line("fees earned", usd_str(perf.fees_earned_usd.as_deref()));
        line("fees unclaimed", usd_str(perf.fees_unclaimed_usd.as_deref()));
        line("fees claimed", usd_str(perf.fees_claimed_usd.as_deref()));
        line("drawdown", pct(perf.drawdown_pct.map(|f| f * 100.0), 2));
        line("pnl source", dash(perf.pnl_source.as_deref()));
        return Ok(());
    }
    // A public slug needs no key.
    let client = client(cli, false)?;
    if cli.json {
        return print_json(&client.get_json(&format!("/public/agents/{id_or_slug}")).await?);
    }
    let a = client.public_agent(id_or_slug).await?;
    let line = |k: &str, v: String| println!("{k:<18} {v}");
    line("slug", dash(a.slug.as_deref()));
    line("dex", dash(a.dex.as_deref()));
    line("pool", dash(a.pool_address.as_deref()));
    line("strategy", dash(a.strategy.as_deref()));
    line("status", dash(a.status.as_deref()));
    line("running since", dash(a.running_since.as_deref()));
    line("followers", a.followers.map(|f| f.to_string()).unwrap_or_else(|| DASH.into()));
    match (&a.performance, a.performance_unavailable.as_deref()) {
        (Some(p), _) => {
            let f = |k: &str| p.get(k).and_then(|v| v.as_f64());
            let s = |k: &str| {
                p.get(k)
                    .and_then(|v| v.as_str().map(str::to_string).or_else(|| v.as_f64().map(|n| n.to_string())))
            };
            line("pnl", usd_str(s("pnl_usd").as_deref()));
            line("pnl %", pct(f("pnl_pct"), 2));
            line("vs hodl %", pct(f("vs_hodl_pct"), 2));
            line("fees earned", usd_str(s("fees_earned_usd").as_deref()));
        }
        (None, Some(reason)) => line("performance", reason.to_string()),
        (None, None) => line("performance", DASH.into()),
    }
    Ok(())
}

/* ---------------------------------------------------------------------- */
/* create, pause, resume, withdraw                                         */
/* ---------------------------------------------------------------------- */

#[allow(clippy::too_many_arguments)]
async fn create_agent(
    cli: &Cli,
    name: &str,
    dex: &str,
    strategy: &str,
    pool: Option<&str>,
    position: Option<&str>,
    config: &str,
    dry_run: bool,
) -> Result<()> {
    let raw = if config == "-" {
        use std::io::Read;
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf).context("could not read the config from stdin")?;
        buf
    } else {
        std::fs::read_to_string(config).with_context(|| format!("could not read {config}"))?
    };
    let config: serde_json::Value =
        serde_json::from_str(&raw).context("the config is not valid JSON")?;
    let client = client(cli, true)?;
    let me = client.whoami().await?;
    let Some(wallet) = me.wallet_address else {
        bail!("the key answered without a wallet address, so no agent can be created");
    };
    let mut agent = ponk::NewAgent::new(name, wallet, dex, strategy, config);
    agent.pool_address = pool.map(str::to_string);
    agent.position_address = position.map(str::to_string);
    // /v1 defaults to LIVE; only a deliberate flag simulates.
    agent.dry_run = dry_run.then_some(true);
    let created = client.create_agent(&agent).await?;
    if cli.json {
        let id = created.id.clone().unwrap_or_default();
        return print_json(&client.get_json(&format!("/v1/agents/{id}")).await?);
    }
    println!(
        "created {} ({}) on {}: {}",
        dash(created.name.as_deref()),
        dash(created.id.as_deref()),
        dash(created.dex.as_deref()),
        match created.dry_run {
            Some(true) => "dry-run, it will simulate and send nothing",
            _ => "live",
        }
    );
    if let Some(warning) = &created.pool_warning {
        println!("pool warning: {warning}");
    }
    Ok(())
}

async fn pause_resume(cli: &Cli, id: &str, pause: bool) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        let action = if pause { "pause" } else { "resume" };
        return print_json(&client.post_json(&format!("/v1/agents/{id}/{action}"), None, None).await?);
    }
    let agent = if pause { client.pause_agent(id).await? } else { client.resume_agent(id).await? };
    println!(
        "{} is now {}",
        dash(agent.name.as_deref()),
        dash(agent.status.as_deref())
    );
    if pause {
        println!("The position stays open and keeps earning; resume with: ponk resume {id}");
    }
    Ok(())
}

async fn withdraw(cli: &Cli, id: &str, lamports: Option<u64>) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        let body = match lamports {
            Some(l) => serde_json::json!({ "lamports": l }),
            None => serde_json::json!({}),
        };
        return print_json(
            &client
                .post_json(
                    &format!("/v1/agents/{id}/withdraw"),
                    Some(&body),
                    Some(client.fund_timeout()),
                )
                .await?,
        );
    }
    let w = client.withdraw(id, lamports).await?;
    println!("withdrew {}", sol(w.lamports));
    println!("to       {}", dash(w.destination.as_deref()));
    println!("signature {}", dash(w.signature.as_deref()));
    Ok(())
}

/* ---------------------------------------------------------------------- */
/* leaderboard, whoami                                                     */
/* ---------------------------------------------------------------------- */

fn leaderboard_path(window: Option<&str>) -> String {
    match window {
        Some(w) => format!("/public/agents-leaderboard?window={}", urlencode(w)),
        None => "/public/agents-leaderboard".to_string(),
    }
}

async fn leaderboard(cli: &Cli, window: Option<&str>) -> Result<()> {
    let client = client(cli, false)?;
    if cli.json {
        return print_json(&client.get_json(&leaderboard_path(window)).await?);
    }
    let board = client.leaderboard(window).await?;
    let row = |r: &ponk::LeaderboardRow| -> Vec<String> {
        vec![
            r.rank.map(|n| n.to_string()).unwrap_or_else(|| DASH.into()),
            dash(r.slug.as_deref()),
            dash(r.dex.as_deref()),
            dash(r.strategy.as_deref()),
            pct(r.score_pct, 2),
            pct(r.pnl_pct, 2),
            pct(r.vs_hodl_pct, 2),
            usd_str(r.pnl_usd.as_deref()),
            usd_str(r.fees_earned_usd.as_deref()),
            r.followers.map(|f| f.to_string()).unwrap_or_else(|| DASH.into()),
        ]
    };
    let mut rows: Vec<Vec<String>> = board.ranked.iter().map(row).collect();
    rows.extend(board.unranked.iter().map(row));
    println!(
        "{}",
        render(
            &["RANK", "SLUG", "DEX", "STRATEGY", "SCORE", "PNL", "VS HODL", "PNL USD", "FEES", "FOLLOWERS"],
            &[
                Align::Right,
                Align::Left,
                Align::Left,
                Align::Left,
                Align::Right,
                Align::Right,
                Align::Right,
                Align::Right,
                Align::Right,
                Align::Right,
            ],
            &rows,
        )
    );
    println!(
        "\nwindow {}: {} ranked, {} unranked. Profile: ponk agent <slug>",
        dash(board.window.as_deref()),
        board.ranked.len(),
        board.unranked.len()
    );
    Ok(())
}

async fn whoami(cli: &Cli) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        return print_json(&client.get_json("/v1/me").await?);
    }
    let me = client.whoami().await?;
    println!("wallet {}", dash(me.wallet_address.as_deref()));
    println!("scope  {}", dash(me.scope.as_deref()));
    println!("key id {}", dash(me.key_id.as_deref()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_detection_separates_own_agents_from_public_slugs() {
        assert!(is_uuid("018f2c4e-9a71-7c3b-8f10-2b6a0d5e77c1"));
        assert!(!is_uuid("sunny-cloud-42"));
        assert!(!is_uuid("018f2c4e9a717c3b8f102b6a0d5e77c1"));
        assert!(!is_uuid("018f2c4e-9a71-7c3b-8f10-2b6a0d5e77cg"));
    }

    #[test]
    fn the_json_paths_match_the_typed_calls() {
        assert_eq!(board_path(BoardView::Hot, None), "/pools/board?view=hot");
        assert_eq!(
            board_path(BoardView::New, Some("SOL USDC")),
            "/pools/board?view=new&q=SOL%20USDC"
        );
        assert_eq!(leaderboard_path(None), "/public/agents-leaderboard");
        assert_eq!(leaderboard_path(Some("7d")), "/public/agents-leaderboard?window=7d");
    }

    #[test]
    fn the_cli_parses_every_documented_command() {
        for argv in [
            vec!["ponk", "pools"],
            vec!["ponk", "pools", "--view", "hot", "--search", "SOL", "--limit", "10"],
            vec!["ponk", "pool", "8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj"],
            vec!["ponk", "pool", "8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj", "--dex", "orca"],
            vec!["ponk", "pool", "8sLbNZoA1cfnvMJLPfp98ZLAnFSYCFApfJKMbiXNLwxj", "--dex", "meteora_damm_v2"],
            vec!["ponk", "agents"],
            vec!["ponk", "agent", "018f2c4e-9a71-7c3b-8f10-2b6a0d5e77c1"],
            vec!["ponk", "agent", "sunny-cloud-42"],
            vec![
                "ponk",
                "create-agent",
                "--name",
                "t",
                "--dex",
                "meteora_dlmm",
                "--strategy",
                "bin_rebalancer",
                "--pool",
                "P",
                "--config",
                "cfg.json",
                "--dry-run",
            ],
            vec![
                "ponk",
                "create-agent",
                "--name",
                "d",
                "--dex",
                "meteora_damm_v2",
                "--strategy",
                "fee_collector",
                "--pool",
                "P",
                "--config",
                "-",
            ],
            vec!["ponk", "pause", "018f2c4e-9a71-7c3b-8f10-2b6a0d5e77c1"],
            vec!["ponk", "resume", "018f2c4e-9a71-7c3b-8f10-2b6a0d5e77c1"],
            vec!["ponk", "withdraw", "018f2c4e-9a71-7c3b-8f10-2b6a0d5e77c1", "--lamports", "100"],
            vec!["ponk", "leaderboard", "--window", "7d"],
            vec!["ponk", "whoami"],
            vec!["ponk", "--json", "pools"],
        ] {
            if let Err(e) = <Cli as clap::Parser>::try_parse_from(&argv) {
                panic!("{argv:?} did not parse: {e}");
            }
        }
    }

    #[test]
    fn damm_v2_is_a_probed_venue_with_no_bins() {
        assert!(POOL_DEXES.contains(&"meteora_damm_v2"));
        assert_eq!(venue_label(Some("meteora_damm_v2")), "damm v2");
        // Its zero-filled bin step and active bin print as a dash, not 0.
        assert!(!has_bins("meteora_damm_v2"));
        assert!(has_bins("meteora_dlmm") && has_bins("orca") && has_bins("raydium"));
    }

    #[test]
    fn the_discover_rows_filter_and_order_without_inventing_figures() {
        let mk = |sym: &str, addr: &str, fees: Option<f64>| -> ponk::OppPool {
            let json = serde_json::json!({
                "key": format!("meteora_dlmm:{addr}"),
                "dex": "meteora_dlmm",
                "address": addr,
                "x": { "mint": "m", "symbol": sym },
                "y": { "mint": "EPj", "symbol": "USDC" },
                "volume": {},
                "fees": { "24h": fees },
            });
            serde_json::from_value(json).unwrap()
        };
        let rows = vec![mk("AAA", "a1", Some(5.0)), mk("BBB", "b1", None), mk("CCC", "c1", Some(9.0))];
        let picked = opp_select(rows.clone(), None);
        // Most 24h fees first; a row without the figure sinks, never zeroes.
        assert_eq!(picked[0].address.as_deref(), Some("c1"));
        assert_eq!(picked[1].address.as_deref(), Some("a1"));
        assert_eq!(picked[2].address.as_deref(), Some("b1"));
        let filtered = opp_select(rows, Some("bbb"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].address.as_deref(), Some("b1"));
    }
}
