"""0.16.0: the boards and trackers on every chain, EVM liquidity shapes, the exit fee.

Same stub-opener approach as test_client.py: no test here opens a socket. The
payloads are the files in `tests/fixtures/allchains`, written field for field
from the API's own Serialize structs (`LeaderboardView`, `CallView`,
`XCallView`, `FeedResponse`, `PreviewResponse`, `PostTokenView`,
`TrendingResponse`, `TokenResponse`, the wallet tracker's `FeedItem` and
`SourceView`, `WeeklyRecapView`, `FeedPrefsView`, `EvmPlanView` with
`LegShape`, the open-cost body with its `shape` block, `EvmWithdrawView` with
`exit_fees`), with illustrative values: one Solana row and one BNB Chain row
per model.
"""

from __future__ import annotations

import json
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import ponk  # noqa: E402
from ponk import (  # noqa: E402
    Agent,
    Call,
    CallersBoard,
    CallPreview,
    CallsFeed,
    EvmLegShape,
    EvmOpenCost,
    EvmPlan,
    FeedPrefs,
    Leaderboard,
    PonkClient,
    PonkPerks,
    TopCalls,
    WalletActivity,
    WalletEvmSource,
    WalletFeed,
    WalletSource,
    WeeklyRecap,
    Withdrawal,
    XCallers,
    XFeed,
    XPostToken,
    XToken,
    XTrending,
    evm,
)
from tests.test_client import StubOpener, client  # noqa: E402

BASE = "https://example.test/api"
FIXTURES = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures", "allchains")
SOL_WALLET = "GYdJfRSHhDAVVEz8optL8aGSQKd5daSZ9Ao3sVLZRkEe"
EVM_WALLET = "0x5aaeb6053f3e94c9b9a09f33669435e7ef1beaed"
BONK = "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263"
CAKE = "eip155:56:0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82"
CAKE_Q = "eip155%3A56%3A0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82"
BSC_POOL = "eip155:56:0x92b7807bf19b7dddf89b706143896d05228f3121"
BSC_POOL_Q = "eip155%3A56%3A0x92b7807bf19b7dddf89b706143896d05228f3121"
ARB_POOL = "eip155:42161:0xc6962004f452be9203591991d15f6b388e09e8d0"
AGENT = "0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b"


def fx(name):
    with open(os.path.join(FIXTURES, name)) as f:
        return json.load(f)


def keyless(payload, status=200):
    opener = StubOpener(payload, status)
    return PonkClient(base_url=BASE, opener=opener), opener


def url(opener):
    return opener.calls[0]["url"]


class TestVersion(unittest.TestCase):
    def test_version_is_0_16_0(self):
        self.assertEqual(ponk.__version__, "0.16.0")


# -- the chain filter ---------------------------------------------------------

#: Each of the nine reads that take `chains`, the path it reads and a fixture
#: it decodes.
READS = [
    ("leaderboard", lambda c, ch: c.leaderboard("7d", chains=ch), "/public/agents-leaderboard", "leaderboard.json"),
    ("calls_feed", lambda c, ch: c.calls_feed(chains=ch), "/public/calls", "calls_feed.json"),
    ("top_calls", lambda c, ch: c.top_calls(chains=ch), "/public/calls/top", "top_calls.json"),
    ("callers_board", lambda c, ch: c.callers_board(chains=ch), "/public/calls/callers", "callers_board.json"),
    ("x_feed", lambda c, ch: c.x_feed(chains=ch), "/public/x/feed", "x_feed.json"),
    ("x_trending", lambda c, ch: c.x_trending(chains=ch), "/public/x/trending", "x_trending.json"),
    ("x_callers", lambda c, ch: c.x_callers(chains=ch), "/public/x/callers", "x_callers.json"),
    ("wallets_feed", lambda c, ch: c.wallets_feed(chains=ch), "/public/wallets/feed", "wallets_feed.json"),
    (
        "wallet_activity",
        lambda c, ch: c.wallet_activity(EVM_WALLET, chains=ch),
        "/public/wallets/" + EVM_WALLET + "/activity",
        "wallet_activity_tracked.json",
    ),
]


