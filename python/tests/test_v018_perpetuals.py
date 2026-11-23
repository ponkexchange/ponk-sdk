"""0.18.0: Perpetuals on Hyperliquid.

Same stub-opener approach as test_client.py: no test here opens a socket. The
payloads are written field for field from the API's own answers
(`routes/perp.rs` market views, `routes/perp_agents.rs` `summary_view`,
`get_one`, `ticket_view`, `create_in`, `owner_request` and the exit card,
`routes/perp_copilot.rs` `PerpPlanView` and `PerpFleetBody`, and
`routes/copilot.rs` `StopRuleView`), with illustrative values.
"""

from __future__ import annotations

import json
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import ponk  # noqa: E402
from ponk import (  # noqa: E402
    CopilotFleet,
    FleetBatch,
    PerpAgent,
    PerpBoard,
    PerpCreated,
    PerpExitCard,
    PerpFleetPlan,
    PerpLedger,
    PerpSaid,
    PonkClient,
    PonkUnprocessableError,
    perpetuals,
)
from tests.test_client import StubOpener, client  # noqa: E402

BASE = "https://example.test/api"
AGENT = "0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b"
FLEET = "6f1d2c3b-4a59-4e6f-8a7b-9c0d1e2f3a4b"
PLAN = "7a1d2c3b-4a59-4e6f-8a7b-9c0d1e2f3a4b"
HL = "0x1111111111111111111111111111111111111111"

WARNING = "Above 10x a sudden 4% price jump can pass the liquidation price before any stop fills."
STOP_ROOM = (
    "Liquidation is about 8.9% away (7.1% with the safety margin Liquidation Guard keeps), less than "
    "twice your 4% stop: a fast move can pass liquidation before the stop fills, and in this zone "
    "Liquidation Guard may cut part of the position before the stop does. A stop of 3.5% or less, or "
    "lower leverage, keeps twice the room."
)


def keyless(payload, status=200):
    opener = StubOpener(payload, status)
    return PonkClient(base_url=BASE, opener=opener), opener


class TestVersion(unittest.TestCase):
    def test_version_is_0_18_0(self):
        self.assertEqual(ponk.__version__, "0.18.0")


class TestHelpers(unittest.TestCase):
    def test_leverage_is_not_capped_and_the_warning_is_the_servers(self):
        self.assertEqual(perpetuals.HIGH_LEVERAGE_WARNING, WARNING)
        self.assertEqual(ponk.HIGH_LEVERAGE_WARNING, WARNING)
        self.assertEqual(perpetuals.HIGH_LEVERAGE_ABOVE, 10)
        self.assertFalse(hasattr(perpetuals, "AGENT_MAX_LEVERAGE"))
        self.assertFalse(hasattr(perpetuals, "leverage_refusal"))
        self.assertEqual(perpetuals.check_leverage(20), 20)
        self.assertEqual(perpetuals.check_leverage(50), 50)
        self.assertEqual(perpetuals.check_leverage(10), 10)
        self.assertEqual(perpetuals.check_leverage(3.0), 3)
        for bad in (0, -1, 2.5, "3", True, None):
            with self.assertRaises(ValueError):
                perpetuals.check_leverage(bad)

    def test_needs_confirmation_above_10x(self):
        self.assertFalse(perpetuals.needs_confirmation(1))
        self.assertFalse(perpetuals.needs_confirmation(10))
        self.assertTrue(perpetuals.needs_confirmation(11))
        self.assertTrue(perpetuals.needs_confirmation(40))
        with self.assertRaises(ValueError):
            perpetuals.needs_confirmation(0)
        with self.assertRaises(ValueError):
            perpetuals.needs_confirmation(True)

    def test_market_ids(self):
        self.assertEqual(perpetuals.market_id("BTC"), "perp:hyperliquid:BTC")
        self.assertEqual(perpetuals.market_id("kPEPE"), "perp:hyperliquid:kPEPE")
        self.assertEqual(perpetuals.market_id("perp:hyperliquid:ETH"), "perp:hyperliquid:ETH")
        self.assertEqual(perpetuals.coin_of("perp:hyperliquid:SOL"), "SOL")
        with self.assertRaises(ValueError):
            perpetuals.market_id("  ")

    def test_direction(self):
        self.assertEqual(perpetuals.check_direction("short"), "short")
        with self.assertRaises(ValueError) as ctx:
            perpetuals.check_direction("up")
        self.assertEqual(str(ctx.exception), "Say long or short.")


BOARD = {
    "venue": "hyperliquid",
    "asOf": 1763900000000,
    "servedAt": 1763900001000,
    "stale": False,
    "lastError": None,
    "fundingIntervalHours": 1,
    "nextFundingTime": 1763902800000,
    "total": 180,
    "markets": [
        {
            "id": "perp:hyperliquid:BTC", "venue": "hyperliquid", "coin": "BTC", "symbol": "BTC-USD",
            "szDecimals": 5, "priceDecimals": 1, "maxLeverage": 40, "leverageTiers": [[0, 40], [150000000, 20]],
            "isolatedOnly": False, "markPx": 97000.5, "oraclePx": 97010.0, "midPx": 97000.0,
            "prevDayPx": 95000.0, "change24hPct": 2.105, "volume24hUsd": 2500000000.0,
            "openInterest": 30000.1, "openInterestUsd": 2910000000.0, "fundingRate": 0.0000125,
            "fundingIntervalHours": 1, "nextFundingTime": 1763902800000, "fundingHourly": 0.0000125,
            "fundingAnnualized": 0.1095, "premium": None, "tradeUrl": "https://app.hyperliquid.xyz/trade/BTC",
        }
    ],
}


