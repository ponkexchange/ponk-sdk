"""0.15.0: agents on Ethereum, Arbitrum, Base, BNB Chain and Robinhood Chain.

Same stub-opener approach as test_client.py: no test here opens a socket. The
payloads are the files in `tests/fixtures/evm`. The chain list, the three pool
boards, the v4 pool and the gas coin token are answers the public routes gave
on 2026-11-10, cut down to a few rows. The agent, wallet, position,
performance, exit, mandate, positions, plan and fleet bodies are written field
for field from the routes document and the server's own Serialize structs
(`EvmWalletView`, `EvmWithdrawView`, `EvmPlanView`, the open-cost body), with
illustrative values.
"""

from __future__ import annotations

import json
import os
import sys
import unittest
from decimal import Decimal

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import ponk  # noqa: E402
from ponk import (  # noqa: E402
    Agent,
    AgentPerformance,
    AgentPosition,
    AgentWallet,
    EvmPlan,
    Mandate,
    PonkClient,
    PonkError,
    PonkNotFoundError,
    PonkUnprocessableError,
    Position,
    ValueHistory,
    Withdrawal,
    evm,
)
from tests.test_client import StubOpener, client  # noqa: E402

BASE = "https://example.test/api"
FIXTURES = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixtures", "evm")
AGENT = "0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2b"
AGENT2 = "0190a1b2-c3d4-7e5f-8a6b-7c8d9e0f1a2c"
SOL = "GYdJfRSHhDAVVEz8optL8aGSQKd5daSZ9Ao3sVLZRkEe"
POOL = "eip155:56:0x92b7807bf19b7dddf89b706143896d05228f3121"
POOL_Q = "eip155%3A56%3A0x92b7807bf19b7dddf89b706143896d05228f3121"
WITHDRAW = "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"


def fx(name):
    with open(os.path.join(FIXTURES, name)) as f:
        return json.load(f)


def keyless(payload, status=200):
    opener = StubOpener(payload, status)
    return PonkClient(base_url=BASE, opener=opener), opener


def sent(opener):
    call = opener.calls[0]
    return call["method"], call["url"]


def auth(opener):
    return {k.lower(): v for k, v in opener.calls[0]["headers"].items()}.get("authorization")


class TestVersion(unittest.TestCase):
    def test_version_is_0_15_0(self):
        self.assertEqual(ponk.__version__, "0.15.0")


class TestIds(unittest.TestCase):
    def test_a_pool_id_is_lowercase_caip(self):
        self.assertEqual(evm.pool_id("bsc", "0x92b7807bF19b7DDdf89b706143896d05228f3121"), POOL)
        self.assertEqual(
            evm.token_id("base", "0x4200000000000000000000000000000000000006"),
            "eip155:8453:0x4200000000000000000000000000000000000006",
        )
        self.assertEqual(
            evm.native_token_id("robinhood"),
            "eip155:4663:0x0000000000000000000000000000000000000000",
        )

    def test_the_chain_ids_are_the_apis(self):
        self.assertEqual(
            evm.EVM_CHAIN_IDS,
            {"ethereum": 1, "arbitrum": 42161, "base": 8453, "bsc": 56, "robinhood": 4663},
        )
        self.assertEqual(evm.CHAINS[0], "solana")
        self.assertEqual(evm.NATIVE_SYMBOL["bsc"], "BNB")
        self.assertEqual({evm.NATIVE_SYMBOL[c] for c in evm.EVM_CHAINS if c != "bsc"}, {"ETH"})

    def test_the_v4_form_carries_the_manager_and_the_pool_id(self):
        manager = "0x000000000004444c5dc75cB358380D2e3dE08A90"
        key = "0x" + "E6" * 32
        pid = evm.v4_pool_id("ethereum", manager, key)
        self.assertEqual(pid, "eip155:1:{0}:{1}".format(manager.lower(), key.lower()))
        parsed = evm.parse_evm_id(pid)
        self.assertEqual((parsed.kind, parsed.chain, parsed.chain_id), ("v4_pool", "ethereum", 1))
        self.assertEqual((parsed.address, parsed.pool_key_hash), (manager.lower(), key.lower()))

    def test_every_id_form_parses(self):
        p = evm.parse_evm_id(POOL)
        self.assertEqual((p.kind, p.chain, p.address), ("address", "bsc", POOL.split(":")[2]))
        self.assertFalse(p.is_native)
        self.assertTrue(evm.parse_evm_id(evm.native_token_id("bsc")).is_native)
        nft = evm.parse_evm_id(evm.position_nft_id("bsc", "0x46A15B0b27311cedF172AB29E4f4766fbE7F4364", 812345))
        self.assertEqual((nft.kind, nft.nft_token_id), ("position_nft", 812345))
        v2 = evm.parse_evm_id(fx("position_full_range.json")["position_address"])
        self.assertEqual((v2.kind, v2.chain), ("v2_position", "base"))
        self.assertEqual(v2.owner, "0x52908400098527886e0f7030069857d2e4169ee7")

    def test_a_solana_address_or_a_foreign_chain_is_never_an_evm_id(self):
        for bad in (
            "So11111111111111111111111111111111111111112",
            "eip155:137:0x92b7807bf19b7dddf89b706143896d05228f3121",
            "eip155:56:0x1234",
            "eip155:56:0x92b7807bf19b7dddf89b706143896d05228f3121:nope",
            None,
        ):
            with self.assertRaises(ValueError):
                evm.parse_evm_id(bad)
        self.assertEqual(evm.chain_of("So11111111111111111111111111111111111111112"), "solana")
        self.assertEqual(evm.chain_of(POOL), "bsc")

    def test_bad_inputs_are_refused_before_an_id_is_built(self):
        with self.assertRaises(ValueError):
            evm.pool_id("polygon", "0x92b7807bf19b7dddf89b706143896d05228f3121")
        with self.assertRaises(ValueError):
            evm.pool_id("bsc", "92b7807bf19b7dddf89b706143896d05228f3121")
        with self.assertRaises(ValueError):
            evm.v4_pool_id("ethereum", "0x000000000004444c5dc75cb358380d2e3de08a90", "0x12")

    def test_venues_know_their_chains_ranges_and_farms(self):
        self.assertEqual(evm.venue_chains("pancakeswap_infinity_cl"), ("bsc",))
        self.assertEqual(set(evm.venue_chains("uniswap_v4")), {"arbitrum", "robinhood", "ethereum"})
        self.assertEqual(evm.venue_chains("aerodrome_slipstream"), ("base",))
        self.assertEqual(evm.venue_chains("meteora_dlmm"), ())
        self.assertEqual(len(evm.EVM_VENUES), 8)
        self.assertTrue(evm.is_full_range_venue("aerodrome_classic"))
        self.assertFalse(evm.is_full_range_venue("uniswap_v3"))
        self.assertEqual(evm.stake_reward_token("pancakeswap_v3"), "CAKE")
        self.assertEqual(evm.stake_reward_token("aerodrome_classic"), "AERO")
        self.assertIsNone(evm.stake_reward_token("pancakeswap_infinity_cl"))

    def test_wei_reads_exactly(self):
        self.assertEqual(evm.wei_to_native("452200000000000"), Decimal("0.0004522"))
        self.assertIsNone(evm.wei_to_native(None))


