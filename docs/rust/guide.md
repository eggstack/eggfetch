# eggfetch-core Rust API Guide

eggfetch-core is the async HTTP client engine that powers the eggfetch ecosystem. All networking, connection pooling, redirect following, authentication, and retry logic lives here. The Python bindings and CLI are thin adapters around this crate.

## Adding as a Dependency

Add `eggfetch-core` to your `Cargo.toml`:

```toml
[dependencies]
eggfetch-core = { version = "0.1", features = ["http1", "tls-rustls"] }
```

Enable optional features as needed:

| Feature | Description |
|---|---|
| `http1` | HTTP/1.1 support (default) |
| `http2` | HTTP/2 support via ALPN |
| `http3` | HTTP/3 over QUIC (experimental) |
| `tls-rustls` | TLS via rustls (default) |
| `cookies` | Cookie jar support |
| `proxy` | HTTP/HTTPS proxy support |
| `multipart` | Multipart form-data encoding |
| `compression-gzip` | Gzip decompression |
| `compression-brotli` | Brotli decompression |
| `compression-zstd` | Zstandard decompression |
| `compression-deflate` | Deflate decompression |

For a full-featured client:

```toml
eggfetch-core = { version = "0.1", features = ["http1", "http2", "tls-rustls", "cookies", "proxy", "multipart", "compression-gzip", "compression-brotli", "compression-zstd"] }
```

## Creating a Client

The `Client` manages connection pooling and shared configuration. Create one and reuse it for multiple requests.

```rust
use eggfetch_core::Client;

// Default client
let client = Client::new();

// Builder pattern
let client = Client::builder()
    .user_agent("my-app/1.0")
    .timeout(Timeout::from_secs(30))
    .max_idle_connections(100)
    .build();
```

## Making Requests

The client provides convenience methods for each HTTP method. All return a `RequestBuilder`:

```rust
let resp = client.get("https://example.com")?.send().await?;
let resp = client.post("https://api.example.com/data")?.send().await?;
let resp = client.put("https://api.example.com/resource")?.send().await?;
let resp = client.delete("https://api.example.com/resource/1")?.send().await?;
let resp = client.patch("https://api.example.com/resource/1")?.send().await?;
let resp = client.head("https://example.com")?.send().await?;
let resp = client.options("https://example.com")?.send().await?;

// Arbitrary method
use http::Method;
let resp = client.request(Method::from_bytes("PURGE")?, "https://example.com")?
    .send().await?;
```

## RequestBuilder API

The builder is fluent and lets you configure headers, query params, body, timeouts, auth, proxy, and retry per-request.

### Headers

```rust
let resp = client.get("https://example.com")?
    .header("Accept", "application/json")
    .header("X-Custom", "value")
    .send().await?;
```

### Query Parameters

```rust
let resp = client.get("https://api.example.com/search")?
    .query("q", "rust")
    .query("page", "1")
    .send().await?;
```

### Request Body

```rust
use bytes::Bytes;

// Raw bytes
let resp = client.post("https://api.example.com/data")?
    .bytes(b"raw payload")
    .send().await?;

// From a RequestBody enum
use eggfetch_core::RequestBody;
let resp = client.post("https://api.example.com/data")?
    .body(RequestBody::from(Bytes::from("hello")))
    .send().await?;
```

### Timeout

```rust
use std::time::Duration;
use eggfetch_core::Timeout;

// Simple: 5 seconds on pool, connect, write, read phases
let resp = client.get("https://example.com")?
    .timeout(Timeout::from_secs(5))
    .send().await?;

// Builder: configure individual phases
let timeout = Timeout::builder()
    .connect(Duration::from_secs(3))
    .read(Duration::from_secs(10))
    .total(Duration::from_secs(30))
    .build();

let resp = client.get("https://slow.example.com")?
    .timeout(timeout)
    .send().await?;
```

### Auth

```rust
use eggfetch_core::{BasicAuth, BearerAuth, AuthScheme};

// Basic auth
let resp = client.get("https://api.example.com")?
    .auth(AuthScheme::basic("user", "pass")?)
    .send().await?;

// Bearer token
let resp = client.get("https://api.example.com")?
    .auth(AuthScheme::bearer("my-token")?)
    .send().await?;

// Disable client-level auth for one request
let resp = client.get("https://public.example.com")?
    .without_auth()
    .send().await?;
```

### Proxy Override

Requires the `proxy` feature.

```rust
use eggfetch_core::Proxy;

let proxy = Proxy::all("http://proxy:8080")?;

// Override client proxy for one request
let resp = client.get("https://example.com")?
    .proxy(&proxy)
    .send().await?;

// Bypass proxy for one request
let resp = client.get("https://internal.example.com")?
    .without_proxy()
    .send().await?;
```

### Retry Override

