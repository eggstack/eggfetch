# eggfetch Roadmap

## Purpose

eggfetch is a Rust-native HTTP client engine with Python bindings that present familiar `requests` and `httpx` style APIs. The project should be designed as a library first, with a CLI and Python package layered on top of the same core engine.

The central implementation principle is that the Rust engine is asynchronous only. Synchronous Python behavior is an adapter concern: the Python sync API should block on the async Rust engine while releasing the GIL. There must not be a second sync networking implementation.

The MVP should prioritize correctness, clean architecture, and protocol expansion foundations over broad feature parity. HTTP/1.1 over TLS, robust request/response modeling, reliable timeout behavior, safe connection reuse, streaming bodies, and familiar Python ergonomics are the essential early goals.

## Non-goals for the MVP

The MVP is not a full drop-in replacement for HTTPX. It should not initially attempt full parity for every HTTPX transport, ASGI/WSGI in-process transport, every authentication flow, advanced proxy mounting behavior, all AnyIO backends, or every requests compatibility edge case.

The project should instead document compatibility clearly and grow toward parity deliberately.

## Architectural shape

The intended workspace layout is:

```text
eggfetch-core/      async Rust HTTP engine
eggfetch-python/    PyO3/maturin Python package
eggfetch-cli/       command-line interface using eggfetch-core
plans/              roadmap and handoff plans
```

The long-term shape may later add:

```text
eggfetch-testing/   local protocol test servers and fixtures
eggfetch-bench/     benchmark harnesses
eggfetch-http3/     optional QUIC/HTTP/3 transport experiments
```

The dependency strategy should be conservative. Initial core dependencies should be limited to what is required for a correct HTTP/1.1 client engine: `tokio`, `hyper`, `hyper-util`, `http`, `http-body-util`, `bytes`, `url`, `rustls`, `tokio-rustls`, and small support crates where justified. Optional capabilities such as HTTP/2, brotli, zstd, multipart, SOCKS proxies, tracing, CLI support, and Python bindings should be behind features or separate crates.

## Core invariants

All network I/O goes through `eggfetch-core`.

The sync Python API owns or borrows a runtime and blocks on the same async engine. It must release the GIL during blocking operations.

The async Python API should initially target `asyncio` semantics. Trio/AnyIO compatibility is a later compatibility track, not an MVP requirement.

Connection pooling, redirect handling, body streaming, timeouts, TLS verification, DNS behavior, proxy behavior, and response decoding must be implemented once in Rust.

The CLI must use `eggfetch-core` directly and contain no independent HTTP behavior.

## MVP capability target

The MVP should support:

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

The Rust API should remain idiomatic rather than Python-shaped:

```rust
let client = eggfetch_core::Client::new();
let response = client
    .get("https://example.com")
    .header("user-agent", "eggfetch")
    .send()
    .await?;
```

## Milestone sequence

### Milestone A: Repository and workspace foundation

Create the cargo workspace, crate boundaries, baseline metadata, CI, linting, formatting, feature policy, MSRV policy, dependency policy, and contributor-facing docs. This milestone should establish the project skeleton without implementing a large networking stack.

### Milestone B: Core request/response model and minimal HTTP engine

Implement the async Rust core with `Client`, `Request`, `RequestBuilder`, `Response`, `Body`, `Headers`, `Method`, `Uri`, and `Error`. Support GET, POST, custom methods, headers, query parameters, byte bodies, HTTPS, and buffered responses.

### Milestone C: Connection management

Implement the connection pool as a first-class subsystem. Support connection reuse, idle expiration, max idle connections, max total connections, graceful client shutdown, and deterministic pool acquisition behavior.

### Milestone D: Timeout system

Implement phase-aware timeout behavior: connect timeout, pool acquisition timeout, write timeout, read timeout, and optional overall request timeout. Errors must identify the timeout phase precisely.

### Milestone E: Streaming foundation

Implement protocol-neutral streaming abstractions for request and response bodies. Support buffered reads, streaming reads, sync Python iterators later, async Python iterators later, chunk boundaries, cancellation behavior, and clear ownership rules.

