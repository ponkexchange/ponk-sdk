"""0.4.0 routes: the leaderboard, sharing, follows, points and referrals.

Same stub-opener approach as test_client.py: pin the method, the path, the
body and the parsed fields, offline.
"""

from __future__ import annotations

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from ponk import (  # noqa: E402
    Leaderboard,
    PonkConflictError,
    PonkNotFoundError,
    PonkUnprocessableError,
)
from tests.test_client import AGENT, StubOpener, client  # noqa: E402
from ponk import PonkClient  # noqa: E402

BASE = "https://example.test/api"

COPY = {
    "dex": "meteora_dlmm",
    "pool_address": "POOL",
    "strategy": "bin_rebalancer",
    "config": {"kind": "bin_rebalancer", "target_bin_width": 11},
}

ROW = {
    "rank": 1,
    "slug": "abc123",
    "dex": "meteora_dlmm",
    "strategy": "bin_rebalancer",
    "pool_address": "POOL",
    "vs_hodl_pct": 4.2,
    "pnl_usd": "12.34",
    "fees_earned_usd": None,
    "spark": [1.0, None, 2.0],
    "followers": 3,
    "copy": COPY,
    "future_field": "kept in raw_json",
}


class SequenceOpener(StubOpener):
    """Replays one payload per call, in order."""

    def __init__(self, payloads):
        super().__init__(payloads[0])
        self.payloads = list(payloads)

    def open(self, request, timeout=None):
        self.payload = self.payloads[len(self.calls)]
        return super().open(request, timeout)


class TestPublicReads(unittest.TestCase):
    def test_leaderboard_parses_rows_and_keeps_unknown_fields(self):
        c, opener = client({"ranked": [ROW], "unranked": [], "window": "7d", "totals": {"shared": 9}})
        board = c.leaderboard(window="7d")
        self.assertIsInstance(board, Leaderboard)
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/agents-leaderboard?window=7d")
        row = board.ranked[0]
        self.assertEqual(row.rank, 1)
        self.assertEqual(row.pnl_usd, "12.34")
        self.assertIsNone(row.fees_earned_usd)
        self.assertEqual(row.copy.strategy, "bin_rebalancer")
        self.assertEqual(row.raw_json["future_field"], "kept in raw_json")
        self.assertEqual(board.totals, {"shared": 9})

    def test_leaderboard_without_window_sends_no_query(self):
        c, opener = client({"ranked": [], "unranked": []})
        c.leaderboard()
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/agents-leaderboard")

    def test_public_agent_and_value_history_paths(self):
        c, opener = client({"slug": "abc123", "copy": COPY, "followers": 2})
        agent = c.public_agent("abc123")
        self.assertEqual(agent.copy.pool_address, "POOL")
        c.public_agent_value_history("abc123", range="30d")
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/agents/abc123")
        self.assertEqual(opener.calls[1]["url"], BASE + "/public/agents/abc123/value-history?range=30d")

    def test_public_points_board_and_referral_resolve(self):
        c, opener = client({"season": None, "participant_count": 0, "entries": []})
        board = c.public_points_leaderboard(limit=10)
        self.assertIsNone(board.season)
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/points/leaderboard?limit=10")
        c, opener = client({"code": "kader", "creator": True})
        self.assertTrue(c.resolve_referral("kader").creator)
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/referral/kader")

    def test_an_unknown_referral_code_raises_not_found(self):
        c, _ = client({"error": {"code": "not_found", "message": "not found"}}, status=404)
        with self.assertRaises(PonkNotFoundError):
            c.resolve_referral("nobody")


class TestAccountReads(unittest.TestCase):
    def test_value_history_and_decisions(self):
        c, opener = client({"range": "24h", "points": [None]})
        h = c.agent_value_history("a", range="24h")
        self.assertEqual(h.points, [None])
        c.agent_decisions("a", hours=48, limit=10)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/a/value-history?range=24h")
        self.assertEqual(opener.calls[1]["url"], BASE + "/v1/agents/a/decisions?hours=48&limit=10")

    def test_points_and_points_board(self):
        c, opener = client(
            {
                "season": {"id": "s1", "name": "Season 1", "starts_at": "2026-10-21", "multiplier": "1"},
                "total_points": "1200",
                "rank": 4,
                "participant_count": 50,
            }
        )
        me = c.points()
        self.assertEqual(me.season.name, "Season 1")
        self.assertEqual(me.total_points, "1200")
        c.points_leaderboard(limit=5, offset=5)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/points")
        self.assertEqual(opener.calls[1]["url"], BASE + "/v1/points/leaderboard?limit=5&offset=5")

    def test_referral_keeps_three_figures_apart(self):
        c, opener = client(
            {"code": "kader", "codes": [{"code": "kader"}], "total_earned_usd": 3.0, "owed_usd": 1.5, "unproven_usd": 0.0}
        )
        r = c.referral()
        self.assertEqual((r.total_earned_usd, r.owed_usd, r.unproven_usd), (3.0, 1.5, 0.0))
        self.assertEqual(r.codes[0].code, "kader")
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/referral")

    def test_creator_dashboard_and_handle_check(self):
        c, opener = client({"handle": "kader", "totals": {"earned_usd": 2.5}, "days": [{"day": "2026-10-21", "signups": 1}]})
        d = c.creator_dashboard(days=14)
        self.assertEqual(d.totals.earned_usd, 2.5)
        self.assertEqual(d.days[0].signups, 1)
        c.check_handle("kader")
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/referral/creator?days=14")
        self.assertEqual(opener.calls[1]["url"], BASE + "/v1/referral/handle/check?handle=kader")

    def test_no_creator_tier_is_not_found(self):
        c, _ = client({"error": {"code": "not_found", "message": "not found"}}, status=404)
        with self.assertRaises(PonkNotFoundError):
            c.creator_dashboard()


