"""0.6.0 routes: the agent fleet (batch create, fund, exit, withdraw, the fleet
read) and the /v1 account parity routes.

Same stub-opener approach as test_client.py. Payloads use the exact field names
of the server's response structs (`routes/fleet.rs`, `routes/v1_parity.rs` and
the session handlers it delegates to).
"""

from __future__ import annotations

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from ponk import (  # noqa: E402
    DEFAULT_FUND_TIMEOUT,
    FleetBatch,
    FundPlan,
    PonkConflictError,
)
from tests.test_client import AGENT, client  # noqa: E402
from tests.test_social import SequenceOpener  # noqa: E402
from ponk import PonkClient  # noqa: E402

BASE = "https://example.test/api"
A1 = "018f0000-0000-7000-8000-000000000001"
A2 = "018f0000-0000-7000-8000-000000000002"

BATCH = {
    "batch_id": "b0000000-0000-4000-8000-000000000000",
    "kind": "create",
    "total": 2,
    "done": 1,
    "failed": 0,
    "running": 1,
    "items": [
        {
            "index": 0,
            "agent_id": A1,
            "outcome": "done",
            "ok": True,
            "status": 200,
            "code": None,
            "error": None,
            "result": {"agent": dict(AGENT, id=A1), "wallet": {"pubkey": "W1", "status": "active", "created": True}},
        },
        {
            "index": 1,
            "agent_id": None,
            "outcome": "running",
            "ok": False,
            "status": 202,
            "code": "running",
            "error": "Still running on the server.",
            "result": None,
        },
    ],
}

PLAN = {
    "owner": "OWNER",
    "ok": True,
    "refusal": None,
    "balance_lamports": 5_000_000_000,
    "total_lamports": 300_000_000,
    "network_fee_lamports": 5000,
    "recent_blockhash": "HASH",
    "last_valid_block_height": 123,
    "transactions": [
        {
            "index": 0,
            "unsigned_tx_b64": "AQAB",
            "transfers": [
                {"agent_id": A1, "to": "W1", "lamports": 150_000_000},
                {"agent_id": A2, "to": "W2", "lamports": 150_000_000},
            ],
            "total_lamports": 300_000_000,
            "simulated": True,
            "units_consumed": 300,
        }
    ],
    "items": [
        {"agent_id": A1, "ok": True, "wallet": "W1", "lamports": 150_000_000, "needs_mandate": True, "refusal": None},
        {"agent_id": A2, "ok": True, "wallet": "W2", "lamports": 150_000_000, "needs_mandate": True, "refusal": None},
    ],
    "needs_mandate": [A1, A2],
    "sign_urls": ["https://ponk.exchange/agents/fund?a=" + A1 + ":150000000," + A2 + ":150000000"],
}


def fleet_page(rows, offset, next_offset, total):
    return {"total": total, "offset": offset, "limit": 1, "next_offset": next_offset, "rows": rows}


ROW = {
    "agent": AGENT,
    "wallet": {"pubkey": "W1", "status": "active", "lamports": 10, "withdrawable_lamports": 5, "token_accounts_with_value": None},
    "mandate": {"status": "active", "seconds_remaining": 86400},
    "performance": {"pnl_usd": None, "current_value_usd": "12.00"},
    "read_errors": [],
}