```rust
use eggfetch_core::RetryPolicy;

let policy = RetryPolicy::builder()
    .max_attempts(3)
    .retry_status(503)
    .build();

let resp = client.get("https://api.example.com")?
    .retry(policy)
    .send().await?;

// Disable retries for one request
let resp = client.get("https://api.example.com")?
    .without_retry()
    .send().await?;
```

### Decompression Override

```rust
// Disable decompression for one request
let resp = client.get("https://example.com")?
    .decompress(false)
    .send().await?;
```

### Building Without Sending

```rust
let request = client.get("https://example.com")?
    .header("Accept", "text/html")
    .query("q", "test")
    .build()?;

// Inspect the request
println!("Method: {}", request.method());
println!("URL: {}", request.url());
```

## Response API

```rust
let mut response = client.get("https://api.example.com/data")?
    .header("Accept", "application/json")
    .send().await?;

// Status and metadata
let status = response.status();           // StatusCode
let version = response.version();         // http::Version
let url = response.url();                 // &Url (final URL after redirects)
let is_success = response.is_success();   // bool (2xx check)

// Headers (http::HeaderMap)
let content_type = response.headers().get("content-type");

// Redirect history
let history = response.history();         // &[HistoryEntry]
for entry in history {
    println!("{} -> {}", entry.status(), entry.url());
}

// Read the body (consumes it exactly once)
let bytes = response.bytes().await?;      // Bytes
let text = response.text().await?;        // String (UTF-8)

// Streaming
let mut stream = response.bytes_stream()?;
while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
    let chunk = chunk?;
    process(chunk);
}

// Text line streaming
let mut lines = response.text_lines()?;
while let Some(line) = futures_util::StreamExt::next(&mut lines).await {
    let line = line?;
    println!("{line}");
}
```

## Headers API

The `Headers` type wraps `http::HeaderMap` with a simpler string-based API:

```rust
use eggfetch_core::Headers;

let mut headers = Headers::new();
headers.insert("Content-Type", "application/json")?;
headers.append("Set-Cookie", "a=1")?;
headers.append("Set-Cookie", "b=2")?;

assert!(headers.contains("content-type"));
assert_eq!(headers.get("content-type").unwrap().to_str().unwrap(), "application/json");
assert_eq!(headers.get_all("set-cookie").len(), 2);
assert_eq!(headers.len(), 2);

for (name, value) in headers.iter() {
    println!("{name}: {value:?}");
}
```

## Error Handling

eggfetch-core uses a single `Error` enum with a `kind()` method for programmatic matching:

```rust
use eggfetch_core::Error;

match client.get("https://example.com")?.send().await {
    Ok(response) => { /* ... */ }
    Err(err) => {
        eprintln!("Error: {err}");
        match err.kind() {
            "invalid_url" => { /* bad URL */ }
            "connect" => { /* connection failed */ }
            "timeout_connect" | "timeout_read" => { /* timeout */ }
            "tls" => { /* TLS error */ }
            "proxy_connect" => { /* proxy error */ }
            "http2_go_away" => { /* HTTP/2 GOAWAY */ }
            "too_many_redirects" => { /* redirect loop */ }
            _ => { /* other */ }
        }
    }
}
```

## Timeout Configuration

Timeouts are phase-aware. Each field is optional:

```rust
use std::time::Duration;
use eggfetch_core::Timeout;

// Simple: same duration on pool, connect, write, read
let t = Timeout::from_secs(5);

// Builder: individual phases
let t = Timeout::builder()
    .pool(Duration::from_secs(2))
    .connect(Duration::from_secs(5))
    .read(Duration::from_secs(30))
    .total(Duration::from_secs(60))
    .build();

// Struct literal
let t = Timeout {
    pool: Some(Duration::from_secs(1)),
    read: Some(Duration::from_secs(10)),
    ..Timeout::default()
};
```

Request-level timeouts override client-level timeouts on a per-field basis. Only the fields present in the request-level timeout replace the corresponding client-level fields.

## Redirect Policy

```rust
use eggfetch_core::RedirectPolicy;

// Default: do not follow redirects
let client = Client::new();

// Follow redirects (up to 20)
let client = Client::builder()
    .follow_redirects(true)
    .max_redirects(20)
    .build();

// Custom policy
let policy = RedirectPolicy::new(true, 10);
let client = Client::builder()
    .redirect_policy(policy)
    .build();
```

On cross-origin redirects, sensitive headers (`Authorization`, `Cookie`, `Proxy-Authorization`) are stripped. On 301/302 POST redirects, the method is rewritten to GET and the body is dropped.

## Authentication

```rust
use eggfetch_core::{Client, AuthScheme};

// Client-level auth applied to every request
let client = Client::builder()
    .auth(AuthScheme::basic("user", "pass")?)
    .build();

// Override per-request
let resp = client.get("https://other-api.com")?
    .auth(AuthScheme::bearer("other-token")?)
    .send().await?;

// Disable for one request
let resp = client.get("https://public.com")?
    .without_auth()
    .send().await?;
```

