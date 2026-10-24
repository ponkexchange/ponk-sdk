"""0.7.0 routes: swap orders, copy and follow, the self-custody loop, the
account surfaces and rank presets.

Same stub-opener approach as test_client.py. Payloads use the field names of
the server's structs (`routes/v1_orders.rs`, `routes/v1_account.rs`,
`routes/v1_rank.rs` and the session handlers they delegate to).
"""

from __future__ import annotations

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from ponk import (  # noqa: E402
    DEFAULT_FUND_TIMEOUT,
    BatchResult,
    CopyLog,
    FollowStatus,
    JupiterSession,
    OrderBatch,
    OrderPlan,
    Prepared,
    __version__,
    dca_order,
    limit_order,
    tp_sl_order,
)
from tests.test_client import client  # noqa: E402
from tests.test_social import SequenceOpener  # noqa: E402
from ponk import PonkClient  # noqa: E402

BASE = "https://example.test/api"
A1 = "018f0000-0000-7000-8000-000000000001"
SOL = "So11111111111111111111111111111111111111112"
USDC = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
JWT = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ4In0.sig"


def header(call, name):
    # urllib title-cases header names.
    return {k.lower(): v for k, v in call["headers"].items()}.get(name.lower())


class TestVersion(unittest.TestCase):
    def test_version_is_070(self):
        self.assertEqual(__version__, "0.7.0")


class TestOrderIntents(unittest.TestCase):
    def test_limit_intent_wire_shape(self):
        o = limit_order(USDC, SOL, 25_000_000, target_usd=150.0)
        self.assertEqual(o, {"kind": "limit", "input_mint": USDC, "output_mint": SOL, "amount": "25000000", "target_usd": 150.0})

    def test_trailing_and_tp_sl(self):
        self.assertEqual(limit_order(SOL, USDC, 1, trailing_bps=500)["trailing_bps"], 500)
        t = tp_sl_order(SOL, USDC, "1000", tp_usd=200.0, sl_usd=120.0)
        self.assertEqual((t["kind"], t["tp_usd"], t["sl_usd"]), ("tp_sl", 200.0, 120.0))

    def test_dca_with_and_without_band(self):
        d = dca_order(USDC, SOL, 10_000_000, 5, 86_400)
        self.assertNotIn("band", d)
        b = dca_order(USDC, SOL, 10_000_000, 5, 86_400, band_trigger_mint=SOL, max_mc_usd=1e9)
        self.assertEqual(b["band"], {"trigger_mint": SOL, "max_mc_usd": 1e9})
        with self.assertRaises(ValueError):
            dca_order(USDC, SOL, 1, 2, 60, max_price_usd=1.0)


