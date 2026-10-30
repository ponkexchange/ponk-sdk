# ponk (Python)

A thin client for the ponk public API: create and drive Solana DLMM liquidity
agents, read their live positions, PnL and fees, and move funds home.

Standard library only. No dependencies, no code generation, one method per
endpoint. It never computes a number the server did not send.

## Install

```bash
pip install ponk
```

0.7.0 brings everything the site does to a key: Jupiter limit, trailing,
TP/SL and DCA orders (a sign link, or self-sign with your own signer
callback), copy and live follow, Copy LP, the self-custody prepare and mark
loop, calls, push, card looks, referral codes, privacy, and preset ranking in
one call (see [Changes in 0.7.0](#changes-in-070)). To install
from this repository
instead:

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

A mandate carries a hard expiry, 365 days by default in the app (30, 90 or
180 when you pick them). Lapsing is a
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
* `mandates()` reads every autonomous agent's mandate at once, the most
  urgent first, with `renew_all_url`: one page where your wallet signs every
  renewal in turn.
* ponk also reminds at 7 days, 2 days and on expiry (`mandate_expiring` and
  `mandate_expired` notifications). A webhook registered from a signed-in
  session receives them; see [Webhooks](#webhooks).

## Methods

| Method | Endpoint | Needs |
| --- | --- | --- |
| `health()` | `GET /health` | nothing |
| `pool(dex, address)` | `GET /pools/{dex}/{address}` | nothing |
| `range_cost(dex, address, bins)` | `GET /pools/{dex}/{address}/range-cost?bins=N` | nothing |
| `ponk_perks(wallet_address)` | `GET /public/ponk/perks/{address}` | nothing |
| `leaderboard(window=None)` | `GET /public/agents-leaderboard?window=24h\|7d\|30d\|all` | nothing |
| `public_agent(slug)` | `GET /public/agents/{slug}` | nothing |
| `public_agent_value_history(slug, range=None)` | `GET /public/agents/{slug}/value-history` | nothing |
| `public_points_leaderboard(limit, offset)` | `GET /public/points/leaderboard` | nothing |
| `resolve_referral(code)` | `GET /public/referral/{code}` | nothing |
| `agents_activity()` | `GET /public/agents-activity` | nothing |
| `ponkillionaires_week()` | `GET /public/ponkillionaires/week` | nothing |
| `x_feed(before=, since=, limit=, category=, ca_only=, min_followers=, account=, q=)` | `GET /public/x/feed` | nothing |
| `x_trending(window=None, limit=None)` | `GET /public/x/trending?window=1h\|24h` | nothing |
| `x_callers(days=None)` | `GET /public/x/callers` | nothing |
| `x_token(mint)` | `GET /public/x/token/{mint}` | nothing |
| `x_accounts()` | `GET /public/x/accounts` | nothing |
| `x_status()` | `GET /public/x/status` | nothing |
| `wallets_feed(before=, limit=, kind=, min_usd=, token=, wallet=)` | `GET /public/wallets/feed` | nothing |
| `wallets_defaults()` | `GET /public/wallets/defaults` | nothing |
| `wallets_status()` | `GET /public/wallets/status` | nothing |
| `wallet_activity(address, before=, limit=, kind=, min_usd=, token=)` | `GET /public/wallets/{address}/activity` | nothing |
| `whoami()` | `GET /v1/me` | `read` |
| `list_agents()` | `GET /v1/agents` | `read` |
| `get_agent(id)` | `GET /v1/agents/{id}` | `read` |
| `agent_performance(id)` | `GET /v1/agents/{id}/performance` | `read` |
| `agent_position(id)` | `GET /v1/agents/{id}/position` | `read` |
| `agent_value_history(id, range=None)` | `GET /v1/agents/{id}/value-history` | `read` |
| `agent_decisions(id, hours=None, limit=None)` | `GET /v1/agents/{id}/decisions`, raw JSON | `read` |
| `agent_wallet(id)` | `GET /v1/agents/{id}/wallet` | `read` |
| `agent_mandate(id)` | `GET /v1/agents/{id}/mandate` | `read` |
| `mandates()` | `GET /v1/agents/mandates` | `read` |
| `list_positions()` | `GET /v1/positions` | `read` |
| `list_logs(limit)` | `GET /v1/logs` (limit 1 to 500, default 100) | `read` |
| `get_public_link(id)` | `GET /v1/agents/{id}/public-link` | `read` |
| `list_follows()` | `GET /v1/follows` | `read` |
| `points()` | `GET /v1/points` | `read` |
| `points_leaderboard(limit, offset)` | `GET /v1/points/leaderboard` | `read` |
| `referral()` | `GET /v1/referral` | `read` |
| `creator_dashboard(days=None)` | `GET /v1/referral/creator` | `read` |
| `check_handle(handle)` | `GET /v1/referral/handle/check` | `read` |
| `get_json(path, params)` | any `GET`, raw JSON | depends on the route |
| `create_agent(...)` | `POST /v1/agents` | `trade` |
| `copy_agent(source, name=, wallet_address=, dry_run=)` | `public_agent` then `POST /v1/agents` from its `copy` block | `trade` |
| `optimize_draft(draft)` | `POST /v1/agents/optimize-draft`, raw JSON | `trade` |
| `pause_agent(id)` | `POST /v1/agents/{id}/pause` | `trade` |
| `resume_agent(id)` | `POST /v1/agents/{id}/resume` | `trade` |
| `set_autonomous(id, enabled)` | `POST /v1/agents/{id}/autonomous` | `trade` |
| `set_dry_run(id, dry_run)` | `POST /v1/agents/{id}/dry-run` | `trade` |
| `set_mode(id, mode)` | `POST /v1/agents/{id}/mode` | `trade` |
| `update_config(id, patch)` | `PATCH /v1/agents/{id}/config` | `trade` |
| `update_exits(id, tp_sl=None, automation=None)` | `POST /v1/agents/{id}/exits` | `trade` |
| `set_auto_compound(id, auto_compound, min_claimable_base_units=None, min_claimable_usd=None)` | `POST /v1/agents/{id}/auto-compound` | `trade` |
| `share_agent(id, hide_usd=False, show_onchain=None)` | `POST /v1/agents/{id}/public-link` | `trade` |
| `unshare_agent(id)` | `DELETE /v1/agents/{id}/public-link` | `trade` |
| `follow(slug)` / `unfollow(slug)` | `POST` / `DELETE /v1/follows/{slug}` | `trade` |
| `claim_handle(handle)` | `POST /v1/referral/handle` | `trade` |
| `compound(id)` | `POST /v1/agents/{id}/compound` | `trade` |
| `withdraw(id, lamports)` | `POST /v1/agents/{id}/withdraw` | `trade` |
| `exit_agent(id)` | `POST /v1/agents/{id}/exit` | `trade` |
| `create_agents(agents, create_wallets=True)` | `POST /v1/agents/batch` | `trade` |
| `fund_prepare(transfers)` | `POST /v1/agents/fund/prepare`, unsigned transactions | `trade` |
| `exit_agents(agent_ids, all_agents=False)` | `POST /v1/agents/batch/exit` | `trade` |
| `withdraw_agents(agent_ids, all_agents=False)` | `POST /v1/agents/batch/withdraw` | `trade` |
| `get_batch(batch_id)` | `GET /v1/agents/batch/{batch_id}` | `read` |
| `fleet(offset=None, limit=None)` / `iter_fleet(limit)` / `fleet_all(limit)` | `GET /v1/agents/fleet` | `read` |
| `create_agent_wallet(id)` | `POST /v1/agents/{id}/wallet` | `trade` |
| `rebalance(id, new_lower_bin_id=, new_upper_bin_id=, strategy=, max_slippage_bps=None)` | `POST /v1/agents/{id}/rebalance` | `trade` |
| `collect_fees(id, destination=None)` | `POST /v1/agents/{id}/collect-fees` | `trade` |
| `rename_agent(id, name)` | `PATCH /v1/agents/{id}` | `trade` |
| `delete_agent(id)` | `DELETE /v1/agents/{id}` | `trade` |
| `agent_proof(id, hours=None)` | `GET /v1/agents/{id}/proof` | `read` |
| `agent_range_history(id, range=None)` | `GET /v1/agents/{id}/range-history` | `read` |
| `custody_withdraw(lamports=None)` | `POST /v1/custody/withdraw` | `trade` |
| `custody_exit()` | `POST /v1/custody/exit` | `trade` |
| `portfolio()` | `GET /v1/portfolio` | `read` |
| `portfolio_history(range=None)` | `GET /v1/portfolio/history` | `read` |
| `list_transactions(limit)` | `GET /v1/transactions` | `read` |
| `list_risk_events(limit)` | `GET /v1/risk-events` | `read` |
| `list_fees(limit)` / `fee_summary()` | `GET /v1/fees`, `GET /v1/fees/summary` | `read` |
| `list_notifications(status, limit)` / `unread_notifications()` | `GET /v1/notifications`, `/unread-count` | `read` |
| `mark_all_notifications_read()` / `mark_notification_read(id)` / `dismiss_notification(id)` | `POST /v1/notifications/read-all`, `/{id}/read`, `/{id}/dismiss` | `trade` |
| `list_recommendations(status, limit)` / `get_recommendation(id)` | `GET /v1/recommendations`, `/{id}` | `read` |
| `review_recommendation` / `approve_recommendation` / `dismiss_recommendation` / `snooze_recommendation(id, minutes)` | `POST /v1/recommendations/{id}/{review,approve,dismiss,snooze}` | `trade` |
| `notify_settings()` / `update_notify_settings(...)` | `GET` / `PUT /v1/notify-settings` | `read` / `trade` |
| `list_presets()` / `create_preset` / `update_preset` / `delete_preset` | `GET` / `POST /v1/presets`, `PUT` / `DELETE /v1/presets/{id}` | `read` / `trade` |

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

Autonomous agents charge a performance fee on realized yield, and a one-time
activation fee of 1% of the capital of an agent's first open (0.5% for a wallet
holding 1,000,000 $PONK), paid inside the transaction that opens it: a failed
open pays nothing, and recenters, re-opens, compounds and top-ups never pay it
again. A referrer earns 0.25% of that capital out of the fee. There is no
charge for a key, for a call, for depositing or for withdrawing.

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

The activation rate is on the same perks read, and `ponk.ACTIVATION_FEE_BPS`,
`ACTIVATION_FEE_HOLDER_BPS` and `ACTIVATION_REFERRER_BPS` name the published
rates. A fee event charged on an activation carries `source == "activation"`:

```python
print("activation", perks.activation_fee.effective_percent, "%")
for e in ponk.list_fees():
    if e.source == "activation":
        print(e.fee_token_mint, e.platform_fee_amount)
```

## People: profiles, follows and the Following feed

Every claimed @handle and every /r/ code opens a public profile. Follow a
person and their agent, LP, call and (when they share them) swap activity
lands in your Following feed, with a Copy action on each row. Every account
is public.

```python
p = ponk.profile("@kader")                     # no key needed
print(p.label, p.social)
for pos in ponk.person_positions("kader").positions:
    print(pos.venue, pos.pool, pos.pnl_pct, pos.copy.mode)

ponk.follow_person("kader")                    # trade key
for row in ponk.following_feed(kinds="agents").rows:
    print(row.actor.name, row.verb, row.pool, row.size_usd, row.copy.mode)

for r in ponk.people_board(window="7d").rows:  # the people board, no key
    print(r.rank, r.person.name, r.pnl_usd, r.roi_pct, r.vs_hodl_pct)

page = ponk.people_search("ch")                # @handle, /r/ code or name, from
while page.people:                             # the first letter, 20 a page
    for hit in page.people:
        print(hit.display_name, hit.handle, hit.followers)
    if not page.next_cursor:
        break
    page = ponk.people_search("ch", cursor=page.next_cursor)

for r in ponk.recommended_people(limit=5).people:   # who to follow, no key
    print(r.name, r.reason)
for r in ponk.similar_people("kader").people:       # traders like this one
    print(r.name, r.reason)
for r in ponk.my_recommended().people:              # for your account
    print(r.name, r.reasons)
ponk.dismiss_recommended(r.key)                     # never suggest again
```

## Privacy

Every Ponk account is public since 0.11.0's server release (2026-10-29).
`privacy`, `set_privacy` and `answer_privacy_prompt` still answer, with the
public state, so older code keeps running; writes are accepted and ignored.

## Wallet Tracker P&L, tracked LPs, X caller cards and Top calls

```python
pnl = ponk.wallet_pnl(WALLET)                  # trading P&L per token, no key
for row in ponk.tracked_lps().rows:            # curated wallets' open LPs
    print(row.wallet["label"], row.position["pool_address"], row.position["pnl_usd"])
card = ponk.x_account("solana", days=30)
print(card.hit_rate, card.x2_rate, card.first_calls)
print(ponk.x_feed(first_calls=True).raw_json["items"][:1])
for call in ponk.top_calls(days=7, source="all").calls:
    print(call)
```

## The copilot (unreleased)

One sentence to a fleet plan, read by ponk's own closed grammar (no language
model anywhere). The copilot routes take a WALLET SESSION ONLY and refuse an
API key, so these methods need `session_token=` and never send `api_key`:

```python
ponk = PonkClient(session_token="...")       # from POST /auth/nonce + /auth/verify
plan = ponk.copilot_plan("put 2 sol into 4 agents on sol pools, stop all if down 10%")
plan = ponk.copilot_wait(plan.plan_id)          # builds, then replays 30 days
totals = plan.replay.result["totals"]
print(totals["net_pnl_usd"], totals["hold_pnl_usd"], totals["net_vs_hold_usd"])
if plan.can_start:
    start = ponk.copilot_start(plan.plan_id)    # agents in dry run, UNSIGNED funding
    print(start.sign_url)                       # your wallet signs there; nothing moves before
fleet = ponk.copilot_fleet(start.fleet_id)
ponk.copilot_stop_fleet(start.fleet_id)         # managed exit of this fleet's agents only
```

`copilot_parse`, `copilot_plan`, `copilot_get_plan`, `copilot_replay`,
`copilot_wait`, `copilot_start`, `copilot_fleets`, `copilot_fleet`,
`copilot_stop_fleet` and `copilot_tighten_stop` (tighter only).

Amounts are SOL, exactly as typed: "$500", "half my sol" or "20% of my wallet"
are refused, never converted. A fleet is 1 to 8 agents on Meteora DLMM pools
only. Name a token by symbol ("jup pools") or by its mint; a symbol is
resolved to a mint. An ambiguous or unverified symbol is refused with
`refusal["suggestions"]`, the tokens carrying it with their mints, most liquid
first, so you can put the one you mean into the sentence. `plan.can_start` turns true once the replay is done, and
`plan.plan["totals"]["ponk_fee_bps"]` is the Ponk fee on fees earned for this
wallet. After a start, `copilot_fleet(...).funding` lists what is left to fund
or approve with a fresh `sign_url`, and `.exit` names any agent whose exit
failed; `copilot_stop_fleet` on a stopping fleet retries only those. The replay is
one past window beside holding, not a forecast.

## Changes in 0.11.0

* **People search.** `people_search(q, cursor=None)` (`PeopleSearch`,
  `SearchHit`), no key.
* **Who to follow.** `recommended_people`, `similar_people` (no key),
  `my_recommended`, `dismiss_recommended` (`Recommendations`,
  `RecommendedPerson`).
* **Mandates.** `mandates()` (`Mandates`, `MandateRow`) with `renew_all_url`.
* **Pool checks are advice.** `create_agent(..., accept_pool_risk=True)`.
* **Copilot.** Leg `warnings` and refusal `suggestions` for new tokens.
* Every account is public: the privacy methods answer the public state.

## Changes in 0.10.0

* **People.** `profile`, `followers`, `following`, `person_activity`,
  `person_positions`, `people_board` and `live_feed` (no key);
  `follow_person`, `unfollow_person`, `my_following` and `following_feed`
  (API key). Models `Person`, `Profile`, `PeopleList`, `FollowState`,
  `MyFollowing`, `SocialFeed`, `ActivityRow`, `CopyAction`, `OpenPosition`,
  `PersonPositions`, `PeopleBoard`.
* **Privacy.** `privacy`, `set_privacy` and `answer_privacy_prompt` over
  `/v1/me/privacy` (`Privacy`, `PrivacySettings`); the leaderboard-privacy
  names stay as aliases.
* **Activation fee.** `PonkPerks.activation_fee`, `FeeEvent.source`
  (`activation`), and the constants `ACTIVATION_FEE_BPS`,
  `ACTIVATION_FEE_HOLDER_BPS` and `ACTIVATION_REFERRER_BPS`.
* **Trackers and calls.** `wallet_pnl`, `tracked_lps`, `x_account`,
  `top_calls`, and `x_feed(first_calls=True)`.

## Changes in 0.7.0

* **Swap orders.** `plan_orders([limit_order(...), tp_sl_order(...),
  dca_order(...)])` validates up to ten orders and returns one link
  (`plan.sign_url`) where your wallet signs them. For a program that holds its
  own key: `orders_sign_in(sign_message)`, then `place_order(token, family,
  body, sign_transaction)`, or the single steps `craft_order_deposit`,
  `create_price_order`, `create_dca_order`, `cancel_order`,
  `confirm_order_cancel`, `list_orders`, `cancel_orders(token,
  all_orders=True)` and `confirm_order_cancels`. The Jupiter token is a
  per-call argument (`X-Jupiter-Session`), never stored. A 504
  `outcome_unknown` on a create or confirm is never retried.
* **Copy and follow.** `copy_source`, `follow_status`, `follow_leader`,
  `update_follow`, `unfollow_leader`, `answer_follow_offer`,
  `list_copy_follows`, `follow_leaders` and `follow_many` (batches), and Copy
  LP: `wallet_follow`, `unfollow_wallet`, `answer_wallet_offer`, `copy_log`,
  `copy_referral`, `attach_copy_referral`.
* **Self-custody loop.** Every prepare returns a `Prepared` (an UNSIGNED
  build for your own wallet) and every mark a `MarkedTx`: the agent
  rebalance (prepare, prepare-split, prepare-open, mark), the fee claim
  (prepare, mark, confirm), the position builders, `lp_withdraw_prepare`
  (Ponk Clouds included, withdraw only), wSOL reclaim, stranded positions and
  cleanup, `positions_sync`, the Clouds referral claim.
* **Account.** Calls, Web Push, card looks, referral codes, the Creator
  application, leaderboard privacy and per-agent visibility, `clear_logs`,
  `weekly_recap`, `my_ponkillionaires`, `telegram_status`,
  `share_position_telegram`, and `rank_presets`.

Still app only: minting a custody mandate, API keys, webhooks and linking
Telegram.

## Changes in 0.6.0

* **Agent fleets.** `create_agents([...])` creates many agents in one call,
  each with its own wallet. `fund_prepare(agent_ids)` returns the UNSIGNED
  transfers that fund them all from your wallet, packed into as few
  transactions as fit: ten agents, one signature. `iter_fleet()` reads every
  agent with its wallet balance, mandate expiry and value, page by page.
  `exit_agents(ids)` and `withdraw_agents(ids)` (or `all_agents=True`) close
  them all at once. A batch keeps running on the server after the response;
  `get_batch(batch_id)` collects what was still `running`.
* **The client never signs.** Your wallet signs the funding transfers, on
  ponk.exchange (`plan.sign_urls`, which also queues each custody mandate an
  agent still needs) or in wallet software you control
  (`plan.transactions[i].unsigned_tx_b64`). The SDK holds no key and asks for
  none.
* **`/v1` parity with the app:** `rebalance`, `collect_fees`, `rename_agent`,
  `delete_agent`, `create_agent_wallet`, `agent_proof`, `agent_range_history`,
  `custody_withdraw`, `custody_exit`, `portfolio`, `portfolio_history`,
  `list_transactions`, `list_risk_events`, `list_fees`, `fee_summary`, the
  notifications, the recommendations, `notify_settings` and the saved presets.
* `AgentWallet` gained `withdrawable_lamports` and `token_accounts_with_value`
  (`None` means the token read failed, never zero tokens).

```python
batch = ponk.create_agents([
    {"name": "fleet %d" % i, "dex": "meteora_dlmm", "pool_address": POOL,
     "strategy": "bin_rebalancer", "dry_run": True,
     "config": {"kind": "bin_rebalancer", "bin_range_width": 20, "rebalance_threshold_bins": 5}}
    for i in range(10)
])
ids = [it.agent_id for it in batch.items if it.ok]
plan = ponk.fund_prepare(ids)            # unsigned; sign at plan.sign_urls
for row in ponk.iter_fleet():
    print(row.agent.name, row.wallet and row.wallet.lamports, row.mandate and row.mandate.status)
ponk.exit_agents(ids)
```

The whole flow, with polling, is [`examples/fleet.py`](examples/fleet.py).

Still app only: minting a custody mandate, API keys and webhooks. A key
cannot produce a wallet signature, so these stay behind a wallet sign-in.

## Changes in 0.5.0

* **Live agents activity, no key:** `agents_activity()` returns the board
  agents' confirmed actions of the last 24 hours and every live hold, with no
  identity on any event.
* **Weekly Ponkillionaires, no key:** `ponkillionaires_week()` returns this
  week's standings, the prizes (next week's agent fee rate plus points) and
  last week's winners.
