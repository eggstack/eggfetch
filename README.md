# eggfetch

eggfetch is a Rust-native HTTP client engine with Python bindings and a CLI tool. The core is async-first: a Rust engine built on tokio and hyper provides connection pooling, timeouts, TLS configuration, and streaming. The Python bindings expose both sync and async APIs; the sync API blocks on the async engine while releasing the GIL, and the async API integrates with asyncio. There is exactly one networking implementation, living entirely in Rust.

> **Status: Milestones A through Z complete.**
> Milestones A–W are implemented as documented. The core engine supports HTTP/2 (ALPN negotiation, multiplexed connections, protocol version reporting), HTTP/3 (QUIC transport via Quinn/h3, feature-gated, experimental), response decompression (gzip, deflate, brotli, zstd), multipart/form-data uploads, cookies, authentication, proxy support (HTTP proxying, HTTPS CONNECT tunneling, proxy authentication, per-request/client proxy configuration, NO_PROXY bypass), TLS configuration (custom CA bundles, client certificates, TLS version policy, verification toggle), streaming, timeouts, connection pooling, and policy-driven retries with exponential backoff. The CLI crate is a full-featured HTTP client with argument parsing, streaming, machine-readable output, and deterministic exit codes.

[![CI](https://github.com/eggstack/eggfetch/actions/workflows/ci.yml/badge.svg)](https://github.com/eggstack/eggfetch/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/eggfetch-core)](https://crates.io/crates/eggfetch-core)
[![PyPI version](https://img.shields.io/pypi/v/eggfetch)](https://pypi.org/project/eggfetch/)
[![License](https://img.shields.io/crates/l/eggfetch-core)](LICENSE-MIT)

## Architecture

eggfetch is a five-crate Cargo workspace:

- **eggfetch-core** owns all HTTP behavior: client, request builder, response, body, headers, timeout, error types, connection pooling, TLS configuration, and streaming. Every networking dependency lives here.
- **eggfetch-cli** is a thin binary that delegates to eggfetch-core for all HTTP work. It handles argument parsing, output formatting, and exit codes only.
- **eggfetch-python** is the Python bindings adapter. It wraps eggfetch-core via PyO3/maturin and exposes sync and async Python APIs. It does not contain its own HTTP logic.
- **eggfetch-ffi** is the C ABI bindings crate. It exposes eggfetch-core over a stable C interface with opaque handles, suitable for binding from C, C++, Swift, Kotlin, Ruby, and other FFI-capable languages.
- **eggfetch-node** is the Node.js N-API binding prototype. It wraps eggfetch-core via napi-rs and provides a thin JavaScript/TypeScript adapter over the async Rust engine.

The invariant is strict: all network I/O goes through eggfetch-core. The CLI, Python, FFI, and Node crates are adapters.

## Installation

**Python:**

```bash
pip install eggfetch
```

**Rust:**

```toml
[dependencies]
eggfetch-core = { version = "0.1", features = ["http1", "tls-rustls"] }
```

**CLI:**

```bash
cargo install eggfetch-cli
```

**GitHub Releases:** Pre-built binaries for Linux, macOS, and Windows are available on the [GitHub Releases](https://github.com/eggstack/eggfetch/releases) page.

## MVP Non-goals

eggfetch is not aiming for full requests/httpx drop-in parity in the MVP. Specifically, the MVP does not include:

- Full transport parity with HTTPX (ASGI/WSGI in-process transports, all SOCKS proxy modes)
- Trio/AnyIO support (asyncio only)
- Every authentication flow, advanced proxy mounting, or requests compatibility edge case

Note: HTTP/3 (QUIC) is now available as an experimental feature behind the `http3` Cargo feature flag. See [Milestone W: HTTP/3](#milestone-w-http3-experimental) below.

See [plans/ROADMAP.md](plans/ROADMAP.md) for the full roadmap and compatibility policy.

## HTTPX Compatibility

eggfetch targets HTTPX 0.28.1 compatibility in measured phases. Phase 0 establishes the compatibility contract:

- **Pinned reference**: `httpx==0.28.1` with a machine-readable compatibility profile in `compat/httpx/0.28.1/`
- **API manifest comparison**: Automated generators compare eggfetch's public API surface against HTTPX's golden manifest
- **Required differential tests**: Fail-closed compatibility tests that cannot be skipped in CI
- **Allowed differences**: Every intentional behavioral difference is documented, categorized, and linked to tests

Key differences from HTTPX:
- **Redirect default**: eggfetch does NOT follow redirects by default (HTTPX does)
- **Event hooks**: Not yet implemented (tracked as required-later)
- **WSGI/ASGI transports**: Not applicable (eggfetch is a network client)

Phase 1 (production semantics) closes these gaps:
- **Timeout defaults**: `Timeout::compat()` provides HTTPX's 5-second network timeout defaults; `Timeout::native()` provides eggfetch's 30-second defaults.
- **Resource limits**: `Limits` object provides HTTPX-compatible pool concurrency control (100 max connections, 20 keep-alive, 5s expiry).
- **Environment trust**: `trust_env=True` (default) reads `HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY`/`NO_PROXY` env vars.

See [`compat/httpx/0.28.1/`](compat/httpx/0.28.1/) for the full profile and [`docs/reference/compatibility.md`](docs/reference/compatibility.md) for the feature matrix.

## HTTPX Drop-In Compatibility

eggfetch provides a drop-in HTTPX 0.28.1 compatibility facade:

```python
from eggfetch.compat.httpx import Client, AsyncClient, Request, Response, URL, Headers

# Drop-in replacement for httpx
client = Client()
response = client.get("https://example.com")
```

Phase 2 (object model and core API parity) and Phase 3 (streaming and bodies) are complete. See [`plans/httpx-drop-in-phase-2-status.md`](plans/httpx-drop-in-phase-2-status.md) and [`plans/httpx-drop-in-phase-3-status.md`](plans/httpx-drop-in-phase-3-status.md) for details, [`compat/httpx/0.28.1/`](compat/httpx/0.28.1/) for the pinned profile, and [`docs/reference/compatibility.md`](docs/reference/compatibility.md) for the feature matrix.

Phase 3 adds: `SyncByteStream`, `AsyncByteStream`, and `ByteStream` base classes; `iter_raw()`/`aiter_raw()` for raw transport bytes; chunk size parameters on all iterators; streaming request bodies (iterables, file-like, custom streams); and multipart passthrough to the native encoder.

Phase 4 adds: transports, mounts, auth, hooks, WSGI/ASGI transports. See [`plans/httpx-drop-in-phase-4-status.md`](plans/httpx-drop-in-phase-4-status.md) for details.

Phase 5 adds: downstream validation, behavior corpus, upstream test inventory, evidence reporting, and compatibility-stage decision. See [`plans/httpx-drop-in-phase-5-status.md`](plans/httpx-drop-in-phase-5-status.md) for details.

The downstream validation portfolio covers 12 representative downstream consumer packages with defined performance budgets. The compatibility stage decision is Stage C (asyncio drop-in), justified by the evidence corpus.

## Target API Shapes

The Python API targets familiar requests/httpx patterns:

```python
import eggfetch as httpx

r = httpx.get("https://example.com", params={"q": "test"}, timeout=5.0)
print(r.status_code)
print(r.headers)
print(r.text)
r.raise_for_status()

with httpx.Client(headers={"User-Agent": "eggfetch"}, limits=httpx.Limits(max_connections=100)) as client:
    r = client.post(
        "https://example.com/api",
        json={"a": 1},
        cookies={"session": "request-only"},
    )

async with httpx.AsyncClient() as client:
    r = await client.get("https://example.com")
```

Streaming responses consume the live body incrementally without buffering the full response in memory:

```python
# Sync streaming
with httpx.Client() as client:
    with client.stream("GET", "https://example.com/large") as response:
        for chunk in response.iter_bytes():
            process(chunk)

# Async streaming
async with httpx.AsyncClient() as client:
    async with client.stream("GET", "https://example.com/large") as response:
        async for chunk in response.aiter_bytes():
            process(chunk)
```

The Rust API remains idiomatic rather than Python-shaped:

```rust
use eggfetch_core::Client;

let client = Client::new();
let mut response = client
    .get("https://example.com")?
    .header("user-agent", "eggfetch")
    .query("q", "test")
    .send()
    .await?;

assert!(response.status().is_success());
let bytes = response.bytes().await?;
```

### Milestone B: Core HTTP Engine (complete)

The core HTTP engine is implemented with the following capabilities:

- **Client / ClientBuilder** -- configurable async HTTP client with builder pattern.
- **Request / RequestBuilder** -- method, URL, headers, query parameters, and body assembly.
- **Response / ResponseBody** -- status, headers, and buffered byte body access.
- **RequestBody** -- supports byte buffers.
- **Headers** -- typed header map backed by `http::HeaderMap`.
- **HTTPS** -- TLS via rustls (default feature `tls-rustls`).
- **Error taxonomy** -- structured `Error` enum covering network, HTTP, timeout, and builder errors.

### Milestone C: Connection Management (complete)

Connection pooling and reuse is implemented with the following capabilities:

- **Connection pooling** -- HTTP/1.1 keep-alive connection reuse via hyper-util's built-in pool.
- **Limits model** -- `Limits` type provides HTTPX-compatible resource limits (`max_connections`, `max_keepalive_connections`, `keepalive_expiry`). `Limits::compat()` returns HTTPX defaults (100 connections, 20 keep-alive, 5s expiry).
- **Max connections per host** -- configurable limit via `PoolConfig::max_connections`.
- **Idle timeout** -- connections closed after a configurable idle period via `PoolConfig::idle_timeout`.
- **Pool metrics** -- `PoolMetrics` exposed via `Client::pool_metrics()` and `ClientBuilder::pool_metrics()`.
- **Graceful degradation** -- waiter cancellation support without deadlocks.

### Milestone D: Timeout System (complete)

Phase-aware timeout behavior is implemented with the following capabilities:

- **Timeout configuration** -- `Timeout` type with per-phase durations (pool, connect, write, read, total).
- **Named timeout defaults** -- `Timeout::compat()` (HTTPX-compatible 5s all phases) and `Timeout::native()` (eggfetch 30s per-phase, no total).
- **Client-level timeouts** -- set via `ClientBuilder::timeout()`.
- **Request-level timeouts** -- set via `RequestBuilder::timeout()`, overriding client defaults per-field.
- **Pool timeout** -- time waiting for a connection slot from the pool. Enforced via `tokio::time::timeout` around acquisition.
- **Total timeout** -- wall-clock cap across the entire request lifecycle. Enforced via `tokio::time::timeout` around the full send.
- **Read timeout** -- enforced per chunk by a wrapper stream. Fires `Error::Timeout { phase: Read }` if no response body chunk arrives within the duration. Resets on every chunk arrival.
- **Write timeout** -- enforced per chunk by a wrapper stream. Fires `Error::Timeout { phase: Write }` if the request body producer does not yield the next chunk within the duration. Resets on every chunk delivery. Applies only to streamed request bodies.
- **Connect timeout** -- accepted and merged; not independently enforced. Use `total` as a backstop.
- **Timeout errors** -- `Error::Timeout { phase, elapsed }` identifies which phase timed out.
- **Cancellation safety** -- cancelled timeout-wrapped operations do not leak pool permits.

### Milestone E: Streaming Foundation (complete)

Protocol-neutral streaming for request and response bodies:

- **RequestBody** -- `Empty`, `Bytes`, and `Stream` variants. `Stream` holds a `BoxBytesStream` with optional known length. Stream bodies are wired through hyper's `StreamBody` and piped to the transport incrementally (no eager buffering). Producers are polled lazily.
- **ResponseBody** -- `Buffered`, `Streaming`, and `Consumed` variants. `bytes()` and `text()` are async. `bytes_stream()` returns a `BoxBytesStream`.
- **Response streaming** -- `bytes_stream()` yields chunks as they arrive; `text_lines()` splits byte stream into text lines.
- **Single-consumption** -- streaming bodies can only be consumed once; double-consume returns an error.
- **Client integration** -- all responses are wrapped as streaming `ResponseBody` via hyper's `Incoming` adapter.
- **Chunked transfer encoding** -- test server supports chunked responses for integration testing.

### Hardening Pass: Streaming, Timeouts, and Pool Lease (complete)

Post-Milestone-E correctness work landed:

- **True streaming uploads** -- request bodies stream into hyper via `UnsyncBoxBody<Bytes, Box<dyn StdError + Send + Sync>>`; no eager buffering.
- **Read timeout** -- `Timeout::read` enforces per-chunk arrival deadline on response body streams.
- **Write timeout** -- `Timeout::write` enforces per-chunk producer deadline on streamed request bodies.
- **Pool lease on body** -- pool permits are attached to streaming response bodies and released only when the body is dropped or fully consumed. This keeps per-origin limits meaningful while bodies are in flight.
- **Origin-keyed pool** -- per-origin limits are keyed by `(scheme, host, port)`. `http://example.com` and `https://example.com` are independent origins.
- **Pool metrics** -- only `acquisition_waits` and `acquisition_cancellations` remain; skeletal socket counters were removed because hyper owns socket lifecycle.

### Milestone F: Python Sync API (complete)

PyO3/maturin Python bindings exposing a synchronous API over the async Rust core:

- **Sync helpers** -- `request`, `get`, `post`, `put`, `patch`, `delete`, `head`, `options` top-level functions.
- **Client** -- `eggfetch.Client` with context manager support, default headers, default timeout, and connection reuse.
- **GIL release** -- all blocking network execution releases the Python GIL via `py.allow_threads`.
- **Buffered responses** -- `Response` exposes `status_code`, `headers`, `url`, `content` (bytes), `text`, `is_success`, and `raise_for_status()`.
- **Case-insensitive headers** -- `Headers` supports `[]` access, `.get()`, `in`, iteration, `keys()`, `values()`, `items()`.
- **Timeout mapping** -- scalar `timeout=float` or `timeout=None` maps to Rust phase-aware timeouts.
- **Exception hierarchy** -- `EggfetchError` base with `RequestError`, `InvalidUrl`, `TimeoutException` (and phase-specific subclasses), `NetworkError`, `ProtocolError`, `BodyError`, `HTTPStatusError`.
- **Unsupported kwargs** -- raises `TypeError` with the unsupported argument name.

### Milestone G: Python Async API (complete)

Async Python API over the async Rust core via pyo3-async-runtimes:

- **AsyncClient** -- `eggfetch.AsyncClient` with `async with` support and connection reuse.
- **Awaitable requests** -- `get`, `post`, `put`, `patch`, `delete`, `head`, `options`, `request` all return awaitables.
- **Async context manager** -- `__aenter__`/`__aexit__` for resource cleanup.
- **Concurrent requests** -- multiple `await client.get(...)` calls can run concurrently.
- **Cancellation** -- cancelling an in-flight request cleans up without resource leaks.
- **Reuses sync response wrappers** -- `PyResponse`, `PyHeaders`, exception hierarchy shared with sync API.

### Milestone H: Response Compatibility Surface (complete)

Requests/HTTPX-compatible Response surface in Python:

- **Response properties** -- `status_code`, `reason_phrase`, `headers`, `url`, `content`, `text`, `encoding`, `http_version`, `history`.
- **Status helpers** -- `is_informational`, `is_success`, `is_redirect`, `is_client_error`, `is_server_error`, `is_error` (all `#[getter]` returning `bool`).
- **JSON** -- `json(**kwargs)` delegates to Python's `json.loads` with kwargs passthrough.
- **Streaming iterators** -- `iter_bytes(chunk_size)`, `iter_text(chunk_size)`, `iter_lines()` return Python iterators over buffered content. For true network streaming that consumes chunks as they arrive, use `client.stream()` which returns a `StreamingResponse`.
- **Close** -- `close()` and `aclose()` (no-ops for buffered responses).
- **Text decoding** -- charset-aware decoding: explicit `encoding` kwarg > Content-Type charset > UTF-8 fallback. Uses `encoding_rs` for non-UTF-8 charsets.
- **Headers.get_list()** -- returns all values for a multi-value header as a list.
- **Improved repr** -- `<Response [200 OK]>` format.
- **Improved raise_for_status()** -- includes reason phrase in error message.

### Milestone I: Request Builder Compatibility Surface (complete)

Requests/HTTPX-compatible request construction in Python:

- **Headers** -- dict or sequence of `(name, value)` pairs. Case-insensitive, validated (no empty names, no bare CR/LF).
- **Params** -- dict or sequence of `(key, value)` pairs. Appended to URL query string, preserving existing query.
- **Content** -- raw body as `bytes`, `str`, or `bytearray`. No auto Content-Type.
- **Data** -- form data as dict or sequence of pairs. Auto Content-Type `application/x-www-form-urlencoded`.
- **JSON** -- JSON-serializable object. Serialized via Python's `json.dumps()`. Auto Content-Type `application/json`.
- **Body mutual exclusion** -- `content`, `data`, `json` are mutually exclusive; more than one raises `TypeError`.
- **Timeout override** -- request-level `timeout` overrides client default per-request.
- **Redirect kwargs** -- `follow_redirects` and `max_redirects` override client policy per-request.

### Milestone N: Semantic Tightening (complete)

Public-API stabilization and correctness audit:

- **True Python streaming** -- `client.stream()` / `async_client.stream()` returns a `StreamingResponse` context manager with `iter_bytes()`, `iter_text()`, `iter_lines()`, `read()`, `text()` (sync) and `aiter_bytes()`, `aiter_text()`, `aiter_lines()`, `aread()`, `text()` (async). Chunks arrive from the network without eager buffering.
- **Body state machine** -- atomic `streaming`, `buffered`, `consumed`, and `closed` states. A response is buffered by `read()`/`aread()`, transferred to `consumed` by an iterator, and becomes `closed` after explicit/context-manager close. Invalid transitions raise named exceptions.
- **Named streaming exceptions** -- `StreamConsumed`, `StreamClosed`, `ResponseNotRead` (all subclass `EggfetchError`).
- **Auth disable sentinel** -- `eggfetch.NOAUTH` disables per-request auth even when the client has auth configured.
- **Cross-origin redirect auth fix** -- client-level auth is no longer reapplied on cross-origin redirect hops.
- **Cookie/auth audit** -- comprehensive test coverage for cookie isolation, auth redaction, precedence, and cross-origin behavior.
- **Sync/async parity** -- 30 parity tests covering cookies, auth, and streaming.

### Validation and packaging pass (complete)

- Sync body reads release the GIL while waiting on the live response.
- Iterator producers are cancelled when their iterator or response is closed; async producers are aborted on iterator drop.
- Client/request headers are merged before redirect security decisions. Cross-origin hops strip raw credentials and recompute only destination-matching jar cookies.
- Request-local `cookies=` values are sent once and are not persisted in a client jar.
- Native TLS roots are preferred; packaged Mozilla roots are used only when native roots are unavailable. Certificate or hostname verification failures never trigger fallback.
- CI covers Ubuntu, macOS, and Windows Python 3.10–3.13, plus clean wheel smoke tests on each operating system.

### Milestone Q: Multipart and File Uploads (complete)

Streaming multipart/form-data request bodies with Python `files=` compatibility:

- **Core model** -- `Multipart`, `Part`, `PartBody`, and `Boundary` types with builder API.
- **Streaming encoder** -- state-machine stream with backpressure; no eager buffering of file contents.
- **Known-length optimization** -- computes `Content-Length` via checked arithmetic when all parts have known sizes; falls back to chunked transfer for unknown-length streams.
- **Boundary generation** -- random alphanumeric boundary with validated custom boundary support.
- **Python `files=` kwarg** -- bytes, tuples `(filename, data)`, `(filename, data, content_type)`, `(filename, data, content_type, headers)`, and `eggfetch.File(path)` wrapper.
- **Mixed `data=` + `files=`** -- form fields from `data=` and file parts from `files=` combined in a single multipart body.
- **Conflict rejection** -- `files=` with `content=` or `json=` raises `TypeError`.
- **Cancellation safety** -- dropped file handles and streams release resources cleanly.

The multipart feature is gated behind the `multipart` feature in eggfetch-core. The Python crate enables it by default.

### Milestone R: Response Compression and Decompression (complete)

Streaming response decompression with Accept-Encoding negotiation:

- **Core model** -- feature-gated `compression-gzip`, `compression-deflate`, `compression-brotli`, and `compression-zstd` features. Each enables a streaming decoder without pulling in unneeded dependencies.
- **Streaming decoders** -- async decompression via `async-compression` wrapped around `BoxBytesStream`. No full-body buffering; natural backpressure preserved.
- **Accept-Encoding negotiation** -- automatic header injection when decompression is enabled. Only compiled-in algorithms are advertised.
- **Content-Encoding parsing** -- ordered list parsing with reverse-order decode (e.g., `gzip, br` decodes brotli first, then gzip).
- **Header policy** -- `Content-Encoding` and `Content-Length` are stripped from decoded response headers.
- **Client configuration** -- `ClientBuilder::automatic_decompression(bool)` controls the default. Enabled by default when compression features are compiled in.
- **Per-request override** -- `RequestBuilder::decompress(bool)` overrides client-level setting.
- **Buffered decompression** -- buffered responses decompress synchronously via `flate2`.
- **Python API** -- `Client(decompress=True)`, `AsyncClient(decompress=True)`, and `client.get(url, decompress=False)` kwarg.
- **Error mapping** -- `DecompressionError` and `UnsupportedContentEncoding` Python exceptions.
- **Resource limits** -- maximum nesting depth of 4 content encodings.
- **Deflate compatibility** -- uses `async-compression`'s deflate decoder which handles zlib-wrapped streams (standard HTTP deflate).

### Milestone T: TLS Configuration (complete)

Deliberate TLS configuration without weakening secure defaults:

- **`TlsConfig`** -- TLS configuration type with builder pattern (`TlsConfigBuilder`).
- **`TrustStore`** -- custom CA bundle support via PEM files or system roots.
- **`ClientIdentity`** -- client certificate and private key for mutual TLS.
- **`TlsVersion`** -- TLS version policy (minimum/maximum supported versions).
- **Verification toggle** -- `TlsConfigBuilder::danger_accept_invalid_certs(true)` with explicit opt-in.
- **SNI behavior** -- Server Name Indication enabled by default, configurable.
- **Python API** -- `Client(verify=..., cert=...)` kwargs for verification and client certificates.
- **PEM parsing** -- `pem-rfc7468` for custom CA bundles and client certificate loading.

#### Enterprise and private-CA usage

When operating behind a corporate proxy or internal PKI, the default trust
store (native OS roots + WebPKI fallback) may not include your organization's
CA. Pass a custom CA bundle to the client constructor:

```python
client = eggfetch.Client(verify="/etc/corp/ca-bundle.pem")
```

A custom bundle **replaces** the default system roots entirely. This is
intentional: it prevents a misconfigured system trust store from silently
undermining the custom policy. If you need both system roots and private CAs,
concatenate them into a single PEM file before passing it in.

Corporate proxy interception (MITM) is only supported if the proxy's signing
CA is present in the configured trust store. If you see certificate
verification failures when a corporate proxy is active, add the proxy's CA to
your bundle.

For mTLS with client certificates, supply both the certificate chain and
private key:

```python
client = eggfetch.Client(
    cert=("/path/to/client-cert.pem", "/path/to/client-key.pem")
)
```

Unencrypted PEM private keys are supported. Encrypted keys will produce a
clear error at construction time.

### Current limitations

- Python 3.10–3.13 and the CI-supported Ubuntu, macOS, and Windows platforms are the current compatibility target. Other platforms may work but are not release-tested yet.
- `connect` timeout is accepted but not independently enforced (use `total` as backstop).
- Trio/AnyIO support deferred to a later milestone.

### Milestone U: Retry and Resilience Policy (complete)

Policy-driven retries with idempotency awareness and exponential backoff:

- **`RetryPolicy`** -- configurable retry policy with builder pattern. Controls max attempts, total elapsed budget, backoff strategy, method eligibility, and status codes.
- **Opt-in by default** -- retries are disabled unless explicitly configured. POST/PATCH never retried by default.
- **Method safety** -- GET, HEAD, OPTIONS are safe to retry. POST, PUT, DELETE, PATCH require explicit opt-in.
- **Body replayability** -- only empty and byte bodies are retried. Stream bodies are never retried.
- **Exponential backoff** -- bounded exponential backoff with jitter. Configurable factor, initial delay, and max delay.
- **`Retry-After` support** -- optional parsing of `Retry-After` headers (seconds).
- **Total deadline** -- a single total deadline spans all retry attempts and backoff sleeps.
- **Per-request override** -- `RequestBuilder::retry(policy)` or `RequestBuilder::without_retry()` overrides client-level policy.
- **Python API** -- `Retry(max_attempts=3, backoff_factor=0.2, statuses={429, 503})` class with `retries=` kwarg on `Client()` and all request methods. `retries=False` disables retries per-request.
- **Error taxonomy** -- `BodyNotReplayableForRetry`, `RetryBudgetExhausted`, `RetryNotConfigured` Python exceptions.

### Milestone V: HTTP/2 (complete)

HTTP/2 support with ALPN negotiation, error taxonomy, forbidden header handling, and protocol version reporting:

- **`HttpVersionPolicy`** -- enum with `Http1Only`, `Http2Only`, and `Auto` variants. Controls which HTTP protocol versions the client may negotiate. Default is `Auto`.
- **ALPN negotiation** -- the client advertises `h2` and/or `http/1.1` via ALPN based on the version policy. When `Auto`, both are advertised; when `Http2Only`, only `h2`; when `Http1Only`, only `http/1.1`.
- **Feature-gated** -- the `http2` feature enables HTTP/2 support in hyper, hyper-util, and hyper-rustls. Without the feature, `Http2Only` and `Auto` silently downgrade to `Http1Only`.
- **HTTP/2 error taxonomy** -- `Http2GoAway`, `Http2StreamReset`, `Http2FlowControl`, `Http2Protocol` error variants with diagnostic information. Hyper h2 errors are classified where possible.
- **Forbidden header stripping** -- `Connection`, `Keep-Alive`, `Proxy-Connection`, `Transfer-Encoding`, `Upgrade`, and non-`trailers` `TE` values are stripped unconditionally before sending.
- **Retry classification** -- `REFUSED_STREAM` errors are classified as retryable for replayable requests. `CANCEL`, `GOAWAY`, and other h2 errors are not retried.
- **Trailers** -- documented as not supported; trailing HEADERS frames are silently dropped.
- **Pool concurrency model** -- documented the separation of logical request concurrency (eggfetch pool), physical connection count (hyper), and per-connection stream limits (h2 `SETTINGS_MAX_CONCURRENT_STREAMS`).
- **Protocol version reporting** -- `response.version()` returns `HTTP_2` when the server negotiates HTTP/2. Python `response.http_version` reports `"HTTP/2"` accurately.
- **Client configuration** -- `ClientBuilder::http_version_policy(policy)` sets the version policy at client construction time.
- **Python API** -- `Client(http2=True)` and `AsyncClient(http2=True)` enable HTTP/2 negotiation (equivalent to `Auto` policy). `http2=False` or omitted uses HTTP/1.1 only.
- **Python error types** -- `Http2Error`, `Http2GoAway`, `Http2StreamReset`, `Http2FlowControlError` exception classes.
- **TLS integration** -- ALPN settings are correctly set on custom TLS configurations and default connector paths. Custom CA bundles, mTLS, and CONNECT tunnels preserve ALPN settings.
- **Backward compatible** -- HTTP/1.1 remains the default. No existing behavior changes unless `http2=True` is explicitly set.
- **Tests** -- 21 HTTP/2-specific Rust tests covering error taxonomy, retry classification, forbidden header stripping, version policy, concurrent requests, stream cancellation, and feature-gated builds.

### Milestone W: HTTP/3 (experimental)

HTTP/3 support via QUIC transport, implemented as an experimental, separately feature-gated capability:

- **`HttpVersionPolicy::Http3Only`** -- new enum variant that restricts the client to HTTP/3 only. The client will attempt a QUIC connection and fail if the server does not support HTTP/3.
- **QUIC transport via Quinn/h3** -- uses the `quinn` crate for QUIC connection management and the `h3` crate for the HTTP/3 protocol layer. QUIC provides built-in TLS 1.3, multiplexed streams without head-of-line blocking, and connection migration.
- **Feature-gated** -- the `http3` feature enables HTTP/3 support. Without the feature, `Http3Only` is not available and `Auto` falls back to HTTP/2 or HTTP/1.1.
- **TLS 1.3 requirement** -- QUIC mandates TLS 1.3. The `http3` feature requires `tls-rustls` and uses rustls for the QUIC TLS handshake.
- **0-RTT disabled** -- early data (0-RTT) is disabled in the initial implementation to avoid replay attack risks. This may be revisited in a future milestone.
- **Python API** -- `Client(http3=True)` and `AsyncClient(http3=True)` enable HTTP/3 negotiation. When `http3=True` is set, the client uses `Http3Only` policy. The Python crate exposes `Http3Error` and `Http3ConnectionError` exception types for QUIC-specific failures.
- **Protocol version reporting** -- `response.version()` returns `HTTP_3` when the server negotiates HTTP/3. Python `response.http_version` reports `"HTTP/3"`.
- **Client configuration** -- `ClientBuilder::http_version_policy(policy)` accepts the new `Http3Only` variant.
- **Backward compatible** -- HTTP/3 is opt-in. No existing behavior changes unless `http3=True` is explicitly set. HTTP/2 and HTTP/1.1 remain the default.
- **Experimental status** -- this feature is experimental. The QUIC/h3 ecosystem is maturing; API surfaces may change. Production use requires thorough testing with target servers.
- **Tests** -- Rust tests covering feature-gated compilation, version policy, protocol version reporting, and error taxonomy.

### Milestone K: CLI (complete)

A full-featured command-line HTTP client powered by eggfetch-core:

- **All HTTP methods** -- GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS, and arbitrary methods via `-X`.
- **Headers and query parameters** -- `-H NAME:VALUE` and `-q NAME=VALUE` (repeatable).
- **Request bodies** -- `--body`, `--body-file`, `--json`, `--form NAME=VALUE`, `--file NAME=@PATH`.
- **Streaming uploads** -- multipart/form-data via `--file` with `--form` combination.
- **Authentication** -- `--auth USER:PASS` (Basic) and `--bearer TOKEN`, with env var fallbacks (`EGGFETCH_AUTH`, `EGGFETCH_BEARER`).
- **Redirects** -- `--follow` (default on) / `--no-follow` with `--max-redirects`.
- **Cookies** -- `--cookie NAME=VALUE` and `--cookie-jar PATH`.
- **Proxy** -- `--proxy URL`, `--proxy-auth USER:PASS`, `--no-proxy DOMAINS`, with env var fallbacks.
- **TLS** -- `--verify` / `--no-verify`, `--cacert`, `--cert` / `--key`.
- **Timeouts** -- `--timeout`, `--connect-timeout`, `--total-timeout`, `--read-timeout`.
- **Retries** -- `--retry N`, `--retry-delay SECS`.
- **HTTP version** -- `--http1`, `--http2`, `--http3`.
- **Body limits** -- `--max-body-size`, `--max-decompression-ratio`.
- **Decompression** -- automatic by default, `--no-compress` to disable.
- **Output** -- streaming body to stdout, `--output PATH`, `--download`, `--include`, `--headers-only`, `--no-body`.
- **File safety** -- `--no-clobber` prevents overwriting existing output files.
- **Machine output** -- `--json-output` (pretty JSON), `--ndjson` (newline-delimited JSON), `--base64` for binary body encoding.
- **Secret safety** -- auth/proxy/cookie headers redacted in verbose output; secrets accept env vars.
- **Shell completions** -- `--generate-completion bash|zsh|fish|powershell|elvish`.
- **Exit codes** -- 0 success, 2 usage, 3 connect/TLS, 4 timeout, 5 protocol, 6 status (with `--check-status`), 7 I/O, 130 interrupted.
- **TTY awareness** -- verbose output on stderr, suppressed in machine mode.
- **Ctrl-C** -- graceful interruption with exit code 130.

### Milestone Y: Documentation and Examples (complete)

Complete user-facing documentation for Rust, Python, and CLI users:

- **Core concepts** -- architecture, request lifecycle, connection pooling, timeouts, streaming, redirects, cookies, authentication, proxy, TLS, retry, compression, and multipart.
- **Rust API guide** -- comprehensive guide with code examples covering Client, RequestBuilder, Response, Headers, errors, and all configuration options.
- **Python API guide** -- sync and async API guide covering Client, AsyncClient, Response, StreamingResponse, auth, cookies, proxy, TLS, retry, and multipart.
- **CLI reference** -- complete flag reference, output formats, exit codes, environment variables, and practical examples.
- **Migration guides** -- step-by-step guides for migrating from requests and HTTPX with side-by-side code comparisons.
- **Compatibility matrix** -- feature comparison table tracking requests/HTTPX compatibility status.
- **Cookbook** -- practical runnable examples for JSON APIs, streaming, auth, proxy, retry, and common patterns.
- **Security guidelines** -- TLS verification, credential handling, decompression limits, and troubleshooting.
- **Error reference** -- Rust Error variants, Python exception hierarchy, and CLI exit codes.

### Milestone Z: Additional Bindings and Frameworks (complete)

C ABI boundary and language binding prototypes for non-Python consumers:

- **C ABI bindings** -- `eggfetch-ffi` exposes eggfetch-core over a stable C interface. Opaque handle pattern for `Client`, `RequestBuilder`, `Response`, and `Error`. All functions are `extern "C"` with `#[repr(C)]` types. Thread-safe: concurrent handles from multiple threads are supported as long as each handle is used from one thread at a time.
- **Opaque handle pattern** -- clients, requests, responses, and errors are returned as opaque pointers (`*mut eggfetch_ffi_client`, etc.). Consumers never see internal struct layouts; lifetime and mutation are managed entirely through the C API surface.
- **Blocking-send runtime bridge** -- the FFI crate embeds a dedicated tokio runtime so that synchronous C callers can drive async requests without managing a Rust async runtime themselves.
- **FFI surface audit** -- every exported function was audited for memory safety, thread safety, and FFI hygiene. No `unsafe` escapes the crate boundary.
- **Node.js N-API prototype** -- `eggfetch-node` wraps eggfetch-core via napi-rs, exposing `Client`, `RequestBuilder`, `Response`, and streaming APIs to JavaScript and TypeScript. Supports both callback and Promise-based patterns.

## Documentation

Complete documentation lives in the `docs/` directory:

| Section | Description |
|---------|-------------|
| [getting-started/](docs/getting-started/) | Installation and quickstart guide |
| [concepts/](docs/concepts/) | Core concepts: architecture, lifecycle, pooling, timeouts, streaming, redirects, cookies, auth, proxy, TLS, retry, compression, multipart |
| [rust/guide.md](docs/rust/guide.md) | Rust API guide with examples |
| [python/guide.md](docs/python/guide.md) | Python sync/async API guide |
| [cli/guide.md](docs/cli/guide.md) | CLI reference and usage guide |
| [migration/](docs/migration/) | Migration guides from requests and HTTPX |
| [cookbook/](docs/cookbook/) | Practical runnable examples |
| [reference/](docs/reference/) | Compatibility matrix, feature matrix, error reference |
| [security/](docs/security/) | Security guidelines and troubleshooting |
| [architecture/](docs/architecture/) | Internal architecture documentation |
| [ffi/](docs/ffi/) | C ABI and FFI binding guide |
| [releases/](docs/releases/) | Release process and compatibility policy |

## Repository Layout

```text
Cargo.toml               workspace root
README.md
CHANGELOG.md
LICENSE-MIT
LICENSE-APACHE
SECURITY.md
CONTRIBUTING.md
AGENTS.md
rust-toolchain.toml      stable toolchain with rustfmt + clippy
rustfmt.toml             max_width 100
.clippy.toml             pedantic clippy config
.github/workflows/ci.yml CI pipeline
crates/
  eggfetch-core/         async HTTP engine
  eggfetch-cli/          CLI binary
  eggfetch-python/       Python bindings
    src/                 Rust adapter modules (PyO3)
    python/eggfetch/     Python package (__init__.py)
    tests/               Python tests
    pyproject.toml       maturin build config
  eggfetch-ffi/          C ABI bindings for eggfetch-core
  eggfetch-node/         Node.js N-API binding prototype
  eggfetch-bench/        Criterion-based benchmarks
docs/
  getting-started/       installation and quickstart
  concepts/              core concept explanations
  rust/                  Rust API guide
  python/                Python API guide
  cli/                   CLI reference
  migration/             migration guides (requests, HTTPX)
  cookbook/               practical examples
  reference/             compatibility matrix, feature matrix, errors
  security/              security guidelines and troubleshooting
  architecture/          internal architecture documentation
  ffi/                   CABI and FFI binding guide
  releases/              release process and compatibility policy
plans/                   milestone plans and roadmap
```

## Development

The workspace requires network dependencies (hyper, tokio, rustls, etc.) for eggfetch-core. To get started:

```sh
cargo check --workspace
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
```

### CI

CI runs format, clippy, and test checks on pushes and pull requests to `main` for informational visibility. It is not a merge gate.

### Python package

The Python package uses maturin for building. Requires Python 3.10+ and a virtual environment:

```sh
python3 -m venv .venv
source .venv/bin/activate
pip install pytest pytest-asyncio
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop -m crates/eggfetch-python/Cargo.toml
python -m pytest crates/eggfetch-python/tests/
maturin build -m crates/eggfetch-python/Cargo.toml
```

### Benchmarking

Criterion-based benchmarks live in `crates/eggfetch-bench/`. There are three benchmark suites:

- **microbench** -- measures URL construction, header operations, cookie matching, auth encoding, multipart assembly, decompression, retry policy, and request building.
- **e2e** -- measures one-shot requests, warm client reuse, concurrent requests, varying body sizes, streaming, uploads, and HTTP version comparison.
- **resources** -- measures buffered vs streaming body consumption, long-lived client stability, pool saturation, and header parsing performance.

Run all suites:

```sh
cargo bench -p eggfetch-bench
```

Run an individual suite:

```sh
cargo bench -p eggfetch-bench --bench microbench
cargo bench -p eggfetch-bench --bench e2e
cargo bench -p eggfetch-bench --bench resources
```

### Fuzzing

Fuzz targets exercise high-risk parsers and state machines using cargo-fuzz and libFuzzer. Proptest property tests verify round-trip and state-machine invariants across cookies, redirects, retries, and decompression.

Run a fuzz target:

```sh
cd fuzz && cargo +nightly fuzz run <target>
```

Run property tests:

```sh
cargo test -p eggfetch-core --all-features
```

## MSRV

The minimum supported Rust version is **1.80**. This is specified in `workspace.package.rust-version` and enforced by `rust-toolchain.toml`.

## Security

eggfetch follows a security-hardening program covering dependencies, TLS, redirects, auth, cookies, proxies, decompression, multipart, retries, protocol handling, Python bindings, and CLI output.

**Dependency auditing:** `cargo-deny` and `cargo-audit` are configured in `deny.toml` and run in CI on every push and pull request. The advisory database is checked continuously.

**Secret redaction:** All `Debug`, `Display`, error, and log output redacts credentials, cookies, bearer tokens, proxy passwords, and URL userinfo. A centralized `redact` module provides `redact_headers()`, `redact_url()`, and `is_sensitive_header()` helpers with regression tests.

**Threat model:** See [docs/architecture/threat-model.md](docs/architecture/threat-model.md) for asset inventory, trust boundaries, attacker capabilities, and security properties.

**Security reviews:** See [docs/architecture/security-reviews.md](docs/architecture/security-reviews.md) for subsystem-level review records (TLS, redirect/auth/cookie, proxy, body handling, retry).

**Vulnerability reporting:** See [SECURITY.md](SECURITY.md) for private reporting instructions, response SLAs, and embargo policy.

## License

eggfetch is dual-licensed under [MIT](LICENSE-MIT) and [Apache License, Version 2.0](LICENSE-APACHE). You may use this project under either license.

## Further Reading

**User documentation:**

- [docs/getting-started/quickstart.md](docs/getting-started/quickstart.md) -- quick start guide.
- [docs/getting-started/installation.md](docs/getting-started/installation.md) -- installation instructions.
- [docs/concepts/](docs/concepts/) -- core concept explanations (architecture, lifecycle, timeouts, streaming, etc.).
- [docs/rust/guide.md](docs/rust/guide.md) -- Rust API guide.
- [docs/python/guide.md](docs/python/guide.md) -- Python API guide.
- [docs/cli/guide.md](docs/cli/guide.md) -- CLI reference.
- [docs/migration/](docs/migration/) -- migration guides from requests and HTTPX.
- [docs/cookbook/](docs/cookbook/) -- practical cookbook examples.
- [docs/reference/compatibility.md](docs/reference/compatibility.md) -- feature compatibility matrix.
- [docs/security/guidelines.md](docs/security/guidelines.md) -- security guidelines.

**Project plans and architecture:**

- [plans/ROADMAP.md](plans/ROADMAP.md) -- full project roadmap, milestone sequence, correctness priorities, and release criteria.
- [docs/architecture/overview.md](docs/architecture/overview.md) -- internal architecture documentation.
- [docs/architecture/dependency-policy.md](docs/architecture/dependency-policy.md) -- dependency management policy.
- [docs/architecture/feature-flags.md](docs/architecture/feature-flags.md) -- feature flag reference.
- [CHANGELOG.md](CHANGELOG.md) -- release history and changes.
- [plans/milestone-y-documentation-examples.md](plans/milestone-y-documentation-examples.md) -- the plan for Milestone Y (documentation and examples).
