//! Ordinary default-feature consumer (Profile C, informational).
//!
//! Non-product fixture: same JSON + streaming workload as `reqwest-json`
//! but built on reqwest default features (native-TLS + charset + http2 +
//! system-proxy) plus `json`/`stream`. Kept separate from the minimal
//! profiles because defaults intentionally include different conveniences.

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Payload {
    key: String,
    count: u32,
}

async fn run(base: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    let resp = client.get(format!("{base}/get")).send().await?;
    let mut stream = resp.bytes_stream();
    let mut total: u64 = 0;
    while let Some(chunk) = stream.next().await {
        total += chunk?.len() as u64;
    }
    println!("get bytes: {total}");

    let payload = Payload {
        key: "value".to_string(),
        count: 3,
    };
    let resp = client
        .post(format!("{base}/post"))
        .json(&payload)
        .send()
        .await?;
    let echoed: serde_json::Value = resp.json().await?;
    println!("echoed: {echoed}");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("EGGFETCH_FIXTURE_NOOP").is_ok() {
        let _client = reqwest::Client::new();
        println!("ok");
        return Ok(());
    }
    let base = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://example.com".to_string());
    run(&base).await
}
