# eggfetch Revised Roadmap

> **Note:** Completed plan files in this directory are historical records, not active CI, verification, or release requirements. The normative verification and release policy is in `docs/verification-policy.md` and `docs/releases/process.md`.

## Purpose

eggfetch is a Rust-native HTTP client platform with Python bindings and a CLI layered on top of a single async engine. The project has completed its initial architectural and MVP-construction phase: the Rust core, Python sync and async APIs, response compatibility surface, common request body handling, redirects, streaming foundations, timeouts, pooling, HTTP/2, and HTTP/3 (experimental) are all in place.

The remaining roadmap is therefore not primarily about proving feasibility. It is about tightening semantics, completing the expected HTTP-client feature set, expanding transport capabilities, and establishing production-grade release, security, testing, and documentation practices.

## Current closure work (2026-09-03)

Two ordered closure passes are active after the recent post-qualification hardening work:

1. `httpx-parity-corrective-08-post-hardening-requalification-and-closure.md` — audit the executable changes since the 2026-08-24 HTTPX 0.28.1 qualification, freeze one final executable/test SHA, rerun the complete exact-SHA qualification evidence, and renew the bounded Stage C claim only if every gate passes.
2. `documentation-broad-truth-refresh-after-requalification.md` — after the renewed qualification is recorded, perform a repository-wide documentation truth pass as a documentation/ledger-only descendant so migration, compatibility, architecture, contributor, verification, release, and guide material all describe the current implementation consistently.

Execution order is strict: **Corrective 08 first, documentation refresh second**. Any executable/test/build/validation/packaging change discovered during the documentation pass reopens Corrective 08 and requires a new frozen SHA.

## Architectural invariants

The following remain non-negotiable:

- All network I/O lives in `eggfetch-core`.
- The Rust engine remains async-first.
- Python sync behavior blocks on the async engine while releasing the GIL.
- Python async behavior targets `asyncio` first.
- The CLI contains no independent HTTP implementation.
- Rust APIs remain idiomatic rather than mirroring Python conventions.
- Optional capabilities remain feature-gated or isolated in higher-level crates where practical.
- Compatibility claims must match tested behavior.

## Completed foundation

The following work is considered complete enough to serve as the platform for the revised roadmap:

- repository/workspace foundation
- async Rust HTTP/1.1 engine over hyper/tokio/rustls
- request/response/header/body abstractions
- connection pooling and per-origin permit control
- phase-aware timeout model with implemented pool/total/read semantics where supported
- streamed Rust request and response bodies
- sync Python API
- async Python API
- Python response compatibility surface
- Python request kwargs for params, headers, content, form data, and JSON
- redirect engine with method rewriting, sensitive-header stripping, history, and limits
- cookie subsystem with RFC 6265 parsing, domain/path matching, cookie jar, and client-level cookie state
- authentication subsystem with Basic and Bearer token support, client and request-level auth, precedence resolution, cross-origin credential stripping, and URL credential conversion
- proxy subsystem with HTTP proxying, HTTPS CONNECT tunneling, proxy authentication, per-request/client proxy configuration, and NO_PROXY bypass
- TLS configuration with custom CA bundles, client certificates, TLS version policy, verification toggle, and SNI behavior
- HTTP/2 support with ALPN negotiation, multiplexed connections, protocol version reporting, error taxonomy, forbidden header stripping, retry classification, and pool concurrency model
- HTTP/3 support (experimental) via QUIC transport with Quinn/h3, feature-gated behind `http3`

These capabilities should continue receiving corrective maintenance, but the roadmap no longer treats them as greenfield milestones.

# Phase 1: Semantic Tightening and Public-API Stabilization

## Milestone N: Post-Milestone J semantic tightening

Make every already-exposed API behave exactly as documented before adding broad new features.

Primary work:

- true Python network streaming rather than buffered iterators
- redirect/body replay correctness for streamed uploads
- logical total timeout across redirect chains
- lifecycle-safe, memory-bounded redirect history
- deterministic response-decoding policy
- sync/async Python API parity audit
- clean wheel builds across supported Python versions/platforms
- visible CI checks
- repository-history audit for the accidentally committed virtual environment
- compatibility documentation truth pass

