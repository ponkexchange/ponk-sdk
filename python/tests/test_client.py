"""Offline tests: a stub opener stands in for the network.

They pin the three things a client can get wrong without anyone noticing: the
method and path it sends, the fields it parses, and the exception it raises for
each status of the shared error envelope.

    cd python && python3 -m pytest        (or: python3 -m unittest discover -s tests -v)
"""

from __future__ import annotations

import io
import json
import os
import sys
import unittest
import urllib.error

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from ponk import (  # noqa: E402
    PonkBadRequestError,
    PonkAuthError,
    PonkClient,
    PonkConflictError,
    PonkForbiddenError,
    PonkNotFoundError,
    PonkServerError,
    PonkUnprocessableError,
)


class _Response(io.BytesIO):
    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
        return False


class StubOpener:
    """Records the request it was given and replays a canned body."""

    def __init__(self, payload, status=200):
        self.payload = payload
        self.status = status
        self.calls = []

    def open(self, request, timeout=None):
        self.calls.append(
            {
                "method": request.get_method(),
                "url": request.full_url,
                "headers": dict(request.header_items()),
                "body": json.loads(request.data.decode()) if request.data else None,
                "timeout": timeout,
            }
        )
        body = json.dumps(self.payload).encode()
        if self.status >= 400:
            raise urllib.error.HTTPError(
                request.full_url, self.status, "error", {}, io.BytesIO(body)
            )
        return _Response(body)


def client(payload, status=200):
    opener = StubOpener(payload, status)
    return PonkClient(api_key="ponk_live_test", base_url="https://example.test/api", opener=opener), opener


AGENT = {
    "id": "018f0000-0000-7000-8000-000000000000",
    "name": "sol-usdc runner",
    "wallet_address": "GYdJ",
    "dex": "meteora_dlmm",
    "pool_address": "POOL",
    "position_address": None,
    "status": "running",
    "strategy": "bin_rebalancer",
    "dry_run": True,
    "config": {"kind": "bin_rebalancer", "bin_range_width": 20},
    "created_at": "2026-09-01T00:00:00Z",
    "updated_at": "2026-09-01T00:00:00Z",
    "mode": "auto",
    "wallet_source": "managed",
    "origin": "api",
    "managed_fee_bps": 1500,
}


