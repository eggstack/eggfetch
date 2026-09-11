# Architecture Overview

eggfetch is a Rust-native async HTTP client engine with Python bindings, a CLI tool, C ABI bindings, and a Node.js prototype. The core crate owns all HTTP behavior; every other crate is a thin adapter that delegates to it.

This document is the bird's-eye view: what each discrete module, tool, and capability does, how they fit together, and where to go for a focused review. Each section ends with a link to a dedicated deep-dive document in this directory.

## Table of Contents

- [Design Principles](#design-principles)
- [Workspace Layout](#workspace-layout)
- [Crate Dependency Graph](#crate-dependency-graph)
- [How to Use This Documentation](#how-to-use-this-documentation)
- [Modules](#modules)
  - [eggfetch-core (the engine)](#eggfetch-core-the-engine)
  - [eggfetch-cli (the CLI)](#eggfetch-cli-the-cli)
  - [eggfetch-python + compat facades](#eggfetch-python--compat-facades-the-python-bindings)
  - [eggfetch-ffi (the C ABI)](#eggfetch-ffi-the-c-abi)
  - [eggfetch-node (the Node.js prototype)](#eggfetch-node-the-nodejs-prototype)
  - [eggfetch-bench (benchmarks)](#eggfetch-bench-benchmarks)
- [Tools](#tools)
  - [Validation entry point (`scripts/check.sh`)](#validation-entry-point-scriptschecksh)
  - [Compatibility tooling (`compat/` + `scripts/`)](#compatibility-tooling-compat--scripts)
  - [Fuzzing (`fuzz/`)](#fuzzing-fuzz)
  - [HTTP/3 qualification (`qualification/http3/` + `scripts/h3_*.py`)](#http3-qualification-qualificationhttp3--scriptsh3_py)
  - [Examples (`examples/`)](#examples-examples)
  - [Skill workflows (`.skills/`)](#skill-workflows-skills)
- [Capabilities](#capabilities)
  - [Protocols (H1/H2/H3)](#protocols-h1h2h3)
  - [Pooling, timeouts, observability](#pooling-timeouts-observability)
  - [TLS, proxy, SOCKS, UDS](#tls-proxy-socks-uds)
  - [Auth, redirect, retry](#auth-redirect-retry)
  - [Cookies, multipart, compression](#cookies-multipart-compression)
  - [Streaming, trailers, upgrades (101/SSE/WS)](#streaming-trailers-upgrades-101ssews)
  - [HTTPX compatibility facades](#httpx-compatibility-facades)
  - [Security posture](#security-posture)
- [Deep-Dive Index](#deep-dive-index)
- [Cross-Cutting Concerns](#cross-cutting-concerns)
- [Request Lifecycle (Summary)](#request-lifecycle-summary)
- [Key External Dependencies](#key-external-dependencies)
- [Current Status](#current-status)

## Design Principles

1. **Single networking implementation** — all HTTP logic lives in `eggfetch-core`. CLI, Python, FFI, and Node never touch the network directly.
2. **Async-first** — the Rust engine is async-only (tokio). Synchronous APIs are adapter-layer concerns that block on the async engine (Python sync releases the GIL; Node prototype uses `spawn_blocking` over FFI).
3. **Feature-gated modularity** — default is HTTP/1.1 + Rustls TLS. HTTP/2, HTTP/3, cookies, compression, multipart, and proxy are opt-in via Cargo features.
4. **Security by default** — `unsafe_code = "forbid"` workspace-wide (only `eggfetch-ffi` and `eggfetch-node` override to `"allow"` for FFI/N-API), credential redaction, CR/LF injection prevention, fail-closed TLS translation.
5. **Typed reconstruction, no silent drops** — request rebuilds for retry/redirect go through exhaustive helpers (`RequestParts::retry_request`, `into_request`, `advance_redirect_hop`); a new field must fail to compile, never be silently dropped.

## Workspace Layout

```
eggfetch/
├── Cargo.toml          # Workspace root (resolver v2, 6 member crates)
├── crates/
│   ├── eggfetch-core/      Async HTTP engine — all networking lives here
│   ├── eggfetch-cli/       CLI binary — argument parsing, output formatting
│   ├── eggfetch-python/    Python bindings via PyO3/maturin (+ compat facades)
│   ├── eggfetch-ffi/       C ABI bindings — opaque handle pattern
│   ├── eggfetch-node/      Node.js N-API prototype (wraps FFI)
│   └── eggfetch-bench/     Criterion benchmarks (not published)
├── compat/                 Versioned compatibility profiles (httpx/0.28.1,
│                            httpx2/2.12.0, httpx/1.0-preview) + downstream
│                            fixtures, shim, and controlled-replacement suites
├── docs/
│   ├── architecture/       This directory — internal architecture docs
│   ├── concepts/           User-facing concept guides (lifecycle, TLS, proxy…)
│   ├── python|rust|cli|   Per-surface user guides
│   ├── reference/          Compatibility/feature/error matrices
│   └── verification-policy.md  Normative CI/verification/release policy
├── examples/               Rust consumer example
├── fuzz/                   cargo-fuzz (12 targets) + corpus + regressions
├── plans/                  Milestone plans and roadmap (historical record)
├── qualification/http3/    H3 machine-readable corpus + runners (opt-in)
└── scripts/                check.sh tiers, manifest/compare, H3 runners,
                            doc checkers, wheel/package validators
```

## Crate Dependency Graph

```
eggfetch-core  ←  eggfetch-cli
              ←  eggfetch-python (via PyO3)
              ←  eggfetch-ffi  ←  eggfetch-node (via napi-rs)
eggfetch-bench → eggfetch-core (dev-only harness, not published)
```

**Hard rules** (see `AGENTS.md`): `eggfetch-core` has no PyO3/clap/CLI parsing; `eggfetch-cli`/`eggfetch-python` have no direct hyper/tokio TCP — all I/O through core. No parallel sync networking path; Python sync blocks on the async engine with GIL released.

## How to Use This Documentation

This `overview.md` is the entry point. For a focused review of any component, follow the links in the [Deep-Dive Index](#deep-dive-index). Each deep-dive document is self-contained and links back here.

**Review workflow:**

1. Start here for the bird's eye view of all modules, tools, and capabilities.
2. Pick a component from the Deep-Dive Index to understand implementation details.
3. For cross-cutting concerns (security, features, dependencies), see [Cross-Cutting Concerns](#cross-cutting-concerns).

**Navigation between docs:** Every deep-dive document links back to this overview. Cross-references between related deep-dives are included where relevant (e.g., `core-engine.md` links to `core-body-streaming.md` for body details).

## Modules

### eggfetch-core (the engine)

All HTTP behavior lives here (~29.7k lines across 27 source files plus `transport/` and `stream/` trees). This is the single authority for networking — no other crate performs I/O.

| Module | Public? | Purpose |
|--------|---------|---------|
| `client` | Yes | `Client`, `ClientBuilder` — entry point for all requests. Holds hyper clients (standard, direct, UDS, SOCKS, H3), pool, config. Builder pattern with comprehensive configuration. |
| `request` | Yes | `Request`, `RequestBuilder`, `ProxyOverride`, `TransportHints` — fluent request construction (`header()`, `query()`, `body()`, `timeout()`, `auth()`, `decompress()`, `proxy()`, `retry()`). `TransportHints` carries wire-level overrides (`target`, `sni_hostname`, `trace`) that do not affect logical URL semantics; hints survive retry reconstruction and are cleared on redirect hops. `send()` delegates to client. |
| `response` | Yes | `Response`, `HistoryEntry` — status, version, headers, URL, body, redirect history, trailers (`trailers()` after EOF). Consumption: `bytes()`, `text()`, `bytes_stream()`, `raw_bytes_stream()`, `text_lines()`. |
| `body` | Yes | `RequestBody`, `ResponseBody`, `BoxBytesStream`, `SharedTrailers` — single-consumption body model. Request: `Empty \| Bytes \| Stream`. Response: `Buffered \| Streaming \| EncodedStreaming \| Consumed`. Streaming bodies carry pool permits via `PoolGuardArc` (RAII); trailers populate without buffering via shared store. |
| `headers` | Yes | `Headers` — case-insensitive header map wrapper around `http::HeaderMap`. |
| `error` | Yes | `Error` enum (47 variants), `Result<T>` alias. Comprehensive taxonomy (`InvalidUrl` … `Http2*`, `H3*`, `TraceCallbackAborted`) with `kind()` returning static strings for programmatic matching. |
| `auth` | Yes | `AuthScheme`, `BasicAuth`, `BearerAuth` — CR/LF injection prevention, redacted `Debug`/`Display`. Precedence: request > disabled > client > none. |
| `compression` | Yes | `ContentCoding`, `DecompressionLimit` — streaming decompression (gzip, brotli, zstd, deflate). Zip-bomb protection via max decoded size and ratio. |
| `cookie` | Yes | `CookieJar`, `Cookie`, `SameSite` — RFC 6265 jar with domain/path matching, cross-origin stripping, thread-safe storage. (cfg `cookies`) |
| `http_version` | Yes | `HttpVersionPolicy` — HTTP/1.1, HTTP/2, HTTP/3 negotiation (`Auto` / `Http2Only` / `Http3Only`). |
| `limits` | Yes | `Limits` — logical in-flight limits (`max_in_flight_requests*`, aliases `max_connections*`) + physical idle caps. |
| `multipart` | Yes | `Multipart`, `Boundary`, `Part`, `PartBody`, `MultipartEncoder` — streaming multipart/form-data with known-length optimization. (cfg `multipart`) |
| `network_stream` | Yes | `NetworkStream`, `UpgradedStream`, `UpgradedStreamVariant` (`Tcp`/`Tls`/`Adapter`), `ConnectionMetadata`, `TlsInfo`, `ExtraInfo` — writable IO for 101 only; direct upgrades carry real addrs/TLS, UDS reports `Unix` without IPs, opaque stays unavailable. |
| `pool` | Yes | `Pool`, `PoolConfig`, `PoolGuard`, `OriginKey`, `PoolMetrics` — semaphore-based concurrency limiter keyed by `(scheme, host, port)` + optional proxy route. |
| `transport/metrics` | Yes | `TransportMetrics`, `TransportSnapshot` — connector/DNS/TLS, UDS/proxy, H3 creation/eviction, Alt-Svc learned/expired/cleared/rejected, H3 attempted/suppressed/fallback/drain/close/reconnect, 101 upgrades. Separate from `PoolMetrics`; Hyper reuse absent by design. |
| `proxy` | Yes | `Proxy`, `ProxyConfig`, `ProxyAuth`, `NoProxy`, `NoProxyRule`, `ProxyDecision` — HTTP forwarding, HTTPS CONNECT tunneling, SOCKS5. Per-request override model. (cfg `proxy`) |
| `redact` | Yes | `redact_headers()`, `redact_url()`, `SENSITIVE_HEADERS` — centralized secret redaction for all `Debug`/`Display`/error output. |
| `redirect` | Yes | `RedirectPolicy`, `redirect_method()`, `build_redirect_request()` — method rewrites (303→GET), cross-origin header stripping, body replayability checks. |
| `retry` | Yes | `RetryPolicy`, `RetryPolicyBuilder`, `BackoffPolicy`, `MethodPolicy`, `StatusPolicy`, `RetryCause` — exponential backoff+jitter, `Retry-After` support. POST/PATCH not retried by default. |
| `timeout` | Yes | `Timeout`, `TimeoutBuilder`, `TimeoutPhase` — 7 phases (Pool, Connect, ProxyConnect, ProxyTls, Write, Read, Total). Request-level overrides merge with client-level per-field. |
| `tls` | Yes | `TlsConfig`, `TlsConfigBuilder`, `TlsVersion`, `TrustStore`, `ClientIdentity` — custom CA bundles, mTLS certs, verification toggle, version bounds. |
| `trace` | Yes | `TraceObserver`, `TraceEvent` — synchronous lifecycle callbacks; coroutine callbacks rejected at adapters. |
| `pipeline` | No | `send_with_retry()`, `send_with_redirects()`, `send_single_request()` — lifecycle orchestration: retry loop (typed `RequestParts::retry_request`) → redirect loop → preparation (`PreparedRequest`) → declarative dispatch (`TransportRoute::select_route`) → common post-transport policy. |
| `transport` | Mixed | 11 files: `mod` (hyper client type aliases), `direct`, `direct_connector` (socket options + local bind), `proxy`, `socks` (per-route persistent pools), `uds`, `http3` (QUIC/draining, explicit `H3DispatchError`), `alt_svc` (authenticated cache + suppressor), `connect`, `connect_timeout`, `metrics`. `direct` owns the shared Hyper response lifecycle (`finish_hyper_response`, `wrap_incoming`, `await_upgrade`). |
| `stream` | No | Per-chunk read/write timeout wrappers (`read_timeout`, `write_timeout`). |
| `h2_headers` | No | HTTP/2 forbidden-header stripping. |
| `response_decode` | No | Content-Encoding parsing and decompression dispatch. |
| `config` | No | Dead placeholder (`Config { _private }`); pool/timeout/redirect config live in their own modules. Not exported from `lib.rs`. |

**Deep dive:** [core-engine.md](core-engine.md) · [core-body-streaming.md](core-body-streaming.md) · [core-timeout-pool.md](core-timeout-pool.md) · [core-auth-redirect-retry.md](core-auth-redirect-retry.md) · [core-tls-proxy-protocols.md](core-tls-proxy-protocols.md) · [core-cookies-multipart-compression.md](core-cookies-multipart-compression.md)

### eggfetch-cli (the CLI)

Single source file (`main.rs`, ~1.8k lines). Thin binary over `eggfetch-core` (enables `cookies`, `multipart`, `proxy`):

- **Argument parsing**: clap-based (`#[derive(Parser)]`), maps flags to `ClientBuilder`/`RequestBuilder` calls
- **Body modes**: `--body`, `--body-file`, `--json`, `--form`, `--file @path`
- **Output formatting**: human, headers-only, JSON, NDJSON modes; streaming to stdout or file (`-o`)
- **Download mode**: filename derivation from URL/headers (`--download`)
- **Binary encoding**: `--base64` for binary bodies
- **Exit codes**: 8 codes (0=success, 2=usage, 3=connect/TLS, 4=timeout, 5=protocol, 6=status, 7=I/O, 130=interrupted)
- **Streaming**: body streams to stdout incrementally via `bytes_stream()`
- **Shell completions**: `--generate-completion` for bash/zsh/fish/powershell

**Deep dive:** [cli.md](cli.md)

### eggfetch-python + compat facades (the Python bindings)

19 Rust source modules via PyO3/maturin (~9.2k lines). Enables all core features including HTTP/2, HTTP/3, cookies, multipart, proxy, all compressions.

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
| `network_stream.rs` | `PyNetworkStream` (sync) / `PyAsyncNetworkStream` (async) behind `EitherNetworkStream`; `start_tls`, `get_extra_info`. Clones share one stream (Arc/Mutex); leading rewind bytes honored. |
| `trace_bridge.rs` | `PyTraceObserver` — wraps sync Python callables as core `TraceObserver`; rejects coroutines eagerly with `TypeError`. |
| `errors.rs` | Exception hierarchy: `EggfetchError` → `RequestError` (nesting `InvalidUrl`, `TimeoutException`, `NetworkError`, `ProtocolError`, `BodyError`, `ProxyError`, retry/H2/H3 errors) plus `HTTPStatusError`, `UnsupportedKwarg`, stream-state errors. |
| `limits.rs` | `PyLimits` — pool concurrency limits. |

Python package surface (`python/eggfetch/__init__.py` + `compat/`): native `Client`/`AsyncClient`, `Response`, `Headers`, `Cookies`, `Timeout`, `Limits`, auth types, streaming iterators, top-level verbs, plus versioned facades `eggfetch.compat.httpx` (0.28.1, 21 modules: `_client`, `_response`, `_request`, `_urls`, `_headers`, `_cookies`, `_auth`, `_timeout`, `_proxy`, `_transports`, `_stream`, `_exceptions`, `_status_codes`, `_mock`, `_asgi`, `_wsgi`, `_ssl_context`, `_diagnostics`, `_limits`) and `eggfetch.compat.httpx2` (2.12.0: `_api`, `_config`, `_sse`, …). SSE is Python framing over streamed responses; WebSocket uses wsproto over the core 101 `network_stream` — never raw sockets from Python. See [Capabilities](#httpx-compatibility-facades).

**Deep dive:** [python-bindings.md](python-bindings.md)

### eggfetch-ffi (the C ABI)

10 source modules (~2.3k lines). Sole `unsafe_code = "allow"` crate alongside Node (required for FFI):

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

Thread safety: `ClientHandle` is `Send + Sync` (shared). `RequestHandle`, `ResponseHandle`, `StreamingResponseHandle`, `ErrorHandle` are single-thread, single-use. Default features: `http1`, `tls-rustls`, `cookies`, `proxy`, `compression-gzip`; `http2`/`http3`/`multipart`/more codecs opt-in.

**Deep dive:** [ffi-and-node.md](ffi-and-node.md)

### eggfetch-node (the Node.js prototype)

3 source modules (~450 lines). Explicitly experimental prototype that wraps the blocking `eggfetch-ffi` surface inside `spawn_blocking` (string-only bodies, buffered responses, unstructured errors, stub declarations):

| Module | Purpose |
|--------|---------|
| `client` | `EggfetchClient` — wraps FFI client (raw-pointer handle) |
| `response` | Buffered response wrapper |
| `lib` | N-API module registration |

**Deep dive:** [ffi-and-node.md](ffi-and-node.md)

### eggfetch-bench (benchmarks)

Criterion-based harnesses (not published): shared `BenchServer` blocking test server in `src/lib.rs` plus three suites and one binary in `benchmarks/` — `microbench` (core-internal), `e2e` (full-client), `resources` (resource-oriented), and `resource_monitor` (RSS regression monitor binary). Core built with `http1`, `http2`, `tls-rustls`, `json`, `proxy` + cookies/multipart/all compressions.

**Deep dive:** [benchmarks.md](benchmarks.md)

## Tools

Tools are the scripts, suites, and workflows that validate, qualify, and document the modules above. They contain no product networking logic.

### Validation entry point (`scripts/check.sh`)

Single source of validation truth (CI repeats it on ubuntu-latest; see `docs/verification-policy.md`):

- **Tier 1** (`./scripts/check.sh`, required before every commit): `cargo fmt --check`, `check_lint_suppressions.sh`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1` (single-threaded: RSS tests are concurrency-sensitive), `maturin develop`, `pytest crates/eggfetch-python/tests/ -q --ignore=…/compat`. Refuses to run outside an active venv with Python 3.10+, maturin, pytest, pytest-asyncio.
- **Tier 2** (`extended`, before release): full compat (`EGGFETCH_COMPAT_REQUIRED=1 pytest …/compat/ --strict-markers`), API oracle, feature matrix, MSRV, docs, FFI, soak.
- **Tier 3** (`package`, before publish): crate packaging + wheel build/smoke.
- Helpers in the same directory: `check_doc_examples.py`, `check_doc_links.py`, `check_lint_suppressions.sh` (rejects `allow(warnings)`, `clippy::all/pedantic/nursery/restriction`; specific lints need justifying comments), `validate_*` (package content, internal deps, release versions, wheel coverage), `wheel_smoke.py`, `stage_c_categories.py`.

**Deep dive:** [build-ci.md](build-ci.md)

### Compatibility tooling (`compat/` + `scripts/`)

- `compat/httpx/0.28.1/` and `compat/httpx2/2.12.0/`: pinned profiles (`profile.toml`), machine-readable API manifests (`reference-api.json`), allowed/resolved difference ledgers, parity cases, upstream test inventory, resource/performance budgets. Never hand-edit generated manifests — regenerate via `scripts/generate_httpx_api_manifest.py` + `scripts/compare_httpx_api_manifest.py`.
- `compat/httpx/1.0-preview/`: reconnaissance only (36-symbol manifest, delta notes, `preview-status.toml`, `redesign-notes.md`). No compatibility promise.
- `compat/downstream/`, `compat/httpx-shim/`, `compat/httpx-controlled-replacement/`: downstream behavioral fixtures, import shim, and pinned-wheel replacement suite with runners `scripts/run_downstream_compat.py`, `scripts/run_isolated_downstream.py`.

**Deep dive:** [python-bindings.md](python-bindings.md) (facade semantics) · [testing-fuzzing.md](testing-fuzzing.md) (compat suites)

### Fuzzing (`fuzz/`)

12 `cargo-fuzz` targets (`fuzz_alt_svc`, `fuzz_compression`, `fuzz_cookie`, `fuzz_headers`, `fuzz_multipart`, `fuzz_proxy`, `fuzz_proxy_response`, `fuzz_redirect`, `fuzz_retry`, `fuzz_timeout`, `fuzz_tls`, `fuzz_url`) plus `corpus/` seeds and `regressions/`, complemented by proptest property tests in core. Workflow and triage live in the fuzz-testing skill.

**Deep dive:** [testing-fuzzing.md](testing-fuzzing.md)

### HTTP/3 qualification (`qualification/http3/` + `scripts/h3_*.py`)

Machine-readable corpus (`corpus.json`), server manifests (`servers.example.json`), impairment matrix (`impairment-matrix.json`), and runners (`scripts/h3_qualification.py`, `scripts/h3_impairment.py`, `scripts/test_h3_qualification.py`). Deterministic loopback controls run in the normal Rust suite; independent-server, impairment, and public-origin qualification are opt-in and never routine CI gates. Local control-only run: `python3 scripts/h3_qualification.py --local-only --output /tmp/eggfetch-h3-local.json`.

**Deep dive:** [core-tls-proxy-protocols.md](core-tls-proxy-protocols.md) (§ "Production Graduation Decision")

### Examples (`examples/`)

`rust_consumer.rs` — minimal external-consumer smoke example for the core crate API.

### Skill workflows (`.skills/`)

Task workflows that encode repo policy: `rust-development.md`, `python-bindings.md`, `cli-development.md`, `ffi-development.md`, `fuzz-testing.md`, `security-review.md`, `release-process.md`, `documentation.md`. Architecture entry point for agents is this overview.

## Capabilities

Capabilities are the user-visible behaviors. Each is implemented once in `eggfetch-core` and surfaced through the adapters above.

### Protocols (H1/H2/H3)

HTTP/1.1 (default), HTTP/2 via ALPN (plus h2c prior knowledge and `Http2Only` enforced at ALPN + `http2_only(true)` + `Connected::negotiated_h2()` layers), and experimental HTTP/3 over QUIC (pinned h3 0.0.8, bounded per-origin cache, shared connect budget with address fallback, phase-correct timeouts, keepalive-derived idle, authenticated Alt-Svc discovery with suppression/safe fallback/draining). CONNECT-proxy origin framing stays H1.1; H2 `stream_id` is metadata-only. H3 graduation gate/blockers: [core-tls-proxy-protocols.md](core-tls-proxy-protocols.md) § "Production Graduation Decision".

**Deep dive:** [core-tls-proxy-protocols.md](core-tls-proxy-protocols.md) · [feature-flags.md](feature-flags.md)

### Pooling, timeouts, observability

Semaphore-based logical in-flight concurrency (`max_in_flight_requests*`, `max_connections*` aliases) with per-origin limits and `PoolMetrics` (waits/cancellations), separate from `TransportMetrics` (connector/DNS/TLS attempts, H3 create/evict, Alt-Svc learned/expired/cleared/rejected, H3 attempted/suppressed/fallback/drain/close/reconnect, 101 counts; Quinn RTT/path/route snapshots on H3 builds). Phase-aware timeouts: pool, connect, proxy-connect, proxy-TLS, write, read, total — with cancellation safety and per-request merges. Never synthesize native `total` from HTTPX facade timeouts.

**Deep dive:** [core-timeout-pool.md](core-timeout-pool.md)

### TLS, proxy, SOCKS, UDS

rustls with custom CA bundles, mTLS client certs, version policy, verification toggle; `ssl.SSLContext` translation is fail-closed (construction fingerprint; subclasses/unrepresentable state → `TypeError`; proxy TLS comes only from `Proxy(ssl_context=…)`). HTTP forwarding, HTTPS CONNECT tunneling, proxy auth, per-request override, `NO_PROXY` bypass (compat env parser vs richer native `NoProxy::parse()` — intentionally not unified). SOCKS5 via per-route persistent Hyper pools. UDS route for local sockets. Transport dispatch order `prepare_single_request()` + `select_route()`: UDS → direct → proxy/SOCKS → SNI → H3 → standard, sharing one post-transport policy.

**Deep dive:** [core-tls-proxy-protocols.md](core-tls-proxy-protocols.md)

### Auth, redirect, retry

Basic/Bearer with redaction and CR/LF rejection; precedence request > disabled > client > none; URL-embedded credentials rejected. Redirects: 303→GET rewrites, cross-origin stripping, buffered-body replay (one-shot streams rejected before next hop). Retry: policy-driven exponential backoff+jitter, `Retry-After`, per-method/status policies (POST/PATCH off by default), replay checks.

**Deep dive:** [core-auth-redirect-retry.md](core-auth-redirect-retry.md)

### Cookies, multipart, compression

RFC 6265 jar (domain/path matching, cross-origin stripping, thread-safe), streaming multipart/form-data with known-length optimization and part-header validation, and feature-gated streaming decompression (gzip/brotli/zstd/deflate) with zip-bomb limits (max size + ratio) and chained-decoder caps.

**Deep dive:** [core-cookies-multipart-compression.md](core-cookies-multipart-compression.md)

### Streaming, trailers, upgrades (101/SSE/WS)

Request/response bodies stream without eager buffering (`bytes_stream()`, `text_lines()`, raw-vs-decoded boundary). H1 chunked trailers, H2 trailing HEADERS, H3 trailing headers captured without buffering (`Response::trailers()` after EOF; H1 duplicate same-name collapse documented upstream). Only 101 responses own a writable `network_stream` (`response.extensions["network_stream"]`); pooled responses are `None`, CONNECT tunnels never surfaced (use body iterator). `start_tls` allows only inner-`Tcp` variants. Sync `Client.stream()` yields `PyNetworkStream`, async yields `PyAsyncNetworkStream`; clones share one stream. SSE is Python framing over streamed responses; WebSocket uses wsproto over the core 101 stream.

**Deep dive:** [core-body-streaming.md](core-body-streaming.md) · [core-engine.md](core-engine.md) · [python-bindings.md](python-bindings.md)

### HTTPX compatibility facades

Two versioned, independent facades over the single engine that coexist without cross-mutation: `eggfetch.compat.httpx` (0.28.1) and `eggfetch.compat.httpx2` (2.12.0), each Stage C qualified on frozen SHA `78a77ea15…` (`compat/*/profile.toml`); any executable change invalidates qualification. `compat/httpx/1.0-preview/` is reconnaissance only. Key mappings: timeouts map only `connect`/`read`/`write`/`pool` (preserve omitted-vs-`None`; reject bare `Timeout()`); proxy setup uses one monotonic deadline; `Proxy(headers=…)` rides the proxy leg only; sync trace callbacks ok, coroutines rejected. HTTPX2 adds `FunctionAuth`, `Origin`/`URL.origin`, `QUERY`, `Headers` merge operators, `alias_httpx()`, truststore OS-trust default, RFC 9110 renames, SSE (`EventSource`) + optional WS (`pip install httpx2[ws]`). Residuals are documented, not papered over (`docs/residual-differences.md`). See `README.md` § "HTTPX Compatibility" for the full delta list.

**Deep dive:** [python-bindings.md](python-bindings.md)

### Security posture

`cargo-deny` (`deny.toml`), secret redaction in all `Debug`/`Display`/`__repr__`/errors via `eggfetch_core::redact` (`authorization`, `proxy-authorization`, `cookie`, `set-cookie`), threat model, security reviews/findings, incident runbook, release checklist. See [Cross-Cutting Concerns](#cross-cutting-concerns).

## Deep-Dive Index

Each component has a dedicated document for detailed review:

| Component | Document | What It Covers |
|-----------|----------|----------------|
| **Core Engine** | [core-engine.md](core-engine.md) | Client, RequestBuilder, Response, pipeline lifecycle, error taxonomy |
| **Body & Streaming** | [core-body-streaming.md](core-body-streaming.md) | RequestBody, ResponseBody, streaming adapters, pool permit lifecycle |
| **Timeout & Pool** | [core-timeout-pool.md](core-timeout-pool.md) | Phase-aware timeouts, semaphore-based concurrency pool, origin keying |
| **Auth, Redirect & Retry** | [core-auth-redirect-retry.md](core-auth-redirect-retry.md) | Authentication, redirect following, retry with backoff |
| **TLS, Proxy & Protocols** | [core-tls-proxy-protocols.md](core-tls-proxy-protocols.md) | TLS config, HTTP proxy/CONNECT, SOCKS, UDS, HTTP/2, HTTP/3 + graduation gate |
| **Cookies, Multipart & Compression** | [core-cookies-multipart-compression.md](core-cookies-multipart-compression.md) | RFC 6265 cookies, multipart/form-data, decompression |
| **CLI** | [cli.md](cli.md) | Argument model, output modes, exit codes |
| **Python Bindings** | [python-bindings.md](python-bindings.md) | Sync/async adapter, PyO3 bridge, native + HTTPX/HTTPX2 facades, SSE/WS, upgrades |
| **FFI & Node** | [ffi-and-node.md](ffi-and-node.md) | C ABI handles, runtime bridge, N-API prototype |
| **Testing & Fuzzing** | [testing-fuzzing.md](testing-fuzzing.md) | Unit/integration tests, property tests, fuzz targets, compat suites |
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

Normative verification tiers and complexity budget: `../verification-policy.md`. User-facing concept guides: `../concepts/`; compatibility/feature/error matrices: `../reference/`; known intentional deltas: `../residual-differences.md`.

## Request Lifecycle (Summary)

```
Client::send()
  → retry loop (send_with_retry via RequestParts::retry_request, total deadline shrinks)
    → redirect loop (send_with_redirects via shared HopBuildParams builder)
      → header merge (client defaults + request overrides)
      → hop build (cookies, auth, hints; first hop preserves hints, later hops clear)
      → redirect transformation (single advance_redirect_hop step)
      → preparation (prepare_single_request → PreparedRequest)
        → accept-encoding / Content-Length / user-agent / H2 stripping / size check
        → proxy resolution + origin keying + pool acquisition
        → write-timeout wrapping + remaining-total/deadline computation
      → transport dispatch (select_route → UDS / Direct / Proxy / SNI / H3 / Standard)
      → common post-transport policy (Alt-Svc learning, decompression, decoded-size limit)
      → read timeout + pool lease attachment
```

### Transport Dispatch Order

`send_single_request()` separates preparation from execution. After
`prepare_single_request()` builds a `PreparedRequest`, `select_route()`
selects one declarative route (precedence unchanged, directly unit-tested):

1. **Unix Domain Socket** — when `ClientBuilder::uds_path()` is configured
2. **Specialized direct** — when a direct connector (socket options / local
   address) is configured and no proxy applies
3. **Proxy / SOCKS** — effective proxy or SOCKS path (SOCKS uses a
   per-route persistent Hyper pool)
4. **SNI override direct** — cached SNI-specific client when
   `TransportHints::sni_hostname` is set
5. **HTTP/3 (QUIC, experimental)** — `Http3Only` always; `Auto { allow_http3: true }` only
   when a fresh Alt-Svc alternative is cached and not suppressed (otherwise
   standard); `Auto { allow_http3: false }` never. Requires the `http3`
   feature. Graduation is **retained experimental** this milestone; blockers
   and evidence live in [core-tls-proxy-protocols.md](core-tls-proxy-protocols.md)
   (§ "Production Graduation Decision") and `tests/h3_interop_qualification.rs`.
6. **Standard Hyper direct** — default TCP path (also safe `Auto` fallback
   for pre-commit replayable H3 failures, same deadlines/TLS).

H3 never bypasses proxy rules because proxy routes are selected first.
All routes share one post-transport policy; UDS and specialized-direct
responses now flow through the same Alt-Svc learning (learnable routes
only), decompression, and lease handling as the standard path.

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
| `quinn` + `h3` + `h3-quinn` | QUIC/HTTP/3 (optional, pinned h3 0.0.8) |
| `pyo3` + `maturin` | Python bindings |
| `clap` 4 | CLI argument parsing |
| `napi-rs` | Node.js N-API bindings |
| `thiserror` | Error derive macros |
| `dashmap` | Concurrent hash map (pool origins, cookie jar) |
| `async-compression` | Streaming decompression |
| `cookie` | RFC 6265 cookie parsing |

Dependency policy (audits, bans, licenses): [dependency-policy.md](dependency-policy.md) + `deny.toml`.

## Current Status

All milestones A–Z are complete. Test counts change with every commit; the
evidence bound to the qualified executable SHA is recorded in
`plans/httpx-parity-correction-status.md`. MSRV is Rust 1.80 (`workspace.package.rust-version`; `rust-toolchain.toml` pins stable for development). CI enforces `RUSTFLAGS=-D warnings` with pedantic clippy. Python support is 3.10–3.13 (asyncio only; Trio/AnyIO out of scope).

The `test-util` feature enables `tokio/test-util` for deterministic time testing in timeout-related tests.
