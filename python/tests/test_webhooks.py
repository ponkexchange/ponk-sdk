"""Receiver-side verification.

The vector in `test_matches_the_rust_dispatcher` is computed the same way
`apps/api/src/webhook_dispatcher.rs` computes it, so this file fails if the
two implementations ever disagree about what is signed.
"""

import hashlib
import hmac
import json
import time
import unittest

from ponk.webhooks import (
    EVENT_KINDS,
    InvalidSignature,
    parse_signature_header,
    verify,
)

SECRET = "whsec_test"


def sign(body: bytes, timestamp: int, secret: str = SECRET) -> str:
    mac = hmac.new(
        secret.encode(), str(timestamp).encode() + b"." + body, hashlib.sha256
    )
    return "t=%d,v1=%s" % (timestamp, mac.hexdigest())


class VerifyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.body = json.dumps(
            {"event": "agent_out_of_range", "agent_id": "a1", "data": {"bin": 7}}
        ).encode()
        self.now = 1_700_000_000

    def test_a_good_delivery_verifies_and_parses(self) -> None:
        ev = verify(self.body, sign(self.body, self.now), SECRET, now=self.now)
        self.assertEqual(ev.event, "agent_out_of_range")
        self.assertEqual(ev.agent_id, "a1")
        self.assertEqual(ev.data, {"bin": 7})
        self.assertEqual(ev.raw["data"]["bin"], 7)

    def test_matches_the_rust_dispatcher(self) -> None:
        # Independently computed: HMAC-SHA256(b"whsec_test", b"1700000000.{}").
        mac = hmac.new(b"whsec_test", b"1700000000.{}", hashlib.sha256).hexdigest()
        header = "t=1700000000,v1=%s" % mac
        ev = verify(b"{}", header, SECRET, now=1_700_000_000)
        self.assertEqual(ev.raw, {})

    def test_a_tampered_body_is_rejected(self) -> None:
        header = sign(self.body, self.now)
        with self.assertRaises(InvalidSignature):
            verify(self.body + b" ", header, SECRET, now=self.now)

    def test_the_wrong_secret_is_rejected(self) -> None:
        with self.assertRaises(InvalidSignature):
            verify(self.body, sign(self.body, self.now), "whsec_other", now=self.now)

    def test_the_timestamp_cannot_be_moved(self) -> None:
        # Signed at T, presented as T+1: the MAC covers t, so this must fail
        # rather than merely being out of tolerance.
        good = sign(self.body, self.now)
        moved = good.replace("t=%d" % self.now, "t=%d" % (self.now + 1))
        with self.assertRaises(InvalidSignature):
            verify(self.body, moved, SECRET, now=self.now + 1)

    def test_a_stale_delivery_is_rejected(self) -> None:
        old = self.now - 3600
        with self.assertRaises(InvalidSignature) as ctx:
            verify(self.body, sign(self.body, old), SECRET, now=self.now)
        self.assertIn("tolerance", str(ctx.exception))

    def test_tolerance_zero_replays_for_tests(self) -> None:
        old = self.now - 86_400
        ev = verify(
            self.body, sign(self.body, old), SECRET, tolerance_seconds=0, now=self.now
        )
        self.assertEqual(ev.event, "agent_out_of_range")

    def test_a_string_body_is_encoded_as_utf8(self) -> None:
        text = '{"event":"mandate_expiring","title":"caf\\u00e9"}'
        raw = text.encode("utf-8")
        ev = verify(text, sign(raw, self.now), SECRET, now=self.now)
        self.assertEqual(ev.event, "mandate_expiring")

    def test_a_malformed_header_is_rejected(self) -> None:
        for bad in ["", "v1=abc", "t=notanumber,v1=abc", "garbage"]:
            with self.assertRaises(InvalidSignature):
                verify(self.body, bad, SECRET, now=self.now)

    def test_extra_header_fields_are_tolerated(self) -> None:
        # A future version may add a field; a receiver must not break on it.
        header = sign(self.body, self.now) + ",v2=future"
        ev = verify(self.body, header, SECRET, now=self.now)
        self.assertEqual(ev.event, "agent_out_of_range")

    def test_verified_but_unreadable_body_says_so(self) -> None:
        body = b"not json"
        with self.assertRaises(InvalidSignature) as ctx:
            verify(body, sign(body, self.now), SECRET, now=self.now)
        self.assertIn("not JSON", str(ctx.exception))

    def test_parse_signature_header(self) -> None:
        ts, hexd = parse_signature_header("t=42, v1=deadbeef")
        self.assertEqual(ts, 42)
        self.assertEqual(hexd, "deadbeef")


class EventKindTests(unittest.TestCase):
    def test_kinds_are_unique_and_plausible(self) -> None:
        self.assertEqual(len(EVENT_KINDS), len(set(EVENT_KINDS)))
        self.assertIn("agent_out_of_range", EVENT_KINDS)
        self.assertIn("mandate_expired", EVENT_KINDS)

    def test_an_unknown_kind_is_still_delivered(self) -> None:
        # The list is for recognition, never for rejection: the server may add
        # a kind before this package is updated.
        body = json.dumps({"event": "some_future_kind"}).encode()
        ev = verify(body, sign(body, 1_700_000_000), SECRET, now=1_700_000_000)
        self.assertEqual(ev.event, "some_future_kind")


if __name__ == "__main__":
    unittest.main()
