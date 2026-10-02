# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.2.2] - 2026-10-02

### Added

- `eggfetch-core`: native transport failure classification. `Error` gains one
  additive method, `transport_failure_kind()`, returning
  `Option<TransportFailureKind>` with the new `#[non_exhaustive]` enum
  `TransportFailureKind::{Connect, Tls, Protocol, Cancelled}` (`None` when the
  typed evidence does not prove a category). The classifier walks the
  `source()`/`get_ref()` chain with TLS-before-connect precedence and treats a
  premature-body/malformed-framing `io::Error` observed at the
  `NativeResponseBody` polling boundary as `Protocol`. It is diagnostic only:
  `Error` itself is unchanged (no new variants, no `#[non_exhaustive]`,
  unchanged `kind()` tokens and `Display`), and
  `RetryPolicy::is_error_retryable()` never consults it. This is a Rust-core
  surface only — the Python, CLI, FFI, and Node adapters do not gain an
  equivalent classifier API.

### Fixed

- Correctness and hardening fixes from the internal audit that landed after
  0.2.1, all verified by the qualified Tier 1/Tier 2 gates on this candidate:
  - Core: timeout arithmetic saturates instead of panicking on huge finite
    values (`Duration::MAX`-scale totals, `started + total` overflow); CONNECT
    target formatting rejects `/`, `?`, `#`, and `@` in the authority;
    low-level transport retry is limited to connect-level failures plus
    stale-pool cancellations, so mid-request and user errors no longer retry;
    proxy configuration fails closed on a missing host rather than falling back
    to localhost; an empty CA directory fails closed for the `ca_certificate`
    (replace) variant; SOCKS deadline overflow is treated as an
    already-expired `Total` timeout; `Content-Length` conflicts are rejected;
    proxy byte budgets, cross-origin `Referer`/`Origin` stripping, proxy-auth
    conflict handling, redirect `SNI`/pinning retention, and total-timeout
    tie-breaking (`Total` wins) are enforced.
  - Redaction: `proxy_rejection_body` redacts credential-looking text on token
    boundaries, the redact helper always truncates at `?`/`#`,
    `TransportHints` `Debug` logs target length only, and the HTTPX cookie
    `__repr__` redacts values.
  - Python: `ssl.SSLContext` translation rejects security-semantic
    `OP_UNSAFE_LEGACY_RENEGOTIATION`, `OP_LEGACY_SERVER_CONNECT`,
    `OP_NO_TICKET`, and `OP_NO_RENEGOTIATION`; websocket failures raised by
    native eggfetch errors are translated to `WebSocketNetworkError`;
    async streaming iterators spawn on the client runtime handle like sync
    iterators; `__aiter__` is checked before `__iter__`; `is_upgraded` uses a
    construction-time flag; `SSLContext` client certificates are documented as
    verify-only.
  - CLI: `--no-follow` now parses (`overrides_with` instead of
    `conflicts_with`); JSON and NDJSON redirect URLs are routed through
    `safe_url_for_display` so userinfo, query, and fragment are stripped.
  - FFI: out-pointers are nulled on allocation failure, the tracking pointer
    is cleared before `Box::from_raw` so a panic fallback cannot double-free,
    the builder is restored on `Err`, and the bounded C-string scan no longer
    uses the signed-only `cast_unsigned()` conversion — that made
    `eggfetch-ffi` fail to compile on targets where `c_char` is `u8`
    (aarch64 Linux and other unsigned-`c_char` targets).
  - Node: header and body FFI failures propagate as `napi::Error` instead of a
    silent skip or empty body.
- Python: async trace callbacks that are not coroutine-callable are rejected
  with `TypeError` before dispatch, restoring error-slot-first precedence over
  the transport result.

### Changed

- No dependency, feature-flag, feature-default, or MSRV change. MSRV remains
  Rust 1.89. The `Error` enum, existing error kinds, timeout/dialer/
  `NetworkFailureKind` semantics, and the public Rust/C/Python/HTTPX/CLI APIs
  are otherwise unchanged; the changes above are bug fixes plus one additive
  Rust enum and method. Qualification/test hardening and documentation work
  landed in the same window (pedantic-clippy drift fixes on stable 1.99, and a
  fix to `scripts/check_rust_public_api.py` so both halves of the public API
  oracle share one pinned toolchain); neither changes runtime behavior.

## [0.2.1] - 2026-09-26

