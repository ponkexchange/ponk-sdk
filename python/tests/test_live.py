"""0.5.0 routes: live agents activity, the weekly race, the X Tracker, the
Wallet Tracker and referral tiers.

Same stub-opener approach as test_client.py. The payloads use the exact key
spelling the server sends: snake_case for the activity feed and the referral
view, camelCase (serde `rename_all = "camelCase"`) for the trackers and the
weekly race, so `accounts_1h` arrives as `accounts1h`.
"""

from __future__ import annotations

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from ponk import PonkBadRequestError, PonkClient  # noqa: E402
from tests.test_client import StubOpener, client  # noqa: E402

BASE = "https://example.test/api"
MINT = "So11111111111111111111111111111111111111112"
USDC = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"


def public(payload, status=200):
    opener = StubOpener(payload, status)
    return PonkClient(base_url=BASE, opener=opener), opener


ACCOUNT = {
    "handle": "caller",
    "screenName": "Caller",
    "name": "A Caller",
    "avatarUrl": None,
    "followers": 1200,
    "category": "kol",
}


class TestAgentsActivity(unittest.TestCase):
    def test_reads_holds_and_actions_without_identity(self):
        c, opener = public(
            {
                "events": [
                    {
                        "key": "2ca4fc912a33",
                        "kind": "hold",
                        "at": "2026-10-23T03:00:00Z",
                        "coarse": True,
                        "dex": "orca",
                        "token_x_mint": MINT,
                        "token_y_mint": USDC,
                        "usd": None,
                        "usd_kind": None,
                        "reason_key": "compound_below_usd_floor_hold",
                        "evaluations": 41,
                        "last_seen_at": "2026-10-23T04:10:00Z",
                    },
                    {
                        "key": "9f00aa",
                        "kind": "compound",
                        "at": "2026-10-23T02:14:09Z",
                        "coarse": False,
                        "dex": "meteora_dlmm",
                        "token_x_mint": MINT,
                        "token_y_mint": USDC,
                        "usd": "1.234567",
                        "usd_kind": "fees",
                        "reason_key": None,
                        "evaluations": None,
                        "last_seen_at": None,
                    },
                ],
                "holding": 7,
                "computed_at": "2026-10-23T04:10:05Z",
            }
        )
        feed = c.agents_activity()
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/agents-activity")
        self.assertNotIn("Authorization", opener.calls[0]["headers"])
        self.assertEqual(feed.holding, 7)
        hold, compound = feed.events
        self.assertTrue(hold.coarse)
        self.assertEqual(hold.evaluations, 41)
        self.assertIsNone(hold.usd)
        # Money stays a decimal string, never a float.
        self.assertEqual(compound.usd, "1.234567")
        self.assertEqual(compound.usd_kind, "fees")


class TestPonkillionairesWeek(unittest.TestCase):
    def test_reads_camel_case_standings_prizes_and_winners(self):
        c, opener = public(
            {
                "weekStart": "2026-10-19T00:00:00Z",
                "resetsAt": "2026-10-26T00:00:00Z",
                "prizes": [{"rank": 1, "feeRatePct": 2.0, "points": 50000}],
                "minPeakUsd": 250,
                "minRunningDays": 3,
                "standings": [
                    {
                        "rank": 1,
                        "pnlUsd": 41.5,
                        "agents": 2,
                        "deployedUsd": 900.0,
                        "prize": {"rank": 1, "feeRatePct": 2.0, "points": 50000},
                        "ownerKey": "k1",
                        "handle": None,
                        "displayName": "kader",
                        "wallet": None,
                    },
                    {
                        "rank": 4,
                        "pnlUsd": -3.0,
                        "agents": 1,
                        "deployedUsd": 300.0,
                        "prize": None,
                        "ownerKey": "k4",
                        "handle": None,
                        "displayName": None,
                        "wallet": None,
                    },
                ],
                "lastWeek": {
                    "weekStart": "2026-10-12T00:00:00Z",
                    "settled": True,
                    "winners": [
                        {
                            "rank": 1,
                            "pnlUsd": 80.0,
                            "agents": 3,
                            "feeRatePct": 2.0,
                            "points": 50000.0,
                            "rateFrom": "2026-10-19T00:00:00Z",
                            "rateUntil": "2026-10-26T00:00:00Z",
                            "ownerKey": "k9",
                            "handle": "maker",
                            "displayName": None,
                            "wallet": None,
                        }
                    ],
                },
                "computedAt": "2026-10-23T04:00:00Z",
            }
        )
        week = c.ponkillionaires_week()
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/ponkillionaires/week")
        self.assertEqual(week.min_peak_usd, 250)
        self.assertEqual(week.prizes[0].fee_rate_pct, 2.0)
        top, fourth = week.standings
        self.assertEqual(top.display_name, "kader")
        self.assertEqual(top.prize.points, 50000)
        self.assertIsNone(fourth.prize)
        self.assertTrue(week.last_week.settled)
        self.assertEqual(week.last_week.winners[0].handle, "maker")
        self.assertEqual(week.last_week.winners[0].rate_until, "2026-10-26T00:00:00Z")


