"""ponk - a thin Python client for the ponk public API.

    from ponk import PonkClient

    ponk = PonkClient(api_key="ponk_live_...")
    print(ponk.whoami().wallet_address)

See `README.md` for the quickstart and `ponk.client.PonkClient` for the method
list. Standard library only.
"""

from ._version import __version__
from .client import (
    DEFAULT_BASE_URL,
    DEFAULT_FUND_TIMEOUT,
    DEFAULT_TIMEOUT,
    PonkClient,
)
from .webhooks import (
    EVENT_KINDS,
    InvalidSignature,
    WebhookEvent,
    parse_signature_header,
    verify,
)
from .errors import (
    PonkAPIError,
    PonkAuthError,
    PonkBadRequestError,
    PonkConflictError,
    PonkError,
    PonkForbiddenError,
    PonkNotFoundError,
    PonkRateLimitedError,
    PonkServerError,
    PonkTransportError,
    PonkUnprocessableError,
)
from .models import (
    ActionLog,
    ActionReceipt,
    Agent,
    AgentPerformance,
    AgentPosition,
    AgentWallet,
    AutonomousState,
    BinShare,
    ClaimPayout,
    ClaimPayoutToken,
    ComponentStatus,
    ExitSnapshot,
    FeeRates,
    Health,
    HealthChecks,
    Mandate,
    PonkPerks,
    PoolSnapshot,
    Position,
    RangeCost,
    TokenAmount,
    WhoAmI,
    Withdrawal,
)


__all__ = [
    "EVENT_KINDS",
    "InvalidSignature",
    "WebhookEvent",
    "parse_signature_header",
    "verify",
    "__version__",
    "DEFAULT_BASE_URL",
    "DEFAULT_FUND_TIMEOUT",
    "DEFAULT_TIMEOUT",
    "PonkClient",
    "PonkAPIError",
    "PonkAuthError",
    "PonkBadRequestError",
    "PonkConflictError",
    "PonkError",
    "PonkForbiddenError",
    "PonkNotFoundError",
    "PonkRateLimitedError",
    "PonkServerError",
    "PonkTransportError",
    "PonkUnprocessableError",
    "ActionLog",
    "ActionReceipt",
    "Agent",
    "AgentPerformance",
    "AgentPosition",
    "AgentWallet",
    "AutonomousState",
    "BinShare",
    "ClaimPayout",
    "ClaimPayoutToken",
    "ComponentStatus",
    "ExitSnapshot",
    "FeeRates",
    "Health",
    "HealthChecks",
    "Mandate",
    "PonkPerks",
    "PoolSnapshot",
    "Position",
    "RangeCost",
    "TokenAmount",
    "WhoAmI",
    "Withdrawal",
]
