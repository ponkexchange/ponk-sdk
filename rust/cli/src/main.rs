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
//!   by `agents`, `agent <uuid>`, `create-agent`, `create-evm-agent`,
//!   `wallet`, `pause`, `resume`, `withdraw`, `exit`, `open-cost`, `plan`,
//!   `start-plan`, `evm-fleet`, `feed-prefs` and `whoami`. The pool, chain
//!   and leaderboard reads and `fund-link` need none.
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
    about = "Ponk from the terminal: LP pools and agents on Solana (Meteora DLMM, Meteora DAMM v2, Orca, Raydium) and on Ethereum, Arbitrum, Base, BNB Chain and Robinhood Chain (Uniswap, PancakeSwap, Aerodrome), the leaderboard."
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
enum FeedPrefsCmd {
    /// Save the filter: a comma list of chain ids, or all.
    Set { chains: String },
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
        /// Only agents on these chains: a comma list of solana, ethereum,
        /// arbitrum, base, bsc (or bnb), robinhood, or all. Ranks are
        /// numbered again from 1 over the rows kept.
        #[arg(long)]
        chains: Option<String>,
    },
    /// Your saved chain filter: the chains every signed-in board and tracker
    /// read covers when it names none. `ponk feed-prefs set bsc,solana` (or
    /// `all`) saves it, with a trade key.
    FeedPrefs {
        #[command(subcommand)]
        action: Option<FeedPrefsCmd>,
    },
    /// Confirm the key works and what it is allowed to do.
    Whoami,
    /// Every chain ponk runs on, its gas coin and its venues.
    Chains,
    /// LP pools on an EVM chain, most 24h LP fees first.
    EvmPools {
        /// ethereum, arbitrum, base, bsc or robinhood.
        chain: String,
        /// One venue: uniswap_v2, uniswap_v3, uniswap_v4, pancakeswap_v2,
        /// pancakeswap_v3, pancakeswap_infinity_cl, aerodrome_slipstream or
        /// aerodrome_classic.
        #[arg(long)]
        venue: Option<String>,
        /// Filter by token symbol or pool id.
        #[arg(long)]
        search: Option<String>,
        /// Rows to print.
        #[arg(long, default_value_t = 25)]
        limit: usize,
    },
    /// One EVM pool by its id (eip155:<chain id>:0x...).
    EvmPool { id: String },
    /// What the first open of an agent costs on an EVM venue: gas, the
    /// reserve kept for the exit, and the fee rates your key's agents pay.
    OpenCost {
        chain: String,
        /// The venue id.
        #[arg(long)]
        dex: String,
        /// The pool id, to check it against the venue and chain.
        #[arg(long)]
        pool: Option<String>,
        /// Quote an open that does not stake for farm rewards.
        #[arg(long)]
        no_stake: bool,
        /// Quote a liquidity shape: spot, curve, bid_ask or spot_bid_ask.
        /// Adds how many ranges the agent would open now and the gas of
        /// opening and later closing them, where shapes run.
        #[arg(long)]
        shape: Option<String>,
        /// With --shape: the range count asked, auto or 1 to 7.
        #[arg(long)]
        ranges: Option<String>,
        /// With --shape: the deposit in dollars.
        #[arg(long)]
        amount_usd: Option<f64>,
        /// With --shape: percent either side of the price, 1 to 50.
        #[arg(long)]
        range_pct: Option<f64>,
        /// With --shape: below or above, for a one-sided band.
        #[arg(long)]
        side: Option<String>,
    },
    /// Create an agent on an EVM chain, then print its deposit address and
    /// the link where your own wallets sign its mandate. Live by default.
    CreateEvmAgent {
        #[arg(long)]
        name: String,
        /// ethereum, arbitrum, base, bsc or robinhood.
        #[arg(long)]
        chain: String,
        /// The venue id.
        #[arg(long)]
        dex: String,
        /// The pool id (eip155:<chain id>:0x...), from `ponk evm-pools`.
        #[arg(long)]
        pool: String,
        /// How far the range reaches below the price, as a fraction: 0.05 is
        /// 5%. Range venues only. 0 on one side is a one-sided range.
        #[arg(long)]
        down_pct: Option<f64>,
        /// How far the range reaches above the price, as a fraction.
        #[arg(long)]
        up_pct: Option<f64>,
        /// The liquidity shape: spot (one even range, the default), curve,
        /// bid_ask or spot_bid_ask. Several ranges managed as one position on
        /// Uniswap v3 (Arbitrum, Ethereum, Robinhood Chain), Uniswap v4
        /// (Arbitrum, Ethereum), PancakeSwap v3 and Infinity CL, Aerodrome
        /// Slipstream; one range on any other range venue.
        #[arg(long)]
        shape: Option<String>,
        /// How many ranges the shape opens as: auto (from live gas, within 3%
        /// of the deposit) or 1 to 7.
        #[arg(long)]
        shape_ranges: Option<String>,
        /// Your own strategy id, with --config.
        #[arg(long)]
        strategy: Option<String>,
        /// Path to your own strategy config JSON, or - for stdin, instead of
        /// --down-pct and --up-pct.
        #[arg(long)]
        config: Option<String>,
        /// Stake the position for farm rewards: CAKE on pancakeswap_v3, AERO
        /// on Aerodrome (there, AERO instead of swap fees).
        #[arg(long)]
        stake: bool,
        /// The EVM address you intend withdrawals to go to, EIP-55 mixed case.
        #[arg(long)]
        withdrawal_address: Option<String>,
        /// Simulate: evaluate and log every decision, send nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// An agent's own wallet: its deposit address, balance, what the first
    /// open needs, and its mandate status.
    Wallet { id: String },
    /// The ponk.exchange link where your own wallets fund agents and sign
    /// for them. Signs nothing and needs no key.
    FundLink {
        /// One or more agent ids.
        #[arg(required = true)]
        ids: Vec<String>,
        /// The agents' chain. An agent on an EVM chain is funded in that
        /// chain's own coin, so its link carries no amount.
        #[arg(long)]
        chain: Option<String>,
        /// Lamports to send to each Solana agent. Omit to sign only.
        #[arg(long)]
        lamports: Option<u64>,
    },
    /// Every EVM agent you own and what each still needs from you.
    EvmFleet,
    /// Plan one sentence that names an EVM chain, in dollars:
    /// ponk plan "$200 in the top 5 bnb pools". Creates nothing.
    Plan {
        text: String,
        /// A liquidity shape for every leg: spot, curve or bid_ask.
        #[arg(long)]
        shape: Option<String>,
        /// A one-sided range: below or above the price.
        #[arg(long)]
        one_sided: Option<String>,
    },
    /// Start a plan: one agent per pool. Moves no money; prints the link
    /// where your own wallets sign and fund.
    StartPlan { id: String },
    /// Stop the agent, close its position and send everything home. On an
    /// EVM chain that is the mandate's withdrawal address.
    Exit {
        id: String,
        /// Required: an exit closes the position.
        #[arg(long)]
        confirm: bool,
    },
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
        Cmd::Leaderboard { window, chains } => leaderboard(&cli, window.as_deref(), chains.as_deref()).await,
        Cmd::FeedPrefs { action } => feed_prefs(&cli, action.as_ref()).await,
        Cmd::Whoami => whoami(&cli).await,
        Cmd::Chains => chains(&cli).await,
        Cmd::EvmPools { chain, venue, search, limit } => {
            evm_pools(&cli, chain, venue.as_deref(), search.as_deref(), *limit).await
        }
        Cmd::EvmPool { id } => evm_pool(&cli, id).await,
        Cmd::OpenCost { chain, dex, pool, no_stake, shape, ranges, amount_usd, range_pct, side } => {
            let q = ponk::OpenCostQuery {
                pool: pool.clone(),
                stake: no_stake.then_some(false),
                shape: shape.clone(),
                ranges: ranges.clone(),
                amount_usd: *amount_usd,
                range_pct: *range_pct,
                side: side.clone(),
                ..ponk::OpenCostQuery::new(dex.as_str())
            };
            open_cost(&cli, chain, &q).await
        }
        Cmd::CreateEvmAgent {
            name,
            chain,
            dex,
            pool,
            down_pct,
            up_pct,
            shape,
            shape_ranges,
            strategy,
            config,
            stake,
            withdrawal_address,
            dry_run,
        } => {
            create_evm_agent(
                &cli,
                EvmCreate {
                    name,
                    chain,
                    dex,
                    pool,
                    down_pct: *down_pct,
                    up_pct: *up_pct,
                    shape: shape.as_deref(),
                    shape_ranges: shape_ranges.as_deref(),
                    strategy: strategy.as_deref(),
                    config: config.as_deref(),
                    stake: *stake,
                    withdrawal_address: withdrawal_address.as_deref(),
                    dry_run: *dry_run,
                },
            )
            .await
        }
        Cmd::Wallet { id } => wallet(&cli, id).await,
        Cmd::FundLink { ids, chain, lamports } => fund_link(ids, chain.as_deref(), *lamports),
        Cmd::EvmFleet => evm_fleet(&cli).await,
        Cmd::Plan { text, shape, one_sided } => plan(&cli, text, shape.as_deref(), one_sided.as_deref()).await,
        Cmd::StartPlan { id } => start_plan(&cli, id).await,
        Cmd::Exit { id, confirm } => exit(&cli, id, *confirm).await,
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
        Some("uniswap_v2") => "uniswap v2",
        Some("uniswap_v3") => "uniswap v3",
        Some("uniswap_v4") => "uniswap v4",
        Some("pancakeswap_v2") => "pancake v2",
        Some("pancakeswap_v3") => "pancake v3",
        Some("pancakeswap_infinity_cl") => "pancake infinity",
        Some("aerodrome_slipstream") => "slipstream",
        Some("aerodrome_classic") => "aerodrome",
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
                dash(a.chain.as_deref()),
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
            &["NAME", "ID", "CHAIN", "DEX", "STRATEGY", "STATUS", "MODE", "RUN", "POOL"],
            &[
                Align::Left,
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
        line("chain", dash(a.chain.as_deref()));
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
        if let Some(farm) = &perf.farm_rewards {
            let token = farm.token.as_deref().unwrap_or("reward");
            line(
                "farm pending",
                format!("{} {token} ({})", dash(farm.pending_amount.as_deref()), usd_str(farm.pending_usd.as_deref())),
            );
            line("farm harvested", usd_str(farm.harvested_usd.as_deref()));
            line(
                "farm apr",
                match farm.apr_basis.as_deref() {
                    Some("position") => format!("{} (this position now)", pct(farm.apr_pct, 2)),
                    Some(_) => format!("{} (the pool's rate)", pct(farm.apr_pct, 2)),
                    None => pct(farm.apr_pct, 2),
                },
            );
        }
        if let Some(why) = &perf.figures_unavailable {
            line("figures", why.clone());
        }
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
    if w.chain.is_some() {
        print_evm_move(&w);
        return Ok(());
    }
    println!("withdrew {}", sol(w.lamports));
    println!("to       {}", dash(w.destination.as_deref()));
    println!("signature {}", dash(w.signature.as_deref()));
    Ok(())
}

/// An EVM withdraw or exit, as the server stated it: its sentence, what this
/// call sent, and every transaction hash.
fn print_evm_move(w: &ponk::Withdrawal) {
    let chain = w.chain.as_deref().unwrap_or("");
    let coin = ponk::evm::native_symbol(chain).unwrap_or("");
    println!("{}: {}", chain, dash(w.status.as_deref()));
    if let Some(sentence) = &w.sentence {
        println!("{sentence}");
    }
    println!("to       {}", dash(w.destination.as_deref()));
    if let Some(native) = w.amount_wei.as_deref().and_then(ponk::evm::wei_to_native) {
        println!("sent     {native} {coin}");
    }
    for r in &w.received {
        let token = r.token.as_deref().unwrap_or(DASH);
        if ponk::evm::parse_evm_id(token).is_ok_and(|id| id.is_native()) {
            continue;
        }
        println!("token    {} base units of {token}", dash(r.amount.as_deref()));
    }
    if w.tx_hashes.is_empty() {
        println!("No transaction was sent.");
    }
    for hash in &w.tx_hashes {
        println!("tx       {hash}");
    }
}

async fn exit(cli: &Cli, id: &str, confirm: bool) -> Result<()> {
    if !confirm {
        bail!(
            "an exit stops the agent, closes its position and sends everything home. \
             Run it again with --confirm: ponk exit {id} --confirm"
        );
    }
    let client = client(cli, true)?;
    if cli.json {
        return print_json(
            &client
                .post_json(&format!("/v1/agents/{id}/exit"), None, Some(client.fund_timeout()))
                .await?,
        );
    }
    let w = client.exit_agent(id, None).await?;
    if w.chain.is_some() {
        print_evm_move(&w);
        return Ok(());
    }
    println!("exited   {}", sol(w.lamports));
    println!("to       {}", dash(w.destination.as_deref()));
    println!("signature {}", dash(w.signature.as_deref()));
    Ok(())
}

/* ---------------------------------------------------------------------- */
/* EVM chains: pools, open cost, create, wallet, fund link, plans          */
/* ---------------------------------------------------------------------- */

fn evm_board_path(chain: &str, venue: Option<&str>) -> String {
    match venue {
        Some(v) => format!("/public/evm/{}/board?venue={}", urlencode(chain), urlencode(v)),
        None => format!("/public/evm/{}/board", urlencode(chain)),
    }
}

fn evm_fees_24h(p: &ponk::EvmPool) -> Option<f64> {
    p.fees.as_ref().and_then(|f| f.h24)
}

/// Filter by symbol or id, then order by 24h LP fees, pools with a figure
/// first. A filter and an ordering of real rows, never a computed figure.
fn evm_select(mut pools: Vec<ponk::EvmPool>, search: Option<&str>) -> Vec<ponk::EvmPool> {
    if let Some(q) = search {
        let q = q.to_lowercase();
        pools.retain(|p| {
            p.pair().to_lowercase().contains(&q)
                || p.address.as_deref().is_some_and(|a| a.to_lowercase().contains(&q))
        });
    }
    pools.sort_by(|a, b| match (evm_fees_24h(b), evm_fees_24h(a)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => std::cmp::Ordering::Equal,
    });
    pools
}

fn fee_tier(p: &ponk::EvmPool) -> String {
    match (p.dynamic_fee, p.fee_pct) {
        (Some(true), _) => "dynamic".to_string(),
        (_, Some(f)) => format!("{f}%"),
        _ => DASH.to_string(),
    }
}

fn farm_text(p: &ponk::EvmPool) -> String {
    match &p.farm {
        None => DASH.to_string(),
        Some(f) => {
            let token = f.token.as_deref().unwrap_or("farm");
            match (f.live, f.apr_pct) {
                (Some(true), Some(apr)) => format!("{token} {apr:.2}%"),
                (Some(true), None) => format!("{token} live"),
                _ => format!("{token} not live"),
            }
        }
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Wei as whole coins with the symbol; `-` when the server sent none.
fn native(wei: Option<&str>, symbol: &str) -> String {
    match wei.and_then(ponk::evm::wei_to_native) {
        Some(n) => format!("{n} {symbol}"),
        None => DASH.to_string(),
    }
}

async fn chains(cli: &Cli) -> Result<()> {
    let client = client(cli, false)?;
    if cli.json {
        return print_json(&client.get_json("/public/evm/chains").await?);
    }
    let chains = client.evm_chains().await?;
    let rows: Vec<Vec<String>> = chains
        .chains
        .iter()
        .map(|c| {
            vec![
                dash(c.id.as_deref()),
                dash(c.label.as_deref()),
                c.evm_chain_id.map(|n| n.to_string()).unwrap_or_else(|| DASH.into()),
                dash(c.native_symbol.as_deref()),
                c.venues.join(", "),
            ]
        })
        .collect();
    println!(
        "{}",
        render(
            &["CHAIN", "NAME", "CHAIN ID", "COIN", "VENUES"],
            &[Align::Left, Align::Left, Align::Right, Align::Left, Align::Left],
            &rows,
        )
    );
    Ok(())
}

async fn evm_pools(cli: &Cli, chain: &str, venue: Option<&str>, search: Option<&str>, limit: usize) -> Result<()> {
    let client = client(cli, false)?;
    if cli.json {
        return print_json(&client.get_json(&evm_board_path(chain, venue)).await?);
    }
    let board = client.evm_pools(chain, venue).await?;
    if !board.ready() {
        let why = board.status.as_ref().and_then(|s| s.last_error.clone());
        println!("No pools yet for {chain}: {}", why.unwrap_or_else(|| "the board has not been read".into()));
        return Ok(());
    }
    let now = now_ms();
    let picked = evm_select(board.pools, search);
    let rows: Vec<Vec<String>> = picked
        .iter()
        .take(limit)
        .map(|p| {
            let flags = p.flags(now);
            vec![
                p.pair(),
                venue_label(p.dex.as_deref()).to_string(),
                fee_tier(p),
                usd(p.tvl),
                usd(p.volume.as_ref().and_then(|v| v.h24)),
                usd(evm_fees_24h(p)),
                farm_text(p),
                if flags.is_empty() { DASH.to_string() } else { flags.join(" ") },
                dash(p.address.as_deref()),
            ]
        })
        .collect();
    println!(
        "{}",
        render(
            &["PAIR", "VENUE", "FEE", "TVL", "VOL 24H", "FEES 24H", "FARM APR", "FLAGS", "POOL ID"],
            &[
                Align::Left,
                Align::Left,
                Align::Right,
                Align::Right,
                Align::Right,
                Align::Right,
                Align::Left,
                Align::Left,
                Align::Left,
            ],
            &rows,
        )
    );
    println!(
        "\n{} of {} pools on {chain}. FARM APR on a concentrated pool is the rate of a staked position \
         spread like the whole pool. Detail: ponk evm-pool <POOL ID>",
        rows.len(),
        picked.len()
    );
    Ok(())
}

async fn evm_pool(cli: &Cli, id: &str) -> Result<()> {
    let client = client(cli, false)?;
    if cli.json {
        return print_json(&client.get_json(&format!("/public/evm/pools/{}", urlencode(id))).await?);
    }
    let p = client.evm_pool(id).await?;
    let line = |k: &str, v: String| println!("{k:<14} {v}");
    line("pair", p.pair());
    line("pool id", dash(p.address.as_deref()));
    line("chain", dash(p.chain.as_deref()));
    line("venue", dash(p.dex.as_deref()));
    line("fee tier", fee_tier(&p));
    line("tick spacing", p.bin_step.map(|b| b.to_string()).unwrap_or_else(|| DASH.into()));
    line("price", price(p.price));
    line("tvl", usd(p.tvl));
    line("volume 24h", usd(p.volume.as_ref().and_then(|v| v.h24)));
    line("lp fees 24h", usd(evm_fees_24h(&p)));
    line("farm", farm_text(&p));
    let flags = p.flags(now_ms());
    line("flags", if flags.is_empty() { DASH.to_string() } else { flags.join(" ") });
    for why in &p.data_insufficient {
        line("not read", why.clone());
    }
    Ok(())
}

fn open_cost_path(chain: &str, q: &ponk::OpenCostQuery) -> String {
    let mut path = format!("/v1/evm/{}/open-cost?dex={}", urlencode(chain), urlencode(&q.dex));
    let mut add = |k: &str, v: Option<String>| {
        if let Some(v) = v {
            path.push_str(&format!("&{k}={}", urlencode(&v)));
        }
    };
    add("pool", q.pool.clone());
    add("stake", q.stake.map(|s| s.to_string()));
    add("shape", q.shape.clone());
    add("ranges", q.ranges.clone());
    add("amount_usd", q.amount_usd.map(|v| v.to_string()));
    add("range_pct", q.range_pct.map(|v| v.to_string()));
    add("side", q.side.clone());
    path
}

fn bps(v: Option<u16>) -> String {
    match v {
        Some(b) => format!("{}%", f64::from(b) / 100.0),
        None => DASH.to_string(),
    }
}

fn print_open_cost(c: &ponk::EvmOpenCost) {
    let coin = c.native_symbol.as_deref().unwrap_or("");
    let line = |k: &str, v: String| println!("{k:<18} {v}");
    line("gas, first open", format!("{} ({})", native(c.gas_wei.as_deref(), coin), usd_str(c.gas_usd.as_deref())));
    line("kept for the exit", native(c.reserve_wei.as_deref(), coin));
    line("least to hold", format!("{}, plus the amount to deploy", native(c.min_open_wei.as_deref(), coin)));
    line("coin price", usd_str(c.native_usd.as_deref()));
    line("ponk fee", format!(
        "{} of the first open, once; {} of fees earned",
        bps(c.activation_fee_bps),
        bps(c.performance_fee_bps)
    ));
    line("exit fee", match c.exit_fee_bps {
        Some(b) if b > 0 => format!("{} of what an agent that opened a position sends you (half with 1M PONK)", bps(Some(b))),
        Some(_) => "none".to_string(),
        None => DASH.to_string(),
    });
    if let Some(shape) = &c.shape {
        let s = |k: &str| shape.get(k).and_then(|v| v.as_str()).map(str::to_string);
        let n = |k: &str| shape.get(k).and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or_else(|| DASH.into());
        line("shape", format!(
            "{}, {} range(s) now (asked {})",
            s("label").unwrap_or_else(|| DASH.into()),
            n("ranges"),
            n("ranges_asked")
        ));
        if let Some(rows) = shape.get("entry_cost").and_then(|v| v.as_array()) {
            for r in rows {
                let ranges = r.get("ranges").and_then(|v| v.as_u64()).map(|v| v.to_string()).unwrap_or_else(|| DASH.into());
                let usd = r.get("usd").and_then(|v| v.as_f64());
                let pct_of = r.get("pct").and_then(|v| v.as_f64());
                line(
                    &format!("  {ranges} range(s)"),
                    format!("{} to open and later close{}", table::usd(usd), pct_of.map(|p| format!(" ({p:.2}%)")).unwrap_or_default()),
                );
            }
        }
        if let Some(note) = s("note") {
            line("", note);
        }
    }
}

async fn open_cost(cli: &Cli, chain: &str, q: &ponk::OpenCostQuery) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        return print_json(&client.get_json(&open_cost_path(chain, q)).await?);
    }
    let cost = client.evm_open_cost_with(chain, q).await?;
    print_open_cost(&cost);
    Ok(())
}

struct EvmCreate<'a> {
    name: &'a str,
    chain: &'a str,
    dex: &'a str,
    pool: &'a str,
    down_pct: Option<f64>,
    up_pct: Option<f64>,
    shape: Option<&'a str>,
    shape_ranges: Option<&'a str>,
    strategy: Option<&'a str>,
    config: Option<&'a str>,
    stake: bool,
    withdrawal_address: Option<&'a str>,
    dry_run: bool,
}