class TestMarkets(unittest.TestCase):
    def test_board_reads_camel_case_and_keeps_null(self):
        c, opener = keyless(BOARD)
        board = c.perp_markets("bt", sort="oi", limit=5)
        call = opener.calls[0]
        self.assertEqual(call["method"], "GET")
        self.assertTrue(call["url"].startswith(BASE + "/public/perp/markets?"))
        self.assertIn("q=bt", call["url"])
        self.assertIn("sort=oi", call["url"])
        self.assertNotIn("Authorization", call["headers"])
        self.assertIsInstance(board, PerpBoard)
        m = board.markets[0]
        self.assertEqual((m.coin, m.max_leverage, m.sz_decimals), ("BTC", 40, 5))
        self.assertEqual(m.change_24h_pct, 2.105)
        self.assertEqual(m.volume_24h_usd, 2500000000.0)
        self.assertIsNone(m.premium)
        self.assertEqual(board.next_funding_time, 1763902800000)

    def test_one_market_candles_funding(self):
        c, opener = keyless({"venue": "hyperliquid", "stale": False, "market": BOARD["markets"][0]})
        self.assertEqual(c.perp_market("perp:hyperliquid:BTC").market.trade_url, "https://app.hyperliquid.xyz/trade/BTC")
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/perp/markets/perp%3Ahyperliquid%3ABTC")

        c, opener = keyless({"id": "perp:hyperliquid:BTC", "coin": "BTC", "interval": "4h", "asOf": 1, "stale": False,
                             "historyLimit": 5000, "oldestAvailable": 0,
                             "candles": [{"t": 1, "o": 1.0, "h": 2.0, "l": 0.5, "c": 1.5, "v": 10.0, "n": 3}]})
        candles = c.perp_candles("BTC", "4h", 2)
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/perp/markets/BTC/candles?interval=4h&bars=2")
        self.assertEqual((candles.history_limit, candles.candles[0].l, candles.candles[0].n), (5000, 0.5, 3))

        c, opener = keyless({"id": "perp:hyperliquid:BTC", "coin": "BTC", "days": 3, "asOf": 1, "stale": False,
                             "fundingIntervalHours": 1, "nextFundingTime": 2, "currentRate": None,
                             "records": [{"time": 1, "rate": -0.00001, "annualized": -0.0876, "premium": None}]})
        f = c.perp_funding_history("BTC", 3)
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/perp/markets/BTC/funding?days=3")
        self.assertIsNone(f.current_rate)
        self.assertEqual(f.records[0].rate, -0.00001)

    def test_wallet_reads_and_book(self):
        pos = {"market": "perp:hyperliquid:ETH", "coin": "ETH", "side": "short", "size": 2.0, "entryPx": 3100.0,
               "markPx": 3000.0, "positionValue": 6000.0, "unrealizedPnl": 200.0, "returnOnEquity": 0.1,
               "liquidationPx": None, "marginUsed": 600.0, "marginMode": "cross", "leverage": 10,
               "cumFundingSinceOpen": -1.2}
        c, opener = keyless({"address": HL, "account": {"accountValue": 1000.0, "withdrawable": None},
                             "positions": [pos], "asOfMs": 5, "stale": False})
        w = c.perp_wallet(HL)
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/wallets/{0}/perp".format(HL))
        self.assertIsNone(w.positions[0].liquidation_px)
        self.assertEqual(w.positions[0].cum_funding_since_open, -1.2)

        c, opener = keyless({"address": HL, "markets": [{"market": "perp:hyperliquid:ETH", "coin": "ETH",
                             "realizedPnl": 10.0, "fees": 1.0, "net": 9.0, "volumeUsd": 100.0, "fills": 4,
                             "closes": 1, "liquidations": 0, "unrealizedPnl": None}],
                             "summary": {"realizedPnl": 10.0, "fees": 1.0, "net": 9.0, "unrealizedPnl": None,
                                         "volumeUsd": 100.0, "fills": 4, "closes": 1, "liquidations": 0,
                                         "windows": {"d1": None, "d7": 9.0, "d30": 9.0},
                                         "coveredFromMs": 1, "fullWindow": False},
                             "excludes": ["funding"], "windowDays": 30, "fillsRead": 4, "readAt": "2026-11-23T00:00:00Z"})
        p = c.perp_wallet_pnl(HL)
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/wallets/{0}/perp/pnl".format(HL))
        self.assertEqual(p.excludes, ["funding"])
        self.assertFalse(p.summary.full_window)
        self.assertIsNone(p.summary.windows["d1"])

        c, opener = keyless({"scope": "curated", "market": "BTC", "rows": [{"wallet": {"address": HL}, "position": pos,
                             "asOf": None}], "markets": [{"market": "perp:hyperliquid:BTC", "coin": "BTC", "longs": 0,
                             "shorts": 1, "longUsd": None, "shortUsd": 6000.0}], "walletsInScope": 30,
                             "walletsRead": 29, "readAt": "2026-11-23T00:00:00Z"})
        b = c.perp_book("BTC")
        self.assertEqual(opener.calls[0]["url"], BASE + "/public/wallets/perp/book?market=BTC")
        self.assertIsNone(b.markets[0].long_usd)
        self.assertEqual(b.rows[0].position.side, "short")
        self.assertEqual(b.wallets_in_scope, 30)

    def test_parse(self):
        c, opener = keyless({"ok": False, "sentence": "Say long or short."})
        p = c.perp_parse("btc 3x")
        self.assertEqual(opener.calls[0]["body"], {"text": "btc 3x"})
        self.assertFalse(p.ok)
        self.assertEqual(p.sentence, "Say long or short.")


