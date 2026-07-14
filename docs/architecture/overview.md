# Architecture Overview

This document describes the architecture of eggfetch. The post-Milestone-W state is the latest completed work. The core crate provides protocol-neutral streaming for request and response bodies, redirect following with configurable policy, total timeout across redirect chains, metadata-only redirect history, replay-safe request bodies, an RFC 6265 cookie subsystem, an authentication subsystem with Basic and Bearer token support, precedence resolution, request-level auth disable, cross-origin credential stripping, streaming multipart/form-data request bodies, response decompression (gzip, deflate, brotli, zstd) with Accept-Encoding negotiation, decoded-body resource limits (max size, decompression ratio), a proxy subsystem (HTTP proxying, HTTPS CONNECT tunneling, proxy authentication, per-request/client proxy configuration, NO_PROXY bypass), TLS configuration (custom CA bundles, client certificates, TLS version policy, verification toggle, SNI behavior), HTTP/2 support (ALPN negotiation, multiplexed connections, protocol version reporting, HTTP/2 error taxonomy, forbidden header stripping, retry classification for REFUSED_STREAM, trailers documentation, pool concurrency model documentation), and HTTP/3 support (QUIC transport via Quinn/h3, feature-gated, experimental, `Http3Only` policy, TLS 1.3 requirement, 0-RTT disabled). The Python crate exposes both sync and async APIs over the async Rust core via PyO3/maturin, with buffered and live streaming response surfaces, request-local and client-level cookies, redirect support, sync/async parity, `BasicAuth`/`BearerAuth` classes, `auth=` on request methods, `NOAUTH`, named streaming exceptions, `files=` kwarg for multipart uploads, `decompress=` kwarg for decompression control, `verify=`/`cert=` kwargs for TLS configuration, `http2=` kwarg for HTTP/2 negotiation, `http3=` kwarg for HTTP/3 (experimental), and HTTP/2-specific exception types (`Http2Error`, `Http2GoAway`, `Http2StreamReset`, `Http2FlowControlError`) and HTTP/3-specific exception types (`Http3Error`, `Http3ConnectionError`).

The post-E hardening pass landed true streaming request bodies, per-chunk read/write timeouts, pool permits tied to the full response body lifecycle, and origin-keyed pool limits.

## Three-Crate Workspace

eggfetch is a Cargo workspace with three crates:

- **eggfetch-core** is the async Rust HTTP engine. It owns all networking, connection management, TLS configuration, body handling, and error types. Every dependency that touches the network lives here.
- **eggfetch-cli** is a thin binary that wraps eggfetch-core. It handles argument parsing (eventually via clap), terminal output formatting, exit code mapping, and body/header display. It contains no independent HTTP behavior.
- **eggfetch-python** is the Python bindings adapter. It uses PyO3 and maturin to expose eggfetch-core to Python. It does not duplicate request execution logic.

The reason eggfetch-core owns all HTTP behavior is to maintain a single networking implementation. If HTTP logic were split across crates, behavioral consistency would be impossible to guarantee and the test surface would multiply.

## Async-First Invariant

The Rust engine is async-only. There is no synchronous Rust API. The `Client` type exposes methods that return futures. Callers drive execution with a tokio runtime (or another async executor that hyper supports).

This design keeps the core simple: one code path, one set of state machines, no conditional compilation for sync vs async. Synchronous behavior is an adapter concern, not a core concern.

## Core Types

The following types form the public API of eggfetch-core:

- **`Client`** -- async HTTP client. Created via `Client::new()` or `ClientBuilder`. Owns the connection pool and TLS configuration.
- **`ClientBuilder`** -- builder for configuring a `Client` before construction, including TLS settings.
- **`Request`** -- a fully-formed HTTP request (method, URI, headers, body).
- **`RequestBuilder`** -- accumulates method, URL, headers, query parameters, and body before producing a `Request`.
- **`Response`** -- an HTTP response with status, headers, streaming body, and redirect history.
- **`RequestBody`** -- request body type: `Empty`, `Bytes`, or `Stream` (a boxed `Stream<Item = Result<Bytes>>`).
- **`ResponseBody`** -- response body type: `Buffered` (collected bytes), `Streaming` (live chunk stream), or `Consumed` (already consumed).
- **`Headers`** -- typed header map backed by `http::HeaderMap`.
- **`Error`** -- structured error enum covering network, HTTP, timeout, and builder errors.
- **`Cookie`** -- an HTTP cookie with name, value, domain, path, secure, httpOnly, SameSite, and expiry attributes.
- **`CookieJar`** -- thread-safe cookie storage with RFC 6265 domain/path matching and automatic Set-Cookie ingestion.
- **`SameSite`** -- cookie SameSite attribute (Strict, Lax, None).
- **`AuthScheme`** -- authentication scheme enum: `Basic(BasicAuth)` or `Bearer(BearerAuth)`. Applies the `Authorization` header. Secrets are redacted in Debug/Display.
- **`BasicAuth`** -- HTTP Basic authentication credentials (username, password). Base64-encodes `username:password` for the `Authorization` header.
- **`BearerAuth`** -- HTTP Bearer token authentication. Sets `Authorization: Bearer <token>`.
- **`Multipart`** -- multipart/form-data request body with boundary and parts. Feature-gated behind `multipart`. Provides builder API for text fields, byte parts, and streaming parts.
- **`Part`** -- a single multipart part with name, filename, content type, headers, and body.
- **`PartBody`** -- multipart part body: `Bytes` or `Stream` with optional known length.
- **`Boundary`** -- validated multipart boundary string. Random generation or custom validated.
- **`ContentCoding`** -- content coding enum: `Gzip`, `Deflate`, `Brotli`, `Zstd`. Feature-gated behind compression features.
- **`TlsConfig`** -- TLS configuration type with builder pattern. Custom CA bundles, client certificates, TLS version policy, verification toggle, and SNI behavior.
- **`TlsConfigBuilder`** -- builder for constructing `TlsConfig` instances.
- **`TrustStore`** -- custom CA bundle support via PEM files or system roots.
- **`ClientIdentity`** -- client certificate and private key for mutual TLS authentication.
- **`TlsVersion`** -- TLS version policy (minimum/maximum supported versions).

