"""Open 10 agents, fund them with one signature, close them all.

    PONK_API_KEY=ponk_live_... PONK_POOL=<meteora dlmm pool> python3 examples/fleet.py

Needs a `trade` key ("Read and act"). By default every agent is a DRY RUN:
it runs its whole decision loop and logs what it would do without sending a
transaction. Set PONK_LIVE=1 for live agents. The funding step moves real SOL
even for dry-run agents, if you sign it; the exit at the end sweeps it home.

What this script never does: sign, broadcast, or ask for a private key. The
SDK cannot sign and neither can the ponk server, which holds no key for your
wallet. `fund_prepare` returns UNSIGNED transfers from your wallet to your
agents' own wallets, and your wallet signs them: open the printed link on
ponk.exchange with your wallet connected, or sign the base64 transactions in
wallet software you already control (see `sign_with_your_own_wallet` below).

The steps:

1. `create_agents`: 10 agents in one call, each with its own managed wallet.
2. `fund_prepare`: one unsigned transaction carrying all 10 transfers.
3. You sign it (on ponk.exchange, which then queues the custody mandate each
   agent needs before it can act on its funds).
4. `iter_fleet`: every agent with its wallet balance, mandate and value.
5. `exit_agents`: close every position and sweep every wallet home, then
   `get_batch` until nothing is still running.
"""

from __future__ import annotations

import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))

from ponk import PonkClient  # noqa: E402

COUNT = 10


def lamports_to_sol(lamports):
    return "-" if lamports is None else "{0:.4f}".format(lamports / 1e9)


def sign_with_your_own_wallet(plan):
    """How a caller that already controls its wallet in code would sign.

    Comment only. This SDK holds no key and never signs. Each
    `plan.transactions[i].unsigned_tx_b64` is a legacy Solana transaction,
    base64 of the wire format, with `plan.owner` as fee payer and only signer.
    With a wallet library you already use (for example `solders`):

        raw = base64.b64decode(tx.unsigned_tx_b64)
        # deserialize it as a legacy Transaction, have YOUR signer sign the
        # message (it already carries plan.recent_blockhash), then send it
        # before plan.last_valid_block_height and wait for confirmation.

    The agents still need a custody mandate before they act on the funds
    (`plan.needs_mandate`). A mandate is a wallet signature made in the ponk
    app; the signing page from `plan.sign_urls` queues it for you.
    """


def main():
    ponk = PonkClient(api_key=os.environ["PONK_API_KEY"], base_url=os.environ.get("PONK_BASE_URL", "https://ponk.exchange/api"))
    pool = os.environ["PONK_POOL"]
    live = os.environ.get("PONK_LIVE") == "1"

    me = ponk.whoami()
    print("acting as", me.wallet_address, "scope", me.scope, "live" if live else "dry run")

    # 1. Create the fleet. wallet_address is filled in by the server.
    batch = ponk.create_agents(
        [
            {
                "name": "fleet {0}".format(i + 1),
                "dex": "meteora_dlmm",
                "pool_address": pool,
                "strategy": "bin_rebalancer",
                "config": {"kind": "bin_rebalancer", "bin_range_width": 20, "rebalance_threshold_bins": 5},
                "dry_run": not live,
            }
            for i in range(COUNT)
        ]
    )
    while batch.running:
        time.sleep(5)
        batch = ponk.get_batch(batch.batch_id)
    agent_ids = [item.agent_id for item in batch.items if item.ok and item.agent_id]
    for item in batch.items:
        if not item.ok:
            print("agent", item.index, "not created:", item.error)
    print("created", len(agent_ids), "of", batch.total)
    if not agent_ids:
        return

    # 2. One unsigned transaction funding every agent to its opening balance.
    plan = ponk.fund_prepare(agent_ids)
    for it in plan.items:
        if not it.ok:
            print("  left out", it.agent_id, "-", it.refusal)
    if not plan.ok:
        print("funding refused:", plan.refusal)
    else:
        print(
            "fund {0} SOL to {1} agents in {2} transaction(s), network fee {3} SOL".format(
                lamports_to_sol(plan.total_lamports),
                sum(1 for it in plan.items if it.ok),
                len(plan.transactions),
                lamports_to_sol(plan.network_fee_lamports),
            )
        )
        # 3. Your wallet signs. This script does not and cannot.
        print("sign with your wallet on ponk.exchange:")
        for url in plan.sign_urls:
            print("  ", url)
        sign_with_your_own_wallet(plan)
        input("press Enter once you have signed ")

    # 4. The fleet, every page.
    for row in ponk.iter_fleet():
        a, w, m, p = row.agent, row.wallet, row.mandate, row.performance
        if a.id not in agent_ids:
            continue
        print(
            "{0:<10} {1:<9} wallet {2:>8} SOL  mandate {3:<8} value {4}".format(
                a.name,
                a.status,
                lamports_to_sol(w.lamports if w else None),
                m.status if m else "none",
                # None means could not be priced, never zero.
                (p.current_value_usd if p else None) or "-",
            ),
            "; ".join(row.read_errors),
        )

    # 5. Close them all, and collect what was still running at the response.
    out = ponk.exit_agents(agent_ids)
    while out.running:
        time.sleep(5)
        out = ponk.get_batch(out.batch_id)
    print("exit: {0} done, {1} failed".format(out.done, out.failed))
    for item in out.items:
        if item.outcome == "failed":
            print("  ", item.agent_id, item.code, item.error)


if __name__ == "__main__":
    main()
