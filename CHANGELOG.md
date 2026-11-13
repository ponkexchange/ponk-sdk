# Changelog

Both clients carry the same version from 0.4.0 on. Python lives in
`python/ponk/_version.py`, Rust in `rust/Cargo.toml`.

## 0.16.0

Both clients and the `ponk-cli` crate. The agents leaderboard, Calls, the X
Tracker and the Wallet Tracker now cover Solana, BNB Chain, Base, Ethereum,
Arbitrum and Robinhood Chain.

- **The chain filter.** `chains` on `leaderboard`, `calls_feed`, `top_calls`,
  `callers_board`, `x_feed`, `x_trending`, `x_callers`, `wallets_feed` and
  `wallet_activity`: a list of chain ids, or `all`. Every answer carries
  `chains`, the ids it covers. Rust: the `_on` methods, `CallsFeedQuery`, and
  `chains` on `XFeedQuery` and `WalletFeedQuery`.
- **The saved filter.** `feed_prefs` and `set_feed_prefs`
  (`GET` and `PATCH /v1/feed-prefs`): one choice per account, used by a
  signed-in read that names no chains.
- **EVM ids.** A token is a Solana mint or an EVM token id
  (`eip155:<chain id>:0x...`) on `x_token`, `calls_feed`, `call_preview`,
  `post_call` and the wallet feed's `token`; `call_preview` and `post_call`
  also take an EVM pool id and call the pool's token. A wallet is a Solana or
  an EVM address on `wallets_feed` and `wallet_activity`.
- **Calls on EVM tokens.** `Call.chain` and `Call.price_source` (`chainlink`,
  `onchain_pool`, `coingecko`, `defillama` or `dexscreener`). The Holder mark
  on an EVM call is the caller's own EVM agents whose pool holds the token.
- **X Tracker.** Each token a post names carries `chain`, and `chains` when
  the address is a token on several chains. `XToken.chain` and
  `XToken.price_source`. Rust: `XPost.tokens` is now `Vec<XPostToken>`.
- **Wallet Tracker.** `WalletEvent.chain` and `WalletEvent.explorer_url`;
  `WalletSource.evm`, the read state per EVM chain.
- **Leaderboard and recap.** `LeaderboardRow.chain`; `WeeklyRecap` with
  `best_agent.chain`.
- **CLI.** `ponk leaderboard --chains`, `ponk feed-prefs`,
  `ponk feed-prefs set`.
- **Liquidity shapes on EVM agents.** `spot`, `curve`, `bid_ask` and
  `spot_bid_ask` (`ponk.evm.SHAPES`), written as the config's
  `liquidity_shape`. On Uniswap v3 (Arbitrum, Ethereum, Robinhood Chain),
  Uniswap v4 (Arbitrum, Ethereum), PancakeSwap v3 and Infinity CL (BNB Chain)
  and Aerodrome Slipstream (Base) a shape other than Spot opens as several
  ranges managed as one position, `shape_ranges` `auto` (from live gas, within
  3% of the deposit) or 1 to 7 (`config.evm.shape_ranges`); on any other range
  venue it opens one range. Python: `shape` and `shape_ranges` on
  `create_evm_agent`, `ponk.evm.with_shape`, `SHAPE_VENUES`,
  `is_shape_venue`. Rust: `NewAgent::shape`, `NewAgent::shape_ranges`,
  `evm::with_shape`, `evm::ShapeRanges`. A shape on a full-range venue, or a
  range count where one range opens, is refused before a request.
- **One-sided ranges.** `down_pct` or `up_pct` of 0 is a band wholly above or
  below the price (`range_config` refused 0 before); both 0 is refused.
- **Shape quotes and shaped plans.** `evm_open_cost` takes `shape`, `ranges`,
  `amount_usd`, `range_pct`, `side`, `width_ticks` and `agent` and returns
  `shape` (the ranges it opens as now and the gas of each count against the
  3% limit); Rust: `OpenCostQuery`, `evm_open_cost_with`,
  `evm_open_cost_public_with`. `copilot_evm_plan` takes `shape` and
  `one_sided` (Rust: `copilot_evm_plan_shaped`) and each leg carries
  `EvmLegShape`. CLI: `--shape` and `--shape-ranges` on `create-evm-agent`,
  `--shape`, `--ranges`, `--amount-usd`, `--range-pct` and `--side` on
  `open-cost`, `--shape` and `--one-sided` on `plan`.
