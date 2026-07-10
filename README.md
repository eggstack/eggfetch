# eggfetch

eggfetch is a Rust-native HTTP client engine with Python bindings and a CLI tool. The core is async-first: a Rust engine built on tokio and hyper provides connection pooling, timeouts, TLS, and streaming. The Python bindings expose both sync and async APIs; the sync API blocks on the async engine while releasing the GIL, and the async API integrates with asyncio. There is exactly one networking implementation, living entirely in Rust.

> **Status: Milestone J complete / Redirect engine.**
> The workspace builds, passes lints, and executes real HTTP requests. `eggfetch-core` provides an async `Client`, `Request`/`RequestBuilder`/`Response`, headers, query parameters, streaming request/response bodies, HTTPS via rustls, connection pooling (max connections per host, idle timeout), phase-aware timeout system (pool, connect, write, read, total), redirect following with configurable policy, and a structured error taxonomy. The Python package `eggfetch` exposes sync helpers (`get`, `post`, etc.), a `Client` with context manager support, an `AsyncClient` with `async with` and `await` support, buffered responses with requests/httpx-compatible properties (`status_code`, `reason_phrase`, `headers`, `url`, `content`, `text`, `encoding`, `http_version`, `history`), status helpers (`is_informational`, `is_success`, `is_redirect`, `is_client_error`, `is_server_error`, `is_error`), methods (`json()`, `raise_for_status()`, `iter_bytes()`, `iter_text()`, `iter_lines()`, `close()`/`aclose()`), charset-aware text decoding via `encoding_rs`, multi-value header support (`Headers.get_list()`), case-insensitive headers, request body kwargs (`content`, `data`, `json`), form encoding, JSON body serialization, body kwarg mutual exclusion, `follow_redirects`/`max_redirects` kwargs, and a structured exception hierarchy. CLI crate remains a stub.

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
- **Streaming iterators** -- `iter_bytes(chunk_size)`, `iter_text(chunk_size)`, `iter_lines()` return Python iterators over buffered content. These are not true network streaming; the full body is buffered before iteration. True network streaming (consuming chunks as they arrive) is planned for a later milestone.
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

### Current limitations (Milestone J)

- No cookies, auth, multipart files.
- No `stream`, `proxies`, `verify`, or `cert` kwargs.
- `connect` timeout is accepted but not independently enforced (use `total` as backstop).
- Streaming iterators return one-shot iterators over buffered content (no true network streaming iteration yet).
- Trio/AnyIO support deferred to a later milestone.

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
  eggfetch-core/         async HTTP engine (Milestone J complete)
  eggfetch-cli/          CLI binary (stub)
  eggfetch-python/       Python bindings (Milestone J complete)
    src/                 Rust adapter modules (PyO3)
    python/eggfetch/     Python package (__init__.py)
    tests/               Python tests
    pyproject.toml       maturin build config
docs/
  architecture/          architecture documentation
  plans/
  ROADMAP.md                    full milestone roadmap
  post-milestone-j-tightening.md  post-J corrective pass plan
  milestone-a-repository-foundation.md
  milestone-b-core-http-engine.md
  milestone-c-connection-management.md
  milestone-d-timeout-system.md
  milestone-e-streaming-foundation.md
  milestone-f-python-sync-api.md
  milestone-g-python-async-api.md
  milestone-h-response-compatibility.md
  milestone-i-request-builder-compatibility.md
  milestone-j-redirect-engine.md
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
- [plans/milestone-g-python-async-api.md](plans/milestone-g-python-async-api.md) -- the plan for Milestone G (Python async API).
- [plans/milestone-h-response-compatibility.md](plans/milestone-h-response-compatibility.md) -- the plan for Milestone H (response compatibility surface).
- [plans/milestone-i-request-builder-compatibility.md](plans/milestone-i-request-builder-compatibility.md) -- the plan for Milestone I (request builder compatibility).
- [plans/milestone-j-redirect-engine.md](plans/milestone-j-redirect-engine.md) -- the plan for Milestone J (redirect engine).
- [plans/post-milestone-j-tightening.md](plans/post-milestone-j-tightening.md) -- post-J corrective pass: redirect body buffering fix, async response construction fix, documentation truth pass.