class TestFundLink(unittest.TestCase):
    def test_an_evm_agent_is_listed_with_zero(self):
        self.assertEqual(
            evm.fund_link(AGENT, chain="bsc"),
            "https://ponk.exchange/agents/fund?a={0}:0".format(AGENT),
        )
        # The amount field is lamports, so it never carries to an EVM agent.
        self.assertEqual(
            evm.fund_link(AGENT, 5_000_000, chain="base"),
            "https://ponk.exchange/agents/fund?a={0}:0".format(AGENT),
        )

    def test_a_solana_agent_carries_its_lamports(self):
        self.assertEqual(
            evm.fund_link(AGENT, 150_000_000),
            "https://ponk.exchange/agents/fund?a={0}:150000000".format(AGENT),
        )
        for bad in (0, -1, 10**16, True, 1.5):
            self.assertTrue(evm.fund_link(AGENT, bad).endswith(":0"), bad)

    def test_ids_are_cleaned_and_listed_once(self):
        links = evm.fund_links([AGENT.upper(), " " + AGENT, "not-an-id", {"id": AGENT2, "chain": "bsc"}])
        self.assertEqual(links, ["https://ponk.exchange/agents/fund?a={0}:0,{1}:0".format(AGENT, AGENT2)])
        self.assertEqual(evm.fund_links(["nope"]), [])
        with self.assertRaises(ValueError):
            evm.fund_link("nope")

    def test_forty_agents_to_a_link(self):
        ids = ["00000000-0000-7000-8000-{0:012x}".format(i) for i in range(85)]
        links = evm.fund_links(ids)
        self.assertEqual([link.count(":0") for link in links], [40, 40, 5])
        for i in ids:
            self.assertEqual(sum(1 for link in links if i in link), 1)

    def test_top_up_is_added_only_when_something_is_funded(self):
        funded = evm.fund_links([{"id": AGENT, "lamports": 1000}], top_up=True)
        self.assertTrue(funded[0].endswith("{0}:1000&mode=top_up".format(AGENT)))
        unfunded = evm.fund_links([{"id": AGENT, "chain": "bsc", "lamports": 1000}], top_up=True)
        self.assertTrue(unfunded[0].endswith("{0}:0".format(AGENT)))

    def test_agents_and_plan_agents_are_accepted_as_they_are(self):
        agent = Agent.from_dict(fx("agent.json"))
        started = EvmPlan.from_dict(fx("plan_started.json"))
        self.assertEqual(evm.fund_links([agent]), started.sign_urls)
        # The leg that was not created has no id, so it is left out.
        self.assertEqual(evm.fund_links(started.agents), started.sign_urls)

    def test_the_agent_page(self):
        self.assertEqual(evm.agent_page_url(AGENT.upper()), "https://ponk.exchange/agents/" + AGENT)


