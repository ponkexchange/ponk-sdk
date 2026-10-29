"""0.10.0: people (profiles, follows, the Following feed, the people board),
the privacy model, the activation fee, Wallet Tracker P&L and LPs, X account
cards and Top calls.

Same stub-opener approach as test_client.py: pin the method, the path, the
body and the parsed fields, offline. Payloads use the server's field names
(apps/api/src/routes/social.rs, agent_public.rs, ponk_perks.rs, fees.rs).
"""

from __future__ import annotations

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import ponk  # noqa: E402
from ponk import (  # noqa: E402
    FeeEvent,
    FollowState,
    PeopleBoard,
    PonkPerks,
    Privacy,
    SocialFeed,
)
from tests.test_client import client  # noqa: E402

BASE = "https://example.test/api"

PERSON = {
    "key": "kader",
    "owner_key": "db4bfd2fecff",
    "name": "@kader",
    "handle": "kader",
    "display_name": None,
    "wallet": None,
    "avatar_version": None,
    "is_public": True,
    "usd_hidden": False,
}

FOLLOW = {
    "key": "kader",
    "following": True,
    "followers": 3,
    "notify_telegram": True,
    "notify_push": True,
    "muted": False,
    "private": False,
}

FEED = {
    "rows": [
        {
            "id": 7,
            "kind": "agent_open",
            "actor": PERSON,
            "verb": "Opened",
            "venue": "meteora",
            "pool": "POOL",
            "range_lo": 142.1,
            "range_hi": 151.3,
            "size_usd": 2400.0,
            "size_rounded": True,
            "pnl_usd": None,
            "usd_hidden": False,
            "count": 1,
            "first_at": "2026-10-28T20:00:00Z",
            "at": "2026-10-28T20:00:00Z",
            "time_rounded": False,
            "copy": {"mode": "agent", "slug": "abc", "venue": "meteora", "pool": "POOL"},
        }
    ],
    "next_cursor": "c1",
    "following": 4,
    "following_private": 1,
}

PRIVACY = {
    "visible": True,
    "show_name": True,
    "show_wallet": False,
    "display_name": "Kader",
    "saved": True,
    "settings": {"is_public": True, "show_wallet": False, "show_usd": True, "share_swaps": False, "rank_anon": True},
    "activity_ack": True,
    "needs_ack": False,
    "handle": "kader",
    "owner_key": "db4bfd2fecff",
    "named_key": "db4bfd2fecff",
    "anon_key": "aaaaaaaaaaaa",
    "profile_key": "kader",
    "agents": 4,
    "agents_hidden": 1,
}


class TestPeople(unittest.TestCase):
    def test_profile_strips_the_at_and_reads_publicly(self):
        c, opener = client({"key": "kader", "label": "@kader", "is_public": True, "social": {"followers": 3}})
        p = c.profile("@kader")
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/people/kader")
        self.assertEqual(p.label, "@kader")
        self.assertTrue(p.is_public)

    def test_follow_and_unfollow_a_person(self):
        c, opener = client(FOLLOW)
        s = c.follow_person("kader")
        c.unfollow_person("@kader")
        self.assertEqual(opener.calls[0]["method"], "POST")
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/people/kader/follow")
        self.assertEqual(opener.calls[1]["method"], "DELETE")
        self.assertIsInstance(s, FollowState)
        self.assertEqual(s.followers, 3)

    def test_followers_list_pages_by_cursor(self):
        c, opener = client({"people": [dict(PERSON, followed_at="2026-10-28T00:00:00Z")], "private_count": 2, "total": 3, "next_cursor": None, "hidden": False})
        lst = c.followers("kader", cursor="x")
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/people/kader/followers?cursor=x")
        self.assertEqual(lst.people[0].handle, "kader")
        self.assertEqual(lst.people[0].followed_at, "2026-10-28T00:00:00Z")
        self.assertEqual(lst.private_count, 2)

    def test_following_feed_parses_rows_actor_and_copy(self):
        c, opener = client(FEED)
        f = c.following_feed(kinds="agents", pnl_only=True)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/social/feed?kinds=agents&pnl=true")
        self.assertIsInstance(f, SocialFeed)
        row = f.rows[0]
        self.assertEqual(row.actor.handle, "kader")
        self.assertEqual(row.verb, "Opened")
        self.assertEqual(row.copy.mode, "agent")
        self.assertIsNone(row.pnl_usd)
        self.assertEqual(f.following_private, 1)

    def test_people_board(self):
        c, opener = client({"window": "7d", "sort": "pnl", "rows": [{"rank": 1, "person": PERSON, "pnl_usd": 613.0, "roi_pct": 5.2}], "basis": "b"})
        b = c.people_board(window="7d")
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/people-leaderboard?window=7d")
        self.assertIsInstance(b, PeopleBoard)
        self.assertEqual(b.rows[0].person.key, "kader")
        self.assertEqual(b.rows[0].pnl_usd, 613.0)


