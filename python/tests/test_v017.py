"""0.17.0: collect all, one position shape, the registry, any wallet's value
line, farming rewards on Solana agents, PancakeSwap Infinity Bin, and the
smaller additions on the feeds, the X Tracker, follows and the leaderboard.

Same stub-opener approach as test_client.py: no test here opens a socket. The
payloads are written field for field from the API's own Serialize structs
(`BatchView` and `BatchItem`, `AgentPositionsView` with `PositionEnvelope`
and `Observed`, the public registry document, `PublicWalletHistoryView`,
`AgentFarmRewards` and `AgentRewardLine`, `FollowState` with
`ViewerCounts`, the X Tracker's `DirectoryEntry`, `FeedRow` with
`TokenLabel`), with illustrative values.
"""

from __future__ import annotations

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import ponk  # noqa: E402
from ponk import (  # noqa: E402
    AgentPerformance,
    AgentPositions,
    AgentsActivity,
    FleetBatch,
    FollowState,
    PonkClient,
    Registry,
    SocialFeed,
    WalletHistory,
    XDirectory,
    evm,
)
from tests.test_client import StubOpener, client  # noqa: E402

BASE = "https://example.test/api"
AGENT = "0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b"
AGENT2 = "0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2c"
WALLET = "GYdJfRSHhDAVVEz8optL8aGSQKd5daSZ9Ao3sVLZRkEe"
USDC = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
SOL = "So11111111111111111111111111111111111111112"


def keyless(payload, status=200):
    opener = StubOpener(payload, status)
    return PonkClient(base_url=BASE, opener=opener), opener


class TestVersion(unittest.TestCase):
    def test_version_is_0_17_1(self):
        self.assertEqual(ponk.__version__, "0.17.1")


CLAIM_BATCH = {
    "batch_id": "6f1d2c3b-4a59-4e6f-8a7b-9c0d1e2f3a4b",
    "kind": "claim",
    "total": 3,
    "done": 1,
    "failed": 0,
    "running": 0,
    "unknown": 1,
    "skipped": 1,
    "items": [
        {"index": 0, "agent_id": AGENT, "outcome": "done", "ok": True, "status": 200, "code": None,
         "error": None, "result": {"action_status": "confirmed", "signature": "5sig"}},
        {"index": 1, "agent_id": AGENT2, "outcome": "unknown", "ok": False, "status": 200,
         "code": "outcome_unknown",
         "error": "The claim was sent and its landing is not confirmed yet. Do not claim again: check this signature first. 4sig",
         "result": {"action_status": "submitted", "signature": "4sig"}},
        {"index": 2, "agent_id": "0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2d", "outcome": "skipped", "ok": False,
         "status": 422, "code": "nothing_to_collect",
         "error": "Ponk Clouds fees grow the position itself, so there is nothing to collect. Nothing was sent.",
         "result": None},
    ],
}


class TestClaimAgents(unittest.TestCase):
    def test_named_agents_go_to_the_owner_by_default(self):
        c, opener = client(CLAIM_BATCH)
        b = c.claim_agents([AGENT, AGENT2])
        call = opener.calls[0]
        self.assertEqual(call["method"], "POST")
        self.assertTrue(call["url"].endswith("/v1/agents/batch/claim"))
        self.assertEqual(call["body"], {"agent_ids": [AGENT, AGENT2]})
        self.assertEqual(call["timeout"], c.fund_timeout)
        self.assertIsInstance(b, FleetBatch)
        self.assertEqual((b.kind, b.total, b.done, b.unknown, b.skipped), ("claim", 3, 1, 1, 1))
        self.assertEqual([i.outcome for i in b.items], ["done", "unknown", "skipped"])
        self.assertEqual(b.items[2].code, "nothing_to_collect")

    def test_all_and_destination(self):
        c, opener = client(CLAIM_BATCH)
        c.claim_agents(all_agents=True, destination="agent")
        self.assertEqual(opener.calls[0]["body"], {"all": True, "destination": "agent"})

    def test_refusals_before_a_request(self):
        c, opener = client(CLAIM_BATCH)
        with self.assertRaises(ValueError):
            c.claim_agents([AGENT], all_agents=True)
        with self.assertRaises(ValueError):
            c.claim_agents()
        with self.assertRaises(ValueError):
            c.claim_agents([AGENT], destination="elsewhere")
        with self.assertRaises(TypeError):
            c.claim_agents(AGENT)
        self.assertEqual(opener.calls, [])


