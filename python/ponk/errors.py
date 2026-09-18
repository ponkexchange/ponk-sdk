"""Exceptions for the ponk public API.

Every error the API returns carries the same JSON envelope:

    {"error": {"code": "...", "message": "...", "request_id": "..."}}

`error_from_response` turns one of those into the exception below that matches
the HTTP status. The `code` string is kept verbatim rather than parsed into an
enum: 422 can carry a risk code (a Token-2022 mint on Orca, a transfer-hook
mint) and inventing a closed set here would drop codes the server adds later.

Nothing is guessed. If the body is not the envelope (a proxy 502, an HTML error
page), `code` is `http_<status>` and `body` holds what actually arrived.
"""

from __future__ import annotations

from typing import Any, Mapping, Optional

__all__ = [
    "PonkError",
    "PonkTransportError",
    "PonkAPIError",
    "PonkBadRequestError",
    "PonkAuthError",
    "PonkForbiddenError",
    "PonkNotFoundError",
    "PonkConflictError",
    "PonkRateLimitedError",
    "PonkUnprocessableError",
    "PonkServerError",
    "error_from_response",
]


class PonkError(Exception):
    """Base class. Catch this to catch everything this client raises."""


class PonkTransportError(PonkError):
    """The request never produced an HTTP response.

    A DNS failure, a refused connection, a TLS failure or a timeout. A
    fund-moving call that times out has NOT necessarily failed: the server
    keeps running the work. See the README on retrying an exit.
    """


class PonkAPIError(PonkError):
    """The server answered with a non-2xx status."""

    def __init__(
        self,
        message: str,
        *,
        status: int,
        code: str,
        request_id: Optional[str] = None,
        body: Any = None,
    ) -> None:
        super().__init__(message)
        self.message = message
        self.status = status
        self.code = code
        self.request_id = request_id
        self.body = body

    def __str__(self) -> str:
        parts = ["ponk api {0}: {1} ({2})".format(self.status, self.message, self.code)]
        if self.request_id:
            parts.append("request_id={0}".format(self.request_id))
        return " ".join(parts)


class PonkBadRequestError(PonkAPIError):
    """400. The request was malformed, or a strategy config did not validate.

    `code` is `invalid_strategy_config` when the config did not match the
    declared strategy; `message` names the offending field.
    """


class PonkAuthError(PonkAPIError):
    """401. Missing, malformed, unknown or revoked key.

    Unknown and revoked are deliberately indistinguishable. Do not retry.
    """


class PonkForbiddenError(PonkAPIError):
    """403. The key is valid but read-only. Mint a `trade` key. Do not retry."""


class PonkNotFoundError(PonkAPIError):
    """404. No such object, or it belongs to another account.

    The two are deliberately indistinguishable, so a key cannot be used to
    probe for another account's agents.
    """


class PonkConflictError(PonkAPIError):
    """409. Conflicts with in-flight work, for example an exit already running."""


class PonkRateLimitedError(PonkAPIError):
    """429. Back off and retry."""


class PonkUnprocessableError(PonkAPIError):
    """422. Understood but cannot apply.

    Compounding an agent with no open position, or a venue capability the
    platform refuses to execute. `code` carries the risk code when there is
    one.
    """


class PonkServerError(PonkAPIError):
    """5xx. The message is deliberately generic; quote `request_id` to support."""


_BY_STATUS = {
    400: PonkBadRequestError,
    401: PonkAuthError,
    403: PonkForbiddenError,
    404: PonkNotFoundError,
    409: PonkConflictError,
    422: PonkUnprocessableError,
    429: PonkRateLimitedError,
}


def error_from_response(status: int, payload: Any) -> PonkAPIError:
    """Build the exception for one non-2xx response.

    `payload` is the decoded JSON body, or the raw text when it was not JSON.
    """
    code = "http_{0}".format(status)
    message = "request failed with status {0}".format(status)
    request_id = None

    if isinstance(payload, Mapping):
        envelope = payload.get("error")
        if isinstance(envelope, Mapping):
            code = str(envelope.get("code") or code)
            raw_message = envelope.get("message")
            if raw_message:
                message = str(raw_message)
            raw_request_id = envelope.get("request_id")
            if raw_request_id:
                request_id = str(raw_request_id)

    cls = _BY_STATUS.get(status)
    if cls is None:
        cls = PonkServerError if status >= 500 else PonkAPIError
    return cls(
        message,
        status=status,
        code=code,
        request_id=request_id,
        body=payload,
    )