class TestRequests(unittest.TestCase):
    def test_whoami_sends_bearer_key(self):
        c, opener = client({"wallet_address": "GYdJ", "scope": "trade", "key_id": "018f"})
        me = c.whoami()
        self.assertEqual(me.scope, "trade")
        call = opener.calls[0]
        self.assertEqual(call["method"], "GET")
        self.assertEqual(call["url"], "https://example.test/api/v1/me")
        self.assertEqual(call["headers"]["Authorization"], "Bearer ponk_live_test")

    def test_no_key_sends_no_authorization_header(self):
        opener = StubOpener({"status": "ok"})
        c = PonkClient(base_url="https://example.test/api", opener=opener)
        c.health()
        self.assertNotIn("Authorization", opener.calls[0]["headers"])

    def test_agent_parses_and_keeps_raw_json(self):
        c, _ = client(AGENT)
        agent = c.get_agent(AGENT["id"])
        self.assertEqual(agent.name, "sol-usdc runner")
        self.assertEqual(agent.managed_fee_bps, 1500)
        self.assertEqual(agent.config["bin_range_width"], 20)
        self.assertEqual(agent.raw_json, AGENT)

    def test_unknown_field_survives_in_raw_json(self):
        payload = dict(AGENT, brand_new_field=7)
        c, _ = client(payload)
        agent = c.get_agent(AGENT["id"])
        self.assertEqual(agent.raw_json["brand_new_field"], 7)

    def test_null_usd_stays_none_never_zero(self):
        c, _ = client({"pnl_usd": None, "current_value_usd": "1234.56", "pnl_source": "meteora_datapi"})
        perf = c.agent_performance("a")
        self.assertIsNone(perf.pnl_usd)
        self.assertEqual(perf.current_value_usd, "1234.56")

    def test_list_logs_passes_limit(self):
        c, opener = client([])
        c.list_logs(limit=20)
        self.assertEqual(opener.calls[0]["url"], "https://example.test/api/v1/logs?limit=20")

    def test_list_logs_omits_limit_when_unset(self):
        c, opener = client([])
        c.list_logs()
        self.assertEqual(opener.calls[0]["url"], "https://example.test/api/v1/logs")

    def test_create_agent_omits_unset_optionals(self):
        c, opener = client(AGENT)
        c.create_agent(
            name="n",
            wallet_address="GYdJ",
            dex="meteora_dlmm",
            strategy="bin_rebalancer",
            config={"kind": "bin_rebalancer"},
            pool_address="POOL",
            dry_run=True,
        )
        body = opener.calls[0]["body"]
        self.assertEqual(opener.calls[0]["method"], "POST")
        self.assertEqual(body["pool_address"], "POOL")
        self.assertNotIn("position_address", body)

    def test_create_agent_always_sends_dry_run_explicitly(self):
        # On /v1 an OMITTED dry_run means a LIVE agent (the app defaults to
        # true, the API to false). The client never leaves it to the server.
        for flag in (True, False):
            c, opener = client(AGENT)
            c.create_agent(
                name="n",
                wallet_address="GYdJ",
                dex="meteora_dlmm",
                strategy="bin_rebalancer",
                config={"kind": "bin_rebalancer"},
                pool_address="POOL",
                dry_run=flag,
            )
            self.assertIs(opener.calls[0]["body"]["dry_run"], flag)

    def test_create_agent_requires_dry_run(self):
        c, opener = client(AGENT)
        with self.assertRaises(TypeError):
            c.create_agent(  # type: ignore[call-arg]
                name="n",
                wallet_address="GYdJ",
                dex="meteora_dlmm",
                strategy="bin_rebalancer",
                config={"kind": "bin_rebalancer"},
                pool_address="POOL",
            )
        self.assertEqual(opener.calls, [])

    def test_create_agent_refuses_a_non_boolean_dry_run(self):
        # "true", 1 and None are all refused before anything is sent, the way
        # the server refuses them, rather than coerced into a live agent.
        for weird in ("true", 1, 0, None):
            c, opener = client(AGENT)
            with self.assertRaises(TypeError):
                c.create_agent(
                    name="n",
                    wallet_address="GYdJ",
                    dex="meteora_dlmm",
                    strategy="bin_rebalancer",
                    config={"kind": "bin_rebalancer"},
                    pool_address="POOL",
                    dry_run=weird,  # type: ignore[arg-type]
                )
            self.assertEqual(opener.calls, [])

    def test_agent_mandate(self):
        payload = {
            "mandate_id": "m-1",
            "status": "expired",
            "expires_at": "2026-09-01T00:00:00Z",
            "granted_at": "2026-08-02T00:00:00Z",
            "seconds_remaining": -86400,
            "actions": ["claim_fees", "rebalance"],
            "max_action_value_usd": None,
            "max_daily_transactions": 50,
            "max_slippage_bps": 100,
            "pool_allowlist": ["POOL"],
            "withdrawal_address": "GYdJ",
        }
        c, opener = client(payload)
        m = c.agent_mandate("a")
        self.assertEqual(opener.calls[0]["method"], "GET")
        self.assertEqual(opener.calls[0]["url"], "https://example.test/api/v1/agents/a/mandate")
        self.assertEqual(m.status, "expired")
        self.assertEqual(m.seconds_remaining, -86400)
        # null is "no per-action ceiling", and must stay None, never 0.
        self.assertIsNone(m.max_action_value_usd)
        self.assertEqual(m.actions, ["claim_fees", "rebalance"])
        self.assertEqual(m.pool_allowlist, ["POOL"])
        self.assertEqual(m.withdrawal_address, "GYdJ")

    def test_agent_mandate_404_means_not_autonomous(self):
        c, _ = client({"error": {"code": "not_found", "message": "not found"}}, status=404)
        with self.assertRaises(PonkNotFoundError):
            c.agent_mandate("a")

    def test_set_autonomous_sends_only_enabled(self):
        view = {
            "agent_id": "a",
            "wallet_source": "managed",
            "mode": "auto",
            "managed_wallet": "MW",
            "mandate_id": "m-1",
            "withdrawal_locked_to": "GYdJ",
            "status": "stopped",
            "dry_run": False,
        }
        c, opener = client(view)
        out = c.set_autonomous("a", True)
        call = opener.calls[0]
        self.assertEqual(call["method"], "POST")
        self.assertEqual(call["url"], "https://example.test/api/v1/agents/a/autonomous")
        # Never a grant: the server answers 422 to any body carrying one.
        self.assertEqual(call["body"], {"enabled": True})
        self.assertEqual(out.mode, "auto")
        self.assertEqual(out.status, "stopped")
        self.assertIs(out.dry_run, False)

    def test_set_autonomous_disable(self):
        c, opener = client({"agent_id": "a", "wallet_source": "external", "mode": "manual",
                            "managed_wallet": None, "mandate_id": None,
                            "withdrawal_locked_to": None, "status": "running", "dry_run": False})
        out = c.set_autonomous("a", False)
        self.assertEqual(opener.calls[0]["body"], {"enabled": False})
        self.assertEqual(out.wallet_source, "external")
        self.assertIsNone(out.mandate_id)

    def test_set_autonomous_refuses_a_non_boolean(self):
        c, opener = client({})
        with self.assertRaises(TypeError):
            c.set_autonomous("a", "true")  # type: ignore[arg-type]
        self.assertEqual(opener.calls, [])

    def test_set_autonomous_without_a_mandate_is_unprocessable(self):
        c, _ = client(
            {"error": {"code": "unprocessable", "message": "This agent's custody mandate expired"}},
            status=422,
        )
        with self.assertRaises(PonkUnprocessableError):
            c.set_autonomous("a", True)

    def test_range_cost(self):
        payload = {
            "pool_address": "POOL",
            "dex": "meteora_dlmm",
            "bin_step": 4,
            "active_bin_id": 100,
            "requested_bins": 2000,
            "total_bins": 1400,
            "max_total_bins": 1400,
            "clamped": True,
            "lower_bin_id": -600,
            "upper_bin_id": 799,
            "down_pct": -24.4,
            "up_pct": 32.3,
            "position_rent_lamports": 100,
            "bin_arrays_total": 21,
            "bin_arrays_existing": 3,
            "bin_arrays_missing": 18,
            "bin_array_rent_lamports": 200,
            "bin_array_probe": "probed",
            "resize_tx_count": 15,
            "tx_fee_lamports": 80000,
            "total_upfront_lamports": 80300,
            "refundable_lamports": 100,
            "non_refundable_lamports": 80200,
        }
        c, opener = client(payload)
        q = c.range_cost("meteora_dlmm", "POOL", 2000)
        self.assertEqual(
            opener.calls[0]["url"],
            "https://example.test/api/pools/meteora_dlmm/POOL/range-cost?bins=2000",
        )
        self.assertTrue(q.clamped)
        self.assertEqual(q.total_bins, 1400)
        self.assertEqual(q.bin_array_probe, "probed")
        self.assertEqual(q.non_refundable_lamports, 80200)

    def test_range_cost_off_meteora_is_unprocessable(self):
        c, _ = client({"error": {"code": "unprocessable", "message": "only Meteora"}}, status=422)
        with self.assertRaises(PonkUnprocessableError):
            c.range_cost("orca", "POOL", 70)

    def test_range_cost_zero_bins_is_bad_request(self):
        c, _ = client({"error": {"code": "bad_request", "message": "bins must be at least 1"}}, status=400)
        with self.assertRaises(PonkBadRequestError):
            c.range_cost("meteora_dlmm", "POOL", 0)

    def test_user_agent_carries_the_package_version(self):
        import ponk

        c, opener = client({"wallet_address": "GYdJ", "scope": "read", "key_id": "k"})
        c.whoami()
        self.assertEqual(
            opener.calls[0]["headers"]["User-agent"], "ponk-python/" + ponk.__version__
        )

    def test_withdraw_sends_lamports_and_no_destination(self):
        c, opener = client({"signature": "sig", "lamports": 500000000, "destination": "GYdJ", "source": "AGENT"})
        out = c.withdraw("a", lamports=500_000_000)
        self.assertEqual(out.destination, "GYdJ")
        self.assertEqual(opener.calls[0]["body"], {"lamports": 500000000})

    def test_fund_calls_use_the_longer_timeout(self):
        c, opener = client({"signature": "sig"})
        c.exit_agent("a")
        self.assertEqual(opener.calls[0]["timeout"], c.fund_timeout)
        self.assertGreater(c.fund_timeout, c.timeout)

    def test_path_segments_are_encoded(self):
        c, opener = client(AGENT)
        c.get_agent("../../health")
        self.assertEqual(
            opener.calls[0]["url"], "https://example.test/api/v1/agents/..%2F..%2Fhealth"
        )

    def test_nested_models(self):
        c, _ = client(
            {
                "position_address": "P",
                "in_range": False,
                "dex": "meteora_dlmm",
                "bins": [{"bin_id": 1, "weight": 0.5, "is_active": True, "in_target_range": True}],
                "price_samples": [1.0, 1.1],
            }
        )
        pos = c.agent_position("a")
        self.assertEqual(len(pos.bins), 1)
        self.assertTrue(pos.bins[0].is_active)
        self.assertEqual(pos.price_samples, [1.0, 1.1])

    def test_token_amount_is_exact(self):
        c, _ = client(
            [
                {
                    "id": "p",
                    "fees_claimable_x": {"raw": 1234567, "decimals": 6},
                    "claimable_fees_usd": None,
                }
            ]
        )
        positions = c.list_positions()
        self.assertEqual(str(positions[0].fees_claimable_x.to_decimal()), "1.234567")
        self.assertIsNone(positions[0].claimable_fees_usd)

    def test_perks_camel_case(self):
        c, _ = client(
            {
                "mint": "MINT",
                "thresholdUi": 1000000,
                "balanceUi": 0.0,
                "holds": False,
                "shortfallUi": 1000000.0,
                "discountPercent": 10,
                "apiAgentFee": {
                    "baseBps": 1500,
                    "effectiveBps": 1500,
                    "basePercent": 15.0,
                    "effectivePercent": 15.0,
                    "holderBps": 1350,
                    "holderPercent": 13.5,
                },
            }
        )
        perks = c.ponk_perks("GYdJ")
        self.assertEqual(perks.threshold_ui, 1000000)
        self.assertFalse(perks.holds)
        self.assertEqual(perks.api_agent_fee.holder_percent, 13.5)

    def test_pool_human_price(self):
        c, _ = client(
            {
                "dex": "meteora_dlmm",
                # A SOL(9)/USDC(6) pool at $150: raw is USDC base units per SOL
                # base unit, so the human price is raw * 10 ** (9 - 6).
                "current_price": "0.15",
                "token_x_decimals": 9,
                "token_y_decimals": 6,
            }
        )
        pool = c.pool("meteora_dlmm", "POOL")
        self.assertEqual(str(pool.human_price()), "150.00")

    def test_pool_human_price_is_none_when_unknown(self):
        c, _ = client({"dex": "orca", "current_price": None})
        self.assertIsNone(c.pool("orca", "POOL").human_price())