class TestFleetBatches(unittest.TestCase):
    def test_create_agents_posts_the_batch_and_parses_items(self):
        c, opener = client(BATCH)
        batch = c.create_agents(
            [
                {"name": "a", "dex": "meteora_dlmm", "strategy": "bin_rebalancer", "config": {}, "pool_address": "P", "dry_run": True},
                {"name": "b", "dex": "meteora_dlmm", "strategy": "bin_rebalancer", "config": {}, "pool_address": "P", "dry_run": True},
            ]
        )
        call = opener.calls[0]
        self.assertEqual(call["method"], "POST")
        self.assertEqual(call["url"], BASE + "/v1/agents/batch")
        self.assertEqual(call["body"]["create_wallets"], True)
        self.assertEqual(len(call["body"]["agents"]), 2)
        self.assertEqual(call["timeout"], DEFAULT_FUND_TIMEOUT)
        self.assertIsInstance(batch, FleetBatch)
        self.assertEqual((batch.done, batch.running), (1, 1))
        self.assertEqual(batch.items[0].agent().id, A1)
        self.assertIsNone(batch.items[1].agent())
        self.assertEqual(batch.items[1].outcome, "running")

    def test_create_agents_refuses_an_item_without_dry_run(self):
        c, opener = client(BATCH)
        with self.assertRaises(TypeError):
            c.create_agents([{"name": "a", "dex": "meteora_dlmm", "strategy": "x", "config": {}}])
        self.assertEqual(opener.calls, [])

    def test_fund_prepare_sends_ids_only_and_parses_the_plan(self):
        c, opener = client(PLAN)
        plan = c.fund_prepare([A1, {"agent_id": A2, "lamports": 150_000_000}])
        call = opener.calls[0]
        self.assertEqual(call["url"], BASE + "/v1/agents/fund/prepare")
        self.assertEqual(call["body"], {"transfers": [{"agent_id": A1}, {"agent_id": A2, "lamports": 150_000_000}]})
        self.assertIsInstance(plan, FundPlan)
        self.assertTrue(plan.ok)
        self.assertEqual(len(plan.transactions), 1)
        self.assertEqual(plan.transactions[0].transfers[1].to, "W2")
        self.assertEqual(plan.needs_mandate, [A1, A2])
        self.assertTrue(plan.sign_urls[0].startswith("https://ponk.exchange/agents/fund?a="))

    def test_a_refused_plan_is_a_result_not_an_exception(self):
        c, _ = client(dict(PLAN, ok=False, refusal="Your wallet holds 0.0100 SOL.", transactions=[]))
        plan = c.fund_prepare([A1])
        self.assertFalse(plan.ok)
        self.assertIn("SOL", plan.refusal)

    def test_exit_and_withdraw_target_named_agents_or_all(self):
        c, opener = client(dict(BATCH, kind="exit"))
        c.exit_agents([A1, A2])
        c.exit_agents(all_agents=True)
        c.withdraw_agents([A1])
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/batch/exit")
        self.assertEqual(opener.calls[0]["body"], {"agent_ids": [A1, A2]})
        self.assertEqual(opener.calls[1]["body"], {"all": True})
        self.assertEqual(opener.calls[2]["url"], BASE + "/v1/agents/batch/withdraw")
        self.assertEqual(opener.calls[0]["timeout"], DEFAULT_FUND_TIMEOUT)

    def test_a_batch_target_must_be_named_or_all_not_both(self):
        c, opener = client(BATCH)
        with self.assertRaises(ValueError):
            c.exit_agents()
        with self.assertRaises(ValueError):
            c.exit_agents([A1], all_agents=True)
        with self.assertRaises(TypeError):
            c.withdraw_agents(A1)
        self.assertEqual(opener.calls, [])

    def test_get_batch_path(self):
        c, opener = client(BATCH)
        c.get_batch("b0000000-0000-4000-8000-000000000000")
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/batch/b0000000-0000-4000-8000-000000000000")

    def test_an_exit_item_exposes_its_withdrawal(self):
        item = dict(BATCH["items"][0], result={"signature": "SIG", "lamports": 9, "destination": "OWNER", "source": "W1"})
        c, _ = client(dict(BATCH, kind="exit", items=[item]))
        w = c.get_batch("b").items[0].withdrawal()
        self.assertEqual((w.signature, w.destination), ("SIG", "OWNER"))


class TestFleetRead(unittest.TestCase):
    def test_fleet_page_parses_typed_parts(self):
        c, opener = client(fleet_page([ROW], 0, None, 1))
        page = c.fleet(limit=50)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/fleet?limit=50")
        row = page.rows[0]
        self.assertEqual(row.agent.id, AGENT["id"])
        self.assertEqual(row.wallet.withdrawable_lamports, 5)
        self.assertIsNone(row.wallet.token_accounts_with_value)
        self.assertEqual(row.mandate.seconds_remaining, 86400)
        self.assertIsNone(row.performance.pnl_usd)

    def test_iter_fleet_follows_next_offset_to_the_end(self):
        opener = SequenceOpener(
            [
                fleet_page([ROW], 0, 1, 3),
                fleet_page([ROW], 1, 2, 3),
                fleet_page([dict(ROW, wallet=None)], 2, None, 3),
            ]
        )
        c = PonkClient(api_key="k", base_url=BASE, opener=opener)
        rows = c.fleet_all(limit=1)
        self.assertEqual(len(rows), 3)
        self.assertIsNone(rows[2].wallet)
        self.assertEqual(
            [call["url"] for call in opener.calls],
            [
                BASE + "/v1/agents/fleet?limit=1",
                BASE + "/v1/agents/fleet?offset=1&limit=1",
                BASE + "/v1/agents/fleet?offset=2&limit=1",
            ],
        )

    def test_create_agent_wallet_posts(self):
        c, opener = client({"pubkey": "W1", "status": "active", "created_at": "2026-10-23T00:00:00Z", "created": False})
        w = c.create_agent_wallet(A1)
        self.assertEqual(opener.calls[0]["method"], "POST")
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/" + A1 + "/wallet")
        self.assertFalse(w.created)


