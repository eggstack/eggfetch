# Architecture Overview

This document describes the architecture of eggfetch. Milestone E is complete: the core crate provides protocol-neutral streaming for request and response bodies, building on the timeout system, connection pooling, and HTTP/1.1 engine from earlier milestones.

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

The pool timeout is applied via `tokio::time::timeout` around pool acquisition. The total timeout wraps the entire send+collect lifecycle. Individual connect/write/read phases are not isolable through hyper-util's legacy client API, so the total timeout provides the wall-clock guarantee. This is a documented limitation that will be revisited when the transport layer is abstracted further.

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

Request bodies support three variants: `Empty`, `Bytes` (fixed buffer), and `Stream` (chunked upload). Stream request bodies are buffered into `Full<Bytes>` before sending via `into_hyper_body()`. Full streamed uploads (pipe-through) are deferred to a later milestone.

### Single-Consumption Semantics

Body types are single-consume. Calling `bytes_stream()` on a streaming body replaces it with `Consumed`; a second call returns an error. Calling `bytes()` on a consumed body also returns an error. This prevents accidental double-reads and enforces ownership transfer.

## Transport Stack

The transport layer is built on:

- **hyper** -- HTTP/1.1 protocol implementation.
- **hyper-util** -- high-level client utilities (connection handling, IO traits).
- **hyper-rustls** -- TLS integration via rustls, providing HTTPS support.
- **tokio** -- async runtime powering I/O and timers.
- **tokio-rustls** -- async TLS streams for tokio + rustls.
- **rustls** -- memory-safe TLS implementation, preferred over native TLS for portability.

## Python Sync Adapter (Milestone F)

The Python sync API will own or borrow a tokio runtime internally. When a user calls `eggfetch.get(...)`, the sync adapter submits the request to the async engine and blocks the calling thread until the response arrives. During this blocking wait, the adapter releases the Python GIL so other Python threads can make progress. Response bodies will be buffered (via `bytes().await`) for the sync API, providing a simple `bytes`/`text` property interface.

The sync adapter does not contain its own TCP connections, TLS handshakes, or body parsing. It delegates entirely to eggfetch-core.

## Python Async Adapter (Milestone G)

The Python async API will expose `AsyncClient` with `__aenter__`/`__aexit__` and awaitable request methods. It targets asyncio first. Trio/AnyIO compatibility is a later goal.

The async adapter bridges eggfetch-core's Rust futures to Python coroutines using pyo3-async-runtimes (or an equivalent adapter if justified).

## Future Crates

The project may later add:

- **eggfetch-testing**: local protocol test servers, fixtures, and differential testing against requests/HTTPX.
- **eggfetch-bench**: benchmark harnesses for throughput, latency, and memory.
- **eggfetch-http3**: optional QUIC/HTTP/3 transport experiments, behind a feature flag.

These crates do not exist yet. They will be added when the core engine is stable enough to test and measure.

## Current State

Milestone E is complete. The core crate provides a working async HTTP client with HTTPS support, request/response modeling, headers, query parameters, streaming request/response bodies, connection pooling, phase-aware timeouts (pool, connect, write, read, total), and a structured error type with timeout phase identification. The CLI and Python crates remain stubs. Redirects, advanced features, and Python bindings are planned for subsequent milestones.
