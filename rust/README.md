# ponk (Rust)

A thin client for the ponk public API: create and drive Solana DLMM liquidity
agents, read their live positions, PnL and fees, and move funds home.

One method per endpoint, typed results, four dependencies (plus two for webhook
verification, behind a default feature), no code generation. It never computes
a number the server did not send.

## Add it

The crate is not on crates.io. Depend on it from git, or by path from a local
clone:

```toml
[dependencies]
ponk = { git = "https://github.com/ponkexchange/ponk-sdk" }
# or, from a local clone:
# ponk = { path = "../ponk-sdk/rust" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

It is its own workspace, so it builds on its own and pulls nothing from the
ponk server workspace. TLS is a feature: `rustls-tls` by default, or

```toml
ponk = { git = "https://github.com/ponkexchange/ponk-sdk", default-features = false, features = ["native-tls", "webhooks"] }
```

to use the platform's own TLS instead. Leave `webhooks` out of that list if
you never receive webhooks, and `hmac` and `sha2` drop out of the build.

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

```rust
use ponk::{Client, Error, DEFAULT_BASE_URL};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let ponk = Client::new(DEFAULT_BASE_URL, std::env::var("PONK_API_KEY").unwrap())?;

    let me = ponk.whoami().await?;
    println!("acting as {:?} with scope {:?}", me.wallet_address, me.scope);

    for agent in ponk.list_agents().await? {
        let id = agent.id.clone().unwrap_or_default();
        let perf = ponk.agent_performance(&id).await?;

        // Every USD field is a string or None. None means "could not be
        // priced", never zero, so print a dash rather than a number you do
        // not have.
        println!(
            "{:?}  value {}  pnl {}",
            agent.name,
            perf.current_value_usd.as_deref().unwrap_or("-"),
            perf.pnl_usd.as_deref().unwrap_or("-"),
        );
    }
    Ok(())
}
```

The same thing runs as an example:

```bash
PONK_API_KEY=ponk_live_... cargo run --example quickstart
```

Create an agent, watch it think, then let it trade:

```rust
use ponk::NewAgent;

let agent = ponk
    .create_agent(
        &NewAgent::new(
            "sol-usdc runner",
            me.wallet_address.clone().unwrap(),  // must be the key's own wallet
            "meteora_dlmm",                      // meteora_dlmm | orca | ponk_clouds | raydium
            "bin_rebalancer",
            serde_json::json!({
                "kind": "bin_rebalancer",
                "bin_range_width": 20,
                "rebalance_threshold_bins": 5
            }),
        )
        .pool_address("POOL_ADDRESS")
        .dry_run(true),                          // left unset, /v1 creates a LIVE agent
    )
    .await?;

// A dry-run agent runs its whole strategy loop and logs every decision to
// list_logs without sending a transaction. Read those logs before going live.
ponk.set_dry_run(agent.id.as_deref().unwrap(), false).await?;
```

Move funds home:

```rust
let wallet = ponk.agent_wallet(&id).await?;
println!("{:?} holds {:?} lamports", wallet.pubkey, wallet.lamports);