### TLS trust stores

The Rustls connector first loads the operating system's native roots. If the
native root store is unavailable (for example, in a minimal container), it
uses the packaged Mozilla/WebPKI roots. Both paths retain certificate-chain
and hostname verification. A certificate or hostname verification failure is
not a reason to try the packaged roots, and private or enterprise CAs are not
automatically included in the fallback set.

## Timeout System

eggfetch implements phase-aware timeouts that map to specific segments of the request lifecycle:

- **Pool timeout**: time waiting for a connection slot from the concurrency pool.
- **Connect timeout**: time to establish TCP connection and TLS handshake (including DNS).
- **Write timeout**: time to send request headers and body (documented; hyper-util does not isolate this phase).
- **Read timeout**: time to wait for response headers or body chunks (documented; hyper-util does not isolate this phase).
- **Total timeout**: wall-clock cap across the entire request lifecycle.

### Configuration

Timeouts are configured at two levels:

- **Client-level**: `ClientBuilder::timeout(Timeout::from_secs(10))` sets defaults for all requests.
- **Request-level**: `RequestBuilder::timeout(Timeout::from_secs(2))` overrides client defaults per-field.

Request-level overrides are per-field: only fields present in the request-level `Timeout` replace the corresponding client-level fields.

### Error Model

Timeout errors carry phase identity:

```rust
Error::Timeout { phase: TimeoutPhase::Read, elapsed: Duration::from_secs(5) }
```

This enables Python bindings to map to specific exception classes (`ConnectTimeout`, `ReadTimeout`, etc.).

### Implementation Notes

- **Pool timeout**: enforced via `tokio::time::timeout` around pool acquisition.
- **Total timeout**: enforced via `tokio::time::timeout` around the full send.
- **Read timeout**: enforced by a per-chunk wrapper stream (`stream::ReadTimeoutStream`) that fires `Error::Timeout { phase: Read }` if no response body chunk arrives within the configured duration. The deadline resets on every chunk arrival.
- **Write timeout**: enforced by a per-chunk wrapper stream (`stream::WriteTimeoutStream`) that fires `Error::Timeout { phase: Write }` if the request body producer does not yield the next chunk within the configured duration. The deadline resets on every chunk delivery. Only applies to streamed request bodies; buffered bodies complete synchronously.
- **Connect timeout**: accepted and merged but not independently enforced. hyper-util's legacy client does not expose a connect-phase deadline. `total` should be used as a backstop.

### Cancellation Safety

Cancelled timeout-wrapped operations release pool permits cleanly. The pool uses `OwnedSemaphorePermit` with RAII drop semantics.

## Streaming

The body layer provides protocol-neutral streaming for both request and response bodies.

### Response Streaming

All responses from the client are wrapped as `ResponseBody::Streaming` by default. The `wrap_incoming()` adapter converts hyper's `Incoming` type into a `BoxBytesStream`. Callers choose their consumption model:

- **Buffered**: `response.bytes().await` or `response.text().await` collects the full body.
- **Streaming**: `response.bytes_stream()` returns a `BoxBytesStream` for chunk-by-chunk processing.
- **Line streaming**: `response.text_lines()` splits the byte stream into text lines.

### Request Streaming

Request bodies support three variants: `Empty`, `Bytes` (fixed buffer), and `Stream` (chunked upload). Stream request bodies are wrapped in a hyper `StreamBody` and piped through to the transport incrementally with no eager buffering. When `length` is `Some(n)`, the body is sent as a `Content-Length`-delimited body. When `length` is `None`, hyper's HTTP/1.1 machinery selects a safe transfer mode (e.g. chunked transfer encoding).

The producer stream is polled lazily: each chunk is sent as soon as it is produced, so a slow producer backpressures the transport.

### Single-Consumption Semantics

Body types are single-consume. Calling `bytes_stream()` on a streaming body replaces it with `Consumed`; a second call returns an error. Calling `bytes()` on a consumed body also returns an error. This prevents accidental double-reads and enforces ownership transfer.

