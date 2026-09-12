//! Minimal reqwest HTTPS streaming consumer (Profile A comparison).
//!
//! Non-product qualification fixture, semantically equivalent to
//! `eggfetch-min`: one reusable client, HTTPS GET, streaming iteration.
//! No cookies, proxy, compression, multipart, or H2 beyond reqwest defaults.

use futures_util::StreamExt;

async fn run(url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let resp = client.get(url).send().await?;
    println!("status: {}", resp.status());
    let mut stream = resp.bytes_stream();
    let mut total: u64 = 0;
    while let Some(chunk) = stream.next().await {
        total += chunk?.len() as u64;
    }
    println!("bytes: {total}");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("EGGFETCH_FIXTURE_NOOP").is_ok() {
        let _client = reqwest::Client::new();
        println!("ok");
        return Ok(());
    }
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://example.com/".to_string());
    run(&url).await
}
