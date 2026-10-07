"""Verify webhook deliveries from ponk.

ponk POSTs a signed JSON body to a URL you register. This module checks that
a delivery really came from ponk and has not been replayed. It is the whole
receiver side of the feature, in the standard library.

WHY THERE IS NO `client.create_webhook()`. Registering an endpoint is an
account-level act and the API scopes it to a signed-in SESSION, not to an API
key: a key cannot mint keys and it cannot register a URL that will receive
your account's activity either. So a key-holding client like this one cannot
manage endpoints, and pretending otherwise would ship a method that answers
401 forever. Register an endpoint from a signed-in session with
`POST /me/webhooks`, keep the `whsec_...` signing secret its response
carries, then verify here.

    from ponk.webhooks import verify, InvalidSignature

    @app.post("/ponk")
    def receive(request):
        try:
            event = verify(
                raw_body=request.get_data(),          # BYTES, not the parsed dict
                signature_header=request.headers["X-Ponk-Signature"],
                secret=MY_WEBHOOK_SECRET,
            )
        except InvalidSignature:
            return "", 400
        ...
        return "", 200            # anything but 2xx is a failure and will retry

THE ONE MISTAKE THAT MATTERS: verify the RAW BYTES you received. The MAC
covers the exact body on the wire, and `json.dumps(json.loads(body))` is not
guaranteed to reproduce it, so verifying a re-serialized dict will fail for
reasons that look like a ponk bug and are not.
"""

from __future__ import annotations

import hashlib
import hmac
import json
import time
from typing import Any, Mapping, Optional, Union

__all__ = [
    "EVENT_KINDS",
    "InvalidSignature",
    "WebhookEvent",
    "parse_signature_header",
    "verify",
]

#: Default replay window, in seconds, either side of the signed timestamp.
#: Five minutes is enough for clock skew between two hosts and short enough
#: that a captured delivery is not replayable an hour later.
DEFAULT_TOLERANCE_SECONDS = 300

#: Every event kind ponk can send, as of this release. Kept as a tuple for
#: recognition, NOT used to reject: the server may add a kind before this
#: package is updated, and dropping an unknown event would be worse than
#: handing it to you. `GET /me/webhooks/kinds` is the live list.
EVENT_KINDS = (
    "agent_recommendation",
    "agent_auto_executed",
    "agent_auto_exec_blocked",
    "agent_auto_exec_failed",
    "agent_quarantined",
    "agent_kill_switch",
    "agent_tp_sl_stop",
    "agent_rebalance_unverified",
    "agent_autonomous_enabled",
    "agent_autonomous_revoked",
    "agent_entry_opened",
    "agent_out_of_range",
    "agent_stalled",
    "agent_low_exit_gas",
    "agent_shared_wallet_hold",
    "mandate_expiring",
    "mandate_expired",
    "alert_rule",
)


class InvalidSignature(Exception):
    """A delivery did not verify. Answer non-2xx and do not act on the body.

    The message says which check failed because a receiver that cannot tell
    "wrong secret" from "too old" cannot debug its own integration. It never
    contains the secret or the computed digest.
    """


class WebhookEvent:
    """One verified delivery.

    Attributes mirror the JSON body one for one. `data` is the event's own
    metadata and its shape varies by `event`, so it stays a plain dict rather
    than being coerced into a class that would have to guess.
    """

    __slots__ = (
        "event",
        "event_id",
        "occurred_at",
        "severity",
        "title",
        "message",
        "user_id",
        "agent_id",
        "data",
        "raw",
    )

    def __init__(self, payload: Mapping[str, Any]) -> None:
        self.event: str = payload.get("event", "")
        self.event_id: Optional[str] = payload.get("event_id")
        self.occurred_at: Optional[str] = payload.get("occurred_at")
        self.severity: Optional[str] = payload.get("severity")
        self.title: Optional[str] = payload.get("title")
        self.message: Optional[str] = payload.get("message")
        self.user_id: Optional[str] = payload.get("user_id")
        self.agent_id: Optional[str] = payload.get("agent_id")
        self.data: Optional[Mapping[str, Any]] = payload.get("data")
        #: The decoded body exactly as sent, for anything not named above.
        self.raw: Mapping[str, Any] = payload

    def __repr__(self) -> str:  # pragma: no cover - debugging aid
        return "WebhookEvent(event=%r, agent_id=%r)" % (self.event, self.agent_id)


def parse_signature_header(header: str) -> "tuple[int, str]":
    """Split `t=<unix>,v1=<hex>` into its parts.

    Tolerates spaces and extra comma-separated fields, because a future
    version may add one and a receiver that breaks on it is a receiver that
    breaks on an upgrade it did not make.
    """
    parts = {}
    for chunk in header.split(","):
        if "=" not in chunk:
            continue
        key, _, value = chunk.partition("=")
        parts[key.strip()] = value.strip()
    if "t" not in parts or "v1" not in parts:
        raise InvalidSignature(
            "signature header is missing t= or v1=; got %r" % (header[:80],)
        )
    try:
        timestamp = int(parts["t"])
    except ValueError:
        raise InvalidSignature("signature timestamp is not an integer") from None
    return timestamp, parts["v1"]


def verify(
    raw_body: Union[bytes, str],
    signature_header: str,
    secret: str,
    tolerance_seconds: int = DEFAULT_TOLERANCE_SECONDS,
    now: Optional[float] = None,
) -> WebhookEvent:
    """Check a delivery and return it parsed. Raises `InvalidSignature`.

    `raw_body` is what arrived, bytes for preference. A `str` is encoded as
    UTF-8, which is what the sender used.

    `tolerance_seconds` bounds how old a delivery may be. Pass 0 to disable
    the age check, which you want only when replaying a captured delivery in
    a test; in production it is the replay window.
    """
    timestamp, sent_hex = parse_signature_header(signature_header)

    if isinstance(raw_body, str):
        body_bytes = raw_body.encode("utf-8")
    else:
        body_bytes = raw_body

    if tolerance_seconds:
        current = time.time() if now is None else now
        age = abs(current - timestamp)
        if age > tolerance_seconds:
            raise InvalidSignature(
                "delivery is %d seconds old, outside the %d second tolerance"
                % (int(age), tolerance_seconds)
            )

    # The MAC covers "<t>.<raw body>". The timestamp is INSIDE it, so an
    # attacker cannot age a captured delivery forward without breaking v1.
    signed = str(timestamp).encode("ascii") + b"." + body_bytes
    expected = hmac.new(secret.encode("utf-8"), signed, hashlib.sha256).hexdigest()

    # compare_digest, never ==. A byte-at-a-time comparison leaks how much of
    # a forged digest was right, which is enough to build the rest of it.
    if not hmac.compare_digest(expected, sent_hex):
        raise InvalidSignature(
            "signature does not match; check you are verifying the RAW request "
            "body and using the secret for this endpoint"
        )

    try:
        payload = json.loads(body_bytes.decode("utf-8"))
    except (UnicodeDecodeError, ValueError) as exc:
        # Verified but unreadable: worth its own message, because the secret
        # is evidently right and the problem is upstream of this library.
        raise InvalidSignature("delivery verified but its body is not JSON: %s" % exc)

    if not isinstance(payload, dict):
        raise InvalidSignature("delivery body is not a JSON object")

    return WebhookEvent(payload)