### Changed

- No runtime or user-visible change: public Rust/Python/C/CLI/HTTPX APIs,
  feature graph and defaults, MSRV (1.89), and dependency policy are
  unchanged from 0.2.0. This patch resynchronizes the coordinated release
  identity (`eggfetch-http-connect`, `eggfetch-core`, `eggfetch-cli`,
  `eggfetch-ffi`, `eggfetch-python`, `eggfetch-node`, plus the PyPI
  `eggfetch` distribution) on 0.2.1 because the historical `v0.2.0` tag
  already exists and must not be moved.
- Qualification/test hardening since `v0.2.0` (already bound in Stage C
  `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`): hermetic Windows-TLS
  response-completeness matrix in
  `crates/eggfetch-core/tests/tls_response_completeness.rs` plus the
  "Origin TLS shutdown contract" subsection in
  `docs/architecture/core-tls-proxy-protocols.md`. No production transport,
  public API, or dependency change.
- Benchmark/qualification harness only (`eggfetch-bench` is never
  published): deterministic protocol fixtures, `BenchProxy` repair, and
  `native_streaming_tail` coverage; corresponding `Cargo.lock` entries are
  bench-dev-only.
- Docs/planning only: architecture guide touch-ups, planning-system
  migration, M006/M006C1/M006C2 closure records, and the M001A/M002/M001B
  0.2.1 release-train decomposition. No CI workflow, packaging, or
  compatibility-profile change.

## [0.2.0] - 2026-09-22

### Fixed