class TestXTracker(unittest.TestCase):
    def test_feed_sends_camel_case_filters_and_parses_both_item_types(self):
        c, opener = public(
            {
                "items": [
                    {
                        "type": "post",
                        "key": "p:caller:1",
                        "account": ACCOUNT,
                        "post": {
                            "id": "1",
                            "kind": "post",
                            "url": "https://x.com/Caller/status/1",
                            "text": "watching this one",
                            "createdAt": "2026-10-23T04:00:00Z",
                            "seenAt": "2026-10-23T04:00:20Z",
                            "latencySecs": 20,
                            "isBackfill": False,
                            "replyTo": None,
                            "author": {"screenName": "Caller", "name": "A Caller", "avatarUrl": None},
                            "media": [],
                            "quote": None,
                            "card": None,
                            "metrics": {"views": 10, "likes": 2, "replies": None, "reposts": 0},
                            "tokens": [{"mint": MINT, "symbol": "SOL"}],
                            "cashtags": ["SOL"],
                            "deletedAt": None,
                        },
                    },
                    {
                        "type": "event",
                        "key": "e:5",
                        "account": ACCOUNT,
                        "event": {
                            "id": 5,
                            "kind": "post_deleted",
                            "before": None,
                            "after": None,
                            "detail": None,
                            "at": "2026-10-23T03:00:00Z",
                            "post": {"id": "0", "url": None, "text": "gone", "createdAt": None, "tokens": []},
                        },
                    },
                ],
                "nextBefore": "2026-10-23T03:00:00Z",
                "source": {"name": "FxTwitter", "state": "live", "accountsFresh": 150, "accountsActive": 159},
            }
        )
        feed = c.x_feed(limit=2, ca_only=True, min_followers=1000, category="kol,news")
        url = opener.calls[0]["url"]
        self.assertTrue(url.startswith(BASE + "/public/x/feed?"), url)
        for part in ("limit=2", "ca=1", "minFollowers=1000", "category=kol%2Cnews"):
            self.assertIn(part, url)
        self.assertNotIn("before=", url)
        post, event = feed.items
        self.assertEqual(post.type, "post")
        self.assertEqual(post.account.screen_name, "Caller")
        self.assertEqual(post.post.tokens[0].symbol, "SOL")
        self.assertEqual(post.post.author.screen_name, "Caller")
        self.assertIsNone(post.post.metrics.replies)
        self.assertEqual(event.type, "event")
        self.assertEqual(event.event.post.text, "gone")
        self.assertEqual(feed.next_before, "2026-10-23T03:00:00Z")
        self.assertEqual(feed.source.accounts_fresh, 150)

    def test_ca_only_false_sends_no_ca_parameter(self):
        c, opener = public({"items": [], "nextBefore": None, "source": {}})
        c.x_feed()
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/x/feed")

    def test_trending_reads_digit_suffixed_keys(self):
        c, opener = public(
            {
                "window": "24h",
                "tokens": [
                    {
                        "mint": MINT,
                        "symbol": "SOL",
                        "name": "Wrapped SOL",
                        "accounts1h": 1,
                        "accounts24h": 4,
                        "mentions24h": 9,
                        "lastAt": "2026-10-23T04:00:00Z",
                        "firstCall": {
                            "handle": "caller",
                            "screenName": "Caller",
                            "avatarUrl": None,
                            "postId": "1",
                            "url": "https://x.com/Caller/status/1",
                            "at": "2026-10-22T04:00:00Z",
                            "priceUsd": None,
                            "mcapUsd": None,
                        },
                        "priceUsd": 190.0,
                        "mcapUsd": None,
                        "multiple": None,
                    }
                ],
                "computedAt": "2026-10-23T04:00:01Z",
            }
        )
        t = c.x_trending(window="24h", limit=5)
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/x/trending?window=24h&limit=5")
        tok = t.tokens[0]
        self.assertEqual((tok.accounts_1h, tok.accounts_24h, tok.mentions_24h), (1, 4, 9))
        self.assertEqual(tok.first_call.post_id, "1")
        # No entry price means no multiple, never a made-up one.
        self.assertIsNone(tok.first_call.price_usd)
        self.assertIsNone(tok.multiple)

    def test_callers_flatten_the_stats_into_the_row(self):
        c, opener = public(
            {
                "days": 7,
                "callers": [
                    {
                        "account": ACCOUNT,
                        "calls": 6,
                        "pricedCalls": 4,
                        "median1h": 1.1,
                        "median24h": 1.4,
                        "best": 3.0,
                        "hitRate": 0.5,
                        "scored24h": 4,
                        "lastCallAt": "2026-10-23T01:00:00Z",
                    }
                ],
                "computedAt": "2026-10-23T04:00:00Z",
            }
        )
        row = c.x_callers(days=7).callers[0]
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/x/callers?days=7")
        self.assertEqual(row.account.handle, "caller")
        self.assertEqual((row.priced_calls, row.median_24h, row.hit_rate, row.scored_24h), (4, 1.4, 0.5, 4))

    def test_token_accounts_and_status(self):
        c, opener = public(
            {
                "mint": MINT,
                "symbol": "SOL",
                "accounts1h": 0,
                "accounts24h": 2,
                "firstCall": None,
                "priceUsd": 190.0,
                "multiple": None,
                "mentions": [
                    {
                        "account": ACCOUNT,
                        "postId": "1",
                        "url": "u",
                        "text": "t",
                        "kind": "post",
                        "at": "2026-10-23T00:00:00Z",
                        "priceUsd": 180.0,
                        "mcapUsd": None,
                        "deleted": False,
                    }
                ],
            }
        )
        tok = c.x_token(MINT)
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/x/token/" + MINT)
        self.assertEqual(tok.accounts_24h, 2)
        self.assertFalse(tok.mentions[0].deleted)

        c, opener = public(
            {
                "accounts": [dict(ACCOUNT, isDefault=True, status="active", lastOkAt=None, followed=False, muted=False, alerting=False)],
                "hiddenCategories": [],
                "categories": ["kol", "news"],
            }
        )
        d = c.x_accounts()
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/x/accounts")
        self.assertTrue(d.accounts[0].is_default)
        self.assertEqual(d.categories, ["kol", "news"])

        c, opener = public({"name": "FxTwitter", "state": "live", "requests1h": 5721, "rateLimited1h": 0, "hotSecs": 20})
        s = c.x_status()
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/x/status")
        self.assertEqual((s.state, s.requests_1h, s.rate_limited_1h, s.hot_secs), ("live", 5721, 0, 20))

    def test_a_bad_mint_raises_bad_request(self):
        c, _ = public({"error": {"code": "bad_request", "message": "not a Solana address"}}, status=400)
        with self.assertRaises(PonkBadRequestError):
            c.x_token("nope")