class TestChainFilter(unittest.TestCase):
    def test_every_read_sends_the_chains_as_one_comma_list(self):
        for name, call, path, fixture in READS:
            with self.subTest(name):
                c, opener = keyless(fx(fixture))
                call(c, ["solana", "bsc"])
                sent = url(opener)
                self.assertTrue(sent.startswith(BASE + path), sent)
                self.assertIn("chains=solana%2Cbsc", sent)

    def test_none_sends_no_chains(self):
        for name, call, _path, fixture in READS:
            with self.subTest(name):
                c, opener = keyless(fx(fixture))
                call(c, None)
                self.assertNotIn("chains=", url(opener))

    def test_all_passes_through_and_bnb_is_bsc(self):
        c, opener = keyless(fx("leaderboard.json"))
        c.leaderboard(chains="all")
        self.assertEqual(url(opener), BASE + "/public/agents-leaderboard?chains=all")
        c, opener = keyless(fx("leaderboard_bsc.json"))
        c.leaderboard(chains=["bnb"])
        self.assertEqual(url(opener), BASE + "/public/agents-leaderboard?chains=bsc")
        # A repeat is sent once, in the order given.
        c, opener = keyless(fx("leaderboard.json"))
        c.leaderboard(chains=["bsc", "solana", "bnb"])
        self.assertTrue(url(opener).endswith("chains=bsc%2Csolana"))

    def test_an_unknown_chain_raises_before_a_request(self):
        for name, call, _path, fixture in READS:
            with self.subTest(name):
                c, opener = keyless(fx(fixture))
                with self.assertRaises(ValueError):
                    call(c, ["solana", "polygon"])
                self.assertEqual(opener.calls, [])

    def test_the_helper_is_the_specs_rule(self):
        from ponk.client import _chains_param

        self.assertIsNone(_chains_param(None))
        self.assertEqual(_chains_param("all"), "all")
        self.assertEqual(_chains_param(["solana", "bsc"]), "solana,bsc")
        self.assertEqual(_chains_param("bnb"), "bsc")
        self.assertEqual(_chains_param(list(evm.CHAINS)), ",".join(evm.CHAINS))
        with self.assertRaises(ValueError):
            _chains_param(["sol"])


# -- EVM ids ------------------------------------------------------------------


class TestEvmIds(unittest.TestCase):
    def test_x_token_takes_a_solana_mint_or_an_evm_token_id(self):
        c, opener = keyless(fx("x_token_bsc.json"))
        t = c.x_token(evm.token_id("bsc", "0x0E09FaBB73Bd3Ade0a17ECC321fD13a19e81cE82"))
        self.assertEqual(url(opener), BASE + "/public/x/token/" + CAKE_Q)
        self.assertIsInstance(t, XToken)
        self.assertEqual((t.chain, t.price_source, t.mint), ("bsc", "onchain_pool", CAKE))
        c, opener = keyless(fx("x_token_solana.json"))
        t = c.x_token(BONK)
        self.assertEqual(url(opener), BASE + "/public/x/token/" + BONK)
        self.assertEqual((t.chain, t.price_source), ("solana", None))

    def test_calls_feed_filters_on_an_evm_token(self):
        c, opener = keyless(fx("calls_feed.json"))
        c.calls_feed(mint=CAKE)
        self.assertEqual(url(opener), BASE + "/public/calls?mint=" + CAKE_Q)
        c, opener = keyless(fx("calls_feed.json"))
        c.calls_feed(mint=BONK, wallet=SOL_WALLET)
        self.assertEqual(url(opener), BASE + "/public/calls?mint=" + BONK + "&wallet=" + SOL_WALLET)

    def test_call_preview_and_post_call_take_the_three_forms(self):
        for mint, fixture in ((BONK, "call_preview_solana.json"), (CAKE, "call_preview_bsc.json"), (BSC_POOL, "call_preview_bsc.json")):
            with self.subTest(mint):
                c, opener = client(fx(fixture))
                p = c.call_preview(mint)
                self.assertEqual(opener.calls[0]["method"], "GET")
                self.assertTrue(url(opener).startswith(BASE + "/v1/calls/preview?mint="))
                self.assertIsInstance(p, CallPreview)
        # A pool resolves to its token: the answer's mint is the token's id.
        c, _ = client(fx("call_preview_bsc.json"))
        p = c.call_preview(BSC_POOL)
        self.assertEqual((p.chain, p.mint, p.price_source), ("bsc", CAKE, "onchain_pool"))
        self.assertTrue(p.has_position and p.clears_floor)
        self.assertEqual((p.position_usd, p.min_position_usd), (261.4, 10.0))
        self.assertEqual(p.attached["lpPositions"], 1)
        for mint, fixture in ((BONK, "call_solana.json"), (CAKE, "call_bsc.json"), (BSC_POOL, "call_bsc.json")):
            with self.subTest("post " + mint):
                c, opener = client(fx(fixture))
                call = c.post_call(mint, "gm")
                self.assertEqual(opener.calls[0]["method"], "POST")
                self.assertEqual(url(opener), BASE + "/v1/calls")
                self.assertEqual(opener.calls[0]["body"], {"mint": mint, "text": "gm"})
                self.assertIsInstance(call, Call)

    def test_wallet_activity_takes_an_evm_address(self):
        c, opener = keyless(fx("wallet_activity_untracked.json"))
        a = c.wallet_activity(EVM_WALLET)
        self.assertEqual(url(opener), BASE + "/public/wallets/" + EVM_WALLET + "/activity")
        self.assertIsInstance(a, WalletActivity)
        self.assertEqual((a.mode, a.items), ("untracked", []))

    def test_the_wallet_feed_filters_on_an_evm_token_and_wallet(self):
        c, opener = keyless(fx("wallets_feed.json"))
        c.wallets_feed(token=CAKE, wallet=EVM_WALLET)
        self.assertEqual(
            url(opener), BASE + "/public/wallets/feed?token=" + CAKE_Q + "&wallet=" + EVM_WALLET
        )

    def test_the_colons_of_an_evm_id_are_percent_encoded_in_a_path(self):
        c, opener = keyless(fx("x_token_bsc.json"))
        c.x_token("eip155:56:0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82")
        self.assertEqual(
            url(opener),
            BASE + "/public/x/token/eip155%3A56%3A0x0e09fabb73bd3ade0a17ecc321fd13a19e81ce82",
        )


