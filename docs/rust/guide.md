# eggfetch-core Rust API Guide

eggfetch-core is the async HTTP client engine that powers the eggfetch ecosystem. All networking, connection pooling, redirect following, authentication, and retry logic lives here. The Python bindings and CLI are thin adapters around this crate.

## Adding as a Dependency

Add `eggfetch-core` to your `Cargo.toml`:

```toml
[dependencies]
eggfetch-core = { version = "0.1" } # H1 + Rustls + native roots/WebPKI fallback
```

Enable optional features as needed:

| Feature | Description |
|---|---|
| `http1` | HTTP/1.1 support (default) |
| `http2` | HTTP/2 support via ALPN |
| `http3` | HTTP/3 over QUIC (experimental) |
| `tls-rustls` | TLS via rustls (default) |
| `tls-native-roots` | Prefer the operating system trust store; implies `tls-rustls` (default) |
| `cookies` | Cookie jar support |
| `proxy` | HTTP/HTTPS proxy support |
| `multipart` | Multipart form-data encoding |
| `json` | Native JSON request/response helpers via optional Serde dependencies |
| `compression-gzip` | Gzip decompression |
| `compression-brotli` | Brotli decompression |
| `compression-zstd` | Zstandard decompression |
| `compression-deflate` | Deflate decompression |

For a typical H1/H2 client with cookies, proxying, multipart, all response
compression codecs, and native JSON helpers:

```toml
eggfetch-core = { version = "0.1", features = ["http1", "http2", "tls-rustls", "tls-native-roots", "cookies", "proxy", "multipart", "compression-gzip", "compression-brotli", "compression-zstd", "compression-deflate", "json"] }
# Add this when deriving request/response types for the `json` helpers.
serde = { version = "1", features = ["derive"] }
```

For a minimal embedded HTTPS client (deterministic WebPKI roots):

```toml
eggfetch-core = { version = "0.1", default-features = false, features = ["http1", "tls-rustls"] }
```

`http1` alone is cleartext-only. The opt-in `json` feature adds native
`RequestBuilder::json()` and `Response::json()` helpers; it does not change
the default dependency graph. Measured downstream size/dependency evidence lives in
`docs/architecture/embedded-footprint.md` — the current record is not a
footprint win, so do not describe migration as slimming.

The profile recipes and their excluded capabilities are listed in
[`docs/architecture/feature-flags.md`](../architecture/feature-flags.md).

## Caller-owned transport and embedded guardrails

Native consumers that already own routing can supply a raw Tokio stream while
keeping HTTP and destination TLS in eggfetch:

```rust
use eggfetch_core::{Client, DialFuture, DialTarget, Dialer, DialError, DialStream};

struct MyDialer;

impl Dialer for MyDialer {
    fn dial(&self, target: DialTarget) -> DialFuture<'_> {
        Box::pin(async move {
            let stream = tokio::net::TcpStream::connect((target.host(), target.port()))
                .await
                .map_err(|error| DialError::with_source(
                    eggfetch_core::DialErrorKind::Connection,
                    "caller route failed",
                    error,
                ))?;
            Ok(Box::new(stream) as DialStream)
        })
    }
}

let client = Client::builder()
    .dialer(MyDialer)
    .retry_canceled_requests(false)
    .physical_connection_policy(eggfetch_core::PhysicalConnectionPolicy {
        max_live: Some(8),
        admission_timeout: Some(std::time::Duration::from_secs(2)),
    })
    .transport_io_timeout(eggfetch_core::TransportIoTimeout {
        read: Some(std::time::Duration::from_secs(30)),
        write: Some(std::time::Duration::from_secs(30)),
    })
    .build();
```

The dialer receives only logical host/port data. Eggfetch owns `Host`, HTTPS
SNI/certificate verification, HTTP framing, pooling, redirects, retries, and
response streaming. Custom dialing is incompatible with built-in proxy, UDS,
resolved-target, local socket, socket-option, and HTTP/3 routes; conflicts
fail before network I/O. A request `target` override changes only the wire
path/query while the logical authority remains in place for dialing and TLS.
`retry_canceled_requests` controls only Hyper's
implicit stale-idle-connection retry and is independent of `RetryPolicy`.
It defaults to `true` for compatibility. Set it to `false` when one logical
call must expose a stale pooled-connection failure instead of allowing Hyper
to retry it internally; an eligible eggfetch `RetryPolicy` can still perform a
later logical attempt. The control applies to Hyper HTTP/1/2 routes only;
HTTP/3 uses its separate QUIC lifecycle and retry rules.
`PhysicalConnectionPolicy` caps live Hyper connections, including idle pooled
connections, while `TransportIoTimeout` guards inactivity after establishment.
They are native Hyper-route controls: the policy covers standard, direct,
resolved-target, SNI, custom-dialer, UDS, SOCKS, HTTP forward-proxy, and
compatible HTTPS CONNECT clients, while the experimental HTTP/3/QUIC path
keeps its existing controls. `max_live` must be greater than zero when set, and
`admission_timeout` is meaningful only with a live cap. Use
`client.transport_metrics().snapshot()` for admission/live/high-water and
I/O-timeout counters. Admission waits are distinct from `Timeout.pool`, and
transport I/O timeouts are distinct from request-body/response-body timeouts.

