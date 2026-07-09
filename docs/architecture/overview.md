# Architecture Overview

This document describes the intended architecture of eggfetch. At the time of writing (Milestone A), the crates are stubs. This is a forward-looking design doc; the actual networking engine lands in later milestones.

## Three-Crate Workspace

eggfetch is a Cargo workspace with three crates:

- **eggfetch-core** is the async Rust HTTP engine. It owns all networking, connection management, TLS, timeouts, body streaming, redirect handling, cookie handling, proxy handling, and error types. Every dependency that touches the network lives here.
- **eggfetch-cli** is a thin binary that wraps eggfetch-core. It handles argument parsing (eventually via clap), terminal output formatting, exit code mapping, and body/header display. It contains no independent HTTP behavior.
- **eggfetch-python** is the Python bindings adapter. It uses PyO3 and maturin to expose eggfetch-core to Python. It does not duplicate request execution logic.

The reason eggfetch-core owns all HTTP behavior is to maintain a single networking implementation. If HTTP logic were split across crates, behavioral consistency would be impossible to guarantee and the test surface would multiply.

## Async-First Invariant

The Rust engine is async-only. There is no synchronous Rust API. The `Client` type exposes methods that return futures. Callers drive execution with a tokio runtime (or another async executor that hyper supports).

This design keeps the core simple: one code path, one set of state machines, no conditional compilation for sync vs async. Synchronous behavior is an adapter concern, not a core concern.

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

The workspace is a skeleton. The core crate contains placeholder types (`Client`, `Request`, `RequestBuilder`, `Response`, `Body`, `Headers`, `Method`, `Error`, `Config`, `Timeout`) that compile but do nothing. The CLI prints a version string. The Python crate re-exports eggfetch-core to exercise the dependency boundary.

No code makes network calls. No HTTP requests are executed. The type shapes reflect the intended API, not current behavior.
