# ponk (Python)

A thin client for the ponk public API: create and drive Solana DLMM liquidity
agents, read their live positions, PnL and fees, and move funds home.

Standard library only. No dependencies, no code generation, one method per
endpoint. It never computes a number the server did not send.

## Install

```bash
pip install ponk
```

PyPI carries 0.2.0. This directory is 0.3.0, which adds `agent_mandate`,
`set_autonomous` and `range_cost` and makes `create_agent`'s `dry_run`
required (see [Changes in 0.3.0](#changes-in-030)). Until 0.3.0 is on PyPI,
install it from this repository:

```bash
git clone https://github.com/ponkexchange/ponk-sdk
pip install ./ponk-sdk/python
```

or put the `python` directory on `PYTHONPATH` and `import ponk`.

Python 3.8 or newer.

## Webhooks

ponk POSTs a signed JSON body to a URL you register. Verify it before you
act on it:

```python
from ponk import verify, InvalidSignature

@app.post("/ponk")
def receive(request):
    try:
        event = verify(
            raw_body=request.get_data(),               # BYTES, as received
            signature_header=request.headers["X-Ponk-Signature"],
            secret=MY_WEBHOOK_SECRET,
        )
    except InvalidSignature:
        return "", 400
    if event.event == "agent_out_of_range":
        page_someone(event.agent_id)
    return "", 200        # anything but 2xx is a failure, and ponk retries
```

Verify the **raw bytes you received**. The signature covers the exact body on
the wire, and `json.dumps(json.loads(body))` is not guaranteed to reproduce
it, so verifying a re-serialized dict fails for reasons that look like a ponk
bug and are not.

`verify` checks the HMAC and the age of the delivery, and raises
`InvalidSignature` with a message saying which of the two failed. The
timestamp is inside the MAC, so a captured delivery cannot be aged forward.

Registering endpoints is not in this client, and that is deliberate: the API
scopes it to a signed-in session rather than to an API key, exactly as it does
for minting keys. Register them in the app, under Settings, then verify here.

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
    dex="meteora_dlmm",                 # meteora_dlmm | orca | ponk_clouds | raydium
    pool_address="POOL_ADDRESS",
    strategy="bin_rebalancer",
    config={"kind": "bin_rebalancer", "bin_range_width": 20, "rebalance_threshold_bins": 5},
    dry_run=True,                       # required: True simulates, False is live
)

# A dry-run agent runs its whole strategy loop and logs every decision to
# list_logs without sending a transaction. Read those logs before going live.
ponk.set_dry_run(agent.id, False)
```

`dry_run` is required by this client even though the server accepts it
omitted, because on `POST /v1/agents` an omitted `dry_run` creates a **live**
agent, the opposite of the app's default. A non-boolean raises `TypeError`
before anything is sent. A live agent created here is still non-custodial: it
recommends, and cannot sign for funds until a mandate signed in the app makes
it autonomous.

For Degen Mode, `strategy="entry_agent"`: the agent starts with no pool, so
`pool_address` and `position_address` are ignored, and its config's `venues`
must include one where ponk can discover pools, which today is only
`meteora_dlmm`.

Price a wide Meteora range before you ask for one. The quote separates the
rent you get back from the rent you do not:

```python
q = ponk.range_cost("meteora_dlmm", "POOL_ADDRESS", bins=400)   # TOTAL bins
print(q.total_bins, "bins,", q.down_pct, "% to", q.up_pct, "%")
print("hold", q.total_upfront_lamports, "lamports;",
      q.non_refundable_lamports, "of it does not come back")
if q.clamped:
    print("the venue caps a position at", q.max_total_bins, "bins")
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
* **It cannot create a custody mandate.** A mandate is a signature from your
  wallet, so it is granted in the app with your wallet connected. Once an
  agent has one, a `trade` key can use it: `set_autonomous(id, True)` turns
  autonomous signing on under that existing, unexpired mandate, and
  `set_autonomous(id, False)` turns it off, which is always allowed. This
  client never sends a `grant`, and the server refuses a key request that
  carries one.
* **It cannot act on an agent that holds no wallet of its own.** `compound`,
  `withdraw`, `exit` and `agent_wallet` operate on the isolated wallet an
  autonomous agent owns. A self-custody agent has no such wallet, so those four
  do not apply to it. Reads, pause, resume, dry-run and mode do.

`withdraw` and `exit` have no destination parameter and cannot be given one.
The server locks the destination to the wallet that owns the agent. A stolen
key can move your funds home. It cannot move them anywhere else.

## The mandate expires

A mandate carries a hard expiry, 30 days by default in the app. Lapsing is a
clock fact with nothing on chain to mark it: the agent keeps running and every
signature it asks for is denied, so it quietly stops compounding, claiming and
recentering with its funds still deployed. Read the expiry instead of waiting
for the silence:

```python
from ponk import PonkNotFoundError

try:
    m = ponk.agent_mandate(agent.id)
except PonkNotFoundError:
    m = None            # no mandate: this agent is not autonomous

if m is not None:
    # seconds_remaining is computed by the server and goes negative once the
    # mandate has lapsed, so it does not depend on your clock.
    if m.status == "expired":
        print("expired", m.expires_at, "- renew it in the app")
    elif m.seconds_remaining < 3 * 86400:
        print("expires", m.expires_at, "- renew it in the app soon")
```

* `max_action_value_usd` is `None` when there is no per-action ceiling. That
  means unlimited, not zero.
* `set_autonomous(id, False)` does not revoke the mandate. A later
  `set_autonomous(id, True)` re-arms the agent while the mandate is unexpired.
  Enabling also sets the agent live, but it does not restart an agent a
  previous exit left `stopped`: read `status` on the result and call
  `resume_agent` if you need the loop running.
* `set_autonomous(id, True)` needs the agent's own managed wallet with an
  active, unexpired mandate that still pays out to this account's wallet.
  Otherwise it raises `PonkUnprocessableError` with a sentence saying which.
* `compound` can return a receipt and still have done nothing. A mandate
  denial is recorded on the action, not raised, so read `signature is None`
  as "nothing reached the chain" and `error_message` for why.
* A lapsed mandate never traps funds. `withdraw` and `exit` deliberately
  bypass it and keep working after expiry.
* ponk also sends `mandate_expiring` (from five days ahead) and
  `mandate_expired` notifications. A webhook registered from a signed-in
  session receives them; see [Webhooks](#webhooks).

## Methods

| Method | Endpoint | Needs |
| --- | --- | --- |
| `health()` | `GET /health` | nothing |
| `pool(dex, address)` | `GET /pools/{dex}/{address}` | nothing |
| `range_cost(dex, address, bins)` | `GET /pools/{dex}/{address}/range-cost?bins=N` | nothing |
| `ponk_perks(wallet_address)` | `GET /public/ponk/perks/{address}` | nothing |
| `whoami()` | `GET /v1/me` | `read` |
| `list_agents()` | `GET /v1/agents` | `read` |
| `get_agent(id)` | `GET /v1/agents/{id}` | `read` |
| `agent_performance(id)` | `GET /v1/agents/{id}/performance` | `read` |
| `agent_position(id)` | `GET /v1/agents/{id}/position` | `read` |
| `agent_wallet(id)` | `GET /v1/agents/{id}/wallet` | `read` |
| `agent_mandate(id)` | `GET /v1/agents/{id}/mandate` | `read` |
| `list_positions()` | `GET /v1/positions` | `read` |
| `list_logs(limit)` | `GET /v1/logs` (limit 1 to 500, default 100) | `read` |
| `get_json(path, params)` | any `GET`, raw JSON | depends on the route |
| `create_agent(...)` | `POST /v1/agents` | `trade` |
| `pause_agent(id)` | `POST /v1/agents/{id}/pause` | `trade` |
| `resume_agent(id)` | `POST /v1/agents/{id}/resume` | `trade` |
| `set_autonomous(id, enabled)` | `POST /v1/agents/{id}/autonomous` | `trade` |
| `set_dry_run(id, dry_run)` | `POST /v1/agents/{id}/dry-run` | `trade` |
| `set_mode(id, mode)` | `POST /v1/agents/{id}/mode` | `trade` |
| `compound(id)` | `POST /v1/agents/{id}/compound` | `trade` |
| `withdraw(id, lamports)` | `POST /v1/agents/{id}/withdraw` | `trade` |
| `exit_agent(id)` | `POST /v1/agents/{id}/exit` | `trade` |

Receiver-side, needing no key and no network:

| Function | Purpose |
| --- | --- |
| `verify(raw_body, signature_header, secret)` | Check a webhook delivery and parse it |
| `parse_signature_header(header)` | Split `t=`/`v1=` out of the header |
| `EVENT_KINDS` | Every kind ponk sends, for recognition, never for rejection |

The first four need no key at all. `PonkClient()` with no `api_key` reaches
them and nothing else. `pool` and `range_cost` are rate limited per IP. `health()` returns its report even when the API answers
503, because which component is down is the whole point of that call.

This client covers the key-authenticated `/v1` API plus those four public
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
| 429 | `PonkRateLimitedError` | Back off and retry. Only the public per-IP routes (`pool`, `range_cost`, `ponk_perks`) are rate limited; `/v1` is not. |
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

## Changes in 0.3.0

* **Breaking: `create_agent` requires `dry_run`.** 0.2.0 documented an
  omitted `dry_run` as a dry run. On `/v1` it is the opposite: an omitted
  `dry_run` creates a live agent. Code that relied on the old wording was
  creating live agents, so 0.3.0 makes the choice explicit and raises
  `TypeError` if it is missing or not a `bool`.
* `agent_mandate(id)` reads the live custody mandate, including its expiry.
* `set_autonomous(id, enabled)` turns autonomous signing on under an existing
  mandate, or off. It never creates a mandate.
* `range_cost(dex, address, bins)` quotes what a Meteora range costs to open.
* `get_json(path, params)` reaches any GET this client does not name.
* The User-Agent now carries the real package version (it said 0.1.0).

## Tests

Offline, no network, no fixtures to refresh:

```bash
cd python && python3 -m pytest
```

`python3 -m unittest discover -s tests -v` runs the same suite without
pytest.
