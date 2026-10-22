"""A thin client for the ponk public API.

One method per endpoint, no dependencies beyond the standard library, and no
behaviour of its own: it builds the request, sends it, maps the error envelope
to an exception and parses the body into the types in `ponk.models`. It never
computes, rounds or fills in a number the server did not send.

    from ponk import PonkClient

    ponk = PonkClient(api_key="ponk_live_...")
    me = ponk.whoami()
    for agent in ponk.list_agents():
        print(agent.name, agent.status)

What a key cannot do, by construction rather than by configuration:

* It cannot sign with your connected wallet. Non-custodial actions still need
  your signature in the app; this client can read their state, not produce it.
* It cannot create a custody mandate. A mandate is a wallet signature over an
  exact canonical text, and it can only be produced in the ponk app with your
  wallet connected. Once one exists for an agent, a `trade` key CAN use it:
  `set_autonomous(agent_id, True)` turns autonomous signing on under that
  existing, unexpired mandate (the agent must have its own managed wallet),
  and `set_autonomous(agent_id, False)` turns it off, which is always allowed.
  The server refuses a request that carries a `grant`, so this client never
  sends one.
* `compound`, `withdraw`, `exit` and `agent_wallet` act on the isolated wallet
  an autonomous agent owns. A self-custody agent has no such wallet, so those
  four do not apply to it.

Withdrawals and exits have no destination parameter and cannot be given one.
The server locks the destination to the wallet that owns the agent.

The mandate expires, and that is the failure to design for
---------------------------------------------------------

A mandate carries a hard expiry, and the grant the app asks you to sign runs 30
days by default. Lapsing is a pure clock fact: no transaction and nothing on
chain marks it. The agent keeps running and every signature it asks for is
denied, so what you see from here is an agent that quietly stopped
compounding, claiming and recentering with its funds still deployed. The
symptom is silence, not an exception. Four consequences for your code:

* A fund call can return a receipt and still have done nothing. `compound`
  raises only on an HTTP error, and a mandate denial is not one: it lands as
  HTTP 200 carrying a FAILED receipt. Read `ActionReceipt.status`,
  `action_status` and `error_message`, and read `signature is None` as
  "nothing reached the chain". Never read the absence of an exception as
  success.
* Read the expiry, do not infer it. `agent_mandate(agent_id)` returns the
  live mandate with `expires_at`, `status` (`active` or `expired`) and a
  server-computed `seconds_remaining` that goes negative once it has lapsed.
  It raises `PonkNotFoundError` when the agent has no mandate at all. ponk
  also sends `mandate_expiring` (from five days ahead) and `mandate_expired`
  notifications, which a webhook registered from a signed-in session
  receives.
* Turning autonomy off does not revoke the mandate. `set_autonomous(id,
  False)` stops autonomous signing but leaves the mandate in place, so a
  later `set_autonomous(id, True)` re-arms it for as long as it is unexpired.
  Revoking the mandate itself is done in the app.
* A lapsed mandate can never trap your funds. `withdraw` and `exit` are
  user-initiated and deliberately bypass the mandate, so both keep working
  after it expires. Renewing it needs a fresh wallet signature in the app.
"""

from __future__ import annotations

import json
import socket
import urllib.error
import urllib.parse
import urllib.request
from typing import Any, Dict, List, Mapping, Optional

from ._version import __version__
from .errors import PonkTransportError, error_from_response
from .models import (
    ActionLog,
    ActionReceipt,
    Agent,
    AgentPerformance,
    AgentPosition,
    AgentWallet,
    AutonomousState,
    CreatorDashboard,
    Follow,
    Follows,
    HandleCheck,
    Health,
    Leaderboard,
    LinkRevoked,
    Mandate,
    MyPoints,
    PointsLeaderboard,
    PonkPerks,
    PoolSnapshot,
    Position,
    PublicAgent,
    PublicLink,
    PublicReferral,
    RangeCost,
    Referral,
    ValueHistory,
    WhoAmI,
    Withdrawal,
)

__all__ = ["PonkClient", "DEFAULT_BASE_URL", "DEFAULT_TIMEOUT", "DEFAULT_FUND_TIMEOUT"]

DEFAULT_BASE_URL = "https://ponk.exchange/api"