# -- the saved filter ---------------------------------------------------------


class TestFeedPrefs(unittest.TestCase):
    def test_read(self):
        c, opener = client(fx("feed_prefs.json"))
        p = c.feed_prefs()
        self.assertEqual((opener.calls[0]["method"], url(opener)), ("GET", BASE + "/v1/feed-prefs"))
        self.assertIsInstance(p, FeedPrefs)
        self.assertEqual((p.chains, p.all), (["solana", "bsc"], False))

    def test_save_a_list(self):
        c, opener = client(fx("feed_prefs.json"))
        c.set_feed_prefs(["solana", "bnb"])
        call = opener.calls[0]
        self.assertEqual((call["method"], call["url"]), ("PATCH", BASE + "/v1/feed-prefs"))
        self.assertEqual(call["body"], {"chains": ["solana", "bsc"]})

    def test_save_all(self):
        for value in ("all", None, []):
            with self.subTest(value):
                c, opener = client(fx("feed_prefs_all.json"))
                p = c.set_feed_prefs(value)
                self.assertEqual(opener.calls[0]["method"], "PATCH")
                self.assertEqual(opener.calls[0]["body"], {"chains": "all"})
                self.assertTrue(p.all)
                self.assertEqual(p.chains, list(evm.CHAINS))

    def test_an_unknown_chain_is_refused_before_a_request(self):
        c, opener = client(fx("feed_prefs.json"))
        with self.assertRaises(ValueError):
            c.set_feed_prefs(["polygon"])
        self.assertEqual(opener.calls, [])


# -- the models ----------------------------------------------------------------