### Pool Permit Lifecycle (Lease on Body)

Streaming response bodies carry an internal `Arc<PoolGuard>` that holds the pool permits acquired for the request. The permits are released when the response body is dropped or fully consumed. Buffered and already-consumed responses do not carry a lease.

This guarantees that per-origin concurrency limits remain meaningful while response bodies are in flight: a streaming response that is held but not consumed continues to occupy its pool slot. If a caller wants to free a slot before consuming the body, they must drop or fully buffer the response.

## Authentication (Milestone P)

eggfetch implements an authentication subsystem with the following capabilities:

- **Basic authentication** -- `BasicAuth::new(username, password)` encodes credentials as Base64 and sets the `Authorization: Basic <encoded>` header.
- **Bearer authentication** -- `BearerAuth::new(token)` sets the `Authorization: Bearer <token>` header.
- **Secret redaction** -- `AuthScheme`, `BasicAuth`, and `BearerAuth` implement custom `Debug` and `Display` traits that redact sensitive values. Credentials are never printed in logs or error messages.
- **Input validation** -- usernames must not contain `:`; usernames and passwords may be UTF-8, and bearer tokens may be empty, contain spaces, or contain UTF-8 accepted by the underlying HTTP header type. CR/LF is rejected and the generated header is validated at construction. Violations return `Error::InvalidAuthHeader`.
- **Client-level auth** -- `ClientBuilder::auth(auth)` sets a default auth scheme for all requests through that client.
- **Request-level auth** -- `RequestBuilder::auth(auth)` overrides client-level auth for a single request.
- **Precedence resolution** -- `resolve_request_auth()` applies the following rules:
  1. If request-level explicit auth is set, use it.
  2. If request-level auth is disabled (via `without_auth()`), no auth is applied.
  3. If client-level auth is set, use it.
  4. Otherwise, no auth.
- **Cross-origin redirect stripping** -- the redirect engine strips `Authorization` headers on cross-origin redirects via `SENSITIVE_HEADERS`. Client-level auth is NOT reapplied on cross-origin redirect hops. Same-origin redirects do reapply client-level auth.
- **Request-level auth disable** -- `RequestBuilder::without_auth()` sets an explicit flag that prevents client-level auth from being applied to a specific request, even when the client has auth configured. This is useful for requests to endpoints that must not carry credentials.
- **URL credentials** -- URL userinfo (e.g., `https://user:pass@host/`) is rejected. Configure `BasicAuth` or another explicit auth scheme instead; the password is not echoed in the resulting error.

### Python Auth API

The Python bindings expose:

- `BasicAuth(username, password)` -- creates a Basic auth object.
- `BearerAuth(token)` -- creates a Bearer auth object.
- `auth=(username, password)` tuple shorthand -- automatically converted to Basic auth.
- `auth=None` on request methods -- uses client-level auth (default behavior).
- `auth=BasicAuth(...)` on request methods -- overrides client-level auth.
- `auth=eggfetch.NOAUTH` on request methods -- disables auth for that request, even if the client has auth configured. `NOAUTH` is a sentinel object (not `None`).

## Python Streaming API (Milestone N)

The Python crate exposes true network streaming via `client.stream()`. This is distinct from the buffered `response.iter_bytes()` which iterates over cached content.

### StreamingResponse

`client.stream("GET", url)` returns a `StreamingResponse` context manager. The response body is consumed incrementally — chunks are read from the network as they arrive, not buffered in memory first.

**Sync API:**

```python
with client.stream("GET", url) as resp:
    for chunk in resp.iter_bytes():
        process(chunk)  # each chunk arrives from the network
    # or: resp.iter_text(), resp.iter_lines(), resp.read(), resp.text
```

**Async API:**

```python
async with client.stream("GET", url) as resp:
    async for chunk in resp.aiter_bytes():
        process(chunk)  # each chunk arrives from the network
    # or: resp.aiter_text(), resp.aiter_lines(), resp.aread(), resp.text
```

### StreamingResponse Lifecycle

- **State machine**: `StreamingResponse` uses four atomic states. A live response starts `streaming`; `read()`/`aread()` transitions it to `buffered`, iterators transfer ownership to `consumed`, and explicit or context-manager close transitions it to `closed`. These are terminal ownership transitions, not a required linear sequence.
- **GIL release**: sync iteration and blocking body reads release the Python GIL while waiting.
- **Pool permit**: the streaming response holds a pool permit (via `PoolGuard`) that is released when the body is fully consumed, explicitly closed, or dropped. Early `break` or cancellation also releases the permit.
- **Single consumption**: a stream can only be consumed once. Calling `read()` then `iter_bytes()` raises `StreamConsumed`; calling `text()` after an iterator has taken ownership does the same. Calling a body operation after `close()` raises `StreamClosed`.
- **Cancellation**: response close signals active readers and iterator producers; dropping an async iterator aborts its producer task, while dropping a sync iterator closes its worker channel.
- **Incremental decoding**: `iter_text()` / `aiter_text()` decode per-chunk using `encoding_rs`, correctly handling multibyte code points split across chunk boundaries. `iter_lines()` / `aiter_lines()` buffer partial lines across chunks.

