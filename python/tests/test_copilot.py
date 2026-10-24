"""The copilot routes: session only, the exact paths and bodies, and the
shapes parsed from the server's own field names (`routes/copilot.rs`).
"""

from __future__ import annotations

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from ponk import (  # noqa: E402
    DEFAULT_FUND_TIMEOUT,
    CopilotFleet,
    CopilotPlan,
    CopilotStart,
    FleetBatch,
    FundPlan,
    PonkClient,
    PonkError,
)
from tests.test_social import SequenceOpener  # noqa: E402

BASE = "https://example.test/api"
PLAN = "11111111-2222-4333-8444-555555555555"
FLEET = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee"
SESSION = "session-token"
KEY = "ponk_live_test"

READY = {
    "plan_id": PLAN,
    "state": "ready",
    "progress": "Ready.",
    "text": "put 2 sol into 4 agents",
    "expires_in_secs": 800,
    "plan": {"legs": [], "totals": {"agents": 4}},
    "refusal": None,
    "replay": {"state": "idle", "done": 0, "total": 0, "result": None},
    "can_start": True,
    "poll_after_ms": None,
}
DONE = dict(
    READY,
    replay={
        "state": "done",
        "done": 4,
        "total": 4,
        "result": {"totals": {"net_pnl_usd": 3.1, "hold_pnl_usd": 5.4, "net_vs_hold_usd": -2.3}},
    },
)


def session_client(payloads):
    opener = SequenceOpener(payloads)
    return PonkClient(api_key=KEY, base_url=BASE, session_token=SESSION, opener=opener), opener


class TestCopilot(unittest.TestCase):
    def test_copilot_sends_the_session_never_the_key(self):
        c, opener = session_client([{"ok": True, "intent": {"kind": "deploy"}, "echo": "Start."}])
        parsed = c.copilot_parse("put 2 sol into 4 agents")
        self.assertTrue(parsed.ok)
        call = opener.calls[0]
        self.assertEqual((call["method"], call["url"]), ("POST", BASE + "/copilot/parse"))
        self.assertEqual(call["body"], {"text": "put 2 sol into 4 agents"})
        self.assertEqual(call["headers"]["Authorization"], "Bearer " + SESSION)

    def test_a_key_alone_never_reaches_a_copilot_route(self):
        opener = SequenceOpener([{}])
        c = PonkClient(api_key=KEY, base_url=BASE, opener=opener)
        with self.assertRaises(PonkError):
            c.copilot_plan("put 2 sol into 4 agents")
        self.assertEqual(opener.calls, [])

    def test_wait_starts_the_replay_once_and_returns_when_done(self):
        c, opener = session_client([READY, READY, DONE])
        slept = []
        plan = c.copilot_wait(PLAN, sleep=slept.append)
        self.assertIsInstance(plan, CopilotPlan)
        self.assertEqual(plan.replay.state, "done")
        self.assertEqual(plan.replay.result["totals"]["net_vs_hold_usd"], -2.3)
        urls = [(x["method"], x["url"]) for x in opener.calls]
        self.assertEqual(
            urls,
            [
                ("GET", BASE + "/copilot/plans/" + PLAN),
                ("POST", BASE + "/copilot/plans/" + PLAN + "/replay"),
                ("GET", BASE + "/copilot/plans/" + PLAN),
            ],
        )

    def test_start_parses_the_unsigned_funding_and_the_signing_page(self):
        body = {
            "fleet_id": FLEET,
            "already_started": False,
            "created": {"batch_id": "b", "kind": "create", "total": 1, "done": 1, "failed": 0, "running": 0, "items": []},
            "funding": {"owner": "W", "ok": True, "total_lamports": 10, "transactions": [{"unsigned_tx_b64": "AA"}], "sign_urls": ["https://ponk.exchange/agents/fund?a=x:10"]},
            "mandate_queue": [PLAN],
            "sign_url": "https://ponk.exchange/agents/fund?a=x:10",
            "next": "Sign 1 funding transaction.",
        }
        c, opener = session_client([body])
        s = c.copilot_start(PLAN)
        self.assertIsInstance(s, CopilotStart)
        self.assertIsInstance(s.created, FleetBatch)
        self.assertIsInstance(s.funding, FundPlan)
        self.assertEqual(s.funding.transactions[0].unsigned_tx_b64, "AA")
        self.assertEqual(s.mandate_queue, [PLAN])
        self.assertEqual(opener.calls[0]["url"], BASE + "/copilot/plans/" + PLAN + "/start")
        self.assertEqual(opener.calls[0]["timeout"], DEFAULT_FUND_TIMEOUT)

    def test_fleet_read_stop_and_tighten(self):
        fleet = {
            "fleet": {"id": FLEET, "status": "active", "agents": 1, "stop_drawdown_pct": 10.0},
            "stop_now": {"state": "holding"},
            "totals": {"value_usd": None, "cost_basis_usd": None, "pnl_usd": None, "unvalued": 1},
            "rows": [],
            "missing_agents": [],
            "exit": {"done": 0, "failed": 1, "running": 0, "failed_agents": [{"agent_id": "a", "name": "Fleet 1 SOL-USDC", "error": "Short of gas."}]},
            "funding": {"unfunded": 0, "mandates_needed": 1, "creating": 0, "sign_url": "https://ponk.exchange/agents/fund?a=a:0", "agents": []},
        }
        batch = {"batch_id": "b", "kind": "exit", "total": 1, "done": 0, "failed": 0, "running": 1, "items": []}
        c, opener = session_client([[{"id": FLEET, "status": "active"}], fleet, batch, {"id": FLEET, "stop_drawdown_pct": 5.0}])
        self.assertEqual(c.copilot_fleets()[0].id, FLEET)
        f = c.copilot_fleet(FLEET)
        self.assertIsInstance(f, CopilotFleet)
        self.assertIsNone(f.totals["value_usd"])
        self.assertEqual(f.fleet.status, "active")
        self.assertEqual(f.exit["failed_agents"][0]["name"], "Fleet 1 SOL-USDC")
        self.assertEqual(f.funding["mandates_needed"], 1)
        self.assertEqual(c.copilot_stop_fleet(FLEET).running, 1)
        self.assertEqual(c.copilot_tighten_stop(FLEET, 5).stop_drawdown_pct, 5.0)
        self.assertEqual(opener.calls[2]["url"], BASE + "/copilot/fleets/" + FLEET + "/stop")
        self.assertEqual(opener.calls[3]["body"], {"drawdown_pct": 5})
        self.assertTrue(all(x["headers"]["Authorization"] == "Bearer " + SESSION for x in opener.calls))


    def test_wait_stops_on_an_expired_plan(self):
        expired = {
            "plan_id": PLAN,
            "state": "expired",
            "refusal": {"code": "plan_expired", "message": "This plan expired."},
            "replay": {"state": "idle"},
        }
        c, opener = session_client([expired])
        plan = c.copilot_wait(PLAN, sleep=lambda _s: None)
        self.assertEqual(plan.state, "expired")
        self.assertEqual(plan.refusal["code"], "plan_expired")
        self.assertEqual(len(opener.calls), 1)


if __name__ == "__main__":
    unittest.main()