TICKET = {
    "sentence": "Long BTC 3x with $500, stop 3% at $94,090.",
    "side": "long", "coin": "BTC", "leverage": 3, "marginUsd": 500.0, "notionalUsd": 1500.0,
    "size": 0.01546, "liquidationPrice": 65000.0, "liquidationMove": 0.33, "stopPrice": 94090.0,
    "stopFraction": 0.03, "stopDefaulted": False, "lossAtStopUsd": 45.0, "takeProfitPrice": None,
    "highLeverage": False,
}
CREATED = {
    "id": AGENT, "network": "testnet", "practice": False, "ticket": TICKET["sentence"],
    "liquidationPrice": 65000.0, "stopPrice": 94090.0, "ticketFigures": TICKET,
    "fees": {"activationBps": 150, "performanceBps": 1500, "exitBps": 100},
    "next": "Authorize the agent, then fund its Hyperliquid account.",
}


class TestCreate(unittest.TestCase):
    def test_create_posts_the_camel_case_body(self):
        c, opener = client(CREATED)
        a = c.create_perp_agent(coin="BTC", direction="long", leverage=3, capital_usd="500",
                                stop_loss_pct=3, take_profit_pct=10, name="btc runner")
        call = opener.calls[0]
        self.assertEqual((call["method"], call["url"]), ("POST", BASE + "/v1/perp-agents"))
        self.assertEqual(call["body"], {"coin": "BTC", "direction": "long", "leverage": 3, "capitalUsd": "500",
                                        "stopLossPct": 3, "takeProfitPct": 10, "name": "btc runner",
                                        "practice": False})
        self.assertEqual(call["headers"]["Authorization"], "Bearer ponk_live_test")
        self.assertIsInstance(a, PerpCreated)
        self.assertEqual(a.ticket_figures.loss_at_stop_usd, 45.0)
        self.assertIsNone(a.ticket_figures.take_profit_price)
        self.assertEqual(a.fees.exit_bps, 100)

    def test_above_10x_is_sent_with_the_typed_name(self):
        created = dict(CREATED, warnings=[WARNING + " At 20x the liquidation is 4.6% away."])
        c, opener = client(created)
        a = c.create_perp_agent(coin="BTC", direction="long", leverage=20, capital_usd=500, stop_loss_pct=3,
                                name="BTC 20x", confirmation_text="BTC 20x")
        self.assertEqual(opener.calls[0]["body"]["leverage"], 20)
        self.assertEqual(opener.calls[0]["body"]["confirmationText"], "BTC 20x")
        self.assertTrue(a.warnings[0].startswith(WARNING))
        c, opener = client({"ok": True, "network": "testnet", "practice": False, "name": "BTC",
                            "ticket": dict(TICKET, leverage=20, highLeverage=True), "fees": None,
                            "needsConfirmation": True, "warnings": [WARNING, "The long side is crowded."]})
        p = c.preview_perp_agent(coin="BTC", direction="long", leverage=20, capital_usd=500, stop_loss_pct=3)
        self.assertNotIn("confirmationText", opener.calls[0]["body"])
        self.assertTrue(p.needs_confirmation)
        self.assertEqual(p.warnings, [WARNING, "The long side is crowded."])
        self.assertTrue(p.ticket.high_leverage)

    def test_the_shape_is_still_checked_before_sending(self):
        c, opener = client(CREATED)
        for bad in (0, -3, 2.5):
            with self.assertRaises(ValueError):
                c.create_perp_agent(coin="BTC", direction="long", leverage=bad, capital_usd=500, stop_loss_pct=3)
        with self.assertRaises(ValueError):
            c.create_perp_agent(coin="BTC", direction="sideways", leverage=2, capital_usd=500, stop_loss_pct=3)
        self.assertEqual(opener.calls, [])

    def test_an_unacknowledged_high_leverage_is_the_servers_sentence(self):
        msg = WARNING + " To go ahead at 25x, type the agent's name (BTC) to confirm."
        c, _ = client({"error": {"code": "unprocessable", "request_id": "r", "message": msg}}, status=422)
        with self.assertRaises(PonkUnprocessableError) as ctx:
            c.create_perp_agent(coin="BTC", direction="long", leverage=25, capital_usd=500, stop_loss_pct=3)
        self.assertIn("type the agent's name (BTC)", str(ctx.exception))

    def test_a_refused_ticket_is_the_servers_sentence(self):
        body = {"error": {"code": "unprocessable", "request_id": "r",
                          "message": "Hyperliquid's smallest order is $10, so start with at least $10."}}
        c, _ = client(body, status=422)
        with self.assertRaises(PonkUnprocessableError) as ctx:
            c.create_perp_agent(coin="BTC", direction="long", leverage=2, capital_usd=5, stop_loss_pct=3)
        self.assertIn("smallest order", str(ctx.exception))

    def test_preview_ok_and_refused(self):
        c, opener = client({"ok": True, "network": "mainnet", "practice": True, "name": "BTC",
                            "ticket": TICKET, "fees": None, "needsConfirmation": False})
        p = c.preview_perp_agent(coin="BTC", direction="long", leverage=3, capital_usd=500, stop_loss_pct=3,
                                 practice=True)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/perp-agents/preview")
        self.assertTrue(opener.calls[0]["body"]["practice"])
        self.assertTrue(p.ok)
        self.assertIsNone(p.fees)
        self.assertEqual(p.ticket.size, 0.01546)
        # An older server sends no warnings: the list is empty.
        self.assertEqual(p.warnings, [])
        c, _ = client({"ok": False, "sentence": "The liquidation sits too close to the stop."})
        p = c.preview_perp_agent(coin="BTC", direction="long", leverage=3, capital_usd=500, stop_loss_pct=3)
        self.assertFalse(p.ok)
        self.assertIsNone(p.ticket)

    def test_say_answers_either_shape(self):
        c, opener = client(CREATED)
        s = c.perp_say("long btc 3x 500 stop 3")
        self.assertEqual(opener.calls[0]["body"], {"text": "long btc 3x 500 stop 3"})
        self.assertIsInstance(s, PerpSaid)
        self.assertTrue(s.created)
        c, opener = client({"agentId": AGENT, "request": "close",
                            "sentence": "Closing at your word. The agent keeps its funds on Hyperliquid and opens nothing new."})
        s = c.perp_say("close my btc long", confirm=True)
        self.assertEqual(opener.calls[0]["body"], {"text": "close my btc long", "confirm": True})
        self.assertFalse(s.created)
        self.assertEqual((s.agent_id, s.request), (AGENT, "close"))

    def test_say_above_10x_sends_the_typed_name(self):
        c, opener = client(CREATED)
        c.perp_say("long btc 20x 500 stop 1", confirmation_text="BTC")
        self.assertEqual(opener.calls[0]["body"], {"text": "long btc 20x 500 stop 1", "confirmationText": "BTC"})


