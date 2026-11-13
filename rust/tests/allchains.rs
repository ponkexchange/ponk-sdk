//! 0.16.0: the boards and trackers on every chain, EVM liquidity shapes and
//! the exit fee.
//!
//! The bodies are the files in `tests/fixtures/allchains`, written field for
//! field from the API's own Serialize structs (`LeaderboardView`, `CallView`,
//! `XCallView`, `PreviewResponse`, `PostTokenView`, `TrendingResponse`,
//! `TokenResponse`, the wallet tracker's `FeedItem` and `SourceView`,
//! `WeeklyRecapView`, `FeedPrefsView`, `EvmPlanView` with `LegShape`, the
//! open-cost body with its `shape` block, `EvmWithdrawView` with
//! `exit_fees`), with illustrative values: one Solana row and one BNB Chain
//! row per model. The request tests answer from a one-request local server
//! and pin the request line and body the client sent.

use ponk::{
    CallPreview, CallersBoard, CallsFeed, CallsFeedQuery, Client, Error, EvmOpenCost, EvmPlan,
    FeedChains, FeedPrefs, Leaderboard, NewAgent, OpenCostQuery, TopCalls, WalletActivity,
    WalletFeed, WalletFeedQuery, WalletSource, WeeklyRecap, Withdrawal, XCallers, XFeed,
    XFeedQuery, XToken, XTrending,
};

const SOL_WALLET: &str = "GYdJfRSHhDAVVEz8optL8aGSQKd5daSZ9Ao3sVLZRkEe";
const EVM_WALLET: &str = "0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed";
const BONK: &str = "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263";
const CAKE: &str = "eip155:56:0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82";
const CAKE_SEG: &str = "eip155%3A56%3A0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82";
const BSC_POOL: &str = "eip155:56:0x92b7807bf19b7dddf89b706143896d05228f3121";
const ARB_POOL: &str = "eip155:42161:0xc6962004f452be9203591991d15f6b388e09e8d0";

macro_rules! fx {
    ($name:literal) => {
        include_str!(concat!("fixtures/allchains/", $name))
    };
}

fn decode<T: serde::de::DeserializeOwned>(body: &str) -> T {
    serde_json::from_str(body).unwrap()
}

/// A one-request local server answering `body`; returns the raw request.
fn one_shot(body: &'static str) -> (String, std::thread::JoinHandle<String>) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            let n = sock.read(&mut chunk).unwrap();
            buf.extend_from_slice(&chunk[..n]);
            let text = String::from_utf8_lossy(&buf).to_string();
            if let Some(h) = text.find("\r\n\r\n") {
                let len = text[..h]
                    .lines()
                    .find_map(|l| {
                        let l = l.to_ascii_lowercase();
                        l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if buf.len() >= h + 4 + len {
                    break;
                }
            }
            if n == 0 {
                break;
            }
        }
        write!(
            sock,
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
        String::from_utf8(buf).unwrap()
    });
    (format!("http://{addr}"), handle)
}

fn request_line(raw: &str) -> &str {
    raw.lines().next().unwrap_or("")
}

fn body_of(raw: &str) -> serde_json::Value {
    let at = raw.find("\r\n\r\n").expect("a request has a header block");
    serde_json::from_str(&raw[at + 4..]).unwrap()
}

/// No server listens here: a request that was refused before it was sent
/// never reaches it.
fn nowhere() -> Client {
    Client::new("http://127.0.0.1:9", "ponk_live_k").unwrap()
}

// -- the models ---------------------------------------------------------------

