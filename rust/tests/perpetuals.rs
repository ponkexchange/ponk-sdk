//! 0.18.0: Perpetuals on Hyperliquid. Bodies are written field for field
//! from the API's own answers (`routes/perp.rs`, `routes/perp_agents.rs`,
//! `routes/perp_copilot.rs` and `routes/copilot.rs`); the request tests
//! answer from a one-request local server.

use ponk::perpetuals::{self, PerpTrade};
use ponk::{
    Client, CopilotFleet, Error, PerpAgent, PerpBoard, PerpExitCard, PerpFleetPlan, PerpFleetStop,
    PerpLedger, PerpPreview, PerpTicket,
};

const AGENT: &str = "0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b";
const FLEET: &str = "6f1d2c3b-4a59-4e6f-8a7b-9c0d1e2f3a4b";
const WARNING: &str = "Above 10x a sudden 4% price jump can pass the liquidation price before any stop fills.";
const STOP_ROOM: &str = "Liquidation is about 8.9% away (7.1% with the safety margin Liquidation Guard keeps), less than twice your 4% stop: a fast move can pass liquidation before the stop fills, and in this zone Liquidation Guard may cut part of the position before the stop does. A stop of 3.5% or less, or lower leverage, keeps twice the room.";

fn decode<T: serde::de::DeserializeOwned>(body: &str) -> T {
    serde_json::from_str(body).unwrap()
}

fn one_shot(body: &'static str) -> (String, std::thread::JoinHandle<String>) {
    one_shot_status("200 OK", body.to_string())
}

fn one_shot_status(status: &'static str, body: String) -> (String, std::thread::JoinHandle<String>) {
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
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
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

#[test]
fn leverage_is_not_capped_and_the_warning_is_the_servers() {
    assert_eq!(perpetuals::HIGH_LEVERAGE_WARNING, WARNING);
    assert_eq!(perpetuals::HIGH_LEVERAGE_ABOVE, 10);
    assert_eq!(perpetuals::check_leverage(20).unwrap(), 20);
    assert_eq!(perpetuals::check_leverage(50).unwrap(), 50);
    assert_eq!(perpetuals::check_leverage(10).unwrap(), 10);
    assert!(perpetuals::check_leverage(0).is_err());
    assert!(!perpetuals::needs_confirmation(10));
    assert!(perpetuals::needs_confirmation(11));
    assert!(PerpTrade::new("BTC", "long", 25, "500", "3").check().is_ok());
    assert!(PerpTrade::new("BTC", "long", 0, "500", "3").check().is_err());
    assert!(matches!(PerpTrade::new("BTC", "up", 2, "500", "3").check(), Err(Error::InvalidArgument(s)) if s == "Say long or short."));
    assert_eq!(perpetuals::market_id("kPEPE"), "perp:hyperliquid:kPEPE");
    assert_eq!(perpetuals::market_id("perp:hyperliquid:ETH"), "perp:hyperliquid:ETH");
    assert_eq!(perpetuals::coin_of("perp:hyperliquid:SOL"), "SOL");
}

#[tokio::test]
async fn the_shape_is_checked_before_a_request() {
    // Port 9 is never listened on: a request would fail as transport, not
    // as the argument error.
    let c = Client::new("http://127.0.0.1:9", "ponk_live_k").unwrap();
    let t = PerpTrade::new("BTC", "long", 0, "500", "3");
    assert!(matches!(c.create_perp_agent(&t).await, Err(Error::InvalidArgument(_))));
    assert!(matches!(c.preview_perp_agent(&t).await, Err(Error::InvalidArgument(_))));
    assert!(matches!(c.exit_perp_agent(AGENT, false).await, Err(Error::InvalidArgument(_))));
    assert!(matches!(c.close_perp_agent(AGENT, false).await, Err(Error::InvalidArgument(_))));
}

#[tokio::test]
async fn above_10x_is_sent_with_the_typed_name() {
    let (base, h) = one_shot(
        r#"{"id":"0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b","network":"testnet","practice":false,"ticket":"Long BTC 20x","liquidationPrice":93000.0,"stopPrice":95000.0,
        "ticketFigures":{"leverage":20,"highLeverage":true},"fees":null,
        "warnings":["Above 10x a sudden 4% price jump can pass the liquidation price before any stop fills."],"next":"Authorize the agent, then fund its Hyperliquid account."}"#,
    );
    let t = PerpTrade::new("BTC", "long", 20, "500", "2").name("BTC 20x").confirmation_text("BTC 20x");
    let a = Client::new(&base, "ponk_live_k").unwrap().create_perp_agent(&t).await.unwrap();
    let body = body_of(&h.join().unwrap());
    assert_eq!(body["leverage"], 20);
    assert_eq!(body["confirmationText"], "BTC 20x");
    assert_eq!(a.warnings, vec![WARNING.to_string()]);
    assert_eq!(a.ticket_figures.unwrap().high_leverage, Some(true));
}