AGENT_VIEW = {
    "account": {
        "network": "testnet", "practice": False, "coin": "BTC", "readAtMs": 10, "ageMs": 2000, "stale": False,
        "bookingPending": False,
        "balances": {"perpsValueUsd": "512.4", "withdrawableUsd": "12.1", "spotUsdc": None, "accountMode": "standard"},
        "position": {"side": "long", "size": "0.01546", "entryPrice": "97000", "markPrice": "97100",
                     "notionalUsd": "1501.1", "unrealizedPnlUsd": "1.5", "marginUsd": "501.5",
                     "marginUsage": 0.06, "liquidationPrice": 65000.0, "stopPrice": "94090"},
    },
    "exitFeeBps": 100,
    "id": AGENT,
    "status": "running",
    "agentAccount": HL,
    "config": {"network": "testnet", "coin": "BTC", "direction": "long", "leverage": 3, "capitalUsd": "500",
               "stopLossPct": "3", "takeProfitPct": None, "practice": False},
    "decision": {"codes": ["HOLD"], "sentences": ["Holding: nothing is worth paying for."], "sent": [],
                 "at": "2026-11-23T00:00:00Z"},
    "realized": None,
    "exit": None,
}


class TestAgentReads(unittest.TestCase):
    def test_agent(self):
        c, opener = client(AGENT_VIEW)
        a = c.perp_agent(AGENT)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/perp-agents/" + AGENT)
        self.assertIsInstance(a, PerpAgent)
        self.assertEqual(a.account.balances.perps_value_usd, "512.4")
        self.assertIsNone(a.account.balances.spot_usdc)
        self.assertEqual(a.account.position.margin_usage, 0.06)
        self.assertEqual(a.config.stop_loss_pct, "3")
        self.assertEqual(a.decision.codes, ["HOLD"])
        self.assertIsNone(a.realized)

    def test_unavailable_account(self):
        c, opener = client({"unavailable": "The agent's Hyperliquid account is made when you authorize it."})
        acct = c.perp_agent_balances(AGENT)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/perp-agents/{0}/balances".format(AGENT))
        self.assertIn("authorize", acct.unavailable)
        self.assertIsNone(acct.balances)

    def test_list_ledger_decisions_card(self):
        c, opener = client([{"id": AGENT, "name": "BTC", "status": "running", "createdAt": "2026-11-23T00:00:00Z",
                             "network": "testnet", "coin": "BTC", "direction": "long", "leverage": 3,
                             "capitalUsd": "500", "practice": False, "realizedNetUsd": None,
                             "decision": {"code": "HOLD", "at": "2026-11-23T00:00:00Z"}, "exiting": False,
                             "bookingPending": True}])
        rows = c.perp_agents()
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/perp-agents")
        self.assertIsNone(rows[0].realized_net_usd)
        self.assertTrue(rows[0].booking_pending)

        ledger = {"orders": [{"purpose": "stop", "side": "sell", "type": "trigger", "size": "0.01546",
                              "reduceOnly": True, "status": "resting", "sentence": None, "at": "t"}],
                  "fills": [{"coin": "BTC", "side": "buy", "px": "97000", "sz": "0.01546", "dir": "Open Long",
                             "closedPnl": "0", "fee": "0.52", "liquidation": False, "timeMs": 1}],
                  "funding": [{"coin": "BTC", "usdc": "-0.01", "fundingRate": "0.0000125", "timeMs": 2}],
                  "fees": [{"kind": "activation", "recipient": "treasury", "amountUsd": "7.5", "status": "confirmed",
                            "at": "t"}],
                  "withdrawals": []}
        c, opener = client(ledger)
        l = c.perp_agent_ledger(AGENT)
        self.assertIsInstance(l, PerpLedger)
        self.assertTrue(l.orders[0].reduce_only)
        self.assertEqual(l.fills[0].closed_pnl, "0")
        self.assertEqual(l.funding[0].usdc, "-0.01")
        self.assertEqual(l.fees[0].amount_usd, "7.5")

        c, opener = client([{"codes": ["WAITING_FOR_FUNDS"], "sentences": ["Waiting for funds."], "sent": [],
                             "practice": False, "at": "t"}])
        d = c.perp_agent_decisions(AGENT)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/perp-agents/{0}/decisions".format(AGENT))
        self.assertEqual(d[0].codes, ["WAITING_FOR_FUNDS"])

        c, opener = client({"kind": "perp", "agent_id": AGENT, "venue": "hyperliquid", "venue_name": "Hyperliquid",
                            "market": "BTC-USD", "coin": "BTC", "side": "long", "leverage": 3,
                            "high_leverage": False, "practice": False, "network": "testnet", "pnl_usd": None,
                            "pnl_pct": None, "pnl_unavailable": "A transfer is still settling.", "note": None,
                            "capital_usd": 500.0, "fees_lifetime_usd": None, "il_usd": None, "swap_fee_bps": None,
                            "bin_step": None, "referral_code": None, "opened_at": "t", "closed_at": None,
                            "duration_secs": None, "position_open": False, "realized": True, "liquidated": False,
                            "exit_signature": None})
        card = c.perp_agent_exit_card(AGENT)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/perp-agents/{0}/exit-card".format(AGENT))
        self.assertIsInstance(card, PerpExitCard)
        self.assertIsNone(card.pnl_usd)
        self.assertEqual(card.pnl_unavailable, "A transfer is still settling.")