- **The exit fee.** A withdrawal or an exit from an agent that has opened a
  position pays 1% of what leaves (100 bps; 50 for a wallet holding 1,000,000
  PONK), taken in kind, on every chain. `Agent.exit_fee_bps`,
  `PonkPerks.exit_fee`, `Withdrawal.exit_fees` and `Withdrawal.exit_fee_bps`
  (what an EVM withdrawal or exit paid), and `exit_yield_fee_bps` and
  `exit_swap_fee_bps` on `EvmOpenCost`. The docs that said no exit fee is
  charged are corrected. Python: `ponk.EXIT_FEE_BPS`,
  `ponk.EXIT_FEE_HOLDER_BPS`. CLI: `open-cost` prints the exit fee.
- **Typed answers.** Python `calls_feed` returns `CallsFeed`,
  `callers_board` returns `CallersBoard` and `weekly_recap` returns
  `WeeklyRecap` (each was the decoded body; `raw_json` keeps it). Rust keeps
  its 0.15.0 signatures and adds `calls_feed_with`, `callers_board_on` and
  `weekly_recap_typed`.
- **Fixed: calls read camelCase.** `Call` and `CallPreview` now read the
  API's camelCase keys (`priceUsd`, `multipleNow`, `hasPosition`, ...), which
  0.15.0 left `None`.

## 0.15.0 (2026-11-10)

Both clients and the `ponk-cli` crate. Ponk agents now run on five EVM chains
beside Solana: Ethereum, Arbitrum, Base, BNB Chain and Robinhood Chain.

- **Chains and venues.** `chain` is on every agent, position, wallet view and
  mandate: `solana`, `ethereum`, `arbitrum`, `base`, `bsc` or `robinhood`. The
  EVM venue ids are `uniswap_v2`, `uniswap_v3`, `uniswap_v4` (Ethereum,
  Arbitrum, Robinhood Chain), `pancakeswap_v2`, `pancakeswap_v3`,
  `pancakeswap_infinity_cl` (BNB Chain) and `aerodrome_slipstream`,
  `aerodrome_classic` (Base). The tables are `ponk.evm` in Python and
  `ponk::evm` in Rust, with `venue_chains`, `is_full_range_venue` and
  `stake_reward_token`.
- **CAIP ids.** `pool_id`, `token_id`, `native_token_id`, `v4_pool_id` (the
  pool manager and the 32 byte pool id, for Uniswap v4 and PancakeSwap
  Infinity CL), `position_nft_id`, `parse_evm_id` and `chain_of`. A Solana
  address or an id on a chain ponk does not run on is never read as one.
- **Pools.** `evm_chains` (`GET /public/evm/chains`), `evm_pools`
  (`GET /public/evm/{chain}/board?venue=`), `evm_pool`
  (`GET /public/evm/pools/{id}`) and `evm_token`
  (`GET /public/evm/tokens/{id}`), each with its `/v1` twin (`public=False` in
  Python, the `_keyed` methods in Rust). A pool carries its fee tier in
  percent, `dynamic_fee`, TVL, volume and LP fees per window, `farm`
  (`token`, `live`, `apr_pct`), the hook, and `flags()`: `unverified_token`,
  `hook_not_agent_eligible`, `under_1h_old`.
- **Open cost.** `evm_open_cost` (`GET /v1/evm/{chain}/open-cost`): the
  measured gas of the first open, the exit reserve and the least to hold, in
  wei, and the two fee rates a key's agents pay (150 and 1500 basis points).
  The keyless route states the rates of an agent created on ponk.exchange.
- **Create.** `create_evm_agent` (`POST /v1/agents` with `chain`): the range
  as `down_pct` and `up_pct` fractions on a range venue, compound and hold on
  a full-range venue, `stake` for CAKE or AERO, and `withdrawal_address`.
  Rust: `NewAgent::evm_range`, `NewAgent::evm_full_range`, `.stake()`,
  `.withdrawal_address()`, `.chain()`. `create_agent` takes `chain` and
  `withdrawal_address` too. `ponk.evm.range_config` builds the config the
  server's own planner sends.