#[test]
fn the_preview_carries_warnings_and_an_older_one_none() {
    let p: PerpPreview = decode(
        r#"{"ok":true,"network":"testnet","practice":false,"name":"BTC","ticket":{"leverage":20,"highLeverage":true},"fees":null,
        "needsConfirmation":true,"warnings":["The long side is crowded.","Above 10x a sudden 4% price jump can pass the liquidation price before any stop fills."]}"#,
    );
    assert_eq!(p.needs_confirmation, Some(true));
    assert_eq!(p.warnings.len(), 2);
    assert_eq!(p.warnings[1], WARNING);
    let older: PerpPreview = decode(r#"{"ok":true,"name":"BTC","needsConfirmation":false}"#);
    assert!(older.warnings.is_empty());
}

#[test]
fn the_board_reads_camel_case() {
    let b: PerpBoard = decode(
        r#"{"venue":"hyperliquid","asOf":1,"servedAt":2,"stale":false,"lastError":null,"fundingIntervalHours":1,"nextFundingTime":3,"total":180,
        "markets":[{"id":"perp:hyperliquid:BTC","venue":"hyperliquid","coin":"BTC","symbol":"BTC-USD","szDecimals":5,"priceDecimals":1,"maxLeverage":40,
        "leverageTiers":[[0,40],[150000000,20]],"isolatedOnly":false,"markPx":97000.5,"oraclePx":null,"midPx":null,"prevDayPx":95000.0,"change24hPct":2.1,
        "volume24hUsd":2500000000.0,"openInterest":30000.1,"openInterestUsd":null,"fundingRate":0.0000125,"fundingIntervalHours":1,"nextFundingTime":3,
        "fundingHourly":0.0000125,"fundingAnnualized":0.1095,"premium":null,"tradeUrl":"https://app.hyperliquid.xyz/trade/BTC"}]}"#,
    );
    let m = &b.markets[0];
    assert_eq!((m.max_leverage, m.sz_decimals, m.change24h_pct), (Some(40), Some(5), Some(2.1)));
    assert_eq!(m.leverage_tiers[1], (150000000.0, 20));
    assert_eq!(m.oracle_px, None);
    assert_eq!(b.total, Some(180));
}