EVENT = {
    "key": "sig:abc",
    "signature": "abc",
    "kind": "buy",
    "venue": "jupiter",
    "pool": None,
    "position": None,
    "tokenMint": MINT,
    "legs": [
        {"mint": USDC, "symbol": "USDC", "amount": "1000000", "decimals": 6, "dir": "out", "usd": 1.0},
        {"mint": MINT, "symbol": "SOL", "amount": "5000000", "decimals": 9, "dir": "in", "usd": None},
    ],
    "usd": 1.0,
    "blockTime": "2026-10-23T04:00:00Z",
    "seenAt": "2026-10-23T04:00:03Z",
    "isBackfill": False,
    "wallet": {"address": "W1", "label": "Top LP", "emoji": None, "color": None, "source": "ponkillionaire", "agentSlug": "abc123"},
}


class TestWalletTracker(unittest.TestCase):
    def test_feed_sends_filters_and_parses_legs(self):
        c, opener = public(
            {
                "items": [EVENT],
                "nextBefore": "1761192000000_42",
                "source": {"state": "live", "walletsFresh": 10, "walletsTracked": 12, "lastOkAt": None},
            }
        )
        feed = c.wallets_feed(kind="buy,sell", min_usd=50, limit=10)
        url = opener.calls[0]["url"]
        for part in ("kind=buy%2Csell", "minUsd=50", "limit=10"):
            self.assertIn(part, url)
        row = feed.items[0]
        self.assertEqual(row.token_mint, MINT)
        self.assertEqual(row.legs[0].amount, "1000000")
        self.assertIsNone(row.legs[1].usd)
        self.assertEqual(row.wallet.agent_slug, "abc123")
        self.assertEqual(feed.next_before, "1761192000000_42")
        self.assertEqual(feed.source.wallets_tracked, 12)

    def test_activity_defaults_and_status(self):
        c, opener = public({"address": "W1", "mode": "live", "items": [EVENT], "nextBefore": None, "lastOkAt": "2026-10-23T04:00:00Z"})
        a = c.wallet_activity("W1", kind="lp")
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/wallets/W1/activity?kind=lp")
        self.assertEqual(a.mode, "live")
        self.assertEqual(a.items[0].signature, "abc")

        c, opener = public({"defaults": [{"address": "W1", "label": "Top LP", "kind": "ponkillionaire", "agentSlug": None, "source": None, "muted": False, "lastActivityAt": None}]})
        d = c.wallets_defaults()
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/wallets/defaults")
        self.assertEqual(d.defaults[0].kind, "ponkillionaire")

        c, opener = public({"state": "starting", "walletsFresh": 0, "walletsTracked": 0, "lastOkAt": None})
        self.assertEqual(c.wallets_status().state, "starting")
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/wallets/status")

    def test_an_address_cannot_change_the_route(self):
        c, opener = public({"address": "x", "mode": "live", "items": []})
        c.wallet_activity("../../health")
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/wallets/..%2F..%2Fhealth/activity")