POSITIONS = {
    "agent_id": AGENT,
    "kind": "lp",
    "positions": [
        {
            "kind": "lp",
            "venue": "uniswap_v3",
            "chain": "arbitrum",
            "id": "eip155:42161:0xc36442b4a4522e871399cd717abdd847ab11fe88:41000",
            "summary": {
                "status": "open",
                "value_usd": {"state": "known", "value": "608.00"},
                "cost_basis_usd": {"state": "known", "value": "600.00"},
                "pnl_usd": {"state": "known", "value": "8.00"},
                "yield_usd": {"state": "absent", "absence": {"kind": "unpriced", "why": "The position is open and this figure could not be priced in USD on this read."}},
            },
            "detail": {"kind": "lp", "pool": "eip155:42161:0xc6962004f452be9203591991d15f6b388e09e8d0",
                       "geometry": "ticks", "has_range": True, "reward_token": None},
        }
    ],
}


class TestAgentPositions(unittest.TestCase):
    def test_reads_the_envelope(self):
        c, opener = client(POSITIONS)
        p = c.agent_positions(AGENT)
        self.assertEqual(opener.calls[0]["method"], "GET")
        self.assertTrue(opener.calls[0]["url"].endswith("/v1/agents/{0}/positions".format(AGENT)))
        self.assertIsInstance(p, AgentPositions)
        e = p.positions[0]
        self.assertEqual((e.venue, e.chain, e.detail.geometry), ("uniswap_v3", "arbitrum", "ticks"))
        self.assertTrue(e.summary.pnl_usd.known)
        self.assertEqual(e.summary.pnl_usd.value, "8.00")
        self.assertFalse(e.summary.yield_usd.known)
        self.assertEqual(e.summary.yield_usd.absence.kind, "unpriced")
        self.assertIsNone(e.summary.yield_usd.value)


class TestAgentAction(unittest.TestCase):
    def test_posts_the_named_action(self):
        c, opener = client({"action_status": "confirmed", "signature": "5sig"})
        r = c.agent_action(AGENT, "claim", {"destination": "connected"})
        call = opener.calls[0]
        self.assertTrue(call["url"].endswith("/v1/agents/{0}/actions/claim".format(AGENT)))
        self.assertEqual(call["body"], {"destination": "connected"})
        self.assertEqual(r["signature"], "5sig")

    def test_an_unknown_action_is_refused_locally(self):
        c, opener = client({})
        with self.assertRaises(ValueError):
            c.agent_action(AGENT, "liquidate")
        self.assertEqual(opener.calls, [])


REGISTRY = {
    "schema": 1,
    "version": "1.10.2",
    "evm_version": "1.2.0",
    "revised": "2026-11-14",
    "supports": ["supported", "supported_no_ui", "unsupported", "inert", "unknown"],
    "capabilities": [{"id": "auto_compound", "label": "Compound unattended"}],
    "kinds": [{"id": "lp", "label": "LP", "status": "live", "actions": [
        {"id": "claim", "action": "claim_fees", "capability": "manual_claim_fees_managed", "destructive": False, "label": "Claim fees"}]}],
    "chains": [{"id": "bsc", "label": "BNB Chain", "is_evm": True, "evm_chain_id": 56, "native_symbol": "BNB",
                "native_decimals": 18, "explorer": "https://bscscan.com", "status": "live"}],
    "venues": [
        {"id": "pancakeswap_infinity_bin", "label": "PancakeSwap Infinity Bin", "short": "Infinity Bin",
         "platform": "PancakeSwap", "family": "pancake_swap", "kind": "lp", "chains": ["bsc"], "is_evm": True,
         "gated": False, "farm": "infinity_merkle", "reward_token": "CAKE", "fees_compound_in_place": True,
         "geometry": {"has_position_range": True, "range_unit": "ticks", "max_bins_per_open": None, "max_position_bins": None},
         "capabilities": {"bsc": {"auto_compound": "unsupported", "auto_claim_fees": "supported",
                                  "autonomous_rebalance": "supported"}}}
    ],
}


