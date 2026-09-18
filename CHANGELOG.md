# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

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

## [Unreleased]

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

[Unreleased]: https://github.com/eggstack/eggfetch/compare/v0.1.5...HEAD
[0.1.5]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.5
[0.1.4]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.4
[0.1.3]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.3
[0.1.2]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.2
[0.1.1]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.1
[0.1.0]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.0