class TestWrites(unittest.TestCase):
    def test_share_sends_only_what_was_asked(self):
        c, opener = client({"slug": "abc123", "path": "/a/abc123", "hide_usd": True, "show_onchain": False})
        link = c.share_agent("a", hide_usd=True)
        self.assertEqual(link.path, "/a/abc123")
        self.assertEqual(opener.calls[0]["method"], "POST")
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/a/public-link")
        self.assertEqual(opener.calls[0]["body"], {"hide_usd": True})
        c.share_agent("a", show_onchain=True)
        self.assertEqual(opener.calls[1]["body"], {"hide_usd": False, "show_onchain": True})

    def test_unshare_and_get_link(self):
        c, opener = client({"revoked": True})
        self.assertTrue(c.unshare_agent("a").revoked)
        self.assertEqual(opener.calls[0]["method"], "DELETE")
        c.get_public_link("a")
        self.assertEqual(opener.calls[1]["method"], "GET")
        self.assertEqual(opener.calls[1]["url"], BASE + "/v1/agents/a/public-link")

    def test_follow_unfollow_and_list(self):
        c, opener = client({"slug": "abc123", "following": True, "followers": 4})
        self.assertEqual(c.follow("abc123").followers, 4)
        c.unfollow("abc123")
        self.assertEqual((opener.calls[0]["method"], opener.calls[0]["url"]), ("POST", BASE + "/v1/follows/abc123"))
        self.assertEqual((opener.calls[1]["method"], opener.calls[1]["url"]), ("DELETE", BASE + "/v1/follows/abc123"))
        c, opener = client({"slugs": ["abc123"]})
        self.assertEqual(c.list_follows().slugs, ["abc123"])

    def test_following_your_own_agent_is_refused(self):
        c, _ = client({"error": {"code": "unprocessable", "message": "This is your own agent."}}, status=422)
        with self.assertRaises(PonkUnprocessableError):
            c.follow("mine")

    def test_claim_handle_and_a_taken_handle(self):
        c, opener = client({"code": "kader", "handle": "kader", "creator_handle": "kader"})
        self.assertEqual(c.claim_handle("kader").creator_handle, "kader")
        self.assertEqual(opener.calls[0]["body"], {"handle": "kader"})
        c, _ = client({"error": {"code": "conflict", "message": "taken"}}, status=409)
        with self.assertRaises(PonkConflictError):
            c.claim_handle("taken")

    def test_copy_agent_reads_the_profile_then_creates_from_its_copy_block(self):
        opener = SequenceOpener([{"slug": "abc123", "copy": COPY}, AGENT])
        c = PonkClient(api_key="ponk_live_test", base_url=BASE, opener=opener)
        c.copy_agent("abc123", name="copy", wallet_address="GYdJ", dry_run=False,
                     config_overrides={"cooldown_sec": 600})
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/agents/abc123")
        create = opener.calls[1]
        self.assertEqual((create["method"], create["url"]), ("POST", BASE + "/v1/agents"))
        self.assertEqual(create["body"]["strategy"], "bin_rebalancer")
        self.assertEqual(create["body"]["pool_address"], "POOL")
        self.assertEqual(create["body"]["dry_run"], False)
        self.assertEqual(create["body"]["config"]["target_bin_width"], 11)
        self.assertEqual(create["body"]["config"]["cooldown_sec"], 600)

    def test_copy_agent_from_a_leaderboard_row_needs_no_second_read(self):
        opener = SequenceOpener([{"ranked": [ROW], "unranked": []}, AGENT])
        c = PonkClient(api_key="ponk_live_test", base_url=BASE, opener=opener)
        row = c.leaderboard().ranked[0]
        c.copy_agent(row, name="copy", wallet_address="GYdJ", dry_run=True)
        self.assertEqual(len(opener.calls), 2)
        self.assertEqual(opener.calls[1]["body"]["dry_run"], True)

    def test_optimize_draft_posts_the_draft(self):
        c, opener = client({"candidates": []})
        c.optimize_draft({"dex": "meteora_dlmm", "pool_address": "POOL"})
        self.assertEqual((opener.calls[0]["method"], opener.calls[0]["url"]), ("POST", BASE + "/v1/agents/optimize-draft"))


if __name__ == "__main__":
    unittest.main()
