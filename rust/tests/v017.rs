//! 0.17.0: collect all, one position shape, the registry, any wallet's value
//! line, farming rewards on Solana agents, PancakeSwap Infinity Bin, and the
//! smaller additions on the feeds, the X Tracker, follows and the
//! leaderboard. Bodies are written field for field from the API's Serialize
//! structs; the request tests answer from a one-request local server.

use ponk::{
    AgentPerformance, AgentPositions, AgentsActivity, Client, Error, FleetBatch, FollowState,
    LeaderboardRow, Registry, SocialFeed, WalletHistory, XDirectory,
};

const AGENT: &str = "0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b";

fn decode<T: serde::de::DeserializeOwned>(body: &str) -> T {
    serde_json::from_str(body).unwrap()
}

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

const CLAIM_BATCH: &str = r#"{"batch_id":"6f1d2c3b-4a59-4e6f-8a7b-9c0d1e2f3a4b","kind":"claim","total":3,"done":1,"failed":0,"running":0,"unknown":1,"skipped":1,"items":[
 {"index":0,"agent_id":"a","outcome":"done","ok":true,"status":200,"code":null,"error":null,"result":{"action_status":"confirmed"}},
 {"index":1,"agent_id":"b","outcome":"unknown","ok":false,"status":200,"code":"outcome_unknown","error":"The claim was sent and its landing is not confirmed yet. Do not claim again: check this signature first. 4sig","result":null},
 {"index":2,"agent_id":"c","outcome":"skipped","ok":false,"status":422,"code":"nothing_to_collect","error":"Ponk Clouds fees grow the position itself, so there is nothing to collect. Nothing was sent.","result":null}]}"#;

#[test]
fn a_claim_batch_counts_unknown_and_skipped() {
    let b: FleetBatch = decode(CLAIM_BATCH);
    assert_eq!((b.kind.as_deref(), b.total, b.done, b.unknown, b.skipped), (Some("claim"), 3, 1, 1, 1));
    assert_eq!(b.items[2].code.as_deref(), Some("nothing_to_collect"));
}

#[tokio::test]
async fn claim_agents_posts_the_ids_and_the_destination() {
    let (base, h) = one_shot(CLAIM_BATCH);
    Client::new(&base, "ponk_live_k").unwrap().claim_agents(&[AGENT], Some("agent")).await.unwrap();
    let raw = h.join().unwrap();
    assert_eq!(request_line(&raw), "POST /v1/agents/batch/claim HTTP/1.1");
    assert_eq!(body_of(&raw), serde_json::json!({ "agent_ids": [AGENT], "destination": "agent" }));

    let (base, h) = one_shot(CLAIM_BATCH);
    Client::new(&base, "ponk_live_k").unwrap().claim_all_agents(None).await.unwrap();
    assert_eq!(body_of(&h.join().unwrap()), serde_json::json!({ "all": true }));
}

#[tokio::test]
async fn agent_action_refuses_an_unknown_action_before_a_request() {
    let c = Client::new("http://127.0.0.1:9", "ponk_live_k").unwrap();
    assert!(matches!(c.agent_action(AGENT, "liquidate", None).await, Err(Error::InvalidArgument(_))));
    let (base, h) = one_shot(r#"{"action_status":"confirmed"}"#);
    let v = Client::new(&base, "ponk_live_k").unwrap().agent_action(AGENT, "compound", None).await.unwrap();
    assert_eq!(request_line(&h.join().unwrap()), format!("POST /v1/agents/{AGENT}/actions/compound HTTP/1.1"));
    assert_eq!(v["action_status"], "confirmed");
}

#[test]
fn positions_read_observed_figures() {
    let p: AgentPositions = decode(
        r#"{"agent_id":"a","kind":"lp","positions":[{"kind":"lp","venue":"uniswap_v3","chain":"arbitrum",
        "id":"eip155:42161:0xc36442b4a4522e871399cd717abdd847ab11fe88:41000",
        "summary":{"status":"open","value_usd":{"state":"known","value":"608.00"},"cost_basis_usd":{"state":"known","value":"600.00"},
        "pnl_usd":{"state":"known","value":"8.00"},"yield_usd":{"state":"absent","absence":{"kind":"unpriced","why":"No price."}}},
        "detail":{"kind":"lp","pool":"eip155:42161:0xc6962004f452be9203591991d15f6b388e09e8d0","geometry":"ticks","has_range":true,"reward_token":null}}]}"#,
    );
    let e = &p.positions[0];
    assert!(e.summary.pnl_usd.is_known());
    assert_eq!(e.summary.pnl_usd.value.as_deref(), Some("8.00"));
    assert!(!e.summary.yield_usd.is_known());
    assert_eq!(e.summary.yield_usd.absence.as_ref().unwrap().kind.as_deref(), Some("unpriced"));
    assert_eq!(e.detail.geometry.as_deref(), Some("ticks"));
}