#[test]
fn the_leaderboard_carries_chains_and_each_rows_chain() {
    let b: Leaderboard = decode(fx!("leaderboard.json"));
    assert_eq!(b.chains, ponk::evm::CHAINS);
    let (sol, bsc) = (&b.ranked[0], &b.ranked[1]);
    assert_eq!((sol.chain.as_deref(), sol.dex.as_deref()), (Some("solana"), Some("meteora_dlmm")));
    assert_eq!((bsc.chain.as_deref(), bsc.pool_address.as_deref()), (Some("bsc"), Some(BSC_POOL)));
    assert_eq!(ponk::evm::parse_evm_id(bsc.token_x_mint.as_deref().unwrap()).unwrap().chain, "bsc");
    let one: Leaderboard = decode(fx!("leaderboard_bsc.json"));
    assert_eq!((one.chains.as_slice(), one.ranked[0].rank), (&["bsc".to_string()][..], Some(1)));
    assert_eq!(one.totals.unwrap()["pnl_usd"], "6.20");
}

#[test]
fn the_calls_feed_decodes_ponk_and_x_calls_on_both_chains() {
    let f: CallsFeed = decode(fx!("calls_feed.json"));
    assert_eq!(f.chains, ["solana", "bsc"]);
    assert_eq!(f.next_before.as_deref(), Some("2026-11-12T07:30:00Z"));
    let (evm, x, sol) = (&f.calls[0], &f.calls[1], &f.calls[2]);
    assert_eq!(
        (evm.source.as_deref(), evm.chain.as_deref(), evm.price_source.as_deref()),
        (Some("ponk"), Some("bsc"), Some("onchain_pool"))
    );
    assert_eq!(evm.mint.as_deref(), Some(CAKE));
    // The EVM boards state no market cap: a dash, never zero.
    assert_eq!((evm.mcap_usd, evm.mcap_now_usd), (None, None));
    assert_eq!((evm.auto, evm.status.as_deref()), (Some(false), Some("open")));
    assert_eq!((evm.price_usd, evm.multiple_1h), (Some(2.41), Some(1.012)));
    assert_eq!(evm.attached.as_ref().unwrap()["lpPositions"], 1);
    assert_eq!((x.source.as_deref(), x.chain.as_deref()), (Some("x"), Some("bsc")));
    assert_eq!(x.post_id.as_deref(), Some("1856000000000000001"));
    assert_eq!((sol.chain.as_deref(), sol.price_source.as_deref()), (Some("solana"), None));
    assert_eq!(sol.mcap_usd, Some(1_910_000_000.0));
}

#[test]
fn a_call_and_its_preview_read_their_camel_case_fields() {
    let c: ponk::Call = decode(fx!("call_solana.json"));
    assert_eq!((c.chain.as_deref(), c.price_usd, c.multiple_now), (Some("solana"), Some(0.0000231), Some(1.0563)));
    let c: ponk::Call = decode(fx!("call_bsc.json"));
    assert_eq!((c.price_source.as_deref(), c.peak_multiple), (Some("onchain_pool"), Some(1.0498)));
    let p: CallPreview = decode(fx!("call_preview_bsc.json"));
    assert_eq!((p.chain.as_deref(), p.mint.as_deref(), p.price_source.as_deref()), (Some("bsc"), Some(CAKE), Some("onchain_pool")));
    assert_eq!((p.has_position, p.clears_floor, p.position_usd), (Some(true), Some(true), Some(261.4)));
    let p: CallPreview = decode(fx!("call_preview_solana.json"));
    assert_eq!((p.chain.as_deref(), p.price_source.as_deref(), p.min_position_usd), (Some("solana"), None, Some(10.0)));
}

#[test]
fn top_calls_and_the_callers_board_carry_chains() {
    let t: TopCalls = decode(fx!("top_calls.json"));
    assert_eq!(t.chains, ["solana", "bsc"]);
    let b: CallersBoard = decode(fx!("callers_board.json"));
    assert_eq!((b.days, b.chains.as_slice()), (Some(30), &["bsc".to_string()][..]));
    assert_eq!(b.callers[0]["source"], "ponk");
}