class TestChainsAndPools(unittest.TestCase):
    def test_chains_lists_six_chains_with_their_venues(self):
        c, opener = keyless(fx("chains.json"))
        chains = c.evm_chains()
        self.assertEqual(sent(opener), ("GET", BASE + "/public/evm/chains"))
        self.assertIsNone(auth(opener))
        self.assertTrue(chains.read_enabled)
        self.assertEqual(sorted(x.id for x in chains.chains), sorted(evm.CHAINS))
        bsc = chains.chain("bsc")
        self.assertEqual((bsc.evm_chain_id, bsc.native_symbol, bsc.label), (56, "BNB", "BNB Chain"))
        self.assertIn("pancakeswap_v3", bsc.venues)
        self.assertIsNone(chains.chain("solana").evm_chain_id)
        self.assertIsNone(chains.chain("polygon"))
        for row in chains.chains:
            if row.evm_chain_id is not None:
                self.assertEqual(evm.EVM_CHAIN_IDS[row.id], row.evm_chain_id)
                self.assertEqual(evm.NATIVE_SYMBOL[row.id], row.native_symbol)
                for venue in row.venues:
                    self.assertIn(row.id, evm.venue_chains(venue), venue)

    def test_the_keyed_twin_sends_the_key(self):
        c, opener = client(fx("chains.json"))
        c.evm_chains(public=False)
        self.assertEqual(sent(opener), ("GET", BASE + "/v1/evm/chains"))
        self.assertEqual(auth(opener), "Bearer ponk_live_test")

    def test_the_bnb_board_reads_fee_tier_tvl_windows_farm_and_flags(self):
        c, opener = keyless(fx("board_bsc.json"))
        board = c.evm_pools("bsc", venue="pancakeswap_v3")
        self.assertEqual(sent(opener), ("GET", BASE + "/public/evm/bsc/board?venue=pancakeswap_v3"))
        self.assertTrue(board.ready)
        self.assertEqual(board.chain, "bsc")
        self.assertFalse(board.status.stale)
        v3, v2 = board.pools
        self.assertEqual((v3.dex, v3.address, v3.pair), ("pancakeswap_v3", POOL, "USDT/USDC"))
        self.assertEqual((v3.fee_pct, v3.dynamic_fee, v3.bin_step), (0.01, False, 1))
        self.assertGreater(v3.tvl, 1_000_000)
        self.assertEqual(v3.tvl_source, "onchain")
        self.assertGreater(v3.volume.h24, v3.volume.h1)
        self.assertGreater(v3.fees.h24, 0)
        # A window with no reading is None, never 0.
        self.assertIsNone(v3.fees.h4)
        self.assertEqual((v3.farm.token, v3.farm.live), ("CAKE", True))
        self.assertIsInstance(v3.farm.apr_pct, float)
        self.assertEqual(v3.x.mint, "eip155:56:0x55d398326f99059ff775485246999027b3197955")
        self.assertEqual(v3.x.decimals, 18)
        self.assertEqual(v3.flags(), [])
        # PancakeSwap v2: full range, no farm, and a token off the list.
        self.assertIsNone(v2.farm)
        self.assertIsNone(v2.bin_step)
        self.assertEqual(v2.flags(), ["unverified_token"])
        self.assertEqual(evm.parse_evm_id(v2.address).chain, "bsc")

    def test_a_dynamic_fee_v4_pool_has_no_fee_tier_and_a_hook_flag(self):
        c, opener = keyless(fx("board_ethereum.json"))
        board = c.evm_pools("ethereum")
        self.assertEqual(sent(opener), ("GET", BASE + "/public/evm/ethereum/board"))
        hookless, dynamic = board.pools
        self.assertEqual(evm.parse_evm_id(hookless.address).kind, "v4_pool")
        # The zero address is a pool with no hook, so nothing is flagged.
        self.assertEqual(hookless.flags(), [])
        self.assertEqual(hookless.fee_pct, 0.0005)
        self.assertTrue(dynamic.dynamic_fee)
        self.assertIsNone(dynamic.fee_pct)
        self.assertIsNone(dynamic.fees.h24)
        self.assertFalse(dynamic.hooks.agent_eligible)
        self.assertTrue(dynamic.hooks.touches_lp_funds)
        self.assertEqual(dynamic.flags(), ["hook_not_agent_eligible"])
        self.assertTrue(dynamic.data_insufficient[0].startswith("fees:"))

    def test_a_pool_under_an_hour_old_is_flagged(self):
        pool = ponk.EvmPool.from_dict(fx("board_bsc.json")["pools"][0])
        self.assertEqual(pool.flags(now_ms=pool.created_at + 60_000), ["under_1h_old"])
        self.assertEqual(pool.flags(now_ms=pool.created_at + 3_600_000), [])

    def test_the_base_board_carries_aero_farms_and_gauge_facts(self):
        c, _ = keyless(fx("board_base.json"))
        board = c.evm_pools("base")
        self.assertEqual({p.dex for p in board.pools}, {"aerodrome_slipstream", "aerodrome_classic"})
        for p in board.pools:
            self.assertEqual((p.farm.token, p.farm.live), ("AERO", True))
        self.assertEqual({f["key"] for f in board.aerodrome}, {p.key for p in board.pools})

    def test_an_unread_chain_is_an_empty_board_not_an_error(self):
        c, _ = keyless(
            {
                "chain": "arbitrum",
                "asOf": 0,
                "servedAt": 1794371669624,
                "pools": [],
                "ok": {},
                "status": {"lastError": "the first read of this chain has not completed yet"},
            }
        )
        board = c.evm_pools("arbitrum")
        self.assertFalse(board.ready)
        self.assertEqual(board.pools, [])
        self.assertIn("first read", board.status.last_error)

    def test_pool_detail_encodes_the_id_as_one_segment(self):
        body = fx("pool_v4.json")
        c, opener = keyless(body)
        pool = c.evm_pool(body["address"])
        self.assertEqual(
            sent(opener),
            ("GET", BASE + "/public/evm/pools/" + body["address"].replace(":", "%3A")),
        )
        self.assertEqual(pool.address, body["address"])
        self.assertEqual(pool.raw_json["sqrtPriceX96"], pool.sqrt_price_x96)
        c, opener = client(fx("pool_base.json"))
        pool = c.evm_pool("eip155:8453:0x4a7af46bc8abc00048c6c7920a1488daf568366c", public=False)
        self.assertTrue(sent(opener)[1].startswith(BASE + "/v1/evm/pools/eip155%3A8453%3A0x4a7a"))
        self.assertTrue(pool.aerodrome["gaugeLive"])

    def test_the_gas_coin_is_a_token_with_a_price(self):
        c, opener = keyless(fx("token_native.json"))
        token = c.evm_token(evm.native_token_id("bsc"))
        self.assertTrue(sent(opener)[1].endswith("/public/evm/tokens/eip155%3A56%3A0x" + "0" * 40))
        self.assertEqual((token.symbol, token.decimals, token.price_source), ("BNB", 18, "chainlink"))
        self.assertGreater(token.price_usd, 0)
        self.assertTrue(evm.parse_evm_id(token.mint).is_native)

    def test_the_apis_sentence_is_the_error(self):
        c, _ = keyless(
            {"error": {"code": "unprocessable", "message": "Solana is not read yet: EVM reads cover Arbitrum, Robinhood Chain, Ethereum, BNB Chain, Base", "request_id": "r1"}},
            status=422,
        )
        with self.assertRaises(PonkUnprocessableError) as ctx:
            c.evm_pools("solana")
        self.assertTrue(ctx.exception.message.startswith("Solana is not read yet"))
        c, _ = keyless({"error": {"code": "not_found", "message": "not found"}}, status=404)
        with self.assertRaises(PonkNotFoundError):
            c.evm_pool(POOL)


