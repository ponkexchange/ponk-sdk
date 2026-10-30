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

Fleets: many agents in one call
-------------------------------

`create_agents` creates many agents (each with its own wallet),
`fund_prepare` returns the UNSIGNED transfers that fund them all from your
wallet in as few transactions as fit (ten agents, one signature), `iter_fleet`
reads every agent with its wallet, mandate and value, and `exit_agents` /
`withdraw_agents` close them all at once. The funding transfers are signed by
your wallet on ponk.exchange (`FundPlan.sign_urls`) or in wallet software you
control; this client never signs and never asks for a private key.

The mandate expires, and that is the failure to design for
---------------------------------------------------------

A mandate carries a hard expiry, and the grant the app asks you to sign runs
365 days by default (30, 90 or 180 when you pick them). Lapsing is a pure clock fact: no transaction and nothing on
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
  It raises `PonkNotFoundError` when the agent has no mandate at all.
  `mandates()` reads every autonomous agent's mandate at once, the ones that
  need renewing first, with the one link that renews them all. ponk also
  reminds at 7 days, 2 days and on expiry (`mandate_expiring` and
  `mandate_expired` notifications, which a webhook registered from a
  signed-in session receives).
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
from typing import Any, Callable, Dict, Iterator, List, Mapping, Optional, Sequence

from ._version import __version__
from .errors import PonkError, PonkTransportError, error_from_response
from .models import (
    FollowState,
    MyFollowing,
    PeopleBoard,
    PeopleList,
    PersonPositions,
    Privacy,
    Profile,
    SocialFeed,
    TopCalls,
    TrackedLps,
    WalletPnl,
    XAccountCard,
    CopilotFleet,
    CopilotFleetSummary,
    CopilotParse,
    CopilotPlan,
    CopilotStart,
    ActionLog,
    AgentsActivity,
    CuratedWallets,
    PonkillionairesWeek,
    WalletActivity,
    WalletFeed,
    WalletSource,
    XCallers,
    XDirectory,
    XFeed,
    XSource,
    XToken,
    XTrending,
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
    Mandates,
    PeopleSearch,
    Recommendations,
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
    ExitSwapLeg,
    ExitToSol,
    AgentProof,
    AgentWalletCreated,
    FeeEvent,
    FeeSummary,
    Fleet,
    FleetBatch,
    FleetRow,
    FundPlan,
    Notification,
    NotifySettings,
    Portfolio,
    PortfolioHistory,
    Preset,
    RangeHistory,
    ReadAll,
    RebalanceReceipt,
    Recommendation,
    RiskEvent,
    Transaction,
    UnreadCount,
    OrderPlan,
    JupiterChallenge,
    JupiterSession,
    DepositCraft,
    OrderCreated,
    OrderCancel,
    OrderBatch,
    FollowStatus,
    CopyFollows,
    CopyLog,
    CopyReferralOffer,
    CopyReferralAttach,
    BatchResult,
    Prepared,
    MarkedTx,
    StrandedPositions,
    CallPreview,
    Call,
    PushSettings,
    PushTest,
    CardLook,
    LeaderboardPrivacy,
    AgentVisibility,
    LogsCleared,
    TelegramStatus,
    TelegramShared,
    PresetRank,
)

