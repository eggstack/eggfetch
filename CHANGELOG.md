# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Python `request.extensions` accepts typed keys `target` (str or bytes), `sni_hostname` (str), and `trace` (callable).  These are extracted once by `extract_native_extensions` into `eggfetch_core::TransportHints` before dispatch.  Unknown keys are passed through to the HTTPX compatibility facade.
- Native `PyTraceObserver` (in `crates/eggfetch-python/src/trace_bridge.rs`) bridges Python callables into `eggfetch_core::trace::TraceObserver` so callables never enter the core crate.  Async callbacks (detected via `inspect.iscoroutinefunction`) supplied to sync `Client` produce a `TypeError`; `AsyncClient` accepts coroutine callbacks.
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

[Unreleased]: https://github.com/eggstack/eggfetch/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/eggstack/eggfetch/releases/tag/v0.1.0
