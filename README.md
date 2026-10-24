<p align="center">
  <img src=".github/banner.png" alt="ponk.exchange - Autonomous Liquidity Management, Built on Solana" width="640">
</p>

<p align="center">
  <a href="https://ponk.exchange"><b>ponk.exchange</b></a> &nbsp;·&nbsp;
  <a href="https://ponk.exchange/docs/developers/agents-api">API reference</a> &nbsp;·&nbsp;
  <a href="https://ponk.exchange/settings">Get a key</a> &nbsp;·&nbsp;
  <a href="https://github.com/ponkexchange/ponkexchange">Main repo</a>
</p>

<p align="center">
  <img alt="Python 3.8+" src="https://img.shields.io/badge/python-3.8%2B-blue">
  <img alt="Rust 1.85+" src="https://img.shields.io/badge/rust-1.85%2B-orange">
  <img alt="License Apache-2.0" src="https://img.shields.io/badge/license-Apache--2.0-green">
  <img alt="Runtime dependencies: none (Python)" src="https://img.shields.io/badge/python%20deps-0-lightgrey">
</p>

---

Official clients for the **ponk public API**. Create and drive Solana
concentrated-liquidity agents, read their live positions, PnL and fees, and
move funds home, from Python or Rust.

An agent opens a concentrated position, collects the fees, compounds them, and
moves the range when the price does, across **Meteora DLMM**, **Orca
Whirlpools**, **Raydium CLMM** and **Ponk Clouds**. These clients are how you
drive that from your own code instead of the app.

| | |
|---|---|
| [`python/`](python) | Standard library only. Zero runtime dependencies. |
| [`rust/`](rust) | `reqwest` + `serde`, async, `rustls` by default. |

## Install

```bash
# Python, from PyPI
pip install ponk

# Python, straight from this repository
pip install "git+https://github.com/ponkexchange/ponk-sdk#subdirectory=python"
```

```toml
# Rust, Cargo.toml. Cargo finds the `ponk` package inside the repo itself,
# so there is no subdirectory key to set (there is no such key).
[dependencies]
ponk = { git = "https://github.com/ponkexchange/ponk-sdk" }
```

## Get a key