#[tokio::test]
async fn markets_are_public_and_take_their_query() {
    let (base, h) = one_shot(r#"{"venue":"hyperliquid","total":0,"markets":[]}"#);
    Client::public(&base).unwrap().perp_markets(Some("bt"), Some("oi"), Some(5)).await.unwrap();
    let raw = h.join().unwrap();
    assert_eq!(request_line(&raw), "GET /public/perp/markets?q=bt&sort=oi&limit=5 HTTP/1.1");
    assert!(!raw.to_ascii_lowercase().contains("authorization:"));

    let (base, h) = one_shot(r#"{"id":"perp:hyperliquid:BTC","coin":"BTC","interval":"4h","asOf":1,"stale":false,"historyLimit":5000,"oldestAvailable":0,"candles":[{"t":1,"o":1.0,"h":2.0,"l":0.5,"c":1.5,"v":null,"n":3}]}"#);
    let c = Client::public(&base).unwrap().perp_candles("perp:hyperliquid:BTC", Some("4h"), Some(2)).await.unwrap();
    assert_eq!(request_line(&h.join().unwrap()), "GET /public/perp/markets/perp%3Ahyperliquid%3ABTC/candles?interval=4h&bars=2 HTTP/1.1");
    assert_eq!((c.candles[0].l, c.candles[0].v), (Some(0.5), None));
}

#[tokio::test]
async fn create_posts_the_typed_trade() {
    let (base, h) = one_shot(
        r#"{"id":"0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b","network":"testnet","practice":false,"ticket":"Long BTC 3x with $500, stop 3%.","liquidationPrice":65000.0,"stopPrice":94090.0,
        "ticketFigures":{"sentence":"Long BTC 3x","side":"long","coin":"BTC","leverage":3,"marginUsd":500.0,"notionalUsd":1500.0,"size":0.01546,"liquidationPrice":65000.0,
        "liquidationMove":0.33,"stopPrice":94090.0,"stopFraction":0.03,"stopDefaulted":false,"lossAtStopUsd":45.0,"takeProfitPrice":null,"highLeverage":false},
        "fees":{"activationBps":150,"performanceBps":1500,"exitBps":100},"next":"Authorize the agent, then fund its Hyperliquid account."}"#,
    );
    let t = PerpTrade::new("BTC", "long", 3, "500", "3").take_profit_pct("10").name("btc runner");
    let a = Client::new(&base, "ponk_live_k").unwrap().create_perp_agent(&t).await.unwrap();
    let raw = h.join().unwrap();
    assert_eq!(request_line(&raw), "POST /v1/perp-agents HTTP/1.1");
    assert_eq!(
        body_of(&raw),
        serde_json::json!({"coin":"BTC","direction":"long","leverage":3,"capitalUsd":"500","stopLossPct":"3","takeProfitPct":"10","name":"btc runner","practice":false})
    );
    assert!(a.created());
    assert_eq!(a.ticket_figures.as_ref().unwrap().loss_at_stop_usd, Some(45.0));
    assert_eq!(a.fees.as_ref().unwrap().exit_bps, Some(100));
}

#[tokio::test]
async fn say_close_and_stop() {
    let (base, h) = one_shot(r#"{"agentId":"0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b","request":"close","sentence":"Closing at your word."}"#);
    let s = Client::new(&base, "ponk_live_k").unwrap().perp_say("close my btc long", true).await.unwrap();
    let raw = h.join().unwrap();
    assert_eq!(request_line(&raw), "POST /v1/perp-agents/say HTTP/1.1");
    assert_eq!(body_of(&raw), serde_json::json!({"text":"close my btc long","confirm":true}));
    assert!(!s.created());
    assert_eq!(s.request.as_deref(), Some("close"));

    // Above 10x the typed name is the acknowledgement, never confirm.
    let (base, h) = one_shot(r#"{"id":"0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b","ticket":"Long BTC 20x."}"#);
    Client::new(&base, "ponk_live_k").unwrap().perp_say_confirmed("long btc 20x 500 stop 1", "BTC").await.unwrap();
    assert_eq!(body_of(&h.join().unwrap()), serde_json::json!({"text":"long btc 20x 500 stop 1","confirmationText":"BTC"}));

    let (base, h) = one_shot(r#"{"agentId":"a","request":"tighten_stop","sentence":"Moving the stop."}"#);
    Client::new(&base, "ponk_live_k").unwrap().tighten_perp_stop(AGENT, "2").await.unwrap();
    let raw = h.join().unwrap();
    assert_eq!(request_line(&raw), format!("POST /v1/perp-agents/{AGENT}/stop HTTP/1.1"));
    assert_eq!(body_of(&raw), serde_json::json!({"stopPct":"2"}));

    let (base, h) = one_shot(r#"{"status":"requested","destination":"0x11","exitFeeBps":100,"sentence":"Closing."}"#);
    let e = Client::new(&base, "ponk_live_k").unwrap().exit_perp_agent(AGENT, true).await.unwrap();
    let raw = h.join().unwrap();
    assert_eq!(request_line(&raw), format!("POST /v1/perp-agents/{AGENT}/exit HTTP/1.1"));
    assert_eq!(body_of(&raw), serde_json::json!({"confirm":true}));
    assert_eq!(e.exit_fee_bps, Some(100));
}

#[test]
fn the_agent_reads_strings_and_nulls() {
    let a: PerpAgent = decode(
        r#"{"account":{"network":"testnet","practice":false,"coin":"BTC","readAtMs":10,"ageMs":2000,"stale":false,"bookingPending":false,
        "balances":{"perpsValueUsd":"512.4","withdrawableUsd":"12.1","spotUsdc":null,"accountMode":"standard"},
        "position":{"side":"long","size":"0.01546","entryPrice":"97000","markPrice":null,"notionalUsd":"1501.1","unrealizedPnlUsd":"1.5","marginUsd":"501.5","marginUsage":0.06,"liquidationPrice":65000.0,"stopPrice":"94090"}},
        "exitFeeBps":100,"id":"x","status":"running","agentAccount":"0x11",
        "config":{"network":"testnet","coin":"BTC","direction":"long","leverage":3,"capitalUsd":"500","stopLossPct":"3","takeProfitPct":null,"practice":false},
        "decision":{"codes":["HOLD"],"sentences":["Holding."],"sent":[],"at":"t"},"realized":null,"exit":null}"#,
    );
    let acct = a.account.unwrap();
    assert_eq!(acct.balances.as_ref().unwrap().perps_value_usd.as_deref(), Some("512.4"));
    assert_eq!(acct.balances.as_ref().unwrap().spot_usdc, None);
    assert_eq!(acct.position.as_ref().unwrap().mark_price, None);
    assert_eq!(a.config.unwrap().stop_loss_pct.as_deref(), Some("3"));
    assert!(a.realized.is_none());

    let l: PerpLedger = decode(
        r#"{"orders":[{"purpose":"stop","side":"sell","type":"trigger","size":"0.01546","reduceOnly":true,"status":"resting","sentence":null,"at":"t"}],
        "fills":[{"coin":"BTC","side":"buy","px":"97000","sz":"0.01546","dir":"Open Long","closedPnl":"0","fee":"0.52","liquidation":false,"timeMs":1}],
        "funding":[{"coin":"BTC","usdc":"-0.01","fundingRate":"0.0000125","timeMs":2}],"fees":[],"withdrawals":[]}"#,
    );
    assert_eq!(l.orders[0].order_type.as_deref(), Some("trigger"));
    assert_eq!(l.funding[0].usdc.as_deref(), Some("-0.01"));

    let card: PerpExitCard = decode(
        r#"{"kind":"perp","agent_id":"x","venue":"hyperliquid","market":"BTC-USD","side":"long","leverage":3,"pnl_usd":null,"pnl_unavailable":"A transfer is still settling.","fees_lifetime_usd":null,"il_usd":null,"exit_signature":null,"liquidated":false}"#,
    );
    assert_eq!(card.pnl_usd, None);
    assert_eq!(card.pnl_unavailable.as_deref(), Some("A transfer is still settling."));
}

#[test]
fn a_perp_fleet_and_its_plan() {
    let f: CopilotFleet = decode(
        r#"{"kind":"perp","fleet":{"id":"f","kind":"perp","intent_text":"x","status":"active","agents":2},"stop_now":{},"totals":{},"rows":[],"missing_agents":[],
        "perp":{"network":"testnet","practice":false,"exit_fee_bps":100,"rows":[{"agent_id":"a","name":"BTC Long 2x","pnl_usd":null,"pnl_reason":"Closed, settling.","booking_pending":true,"needs":"nothing","close":{"state":"open","sentence":null}}],
        "totals":{"value_usd":null,"notional_usd":0.0,"margin_usd":0.0,"open_positions":0,"reason":"An agent is settling."},"funding":{"to_authorize":1,"to_fund":1,"sign_url":null,"sentence":null}}}"#,
    );
    let p = f.perp.unwrap();
    assert_eq!(f.kind.as_deref(), Some("perp"));
    assert_eq!(p.rows[0].pnl_reason.as_deref(), Some("Closed, settling."));
    assert_eq!(p.totals.unwrap().value_usd, None);

    let plan: PerpFleetPlan = decode(
        r#"{"id":"p","kind":"perp","state":"ready","text":"x","echo":"y","refusal":null,"network":"testnet","practice":false,
        "legs":[{"index":0,"name":"BTC Long 2x","coin":"BTC","direction":"long","leverage":2,"leverage_defaulted":true,"capital_usd":"500","stop_pct":3.0,"stop_defaulted":true,
        "take_profit_pct":null,"ticket":{"lossAtStopUsd":30.0},"high_leverage":false,"needs_confirmation":false,"note":null,"refusal":null}],
        "totals":{"agents":1,"capital_usd":"500","notional_usd":1000.0,"loss_at_stops_usd":30.0},"fleet_stop_pct":10.0,"fees":null,"fees_for":"api",
        "signatures":null,"notes":[],"can_start":true,"fleet_id":null,"agents":[],"sign_url":null}"#,
    );
    assert_eq!(plan.legs[0].ticket.as_ref().unwrap().loss_at_stop_usd, Some(30.0));
    assert_eq!(plan.fees_for.as_deref(), Some("api"));
}