Credentials are redacted in `Debug` and `Display` output. CR/LF characters in credentials are rejected to prevent header injection.

## Proxy Configuration

Requires the `proxy` feature.

```rust
use eggfetch_core::{Proxy, NoProxy};

let proxy = Proxy::all("http://proxy:8080")?;

// With NO_PROXY bypass rules
let no_proxy = NoProxy::parse("localhost,127.0.0.1,.internal.com")?;
let proxy = proxy.no_proxy(no_proxy);

// With proxy authentication
use eggfetch_core::ProxyAuth;
let proxy = proxy.auth(ProxyAuth::basic("user", "pass")?);

let client = Client::builder()
    .proxy(proxy)
    .build();
```

eggfetch does not read `HTTP_PROXY` or `HTTPS_PROXY` environment variables. Proxy configuration is explicit only.

## TLS Configuration

```rust
use eggfetch_core::TlsConfig;

// Default (native roots with WebPKI fallback)
let config = TlsConfig::default();

// Custom CA bundle
let config = TlsConfig::builder()
    .ca_certificate_path("/path/to/ca-bundle.pem")?
    .build();

// Client certificate (mTLS)
let config = TlsConfig::builder()
    .client_cert_path("/path/to/cert.pem", "/path/to/key.pem")?
    .build();

// Disable verification (testing only)
let config = TlsConfig::builder()
    .danger_accept_invalid_certs(true)
    .build();

// TLS version bounds
let config = TlsConfig::builder()
    .min_version(TlsVersion::Tls12)
    .max_version(TlsVersion::Tls13)
    .build();

let client = Client::builder()
    .tls_config(config)
    .build();
```

## Retry Policy

```rust
use eggfetch_core::RetryPolicy;
use std::time::Duration;

let policy = RetryPolicy::builder()
    .max_attempts(3)
    .backoff_factor(0.2)
    .initial_delay(Duration::from_millis(500))
    .max_delay(Duration::from_secs(30))
    .retry_status(429)
    .retry_status(503)
    .respect_retry_after(true)
    .max_elapsed(Duration::from_secs(120))
    .build();

let client = Client::builder()
    .retry(policy)
    .build();
```

Only safe methods (GET, HEAD, OPTIONS) are retried by default. POST and PUT must be explicitly opted in with `.allow_post_retry()` or `.allow_put_retry()`. Streaming request bodies are not retried unless a replay factory is provided.

## Cookie Jar

Requires the `cookies` feature.

```rust
use eggfetch_core::{Client, cookie::CookieJar};

let jar = CookieJar::new();

let client = Client::builder()
    .cookie_jar(jar.clone())
    .build();

// Make requests -- cookies are automatically managed
let resp = client.get("https://example.com/login")?.send().await?;

// Inspect the jar
let cookies = client.cookies();
for cookie in cookies.iter() {
    println!("{}={}", cookie.name(), cookie.value());
}
```

## Multipart Uploads

Requires the `multipart` feature.

```rust
use eggfetch_core::multipart::Multipart;
use bytes::Bytes;

let multipart = Multipart::new()
    .text("field", "value")?
    .bytes("file", "photo.jpg", "image/jpeg", Bytes::from(raw_image_data))?
    .into_body();

let resp = client.post("https://upload.example.com")?
    .body(multipart)
    .send().await?;
```

## HTTP Version Selection

```rust
use eggfetch_core::{Client, HttpVersionPolicy};

// HTTP/1.1 only
let client = Client::builder()
    .http_version_policy(HttpVersionPolicy::Http1Only)
    .build();

// HTTP/2 only (requires http2 feature)
let client = Client::builder()
    .http_version_policy(HttpVersionPolicy::Http2Only)
    .build();

// Auto-negotiate (default)
let client = Client::builder()
    .http_version_policy(HttpVersionPolicy::Auto { allow_http3: false })
    .build();
```

## Connection Pool Metrics

```rust
let client = Client::new();

// After making some requests...
let metrics = client.pool_metrics();
// PoolMetrics exposes idle/active connection counts for inspection.
```

## Full Example

```rust
use eggfetch_core::{Client, AuthScheme, Timeout};
use std::time::Duration;

#[tokio::main]
async fn main() -> eggfetch_core::Result<()> {
    let client = Client::builder()
        .user_agent("my-app/1.0")
        .timeout(Timeout::from_secs(30))
        .auth(AuthScheme::bearer("my-api-token")?)
        .follow_redirects(true)
        .max_redirects(5)
        .build();

    let mut resp = client
        .get("https://api.example.com/users")?
        .query("page", "1")
        .header("Accept", "application/json")
        .send()
        .await?;

    if resp.is_success() {
        let body = resp.text().await?;
        println!("Response: {body}");
    } else {
        eprintln!("Error: {} {}", resp.status(), resp.version());
    }

    Ok(())
}
```
