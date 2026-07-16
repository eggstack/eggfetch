# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
