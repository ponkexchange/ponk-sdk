# Changelog

Both clients carry the same version from 0.4.0 on. Python lives in
`python/ponk/_version.py`, Rust in `rust/Cargo.toml`.

## Unreleased

Added, in the Python client:

- **Exit to SOL.** `exit_agent`, `exit_agents` and `custody_exit` take an
  optional `to_sol`: `True` swaps the closed position's leftover tokens to SOL
  before the sweep (ponk fee 0.2% of the swapped amount), `False` sends them as
  they are with no swap and no fee, omitted uses the agent's own setting
  (default `True`). A withdrawal swaps nothing and takes no `to_sol`.
  `Withdrawal.exit_swaps` (`ExitSwapLeg`: swapped, kept with its reason, or
  outcome unknown with its signature) and `Withdrawal.exit_notes` report each
  token. `get_exit_to_sol` and `set_exit_to_sol` (`ExitToSol`) read and write
  the agent setting that automatic exits (stops, kills, step-outs) honour.

## 0.8.0 (2026-10-28)

- Ponk Copilot works with an API key as well as a wallet session: one sentence to a plan, a 30-day replay against holding, a signing link that funds the fleet, the live fleet read, stop and close. A read-scope key plans and reads; a trade-scope key starts and stops. Fleets started with a key pay the API fee (15%), a wallet session pays the web fee.

Added, in the Python client:

- **The copilot**, wallet session only: `PonkClient(session_token=...)` and
  `copilot_parse`, `copilot_plan`, `copilot_get_plan`, `copilot_replay`,
  `copilot_wait`, `copilot_start` (agents in dry run, unsigned funding, the
  signing page), `copilot_fleets`, `copilot_fleet`, `copilot_stop_fleet` and
  `copilot_tighten_stop`, over `/copilot/...`. These methods never send the
  API key; without a session they raise `PonkError` and send nothing.
  The grammar is SOL only (a dollar amount or a share of the wallet is
  refused, never converted), a fleet is 1 to 8 agents, a token can be named by
  its symbol (resolved to a mint, an ambiguous one refused) or its mint, and
  only Meteora DLMM pools are planned. A plan can start only once its 30 day
  replay is done. `CopilotPlan.signed_in`, `CopilotStart.funding_state` and
  `CopilotFleet.exit` / `funding` carry what the fleet still needs and which
  exits failed, by name; `copilot_stop_fleet` on a stopping fleet retries only
  the failed exits.

## 0.7.0 (2026-10-23)

Everything a user can do on ponk.exchange, now from a key. Added, in both the
Python and the Rust client:

- **Swap orders** (Jupiter Trigger V2 limit, trailing, take-profit and
  stop-loss brackets, and Jupiter Recurring DCA, time based or inside a price
  or market cap band). Two doors, neither signs:
  - `plan_orders` (`POST /v1/orders/plan`): validate up to ten orders and any
    cancels and get ONE link on ponk.exchange where your wallet signs them
    all. Python helpers `limit_order`, `tp_sl_order`, `dca_order` (Rust:
    `OrderIntent::limit`, `::trailing`, `::dca`) build the intents.
  - Self-sign, for a program that holds its own wallet key:
    `orders_challenge` / `orders_verify` (Rust: `orders_verify_message`,
    `orders_verify_transaction`), `orders_refresh`, `orders_logout`,
    `order_mint_support`, `craft_order_deposit` (an UNSIGNED deposit),
    `create_price_order`, `create_dca_order`, `cancel_order` (an UNSIGNED
    withdrawal), `confirm_order_cancel`, `list_orders`, and the batches
    `cancel_orders` (by id or all) and `confirm_order_cancels`. Python adds
    `orders_sign_in(sign_message)` and `place_order(..., sign_transaction)`,
    which take YOUR signing callback; the SDK still holds no key.
  - The Jupiter access token is an argument of each call, sent as
    `X-Jupiter-Session`, never stored. The wallet is always the key's own.
    Create and confirm take the fund timeout; a 504 `outcome_unknown` there
    means the transaction may have landed, so read `list_orders` and never
    retry blindly. Ponk adds no fee to orders, as on the site.