__all__ = [
    "PonkClient",
    "DEFAULT_BASE_URL",
    "DEFAULT_TIMEOUT",
    "DEFAULT_FUND_TIMEOUT",
    "limit_order",
    "tp_sl_order",
    "dca_order",
]

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
        session_token: a signed-in wallet session (Sign-In-With-Solana on
            ponk.exchange, `POST /auth/nonce` then `POST /auth/verify`). The
            copilot methods use it when present, otherwise the API key: a
            read-scope key can parse, plan, replay and read fleets, a
            trade-scope key can also start and stop them. Fleets started with
            a key pay the API fee (15%); a wallet session pays the web fee.
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
        session_token: Optional[str] = None,
        timeout: float = DEFAULT_TIMEOUT,
        fund_timeout: float = DEFAULT_FUND_TIMEOUT,
        opener: Optional[urllib.request.OpenerDirector] = None,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.session_token = session_token
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
        headers: Optional[Mapping[str, str]] = None,
        session_only: bool = False,
    ) -> Any:
        url = self.base_url + path
        if params:
            query = {k: v for k, v in params.items() if v is not None}
            if query:
                url = url + "?" + urllib.parse.urlencode(query)

        data = None
        extra_headers = headers
        headers = {"Accept": "application/json", "User-Agent": _USER_AGENT}
        if body is not None:
            data = json.dumps(body).encode("utf-8")
            headers["Content-Type"] = "application/json"
        if session_only:
            # The copilot routes: a wallet session when this client holds one,
            # otherwise the API key (read scope plans, trade scope starts).
            credential = self.session_token or self.api_key
            if not credential:
                raise PonkError(
                    "{0} {1} needs a wallet session or an API key: construct the "
                    "client with session_token= or api_key=. Nothing was "
                    "sent.".format(method, path)
                )
            headers["Authorization"] = "Bearer " + credential
        elif self.api_key:
            headers["Authorization"] = "Bearer " + self.api_key
        if extra_headers:
            headers.update(extra_headers)

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

    def agents_activity(self) -> AgentsActivity:
        """`GET /public/agents-activity`. What the board's agents are doing now.

        The confirmed actions (open, rebalance, compound, claim, exit) of the
        last 24 hours and every live hold with the reason the agent is holding
        and how many times it has checked. Same agents and same privacy as the
        leaderboard; no event carries an agent id, a slug, a name or a wallet.
        No key needed.
        """
        return AgentsActivity.from_dict(self._request("GET", "/public/agents-activity"))

    def ponkillionaires_week(self) -> PonkillionairesWeek:
        """`GET /public/ponkillionaires/week`. This week's race and last week's winners.

        The top three by profit win a lower agent fee rate for the following
        week plus points (`prizes`). Owners appear under the identity they
        chose on the board. No key needed.
        """
        return PonkillionairesWeek.from_dict(
            self._request("GET", "/public/ponkillionaires/week")
        )

    # -- People, public ----------------------------------------------------

    def profile(self, key: str) -> Profile:
        """`GET /public/people/{key}`. A public profile.

        `key` is an @handle (with or without the @), any /r/ referral code, a
        shown wallet, or a `t-<owner_key>` tag. A private account answers with
        its handle, bio, X and picture only. Raises `PonkNotFoundError` for a
        key that names nothing public. No key needed.
        """
        return Profile.from_dict(
            self._request("GET", "/public/people/{0}".format(_seg(key.lstrip("@"))))
        )

    def followers(self, key: str, cursor: Optional[str] = None) -> PeopleList:
        """`GET /public/people/{key}/followers`. Who follows this person.

        Private followers are counted, never named. Page with `next_cursor`.
        """
        return PeopleList.from_dict(
            self._request(
                "GET", "/public/people/{0}/followers".format(_seg(key.lstrip("@"))), params={"cursor": cursor}
            )
        )

    def following(self, key: str, cursor: Optional[str] = None) -> PeopleList:
        """`GET /public/people/{key}/following`. Who this person follows."""
        return PeopleList.from_dict(
            self._request(
                "GET", "/public/people/{0}/following".format(_seg(key.lstrip("@"))), params={"cursor": cursor}
            )
        )

    def person_activity(
        self,
        key: str,
        *,
        cursor: Optional[str] = None,
        kinds: Optional[str] = None,
        pnl_only: bool = False,
    ) -> SocialFeed:
        """`GET /public/people/{key}/activity`. One person's agent, LP, call and
        (when they share them) swap activity, newest first.

        `kinds` is `all`, `agents`, `lp`, `swaps` or `calls`; `pnl_only` keeps
        closes and exits with a P&L.
        """
        return SocialFeed.from_dict(
            self._request(
                "GET",
                "/public/people/{0}/activity".format(_seg(key.lstrip("@"))),
                params={"cursor": cursor, "kinds": kinds, "pnl": "true" if pnl_only else None},
            )
        )

    def person_positions(self, key: str) -> PersonPositions:
        """`GET /public/people/{key}/positions`. A person's open positions on
        Meteora, Orca and Raydium, each with its Copy action."""
        return PersonPositions.from_dict(
            self._request("GET", "/public/people/{0}/positions".format(_seg(key.lstrip("@"))))
        )

    def people_search(self, q: str, cursor: Optional[str] = None) -> PeopleSearch:
        """`GET /public/people-search?q=&cursor=`. Ponk accounts by @handle
        (the @ is optional), creator handle, /r/ code or display name, from the
        first letter, case-insensitive.

        An exact handle ranks first, then names that start with `q`, then names
        that contain it, the most followed first. Twenty a page: pass
        `next_cursor` back as `cursor`. No key needed.
        """
        return PeopleSearch.from_dict(
            self._request("GET", "/public/people-search", params={"q": q, "cursor": cursor})
        )

    def recommended_people(self, limit: Optional[int] = None) -> Recommendations:
        """`GET /public/social/recommended?limit=`. Who to follow, with the
        reason for each, for a reader with no account. `limit` is 1 to 30
        (default 10). `my_recommended` is the same for your own account."""
        return Recommendations.from_dict(
            self._request("GET", "/public/social/recommended", params={"limit": limit})
        )

    def similar_people(self, key: str, limit: Optional[int] = None) -> Recommendations:
        """`GET /public/people/{key}/similar?limit=`. Traders like this one:
        the same pools in the last 30 days, and the people this person's
        followers also follow. Never the person themself."""
        return Recommendations.from_dict(
            self._request(
                "GET", "/public/people/{0}/similar".format(_seg(key.lstrip("@"))), params={"limit": limit}
            )
        )

    def people_board(
        self, window: Optional[str] = None, sort: Optional[str] = None, limit: Optional[int] = None
    ) -> PeopleBoard:
        """`GET /public/people-leaderboard`. Ponk accounts ranked over `window`
        (`24h`, `7d`, `30d`) by P&L, with ROI and vs HODL beside it."""
        return PeopleBoard.from_dict(
            self._request(
                "GET", "/public/people-leaderboard", params={"window": window, "sort": sort, "limit": limit}
            )
        )

    def live_feed(
        self, *, cursor: Optional[str] = None, kinds: Optional[str] = None, pnl_only: bool = False
    ) -> SocialFeed:
        """`GET /public/social/feed`. Activity of every public account, newest
        first (the signed-out Following tab)."""
        return SocialFeed.from_dict(
            self._request(
                "GET",
                "/public/social/feed",
                params={"cursor": cursor, "kinds": kinds, "pnl": "true" if pnl_only else None},
            )
        )

    # -- X Tracker, public -------------------------------------------------

    def x_feed(
        self,
        *,
        before: Optional[str] = None,
        since: Optional[str] = None,
        limit: Optional[int] = None,
        category: Optional[str] = None,
        ca_only: bool = False,
        min_followers: Optional[int] = None,
        account: Optional[str] = None,
        q: Optional[str] = None,
        first_calls: bool = False,
    ) -> XFeed:
        """`GET /public/x/feed`. The curated X accounts' posts and events, newest first.

        `before` (RFC 3339) pages back: pass the previous page's
        `next_before`. `since` returns only what was seen after that time.
        `limit` is 1 to 100 (default 50). `category` is a comma-separated list
        of directory categories (`x_accounts().categories`). `ca_only` keeps
        posts that name a token. `account` is one handle, `q` a text search.
        `first_calls` keeps only posts that were the first call of a token.
        """
        return XFeed.from_dict(
            self._request(
                "GET",
                "/public/x/feed",
                params={
                    "before": before,
                    "since": since,
                    "limit": limit,
                    "category": category,
                    "ca": "1" if ca_only else None,
                    "minFollowers": min_followers,
                    "account": account,
                    "q": q,
                    "first": "1" if first_calls else None,
                },
            )
        )

    def x_trending(self, window: Optional[str] = None, limit: Optional[int] = None) -> XTrending:
        """`GET /public/x/trending?window=&limit=`. Tokens trending on X.

        `window` is `1h` (default) or `24h`; `limit` is 1 to 50 (default 25).
        Each token carries its first caller and the multiple since that call.
        """
        return XTrending.from_dict(
            self._request("GET", "/public/x/trending", params={"window": window, "limit": limit})
        )

    def x_callers(self, days: Optional[int] = None) -> XCallers:
        """`GET /public/x/callers?days=`. The caller leaderboard, up to 50 accounts.

        `days` is 1 to 90 (default 30). Ranked by median 24h multiple, then by
        calls.
        """
        return XCallers.from_dict(
            self._request("GET", "/public/x/callers", params={"days": days})
        )

    def calls_feed(
        self,
        source: Optional[str] = None,
        mint: Optional[str] = None,
        wallet: Optional[str] = None,
        before: Optional[str] = None,
        limit: Optional[int] = None,
    ) -> Any:
        """`GET /public/calls`. The Ponk Calls feed, newest first. `source` is
        `ponk` (default), `x` or `all`; `before` an RFC 3339 cursor. Public."""
        return self._request(
            "GET",
            "/public/calls",
            params={"source": source, "mint": mint, "wallet": wallet, "before": before, "limit": limit},
        )

    def callers_board(self, days: Optional[int] = None, source: Optional[str] = None) -> Any:
        """`GET /public/calls/callers`. The callers board, ranked by median 24h
        multiple, then calls. Public."""
        return self._request("GET", "/public/calls/callers", params={"days": days, "source": source})

    def caller(self, who: str) -> Any:
        """`GET /public/calls/caller/{who}`. One caller's profile and calls. Public."""
        return self._request("GET", "/public/calls/caller/" + _seg(who))

    def call(self, call_id: str) -> Any:
        """`GET /public/calls/{id}`. One call with its outcome so far. Public."""
        return self._request("GET", "/public/calls/" + _seg(call_id))

    def x_account(self, handle: str, days: Optional[int] = None) -> XAccountCard:
        """`GET /public/x/account/{handle}`. One X account's caller card: calls,
        multiples, hit rate, 2x rate, first calls, rank and recent calls over
        `days` (default 30)."""
        return XAccountCard.from_dict(
            self._request(
                "GET", "/public/x/account/{0}".format(_seg(handle.lstrip("@"))), params={"days": days}
            )
        )

    def top_calls(
        self, days: Optional[int] = None, limit: Optional[int] = None, source: Optional[str] = None
    ) -> TopCalls:
        """`GET /public/calls/top`. The best calls of the last `days` (default 7,
        up to 90), ranked by their multiple since the call. `source` is `ponk`
        (default), `x` or `all`."""
        return TopCalls.from_dict(
            self._request("GET", "/public/calls/top", params={"days": days, "limit": limit, "source": source})
        )

    def x_token(self, mint: str) -> XToken:
        """`GET /public/x/token/{mint}`. Every tracked mention of one token.

        Raises `PonkBadRequestError` for a string that is not a Solana address.
        """
        return XToken.from_dict(
            self._request("GET", "/public/x/token/{0}".format(_seg(mint)))
        )

    def x_accounts(self) -> XDirectory:
        """`GET /public/x/accounts`. The curated account directory and its categories."""
        return XDirectory.from_dict(self._request("GET", "/public/x/accounts"))

    def x_status(self) -> XSource:
        """`GET /public/x/status`. The feed source's state: `live`, `degraded`, `down` or `starting`."""
        return XSource.from_dict(self._request("GET", "/public/x/status"))

    # -- Wallet Tracker, public --------------------------------------------

    def wallets_feed(
        self,
        *,
        before: Optional[str] = None,
        limit: Optional[int] = None,
        kind: Optional[str] = None,
        min_usd: Optional[float] = None,
        token: Optional[str] = None,
        wallet: Optional[str] = None,
    ) -> WalletFeed:
        """`GET /public/wallets/feed`. The curated wallets' swaps and LP actions, merged.

        `before` is the previous page's `next_before`. `kind` is a
        comma-separated filter: `buy`, `sell`, `swap`, `lp` (every liquidity
        kind) or any single kind. `min_usd` drops smaller rows, `token` keeps
        rows touching one mint, `wallet` keeps one wallet.
        """
        return WalletFeed.from_dict(
            self._request(
                "GET",
                "/public/wallets/feed",
                params={
                    "before": before,
                    "limit": limit,
                    "kind": kind,
                    "minUsd": min_usd,
                    "token": token,
                    "wallet": wallet,
                },
            )
        )

    def wallet_pnl(self, address: str) -> WalletPnl:
        """`GET /public/wallets/{address}/pnl`. Trading P&L per token for any
        wallet, from its swaps. No key needed."""
        return WalletPnl.from_dict(
            self._request("GET", "/public/wallets/{0}/pnl".format(_seg(address)))
        )

    def tracked_lps(self) -> TrackedLps:
        """`GET /public/wallets/positions`. Open LP positions of the curated
        wallets on Meteora, Orca and Raydium."""
        return TrackedLps.from_dict(self._request("GET", "/public/wallets/positions"))

    def wallets_defaults(self) -> CuratedWallets:
        """`GET /public/wallets/defaults`. The curated wallet list and where each comes from."""
        return CuratedWallets.from_dict(self._request("GET", "/public/wallets/defaults"))

    def wallets_status(self) -> WalletSource:
        """`GET /public/wallets/status`. The chain reader's state: `live`, `slowed` or `starting`."""
        return WalletSource.from_dict(self._request("GET", "/public/wallets/status"))

    def wallet_activity(
        self,
        address: str,
        *,
        before: Optional[str] = None,
        limit: Optional[int] = None,
        kind: Optional[str] = None,
        min_usd: Optional[float] = None,
        token: Optional[str] = None,
    ) -> WalletActivity:
        """`GET /public/wallets/{address}/activity`. Any wallet's recent swaps and LP actions.

        A wallet the tracker already stores is served from there
        (`mode == "tracked"`, pageable with `before`). Any other wallet is read
        from chain for this request (`mode == "live"`, one page). Rate limited
        per IP. Filters as in `wallets_feed`.
        """
        return WalletActivity.from_dict(
            self._request(
                "GET",
                "/public/wallets/{0}/activity".format(_seg(address)),
                params={
                    "before": before,
                    "limit": limit,
                    "kind": kind,
                    "minUsd": min_usd,
                    "token": token,
                },
            )
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

    def mandates(self) -> Mandates:
        """`GET /v1/agents/mandates`. Every autonomous agent's mandate, the
        ones that need renewing first.

        Each row's `state` is `active`, `expiring` (inside
        `expiring_within_days`), `expired` or `none` (never signed), with its
        own `renew_url`. `renew_all_url` is the one page where your wallet signs
        every renewal in turn, `None` when nothing needs it. Reads only: a
        mandate is never extended without your wallet's signature.
        """
        return Mandates.from_dict(self._request("GET", "/v1/agents/mandates"))

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
        """`GET /v1/referral`. Codes, handle, referred count, referral money and tier.

        Earned, owed and unproven are separate facts; never add them. `tier`
        is your current share of ponk's platform fee and the next rung;
        `programme.ladder` is the whole ladder (one level: 20%, 25% from
        $1,000 and 30% from $5,000 of referred platform fees over 30 days).
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
        accept_pool_risk: bool = False,
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

        The deploy gate refuses a pool that is young (under 30 days), thin
        (under $50K liquidity), quiet (under $10K 24h volume) or unsettled (a
        100% move) with a 422 that names the reason. Those checks are advice:
        `accept_pool_risk=True` creates the agent on that pool anyway, after
        you have read the reason. Anything that would make the transaction
        itself fail still refuses.
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
        if accept_pool_risk:
            body["accept_pool_risk"] = True
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

    def follow_person(self, key: str) -> FollowState:
        """`POST /v1/people/{key}/follow`. Follow a person: their agent, LP,
        call and shared swap activity lands in your Following feed, and you are
        told in the bell and on Telegram when linked. Idempotent. Needs a
        `trade` key."""
        return FollowState.from_dict(
            self._request("POST", "/v1/people/{0}/follow".format(_seg(key.lstrip("@"))))
        )

    def unfollow_person(self, key: str) -> FollowState:
        """`DELETE /v1/people/{key}/follow`. Stop following. Idempotent. Needs a
        `trade` key."""
        return FollowState.from_dict(
            self._request("DELETE", "/v1/people/{0}/follow".format(_seg(key.lstrip("@"))))
        )

    def my_recommended(self, limit: Optional[int] = None) -> Recommendations:
        """`GET /v1/social/recommended?limit=`. Who to follow, for your
        account: also ranked by who the people you follow follow, leaving out
        who you already follow or dismissed."""
        return Recommendations.from_dict(
            self._request("GET", "/v1/social/recommended", params={"limit": limit})
        )

    def dismiss_recommended(self, key: str) -> Dict[str, Any]:
        """`POST /v1/social/recommended/{key}/dismiss`. Never suggest this
        person again. Answers `{"key", "dismissed": true}`. Needs a `trade`
        key."""
        return self._request(
            "POST", "/v1/social/recommended/{0}/dismiss".format(_seg(key.lstrip("@")))
        )

    def my_following(self) -> MyFollowing:
        """`GET /v1/me/following`. Everyone you follow, private accounts
        included, plus the shared agents you follow one by one."""
        return MyFollowing.from_dict(self._request("GET", "/v1/me/following"))

    def following_feed(
        self, *, cursor: Optional[str] = None, kinds: Optional[str] = None, pnl_only: bool = False
    ) -> SocialFeed:
        """`GET /v1/social/feed`. Your Following feed, newest first. `kinds` is
        `all`, `agents`, `lp`, `swaps` or `calls`; `pnl_only` keeps closes and
        exits with a P&L. Page with `next_cursor`."""
        return SocialFeed.from_dict(
            self._request(
                "GET",
                "/v1/social/feed",
                params={"cursor": cursor, "kinds": kinds, "pnl": "true" if pnl_only else None},
            )
        )

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

    def exit_agent(self, agent_id: str, *, to_sol: Optional[bool] = None) -> Withdrawal:
        """`POST /v1/agents/{id}/exit`. The full stop.

        Halts the agent, closes its position and sweeps every token plus native
        SOL back to the owner's wallet. Idempotent: if a call times out the
        work keeps running on the server, and calling again continues it rather
        than double-sending.

        `to_sol=True` swaps the closed position's leftover tokens to SOL before
        the sweep (ponk fee 0.2% of the swapped amount); `to_sol=False` sends
        them as they are, no swap, no fee. Omitted, the agent's own
        `exit_to_sol` setting decides, and it defaults to True. A token that
        cannot be swapped is sent as it is; `Withdrawal.exit_notes` says why.
        """
        return Withdrawal.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/exit".format(_seg(agent_id)),
                body=_to_sol_body(to_sol),
                timeout=self.fund_timeout,
            )
        )

    def get_exit_to_sol(self, agent_id: str) -> ExitToSol:
        """`GET /v1/agents/{id}/exit-to-sol`. Whether this agent's exits swap
        leftover tokens to SOL (default True)."""
        return ExitToSol.from_dict(
            self._request("GET", "/v1/agents/{0}/exit-to-sol".format(_seg(agent_id)))
        )

    def set_exit_to_sol(self, agent_id: str, exit_to_sol: bool) -> ExitToSol:
        """`POST /v1/agents/{id}/exit-to-sol`. Turn the exit-to-SOL swap on or off.

        Automatic exits (take-profit, stop-loss, price stop, drawdown kill,
        bleed exit, custom kill rule, Degen Mode) honour it and leave the SOL
        in the agent wallet; a manual exit that does not pass `to_sol` uses it.
        Moves no funds. Needs a `trade` key.
        """
        if not isinstance(exit_to_sol, bool):
            raise TypeError("exit_to_sol must be a bool")
        return ExitToSol.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/exit-to-sol".format(_seg(agent_id)),
                body={"exit_to_sol": exit_to_sol},
            )
        )

    # -- the agent fleet (0.6.0) ---------------------------------------------

    def create_agents(
        self,
        agents: Sequence[Mapping[str, Any]],
        *,
        create_wallets: bool = True,
    ) -> FleetBatch:
        """`POST /v1/agents/batch`. Create many agents in one call.

        Each element is a `create_agent` body (`name`, `dex`, `strategy`,
        `config`, `pool_address`, `dry_run`, and `accept_pool_risk` to deploy
        past the pool checks). `wallet_address` may be left
        out: the server fills in the key's own wallet, the only value a create
        accepts. Each agent is created on its own through the same handler as
        `create_agent`, so one bad pool refuses only that item.

        `dry_run` is REQUIRED on every element, as in `create_agent`, because
        an omitted `dry_run` on `/v1` creates a LIVE agent. An element without
        a boolean `dry_run` raises `TypeError` before anything is sent.

        `create_wallets=True` (the default) also makes each new agent's own
        managed wallet, so the fleet is ready for `fund_prepare` in one step.
        A wallet that could not be made is reported on its item
        (`result["wallet_error"]`) and never undoes the create.

        Read the returned `FleetBatch` per item. `item.agent()` is the created
        agent; a `running` item is still going and `get_batch` collects it.
        Needs a `trade` key. Takes the fund timeout.
        """
        bodies: List[Dict[str, Any]] = []
        for i, a in enumerate(agents):
            body = dict(a)
            if not isinstance(body.get("dry_run"), bool):
                raise TypeError(
                    "agents[{0}] needs dry_run=True or False, got {1!r}. On /v1 an "
                    "omitted dry_run means a LIVE agent, so this client makes you "
                    "say which one you want.".format(i, body.get("dry_run"))
                )
            bodies.append(body)
        if not bodies:
            raise ValueError("agents must list at least one agent")
        return FleetBatch.from_dict(
            self._request(
                "POST",
                "/v1/agents/batch",
                body={"agents": bodies, "create_wallets": create_wallets},
                timeout=self.fund_timeout,
            )
        )

    def fund_prepare(self, transfers: Sequence[Any]) -> FundPlan:
        """`POST /v1/agents/fund/prepare`. Build the transfers that fund many agents.

        Each element is an agent id (top its wallet up to the venue's
        recommended opening balance), or a mapping
        `{"agent_id": ..., "lamports": ...}` for an exact amount. List each
        agent once.

        The server resolves every destination itself, under the owner scope:
        a request names agent ids, never addresses. It returns UNSIGNED
        SystemProgram transfers from the key's own wallet, packed into as few
        transactions as fit (ten agents fit in one, so one signature), each
        simulated against mainnet first.

        THIS CLIENT NEVER SIGNS, and neither does the server: it holds no key
        for your wallet. Open each of `plan.sign_urls` on ponk.exchange with
        your wallet connected and sign there, or sign
        `plan.transactions[i].unsigned_tx_b64` in wallet software you already
        control. `plan.needs_mandate` lists the agents that still need a
        custody mandate signed in the app before they can act on the funds.

        A plan the server refuses (balance short, a failed simulation) comes
        back with `ok` false and `refusal` set, not as an exception. Needs a
        `trade` key.
        """
        items: List[Dict[str, Any]] = []
        for t in transfers:
            if isinstance(t, str):
                items.append({"agent_id": t})
            elif isinstance(t, Mapping):
                item: Dict[str, Any] = {"agent_id": t["agent_id"]}
                if t.get("lamports") is not None:
                    item["lamports"] = int(t["lamports"])
                items.append(item)
            else:
                raise TypeError(
                    "each transfer is an agent id or {'agent_id': ..., 'lamports': ...}, "
                    "got {0!r}".format(t)
                )
        if not items:
            raise ValueError("transfers must list at least one agent")
        return FundPlan.from_dict(
            self._request("POST", "/v1/agents/fund/prepare", body={"transfers": items})
        )

    def exit_agents(
        self,
        agent_ids: Optional[Sequence[str]] = None,
        *,
        all_agents: bool = False,
        to_sol: Optional[bool] = None,
    ) -> FleetBatch:
        """`POST /v1/agents/batch/exit`. Exit many agents at once.

        Each listed agent is halted, its position closed and its wallet swept
        to the owner's own wallet, through the same detached, mandate-enforced
        exit as `exit_agent`. One agent failing does not stop the others.
        Pass `agent_ids`, or `all_agents=True` for every agent of this account
        that has a managed wallet; not both.

        The destination is locked to the owner's wallet by the server and
        cannot be named. The batch keeps running after the response, so a
        `running` item is not lost: poll `get_batch(batch.batch_id)`. Needs a
        `trade` key. Takes the fund timeout.

        `to_sol` applies to every agent of the batch, as in `exit_agent`;
        omitted, each agent's own `exit_to_sol` setting decides.
        """
        body = dict(_batch_target(agent_ids, all_agents))
        body.update(_to_sol_body(to_sol))
        return FleetBatch.from_dict(
            self._request(
                "POST",
                "/v1/agents/batch/exit",
                body=body,
                timeout=self.fund_timeout,
            )
        )

    def withdraw_agents(
        self, agent_ids: Optional[Sequence[str]] = None, *, all_agents: bool = False
    ) -> FleetBatch:
        """`POST /v1/agents/batch/withdraw`. Sweep many agent wallets' spendable SOL home.

        The agents keep running; positions stay open. Same targeting and
        polling as `exit_agents`. Needs a `trade` key. Takes the fund timeout.
        A withdrawal swaps nothing: it moves the spendable SOL only, so there is
        no `to_sol` here (an exit's leftover tokens exist only once a position
        is closed).
        """
        return FleetBatch.from_dict(
            self._request(
                "POST",
                "/v1/agents/batch/withdraw",
                body=_batch_target(agent_ids, all_agents),
                timeout=self.fund_timeout,
            )
        )

    def get_batch(self, batch_id: str) -> FleetBatch:
        """`GET /v1/agents/batch/{batch_id}`. The latest state of a batch this account started.

        Another account's batch id is 404, like a missing one. A finished
        batch is kept for 30 minutes.
        """
        return FleetBatch.from_dict(
            self._request("GET", "/v1/agents/batch/{0}".format(_seg(batch_id)))
        )

    def fleet(self, offset: Optional[int] = None, limit: Optional[int] = None) -> Fleet:
        """`GET /v1/agents/fleet?offset&limit`. One page of every agent with its wallet,
        mandate expiry and value.

        `limit` defaults to 25 and is clamped to 1 to 100 on the server. Pass
        the page's `next_offset` as `offset` for the next one, or use
        `iter_fleet` / `fleet_all`.
        """
        return Fleet.from_dict(
            self._request("GET", "/v1/agents/fleet", params={"offset": offset, "limit": limit})
        )

    def iter_fleet(self, limit: Optional[int] = None) -> Iterator[FleetRow]:
        """Every fleet row, following `next_offset` page by page.

        One request per page, made only when the previous page is used up.
        """
        offset: Optional[int] = None
        while True:
            page = self.fleet(offset=offset, limit=limit)
            for row in page.rows:
                yield row
            if page.next_offset is None or page.next_offset == offset:
                return
            offset = page.next_offset

    def fleet_all(self, limit: Optional[int] = None) -> List[FleetRow]:
        """`iter_fleet` collected into a list."""
        return list(self.iter_fleet(limit=limit))

    def create_agent_wallet(self, agent_id: str) -> AgentWalletCreated:
        """`POST /v1/agents/{id}/wallet`. Create the agent's own managed wallet.

        Idempotent: an agent that already has one gets it back with
        `created=False`. An empty wallet grants no authority; nothing signs
        from it until the owner signs a custody mandate in the app, which a
        key cannot do. Needs a `trade` key.
        """
        return AgentWalletCreated.from_dict(
            self._request("POST", "/v1/agents/{0}/wallet".format(_seg(agent_id)))
        )

    # -- the copilot (wallet session, or an API key) ---------------------------

    def copilot_parse(self, text: str) -> CopilotParse:
        """`POST /copilot/parse`. Read one sentence with ponk's closed grammar.

        Pure on the server: reads nothing, writes nothing. `ok` false carries
        `refusal` (a `code`, a sentence, the words not understood and example
        sentences that do parse). No language model runs anywhere. Needs
        `session_token` or an API key.
        """
        return CopilotParse.from_dict(
            self._request("POST", "/copilot/parse", body={"text": text}, session_only=True)
        )

    def copilot_plan(self, text: str) -> CopilotPlan:
        """`POST /copilot/plans`. Start building a plan from one sentence.

        Returns at once with `state` `building`; poll `copilot_get_plan` (or
        use `copilot_wait`). A sentence that does not parse, or that stops or
        shows a fleet rather than planning one, raises
        `PonkUnprocessableError` with the parser's sentence. Creates nothing
        and moves nothing. Plans last 15 minutes.
        """
        return CopilotPlan.from_dict(
            self._request("POST", "/copilot/plans", body={"text": text}, session_only=True)
        )

    def copilot_get_plan(self, plan_id: str) -> CopilotPlan:
        """`GET /copilot/plans/{id}`. The plan, its progress and its replay."""
        return CopilotPlan.from_dict(
            self._request("GET", "/copilot/plans/{0}".format(_seg(plan_id)), session_only=True)
        )

    def copilot_replay(self, plan_id: str) -> CopilotPlan:
        """`POST /copilot/plans/{id}/replay`. Replay every leg over the last 30 days.

        The replay runs on the server; poll the plan until `replay.state` is
        `done`. A repeat while it runs or after it finished returns the plan
        as it is. Read `replay.result` as sent: net, holding and net against
        holding side by side. It is one past window, not a forecast.
        """
        return CopilotPlan.from_dict(
            self._request(
                "POST", "/copilot/plans/{0}/replay".format(_seg(plan_id)), session_only=True
            )
        )

    def copilot_wait(
        self,
        plan_id: str,
        *,
        replay: bool = True,
        timeout: float = 300.0,
        sleep: Any = None,
    ) -> CopilotPlan:
        """Poll a plan until it is ready (and replayed, with `replay=True`) or refused.

        Starts the replay once the plan is ready when it has not run. Returns
        the last read when `timeout` seconds pass, so check `state` and
        `replay.state` rather than assuming it finished. `sleep` defaults to
        `time.sleep` and is a parameter for tests.
        """
        import time

        pause = sleep or time.sleep
        deadline = time.monotonic() + timeout
        asked = False
        while True:
            plan = self.copilot_get_plan(plan_id)
            rstate = plan.replay.state if plan.replay else None
            if plan.state == "ready" and replay and rstate == "idle" and not asked:
                asked = True
                self.copilot_replay(plan_id)
                continue
            settled = plan.state in ("refused", "expired") or (
                plan.state == "ready" and (not replay or rstate == "done")
            )
            if settled or time.monotonic() >= deadline:
                return plan
            pause(max((plan.poll_after_ms or 2000) / 1000.0, 1.0))

    def copilot_start(self, plan_id: str) -> CopilotStart:
        """`POST /copilot/plans/{id}/start`. Create the agents and prepare the funding.

        The agents are created in dry run, each with its own empty managed
        wallet, and the funding comes back as UNSIGNED transfers from your
        wallet (`funding`, a `FundPlan`) plus `sign_url`, the ponk.exchange
        page where your wallet signs them and then one custody mandate per
        agent (`mandate_queue`). NOTHING MOVES until your wallet signs; this
        client never signs. A plan starts at most once: a repeat returns
        `already_started` with the existing `fleet_id`. Takes the fund timeout.
        """
        return CopilotStart.from_dict(
            self._request(
                "POST",
                "/copilot/plans/{0}/start".format(_seg(plan_id)),
                timeout=self.fund_timeout,
                session_only=True,
            )
        )

    def copilot_fleets(self) -> List[CopilotFleetSummary]:
        """`GET /copilot/fleets`. Every fleet this wallet started from a plan."""
        return CopilotFleetSummary.from_list(
            self._request("GET", "/copilot/fleets", session_only=True)
        )

    def copilot_fleet(self, fleet_id: str) -> CopilotFleet:
        """`GET /copilot/fleets/{id}`. The fleet live: totals, the stop now, every agent.

        USD figures the server could not price are `None`, never zero, and
        `totals["unvalued"]` counts the agents left out of the totals.
        """
        return CopilotFleet.from_dict(
            self._request("GET", "/copilot/fleets/{0}".format(_seg(fleet_id)), session_only=True)
        )

    def copilot_stop_fleet(self, fleet_id: str) -> FleetBatch:
        """`POST /copilot/fleets/{id}/stop`. Close every agent in this fleet.

        Each agent of this fleet only is stopped, its position closed and its
        own wallet swept to your linked wallet through the managed exit under
        the mandate you signed; the destination cannot be named. The exits run
        detached: a `running` item is still going, not failed, and
        `copilot_fleet` shows where each agent got to, and its `exit` names
        every agent whose exit failed. On a fleet that is `stopping` with
        failed exits, calling this again retries ONLY those exits; on a
        stopped fleet it answers 409. Takes the fund timeout.
        """
        return FleetBatch.from_dict(
            self._request(
                "POST",
                "/copilot/fleets/{0}/stop".format(_seg(fleet_id)),
                timeout=self.fund_timeout,
                session_only=True,
            )
        )

    def copilot_tighten_stop(self, fleet_id: str, drawdown_pct: float) -> CopilotFleetSummary:
        """`POST /copilot/fleets/{id}/stop-rule`. Make the fleet stop tighter.

        `drawdown_pct` is percent below what went in, 1 to 50. Only a tighter
        (smaller) stop than the current one is accepted, and only while the
        fleet is active: a looser one raises `PonkUnprocessableError`.
        """
        return CopilotFleetSummary.from_dict(
            self._request(
                "POST",
                "/copilot/fleets/{0}/stop-rule".format(_seg(fleet_id)),
                body={"drawdown_pct": drawdown_pct},
                session_only=True,
            )
        )

    # -- agents, more actions (0.6.0) ----------------------------------------

    def rebalance(
        self,
        agent_id: str,
        *,
        new_lower_bin_id: int,
        new_upper_bin_id: int,
        strategy: str,
        max_slippage_bps: Optional[int] = None,
    ) -> RebalanceReceipt:
        """`POST /v1/agents/{id}/rebalance`. Close and reopen a managed agent's range now.

        Managed agents only. The agent's wallet signs under its mandate, which
        bounds the pool, slippage and value. `strategy` is the liquidity shape
        (for example `spot`). `max_slippage_bps` defaults to 50 on the server.
        A second call while one runs gets the running action back, and a
        rebalance within 60 seconds of a confirmed one is refused.

        RETURNING IS NOT SUCCEEDING, as for `compound`: read `signature` and
        `status` on the receipt. Needs a `trade` key. Takes the fund timeout.
        """
        body: Dict[str, Any] = {
            "new_lower_bin_id": new_lower_bin_id,
            "new_upper_bin_id": new_upper_bin_id,
            "strategy": strategy,
        }
        if max_slippage_bps is not None:
            body["max_slippage_bps"] = max_slippage_bps
        return RebalanceReceipt.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/rebalance".format(_seg(agent_id)),
                body=body,
                timeout=self.fund_timeout,
            )
        )

    def collect_fees(self, agent_id: str, destination: Optional[str] = None) -> ActionReceipt:
        """`POST /v1/agents/{id}/collect-fees`. Claim a managed agent's fees now.

        `destination` is `agent` (keep them in the agent wallet so it can
        redeploy them, the server default when omitted) or `connected` (pay
        them out to the owner's wallet). It is a choice, not an address: the
        server resolves the owner's wallet itself. Read `payout` on the
        receipt; `None` means the fees are still in the agent wallet. Needs a
        `trade` key. Takes the fund timeout.
        """
        if destination is not None and destination not in ("agent", "connected"):
            raise ValueError("destination must be 'agent' or 'connected', got {0!r}".format(destination))
        body = {"destination": destination} if destination is not None else None
        return ActionReceipt.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/collect-fees".format(_seg(agent_id)),
                body=body,
                timeout=self.fund_timeout,
            )
        )

    def rename_agent(self, agent_id: str, name: str) -> Agent:
        """`PATCH /v1/agents/{id}`. Rename an agent (1 to 100 characters)."""
        return Agent.from_dict(
            self._request("PATCH", "/v1/agents/{0}".format(_seg(agent_id)), body={"name": name})
        )

    def delete_agent(self, agent_id: str) -> None:
        """`DELETE /v1/agents/{id}`. Delete a finished agent.

        Refused with `PonkConflictError` while the agent still holds an open
        position or a balance: exit and withdraw first. Deleting never moves
        funds. Needs a `trade` key.
        """
        self._request("DELETE", "/v1/agents/{0}".format(_seg(agent_id)))

    def agent_proof(self, agent_id: str, hours: Optional[int] = None) -> AgentProof:
        """`GET /v1/agents/{id}/proof?hours=`. What each action cost and earned.

        `hours` defaults to 168 (7 days), clamped to 1 to 720.
        """
        return AgentProof.from_dict(
            self._request(
                "GET", "/v1/agents/{0}/proof".format(_seg(agent_id)), params={"hours": hours}
            )
        )

    def agent_range_history(self, agent_id: str, range: Optional[str] = None) -> RangeHistory:
        """`GET /v1/agents/{id}/range-history?range=`. The price against the range over time."""
        return RangeHistory.from_dict(
            self._request(
                "GET",
                "/v1/agents/{0}/range-history".format(_seg(agent_id)),
                params={"range": range},
            )
        )

    # -- the legacy shared wallet (0.6.0) ------------------------------------

    def custody_withdraw(self, lamports: Optional[int] = None) -> Withdrawal:
        """`POST /v1/custody/withdraw`. SOL out of the legacy per-user shared wallet.

        Omit `lamports` to sweep what is spendable. The destination is the
        owner's own wallet and cannot be named. Agents with their own wallet
        use `withdraw` or `withdraw_agents`. Needs a `trade` key.
        """
        body: Dict[str, Any] = {}
        if lamports is not None:
            body["lamports"] = lamports
        return Withdrawal.from_dict(
            self._request("POST", "/v1/custody/withdraw", body=body, timeout=self.fund_timeout)
        )

    def custody_exit(self, *, to_sol: Optional[bool] = None) -> Withdrawal:
        """`POST /v1/custody/exit`. Exit every managed agent and sweep every managed wallet home.

        Closes every managed position and sweeps every managed wallet of this
        account, the shared one included, to the owner's own wallet. Needs a
        `trade` key. Takes the fund timeout. `to_sol` as in `exit_agent`.
        """
        return Withdrawal.from_dict(
            self._request(
                "POST",
                "/v1/custody/exit",
                body=_to_sol_body(to_sol),
                timeout=self.fund_timeout,
            )
        )

    # -- account reads (0.6.0) -----------------------------------------------

    def portfolio(self) -> Portfolio:
        """`GET /v1/portfolio`. The account's LP portfolio summary."""
        return Portfolio.from_dict(self._request("GET", "/v1/portfolio"))

    def portfolio_history(self, range: Optional[str] = None) -> PortfolioHistory:
        """`GET /v1/portfolio/history?range=`. `range` is `24h`, `7d`, `30d`, `90d` or `all`."""
        return PortfolioHistory.from_dict(
            self._request("GET", "/v1/portfolio/history", params={"range": range})
        )

    def list_transactions(self, limit: Optional[int] = None) -> List[Transaction]:
        """`GET /v1/transactions?limit=`. Transactions the account's agents sent, newest first."""
        return Transaction.from_list(
            self._request("GET", "/v1/transactions", params={"limit": limit})
        )

    def list_risk_events(self, limit: Optional[int] = None) -> List[RiskEvent]:
        """`GET /v1/risk-events?limit=`. What the risk engine flagged or refused."""
        return RiskEvent.from_list(
            self._request("GET", "/v1/risk-events", params={"limit": limit})
        )

    def list_fees(self, limit: Optional[int] = None) -> List[FeeEvent]:
        """`GET /v1/fees?limit=`. Platform fee events on claimed LP fees, newest first."""
        return FeeEvent.from_list(self._request("GET", "/v1/fees", params={"limit": limit}))

    def fee_summary(self) -> FeeSummary:
        """`GET /v1/fees/summary`. Fee totals for the account."""
        return FeeSummary.from_dict(self._request("GET", "/v1/fees/summary"))

    # -- notifications and recommendations (0.6.0) ---------------------------

    def list_notifications(
        self, status: Optional[str] = None, limit: Optional[int] = None
    ) -> List[Notification]:
        """`GET /v1/notifications?status&limit`. `limit` defaults to 50."""
        return Notification.from_list(
            self._request("GET", "/v1/notifications", params={"status": status, "limit": limit})
        )

    def unread_notifications(self) -> UnreadCount:
        """`GET /v1/notifications/unread-count`."""
        return UnreadCount.from_dict(self._request("GET", "/v1/notifications/unread-count"))

    def mark_all_notifications_read(self) -> ReadAll:
        """`POST /v1/notifications/read-all`. Needs a `trade` key."""
        return ReadAll.from_dict(self._request("POST", "/v1/notifications/read-all"))

    def mark_notification_read(self, notification_id: str) -> None:
        """`POST /v1/notifications/{id}/read`. Needs a `trade` key."""
        self._request("POST", "/v1/notifications/{0}/read".format(_seg(notification_id)))

    def dismiss_notification(self, notification_id: str) -> None:
        """`POST /v1/notifications/{id}/dismiss`. Needs a `trade` key."""
        self._request("POST", "/v1/notifications/{0}/dismiss".format(_seg(notification_id)))

    def list_recommendations(
        self, status: Optional[str] = None, limit: Optional[int] = None
    ) -> List[Recommendation]:
        """`GET /v1/recommendations?status&limit`. `limit` defaults to 50."""
        return Recommendation.from_list(
            self._request("GET", "/v1/recommendations", params={"status": status, "limit": limit})
        )

    def get_recommendation(self, recommendation_id: str) -> Recommendation:
        """`GET /v1/recommendations/{id}`."""
        return Recommendation.from_dict(
            self._request("GET", "/v1/recommendations/{0}".format(_seg(recommendation_id)))
        )

    def review_recommendation(self, recommendation_id: str) -> Recommendation:
        """`POST /v1/recommendations/{id}/review`. Mark it seen. Needs a `trade` key."""
        return self._recommendation_action(recommendation_id, "review")

    def approve_recommendation(self, recommendation_id: str) -> Recommendation:
        """`POST /v1/recommendations/{id}/approve`. Flips the status only.

        No transaction is built or signed by approving. Needs a `trade` key.
        """
        return self._recommendation_action(recommendation_id, "approve")

    def dismiss_recommendation(self, recommendation_id: str) -> Recommendation:
        """`POST /v1/recommendations/{id}/dismiss`. Needs a `trade` key."""
        return self._recommendation_action(recommendation_id, "dismiss")

    def snooze_recommendation(
        self, recommendation_id: str, minutes: Optional[int] = None
    ) -> Recommendation:
        """`POST /v1/recommendations/{id}/snooze`. `minutes` uses the server default when unset."""
        return self._recommendation_action(
            recommendation_id, "snooze", body={"minutes": minutes}
        )

    def _recommendation_action(
        self, recommendation_id: str, action: str, body: Optional[Mapping[str, Any]] = None
    ) -> Recommendation:
        return Recommendation.from_dict(
            self._request(
                "POST",
                "/v1/recommendations/{0}/{1}".format(_seg(recommendation_id), action),
                body=body,
            )
        )

    # -- settings: notifications and saved presets (0.6.0) -------------------

    def notify_settings(self) -> NotifySettings:
        """`GET /v1/notify-settings`. Which Telegram messages the account receives."""
        return NotifySettings.from_dict(self._request("GET", "/v1/notify-settings"))

    def update_notify_settings(
        self,
        *,
        rebalance_telegram: Optional[bool] = None,
        mandate_telegram: Optional[bool] = None,
        weekly_recap_telegram: Optional[bool] = None,
    ) -> NotifySettings:
        """`PUT /v1/notify-settings`. Only the switches you pass are changed. Needs a `trade` key."""
        body = {
            k: v
            for k, v in (
                ("rebalance_telegram", rebalance_telegram),
                ("mandate_telegram", mandate_telegram),
                ("weekly_recap_telegram", weekly_recap_telegram),
            )
            if v is not None
        }
        return NotifySettings.from_dict(self._request("PUT", "/v1/notify-settings", body=body))

    def list_presets(self) -> List[Preset]:
        """`GET /v1/presets`. The account's saved agent presets."""
        return Preset.from_list(self._request("GET", "/v1/presets"))

    def create_preset(
        self, name: str, overrides: Mapping[str, Any], dex: Optional[str] = None
    ) -> Preset:
        """`POST /v1/presets`. Save a preset (name 1 to 60 characters).

        `overrides` is validated and clamped by the server, and the stored,
        normalized document comes back. `dex` limits it to one venue; `None`
        is any venue except Ponk Clouds, which presets cannot target. Needs a
        `trade` key.
        """
        return Preset.from_dict(
            self._request("POST", "/v1/presets", body=_preset_body(name, overrides, dex))
        )

    def update_preset(
        self, preset_id: str, name: str, overrides: Mapping[str, Any], dex: Optional[str] = None
    ) -> Preset:
        """`PUT /v1/presets/{id}`. Replace a preset in place, keeping its id. Needs a `trade` key."""
        return Preset.from_dict(
            self._request(
                "PUT",
                "/v1/presets/{0}".format(_seg(preset_id)),
                body=_preset_body(name, overrides, dex),
            )
        )

    def delete_preset(self, preset_id: str) -> None:
        """`DELETE /v1/presets/{id}`. Needs a `trade` key."""
        self._request("DELETE", "/v1/presets/{0}".format(_seg(preset_id)))


    # -- swap orders: Jupiter Trigger V2 and Recurring (0.7.0) ---------------
    #
    # Two doors, and neither signs. `plan_orders` validates orders and returns
    # one link on ponk.exchange where the owner's wallet signs them all (the
    # site's own order builders run there). A program that holds its own
    # wallet key uses the self-sign steps instead: `orders_challenge` and
    # `orders_verify` for a Jupiter session, `craft_order_deposit` for an
    # UNSIGNED deposit, `create_price_order` / `create_dca_order` with the
    # signed deposit, `cancel_order` for an UNSIGNED withdrawal and
    # `confirm_order_cancel` to land it.
    #
    # The Jupiter access token is an argument of each call, sent as the
    # X-Jupiter-Session header. This client never keeps it.
    #
    # Every wallet field is the key owner's: the server fills it in and
    # refuses a body that names another wallet. Ponk adds no fee to orders.
    #
    # OUTCOME UNKNOWN. The create and confirm calls land a transaction during
    # the request. A 504 with code `outcome_unknown` (raised as
    # `PonkServerError`) means it may have landed: read `list_orders` before
    # doing anything else, and never retry the same create blindly, or the
    # same order can be funded twice.

    def plan_orders(
        self,
        orders: Optional[Sequence[Mapping[str, Any]]] = None,
        *,
        cancel: Optional[Sequence[Mapping[str, Any]]] = None,
        cancel_all: bool = False,
    ) -> OrderPlan:
        """`POST /v1/orders/plan`. Validate orders and get the one link to sign them.

        `orders` are intents built with `limit_order`, `tp_sl_order` or
        `dca_order` (at most ten per link). `cancel` lists
        `{"family": "price"|"dca", "id": ...}`; `cancel_all=True` cancels every
        open order and active DCA plan, read fresh on the page. Nothing is
        created, signed or sent. A read key is enough: the wallet approves
        every deposit on the page.
        """
        body: Dict[str, Any] = {
            "orders": [dict(o) for o in (orders or [])],
            "cancel": [dict(c) for c in (cancel or [])],
            "cancel_all": bool(cancel_all),
        }
        return OrderPlan.from_dict(self._request("POST", "/v1/orders/plan", body=body))

    def orders_challenge(self, type: str = "message") -> JupiterChallenge:
        """`POST /v1/orders/auth/challenge`. Jupiter's sign-in challenge for the key's wallet.

        `type` is `message` (sign the text) or `transaction` (a memo-only
        transaction that is never sent, for wallets that cannot sign text).
        """
        return JupiterChallenge.from_dict(
            self._request("POST", "/v1/orders/auth/challenge", body={"type": type})
        )

    def orders_verify(
        self, *, signature: Optional[str] = None, signed_transaction: Optional[str] = None
    ) -> JupiterSession:
        """`POST /v1/orders/auth/verify`. Trade the wallet's signature of the challenge
        for a Jupiter session.

        Give `signature` (base58, for a message challenge) or
        `signed_transaction` (base64, for a transaction challenge), not both.
        """
        if (signature is None) == (signed_transaction is None):
            raise ValueError("give signature (message) or signed_transaction (transaction), exactly one")
        body: Dict[str, Any] = (
            {"type": "message", "signature": signature}
            if signature is not None
            else {"type": "transaction", "signedTransaction": signed_transaction}
        )
        return JupiterSession.from_dict(self._request("POST", "/v1/orders/auth/verify", body=body))

    def orders_refresh(self, refresh_token: str) -> JupiterSession:
        """`POST /v1/orders/auth/refresh`. A fresh access token for the session."""
        return JupiterSession.from_dict(
            self._request("POST", "/v1/orders/auth/refresh", body={"refreshToken": refresh_token})
        )

    def orders_logout(self, refresh_token: str) -> Any:
        """`POST /v1/orders/auth/logout`. End the Jupiter session."""
        return self._request("POST", "/v1/orders/auth/logout", body={"refreshToken": refresh_token})

    def order_mint_support(self, mints: Sequence[str]) -> Dict[str, Any]:
        """`GET /v1/orders/mint-support?mints=a,b`. Per mint, `None` when it can be
        used in an order, else the reason Jupiter's deposit would refuse it
        (`"unknown"` when the chain read failed). One to four mints.
        """
        if isinstance(mints, str):
            raise TypeError("mints is a list of mint addresses, not one string")
        return self._request("GET", "/v1/orders/mint-support", params={"mints": ",".join(mints)})

    def craft_order_deposit(
        self,
        jupiter_session: str,
        *,
        input_mint: str,
        output_mint: str,
        amount: str,
        order_type: str,
        order_sub_type: Optional[str] = None,
    ) -> DepositCraft:
        """`POST /v1/orders/deposit/craft`. The UNSIGNED deposit for an order.

        `amount` is base units of `input_mint` (for DCA, per order times the
        count). `order_type` is `price` or `dca`; a price order also takes
        `order_sub_type` `single`, `oco` or `otoco`, which must match the
        create call. The vault is resolved (and registered on first use)
        first. Sign `transaction` with the wallet, then create the order.
        Needs a `trade` key.
        """
        body: Dict[str, Any] = {
            "inputMint": input_mint,
            "outputMint": output_mint,
            "amount": str(amount),
            "orderType": order_type,
        }
        if order_sub_type is not None:
            body["orderSubType"] = order_sub_type
        return DepositCraft.from_dict(
            self._request(
                "POST", "/v1/orders/deposit/craft", body=body, headers=_jupiter(jupiter_session)
            )
        )

    def create_price_order(self, jupiter_session: str, body: Mapping[str, Any]) -> OrderCreated:
        """`POST /v1/orders/price`. Create a limit, trailing, OCO or OTOCO order.

        `body` is Jupiter's create-order body with the signed deposit
        (`depositRequestId`, `depositSignedTx`), for example `orderType`,
        `inputMint`, `outputMint`, `inputAmount`, `triggerMint`, `expiresAt`
        (ms), `triggerCondition` with `triggerPriceUsd` or `trailingBps`,
        `slippageBps`, and `tpPriceUsd` / `slPriceUsd` for a bracket.
        `userPubkey` is filled in by the server. Jupiter lands the deposit
        during this call: on `outcome_unknown` read `list_orders`, never retry
        blindly. Needs a `trade` key. Takes the fund timeout.
        """
        return OrderCreated.from_dict(
            self._request(
                "POST",
                "/v1/orders/price",
                body=dict(body),
                headers=_jupiter(jupiter_session),
                timeout=self.fund_timeout,
            )
        )

    def create_dca_order(self, jupiter_session: str, body: Mapping[str, Any]) -> OrderCreated:
        """`POST /v1/orders/dca`. Create a DCA plan with the signed deposit.

        `body`: `inputMint`, `outputMint`, `inputAmount` (per order times
        `orderCount`), `orderCount`, `intervalSeconds`, `orderType`
        (`time_based`, or `price_conditional` with `triggerMint` and
        `minPriceUsd` and/or `maxPriceUsd`), `depositRequestId`,
        `depositSignedTx`. Same outcome-unknown rule as `create_price_order`.
        Needs a `trade` key. Takes the fund timeout.
        """
        return OrderCreated.from_dict(
            self._request(
                "POST",
                "/v1/orders/dca",
                body=dict(body),
                headers=_jupiter(jupiter_session),
                timeout=self.fund_timeout,
            )
        )

    def cancel_order(self, jupiter_session: str, family: str, order_id: str) -> OrderCancel:
        """`POST /v1/orders/{family}/{id}/cancel`. Stop an order and get its UNSIGNED withdrawal.

        `family` is `price` or `dca`. An expired price order is withdrawn the
        same way. Calling it again on a half-done cancel re-crafts the
        withdrawal. Needs a `trade` key.
        """
        return OrderCancel.from_dict(
            self._request(
                "POST",
                "/v1/orders/{0}/{1}/cancel".format(_family(family), _seg(order_id)),
                headers=_jupiter(jupiter_session),
            )
        )

    def confirm_order_cancel(
        self,
        jupiter_session: str,
        family: str,
        order_id: str,
        *,
        signed_transaction: str,
        cancel_request_id: str,
    ) -> OrderCancel:
        """`POST /v1/orders/{family}/{id}/confirm-cancel`. Land the signed withdrawal.

        The funds return to the owner's wallet. On `outcome_unknown` read
        `list_orders`; the withdrawal may have landed. Needs a `trade` key.
        Takes the fund timeout.
        """
        return OrderCancel.from_dict(
            self._request(
                "POST",
                "/v1/orders/{0}/{1}/confirm-cancel".format(_family(family), _seg(order_id)),
                body={"signedTransaction": signed_transaction, "cancelRequestId": cancel_request_id},
                headers=_jupiter(jupiter_session),
                timeout=self.fund_timeout,
            )
        )

    def list_orders(
        self,
        jupiter_session: str,
        *,
        family: Optional[str] = None,
        state: Optional[str] = None,
        limit: Optional[int] = None,
        offset: Optional[int] = None,
        mint: Optional[str] = None,
    ) -> Dict[str, Any]:
        """`GET /v1/orders`. Open orders (`state="active"`) and history (`"past"`).

        With `family` (`price` or `dca`) the body is Jupiter's history for
        that family (`{"orders": [...]}`); without it, both, as
        `{"price": ..., "dca": ...}`. Jupiter's own shape, returned as is.
        """
        params = {"family": family, "state": state, "limit": limit, "offset": offset, "mint": mint}
        return self._request("GET", "/v1/orders", params=params, headers=_jupiter(jupiter_session))

    def cancel_orders(
        self,
        jupiter_session: str,
        orders: Optional[Sequence[Mapping[str, Any]]] = None,
        *,
        all_orders: bool = False,
    ) -> OrderBatch:
        """`POST /v1/orders/cancel`. Cancel step one for many orders at once.

        `orders` lists `{"family", "id"}` (at most 50), or `all_orders=True`
        cancels every open order, expired price order and active DCA plan,
        read fresh. Each item carries its UNSIGNED withdrawal
        (`item.cancel().transaction`); nothing moves until
        `confirm_order_cancels`. Needs a `trade` key.
        """
        body: Dict[str, Any] = {"orders": [dict(o) for o in (orders or [])], "all": bool(all_orders)}
        if not body["orders"] and not all_orders:
            raise ValueError("name the orders, or pass all_orders=True")
        return OrderBatch.from_dict(
            self._request("POST", "/v1/orders/cancel", body=body, headers=_jupiter(jupiter_session))
        )

    def confirm_order_cancels(
        self, jupiter_session: str, items: Sequence[Mapping[str, Any]]
    ) -> OrderBatch:
        """`POST /v1/orders/confirm-cancel`. Land up to ten signed withdrawals.

        Each item is `{"family", "id", "signedTransaction",
        "cancelRequestId"}`. An item whose answer was lost reports
        `outcome_unknown`, never retried here. Needs a `trade` key. Takes the
        fund timeout.
        """
        return OrderBatch.from_dict(
            self._request(
                "POST",
                "/v1/orders/confirm-cancel",
                body={"items": [dict(i) for i in items]},
                headers=_jupiter(jupiter_session),
                timeout=self.fund_timeout,
            )
        )

    def orders_sign_in(self, sign_message: Callable[[str], str]) -> JupiterSession:
        """Challenge, sign and verify in one call.

        `sign_message` takes Jupiter's challenge text and returns the wallet's
        base58 ed25519 signature of its UTF-8 bytes. Your code signs; this
        client never sees a key. Check the text says it authenticates the
        wallet you expect before signing.
        """
        c = self.orders_challenge("message")
        if not c.challenge:
            raise ValueError("Jupiter sent no challenge text")
        return self.orders_verify(signature=sign_message(c.challenge))

    def place_order(
        self,
        jupiter_session: str,
        family: str,
        body: Mapping[str, Any],
        sign_transaction: Callable[[str], str],
    ) -> OrderCreated:
        """Craft the deposit, have your code sign it, create the order.

        `body` is the create body WITHOUT the deposit fields (see
        `create_price_order` / `create_dca_order`); `inputAmount`,
        `inputMint`, `outputMint` and, for a price order, `orderType` are read
        from it for the craft. `sign_transaction` takes the UNSIGNED base64
        deposit and returns it signed, base64. The crafted amount and mint are
        checked against `body` before your signer is called.
        """
        fam = _family(family)
        sub = body.get("orderType") if fam == "price" else None
        craft = self.craft_order_deposit(
            jupiter_session,
            input_mint=str(body["inputMint"]),
            output_mint=str(body["outputMint"]),
            amount=str(body["inputAmount"]),
            order_type=fam,
            order_sub_type=sub,
        )
        if craft.amount != str(body["inputAmount"]) or craft.mint != str(body["inputMint"]):
            raise ValueError("Jupiter's deposit does not match this order. Nothing was signed.")
        signed = sign_transaction(str(craft.transaction))
        full = dict(body, depositRequestId=craft.request_id, depositSignedTx=signed)
        if fam == "price":
            return self.create_price_order(jupiter_session, full)
        return self.create_dca_order(jupiter_session, full)

    # -- copy, live follow and Copy LP (0.7.0) --------------------------------

    def copy_source(
        self,
        agent_id: str,
        source: str,
        *,
        follow: bool = False,
        auto_follow_pool_moves: bool = False,
    ) -> FollowStatus:
        """`POST /v1/agents/{id}/copy-source`. Record that this agent copies a board agent.

        `source` is the board row's key (`LeaderboardRow` `copy` block). Every
        mirrored setting the create body did not carry is brought over, and
        `follow=True` links it so the leader's config changes follow live.
        Config only, never funds. Needs a `trade` key.
        """
        body = {"source": source, "follow": follow, "auto_follow_pool_moves": auto_follow_pool_moves}
        return FollowStatus.from_dict(
            self._request("POST", "/v1/agents/{0}/copy-source".format(_seg(agent_id)), body=body)
        )

    def follow_status(self, agent_id: str) -> FollowStatus:
        """`GET /v1/agents/{id}/follow`. Whether this copy follows its leader, and what is paused."""
        return FollowStatus.from_dict(self._request("GET", "/v1/agents/{0}/follow".format(_seg(agent_id))))

    def follow_leader(self, agent_id: str, *, auto_follow_pool_moves: bool = False) -> FollowStatus:
        """`POST /v1/agents/{id}/follow`. Follow, live, the agent this copy came from.

        `auto_follow_pool_moves=True` acts on the leader's pool moves and
        exits (through the managed close and self-open, every gate) instead of
        only offering them. Needs a `trade` key.
        """
        return FollowStatus.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/follow".format(_seg(agent_id)),
                body={"auto_follow_pool_moves": auto_follow_pool_moves},
            )
        )

    def update_follow(
        self,
        agent_id: str,
        *,
        auto_follow_pool_moves: Optional[bool] = None,
        resume_keys: Optional[Sequence[str]] = None,
        pause_keys: Optional[Sequence[str]] = None,
    ) -> FollowStatus:
        """`PATCH /v1/agents/{id}/follow`. Change the follow: pool moves, or pause and
        resume mirrored keys. Needs a `trade` key."""
        body: Dict[str, Any] = {}
        if auto_follow_pool_moves is not None:
            body["auto_follow_pool_moves"] = auto_follow_pool_moves
        if resume_keys is not None:
            body["resume_keys"] = list(resume_keys)
        if pause_keys is not None:
            body["pause_keys"] = list(pause_keys)
        return FollowStatus.from_dict(
            self._request("PATCH", "/v1/agents/{0}/follow".format(_seg(agent_id)), body=body)
        )

    def unfollow_leader(self, agent_id: str) -> FollowStatus:
        """`DELETE /v1/agents/{id}/follow`. Stop following; the copy keeps its config. Needs a `trade` key."""
        return FollowStatus.from_dict(self._request("DELETE", "/v1/agents/{0}/follow".format(_seg(agent_id))))

    def answer_follow_offer(self, agent_id: str, accept: bool) -> FollowStatus:
        """`POST /v1/agents/{id}/follow/offer`. Accept or decline the leader's pool move or exit.

        Accept runs the managed close into the agent's own wallet and, for a
        move, the self-open on the new pool, under the mandate and every gate.
        Decline changes nothing. Needs a `trade` key.
        """
        return FollowStatus.from_dict(
            self._request(
                "POST", "/v1/agents/{0}/follow/offer".format(_seg(agent_id)), body={"accept": bool(accept)}
            )
        )

    def list_copy_follows(self) -> CopyFollows:
        """`GET /v1/copy-follows`. Every copy of this account and its follow state."""
        return CopyFollows.from_dict(self._request("GET", "/v1/copy-follows"))

    def follow_leaders(
        self,
        agent_ids: Sequence[str],
        *,
        auto_follow_pool_moves: bool = False,
        unfollow: bool = False,
    ) -> BatchResult:
        """`POST /v1/agents/follow/batch`. Live follow (or `unfollow=True`) for many copies.

        At most 50. Each runs through the single-agent handler; one refusal
        does not stop the others. Needs a `trade` key.
        """
        if isinstance(agent_ids, str):
            raise TypeError("agent_ids is a list of ids, not one string")
        items = [{"agent_id": a, "auto_follow_pool_moves": auto_follow_pool_moves} for a in agent_ids]
        return BatchResult.from_dict(
            self._request("POST", "/v1/agents/follow/batch", body={"items": items, "unfollow": unfollow})
        )

    def follow_many(self, slugs: Sequence[str], *, unfollow: bool = False) -> BatchResult:
        """`POST /v1/follows/batch`. Follow (or unfollow) many shared agents by slug for
        their alerts. Nothing is mirrored or moved. Needs a `trade` key."""
        if isinstance(slugs, str):
            raise TypeError("slugs is a list, not one string")
        return BatchResult.from_dict(
            self._request("POST", "/v1/follows/batch", body={"slugs": list(slugs), "unfollow": unfollow})
        )

    def copy_log(self, agent_id: str) -> CopyLog:
        """`GET /v1/agents/{id}/copy-log`. The Copy LP mirror and what it did or declined."""
        return CopyLog.from_dict(self._request("GET", "/v1/agents/{0}/copy-log".format(_seg(agent_id))))

    def wallet_follow(
        self,
        agent_id: str,
        *,
        leader: Optional[str] = None,
        position: Optional[str] = None,
        size_pct: Optional[float] = None,
        max_usd_per_action: Optional[float] = None,
        pools: Optional[Sequence[str]] = None,
        min_tvl_usd: Optional[float] = None,
        auto_mirror: bool = False,
    ) -> CopyLog:
        """`POST /v1/agents/{id}/wallet-follow`. Mirror a wallet's LP moves with this agent.

        `leader` is required the first time; naming another wallet later
        starts over from its newest action. `size_pct` of the leader's size,
        capped at `max_usd_per_action`; `pools` and `min_tvl_usd` filter;
        `auto_mirror=False` (ask) offers each move, `True` acts on it under
        the mandate. Moves no funds by itself. Needs a `trade` key.
        """
        body: Dict[str, Any] = {"auto_mirror": bool(auto_mirror)}
        for k, v in (
            ("leader", leader),
            ("position", position),
            ("size_pct", size_pct),
            ("max_usd_per_action", max_usd_per_action),
            ("min_tvl_usd", min_tvl_usd),
        ):
            if v is not None:
                body[k] = v
        if pools is not None:
            body["pools"] = list(pools)
        return CopyLog.from_dict(
            self._request("POST", "/v1/agents/{0}/wallet-follow".format(_seg(agent_id)), body=body)
        )

    def unfollow_wallet(self, agent_id: str) -> CopyLog:
        """`DELETE /v1/agents/{id}/wallet-follow`. Stop mirroring. Needs a `trade` key."""
        return CopyLog.from_dict(self._request("DELETE", "/v1/agents/{0}/wallet-follow".format(_seg(agent_id))))

    def answer_wallet_offer(self, agent_id: str, accept: bool) -> CopyLog:
        """`POST /v1/agents/{id}/wallet-follow/offer`. Act on (or decline) a mirrored move
        waiting in ask mode. Needs a `trade` key."""
        return CopyLog.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/wallet-follow/offer".format(_seg(agent_id)),
                body={"accept": bool(accept)},
            )
        )

    def copy_referral(self, key: str) -> CopyReferralOffer:
        """`GET /v1/copy-referral/{key}`. Whether a copied board row offers its owner's referral."""
        return CopyReferralOffer.from_dict(self._request("GET", "/v1/copy-referral/{0}".format(_seg(key))))

    def attach_copy_referral(self, key: str) -> CopyReferralAttach:
        """`POST /v1/copy-referral/{key}`. Attach the copied owner's referral. Needs a `trade` key."""
        return CopyReferralAttach.from_dict(self._request("POST", "/v1/copy-referral/{0}".format(_seg(key))))

    # -- the self-custody loop: prepare, sign in your wallet, mark (0.7.0) ----
    #
    # Every prepare returns UNSIGNED transactions for the key owner's own
    # wallet (the server reads the wallet from the account, never from the
    # body). Sign in wallet software you control, send, then report the
    # signature with the matching mark call, which re-reads the chain before
    # it believes it. Each needs a `trade` key.

    def agent_rebalance_prepare(
        self,
        agent_id: str,
        *,
        new_lower_bin_id: int,
        new_upper_bin_id: int,
        strategy: str,
        max_slippage_bps: int,
        **extra: Any,
    ) -> Prepared:
        """`POST /v1/agents/{id}/rebalance/prepare`. A self-custody agent's rebalance, unsigned."""
        body = dict(
            extra,
            new_lower_bin_id=new_lower_bin_id,
            new_upper_bin_id=new_upper_bin_id,
            strategy=strategy,
            max_slippage_bps=max_slippage_bps,
        )
        return Prepared.from_dict(
            self._request("POST", "/v1/agents/{0}/rebalance/prepare".format(_seg(agent_id)), body=body)
        )

    def agent_rebalance_prepare_split(
        self,
        agent_id: str,
        *,
        new_lower_bin_id: int,
        new_upper_bin_id: int,
        strategy: str,
        max_slippage_bps: int,
        simulate: bool = False,
        **extra: Any,
    ) -> Prepared:
        """`POST /v1/agents/{id}/rebalance/prepare-split`. The close leg of a split rebalance.

        Send it, then build the open leg with `agent_rebalance_prepare_open`
        against a fresh blockhash. `simulate=True` simulates both legs first.
        """
        body = dict(
            extra,
            new_lower_bin_id=new_lower_bin_id,
            new_upper_bin_id=new_upper_bin_id,
            strategy=strategy,
            max_slippage_bps=max_slippage_bps,
        )
        return Prepared.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/rebalance/prepare-split".format(_seg(agent_id)),
                body=body,
                params={"simulate": "true"} if simulate else None,
            )
        )

    def agent_rebalance_prepare_open(self, agent_id: str, body: Mapping[str, Any]) -> Prepared:
        """`POST /v1/agents/{id}/rebalance/prepare-open`. The open leg, after the close confirmed.

        `body`: `close_signature`, `new_lower_bin_id`, `new_upper_bin_id`,
        `strategy`, `max_slippage_bps`, `fee_event_ids`. Sign `open_tx_b64`.
        """
        return Prepared.from_dict(
            self._request(
                "POST", "/v1/agents/{0}/rebalance/prepare-open".format(_seg(agent_id)), body=dict(body)
            )
        )

    def agent_rebalance_mark(
        self,
        agent_id: str,
        *,
        signature: str,
        new_position_address: str,
        close_signature: Optional[str] = None,
    ) -> MarkedTx:
        """`POST /v1/agents/{id}/rebalance/mark-signature`. Report the rebalance your wallet sent."""
        body: Dict[str, Any] = {"signature": signature, "new_position_address": new_position_address}
        if close_signature is not None:
            body["close_signature"] = close_signature
        return MarkedTx.from_dict(
            self._request("POST", "/v1/agents/{0}/rebalance/mark-signature".format(_seg(agent_id)), body=body)
        )

    def agent_fees_prepare(self, agent_id: str) -> Prepared:
        """`POST /v1/agents/{id}/collect-fees/prepare`. The fee claim, unsigned, with the
        platform fee split shown and `draft_event_ids` to mark."""
        return Prepared.from_dict(
            self._request("POST", "/v1/agents/{0}/collect-fees/prepare".format(_seg(agent_id)))
        )

    def agent_fees_mark(self, agent_id: str, *, draft_event_ids: Sequence[str], signature: str) -> MarkedTx:
        """`POST /v1/agents/{id}/collect-fees/mark-signature`. Attach the claim's signature
        to its draft fee events."""
        return MarkedTx.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/collect-fees/mark-signature".format(_seg(agent_id)),
                body={"draft_event_ids": list(draft_event_ids), "signature": signature},
            )
        )

    def agent_fees_confirm(self, agent_id: str, *, signature: str, fee_token_mints: Sequence[str]) -> MarkedTx:
        """`POST /v1/agents/{id}/collect-fees/confirm`. Confirm the fee events once the claim landed."""
        return MarkedTx.from_dict(
            self._request(
                "POST",
                "/v1/agents/{0}/collect-fees/confirm".format(_seg(agent_id)),
                body={"signature": signature, "fee_token_mints": list(fee_token_mints)},
            )
        )

    def position_open_prepare(self, body: Mapping[str, Any]) -> Prepared:
        """`POST /v1/positions/open/prepare`. Open a position from the key's wallet, unsigned.

        `body`: `pool_address`, `dex`, `amount_x`, `amount_y` (base units),
        `lower_bin_id`, `width`, `strategy`, and the route's optional fields.
        """
        return Prepared.from_dict(self._request("POST", "/v1/positions/open/prepare", body=dict(body)))

    def position_mark_tx(
        self,
        *,
        signature: str,
        action_type: str,
        dex: str,
        position_address: Optional[str] = None,
        **extra: Any,
    ) -> MarkedTx:
        """`POST /v1/positions/mark-tx`. Record a transaction your wallet sent (open, claim,
        compound, rebalance, withdraw) so the ledger, PnL and fee events see it.
        Pass `fee_event_ids` and the route's other fields through `extra`."""
        body = dict(extra, signature=signature, action_type=action_type, dex=dex)
        if position_address is not None:
            body["position_address"] = position_address
        return MarkedTx.from_dict(self._request("POST", "/v1/positions/mark-tx", body=body))

    def position_rebalance_split(self, position_address: str, body: Mapping[str, Any]) -> Prepared:
        """`POST /v1/positions/{position}/rebalance/prepare-split`. Close leg of a Meteora rebalance.

        `body`: `new_lower_bin_id`, `new_upper_bin_id`, `strategy`,
        `max_slippage_bps`, and the route's optional fields.
        """
        return Prepared.from_dict(
            self._request(
                "POST",
                "/v1/positions/{0}/rebalance/prepare-split".format(_seg(position_address)),
                body=dict(body),
            )
        )

    def position_rebalance_open(self, position_address: str, body: Mapping[str, Any]) -> Prepared:
        """`POST /v1/positions/{position}/rebalance/prepare-open`. The open leg after the close.

        `body`: `pool_address` plus the reopen fields (`close_signature`,
        `new_lower_bin_id`, `new_upper_bin_id`, `strategy`,
        `max_slippage_bps`, `fee_event_ids`).
        """
        return Prepared.from_dict(
            self._request(
                "POST",
                "/v1/positions/{0}/rebalance/prepare-open".format(_seg(position_address)),
                body=dict(body),
            )
        )

    def position_rebalance_tick(
        self, position_address: str, *, new_lower: int, new_upper: int, max_slippage_bps: int, **extra: Any
    ) -> Prepared:
        """`POST /v1/positions/{position}/rebalance/prepare`. Orca or Raydium recenter, one transaction."""
        body = dict(extra, new_lower=new_lower, new_upper=new_upper, max_slippage_bps=max_slippage_bps)
        return Prepared.from_dict(
            self._request(
                "POST", "/v1/positions/{0}/rebalance/prepare".format(_seg(position_address)), body=body
            )
        )

    def position_compound_prepare(self, position_address: str) -> Prepared:
        """`POST /v1/positions/{position}/compound/prepare`. Claim and re-deposit, unsigned."""
        return Prepared.from_dict(
            self._request("POST", "/v1/positions/{0}/compound/prepare".format(_seg(position_address)))
        )

    def position_claim_prepare(self, position_address: str) -> Prepared:
        """`POST /v1/positions/{position}/claim-fees/prepare`. Claim fees, unsigned."""
        return Prepared.from_dict(
            self._request("POST", "/v1/positions/{0}/claim-fees/prepare".format(_seg(position_address)))
        )

    def lp_withdraw_prepare(
        self, dex: str, *, pool_address: str, position: str, bps_to_remove: int
    ) -> Prepared:
        """`POST /v1/lp/{dex}/withdraw/prepare`. Withdraw from your own position, unsigned.

        `dex` is `meteora_dlmm`, `orca`, `raydium` or `ponk_clouds` (Ponk
        Clouds is closed to new liquidity and a held position can always be
        withdrawn). `bps_to_remove` 1 to 10000; 10000 removes everything and
        closes the position. The owner is the key's wallet.
        """
        body = {"pool_address": pool_address, "position": position, "bps_to_remove": int(bps_to_remove)}
        return Prepared.from_dict(
            self._request("POST", "/v1/lp/{0}/withdraw/prepare".format(_seg(dex)), body=body)
        )

    def reclaim_wsol_prepare(self) -> Prepared:
        """`POST /v1/wallet/reclaim-wsol/prepare`. Unwrap the wallet's wSOL and reclaim its rent."""
        return Prepared.from_dict(self._request("POST", "/v1/wallet/reclaim-wsol/prepare"))

    def stranded_positions(self) -> StrandedPositions:
        """`GET /v1/wallet/stranded-positions`. Empty positions still holding rent."""
        return StrandedPositions.from_dict(self._request("GET", "/v1/wallet/stranded-positions"))

    def cleanup_position_prepare(self, position_address: str) -> Prepared:
        """`POST /v1/wallet/cleanup-position/prepare`. Close a stranded position, unsigned."""
        return Prepared.from_dict(
            self._request(
                "POST", "/v1/wallet/cleanup-position/prepare", body={"position_address": position_address}
            )
        )

    def positions_sync(
        self,
        *,
        pool_address: Optional[str] = None,
        position_address: Optional[str] = None,
        dex: Optional[str] = None,
    ) -> List[Position]:
        """`POST /v1/positions/sync`. Re-read the wallet's positions from chain. Needs a `trade` key."""
        body = {
            k: v
            for k, v in (("pool_address", pool_address), ("position_address", position_address), ("dex", dex))
            if v is not None
        }
        return Position.from_list(self._request("POST", "/v1/positions/sync", body=body))

    def share_position_telegram(self, card: Mapping[str, Any]) -> TelegramShared:
        """`POST /v1/positions/share-telegram`. Post a PnL card to your linked Telegram.

        `card`: `win`, `fee_pct`, and the optional figures (`pnl_usd`,
        `pnl_pct`, `fees_usd`, `il_usd`, `capital_usd`, `bin_step`,
        `held_d`, `held_h`, `held_m`, `referral_code`). Linking Telegram
        itself is done in the app. Needs a `trade` key.
        """
        return TelegramShared.from_dict(self._request("POST", "/v1/positions/share-telegram", body=dict(card)))

    def clouds_referrals(self) -> Dict[str, Any]:
        """`GET /v1/clouds/referrals`. Your Ponk Clouds referral accruals, read from chain."""
        return self._request("GET", "/v1/clouds/referrals")

    def clouds_referral_claim_prepare(self, pool: str) -> Prepared:
        """`POST /v1/clouds/referrals/claim/prepare`. Claim one pool's referral accrual, unsigned."""
        return Prepared.from_dict(
            self._request("POST", "/v1/clouds/referrals/claim/prepare", body={"pool": pool})
        )

    # -- account: calls, push, cards, referral, privacy, logs (0.7.0) --------

    def call_preview(self, mint: str) -> CallPreview:
        """`GET /v1/calls/preview?mint=`. What a call on this token would attach."""
        return CallPreview.from_dict(self._request("GET", "/v1/calls/preview", params={"mint": mint}))

    def post_call(self, mint: str, text: str) -> Call:
        """`POST /v1/calls`. Post a Ponk Call on any token. The server reads your
        position from chain: holding it earns the call the Holder mark, and no
        position is needed to call. One call per token every 6 hours. Needs a
        `trade` key."""
        return Call.from_dict(self._request("POST", "/v1/calls", body={"mint": mint, "text": text}))

    def delete_call(self, call_id: str) -> Any:
        """`DELETE /v1/calls/{id}`. Delete your own call. Needs a `trade` key."""
        return self._request("DELETE", "/v1/calls/{0}".format(_seg(call_id)))

    def push_settings(self, endpoint: Optional[str] = None) -> PushSettings:
        """`GET /v1/push?endpoint=`. Web Push switches and how many devices are subscribed."""
        return PushSettings.from_dict(self._request("GET", "/v1/push", params={"endpoint": endpoint}))

    def set_push_prefs(self, prefs: Mapping[str, Any]) -> Dict[str, Any]:
        """`PUT /v1/push/prefs`. Switch push kinds on or off; only the keys passed change.
        Needs a `trade` key."""
        return self._request("PUT", "/v1/push/prefs", body=dict(prefs))

    def push_subscribe(self, endpoint: str, *, p256dh: str, auth: str) -> None:
        """`POST /v1/push/subscriptions`. Register a browser push subscription (only a
        browser push service's endpoint is accepted). Needs a `trade` key."""
        self._request(
            "POST", "/v1/push/subscriptions", body={"endpoint": endpoint, "keys": {"p256dh": p256dh, "auth": auth}}
        )

    def push_unsubscribe(self, endpoint: str) -> None:
        """`DELETE /v1/push/subscriptions`. Remove one subscription. Needs a `trade` key."""
        self._request("DELETE", "/v1/push/subscriptions", body={"endpoint": endpoint})

    def push_test(self) -> PushTest:
        """`POST /v1/push/test`. Send a test push to every device (once a minute). Needs a `trade` key."""
        return PushTest.from_dict(self._request("POST", "/v1/push/test"))

    def pnl_card(self) -> CardLook:
        """`GET /v1/pnl-card`. The PnL share card's saved look."""
        return CardLook.from_dict(self._request("GET", "/v1/pnl-card"))

    def set_pnl_card(self, prefs: Mapping[str, Any]) -> CardLook:
        """`PUT /v1/pnl-card`. Save the PnL card look (`background`, `fields`, `units`,
        `identity`, `custom_name`, `avatar`, `show_referral`, `show_qr`, `accent`).
        Needs a `trade` key."""
        return CardLook.from_dict(self._request("PUT", "/v1/pnl-card", body=dict(prefs)))

    def referral_card(self) -> CardLook:
        """`GET /v1/referral-card`. The referral card's saved look."""
        return CardLook.from_dict(self._request("GET", "/v1/referral-card"))

    def set_referral_card(self, prefs: Mapping[str, Any]) -> CardLook:
        """`PUT /v1/referral-card`. Save the referral card look. Needs a `trade` key."""
        return CardLook.from_dict(self._request("PUT", "/v1/referral-card", body=dict(prefs)))

    def set_referral_code(self, code: str) -> Referral:
        """`POST /v1/referral/code`. Claim the referral code this account was referred by.
        Needs a `trade` key."""
        return Referral.from_dict(self._request("POST", "/v1/referral/code", body={"code": code}))

    def add_referral_code(self, code: str, label: Optional[str] = None) -> Referral:
        """`POST /v1/referral/codes`. Add one of your own referral codes. Needs a `trade` key."""
        return Referral.from_dict(
            self._request("POST", "/v1/referral/codes", body={"code": code, "label": label})
        )

    def apply_creator(self, *, handle: Optional[str] = None, note: Optional[str] = None) -> Referral:
        """`POST /v1/referral/creator/apply`. Apply for the Creator tier. Needs a `trade` key."""
        return Referral.from_dict(
            self._request("POST", "/v1/referral/creator/apply", body={"handle": handle, "note": note})
        )

    def privacy(self) -> Privacy:
        """`GET /v1/me/privacy`. Since 2026-10-29 every Ponk account is
        public, so this answers the public state. Kept for older code."""
        return Privacy.from_dict(self._request("GET", "/v1/me/privacy"))

    def set_privacy(
        self,
        *,
        is_public: bool,
        show_wallet: bool = False,
        show_usd: bool = True,
        share_swaps: bool = False,
        rank_anon: bool = True,
    ) -> Privacy:
        """`PUT /v1/me/privacy`. Every account is public since 2026-10-29: the
        server accepts this body, ignores it and answers the public state. Kept
        so older code does not break. Needs a `trade` key."""
        body = {
            "is_public": bool(is_public),
            "show_wallet": bool(show_wallet),
            "show_usd": bool(show_usd),
            "share_swaps": bool(share_swaps),
            "rank_anon": bool(rank_anon),
        }
        return Privacy.from_dict(self._request("PUT", "/v1/me/privacy", body=body))

    def answer_privacy_prompt(self, keep_public: bool) -> Privacy:
        """`POST /v1/me/privacy/ack`. The prompt it answered no longer exists;
        the server accepts and ignores it and answers the public state. Needs a
        `trade` key."""
        return Privacy.from_dict(
            self._request("POST", "/v1/me/privacy/ack", body={"keep_public": bool(keep_public)})
        )

    def leaderboard_privacy(self) -> LeaderboardPrivacy:
        """`GET /v1/leaderboard-privacy`. How your agents appear on the public board."""
        return LeaderboardPrivacy.from_dict(self._request("GET", "/v1/leaderboard-privacy"))

    def set_leaderboard_privacy(
        self, *, visible: bool, show_name: bool, show_wallet: bool, display_name: Optional[str] = None
    ) -> LeaderboardPrivacy:
        """`PUT /v1/leaderboard-privacy`. Needs a `trade` key."""
        body = {"visible": visible, "show_name": show_name, "show_wallet": show_wallet, "display_name": display_name}
        return LeaderboardPrivacy.from_dict(self._request("PUT", "/v1/leaderboard-privacy", body=body))

    def agent_visibility(self, agent_id: str) -> AgentVisibility:
        """`GET /v1/agents/{id}/leaderboard-visibility`."""
        return AgentVisibility.from_dict(
            self._request("GET", "/v1/agents/{0}/leaderboard-visibility".format(_seg(agent_id)))
        )

    def set_agent_visibility(self, agent_id: str, hidden: bool) -> AgentVisibility:
        """`PUT /v1/agents/{id}/leaderboard-visibility`. Keep one agent off the board. Needs a `trade` key."""
        return AgentVisibility.from_dict(
            self._request(
                "PUT", "/v1/agents/{0}/leaderboard-visibility".format(_seg(agent_id)), body={"hidden": bool(hidden)}
            )
        )

    def clear_logs(self) -> LogsCleared:
        """`DELETE /v1/logs`. Hide the activity log up to now. The fund ledger is never
        deleted. Needs a `trade` key."""
        return LogsCleared.from_dict(self._request("DELETE", "/v1/logs"))

    def weekly_recap(self) -> Dict[str, Any]:
        """`GET /v1/recap/weekly`. The weekly recap card's figures."""
        return self._request("GET", "/v1/recap/weekly")

    def my_ponkillionaires(self) -> Dict[str, Any]:
        """`GET /v1/ponkillionaires`. Your standing in this week's race."""
        return self._request("GET", "/v1/ponkillionaires")

    def telegram_status(self) -> TelegramStatus:
        """`GET /v1/telegram`. Whether Telegram is linked. Linking is done in the app."""
        return TelegramStatus.from_dict(self._request("GET", "/v1/telegram"))

    # -- rank presets (0.7.0) ----------------------------------------------------

    def rank_presets(
        self,
        dex: str,
        pool: str,
        *,
        window_days: Optional[int] = None,
        capital_usd: Optional[float] = None,
        include_mine: Optional[bool] = None,
    ) -> PresetRank:
        """`POST /v1/presets/rank`. Every wizard preset, and your saved models, replayed on
        one pool's window by the site's Plot engine and ranked by net result.

        `dex` is `meteora_dlmm`, `orca` or `raydium`; `window_days` 7, 14, 30
        or 90. A pure simulation: nothing moves. Six ranks a minute per
        account (429 `PonkRateLimitedError` past that); a repeat for the same
        pool and window is cached.
        """
        body = {
            "dex": dex,
            "pool": pool,
            "window_days": window_days,
            "capital_usd": capital_usd,
            "include_mine": include_mine,
        }
        return PresetRank.from_dict(self._request("POST", "/v1/presets/rank", body=body))


