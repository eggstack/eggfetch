# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

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