class TestRegistry(unittest.TestCase):
    def test_reads_without_a_key(self):
        c, opener = keyless(REGISTRY)
        r = c.registry()
        self.assertTrue(opener.calls[0]["url"].endswith("/public/registry"))
        self.assertNotIn("X-API-Key", opener.calls[0]["headers"])
        self.assertIsInstance(r, Registry)
        self.assertEqual(r.version, "1.10.2")
        self.assertEqual(r.chains[0].evm_chain_id, 56)
        self.assertTrue(r.supports_on("pancakeswap_infinity_bin", "bsc", "autonomous_rebalance"))
        self.assertFalse(r.supports_on("pancakeswap_infinity_bin", "bsc", "auto_compound"))
        self.assertFalse(r.supports_on("pancakeswap_infinity_bin", "base", "auto_claim_fees"))
        self.assertIsNone(r.venue("nowhere"))
        self.assertEqual(r.venue("pancakeswap_infinity_bin").reward_token, "CAKE")


class TestWalletHistory(unittest.TestCase):
    def test_reads_any_wallet(self):
        body = {"range": "7d", "bucket_secs": 3600, "since": "2026-11-10T00:00:00Z", "until": "2026-11-17T00:00:00Z",
                "points": [{"t": "2026-11-16T00:00:00Z", "value_usd": 120.5}, {"t": "2026-11-16T01:00:00Z", "value_usd": None}],
                "incomplete_buckets": 1, "positions": 2, "first_record_at": "2026-10-01T00:00:00Z"}
        c, opener = keyless(body)
        h = c.wallet_history(WALLET, "7d")
        self.assertTrue(opener.calls[0]["url"].endswith("/public/wallet/{0}/history?range=7d".format(WALLET)))
        self.assertIsInstance(h, WalletHistory)
        self.assertEqual((h.positions, h.incomplete_buckets), (2, 1))
        self.assertIsNone(h.points[1].value_usd)


class TestSolanaFarmRewards(unittest.TestCase):
    def test_performance_reads_the_reward_lines_and_block(self):
        body = {
            "current_value_usd": "500.00",
            "rewards_claimable_usd": None,
            "rewards": [{"mint": "orcaEKTdK7LKz57vaAYr9QeNsVEPfiu6QeMU1kektZE", "amount": "1.5", "usd": None}],
            "farm_rewards": {
                "pending_usd": None, "pending_unavailable": "unpriced",
                "claimed_usd": "3.10", "claimed_unavailable": None, "fee_booked_usd": "0.31",
                "mints": [{"mint": "orcaEKTdK7LKz57vaAYr9QeNsVEPfiu6QeMU1kektZE", "pending_amount": "1.5",
                           "pending_usd": None, "claimed_amount": None,
                           "claimed_amount_unavailable": "fee_converted_to_sol", "claimed_usd": "3.10",
                           "fee_booked_amount": None, "fee_booked_usd": "0.31", "claims": 2}],
            },
            "vs_hodl_scope": "current_position",
            "vs_hodl_unavailable": None,
        }
        c, _ = client(body)
        p = c.agent_performance(AGENT)
        self.assertIsInstance(p, AgentPerformance)
        self.assertIsNone(p.rewards_claimable_usd)
        self.assertEqual(p.rewards[0].amount, "1.5")
        self.assertEqual(p.farm_rewards.pending_unavailable, "unpriced")
        self.assertEqual(p.farm_rewards.claimed_usd, "3.10")
        m = p.farm_rewards.mints[0]
        self.assertEqual((m.claims, m.claimed_amount_unavailable), (2, "fee_converted_to_sol"))
        self.assertEqual(p.vs_hodl_scope, "current_position")