#[test]
fn x_feed_tokens_carry_their_chain() {
    let f: XFeed = decode(fx!("x_feed.json"));
    assert_eq!(f.chains, ponk::evm::CHAINS);
    let post = f.items[0].post.as_ref().unwrap();
    let (bsc, sol, ambiguous) = (&post.tokens[0], &post.tokens[1], &post.tokens[2]);
    assert_eq!((bsc.chain.as_deref(), bsc.mint.as_deref(), bsc.first_call), (Some("bsc"), Some(CAKE), Some(true)));
    assert_eq!((bsc.call_price_usd, bsc.multiple), (Some(2.38), Some(1.063)));
    assert_eq!((sol.chain.as_deref(), sol.mint.as_deref()), (Some("solana"), Some(BONK)));
    // An address on several chains is never placed on one: no price.
    assert_eq!(ambiguous.chain.as_deref(), Some("evm"));
    assert_eq!(ambiguous.chains, ["ethereum", "bsc"]);
    assert_eq!((ambiguous.price_usd, ambiguous.multiple), (None, None));
    let refs = &f.items[1].event.as_ref().unwrap().post.as_ref().unwrap().tokens;
    assert_eq!(refs.iter().map(|t| t.chain.as_deref().unwrap()).collect::<Vec<_>>(), ["bsc", "solana"]);
}

#[test]
fn x_trending_callers_and_token_carry_chains() {
    let t: XTrending = decode(fx!("x_trending.json"));
    assert_eq!(t.chains, ["solana", "bsc"]);
    assert_eq!((t.tokens[0].chain.as_deref(), t.tokens[0].accounts_1h), (Some("bsc"), Some(3)));
    assert!(t.tokens[0].chains.is_empty());
    let c: XCallers = decode(fx!("x_callers.json"));
    assert_eq!((c.chains.as_slice(), c.callers[0].calls), (&["bsc".to_string()][..], Some(6)));
    let tok: XToken = decode(fx!("x_token_bsc.json"));
    assert_eq!((tok.chain.as_deref(), tok.price_source.as_deref()), (Some("bsc"), Some("onchain_pool")));
    let tok: XToken = decode(fx!("x_token_solana.json"));
    assert_eq!((tok.chain.as_deref(), tok.price_source.as_deref()), (Some("solana"), None));
}

#[test]
fn wallet_events_carry_chain_and_explorer() {
    let f: WalletFeed = decode(fx!("wallets_feed.json"));
    assert_eq!(f.chains, ["solana", "bsc"]);
    let (bsc, sol) = (&f.items[0], &f.items[1]);
    assert_eq!(bsc.chain.as_deref(), Some("bsc"));
    let url = bsc.explorer_url.as_deref().unwrap();
    assert!(url.starts_with("https://bscscan.com/tx/0x"), "{url}");
    assert_eq!(bsc.signature.as_deref(), url.rsplit('/').next());
    assert_eq!((bsc.pool.as_deref(), bsc.token_mint.as_deref()), (Some(BSC_POOL), Some(CAKE)));
    assert_eq!(bsc.wallet.as_ref().unwrap().address.as_deref(), Some(EVM_WALLET));
    assert!(sol.explorer_url.as_deref().unwrap().starts_with("https://solscan.io/tx/"));
    assert_eq!(f.source.unwrap().evm[0].chain.as_deref(), Some("bsc"));
}

#[test]
fn the_wallet_source_lists_each_evm_chain() {
    let s: WalletSource = decode(fx!("wallets_status.json"));
    let (bsc, base) = (&s.evm[0], &s.evm[1]);
    assert_eq!((bsc.cadence.as_deref(), bsc.poll_minutes, bsc.wallets_read), (Some("webhook"), Some(5), Some(3)));
    assert_eq!((base.state.as_deref(), base.cadence.as_deref(), base.last_ok_at.as_deref()), (Some("starting"), Some("poll"), None));
    let a: WalletActivity = decode(fx!("wallet_activity_untracked.json"));
    assert_eq!((a.mode.as_deref(), a.items.len()), (Some("untracked"), 0));
    let a: WalletActivity = decode(fx!("wallet_activity_tracked.json"));
    assert_eq!((a.mode.as_deref(), a.items[0].chain.as_deref()), (Some("tracked"), Some("bsc")));
}