class TestErrors(unittest.TestCase):
    def envelope(self, code, message="nope", request_id="req-1"):
        return {"error": {"code": code, "message": message, "request_id": request_id}}

    def test_401(self):
        c, _ = client(self.envelope("unauthorized", "authentication required"), status=401)
        with self.assertRaises(PonkAuthError) as caught:
            c.whoami()
        self.assertEqual(caught.exception.status, 401)
        self.assertEqual(caught.exception.code, "unauthorized")
        self.assertEqual(caught.exception.request_id, "req-1")

    def test_403_read_only_key(self):
        c, _ = client(self.envelope("forbidden"), status=403)
        with self.assertRaises(PonkForbiddenError):
            c.pause_agent("a")

    def test_404(self):
        c, _ = client(self.envelope("not_found"), status=404)
        with self.assertRaises(PonkNotFoundError):
            c.get_agent("a")

    def test_409(self):
        c, _ = client(self.envelope("conflict", "exit already running"), status=409)
        with self.assertRaises(PonkConflictError) as caught:
            c.exit_agent("a")
        self.assertIn("exit already running", str(caught.exception))

    def test_422_keeps_the_risk_code(self):
        c, _ = client(self.envelope("token_2022_unsupported", "Orca cannot hold this mint"), status=422)
        with self.assertRaises(PonkUnprocessableError) as caught:
            c.compound("a")
        self.assertEqual(caught.exception.code, "token_2022_unsupported")

    def test_degraded_health_returns_its_report_instead_of_raising(self):
        # /health answers 503 with the full report of what is down, and that
        # report is the point of the call.
        report = {
            "status": "degraded",
            "time": "2026-09-17T00:00:00Z",
            "checks": {
                "database": {"healthy": True, "detail": None, "latency_ms": 3},
                "helius": {"healthy": False, "detail": "all keys capped", "latency_ms": None},
            },
        }
        opener = StubOpener(report, 503)
        c = PonkClient(base_url="https://example.test/api", opener=opener)
        health = c.health()
        self.assertEqual(health.status, "degraded")
        self.assertFalse(health.checks.helius.healthy)
        self.assertEqual(health.checks.helius.detail, "all keys capped")

    def test_a_503_error_envelope_still_raises(self):
        c, _ = client(self.envelope("internal_error", "internal error"), status=503)
        with self.assertRaises(PonkServerError):
            c.health()

    def test_500(self):
        c, _ = client(self.envelope("internal_error", "internal error"), status=500)
        with self.assertRaises(PonkServerError):
            c.list_agents()

    def test_non_json_body_from_a_proxy(self):
        opener = StubOpener(None, 502)
        opener.payload = "<html>502 Bad Gateway</html>"
        c = PonkClient(api_key="k", base_url="https://example.test/api", opener=opener)
        with self.assertRaises(PonkServerError) as caught:
            c.list_agents()
        self.assertEqual(caught.exception.code, "http_502")


if __name__ == "__main__":
    unittest.main()