### Buffered vs Streaming Iteration

| API | Behavior | Use case |
|-----|----------|----------|
| `response.iter_bytes()` | Iterates over pre-buffered chunks | Small responses, simple iteration |
| `response.text` | Returns fully buffered text | Small responses, read-once access |
| `client.stream().iter_bytes()` | Streams chunks from network | Large responses, backpressure control |
| `client.stream().read()` | Reads remaining stream into memory | When you need the full body but started streaming |

## Cookie Subsystem (Milestone O)

eggfetch implements RFC 6265 cookie handling with domain/path matching, thread-safe storage, and client-level state.

### Cookie Semantics

- **Parsing**: `Set-Cookie` response headers are parsed into `Cookie` structs with name, value, domain, path, secure, httpOnly, SameSite, and expiry attributes.
- **Storage**: `CookieJar` is a thread-safe cookie store. Each `Client` owns a `CookieJar` that accumulates cookies across requests.
- **Matching**: on each request, the client selects cookies whose domain matches the request host (including subdomains for domain cookies) and whose path is a prefix of the request path.
- **Secure flag**: cookies with `secure=true` are only sent over HTTPS.
- **Expiry**: cookies with `Max-Age=0` or past `Expires` are removed. Negative `Max-Age` is treated as zero.
- **Host-only vs domain**: cookies set without a `Domain` attribute are host-only and not sent to subdomains. Cookies set with an explicit `Domain` attribute are sent to the specified domain and all subdomains.

### Python Cookie API

- `Client(cookies={"name": "value"})` -- initializes the client jar with pre-set cookies.
- `client.get(url, cookies={"name": "value"})` -- sends request-local cookies without adding them to the persistent jar. The same keyword is available on the other request methods and `stream()`.
- `response.cookies` -- returns a `Cookies` mapping of `Set-Cookie` values from the response.
- Client jar cookies are sent on all matching requests (same-origin and cross-origin where domain/path match).

### Cookie/Auth Interaction

Disabling auth (via `auth=eggfetch.NOAUTH`) does not affect cookie handling. Cookies are sent independently of the auth state.

## Request Pipeline Order

When the client sends a request, transformations are applied in this order:

1. **Header merge**: client defaults and request headers are merged with request-level replacement semantics before redirect security decisions.
2. **Request overrides**: params, body, timeout, redirect policy, request-local cookies, and request auth are applied.
3. **Cookie selection**: matching client-jar cookies are computed for the current destination.
4. **Auth resolution**: `resolve_request_auth()` applies precedence rules (request auth > disabled > client auth > none).
5. **Validation and send**: body length and headers are validated, then the request is sent.
6. **Response state**: `Set-Cookie` is ingested before the next redirect hop.
7. **Redirect safety**: cross-origin hops strip `Authorization`, `Cookie`, and `Proxy-Authorization`; client auth is not reapplied, and safe destination jar cookies are recomputed.

On redirect, steps 3-6 repeat for the new destination URL (but client-level auth is suppressed on cross-origin hops, and cookies are recomputed for the new destination).

## Multipart (Milestone Q)

eggfetch implements streaming multipart/form-data request bodies for file uploads and mixed form+file payloads.

### Core Model

- **`Multipart`** -- owns a `Boundary` and a list of `Part`s. Provides a builder API: `Multipart::new().text("field", "value").bytes("file", "name", "type", data).stream(...)`.
- **`Part`** -- a single part with `name`, optional `filename`, optional `content_type`, optional extra `headers`, and a `PartBody`.
- **`PartBody`** -- `Bytes(Bytes)` or `Stream { stream: BoxBytesStream, length: Option<u64> }`.
- **`Boundary`** -- a validated multipart boundary string. Random generation uses a xorshift PRNG seeded from `SystemTime` + atomic counter (50 alphanumeric characters). Custom boundaries are validated via `Boundary::try_new()`.

### Streaming Encoder

`MultipartEncoder` implements `Stream<Item = Result<Bytes>>` as a state machine:

- **PartHeader** -- emits the boundary line, `Content-Disposition`, optional `Content-Type`, optional custom headers, and the blank line separator.
- **PartBody** -- polls the current part's body stream and forwards chunks.
- **TrailingCrlf** -- emits the `\r\n` after the part body.
- **FinalBoundary** -- emits `--boundary--\r\n`.
- **Done** -- stream complete.

The encoder preserves backpressure: it does not eagerly buffer file contents. A slow part body stream naturally backpressures the encoder.

### Known-Length Calculation

`Multipart::content_length()` uses checked arithmetic to sum all part header lengths, body lengths, boundary overhead, and the final terminator. Returns `Some(u64)` only when every part has a known length; returns `None` when any part is a stream with unknown length. The caller can use `Content-Length` for known-length bodies or fall back to chunked transfer encoding.

### Replayability