- **Read an EVM agent with the methods you already use.** `agent_wallet`
  answers the EVM view (`address`, `native_symbol`, `balance_wei`,
  `reserve_wei`, `min_open_wei`, `withdrawable_wei`, `balance_usd`,
  `native_usd`, `withdrawal_address`, `mandate_status`), `agent_position` is
  in ticks with `bin_step`, `chain` and `staked`, `agent_performance` adds
  `farm_rewards` (`token`, `pending_amount`, `pending_usd`, `harvested_usd`,
  `staked`, `apr_pct`, `apr_basis`), `pnl_note` and `figures_unavailable`,
  and `agent_decisions`, `agent_value_history`, `agent_mandate` and
  `list_positions` cover EVM agents. `pnl_pct` and `drawdown_pct` on the
  performance view are fractions on every chain.
- **Control.** `pause_agent`, `resume_agent` and `update_config` work on an
  EVM agent; `set_stake_rewards` sends `{"evm": {"stake": bool}}`. `withdraw`
  and `exit_agent` return what the call really sent: `tx_hashes`,
  `amount_wei`, `received` (`token`, `amount`, `tx_hash`), `status` and
  `sentence`.
- **Copilot.** `copilot_evm_plan`, `copilot_evm_get_plan` and
  `copilot_evm_start` (`/copilot/evm/plans`): one sentence in dollars across
  one or more chains, with `total_usd`, `funding_by_chain`, the legs, what
  was left out and why, and after a start the agents, `sign_urls` and
  `fleet_id`. `evm_fleet` (`GET /agents/evm/fleet`) lists every EVM agent and
  what it needs.
- **The fund-and-sign link.** `fund_link` and `fund_links` build
  `https://ponk.exchange/agents/fund?a=<id>:<amount>`, 40 agents to a link,
  by the rule the page parses. An EVM agent is listed with 0. The clients
  expose the mandate's status and this link; signing stays in your own
  wallets on ponk.exchange.
- **CLI**: `chains`, `evm-pools`, `evm-pool`, `open-cost`, `create-evm-agent`,
  `wallet`, `fund-link`, `evm-fleet`, `plan`, `start-plan` and `exit`.
  `agents` and `agent` show the chain, `agent` shows farm rewards, and
  `withdraw` prints an EVM withdrawal in the server's own sentence.
- **Fixed, Rust**: `list_positions` failed to decode for an account holding
  an EVM position, because that row's `fees_claimable_x.raw` is a decimal
  string. `TokenAmount.raw` now reads a number or a string.
- **Python**: `Agent` gains `activation_fee_bps`, `waiting` and
  `pool_warning`, which the Rust client already had, and
  `ACTIVATION_FEE_API_BPS` (150) and `ACTIVATION_FEE_API_HOLDER_BPS` (75).
- **Rust, breaking for struct literals**: `NewAgent` gains `chain` and
  `withdrawal_address`, and `Error` gains `InvalidArgument`. Build a
  `NewAgent` with `NewAgent::new` and match `Error` with a wildcard arm.

Solana behaviour is unchanged: every earlier test passes as it was.

## 0.14.0 (2026-11-05)

Both clients and the `ponk-cli` crate. The Python client skips 0.13.0 (that
release was Rust only), so the two version numbers line up again.

- **Meteora DAMM v2** (`meteora_damm_v2`) is a venue in both clients and the
  CLI. A position there is a share of the pool with NO range, so nothing on it
  takes a bin, a tick or a width, and only `monitor_only` and `fee_collector`
  agents run on it. New self-custody builders, each returning an UNSIGNED
  transaction for the `owner` you name, with no key needed:
  `damm_v2_open_prepare` (`POST /public/lp/meteora_damm_v2/open/prepare`:
  `pool_address`, `owner`, `amount_x`, `amount_y` in base units sent as
  strings, optional `max_slippage_bps`; the result names `position_address`
  and `position_nft_mint`) and `damm_v2_claim_prepare`
  (`POST /public/lp/meteora_damm_v2/claim/prepare`: `pool_address`,
  `position`, `owner`; Ponk's fee review in `fee`, drafts in
  `fee_event_ids`).
- **Meteora DAMM v2 agents run autonomously.** The server put the venue on
  its autonomy allow-list on 2026-11-05, after every managed leg was simulated
  through its executor against live mainnet pools. A `fee_collector` created
  with no `position_address` opens its own share of the pool from its agent
  wallet once it is funded and autonomous (`set_autonomous` under a mandate
  signed in the app): it holds back the venue's SOL reserve, swaps part of the
  SOL into the pool's other token at the pool's own deposit ratio, and needs
  at least $5 deployable. It then compounds (`compound` reaches DAMM v2) or
  claims with Ponk's fee, exits on `stop_loss_price`, `take_profit_price` and
  the PnL thresholds (`price_stop_drop_pct` is still refused there), and
  `exit_agent` claims, drains and closes the position; a position whose
  liquidity is fully locked is claimed and kept, and the exit reports it
  incomplete. An agent that is not autonomous still needs a position you
  hold. No client method changed for this: the docstrings say it.