#[test]
fn the_weekly_recap_names_the_best_agents_chain() {
    let r: WeeklyRecap = decode(fx!("weekly_recap_bsc.json"));
    assert_eq!((r.agents, r.rank, r.ranked, r.complete), (Some(2), Some(7), Some(64), Some(false)));
    let best = r.best_agent.unwrap();
    assert_eq!((best.chain.as_deref(), best.pool_address.as_deref()), (Some("bsc"), Some(BSC_POOL)));
    let r: WeeklyRecap = decode(fx!("weekly_recap_solana.json"));
    assert_eq!(r.best_agent.unwrap().chain.as_deref(), Some("solana"));
}

#[test]
fn feed_prefs_decode() {
    let p: FeedPrefs = decode(fx!("feed_prefs.json"));
    assert_eq!((p.chains.as_slice(), p.all), (&["solana".to_string(), "bsc".to_string()][..], Some(false)));
    let p: FeedPrefs = decode(fx!("feed_prefs_all.json"));
    assert_eq!((p.chains, p.all), (ponk::evm::CHAINS.map(String::from).to_vec(), Some(true)));
}

// -- the chain filter ---------------------------------------------------------

#[tokio::test]
async fn each_on_method_sends_the_chains_as_one_comma_list() {
    let both: &[&str] = &["solana", "bsc"];

    let (base, server) = one_shot(fx!("leaderboard.json"));
    Client::public(base).unwrap().leaderboard_on(Some("7d"), Some(both)).await.unwrap();
    assert_eq!(request_line(&server.join().unwrap()), "GET /public/agents-leaderboard?window=7d&chains=solana%2Cbsc HTTP/1.1");

    let (base, server) = one_shot(fx!("top_calls.json"));
    Client::public(base).unwrap().top_calls_on(Some(7), None, Some("all"), Some(both)).await.unwrap();
    assert_eq!(request_line(&server.join().unwrap()), "GET /public/calls/top?days=7&source=all&chains=solana%2Cbsc HTTP/1.1");

    let (base, server) = one_shot(fx!("callers_board.json"));
    let b = Client::public(base).unwrap().callers_board_on(Some(30), None, Some(&["bnb"])).await.unwrap();
    assert_eq!(request_line(&server.join().unwrap()), "GET /public/calls/callers?days=30&chains=bsc HTTP/1.1");
    assert_eq!(b.chains, ["bsc"]);

    let (base, server) = one_shot(fx!("x_trending.json"));
    Client::public(base).unwrap().x_trending_on(Some("24h"), Some(10), Some(both)).await.unwrap();
    assert_eq!(request_line(&server.join().unwrap()), "GET /public/x/trending?window=24h&limit=10&chains=solana%2Cbsc HTTP/1.1");

    let (base, server) = one_shot(fx!("x_callers.json"));
    Client::public(base).unwrap().x_callers_on(None, Some(&["all"])).await.unwrap();
    assert_eq!(request_line(&server.join().unwrap()), "GET /public/x/callers?chains=all HTTP/1.1");

    // None sends nothing.
    let (base, server) = one_shot(fx!("leaderboard.json"));
    Client::public(base).unwrap().leaderboard_on(None, None).await.unwrap();
    assert_eq!(request_line(&server.join().unwrap()), "GET /public/agents-leaderboard HTTP/1.1");
}

