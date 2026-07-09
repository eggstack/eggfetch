# eggfetch

eggfetch is a Rust-native HTTP client engine with Python bindings and a CLI tool. The core is async-first: a Rust engine built on tokio and hyper provides connection pooling, timeouts, TLS, and streaming. The Python bindings expose both sync and async APIs; the sync API blocks on the async engine while releasing the GIL, and the async API integrates with asyncio. There is exactly one networking implementation, living entirely in Rust.

> **Status: Milestone F complete / Python sync API.**
> The workspace builds, passes lints, and executes real HTTP requests. `eggfetch-core` provides an async `Client`, `Request`/`RequestBuilder`/`Response`, headers, query parameters, streaming request/response bodies, HTTPS via rustls, connection pooling (max connections per host, idle timeout), phase-aware timeout system (pool, connect, write, read, total), and a structured error taxonomy. The Python package `eggfetch` exposes sync helpers (`get`, `post`, etc.), a `Client` with context manager support, buffered responses, case-insensitive headers, and a structured exception hierarchy. CLI crate remains a stub.

## Architecture

eggfetch is a three-crate Cargo workspace:

- **eggfetch-core** owns all HTTP behavior: client, request builder, response, body, headers, timeout, error types, connection pooling, TLS, and streaming. Every networking dependency lives here.
- **eggfetch-cli** is a thin binary that delegates to eggfetch-core for all HTTP work. It handles argument parsing, output formatting, and exit codes only.
- **eggfetch-python** is the Python bindings adapter. It wraps eggfetch-core via PyO3/maturin and exposes sync and async Python APIs. It does not contain its own HTTP logic.

The invariant is strict: all network I/O goes through eggfetch-core. The CLI and Python crates are adapters.

## MVP Non-goals

eggfetch is not aiming for full requests/httpx drop-in parity in the MVP. Specifically, the MVP does not include:

- Full transport parity with HTTPX (ASGI/WSGI in-process transports, all SOCKS proxy modes)
- Trio/AnyIO support (asyncio only)
- HTTP/3 (QUIC transport)
- Every authentication flow, advanced proxy mounting, or requests compatibility edge case

See [plans/ROADMAP.md](plans/ROADMAP.md) for the full roadmap and compatibility policy.

## Target API Shapes

The Python API targets familiar requests/httpx patterns:

```python
import eggfetch as httpx

r = httpx.get("https://example.com", params={"q": "test"}, timeout=5.0)
print(r.status_code)
print(r.headers)
print(r.text)
r.raise_for_status()

with httpx.Client(headers={"User-Agent": "eggfetch"}) as client:
    r = client.post("https://example.com/api", json={"a": 1})

async with httpx.AsyncClient() as client:
    r = await client.get("https://example.com")
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
- **Max connections per host** -- configurable limit via `PoolConfig::max_connections`.
- **Idle timeout** -- connections closed after a configurable idle period via `PoolConfig::idle_timeout`.
- **Pool metrics** -- `PoolMetrics` exposed via `Client::pool_metrics()` and `ClientBuilder::pool_metrics()`.
- **Graceful degradation** -- waiter cancellation support without deadlocks.

### Milestone D: Timeout System (complete)

Phase-aware timeout behavior is implemented with the following capabilities:

- **Timeout configuration** -- `Timeout` type with per-phase durations (pool, connect, write, read, total).
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

### Current limitations (Milestone F)

- Sync only; async Python API deferred to Milestone G.
- No redirects, cookies, auth, files, JSON body, streaming response iteration.
- No `follow_redirects`, `stream`, `proxies`, `verify`, or `cert` kwargs.
- `connect` timeout is accepted but not independently enforced (use `total` as backstop).

## Repository Layout

```text
Cargo.toml               workspace root
README.md
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
  eggfetch-core/         async HTTP engine (Milestone E complete)
  eggfetch-cli/          CLI binary (stub)
  eggfetch-python/       Python bindings (Milestone F complete)
    src/                 Rust adapter modules (PyO3)
    python/eggfetch/     Python package (__init__.py)
    tests/               Python tests
    pyproject.toml       maturin build config
docs/
  architecture/          architecture documentation
plans/
  ROADMAP.md             full milestone roadmap
  milestone-a-repository-foundation.md
  milestone-b-core-http-engine.md
  milestone-c-connection-management.md
  milestone-d-timeout-system.md
  milestone-e-streaming-foundation.md
  milestone-f-python-sync-api.md
  hardening-correctness-before-python.md
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

### Python package

The Python package uses maturin for building. Requires Python 3.9+ and a virtual environment:

```sh
python3 -m venv .venv
source .venv/bin/activate
pip install pytest
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop -m crates/eggfetch-python/Cargo.toml
python -m pytest crates/eggfetch-python/tests/
```

## MSRV

The minimum supported Rust version is **1.80**. This is specified in `workspace.package.rust-version` and enforced by `rust-toolchain.toml`.

## License

eggfetch is dual-licensed under [MIT](LICENSE-MIT) and [Apache License, Version 2.0](LICENSE-APACHE). You may use this project under either license.

## Further Reading

- [plans/ROADMAP.md](plans/ROADMAP.md) -- full project roadmap, milestone sequence, correctness priorities, and release criteria.
- [plans/milestone-a-repository-foundation.md](plans/milestone-a-repository-foundation.md) -- the plan for Milestone A (workspace foundation, linting, CI, documentation).
- [plans/milestone-b-core-http-engine.md](plans/milestone-b-core-http-engine.md) -- the plan for Milestone B (core request/response model and HTTP engine).
- [plans/milestone-c-connection-management.md](plans/milestone-c-connection-management.md) -- the plan for Milestone C (connection pooling and management).
- [plans/milestone-d-timeout-system.md](plans/milestone-d-timeout-system.md) -- the plan for Milestone D (timeout system).
- [plans/milestone-e-streaming-foundation.md](plans/milestone-e-streaming-foundation.md) -- the plan for Milestone E (streaming foundation).
- [plans/milestone-f-python-sync-api.md](plans/milestone-f-python-sync-api.md) -- the plan for Milestone F (Python sync API).