class TestOwnerActions(unittest.TestCase):
    def test_close_needs_confirm(self):
        c, opener = client({"agentId": AGENT, "request": "close", "sentence": "Closing at your word."})
        with self.assertRaises(ValueError):
            c.close_perp_agent(AGENT, confirm=False)
        self.assertEqual(opener.calls, [])
        a = c.close_perp_agent(AGENT, confirm=True)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/perp-agents/{0}/close".format(AGENT))
        self.assertEqual(opener.calls[0]["body"], {"confirm": True})
        self.assertEqual(a.request, "close")

    def test_tighten_stop(self):
        c, opener = client({"agentId": AGENT, "request": "tighten_stop", "sentence": "Moving the stop."})
        c.tighten_perp_stop(AGENT, 2)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/perp-agents/{0}/stop".format(AGENT))
        self.assertEqual(opener.calls[0]["body"], {"stopPct": 2})

    def test_exit_needs_confirm_and_takes_the_fund_timeout(self):
        c, opener = client({"status": "requested", "destination": HL, "exitFeeBps": 100, "sentence": "Closing."})
        with self.assertRaises(ValueError):
            c.exit_perp_agent(AGENT, confirm="yes")
        self.assertEqual(opener.calls, [])
        e = c.exit_perp_agent(AGENT, confirm=True)
        call = opener.calls[0]
        self.assertEqual((call["url"], call["body"]), (BASE + "/v1/perp-agents/{0}/exit".format(AGENT), {"confirm": True}))
        self.assertEqual(call["timeout"], c.fund_timeout)
        self.assertEqual((e.status, e.destination, e.exit_fee_bps), ("requested", HL, 100))

    def test_pause_is_the_shared_route(self):
        c, opener = client({"id": AGENT, "status": "paused", "kind": "perps", "chain": "hyperliquid"})
        a = c.pause_agent(AGENT)
        self.assertEqual(opener.calls[0]["url"], BASE + "/v1/agents/{0}/pause".format(AGENT))
        self.assertEqual(a.status, "paused")


PERP_PLAN = {
    "id": PLAN, "kind": "perp", "state": "ready", "text": "put $1,000 across 2 Perpetuals agents, long BTC, short SOL",
    "echo": "2 agents, $500 each: long BTC 2x (default), short SOL 2x (default), stop 3% (default).",
    "refusal": None, "network": "testnet", "practice": False,
    "legs": [
        {"index": 0, "name": "BTC Long 2x", "coin": "BTC", "direction": "long", "leverage": 2, "leverage_defaulted": True,
         "capital_usd": "500", "stop_pct": 3.0, "stop_defaulted": True, "take_profit_pct": None, "ticket": TICKET,
         "high_leverage": False, "needs_confirmation": False, "note": None, "refusal": None},
        {"index": 1, "name": "SOL Short 2x", "coin": "SOL", "direction": "short", "leverage": 2,
         "leverage_defaulted": True, "capital_usd": "500", "stop_pct": 3.0, "stop_defaulted": True,
         "take_profit_pct": None, "ticket": None, "high_leverage": False, "needs_confirmation": False,
         "note": None, "refusal": "Funding is crowded on the short side."},
    ],
    "totals": {"agents": 1, "capital_usd": "500", "notional_usd": 1000.0, "loss_at_stops_usd": 30.0},
    "fleet_stop_pct": 10.0, "fees": {"activationBps": 150}, "fees_for": "api",
    "signatures": "Each agent is authorized with its own signature.", "notes": [], "can_start": True,
    "fleet_id": None, "agents": [], "sign_url": None,
}