class TestOrders(unittest.TestCase):
    def test_plan_orders_posts_intents_and_parses_the_link(self):
        c, opener = client({"sign_url": "https://ponk.exchange/orders/sign?w=W&c=all", "wallet": "W", "orders": [], "cancel": [], "cancel_all": True, "signatures": "x", "notes": []})
        plan = c.plan_orders([limit_order(USDC, SOL, 1, target_usd=1.0)], cancel_all=True)
        call = opener.calls[0]
        self.assertEqual((call["method"], call["url"]), ("POST", BASE + "/v1/orders/plan"))
        self.assertEqual(call["body"]["cancel_all"], True)
        self.assertEqual(call["body"]["orders"][0]["kind"], "limit")
        self.assertIsInstance(plan, OrderPlan)
        self.assertTrue(plan.sign_url.startswith("https://ponk.exchange/orders/sign"))

    def test_sign_in_verify_and_session_parse(self):
        opener = SequenceOpener([
            {"type": "message", "challenge": "Sign this message to authenticate with Jupiter"},
            {"accessToken": JWT, "refreshToken": "R" * 20, "expiresAt": "2026-10-23T00:15:00Z"},
        ])
        c = PonkClient(api_key="ponk_live_test", base_url=BASE, opener=opener)
        seen = []
        s = c.orders_sign_in(lambda text: seen.append(text) or "5" * 88)
        self.assertIsInstance(s, JupiterSession)
        self.assertEqual(s.access_token, JWT)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/orders/auth/challenge")
        self.assertEqual(opener.calls[1]["body"], {"type": "message", "signature": "5" * 88})
        # The client never sends a wallet: the server pins it to the key's own.
        self.assertNotIn("walletPubkey", opener.calls[1]["body"])
        self.assertEqual(seen[0], "Sign this message to authenticate with Jupiter")

    def test_verify_needs_exactly_one_proof(self):
        c, opener = client({})
        with self.assertRaises(ValueError):
            c.orders_verify()
        with self.assertRaises(ValueError):
            c.orders_verify(signature="a", signed_transaction="b")
        self.assertEqual(opener.calls, [])

    def test_order_calls_carry_the_jupiter_session_header_not_the_key(self):
        c, opener = client({"transaction": "AQAB", "requestId": "req-1", "mint": USDC, "amount": "25000000"})
        craft = c.craft_order_deposit(JWT, input_mint=USDC, output_mint=SOL, amount="25000000", order_type="price", order_sub_type="single")
        call = opener.calls[0]
        self.assertEqual(call["url"], BASE + "/v1/orders/deposit/craft")
        self.assertEqual(header(call, "X-Jupiter-Session"), JWT)
        self.assertEqual(header(call, "Authorization"), "Bearer ponk_live_test")
        self.assertNotIn("userAddress", call["body"])
        self.assertEqual(call["body"]["orderSubType"], "single")
        self.assertEqual(craft.request_id, "req-1")
        self.assertIsNone(c.api_key and getattr(c, "jupiter_session", None))

    def test_create_and_confirm_take_the_fund_timeout(self):
        c, opener = client({"id": "ord-12345678", "txSignature": "SIG"})
        made = c.create_price_order(JWT, {"orderType": "single", "inputMint": USDC})
        self.assertEqual(made.tx_signature, "SIG")
        c.create_dca_order(JWT, {"orderType": "time_based"})
        c.confirm_order_cancel(JWT, "dca", "ord-12345678", signed_transaction="AQ==", cancel_request_id="r1")
        self.assertEqual([x["timeout"] for x in opener.calls], [DEFAULT_FUND_TIMEOUT] * 3)
        self.assertEqual(opener.calls[1]["url"], BASE + "/v1/orders/dca")
        self.assertEqual(opener.calls[2]["url"], BASE + "/v1/orders/dca/ord-12345678/confirm-cancel")
        self.assertEqual(opener.calls[2]["body"], {"signedTransaction": "AQ==", "cancelRequestId": "r1"})

    def test_cancel_list_and_batches(self):
        c, opener = client({"total": 1, "prepared": 1, "failed": 0, "items": [{"family": "price", "id": "ord-1", "ok": True, "result": {"id": "ord-1", "transaction": "AQ", "requestId": "rq"}}]})
        batch = c.cancel_orders(JWT, all_orders=True)
        self.assertIsInstance(batch, OrderBatch)
        self.assertEqual(batch.items[0].cancel().request_id, "rq")
        self.assertEqual(opener.calls[0]["body"], {"orders": [], "all": True})
        c.cancel_order(JWT, "price", "ord-12345678")
        self.assertEqual(opener.calls[1]["url"], BASE + "/v1/orders/price/ord-12345678/cancel")
        c.list_orders(JWT, family="price", state="active", limit=10)
        self.assertEqual(opener.calls[2]["url"], BASE + "/v1/orders?family=price&state=active&limit=10")
        c.confirm_order_cancels(JWT, [{"family": "price", "id": "ord-1", "signedTransaction": "A", "cancelRequestId": "r"}])
        self.assertEqual(opener.calls[3]["url"], BASE + "/v1/orders/confirm-cancel")
        self.assertEqual(opener.calls[3]["timeout"], DEFAULT_FUND_TIMEOUT)
        with self.assertRaises(ValueError):
            c.cancel_orders(JWT)
        with self.assertRaises(ValueError):
            c.cancel_order(JWT, "swap", "x")
        with self.assertRaises(ValueError):
            c.list_orders("")

    def test_place_order_crafts_signs_and_creates(self):
        opener = SequenceOpener([
            {"transaction": "UNSIGNED", "requestId": "rq", "mint": USDC, "amount": "25000000"},
            {"id": "ord-12345678"},
        ])
        c = PonkClient(api_key="ponk_live_test", base_url=BASE, opener=opener)
        made = c.place_order(JWT, "price", {"orderType": "single", "inputMint": USDC, "outputMint": SOL, "inputAmount": "25000000"}, lambda tx: tx + "-SIGNED")
        self.assertEqual(made.id, "ord-12345678")
        self.assertEqual(opener.calls[0]["body"]["orderSubType"], "single")
        self.assertEqual(opener.calls[1]["body"]["depositSignedTx"], "UNSIGNED-SIGNED")
        self.assertEqual(opener.calls[1]["body"]["depositRequestId"], "rq")

    def test_place_order_refuses_a_mismatched_deposit_before_signing(self):
        opener = SequenceOpener([{"transaction": "T", "requestId": "rq", "mint": SOL, "amount": "1"}])
        c = PonkClient(api_key="ponk_live_test", base_url=BASE, opener=opener)
        signed = []
        with self.assertRaises(ValueError):
            c.place_order(JWT, "dca", {"inputMint": USDC, "outputMint": SOL, "inputAmount": "25000000"}, lambda tx: signed.append(tx) or tx)
        self.assertEqual(signed, [])
        self.assertEqual(len(opener.calls), 1)


