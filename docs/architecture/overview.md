# Architecture Overview

eggfetch is a Rust-native async HTTP client engine with Python bindings and a CLI tool. The core crate owns all HTTP behavior; every other crate is a thin adapter.

## Design Principles

1. **Single networking implementation** — all HTTP logic lives in `eggfetch-core`. CLI, Python, FFI, and Node never touch the network directly.
2. **Async-first** — the Rust engine is async-only (tokio). Synchronous APIs are adapter-layer concerns that block on the async engine.
3. **Feature-gated modularity** — default is HTTP/1.1 + Rustls TLS. HTTP/2, HTTP/3, cookies, compression, multipart, and proxy are opt-in via Cargo features.
4. **Security by default** — `unsafe_code = "forbid"` workspace-wide (FFI/Node exceptions), credential redaction, CR/LF injection prevention.

## Workspace Layout

```
eggfetch/
├── crates/
│   ├── eggfetch-core/      Async HTTP engine — all networking lives here
│   ├── eggfetch-cli/       CLI binary — argument parsing, output formatting
│   ├── eggfetch-python/    Python bindings via PyO3/maturin
│   ├── eggfetch-ffi/       C ABI bindings — opaque handle pattern
│   ├── eggfetch-node/      Node.js N-API prototype (wraps FFI)
│   └── eggfetch-bench/     Criterion benchmarks (not published)
├── fuzz/                   cargo-fuzz + proptest property tests
├── docs/                   User-facing documentation
│   └── architecture/       This directory — internal architecture docs
├── scripts/                CI helpers, doc checkers, smoke tests
└── plans/                  Milestone plans and roadmap
```

## Crate Dependency Graph

```
eggfetch-core  ←  eggfetch-cli
             ←  eggfetch-python (via PyO3)
             ←  eggfetch-ffi  ←  eggfetch-node (via napi-rs)
```

**Hard rule**: no parallel synchronous networking path. Python sync blocks on async Rust engine.

## Module Map

### eggfetch-core (the engine)

All HTTP behavior lives here. 26 source modules:

| Module | Public? | Purpose |
|--------|---------|---------|
| `client` | Yes | `Client`, `ClientBuilder` — entry point for all requests |
| `request` | Yes | `Request`, `RequestBuilder` — fluent request construction |
| `response` | Yes | `Response`, `HistoryEntry` — response + redirect history |
| `body` | Yes | `RequestBody`, `ResponseBody`, `BoxBytesStream` |
| `headers` | Yes | `Headers` — case-insensitive header map wrapper |
| `error` | Yes | `Error` enum (30+ variants), `Result<T>` alias |
| `pipeline` | No | Full request lifecycle orchestration (retry → redirect → send) |
| `transport` | No | Direct, proxy, HTTP/3 transport dispatch |
| `stream` | No | Per-chunk read/write timeout wrappers |
| `pool` | No | Semaphore-based concurrency pool, origin keying |
| `timeouts` | No | Phase-aware timeout configuration and enforcement |
| `auth` | No | Basic/Bearer auth, secret redaction, precedence resolution |
| `redirect` | No | Redirect policy, method rewriting, header stripping |
| `retry` | No | Retry policy, exponential backoff, body replay check |
| `cookies` | No | RFC 6265 cookie jar, domain/path matching |
| `multipart` | No | Streaming multipart/form-data encoder |
| `compression` | No | Streaming decompression (gzip, brotli, zstd, deflate) |
| `proxy` | No | HTTP proxy, CONNECT tunneling, NO_PROXY |
| `tls` | No | TLS configuration, trust store, mTLS, verification toggle |
| `transport` | No | Direct, proxy, HTTP/3 transport dispatch |
| `redact` | No | Credential redaction for Debug/Display output |
| `config` | No | Shared configuration types |
| `limits` | No | Pool concurrency limits (HTTPX-compatible) |
| `h2_headers` | No | HTTP/2 forbidden header stripping |
| `http_version_policy` | No | HTTP/1.1, HTTP/2, HTTP/3 version negotiation |
| `response_decoding` | No | Content-Encoding parsing and decompression dispatch |

### eggfetch-cli (the CLI)

Single source file (`main.rs`). Thin binary over `eggfetch-core`:

- **Argument parsing**: clap-based, maps flags to `ClientBuilder`/`RequestBuilder` calls
- **Output formatting**: human, headers-only, JSON, NDJSON modes
- **Exit codes**: 7 distinct codes for success, usage, connect, timeout, protocol, status, I/O
- **Streaming**: body streams to stdout incrementally via `bytes_stream()`

### eggfetch-python (the Python bindings)

16 source modules via PyO3/maturin:

| Module | Purpose |
|--------|---------|
| `lib.rs` | Module registration + top-level functions (`get`, `post`, etc.) |
| `client.rs` | `Client` — sync adapter with persistent runtime |
| `async_client.rs` | `AsyncClient` — async adapter targeting asyncio |
| `response.rs` | `PyResponse` — buffered response surface |
| `headers.rs` | `PyHeaders` — header wrapper |
| `auth.rs` | `BasicAuth`, `BearerAuth` |
| `cookies.rs` | Cookie handling |
| `proxy.rs` | Proxy configuration |
| `retry.rs` | Retry configuration |
| `timeout.rs` | Timeout configuration |
| `tls.rs` | TLS configuration (`verify`, `cert` kwargs) |
| `multipart.rs` | `File` wrapper for multipart uploads |
| `streaming.rs` | `StreamingResponse` — sync/async iterators |
| `conversion.rs` | Python↔Rust type conversion (shared by sync/async) |
| `errors.py` | Exception hierarchy |

