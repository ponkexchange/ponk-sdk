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
# Python
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
| **Read** | `whoami` · `list_agents` · `get_agent` · `agent_performance` · `agent_position` · `agent_wallet` · `list_positions` · `list_logs` |
| **Act** (`trade` scope) | `create_agent` · `pause_agent` · `resume_agent` · `set_dry_run` · `set_mode` · `compound` · `withdraw` · `exit_agent` |
| **Public** (no key) | `health` · `pool` · `ponk_perks` |

`create_agent` accepts `dry_run=True`, which runs the whole decision loop and
records what it *would* have done without sending a transaction. Start there.

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

## Versioning

Both clients are `0.1.0` and track the deployed API. The API is versioned at
`/v1`; a breaking change there gets a new path, not a silent redefinition of an
existing field.

## License

Apache-2.0. See [LICENSE](LICENSE).