#[test]
fn agent_stops_read_answers_beside_the_count() {
    // The newer server names the list `answers`; `agents` stays the count.
    let s = PerpFleetStop::from_value(serde_json::json!({
        "id": FLEET, "kind": "perp", "status": "active", "agents": 2, "stop_drawdown_pct": 10.0,
        "answers": [{"agent_id": "a", "name": "BTC Long 2x", "ok": false, "sentence": "The stop is already tighter."}]
    }))
    .unwrap();
    assert_eq!(s.answers[0].sentence.as_deref(), Some("The stop is already tighter."));
    assert_eq!(s.fleet.agents, Some(2));
    // Both lists: `answers` is the one read.
    let s = PerpFleetStop::from_value(serde_json::json!({
        "id": FLEET, "agents": [{"agent_id": "old"}], "answers": [{"agent_id": "new"}]
    }))
    .unwrap();
    assert_eq!(s.answers.len(), 1);
    assert_eq!(s.answers[0].agent_id.as_deref(), Some("new"));
    assert_eq!(s.fleet.agents, None);
}

#[tokio::test]
async fn agent_stops_keep_the_older_agents_list() {
    // An older server: the fleet summary's `agents` count and the answers
    // list share a key; the list is written last, and that is the one kept.
    let (base, h) = one_shot(
        r#"{"id":"6f1d2c3b-4a59-4e6f-8a7b-9c0d1e2f3a4b","kind":"perp","intent_text":"x","status":"active","agents":2,"stop_drawdown_pct":10.0,
        "agents":[{"agent_id":"a","name":"BTC Long 2x","ok":true,"sentence":"Moving the stop at your word on the next tick."}]}"#,
    );
    let s: PerpFleetStop = Client::new(&base, "ponk_live_k").unwrap().copilot_tighten_agent_stops(FLEET, 2.0).await.unwrap();
    let raw = h.join().unwrap();
    assert_eq!(request_line(&raw), format!("POST /copilot/fleets/{FLEET}/stop-rule HTTP/1.1"));
    assert_eq!(body_of(&raw), serde_json::json!({"stop_pct":2.0}));
    assert_eq!(s.answers[0].ok, Some(true));
    assert_eq!(s.fleet.kind.as_deref(), Some("perp"));
    assert_eq!(s.fleet.agents, None);
}