### Integration boundary

For an embedding application, keep routing and route/isolation policy outside
eggfetch, construct a client for the policy domain as needed, and optionally
provide a `Dialer` for the raw stream. Eggfetch then owns destination
HTTP/TLS, pooling, request timeouts, response streaming, and the ordinary
`RetryPolicy`; the native canceled-request setting, physical connection cap,
and transport-I/O inactivity policy remain separately configurable.

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

### Structured native request failures

Use the opt-in detailed entry point when native Rust code needs to distinguish
timeouts from evidence-backed connection failures without matching display
strings:

```rust
use eggfetch_core::{Client, NetworkFailureKind};

let failure = Client::new()
    .get("https://service.example")?
    .send_detailed()
    .await
    .expect_err("example endpoint should be unavailable");

match failure.network_failure_kind() {
    Some(NetworkFailureKind::Dns) => eprintln!("DNS resolution failed"),
    Some(NetworkFailureKind::ConnectionRefused) => eprintln!("connection refused"),
    Some(NetworkFailureKind::Connect) => eprintln!("connection establishment failed"),
    None if failure.is_timeout() => eprintln!("timeout: {:?}", failure.timeout_phase()),
    None => eprintln!("request failed: {}", failure.error()),
}
```

`RequestFailure::into_error()` recovers the unchanged public `Error`. Standard
HTTP/HTTPS resolver failures can report `Dns`; network subtypes remain
evidence-backed and route-dependent, so an absent subtype means the current
transport boundary could not prove DNS or refusal. The public `Error` and
`Error::kind()` are unchanged, including the standard route's `hyper_client`
category.

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

### Native HTTP body interoperability

When an application already owns an `http_body::Body`, the native transport
surface preserves its DATA and trailer frames:

```rust
use bytes::Bytes;
use eggfetch_core::{Client, NativeRequestOptions};
use http_body::Frame;
use http_body_util::StreamBody;

let body = StreamBody::new(futures_util::stream::iter([
    Ok::<_, std::convert::Infallible>(Frame::data(Bytes::from_static(b"payload"))),
]));
let request = http::Request::post("https://api.example.com/upload")
    .body(body)?;
let response = Client::new()
    .execute_http_body(request, NativeRequestOptions::default())
    .await?;
```

Consume `response.into_body()` by polling `http_body::Body::poll_frame` (or
an appropriate `http-body-util` adapter) to observe DATA and trailer frames.
This one-shot API does not apply redirects, logical retries, cookies, auth,
decompression, or decoded-body limits. Read timeouts start on the first body
poll and reset after each frame; delaying consumption after response headers
does not consume that budget. It is intended for native gateways, service
meshes, middleware, and custom-network clients; the existing `RequestBuilder`
API remains the application-oriented choice.

### Native Tower service interoperability

For callers using Tower-native middleware, `Client::native_service()` is a
small wrapper around the same `execute_http_body()` path:

```rust
use bytes::Bytes;
use eggfetch_core::Client;
use http_body_util::Empty;
use tower_service::Service;

let mut service = Client::new().native_service();
let request = http::Request::get("https://api.example.com/")
    .body(Empty::<Bytes>::new())?;
let response = service.call(request).await?;
```

`NativeHttpService` is cloneable and accepts the same body bounds as
`execute_http_body()`. Use `NativeHttpService::new(client)` or
`.with_options(NativeRequestOptions)` when service-instance defaults are
clearer than direct calls. Its `poll_ready()` always returns ready: Tower
readiness means the adapter can accept a request, while origin-aware logical
pool admission, physical connection admission, and transport backpressure
occur inside the future returned by `call()`. A Tower `LoadShed` layer placed
directly around this service therefore does not observe eggfetch pool
saturation; callers may add their own global policy layers.

The service preserves native DATA/trailer and lifecycle behavior and does not
add redirects, logical retries, cookies, authentication, decompression, or
decoded-body limits. Middleware-local request extensions can be used before
the adapter, but arbitrary extensions are not guaranteed to reach Hyper. The
full `tower` framework and Tonic/gRPC remain optional caller/qualification
dependencies; neither is an eggfetch-core feature.

