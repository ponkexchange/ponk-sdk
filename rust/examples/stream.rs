//! Stream the public social feed and print each event as it happens.
//!
//!     cargo run --example stream
//!
//! No key needed: the public stream is anonymous. Press Ctrl-C to stop.
//!
//! For your own following graph instead of everyone's, pass a key and set
//! `public` to `false`:
//!
//!     let ponk = Client::new(base, std::env::var("PONK_API_KEY")?)?;
//!     let mut stream = ponk.stream_social(false).await?;
//!
//! The transport is Server-Sent Events, not a WebSocket. The stream holds its
//! connection open and blocks between events; a `resync` means reload from
//! `live_feed` because the stream may have missed rows.

use ponk::{Client, Error, DEFAULT_BASE_URL};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let base = std::env::var("PONK_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
    // The public stream needs no key, so a public client is enough.
    let ponk = Client::public(base)?;

    println!("connecting to the public social stream (Ctrl-C to stop)...");
    let mut stream = ponk.stream_social(true).await?;

    while let Some(event) = stream.next_event().await? {
        match event.event.as_str() {
            "ready" => println!("connected."),
            "resync" => println!("resync: reload from live_feed() to fill the gap."),
            // The row is the social event exactly as the server shaped it; its
            // fields are printed as-is rather than assumed.
            name => println!("{}: {}", name, event.data),
        }
    }

    println!("stream closed by the server.");
    Ok(())
}