class TestAgentParity(unittest.TestCase):
    def test_rebalance_sends_the_range_on_the_fund_timeout(self):
        c, opener = client({"action_id": "x", "signature": None, "status": "failed", "action_status": "failed", "position_address": None, "risk_approved": False, "risk_failed_checks": ["cooldown"]})
        r = c.rebalance(A1, new_lower_bin_id=-10, new_upper_bin_id=10, strategy="spot")
        call = opener.calls[0]
        self.assertEqual(call["url"], BASE + "/v1/agents/" + A1 + "/rebalance")
        self.assertEqual(call["body"], {"new_lower_bin_id": -10, "new_upper_bin_id": 10, "strategy": "spot"})
        self.assertEqual(call["timeout"], DEFAULT_FUND_TIMEOUT)
        self.assertIsNone(r.signature)
        self.assertEqual(r.risk_failed_checks, ["cooldown"])

    def test_collect_fees_sends_a_destination_only_when_chosen(self):
        c, opener = client({"action_id": "x", "status": "confirmed", "payout": None})
        c.collect_fees(A1)
        c.collect_fees(A1, destination="connected")
        self.assertIsNone(opener.calls[0]["body"])
        self.assertEqual(opener.calls[1]["body"], {"destination": "connected"})
        with self.assertRaises(ValueError):
            c.collect_fees(A1, destination="SomeAddress")

    def test_rename_and_delete(self):
        c, opener = client(dict(AGENT, name="new"))
        self.assertEqual(c.rename_agent(A1, "new").name, "new")
        self.assertEqual(opener.calls[0]["method"], "PATCH")
        self.assertEqual(opener.calls[0]["body"], {"name": "new"})
        c, opener = client(None)
        self.assertIsNone(c.delete_agent(A1))
        self.assertEqual(opener.calls[0]["method"], "DELETE")
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/" + A1)

    def test_a_refused_delete_raises_conflict(self):
        c, _ = client({"error": {"code": "conflict", "message": "exit first"}}, status=409)
        with self.assertRaises(PonkConflictError):
            c.delete_agent(A1)

    def test_proof_and_range_history(self):
        c, opener = client(
            {
                "window_hours": 168,
                "time_in_range": {"in_range_secs": 10, "observed_secs": 20, "pct": 50.0, "readings": 3},
                "actions": [{"kind": "rebalance", "costs": {"network": {"usd": "0.01", "reason": None}, "swap": {"usd": None, "reason": "no swap"}, "total_usd": None}}],
                "earned_usd": None,
            }
        )
        proof = c.agent_proof(A1, hours=24)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/" + A1 + "/proof?hours=24")
        self.assertEqual(proof.time_in_range.pct, 50.0)
        self.assertEqual(proof.actions[0].costs.swap.reason, "no swap")
        self.assertIsNone(proof.earned_usd)
        c, opener = client({"range": "7d", "points": [{"t": "2026-10-23T00:00:00Z", "active": 1, "lower": 0, "upper": 2, "in_range": True}]})
        h = c.agent_range_history(A1, range="7d")
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/" + A1 + "/range-history?range=7d")
        self.assertTrue(h.points[0].in_range)

    def test_custody_routes(self):
        c, opener = client({"signature": "S", "lamports": 1, "destination": "OWNER", "source": "SHARED"})
        c.custody_withdraw()
        c.custody_exit()
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/custody/withdraw")
        self.assertEqual(opener.calls[0]["body"], {})
        self.assertEqual(opener.calls[1]["url"], BASE + "/v1/custody/exit")
        self.assertEqual(opener.calls[1]["timeout"], DEFAULT_FUND_TIMEOUT)