def _jupiter(token: str) -> Dict[str, str]:
    """The X-Jupiter-Session header for one call. The token is not kept."""
    if not token or not isinstance(token, str):
        raise ValueError("jupiter_session is the access token from orders_verify / orders_sign_in")
    return {"X-Jupiter-Session": token}


def _family(family: str) -> str:
    if family not in ("price", "dca"):
        raise ValueError("family must be 'price' or 'dca', got {0!r}".format(family))
    return family


def _drop_none(d: Dict[str, Any]) -> Dict[str, Any]:
    return {k: v for k, v in d.items() if v is not None}


def limit_order(
    input_mint: str,
    output_mint: str,
    amount: Any,
    *,
    target_usd: Optional[float] = None,
    target_mc_usd: Optional[float] = None,
    trailing_bps: Optional[int] = None,
    tp_usd: Optional[float] = None,
    sl_usd: Optional[float] = None,
    slippage_bps: Optional[int] = None,
    expires_in_secs: Optional[int] = None,
) -> Dict[str, Any]:
    """An order intent for `plan_orders`: a limit (`target_usd` or
    `target_mc_usd`) or a trailing order (`trailing_bps`, 50 to 9000), exactly
    one. `tp_usd` with `sl_usd` arms a take-profit and stop-loss on what it
    buys. `amount` is base units of `input_mint`. Expiry defaults to 7 days
    from signing, at most 30."""
    return _drop_none(
        {
            "kind": "limit",
            "input_mint": input_mint,
            "output_mint": output_mint,
            "amount": str(amount),
            "target_usd": target_usd,
            "target_mc_usd": target_mc_usd,
            "trailing_bps": trailing_bps,
            "tp_usd": tp_usd,
            "sl_usd": sl_usd,
            "slippage_bps": slippage_bps,
            "expires_in_secs": expires_in_secs,
        }
    )