A multipart body is replayable only when all parts are `PartBody::Bytes`. If any part is a `PartBody::Stream`, the multipart is non-replayable. Redirect behavior for 307/308 rejects non-replayable multipart bodies (same as other stream request bodies).

### Python API

The Python bindings expose multipart via the `files=` kwarg on request methods:

```python
# Bytes directly
eggfetch.post(url, files={"file": b"data"})

# With filename and content type
eggfetch.post(url, files={"file": ("report.txt", b"contents", "text/plain")})

# Mixed data + files
eggfetch.post(url, data={"description": "sample"}, files={"file": open("data.bin", "rb")})

# Path-backed file via eggfetch.File
eggfetch.post(url, files={"file": eggfetch.File("/path/to/file.pdf")})
```

Supported `files=` forms:
- bytes value directly
- `(filename, bytes)` tuple
- `(filename, bytes, content_type)` triple
- `(filename, bytes, content_type, headers)` quad
- `eggfetch.File(path, filename=None, content_type=None)` wrapper

`files=` + `data=` combines form fields (from `data=`) and file parts (from `files=`) in a single multipart body. `files=` + `content=` or `files=` + `json=` raises `TypeError`.

`eggfetch.File` wraps a filesystem path. The file is read synchronously via `std::fs::read` (blocking in GIL context). This is acceptable for the initial implementation; true async file streaming may be added later.

## Pool Keying

Per-origin pool limits are keyed by `(scheme, host, port)`, where the port uses the scheme's default when not explicit. `http://example.com:80` and `http://example.com` share a per-origin limit; `http://example.com` and `https://example.com` are independent; `http://example.com:8080` is distinct from `http://example.com`.

When a proxy is involved, the pool key extends to `(proxy_origin, destination_origin, tunnel_mode)`, where `tunnel_mode` distinguishes HTTP forwarding from HTTPS CONNECT tunneling. Direct and proxied requests to the same destination have independent concurrency slots. Pool permits are semaphore-based concurrency tokens — they control how many requests may be in flight per origin, but they do not represent or manage TCP connections. The current implementation does not reuse TCP connections or tunnels; each request opens a fresh connection and releases the permit when the response body is consumed or dropped. See `docs/architecture/dependency-policy.md` for the full pool key specification.

### Environment-Variable Proxy Policy

eggfetch does **not** read `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, or `NO_PROXY` environment variables. Proxy configuration is always explicit via `ClientBuilder::proxy()` or `RequestBuilder::proxy()`. This is a deliberate design choice: implicit environment-variable proxying can cause surprising behavior in library contexts where the caller does not control the process environment.

## Pool Metrics

`PoolMetrics` exposes only `acquisition_waits` and `acquisition_cancellations`. Socket-level counters (connections_opened/reused/closed) were removed because hyper owns socket lifecycle and eggfetch cannot observe individual socket events through its current integration.

## TLS Configuration (Milestone T)

eggfetch implements deliberate TLS configuration without weakening secure defaults:

- **`TlsConfig`** -- TLS configuration type with builder pattern. Configures custom CA bundles, client certificates, TLS version policy, verification toggle, and SNI behavior.
- **`TlsConfigBuilder`** -- builder for constructing `TlsConfig` instances. Provides fluent API for setting trust stores, client identities, version constraints, and verification options.
- **`TrustStore`** -- custom CA bundle support. Loads PEM-encoded CA certificates for custom trust anchors. Falls back to system roots when no custom store is configured.
- **`ClientIdentity`** -- client certificate and private key for mutual TLS authentication. Supports PEM-encoded certificate chains and private keys.
- **`TlsVersion`** -- TLS version policy. Configures minimum and maximum supported TLS versions (e.g., TLS 1.2, TLS 1.3).

### Configuration Surface

TLS configuration is applied at two levels:

- **Client-level**: `ClientBuilder::tls_config(TlsConfig::builder().trust_store(...).build())` sets default TLS behavior for all requests through that client.
- **Verification toggle**: `TlsConfigBuilder::danger_accept_invalid_certs(true)` explicitly disables certificate verification with an opt-in API. This is a deliberate escape hatch with documentation warnings.

### Trust Store Hierarchy

The trust store resolution follows this order:

1. Custom `TrustStore` (if provided via `TlsConfig`)
2. Operating system native roots (if available)
3. Packaged Mozilla/WebPKI roots (fallback for minimal containers)

Certificate-chain and hostname verification are always enforced unless explicitly disabled via the verification toggle.

### Python API

The Python bindings expose TLS configuration via `Client` and `AsyncClient`:

```python
# Disable verification (insecure, requires explicit opt-in)
client = eggfetch.Client(verify=False)

# Custom CA bundle
client = eggfetch.Client(verify="/path/to/ca-bundle.pem")

