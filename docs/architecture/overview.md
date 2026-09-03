# Architecture Overview

eggfetch is a Rust-native async HTTP client engine with Python bindings, a CLI tool, C ABI bindings, and a Node.js prototype. The core crate owns all HTTP behavior; every other crate is a thin adapter that delegates to it.

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
├── Cargo.toml          # Workspace root (resolver v2, 6 member crates)
├── crates/
│   ├── eggfetch-core/      Async HTTP engine — all networking lives here
│   ├── eggfetch-cli/       CLI binary — argument parsing, output formatting
│   ├── eggfetch-python/    Python bindings via PyO3/maturin
│   ├── eggfetch-ffi/       C ABI bindings — opaque handle pattern
│   ├── eggfetch-node/      Node.js N-API prototype (wraps FFI)
│   └── eggfetch-bench/     Criterion benchmarks (not published)
├── compat/                 HTTPX compatibility profiles (httpx/0.28.1) + downstream
│                            fixtures, shim, and controlled-replacement suites
├── docs/
│   └── architecture/       This directory — internal architecture docs
├── examples/               Usage examples
├── fuzz/                   cargo-fuzz + proptest property tests
├── plans/                  Milestone plans and roadmap
└── scripts/                CI helpers, doc checkers, smoke tests
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

All HTTP behavior lives here. 26 source modules including `stream` and `transport` submodules. This is the single authority for networking — no other crate performs I/O.

| Module | Public? | Purpose |
|--------|---------|---------|
| `client` | Yes | `Client`, `ClientBuilder` — entry point for all requests. Holds hyper clients (standard, direct, UDS, SOCKS, H3), pool, config. Builder pattern with comprehensive configuration. |
| `request` | Yes | `Request`, `RequestBuilder`, `ProxyOverride`, `TransportHints` — fluent request construction. Methods: `header()`, `query()`, `body()`, `timeout()`, `auth()`, `decompress()`, `proxy()`, `retry()`. `TransportHints` carries wire-level overrides (`target`, `sni_hostname`, `trace`) that do not affect logical URL semantics; hints survive retry reconstruction and are cleared on redirect hops. `send()` delegates to client. |
| `response` | Yes | `Response`, `HistoryEntry` — incoming response with status, version, headers, URL, body, redirect history. Consumption: `bytes()`, `text()`, `bytes_stream()`, `raw_bytes_stream()`, `text_lines()`. |
| `body` | Yes | `RequestBody`, `ResponseBody`, `BoxBytesStream` — single-consumption body model. Request: `Empty | Bytes | Stream`. Response: `Buffered | Streaming | EncodedStreaming | Consumed`. Streaming bodies carry pool permits via `PoolGuardArc` (RAII). |
| `headers` | Yes | `Headers` — case-insensitive header map wrapper around `http::HeaderMap`. |
| `error` | Yes | `Error` enum (47 variants), `Result<T>` alias. Comprehensive error taxonomy with `kind()` method returning static strings for programmatic matching. |
| `auth` | Yes | `AuthScheme`, `BasicAuth`, `BearerAuth` — CR/LF injection prevention, redacted `Debug`/`Display`. Precedence: request > disabled > client > none. |
| `compression` | Yes | `ContentCoding`, `DecompressionLimit` — streaming decompression (gzip, brotli, zstd, deflate). Zip-bomb protection via max decoded size and ratio. |
| `cookie` | Yes | `CookieJar`, `Cookie`, `SameSite` — RFC 6265 cookie jar with domain/path matching, cross-origin stripping, thread-safe storage. (cfg `cookies`) |
| `http_version` | Yes | `HttpVersionPolicy` — HTTP/1.1, HTTP/2, HTTP/3 version negotiation. |
| `limits` | Yes | `Limits` — resource limits (max connections, idle connections, per-host limits). HTTPX-compatible. |
| `multipart` | Yes | `Multipart`, `Boundary`, `Part`, `PartBody`, `MultipartEncoder` — streaming multipart/form-data with known-length optimization. (cfg `multipart`) |
| `network_stream` | Yes | `NetworkStream`, `UpgradedStream`, `UpgradedStreamVariant` (`Tcp`/`Tls`/`Adapter`), `ConnectionMetadata`, `TlsInfo`, `ExtraInfo` — writable IO for 101 Switching Protocols responses only; gates `start_tls` eligibility and carries connection metadata surfaced to adapters. |
| `pool` | Yes | `Pool`, `PoolConfig`, `PoolGuard`, `OriginKey`, `PoolMetrics` — semaphore-based concurrency limiter. Global + per-origin limits. Origin keyed by `(scheme, host, port)` + optional proxy route. |
| `proxy` | Yes | `Proxy`, `ProxyConfig`, `ProxyAuth`, `NoProxy`, `NoProxyRule`, `ProxyDecision` — HTTP forwarding, HTTPS CONNECT tunneling, SOCKS5. Per-request override model. NO_PROXY with 12 rule types. (cfg `proxy`) |
| `redact` | Yes | `redact_headers()`, `redact_url()`, `SENSITIVE_HEADERS` — centralized secret redaction for all `Debug`/`Display`/error output. |
| `redirect` | Yes | `RedirectPolicy`, `redirect_method()`, `build_redirect_request()` — redirect following: method rewrites (303→GET), cross-origin header stripping, body replayability checks. |
| `retry` | Yes | `RetryPolicy`, `RetryPolicyBuilder`, `BackoffPolicy`, `MethodPolicy`, `StatusPolicy`, `RetryCause` — policy-driven retry with exponential backoff+jitter. Respects `Retry-After` headers. POST/PATCH not retried by default. |
| `timeout` | Yes | `Timeout`, `TimeoutBuilder`, `TimeoutPhase` — 7 distinct phases (Pool, Connect, ProxyConnect, ProxyTls, Write, Read, Total). Request-level overrides merge with client-level per-field. |
| `tls` | Yes | `TlsConfig`, `TlsConfigBuilder`, `TlsVersion`, `TrustStore`, `ClientIdentity` — custom CA bundles, mTLS client certs, verification toggle, TLS version bounds. |
| `trace` | Yes | `TraceObserver`, `TraceEvent` — synchronous callback observer for dispatch events (request/response/redirect/retry). Coroutine callbacks are rejected at adapter boundaries; core only ever sees sync observers. |
| `pipeline` | No | `send_with_retry()`, `send_with_redirects()`, `send_single_request()` — full request lifecycle orchestration. Retry loop → redirect loop → header merge → cookie injection → auth → pool acquire → timeout → transport → decompression. |
| `transport` | No | 9 submodules: `direct`, `direct_connector`, `proxy`, `socks`, `uds`, `http3`, `connect`, `connect_timeout`, `mod`. Type aliases for hyper clients with timeout wrappers. |
| `stream` | No | Per-chunk read/write timeout wrappers. |
| `h2_headers` | No | HTTP/2 forbidden header stripping. |
| `response_decode` | No | Content-Encoding parsing and decompression dispatch. |