#: Read and control calls. Anything slower than this is a bug on the server.
DEFAULT_TIMEOUT = 30.0

#: compound, withdraw and exit send several transactions and wait for each
#: confirmation, so they get their own, longer timeout.
DEFAULT_FUND_TIMEOUT = 180.0

_USER_AGENT = "ponk-python/" + __version__


class PonkClient:
    """Construct with the base URL and your API key.

    Args:
        api_key: a `ponk_live_...` secret from Settings, API keys. Leave it out
            to reach only the endpoints that need no key (`health`, `pool`,
            `range_cost`, `ponk_perks`); every other method will then get a
            401 from the server.
        base_url: defaults to production. Point it elsewhere to test.
        timeout: seconds for read and control calls.
        fund_timeout: seconds for compound, withdraw and exit.
        opener: a `urllib.request.OpenerDirector` to send through, for a proxy
            or for tests.
    """

    def __init__(
        self,
        api_key: Optional[str] = None,
        base_url: str = DEFAULT_BASE_URL,
        *,
        timeout: float = DEFAULT_TIMEOUT,
        fund_timeout: float = DEFAULT_FUND_TIMEOUT,
        opener: Optional[urllib.request.OpenerDirector] = None,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.timeout = timeout
        self.fund_timeout = fund_timeout
        self._opener = opener or urllib.request.build_opener()

    # -- transport ---------------------------------------------------------

    def _request(
        self,
        method: str,
        path: str,
        *,
        body: Optional[Mapping[str, Any]] = None,
        params: Optional[Mapping[str, Any]] = None,
        timeout: Optional[float] = None,
        allow_statuses: tuple = (),
    ) -> Any:
        url = self.base_url + path
        if params:
            query = {k: v for k, v in params.items() if v is not None}
            if query:
                url = url + "?" + urllib.parse.urlencode(query)

        data = None
        headers = {"Accept": "application/json", "User-Agent": _USER_AGENT}
        if body is not None:
            data = json.dumps(body).encode("utf-8")
            headers["Content-Type"] = "application/json"
        if self.api_key:
            headers["Authorization"] = "Bearer " + self.api_key

        request = urllib.request.Request(url, data=data, headers=headers, method=method)
        try:
            with self._opener.open(request, timeout=timeout or self.timeout) as response:
                return _decode(response.read())
        except urllib.error.HTTPError as exc:  # a real response, non-2xx
            payload = _decode(exc.read())
            # A status the caller declared meaningful, carrying a real body
            # rather than the error envelope, is a result and not a failure.
            # `/health` answers 503 with the full report of what is down.
            if (
                exc.code in allow_statuses
                and isinstance(payload, Mapping)
                and "error" not in payload
            ):
                return payload
            raise error_from_response(exc.code, payload) from None
        except urllib.error.URLError as exc:
            raise PonkTransportError("{0} {1} failed: {2}".format(method, url, exc.reason)) from exc
        except socket.timeout as exc:
            raise PonkTransportError(
                "{0} {1} timed out after {2}s".format(method, url, timeout or self.timeout)
            ) from exc

    # -- public, no key required -------------------------------------------

    def health(self) -> Health:
        """`GET /health`. Whether the API and its database and RPC are up.

        A degraded API answers 503 with the full report of which component is
        down, and that report is the point of the call, so this returns it
        rather than raising. Check `status` (`ok` or `degraded`) and
        `checks.database.healthy` / `checks.helius.healthy`.
        """
        return Health.from_dict(self._request("GET", "/health", allow_statuses=(503,)))

    def pool(self, dex: str, address: str) -> PoolSnapshot:
        """`GET /pools/{dex}/{address}`. One pool, decoded from chain.

        `dex` is `meteora_dlmm`, `orca`, `ponk_clouds` or `raydium`. This
        route is rate limited per IP (a 429 raises `PonkRateLimitedError`): it
        spends the same RPC budget the live agents use.
        """
        return PoolSnapshot.from_dict(
            self._request("GET", "/pools/{0}/{1}".format(_seg(dex), _seg(address)))
        )

    def range_cost(self, dex: str, address: str, bins: int) -> RangeCost:
        """`GET /pools/{dex}/{address}/range-cost?bins=N`. What a range costs.

        `bins` is the TOTAL width, centred on the active bin. The quote splits
        the SOL a wallet must hold before opening into the refundable position
        rent and the non-refundable bin-array rent plus signature fees, with
        the bin arrays that already exist probed on chain.

        Meteora DLMM only: any other `dex` raises `PonkUnprocessableError`
        rather than returning an approximation. `bins=0` raises
        `PonkBadRequestError`. A request wider than the venue allows is
        clamped, and `clamped` / `max_total_bins` on the result say so. Rate
        limited per IP, like `pool`.
        """
        return RangeCost.from_dict(
            self._request(
                "GET",
                "/pools/{0}/{1}/range-cost".format(_seg(dex), _seg(address)),
                params={"bins": bins},
            )
        )

    def ponk_perks(self, wallet_address: str) -> PonkPerks:
        """`GET /public/ponk/perks/{address}`. What fees a wallet pays.

        Reads only the wallet's public $PONK balance. Works for any wallet, no
        key and no signature.
        """
        return PonkPerks.from_dict(
            self._request("GET", "/public/ponk/perks/{0}".format(_seg(wallet_address)))
        )

    def leaderboard(self, window: Optional[str] = None) -> Leaderboard:
        """`GET /public/agents-leaderboard?window=`. Top Ponkillionaires.

        Every agent its owner chose to share, ranked against holding. `window`
        is `24h`, `7d`, `30d` or `all` (server default when omitted). No key
        needed; rate limited per IP. Each row carries `copy`, so a row can be
        copied with `copy_agent` without a second read.
        """
        return Leaderboard.from_dict(
            self._request("GET", "/public/agents-leaderboard", params={"window": window})
        )

    def public_agent(self, slug: str) -> PublicAgent:
        """`GET /public/agents/{slug}`. A shared agent's public profile.

        No key needed. Raises `PonkNotFoundError` for an unknown slug; a slug
        whose owner stopped sharing it answers 410, raised as `PonkAPIError`.
        """
        return PublicAgent.from_dict(
            self._request("GET", "/public/agents/{0}".format(_seg(slug)))
        )

    def public_agent_value_history(self, slug: str, range: Optional[str] = None) -> ValueHistory:
        """`GET /public/agents/{slug}/value-history?range=`. The profile's chart.

        `range` is `24h`, `7d`, `30d` or `all`. A link that hides USD answers
        404, because every point is a dollar figure. No key needed.
        """
        return ValueHistory.from_dict(
            self._request(
                "GET",
                "/public/agents/{0}/value-history".format(_seg(slug)),
                params={"range": range},
            )
        )

    def public_points_leaderboard(
        self, limit: Optional[int] = None, offset: Optional[int] = None
    ) -> PointsLeaderboard:
        """`GET /public/points/leaderboard`. The points season board, signed out.

        Handles are referral codes, never wallets; `is_me` is always false
        here. `limit` is clamped to 1 to 100 on the server.
        """
        return PointsLeaderboard.from_dict(
            self._request(
                "GET", "/public/points/leaderboard", params={"limit": limit, "offset": offset}
            )
        )

    def resolve_referral(self, code: str) -> PublicReferral:
        """`GET /public/referral/{code}`. Whether a referral code or handle exists.

        Raises `PonkNotFoundError` when it does not. Reveals no wallet and no
        figure. No key needed.
        """
        return PublicReferral.from_dict(
            self._request("GET", "/public/referral/{0}".format(_seg(code)))
        )

    # -- identity ----------------------------------------------------------

    def whoami(self) -> WhoAmI:
        """`GET /v1/me`. The first call any integration should make."""
        return WhoAmI.from_dict(self._request("GET", "/v1/me"))

    # -- agents, read ------------------------------------------------------

    def list_agents(self) -> List[Agent]:
        """`GET /v1/agents`. Every agent this key's wallet owns."""
        return Agent.from_list(self._request("GET", "/v1/agents"))

    def get_agent(self, agent_id: str) -> Agent:
        """`GET /v1/agents/{id}`. A foreign or missing id is 404 alike."""
        return Agent.from_dict(self._request("GET", "/v1/agents/{0}".format(_seg(agent_id))))

    def agent_performance(self, agent_id: str) -> AgentPerformance:
        """`GET /v1/agents/{id}/performance`. Live value, PnL, fees, IL."""
        return AgentPerformance.from_dict(
            self._request("GET", "/v1/agents/{0}/performance".format(_seg(agent_id)))
        )

    def agent_position(self, agent_id: str) -> AgentPosition:
        """`GET /v1/agents/{id}/position`. The live on-chain range."""
        return AgentPosition.from_dict(
            self._request("GET", "/v1/agents/{0}/position".format(_seg(agent_id)))
        )

    def agent_wallet(self, agent_id: str) -> AgentWallet:
        """`GET /v1/agents/{id}/wallet`. The agent's own managed wallet.

        Autonomous agents only.
        """
        return AgentWallet.from_dict(
            self._request("GET", "/v1/agents/{0}/wallet".format(_seg(agent_id)))
        )

    def agent_mandate(self, agent_id: str) -> Mandate:
        """`GET /v1/agents/{id}/mandate`. The custody mandate the agent signs under.

        What ponk may do for this agent, the caps, and when that authority
        runs out. Lapsing is a pure clock fact that nothing on chain records,
        so poll this rather than waiting for a failure:

            m = ponk.agent_mandate(agent_id)
            if m.seconds_remaining is not None and m.seconds_remaining < 3 * 86400:
                warn("renew the mandate in the app", m.expires_at)

        Raises `PonkNotFoundError` when the agent has no managed wallet or no
        active mandate, which means "not autonomous". Any key scope may read
        it.
        """
        return Mandate.from_dict(
            self._request("GET", "/v1/agents/{0}/mandate".format(_seg(agent_id)))
        )

    def list_positions(self) -> List[Position]:
        """`GET /v1/positions`. Every LP position the wallet holds.

        Agent-managed or not, re-read from chain on each call.
        """
        return Position.from_list(self._request("GET", "/v1/positions"))

    def list_logs(self, limit: Optional[int] = None) -> List[ActionLog]:
        """`GET /v1/logs`. Recent agent activity, newest first.

        `limit` defaults to 100 on the server and is clamped to 1 to 500.
        """
        return ActionLog.from_list(
            self._request("GET", "/v1/logs", params={"limit": limit})
        )

    def agent_value_history(self, agent_id: str, range: Optional[str] = None) -> ValueHistory:
        """`GET /v1/agents/{id}/value-history?range=`. Value against holding.

        The position's value per bucket next to what its opening tokens would
        be worth held. `range` is `24h`, `7d` (server default), `30d` or `all`.
        A bucket with no reading is `None`, never filled in.
        """
        return ValueHistory.from_dict(
            self._request(
                "GET",
                "/v1/agents/{0}/value-history".format(_seg(agent_id)),
                params={"range": range},
            )
        )

    def agent_decisions(
        self, agent_id: str, hours: Optional[int] = None, limit: Optional[int] = None
    ) -> Dict[str, Any]:
        """`GET /v1/agents/{id}/decisions?hours&limit`. Why the agent acted or held.

        The server-computed agent state, what it evaluated, what it declined
        and why, and which economics basis ran. Returned as the decoded JSON
        object. `hours` defaults to 24 (1 to 720), `limit` to 50.
        """
        return self._request(
            "GET",
            "/v1/agents/{0}/decisions".format(_seg(agent_id)),
            params={"hours": hours, "limit": limit},
        )

    def get_public_link(self, agent_id: str) -> PublicLink:
        """`GET /v1/agents/{id}/public-link`. The agent's live leaderboard link.

        Raises `PonkNotFoundError` when the agent is not shared.
        """
        return PublicLink.from_dict(
            self._request("GET", "/v1/agents/{0}/public-link".format(_seg(agent_id)))
        )

    def list_follows(self) -> Follows:
        """`GET /v1/follows`. Slugs of the shared agents this account follows."""
        return Follows.from_dict(self._request("GET", "/v1/follows"))

    def points(self) -> MyPoints:
        """`GET /v1/points`. This account's standing in the active points season."""
        return MyPoints.from_dict(self._request("GET", "/v1/points"))

    def points_leaderboard(
        self, limit: Optional[int] = None, offset: Optional[int] = None
    ) -> PointsLeaderboard:
        """`GET /v1/points/leaderboard`. The season board with your row marked `is_me`."""
        return PointsLeaderboard.from_dict(
            self._request("GET", "/v1/points/leaderboard", params={"limit": limit, "offset": offset})
        )

    def referral(self) -> Referral:
        """`GET /v1/referral`. Codes, handle, referred count and referral money.

        Earned, owed and unproven are separate facts; never add them.
        """
        return Referral.from_dict(self._request("GET", "/v1/referral"))

    def creator_dashboard(self, days: Optional[int] = None) -> CreatorDashboard:
        """`GET /v1/referral/creator?days=`. The Creator dashboard by UTC day.

        `days` defaults to 30 (clamped 7 to 90). Raises `PonkNotFoundError`
        for an account without the Creator tier; claiming a handle grants it.
        """
        return CreatorDashboard.from_dict(
            self._request("GET", "/v1/referral/creator", params={"days": days})
        )

    def check_handle(self, handle: str) -> HandleCheck:
        """`GET /v1/referral/handle/check?handle=`. Can this account claim /r/<handle>."""
        return HandleCheck.from_dict(
            self._request("GET", "/v1/referral/handle/check", params={"handle": handle})
        )

    def get_json(self, path: str, params: Optional[Mapping[str, Any]] = None) -> Any:
        """Any GET this client does not name, decoded as raw JSON.

        The escape hatch for a field or a route added to the API after this
        version, so an integration is never blocked waiting for a release. The
        Rust client has carried the same method; this is the Python half of
        that parity.

        `path` starts with a slash and is appended to the base URL. It is sent
        verbatim, so build it yourself and quote anything that came from
        outside your own code.
        """
        return self._request("GET", path, params=params)

    # -- agents, write (needs a `trade` key) -------------------------------

    def create_agent(
        self,
        *,
        name: str,
        wallet_address: str,
        dex: str,
        strategy: str,
        config: Mapping[str, Any],
        pool_address: Optional[str] = None,
        position_address: Optional[str] = None,
        dry_run: bool,
    ) -> Agent:
        """`POST /v1/agents`. Create an agent.

        `wallet_address` must be this key's own wallet; anything else is
        rejected as a cross-account attempt. `config` is validated against
        `strategy`, so a mismatched shape comes back as a field-precise 400
        rather than a broken agent.

        An agent created here is stamped `origin='api'` and pays the API
        performance rate for its whole life, including after you enable
        autonomous mode for it in the app. `ponk_perks(wallet).api_agent_fee`
        is that rate.

        `dry_run` is REQUIRED here, although the server would accept it
        omitted, because the `/v1` default is the opposite of the app's: an
        omitted `dry_run` on `POST /v1/agents` creates a LIVE agent
        (`dry_run=false`). Pass `dry_run=True` for an agent that runs its whole
        strategy loop and logs every decision to `list_logs` without sending a
        transaction, or `dry_run=False` for a live one. Anything that is not a
        `bool` raises `TypeError` before a request is sent; the server refuses
        non-booleans too.

        A live agent created here is still non-custodial (`wallet_source`
        `external`, `mode` `manual`) until a mandate signed in the app makes it
        autonomous, so it can recommend but cannot sign for funds.

        For `strategy="entry_agent"` (Degen Mode) the agent starts with no
        pool: `pool_address` and `position_address` are ignored, and the
        config's `venues` must include one where ponk can discover pools,
        which today is only `meteora_dlmm`.
        Its config may carry a bin step filter, `min_bin_step` and
        `max_bin_step` (each 1 to 1000, max at least min), so the screener only
        enters pools inside that band.
        """
        if not isinstance(dry_run, bool):
            raise TypeError(
                "dry_run must be True or False, got {0!r}. On /v1 an omitted "
                "dry_run means a LIVE agent, so this client makes you say "
                "which one you want.".format(dry_run)
            )
        body: Dict[str, Any] = {
            "name": name,
            "wallet_address": wallet_address,
            "dex": dex,
            "strategy": strategy,
            "config": config,
            "dry_run": dry_run,
        }
        if pool_address is not None:
            body["pool_address"] = pool_address
        if position_address is not None:
            body["position_address"] = position_address
        return Agent.from_dict(self._request("POST", "/v1/agents", body=body))

    def pause_agent(self, agent_id: str) -> Agent:
        """`POST /v1/agents/{id}/pause`. Stop the loop.

        The position stays open and keeps earning. Nothing is closed or swept.
        """
        return Agent.from_dict(
            self._request("POST", "/v1/agents/{0}/pause".format(_seg(agent_id)))
        )

    def resume_agent(self, agent_id: str) -> Agent:
        """`POST /v1/agents/{id}/resume`. Restart the loop."""
        return Agent.from_dict(
            self._request("POST", "/v1/agents/{0}/resume".format(_seg(agent_id)))
        )

    def set_dry_run(self, agent_id: str, dry_run: bool) -> Agent:
        """`POST /v1/agents/{id}/dry-run`. Simulate instead of sending."""
        return Agent.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/dry-run".format(_seg(agent_id)),
                body={"dry_run": dry_run},
            )
        )

    def set_autonomous(self, agent_id: str, enabled: bool) -> AutonomousState:
        """`POST /v1/agents/{id}/autonomous`. Start or stop autonomous signing.

        `True` turns autonomous signing on USING the custody mandate the owner
        already signed in the app. It creates no mandate and cannot: the
        server requires the agent's own managed wallet with an active,
        unexpired mandate whose withdrawal address is still this account's
        wallet, and refuses with `PonkUnprocessableError` (and a sentence
        saying which) otherwise. Enabling also sets the agent live
        (`dry_run=false`) and sends the owner a notification naming "a ponk
        API key" as the credential that did it.

        `False` turns it off and is always allowed. It touches no funds: any
        position and balance stay in the managed wallet and stay withdrawable.
        It does NOT revoke the mandate, so `True` can re-arm the agent later
        while the mandate is unexpired.

        Only `{"enabled": ...}` is sent, never a `grant`; the server answers
        422 to a body carrying one. Read `status` on the result: enabling does
        not restart a `stopped` agent, which still needs `resume_agent`.
        """
        if not isinstance(enabled, bool):
            raise TypeError("enabled must be True or False, got {0!r}".format(enabled))
        return AutonomousState.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/autonomous".format(_seg(agent_id)),
                body={"enabled": enabled},
            )
        )

    def set_mode(self, agent_id: str, mode: str) -> Agent:
        """`POST /v1/agents/{id}/mode`. `manual` or `auto`."""
        return Agent.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/mode".format(_seg(agent_id)),
                body={"mode": mode},
            )
        )

    def update_config(self, agent_id: str, patch: Mapping[str, Any]) -> Agent:
        """`PATCH /v1/agents/{id}/config`. Change how an agent recenters.

        `patch` is a flat partial patch: a key with a value sets it, a key set
        to `None` clears it back to its default, an absent key is unchanged.
        On a bin rebalancer, conservative or volatility-aware agent the keys
        are `recenter_mode` (`recenter`, `ping_pong` or `extend`),
        `rebalance_direction`, `liquidity_shape`, `ping_pong_bid_bins`,
        `ping_pong_ask_bins`, `ping_pong_spread_bins`, `shape_below`,
        `shape_above`, `ratio_below_pct`, `rebalance_buffer`,
        `schedule_secs`, `cooldown_sec` (60 to 86400) and
        `max_rebalances_per_day` (1 to 96). `recenter_mode="keep_side"` with
        `keep_side` (`token_x` or `token_y`) keeps the position one-sided on
        that token. On Meteora, `reshape` (`{"shape", "trigger_bins",
        "buffer_bins"?, "alternate"?}`, trigger 1 to 51) re-deposits into the
        same position at N bins out instead of closing and reopening; `None`
        turns it off. Adaptive range recovery takes `rebalance_cooldown_sec`
        and `max_rebalances_per_day`. Any other key, the range width included, raises
        `PonkUnprocessableError` naming it. Moves no funds; applies to the
        next recenter.
        """
        return Agent.from_dict(
            self._request(
                "PATCH",
                "/v1/agents/{0}/config".format(_seg(agent_id)),
                body=dict(patch),
            )
        )

    def update_exits(
        self,
        agent_id: str,
        tp_sl: Optional[Mapping[str, Any]] = None,
        automation: Optional[Mapping[str, Any]] = None,
    ) -> Agent:
        """`POST /v1/agents/{id}/exits`. Change exits and fee handling.

        `tp_sl` and `automation` are partial patches with the same rule as
        `update_config` (`None` inside a block clears that key). A block left
        unset is not sent and stays as stored. Moves no funds.

        Besides `stop_loss_pct` and `take_profit_pct`, `tp_sl` takes
        `take_profit_market_cap_usd` (the volatile token's market cap),
        `take_profit_position_value_usd` (the position's two legs) and
        `take_profit_value_with_fees_usd` (legs plus unclaimed fees), each a
        positive USD figure.
        """
        body: Dict[str, Any] = {}
        if tp_sl is not None:
            body["tp_sl"] = dict(tp_sl)
        if automation is not None:
            body["automation"] = dict(automation)
        return Agent.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/exits".format(_seg(agent_id)),
                body=body,
            )
        )

    def set_auto_compound(
        self,
        agent_id: str,
        auto_compound: bool,
        min_claimable_base_units: Optional[int] = None,
        min_claimable_usd: Optional[float] = None,
    ) -> Agent:
        """`POST /v1/agents/{id}/auto-compound`. Turn auto-compound on or off.

        Unset floors are not sent: the base-units floor then defaults to 0
        (the gas dust gate) and the dollar floor stays off. Turning it on while
        the agent accumulates fees raises `PonkUnprocessableError`. Moves no
        funds.
        """
        body: Dict[str, Any] = {"auto_compound": auto_compound}
        if min_claimable_base_units is not None:
            body["compound_min_claimable_base_units"] = min_claimable_base_units
        if min_claimable_usd is not None:
            body["compound_min_claimable_usd"] = min_claimable_usd
        return Agent.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/auto-compound".format(_seg(agent_id)),
                body=body,
            )
        )

    def optimize_draft(self, draft: Mapping[str, Any]) -> Dict[str, Any]:
        """`POST /v1/agents/optimize-draft`. Rank settings for a draft agent.

        The same answer the app's builder shows, for a draft `create_agent`
        body. Returned as the decoded JSON object. Needs a `trade` key (the
        scope `create_agent` needs). Moves no funds and creates nothing.
        """
        return self._request("POST", "/v1/agents/optimize-draft", body=dict(draft))

    def copy_agent(
        self,
        source: Any,
        *,
        name: str,
        wallet_address: str,
        dry_run: bool,
        config_overrides: Optional[Mapping[str, Any]] = None,
    ) -> Agent:
        """Copy a shared agent: `create_agent` from its public `copy` block.

        `source` is a slug, a `PublicAgent` or a `LeaderboardRow`. A slug is
        read with `public_agent` first. The new agent is yours, on your
        wallet, with the same venue, pool, strategy and config; nothing of the
        owner's is shared. `config_overrides` is merged over the copied config
        (for example a narrower range). `dry_run` is required, as in
        `create_agent`.
        """
        if isinstance(source, str):
            source = self.public_agent(source)
        copy = getattr(source, "copy", None)
        if copy is None or not copy.strategy or not copy.dex:
            raise ValueError("that agent carries no copy block to start from")
        config: Dict[str, Any] = dict(copy.config or {})
        if config_overrides:
            config.update(config_overrides)
        return self.create_agent(
            name=name,
            wallet_address=wallet_address,
            dex=copy.dex,
            pool_address=copy.pool_address,
            strategy=copy.strategy,
            config=config,
            dry_run=dry_run,
        )

    def share_agent(
        self,
        agent_id: str,
        *,
        hide_usd: bool = False,
        show_onchain: Optional[bool] = None,
    ) -> PublicLink:
        """`POST /v1/agents/{id}/public-link`. Publish an agent to the leaderboard.

        Idempotent: one live link per agent, and calling again changes its
        privacy switches. `hide_usd=True` shares percentages only.
        `show_onchain` publishes the agent wallet, position and signatures;
        left as `None` it keeps what the link has (a new link starts without
        it), and it is always off while `hide_usd` is set. Ponk Clouds agents
        cannot be shared while Clouds is parked (`PonkUnprocessableError`).
        """
        body: Dict[str, Any] = {"hide_usd": hide_usd}
        if show_onchain is not None:
            body["show_onchain"] = show_onchain
        return PublicLink.from_dict(
            self._request("POST", "/v1/agents/{0}/public-link".format(_seg(agent_id)), body=body)
        )

    def unshare_agent(self, agent_id: str) -> LinkRevoked:
        """`DELETE /v1/agents/{id}/public-link`. Stop sharing. The slug stays dead."""
        return LinkRevoked.from_dict(
            self._request("DELETE", "/v1/agents/{0}/public-link".format(_seg(agent_id)))
        )

    def follow(self, slug: str) -> Follow:
        """`POST /v1/follows/{slug}`. Be told when a shared agent opens, rebalances or exits.

        Idempotent. Following your own agent raises `PonkUnprocessableError`.
        """
        return Follow.from_dict(self._request("POST", "/v1/follows/{0}".format(_seg(slug))))

    def unfollow(self, slug: str) -> Follow:
        """`DELETE /v1/follows/{slug}`. Stop following. Idempotent."""
        return Follow.from_dict(self._request("DELETE", "/v1/follows/{0}".format(_seg(slug))))

    def claim_handle(self, handle: str) -> Referral:
        """`POST /v1/referral/handle`. Claim or change this account's /r/<handle>.

        The old handle keeps working as a referral code, and a handle makes
        the account a Creator. Raises `PonkConflictError` when someone else
        owns it and `PonkBadRequestError` for a handle the rules refuse.
        """
        return Referral.from_dict(
            self._request("POST", "/v1/referral/handle", body={"handle": handle})
        )

    # -- agents, fund moving (needs a `trade` key) -------------------------

    def compound(self, agent_id: str) -> ActionReceipt:
        """`POST /v1/agents/{id}/compound`. Claim fees and redeposit them now.

        Meteora DLMM autonomous agents only. The range is read from chain, so
        the re-deposit cannot be redirected. Takes the fund timeout.

        RETURNING IS NOT SUCCEEDING. A refusal at signing time, most commonly
        an expired custody mandate but also a risk halt or a daily-cap
        exhaustion, is recorded on the action rather than raised: the call
        answers HTTP 200 and this method returns a receipt whose `status` is
        failed, whose `signature` is `None`, and whose `error_message` says
        why. So check the receipt, not the absence of an exception:

            r = ponk.compound(agent_id)
            if r.signature is None:
                # nothing reached the chain; r.error_message says why, and
                # r.risk_failed_checks is populated when the risk engine was
                # the one that refused.
                ...
        """
        return ActionReceipt.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/compound".format(_seg(agent_id)),
                timeout=self.fund_timeout,
            )
        )

    def withdraw(self, agent_id: str, lamports: Optional[int] = None) -> Withdrawal:
        """`POST /v1/agents/{id}/withdraw`. Move SOL out of the agent's wallet.

        Omit `lamports` to sweep everything spendable. The destination is the
        wallet that owns the agent and cannot be named by the request.
        """
        body: Dict[str, Any] = {}
        if lamports is not None:
            body["lamports"] = lamports
        return Withdrawal.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/withdraw".format(_seg(agent_id)),
                body=body,
                timeout=self.fund_timeout,
            )
        )

    def exit_agent(self, agent_id: str) -> Withdrawal:
        """`POST /v1/agents/{id}/exit`. The full stop.

        Halts the agent, closes its position and sweeps every token plus native
        SOL back to the owner's wallet. Idempotent: if a call times out the
        work keeps running on the server, and calling again continues it rather
        than double-sending.
        """
        return Withdrawal.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/exit".format(_seg(agent_id)),
                timeout=self.fund_timeout,
            )
        )


def _seg(value: str) -> str:
    """Percent-encode one path segment, so an id can never change the route."""
    return urllib.parse.quote(str(value), safe="")


def _decode(raw: bytes) -> Any:
    """Decode a body as JSON, falling back to the text the server actually sent."""
    if not raw:
        return None
    text = raw.decode("utf-8", errors="replace")
    try:
        return json.loads(text)
    except ValueError:
        return text
