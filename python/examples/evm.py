"""Agents on an EVM chain: list the pools on BNB Chain, quote the first open,
and (only with PONK_CREATE=1) create an agent on the pool with the most LP
fees, then print its deposit address and the fund-and-sign link.

    PONK_API_KEY=ponk_live_... python3 examples/evm.py
    PONK_API_KEY=ponk_live_... PONK_CREATE=1 python3 examples/evm.py

Nothing here moves money. An agent acts only after you send its chain's coin
to its address and sign its mandate with your own wallets on ponk.exchange.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from ponk import PonkClient, evm, fund_link  # noqa: E402

CHAIN = "bsc"
VENUE = "pancakeswap_v3"


def main() -> None:
    ponk = PonkClient(api_key=os.environ["PONK_API_KEY"])

    board = ponk.evm_pools(CHAIN, venue=VENUE)
    # A flagged pool (an unverified token, a hook an agent cannot use, under
    # an hour old) is left out, as the copilot leaves it out.
    pools = [p for p in board.pools if p.fees and p.fees.h24 and not p.flags()]
    if not pools:
        raise SystemExit("the board has no unflagged pool with fees right now")
    pool = max(pools, key=lambda p: p.fees.h24)
    print(pool.pair, "fee tier", pool.fee_pct, "% TVL", pool.tvl, "LP fees 24h", pool.fees.h24)
    if pool.farm:
        print("farm", pool.farm.token, "live", pool.farm.live, "pool APR %", pool.farm.apr_pct)

    # The rates on this quote are the ones an agent this key creates pays.
    cost = ponk.evm_open_cost(CHAIN, VENUE, pool=pool.address)
    coin = evm.NATIVE_SYMBOL[CHAIN]
    print(
        "first open: at least", evm.wei_to_native(cost.min_open_wei) or "-", coin,
        "in the agent wallet, plus the amount to deploy; ponk fee",
        cost.activation_fee_bps, "bps of the first open,", cost.performance_fee_bps, "bps of fees earned",
    )

    if os.environ.get("PONK_CREATE") != "1":
        print("set PONK_CREATE=1 to create the agent")
        return

    agent = ponk.create_evm_agent(
        name="bnb " + pool.pair,
        wallet_address=ponk.whoami().wallet_address,
        chain=CHAIN,
        dex=VENUE,
        pool_address=pool.address,
        down_pct=0.05,  # fractions: 5% either side of the price
        up_pct=0.05,
        stake=True,  # farm CAKE
        dry_run=False,
    )
    wallet = ponk.create_agent_wallet(agent.id)
    print("send", coin, "to", wallet.address)
    print("then sign:", fund_link(agent.id, chain=CHAIN))


if __name__ == "__main__":
    main()