ponk.withdraw(&id, Some(500_000_000)).await?;  // 0.5 SOL, or None to sweep
let result = ponk.exit(&id).await?;            // stop, close, sweep everything
println!("swept to {:?} sig {:?}", result.destination, result.signature);
```

## What a key cannot do

Three limits are structural. They are properties of how custody works in ponk,
not settings that can be turned off.

* **It cannot sign with your connected wallet.** Non-custodial actions still
  need your signature in the app. This crate can read their state, not produce
  the signature.
* **It cannot mint a custody mandate, and it cannot revoke one.** The mandate
  is a wallet signature over an exact text, produced in the app with your
  wallet connected. What a `trade` key can do is use a mandate that already
  exists: `set_autonomous(id, true)` turns autonomous signing on for an agent
  whose own agent wallet holds an active, unexpired mandate, and
  `set_autonomous(id, false)` turns it off at any time. Turning it off does
  not revoke the mandate, so a later `true` can re-arm the agent until the
  mandate expires. So: create the agent here if you like, sign its mandate
  once in the app, and from then on this crate can drive it.
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
| `range_cost(dex, address, total_bins)` | `GET /pools/{dex}/{address}/range-cost?bins=N` | nothing |
| `ponk_perks(wallet)` | `GET /public/ponk/perks/{address}` | nothing |
| `leaderboard(window)` | `GET /public/agents-leaderboard?window=` | nothing |
| `public_agent(slug)` | `GET /public/agents/{slug}` | nothing |
| `public_agent_value_history(slug, range)` | `GET /public/agents/{slug}/value-history` | nothing |
| `public_points_leaderboard(limit, offset)` | `GET /public/points/leaderboard` | nothing |
| `resolve_referral(code)` | `GET /public/referral/{code}` | nothing |
| `agents_activity()` | `GET /public/agents-activity` | nothing |
| `ponkillionaires_week()` | `GET /public/ponkillionaires/week` | nothing |
| `x_feed(&XFeedQuery)` | `GET /public/x/feed` | nothing |
| `x_trending(window, limit)` | `GET /public/x/trending?window=1h\|24h` | nothing |
| `x_callers(days)` | `GET /public/x/callers` | nothing |
| `x_token(mint)` | `GET /public/x/token/{mint}` | nothing |
| `x_accounts()` | `GET /public/x/accounts` | nothing |
| `x_status()` | `GET /public/x/status` | nothing |
| `wallets_feed(&WalletFeedQuery)` | `GET /public/wallets/feed` | nothing |
| `wallets_defaults()` | `GET /public/wallets/defaults` | nothing |
| `wallets_status()` | `GET /public/wallets/status` | nothing |
| `wallet_activity(address, &WalletFeedQuery)` | `GET /public/wallets/{address}/activity` | nothing |
| `whoami()` | `GET /v1/me` | `read` |
| `list_agents()` | `GET /v1/agents` | `read` |
| `get_agent(id)` | `GET /v1/agents/{id}` | `read` |
| `agent_performance(id)` | `GET /v1/agents/{id}/performance` | `read` |
| `agent_position(id)` | `GET /v1/agents/{id}/position` | `read` |
| `agent_value_history(id, range)` | `GET /v1/agents/{id}/value-history` | `read` |
| `agent_decisions(id, hours, limit)` | `GET /v1/agents/{id}/decisions`, raw JSON | `read` |
| `agent_wallet(id)` | `GET /v1/agents/{id}/wallet` | `read` |
| `agent_mandate(id)` | `GET /v1/agents/{id}/mandate` | `read` |
| `list_positions()` | `GET /v1/positions` | `read` |
| `list_logs(limit)` | `GET /v1/logs` | `read` |
| `get_public_link(id)` | `GET /v1/agents/{id}/public-link` | `read` |
| `list_follows()` | `GET /v1/follows` | `read` |
| `points()` | `GET /v1/points` | `read` |
| `points_leaderboard(limit, offset)` | `GET /v1/points/leaderboard` | `read` |
| `referral()` | `GET /v1/referral` | `read` |
| `creator_dashboard(days)` | `GET /v1/referral/creator` | `read` |
| `check_handle(handle)` | `GET /v1/referral/handle/check` | `read` |
| `get_json(path)` | any GET | `read` |
| `create_agent(&NewAgent)` | `POST /v1/agents` (copy an agent with `row.copy.to_new_agent(name, wallet)`) | `trade` |
| `optimize_draft(&draft)` | `POST /v1/agents/optimize-draft`, raw JSON | `trade` |
| `set_autonomous(id, bool)` | `POST /v1/agents/{id}/autonomous` | `trade` |
| `pause_agent(id)` | `POST /v1/agents/{id}/pause` | `trade` |
| `resume_agent(id)` | `POST /v1/agents/{id}/resume` | `trade` |
| `set_dry_run(id, bool)` | `POST /v1/agents/{id}/dry-run` | `trade` |
| `set_mode(id, mode)` | `POST /v1/agents/{id}/mode` | `trade` |
| `update_config(id, &patch)` | `PATCH /v1/agents/{id}/config` | `trade` |
| `update_exits(id, tp_sl, automation)` | `POST /v1/agents/{id}/exits` | `trade` |
| `set_auto_compound(id, on, min_base_units, min_usd)` | `POST /v1/agents/{id}/auto-compound` | `trade` |
| `share_agent(id, hide_usd, show_onchain)` | `POST /v1/agents/{id}/public-link` | `trade` |
| `unshare_agent(id)` | `DELETE /v1/agents/{id}/public-link` | `trade` |
| `follow(slug)` / `unfollow(slug)` | `POST` / `DELETE /v1/follows/{slug}` | `trade` |
| `claim_handle(handle)` | `POST /v1/referral/handle` | `trade` |
| `compound(id)` | `POST /v1/agents/{id}/compound` | `trade` |
| `withdraw(id, lamports)` | `POST /v1/agents/{id}/withdraw` | `trade` |
| `exit(id)` | `POST /v1/agents/{id}/exit` | `trade` |
| `create_agents(&[NewAgent], create_wallets)` | `POST /v1/agents/batch` | `trade` |
| `fund_prepare(&[FundRequest])` | `POST /v1/agents/fund/prepare`, unsigned transactions | `trade` |
| `exit_agents(&ids)` / `exit_all_agents()` | `POST /v1/agents/batch/exit` | `trade` |
| `withdraw_agents(&ids)` / `withdraw_all_agents()` | `POST /v1/agents/batch/withdraw` | `trade` |
| `get_batch(batch_id)` | `GET /v1/agents/batch/{batch_id}` | `read` |
| `fleet(offset, limit)` / `fleet_all(limit)` | `GET /v1/agents/fleet` | `read` |
| `create_agent_wallet(id)` | `POST /v1/agents/{id}/wallet` | `trade` |
| `rebalance(id, &RebalanceRequest)` | `POST /v1/agents/{id}/rebalance` | `trade` |
| `collect_fees(id, destination)` | `POST /v1/agents/{id}/collect-fees` | `trade` |
| `rename_agent(id, name)` / `delete_agent(id)` | `PATCH` / `DELETE /v1/agents/{id}` | `trade` |
| `agent_proof(id, hours)` / `agent_range_history(id, range)` | `GET /v1/agents/{id}/proof`, `/range-history` | `read` |
| `custody_withdraw(lamports)` / `custody_exit()` | `POST /v1/custody/withdraw`, `/exit` | `trade` |
| `portfolio()` / `portfolio_history(range)` | `GET /v1/portfolio`, `/history` | `read` |
| `list_transactions(limit)` / `list_risk_events(limit)` | `GET /v1/transactions`, `/v1/risk-events` | `read` |
| `list_fees(limit)` / `fee_summary()` | `GET /v1/fees`, `/v1/fees/summary` | `read` |
| `list_notifications(status, limit)` / `unread_notifications()` | `GET /v1/notifications`, `/unread-count` | `read` |
| `mark_all_notifications_read()` / `mark_notification_read(id)` / `dismiss_notification(id)` | `POST /v1/notifications/...` | `trade` |
| `list_recommendations(status, limit)` / `get_recommendation(id)` | `GET /v1/recommendations` | `read` |
| `review_` / `approve_` / `dismiss_recommendation(id)` / `snooze_recommendation(id, minutes)` | `POST /v1/recommendations/{id}/...` | `trade` |
| `notify_settings()` / `update_notify_settings(&NotifySettings)` | `GET` / `PUT /v1/notify-settings` | `read` / `trade` |
| `list_presets()` / `create_preset(&NewPreset)` / `update_preset(id, &NewPreset)` / `delete_preset(id)` | `/v1/presets` | `read` / `trade` |
| `plan_orders(&[OrderIntent], &[OrderRef], cancel_all)` | `POST /v1/orders/plan`, one sign link | `read` |
| `orders_challenge(kind)` / `orders_verify_message(sig)` / `orders_verify_transaction(tx)` / `orders_refresh(t)` / `orders_logout(t)` | `POST /v1/orders/auth/*` | `read` |
| `order_mint_support(&mints)` | `GET /v1/orders/mint-support` | `read` |
| `craft_order_deposit(session, in, out, amount, type, sub)` | `POST /v1/orders/deposit/craft`, unsigned | `trade` |
| `create_price_order(session, &body)` / `create_dca_order(session, &body)` | `POST /v1/orders/price`, `/dca` | `trade` |
| `cancel_order(session, family, id)` / `confirm_order_cancel(session, family, id, tx, request_id)` | `POST /v1/orders/{family}/{id}/cancel`, `/confirm-cancel` | `trade` |
| `cancel_orders(session, &refs, all)` / `confirm_order_cancels(session, &items)` | `POST /v1/orders/cancel`, `/confirm-cancel` | `trade` |
| `list_orders(session, family, state, limit)` | `GET /v1/orders` | `read` |
| `copy_source` / `follow_leader` / `update_follow` / `unfollow_leader` / `answer_follow_offer` | `/v1/agents/{id}/copy-source`, `/follow`, `/follow/offer` | `trade` |
| `follow_status(id)` / `list_copy_follows()` / `copy_log(id)` | `GET /v1/agents/{id}/follow`, `/v1/copy-follows`, `/copy-log` | `read` |
| `follow_leaders(&ids, auto, unfollow)` / `follow_many(&slugs, unfollow)` | `POST /v1/agents/follow/batch`, `/v1/follows/batch` | `trade` |
| `wallet_follow(id, &WalletFollowRequest)` / `unfollow_wallet(id)` / `answer_wallet_offer(id, accept)` | `/v1/agents/{id}/wallet-follow` | `trade` |
| `copy_referral(key)` / `attach_copy_referral(key)` | `GET` / `POST /v1/copy-referral/{key}` | `read` / `trade` |
| `agent_rebalance_prepare` / `_prepare_split` / `_prepare_open` / `agent_rebalance_mark` | `POST /v1/agents/{id}/rebalance/*`, unsigned | `trade` |
| `agent_fees_prepare` / `agent_fees_mark` / `agent_fees_confirm` | `POST /v1/agents/{id}/collect-fees/*` | `trade` |
| `position_open_prepare` / `position_mark_tx` / `position_rebalance_split` / `position_rebalance_open` / `position_rebalance_tick` / `position_compound_prepare` / `position_claim_prepare` | `POST /v1/positions/...`, unsigned | `trade` |
| `lp_withdraw_prepare(dex, pool, position, bps)` | `POST /v1/lp/{dex}/withdraw/prepare` (incl. `ponk_clouds`) | `trade` |
| `reclaim_wsol_prepare()` / `stranded_positions()` / `cleanup_position_prepare(pos)` | `/v1/wallet/...` | `trade` / `read` |
| `positions_sync(&filter)` / `share_position_telegram(&card)` | `POST /v1/positions/sync`, `/share-telegram` | `trade` |
| `clouds_referrals()` / `clouds_referral_claim_prepare(pool)` | `/v1/clouds/referrals` | `read` / `trade` |
| `call_preview(mint)` / `post_call(mint, text)` / `delete_call(id)` | `/v1/calls` | `read` / `trade` |
| `push_settings` / `set_push_prefs` / `push_subscribe` / `push_unsubscribe` / `push_test` | `/v1/push` | `read` / `trade` |
| `pnl_card` / `set_pnl_card` / `referral_card` / `set_referral_card` | `/v1/pnl-card`, `/v1/referral-card` | `read` / `trade` |
| `set_referral_code` / `add_referral_code` / `apply_creator` | `POST /v1/referral/code`, `/codes`, `/creator/apply` | `trade` |
| `leaderboard_privacy` / `set_leaderboard_privacy(&PrivacyRequest)` / `agent_visibility` / `set_agent_visibility` | `/v1/leaderboard-privacy`, `/v1/agents/{id}/leaderboard-visibility` | `read` / `trade` |
| `clear_logs()` | `DELETE /v1/logs` | `trade` |
| `weekly_recap()` / `my_ponkillionaires()` / `telegram_status()` | `GET /v1/recap/weekly`, `/v1/ponkillionaires`, `/v1/telegram` | `read` |
| `rank_presets(&RankRequest)` | `POST /v1/presets/rank` | `read` |

The order calls take the caller's Jupiter access token as `session` and send
it as `X-Jupiter-Session` on that one request (`JUPITER_SESSION_HEADER`); the
client never keeps it. Create and confirm take the fund timeout, and a 504
`outcome_unknown` there means the transaction may have landed: read
`list_orders` before anything else, never retry blindly. Every prepare returns
an UNSIGNED `Prepared` for your own wallet; this crate never signs.

`fund_prepare` returns UNSIGNED transactions and this crate never signs them:
open each of `plan.sign_urls` on ponk.exchange with your wallet connected, or
sign `unsigned_tx_b64` in wallet software you control. A batch keeps running
on the server after its response; poll `get_batch` until `running == 0`.

The first four need no key at all: `Client::public(base_url)` reaches them and
nothing else. `pool`, `range_cost` and `ponk_perks` are rate limited per IP.
`range_cost` is Meteora DLMM only (any other venue answers 422), takes the
TOTAL bin count centred on the active bin, and clamps a width past the venue's
ceiling rather than refusing it, with `clamped` saying so. `health()` returns its report even when the API answers 503,
because which component is down is the whole point of that call. `get_json` is
the escape hatch for a route or a field added to the API after this version.

This crate covers the key-authenticated `/v1` API plus those public reads,
and webhook verification in `ponk::webhooks`. What stays behind a wallet
session, by design, is minting a custody mandate, API keys, registering
webhooks and linking Telegram.

## The mandate expires

A custody mandate carries a hard expiry, 30 days by default for the grant the
app asks you to sign. Lapsing is a clock fact with no transaction behind it:
the agent keeps ticking and every signature it asks for is denied, so it looks
exactly like an agent with nothing to do. Read it rather than assume it:

```rust
let m = ponk.agent_mandate(&id).await?;       // 404: no active grant
if m.status.as_deref() == Some("expired") {
    // Renew it in the app. Withdraw and exit keep working meanwhile.
}
println!("{:?} seconds left", m.seconds_remaining);  // negative once lapsed
```

A fund call can return `Ok` and still have done nothing: a mandate denial on
`compound` arrives as a 200 whose receipt has `signature: None` and a failed
`status`. Read the receipt, never just the `Ok`.

## Webhooks

Register an endpoint from a signed-in session with `POST /me/webhooks`
(`{"url": "https://...", "event_kinds": ["mandate_expiring", "mandate_expired"]}`,
or omit `event_kinds` for everything). Those routes take a wallet session, not
an API key, so this crate cannot register one. The create response carries the
`whsec_...` signing secret. Then verify every delivery against the RAW request
bytes:

```rust
use ponk::webhooks::verify;

// `signature` is the X-Ponk-Signature header value (ponk::webhooks::SIGNATURE_HEADER).
fn receive(raw_body: &[u8], signature: Option<&str>, secret: &str) -> u16 {
    let Some(sig) = signature else {
        return 400;
    };
    match verify(raw_body, sig, secret) {
        Ok(event) => {
            println!("{} for agent {:?}", event.event, event.agent_id);
            200                       // only a 2xx counts as delivered
        }
        Err(_) => 400,                // do not act on the body
    }
}
```

`X-Ponk-Signature` is `t=<unix>,v1=<hex HMAC-SHA256(secret, "<t>.<raw body>")>`.
`verify` checks it in constant time and refuses a timestamp more than 300
seconds from now; `verify_at` takes your own window and clock. Verify the bytes
you received, not a re-serialization of the parsed JSON, which is not
guaranteed to reproduce them. `X-Ponk-Delivery` stays the same across retries
of one delivery, so deduplicate on it. `webhooks::EVENT_KINDS` lists the 18
kinds this release knows; `verify` accepts any kind, so a newer one is still
handed to you.

## Errors

Every error carries the same envelope:

```json
{"error": {"code": "conflict", "message": "exit already running", "request_id": "..."}}
```

`Error::Api(ApiError)` holds it with the status. `ApiError::kind()` gives the
group to branch on, and `code` is kept verbatim because a 422 can carry a risk
code the server added after this crate was written.

| Kind | Status | What to do |
| --- | --- | --- |
| `BadRequest` | 400 | Fix the request. `code` is `invalid_strategy_config` when the config did not match the strategy, and `message` names the field. |
| `Unauthorized` | 401 | Missing, malformed, unknown or revoked key. The last two are deliberately indistinguishable. Do not retry. |
| `Forbidden` | 403 | The key is valid but read-only. Mint a `trade` key. Do not retry. |
| `NotFound` | 404 | No such object, or it belongs to another account. Also deliberately indistinguishable. |
| `Conflict` | 409 | Conflicts with in-flight work, for example an exit already running. Retry after a pause. |
| `Unprocessable` | 422 | Understood but cannot apply, for example compounding an agent with no open position, or `set_autonomous(id, true)` on an agent with no unexpired mandate. |
| `RateLimited` | 429 | Back off and retry. Only the public routes are rate limited; `/v1` never answers 429. |
| `Server` | 5xx | Quote `request_id` to support. |

`ApiError::is_retryable()` answers the common question directly. The other
variants are `Error::Transport` (DNS, connection, TLS, timeout),
`Error::InvalidBaseUrl` and `Error::Decode`.

```rust
use ponk::{Error, ErrorKind};

match ponk.exit(&id).await {
    Ok(result) => println!("swept to {:?}", result.destination),
    Err(e) if e.is_timeout() => {
        // A fund call that times out has NOT necessarily failed: the work
        // keeps running on the server. Exit is idempotent, so calling it
        // again continues it rather than double-sending.
        ponk.exit(&id).await?;
    }
    Err(Error::Api(api)) if api.kind() == ErrorKind::Conflict => {
        // An exit is already running. It will finish on its own.
    }
    Err(e) => return Err(e),
}
```

`compound`, `withdraw` and `exit` send several transactions and wait for each
confirmation, so they can take a minute. They use `fund_timeout` (180s), set
separately from `timeout` (30s) for everything else:

```rust
use std::time::Duration;

let ponk = ponk::Client::builder(ponk::DEFAULT_BASE_URL)
    .api_key(key)
    .timeout(Duration::from_secs(10))
    .fund_timeout(Duration::from_secs(300))
    .build()?;
```

## Reading the numbers

* **A `None` USD figure means unknown, not zero.** An unpriceable pool returns
  `null` rather than a fabricated 0. Render it as a dash. Treating it as zero
  silently understates a position.
* **USD amounts on the agent and position endpoints are strings.** They are
  strings precisely so a float cannot round them. Parse into your decimal type
  before doing arithmetic on money.
* **`claimable_fees_usd` and `fee_pnl_usd` are different numbers.** The first is
  the unclaimed fees sitting on the position right now, the second is lifetime
  fees earned, claimed plus unclaimed. Do not add them.
* **PnL decomposes as Total = Price + Fee**: `pnl_usd`, `price_pnl_usd`,
  `fee_pnl_usd`. `pnl_source` says whether the figures are Meteora's own
  (`meteora_datapi`) or ponk's on-chain valuation (`oracle_estimate`).
* **`cost_basis_estimated`** marks a position ponk did not open. The basis is
  then a first-observation stamp, so lifetime PnL stays `None` instead of
  asserting a gain that contradicts your real numbers.
* **`PoolSnapshot::current_price` is raw**, token-Y base units per token-X base
  unit. `PoolSnapshot::human_price()` applies the decimals, and returns `None`
  rather than a guess when an input is missing.

## Fees

Autonomous agents charge a performance fee on realized yield only. Principal is
never touched and no fee is taken on an unrealized gain. There is no charge for
a key, for a call, for creating an agent, for depositing or for withdrawing.

**An agent created through this API pays the API rate, not the app rate.** The
rate is fixed when the agent is created and follows that agent for its whole
life, including after you enable autonomous mode for it in the app. Read both
rates live rather than hardcoding them:

```rust
let perks = ponk.ponk_perks(&wallet).await?;
println!("app agents {:?}%", perks.managed_agent_fee.and_then(|f| f.effective_percent));
println!("api agents {:?}%", perks.api_agent_fee.and_then(|f| f.effective_percent));
```

`Agent::managed_fee_bps` is the rate that specific agent's skim actually
charges, before any $PONK holder discount.

## Examples

```bash
PONK_API_KEY=ponk_live_... cargo run --example quickstart
PONK_API_KEY=ponk_live_... cargo run --example leaderboard
```

`leaderboard` reads the Top Ponkillionaires board, prints how the top row
would be copied (it creates nothing unless `PONK_COPY=1`), and prints your
points and referral standing.

## Tests

Offline, no network, no fixtures to refresh:

```bash
cargo test --manifest-path rust/Cargo.toml
```