class TestCopilot(unittest.TestCase):
    def test_plan_sends_the_key_when_held(self):
        c, opener = client(PERP_PLAN)
        p = c.copilot_perp_plan("put $1,000 across 2 Perpetuals agents, long BTC, short SOL")
        call = opener.calls[0]
        self.assertEqual((call["method"], call["url"]), ("POST", BASE + "/copilot/perp/plans"))
        self.assertEqual(call["headers"]["Authorization"], "Bearer ponk_live_test")
        self.assertIsInstance(p, PerpFleetPlan)
        self.assertEqual(p.legs[1].refusal, "Funding is crowded on the short side.")
        self.assertEqual(p.totals.loss_at_stops_usd, 30.0)
        self.assertEqual(p.fees_for, "api")

    def test_plan_is_public_without_a_credential(self):
        c, opener = keyless(PERP_PLAN)
        c.copilot_perp_plan("long btc", practice=True)
        self.assertNotIn("Authorization", opener.calls[0]["headers"])
        self.assertEqual(opener.calls[0]["body"], {"text": "long btc", "practice": True})

    def test_start(self):
        started = dict(PERP_PLAN, state="started", fleet_id=FLEET,
                       agents=[{"index": 0, "name": "BTC Long 2x", "agent_id": AGENT, "created": True}],
                       sign_url="https://ponk.exchange/copilot/fleets/" + FLEET)
        c, opener = client(started)
        p = c.copilot_perp_start(PLAN)
        call = opener.calls[0]
        self.assertEqual(call["url"], BASE + "/copilot/perp/plans/{0}/start".format(PLAN))
        self.assertEqual(call["timeout"], c.fund_timeout)
        self.assertEqual(call["body"], {})
        self.assertEqual((p.state, p.fleet_id), ("started", FLEET))
        self.assertTrue(p.agents[0]["created"])

    def test_start_sends_the_typed_names_by_leg_index(self):
        c, opener = client(dict(PERP_PLAN, state="started", fleet_id=FLEET))
        c.copilot_perp_start(PLAN, confirmations={0: "BTC Long 2x", "1": "SOL Short 2x"})
        self.assertEqual(opener.calls[0]["body"], {"confirmations": {"0": "BTC Long 2x", "1": "SOL Short 2x"}})

    def test_perp_fleet_live(self):
        fleet = {
            "kind": "perp",
            "fleet": {"id": FLEET, "kind": "perp", "intent_text": "x", "status": "active", "agents": 2,
                      "stop_drawdown_pct": 10.0, "stop_armed": True},
            "stop_now": {"state": "holding"}, "totals": {}, "rows": [], "missing_agents": [], "exit": None,
            "funding": {}, "as_of": "t", "poll_after_ms": 5000,
            "perp": {
                "network": "testnet", "practice": False, "exit_fee_bps": 100,
                "rows": [{"agent_id": AGENT, "name": "BTC Long 2x", "status": "running", "coin": "BTC",
                          "direction": "long", "leverage": 2, "practice": False, "value_usd": None,
                          "pnl_usd": None, "pnl_reason": "Closed, settling.", "booking_pending": True,
                          "position": None, "mandate": "active", "funded": True, "needs": "nothing",
                          "close": {"state": "open", "sentence": None}}],
                "totals": {"value_usd": None, "notional_usd": 0.0, "margin_usd": 0.0, "open_positions": 0,
                           "reason": "An agent is settling."},
                "funding": {"to_authorize": 1, "to_fund": 1, "sign_url": None, "sentence": None},
            },
        }
        c, _ = client(fleet)
        f = c.copilot_fleet(FLEET)
        self.assertIsInstance(f, CopilotFleet)
        self.assertEqual((f.kind, f.fleet.kind), ("perp", "perp"))
        self.assertEqual(f.perp.rows[0].pnl_reason, "Closed, settling.")
        self.assertIsNone(f.perp.totals.value_usd)
        self.assertEqual(f.perp.funding.to_authorize, 1)

    def test_agent_stops_read_answers_beside_the_count(self):
        # The newer server names the list `answers`; `agents` stays the count.
        payload = {"id": FLEET, "kind": "perp", "intent_text": "x", "status": "active", "agents": 2,
                   "stop_drawdown_pct": 10.0,
                   "answers": [{"agent_id": AGENT, "name": "BTC Long 2x", "ok": False,
                                "sentence": "The stop is already tighter."}]}
        c, _ = client(payload)
        s = c.copilot_tighten_agent_stops(FLEET, 2.0)
        self.assertEqual(s.answers[0]["sentence"], "The stop is already tighter.")
        self.assertEqual(s.agents, 2)

    def test_agent_stops_prefer_answers_when_both_lists_arrive(self):
        payload = {"id": FLEET, "kind": "perp", "status": "active",
                   "agents": [{"agent_id": "old", "ok": True}],
                   "answers": [{"agent_id": "new", "ok": True}]}
        c, _ = client(payload)
        s = c.copilot_tighten_agent_stops(FLEET, 2.0)
        self.assertEqual([a["agent_id"] for a in s.answers], ["new"])
        self.assertIsNone(s.agents)

    def test_agent_stops_read_the_older_agents_list(self):
        # An older StopRuleView flattens the fleet summary, whose `agents` is
        # a count, beside the answers list under the same key: the list is
        # written last, so a JSON reader keeps the list.
        raw = ('{"id":"%s","kind":"perp","intent_text":"x","status":"active","agents":2,'
               '"stop_drawdown_pct":10.0,"agents":[{"agent_id":"%s","name":"BTC Long 2x","ok":true,'
               '"sentence":"Moving the stop at your word on the next tick."}]}' % (FLEET, AGENT))
        c, opener = client(json.loads(raw))
        s = c.copilot_tighten_agent_stops(FLEET, 2.0)
        self.assertEqual(opener.calls[0]["url"], BASE + "/copilot/fleets/{0}/stop-rule".format(FLEET))
        self.assertEqual(opener.calls[0]["body"], {"stop_pct": 2.0})
        self.assertEqual(s.answers[0]["ok"], True)
        self.assertIsNone(s.agents)
        self.assertEqual(s.kind, "perp")

    def test_pause_and_resume_fleet(self):
        batch = {"batch_id": FLEET, "kind": "pause", "total": 1, "done": 1, "failed": 0, "running": 0,
                 "unknown": 0, "skipped": 0, "items": [{"index": 0, "agent_id": AGENT, "outcome": "done",
                                                         "ok": True, "status": 200, "code": "paused"}]}
        c, opener = client(batch)
        b = c.copilot_pause_fleet(FLEET)
        self.assertEqual(opener.calls[0]["url"], BASE + "/copilot/fleets/{0}/pause".format(FLEET))
        self.assertIsInstance(b, FleetBatch)
        c.copilot_resume_fleet(FLEET)
        self.assertEqual(opener.calls[1]["url"], BASE + "/copilot/fleets/{0}/resume".format(FLEET))