### Milestone F: Python sync API

Introduce PyO3/maturin packaging and expose `request`, `get`, `post`, `put`, `patch`, `delete`, `head`, `options`, and `Client`. The sync API must block on the async Rust engine and release the GIL.

### Milestone G: Python async API

Expose `AsyncClient`, awaitable requests, async context managers, and async response streaming targeting `asyncio` first.

### Milestone H: Response compatibility surface (complete)

Add Python-facing `status_code`, `headers`, `content`, `text`, `json()`, `raise_for_status()`, `iter_bytes()`, `iter_text()`, `iter_lines()`, and response history. Implemented: reason_phrase, http_version, encoding, status helpers (is_informational, is_success, is_redirect, is_client_error, is_server_error, is_error), charset-aware text decoding, multi-value header support (get_list), close/aclose, improved repr.

### Milestone I: Request builder compatibility surface (complete)

Add Python-facing `params`, `data`, `json`, `files` foundation, auth hooks foundation, cookies foundation, default headers, and client-level configuration merging. Implemented: `params` (dict/sequence of pairs), `headers` (dict/sequence of pairs), `content` (raw bytes/str), `data` (form-encoded dict/sequence of pairs), `json` (JSON-serialized via Python `json.dumps`), body kwarg mutual exclusion (`content`/`data`/`json`), auto Content-Type for form and JSON bodies, user Content-Type preservation, and `timeout` kwarg.

### Milestone J: Redirect engine (complete)

Implement method rewrite/preservation behavior, loop detection, max redirect enforcement, redirect history, header stripping across origins, and configurable redirect policies. Implemented: `RedirectPolicy` (follow, max_redirects), redirect status detection (301/302/303/307/308), method rewriting (POST→GET on 301/302/303, preserve on 307/308), body header stripping when body dropped, cross-origin sensitive header stripping (`Authorization`, `Cookie`, `Proxy-Authorization`), URL resolution via `url::Url::join`, scheme validation (http/https only), redirect history on `Response`, `follow_redirects`/`max_redirects` Python kwargs, and `TooManyRedirects` exception.

### Milestone K: CLI

Build an `eggfetch` CLI using the same engine. Support methods, headers, JSON body, form body, body file, output body, output headers, timings, redirects, timeout options, HTTP version display, and machine-readable output.

### Milestone L: Correctness and differential testing

Build a local protocol test harness and compare common behaviors against requests and HTTPX where appropriate. Test redirects, timeouts, streaming, TLS, malformed responses, connection reuse, concurrency, cancellation, and decoding.

### Milestone M: Documentation and public MVP preparation

Document architecture, Rust API, Python API, CLI, compatibility limits, timeout semantics, streaming semantics, dependency policy, security policy, and migration examples.

## Correctness priorities

The project should explicitly test and document:

- request method and header preservation
- URI parsing and query serialization
- duplicate headers and case-insensitive lookup
- content-length versus chunked body behavior
- body reuse rules
- redirect method rewriting rules
- cross-origin redirect header handling
- connection reuse versus close behavior
- TLS verification failures
- DNS failures
- connect, read, write, pool, and total timeout errors
- cancellation and drop behavior
- streaming partial reads
- decompression boundaries when added

## Compatibility policy

eggfetch should prefer familiar semantics over pretending to be fully drop-in compatible before that is true. Public docs should state which requests/httpx behaviors are supported, unsupported, or intentionally different.

The Python package may expose an `eggfetch` top-level module first. A later compatibility package or alias can be considered if the project reaches sufficiently high parity.

## Release criteria for MVP

The MVP is ready when:

- the Rust core can issue correct HTTPS requests through a reusable async client
- connection pooling is deterministic and tested
- timeout phases are tested and exposed as precise errors
- response bodies can be buffered or streamed
- the Python sync API supports common requests/httpx-style calls
- the Python async API supports common HTTPX-style calls
- the CLI exercises the same engine
- local correctness tests cover common protocol edge cases
- compatibility limits are documented
- the dependency tree is reviewed and justifiable
