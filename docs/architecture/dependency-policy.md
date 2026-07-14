# Dependency Policy

eggfetch follows a conservative dependency policy. Every dependency must have an explicit reason. The project trades breadth of features for correctness, auditability, and a small transitive tree.

## Current Posture

As of the post-Milestone-S corrective integration pass, eggfetch-core has the
following direct dependencies:

- **bytes** -- efficient byte buffer types for request and response bodies.
- **dashmap** -- concurrent hash map for per-host pool semaphore storage.
- **futures-core** -- `Stream` trait definition.
- **futures-util** -- `StreamExt` for stream combinators.
- **http** -- standard HTTP types (`Method`, `StatusCode`, `HeaderMap`, `Uri`).
- **http-body** -- body trait abstraction.
- **http-body-util** -- body combinators for http-body (`Full`, `Empty`).
- **hyper** -- HTTP/1.1 protocol implementation.
- **hyper-util** -- high-level client utilities built on hyper.
- **hyper-rustls** -- TLS integration via rustls, with native roots preferred
  and packaged Mozilla roots as a verified fallback when the host keychain is
  unavailable.
- **rustls** -- memory-safe TLS implementation.
- **tokio** -- async runtime.
- **tokio-rustls** -- async TLS streams for tokio + rustls.
- **url** -- URI parsing and query string serialization.
- **thiserror** -- ergonomic error definitions.
- **cookie** -- RFC 6265 cookie parsing and representation (optional, behind `cookies` feature).
- **base64** -- Basic authentication credential encoding.
- **flate2** -- buffered gzip/deflate decompression for non-streaming response reads.
- **getrandom** -- cryptographically secure random bytes for multipart boundary generation.
- **httparse** -- low-level HTTP response parsing for proxy response status line and header extraction.

These are small, well-audited crates with minimal transitive trees. They are the minimum required to build a working HTTPS client.

## Optional Later Dependencies

Features that are not core to HTTP/1.1 client behavior are optional and feature-gated:

- **pyo3**, **pyo3-async-runtimes** -- Python bindings (eggfetch-python crate only).
- **encoding_rs** -- charset decoding for non-UTF-8 responses (eggfetch-python crate only).
- **clap** -- CLI argument parsing (eggfetch-cli crate only).
- **serde**, **serde_json** -- reserved for future Rust-native JSON serialization; not currently dependencies.
- **async-compression**, **flate2** -- streaming and buffered decompression for gzip, brotli, deflate, and zstd (optional, behind compression features).
- **tracing** -- reserved for structured logging; not currently a dependency.

These dependencies stay optional. They do not enter `default` features without discussion.

## Selection Criteria

When evaluating a new dependency:

1. **Explicit reason required.** Every dependency must solve a real problem. "It might be useful" is not a reason.
2. **Prefer Rustls over native TLS.** Rustls is memory-safe, portable, and has a smaller audit surface than OpenSSL or platform-native TLS.
3. **Minimize transitive trees.** A convenience crate that pulls in 30 transitive dependencies needs a strong justification. Prefer direct, focused crates.
4. **Avoid proc-macro-heavy crates.** Proc macros slow compilation and expand the attack surface. Use them only when they materially improve correctness or maintainability (e.g., thiserror for error definitions).
5. **Keep features non-default.** Optional behavior behind feature flags. Users pay only for what they use.

## Audit Tools

The project plans to use:

- **cargo-deny** for license compliance, advisory database checks, and duplicate dependency detection.
- **cargo-audit** for known vulnerability scanning against the RustSec advisory database.

These tools are not yet wired into CI. Adding them is part of the pre-release hardening work. Every dependency in the tree must have an explicit reason documented in code or review.

## Proxy Transport Behavior

The current proxy implementation creates a new TCP connection for each
request through a proxy. The connection pool controls logical concurrency
permits (preventing unbounded parallel requests) but does not maintain a
socket pool for proxy connections.

For HTTP forward proxying, each request:
1. Opens a new TCP connection to the proxy
2. Sends the request with absolute-form URI
3. Reads the response
4. Closes the connection

For HTTPS CONNECT tunneling, each request:
1. Opens a new TCP connection to the proxy
2. Sends a CONNECT request
3. Reads the 200 response
4. Performs a TLS handshake through the tunnel
5. Sends the HTTP request over TLS
6. Reads the response
7. Closes the connection

This means proxy connections are not reused across requests. The pool
permit key includes proxy origin to prevent excessive parallel connections,
but each permit corresponds to a fresh socket.

Future work may introduce transport-level connection pooling for proxy
connections, keyed by `(proxy origin, destination origin, tunnel mode,
TLS config)`.

## Decoded-Body Limits

The client supports configurable limits on decompressed response bodies
to prevent excessive memory consumption from compressed responses that
expand to large sizes.

- `max_decoded_body_size`: Hard limit on total decoded bytes. When
  exceeded during streaming or buffered reads, returns
  `Error::DecodedBodyLimit`. Default: unlimited.
- `max_decompression_ratio`: Optional limit comparing decoded bytes to
  compressed bytes. Applied once enough input has been observed to make
  a meaningful comparison. Default: unlimited.

These limits are enforced in both streaming and buffered paths. When a
limit is exceeded, the underlying response stream is discarded and the
pool lease is released.

## TLS root policy

`hyper-rustls` is configured with both native and packaged WebPKI root
support. Native roots are attempted first. The packaged Mozilla roots are a
construction fallback only when the native store is unavailable; they are not
tried after a certificate-chain or hostname verification failure. Enterprise
or private CAs therefore require a future explicit trust-store configuration
surface and are not silently trusted by the fallback.