Plus the HTTPX compatibility facade in `eggfetch/compat/httpx/`.

### eggfetch-ffi (the C ABI)

10 source modules. Sole `unsafe_code = "allow"` crate:

| Module | Purpose |
|--------|---------|
| `handle` | Opaque handle type definitions (`*mut eggfetch_ffi_client`, etc.) |
| `client` | Client creation and configuration |
| `request` | Request building |
| `response` | Response reading |
| `ffi_response` | Response data extraction for FFI |
| `error` | Error inspection |
| `builder` | Builder configuration helpers |
| `runtime` | Global tokio runtime management (`OnceLock<Runtime>`) |
| `streaming` | Streaming body support |
| `lib` | C API entry points, string/memory management |

### eggfetch-node (the Node.js prototype)

3 source modules. Prototype stage:

| Module | Purpose |
|--------|---------|
| `client` | `EggfetchClient` — wraps FFI client |
| `response` | Response wrapper |
| `lib` | N-API module registration |

### eggfetch-bench (benchmarks)

Criterion-based benchmarks in three suites: `microbench`, `e2e`, `resources`. Not published.

## Deep-Dive Index

Each component has a dedicated document for detailed review:

| Component | Document | What It Covers |
|-----------|----------|----------------|
| **Core Engine** | [core-engine.md](core-engine.md) | Client, RequestBuilder, Response, pipeline lifecycle, error taxonomy |
| **Body & Streaming** | [core-body-streaming.md](core-body-streaming.md) | RequestBody, ResponseBody, streaming adapters, pool permit lifecycle |
| **Timeout & Pool** | [core-timeout-pool.md](core-timeout-pool.md) | Phase-aware timeouts, semaphore-based concurrency pool, origin keying |
| **Auth, Redirect & Retry** | [core-auth-redirect-retry.md](core-auth-redirect-retry.md) | Authentication, redirect following, retry with backoff |
| **TLS, Proxy & Protocols** | [core-tls-proxy-protocols.md](core-tls-proxy-protocols.md) | TLS config, HTTP proxy/CONNECT, HTTP/2, HTTP/3 |
| **Cookies, Multipart & Compression** | [core-cookies-multipart-compression.md](core-cookies-multipart-compression.md) | RFC 6265 cookies, multipart/form-data, decompression |
| **CLI** | [cli.md](cli.md) | Argument model, output modes, exit codes |
| **Python Bindings** | [python-bindings.md](python-bindings.md) | Sync/async adapter, PyO3 bridge, Python API surface |
| **FFI & Node** | [ffi-and-node.md](ffi-and-node.md) | C ABI handles, runtime bridge, N-API prototype |
| **Testing & Fuzzing** | [testing-fuzzing.md](testing-fuzzing.md) | Unit/integration tests, property tests, fuzz targets |
| **Build & CI** | [build-ci.md](build-ci.md) | CI pipeline, lint policy, MSRV, release process |

## Cross-Cutting Concerns

| Concern | Document |
|---------|----------|
| Feature flags reference | [feature-flags.md](feature-flags.md) |
| Dependency policy | [dependency-policy.md](dependency-policy.md) |
| Threat model | [threat-model.md](threat-model.md) |
| Security findings | [security-findings.md](security-findings.md) |
| Security reviews | [security-reviews.md](security-reviews.md) |
| Incident runbook | [incident-runbook.md](incident-runbook.md) |
| Release security checklist | [release-security-checklist.md](release-security-checklist.md) |

## Request Lifecycle (Summary)

```
Client::send()
  → retry loop (send_with_retry)
    → redirect loop (send_with_redirects)
      → header merge (client defaults + request overrides)
      → cookie selection (from jar)
      → auth resolution (request > disabled > client > none)
      → pool acquisition (with timeout)
      → write timeout wrapping (stream bodies)
      → Content-Length application
      → HTTP/2 forbidden header stripping
      → transport dispatch (direct / proxy / HTTP3)
      → decompression wrapping
      → read timeout + pool lease attachment
```

## Key External Dependencies

| Crate | Role |
|-------|------|
| `hyper` 1.x + `hyper-util` | HTTP/1.1 and HTTP/2 engine |
| `hyper-rustls` | TLS connector integration |
| `rustls` 0.23 + `tokio-rustls` | Memory-safe TLS |
| `tokio` | Async runtime |
| `http` / `http-body` / `http-body-util` | HTTP types |
| `quinn` + `h3` + `h3-quinn` | QUIC/HTTP/3 (optional) |
| `pyo3` + `maturin` | Python bindings |
| `clap` 4 | CLI argument parsing |
| `napi-rs` | Node.js N-API bindings |

## Current Status

All milestones A–Z are complete. The workspace provides ~880+ Rust tests, ~1170+ Python tests, and ~40+ FFI tests. MSRV is Rust 1.80. CI enforces `RUSTFLAGS=-D warnings` with pedantic clippy.

The `test-util` feature enables `tokio/test-util` for deterministic time testing in timeout-related tests.