class TestOpenCost(unittest.TestCase):
    def test_a_key_reads_the_v1_quote_with_the_api_rates(self):
        c, opener = client(fx("open_cost_v1.json"))
        cost = c.evm_open_cost("bsc", "pancakeswap_v3", pool=POOL, stake=True)
        self.assertEqual(
            sent(opener),
            ("GET", BASE + "/v1/evm/bsc/open-cost?dex=pancakeswap_v3&pool=" + POOL_Q + "&stake=true"),
        )
        self.assertEqual((cost.activation_fee_bps, cost.performance_fee_bps, cost.exit_fee_bps), (150, 1500, 0))
        self.assertEqual(cost.activation_fee_bps, ponk.ACTIVATION_FEE_API_BPS)
        self.assertEqual(cost.native_symbol, "BNB")
        # Wei stays a string; the minimum is the reserve plus the gas budget.
        self.assertEqual(cost.min_open_wei, "452200000000000")
        self.assertEqual(evm.wei_to_native(cost.min_open_wei), Decimal("0.0004522"))
        self.assertEqual((cost.gas_usd, cost.native_usd), ("0.0560", "680.65"))

    def test_without_a_key_it_reads_the_public_quote(self):
        c, opener = keyless(dict(fx("open_cost_v1.json"), activation_fee_bps=100, performance_fee_bps=800))
        cost = c.evm_open_cost("bsc", "pancakeswap_v3")
        self.assertEqual(sent(opener), ("GET", BASE + "/public/evm/bsc/open-cost?dex=pancakeswap_v3"))
        self.assertEqual((cost.activation_fee_bps, cost.performance_fee_bps), (100, 800))

    def test_public_can_be_forced_and_stake_false_is_sent(self):
        c, opener = client(fx("open_cost_v1.json"))
        c.evm_open_cost("bsc", "pancakeswap_v3", stake=False, public=True)
        self.assertEqual(
            sent(opener), ("GET", BASE + "/public/evm/bsc/open-cost?dex=pancakeswap_v3&stake=false")
        )

    def test_an_unread_chain_quotes_nothing_rather_than_guessing(self):
        c, _ = client(dict(fx("open_cost_v1.json"), gas_wei=None, gas_usd=None, min_open_wei=None))
        cost = c.evm_open_cost("bsc", "pancakeswap_v3")
        self.assertIsNone(cost.gas_wei)
        self.assertIsNone(cost.min_open_wei)