STOP_ROOM_TICKET = dict(TICKET, leverage=5, highLeverage=False, needsConfirmation=True,
                        stopRoomWarning=STOP_ROOM, atrWarning=None)
STOP_ROOM_PREVIEW = {"ok": True, "network": "testnet", "practice": False, "name": "BTC",
                     "ticket": STOP_ROOM_TICKET, "fees": None, "needsConfirmation": True,
                     "warnings": ["The long side is crowded.", STOP_ROOM],
                     "confirmationWarnings": [STOP_ROOM]}


class TestStopRoomAcknowledgement(unittest.TestCase):
    """The stop rule: a stop inside liquidation but with liquidation closer
    than twice its distance is a warning acknowledged with the typed name,
    at any leverage; the preview is the authority."""

    def test_the_preview_is_the_authority_at_5x(self):
        c, opener = client(STOP_ROOM_PREVIEW)
        p = c.preview_perp_agent(coin="BTC", direction="long", leverage=5, capital_usd=200, stop_loss_pct=4)
        self.assertNotIn("confirmationText", opener.calls[0]["body"])
        self.assertTrue(p.needs_confirmation)
        self.assertEqual(p.confirmation_warnings, [STOP_ROOM])
        self.assertEqual(p.warnings, ["The long side is crowded.", STOP_ROOM])
        # The leverage alone says no; the preview says yes.
        self.assertFalse(perpetuals.needs_confirmation(5))
        self.assertTrue(perpetuals.needs_confirmation(p))
        self.assertTrue(perpetuals.needs_confirmation(STOP_ROOM_PREVIEW))
        self.assertTrue(perpetuals.needs_confirmation(p.ticket))
        t = p.ticket
        self.assertTrue(t.needs_confirmation)
        self.assertEqual(t.stop_room_warning, STOP_ROOM)
        self.assertIsNone(t.atr_warning)

    def test_needs_confirmation_reads_the_server_flag_not_the_leverage(self):
        self.assertFalse(perpetuals.needs_confirmation({"ok": True, "needsConfirmation": False}))
        self.assertTrue(perpetuals.needs_confirmation({"needs_confirmation": True}))
        # A refused preview, or an older answer without the flag, is no.
        self.assertFalse(perpetuals.needs_confirmation({"ok": False, "sentence": "x"}))
        c, _ = client({"ok": True, "name": "BTC", "ticket": dict(TICKET, leverage=20), "needsConfirmation": True,
                       "warnings": [WARNING], "confirmationWarnings": [WARNING]})
        p = c.preview_perp_agent(coin="BTC", direction="long", leverage=20, capital_usd=200, stop_loss_pct=1)
        self.assertTrue(perpetuals.needs_confirmation(p))
        c, _ = client({"ok": True, "name": "BTC", "ticket": TICKET, "needsConfirmation": False})
        p = c.preview_perp_agent(coin="BTC", direction="long", leverage=3, capital_usd=200, stop_loss_pct=3)
        self.assertFalse(perpetuals.needs_confirmation(p))
        self.assertEqual(p.confirmation_warnings, [])
        self.assertIsNone(p.ticket.stop_room_warning)

    def test_create_sends_the_name_at_5x_and_reads_the_ticket(self):
        created = dict(CREATED, ticketFigures=STOP_ROOM_TICKET, warnings=[STOP_ROOM])
        c, opener = client(created)
        a = c.create_perp_agent(coin="BTC", direction="long", leverage=5, capital_usd=200, stop_loss_pct=4,
                                confirmation_text="btc")
        self.assertEqual(opener.calls[0]["body"]["confirmationText"], "btc")
        self.assertEqual(opener.calls[0]["body"]["leverage"], 5)
        self.assertTrue(a.ticket_figures.needs_confirmation)
        self.assertEqual(a.ticket_figures.stop_room_warning, STOP_ROOM)
        self.assertEqual(a.warnings, [STOP_ROOM])

    def test_create_without_the_name_is_the_servers_sentence(self):
        msg = STOP_ROOM + " To go ahead, type the agent's name (BTC) to confirm."
        c, _ = client({"error": {"code": "unprocessable", "request_id": "r", "message": msg}}, status=422)
        with self.assertRaises(PonkUnprocessableError) as ctx:
            c.create_perp_agent(coin="BTC", direction="long", leverage=5, capital_usd=200, stop_loss_pct=4)
        self.assertIn("type the agent's name (BTC) to confirm", str(ctx.exception))

    def test_a_stop_past_liquidation_is_still_refused(self):
        c, _ = client({"ok": False, "sentence": "A 12% stop sits past the liquidation price (8.9% away). "
                                                "A stop of 7% or less, or lower leverage, would work."})
        p = c.preview_perp_agent(coin="BTC", direction="long", leverage=10, capital_usd=200, stop_loss_pct=12)
        self.assertFalse(p.ok)
        self.assertFalse(perpetuals.needs_confirmation(p))
        self.assertIn("would work", p.sentence)

    def test_say_sends_the_typed_name_at_any_leverage(self):
        c, opener = client(dict(CREATED, warnings=[STOP_ROOM]))
        s = c.perp_say("long btc 5x 200 stop 4", confirmation_text="BTC")
        self.assertEqual(opener.calls[0]["body"], {"text": "long btc 5x 200 stop 4", "confirmationText": "BTC"})
        self.assertTrue(s.created)
        msg = (STOP_ROOM + " To go ahead, send the same line again with confirmationText set to the agent's "
               "name, BTC, typed by the user. confirm: true alone does not acknowledge it. Nothing was created.")
        c, opener = client({"error": {"code": "unprocessable", "request_id": "r", "message": msg}}, status=422)
        with self.assertRaises(PonkUnprocessableError) as ctx:
            c.perp_say("long btc 5x 200 stop 4", confirm=True)
        self.assertEqual(opener.calls[0]["body"], {"text": "long btc 5x 200 stop 4", "confirm": True})
        self.assertIn("Nothing was created", str(ctx.exception))

    def test_mixed_case_tickers_are_sent_as_given(self):
        c, opener = client(dict(CREATED, ticketFigures=dict(TICKET, coin="kPEPE")))
        c.create_perp_agent(coin="kPEPE", direction="long", leverage=20, capital_usd=50, stop_loss_pct=2,
                            name="kPEPE Long 20x", confirmation_text="KPEPE Long 20x")
        body = opener.calls[0]["body"]
        self.assertEqual((body["coin"], body["name"], body["confirmationText"]),
                         ("kPEPE", "kPEPE Long 20x", "KPEPE Long 20x"))
        self.assertEqual(perpetuals.market_id("kPEPE"), "perp:hyperliquid:kPEPE")

    def test_agent_config_warnings(self):
        view = dict(AGENT_VIEW, config=dict(AGENT_VIEW["config"], leverage=5, warnings=[STOP_ROOM]))
        c, _ = client(view)
        self.assertEqual(c.perp_agent(AGENT).config.warnings, [STOP_ROOM])
        c, _ = client(AGENT_VIEW)
        self.assertEqual(c.perp_agent(AGENT).config.warnings, [])

    def test_plan_legs_carry_their_confirmation_warnings(self):
        leg = dict(PERP_PLAN["legs"][0], name="BTC Long 5x", leverage=5, stop_pct=4.0,
                   ticket=STOP_ROOM_TICKET, needs_confirmation=True, confirmation_warnings=[STOP_ROOM])
        c, _ = client(dict(PERP_PLAN, legs=[leg, PERP_PLAN["legs"][1]]))
        p = c.copilot_perp_plan("long btc 5x stop 4")
        self.assertTrue(p.legs[0].needs_confirmation)
        self.assertTrue(perpetuals.needs_confirmation(p.legs[0]))
        self.assertEqual(p.legs[0].confirmation_warnings, [STOP_ROOM])
        self.assertEqual(p.legs[0].ticket["stopRoomWarning"], STOP_ROOM)
        self.assertTrue(p.legs[0].ticket["needsConfirmation"])
        # An older plan has no list: it is empty.
        self.assertEqual(p.legs[1].confirmation_warnings, [])
        self.assertFalse(perpetuals.needs_confirmation(p.legs[1]))


if __name__ == "__main__":
    unittest.main()
