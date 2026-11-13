//! Each fixture is one response body, transcribed field for field from the
//! Serialize struct the server builds it from (named beside each test), in
//! declaration order. The values are illustrative; the field names, types and
//! which fields can be null are the server's, so a model that drifts from the
//! wire fails here rather than in someone's integration.

use ponk::{
    Agent, DlmmPoolPage, Leaderboard, OppBoard, PoolBoard, PoolSnapshot, PublicAgent,
    ValueHistory, WalletPnl, WhoAmI, Withdrawal,
};

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("could not read {path}: {e}"))
}

/// apps/api/src/routes/v1.rs `WhoAmIView`.
#[test]
fn whoami_decodes() {
    let me: WhoAmI = serde_json::from_str(&fixture("whoami.json")).unwrap();
    assert_eq!(me.scope.as_deref(), Some("trade"));
    assert!(me.wallet_address.as_deref().unwrap().starts_with("GYdJ"));
}

/// apps/api/src/routes/agents.rs `AgentView`, as a create response (the only
/// place `pool_warning` is set).
#[test]
fn an_agent_decodes_with_the_create_warning_and_fees() {
    let a: Agent = serde_json::from_str(&fixture("agent.json")).unwrap();
    assert_eq!(a.dex.as_deref(), Some("meteora_dlmm"));
    assert_eq!(a.dry_run, Some(false));
    assert_eq!(a.origin.as_deref(), Some("api"));
    assert_eq!(a.managed_fee_bps, Some(1500));
    assert_eq!(a.activation_fee_bps, Some(100));
    // The deploy gate warns, it never refuses: the warning rides the agent.
    assert!(a.pool_warning.as_deref().unwrap().contains("$412"));
    assert!(a.waiting.is_none() || a.waiting == Some(serde_json::Value::Null));
}

/// apps/api/src/routes/agent_public.rs `LeaderboardView` / `LeaderboardRow`.
#[test]
fn the_leaderboard_decodes_with_usd_as_strings() {
    let b: Leaderboard = serde_json::from_str(&fixture("leaderboard.json")).unwrap();
    assert_eq!(b.window.as_deref(), Some("7d"));
    let r = &b.ranked[0];
    assert_eq!(r.rank, Some(1));
    // Money arrives as decimal strings, never floats.
    assert_eq!(r.pnl_usd.as_deref(), Some("12.41"));
    assert_eq!(r.fees_earned_usd.as_deref(), Some("44.90"));
    // A spark gap is null, not zero.
    assert_eq!(r.spark[2], None);
    assert_eq!(r.copy.strategy.as_deref(), Some("bin_rebalancer"));
}

/// apps/api/src/routes/agent_public.rs `LeaderboardView` under
/// `?chains=bsc`: one BNB Chain row, ranked from 1, the totals of that row,
/// and `chains` echoing the filter (0.16.0).
#[test]
fn the_leaderboard_decodes_a_bsc_row_under_a_chain_filter() {
    let b: Leaderboard = serde_json::from_str(&fixture("leaderboard_bsc.json")).unwrap();
    assert_eq!(b.chains, ["bsc"]);
    let r = &b.ranked[0];
    assert_eq!((r.rank, r.chain.as_deref(), r.dex.as_deref()), (Some(1), Some("bsc"), Some("pancakeswap_v3")));
    // On an EVM row the pool and both tokens are CAIP ids.
    for id in [&r.pool_address, &r.token_x_mint, &r.token_y_mint] {
        assert_eq!(ponk::evm::parse_evm_id(id.as_deref().unwrap()).unwrap().chain, "bsc");
    }
    assert_eq!(b.totals.unwrap()["pnl_usd"], "6.20");
    assert_eq!(r.pnl_usd.as_deref(), Some("6.20"));
}

/// apps/api/src/routes/agent_public.rs `PublicAgentView`.
#[test]
fn a_public_agent_decodes_across_venues() {
    let a: PublicAgent = serde_json::from_str(&fixture("public_agent.json")).unwrap();
    assert_eq!(a.dex.as_deref(), Some("orca"));
    assert_eq!(a.followers, Some(4));
    let perf = a.performance.unwrap();
    assert_eq!(perf.get("pnl_usd").and_then(|v| v.as_str()), Some("12.41"));
    assert_eq!(a.performance_unavailable, None);
}