- **Add and withdraw for any owner**: `lp_add_liquidity_prepare`
  (`POST /public/lp/{dex}/add-liquidity/prepare`, `meteora_damm_v2`, `orca`
  or `raydium`) and `lp_withdraw_prepare_public`
  (`POST /public/lp/{dex}/withdraw/prepare`, `meteora_damm_v2`, `orca`,
  `raydium` or `ponk_clouds`). The key route `lp_withdraw_prepare`
  (`POST /v1/lp/{dex}/withdraw/prepare`) documents `meteora_damm_v2` too.
  `Prepared` gains `position_nft_mint`, `fee` and `closes_position` (a DAMM v2
  withdraw says whether it also closes the position).
- **Pool history**: `pool_history` (`GET /public/pools/{dex}/{address}/history`,
  `resolution` `day` or `hour`, `buckets` up to 90 days or 96 hours) and its
  key-budget twin (`public=False` in Python, `pool_history_keyed` in Rust,
  `GET /v1/pools/{dex}/{address}/history`). Volume, fees, LP and protocol
  fees, fee rate and TVL per bucket on Meteora DLMM, Orca and Raydium, each
  field's source in `sources`, the LVR edge where measured. A bucket no source
  has is absent, never zero.
- **The sell check on a pool read**: `PoolSnapshot` gains `sell_warning`,
  `mispricing_warning` and `price_divergence` (and, in Python, the
  `token_price` fields the Rust client already had). Advice, never a block.
- **Copy alerts**: `NotifySettings` gains `copy_alerts` (push and Telegram for
  every action a Copy LP follow completes) and `comments_telegram`, and
  `update_notify_settings` sets either.
- The Rust Discover board's `OppSources` gains `damm_v2` (`dammV2` on the
  wire).
- CLI: `ponk pool` probes `meteora_damm_v2` and accepts it in `--dex` (its
  zero-filled bin step and active bin print as a dash), prints the price and
  sell warnings when the pool has them, and `ponk pools` labels DAMM v2 rows
  and says when that source did not answer.

## 0.13.0 (2026-11-04)

Rust only; the Python client is unchanged at 0.12.0.

- **A command line.** `rust/cli` is a new crate, `ponk-cli`, whose binary is
  `ponk`: `pools` (the Discover board across Meteora DLMM, Orca Whirlpools and
  Raydium CLMM, or a Meteora board view with `--view hot|new|volume|tvl`),
  `pool <address>` (probes the venues unless `--dex` names one), `agents`,
  `agent <id or public slug>`, `create-agent`, `pause`, `resume`, `withdraw`,
  `leaderboard` and `whoami`. Configuration is `PONK_API_KEY`, `PONK_API_URL`
  and `PONK_WEB_URL`; `--json` prints the raw response for the same request.
  Tables are aligned, and a figure the server did not send is a dash, never a
  zero. Install: `cargo install --git
  https://github.com/ponkexchange/ponk-sdk ponk-cli`.
- **Pool market lists**, in the Rust client: `hot_pools` (the app's Meteora
  pool board and its views), `opportunities` (the Discover board: Meteora,
  Orca and Raydium in one read, each row with both legs' risk reads and
  per-window volume and fees, plus per-source health in `ok`) and
  `dlmm_pools` (the venue's paginated DLMM list; its `apr` is percent per
  DAY, as upstream reports it). The two app-served boards read from a new
  `web_url` (derived from the base URL, or set on the builder), and never
  carry the API key.
- **Raw escape hatches**: `post_json` beside the existing `get_json`, and
  `get_web_json` for the web origin.
- `Agent` now carries `activation_fee_bps`, `waiting` and the create
  response's `pool_warning` (the deploy gate warns, it never refuses), and
  `PoolSnapshot` the `token_price` fields the pool route serves.

## 0.12.0 (2026-11-02)

Added, in the Python client:

