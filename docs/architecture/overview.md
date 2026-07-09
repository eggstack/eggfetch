# Architecture Overview

This document describes the architecture of eggfetch. Milestone C is complete: the core crate provides connection pooling, idle timeout, and HTTP/1.1 keep-alive reuse on top of the Milestone B HTTP engine.

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
- **`Response`** -- an HTTP response with status, headers, and a buffered body.
- **`RequestBody`** -- request body type (currently byte buffers; streaming lands in Milestone E).
- **`ResponseBody`** -- response body type (currently buffered bytes).
- **`Headers`** -- typed header map backed by `http::HeaderMap`.
- **`Error`** -- structured error enum covering network, HTTP, timeout, and builder errors.

## Transport Stack

The transport layer is built on:

- **hyper** -- HTTP/1.1 protocol implementation.
- **hyper-util** -- high-level client utilities (connection handling, IO traits).
- **hyper-rustls** -- TLS integration via rustls, providing HTTPS support.
- **tokio** -- async runtime powering I/O and timers.
- **tokio-rustls** -- async TLS streams for tokio + rustls.
- **rustls** -- memory-safe TLS implementation, preferred over native TLS for portability.

## Python Sync Adapter (Milestone F)

The Python sync API will own or borrow a tokio runtime internally. When a user calls `eggfetch.get(...)`, the sync adapter submits the request to the async engine and blocks the calling thread until the response arrives. During this blocking wait, the adapter releases the Python GIL so other Python threads can make progress.

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

Milestone C is complete. The core crate provides a working async HTTP client with HTTPS support, request/response modeling, headers, query parameters, byte bodies, connection pooling (max connections per host, idle timeout), HTTP/1.1 keep-alive reuse, and a structured error type. The CLI and Python crates remain stubs. Streaming, timeouts, and advanced features are planned for subsequent milestones.