/// apps/api/src/routes/agent_value_history.rs `ValueHistoryView`.
#[test]
fn value_history_keeps_gaps_as_nulls() {
    let h: ValueHistory = serde_json::from_str(&fixture("value_history.json")).unwrap();
    assert_eq!(h.range.as_deref(), Some("7d"));
    assert_eq!(h.points.len(), 3);
    // Bucket 2 has no reading; it decodes as null, never interpolated.
    assert!(h.points[1].get("value_usd").unwrap().is_null());
}

/// apps/api/src/routes/wallet_tracker.rs `PnlResponse` (camelCase).
#[test]
fn wallet_pnl_decodes() {
    let p: WalletPnl = serde_json::from_str(&fixture("wallet_pnl.json")).unwrap();
    assert_eq!(p.sol_usd, Some(151.4));
    assert_eq!(p.balances_read, Some(true));
    assert_eq!(p.transactions_read, Some(412));
    assert_eq!(p.tokens.as_ref().unwrap().len(), 1);
}

/// apps/api/src/routes/pools.rs `PoolView`: a flattened `PoolSnapshot` plus
/// the token price the exit triggers compare against. The raydium row proves
/// the same shape covers the tick venues, not only Meteora bins.
#[test]
fn a_pool_snapshot_decodes_with_its_token_price() {
    let p: PoolSnapshot = serde_json::from_str(&fixture("pool.json")).unwrap();
    assert_eq!(p.dex.as_deref(), Some("raydium"));
    // current_price is a RAW decimal string; human_price adjusts decimals.
    let human = p.human_price().unwrap();
    assert!((human - 150.2).abs() < 1e-9, "{human}");
    assert_eq!(p.token_price, Some(150.2));
    assert_eq!(p.tvl_usd.as_deref(), Some("1250000.00"));
}

/// apps/api/src/routes/custody.rs withdraw response.
#[test]
fn a_withdrawal_decodes() {
    let w: Withdrawal = serde_json::from_str(&fixture("withdrawal.json")).unwrap();
    assert_eq!(w.lamports, Some(1_500_000_000));
    assert!(w.signature.is_some());
    assert!(w.destination.as_deref().unwrap().starts_with("GYdJ"));
}

/// apps/web/app/(trade)/pools/board/route.ts, rows shaped by `toBoardPool`
/// (lib/dex/meteoraBoard.server.ts).
#[test]
fn the_pool_board_decodes() {
    let b: PoolBoard = serde_json::from_str(&fixture("board.json")).unwrap();
    let p = &b.pools[0];
    assert_eq!(p.bin_step, Some(10));
    assert_eq!(p.fees_24h_usd, Some(812.4));
    assert_eq!(p.token_x.as_ref().unwrap().verified, Some(true));
}

/// apps/web/app/(trade)/discover/feed/route.ts, the `OppFeed` shape in
/// apps/web/lib/opportunities.ts. One Meteora row and one Raydium row, so all
/// of the cross-venue board's venue spellings are pinned.
#[test]
fn the_discover_feed_decodes_across_venues() {
    let f: OppBoard = serde_json::from_str(&fixture("opportunities.json")).unwrap();
    assert_eq!(f.pools.len(), 2);
    assert_eq!(f.pools[0].dex.as_deref(), Some("meteora_dlmm"));
    assert_eq!(f.pools[1].dex.as_deref(), Some("raydium"));
    // A window the venue does not publish is null inside the map.
    assert_eq!(f.pools[1].volume.get("1h"), Some(&None));
    assert_eq!(f.pools[1].volume.get("24h"), Some(&Some(510000.0)));
    let ok = f.ok.unwrap();
    assert_eq!((ok.meteora, ok.orca, ok.raydium), (Some(true), Some(true), Some(true)));
}

/// dex/services/api/src/routes/dlmm.rs `list`, enveloped as `{"data": ...}`
/// with `DlmmPoolPage` (dex/crates/core-types) inside.
#[test]
fn the_dlmm_pool_page_decodes_from_its_envelope() {
    #[derive(serde::Deserialize)]
    struct Envelope {
        data: DlmmPoolPage,
    }
    let e: Envelope = serde_json::from_str(&fixture("dlmm_pools.json")).unwrap();
    assert_eq!(e.data.total, Some(5000));
    let p = &e.data.pools[0];
    assert_eq!(p.bin_step, Some(10));
    // The venue's apr is percent per DAY and decodes untouched.
    assert_eq!(p.apr, Some(0.065));
}
