//! Minimal embedded HTTPS streaming consumer (Profile A).
//!
//! Non-product qualification fixture: constructs one reusable client,
//! performs an HTTPS GET, and iterates the response as a stream.
//! The binary compiles these paths so release size reflects the HTTP stack,
//! not benchmark scaffolding. No cookies, proxy, compression, multipart,
//! H2, or H3 are exercised here.

use eggfetch_core::Client;
use futures_util::StreamExt;

async fn run(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    // One reusable client for the process lifetime.
    let client = Client::new();
    let mut resp = client.get(url)?.send().await?;
    let status = resp.status();
    println!("status: {status}");
    let mut stream = resp.bytes_stream()?;
    let mut total: u64 = 0;
    while let Some(chunk) = stream.next().await {
        total += chunk?.len() as u64;
    }
    println!("bytes: {total}");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Smoke path constructs a client without I/O so loopback checks do not
    // require network access.
    if std::env::var("EGGFETCH_FIXTURE_NOOP").is_ok() {
        let _client = Client::new();
        println!("ok");
        return Ok(());
    }
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://example.com/".to_string());
    run(&url).await
}