class TestModels(unittest.TestCase):
    def test_leaderboard_rows_carry_their_chain(self):
        b = Leaderboard.from_dict(fx("leaderboard.json"))
        self.assertEqual(b.chains, list(evm.CHAINS))
        sol, bsc = b.ranked
        self.assertEqual((sol.chain, sol.dex), ("solana", "meteora_dlmm"))
        self.assertEqual((bsc.chain, bsc.dex, bsc.pool_address), ("bsc", "pancakeswap_v3", BSC_POOL))
        self.assertEqual(evm.parse_evm_id(bsc.token_x_mint).chain, "bsc")
        one = Leaderboard.from_dict(fx("leaderboard_bsc.json"))
        self.assertEqual((one.chains, one.ranked[0].rank), (["bsc"], 1))
        self.assertEqual(one.totals["pnl_usd"], one.ranked[0].pnl_usd)

    def test_calls_feed_decodes_ponk_and_x_calls_on_both_chains(self):
        f = CallsFeed.from_dict(fx("calls_feed.json"))
        self.assertEqual((f.chains, f.next_before), (["solana", "bsc"], "2026-11-12T07:30:00Z"))
        evm_call, x_call, sol_call = f.calls
        self.assertEqual((evm_call.source, evm_call.chain, evm_call.price_source), ("ponk", "bsc", "onchain_pool"))
        self.assertEqual(evm_call.mint, CAKE)
        # The EVM boards state no market cap: a dash, never zero.
        self.assertIsNone(evm_call.mcap_usd)
        self.assertIsNone(evm_call.mcap_now_usd)
        self.assertEqual((evm_call.auto, evm_call.status), (False, "open"))
        self.assertEqual((evm_call.price_usd, evm_call.multiple_1h), (2.41, 1.012))
        self.assertEqual(evm_call.attached["lpPositions"], 1)
        self.assertEqual((x_call.source, x_call.chain, x_call.post_id), ("x", "bsc", "1856000000000000001"))
        self.assertEqual(x_call.account["handle"], "bnbcaller")
        self.assertEqual((sol_call.chain, sol_call.price_source), ("solana", None))
        self.assertEqual(sol_call.mcap_usd, 1_910_000_000.0)
        self.assertEqual(f.raw_json["calls"][0]["priceSource"], "onchain_pool")

    def test_a_posted_call_reads_its_camel_case_figures(self):
        sol = Call.from_dict(fx("call_solana.json"))
        self.assertEqual((sol.chain, sol.price_usd, sol.multiple_now), ("solana", 0.0000231, 1.0563))
        bsc = Call.from_dict(fx("call_bsc.json"))
        self.assertEqual((bsc.chain, bsc.price_source, bsc.peak_multiple), ("bsc", "onchain_pool", 1.0498))

    def test_top_calls_and_callers_board_carry_chains(self):
        t = TopCalls.from_dict(fx("top_calls.json"))
        self.assertEqual(t.chains, ["solana", "bsc"])
        self.assertEqual(t.calls[0]["chain"], "bsc")
        b = CallersBoard.from_dict(fx("callers_board.json"))
        self.assertEqual((b.days, b.chains, b.computed_at), (30, ["bsc"], "2026-11-12T09:00:00Z"))
        self.assertEqual(b.callers[0]["source"], "ponk")

    def test_x_feed_tokens_carry_their_chain(self):
        f = XFeed.from_dict(fx("x_feed.json"))
        self.assertEqual(f.chains, list(evm.CHAINS))
        post = f.items[0].post
        bsc, sol, ambiguous = post.tokens
        self.assertIsInstance(bsc, XPostToken)
        self.assertEqual((bsc.chain, bsc.mint, bsc.chains, bsc.first_call), ("bsc", CAKE, [], True))
        self.assertEqual((bsc.call_price_usd, bsc.multiple), (2.38, 1.063))
        self.assertEqual((sol.chain, sol.mint), ("solana", BONK))
        # An address on several chains is never placed on one: no price.
        self.assertEqual((ambiguous.chain, ambiguous.chains), ("evm", ["ethereum", "bsc"]))
        self.assertTrue(ambiguous.mint.startswith("0x"))
        self.assertIsNone(ambiguous.price_usd)
        self.assertIsNone(ambiguous.multiple)
        event_tokens = f.items[1].event.post.tokens
        self.assertEqual([t.chain for t in event_tokens], ["bsc", "solana"])

    def test_x_trending_and_callers_carry_chains(self):
        t = XTrending.from_dict(fx("x_trending.json"))
        self.assertEqual(t.chains, ["solana", "bsc"])
        self.assertEqual([(x.chain, x.chains) for x in t.tokens], [("bsc", []), ("solana", [])])
        self.assertEqual(t.tokens[0].accounts_1h, 3)
        c = XCallers.from_dict(fx("x_callers.json"))
        self.assertEqual((c.chains, c.callers[0].calls), (["bsc"], 6))

    def test_wallet_events_carry_chain_and_explorer(self):
        f = WalletFeed.from_dict(fx("wallets_feed.json"))
        self.assertEqual(f.chains, ["solana", "bsc"])
        bsc, sol = f.items
        self.assertEqual(bsc.chain, "bsc")
        self.assertTrue(bsc.explorer_url.startswith("https://bscscan.com/tx/0x"))
        self.assertEqual(bsc.signature, bsc.explorer_url.rsplit("/", 1)[1])
        self.assertEqual((bsc.pool, bsc.token_mint, bsc.venue), (BSC_POOL, CAKE, "pancakeswap_v3"))
        self.assertEqual(bsc.legs[0].mint, "eip155:56:0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c")
        self.assertEqual(bsc.wallet.address, EVM_WALLET)
        self.assertEqual(sol.chain, "solana")
        self.assertTrue(sol.explorer_url.startswith("https://solscan.io/tx/"))

    def test_the_wallet_source_lists_each_evm_chain(self):
        s = WalletSource.from_dict(fx("wallets_status.json"))
        self.assertEqual(s.state, "live")
        bsc, base = s.evm
        self.assertIsInstance(bsc, WalletEvmSource)
        self.assertEqual((bsc.chain, bsc.cadence, bsc.poll_minutes, bsc.wallets_read), ("bsc", "webhook", 5, 3))
        self.assertEqual((base.chain, base.state, base.cadence, base.last_ok_at), ("base", "starting", "poll", None))
        f = WalletFeed.from_dict(fx("wallets_feed.json"))
        self.assertEqual(f.source.evm[0].chain, "bsc")

    def test_weekly_recap_names_the_best_agents_chain(self):
        c, opener = client(fx("weekly_recap_bsc.json"))
        r = c.weekly_recap()
        self.assertEqual(url(opener), BASE + "/v1/recap/weekly")
        self.assertIsInstance(r, WeeklyRecap)
        self.assertEqual((r.agents, r.rank, r.ranked, r.complete), (2, 7, 64, False))
        self.assertEqual((r.best_agent.chain, r.best_agent.dex, r.best_agent.pool_address), ("bsc", "pancakeswap_v3", BSC_POOL))
        sol = WeeklyRecap.from_dict(fx("weekly_recap_solana.json"))
        self.assertEqual((sol.best_agent.chain, sol.best_agent.pnl_usd), ("solana", 12.41))