#[tokio::test]
async fn the_registry_answers_what_a_venue_can_do() {
    let (base, h) = one_shot(
        r#"{"schema":1,"version":"1.10.2","revised":"2026-11-14","supports":["supported","unsupported"],"chains":[{"id":"bsc","is_evm":true,"evm_chain_id":56}],
        "kinds":[{"id":"lp"}],"capabilities":[{"id":"auto_compound"}],
        "venues":[{"id":"pancakeswap_infinity_bin","chains":["bsc"],"reward_token":"CAKE","geometry":{"range_unit":"ticks"},
        "capabilities":{"bsc":{"auto_compound":"unsupported","auto_claim_fees":"supported"}}}]}"#,
    );
    let r: Registry = Client::public(&base).unwrap().registry().await.unwrap();
    assert_eq!(request_line(&h.join().unwrap()), "GET /public/registry HTTP/1.1");
    assert!(r.supports_on("pancakeswap_infinity_bin", "bsc", "auto_claim_fees"));
    assert!(!r.supports_on("pancakeswap_infinity_bin", "bsc", "auto_compound"));
    assert!(!r.supports_on("pancakeswap_infinity_bin", "base", "auto_claim_fees"));
    assert_eq!(r.chains[0].evm_chain_id, Some(56));
}

#[tokio::test]
async fn wallet_history_reads_any_wallet() {
    let (base, h) = one_shot(
        r#"{"range":"7d","bucket_secs":3600,"since":"x","until":"y","points":[{"t":"x","value_usd":1.5},{"t":"y","value_usd":null}],"incomplete_buckets":1,"positions":2,"first_record_at":null}"#,
    );
    let w: WalletHistory = Client::public(&base).unwrap().wallet_history("GYdJ", Some("7d")).await.unwrap();
    assert_eq!(request_line(&h.join().unwrap()), "GET /public/wallet/GYdJ/history?range=7d HTTP/1.1");
    assert_eq!((w.positions, w.points[1].value_usd), (Some(2), None));
}

#[test]
fn a_solana_agent_reads_its_farming_rewards() {
    let p: AgentPerformance = decode(
        r#"{"rewards_claimable_usd":null,"rewards":[{"mint":"orca","amount":"1.5","usd":null}],
        "farm_rewards":{"pending_usd":null,"pending_unavailable":"unpriced","claimed_usd":"3.10","claimed_unavailable":null,"fee_booked_usd":"0.31",
        "mints":[{"mint":"orca","pending_amount":"1.5","pending_usd":null,"claimed_amount":null,"claimed_amount_unavailable":"fee_converted_to_sol","claimed_usd":"3.10","fee_booked_amount":null,"fee_booked_usd":"0.31","claims":2}]},
        "vs_hodl_scope":"current_position"}"#,
    );
    assert_eq!(p.rewards[0].amount.as_deref(), Some("1.5"));
    let f = p.farm_rewards.unwrap();
    assert_eq!(f.pending_unavailable.as_deref(), Some("unpriced"));
    assert_eq!(f.mints[0].claims, Some(2));
    assert_eq!(p.vs_hodl_scope.as_deref(), Some("current_position"));
}

#[test]
fn feeds_follows_and_the_tracker_carry_their_new_fields() {
    let f: SocialFeed = decode(
        r#"{"rows":[{"id":7,"kind":"agent_open","verb":"Opened","actor":{"key":"alex"},"token_x":{"symbol":"SOL","logo":"/token-icon/x.webp"},"token_y":{"symbol":"USDC","logo":null}}],"next_cursor":null}"#,
    );
    assert_eq!(f.rows.as_ref().unwrap()[0].token_x.as_ref().unwrap().symbol.as_deref(), Some("SOL"));
    let a: AgentsActivity = decode(r#"{"events":[{"key":"k","kind":"hold","token_x":{"symbol":"SOL"}}],"holding":4,"checks":357}"#);
    assert_eq!(a.checks, Some(357));
    let s: FollowState = decode(r#"{"key":"alex","following":true,"followers":12,"following_count":3,"viewer":{"key":"me","followers":1,"following":9}}"#);
    let v = s.viewer.unwrap();
    assert_eq!((s.following_count, v.person.key.as_deref(), v.following), (Some(3), Some("me"), Some(9)));
    let d: XDirectory = decode(r#"{"accounts":[{"handle":"a","readState":"unreadable","lastOkAt":null}],"hiddenCategories":[],"categories":[]}"#);
    assert_eq!(d.accounts[0].read_state.as_deref(), Some("unreadable"));
    let r: LeaderboardRow = decode(r#"{"slug":"s","spark":[1.0,null],"spark_from":"a","spark_until":"b"}"#);
    assert_eq!(r.spark_until.as_deref(), Some("b"));
}

#[test]
fn infinity_bin_is_a_bnb_chain_range_venue() {
    assert_eq!(ponk::evm::venue_chains("pancakeswap_infinity_bin"), ["bsc"]);
    assert!(!ponk::evm::is_full_range_venue("pancakeswap_infinity_bin"));
}