#[tokio::test]
async fn the_query_structs_carry_chains() {
    let (base, server) = one_shot(fx!("calls_feed.json"));
    let q = CallsFeedQuery {
        mint: Some(CAKE.into()),
        wallet: Some(SOL_WALLET.into()),
        chains: Some(vec!["bsc".into()]),
        ..Default::default()
    };
    let f = Client::public(base).unwrap().calls_feed_with(&q).await.unwrap();
    assert_eq!(
        request_line(&server.join().unwrap()),
        format!("GET /public/calls?mint={CAKE_SEG}&wallet={SOL_WALLET}&chains=bsc HTTP/1.1")
    );
    assert_eq!(f.calls.len(), 3);

    let (base, server) = one_shot(fx!("x_feed.json"));
    let q = XFeedQuery { first_calls: true, chains: Some(vec!["solana".into(), "bsc".into()]), ..Default::default() };
    Client::public(base).unwrap().x_feed(&q).await.unwrap();
    assert_eq!(request_line(&server.join().unwrap()), "GET /public/x/feed?first=1&chains=solana%2Cbsc HTTP/1.1");

    let (base, server) = one_shot(fx!("wallets_feed.json"));
    let q = WalletFeedQuery {
        token: Some(CAKE.into()),
        wallet: Some(EVM_WALLET.into()),
        chains: Some(vec!["bsc".into()]),
        ..Default::default()
    };
    Client::public(base).unwrap().wallets_feed(&q).await.unwrap();
    assert_eq!(
        request_line(&server.join().unwrap()),
        format!("GET /public/wallets/feed?token={CAKE_SEG}&wallet={EVM_WALLET}&chains=bsc HTTP/1.1")
    );

    let (base, server) = one_shot(fx!("wallet_activity_untracked.json"));
    let q = WalletFeedQuery { chains: Some(vec!["bsc".into()]), ..Default::default() };
    let a = Client::public(base).unwrap().wallet_activity(EVM_WALLET, &q).await.unwrap();
    assert_eq!(request_line(&server.join().unwrap()), format!("GET /public/wallets/{EVM_WALLET}/activity?chains=bsc HTTP/1.1"));
    assert_eq!(a.mode.as_deref(), Some("untracked"));
}

#[tokio::test]
async fn an_unknown_chain_is_refused_before_a_request() {
    let c = nowhere();
    assert!(matches!(c.leaderboard_on(None, Some(&["polygon"])).await, Err(Error::InvalidArgument(_))));
    assert!(matches!(c.x_trending_on(None, None, Some(&["sol"])).await, Err(Error::InvalidArgument(_))));
    let q = XFeedQuery { chains: Some(vec!["matic".into()]), ..Default::default() };
    assert!(matches!(c.x_feed(&q).await, Err(Error::InvalidArgument(_))));
    let q = WalletFeedQuery { chains: Some(vec!["avalanche".into()]), ..Default::default() };
    assert!(matches!(c.wallets_feed(&q).await, Err(Error::InvalidArgument(_))));
    let q = CallsFeedQuery { chains: Some(vec!["tron".into()]), ..Default::default() };
    assert!(matches!(c.calls_feed_with(&q).await, Err(Error::InvalidArgument(_))));
    assert!(matches!(
        c.set_feed_prefs(&FeedChains::Only(vec!["polygon".into()])).await,
        Err(Error::InvalidArgument(_))
    ));
}

#[tokio::test]
async fn x_token_percent_encodes_an_evm_id_in_the_path() {
    let (base, server) = one_shot(fx!("x_token_bsc.json"));
    let t = Client::public(base).unwrap().x_token(CAKE).await.unwrap();
    assert_eq!(
        request_line(&server.join().unwrap()),
        "GET /public/x/token/eip155%3A56%3A0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82 HTTP/1.1"
    );
    assert_eq!(t.chain.as_deref(), Some("bsc"));
}