* **X Tracker, no key:** `x_feed`, `x_trending`, `x_callers`, `x_token`,
  `x_accounts`, `x_status`.
* **Wallet Tracker, no key:** `wallets_feed`, `wallets_defaults`,
  `wallets_status`, and `wallet_activity(address)` for any wallet.
* **Referral tiers:** `referral()` now carries `tier` (`current`,
  `share_bps`, `referred_fees_usd`, `window_days`, `next`) and `programme`
  (`ladder`, `window_days`). One level only: 20%, 25% from $1,000 and 30% from
  $5,000 of referred platform fees over 30 days.
* The tracker and weekly race responses are camelCase on the wire; the models
  expose snake_case attributes (`accounts_1h` reads `accounts1h`).
* `PublicLink.path` is documented as `/ponkillionaires/<slug>`, the path the
  server sends.

Not in the client, by design: following X accounts, tracked wallets, live copy
follow, mandate reminder settings and the PnL and referral card settings are
app session routes (`/me/*`, `/agents/{id}/follow`) that need a wallet
sign-in, not an API key.

```python
ponk = PonkClient()
hot = ponk.x_trending(window="24h", limit=10)
week = ponk.ponkillionaires_week()
tier = PonkClient(api_key=KEY).referral().tier
print(tier.current, tier.share_bps / 100, "%", tier.next.min_fees_usd if tier.next else "top")
```