### TLS provider and additional roots

Rustls provider choice is local to `TlsConfig`:

```rust
use std::sync::Arc;
let tls = eggfetch_core::TlsConfig::builder()
    .crypto_provider(Arc::new(rustls::crypto::ring::default_provider()))
    .additional_ca_certificate_pem(include_bytes!("private-ca.pem"))?
    .build();
let client = Client::builder().tls_config(tls).build();
```

`ca_certificate_*` methods retain replacement semantics. The
`additional_ca_certificate_*` methods augment the selected native/WebPKI or
custom base. `add_ca_certificate_path` is intentionally unchanged. The
provider is not installed globally and is used for both verification and

To use another Rustls provider, add that provider as a direct dependency in
the application and pass its `Arc<CryptoProvider>` in the same way. For
example, an AWS-LC application selects the `aws-lc-rs` Rustls feature and
uses `rustls::crypto::aws_lc_rs::default_provider()`. This does not make
eggfetch FIPS-validated or post-quantum capable; those properties depend on
the exact provider build and configuration. The isolated
[`native-http-body-tls` qualification fixture](../../qualification/native-http-body-tls/)
demonstrates the external-provider, mTLS, custom-dialer, and private-root
path.

With the `json` feature, request and response values can use Serde directly:

```rust
#[derive(serde::Serialize)]
struct CreateUser<'a> { name: &'a str }

#[derive(serde::Deserialize)]
struct User { id: u64 }

let response = client.post("https://api.example.com/users")?
    .json(&CreateUser { name: "Ada" })?
    .send().await?;
let user: User = response.json().await?;
```

Native `Response::json()` parsing is explicit: it does not require or validate
`Content-Type`, consumes the response body once, and uses the normal decoded
body/decompression limits. `RequestBuilder::json()` replaces an earlier body;
a later body setter replaces its bytes but leaves an already-set content type.
Per-request decoded-body limit overrides take precedence over client settings
and persist across retries and redirects.

For native direct routing, `resolved_addresses()` pins the physical TCP
destinations while the request URL still controls Host, HTTPS certificate
identity, and SNI:

```text
logical URL: https://service.example/data
Host/SNI:    service.example
physical:    203.0.113.10:443 (caller-supplied)
```

```rust
use std::net::SocketAddr;

let address: SocketAddr = "203.0.113.10:443".parse()?;
let response = client.get("https://service.example")?
    .resolved_addresses([address])
    .send().await?;
```

The supplied addresses are used exactly and DNS is never attempted, including
on retries. Each address must use the URL's effective HTTP/HTTPS port. A
same-origin redirect retains the destination snapshot; a cross-origin
redirect, configured proxy, Unix-domain-socket route, or HTTP/3 route fails
closed before network I/O. Static requests use a bounded direct route cache
keyed by origin plus the ordered snapshot plus SNI, so ordinary pooled
connections cannot bypass the pin and identical routes reuse Hyper keep-alive
(H1) / multiplexed (H2) connections. This is distinct from local
source-address binding and from an SNI override, and is a routing primitive
rather than an SSRF policy.

### Proxied physical route pinning

Proxy peer and proxied target addresses are separate native controls:

```rust
let proxy = Proxy::all("https://proxy.example:8443")?
    .resolved_addresses(["198.51.100.20:8443".parse()?])?;
let client = Client::builder().proxy(proxy).build();

let response = client
    .get("https://service.example/data")?
    .proxy_target_addresses(["203.0.113.10:443".parse()?])
    .send().await?;
```

`Proxy::resolved_addresses()` pins only the TCP peer of the logical proxy;
the proxy URL still controls matching, credentials, and HTTPS-proxy TLS
identity. `RequestBuilder::proxy_target_addresses()` pins only the ultimate
destination communicated through the proxy. HTTPS CONNECT and local-DNS
`socks5://` use the supplied target without origin DNS and preserve the
logical origin URL, `Host`, SNI, and certificate name. `socks5h://` target
pinning and plaintext HTTP forward-proxy target pinning fail closed before
proxy I/O. Empty sets and port mismatches are rejected. The caller must
validate these physical addresses; eggfetch does not add an authorization or
SSRF policy engine.