**Transport layer** (`transport/`):

| Submodule | Purpose |
|-----------|---------|
| `direct` | Direct TCP connections |
| `direct_connector` | Direct TCP with socket options and local address binding |
| `proxy` | HTTP proxy forwarding and HTTPS CONNECT tunneling |
| `socks` | SOCKS5 proxy connector with persistent per-route hyper pools |
| `uds` | Unix domain socket connections |
| `http3` | HTTP/3 over QUIC via Quinn |
| `connect` | TLS connect logic for proxy tunnels |
| `connect_timeout` | Connect-phase timeout wrapper for any connector |

**Deep dive:** [core-engine.md](core-engine.md) · [core-body-streaming.md](core-body-streaming.md) · [core-timeout-pool.md](core-timeout-pool.md) · [core-auth-redirect-retry.md](core-auth-redirect-retry.md) · [core-tls-proxy-protocols.md](core-tls-proxy-protocols.md) · [core-cookies-multipart-compression.md](core-cookies-multipart-compression.md)

### eggfetch-cli (the CLI)

Single source file (`main.rs`, ~1700 lines). Thin binary over `eggfetch-core`:

- **Argument parsing**: clap-based (`#[derive(Parser)]`), maps flags to `ClientBuilder`/`RequestBuilder` calls
- **Body modes**: `--body`, `--body-file`, `--json`, `--form`, `--file @path`
- **Output formatting**: human, headers-only, JSON, NDJSON modes; streaming to stdout or file (`-o`)
- **Download mode**: filename derivation from URL/headers (`--download`)
- **Binary encoding**: `--base64` for binary bodies
- **Exit codes**: 8 codes (0=success, 2=usage, 3=connect/TLS, 4=timeout, 5=protocol, 6=status, 7=I/O, 130=interrupted)
- **Streaming**: body streams to stdout incrementally via `bytes_stream()`
- **Shell completions**: `--generate-completion` for bash/zsh/fish/powershell

**Deep dive:** [cli.md](cli.md)

### eggfetch-python (the Python bindings)

19 source modules via PyO3/maturin. Enables all core features including HTTP/3.