class TestAccountParity(unittest.TestCase):
    def test_portfolio_and_history(self):
        c, opener = client({"total_pnl_usd": 1.5, "il_usd": None, "positions": [{"dex": "orca", "value_usd": None}], "series": [{"t": 1, "value_usd": 2.0}]})
        p = c.portfolio()
        self.assertIsNone(p.il_usd)
        self.assertIsNone(p.positions[0].value_usd)
        self.assertEqual(p.series[0].t, 1)
        c, opener = client({"range": "30d", "points": [], "days": [{"date": "2026-10-22", "pnl_usd": 1.0}]})
        h = c.portfolio_history(range="30d")
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/portfolio/history?range=30d")
        self.assertEqual(h.days[0].date, "2026-10-22")

    def test_lists(self):
        for method, path, payload in (
            ("list_transactions", "/v1/transactions?limit=5", [{"id": "t", "signature": "S"}]),
            ("list_risk_events", "/v1/risk-events?limit=5", [{"id": "r", "code": "slippage"}]),
            ("list_fees", "/v1/fees?limit=5", [{"id": "f", "platform_fee_usd": None}]),
        ):
            c, opener = client(payload)
            rows = getattr(c, method)(limit=5)
            self.assertEqual(opener.calls[0]["url"], BASE + path)
            self.assertEqual(len(rows), 1)
        c, _ = client({"total_platform_fees_usd": "1.00", "event_count": 3})
        self.assertEqual(c.fee_summary().event_count, 3)

    def test_notifications(self):
        c, opener = client([{"id": "n", "status": "unread", "metadata": {"k": 1}}])
        self.assertEqual(c.list_notifications(status="unread")[0].metadata, {"k": 1})
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/notifications?status=unread")
        c, _ = client({"unread": 4})
        self.assertEqual(c.unread_notifications().unread, 4)
        c, opener = client({"marked": 4})
        self.assertEqual(c.mark_all_notifications_read().marked, 4)
        self.assertEqual(opener.calls[0]["method"], "POST")
        c, opener = client(None)
        c.mark_notification_read("n1")
        c.dismiss_notification("n1")
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/notifications/n1/read")
        self.assertEqual(opener.calls[1]["url"], BASE + "/v1/notifications/n1/dismiss")

    def test_recommendations(self):
        c, opener = client({"id": "r1", "status": "approved", "transaction_preview": None})
        c.get_recommendation("r1")
        c.review_recommendation("r1")
        self.assertEqual(c.approve_recommendation("r1").status, "approved")
        c.dismiss_recommendation("r1")
        c.snooze_recommendation("r1", minutes=60)
        urls = [call["url"] for call in opener.calls]
        self.assertEqual(
            urls,
            [
                BASE + "/v1/recommendations/r1",
                BASE + "/v1/recommendations/r1/review",
                BASE + "/v1/recommendations/r1/approve",
                BASE + "/v1/recommendations/r1/dismiss",
                BASE + "/v1/recommendations/r1/snooze",
            ],
        )
        self.assertEqual(opener.calls[4]["body"], {"minutes": 60})

    def test_settings_and_presets(self):
        c, opener = client({"rebalance_telegram": False, "mandate_telegram": True, "weekly_recap_telegram": True})
        c.update_notify_settings(rebalance_telegram=False)
        self.assertEqual(opener.calls[0]["method"], "PUT")
        self.assertEqual(opener.calls[0]["body"], {"rebalance_telegram": False})
        c, opener = client({"id": "p1", "name": "tight", "overrides": {"bin_range_width": 10}, "dex": None})
        c.create_preset("tight", {"bin_range_width": 10})
        c.update_preset("p1", "tight", {"bin_range_width": 12}, dex="orca")
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/presets")
        self.assertEqual(opener.calls[0]["body"], {"name": "tight", "overrides": {"bin_range_width": 10}, "dex": None})
        self.assertEqual(opener.calls[1]["method"], "PUT")
        self.assertEqual(opener.calls[1]["url"], BASE + "/v1/presets/p1")
        c, opener = client(None)
        c.delete_preset("p1")
        self.assertEqual(opener.calls[0]["method"], "DELETE")


if __name__ == "__main__":
    unittest.main()
