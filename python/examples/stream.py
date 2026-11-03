"""Stream the public social feed and print each event as it happens.

    python examples/stream.py

No key needed: the public stream is anonymous. Press Ctrl-C to stop.

For your own following graph instead of everyone's, pass a key and set
public=False:

    PonkClient(api_key="ponk_live_...").stream_social(public=False)

The transport is Server-Sent Events, not a WebSocket. stream_social() is a
long-lived generator that blocks between events; a "resync" means reload from
live_feed() because the stream may have missed rows.
"""
from ponk import PonkClient


def main() -> None:
    ponk = PonkClient()
    print("connecting to the public social stream (Ctrl-C to stop)...")
    for event in ponk.stream_social():
        if event.event == "ready":
            print("connected.")
        elif event.event == "resync":
            print("resync: reload from live_feed() to fill the gap.")
        else:
            # The row is the social event exactly as the server shaped it; its
            # fields are printed as-is rather than assumed.
            print("{0}: {1}".format(event.event, event.data))


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        print("\nstopped.")