class TestInfinityBin(unittest.TestCase):
    def test_is_a_bnb_chain_range_venue(self):
        self.assertEqual(evm.venue_chains("pancakeswap_infinity_bin"), ("bsc",))
        self.assertFalse(evm.is_full_range_venue("pancakeswap_infinity_bin"))
        self.assertIsNone(evm.stake_reward_token("pancakeswap_infinity_bin"))
        self.assertFalse(evm.is_shape_venue("pancakeswap_infinity_bin", "bsc"))

    def test_create_takes_a_range(self):
        pool = "eip155:56:0xc697d2898e0d09264376196696c51d7abbbaa4a9:0xa859b22e97f32d4c7b1d9788044697712cb7183d294ed7aa1832799c1739e5cf"
        c, opener = client({"id": AGENT, "dex": "pancakeswap_infinity_bin", "chain": "bsc", "kind": "lp"})
        a = c.create_evm_agent(name="bin", wallet_address=WALLET, dry_run=False, chain="bsc",
                               dex="pancakeswap_infinity_bin", pool_address=pool,
                               down_pct=0.05, up_pct=0.05)
        body = opener.calls[0]["body"]
        self.assertEqual((body["dex"], body["chain"], body["strategy"]), ("pancakeswap_infinity_bin", "bsc", "bin_rebalancer"))
        self.assertEqual(a.kind, "lp")


class TestFeedsAndPeople(unittest.TestCase):
    def test_activity_rows_carry_token_labels(self):
        body = {"rows": [{"id": 7, "kind": "agent_open", "verb": "Opened", "actor": {"key": "alex"},
                          "token_x_mint": SOL, "token_y_mint": USDC,
                          "token_x": {"symbol": "SOL", "logo": "/token-icon/{0}.webp".format(SOL)},
                          "token_y": {"symbol": "USDC", "logo": None}}],
                "next_cursor": None}
        c, _ = keyless(body)
        f = c.live_feed()
        self.assertIsInstance(f, SocialFeed)
        self.assertEqual((f.rows[0].token_x.symbol, f.rows[0].token_y.logo), ("SOL", None))

    def test_agents_activity_checks(self):
        body = {"events": [{"key": "a1", "kind": "hold", "token_x": {"symbol": "SOL", "logo": None}, "token_y": None}],
                "holding": 4, "checks": 357, "computed_at": "2026-11-17T00:00:00Z"}
        c, _ = keyless(body)
        a = c.agents_activity()
        self.assertIsInstance(a, AgentsActivity)
        self.assertEqual(a.checks, 357)
        self.assertEqual(a.events[0].token_x.symbol, "SOL")

    def test_follow_state_counts(self):
        body = {"key": "alex", "following": True, "followers": 12, "notify_telegram": False, "notify_push": False,
                "muted": False, "private": False, "following_count": 3,
                "viewer": {"key": "me", "followers": 1, "following": 9}}
        c, _ = client(body)
        s = c.follow_person("alex")
        self.assertIsInstance(s, FollowState)
        self.assertEqual((s.following_count, s.viewer.key, s.viewer.following), (3, "me", 9))

    def test_x_directory_read_state(self):
        body = {"accounts": [{"handle": "a", "screenName": "a", "status": "active", "readState": "unreadable",
                              "lastOkAt": None, "isDefault": True, "followed": False, "muted": False, "alerting": False}],
                "hiddenCategories": [], "categories": []}
        c, _ = keyless(body)
        d = c.x_accounts()
        self.assertIsInstance(d, XDirectory)
        self.assertEqual(d.accounts[0].read_state, "unreadable")

    def test_leaderboard_spark_span(self):
        row = ponk.LeaderboardRow.from_dict({"slug": "s", "spark": [1.0, None, 1.2],
                                            "spark_from": "2026-11-10T00:00:00Z", "spark_until": "2026-11-12T00:00:00Z"})
        self.assertEqual(row.spark_until, "2026-11-12T00:00:00Z")


if __name__ == "__main__":
    unittest.main()


class TestImpersonates(unittest.TestCase):
    def test_a_look_alike_token_carries_the_real_one(self):
        from ponk.models import EvmToken
        t = EvmToken.from_dict({"symbol": "BNB", "impersonates": {"symbol": "BNB", "address": "0x0000000000000000000000000000000000000000", "native": True}})
        self.assertEqual(t.impersonates["symbol"], "BNB")
        self.assertTrue(t.impersonates["native"])
        self.assertIsNone(EvmToken.from_dict({"symbol": "CAKE"}).impersonates)