- **Live social stream.** `stream_social()` opens `GET /public/social/stream`
  and yields a `StreamEvent` as each event happens, indefinitely: one `ready`
  on connect, a `row` per event, and a `resync` when the client should reload
  from `live_feed`. `stream_social(public=False)` streams your own following
  graph from `GET /me/social/stream` and needs a key. The transport is
  Server-Sent Events, not a WebSocket: the API exposes no WebSocket, and the
  client parses the event stream with the standard library, so the Python
  client still has zero runtime dependencies. `examples/stream.py` prints the
  public feed as it moves.

The Rust client caught up to the Python client at this version (it had stayed
at 0.7.0 while the Python client moved through 0.8.0 to 0.11.0). It now has the
copilot (`copilot_parse`, `copilot_plan`, `copilot_get_plan`, `copilot_replay`,
`copilot_wait`, `copilot_start`, `copilot_fleets`, `copilot_fleet`,
`copilot_stop_fleet`, `copilot_tighten_stop`), the people and social graph
(`profile`, `followers`, `following`, `person_activity`, `person_positions`,
`people_search`, `recommended_people`, `similar_people`, `people_board`,
`live_feed`, `follow_person`, `unfollow_person`, `my_recommended`,
`dismiss_recommended`, `my_following`, `following_feed`), the live social
stream (`stream_social`, over a `SocialStream` that yields a `StreamEvent` per
frame, parsed from the response body with no extra dependency), the privacy
and exit-to-SOL methods (`privacy`, `set_privacy`, `answer_privacy_prompt`,
`get_exit_to_sol`, `set_exit_to_sol`), the one-call order helpers
(`orders_sign_in`, `place_order`), `exit_agent` with the to-SOL choice,
`copy_agent`, `mandates`, `tracked_lps`, `top_calls`, `wallet_pnl`, `x_account`
and `call`. `examples/stream.rs` prints the public feed as it moves. The one
Python surface left out is `iter_fleet`, whose collected form is the existing
`fleet_all`.

## 0.11.0 (2026-10-29)

Added, in the Python client (the Rust client stays at 0.7.0):

- **People search.** `people_search(q, cursor=None)`: accounts by @handle
  (the @ is optional), creator handle, /r/ code or display name, from the
  first letter, twenty a page, each with its follower count. No key needed.
- **Who to follow.** `recommended_people` (no key), `similar_people(key)`,
  `my_recommended` and `dismiss_recommended(key)`, each person with the
  reason they are suggested.
- **Every mandate at once.** `mandates()`: each autonomous agent's mandate
  state (`active`, `expiring`, `expired`, `none`) with its renew link, and
  `renew_all_url` to sign every renewal on one page.
- **Deploy past the pool checks.** `create_agent(..., accept_pool_risk=True)`
  (and `accept_pool_risk` on a `create_agents` element) creates the agent on a
  pool the deploy gate calls young, thin, quiet or unsettled, after you have
  read its reason.
- **Copilot.** A plan that names a new token keeps its pools below the checks
  with `warnings` on each leg, and an `unverified_token` or `ambiguous_token`
  refusal carries `suggestions` (mints, most liquid first). Documented on
  `CopilotPlan`; the fields arrive in `plan` and `refusal` as before.

Changed:

- Every account is public. `privacy`, `set_privacy` and `answer_privacy_prompt`
  answer the public state and the server ignores their writes.
- Any account can post a call on any token; a position earns the Holder mark.
- The app's default mandate is 365 days (was 30).

## 0.10.0 (2026-10-28)

Added, in the Python client (the Rust client stays at 0.7.0):

- **People.** Public profiles for every @handle and /r/ code (`profile`),
  followers and following lists, a person's activity and open positions, the
  people board and the public live feed, no key needed. With an API key:
  `follow_person`, `unfollow_person`, `my_following` and `following_feed`.
- **Privacy.** `privacy`, `set_privacy` (public or private, show wallet, show
  dollar amounts, share swaps) and `answer_privacy_prompt`. The
  leaderboard-privacy methods keep working.
- **Activation fee.** `PonkPerks.activation_fee` (1%, 0.5% for a 1M $PONK
  holder), `FeeEvent.source == "activation"`, and `ACTIVATION_FEE_BPS`,
  `ACTIVATION_FEE_HOLDER_BPS`, `ACTIVATION_REFERRER_BPS` (the referrer's 0.25%).
- **Trackers and calls.** `wallet_pnl`, `tracked_lps`, `x_account`,
  `top_calls`, and `x_feed(first_calls=True)`.

## 0.9.0 (2026-10-28)

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