- Includes the fixes previously prepared as 0.1.8 and 0.1.9, which never
  reached all registries together (crates.io stayed at 0.1.7 while PyPI
  moved to 0.1.8; 0.1.9 was versioned in-tree but unpublished). 0.2.0
  resynchronizes crates.io, PyPI, and GitHub releases on one version:
  - Streaming decompression chunk-boundary corruption (issue #24):
    compressed chunks are decoded identically via `Content-Length` and
    `Transfer-Encoding: chunked` for gzip/Brotli/deflate/zstd (see the
    0.1.9 entry below for detail).
  - Windows PyPI wheel builds under `RUSTFLAGS=-D warnings` (see the
    0.1.8 entry below for detail).

### Changed

- No intentional breaking changes: public Rust/Python/C/CLI/HTTPX APIs,
  feature graph and defaults, MSRV (1.89), and dependency policy are
  unchanged from 0.1.7. The minor bump resynchronizes the registries and
  covers the API-preserving internal decomposition landed since 0.1.7.

## [0.1.9] - 2026-09-19

### Fixed

- Streaming decompression chunk-boundary corruption (issue #24):
  replaced the private `BoxBytesStream` → `AsyncRead` adapter in
  `crates/eggfetch-core/src/compression.rs` with the already-locked
  `tokio_util::io::StreamReader` (0.7.18), retaining the existing
  `tokio::io::BufReader`. Fully consumed compressed chunks are no longer
  retained and re-emitted when the next source item arrives, and empty
  source `Bytes` items no longer surface as zero-byte reads before true
  EOF. Hyper already removes HTTP/1.1 chunk framing before decoding, so
  chunked transfer merely exposed the adapter defect; identical bytes via
  `Content-Length` and `Transfer-Encoding: chunked` now decode identically
  for gzip/Brotli (plus deflate/zstd fragmentation, empty-chunk, awkward
  header/trailer splits, and size/ratio limit enforcement on fragmented
  input). No public Rust/Python/CLI/FFI/Node/HTTPX API, feature graph,
  timeout, raw-body, negotiation, or dependency/MSRV change; decoded chunk
  sizes remain an unstable framing detail.

## [0.1.8] - 2026-09-19

### Fixed

- Windows PyPI wheel builds: the non-Unix fallback in
  `send_uds_route` (`crates/eggfetch-core/src/pipeline/hyper_dispatch.rs`)
  now marks `inner` and `remaining_total` as intentionally unused. Both
  are only read by the Unix branch, so under `RUSTFLAGS=-D warnings`
  all six Windows wheels failed with `unused-variables` while
  Linux/macOS builds stayed green. No behavior change on any platform.
  0.1.8 supersedes 0.1.7 for PyPI (0.1.7 was never uploaded there).

## [0.1.6] - 2026-09-17

### Added

- Strict redirect transport policy: `RedirectDowngradePolicy::{Allow, Deny}`
  on `RedirectPolicy` (default `Allow` for compatibility),
  `RedirectPolicy::strict()` / `with_downgrade()` constructors,
  `ClientBuilder::redirect_downgrade_policy()`, and
  `build_redirect_request_with_redirect_policy()`. Under `Deny`, HTTPS ->
  HTTP downgrades fail with `InvalidRedirectLocation` before second-hop I/O.
- Explicit opt-in environment proxy resolution: `ProxyEnvironment`
  (`from_map()` for tests, `from_env()` snapshot at the call site,
  `resolve()` per URL, `client_proxies()` for builder integration) plus
  fallible `ClientBuilder::proxy_environment()`. Native default stays
  environment-independent. Lowercase proxy variables win, `ALL_PROXY` is the
  per-scheme fallback, `NO_PROXY` uses native parsing and applies before
  dispatch, and invalid values fail closed with redacted errors. Also adds
  `Proxy::http_compat()` / `https_compat()` for credential-bearing
  environment URLs.

### Changed

- `eggfetch-core` unconditional `base64` dependency aligned from 0.22 to
  0.23 (source-compatible `Engine` API); the minimal updater-style feature
  set (`http1,tls-rustls,tls-native-roots,proxy`) now carries a single
  `base64` 0.23 line in its runtime graph.

## [0.1.5] - 2026-09-17

### Changed

- Raised the workspace MSRV from Rust 1.80 to Rust 1.89, while retaining
  Edition 2021 and stable as the normal development toolchain. This is a
  pre-1.0 breaking compatibility change for Rust consumers.

### Fixed

- Native detailed request failures now preserve typed DNS provenance on the
  standard Hyper HTTP/HTTPS route without changing the public `Error` or
  ordinary request behavior.

## [0.1.7] - 2026-09-18

### Fixed

- Native `Timeout.total` is once again one absolute request-lifecycle
  deadline through response-body EOF/trailers for high-level
  (`bytes()`/`text()`/`json()`/`bytes_stream()`/`raw_bytes_stream()`,
  decoded and raw compressed paths) and frame-preserving native
  (`Client::execute_http_body()`, `NativeHttpService`) surfaces. The
  deadline never resets on chunk/frame arrival, decode selection, or first
  body poll; an already-expired deadline reports `TimeoutPhase::Total`
  without accepting a ready chunk, and `Total` wins ties with `Read`.
  Redirect final bodies use the remaining original budget; retry aggregate
  semantics unchanged. No public API, dependency, MSRV, or
  compatibility-facade change.

### Changed

- Resolved-target connection reuse: identical direct `resolved_addresses()`
  routes (same logical origin, same ordered address snapshot, same SNI
  override) now reuse one bounded Hyper client (64 entries) for H1 keep-alive
  / H2 multiplexing instead of building an isolated client per request.
  Routing semantics are unchanged (no DNS fallback, same-origin redirect
  retention, cross-origin fail-closed, proxy/UDS/H3 rejection).

## [0.1.4] - 2026-09-13

### Added

- Native Rust JSON behind the opt-in `json` feature: replayable `RequestBuilder::json()` and single-consumption `Response::json()`. Per-request decoded-body limits override client limits across retries and redirects. Minimal consumers pay nothing for serde.
- Static resolved routing: pinned-destination `TransportHints` (`resolved_target`) for pre-resolved addresses. Static destinations reject proxy, UDS, and H3 combinations before any I/O.
- Runnable examples: `crates/eggfetch-core/examples/quickstart.rs` (GET/POST/timeout/auth/streaming), `examples/python_sync.py`, and `examples/python_async.py`.

### Changed

- Core feature and TLS boundary hardening: all origin TLS routes build through `TlsConfig`/`TrustStore`, and feature-flag implications (`http3`, `proxy`) are explicit about the H1/Rustls capabilities they require.
- README condensed to a quickstart plus grouped feature bullets linking into `docs/`; JSON, resolved-routing, embedded, and httpx-delta details live in their guides.

### Fixed

- Documentation corrected to `response = await client.stream(...)` for the native async API (five guides previously showed the HTTPX-style `async with` form, which only the compat facade supports).

## [0.1.3] - 2026-09-11

### Fixed

- Windows (`x86_64-pc-windows-msvc`) build with `RUSTFLAGS=-D warnings` (PyPI wheel matrix): `cfg(unix)`-gated the Unix-domain-socket-only imports in `transport/uds.rs`, removed the dead non-Unix `unsupported()` fallback (the non-Unix dispatch site in `pipeline.rs` already returns `Error::Unsupported` directly) and the obsolete `Bytes` import keep-alive, and acknowledged the intentionally cross-platform `ClientBuilder::uds_path` field in `build()` on non-Unix targets. No behavior change on any platform; `uds_path()` remains accepted everywhere and still errors at request time off Unix.

## [0.1.2] - 2026-09-11

### Added

- Python `request.extensions` accepts typed keys `target` (str or bytes), `sni_hostname` (str), and `trace` (callable).  These are extracted once by `extract_native_extensions` into `eggfetch_core::TransportHints` before dispatch.  Unknown keys are passed through to the HTTPX compatibility facade.
- Native `PyTraceObserver` (in `crates/eggfetch-python/src/trace_bridge.rs`) bridges Python callables into `eggfetch_core::trace::TraceObserver` so callables never enter the core crate. Sync callbacks work on both `Client` and `AsyncClient`; coroutine callbacks (detected via `inspect.iscoroutinefunction`) are rejected with `TypeError` before dispatch on both APIs because the core `TraceObserver` is synchronous.
- `Response.extensions` and `StreamingResponse.extensions` properties expose `{http_version, reason_phrase, network_stream}` snapshots of the wire state.
- `NetworkStream` (sync, GIL-released) and `AsyncNetworkStream` exposed through `Response.extensions["network_stream"]` for 101 Switching Protocols upgrades. The sync wrapper carries an explicit `tokio::runtime::Handle` and optional `RuntimeLease` so it can drive IO without relying on an ambient runtime. Cloning a `NetworkStream` shares the same underlying `Arc<Mutex<>>` so the IO is shared, not duplicated. Internal HTTPS CONNECT tunnels are never surfaced as writable `NetworkStream` (the canonical access path is the body iterator).
- `NetworkStream.start_tls(ssl_context=..., server_hostname=..., timeout=...)` uses the same safe TLS translation as the default client. It is rejected for Hyper-opaque `Adapter` variants and for streams that are already TLS-wrapped; only inner `Tcp`-variant streams support `start_tls`.
- `UpgradedStream` carries an `UpgradedStreamVariant` (`Tcp`/`Tls`/`Adapter`) classification so callers can detect `start_tls` eligibility before any IO is consumed.

### Fixed

- `pipeline::send_with_redirects` redirects-disabled fast path now reattaches the original `TransportHints` to the reconstructed request, so `target`, `sni_hostname`, and `trace` are no longer silently dropped when internal redirect handling is bypassed.
- `Response.reason_phrase` prefers `wire_reason_phrase()` over the canonical lookup table, so responses with non-canonical status-line reason phrases are surfaced truthfully.

## [0.1.0] - 2026-07-16

### Added

- Core async HTTP engine with connection pooling, timeouts, and streaming
- Python sync and async APIs via PyO3/maturin
- CLI HTTP client with machine-readable output
- C ABI bindings (eggfetch-ffi)
- HTTP/2 support with ALPN negotiation
- HTTP/3 QUIC transport (experimental)
- Response decompression (gzip, deflate, brotli, zstd)
- Multipart/form-data uploads
- Cookie subsystem with RFC 6265 handling
- Authentication (Basic, Bearer)
- HTTP proxy and HTTPS CONNECT tunneling
- TLS configuration (custom CA bundles, client certificates, version policy)
- Retry policy with exponential backoff
- Streaming request and response bodies
- Phase-aware timeout system

### Security

- Secret redaction in all Debug/Display/error output
- Cross-origin redirect credential stripping
- Decompression resource limits (max size, ratio)
- Multipart boundary validation
- Proxy authentication boundary enforcement

[Unreleased]: https://github.com/eggstack/eggfetch/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/eggstack/eggfetch/releases/tag/v0.2.2
[0.2.1]: https://github.com/eggstack/eggfetch/releases/tag/v0.2.1
[0.2.0]: https://github.com/eggstack/eggfetch/releases/tag/v0.2.0
[0.1.9]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.9
[0.1.8]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.8
[0.1.7]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.7
[0.1.6]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.6
[0.1.5]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.5
[0.1.4]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.4
[0.1.3]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.3
[0.1.2]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.2
[0.1.1]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.1
[0.1.0]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.0