- **Copy and follow:** `copy_source`, `follow_status`, `follow_leader`,
  `update_follow`, `unfollow_leader`, `answer_follow_offer`,
  `list_copy_follows`, `follow_leaders` (batch, `POST /v1/agents/follow/batch`),
  `follow_many` (batch slug follows, `POST /v1/follows/batch`), and Copy LP:
  `wallet_follow` (size %, cap, pool filter, auto or ask), `unfollow_wallet`,
  `answer_wallet_offer`, `copy_log`, `copy_referral`, `attach_copy_referral`.
- **The self-custody loop** (unsigned builds for your own wallet, then a mark
  call with the signature): `agent_rebalance_prepare`,
  `agent_rebalance_prepare_split`, `agent_rebalance_prepare_open`,
  `agent_rebalance_mark`, `agent_fees_prepare`, `agent_fees_mark`,
  `agent_fees_confirm`, `position_open_prepare`, `position_mark_tx`,
  `position_rebalance_split`, `position_rebalance_open`,
  `position_rebalance_tick`, `position_compound_prepare`,
  `position_claim_prepare`, `lp_withdraw_prepare` (Meteora, Orca, Raydium and
  Ponk Clouds; Clouds is withdraw only while it is closed to new liquidity),
  `reclaim_wsol_prepare`, `stranded_positions`, `cleanup_position_prepare`,
  `positions_sync`, `share_position_telegram`, `clouds_referrals`,
  `clouds_referral_claim_prepare`.
- **Public Ponk Calls reads:** `calls_feed`, `callers_board`, `caller`
  (Python also `call`), no key needed.
- **Account:** Ponk Calls (`call_preview`, `post_call`, `delete_call`), Web
  Push (`push_settings`, `set_push_prefs`, `push_subscribe`,
  `push_unsubscribe`, `push_test`), the share-card looks (`pnl_card`,
  `set_pnl_card`, `referral_card`, `set_referral_card`), referral codes and the
  Creator application (`set_referral_code`, `add_referral_code`,
  `apply_creator`), leaderboard privacy (`leaderboard_privacy`,
  `set_leaderboard_privacy`, `agent_visibility`, `set_agent_visibility`),
  `clear_logs` (`DELETE /v1/logs`; the fund ledger is never deleted),
  `weekly_recap`, `my_ponkillionaires`, `telegram_status`.
- **`rank_presets`** (`POST /v1/presets/rank`): every wizard preset and your
  saved models replayed on one pool's window by the site's own Plot engine,
  ranked by net result, in one call. Six a minute per account, cached.

Still app only, by design: minting a custody mandate, API keys, webhooks and
linking Telegram.

Versions: Python and Rust 0.6.0 to 0.7.0.

## 0.6.0 (2026-10-23)

Added, in both the Python and the Rust client:

- **Agent fleets.** Create, fund, read and close many agents in one call.
  `create_agents` (`POST /v1/agents/batch`, each agent with its own wallet),
  `fund_prepare` (`POST /v1/agents/fund/prepare`: UNSIGNED transfers from your
  wallet to your agents' wallets, as few transactions as fit, plus signing
  links on ponk.exchange), `fleet` (`GET /v1/agents/fleet`, paged) with
  `fleet_all` (and `iter_fleet` in Python) following `next_offset`,
  `exit_agents` / `withdraw_agents` (`POST /v1/agents/batch/exit` and
  `/withdraw`, by id or all; Rust: `exit_all_agents`, `withdraw_all_agents`)
  and `get_batch` (`GET /v1/agents/batch/{batch_id}`) for what was still
  running when the response was written.
- `create_agent_wallet` (`POST /v1/agents/{id}/wallet`, idempotent).
- **Agent actions:** `rebalance`, `collect_fees` (destination `agent` or
  `connected`, never an address), `rename_agent` (`PATCH /v1/agents/{id}`),
  `delete_agent` (`DELETE /v1/agents/{id}`).
- **Agent reads:** `agent_proof` (cost and earnings per action) and
  `agent_range_history`.
- **Legacy shared wallet:** `custody_withdraw`, `custody_exit`.
- **Account:** `portfolio`, `portfolio_history`, `list_transactions`,
  `list_risk_events`, `list_fees`, `fee_summary`; notifications (list,
  unread count, read all, read, dismiss); recommendations (list, get, review,
  approve, dismiss, snooze); `notify_settings` / `update_notify_settings`;
  saved presets (list, create, update, delete).
- Python example `examples/fleet.py`: open 10 dry-run agents, fund them with
  one signature, read the fleet, exit them all.