class TestCreate(unittest.TestCase):
    def test_a_range_agent_sends_the_chain_the_pool_and_the_range_as_fractions(self):
        c, opener = client(fx("agent.json"))
        agent = c.create_evm_agent(
            name="bnb usdt",
            wallet_address=SOL,
            chain="bsc",
            dex="pancakeswap_v3",
            pool_address=POOL,
            down_pct=0.05,
            up_pct=0.05,
            stake=True,
            withdrawal_address=WITHDRAW,
            dry_run=False,
        )
        call = opener.calls[0]
        self.assertEqual((call["method"], call["url"]), ("POST", BASE + "/v1/agents"))
        self.assertEqual(
            call["body"],
            {
                "name": "bnb usdt",
                "wallet_address": SOL,
                "dex": "pancakeswap_v3",
                "strategy": "bin_rebalancer",
                "config": {
                    "kind": "bin_rebalancer",
                    "target_bin_width": 977,
                    "auto_compound": True,
                    "knobs": {"range_pct": {"down_pct": 0.05, "up_pct": 0.05}},
                    "evm": {"stake": True},
                },
                "dry_run": False,
                "pool_address": POOL,
                "chain": "bsc",
                "withdrawal_address": WITHDRAW,
            },
        )
        # An EVM agent opens its own position: none is ever sent.
        self.assertNotIn("position_address", call["body"])
        self.assertEqual((agent.chain, agent.dex, agent.origin), ("bsc", "pancakeswap_v3", "api"))
        self.assertEqual((agent.wallet_source, agent.mode), ("managed", "auto"))
        # The rates an agent a key creates pays: 1.5% once, 15% of fees.
        self.assertEqual((agent.activation_fee_bps, agent.managed_fee_bps), (150, 1500))
        self.assertEqual(evm.chain_of(agent.pool_address), agent.chain)

    def test_the_tick_width_matches_the_servers_planner(self):
        # routes/copilot evm_plan::leg_config: round(ln(1+f) / ln(1.0001)) * 2 + 1.
        self.assertEqual(evm.range_config(0.06, 0.06)["target_bin_width"], 1167)
        asym = evm.range_config(0.03, 0.12, strategy="conservative_bin_rebalancer")
        self.assertEqual(asym["knobs"], {"range_pct": {"down_pct": 0.03, "up_pct": 0.12}})
        self.assertEqual(asym["kind"], "conservative_bin_rebalancer")
        self.assertNotIn("evm", asym)

    def test_a_range_is_a_fraction_not_a_percent(self):
        for bad in (5, 0, -0.05, 1, "0.05", True, None):
            with self.assertRaises(ValueError):
                evm.range_config(bad, 0.05)

    def test_a_full_range_venue_is_compound_and_hold(self):
        c, opener = client(dict(fx("agent.json"), dex="aerodrome_classic", chain="base", strategy="fee_collector"))
        c.create_evm_agent(
            name="aero",
            wallet_address=SOL,
            chain="base",
            dex="aerodrome_classic",
            pool_address="eip155:8453:0xf634058dc94e6bf8e828e27621320c423bc8b18e",
            stake=True,
            dry_run=True,
        )
        body = opener.calls[0]["body"]
        self.assertEqual(body["strategy"], "fee_collector")
        self.assertEqual(
            body["config"],
            {"kind": "fee_collector", "min_total_pending_base_units": 0, "auto_compound": True, "evm": {"stake": True}},
        )
        self.assertEqual((body["chain"], body["dry_run"]), ("base", True))
        self.assertNotIn("withdrawal_address", body)

    def test_your_own_config_is_sent_as_written(self):
        c, opener = client(fx("agent.json"))
        own = {"kind": "adaptive_range_recovery", "range_pct": {"down_pct": 0.1, "up_pct": 0.2}, "bin_range_width": 10}
        c.create_evm_agent(
            name="x", wallet_address=SOL, chain="arbitrum", dex="uniswap_v3",
            pool_address="eip155:42161:0xc6962004f452be9203591991d15f6b388e09e8d0",
            config=own, dry_run=True,
        )
        body = opener.calls[0]["body"]
        self.assertEqual((body["strategy"], body["config"]), ("adaptive_range_recovery", own))
        self.assertEqual(own, {"kind": "adaptive_range_recovery", "range_pct": {"down_pct": 0.1, "up_pct": 0.2}, "bin_range_width": 10})

    def test_nothing_is_sent_for_a_request_that_cannot_be_right(self):
        c, opener = client(fx("agent.json"))
        common = dict(name="x", wallet_address=SOL, pool_address=POOL, dry_run=True)
        with self.assertRaises(ValueError):  # Solana is create_agent's.
            c.create_evm_agent(chain="solana", dex="orca", down_pct=0.05, up_pct=0.05, **common)
        with self.assertRaises(ValueError):  # a range venue with no range
            c.create_evm_agent(chain="bsc", dex="pancakeswap_v3", **common)
        with self.assertRaises(ValueError):  # a range on a full-range venue
            c.create_evm_agent(chain="bsc", dex="pancakeswap_v2", down_pct=0.05, up_pct=0.05, **common)
        with self.assertRaises(ValueError):  # a range and a config
            c.create_evm_agent(chain="bsc", dex="pancakeswap_v3", down_pct=0.05, up_pct=0.05, config={"kind": "custom"}, **common)
        with self.assertRaises(TypeError):  # dry_run is said out loud
            c.create_evm_agent(name="x", wallet_address=SOL, pool_address=POOL, chain="bsc", dex="pancakeswap_v3", down_pct=0.05, up_pct=0.05, dry_run=None)
        self.assertEqual(opener.calls, [])

    def test_a_refusal_is_the_servers_sentence(self):
        c, _ = client(
            {"error": {"code": "unprocessable", "message": "PancakeSwap v3 does not run on Base.", "request_id": "r2"}},
            status=422,
        )
        with self.assertRaises(PonkUnprocessableError) as ctx:
            c.create_evm_agent(name="x", wallet_address=SOL, chain="base", dex="pancakeswap_v3", pool_address=POOL, down_pct=0.05, up_pct=0.05, dry_run=True)
        self.assertEqual(ctx.exception.message, "PancakeSwap v3 does not run on Base.")
        self.assertEqual(ctx.exception.request_id, "r2")

    def test_a_solana_create_is_byte_for_byte_what_it_was(self):
        c, opener = client(fx("agent.json"))
        c.create_agent(name="a", wallet_address=SOL, dex="orca", strategy="monitor_only", config={}, pool_address="P", dry_run=True)
        self.assertEqual(
            opener.calls[0]["body"],
            {"name": "a", "wallet_address": SOL, "dex": "orca", "strategy": "monitor_only", "config": {}, "dry_run": True, "pool_address": "P"},
        )