| Module | Purpose |
|--------|---------|
| `lib.rs` | Module registration + top-level functions (`get`, `post`, `put`, `patch`, `delete`, `head`, `options`, `request`) |
| `client.rs` | `Client` — sync adapter with persistent runtime. Blocks on async with GIL released. |
| `async_client.rs` | `AsyncClient` — async adapter targeting asyncio event loop. |
| `response.rs` | `PyResponse` — buffered response surface. |
| `headers.rs` | `PyHeaders` — header wrapper. |
| `auth.rs` | `BasicAuth`, `BearerAuth`, `NoAuth`. |
| `cookies.rs` | Cookie handling. |
| `proxy.rs` | Proxy configuration. |
| `retry.rs` | Retry configuration. |
| `timeout.rs` | Timeout configuration. |
| `tls.rs` | TLS configuration (`verify`, `cert` kwargs). |
| `multipart.rs` | `File` wrapper for multipart uploads. |
| `streaming.rs` | `StreamingResponse` — sync/async iterators for bytes, text, lines, raw bytes. |
| `conversion.rs` | Python↔Rust type conversion (shared by sync/async). |
| `extensions.rs` | Request-extension extraction (`target`, `sni_hostname`, `trace`) into core `TransportHints`. |
| `network_stream.rs` | `PyNetworkStream` (sync) / `PyAsyncNetworkStream` (async) wrappers behind `EitherNetworkStream`; `start_tls`, `get_extra_info`. |
| `trace_bridge.rs` | `PyTraceObserver` — wraps sync Python callables as core `TraceObserver`; rejects coroutine callbacks eagerly. |
| `errors.rs` | Exception hierarchy: `EggfetchError` → `RequestError` (nesting `InvalidUrl`, `TimeoutException`, `NetworkError`, `ProtocolError`, `BodyError`, `ProxyError`, retry/H2/H3 errors) plus direct subclasses `HTTPStatusError`, `UnsupportedKwarg`, stream-state errors. See [python-bindings.md](python-bindings.md). |
| `limits.rs` | `PyLimits` — pool concurrency limits. |

Plus the HTTPX compatibility facade in `eggfetch/compat/httpx/`.

**Deep dive:** [python-bindings.md](python-bindings.md)

### eggfetch-ffi (the C ABI)

10 source modules. Sole `unsafe_code = "allow"` crate (required for FFI):

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

Thread safety: `ClientHandle` is `Send + Sync` (shared). `RequestHandle`, `ResponseHandle`, `StreamingResponseHandle`, `ErrorHandle` are single-thread, single-use.

**Deep dive:** [ffi-and-node.md](ffi-and-node.md)

### eggfetch-node (the Node.js prototype)

3 source modules. Prototype stage, wraps `eggfetch-ffi`:

| Module | Purpose |
|--------|---------|
| `client` | `EggfetchClient` — wraps FFI client |
| `response` | Response wrapper |
| `lib` | N-API module registration |

**Deep dive:** [ffi-and-node.md](ffi-and-node.md)

### eggfetch-bench (benchmarks)

Criterion-based benchmarks in `benchmarks/` — three suites (`microbench`, `e2e`, `resources`) plus a `resource_monitor.rs` binary for RSS monitoring. Not published.

**Deep dive:** [benchmarks.md](benchmarks.md)

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
| **Benchmarks** | [benchmarks.md](benchmarks.md) | Criterion suites, BenchServer harness, RSS regression monitor |
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
      → transport dispatch (UDS / direct / direct-with-socket-options / proxy / HTTP3)
      → decompression wrapping
      → read timeout + pool lease attachment
```

### Transport Dispatch Order

The pipeline tries transports in this order within `send_single_request()`:

1. **Unix Domain Socket** — if the URL scheme is `http+unix` or `https+unix`
2. **Direct TCP** — default path, with optional socket options and local address binding
3. **HTTP Proxy** — HTTP forwarding or HTTPS CONNECT tunneling (if proxy configured)
4. **SOCKS5 Proxy** — SOCKS5 connector with per-route persistent pools (if proxy configured)
5. **HTTP/3 (QUIC)** — if `http3` feature enabled and URL scheme is `https`

### Pool Permit Lifecycle

- Streaming response bodies hold a `PoolGuard` (via `Arc`) until consumed or dropped
- Buffered responses (`bytes()`, `text()`) release the permit immediately after reading
- Pool permits are keyed by origin: `(scheme, host, port)` + optional proxy route

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
| `thiserror` | Error derive macros |
| `dashmap` | Concurrent hash map (pool origins) |
| `async-compression` | Streaming decompression |
| `cookie` | RFC 6265 cookie parsing |

## Current Status

All milestones A–Z are complete. Test counts change with every commit; the
evidence bound to the qualified executable SHA is recorded in
`plans/httpx-parity-correction-status.md`. MSRV is Rust 1.80. CI enforces `RUSTFLAGS=-D warnings` with pedantic clippy.

The `test-util` feature enables `tokio/test-util` for deterministic time testing in timeout-related tests.