## Changes in 0.4.0

* **Top Ponkillionaires, no key:** `leaderboard(window)`, `public_agent(slug)`,
  `public_agent_value_history(slug, range)`.
* **Copy an agent:** `copy_agent(slug_or_row, name=, wallet_address=,
  dry_run=, config_overrides=None)` creates an agent on your wallet from a
  shared agent's `copy` block.
* **Share and follow:** `share_agent`, `unshare_agent`, `get_public_link`,
  `follow`, `unfollow`, `list_follows`.
* **Points and referrals:** `points`, `points_leaderboard`,
  `public_points_leaderboard`, `referral`, `creator_dashboard`,
  `check_handle`, `claim_handle`, `resolve_referral`.
* **Agent reads:** `agent_value_history` and `agent_decisions`.
* **Settings edits:** `update_config`, `update_exits`, `set_auto_compound`,
  and `optimize_draft`. The docstrings list the new keys: keep side, reshape,
  cooldown and daily cap, the market cap, value and value plus fees take
  profits, and the Degen Mode bin step filter.

```python
board = ponk.leaderboard(window="7d")
row = board.ranked[0]
mine = ponk.copy_agent(row, name="copy", wallet_address=me.wallet_address, dry_run=True)

standing = ponk.points()
print(standing.season.name if standing.season else "off-season", standing.total_points, standing.rank)
r = ponk.referral()
print("earned", r.total_earned_usd, "owed", r.owed_usd, "unproven", r.unproven_usd)  # never add these
```

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