# -- EVM liquidity shapes ---------------------------------------------------------


class TestShapes(unittest.TestCase):
    def test_the_tables(self):
        self.assertEqual(evm.SHAPES, ("spot", "curve", "bid_ask", "spot_bid_ask"))
        self.assertEqual(evm.PLAN_SHAPES, ("spot", "curve", "bid_ask"))
        self.assertTrue(evm.is_shape_venue("uniswap_v3", "robinhood"))
        self.assertTrue(evm.is_shape_venue("uniswap_v4", "arbitrum"))
        self.assertFalse(evm.is_shape_venue("uniswap_v4", "robinhood"))
        self.assertTrue(evm.is_shape_venue("pancakeswap_infinity_cl", "bsc"))
        self.assertTrue(evm.is_shape_venue("aerodrome_slipstream", "base"))
        self.assertFalse(evm.is_shape_venue("aerodrome_classic", "base"))
        for venue, chains in evm.SHAPE_VENUES.items():
            for chain in chains:
                self.assertIn(chain, evm.EVM_VENUES[venue])

    def test_with_shape_writes_where_the_strategy_keeps_it(self):
        c = evm.with_shape(evm.range_config(0.05, 0.05), "curve", shape_ranges="auto")
        self.assertEqual(c["knobs"]["liquidity_shape"], "curve")
        self.assertEqual(c["knobs"]["range_pct"], {"down_pct": 0.05, "up_pct": 0.05})
        self.assertEqual(c["evm"], {"shape_ranges": "auto"})
        a = evm.with_shape({"kind": "adaptive_range_recovery", "range_pct": {"down_pct": 0.1, "up_pct": 0.1}}, "bid_ask")
        self.assertEqual(a["liquidity_shape"], "bid_ask")
        self.assertNotIn("knobs", a)
        staked = evm.range_config(0.05, 0.05, stake=False, shape="curve", shape_ranges=3)
        self.assertEqual(staked["evm"], {"stake": False, "shape_ranges": 3})
        for bad in ("hybrid", "Curve", 3):
            with self.assertRaises(ValueError):
                evm.with_shape({}, bad)
        for bad in (0, 8, "3", True, 2.5):
            with self.assertRaises(ValueError):
                evm.with_shape({}, "curve", shape_ranges=bad)

    def test_a_one_sided_range_is_one_side_at_zero(self):
        below = evm.range_config(0.05, 0)
        self.assertEqual(below["knobs"]["range_pct"], {"down_pct": 0.05, "up_pct": 0})
        above = evm.range_config(0, 0.08)
        self.assertEqual(above["knobs"]["range_pct"], {"down_pct": 0, "up_pct": 0.08})
        self.assertLess(below["target_bin_width"], evm.range_config(0.05, 0.05)["target_bin_width"])

    def test_create_sends_the_shape_in_the_config(self):
        c, opener = client(fx("../evm/agent.json"))
        c.create_evm_agent(
            name="weth usdc curve",
            wallet_address=SOL_WALLET,
            chain="arbitrum",
            dex="uniswap_v3",
            pool_address=ARB_POOL,
            down_pct=0.05,
            up_pct=0.05,
            dry_run=False,
            shape="curve",
            shape_ranges="auto",
        )
        body = opener.calls[0]["body"]
        self.assertEqual(url(opener), BASE + "/v1/agents")
        self.assertEqual((body["chain"], body["dex"], body["strategy"]), ("arbitrum", "uniswap_v3", "bin_rebalancer"))
        self.assertEqual(body["config"]["knobs"]["liquidity_shape"], "curve")
        self.assertEqual(body["config"]["evm"], {"shape_ranges": "auto"})

    def test_create_with_your_own_adaptive_config_puts_the_shape_on_top(self):
        c, opener = client(fx("../evm/agent.json"))
        c.create_evm_agent(
            name="adaptive",
            wallet_address=SOL_WALLET,
            chain="bsc",
            dex="pancakeswap_v3",
            pool_address=BSC_POOL,
            strategy="adaptive_range_recovery",
            config={"range_pct": {"down_pct": 0.05, "up_pct": 0}},
            dry_run=True,
            shape="bid_ask",
        )
        cfg = opener.calls[0]["body"]["config"]
        self.assertEqual(cfg["liquidity_shape"], "bid_ask")
        self.assertNotIn("knobs", cfg)

    def test_a_shape_is_refused_where_it_cannot_run(self):
        c, opener = client(fx("../evm/agent.json"))
        with self.assertRaises(ValueError):
            c.create_evm_agent(
                name="aero", wallet_address=SOL_WALLET, chain="base", dex="aerodrome_classic",
                pool_address="eip155:8453:0xf634058dc94e6bf8e828e27621320c423bc8b18e",
                dry_run=False, shape="curve",
            )
        # A range count names a venue that holds several ranges.
        with self.assertRaises(ValueError):
            c.create_evm_agent(
                name="rh v4", wallet_address=SOL_WALLET, chain="robinhood", dex="uniswap_v4",
                pool_address="eip155:4663:0x0000000000000000000000000000000000000001",
                down_pct=0.05, up_pct=0.05, dry_run=False, shape="curve", shape_ranges=3,
            )
        self.assertEqual(opener.calls, [])

    def test_a_shape_alone_on_a_one_range_venue_is_sent(self):
        c, opener = client(fx("../evm/agent.json"))
        c.create_evm_agent(
            name="rh v4", wallet_address=SOL_WALLET, chain="robinhood", dex="uniswap_v4",
            pool_address="eip155:4663:0x0000000000000000000000000000000000000001",
            down_pct=0.05, up_pct=0.05, dry_run=False, shape="curve",
        )
        cfg = opener.calls[0]["body"]["config"]
        self.assertEqual(cfg["knobs"]["liquidity_shape"], "curve")
        self.assertNotIn("evm", cfg)

    def test_the_open_cost_quotes_a_shape(self):
        c, opener = client(fx("open_cost_shape.json"))
        cost = c.evm_open_cost(
            "arbitrum", "uniswap_v3", pool=ARB_POOL, shape="curve", ranges="auto",
            amount_usd=1000, range_pct=5, side="below",
        )
        self.assertEqual(
            url(opener),
            BASE + "/v1/evm/arbitrum/open-cost?dex=uniswap_v3&pool=eip155%3A42161%3A0xc6962004f452be9203591991d15f6b388e09e8d0"
            "&shape=curve&ranges=auto&amount_usd=1000&range_pct=5&side=below",
        )
        self.assertIsInstance(cost, EvmOpenCost)
        self.assertEqual((cost.shape["ranges"], cost.shape["several_ranges"]), (5, True))
        self.assertEqual(cost.shape["entry_cost"][2]["ranges"], 5)
        self.assertEqual(cost.exit_fee_bps, 100)
        # No shape asked: no shape parameters sent.
        c, opener = client(fx("../evm/open_cost_v1.json"))
        cost = c.evm_open_cost("bsc", "pancakeswap_v3")
        self.assertEqual(url(opener), BASE + "/v1/evm/bsc/open-cost?dex=pancakeswap_v3")
        self.assertIsNone(cost.shape)

    def test_the_plan_sentence_carries_the_shape_in_the_grammars_words(self):
        c, opener = client(fx("plan_shaped.json"))
        plan = c.copilot_evm_plan("$1000 in the top 1 arbitrum pools", shape="curve")
        self.assertEqual(opener.calls[0]["body"], {"text": "$1000 in the top 1 arbitrum pools, curve shape"})
        self.assertIsInstance(plan, EvmPlan)
        leg = plan.legs[0]
        self.assertIsInstance(leg.shape, EvmLegShape)
        self.assertEqual((leg.shape.kind, leg.shape.ranges, leg.shape.several_ranges), ("curve", 5, True))
        self.assertEqual((leg.shape.entry_cost_usd, leg.shape.entry_cost_limit_pct), ("1.84", 3.0))
        self.assertEqual(leg.config["evm"]["shape_ranges"], "auto")
        c, opener = client(fx("plan_one_sided.json"))
        plan = c.copilot_evm_plan("$1000 in the top 1 arbitrum pools.", shape="bid_ask", one_sided="below")
        self.assertEqual(
            opener.calls[0]["body"]["text"], "$1000 in the top 1 arbitrum pools, bid-ask shape, one sided below"
        )
        leg = plan.legs[0]
        self.assertEqual((leg.shape.kind, leg.shape.one_sided), ("bid_ask", "below"))
        self.assertEqual(leg.config["knobs"]["range_pct"]["up_pct"], 0.0)
        # A sentence with no shape goes as written.
        c, opener = client(fx("plan_shaped.json"))
        c.copilot_evm_plan("$200 in the top 5 bnb pools.")
        self.assertEqual(opener.calls[0]["body"]["text"], "$200 in the top 5 bnb pools.")
        for kw in ({"shape": "spot_bid_ask"}, {"one_sided": "left"}):
            with self.assertRaises(ValueError):
                c.copilot_evm_plan("$200 in bnb", **kw)
        self.assertEqual(len(opener.calls), 1)

    def test_a_plan_without_a_shape_has_none(self):
        with open(os.path.join(os.path.dirname(FIXTURES), "evm", "plan.json")) as f:
            plan = EvmPlan.from_dict(json.load(f))
        self.assertIsNone(plan.legs[0].shape)