class TestPrivacy(unittest.TestCase):
    def test_read_and_set_privacy(self):
        c, opener = client(PRIVACY)
        p = c.privacy()
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/me/privacy")
        self.assertIsInstance(p, Privacy)
        self.assertTrue(p.settings.is_public)
        c.set_privacy(is_public=False)
        self.assertEqual(opener.calls[1]["method"], "PUT")
        self.assertEqual(
            opener.calls[1]["body"],
            {"is_public": False, "show_wallet": False, "show_usd": True, "share_swaps": False, "rank_anon": True},
        )

    def test_answer_the_prompt(self):
        c, opener = client(PRIVACY)
        c.answer_privacy_prompt(keep_public=True)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/me/privacy/ack")
        self.assertEqual(opener.calls[0]["body"], {"keep_public": True})

    def test_old_names_still_work(self):
        c, opener = client(PRIVACY)
        c.leaderboard_privacy()
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/leaderboard-privacy")


class TestActivationFee(unittest.TestCase):
    def test_constants(self):
        self.assertEqual(ponk.ACTIVATION_FEE_BPS, 100)
        self.assertEqual(ponk.ACTIVATION_FEE_HOLDER_BPS, 50)
        self.assertEqual(ponk.ACTIVATION_REFERRER_BPS, 25)

    def test_perks_carry_the_activation_rate(self):
        p = PonkPerks.from_dict({"holds": True, "activationFee": {"baseBps": 100, "effectiveBps": 50, "holderBps": 50}})
        self.assertEqual(p.activation_fee.base_bps, 100)
        self.assertEqual(p.activation_fee.effective_bps, 50)

    def test_fee_event_source(self):
        e = FeeEvent.from_dict({"id": "f", "fee_bps": 100, "source": "activation"})
        self.assertEqual(e.source, "activation")


class TestTrackers(unittest.TestCase):
    def test_wallet_pnl_and_tracked_lps(self):
        c, opener = client({"address": "W", "tokens": [], "solUsd": 99.5, "readAt": "t"})
        r = c.wallet_pnl("W")
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/wallets/W/pnl")
        self.assertEqual(r.sol_usd, 99.5)
        c, opener = client({"scope": "curated", "rows": [{"wallet": {"address": "W"}, "position": {}}], "walletsRead": 10})
        lps = c.tracked_lps()
        self.assertEqual(lps.wallets_read, 10)
        self.assertEqual(lps.rows[0].wallet["address"], "W")

    def test_x_account_card_and_first_calls(self):
        c, opener = client({"days": 30, "hitRate": 0.5, "x2Rate": 0.1, "median24h": 1.02, "firstCalls": 3})
        a = c.x_account("@solana", days=30)
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/x/account/solana?days=30")
        self.assertEqual((a.hit_rate, a.x2_rate, a.median24h, a.first_calls), (0.5, 0.1, 1.02, 3))
        c, opener = client({"items": [], "nextBefore": None})
        c.x_feed(first_calls=True)
        self.assertIn("first=1", opener.calls[0]["url"])

    def test_top_calls(self):
        c, opener = client({"days": 7, "calls": [], "computedAt": "t"})
        t = c.top_calls(source="all")
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/calls/top?source=all")
        self.assertEqual(t.computed_at, "t")


if __name__ == "__main__":
    unittest.main()