/// The create body, built before any request so a bad combination is
/// refused with the fix and nothing is sent.
fn evm_new_agent(a: &EvmCreate<'_>, wallet: &str, own_config: Option<serde_json::Value>) -> Result<ponk::NewAgent> {
    if !ponk::evm::is_evm_chain(a.chain) {
        bail!("--chain is ethereum, arbitrum, base, bsc or robinhood. For a Solana agent use: ponk create-agent");
    }
    let chains = ponk::evm::venue_chains(a.dex);
    if !chains.is_empty() && !chains.contains(&a.chain) {
        bail!("{} runs on {}, not on {}", a.dex, chains.join(", "), a.chain);
    }
    let mut agent = match (own_config, ponk::evm::is_full_range_venue(a.dex)) {
        (Some(config), _) => {
            if a.down_pct.is_some() || a.up_pct.is_some() {
                bail!("pass a range as --down-pct and --up-pct, or your own --config, not both");
            }
            let strategy = a
                .strategy
                .map(str::to_string)
                .or_else(|| config.get("kind").and_then(|k| k.as_str()).map(str::to_string))
                .context("--strategy is required with your own --config")?;
            ponk::NewAgent::new(a.name, wallet, a.dex, strategy, config).pool_address(a.pool).chain(a.chain)
        }
        (None, true) => {
            if a.down_pct.is_some() || a.up_pct.is_some() {
                bail!("{} is a full-range venue: a position there has no range, so leave --down-pct and --up-pct out", a.dex);
            }
            ponk::NewAgent::evm_full_range(a.name, wallet, a.chain, a.dex, a.pool)
        }
        (None, false) => {
            let (Some(down), Some(up)) = (a.down_pct, a.up_pct) else {
                bail!("a range venue needs --down-pct and --up-pct as fractions (0.05 is 5%), or your own --config");
            };
            let mut agent = ponk::NewAgent::evm_range(a.name, wallet, a.chain, a.dex, a.pool, down, up)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if let Some(strategy) = a.strategy {
                agent.strategy = strategy.to_string();
                agent.config["kind"] = strategy.into();
            }
            agent
        }
    };
    if a.stake {
        if ponk::evm::stake_reward_token(a.dex).is_none() {
            bail!("{} has no farm to stake in; --stake is for pancakeswap_v3, aerodrome_slipstream and aerodrome_classic", a.dex);
        }
        agent = agent.stake(true);
    }
    if a.shape.is_some() || a.shape_ranges.is_some() {
        if ponk::evm::is_full_range_venue(a.dex) {
            bail!("{} is a full-range venue: a position there takes no shape, so leave --shape out", a.dex);
        }
        if let Some(shape) = a.shape {
            agent = agent.shape(shape).map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        if let Some(raw) = a.shape_ranges {
            if !ponk::evm::is_shape_venue(a.dex, a.chain) {
                bail!(
                    "{} on {} opens one range, so --shape-ranges does not apply there. Shapes open as several ranges on uniswap_v3 (arbitrum, ethereum, robinhood), uniswap_v4 (arbitrum, ethereum), pancakeswap_v3 and pancakeswap_infinity_cl (bsc) and aerodrome_slipstream (base)",
                    a.dex,
                    a.chain
                );
            }
            let ranges = match raw.trim() {
                r if r.eq_ignore_ascii_case("auto") => ponk::evm::ShapeRanges::Auto,
                r => ponk::evm::ShapeRanges::Fixed(r.parse().map_err(|_| anyhow::anyhow!("--shape-ranges is auto or 1 to 7, got {r}"))?),
            };
            agent = agent.shape_ranges(ranges).map_err(|e| anyhow::anyhow!("{e}"))?;
        }
    }
    if let Some(address) = a.withdrawal_address {
        agent = agent.withdrawal_address(address);
    }
    // /v1 defaults to LIVE; only a deliberate flag simulates.
    agent.dry_run = a.dry_run.then_some(true);
    Ok(agent)
}

async fn create_evm_agent(cli: &Cli, a: EvmCreate<'_>) -> Result<()> {
    let own_config = match a.config {
        Some("-") => {
            use std::io::Read;
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf).context("could not read the config from stdin")?;
            Some(serde_json::from_str(&buf).context("the config is not valid JSON")?)
        }
        Some(path) => {
            let raw = std::fs::read_to_string(path).with_context(|| format!("could not read {path}"))?;
            Some(serde_json::from_str(&raw).context("the config is not valid JSON")?)
        }
        None => None,
    };
    let client = client(cli, true)?;
    let me = client.whoami().await?;
    let Some(wallet) = me.wallet_address else {
        bail!("the key answered without a wallet address, so no agent can be created");
    };
    let agent = evm_new_agent(&a, &wallet, own_config)?;
    let created = client.create_evm_agent(&agent).await?;
    let id = created.id.clone().unwrap_or_default();
    if cli.json {
        return print_json(&client.get_json(&format!("/v1/agents/{id}")).await?);
    }
    println!(
        "created {} ({}) on {} {}: {}",
        dash(created.name.as_deref()),
        dash(created.id.as_deref()),
        dash(created.chain.as_deref()),
        dash(created.dex.as_deref()),
        match created.dry_run {
            Some(true) => "dry-run, it will simulate and send nothing",
            _ => "live",
        }
    );
    println!(
        "ponk fee: {} of the first open, once; {} of fees earned",
        bps(created.activation_fee_bps),
        bps(created.managed_fee_bps)
    );
    let coin = ponk::evm::native_symbol(a.chain).unwrap_or("");
    // The wallet is created here so the deposit address can be shown now.
    let w = client.create_agent_wallet(&id).await?;
    println!("\ndeposit address  {}", dash(w.address.as_deref()));
    println!(
        "least to hold    {}, plus the amount to deploy",
        native(w.min_open_wei.as_deref(), coin)
    );
    println!(
        "\nNothing has moved. Send {coin} you hold on {} to the deposit address, then sign the \
         mandate with your own wallets (your Solana login wallet, then the EVM wallet that will \
         receive withdrawals):",
        a.chain
    );
    if let Some(link) = ponk::evm::fund_link(&id, Some(a.chain)) {
        println!("{link}");
    }
    println!("{}", ponk::evm::agent_page_url(&id));
    Ok(())
}

