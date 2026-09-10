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
| `network_stream` | Yes | `NetworkStream`, `UpgradedStream`, `ConnectionMetadata` — upgrade IO + connection metadata |
| `trace` | Yes | `TraceObserver`, `TraceEvent` — synchronous lifecycle event callbacks |
| `error` | Yes | `Error` enum (47 variants), `Result<T>` alias |
| `pipeline` | Crate-internal | Full request lifecycle orchestration |
| `transport` | Crate-internal | Direct, direct-with-socket-options, UDS, proxy, HTTP/3 transport dispatch |
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

Fluent builder for constructing requests. Supports method, URL, headers, query params, body, auth, proxy override, retry override, timeout override, and transport hints.

```rust
let response = client
    .get("https://api.example.com/users")?
    .header("accept", "application/json")
    .query("page", "1")
    .send()
    .await?;
```

Body sources are mutually exclusive: `body()`, `bytes()`, `stream()`, `json()`, `form()`, `multipart()`.

### Transport Hints

`RequestBuilder::transport_hints()` sets typed wire-level overrides via `TransportHints`:

- `target: Option<Bytes>` — overrides the wire request target (e.g. `OPTIONS *`, absolute-form) without changing the logical URL used for routing, cookies, auth, and proxy selection.
- `sni_hostname: Option<String>` — overrides TLS SNI while preserving TCP destination.
- `trace: Option<Arc<dyn TraceObserver>>` — installs a callback observer for [`TraceEvent`](../../crates/eggfetch-core/src/trace.rs) emissions during dispatch.

Transport hints survive retry reconstruction via the typed
`RequestParts::retry_request()` transformation but are cleared on redirect
hops (destination changed). The redirects-disabled fast path and the
redirect-enabled first hop share one `HopBuildParams` builder, so `target`,
`sni_hostname`, and `trace` cannot diverge between the two entry paths.

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
- `headers()` → `&HeaderMap`
- `url()` → `&Url`
- `wire_content_encoding()` → `Option<&str>` — original wire Content-Encoding (before decompression)
- `wire_content_length()` → `Option<&str>` — original wire Content-Length
- `wire_reason_phrase()` → `Option<&str>` — original wire HTTP/1.x reason phrase
- `bytes()` → buffered body as `Bytes`
- `text()` → buffered body as `String`
- `bytes_stream()` → streaming `BoxBytesStream`
- `text_lines()` → line-by-line text iterator
- `trailers()` → `Option<HeaderMap>` after body EOF (H1 chunked, H2 trailing HEADERS, H3 trailing headers; `None` until arrival, on no-trailers, or on pre-trailer errors; H1 duplicates collapse upstream)
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

### send_single_request phases:

Preparation (`prepare_single_request()` → `PreparedRequest`) centralizes
request policy so transports own only connection/protocol work:

1. Header merge (client defaults + request overrides) and timeout merging
   (per-field) happen in `send_with_redirects()`; the hop itself is built
   by the shared `HopBuildParams` builder (cookies, auth, hints).
2. Preparation normalizes accept-encoding, Content-Length, user-agent, and
   H2-forbidden headers, validates request size, resolves the wire URI
   (`target` override), wraps stream bodies with the write timeout,
   resolves the effective proxy/origin, acquires the pool guard, and
   computes the remaining-total/deadline. One-shot bodies are moved, never
   cloned.
3. Dispatch selects one `TransportRoute` via `select_route()` (UDS →
   specialized-direct → proxy/SOCKS → SNI-direct → H3 → standard Hyper).
   H3 is `Http3Only` direct or `Auto`-discovered (fresh Alt-Svc + not
   suppressed); safe `Auto` fallback to standard is pre-commit replayable
   only. Hyper request scaffolding is built once via `build_hyper_request()`.
4. One common post-transport policy applies to every route: Alt-Svc learning
   (learnable routes only), decompression wrapping, decoded-size limiting,
   then read-timeout + pool-lease attachment.

The standard and specialized-direct Hyper paths share a single
response-lifecycle implementation (`finish_hyper_response()` plus shared
trace helpers, `wrap_incoming` with `SharedTrailers`, and
`await_upgrade` with connector metadata); UDS reuses the same helpers
including 101 upgrade handling (UDS kind without IPs).

## Network Stream and Upgrade Support

`Response` carries an optional `network_stream` field of type `Option<NetworkStream>`:

- For **101 Switching Protocols** responses, Hyper's `OnUpgrade` is captured before consuming the body. The upgrade future is awaited and converted via connector-aware downcasting: direct-connector upgrades recover real local/remote addrs plus TLS version/cipher/ALPN, UDS upgrades report `Unix`/`TlsUnix` without IPs, and standard opaque upgrades remain explicitly unavailable (`None` addrs, default kind). The response body is set to an empty buffered body.
- For **ordinary** responses, `network_stream` is `None` — the connection is managed by the pool and raw IO access would corrupt pool state.
- For **internal HTTPS CONNECT tunnels**, `network_stream` is `None` — the tunnel is owned by the proxy implementation; the canonical access path is the body iterator.
- For **H3**, per-response metadata stays `None`; `TransportKind::Quic` is reserved for future connector-derived endpoint info.
- `UpgradedStream` carries an `UpgradedStreamVariant` (`Tcp`/`Tls`/`Adapter`) classification so callers can detect whether `start_tls` is safe to invoke. Only inner `Tcp` variants support `start_tls`; `Adapter` (Hyper-opaque wrapping of 101 upgrades, including UDS adapter wrapping) and `Tls` (already-encrypted) are rejected before any IO is consumed.
- The `UpgradedStream` provides async `read()`, `write_all()`, `close()`, `flush()`, and `start_tls()` operations, plus `metadata()` for connection info.
- Leading data: downcast branches carry Hyper's `read_buf` explicitly as `leading_data`; the opaque branch preserves Hyper's internal rewind buffer and yields it on the first reads.

