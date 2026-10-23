# Changelog

Both clients carry the same version from 0.4.0 on. Python lives in
`python/ponk/_version.py`, Rust in `rust/Cargo.toml`.

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
