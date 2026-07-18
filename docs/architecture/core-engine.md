# Core Engine Deep Dive

The core engine (`eggfetch-core`) is the sole owner of all HTTP behavior. This document covers the client, request/response types, pipeline lifecycle, and error taxonomy.

See also: [overview.md](overview.md) for the high-level map.

## Module Map

| Module | Public? | Purpose |
|--------|---------|---------|
| `client` | Yes | `Client`, `ClientBuilder` — entry point |
| `request` | Yes | `Request`, `RequestBuilder` — fluent request construction |
| `response` | Yes | `Response`, `HistoryEntry` — response + redirect history |
| `body` | Yes | `RequestBody`, `ResponseBody`, `BoxBytesStream` |
| `headers` | Yes | `Headers` — case-insensitive header map wrapper |
| `error` | Yes | `Error` enum (30+ variants), `Result<T>` alias |
| `pipeline` | Crate-internal | Full request lifecycle orchestration |
| `transport` | Crate-internal | Direct, proxy, HTTP/3 transport dispatch |
| `stream` | Crate-internal | Per-chunk read/write timeout wrappers |

## Client

`Client` owns the connection pool, TLS configuration, and default settings. Created via `Client::new()` (defaults) or `ClientBuilder` (configuration).

```rust
let client = ClientBuilder::new()
    .timeout(Timeout::from_secs(30))
    .default_header("user-agent", "my-app")
    .build()?;
```

Builder-configurable options: headers, timeout, pool, redirects, auth, cookies, proxy, TLS, retry, decompression, HTTP version policy, max body size, max decompression ratio.

`Client` is `Clone` (cheap — internals are `Arc`-wrapped).

## RequestBuilder

Fluent builder for constructing requests. Supports method, URL, headers, query params, body, auth, proxy override, retry override, and timeout override.

```rust
let response = client
    .get("https://api.example.com/users")?
    .header("accept", "application/json")
    .query("page", "1")
    .send()
    .await?;
```

Body sources are mutually exclusive: `body()`, `bytes()`, `stream()`, `json()`, `form()`, `multipart()`.

### Proxy Override

`RequestBuilder::proxy()` accepts `ProxyOverride`:
- `Inherit` — use client-level proxy (default)
- `Direct` — bypass proxy for this request
- `Override(Proxy)` — use a different proxy for this request

## Response

`Response` wraps status, version, headers, URL, body, and redirect history.

Key methods:
- `status()` → `StatusCode`
- `version()` → `http::Version`
- `headers()` → `&Headers`
- `url()` → `&Url`
- `bytes()` → buffered body as `Bytes`
- `text()` → buffered body as `String`
- `bytes_stream()` → streaming `BoxBytesStream`
- `text_lines()` → line-by-line text iterator
- `history()` → `&[HistoryEntry]` (redirect chain)

### HistoryEntry

Metadata-only redirect record: status code, URL, headers (redacted for cross-origin). Does not carry body data.

## Pipeline Lifecycle

The `pipeline` module orchestrates the full request lifecycle. Entry point: `send_with_retry()`.

```
send_with_retry()           ← retry loop
  send_with_redirects()     ← redirect loop
    send_single_request()   ← one HTTP round-trip
```

### send_single_request steps:

1. Header merge (client defaults + request overrides)
2. Cookie selection from jar
3. Auth resolution (`resolve_request_auth`)
4. Timeout merging (per-field)
5. Pool acquisition (with pool timeout)
6. Write timeout wrapping (for stream bodies)
7. Content-Length application
8. HTTP/2 forbidden header stripping (`h2_headers`)
9. Transport dispatch (direct / proxy / HTTP3)
10. Decompression wrapping
11. Read timeout + pool lease attachment

## Error Taxonomy

`Error` is a single `thiserror`-derived enum with 30+ variants. Each variant has a `kind()` method returning a static string for programmatic matching.

| Category | Variants |
|----------|----------|
| **Input validation** | `InvalidUrl`, `InvalidMethod`, `InvalidHeaderName`, `InvalidHeaderValue`, `RequestBuild` |
| **Connection** | `Connect`, `Tls`, `Protocol`, `Hyper`, `HyperClient`, `Io` |
| **Timeout** | `Timeout { phase, elapsed }` — phase is `Pool`, `Connect`, `ProxyConnect`, `ProxyTls`, `Write`, `Read`, or `Total` |
| **Pool** | `Pool` |
| **Redirect** | `InvalidRedirectLocation`, `TooManyRedirects`, `BodyNotReplayableForRedirect` |
| **Auth** | `InvalidAuthHeader`, `ConflictingAuth` |
| **Body** | `Body`, `Decompression`, `UnsupportedContentEncoding`, `DecodedBodyTooLarge`, `DecompressionRatioExceeded` |
| **Proxy** | `InvalidProxyUrl`, `ProxyConnect`, `ProxyAuthRequired`, `ProxyConnectRejected`, `MalformedProxyResponse` |
| **TLS** | `TlsConfig`, `CaBundle`, `ClientCert`, `PrivateKey`, `CertificateVerification`, `HostnameVerification` |
| **Retry** | `BodyNotReplayableForRetry`, `RetryBudgetExhausted`, `RetryNotConfigured` |
| **HTTP/2** | `Http2GoAway`, `Http2StreamReset`, `Http2FlowControl`, `Http2Protocol` |
| **HTTP/3** | `H3Connect`, `H3ConnectionClosed`, `H3Stream`, `H3Protocol` |

`Error` is `Clone` — hyper/IO errors are wrapped in `Arc` to enable cloning.

## Security Invariants

- `unsafe_code = "forbid"` in this crate.
- All `Debug`/`Display` implementations redact secrets via `redact` module.
- CR/LF injection prevention in headers and auth values.
- URL credentials (`user:pass@host`) are rejected.