class TestAgentReads(unittest.TestCase):
    def test_every_agent_carries_a_chain(self):
        solana = dict(fx("agent.json"), chain="solana", dex="orca", pool_address="POOL")
        c, opener = client([solana, fx("agent.json")])
        agents = c.list_agents()
        self.assertEqual(sent(opener), ("GET", BASE + "/v1/agents"))
        self.assertEqual([a.chain for a in agents], ["solana", "bsc"])
        c, opener = client(fx("agent.json"))
        self.assertEqual(c.get_agent(AGENT).chain, "bsc")
        self.assertEqual(sent(opener), ("GET", BASE + "/v1/agents/" + AGENT))

    def test_the_wallet_view_is_wei_usd_and_the_mandate_status(self):
        c, opener = client(fx("wallet.json"))
        w = c.agent_wallet(AGENT)
        self.assertEqual(sent(opener), ("GET", BASE + "/v1/agents/{0}/wallet".format(AGENT)))
        self.assertTrue(w.is_evm)
        self.assertEqual((w.chain, w.native_symbol), ("bsc", "BNB"))
        self.assertEqual(w.address, "0x52908400098527886E0F7030069857D2E4169EE7")
        self.assertEqual(w.balance_wei, "150000000000000000")
        self.assertEqual(evm.wei_to_native(w.balance_wei), Decimal("0.15"))
        self.assertEqual(int(w.withdrawable_wei), int(w.balance_wei) - int(w.reserve_wei))
        self.assertEqual((w.balance_usd, w.native_usd), ("102.10", "680.65"))
        self.assertEqual((w.withdrawal_address, w.mandate_status), (WITHDRAW, "active"))
        # The Solana fields are absent, not zero.
        self.assertIsNone(w.lamports)
        self.assertIsNone(w.pubkey)

    def test_a_new_wallet_has_no_mandate_and_no_withdrawal_address(self):
        c, opener = client(fx("wallet_new.json"))
        made = c.create_agent_wallet(AGENT)
        self.assertEqual(sent(opener), ("POST", BASE + "/v1/agents/{0}/wallet".format(AGENT)))
        self.assertEqual((made.address, made.chain, made.native_symbol), ("0x52908400098527886E0F7030069857D2E4169EE7", "bsc", "BNB"))
        self.assertIsNone(made.mandate_status)
        self.assertEqual(made.min_open_wei, "452200000000000")
        self.assertIsNone(made.raw_json["withdrawal_address"])

    def test_a_solana_wallet_still_reads_as_it_did(self):
        w = AgentWallet.from_dict({"pubkey": "7Q7w", "status": "active", "lamports": 5, "scope": "per_agent"})
        self.assertFalse(w.is_evm)
        self.assertEqual((w.lamports, w.balance_wei), (5, None))

    def test_the_mandate_names_its_chain(self):
        c, opener = client(fx("mandate.json"))
        m = c.agent_mandate(AGENT)
        self.assertEqual(sent(opener), ("GET", BASE + "/v1/agents/{0}/mandate".format(AGENT)))
        self.assertEqual((m.chain, m.status, m.withdrawal_address), ("bsc", "active", WITHDRAW))
        self.assertIsNone(m.max_action_value_usd)
        self.assertIsNone(Mandate.from_dict({"status": "active"}).chain)

    def test_a_range_position_is_in_ticks(self):
        c, opener = client(fx("position.json"))
        p = c.agent_position(AGENT)
        self.assertEqual(sent(opener), ("GET", BASE + "/v1/agents/{0}/position".format(AGENT)))
        self.assertEqual((p.lower_bin_id, p.upper_bin_id, p.active_bin_id, p.bin_step), (-498, 478, -10, 1))
        self.assertEqual((p.chain, p.staked, p.is_full_range, p.in_range), ("bsc", True, False, True))
        self.assertEqual(evm.parse_evm_id(p.position_address).kind, "position_nft")
        self.assertEqual(p.current_price, "0.999006")

    def test_a_full_range_position_has_no_ticks_or_prices(self):
        p = AgentPosition.from_dict(fx("position_full_range.json"))
        self.assertTrue(p.is_full_range)
        self.assertEqual((p.lower_bin_id, p.upper_bin_id, p.bin_step, p.current_price), (None, None, None, None))
        self.assertEqual(p.chain, "base")

    def test_performance_is_usd_with_fractions_and_the_farm_block(self):
        c, opener = client(fx("performance.json"))
        perf = c.agent_performance(AGENT)
        self.assertEqual(sent(opener), ("GET", BASE + "/v1/agents/{0}/performance".format(AGENT)))
        # Fractions: 0.0162 is +1.62%.
        self.assertEqual((perf.pnl_pct, perf.drawdown_pct), (0.0162, 0.0069))
        self.assertEqual((perf.pnl_usd, perf.cost_basis_usd, perf.deposits_usd), ("1.62", "100.00", "100.00"))
        farm = perf.farm_rewards
        self.assertEqual((farm.token, farm.pending_amount, farm.pending_usd, farm.harvested_usd), ("CAKE", "0.412000", "0.74", "1.10"))
        self.assertEqual((farm.staked, farm.apr_pct, farm.apr_basis), (True, 8.2, "position"))
        self.assertIsNone(perf.figures_unavailable)
        self.assertTrue(perf.pnl_note.startswith("Position value"))
        self.assertEqual(perf.chain, "bsc")

    def test_an_unread_chain_is_none_with_the_reason(self):
        perf = AgentPerformance.from_dict(fx("performance_unread.json"))
        self.assertEqual((perf.current_value_usd, perf.pnl_usd, perf.pnl_pct), (None, None, None))
        self.assertEqual(perf.figures_unavailable, "The BNB Smart Chain RPC did not answer.")
        self.assertEqual((perf.farm_rewards.apr_pct, perf.farm_rewards.apr_basis), (None, None))
        # A Solana agent has no farm block at all.
        self.assertIsNone(AgentPerformance.from_dict({"pnl_usd": "1"}).farm_rewards)

    def test_decisions_and_value_history_use_the_same_routes(self):
        c, opener = client({"state": "healthy_no_action", "decisions": []})
        d = c.agent_decisions(AGENT, hours=6)
        self.assertEqual(sent(opener), ("GET", BASE + "/v1/agents/{0}/decisions?hours=6".format(AGENT)))
        self.assertEqual(d["state"], "healthy_no_action")
        c, opener = client(fx("value_history.json"))
        h = c.agent_value_history(AGENT, range="24h")
        self.assertEqual(sent(opener), ("GET", BASE + "/v1/agents/{0}/value-history?range=24h".format(AGENT)))
        self.assertIsInstance(h, ValueHistory)
        self.assertEqual(h.bucket_secs, 900)
        # A bucket with no reading is None, never filled in.
        self.assertEqual([p["value_usd"] for p in h.points], [101.1, None, 101.2])

    def test_positions_list_solana_and_evm_rows_together(self):
        c, opener = client(fx("positions.json"))
        sol_row, evm_row = c.list_positions()
        self.assertEqual(sent(opener), ("GET", BASE + "/v1/positions"))
        self.assertEqual((sol_row.chain, evm_row.chain), ("solana", "bsc"))
        self.assertEqual(sol_row.fees_claimable_x.to_decimal(), Decimal("0.000012345"))
        # Wei arrives as a string and is read exactly, past 2**64 and 2**127.
        self.assertEqual(evm_row.fees_claimable_x.to_decimal(), Decimal("0.21"))
        self.assertEqual(evm_row.fees_claimable_y.raw, "340282366920938463463374607431768211455")
        self.assertEqual(evm_row.fees_claimable_y.to_decimal(), Decimal(2**128 - 1) / Decimal(10**18))
        self.assertEqual((evm_row.dex, evm_row.agent_id, evm_row.pnl_source), ("pancakeswap_v3", AGENT, "evm_runtime"))
        self.assertEqual(evm.parse_evm_id(evm_row.token_x).chain, "bsc")
        self.assertEqual((evm_row.staked, evm_row.farm_rewards.token, evm_row.farm_rewards_usd), (True, "CAKE", 1.84))
        self.assertEqual((evm_row.deposited_usd, evm_row.bin_step, evm_row.is_full_range), (100.0, 1, False))
        self.assertIsNone(evm_row.fee_apr_pct)
        self.assertIsInstance(sol_row, Position)