#[tokio::test]
async fn call_preview_and_post_call_take_an_evm_id() {
    let (base, server) = one_shot(fx!("call_preview_bsc.json"));
    let p = Client::new(base, "ponk_live_k").unwrap().call_preview(BSC_POOL).await.unwrap();
    assert_eq!(
        request_line(&server.join().unwrap()),
        "GET /v1/calls/preview?mint=eip155%3A56%3A0x92b7807bf19b7dddf89b706143896d05228f3121 HTTP/1.1"
    );
    // A pool resolves to its token.
    assert_eq!(p.mint.as_deref(), Some(CAKE));
    let (base, server) = one_shot(fx!("call_bsc.json"));
    let c = Client::new(base, "ponk_live_k").unwrap().post_call(CAKE, "gm").await.unwrap();
    let raw = server.join().unwrap();
    assert_eq!(request_line(&raw), "POST /v1/calls HTTP/1.1");
    assert_eq!(body_of(&raw), serde_json::json!({ "mint": CAKE, "text": "gm" }));
    assert_eq!(c.chain.as_deref(), Some("bsc"));
}

// -- the saved filter ---------------------------------------------------------

#[tokio::test]
async fn feed_prefs_reads_and_saves() {
    let (base, server) = one_shot(fx!("feed_prefs.json"));
    let p = Client::new(base, "ponk_live_k").unwrap().feed_prefs().await.unwrap();
    assert_eq!(request_line(&server.join().unwrap()), "GET /v1/feed-prefs HTTP/1.1");
    assert_eq!(p.all, Some(false));

    let (base, server) = one_shot(fx!("feed_prefs.json"));
    Client::new(base, "ponk_live_k")
        .unwrap()
        .set_feed_prefs(&FeedChains::Only(vec!["solana".into(), "bnb".into()]))
        .await
        .unwrap();
    let raw = server.join().unwrap();
    assert_eq!(request_line(&raw), "PATCH /v1/feed-prefs HTTP/1.1");
    assert_eq!(body_of(&raw), serde_json::json!({ "chains": ["solana", "bsc"] }));

    for chains in [FeedChains::All, FeedChains::Only(vec![])] {
        let (base, server) = one_shot(fx!("feed_prefs_all.json"));
        let p = Client::new(base, "ponk_live_k").unwrap().set_feed_prefs(&chains).await.unwrap();
        let raw = server.join().unwrap();
        assert_eq!(request_line(&raw), "PATCH /v1/feed-prefs HTTP/1.1");
        assert_eq!(body_of(&raw), serde_json::json!({ "chains": "all" }));
        assert_eq!(p.all, Some(true));
    }
    assert_eq!(serde_json::to_value(FeedChains::All).unwrap(), serde_json::json!("all"));
    assert_eq!(
        serde_json::to_value(FeedChains::Only(vec!["bsc".into()])).unwrap(),
        serde_json::json!(["bsc"])
    );
}

#[tokio::test]
async fn weekly_recap_typed_reads_the_same_route() {
    let (base, server) = one_shot(fx!("weekly_recap_bsc.json"));
    let r = Client::new(base, "ponk_live_k").unwrap().weekly_recap_typed().await.unwrap();
    assert_eq!(request_line(&server.join().unwrap()), "GET /v1/recap/weekly HTTP/1.1");
    assert_eq!(r.best_agent.unwrap().chain.as_deref(), Some("bsc"));
}

// -- EVM liquidity shapes -------------------------------------------------------

#[tokio::test]
async fn create_sends_the_shape_in_the_config() {
    let (base, server) = one_shot(include_str!("fixtures/evm/agent.json"));
    let new = NewAgent::evm_range("weth usdc curve", SOL_WALLET, "arbitrum", "uniswap_v3", ARB_POOL, 0.05, 0.05)
        .unwrap()
        .shape("curve")
        .unwrap()
        .shape_ranges(ponk::evm::ShapeRanges::Auto)
        .unwrap();
    Client::new(base, "ponk_live_k").unwrap().create_evm_agent(&new).await.unwrap();
    let raw = server.join().unwrap();
    assert_eq!(request_line(&raw), "POST /v1/agents HTTP/1.1");
    let body = body_of(&raw);
    assert_eq!(body["config"]["knobs"]["liquidity_shape"], "curve");
    assert_eq!(body["config"]["evm"], serde_json::json!({ "shape_ranges": "auto" }));
    assert_eq!(body["chain"], "arbitrum");
}

