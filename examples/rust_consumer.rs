//! Example: Using eggfetch-core directly from Rust.
//!
//! This example demonstrates the core API without going through the FFI layer.
//! Run with: cargo run --example rust_consumer

use eggfetch_core::{Client, Timeout};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client with configuration
    let client = Client::builder()
        .timeout(Timeout::from_secs(30))
        .follow_redirects(true)
        .max_redirects(5)
        .user_agent("eggfetch-example/0.1")
        .automatic_decompression(true)
        .build();

    // Simple GET request
    println!("=== GET request ===");
    let resp = client.get("https://httpbin.org/get").send().await?;
    println!("Status: {}", resp.status());
    println!("URL: {}", resp.url());
    println!("Body length: {} bytes", resp.bytes().await?.len());

    // POST request with JSON body
    println!("\n=== POST request ===");
    let resp = client
        .post("https://httpbin.org/post")
        .header("Content-Type", "application/json")
        .body(serde_json::json!({"key": "value"}).to_string())
        .send()
        await?;
    println!("Status: {}", resp.status());

    // Request with timeout override
    println!("\n=== Request with timeout ===");
    let resp = client
        .get("https://httpbin.org/delay/1")
        .timeout(Timeout::from_secs(5))
        .send()
        .await?;
    println!("Status: {}", resp.status());

    // Request with basic auth
    println!("\n=== Basic auth ===");
    let resp = client
        .get("https://httpbin.org/basic-auth/user/passwd")
        .basic_auth("user", Some("passwd"))
        .send()
        .await?;
    println!("Status: {}", resp.status());

    // Streaming response
    println!("\n=== Streaming response ===");
    let mut resp = client.get("https://httpbin.org/stream/3").send().await?;
    let mut stream = resp.bytes_stream()?;
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        println!("  chunk: {} bytes", chunk.len());
    }

    println!("\nAll examples completed successfully!");
    Ok(())
}
