# Changelog

Both clients carry the same version from 0.4.0 on. Python lives in
`python/ponk/_version.py`, Rust in `rust/Cargo.toml`.

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
