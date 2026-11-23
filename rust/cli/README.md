# ponk-cli

Ponk from the terminal, on Solana, Ethereum, Arbitrum, Base, BNB Chain and
Robinhood Chain. The binary is `ponk`; every request goes through the
[`ponk`](..) crate, so this is a renderer over the same public API the app and
the SDKs use.

```
cargo install ponk-cli
# or: cargo install --git https://github.com/ponkexchange/ponk-sdk ponk-cli
```

## Configuration

| Variable | Meaning |
| --- | --- |
| `PONK_API_KEY` | The `ponk_live_...` key from ponk.exchange Settings, API keys. Needed by `agents`, `agent <uuid>`, `create-agent`, `create-evm-agent`, `wallet`, `pause`, `resume`, `withdraw`, `collect-all`, `exit`, `open-cost`, `plan`, `start-plan`, `evm-fleet`, `feed-prefs` and `whoami`. The pool, chain and leaderboard reads and `fund-link` need none. |
| `PONK_API_URL` | API base, default `https://ponk.exchange/api`. |
| `PONK_WEB_URL` | The web origin for the two app-served boards, only when it is not the API base minus `/api` (local development). |

`--json` on any command prints the raw response for the same request.

## Commands

```
ponk pools                         LP pools across Meteora DLMM, Meteora DAMM v2,
                                   Orca Whirlpools and Raydium CLMM, most 24h
                                   fees first
ponk pools --view hot              a Meteora board view: hot, new, volume, tvl
ponk pool <address> [--dex d]      one pool, decoded from chain (venues probed)
ponk agents                        every agent your key's wallet owns
ponk agent <uuid | public slug>    one agent, yours or a shared one
ponk create-agent --name n --dex meteora_dlmm --strategy bin_rebalancer \
  --pool <address> --config cfg.json [--dry-run]
ponk pause <id> / ponk resume <id>
ponk withdraw <id> [--lamports n]  SOL home; destination is server-locked to
                                   the wallet that owns the agent
ponk collect-all [ids...] [--keep-in-agent]
                                   collect every managed agent's fees to your
                                   wallet; skipped and unknown items say why
ponk leaderboard [--window 7d] [--chains bsc,solana]
                                   Top Ponkillionaires, with a CHAIN column;
                                   --chains (ids or all) ranks from 1 over
                                   the rows kept
ponk feed-prefs                    your saved chain filter
ponk feed-prefs set <ids | all>    save it (trade key)
ponk whoami                        what the key is allowed to do
```

On the EVM chains (`ethereum`, `arbitrum`, `base`, `bsc`, `robinhood`):

```
ponk chains                        every chain, its gas coin and its venues
ponk evm-pools bsc [--venue pancakeswap_v3] [--search usdt] [--limit 25]
                                   the pool board, most 24h LP fees first: fee
                                   tier, TVL, volume, fees, farm APR, flags
ponk evm-pool <pool id>            one pool (eip155:<chain id>:0x...)
ponk open-cost bsc --dex pancakeswap_v3 [--pool <pool id>] [--no-stake]
  [--shape curve --amount-usd 1000 [--ranges auto] [--range-pct 5] [--side below]]
                                   gas of the first open, the exit reserve,
                                   the fee rates your key's agents pay, and
                                   with --shape the ranges it opens as now
ponk create-evm-agent --name n --chain bsc --dex pancakeswap_v3 \
  --pool <pool id> --down-pct 0.05 --up-pct 0.05 [--stake] \
  [--shape curve] [--shape-ranges auto] \
  [--withdrawal-address 0x...] [--dry-run]
                                   creates the agent, prints its deposit
                                   address and the fund-and-sign link
ponk wallet <id>                   deposit address, balance, mandate status
ponk fund-link <id>... [--chain bsc]
                                   the ponk.exchange page where your own
                                   wallets fund and sign; signs nothing
ponk evm-fleet                     every EVM agent and what it needs from you
ponk plan "$200 in the top 5 bnb pools" [--shape curve] [--one-sided below]
ponk start-plan <plan id>          one agent per pool; moves no money
ponk exit <id> --confirm           close the position and send everything home
```