Exit criterion:

> Existing public behavior is stable, lifecycle-safe, tested, and accurately documented.

The post-Milestone-N validation and polish pass is complete before multipart
work begins. It covers Python streaming cancellation and close behavior,
cookie/auth redirect security, deliberate TLS root fallback, feature-matrix
builds, Windows CI, clean wheel smoke tests, public export/repr review, and a
documentation truth pass.

# Phase 2: HTTP Client Completeness

## Milestone O: Cookie subsystem

Implement a first-class cookie model in the Rust core.

Capabilities:

- RFC 6265-style cookie parsing and serialization
- `Cookie`, `CookieJar`, and policy abstractions
- `Set-Cookie` ingestion
- request `Cookie` header generation
- domain/path/secure/expiry matching
- redirect and cross-origin behavior
- Python `client.cookies` and `response.cookies`

The initial implementation should be HTTP-client oriented rather than browser-grade. Public-suffix enforcement and SameSite browser policy may be added later.

## Milestone P: Authentication subsystem

Implement reusable authentication abstractions in the core.

Initial capabilities:

- Basic authentication
- Bearer-token authentication
- auth application at client and request level
- safe redirect behavior and credential stripping
- Python tuple/basic auth compatibility
- structured auth interface for future Digest, API-key, OAuth, and pluggable schemes

Auth must be designed as request transformation/state machinery, not hard-coded special cases in Python.

## Milestone Q: Multipart and file uploads (complete)

Streaming multipart/form-data request bodies in eggfetch-core and Python `files=` compatibility.

Implemented capabilities:

- core multipart model (`Multipart`, `Part`, `PartBody`, `Boundary`)
- text fields, byte parts, and streaming file parts
- boundary generation (random) and custom boundary validation
- per-part headers and content types
- streaming encoder (state-machine, backpressure, no eager buffering)
- known-length calculation (checked arithmetic, only when all parts known)
- Python `files=` kwarg (bytes, tuples, paths via `eggfetch.File`)
- Python `data=` + `files=` combination (multipart fields + files)
- `files=` + `content=`/`json=` conflict rejection
- cancellation-safe streaming (drops file handles and streams)
- feature-gated (`multipart` feature in eggfetch-core)

## Milestone R: Content compression

Implement configurable response decompression.

Initial formats:

- gzip
- deflate
- brotli
- zstd

Requirements:

- feature-gated dependencies
- correct `Content-Encoding` handling
- decompression-bomb limits/policy
- streaming decompression
- raw/decoded body distinction where exposed

Outgoing compression can remain a later optional extension.

# Phase 3: Networking and Transport Features

## Milestone S: Proxy subsystem

Implement:

- HTTP proxying
- HTTPS CONNECT tunneling
- proxy authentication
- per-request/client proxy configuration
- `NO_PROXY`-style bypass matching
- later SOCKS5 support behind a feature

Proxy logic belongs in the core connector/transport layer.

## Milestone T: TLS configuration (complete)

Expose deliberate TLS configuration without weakening secure defaults.

Capabilities:

- verification enabled by default
- custom CA bundles
- verification disable escape hatch with explicit warnings/docs
- client certificates
- TLS version policy
- SNI behavior
- Python `verify=` and `cert=` compatibility

This milestone requires targeted security review.

## Milestone U: Retry and resilience policy (complete)

Policy-driven retries in `eggfetch-core`.

Implemented capabilities:

- `RetryPolicy` and builder with exponential backoff and jitter
- maximum attempts and retry budget
- retryable transport errors (connect, I/O, hyper)
- optional retryable status codes (408, 429, 502, 503, 504)
- idempotency-aware method rules (GET, HEAD, OPTIONS safe by default)
- request-body replayability checks
- bounded exponential backoff with jitter
- `Retry-After` header parsing support
- total-timeout and cancellation integration
- per-request and per-client retry policy override
- opt-in retries (disabled by default)
- Python `Retry` class with `retries=` kwarg on Client and request methods
- comprehensive unit tests for policy, backoff, methods, statuses, body replayability