Both snapshots are immutable across retries and same-origin redirects, never
fall back to DNS, and cannot be reused across cross-origin redirects. Forward
and compatible CONNECT proxy clients use bounded Hyper pools; their route keys
and reusable connectors contain only connection-affecting policy. A request's
logical `Timeout.total` is excluded from cache identity and connector state,
while the outer request dispatch enforces that request's shrinking total
deadline, including when stale pooled connections require reconnection. The SOCKS client cache keys
include proxy-peer and target snapshots so incompatible physical routes do
not share a connection. HTTPS CONNECT candidates advance only on typed 502/504
proxy rejection. Local-SOCKS5 candidates advance only on replies 0x03 (network
unreachable), 0x04 (host unreachable), or 0x05 (connection refused); proxy-wide
authentication, policy, protocol, and malformed-response failures stop.
Applications that want an external proxy chain or policy engine should use the
native `Dialer` seam instead of adding that policy to eggfetch.

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

This logical retry policy is separate from Hyper's lower-level canceled-request
retry. `retry_canceled_requests(false)` disables only that lower-level retry;
it does not change method eligibility, backoff, `Retry-After`, deadlines, or
the requirement that a retry body be replayable.

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

// HTTP/2 only (requires http2 feature). Enforced end-to-end:
// ALPN advertises only `h2`, the hyper-util legacy client is built with
// `http2_only(true)`, and direct/UDS connectors signal ALPN h2 via
// `Connected::negotiated_h2`. Over cleartext TCP this sends the H2
// client preface directly (h2c prior knowledge).
let client = Client::builder()
    .http_version_policy(HttpVersionPolicy::Http2Only)
    .build();

// Auto-negotiate (default)
let client = Client::builder()
    .http_version_policy(HttpVersionPolicy::Auto { allow_http3: false })
    .build();
```

H2-only requests that reach an H1-only server fail with a
`RequestError` / `ConnectError` rather than silently downgrading.
The `stream_id` metadata field exposed by HTTPX is intentionally
absent; see `docs/residual-differences.md`.

### HTTP/3 (experimental, `http3` feature)

HTTP/3 over QUIC remains experimental; the graduation gate, pinned
versions, and named blockers live in
`docs/architecture/core-tls-proxy-protocols.md`
(§ "Production Graduation Decision"). The rules below summarize that
document; it is authoritative on conflicts.

```rust
use eggfetch_core::{Client, HttpVersionPolicy};

// Strict H3-only: direct QUIC, no discovery, no fallback, no suppression.
let client = Client::builder()
    .http_version_policy(HttpVersionPolicy::Http3Only)
    .build();

// Discovery mode: H1/H2 unless a fresh authenticated `h3` Alt-Svc entry
// is cached for the origin and not suppressed.
let client = Client::builder()
    .http_version_policy(HttpVersionPolicy::Auto { allow_http3: true })
    .build();
```

- `Http3Only` is strict direct: any H3 failure is returned, never
  retried at the transport layer and never fallen back to H1/H2.
- `Auto { allow_http3: true }` discovers via the bounded authenticated
  Alt-Svc cache (`h3`-only, `ma`/`clear`, 64 entries); no fresh entry
  means H1/H2. Safe fallback to H1/H2 happens only pre-commit for
  replayable bodies (`Empty`/`Bytes`; one-shot streams never
  duplicate) via an explicit dispatch error; `total`/`connect`/
  `write`/`read` deadlines are reused, never restarted.
- Broken routes are suppressed per origin with exponential backoff;
  only route failures suppress, never timeouts or graceful drains.
- H3 never bypasses proxy rules: UDS, custom-dialer, specialized-direct,
  proxy/SOCKS, and SNI routes are selected first.
- GOAWAY draining evicts only the drained generation for the *next*
  request; in-flight streams complete. Only `H3Connect` is retryable.
- QUIC idle derives from `PoolConfig::idle_timeout` (default 30 s),
  never `Timeout.pool`; bidi stream caps derive from the effective
  per-origin in-flight limit (default 100). Creations, evictions,
  Alt-Svc, attempted/suppressed/fallback/drain/close/reconnect events
  are counted in `TransportMetrics` (`Client::transport_metrics()`).
- Advanced QUIC features (0-RTT, WebTransport, datagrams, MASQUE,
  connection migration) are not implemented in this milestone.

## Connection Pool Metrics

Concurrency limits bound logical in-flight requests (one permit per
request), not physical TCP connections: under H1 one request typically
owns its connection slot, while under H2/H3 many permits multiplex over
one TCP/QUIC connection. Prefer `max_in_flight_requests` /
`max_in_flight_requests_per_origin` in new code; `max_connections` /
`max_connections_per_host` are pre-1.0 aliases (the new name wins when
both are set). `max_idle_connections*` size the physical idle pool and
are separate from in-flight concurrency.

```rust
let client = Client::new();

// After making some requests...
let metrics = client.pool_metrics();
// PoolMetrics exposes logical waits/cancellations only
// (acquisition_waits, acquisition_cancellations); transport
// events live in Client::transport_metrics(). Hyper socket-reuse
// counts are intentionally absent.
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