async fn wallet(cli: &Cli, id: &str) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        return print_json(&client.get_json(&format!("/v1/agents/{id}/wallet")).await?);
    }
    let w = client.agent_wallet(id).await?;
    let line = |k: &str, v: String| println!("{k:<18} {v}");
    if !w.is_evm() {
        line("address", dash(w.pubkey.as_deref()));
        line("balance", sol(w.lamports));
        line("least to open", sol(w.min_open_lamports));
        line("withdrawable", sol(w.withdrawable_lamports));
        line("scope", dash(w.scope.as_deref()));
        return Ok(());
    }
    let coin = w.native_symbol.as_deref().unwrap_or("");
    line("chain", dash(w.chain.as_deref()));
    line("deposit address", dash(w.address.as_deref()));
    line("balance", format!("{} ({})", native(w.balance_wei.as_deref(), coin), usd_str(w.balance_usd.as_deref())));
    line("least to open", format!("{}, plus the amount to deploy", native(w.min_open_wei.as_deref(), coin)));
    line("kept for the exit", native(w.reserve_wei.as_deref(), coin));
    line("withdrawable", native(w.withdrawable_wei.as_deref(), coin));
    line("pays out to", w.withdrawal_address.clone().unwrap_or_else(|| "no address yet: set by the mandate you sign".into()));
    line("mandate", w.mandate_status.clone().unwrap_or_else(|| "none signed yet, so the agent cannot act".into()));
    if w.mandate_status.as_deref() != Some("active") {
        if let Some(link) = ponk::evm::fund_link(id, w.chain.as_deref()) {
            println!("\nSign it with your own wallets: {link}");
        }
    }
    Ok(())
}