#[tokio::test]
async fn a_one_sided_range_and_an_adaptive_shape() {
    let below = NewAgent::evm_range("below", SOL_WALLET, "bsc", "pancakeswap_v3", BSC_POOL, 0.05, 0.0).unwrap();
    assert_eq!(below.config["knobs"]["range_pct"]["up_pct"], 0.0);
    let mut adaptive = NewAgent::evm_range("ar", SOL_WALLET, "bsc", "pancakeswap_v3", BSC_POOL, 0.05, 0.05).unwrap();
    adaptive.strategy = "adaptive_range_recovery".into();
    adaptive.config = serde_json::json!({ "range_pct": { "down_pct": 0.05, "up_pct": 0.05 } });
    let adaptive = adaptive.shape("bid_ask").unwrap();
    assert_eq!(adaptive.config["liquidity_shape"], "bid_ask");
    assert!(adaptive.config.get("knobs").is_none());
    assert!(NewAgent::evm_range("x", SOL_WALLET, "bsc", "pancakeswap_v3", BSC_POOL, 0.05, 0.05).unwrap().shape("hybrid").is_err());
}

#[tokio::test]
async fn a_shape_is_refused_where_it_cannot_run() {
    let c = nowhere();
    let full = NewAgent::evm_full_range("aero", SOL_WALLET, "base", "aerodrome_classic", "eip155:8453:0xf634058dc94e6bf8e828e27621320c423bc8b18e")
        .shape("curve")
        .unwrap();
    assert!(matches!(c.create_evm_agent(&full).await, Err(Error::InvalidArgument(_))));
    let rh_v4 = NewAgent::evm_range("rh", SOL_WALLET, "robinhood", "uniswap_v4", "eip155:4663:0x0000000000000000000000000000000000000001", 0.05, 0.05)
        .unwrap()
        .shape("curve")
        .unwrap()
        .shape_ranges(ponk::evm::ShapeRanges::Fixed(3))
        .unwrap();
    assert!(matches!(c.create_evm_agent(&rh_v4).await, Err(Error::InvalidArgument(_))));
}

#[tokio::test]
async fn the_open_cost_quotes_a_shape() {
    let (base, server) = one_shot(fx!("open_cost_shape.json"));
    let q = OpenCostQuery {
        pool: Some(ARB_POOL.into()),
        shape: Some("curve".into()),
        ranges: Some("auto".into()),
        amount_usd: Some(1000.0),
        range_pct: Some(5.0),
        side: Some("below".into()),
        ..OpenCostQuery::new("uniswap_v3")
    };
    let cost: EvmOpenCost = Client::new(base, "ponk_live_k").unwrap().evm_open_cost_with("arbitrum", &q).await.unwrap();
    assert_eq!(
        request_line(&server.join().unwrap()),
        "GET /v1/evm/arbitrum/open-cost?dex=uniswap_v3&pool=eip155%3A42161%3A0xc6962004f452be9203591991d15f6b388e09e8d0&shape=curve&ranges=auto&amount_usd=1000&range_pct=5&side=below HTTP/1.1"
    );
    let shape = cost.shape.unwrap();
    assert_eq!((shape["ranges"].as_u64(), shape["several_ranges"].as_bool()), (Some(5), Some(true)));
    assert_eq!(cost.exit_fee_bps, Some(100));

    let (base, server) = one_shot(fx!("open_cost_shape.json"));
    Client::public(base)
        .unwrap()
        .evm_open_cost_public_with("bsc", &OpenCostQuery::new("pancakeswap_v3"))
        .await
        .unwrap();
    assert_eq!(request_line(&server.join().unwrap()), "GET /public/evm/bsc/open-cost?dex=pancakeswap_v3 HTTP/1.1");
}

