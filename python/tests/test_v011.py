"""0.11.0: people search, who to follow, every mandate at once, and
accept_pool_risk on create.

Same stub-opener approach as test_client.py. Payloads use the server's field
names (apps/api/src/routes/social.rs, recommend.rs, fleet.rs, agents.rs).
"""

from __future__ import annotations

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from ponk import Mandates, PeopleSearch, Recommendations  # noqa: E402
from tests.test_client import AGENT, client  # noqa: E402
from tests.test_people import PERSON  # noqa: E402

BASE = "https://example.test/api"


class TestV011(unittest.TestCase):
    def test_people_search_pages_by_cursor(self):
        c, opener = client({"people": [dict(PERSON, followers=12)], "next_cursor": "20"})
        r = c.people_search("@ka", cursor="0")
        self.assertIsInstance(r, PeopleSearch)
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/people-search?q=%40ka&cursor=0")
        self.assertEqual(r.people[0].handle, "kader")
        self.assertEqual(r.people[0].followers, 12)
        self.assertEqual(r.next_cursor, "20")

    def test_recommended_and_similar(self):
        row = dict(PERSON, followers=3, reason="Followed by @a", reasons=[{"kind": "followed_by", "text": "Followed by @a"}])
        c, opener = client({"people": [row], "basis": "signals"})
        r = c.recommended_people(limit=5)
        c.similar_people("@kader")
        c.my_recommended()
        self.assertIsInstance(r, Recommendations)
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/social/recommended?limit=5")
        self.assertEqual(opener.calls[1]["url"], BASE + "/public/people/kader/similar")
        self.assertEqual(opener.calls[2]["url"], BASE + "/v1/social/recommended")
        self.assertEqual(r.people[0].reason, "Followed by @a")
        self.assertEqual(r.people[0].reasons[0]["kind"], "followed_by")

    def test_dismiss_recommended(self):
        c, opener = client({"key": "kader", "dismissed": True})
        c.dismiss_recommended("@kader")
        self.assertEqual(opener.calls[0]["method"], "POST")
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/social/recommended/kader/dismiss")

    def test_mandates(self):
        c, opener = client(
            {
                "expiring_within_days": 7,
                "needs_renewal": 1,
                "renew_all_url": "https://ponk.exchange/agents?renew=all",
                "rows": [{"agent_id": "a1", "name": "x", "state": "expired", "expires_at": None, "seconds_remaining": -5, "renew_url": "u"}],
            }
        )
        m = c.mandates()
        self.assertIsInstance(m, Mandates)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/mandates")
        self.assertEqual(m.rows[0].state, "expired")
        self.assertEqual(m.needs_renewal, 1)

    def test_accept_pool_risk_is_sent_only_when_asked(self):
        c, opener = client(AGENT)
        kw = dict(name="n", wallet_address="GYdJ", dex="meteora_dlmm", strategy="bin_rebalancer", config={}, pool_address="POOL", dry_run=True)
        c.create_agent(**kw)
        c.create_agent(accept_pool_risk=True, **kw)
        self.assertNotIn("accept_pool_risk", opener.calls[0]["body"])
        self.assertIs(opener.calls[1]["body"]["accept_pool_risk"], True)


if __name__ == "__main__":
    unittest.main()