FOLLOW = {"copied": True, "following": True, "leader": {"name": "x", "slug": "abc", "key": "k"}, "auto_follow_pool_moves": False, "pool_moves_enabled": True, "mirrored_keys": ["a"], "paused_keys": [], "offer": None, "last_mirrored_at": None, "last_error": None, "not_copied": [{"keys": ["b"], "reason": "r"}]}


class TestCopyFollow(unittest.TestCase):
    def test_copy_source_and_follow_paths(self):
        c, opener = client(FOLLOW)
        st = c.copy_source(A1, "board-key", follow=True)
        self.assertIsInstance(st, FollowStatus)
        self.assertEqual(st.leader.slug, "abc")
        self.assertEqual(st.not_copied[0].reason, "r")
        self.assertEqual(opener.calls[0]["body"], {"source": "board-key", "follow": True, "auto_follow_pool_moves": False})
        c.follow_leader(A1, auto_follow_pool_moves=True)
        c.update_follow(A1, pause_keys=["x"])
        c.unfollow_leader(A1)
        c.answer_follow_offer(A1, True)
        methods = [(x["method"], x["url"]) for x in opener.calls[1:]]
        self.assertEqual(methods, [
            ("POST", BASE + "/v1/agents/" + A1 + "/follow"),
            ("PATCH", BASE + "/v1/agents/" + A1 + "/follow"),
            ("DELETE", BASE + "/v1/agents/" + A1 + "/follow"),
            ("POST", BASE + "/v1/agents/" + A1 + "/follow/offer"),
        ])
        self.assertEqual(opener.calls[2]["body"], {"pause_keys": ["x"]})

    def test_batches(self):
        c, opener = client({"total": 1, "succeeded": 1, "failed": 0, "items": [{"key": A1, "ok": True}]})
        r = c.follow_leaders([A1], unfollow=True)
        self.assertIsInstance(r, BatchResult)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/follow/batch")
        self.assertEqual(opener.calls[0]["body"], {"items": [{"agent_id": A1, "auto_follow_pool_moves": False}], "unfollow": True})
        c.follow_many(["s1", "s2"])
        self.assertEqual(opener.calls[1]["body"], {"slugs": ["s1", "s2"], "unfollow": False})
        with self.assertRaises(TypeError):
            c.follow_many("s1")

    def test_wallet_follow_sends_only_given_settings(self):
        c, opener = client({"follow": {"leader_address": "L", "size_pct": 50.0, "auto_mirror": False}, "entries": []})
        log = c.wallet_follow(A1, leader="L", size_pct=50.0, pools=["P"])
        self.assertIsInstance(log, CopyLog)
        self.assertEqual(log.follow.size_pct, 50.0)
        self.assertEqual(opener.calls[0]["body"], {"auto_mirror": False, "leader": "L", "size_pct": 50.0, "pools": ["P"]})
        c.unfollow_wallet(A1)
        self.assertEqual(opener.calls[1]["method"], "DELETE")
        c.answer_wallet_offer(A1, False)
        self.assertEqual(opener.calls[2]["url"], BASE + "/v1/agents/" + A1 + "/wallet-follow/offer")


