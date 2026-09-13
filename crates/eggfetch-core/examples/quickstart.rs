//! Quickstart for `eggfetch-core`: configured client, GET/POST, timeout
//! override, auth, and streaming.
//!
//! Run against the default demo endpoints:
//!
//! ```sh
//! cargo run -p eggfetch-core --example quickstart
//! ```
//!
//! Or point at any compatible base URL (handy for a local stub):
//!
//! ```sh
//! cargo run -p eggfetch-core --example quickstart -- http://127.0.0.1:8000
//! ```

use eggfetch_core::{AuthScheme, Client, Timeout};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let base = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "https://httpbin.org".to_owned());

    let client = Client::builder()
        .timeout(Timeout::from_secs(30))
        .follow_redirects(true)
        .max_redirects(5)
        .user_agent("eggfetch-example/0.1")
        .automatic_decompression(true)
        .build();

    println!("=== GET request ===");
    let mut resp = client.get(&format!("{base}/get"))?.send().await?;
    let status = resp.status();
    let len = resp.bytes().await?.len();
    println!("Status: {status}");
    println!("Body length: {len} bytes");

    println!("\n=== POST request ===");
    let resp = client
        .post(&format!("{base}/post"))?
        .header("Content-Type", "application/json")
        .body(r#"{"key": "value"}"#)
        .send()
        .await?;
    println!("Status: {}", resp.status());

    println!("\n=== Request with timeout override ===");
    let resp = client
        .get(&format!("{base}/delay/1"))?
        .timeout(Timeout::from_secs(5))
        .send()
        .await?;
    println!("Status: {}", resp.status());

    println!("\n=== Basic auth ===");
    let resp = client
        .get(&format!("{base}/basic-auth/user/passwd"))?
        .auth(AuthScheme::basic("user", "passwd")?)
        .send()
        .await?;
    println!("Status: {}", resp.status());

    println!("\n=== Streaming response ===");
    let mut resp = client.get(&format!("{base}/stream/3"))?.send().await?;
    let mut stream = resp.bytes_stream()?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        println!("  chunk: {} bytes", chunk.len());
    }

    println!("\nAll examples completed successfully!");
    Ok(())
}