class TestControl(unittest.TestCase):
    def test_pause_and_resume_are_the_same_routes(self):
        for method, name in (("pause_agent", "pause"), ("resume_agent", "resume")):
            c, opener = client(dict(fx("agent.json"), status="paused" if name == "pause" else "active"))
            agent = getattr(c, method)(AGENT)
            self.assertEqual(sent(opener), ("POST", BASE + "/v1/agents/{0}/{1}".format(AGENT, name)))
            self.assertEqual(agent.chain, "bsc")

    def test_stake_is_a_config_patch(self):
        c, opener = client(fx("agent.json"))
        agent = c.set_stake_rewards(AGENT, False)
        call = opener.calls[0]
        self.assertEqual((call["method"], call["url"]), ("PATCH", BASE + "/v1/agents/{0}/config".format(AGENT)))
        self.assertEqual(call["body"], {"evm": {"stake": False}})
        self.assertIsInstance(agent, Agent)
        c, opener = client(fx("agent.json"))
        c.update_config(AGENT, evm.stake_config(True))
        self.assertEqual(opener.calls[0]["body"], {"evm": {"stake": True}})

    def test_staking_on_a_venue_with_no_farm_is_refused_in_a_sentence(self):
        c, _ = client({"error": {"code": "unprocessable", "message": "Uniswap v3 has no farm to stake in."}}, status=422)
        with self.assertRaises(PonkUnprocessableError) as ctx:
            c.set_stake_rewards(AGENT, True)
        self.assertIn("no farm", ctx.exception.message)

    def test_an_exit_returns_the_real_amounts(self):
        c, opener = client(fx("exit.json"))
        out = c.exit_agent(AGENT)
        call = opener.calls[0]
        self.assertEqual((call["method"], call["url"], call["body"]), ("POST", BASE + "/v1/agents/{0}/exit".format(AGENT), {}))
        self.assertEqual(call["timeout"], c.fund_timeout)
        self.assertEqual((out.chain, out.status, out.lamports), ("bsc", "completed", 0))
        self.assertEqual(out.destination, WITHDRAW)
        self.assertEqual(out.amount_wei, "149000000000000000")
        self.assertEqual(evm.wei_to_native(out.amount_wei), Decimal("0.149"))
        self.assertEqual(len(out.tx_hashes), 2)
        self.assertEqual(out.signature, out.tx_hashes[-1])
        token, native = out.received
        self.assertEqual((token.amount, token.tx_hash), ("50120000000000000000", out.tx_hashes[0]))
        self.assertFalse(evm.parse_evm_id(token.token).is_native)
        self.assertTrue(evm.parse_evm_id(native.token).is_native)
        self.assertEqual(native.amount, out.amount_wei)
        self.assertIn("reached your withdrawal address", out.sentence)

    def test_an_exit_still_running_says_so_and_states_no_amount(self):
        out = Withdrawal.from_dict(fx("exit_incomplete.json"))
        self.assertEqual((out.status, out.signature, out.amount_wei), ("incomplete", None, None))
        self.assertEqual((out.tx_hashes, out.received), ([], []))
        self.assertIn("keeps running the exit by itself", out.sentence)

    def test_withdraw_uses_the_same_route_and_a_refusal_is_a_sentence(self):
        c, opener = client(dict(fx("exit.json"), received=[fx("exit.json")["received"][1]], tx_hashes=[fx("exit.json")["tx_hashes"][1]]))
        out = c.withdraw(AGENT)
        call = opener.calls[0]
        self.assertEqual((call["method"], call["url"], call["body"]), ("POST", BASE + "/v1/agents/{0}/withdraw".format(AGENT), {}))
        self.assertEqual(len(out.received), 1)
        c, _ = client({"error": {"code": "unprocessable", "message": "This agent has an open position. Exit it to take everything out."}}, status=422)
        with self.assertRaises(PonkUnprocessableError) as ctx:
            c.withdraw(AGENT)
        self.assertIn("Exit", ctx.exception.message)

    def test_a_solana_withdrawal_reads_as_it_did(self):
        out = Withdrawal.from_dict({"signature": "5vJz", "lamports": 1500000000, "destination": SOL, "source": "7Q7w", "pnl": None})
        self.assertEqual((out.lamports, out.chain, out.amount_wei, out.received, out.tx_hashes), (1500000000, None, None, [], []))


