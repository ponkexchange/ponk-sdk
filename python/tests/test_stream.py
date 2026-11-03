"""The Server-Sent Events parser and stream_social(), with no network."""
import unittest

from ponk import PonkClient, StreamEvent
from ponk.client import _iter_sse
from ponk.errors import PonkError


class _FakeResponse:
    def __init__(self, lines):
        self._lines = lines
        self.closed = False

    def __iter__(self):
        return iter(self._lines)

    def close(self):
        self.closed = True


class _FakeOpener:
    def __init__(self, response):
        self._response = response
        self.opened = None

    def open(self, request, timeout=None):
        self.opened = request
        return self._response


SAMPLE = [
    b": keep-alive\n",
    b"event: ready\n",
    b"data: {}\n",
    b"\n",
    b"event: row\n",
    b'data: {"kind": "call", "handle": "alice"}\n',
    b"\n",
    b"event: resync\n",
    b"data: {}\n",
    b"\n",
]


class SseParserTest(unittest.TestCase):
    def test_frames(self):
        frames = list(_iter_sse(iter(SAMPLE)))
        self.assertEqual(
            frames,
            [
                ("ready", "{}"),
                ("row", '{"kind": "call", "handle": "alice"}'),
                ("resync", "{}"),
            ],
        )

    def test_multiline_data_joins_with_newline(self):
        frames = list(_iter_sse(iter([b"data: a\n", b"data: b\n", b"\n"])))
        self.assertEqual(frames, [("message", "a\nb")])

    def test_trailing_frame_without_blank_line_is_dropped(self):
        # SSE dispatches on a blank line; an unterminated final frame is not
        # emitted, which is the correct, lossless behaviour on a cut stream.
        self.assertEqual(list(_iter_sse(iter([b"data: x\n"]))), [])


class StreamSocialTest(unittest.TestCase):
    def test_yields_parsed_events(self):
        opener = _FakeOpener(_FakeResponse(SAMPLE))
        ponk = PonkClient(opener=opener)
        events = list(ponk.stream_social())
        self.assertTrue(all(isinstance(e, StreamEvent) for e in events))
        self.assertEqual([e.event for e in events], ["ready", "row", "resync"])
        self.assertEqual(events[1].data, {"kind": "call", "handle": "alice"})
        self.assertTrue(opener._response.closed)
        self.assertTrue(opener.opened.get_full_url().endswith("/public/social/stream"))
        self.assertEqual(opener.opened.get_header("Accept"), "text/event-stream")

    def test_private_stream_without_a_key_raises(self):
        ponk = PonkClient(opener=_FakeOpener(_FakeResponse([])))
        with self.assertRaises(PonkError):
            list(ponk.stream_social(public=False))

    def test_private_stream_sends_the_key(self):
        opener = _FakeOpener(_FakeResponse(SAMPLE))
        ponk = PonkClient(api_key="ponk_live_test", opener=opener)
        list(ponk.stream_social(public=False))
        self.assertTrue(opener.opened.get_full_url().endswith("/me/social/stream"))
        self.assertEqual(opener.opened.get_header("Authorization"), "Bearer ponk_live_test")


if __name__ == "__main__":
    unittest.main()