#[tokio::test]
async fn fleet_pause_and_plan_start() {
    let (base, h) = one_shot(r#"{"batch_id":"b","kind":"pause","total":1,"done":1,"failed":0,"running":0,"unknown":0,"skipped":0,"items":[]}"#);
    let b = Client::new(&base, "ponk_live_k").unwrap().copilot_pause_fleet(FLEET).await.unwrap();
    assert_eq!(request_line(&h.join().unwrap()), format!("POST /copilot/fleets/{FLEET}/pause HTTP/1.1"));
    assert_eq!(b.kind.as_deref(), Some("pause"));

    let (base, h) = one_shot(r#"{"id":"p","kind":"perp","state":"started","fleet_id":"f","agents":[{"index":0,"agent_id":"a","created":true}],"sign_url":"https://ponk.exchange/x"}"#);
    let p = Client::new(&base, "ponk_live_k").unwrap().copilot_perp_start("p").await.unwrap();
    assert_eq!(request_line(&h.join().unwrap()), "POST /copilot/perp/plans/p/start HTTP/1.1");
    assert_eq!(p.state.as_deref(), Some("started"));
    assert_eq!(p.agents[0]["created"], true);

    let (base, h) = one_shot(r#"{"id":"p","kind":"perp","state":"started","fleet_id":"f","agents":[]}"#);
    let names = std::collections::BTreeMap::from([(0usize, "BTC Long 2x".to_string()), (2, "SOL Short 2x".to_string())]);
    Client::new(&base, "ponk_live_k").unwrap().copilot_perp_start_confirmed("p", &names).await.unwrap();
    let raw = h.join().unwrap();
    assert_eq!(request_line(&raw), "POST /copilot/perp/plans/p/start HTTP/1.1");
    assert_eq!(body_of(&raw), serde_json::json!({"confirmations": {"0": "BTC Long 2x", "2": "SOL Short 2x"}}));
}

// The stop rule: a stop inside liquidation but with liquidation closer than
// twice its distance is a warning acknowledged with the typed name, at any
// leverage. The preview is the authority.

fn stop_room_preview() -> PerpPreview {
    decode(&format!(
        r#"{{"ok":true,"network":"testnet","practice":false,"name":"BTC",
        "ticket":{{"leverage":5,"highLeverage":false,"needsConfirmation":true,"stopRoomWarning":"{STOP_ROOM}","atrWarning":null}},
        "fees":null,"needsConfirmation":true,"warnings":["The long side is crowded.","{STOP_ROOM}"],"confirmationWarnings":["{STOP_ROOM}"]}}"#
    ))
}

#[test]
fn the_preview_is_the_authority_at_5x() {
    let p = stop_room_preview();
    assert!(!perpetuals::needs_confirmation(5));
    assert!(perpetuals::needs_confirmation(&p));
    assert_eq!(p.confirmation_warnings, vec![STOP_ROOM.to_string()]);
    assert_eq!(p.warnings.len(), 2);
    let t = p.ticket.as_ref().unwrap();
    assert!(perpetuals::needs_confirmation(t));
    assert_eq!(t.needs_confirmation, Some(true));
    assert_eq!(t.stop_room_warning.as_deref(), Some(STOP_ROOM));
    assert_eq!(t.atr_warning, None);

    // Above 10x with no stop-room warning, and a plain 3x.
    let high: PerpPreview = decode(&format!(
        r#"{{"ok":true,"needsConfirmation":true,"warnings":["{WARNING}"],"confirmationWarnings":["{WARNING}"]}}"#
    ));
    assert!(perpetuals::needs_confirmation(&high));
    let plain: PerpPreview = decode(r#"{"ok":true,"needsConfirmation":false,"ticket":{"leverage":3}}"#);
    assert!(!perpetuals::needs_confirmation(&plain));
    assert!(plain.confirmation_warnings.is_empty());
    assert_eq!(plain.ticket.as_ref().unwrap().stop_room_warning, None);
    // A refused preview (a stop past liquidation) needs nothing typed: it is refused.
    let refused: PerpPreview = decode(r#"{"ok":false,"sentence":"A 12% stop sits past the liquidation price. A stop of 7% or less would work."}"#);
    assert!(!perpetuals::needs_confirmation(&refused));
    let older: PerpTicket = decode(r#"{"leverage":20,"highLeverage":true}"#);
    assert!(!perpetuals::needs_confirmation(&older));
}

#[tokio::test]
async fn create_at_5x_sends_the_typed_name() {
    let (base, h) = one_shot_status(
        "200 OK",
        format!(
            r#"{{"id":"{AGENT}","network":"testnet","ticket":"Long BTC 5x","ticketFigures":{{"leverage":5,"needsConfirmation":true,"stopRoomWarning":"{STOP_ROOM}","atrWarning":"The stop sits inside BTC's usual daily move."}},"warnings":["{STOP_ROOM}"]}}"#
        ),
    );
    let t = PerpTrade::new("BTC", "long", 5, "200", "4").confirmation_text("btc");
    let a = Client::new(&base, "ponk_live_k").unwrap().create_perp_agent(&t).await.unwrap();
    let body = body_of(&h.join().unwrap());
    assert_eq!((body["leverage"].as_u64(), body["confirmationText"].as_str()), (Some(5), Some("btc")));
    let f = a.ticket_figures.unwrap();
    assert_eq!(f.needs_confirmation, Some(true));
    assert_eq!(f.stop_room_warning.as_deref(), Some(STOP_ROOM));
    assert_eq!(f.atr_warning.as_deref(), Some("The stop sits inside BTC's usual daily move."));
}

#[tokio::test]
async fn create_without_the_name_is_the_servers_sentence() {
    let msg = format!("{STOP_ROOM} To go ahead, type the agent's name (BTC) to confirm.");
    let (base, h) = one_shot_status(
        "422 Unprocessable Entity",
        serde_json::json!({"error": {"code": "unprocessable", "message": msg, "request_id": "r"}}).to_string(),
    );
    let t = PerpTrade::new("BTC", "long", 5, "200", "4");
    let e = Client::new(&base, "ponk_live_k").unwrap().create_perp_agent(&t).await.unwrap_err();
    h.join().unwrap();
    let api = e.api().expect("an API error");
    assert_eq!(api.status, 422);
    assert!(api.message.ends_with("To go ahead, type the agent's name (BTC) to confirm."));
}

#[tokio::test]
async fn say_sends_the_typed_name_at_any_leverage() {
    let (base, h) = one_shot(r#"{"id":"0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b","ticket":"Long BTC 5x."}"#);
    let s = Client::new(&base, "ponk_live_k").unwrap().perp_say_confirmed("long btc 5x 200 stop 4", "BTC").await.unwrap();
    assert!(s.created());
    assert_eq!(body_of(&h.join().unwrap()), serde_json::json!({"text":"long btc 5x 200 stop 4","confirmationText":"BTC"}));

    let msg = format!(
        "{STOP_ROOM} To go ahead, send the same line again with confirmationText set to the agent's name, BTC, typed by the user. confirm: true alone does not acknowledge it. Nothing was created."
    );
    let (base, h) = one_shot_status(
        "422 Unprocessable Entity",
        serde_json::json!({"error": {"code": "unprocessable", "message": msg, "request_id": "r"}}).to_string(),
    );
    let e = Client::new(&base, "ponk_live_k").unwrap().perp_say("long btc 5x 200 stop 4", true).await.unwrap_err();
    assert_eq!(body_of(&h.join().unwrap()), serde_json::json!({"text":"long btc 5x 200 stop 4","confirm":true}));
    assert!(e.api().unwrap().message.ends_with("Nothing was created."));
}

#[test]
fn mixed_case_tickers_are_sent_as_given() {
    let t = PerpTrade::new("kPEPE", "long", 20, "50", "2").name("kPEPE Long 20x").confirmation_text("KPEPE Long 20x");
    let v = serde_json::to_value(&t).unwrap();
    assert_eq!(v["coin"], "kPEPE");
    assert_eq!(v["name"], "kPEPE Long 20x");
    assert_eq!(v["confirmationText"], "KPEPE Long 20x");
}

#[test]
fn agent_config_warnings_and_plan_leg_confirmation_warnings() {
    let a: PerpAgent = decode(&format!(
        r#"{{"id":"x","config":{{"coin":"BTC","direction":"long","leverage":5,"capitalUsd":"200","stopLossPct":"4","warnings":["{STOP_ROOM}"]}}}}"#
    ));
    assert_eq!(a.config.unwrap().warnings, vec![STOP_ROOM.to_string()]);
    let older: PerpAgent = decode(r#"{"id":"x","config":{"coin":"BTC","leverage":3}}"#);
    assert!(older.config.unwrap().warnings.is_empty());

    let plan: PerpFleetPlan = decode(&format!(
        r#"{{"id":"p","kind":"perp","state":"ready","legs":[
        {{"index":0,"name":"BTC Long 5x","coin":"BTC","leverage":5,"stop_pct":4.0,"ticket":{{"needsConfirmation":true,"stopRoomWarning":"{STOP_ROOM}"}},
          "high_leverage":false,"needs_confirmation":true,"confirmation_warnings":["{STOP_ROOM}"],"note":null,"refusal":null}},
        {{"index":1,"name":"SOL Short 2x","coin":"SOL","leverage":2,"needs_confirmation":false,"refusal":null}}]}}"#
    ));
    let leg = &plan.legs[0];
    assert!(perpetuals::needs_confirmation(leg));
    assert_eq!(leg.confirmation_warnings, vec![STOP_ROOM.to_string()]);
    assert_eq!(leg.ticket.as_ref().unwrap().stop_room_warning.as_deref(), Some(STOP_ROOM));
    assert!(!perpetuals::needs_confirmation(&plan.legs[1]));
    assert!(plan.legs[1].confirmation_warnings.is_empty());
}
