# eggfetch Revised Roadmap

## Purpose

eggfetch is a Rust-native HTTP client platform with Python bindings and a CLI layered on top of a single async engine. The project has completed its initial architectural and MVP-construction phase: the Rust core, Python sync and async APIs, response compatibility surface, common request body handling, redirects, streaming foundations, timeouts, and pooling are all in place.

The remaining roadmap is therefore not primarily about proving feasibility. It is about tightening semantics, completing the expected HTTP-client feature set, expanding transport capabilities, and establishing production-grade release, security, testing, and documentation practices.

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

## Milestone W: HTTP/3

Introduce optional HTTP/3 support through a separately gated transport, likely using Quinn and the `h3` ecosystem.

Requirements:

- no disruption to HTTP/1.1 and HTTP/2 defaults
- protocol fallback/selection policy
- QUIC connection lifecycle
- 0-RTT policy if ever enabled
- certificate and timeout integration
- extensive interoperability testing

HTTP/3 is post-MVP and should remain isolated until mature.

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

## Milestone Z: Additional language bindings and framework use

Treat `eggfetch-core` as a reusable engine for future consumers.

Potential tracks:

- Node/N-API
- Ruby
- Perl
- Java/JNI
- Zig/C ABI
- internal eggstack consumers

These should begin only after the core API and semantics are stable.

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
9. Milestone V: HTTP/2
10. Milestone X: full CLI
11. Milestone Y: documentation/examples
12. Production tracks
13. HTTP/3 and additional language bindings

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