# Phase 4: Modern HTTP Protocols

## Milestone V: HTTP/2

Enable and validate HTTP/2 support.

Work includes:

- ALPN negotiation
- multiplexing semantics
- flow-control tests
- pool/concurrency model adjustments
- protocol-version reporting
- HTTP/2-specific error mapping
- load and cancellation testing

The pool abstraction must no longer equate one active request with one TCP connection.

## Milestone W: HTTP/3 (complete)

HTTP/3 over QUIC is implemented as an experimental, separately feature-gated capability.

Implemented capabilities:

- `HttpVersionPolicy::Http3Only` enum variant for HTTP/3-only connections
- QUIC transport via `quinn` crate with `h3` protocol layer
- Feature-gated behind `http3` Cargo feature
- TLS 1.3 requirement enforced (QUIC mandate)
- 0-RTT disabled in initial implementation (replay attack risk mitigation)
- Python `Client(http3=True)` and `AsyncClient(http3=True)`
- Python `Http3Error` and `Http3ConnectionError` exception types
- Protocol version reporting (`HTTP/3`)
- Backward compatible; no existing behavior changes unless explicitly opted in
- Rust tests covering feature-gated compilation, version policy, and error taxonomy

# Phase 5: CLI and Ecosystem

## Milestone X: Full CLI

Expand `eggfetch-cli` into a practical HTTP command-line client using only `eggfetch-core`.

Capabilities:

- all common HTTP methods
- headers and params
- JSON, forms, raw body, and files
- redirects
- cookies and auth
- proxy/TLS options
- streamed upload/download
- response headers/body/timing output
- machine-readable JSON/NDJSON modes
- useful exit codes

## Milestone Y: Documentation and examples

Create complete Rust, Python, and CLI documentation.

Required areas:

- request lifecycle
- connection pooling
- timeouts
- streaming
- redirects
- cookies/auth
- proxies/TLS
- migration from requests and HTTPX
- compatibility matrix
- cookbook examples

Examples should include JSON APIs, large downloads/uploads, SSE-like streaming, authentication, and common third-party APIs.

## Milestone Z: Additional bindings and frameworks (complete)

Treat `eggfetch-core` as a reusable engine for future consumers.

Implemented capabilities:

- C ABI boundary (`eggfetch-ffi`)
- Node.js N-API prototype
- opaque handle pattern
- blocking-send runtime bridge
- FFI surface audit

These began only after the core API and semantics are stable.

# Phase 5.5: Pre-Release Validation

## Milestone L: Correctness and differential testing (complete)

Comprehensive correctness validation and differential testing against reference
implementations (requests, HTTPX).

Implemented capabilities:

- full Rust test suite (~750+ tests) covering construction, streaming, timeouts,
  pools, headers, integration scenarios
- Python sync/async API tests (~463+ tests) covering response compatibility,
  redirect behavior, redirect replay, total timeout across redirects, sync/async
  API parity
- Cookie subsystem tests (parsing, matching, jar operations, client integration, Python API)
- Authentication subsystem tests (Basic/Bearer auth, precedence, cross-origin credential stripping)
- Multipart encoder, boundary, streaming, known-length, Python files= support
- Response decompression tests (gzip, deflate, brotli, zstd streaming decoders)
- Proxy subsystem tests (HTTP proxying, HTTPS CONNECT tunneling, NO_PROXY bypass)
- Retry subsystem tests (policy construction, method safety, backoff, Retry-After)
- Cross-feature integration tests (multipart through proxy, compressed responses through proxy)
- True network streaming tests (sync/async client.stream(), StreamingResponse, chunk iteration)
- HTTP/2 tests (ALPN negotiation, error taxonomy, forbidden header stripping)
- FFI tests (~40+ tests for C ABI handle lifecycle, null safety, request building)
- CLI tests (19 unit tests, 56 subprocess integration tests)
- cargo-fuzz targets for headers, cookies, redirects, multipart, compression, proxy, timeout, retry, TLS, URL
- Proptest property tests for round-trip invariants and state-machine correctness
- Redaction regression tests for secret sanitization

## Milestone M: Documentation and public MVP preparation (complete)

