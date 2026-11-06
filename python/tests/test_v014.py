"""0.14.0: Meteora DAMM v2, the public pool history, the sell check on a pool
read, and the copy_alerts switch.

Same stub-opener approach as test_client.py. Payloads use the field names of
the server's structs: `routes/damm_v2_lp.rs` (OpenRequest, OpenPrepared,
ClaimRequest, ClaimPrepared), `routes/tick_lp.rs` (WithdrawRequest,
WithdrawPrepared, AddRequest), `routes/pool_history.rs` (PoolHistory and its
parts), `routes/pools.rs` (PoolView) and `routes/notify.rs`.
"""

from __future__ import annotations

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from ponk import (  # noqa: E402
    NotifySettings,
    PoolHistory,
    PoolHistoryPoint,
    PoolSnapshot,
    Prepared,
    __version__,
)
from tests.test_client import client  # noqa: E402

BASE = "https://example.test/api"
POOL = "DAMMpool111111111111111111111111111111111111"
POS = "DAMMpos1111111111111111111111111111111111111"
OWNER = "GYdJfRSH11111111111111111111111111111111111"


class TestVersion(unittest.TestCase):
    def test_version_is_0_14_0(self):
        self.assertEqual(__version__, "0.14.0")


class TestDammV2(unittest.TestCase):
    def test_open_sends_two_amounts_as_strings_and_no_range(self):
        c, opener = client(
            {
                "unsigned_tx_b64": "AQAB",
                "recent_blockhash": "H",
                "position_address": POS,
                "position_nft_mint": "NFT",
            }
        )
        p = c.damm_v2_open_prepare(pool_address=POOL, owner=OWNER, amount_x=18446744073709551615, amount_y="0")
        call = opener.calls[0]
        self.assertEqual((call["method"], call["url"]), ("POST", BASE + "/public/lp/meteora_damm_v2/open/prepare"))
        self.assertEqual(
            call["body"],
            {"pool_address": POOL, "owner": OWNER, "amount_x": "18446744073709551615", "amount_y": "0"},
        )
        self.assertIsInstance(p, Prepared)
        self.assertEqual((p.position_address, p.position_nft_mint), (POS, "NFT"))

    def test_open_passes_slippage_only_when_set(self):
        c, opener = client({"unsigned_tx_b64": "AQAB"})
        c.damm_v2_open_prepare(pool_address=POOL, owner=OWNER, amount_x=1, amount_y=2, max_slippage_bps=50)
        self.assertEqual(opener.calls[0]["body"]["max_slippage_bps"], 50)

    def test_claim_names_the_owner_and_reads_the_fee_review(self):
        fee = {"bps": 800, "legs": []}
        c, opener = client({"unsigned_tx_b64": "AQAB", "recent_blockhash": "H", "fee": fee, "fee_event_ids": ["e1"]})
        p = c.damm_v2_claim_prepare(pool_address=POOL, position=POS, owner=OWNER)
        call = opener.calls[0]
        self.assertEqual(call["url"], BASE + "/public/lp/meteora_damm_v2/claim/prepare")
        self.assertEqual(call["body"], {"pool_address": POOL, "position": POS, "owner": OWNER})
        self.assertEqual(p.fee, fee)
        self.assertEqual(p.fee_event_ids, ["e1"])

    def test_withdraw_on_the_key_and_public_routes(self):
        c, opener = client({"unsigned_tx_b64": "AQAB", "fee_event_ids": [], "closes_position": False})
        p = c.lp_withdraw_prepare("meteora_damm_v2", pool_address=POOL, position=POS, bps_to_remove=10000)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/lp/meteora_damm_v2/withdraw/prepare")
        self.assertNotIn("owner", opener.calls[0]["body"])
        self.assertIs(p.closes_position, False)
        c.lp_withdraw_prepare_public(
            "meteora_damm_v2", pool_address=POOL, position=POS, owner=OWNER, bps_to_remove=2500
        )
        self.assertEqual(opener.calls[1]["url"], BASE + "/public/lp/meteora_damm_v2/withdraw/prepare")
        self.assertEqual(
            opener.calls[1]["body"],
            {"pool_address": POOL, "position": POS, "owner": OWNER, "bps_to_remove": 2500},
        )

    def test_add_liquidity(self):
        c, opener = client({"unsigned_tx_b64": "AQAB", "recent_blockhash": "H"})
        c.lp_add_liquidity_prepare(
            "meteora_damm_v2", pool_address=POOL, position=POS, owner=OWNER, amount_x=5, amount_y=7
        )
        call = opener.calls[0]
        self.assertEqual(call["url"], BASE + "/public/lp/meteora_damm_v2/add-liquidity/prepare")
        self.assertEqual(
            call["body"],
            {"pool_address": POOL, "position": POS, "owner": OWNER, "amount_x": "5", "amount_y": "7"},
        )


