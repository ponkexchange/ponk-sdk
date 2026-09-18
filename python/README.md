# ponk (Python)

A thin client for the ponk public API: create and drive Solana DLMM liquidity
agents, read their live positions, PnL and fees, and move funds home.

Standard library only. No dependencies, no code generation, one method per
endpoint. It never computes a number the server did not send.

## Install

The package is not on PyPI. Use it from this repository:

```bash
pip install /path/to/ponk/packages/sdk-python
```

or put the `packages/sdk-python` directory on `PYTHONPATH` and `import ponk`.

Python 3.8 or newer.

## Get a key

Open [Settings, API keys](https://ponk.exchange/settings) in the app with your
wallet. Name the key, pick **Read only** (`read`) or **Read and act**
(`trade`), and copy the secret. It is shown once: the server stores only a hash
of it, so it genuinely cannot be shown again. Lost keys get revoked and
replaced, not recovered.

A key carries exactly the authority of the wallet that minted it. It can never
see or touch another account, and it cannot mint or revoke keys, so a leaked
key cannot extend or outlive its own revocation.

## Quickstart

```python
import os
from ponk import PonkClient

ponk = PonkClient(api_key=os.environ["PONK_API_KEY"])

me = ponk.whoami()
print("acting as", me.wallet_address, "with scope", me.scope)

for agent in ponk.list_agents():
    print(agent.name, agent.status, agent.strategy, "dry_run" if agent.dry_run else "live")

    perf = ponk.agent_performance(agent.id)
    # Every USD field is a string or None. None means "could not be priced",
    # never zero, so print a dash rather than a number you do not have.
    print("  value", perf.current_value_usd or "-", "pnl", perf.pnl_usd or "-")

    pos = ponk.agent_position(agent.id)
    if pos.position_address:
        print("  range", pos.lower_price or "-", "to", pos.upper_price or "-",
              "in range" if pos.in_range else "OUT OF RANGE")

for log in ponk.list_logs(limit=20):
    print(log.created_at, log.action_type, log.status, log.tx_signature or "")
```

Create an agent, watch it think, then let it trade:

```python
agent = ponk.create_agent(
    name="sol-usdc runner",
    wallet_address=me.wallet_address,   # must be the key's own wallet
    dex="meteora_dlmm",                 # meteora_dlmm | orca | ponk_clouds
    pool_address="POOL_ADDRESS",
    strategy="bin_rebalancer",
    config={"kind": "bin_rebalancer", "bin_range_width": 20, "rebalance_threshold_bins": 5},
    dry_run=True,                       # the server's default is also True
)

# A dry-run agent runs its whole strategy loop and logs every decision to
# list_logs without sending a transaction. Read those logs before going live.
ponk.set_dry_run(agent.id, False)
```

Move funds home:

```python
wallet = ponk.agent_wallet(agent.id)
print(wallet.pubkey, wallet.lamports, "lamports")

ponk.withdraw(agent.id, lamports=500_000_000)   # 0.5 SOL, or omit to sweep
result = ponk.exit_agent(agent.id)              # stop, close, sweep everything
print("swept to", result.destination, "sig", result.signature)
```

## What a key cannot do

Three limits are structural. They are properties of how custody works in ponk,
not settings that can be turned off.

* **It cannot sign with your connected wallet.** Non-custodial actions still
  need your signature in the app. This client can read their state, not produce
  the signature.
* **It cannot grant itself custody.** Turning an agent autonomous requires a
  one-time custody mandate you sign with your wallet. Create the agent here if
  you like, then enable autonomous mode once in the app; from then on this
  client can drive it.
* **It cannot act on an agent that holds no wallet of its own.** `compound`,
  `withdraw`, `exit` and `agent_wallet` operate on the isolated wallet an
  autonomous agent owns. A self-custody agent has no such wallet, so those four
  do not apply to it. Reads, pause, resume, dry-run and mode do.

`withdraw` and `exit` have no destination parameter and cannot be given one.
The server locks the destination to the wallet that owns the agent. A stolen
key can move your funds home. It cannot move them anywhere else.

## Methods

| Method | Endpoint | Needs |
| --- | --- | --- |
| `health()` | `GET /health` | nothing |
| `pool(dex, address)` | `GET /pools/{dex}/{address}` | nothing |
| `ponk_perks(wallet_address)` | `GET /public/ponk/perks/{address}` | nothing |
| `whoami()` | `GET /v1/me` | `read` |
| `list_agents()` | `GET /v1/agents` | `read` |
| `get_agent(id)` | `GET /v1/agents/{id}` | `read` |
| `agent_performance(id)` | `GET /v1/agents/{id}/performance` | `read` |
| `agent_position(id)` | `GET /v1/agents/{id}/position` | `read` |
| `agent_wallet(id)` | `GET /v1/agents/{id}/wallet` | `read` |
| `list_positions()` | `GET /v1/positions` | `read` |
| `list_logs(limit)` | `GET /v1/logs` | `read` |
| `create_agent(...)` | `POST /v1/agents` | `trade` |
| `pause_agent(id)` | `POST /v1/agents/{id}/pause` | `trade` |
| `resume_agent(id)` | `POST /v1/agents/{id}/resume` | `trade` |
| `set_dry_run(id, dry_run)` | `POST /v1/agents/{id}/dry-run` | `trade` |
| `set_mode(id, mode)` | `POST /v1/agents/{id}/mode` | `trade` |
| `compound(id)` | `POST /v1/agents/{id}/compound` | `trade` |
| `withdraw(id, lamports)` | `POST /v1/agents/{id}/withdraw` | `trade` |
| `exit_agent(id)` | `POST /v1/agents/{id}/exit` | `trade` |

The first three need no key at all. `PonkClient()` with no `api_key` reaches
them and nothing else. `health()` returns its report even when the API answers
503, because which component is down is the whole point of that call.

This client covers the key-authenticated `/v1` API plus those three public
reads. The rest of the server's surface, the session-authenticated app routes
and the wallet-signed Meteora transaction builders under `/public/meteora`,
needs a wallet signature rather than a key, so it is out of scope here.

## Errors

Every error carries the same envelope, and this client maps it by status:

```json
{"error": {"code": "conflict", "message": "exit already running", "request_id": "..."}}
```

| Status | Exception | What to do |
| --- | --- | --- |
| 400 | `PonkBadRequestError` | Fix the request. `code` is `invalid_strategy_config` when the config did not match the strategy, and `message` names the field. |
| 401 | `PonkAuthError` | Missing, malformed, unknown or revoked key. The last two are deliberately indistinguishable. Do not retry. |
| 403 | `PonkForbiddenError` | The key is valid but read-only. Mint a `trade` key. Do not retry. |
| 404 | `PonkNotFoundError` | No such object, or it belongs to another account. Also deliberately indistinguishable. |
| 409 | `PonkConflictError` | Conflicts with in-flight work, for example an exit already running. Retry after a pause. |
| 422 | `PonkUnprocessableError` | Understood but cannot apply, for example compounding an agent with no open position. `code` carries the risk code when there is one. |
| 429 | `PonkRateLimitedError` | Back off and retry. |
| 5xx | `PonkServerError` | Quote `request_id` to support. |
| no response | `PonkTransportError` | DNS, connection, TLS or timeout. |

All of them subclass `PonkError`. `PonkAPIError` carries `status`, `code`,
`message`, `request_id` and the decoded `body`.

```python
from ponk import PonkAPIError, PonkConflictError, PonkTransportError

try:
    ponk.exit_agent(agent_id)
except PonkConflictError:
    pass                      # an exit is already running; it will finish
except PonkTransportError:
    # A fund call that times out has NOT necessarily failed: the work keeps
    # running on the server. Exit is idempotent, so call it again and it
    # continues rather than double-sending.
    ponk.exit_agent(agent_id)
except PonkAPIError as e:
    print(e.status, e.code, e.message, e.request_id)
```

`compound`, `withdraw` and `exit` send several transactions and wait for each
confirmation, so they can take a minute. They get their own timeout,
`fund_timeout` (180s), separate from `timeout` (30s) for everything else.

## Reading the numbers

* **A null USD figure means unknown, not zero.** An unpriceable pool returns
  `null` rather than a fabricated 0. Render it as a dash. Treating it as zero
  silently understates a position.
* **USD amounts on the agent and position endpoints are strings.** Feed them to
  `decimal.Decimal` before doing arithmetic. They are strings precisely so a
  float cannot round them.
* **`claimable_fees_usd` and `fee_pnl_usd` are different numbers.** The first is
  the unclaimed fees sitting on the position right now, the second is lifetime
  fees earned, claimed plus unclaimed. Do not add them.
* **PnL decomposes as Total = Price + Fee**: `pnl_usd`, `price_pnl_usd`,
  `fee_pnl_usd`. `pnl_source` says whether the figures are Meteora's own
  (`meteora_datapi`) or ponk's on-chain valuation (`oracle_estimate`).
* **`cost_basis_estimated`** marks a position ponk did not open. The basis is
  then a first-observation stamp, so lifetime PnL stays `None` instead of
  asserting a gain that contradicts your real numbers.
* **Every object keeps `raw_json`**, the exact decoded payload, so a field added
  to the API after this client was written is still reachable.

## Fees

Autonomous agents charge a performance fee on realized yield only. Principal is
never touched and no fee is taken on an unrealized gain. There is no charge for
a key, for a call, for creating an agent, for depositing or for withdrawing.

**An agent created through this API pays the API rate, not the app rate.** The
rate is fixed when the agent is created and follows that agent for its whole
life, including after you enable autonomous mode for it in the app. Read both
rates live rather than hardcoding them:

```python
perks = ponk.ponk_perks(me.wallet_address)
print("app agents", perks.managed_agent_fee.effective_percent, "%")
print("api agents", perks.api_agent_fee.effective_percent, "%")
print("holding $PONK would make that", perks.api_agent_fee.holder_percent, "%")
```

`agent.managed_fee_bps` is the rate that specific agent's skim actually
charges, before any $PONK holder discount.

## Tests

Offline, no network, no fixtures to refresh:

```bash
cd packages/sdk-python && python3 -m unittest discover -s tests -v
```