**Ownership rules**: once an upgrade handoff succeeds, the connection is removed from the HTTP pool and must never be reused. Closing the response and closing the upgraded stream are independent operations.

## Trace Observer

The `trace` module defines a typed event vocabulary (derived from httpcore 1.0.9's `Trace` context manager) and the callback trait transports use to emit lifecycle events. Observers are installed per request via `TransportHints::trace` — they survive retry reconstruction and are cleared on redirect hops like all transport hints.

### Events

`TraceEvent` has ten variants, each carrying a `TracePhase` (`Started`/`Complete`/`Failed`) plus structured metadata. Request/response header events are emitted consistently; DNS/connect/TLS phases are observed via `TransportMetrics` to avoid duplicate taxonomy (see `trace.rs` reconciliation docs):

| Event | Extra fields | httpcore dotted name |
|-------|--------------|----------------------|
| `ConnectTcp` | `host`, `port` | `connect_tcp.<phase>` |
| `ConnectUnixSocket` | `path` | `connect_unix_socket.<phase>` |
| `StartTls` | `server_hostname` | `start_tls.<phase>` |
| `Retry` | `delay_ms` | `retry.<phase>` |
| `Close` | — | `close.<phase>` |
| `SendRequestHeaders` | `method`, `target` | `send_request_headers.<phase>` |
| `SendRequestBody` | — | `send_request_body.<phase>` |
| `ReceiveResponseHeaders` | `status` | `receive_response_headers.<phase>` |
| `ReceiveResponseBody` | — | `receive_response_body.<phase>` |
| `ResponseClosed` | — | `response_closed.<phase>` |

Connection-level events map to the `"connection"` logger prefix; HTTP message events map to `"http11"`. `event_to_httpcore_name()` produces these names and `event_to_info()` produces the flat info dictionary (values are opaque `EventValue::{String, U16, U64}`) that the Python compatibility layer forwards to user callbacks.

### Observer Contract

`TraceObserver` is a synchronous, `Send + Sync` callback trait: `on_event(&self, &TraceEvent) -> OnEventAction`. It is invoked inside the transport's async context and **must not block or perform I/O**; for the Python binding, the GIL is acquired only at delivery points, never across network waits.

- `OnEventAction::Continue` lets the transport proceed.
- `OnEventAction::Abort` makes the transport short-circuit and return an error. The observer has already recorded the failure in its binding-owned error slot; callers surface that error after unwinding. This is how a raising Python callback stops dispatch without producing discarded work.

Built-in implementations: `NoopTraceObserver` (discard everything) and `CollectingTraceObserver` (thread-safe event vector with `drain()`/`len()`, used by tests). A callback failure surfaces as `Error::TraceCallbackAborted`.

### Python Bridge

Python callables never enter `eggfetch-core`: `crates/eggfetch-python/src/trace_bridge.rs` wraps them in `PyTraceObserver`. Only sync callables are accepted — coroutine functions are detected via `inspect.iscoroutinefunction()` and rejected eagerly with `TypeError` before dispatch (the core trait is synchronous; an un-awaited coroutine would be silently dropped). Details in [python-bindings.md](python-bindings.md).

## Error Taxonomy

`Error` is a single `thiserror`-derived enum with 47 variants. Each variant has a `kind()` method returning a static string for programmatic matching.

| Category | Variants |
|----------|----------|
| **Input validation** | `InvalidUrl`, `InvalidMethod`, `InvalidHeaderName`, `InvalidHeaderValue`, `RequestBuild` |
| **Connection** | `Connect`, `Tls`, `Protocol`, `Hyper`, `HyperClient`, `Io` |
| **Timeout** | `Timeout { phase, elapsed }` — phase is `Pool`, `Connect`, `ProxyConnect`, `ProxyTls`, `Write`, `Read`, or `Total` |
| **Pool** | `Pool` |
| **Redirect** | `InvalidRedirectLocation`, `TooManyRedirects { followed, max }`, `BodyNotReplayableForRedirect` |
| **Auth** | `InvalidAuthHeader`, `ConflictingAuth` |
| **Body** | `Body`, `Decompression`, `UnsupportedContentEncoding`, `DecodedBodyTooLarge`, `DecompressionRatioExceeded`, `Unsupported` |
| **Proxy** | `InvalidProxyUrl`, `ProxyConnect`, `ProxyAuthRequired`, `ProxyConnectRejected`, `MalformedProxyResponse` |
| **TLS** | `TlsConfig`, `CaBundle`, `ClientCert`, `PrivateKey`, `CertificateVerification`, `HostnameVerification` |
| **Retry** | `BodyNotReplayableForRetry`, `RetryBudgetExhausted { attempts }`, `RetryNotConfigured` |
| **HTTP/2** | `Http2GoAway`, `Http2StreamReset`, `Http2FlowControl`, `Http2Protocol` |
| **HTTP/3** | `H3Connect`, `H3ConnectionClosed`, `H3Stream`, `H3Protocol` |
| **Trace** | `TraceCallbackAborted` |

`Error` is `Clone` — hyper/IO errors are wrapped in `Arc` to enable cloning.

## Security Invariants

- `unsafe_code = "forbid"` in this crate.
- All `Debug`/`Display` implementations redact secrets via `redact` module.
- CR/LF injection prevention in headers and auth values.
- URL credentials (`user:pass@host`) are rejected.