#[tokio::test]
async fn the_plan_sentence_carries_the_shape_in_the_grammars_words() {
    let (base, server) = one_shot(fx!("plan_shaped.json"));
    let plan: EvmPlan = Client::new(base, "ponk_live_k")
        .unwrap()
        .copilot_evm_plan_shaped("$1000 in the top 1 arbitrum pools", Some("curve"), None)
        .await
        .unwrap();
    let raw = server.join().unwrap();
    assert_eq!(request_line(&raw), "POST /copilot/evm/plans HTTP/1.1");
    assert_eq!(body_of(&raw), serde_json::json!({ "text": "$1000 in the top 1 arbitrum pools, curve shape" }));
    let shape = plan.legs()[0].shape.clone().unwrap();
    assert_eq!((shape.kind.as_deref(), shape.ranges, shape.several_ranges), (Some("curve"), Some(5), Some(true)));
    assert_eq!((shape.entry_cost_usd.as_deref(), shape.entry_cost_limit_pct), (Some("1.84"), Some(3.0)));

    let (base, server) = one_shot(fx!("plan_one_sided.json"));
    let plan = Client::new(base, "ponk_live_k")
        .unwrap()
        .copilot_evm_plan_shaped("$1000 in the top 1 arbitrum pools.", Some("bid_ask"), Some("below"))
        .await
        .unwrap();
    assert_eq!(
        body_of(&server.join().unwrap())["text"],
        "$1000 in the top 1 arbitrum pools, bid-ask shape, one sided below"
    );
    assert_eq!(plan.legs()[0].shape.as_ref().unwrap().one_sided.as_deref(), Some("below"));

    let c = nowhere();
    assert!(matches!(c.copilot_evm_plan_shaped("$200 in bnb", Some("spot_bid_ask"), None).await, Err(Error::InvalidArgument(_))));
    assert!(matches!(c.copilot_evm_plan_shaped("$200 in bnb", None, Some("left")).await, Err(Error::InvalidArgument(_))));
    // A plan without a shape has none.
    let plain: EvmPlan = decode(include_str!("fixtures/evm/plan.json"));
    assert!(plain.legs()[0].shape.is_none());
}

// -- the exit fee ---------------------------------------------------------------

#[test]
fn an_evm_exit_lists_what_the_fee_took() {
    let w: Withdrawal = decode(fx!("exit_fee.json"));
    assert_eq!(w.exit_fee_bps, Some(100));
    assert_eq!(w.exit_fees.len(), 2);
    for (got, fee) in w.received.iter().zip(&w.exit_fees) {
        assert_eq!(got.token, fee.token);
        let (got, fee): (u128, u128) = (got.amount.as_deref().unwrap().parse().unwrap(), fee.amount.as_deref().unwrap().parse().unwrap());
        // Taken in kind: the fee is 1% of what left, the owner got the rest.
        assert_eq!(fee * 100, got + fee);
    }
    let none: Withdrawal = decode(r#"{"lamports":1,"exit_fees":[],"exit_fee_bps":null}"#);
    assert_eq!((none.exit_fees.len(), none.exit_fee_bps), (0, None));
}

#[test]
fn the_agent_and_the_perks_state_the_exit_rate() {
    let a: ponk::Agent = decode(r#"{"exit_fee_bps":100,"activation_fee_bps":150}"#);
    assert_eq!(a.exit_fee_bps, Some(100));
    let p: ponk::PonkPerks = decode(
        r#"{"holds":true,"exitFee":{"baseBps":100,"effectiveBps":50,"basePercent":1.0,"effectivePercent":0.5,"holderBps":50,"holderPercent":0.5}}"#,
    );
    let e = p.exit_fee.unwrap();
    assert_eq!((e.base_bps, e.effective_bps, e.holder_bps), (Some(100), Some(50), Some(50)));
}
