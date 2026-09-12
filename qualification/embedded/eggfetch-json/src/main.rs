//! Embedded HTTPS + JSON + streaming consumer (Profile B).
//!
//! Non-product qualification fixture, semantically equivalent to the reqwest
//! JSON fixture: one reusable client, HTTPS GET, JSON request serialization,
//! JSON response deserialization, and streaming response iteration.
//!
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

    // HTTPS GET + streaming iteration.
    let mut resp = client.get(&format!("{base}/get"))?.send().await?;
    let mut stream = resp.bytes_stream()?;
    let mut total: u64 = 0;
    while let Some(chunk) = stream.next().await {
        total += chunk?.len() as u64;
    }
    println!("get bytes: {total}");

    // Native JSON request serialization.
    let payload = Payload {
        key: "value".to_string(),
        count: 3,
    };
    let mut post = client
        .post(&format!("{base}/post"))?
        .json(&payload)?
        .send()
        .await?;
    let echoed: serde_json::Value = post.json().await?;
    println!("echoed: {echoed}");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("EGGFETCH_FIXTURE_NOOP").is_ok() {
        let _client = Client::new();
        // Keep serde paths reachable without I/O.
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