Changed:

- `AgentWallet` gained `withdrawable_lamports` and
  `token_accounts_with_value`.
- Python: `create_agents` requires a boolean `dry_run` on every element, as
  `create_agent` does. Rust: a response with an empty body (a 204) decodes as
  JSON null, so `()` results work.

Neither client signs or holds a key. `fund_prepare` returns unsigned
transactions; your wallet signs them. Minting a custody mandate, API keys and
webhooks stay app only.

Versions: Python and Rust 0.5.0 to 0.6.0.

## 0.5.0 (2026-10-23)

Added, in both the Python and the Rust client, all with no key needed:

- **Live agents activity.** `agents_activity()` reads
  `GET /public/agents-activity`: the board agents' confirmed actions of the
  last 24 hours and every live hold, with the hold reason and check count. No
  event carries an agent id, a slug, a name or a wallet.
- **Weekly Ponkillionaires.** `ponkillionaires_week()` reads
  `GET /public/ponkillionaires/week`: this week's standings, the prizes (the
  following week's agent fee rate plus points) and last week's winners.
- **X Tracker.** `x_feed`, `x_trending`, `x_callers`, `x_token`, `x_accounts`
  and `x_status` (`/public/x/*`).
- **Wallet Tracker.** `wallets_feed`, `wallets_defaults`, `wallets_status`
  and `wallet_activity(address)` (`/public/wallets/*`).

Changed:

- **Referral tiers.** `Referral` gained `tier` (`current`, `share_bps`,
  `referred_fees_usd`, `window_days`, `next`) and `programme` (`ladder`,
  `window_days`). One level only: 20% of ponk's platform fee, 25% from $1,000
  and 30% from $5,000 of referred platform fees over the trailing 30 days.
- `PublicLink.path` is documented as `/ponkillionaires/<slug>`, which is what
  the server sends.
- Python: models can read camelCase responses (the trackers and the weekly
  race) while exposing snake_case attributes. Rust: `XFeedQuery` and
  `WalletFeedQuery` carry the feed filters.

Not added: following X accounts, tracked wallets, live copy follow
(`/agents/{id}/follow`, `/me/copy-follows`), mandate reminder settings and the
PnL and referral card settings. Those are app session routes that take a
wallet sign-in, not an API key, so a key-based client cannot call them.

Versions: Python and Rust 0.4.0 to 0.5.0.

## 0.4.0 (2026-10-21)

Added, in both the Python and the Rust client:

- **Top Ponkillionaires, no key needed.** `leaderboard(window)` reads the
  public board of shared agents ranked against holding (`24h`, `7d`, `30d`,
  `all`); `public_agent(slug)` reads one profile; `public_agent_value_history`
  reads its value chart.
- **Copy an agent.** Every leaderboard row and profile carries a `copy` block
  (venue, pool, strategy, config). Python: `copy_agent(slug_or_row, ...)`.
  Rust: `row.copy.to_new_agent(name, wallet)` then `create_agent`.
- **Share and follow.** `share_agent` / `unshare_agent` / `get_public_link`
  (`/v1/agents/{id}/public-link`), `follow` / `unfollow` / `list_follows`
  (`/v1/follows`).
- **Points, Season 1.** `points()`, `points_leaderboard()` and the public
  `public_points_leaderboard()`.
- **Referrals and Creators.** `referral()`, `creator_dashboard(days)`,
  `check_handle(handle)`, `claim_handle(handle)` (a handle makes the account a
  Creator) and the public `resolve_referral(code)`.
- **Agent reads.** `agent_value_history(id, range)` and
  `agent_decisions(id, hours, limit)`.
- **Settings edits.** `update_config`, `update_exits`, `set_auto_compound` and
  `optimize_draft`. Documented keys now include keep side, Meteora reshape,
  cooldown (`cooldown_sec`) and daily cap (`max_rebalances_per_day`), the
  market cap, position value and value plus fees take profits, and the Degen
  Mode bin step filter (`min_bin_step`, `max_bin_step`).
- Rust example `leaderboard`.

Versions: Python 0.3.0 to 0.4.0. Rust 0.2.0 to 0.4.0, to line the two up.

## 0.3.0

Python: `agent_mandate`, `set_autonomous`, `range_cost`, `get_json`, and
`create_agent` requires `dry_run`. Rust: mandates, autonomy, range cost and
webhook verification.