# Client certificate (certfile, keyfile)
client = eggfetch.Client(cert=("/path/to/cert.pem", "/path/to/key.pem"))
```

### Enterprise and private-CA trust policy

A custom CA bundle **replaces** all default roots (system + WebPKI). This is
intentional: it prevents a misconfigured system trust store from silently
undermining the custom policy. If both system and private CAs are needed,
concatenate them into a single PEM file before passing it to `ca_certificate_pem()`.

Corporate MITM proxies are supported only when the proxy's signing CA is
present in the configured trust store. Certificate verification failures
through a corporate proxy indicate the proxy CA is missing from the bundle.

Client certificates for mTLS require both the certificate chain and the
private key. Unencrypted PEM private keys are supported; encrypted keys
produce an error at construction time. Private key material is never included
in debug output, error messages, or repr diagnostics.

## HTTP/2 (Milestone V)

eggfetch supports HTTP/2 via ALPN negotiation for HTTPS connections. HTTP/2
multiplexes multiple streams over a single TCP connection, providing
better performance for concurrent requests to the same origin.

### Version Policy

The `HttpVersionPolicy` enum controls which HTTP protocol versions the
client may negotiate:

- **`Http1Only`** -- only HTTP/1.1 is permitted. The client advertises
  only `http/1.1` in ALPN and will not accept HTTP/2.
- **`Http2Only`** -- only HTTP/2 is permitted. The client advertises
  only `h2` in ALPN. If the server does not negotiate HTTP/2, the
  connection fails.
- **`Auto`** (default) -- both `h2` and `http/1.1` are advertised. The
  server selects the preferred protocol.

### Feature Gating

HTTP/2 support is behind the `http2` Cargo feature. When the feature is
not enabled, `Http2Only` and `Auto` are silently downgraded to
`Http1Only` at connector construction time. This keeps the default build
small and HTTP/1.1-only.

### ALPN Configuration

ALPN protocols are set on the rustls configuration based on the version
policy. When a custom `TlsConfig` is provided, the policy clears and
rebuilds the ALPN list. When no custom TLS config is used, the connector
builder handles ALPN via `enable_http1()` / `enable_http2()`.

### Connector Construction

The connector is built with the appropriate HTTP version features based
on the policy. For `Http1Only`, only `enable_http1()` is called. For
`Http2Only`, only `enable_http2()`. For `Auto`, both are called, allowing
the connector to negotiate either protocol.

### Protocol Version Reporting

When the server negotiates HTTP/2, `response.version()` returns
`http::Version::HTTP_2`. In Python, `response.http_version` reports
`"HTTP/2"`. This is handled automatically by hyper's response parsing.

### Forbidden Header Stripping

Per RFC 9113, Section 8.2.2, certain HTTP/1.1 connection-specific headers
are forbidden in HTTP/2. The pipeline strips these unconditionally before
sending: `Connection`, `Keep-Alive`, `Proxy-Connection`, `Transfer-Encoding`,
`Upgrade`, and `TE` with any value other than `trailers`. This is safe
because these are hop-by-hop headers that should never be forwarded
end-to-end.

### HTTP/2 Error Taxonomy

eggfetch maps HTTP/2-specific errors to dedicated error variants:

- **`Http2GoAway`** -- the server sent a GOAWAY frame, terminating the
  connection. Includes the last stream ID and debug data.
- **`Http2StreamReset`** -- a stream was reset via RST_STREAM. The reason
  code is included (e.g., `REFUSED_STREAM`, `CANCEL`).
- **`Http2FlowControl`** -- a flow-control error occurred.
- **`Http2Protocol`** -- a generic HTTP/2 protocol error.

Hyper wraps h2 errors internally; eggfetch classifies them by inspecting
the error's display output for known h2 patterns.

### Retry Classification

`REFUSED_STREAM` errors are classified as retryable for replayable
requests. `CANCEL`, `GOAWAY`, flow-control, and protocol errors are not
retried. This aligns with RFC 9113, Section 7.2.4, which recommends
retrying requests that receive a `REFUSED_STREAM` error code.

### Trailers

HTTP trailers (both HTTP/1.1 chunked trailers and HTTP/2 trailing HEADERS
frames) are not currently supported. The `wrap_incoming` adapter only yields
data frames; when a trailers frame arrives, the stream ends normally without
surfacing the trailer headers. This is a known limitation.

### Pool and Concurrency

HTTP/2 multiplexes streams on a single connection, but eggfetch's pool
permits still control logical request concurrency per origin. The pool
does not directly manage TCP connections or HTTP/2 stream counts. hyper
handles connection-level multiplexing internally. The server's
`SETTINGS_MAX_CONCURRENT_STREAMS` is respected by h2 internally; when the
stream limit is reached, new streams are queued by hyper until a slot
opens.

### Python API

```python
# Enable HTTP/2 negotiation
client = eggfetch.Client(http2=True)
r = client.get("https://example.com")
print(r.http_version)  # "HTTP/2" or "HTTP/1.1"

# Async
async with eggfetch.AsyncClient(http2=True) as client:
    r = await client.get("https://example.com")

# Catch h2-specific errors
try:
    r = client.get("https://example.com")
except eggfetch.Http2GoAway as e:
    print(f"Server sent GOAWAY: {e}")
except eggfetch.Http2StreamReset as e:
    print(f"Stream reset: {e}")