fn fund_link(ids: &[String], chain: Option<&str>, lamports: Option<u64>) -> Result<()> {
    let agents: Vec<ponk::evm::FundLinkAgent> = ids
        .iter()
        .map(|id| ponk::evm::FundLinkAgent { id: id.clone(), lamports, chain: chain.map(str::to_string) })
        .collect();
    let links = ponk::evm::fund_links(&agents, false);
    if links.is_empty() {
        bail!("none of those is an agent id");
    }
    for link in links {
        println!("{link}");
    }
    Ok(())
}

async fn evm_fleet(cli: &Cli) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        return print_json(&client.get_json("/agents/evm/fleet").await?);
    }
    let fleet = client.evm_fleet().await?;
    if fleet.agents.is_empty() {
        println!("No agents on an EVM chain yet. ponk create-evm-agent starts one.");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = fleet
        .agents
        .iter()
        .map(|a| {
            vec![
                dash(a.name.as_deref()),
                dash(a.agent_id.as_deref()),
                dash(a.chain.as_deref()),
                venue_label(a.dex.as_deref()).to_string(),
                dash(a.status.as_deref()),
                dash(a.mandate_status.as_deref()),
                match a.needs.as_deref() {
                    Some("authorize_and_fund") => "sign the mandate, then fund".to_string(),
                    Some("authorize") => "mandate lapsed: sign again".to_string(),
                    Some("funding_or_waiting_to_open") => "funding, or waiting to open".to_string(),
                    Some("nothing") => "nothing".to_string(),
                    other => dash(other),
                },
                dash(a.address.as_deref()),
            ]
        })
        .collect();
    println!(
        "{}",
        render(
            &["NAME", "ID", "CHAIN", "VENUE", "STATUS", "MANDATE", "NEEDS", "ADDRESS"],
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
    for link in &fleet.sign_urls {
        println!("\nfund and sign: {link}");
    }
    Ok(())
}

fn print_plan(plan: &ponk::EvmPlan) {
    println!("plan {} ({})", dash(plan.id.as_deref()), dash(plan.state.as_deref()));
    if let Some(echo) = &plan.echo {
        println!("{echo}");
    }
    if let Some(refusal) = &plan.refusal {
        println!("refused: {}", refusal.get("message").and_then(|m| m.as_str()).unwrap_or("no reason was given"));
    }
    for c in &plan.funding_by_chain {
        let coin = c.native_symbol.as_deref().unwrap_or("");
        println!(
            "\n{}: ${} ({} {coin})",
            dash(c.chain.as_deref()),
            dash(c.amount_usd.as_deref()),
            dash(c.amount_native.as_deref())
        );
        if let Some(refusal) = &c.refusal {
            println!("  {refusal}");
        }
        if let Some(alt) = &c.alternative {
            println!("  instead: {}", dash(alt.sentence.as_deref()));
            if let Some(id) = &alt.plan_id {
                println!("  start it with: ponk start-plan {id}");
            }
            if let Some(sentence) = &alt.plan_sentence {
                println!("  plan it with: ponk plan \"{sentence}\"");
            }
        }
        let rows: Vec<Vec<String>> = c
            .legs
            .iter()
            .map(|l| {
                vec![
                    dash(l.pair.as_deref()),
                    venue_label(l.dex.as_deref()).to_string(),
                    format!("${}", dash(l.fund_usd.as_deref())),
                    format!("{} {coin}", dash(l.fund_native.as_deref())),
                    format!("${}", dash(l.deploy_usd.as_deref())),
                    pct(l.range_pct, 1),
                    pct(l.ranked_apr_pct, 2),
                    dash(l.pool_address.as_deref()),
                ]
            })
            .collect();
        let shapes: Vec<String> = c
            .legs
            .iter()
            .filter_map(|l| l.shape.as_ref().and_then(|s| s.sentence.clone()).map(|t| format!("  {}: {t}", dash(l.pair.as_deref()))))
            .collect();
        if !rows.is_empty() {
            println!(
                "{}",
                render(
                    &["PAIR", "VENUE", "SEND", "IN COIN", "DEPLOYS", "RANGE", "INCOME APR", "POOL ID"],
                    &[
                        Align::Left,
                        Align::Left,
                        Align::Right,
                        Align::Right,
                        Align::Right,
                        Align::Right,
                        Align::Right,
                        Align::Left,
                    ],
                    &rows,
                )
            );
        }
        for line in shapes {
            println!("{line}");
        }
    }
    for d in &plan.dropped {
        println!("left out {} on {}: {}", dash(d.pair.as_deref()), dash(d.chain.as_deref()), dash(d.reason.as_deref()));
    }
    if let Some(fees) = &plan.fees {
        println!("\n{fees}");
    }
    for note in &plan.notes {
        println!("{note}");
    }
    for a in &plan.agents {
        match (a.created, a.agent_id.as_deref()) {
            (Some(true), Some(id)) => println!(
                "agent {id}: {} on {}, send {} {}",
                dash(a.pair.as_deref()),
                dash(a.chain.as_deref()),
                dash(a.fund_native.as_deref()),
                dash(a.native_symbol.as_deref())
            ),
            _ => println!("not created, {}: {}", dash(a.pair.as_deref()), dash(a.refusal.as_deref())),
        }
    }
    for link in &plan.sign_urls {
        println!("fund and sign: {link}");
    }
}

/// The plan sentence with the shape and the side in the grammar's own words,
/// exactly as `copilot_evm_plan_shaped` sends it.
fn plan_sentence(text: &str, shape: Option<&str>, one_sided: Option<&str>) -> Result<String> {
    if shape.is_none() && one_sided.is_none() {
        return Ok(text.to_string());
    }
    let mut out = text.trim_end().trim_end_matches('.').to_string();
    if let Some(shape) = shape {
        let word = match shape {
            "spot" => "spot",
            "curve" => "curve",
            "bid_ask" | "bid-ask" => "bid-ask",
            other => bail!("--shape is spot, curve or bid_ask, got {other}"),
        };
        out.push_str(&format!(", {word} shape"));
    }
    if let Some(side) = one_sided {
        if !ponk::evm::ONE_SIDED.contains(&side) {
            bail!("--one-sided is below or above, got {side}");
        }
        out.push_str(&format!(", one sided {side}"));
    }
    Ok(out)
}

async fn plan(cli: &Cli, text: &str, shape: Option<&str>, one_sided: Option<&str>) -> Result<()> {
    let client = client(cli, true)?;
    let text = plan_sentence(text, shape, one_sided)?;
    let body = serde_json::json!({ "text": text });
    if cli.json {
        return print_json(&client.post_json("/copilot/evm/plans", Some(&body), None).await?);
    }
    let plan = client.copilot_evm_plan(&text).await?;
    print_plan(&plan);
    if plan.state.as_deref() == Some("ready") {
        if let Some(id) = &plan.id {
            println!("\nNothing was created. Start it with: ponk start-plan {id}");
        }
    }
    Ok(())
}

async fn start_plan(cli: &Cli, id: &str) -> Result<()> {
    let client = client(cli, true)?;
    if cli.json {
        return print_json(
            &client
                .post_json(&format!("/copilot/evm/plans/{id}/start"), None, Some(client.fund_timeout()))
                .await?,
        );
    }
    let plan = client.copilot_evm_start(id).await?;
    print_plan(&plan);
    Ok(())
}

/* ---------------------------------------------------------------------- */
/* leaderboard, whoami                                                     */
/* ---------------------------------------------------------------------- */

/// The chain ids a `--chains` value names: `all`, or each id checked
/// against the chains ponk runs on (`bnb` is `bsc`), in order, once each.
fn chain_list(value: &str) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for part in value.split(',') {
        let word = part.trim().to_ascii_lowercase();
        if word.is_empty() {
            continue;
        }
        if word == "all" {
            return Ok(vec!["all".into()]);
        }
        let word = if word == "bnb" { "bsc".to_string() } else { word };
        if !ponk::evm::CHAINS.contains(&word.as_str()) {
            bail!("unknown chain {}: chains are {}, or all", part.trim(), ponk::evm::CHAINS.join(", "));
        }
        if !out.contains(&word) {
            out.push(word);
        }
    }
    Ok(out)
}

fn leaderboard_path(window: Option<&str>, chains: &[String]) -> String {
    let mut q: Vec<String> = Vec::new();
    if let Some(w) = window {
        q.push(format!("window={}", urlencode(w)));
    }
    if !chains.is_empty() {
        q.push(format!("chains={}", urlencode(&chains.join(","))));
    }
    if q.is_empty() {
        "/public/agents-leaderboard".to_string()
    } else {
        format!("/public/agents-leaderboard?{}", q.join("&"))
    }
}

async fn feed_prefs(cli: &Cli, action: Option<&FeedPrefsCmd>) -> Result<()> {
    let client = client(cli, true)?;
    let prefs = match action {
        None => {
            if cli.json {
                return print_json(&client.get_json("/v1/feed-prefs").await?);
            }
            client.feed_prefs().await?
        }
        Some(FeedPrefsCmd::Set { chains }) => {
            let ids = chain_list(chains)?;
            let filter = if ids.is_empty() || ids == ["all"] {
                ponk::FeedChains::All
            } else {
                ponk::FeedChains::Only(ids)
            };
            client.set_feed_prefs(&filter).await?
        }
    };
    if cli.json {
        return print_json(&serde_json::json!({ "chains": prefs.chains, "all": prefs.all }));
    }
    match prefs.all {
        Some(true) => println!("every chain ({})", prefs.chains.join(", ")),
        _ if prefs.chains.is_empty() => println!("{DASH}"),
        _ => println!("{}", prefs.chains.join(", ")),
    }
    Ok(())
}

async fn leaderboard(cli: &Cli, window: Option<&str>, chains: Option<&str>) -> Result<()> {
    let client = client(cli, false)?;
    let ids = match chains {
        Some(c) => chain_list(c)?,
        None => Vec::new(),
    };
    if cli.json {
        return print_json(&client.get_json(&leaderboard_path(window, &ids)).await?);
    }
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let board = client.leaderboard_on(window, (!refs.is_empty()).then_some(refs.as_slice())).await?;
    let row = |r: &ponk::LeaderboardRow| -> Vec<String> {
        vec![
            r.rank.map(|n| n.to_string()).unwrap_or_else(|| DASH.into()),
            dash(r.slug.as_deref()),
            dash(r.chain.as_deref()),
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
            &["RANK", "SLUG", "CHAIN", "DEX", "STRATEGY", "SCORE", "PNL", "VS HODL", "PNL USD", "FEES", "FOLLOWERS"],
            &[
                Align::Right,
                Align::Left,
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
        "\nwindow {}, chains {}: {} ranked, {} unranked. Profile: ponk agent <slug>",
        dash(board.window.as_deref()),
        if board.chains.is_empty() { DASH.to_string() } else { board.chains.join(", ") },
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
        assert_eq!(leaderboard_path(None, &[]), "/public/agents-leaderboard");
        assert_eq!(leaderboard_path(Some("7d"), &[]), "/public/agents-leaderboard?window=7d");
        assert_eq!(
            leaderboard_path(Some("7d"), &chain_list("bsc,solana").unwrap()),
            "/public/agents-leaderboard?window=7d&chains=bsc%2Csolana"
        );
        assert_eq!(leaderboard_path(None, &chain_list("all").unwrap()), "/public/agents-leaderboard?chains=all");
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
            vec!["ponk", "leaderboard", "--chains", "bsc,solana"],
            vec!["ponk", "feed-prefs"],
            vec!["ponk", "feed-prefs", "set", "all"],
            vec!["ponk", "feed-prefs", "set", "solana,bsc"],
            vec![
                "ponk", "open-cost", "arbitrum", "--dex", "uniswap_v3", "--shape", "curve", "--ranges", "auto",
                "--amount-usd", "1000", "--range-pct", "5", "--side", "below",
            ],
            vec![
                "ponk", "create-evm-agent", "--name", "c", "--chain", "arbitrum", "--dex", "uniswap_v3",
                "--pool", "eip155:42161:0xc6962004f452be9203591991d15f6b388e09e8d0",
                "--down-pct", "0.05", "--up-pct", "0", "--shape", "curve", "--shape-ranges", "auto",
            ],
            vec!["ponk", "plan", "$200 in an arbitrum pool", "--shape", "curve", "--one-sided", "below"],
            vec!["ponk", "whoami"],
            vec!["ponk", "--json", "pools"],
            vec!["ponk", "chains"],
            vec!["ponk", "evm-pools", "bsc"],
            vec!["ponk", "evm-pools", "base", "--venue", "aerodrome_slipstream", "--search", "weth", "--limit", "5"],
            vec!["ponk", "evm-pool", "eip155:56:0x92b7807bf19b7dddf89b706143896d05228f3121"],
            vec!["ponk", "open-cost", "bsc", "--dex", "pancakeswap_v3", "--no-stake"],
            vec![
                "ponk", "create-evm-agent", "--name", "b", "--chain", "bsc", "--dex", "pancakeswap_v3",
                "--pool", "eip155:56:0x92b7807bf19b7dddf89b706143896d05228f3121",
                "--down-pct", "0.05", "--up-pct", "0.05", "--stake",
                "--withdrawal-address", "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed", "--dry-run",
            ],
            vec!["ponk", "wallet", "018f2c4e-9a71-7c3b-8f10-2b6a0d5e77c1"],
            vec!["ponk", "fund-link", "018f2c4e-9a71-7c3b-8f10-2b6a0d5e77c1", "--chain", "bsc"],
            vec!["ponk", "evm-fleet"],
            vec!["ponk", "plan", "$200 in the top 5 bnb pools"],
            vec!["ponk", "start-plan", "018f2c4e-9a71-7c3b-8f10-2b6a0d5e77c1"],
            vec!["ponk", "exit", "018f2c4e-9a71-7c3b-8f10-2b6a0d5e77c1", "--confirm"],
        ] {
            if let Err(e) = <Cli as clap::Parser>::try_parse_from(&argv) {
                panic!("{argv:?} did not parse: {e}");
            }
        }
    }

    fn evm_args<'a>(dex: &'a str, chain: &'a str) -> EvmCreate<'a> {
        EvmCreate {
            name: "t",
            chain,
            dex,
            pool: "eip155:56:0x92b7807bf19b7dddf89b706143896d05228f3121",
            down_pct: None,
            up_pct: None,
            shape: None,
            shape_ranges: None,
            strategy: None,
            config: None,
            stake: false,
            withdrawal_address: None,
            dry_run: false,
        }
    }

    #[test]
    fn a_shape_is_written_or_refused_before_any_request() {
        let mut a = evm_args("uniswap_v3", "arbitrum");
        a.down_pct = Some(0.05);
        a.up_pct = Some(0.0);
        a.shape = Some("curve");
        a.shape_ranges = Some("auto");
        let agent = evm_new_agent(&a, "W", None).unwrap();
        assert_eq!(agent.config["knobs"]["liquidity_shape"], "curve");
        assert_eq!(agent.config["knobs"]["range_pct"]["up_pct"], 0.0);
        assert_eq!(agent.config["evm"]["shape_ranges"], "auto");
        a.shape_ranges = Some("3");
        assert_eq!(evm_new_agent(&a, "W", None).unwrap().config["evm"]["shape_ranges"], 3);

        let err = |a: &EvmCreate<'_>| evm_new_agent(a, "W", None).unwrap_err().to_string();
        let mut full = evm_args("pancakeswap_v2", "bsc");
        full.shape = Some("curve");
        assert!(err(&full).contains("takes no shape"));
        let mut one_range = evm_args("uniswap_v4", "robinhood");
        one_range.down_pct = Some(0.05);
        one_range.up_pct = Some(0.05);
        one_range.shape = Some("curve");
        // A shape alone is kept and opens one range there.
        assert_eq!(evm_new_agent(&one_range, "W", None).unwrap().config["knobs"]["liquidity_shape"], "curve");
        one_range.shape_ranges = Some("auto");
        assert!(err(&one_range).contains("opens one range"));
        let mut bad = evm_args("uniswap_v3", "arbitrum");
        bad.down_pct = Some(0.05);
        bad.up_pct = Some(0.05);
        bad.shape = Some("hybrid");
        assert!(err(&bad).contains("shape is one of"));
        bad.shape = Some("curve");
        bad.shape_ranges = Some("9");
        assert!(err(&bad).contains("1 to 7"));
    }

    #[test]
    fn the_plan_sentence_and_the_chain_list() {
        assert_eq!(plan_sentence("$200 in bnb", None, None).unwrap(), "$200 in bnb");
        assert_eq!(
            plan_sentence("$200 in an arbitrum pool.", Some("bid_ask"), Some("below")).unwrap(),
            "$200 in an arbitrum pool, bid-ask shape, one sided below"
        );
        assert!(plan_sentence("$200 in bnb", Some("hybrid"), None).is_err());
        assert!(plan_sentence("$200 in bnb", None, Some("left")).is_err());
        assert_eq!(chain_list("bnb, Solana,bsc").unwrap(), ["bsc", "solana"]);
        assert_eq!(chain_list("solana,all").unwrap(), ["all"]);
        assert!(chain_list("polygon").is_err());
    }

    #[test]
    fn an_evm_create_is_built_or_refused_before_any_request() {
        let mut a = evm_args("pancakeswap_v3", "bsc");
        a.down_pct = Some(0.05);
        a.up_pct = Some(0.05);
        a.stake = true;
        let agent = evm_new_agent(&a, "W", None).unwrap();
        assert_eq!(agent.chain.as_deref(), Some("bsc"));
        assert_eq!(agent.strategy, "bin_rebalancer");
        assert_eq!(agent.config["knobs"]["range_pct"]["down_pct"], 0.05);
        assert_eq!(agent.config["evm"]["stake"], true);
        // Live by default, as the API is.
        assert_eq!(agent.dry_run, None);

        let full = evm_new_agent(&evm_args("pancakeswap_v2", "bsc"), "W", None).unwrap();
        assert_eq!(full.strategy, "fee_collector");

        let err = |a: &EvmCreate<'_>| evm_new_agent(a, "W", None).unwrap_err().to_string();
        assert!(err(&evm_args("pancakeswap_v3", "bsc")).contains("--down-pct"));
        assert!(err(&evm_args("pancakeswap_v3", "base")).contains("runs on bsc"));
        assert!(err(&evm_args("orca", "solana")).contains("ponk create-agent"));
        let mut ranged_v2 = evm_args("pancakeswap_v2", "bsc");
        ranged_v2.down_pct = Some(0.05);
        ranged_v2.up_pct = Some(0.05);
        assert!(err(&ranged_v2).contains("full-range venue"));
        let mut no_farm = evm_args("uniswap_v3", "arbitrum");
        no_farm.down_pct = Some(0.05);
        no_farm.up_pct = Some(0.05);
        no_farm.stake = true;
        assert!(err(&no_farm).contains("no farm"));
        let mut percent = evm_args("pancakeswap_v3", "bsc");
        percent.down_pct = Some(5.0);
        percent.up_pct = Some(5.0);
        assert!(err(&percent).contains("fraction"));

        let own = serde_json::json!({ "kind": "adaptive_range_recovery", "range_pct": { "down_pct": 0.1, "up_pct": 0.2 } });
        let agent = evm_new_agent(&evm_args("uniswap_v3", "arbitrum"), "W", Some(own.clone())).unwrap();
        assert_eq!((agent.strategy.as_str(), &agent.config), ("adaptive_range_recovery", &own));
    }

    #[test]
    fn the_evm_json_paths_match_the_typed_calls() {
        assert_eq!(evm_board_path("bsc", None), "/public/evm/bsc/board");
        assert_eq!(evm_board_path("bsc", Some("pancakeswap_v3")), "/public/evm/bsc/board?venue=pancakeswap_v3");
        let q = ponk::OpenCostQuery {
            pool: Some("eip155:56:0xab".into()),
            stake: Some(false),
            ..ponk::OpenCostQuery::new("pancakeswap_v3")
        };
        assert_eq!(
            open_cost_path("bsc", &q),
            "/v1/evm/bsc/open-cost?dex=pancakeswap_v3&pool=eip155%3A56%3A0xab&stake=false"
        );
        assert_eq!(
            open_cost_path("bsc", &ponk::OpenCostQuery::new("pancakeswap_v3")),
            "/v1/evm/bsc/open-cost?dex=pancakeswap_v3"
        );
        let shaped = ponk::OpenCostQuery {
            shape: Some("curve".into()),
            ranges: Some("auto".into()),
            amount_usd: Some(1000.0),
            side: Some("below".into()),
            ..ponk::OpenCostQuery::new("uniswap_v3")
        };
        assert_eq!(
            open_cost_path("arbitrum", &shaped),
            "/v1/evm/arbitrum/open-cost?dex=uniswap_v3&shape=curve&ranges=auto&amount_usd=1000&side=below"
        );
    }

    #[test]
    fn evm_rows_filter_order_and_print_without_inventing_figures() {
        let mk = |sym: &str, addr: &str, fees: Option<f64>| -> ponk::EvmPool {
            serde_json::from_value(serde_json::json!({
                "dex": "pancakeswap_v3", "chain": "bsc", "address": addr,
                "x": { "symbol": sym }, "y": { "symbol": "WBNB" },
                "feePct": 0.05, "dynamicFee": false, "fees": { "24h": fees }, "farm": null,
            }))
            .unwrap()
        };
        let rows = vec![mk("AAA", "a1", Some(5.0)), mk("BBB", "b1", None), mk("CCC", "c1", Some(9.0))];
        let picked = evm_select(rows.clone(), None);
        let order: Vec<&str> = picked.iter().map(|p| p.address.as_deref().unwrap()).collect();
        // Most 24h fees first; a row without the figure sinks, never zeroes.
        assert_eq!(order, vec!["c1", "a1", "b1"]);
        assert_eq!(evm_select(rows, Some("bbb")).len(), 1);
        assert_eq!(fee_tier(&picked[0]), "0.05%");
        assert_eq!(farm_text(&picked[0]), DASH);
        let dynamic: ponk::EvmPool = serde_json::from_value(serde_json::json!({
            "dynamicFee": true, "feePct": null, "farm": { "token": "CAKE", "live": true, "aprPct": 3.239 }
        }))
        .unwrap();
        assert_eq!(fee_tier(&dynamic), "dynamic");
        assert_eq!(farm_text(&dynamic), "CAKE 3.24%");
        assert_eq!(native(Some("452200000000000"), "BNB"), "0.0004522 BNB");
        assert_eq!(native(None, "BNB"), DASH);
        assert_eq!(bps(Some(150)), "1.5%");
        assert_eq!(bps(Some(1500)), "15%");
        assert_eq!(bps(None), DASH);
        assert_eq!(venue_label(Some("aerodrome_slipstream")), "slipstream");
    }

    #[test]
    fn the_fund_link_command_needs_a_real_agent_id() {
        assert!(fund_link(&["nope".to_string()], Some("bsc"), None).is_err());
        assert!(fund_link(&["018f2c4e-9a71-7c3b-8f10-2b6a0d5e77c1".to_string()], Some("bsc"), None).is_ok());
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