class TestReferralTier(unittest.TestCase):
    def test_tier_and_ladder_are_typed(self):
        c, opener = client(
            {
                "code": "abc",
                "codes": [{"code": "abc", "label": None, "created_at": "2026-10-01T00:00:00Z"}],
                "total_earned_usd": 3.5,
                "tier": {
                    "current": 2,
                    "share_bps": 2500,
                    "referred_fees_usd": 1500.0,
                    "window_days": 30,
                    "next": {"tier": 3, "min_fees_usd": 5000, "share_bps": 3000},
                },
                "programme": {
                    "ladder": [
                        {"tier": 1, "min_fees_usd": 0, "share_bps": 2000},
                        {"tier": 2, "min_fees_usd": 1000, "share_bps": 2500},
                        {"tier": 3, "min_fees_usd": 5000, "share_bps": 3000},
                    ],
                    "window_days": 30,
                },
            }
        )
        r = c.referral()
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/referral")
        self.assertEqual(r.codes[0].code, "abc")
        self.assertEqual((r.tier.current, r.tier.share_bps, r.tier.window_days), (2, 2500, 30))
        self.assertEqual(r.tier.next.min_fees_usd, 5000)
        self.assertEqual([s.share_bps for s in r.programme.ladder], [2000, 2500, 3000])

    def test_a_failed_volume_read_shows_no_progress(self):
        c, _ = client(
            {
                "tier": {"current": 1, "share_bps": 2000, "referred_fees_usd": None, "window_days": 30, "next": {"tier": 2, "min_fees_usd": 1000, "share_bps": 2500}},
            }
        )
        t = c.referral().tier
        self.assertIsNone(t.referred_fees_usd)
        self.assertEqual(t.share_bps, 2000)

    def test_at_the_top_there_is_no_next_rung(self):
        c, _ = client({"tier": {"current": 3, "share_bps": 3000, "referred_fees_usd": 9000.0, "window_days": 30, "next": None}})
        self.assertIsNone(c.referral().tier.next)


if __name__ == "__main__":
    unittest.main()
