"""Exit to SOL (0.8.1): the `to_sol` choice on every exit call, the agent's
`exit_to_sol` setting, and the liquidation legs an exit reports.

Same stub-opener approach as test_client.py. Payloads use the exact field
names of the server's `WithdrawView` and `ExitToSolView`.
"""

from __future__ import annotations

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from ponk import ExitSwapLeg, ExitToSol, Withdrawal  # noqa: E402
from tests.test_client import client  # noqa: E402

BASE = "https://example.test/api"
A1 = "018f0000-0000-7000-8000-000000000001"

EXIT = {
    "signature": "S",
    "lamports": 1_000_000_000,
    "destination": "OWNER",
    "source": "AGENTWALLET",
    "exit_swaps": [
        {
            "mint": "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263",
            "amount": 500000000000,
            "decimals": 5,
            "outcome": "swapped",
            "signature": "SWAP",
            "sol_out_lamports": 151700817,
            "fee_lamports": 304009,
        },
        {
            "mint": "J4yJw6LtnDWaYEsu8FXw93xLQWoP6KErE5CtT82yQ8Sg",
            "amount": 999000000,
            "decimals": 6,
            "outcome": "kept",
            "reason": "there is no route to SOL for it",
        },
    ],
    "exit_notes": [
        "Swapped 1 leftover token to SOL. Ponk fee 0.2% of the swapped amount.",
        "Sent J4yJ...Q8Sg to your wallet as it is: there is no route to SOL for it.",
    ],
}


class TestExitToSol(unittest.TestCase):
    def test_exit_agent_sends_no_body_choice_by_default(self):
        c, opener = client(EXIT)
        c.exit_agent(A1)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/" + A1 + "/exit")
        self.assertEqual(opener.calls[0]["body"], {})

    def test_exit_agent_carries_keep_tokens_and_to_sol(self):
        c, opener = client(EXIT)
        c.exit_agent(A1, to_sol=False)
        c.exit_agent(A1, to_sol=True)
        self.assertEqual(opener.calls[0]["body"], {"to_sol": False})
        self.assertEqual(opener.calls[1]["body"], {"to_sol": True})

    def test_to_sol_must_be_a_bool(self):
        c, opener = client(EXIT)
        with self.assertRaises(TypeError):
            c.exit_agent(A1, to_sol="no")  # type: ignore[arg-type]
        self.assertEqual(opener.calls, [])

    def test_exit_agents_and_custody_exit_carry_the_choice(self):
        c, opener = client(dict(EXIT, batch_id="b", kind="exit", total=1, done=1, failed=0, running=0, items=[]))
        c.exit_agents([A1], to_sol=False)
        c.exit_agents(all_agents=True)
        c.custody_exit(to_sol=True)
        c.custody_exit()
        self.assertEqual(opener.calls[0]["body"], {"agent_ids": [A1], "to_sol": False})
        self.assertEqual(opener.calls[1]["body"], {"all": True})
        self.assertEqual(opener.calls[2]["url"], BASE + "/v1/custody/exit")
        self.assertEqual(opener.calls[2]["body"], {"to_sol": True})
        self.assertEqual(opener.calls[3]["body"], {})

    def test_a_withdrawal_never_sends_to_sol(self):
        c, opener = client(dict(EXIT, batch_id="b", kind="withdraw", total=1, done=1, failed=0, running=0, items=[]))
        c.withdraw_agents([A1])
        c.withdraw(A1)
        self.assertNotIn("to_sol", opener.calls[0]["body"])
        self.assertNotIn("to_sol", opener.calls[1]["body"])

    def test_the_exit_result_names_every_leg(self):
        c, _ = client(EXIT)
        w = c.exit_agent(A1)
        self.assertIsInstance(w, Withdrawal)
        self.assertEqual(len(w.exit_swaps), 2)
        self.assertIsInstance(w.exit_swaps[0], ExitSwapLeg)
        self.assertEqual(w.exit_swaps[0].outcome, "swapped")
        self.assertEqual(w.exit_swaps[0].fee_lamports, 304009)
        self.assertEqual(w.exit_swaps[1].outcome, "kept")
        self.assertIn("no route", w.exit_swaps[1].reason)
        self.assertIn("Ponk fee 0.2%", w.exit_notes[0])

    def test_a_keep_tokens_exit_parses_with_no_legs(self):
        c, _ = client({"signature": "S", "lamports": 1, "destination": "OWNER", "source": "W"})
        w = c.exit_agent(A1, to_sol=False)
        self.assertEqual(w.exit_swaps, [])
        self.assertEqual(w.exit_notes, [])

    def test_the_agent_setting_reads_and_writes(self):
        view = {"agent_id": A1, "exit_to_sol": False, "fee_line": "Swaps leftover tokens to SOL. Ponk fee 0.2% of the swapped amount."}
        c, opener = client(view)
        s = c.set_exit_to_sol(A1, False)
        g = c.get_exit_to_sol(A1)
        self.assertIsInstance(s, ExitToSol)
        self.assertFalse(g.exit_to_sol)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/" + A1 + "/exit-to-sol")
        self.assertEqual(opener.calls[0]["method"], "POST")
        self.assertEqual(opener.calls[0]["body"], {"exit_to_sol": False})
        self.assertEqual(opener.calls[1]["method"], "GET")
        with self.assertRaises(TypeError):
            c.set_exit_to_sol(A1, "yes")  # type: ignore[arg-type]


if __name__ == "__main__":
    unittest.main()
