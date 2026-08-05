# Architecture Overview

eggfetch is a Rust-native async HTTP client engine with Python bindings and a CLI tool. The core crate owns all HTTP behavior; every other crate is a thin adapter.

## Table of Contents

- [Design Principles](#design-principles)
- [Workspace Layout](#workspace-layout)
- [Crate Dependency Graph](#crate-dependency-graph)
- [How to Use This Documentation](#how-to-use-this-documentation)
- [Module Map](#module-map)
  - [eggfetch-core (the engine)](#eggfetch-core-the-engine)
  - [eggfetch-cli (the CLI)](#eggfetch-cli-the-cli)
  - [eggfetch-python (the Python bindings)](#eggfetch-python-the-python-bindings)
  - [eggfetch-ffi (the C ABI)](#eggfetch-ffi-the-c-abi)
  - [eggfetch-node (the Node.js prototype)](#eggfetch-node-the-nodejs-prototype)
  - [eggfetch-bench (benchmarks)](#eggfetch-bench-benchmarks)
- [Deep-Dive Index](#deep-dive-index)
- [Cross-Cutting Concerns](#cross-cutting-concerns)
- [Request Lifecycle (Summary)](#request-lifecycle-summary)
- [Key External Dependencies](#key-external-dependencies)
- [Current Status](#current-status)

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

## How to Use This Documentation

This `overview.md` is the entry point. For a focused review of any component, follow the links in the [Deep-Dive Index](#deep-dive-index). Each deep-dive document is self-contained and links back here.

**Review workflow:**

1. Start here for the bird's eye view of all modules and their responsibilities.
2. Pick a component from the Deep-Dive Index to understand implementation details.
3. For cross-cutting concerns (security, features, dependencies), see [Cross-Cutting Concerns](#cross-cutting-concerns).

**Navigation between docs:** Every deep-dive document links back to this overview. Cross-references between related deep-dives are included where relevant (e.g., `core-engine.md` links to `core-body-streaming.md` for body details).

## Module Map

### eggfetch-core (the engine)

All HTTP behavior lives here. 23 source modules (including `stream` and `transport` submodules):

| Module | Public? | Purpose |
|--------|---------|---------|
| `client` | Yes | `Client`, `ClientBuilder` — entry point for all requests |
| `request` | Yes | `Request`, `RequestBuilder` — fluent request construction |
| `response` | Yes | `Response`, `HistoryEntry` — response + redirect history |
| `body` | Yes | `RequestBody`, `ResponseBody`, `BoxBytesStream` |
| `headers` | Yes | `Headers` — case-insensitive header map wrapper |
| `error` | Yes | `Error` enum (46 variants), `Result<T>` alias |
| `auth` | Yes | Basic/Bearer auth, secret redaction, precedence resolution |
| `compression` | Yes | Streaming decompression (gzip, brotli, zstd, deflate) |
| `cookie` | Yes | RFC 6265 cookie jar, domain/path matching (cfg `cookies`) |
| `http_version` | Yes | HTTP/1.1, HTTP/2, HTTP/3 version negotiation |
| `limits` | Yes | Pool concurrency limits (HTTPX-compatible) |
| `multipart` | Yes | Streaming multipart/form-data encoder (cfg `multipart`) |
| `pool` | Yes | Semaphore-based concurrency pool, origin keying |
| `proxy` | Yes | HTTP proxy, CONNECT tunneling, NO_PROXY (cfg `proxy`) |
| `redact` | Yes | Credential redaction for Debug/Display output |
| `redirect` | Yes | Redirect policy, method rewriting, header stripping |
| `retry` | Yes | Retry policy, exponential backoff, body replay check |
| `timeout` | Yes | Phase-aware timeout configuration and enforcement |
| `tls` | Yes | TLS configuration, trust store, mTLS, verification toggle |
| `pipeline` | No | Full request lifecycle orchestration (retry → redirect → send) |
| `transport` | No | Direct, proxy, HTTP/3 transport dispatch |
| `stream` | No | Per-chunk read/write timeout wrappers |
| `h2_headers` | No | HTTP/2 forbidden header stripping |
| `response_decode` | No | Content-Encoding parsing and decompression dispatch |

**Deep dive:** [core-engine.md](core-engine.md) · [core-body-streaming.md](core-body-streaming.md) · [core-timeout-pool.md](core-timeout-pool.md) · [core-auth-redirect-retry.md](core-auth-redirect-retry.md) · [core-tls-proxy-protocols.md](core-tls-proxy-protocols.md) · [core-cookies-multipart-compression.md](core-cookies-multipart-compression.md)

### eggfetch-cli (the CLI)

Single source file (`main.rs`). Thin binary over `eggfetch-core`:

- **Argument parsing**: clap-based, maps flags to `ClientBuilder`/`RequestBuilder` calls
- **Output formatting**: human, headers-only, JSON, NDJSON modes
- **Exit codes**: 7 distinct codes for success, usage, connect, timeout, protocol, status, I/O
- **Streaming**: body streams to stdout incrementally via `bytes_stream()`

**Deep dive:** [cli.md](cli.md)

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
| `errors.rs` | Exception hierarchy |
| `limits.rs` | `PyLimits` — pool concurrency limits |

Plus the HTTPX compatibility facade in `eggfetch/compat/httpx/`.

**Deep dive:** [python-bindings.md](python-bindings.md)

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

**Deep dive:** [ffi-and-node.md](ffi-and-node.md)

### eggfetch-node (the Node.js prototype)

3 source modules. Prototype stage:

| Module | Purpose |
|--------|---------|
| `client` | `EggfetchClient` — wraps FFI client |
| `response` | Response wrapper |
| `lib` | N-API module registration |

**Deep dive:** [ffi-and-node.md](ffi-and-node.md)

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

All milestones A–Z are complete. The workspace provides ~685 Rust tests, ~513 Python tests (non-compat), ~1280 Python tests (compat), and 30 FFI tests. MSRV is Rust 1.80. CI enforces `RUSTFLAGS=-D warnings` with pedantic clippy.

The `test-util` feature enables `tokio/test-util` for deterministic time testing in timeout-related tests.