`--down-pct` and `--up-pct` are fractions: 0.05 is 5% either side of the
price, and 0 on one side is a one-sided range. A full-range venue
(`uniswap_v2`, `pancakeswap_v2`, `aerodrome_classic`) takes neither, and no
shape. `--shape` is `spot`, `curve`, `bid_ask` or `spot_bid_ask`: on Uniswap
v3 (Arbitrum, Ethereum, Robinhood Chain), Uniswap v4 (Arbitrum, Ethereum),
PancakeSwap v3 and Infinity CL (BNB Chain) and Aerodrome Slipstream (Base) a
shape other than Spot opens as several ranges managed as one position
(`--shape-ranges auto` or 1 to 7); on any other range venue it opens one
range. The mandate is signed by your own wallets
on ponk.exchange; the CLI prints the link and never signs.

Perpetuals agents on Hyperliquid (`ponk perpetuals ...`; `markets` and
`plan` need no key):

```
ponk perpetuals markets [--search bt] [--sort volume|oi|change|funding|leverage] [--limit 25]
ponk perpetuals preview --coin BTC --side long --leverage 3 --capital 500 --stop 3 [--tp 10] [--practice]
ponk perpetuals create  --coin BTC --side long --leverage 3 --capital 500 --stop 3 [--tp 10] [--name n] [--practice]
ponk perpetuals create  --coin BTC --side long --leverage 20 --capital 200 --stop 2 --name "BTC 20x" --confirm-name "BTC 20x"
ponk perpetuals agents / agent <id> / decisions <id> / ledger <id> / card <id>
ponk perpetuals say "long btc 3x 500 stop 3" | "close my sol short" --confirm | "tighten btc stop to 2%"
ponk perpetuals close <id> --confirm   close the position; funds stay on the agent's account
ponk perpetuals stop <id> 2            move the stop to 2% from the price, closer only
ponk perpetuals exit <id> --confirm    close, settle, withdraw to the proven address
ponk perpetuals plan "put $1,000 across 2 Perpetuals agents, long BTC, short SOL" [--practice]
ponk perpetuals start-plan <plan id> [--confirm-name 0="BTC Long 20x"]   one agent per leg; moves no money
ponk perpetuals fleet-stop <fleet id> 2 / fleet-pause <fleet id> / fleet-resume <fleet id>
```

`--leverage` goes up to the market's own maximum (`MAX LEV` on `ponk
perpetuals markets`). Above 10x the high leverage warning ("Above 10x a sudden
4% price jump can pass the liquidation price before any stop fills.") is
acknowledged by typing the agent's name with `--confirm-name` (the `--name`
given, or the coin as typed); without it `create` prints the warning and the
flag to add, and sends nothing. At any leverage, a stop inside the
liquidation price but with liquidation closer than twice the stop's distance
is the stop-room warning, acknowledged the same way: only the server knows
it, so `preview` prints the warnings the name acknowledges and the
`--confirm-name` to add, and `create` and `say` add the flag to the server's
sentence when it asks for the name. One name covers every warning, and
`say --confirm-name` works at any leverage. A stop at or past the
liquidation price is refused, naming a stop that would work. On a fleet,
`start-plan` takes one `--confirm-name INDEX=NAME` per leg `plan` asks the
name for, after printing that leg's warnings. `preview`, `create`, `say` and
`plan` print what the confirm step warns about (a crowded side, a thin
market, above 10x, little stop room) before the result, and `agent` prints
the warnings the agent was created with. `ponk pause` and `ponk resume` work on a
Perpetuals agent too. Authorizing the agent and funding its Hyperliquid
account are wallet signatures in the app; the CLI never signs.

Two rules it keeps from the product: a figure the server did not send renders
as `-`, never zero and never a guess, and a create on a pool the deploy gate
dislikes prints the gate's sentence while still creating the agent, because
the gate warns and never refuses.