```

## Transport Stack

The transport layer is built on:

- **hyper** -- HTTP/1.1 and HTTP/2 protocol implementation (HTTP/2 requires the `http2` feature).
- **hyper-util** -- high-level client utilities (connection handling, IO traits).
- **hyper-rustls** -- TLS integration via rustls, providing HTTPS support with ALPN negotiation.
- **tokio** -- async runtime powering I/O and timers.
- **tokio-rustls** -- async TLS streams for tokio + rustls.
- **rustls** -- memory-safe TLS implementation, preferred over native TLS for portability.
- **quinn** -- QUIC transport implementation (HTTP/3 requires the `http3` feature).
- **h3** -- HTTP/3 protocol layer over QUIC (requires the `http3` feature).

## Python Sync Adapter (Milestone F)

The Python sync API owns a tokio runtime and an `eggfetch_core::Client` per `PyClient` instance. When a user calls `eggfetch.get(...)`, the sync adapter:

1. Converts Python arguments to owned Rust types (headers, URL, body bytes, timeout).
2. Releases the GIL via `py.allow_threads`.
3. Blocks on the async Rust engine via `runtime.block_on(future)`.
4. Buffers the response body via `response.bytes().await`.
5. Re-acquires the GIL and returns a `PyResponse` with buffered data.

When a user calls `client.stream("GET", url)`, the sync adapter returns a `StreamingResponse` context manager. Iterating over the response body advances the stream one chunk at a time, releasing the GIL during each read. `read()` and `text()` also release the GIL while waiting for the body.

The sync adapter does not contain its own TCP connections, TLS handshakes, or body parsing. It delegates entirely to eggfetch-core.

Top-level helpers (`get`, `post`, etc.) create a short-lived runtime and client per call. The `PyClient` class owns a persistent runtime and client for connection reuse.

Supported kwargs: `headers`, `params`, `content`, `data`, `json`, `files`, `timeout`, `cookies`, `auth`, `follow_redirects`, `max_redirects`, `verify`, `cert`. Request-local cookies are serialized for the initial destination and are not added to the persistent client jar. Unsupported kwargs raise `TypeError`.

## Request Builder Compatibility (Milestone I)

The Python crate provides a requests/httpx-compatible request construction surface. All methods (top-level helpers, `Client`, and `AsyncClient`) accept the same keyword arguments:

- **`headers`** -- dict or sequence of `(name, value)` pairs. Inserted via `eggfetch_core::Headers`, which validates names and values (no empty names, no bare CR/LF).
- **`params`** -- dict or sequence of `(key, value)` pairs. Appended to the URL query string via `url::Url::query_pairs_mut()`. Existing query parameters are preserved.
- **`content`** -- raw body as `bytes`, `str`, or `bytearray`. Sent as-is with no auto Content-Type.
- **`data`** -- form data as dict or sequence of pairs. Encoded as `application/x-www-form-urlencoded` via `url::form_urlencoded`.
- **`json`** -- JSON-serializable Python object. Serialized via Python's `json.dumps()`. Auto-sets Content-Type to `application/json`.
- **`timeout`** -- float (seconds) or `Timeout` object. Overrides client-level timeout per-request.
- **`cookies`** -- mapping of string names to values for this request only. These cookies do not persist in the client jar and are stripped on a cross-origin redirect.
- **`follow_redirects`** -- bool. Overrides client-level redirect policy per-request.
- **`max_redirects`** -- usize. Overrides client-level max redirects per-request.
- **`verify`** -- bool or str. Client-level TLS verification setting. `False` disables verification; a string path sets a custom CA bundle.
- **`cert`** -- str or tuple. Client certificate for mutual TLS. A string is a single PEM file; a tuple is `(certfile, keyfile)`.

Body kwargs (`content`, `data`, `json`) are mutually exclusive. `files` may be combined with `data` (forming multipart), but `files` conflicts with `content` and `json` and raises `TypeError`. Auto Content-Type is only set for `data` and `json`; explicit Content-Type headers are preserved.

The conversion layer lives in `conversion.rs` and is shared by both sync and async paths. No HTTP logic exists outside `eggfetch-core`.

## Python Async Adapter (Milestone G)

The Python async API exposes `AsyncClient` with `__aenter__`/`__aexit__` and awaitable request methods, targeting asyncio.

The async adapter bridges eggfetch-core's Rust futures to Python coroutines using `pyo3-async-runtimes`. Each request method (`get`, `post`, etc.) uses `future_into_py` to convert the Rust future into a Python awaitable. The async block buffers the response body before returning, so the Python side receives a fully-buffered `PyResponse` (same type as the sync API).

Key design decisions:

- **Unified response construction**: `PyResponse::from_core_response_with_body()` constructs a response from pre-buffered data and handles redirect history conversion. Both sync and async paths use this method, ensuring consistent behavior and history handling.
- **Pre-resolved futures for context manager**: `__aenter__` and `__aexit__` return pre-resolved `asyncio.Future` objects rather than using `future_into_py`, which would attempt to start a nested runtime.
- **Cancellation safety**: cancelling an in-flight request drops the Rust future cleanly; pool permits are released via RAII `PoolGuard`.

## Response Compatibility Surface (Milestone H)

The Python `PyResponse` type presents a requests/httpx-compatible surface over the buffered response data:

- **Properties**: `status_code`, `reason_phrase`, `headers`, `url`, `content`, `text`, `encoding`, `http_version`, `history`.
- **Status helpers**: `is_informational` (1xx), `is_success` (2xx), `is_redirect` (3xx), `is_client_error` (4xx), `is_server_error` (5xx), `is_error` (4xx+5xx).
- **JSON**: `json(**kwargs)` delegates to Python's `json.loads`.
- **Iterators**: `iter_bytes(chunk_size)`, `iter_text(chunk_size)`, `iter_lines()` return Python iterators over buffered content. For true network streaming that consumes chunks as they arrive, use `client.stream()` which returns a `StreamingResponse`.
- **Close**: `close()` and `aclose()` are no-ops for buffered responses.
- **Text decoding**: explicit `encoding` kwarg > Content-Type charset > UTF-8 fallback. Uses `encoding_rs` for non-UTF-8 charsets; `.text` uses lossy decode with replacement characters.
- **Headers.get_list()**: returns all values for a multi-value header as a list.
- **Repr**: `<Response [200 OK]>` format.
- **raise_for_status()**: raises `HTTPStatusError` for 4xx/5xx with reason phrase in message.

All response data is buffered at creation time. Streaming iteration returns Python iterators over the cached chunks. For true network streaming, use `client.stream()` which returns a `StreamingResponse` context manager.

## Future Crates

The project may later add:

- **eggfetch-testing**: local protocol test servers, fixtures, and differential testing against requests/HTTPX.
- **eggfetch-bench**: benchmark harnesses for throughput, latency, and memory.

These crates do not exist yet. They will be added when the core engine is stable enough to test and measure.

## Current State

The post-Milestone-W state is complete. The core crate provides a working async HTTP client with HTTPS support, request/response modeling, headers, query parameters, streaming request/response bodies, connection pooling, phase-aware timeouts (pool, connect, write, read, total), redirect following with configurable policy, `RequestBody::try_clone_for_redirect()` for replay-safe redirects, metadata-only redirect history entries (`HistoryEntry`), total timeout as a single deadline across redirect chains, a cookie subsystem with RFC 6265 parsing, domain/path matching, secure/httpOnly/SameSite attributes, expiry handling, thread-safe `CookieJar`, and client-level cookie state, an authentication subsystem with Basic and Bearer token support, secret redaction, client and request-level auth, precedence resolution, request-level auth disable via `without_auth()`, cross-origin credential stripping (client auth not reapplied on cross-origin redirects), URL credential conversion, streaming multipart/form-data request bodies with `Multipart`, `Part`, `PartBody`, and `Boundary` types, a streaming encoder, known-length calculation, and boundary generation, response decompression (gzip, deflate, brotli, zstd) with Accept-Encoding negotiation, decoded-body resource limits (`max_decoded_body_size`, `max_decompression_ratio`), a proxy subsystem (HTTP proxying, HTTPS CONNECT tunneling, proxy authentication, per-request/client proxy configuration, NO_PROXY bypass), TLS configuration with custom CA bundles, client certificates, TLS version policy, verification toggle, and SNI behavior, HTTP/2 support with ALPN negotiation, `HttpVersionPolicy` enum, multiplexed connections, protocol version reporting, HTTP/2 error taxonomy (GoAway, StreamReset, FlowControl, Protocol), forbidden header stripping, retry classification for REFUSED_STREAM, trailers documentation, and pool concurrency model documentation, and experimental HTTP/3 support via QUIC transport with Quinn/h3, feature-gated behind `http3`, with `Http3Only` policy variant, TLS 1.3 requirement, 0-RTT disabled, and Python `http3=` kwarg. The Python crate exposes sync and async APIs with top-level helpers, `Client` and `AsyncClient` classes, requests/httpx-compatible response properties, status helpers, methods (`json()`, `raise_for_status()`, `iter_bytes()`, `iter_text()`, `iter_lines()`, `close()`/`aclose()`), true network streaming via `client.stream()` returning a `StreamingResponse` context manager (with `iter_bytes()`, `iter_text()`, `iter_lines()`, `read()`, `text()` and async equivalents), charset-aware text decoding with deterministic precedence, multi-value header support, case-insensitive headers, request body kwargs, form encoding, JSON body serialization, body kwarg mutual exclusion, `files=` kwarg with bytes/tuples/`File` wrapper support, `follow_redirects`/`max_redirects` kwargs, `Cookies` mapping wrapper (`client.cookies`, `response.cookies`, `cookies=` kwarg), `proxy=` kwarg, `decompress=` kwarg, `verify=`/`cert=` kwargs, `BasicAuth`/`BearerAuth` classes, `auth=` kwarg on all request methods, `NOAUTH` sentinel for per-request auth disable, streaming-specific exceptions (`StreamConsumed`, `StreamClosed`, `ResponseNotRead`), `http2=` kwarg for HTTP/2 negotiation, and a structured exception hierarchy with sync/async API parity verified by parity tests. The CLI crate remains a stub.