def tp_sl_order(
    input_mint: str,
    output_mint: str,
    amount: Any,
    *,
    tp_usd: Optional[float] = None,
    sl_usd: Optional[float] = None,
    expires_in_secs: Optional[int] = None,
) -> Dict[str, Any]:
    """An intent to sell `amount` of a held `input_mint` into `output_mint` at
    a take-profit, a stop-loss, or both (one fills, the other cancels)."""
    return _drop_none(
        {
            "kind": "tp_sl",
            "input_mint": input_mint,
            "output_mint": output_mint,
            "amount": str(amount),
            "tp_usd": tp_usd,
            "sl_usd": sl_usd,
            "expires_in_secs": expires_in_secs,
        }
    )


def dca_order(
    input_mint: str,
    output_mint: str,
    per_order_amount: Any,
    count: int,
    interval_seconds: int,
    *,
    band_trigger_mint: Optional[str] = None,
    min_price_usd: Optional[float] = None,
    max_price_usd: Optional[float] = None,
    min_mc_usd: Optional[float] = None,
    max_mc_usd: Optional[float] = None,
) -> Dict[str, Any]:
    """A DCA intent: `count` rounds (2 to 1000) of `per_order_amount`, every
    `interval_seconds` (60 to one year). A band (`band_trigger_mint` plus a
    min and/or max price, or a min and/or max market cap, one kind) makes a
    round fill only while the trigger mint is inside it."""
    intent: Dict[str, Any] = {
        "kind": "dca",
        "input_mint": input_mint,
        "output_mint": output_mint,
        "per_order_amount": str(per_order_amount),
        "count": int(count),
        "interval_seconds": int(interval_seconds),
    }
    bounds = _drop_none(
        {
            "min_price_usd": min_price_usd,
            "max_price_usd": max_price_usd,
            "min_mc_usd": min_mc_usd,
            "max_mc_usd": max_mc_usd,
        }
    )
    if band_trigger_mint is not None or bounds:
        if band_trigger_mint is None:
            raise ValueError("a band needs band_trigger_mint (the volatile leg)")
        intent["band"] = dict(bounds, trigger_mint=band_trigger_mint)
    return intent


def _batch_target(agent_ids: Optional[Sequence[str]], all_agents: bool) -> Dict[str, Any]:
    """The body of a batch exit or withdraw: named agents, or all of them, never both."""
    if isinstance(agent_ids, str):
        raise TypeError("agent_ids is a list of ids, not one string")
    ids = list(agent_ids or [])
    if ids and all_agents:
        raise ValueError("pass agent_ids or all_agents=True, not both")
    if not ids and not all_agents:
        raise ValueError("name the agents in agent_ids, or pass all_agents=True")
    return {"all": True} if all_agents else {"agent_ids": ids}


def _to_sol_body(to_sol: Optional[bool]) -> Dict[str, Any]:
    """The optional exit-to-SOL choice of an exit. Omitted, the server applies the
    agent's own `exit_to_sol` setting (default True)."""
    if to_sol is None:
        return {}
    if not isinstance(to_sol, bool):
        raise TypeError("to_sol must be a bool or None")
    return {"to_sol": to_sol}


def _preset_body(name: str, overrides: Mapping[str, Any], dex: Optional[str]) -> Dict[str, Any]:
    return {"name": name, "overrides": dict(overrides), "dex": dex}


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