Open [Settings, API keys](https://ponk.exchange/settings) in the app with your
wallet. Name the key, pick **Read only** (`read`) or **Read and act**
(`trade`), and copy the secret.

It is shown once. The server stores only a hash, so it genuinely cannot be
shown again; a lost key is revoked and replaced, not recovered.

A key carries exactly the authority of the wallet that minted it. It can never
see or touch another account, and it cannot mint or revoke keys, so a leaked
key cannot extend or outlive its own revocation.

## Quickstart

<table>
<tr><th align="left">Python</th><th align="left">Rust</th></tr>
<tr valign="top">
<td>

```python
import os
from ponk import PonkClient

ponk = PonkClient(api_key=os.environ["PONK_API_KEY"])

me = ponk.whoami()
print("acting as", me.wallet_address,
      "scope", me.scope)

for a in ponk.list_agents():
    perf = ponk.agent_performance(a.id)
    # None means "could not be priced",
    # never zero. Print a dash, not a
    # number you do not have.
    print(a.name, a.status,
          perf.pnl_usd or "-")
```

</td>
<td>

```rust
use ponk::Client;

#[tokio::main]
async fn main() -> ponk::Result<()> {
    let ponk = Client::new(
        "https://ponk.exchange/api",
        std::env::var("PONK_API_KEY")
            .unwrap(),
    )?;

    for a in ponk.list_agents().await? {
        let p = ponk
            .agent_performance(&a.id)
            .await?;
        println!("{} {:?}", a.name, p.pnl_usd);
    }
    Ok(())
}
```

</td>
</tr>
</table>

## What the clients deliberately do not do

**They never compute a number the server did not send.** Every USD field
crosses the wire as a string or null, and null means *could not be priced*, not
*zero*. A client that helpfully substituted `0.0` there would turn "we do not
know what this is worth" into "this is worth nothing", and those are opposite
statements to someone deciding whether to withdraw. So the types keep the null
and it is the caller's job to render a dash.

**They never sign, never broadcast, and never hold a private key.** An API key
authorises calls to ponk; it does not authorise a transaction. Actions that
move funds are executed by the agent's own wallet under a scoped, expiring
mandate you granted in the app, and the clients cannot create or widen one.

**They are thin on purpose.** One method per endpoint, no code generation, no
retry-and-hope loop hiding a failure. Errors arrive as distinct types
(`PonkAuthError`, `PonkRateLimitedError`, `PonkConflictError`, and so on)
because "the key is wrong" and "you are going too fast" call for different
handling, and collapsing them into one exception is how a caller ends up
retrying an authentication failure forever.

## What you can drive

| | |
|---|---|
| **Read** | `whoami` · `list_agents` · `get_agent` · `agent_performance` · `agent_position` · `agent_value_history` · `agent_decisions` · `agent_wallet` · `agent_mandate` · `list_positions` · `list_logs` · `get_public_link` · `list_follows` · `points` · `points_leaderboard` · `referral` · `creator_dashboard` · `check_handle` |
| **Act** (`trade` scope) | `create_agent` · `copy_agent` (Python) · `optimize_draft` · `pause_agent` · `resume_agent` · `set_dry_run` · `set_mode` · `update_config` · `update_exits` · `set_auto_compound` · `set_autonomous` · `share_agent` · `unshare_agent` · `follow` · `unfollow` · `claim_handle` · `compound` · `withdraw` · `exit_agent` · `rebalance` · `collect_fees` · `rename_agent` · `delete_agent` · `create_agent_wallet` · `custody_withdraw` · `custody_exit` |
| **Fleet** | `create_agents` · `fund_prepare` · `fleet` · `iter_fleet` / `fleet_all` · `get_batch` · `exit_agents` · `withdraw_agents` |
| **Account** | `portfolio` · `portfolio_history` · `list_transactions` · `list_risk_events` · `list_fees` · `fee_summary` · `agent_proof` · `agent_range_history` · notifications · recommendations · `notify_settings` · presets |
| **Public** (no key) | `health` · `pool` · `range_cost` · `ponk_perks` · `leaderboard` · `public_agent` · `public_agent_value_history` · `public_points_leaderboard` · `resolve_referral` · `agents_activity` · `ponkillionaires_week` |
| **X Tracker** (no key) | `x_feed` · `x_trending` · `x_callers` · `x_token` · `x_accounts` · `x_status` |
| **Wallet Tracker** (no key) | `wallets_feed` · `wallets_defaults` · `wallets_status` · `wallet_activity` |

`create_agent` accepts `dry_run=True`, which runs the whole decision loop and
records what it *would* have done without sending a transaction. Start there.
Say so explicitly: on `POST /v1/agents` an omitted `dry_run` creates a LIVE
agent, the opposite of the app's default, and the Python client requires the
argument for that reason.

`set_autonomous` switches autonomous signing on under a mandate you already
signed in the app, or off. It cannot create a mandate. `agent_mandate` reads
that mandate's expiry, which is the thing to watch: a lapsed mandate stops the
agent signing without any error.

`referral()` carries your referral tier: one level, 20% of ponk's platform
fee on the people you referred, 25% once they paid $1,000 of platform fees
over the trailing 30 days, 30% from $5,000. `tier.next` is the next rung and
`programme.ladder` the whole ladder, so no integration hardcodes a rate.

## Run a fleet

Open ten agents, fund them with one signature, close them all:

```python
batch = ponk.create_agents([dict(body, name="fleet %d" % i, dry_run=True) for i in range(10)])
ids = [it.agent_id for it in batch.items if it.ok]

plan = ponk.fund_prepare(ids)     # UNSIGNED transfers, ten in one transaction
print(plan.sign_urls)             # your wallet signs on ponk.exchange

for row in ponk.iter_fleet():     # wallet balance, mandate expiry, value
    print(row.agent.name, row.wallet and row.wallet.lamports)

out = ponk.exit_agents(ids)       # close every position, sweep every wallet home
while out.running:
    out = ponk.get_batch(out.batch_id)
```

`fund_prepare` names agent ids, never addresses: the server resolves each
agent's own wallet. Neither the SDK nor the server signs; the transfers come
back unsigned for your wallet, on ponk.exchange or in wallet software you
control. The full script is
[`python/examples/fleet.py`](python/examples/fleet.py).

## Watch the market without a key

```python
from ponk import PonkClient

ponk = PonkClient()
for t in ponk.x_trending(window="1h").tokens:
    print(t.symbol, t.accounts_1h, "accounts, first call by", t.first_call.screen_name, t.multiple or "-")

for e in ponk.agents_activity().events:
    print(e.kind, e.dex, e.at, e.usd or "-")

week = ponk.ponkillionaires_week()
for s in week.standings[:3]:
    print(s.rank, s.display_name or s.owner_key, s.pnl_usd)

activity = ponk.wallet_activity(ANY_WALLET_ADDRESS)
print(activity.mode, [(e.kind, e.usd) for e in activity.items[:5]])
```

Following X accounts, tracking your own wallets, live copy follow and the
notification and card settings are app session routes (`/me/*`,
`/agents/{id}/follow`). They take a wallet sign-in, not an API key, so they are
not in these clients. Use them in the app.

## Copy a Top Ponkillionaire

Every agent its owner chose to share is on the public leaderboard, ranked
against holding its opening tokens, and every row carries a `copy` block: the
venue, pool, strategy and config, with nothing of the owner's in it. Copying is
a read and a create on your own wallet; the leaderboard needs no key.

```python
board = ponk.leaderboard(window="7d")
best = board.ranked[0]
print(best.slug, best.vs_hodl_pct, best.pnl_usd or "-")

agent = ponk.copy_agent(best, name="copied " + best.slug,
                        wallet_address=me.wallet_address, dry_run=True)
ponk.follow(best.slug)        # told when it opens, rebalances or exits
ponk.share_agent(agent.id)    # put yours on the board too
```

```rust
let board = ponk.leaderboard(Some("7d")).await?;
let best = &board.ranked[0];
let body = best.copy.to_new_agent("copied", &wallet).expect("copyable");
let agent = ponk.create_agent(&body).await?;
ponk.follow(best.slug.as_deref().unwrap_or_default()).await?;
```

Points (Season 1) and referrals are readable with the same key:
`points()`, `points_leaderboard()`, `referral()` (earned, owed and unproven,
never summed) and, once you claim a handle with `claim_handle`, the Creator
dashboard at `creator_dashboard()`.

## Ranges are the whole game

Concentrated liquidity earns fees only while the price is inside your range.
Too tight and you are out of range earning nothing; too wide and your capital
is spread so thin the fees do not cover divergence. The entire job is choosing
a range, knowing when it stopped being right, and moving it only when moving it
is worth more than it costs.

That last clause is where most automation goes wrong, because a rebalance is
not free: gas, a balancing swap, rent, and a performance fee. An agent that
rebalances on every wobble loses to one that holds. `list_logs` returns the
declines as well as the actions, which is the half most automation never shows
you.

## Security

The API these clients call was assessed by **zauth (Vector)** on 29 September
2026, alongside the web application and the MCP server: a deep scan over 51
endpoints and 58 input vectors, 389 turns. **12 findings: 3 high, 5 medium,
1 low, 3 informational. No critical.**

All 12 findings were remediated on 30 September 2026, each verified against the
live site rather than from the diff. Eight are closed; the missing
Content-Security-Policy is partially fixed, since the policy ships report-only
with a violation collector and so does not block yet.

- [Report (PDF)](https://ponk.exchange/audits/zauth-ponk-exchange-2026-09-29.pdf)
- [Audits](https://ponk.exchange/docs/audits), with the per-finding remediation table

To report something, see
[SECURITY.md](https://github.com/ponkexchange/ponkexchange/blob/main/SECURITY.md).

## Versioning

Changes per release are in [CHANGELOG.md](CHANGELOG.md).

Each client carries its own version (`python/ponk/_version.py`,
`rust/Cargo.toml`) and tracks the deployed API. The API is versioned at
`/v1`; a breaking change there gets a new path, not a silent redefinition of an
existing field.

## License

Apache-2.0. See [LICENSE](LICENSE).