Complete documentation across all surfaces and prepare for public release.

Implemented capabilities:

- Getting started docs (installation, quickstart)
- Concept docs (12 topics: architecture, lifecycle, streaming, timeouts, pooling,
  cookies, authentication, redirects, retry, TLS, proxy, compression, multipart)
- Rust API guide, Python API guide, CLI reference
- Migration guides (from requests, from HTTPX)
- Cookbook examples
- Compatibility matrix (56-row feature comparison)
- Feature matrix (13 feature flags with defaults per surface)
- Error reference (56 Rust variants, Python exception hierarchy, CLI exit codes)
- Versioning policy
- Architecture documentation (overview, dependency policy, feature flags, threat model,
  security reviews, security findings, incident runbook, release security checklist)
- Release process and compatibility policy
- FFI documentation (C ABI guide, architecture, surface audit)
- Security guidelines and troubleshooting

# Phase 6: Production Readiness

## Production Track A: Benchmarking and performance

Build a reproducible benchmark suite comparing eggfetch with relevant clients such as requests, HTTPX, aiohttp, and reqwest.

Measure:

- latency and throughput
- connection reuse
- allocation behavior
- memory under streaming load
- startup/import cost
- sync versus async Python overhead
- redirect/proxy/compression overhead

Performance work must not weaken correctness or security.

## Production Track B: Robustness and fuzzing

Add fuzz targets for:

- headers
- URL/query normalization
- cookie parsing
- redirect resolution
- multipart generation
- compression streams
- timeout state machines

Use cargo-fuzz initially and evaluate OSS-Fuzz later.

## Production Track C: Security hardening

Add:

- cargo-audit
- cargo-deny
- dependency licensing policy
- TLS review
- redirect credential-leak tests
- decompression/resource-exhaustion limits
- proxy/auth review
- security response process

Formal external audit can be considered once the feature surface stabilizes.

## Production Track D: Release engineering

Establish:

- reproducible Rust releases
- multi-platform Python wheels
- PyPI publishing automation
- crates.io publishing policy
- changelog and release notes
- semantic-versioning and deprecation policy
- MSRV and supported-Python policy
- signed provenance/artifacts where practical

# Recommended execution order

The preferred order is:

1. Milestone N: semantic tightening
2. Milestone O: cookies
3. Milestone P: authentication
4. Milestone Q: multipart/files (complete)
5. Milestone R: compression (complete)
6. Milestone S: proxies (complete)
7. Milestone T: TLS configuration (complete)
8. Milestone U: retries (complete)
9. Milestone V: HTTP/2 (complete)
10. Milestone W: HTTP/3 (complete, experimental)
11. Milestone X: full CLI
11. Milestone Y: documentation/examples
12. Production tracks
13. Milestone L: correctness and differential testing (complete)
14. Milestone M: documentation and public MVP preparation (complete)
15. HTTP/3 and additional language bindings
16. Milestone Z: additional bindings and frameworks (complete)

Cookies and authentication may be developed in parallel only after Milestone N is complete and shared request/redirect semantics are stable.

# Compatibility policy

eggfetch should continue to provide familiar requests/httpx semantics without claiming drop-in compatibility prematurely.

Documentation must classify each feature as:

- supported and tested
- partially supported with documented differences
- planned
- intentionally unsupported

Any compatibility alias or drop-in package should wait until a large differential test suite demonstrates that the claim is credible.

# Revised MVP criteria

A public MVP is ready when:

- semantic tightening is complete
- Rust and Python sync/async APIs pass local and CI validation
- real Python streaming works
- redirect/body replay and total-timeout behavior are correct
- cookies, basic/bearer auth, and multipart uploads are available
- the CLI supports common request workflows
- wheels build for the declared Python/platform matrix
- compatibility limitations are explicit
- dependency and security checks are active

# Long-term product position

`eggfetch-core` should be treated as the primary reusable HTTP engine. The Python package and CLI are first-class frontends, not the only consumers.

That positioning preserves the strongest aspect of the design: one audited, high-performance implementation shared across Rust, Python, CLI, future language bindings, and other eggstack projects.
