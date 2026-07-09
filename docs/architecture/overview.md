# Architecture Overview

This document describes the architecture of eggfetch. Milestone H is complete: the core crate provides protocol-neutral streaming for request and response bodies, and the Python crate exposes both sync and async APIs over the async Rust core via PyO3/maturin, with a requests/httpx-compatible response surface.

The post-E hardening pass landed true streaming request bodies, per-chunk read/write timeouts, pool permits tied to the full response body lifecycle, and origin-keyed pool limits.

## Three-Crate Workspace

eggfetch is a Cargo workspace with three crates:

- **eggfetch-core** is the async Rust HTTP engine. It owns all networking, connection management, TLS, body handling, and error types. Every dependency that touches the network lives here.
- **eggfetch-cli** is a thin binary that wraps eggfetch-core. It handles argument parsing (eventually via clap), terminal output formatting, exit code mapping, and body/header display. It contains no independent HTTP behavior.
- **eggfetch-python** is the Python bindings adapter. It uses PyO3 and maturin to expose eggfetch-core to Python. It does not duplicate request execution logic.

The reason eggfetch-core owns all HTTP behavior is to maintain a single networking implementation. If HTTP logic were split across crates, behavioral consistency would be impossible to guarantee and the test surface would multiply.

## Async-First Invariant

The Rust engine is async-only. There is no synchronous Rust API. The `Client` type exposes methods that return futures. Callers drive execution with a tokio runtime (or another async executor that hyper supports).

This design keeps the core simple: one code path, one set of state machines, no conditional compilation for sync vs async. Synchronous behavior is an adapter concern, not a core concern.

## Core Types

The following types form the public API of eggfetch-core:

- **`Client`** -- async HTTP client. Created via `Client::new()` or `ClientBuilder`. Owns the connection pool and TLS configuration.
- **`ClientBuilder`** -- builder for configuring a `Client` before construction.
- **`Request`** -- a fully-formed HTTP request (method, URI, headers, body).
- **`RequestBuilder`** -- accumulates method, URL, headers, query parameters, and body before producing a `Request`.
- **`Response`** -- an HTTP response with status, headers, and a streaming body.
- **`RequestBody`** -- request body type: `Empty`, `Bytes`, or `Stream` (a boxed `Stream<Item = Result<Bytes>>`).
- **`ResponseBody`** -- response body type: `Buffered` (collected bytes), `Streaming` (live chunk stream), or `Consumed` (already consumed).
- **`Headers`** -- typed header map backed by `http::HeaderMap`.
- **`Error`** -- structured error enum covering network, HTTP, timeout, and builder errors.

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

## Pool Keying

Per-origin pool limits are keyed by `(scheme, host, port)`, where the port uses the scheme's default when not explicit. `http://example.com:80` and `http://example.com` share a per-origin limit; `http://example.com` and `https://example.com` are independent; `http://example.com:8080` is distinct from `http://example.com`.

## Pool Metrics

`PoolMetrics` exposes only `acquisition_waits` and `acquisition_cancellations`. Socket-level counters (connections_opened/reused/closed) were removed because hyper owns socket lifecycle and eggfetch cannot observe individual socket events through its current integration.

## Transport Stack

The transport layer is built on:

- **hyper** -- HTTP/1.1 protocol implementation.
- **hyper-util** -- high-level client utilities (connection handling, IO traits).
- **hyper-rustls** -- TLS integration via rustls, providing HTTPS support.
- **tokio** -- async runtime powering I/O and timers.
- **tokio-rustls** -- async TLS streams for tokio + rustls.
- **rustls** -- memory-safe TLS implementation, preferred over native TLS for portability.

## Python Sync Adapter (Milestone F)

The Python sync API owns a tokio runtime and an `eggfetch_core::Client` per `PyClient` instance. When a user calls `eggfetch.get(...)`, the sync adapter:

1. Converts Python arguments to owned Rust types (headers, URL, body bytes, timeout).
2. Releases the GIL via `py.allow_threads`.
3. Blocks on the async Rust engine via `runtime.block_on(future)`.
4. Buffers the response body via `response.bytes().await`.
5. Re-acquires the GIL and returns a `PyResponse` with buffered data.

The sync adapter does not contain its own TCP connections, TLS handshakes, or body parsing. It delegates entirely to eggfetch-core.