class TestCopilot(unittest.TestCase):
    def test_a_plan_states_dollars_and_chains(self):
        c, opener = client(fx("plan.json"))
        plan = c.copilot_evm_plan("$100 in bnb and $50 in ethereum")
        call = opener.calls[0]
        self.assertEqual((call["method"], call["url"]), ("POST", BASE + "/copilot/evm/plans"))
        self.assertEqual(call["body"], {"text": "$100 in bnb and $50 in ethereum"})
        self.assertEqual(auth(opener), "Bearer ponk_live_test")
        self.assertEqual((plan.state, plan.total_usd, plan.chains), ("ready", "150.00", ["bsc", "ethereum"]))
        bsc, eth = plan.funding_by_chain
        self.assertEqual((bsc.amount_usd, bsc.amount_native, bsc.native_symbol), ("100.00", "0.146918", "BNB"))
        (leg,) = plan.legs
        self.assertEqual((leg.dex, leg.pool_address, leg.pair), ("pancakeswap_v3", POOL, "USDT/USDC"))
        self.assertEqual((leg.fund_usd, leg.fund_native, leg.fund_native_wei), ("100.00", "0.146918", "146918000000000000"))
        self.assertEqual((leg.deploy_usd, leg.wallet_floor_usd, leg.range_pct), ("99.69", "0.31", 5.0))
        self.assertEqual(leg.config, evm.range_config(0.05, 0.05))
        self.assertAlmostEqual(leg.ranked_apr_pct, leg.fee_apr_pct + leg.farm_apr_pct)
        # The key's own rates, in the server's sentence.
        self.assertIn("1.5%", plan.fees)
        self.assertIn("15%", plan.fees)

    def test_a_share_under_the_minimum_says_what_works_instead(self):
        plan = EvmPlan.from_dict(fx("plan.json"))
        eth = plan.funding_by_chain[1]
        self.assertEqual((eth.legs, eth.amount_native), ([], None))
        self.assertTrue(eth.refusal.startswith("$50 is not enough for one Ethereum pool"))
        self.assertEqual(eth.minimum_usd, "320")
        alt = eth.alternative
        self.assertEqual((alt.kind, alt.chain, alt.amount_usd), ("same_pair_on_chain", "base", "50.00"))
        self.assertTrue(alt.plan_id)
        self.assertEqual(plan.dropped[0].chain, "ethereum")
        self.assertTrue(plan.dropped[0].reason.startswith("Left out"))

    def test_a_refused_plan_is_an_answer_not_an_exception(self):
        c, _ = client({"id": "p", "state": "refused", "text": "0.5 bnb in bnb pools", "refusal": {"code": "coin_amount", "message": "Say the amount in dollars."}, "funding_by_chain": [], "fees": "f"})
        plan = c.copilot_evm_plan("0.5 bnb in bnb pools")
        self.assertEqual((plan.state, plan.refusal["code"], plan.legs, plan.chains), ("refused", "coin_amount", [], []))

    def test_get_and_start(self):
        pid = fx("plan.json")["id"]
        c, opener = client(fx("plan.json"))
        c.copilot_evm_get_plan(pid)
        self.assertEqual(sent(opener), ("GET", BASE + "/copilot/evm/plans/" + pid))
        c, opener = client(fx("plan_started.json"))
        started = c.copilot_evm_start(pid)
        call = opener.calls[0]
        self.assertEqual((call["method"], call["url"]), ("POST", BASE + "/copilot/evm/plans/{0}/start".format(pid)))
        self.assertEqual(call["timeout"], c.fund_timeout)
        self.assertEqual((started.state, started.fleet_id), ("started", "0190a1b2-cccc-7000-8000-0000000000cc"))
        made, refused = started.agents
        self.assertEqual((made.agent_id, made.created, made.fund_native, made.native_symbol), (AGENT, True, "0.146918", "BNB"))
        self.assertEqual((refused.agent_id, refused.created), (None, False))
        self.assertIn("token list", refused.refusal)
        self.assertEqual(started.sign_urls, [evm.fund_link(AGENT, chain="bsc")])

    def test_the_copilot_needs_a_credential_before_anything_is_sent(self):
        c, opener = keyless(fx("plan.json"))
        with self.assertRaises(PonkError):
            c.copilot_evm_plan("$100 in bnb")
        self.assertEqual(opener.calls, [])

    def test_a_session_is_used_when_the_client_holds_one(self):
        opener = StubOpener(fx("fleet.json"))
        c = PonkClient(api_key="ponk_live_test", session_token="sess", base_url=BASE, opener=opener)
        c.evm_fleet()
        self.assertEqual(auth(opener), "Bearer sess")

    def test_the_fleet_says_what_each_agent_needs(self):
        c, opener = client(fx("fleet.json"))
        fleet = c.evm_fleet()
        self.assertEqual(sent(opener), ("GET", BASE + "/agents/evm/fleet"))
        ready, new = fleet.agents
        self.assertEqual((ready.agent_id, ready.chain, ready.needs, ready.mandate_status), (AGENT, "bsc", "nothing", "active"))
        self.assertEqual((new.chain, new.address, new.mandate_status, new.needs), ("base", None, None, "authorize_and_fund"))
        self.assertEqual(fleet.sign_urls, evm.fund_links(fleet.agents))
        self.assertEqual(fleet.fund_page, "https://ponk.exchange/agents/fund")


@unittest.skipUnless(os.environ.get("PONK_LIVE_TEST") == "1", "set PONK_LIVE_TEST=1 to read the live public pool boards")
class TestLiveRead(unittest.TestCase):
    """Opt in, read only, no key: lists pools on BNB Chain and Ethereum."""

    def test_bnb_and_ethereum_boards_answer(self):
        c = PonkClient()
        for chain in ("bsc", "ethereum"):
            board = c.evm_pools(chain)
            self.assertEqual(board.chain, chain)
            self.assertTrue(board.ready, board.status.last_error if board.status else None)
            self.assertTrue(board.pools)
            venues = {}
            for p in board.pools:
                self.assertEqual(evm.parse_evm_id(p.address).chain, chain)
                self.assertIn(chain, evm.venue_chains(p.dex), p.dex)
                venues[p.dex] = venues.get(p.dex, 0) + 1
            top = max(board.pools, key=lambda p: (p.fees.h24 if p.fees and p.fees.h24 else 0))
            print(
                "\n{0}: {1} pools {2}; most LP fees in 24h: {3} on {4}, fee tier {5}, TVL ${6:,.0f}, "
                "fees 24h ${7:,.0f}, farm {8}".format(
                    chain, len(board.pools), dict(sorted(venues.items())), top.pair, top.dex,
                    "dynamic" if top.dynamic_fee else "{0}%".format(top.fee_pct),
                    top.tvl or 0, top.fees.h24 or 0,
                    "none" if top.farm is None else "{0} live={1}".format(top.farm.token, top.farm.live),
                )
            )


if __name__ == "__main__":
    unittest.main()