HISTORY = {
    "dex": "meteora_dlmm",
    "pool": POOL,
    "resolution": "day",
    "window": {
        "bucket_secs": 86400,
        "from": 1761955200,
        "to": 1762214400,
        "from_iso": "2025-11-01T00:00:00+00:00",
        "to_iso": "2025-11-04T00:00:00+00:00",
        "buckets_in_window": 3,
        "buckets_served": 2,
        "omitted": {"before_pool_created": 0, "no_recorded_activity": 1},
        "pool_created_at": None,
    },
    "sources": {
        "volume_usd": {"status": "ok", "source": "meteora_datapi", "detail": "as published"},
        "fees_usd": {"status": "ok", "source": "meteora_datapi", "detail": "as published"},
        "lp_fees_usd": {"status": "derived", "source": "meteora_datapi", "detail": "fees minus protocol fees"},
        "protocol_fees_usd": {"status": "ok", "source": "meteora_datapi", "detail": "as published"},
        "fee_rate_pct": {"status": "derived", "source": "meteora_datapi", "detail": "fees over volume"},
        "tvl_usd": {"status": "not_published", "source": None, "detail": "no TVL series"},
    },
    "points": [
        {
            "ts": 1761955200,
            "date": "2025-11-01T00:00:00+00:00",
            "volume_usd": 1000.0,
            "fees_usd": 2.5,
            "lp_fees_usd": 2.0,
            "protocol_fees_usd": 0.5,
            "fee_rate_pct": 0.25,
            "tvl_usd": None,
        }
    ],
    "current": {"tvl_usd": 50000.0, "base_fee_pct": 0.2, "fee_is_dynamic": True, "lp_share_pct": 80.0, "source": "meteora_datapi"},
    "edge": {"status": "not_measured_recently", "verdict": None, "reading": None, "admit_r": None, "measured_at": None},
    "generated_at": 1762215000,
}


class TestPoolHistory(unittest.TestCase):
    def test_public_route_and_parse(self):
        c, opener = client(HISTORY)
        h = c.pool_history("meteora_dlmm", POOL, resolution="day", buckets=3)
        self.assertEqual(
            opener.calls[0]["url"],
            BASE + "/public/pools/meteora_dlmm/" + POOL + "/history?resolution=day&buckets=3",
        )
        self.assertIsInstance(h, PoolHistory)
        self.assertEqual(h.window.from_, 1761955200)
        self.assertEqual(h.window.omitted["no_recorded_activity"], 1)
        self.assertIsInstance(h.points[0], PoolHistoryPoint)
        self.assertEqual(h.points[0].fees_usd, 2.5)
        # Absent stays absent: a field no source has is None, never zero.
        self.assertIsNone(h.points[0].tvl_usd)
        self.assertEqual(h.sources.tvl_usd.status, "not_published")
        self.assertIsNone(h.sources.tvl_usd.source)
        self.assertEqual(h.current.lp_share_pct, 80.0)
        self.assertEqual(h.edge.status, "not_measured_recently")

    def test_defaults_send_no_query_and_v1_twin(self):
        c, opener = client(HISTORY)
        c.pool_history("orca", POOL)
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/pools/orca/" + POOL + "/history")
        c.pool_history("raydium", POOL, resolution="hour", public=False)
        self.assertEqual(opener.calls[1]["url"], BASE + "/v1/pools/raydium/" + POOL + "/history?resolution=hour")


class TestPoolAndSettings(unittest.TestCase):
    def test_pool_carries_the_sell_check(self):
        c, opener = client(
            {
                "dex": "meteora_damm_v2",
                "pool_address": POOL,
                "bin_step": 0,
                "active_bin_id": 0,
                "token_price": 1.5,
                "price_divergence": 0.01,
                "mispricing_warning": None,
                "sell_warning": "This token may not be sellable.",
            }
        )
        p = c.pool("meteora_damm_v2", POOL)
        self.assertEqual(opener.calls[0]["url"], BASE + "/pools/meteora_damm_v2/" + POOL)
        self.assertIsInstance(p, PoolSnapshot)
        self.assertEqual(p.sell_warning, "This token may not be sellable.")
        self.assertIsNone(p.mispricing_warning)
        self.assertEqual(p.price_divergence, 0.01)

    def test_copy_alerts_switch(self):
        payload = {
            "rebalance_telegram": False,
            "mandate_telegram": True,
            "weekly_recap_telegram": False,
            "comments_telegram": True,
            "copy_alerts": False,
        }
        c, opener = client(payload)
        s = c.update_notify_settings(copy_alerts=False)
        self.assertEqual(opener.calls[0]["method"], "PUT")
        self.assertEqual(opener.calls[0]["body"], {"copy_alerts": False})
        self.assertIsInstance(s, NotifySettings)
        self.assertIs(s.copy_alerts, False)
        self.assertIs(s.comments_telegram, True)


if __name__ == "__main__":
    unittest.main()