Top-level helpers (`get`, `post`, etc.) create a short-lived runtime and client per call. The `PyClient` class owns a persistent runtime and client for connection reuse.

Supported kwargs: `headers`, `params`, `content`, `timeout`. Unsupported kwargs raise `TypeError`.

## Python Async Adapter (Milestone G)

The Python async API exposes `AsyncClient` with `__aenter__`/`__aexit__` and awaitable request methods, targeting asyncio.

The async adapter bridges eggfetch-core's Rust futures to Python coroutines using `pyo3-async-runtimes`. Each request method (`get`, `post`, etc.) uses `future_into_py` to convert the Rust future into a Python awaitable. The async block buffers the response body before returning, so the Python side receives a fully-buffered `PyResponse` (same type as the sync API).

Key design decisions:

- **No runtime creation in response path**: `PyResponse::from_parts()` constructs a response from pre-buffered data without creating a tokio runtime, avoiding the `Cannot start a runtime from within a runtime` panic.
- **Pre-resolved futures for context manager**: `__aenter__` and `__aexit__` return pre-resolved `asyncio.Future` objects rather than using `future_into_py`, which would attempt to start a nested runtime.
- **Cancellation safety**: cancelling an in-flight request drops the Rust future cleanly; pool permits are released via RAII `PoolGuard`.

## Response Compatibility Surface (Milestone H)

The Python `PyResponse` type presents a requests/httpx-compatible surface over the buffered response data:

- **Properties**: `status_code`, `reason_phrase`, `headers`, `url`, `content`, `text`, `encoding`, `http_version`, `history` (placeholder).
- **Status helpers**: `is_informational` (1xx), `is_success` (2xx), `is_redirect` (3xx), `is_client_error` (4xx), `is_server_error` (5xx), `is_error` (4xx+5xx).
- **JSON**: `json(**kwargs)` delegates to Python's `json.loads`.
- **Iterators**: `iter_bytes(chunk_size)`, `iter_text(chunk_size)`, `iter_lines()` return Python iterators over buffered content. Async equivalents: `aiter_bytes`, `aiter_text`, `aiter_lines`.
- **Close**: `close()` and `aclose()` are no-ops for buffered responses.
- **Text decoding**: explicit `encoding` kwarg > Content-Type charset > UTF-8 fallback. Uses `encoding_rs` for non-UTF-8 charsets; `.text` uses lossy decode with replacement characters.
- **Headers.get_list()**: returns all values for a multi-value header as a list.
- **Repr**: `<Response [200 OK]>` format.
- **raise_for_status()**: raises `HTTPStatusError` for 4xx/5xx with reason phrase in message.

All response data is buffered at creation time. Streaming iteration returns Python iterators over the cached chunks. True network streaming iteration (consuming chunks as they arrive) is deferred to a later milestone when `client.stream()` is implemented.

## Future Crates

The project may later add:

- **eggfetch-testing**: local protocol test servers, fixtures, and differential testing against requests/HTTPX.
- **eggfetch-bench**: benchmark harnesses for throughput, latency, and memory.
- **eggfetch-http3**: optional QUIC/HTTP/3 transport experiments, behind a feature flag.

These crates do not exist yet. They will be added when the core engine is stable enough to test and measure.

## Current State

Milestone H is complete. The core crate provides a working async HTTP client with HTTPS support, request/response modeling, headers, query parameters, streaming request/response bodies, connection pooling, phase-aware timeouts (pool, connect, write, read, total), and a structured error type with timeout phase identification. The Python crate exposes sync and async APIs with top-level helpers, `Client` and `AsyncClient` classes, requests/httpx-compatible response properties (`status_code`, `reason_phrase`, `headers`, `url`, `content`, `text`, `encoding`, `http_version`, `history`), status helpers (`is_informational`, `is_success`, `is_redirect`, `is_client_error`, `is_server_error`, `is_error`), methods (`json()`, `raise_for_status()`, `iter_bytes()`, `iter_text()`, `iter_lines()`, `close()`/`aclose()`), charset-aware text decoding, multi-value header support, case-insensitive headers, and a structured exception hierarchy. The CLI crate remains a stub. Redirects, advanced features, and true streaming response iteration are planned for subsequent milestones.