# -- the exit fee -----------------------------------------------------------------


class TestExitFee(unittest.TestCase):
    def test_the_constants(self):
        self.assertEqual((ponk.EXIT_FEE_BPS, ponk.EXIT_FEE_HOLDER_BPS), (100, 50))

    def test_an_evm_exit_lists_what_the_fee_took(self):
        w = Withdrawal.from_dict(fx("exit_fee.json"))
        self.assertEqual(w.exit_fee_bps, 100)
        self.assertEqual(len(w.exit_fees), 2)
        for got, fee in zip(w.received, w.exit_fees):
            self.assertEqual(got.token, fee.token)
            # Taken in kind: the fee is 1% of what left, the owner got the rest.
            self.assertEqual(int(fee.amount) * 100, int(got.amount) + int(fee.amount))
        none = Withdrawal.from_dict({"lamports": 1, "exit_fees": [], "exit_fee_bps": None})
        self.assertEqual((none.exit_fees, none.exit_fee_bps), ([], None))

    def test_the_agent_and_the_perks_state_the_rate(self):
        a = Agent.from_dict({"id": AGENT, "exit_fee_bps": 100, "activation_fee_bps": 150})
        self.assertEqual(a.exit_fee_bps, ponk.EXIT_FEE_BPS)
        p = PonkPerks.from_dict(
            {
                "holds": True,
                "exitFee": {
                    "baseBps": 100,
                    "effectiveBps": 50,
                    "basePercent": 1.0,
                    "effectivePercent": 0.5,
                    "holderBps": 50,
                    "holderPercent": 0.5,
                },
            }
        )
        self.assertEqual((p.exit_fee.base_bps, p.exit_fee.effective_bps), (100, 50))


if __name__ == "__main__":
    unittest.main()
