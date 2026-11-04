# ponk-cli

Ponk from the terminal. The binary is `ponk`; every request goes through the
[`ponk`](..) crate, so this is a renderer over the same public API the app and
the SDKs use.

```
cargo install --git https://github.com/ponkexchange/ponk-sdk ponk-cli
```

## Configuration

| Variable | Meaning |
| --- | --- |
| `PONK_API_KEY` | The `ponk_live_...` key from ponk.exchange Settings, API keys. Needed by `agents`, `agent <uuid>`, `create-agent`, `pause`, `resume`, `withdraw` and `whoami`. The pool and leaderboard reads need none. |
| `PONK_API_URL` | API base, default `https://ponk.exchange/api`. |
| `PONK_WEB_URL` | The web origin for the two app-served boards, only when it is not the API base minus `/api` (local development). |

`--json` on any command prints the raw response for the same request.

## Commands

```
ponk pools                         LP pools across Meteora DLMM, Orca Whirlpools
                                   and Raydium CLMM, most 24h fees first
ponk pools --view hot              a Meteora board view: hot, new, volume, tvl
ponk pool <address> [--dex d]      one pool, decoded from chain (venues probed)
ponk agents                        every agent your key's wallet owns
ponk agent <uuid | public slug>    one agent, yours or a shared one
ponk create-agent --name n --dex meteora_dlmm --strategy bin_rebalancer \
  --pool <address> --config cfg.json [--dry-run]
ponk pause <id> / ponk resume <id>
ponk withdraw <id> [--lamports n]  SOL home; destination is server-locked to
                                   the wallet that owns the agent
ponk leaderboard [--window 7d]     Top Ponkillionaires
ponk whoami                        what the key is allowed to do
```

Two rules it keeps from the product: a figure the server did not send renders
as `-`, never zero and never a guess, and a create on a pool the deploy gate
dislikes prints the gate's sentence while still creating the agent, because
the gate warns and never refuses.
