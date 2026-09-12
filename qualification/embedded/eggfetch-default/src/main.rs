//! Ordinary default-feature consumer (Profile C, informational).
//!
//! Non-product fixture: same JSON + streaming workload as `eggfetch-json`
//! but built on `eggfetch-core` default features (H1 + Rustls + native
//! roots + `json` flag). Kept separate from the minimal embedding profiles
//! because defaults intentionally include different conveniences.

use eggfetch_core::Client;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Payload {
    key: String,
    count: u32,
}

async fn run(base: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();

    let mut resp = client.get(&format!("{base}/get"))?.send().await?;
    let mut stream = resp.bytes_stream()?;
    let mut total: u64 = 0;
    while let Some(chunk) = stream.next().await {
        total += chunk?.len() as u64;
    }
    println!("get bytes: {total}");

    let payload = Payload {
        key: "value".to_string(),
        count: 3,
    };
    let body = serde_json::to_vec(&payload)?;
    let mut post = client
        .post(&format!("{base}/post"))?
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await?;
    let bytes = post.bytes().await?;
    let echoed: serde_json::Value = serde_json::from_slice(&bytes)?;
    println!("echoed: {echoed}");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("EGGFETCH_FIXTURE_NOOP").is_ok() {
        let _client = Client::new();
        println!("ok");
        return Ok(());
    }
    let base = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://example.com".to_string());
    run(&base).await
}
