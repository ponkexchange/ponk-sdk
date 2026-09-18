"""ponk - a thin Python client for the ponk public API.

    from ponk import PonkClient

    ponk = PonkClient(api_key="ponk_live_...")
    print(ponk.whoami().wallet_address)

See `README.md` for the quickstart and `ponk.client.PonkClient` for the method
list. Standard library only.
"""

from .client import (
    DEFAULT_BASE_URL,
    DEFAULT_FUND_TIMEOUT,
    DEFAULT_TIMEOUT,
    PonkClient,
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
    BinShare,
    ClaimPayout,
    ClaimPayoutToken,
    ComponentStatus,
    ExitSnapshot,
    FeeRates,
    Health,
    HealthChecks,
    PonkPerks,
    PoolSnapshot,
    Position,
    TokenAmount,
    WhoAmI,
    Withdrawal,
)

__version__ = "0.1.0"

__all__ = [
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
    "BinShare",
    "ClaimPayout",
    "ClaimPayoutToken",
    "ComponentStatus",
    "ExitSnapshot",
    "FeeRates",
    "Health",
    "HealthChecks",
    "PonkPerks",
    "PoolSnapshot",
    "Position",
    "TokenAmount",
    "WhoAmI",
    "Withdrawal",
]
