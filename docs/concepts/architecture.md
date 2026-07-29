# Architecture

eggfetch is a six-crate Rust workspace. Understanding the boundaries between crates explains why the library behaves the way it does.

## The Six Crates

### eggfetch-core

The async HTTP engine. Every networking call, TLS handshake, cookie parse, redirect hop, and body decode happens here. There is exactly one implementation of HTTP behavior in the workspace, and it lives in this crate.

If you are reading the source to understand why a request behaves a certain way, start here. The core owns:

- Connection pooling and concurrency limits
- TLS configuration and certificate verification
- Request serialization and response parsing
- Redirect following, retry, decompression, and multipart encoding

### eggfetch-python

A thin adapter that exposes eggfetch-core to Python via PyO3 and maturin. It contains no independent HTTP logic. It converts Python arguments to Rust types, delegates to the core, and wraps the results for Python consumption.

The Python crate enables a broader set of feature flags than the core defaults (cookies, multipart, proxy, compression for all formats, HTTP/2, and HTTP/3) so that the Python API has full functionality out of the box.

### eggfetch-cli

A command-line binary that wraps eggfetch-core. It handles argument parsing, terminal output formatting, and exit code mapping. All network I/O goes through the core crate.

### eggfetch-ffi

C ABI bindings exposing eggfetch-core over a stable C interface. Uses opaque handle patterns and `extern "C"` functions. This crate uses `unsafe_code = "allow"` — the sole exception to the workspace `forbid` policy.

### eggfetch-node

Node.js N-API binding prototype. Wraps eggfetch-core via napi-rs. Also uses `unsafe_code = "allow"`.

### eggfetch-bench

Criterion-based benchmarks. Not published.

## Async-First Design

The Rust engine is async-only. There is no synchronous Rust API. The `Client` type exposes methods that return futures, and callers drive execution with a tokio runtime.

This design keeps the core simple: one code path, one set of state machines, no conditional compilation for sync versus async. Synchronous behavior is an adapter concern, not a core concern.

## How Python Adapters Work

### Sync API

When you call `eggfetch.get(...)`, the sync adapter:

1. Converts Python arguments to owned Rust types
2. Releases the GIL via `py.allow_threads`
3. Blocks on the async Rust engine via `runtime.block_on(future)`
4. Buffers the response body
5. Re-acquires the GIL and returns a `PyResponse`

The GIL is released during all network I/O, so other Python threads can run while a request is in flight.

### Async API

`AsyncClient` targets asyncio. Each request method uses `pyo3-async-runtimes` to bridge Rust futures to Python coroutines. The response is buffered before returning, so the Python side receives a fully-buffered `PyResponse` identical to the sync path.

### Streaming

`client.stream()` returns a `StreamingResponse` context manager. Iterating over the response body advances the stream one chunk at a time, releasing the GIL during each network read.

## Feature Flags

eggfetch-core uses feature flags to keep the default build small. The Python crate enables the full set.

| Feature | Description |
|---------|-------------|
| `http1` | HTTP/1.1 support (default) |
| `http2` | HTTP/2 via ALPN negotiation |
| `http3` | HTTP/3 over QUIC (experimental) |
| `tls-rustls` | TLS via rustls (default) |
| `cookies` | RFC 6265 cookie jar |
| `multipart` | Streaming multipart/form-data |
| `proxy` | HTTP proxy and HTTPS CONNECT tunneling |
| `compression-gzip` | gzip decompression |
| `compression-brotli` | Brotli decompression |
| `compression-zstd` | Zstandard decompression |
| `compression-deflate` | deflate decompression |
| `json` | Reserved for future Rust-native JSON |
| `tracing` | Structured logging (planned) |
| `test-util` | Deterministic time testing (internal) |

See [feature-flags.md](../architecture/feature-flags.md) for the full validation matrix and compilation rules.

## Transport Stack

Under the hood, the transport layer is built on:

- **hyper** -- HTTP/1.1 and HTTP/2 protocol implementation
- **hyper-util** -- high-level client utilities
- **hyper-rustls** -- TLS integration with ALPN negotiation
- **tokio** -- async runtime for I/O and timers
- **rustls** -- memory-safe TLS implementation
- **quinn** / **h3** -- QUIC transport for HTTP/3 (behind the `http3` feature)

## Core Types

The public API of eggfetch-core centers on a few key types:

- **`Client`** -- the async HTTP client, created via `Client::new()` or `ClientBuilder`
- **`ClientBuilder`** -- configures TLS, proxy, auth, timeout, retry, and pool settings before construction
- **`Request`** -- a fully-formed HTTP request (method, URL, headers, body)
- **`RequestBuilder`** -- accumulates headers, body, auth, and other options before sending
- **`Response`** -- an HTTP response with status, headers, streaming body, and redirect history
- **`Error`** -- structured error enum covering network, HTTP, timeout, and builder errors

## Crate Boundary Invariant

eggfetch-core must not depend on PyO3, clap, or CLI argument parsing. The CLI, Python, FFI, and Node crates must not contain independent HTTP behavior. All network I/O goes through eggfetch-core. This invariant is enforced by code review and the workspace dependency structure.

If you find yourself writing HTTP logic outside of eggfetch-core, stop and refactor. There is exactly one networking implementation.