class TestSelfCustody(unittest.TestCase):
    def test_prepares_return_unsigned_builds(self):
        c, opener = client({"unsigned_tx_b64": "AQAB", "recent_blockhash": "H", "new_position_address": "NP"})
        p = c.agent_rebalance_prepare_split(A1, new_lower_bin_id=1, new_upper_bin_id=9, strategy="spot_balanced", max_slippage_bps=100, simulate=True)
        self.assertIsInstance(p, Prepared)
        self.assertEqual(p.unsigned_tx_b64, "AQAB")
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/" + A1 + "/rebalance/prepare-split?simulate=true")
        c.lp_withdraw_prepare("ponk_clouds", pool_address="P", position="POS", bps_to_remove=10000)
        self.assertEqual(opener.calls[1]["url"], BASE + "/v1/lp/ponk_clouds/withdraw/prepare")
        self.assertNotIn("owner", opener.calls[1]["body"])
        c.position_claim_prepare("POS")
        self.assertEqual(opener.calls[2]["url"], BASE + "/v1/positions/POS/claim-fees/prepare")
        c.agent_fees_prepare(A1)
        self.assertEqual(opener.calls[3]["url"], BASE + "/v1/agents/" + A1 + "/collect-fees/prepare")

    def test_marks(self):
        c, opener = client({"tx_id": "t", "fee_events_confirmed": 2})
        m = c.position_mark_tx(signature="S", action_type="claim_fees", dex="meteora_dlmm", position_address="POS", fee_event_ids=["e"])
        self.assertEqual(m.fee_events_confirmed, 2)
        self.assertEqual(opener.calls[0]["body"]["fee_event_ids"], ["e"])
        c.agent_rebalance_mark(A1, signature="S", new_position_address="NP")
        self.assertEqual(opener.calls[1]["body"], {"signature": "S", "new_position_address": "NP"})
        c.agent_fees_confirm(A1, signature="S", fee_token_mints=[SOL])
        self.assertEqual(opener.calls[2]["url"], BASE + "/v1/agents/" + A1 + "/collect-fees/confirm")


class TestAccount(unittest.TestCase):
    def test_paths_and_methods(self):
        c, opener = client({})
        c.call_preview(SOL)
        c.post_call(SOL, "gm")
        c.delete_call("c1")
        c.push_settings()
        c.set_push_prefs({"rebalance": True})
        c.push_subscribe("https://fcm.googleapis.com/x", p256dh="p", auth="a")
        c.push_unsubscribe("https://fcm.googleapis.com/x")
        c.push_test()
        c.set_pnl_card({"background": "dark"})
        c.set_referral_card({"background": "dark"})
        c.set_referral_code("abc")
        c.apply_creator(handle="me")
        c.set_leaderboard_privacy(visible=True, show_name=False, show_wallet=False)
        c.set_agent_visibility(A1, True)
        c.clear_logs()
        c.telegram_status()
        got = [(x["method"], x["url"].replace(BASE, "")) for x in opener.calls]
        self.assertEqual(got, [
            ("GET", "/v1/calls/preview?mint=" + SOL),
            ("POST", "/v1/calls"),
            ("DELETE", "/v1/calls/c1"),
            ("GET", "/v1/push"),
            ("PUT", "/v1/push/prefs"),
            ("POST", "/v1/push/subscriptions"),
            ("DELETE", "/v1/push/subscriptions"),
            ("POST", "/v1/push/test"),
            ("PUT", "/v1/pnl-card"),
            ("PUT", "/v1/referral-card"),
            ("POST", "/v1/referral/code"),
            ("POST", "/v1/referral/creator/apply"),
            ("PUT", "/v1/leaderboard-privacy"),
            ("PUT", "/v1/agents/" + A1 + "/leaderboard-visibility"),
            ("DELETE", "/v1/logs"),
            ("GET", "/v1/telegram"),
        ])
        self.assertEqual(opener.calls[5]["body"], {"endpoint": "https://fcm.googleapis.com/x", "keys": {"p256dh": "p", "auth": "a"}})

    def test_rank_presets(self):
        c, opener = client({"rows": [{"id": "t1", "netPct": 1.2}], "cached": False})
        r = c.rank_presets("meteora_dlmm", "POOL", window_days=7)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/presets/rank")
        self.assertEqual(opener.calls[0]["body"]["window_days"], 7)
        self.assertEqual(r.rows[0]["id"], "t1")


if __name__ == "__main__":
    unittest.main()
