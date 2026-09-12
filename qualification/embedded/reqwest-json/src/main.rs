//! Reqwest HTTPS + JSON + streaming consumer (Profile B comparison).
//!
//! Non-product qualification fixture, semantically equivalent to
//! `eggfetch-json`: one reusable client, HTTPS GET + streaming, JSON POST,
//! and JSON response parsing. Uses reqwest's idiomatic `.json()` helpers;
//! the eggfetch fixture serializes manually until native `json()` lands.

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
        let p = Payload {
            key: "value".to_string(),
            count: 1,
        };
        let body = serde_json::to_vec(&p)?;
        let _back: Payload = serde_json::from_slice(&body)?;
        println!("ok");
        return Ok(());
    }
    let base = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://example.com".to_string());
    run(&base).await
}
