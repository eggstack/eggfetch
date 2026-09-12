# Feature Flags

eggfetch-core uses feature flags to make protocol, TLS, and optional
request/response capabilities explicit. The default remains the ordinary
secure HTTP/1.1 client with Rustls, native roots preferred, and packaged
WebPKI roots available as a construction fallback. `--no-default-features`
is a supported API/build profile with no HTTP protocol implementation
selected; requests return a clear unsupported-feature error until `http1` or
`http2` is enabled.

## Current Features

The following features are declared in `crates/eggfetch-core/Cargo.toml`:

```toml
[features]
default = ["http1", "tls-rustls", "tls-native-roots"]
http1 = ["hyper/http1", "hyper-util/http1", "hyper-rustls?/http1"]
http2 = ["dep:h2", "hyper/http2", "hyper-util/http2", "hyper-rustls?/http2"]
tls-rustls = ["dep:hyper-rustls", "dep:pem-rfc7468", "dep:rustls", "dep:tokio-rustls", "dep:webpki-roots", "hyper-rustls/ring", "hyper-rustls/logging", "hyper-rustls/tls12"]
tls-native-roots = ["tls-rustls", "dep:rustls-native-certs"]
http3 = ["http1", "tls-rustls", "dep:quinn", "dep:h3", "dep:h3-quinn"]
json = []
compression-gzip = ["dep:async-compression", "async-compression/gzip", "dep:tokio-util", "tokio/io-util", "dep:flate2"]
compression-brotli = ["dep:async-compression", "async-compression/brotli", "dep:tokio-util", "tokio/io-util", "dep:brotli"]
compression-zstd = ["dep:async-compression", "async-compression/zstd", "dep:tokio-util", "tokio/io-util", "dep:zstd"]
compression-deflate = ["dep:async-compression", "async-compression/deflate", "dep:tokio-util", "tokio/io-util", "dep:flate2"]
cookies = ["dep:cookie"]
multipart = []
proxy = ["http1", "tls-rustls", "tokio/io-util"]
tracing = ["dep:tracing"]
test-util = ["tokio/test-util"]
```

## Default Features

```toml
default = ["http1", "tls-rustls", "tls-native-roots"]
```

The defaults advertise HTTP/1.1, Rustls TLS, and native-root loading. Cookies
are deliberately not enabled by the core default feature set; the Python
binding enables them for its public cookie API. `http1` can be selected alone
for a cleartext-only build. `tls-rustls` without `tls-native-roots` uses the
packaged WebPKI roots deterministically; `tls-native-roots` adds system-store
loading and is included in the default profile.

## Feature Reference

### http1

**Status:** implemented.
Enables HTTP/1.1 support. This is the primary protocol for the MVP, backed by hyper.
The feature owns H1 support in Hyper and Hyper-util. It does not imply TLS, so
`http1` alone is suitable for cleartext-only embedded clients.

### http2

**Status:** implemented.
Enables HTTP/2 support. When enabled, the client can negotiate HTTP/2 via ALPN for HTTPS connections. The `HttpVersionPolicy` enum controls which protocol versions are advertised. `Auto` (default) advertises both `h2` and `http/1.1`; `Http2Only` advertises only `h2`; `Http1Only` advertises only `http/1.1`. Without this feature, `Http2Only` and `Auto` silently downgrade to `Http1Only`. The Python crate exposes `Client(http2=True)` and `AsyncClient(http2=True)` for enabling HTTP/2 negotiation, and `Client(http1=False, http2=True)` / `AsyncClient(http1=False, http2=True)` for HTTP/2-only prior-knowledge mode.

### http3

**Status:** implemented (experimental).
Enables HTTP/3 support over QUIC. When enabled, the client can negotiate HTTP/3 using the `quinn` crate for QUIC transport and the `h3` crate for the HTTP/3 protocol layer. `HttpVersionPolicy::Http3Only` routes direct QUIC (strict); `Auto { allow_http3: true }` discovers via authenticated Alt-Svc (no fresh entry = H1/H2). QUIC mandates TLS 1.3, so this feature explicitly implies `http1` and `tls-rustls` for discovery/fallback and TLS. 0-RTT (early data) is disabled. The Python crate exposes `Client(http3=True)` and `AsyncClient(http3=True)`, plus `H3Error`, `H3ConnectError`, and `H3ProtocolError` exception types. This feature is experimental; API surfaces may change as the QUIC/h3 ecosystem matures.

Hardened lifecycle policy (see [core-tls-proxy-protocols.md](core-tls-proxy-protocols.md)): bounded 64-entry per-origin QUIC cache with `OnceCell`-shared init and stale-eviction reconnect, multi-address fallback under one shared connect budget, phase-correct connect/total/read/write timeouts, keepalive-derived QUIC idle (default 30 s), pool-derived stream caps, no transport-level retries, plus separate bounded Alt-Svc cache/suppressor, pre-commit replayable-only safe fallback via `H3DispatchError`, and GOAWAY draining (pinned h3 0.0.8 `is_closing()`/`is_h3_no_error()`). `TransportMetrics::h3_diagnostics()` exposes bounded copied Quinn snapshots for HTTP/3 builds; unavailable stream counts are reported as `None`. Tests: `tests/h3_hardening.rs` + `tests/h3_alt_svc_discovery.rs` + `tests/h3_interop_qualification.rs` (GET/HEAD/upload/streaming/multiplex/trailers/early-close/soak/boundedness) plus unit tests in `transport/http3.rs` + `transport/alt_svc.rs` + `transport/metrics.rs` (loopback only). The implementation-neutral corpus, independent-server runner and qualification-only impairment matrix live in `qualification/http3/` and `scripts/`; external absence is an explicit unsupported result. Fuzz: `fuzz/fuzz_targets/fuzz_alt_svc.rs` (parser/cache + suppressor transitions). Graduation: retained experimental this milestone (see "Production Graduation Decision" in core-tls-proxy-protocols.md).

### tls-rustls

**Status:** implemented.
Enables the Rustls transport configuration. Native roots are preferred at
runtime and packaged WebPKI roots are used only when native roots are
unavailable. Verification failures never trigger the fallback. This feature
also supports custom CA bundles via `TrustStore`,
client certificates via `ClientIdentity`, TLS version policy via `TlsVersion`,
verification toggle via `TlsConfigBuilder::danger_accept_invalid_certs(true)`,
and SNI configuration. The Python crate exposes `verify=` and `cert=` kwargs
for TLS configuration. The Rustls transport and its configuration dependencies
are enabled only by `tls-rustls`. Without that feature, HTTPS is rejected at
the transport boundary; cleartext H1 remains available when `http1` is
selected.

### tls-native-roots

**Status:** implemented. Implies `tls-rustls` and enables system/native trust
store loading. The default profile includes it. Without this feature,
`NativeOnly` fails clearly at TLS configuration construction and the default
`NativeWithWebPkiFallback` policy resolves to packaged WebPKI roots. The
native store is never tried after certificate-chain or hostname verification
failure.

### json

**Status:** reserved flag; currently owns no dependencies and enables no
core behavior. `eggfetch-bench` and the `qualification/embedded/`
fixtures select it as a no-op marker; downstream JSON today uses
`serde_json` directly. The Python crate delivers JSON body support via
Python's `json.dumps()`, not through a Rust-side feature gate. The flag
is reserved for future Rust-native JSON serialization (e.g., serde
integration in `eggfetch-core`).

### compression-gzip

**Status:** implemented.
Enables gzip decompression of response bodies. This is behind a feature flag to avoid pulling in compression dependencies for users who do not need them. Uses `async-compression` for streaming decode and `flate2` for buffered decode. Enables `Content-Encoding: gzip` transparent decompression.

### compression-brotli

**Status:** implemented.
Enables Brotli decompression of response bodies. Uses `async-compression` for streaming decode. Enables `Content-Encoding: br` transparent decompression.

### compression-zstd

**Status:** implemented.
Enables Zstandard decompression of response bodies. Uses `async-compression` for streaming decode. Enables `Content-Encoding: zstd` transparent decompression.

### compression-deflate

**Status:** implemented.
Enables deflate decompression of response bodies. Uses `async-compression` for streaming decode. HTTP deflate is typically zlib-wrapped; this decoder handles the standard format. Enables `Content-Encoding: deflate` transparent decompression.

### cookies

**Status:** implemented.
Enables cookie jar support for persistent cookies across requests. Provides RFC 6265 cookie parsing, domain/path matching, cookie jar with thread-safe storage, and automatic Set-Cookie ingestion on responses. The Python crate exposes `client.cookies`, `response.cookies`, and a `cookies=` kwarg for initial cookies.

Python request-local `cookies=` values are serialized into the request header,
are not persisted in the client jar, and are removed on cross-origin redirects.

### multipart

**Status:** implemented.
Enables streaming multipart/form-data request bodies. Provides `Multipart`, `Part`, `PartBody`, and `Boundary` types with a builder API, a streaming encoder backed by a state machine, known-length calculation when all parts have known sizes, boundary generation and validation, and per-part headers and content types. The Python crate exposes `files=` kwarg support including bytes, tuples, path-backed `File` wrapper, and mixed `data=` + `files=`.

Python `files=` accepts bytes, `(filename, data)` tuples, `(filename, data, content_type)` triples, `(filename, data, content_type, headers)` quads, and `eggfetch.File(path)` objects. Files are read via synchronous std::fs (blocking in GIL context) for path-backed parts. Cancellation safely drops file handles and streams.

Boundary generation uses the foundational `getrandom` dependency to seed its
internal xorshift PRNG. It remains unconditional because retry backoff jitter
also uses the same crate; gating it on `multipart` would break retry behavior.

### proxy

**Status:** implemented.
Enables HTTP proxy and SOCKS5 proxy support in eggfetch-core. The feature
implies `http1` and `tls-rustls` because HTTPS proxy endpoints, CONNECT
tunnels, and HTTPS-over-SOCKS require the Rustls transport. Provides HTTP
proxying, HTTPS CONNECT tunneling, SOCKS5 tunneling (socks5:// and socks5h://),
proxy authentication, per-request and per-client proxy configuration via
`ClientBuilder::proxy()` and `RequestBuilder::proxy()`, and `NO_PROXY`-style
bypass behavior. The Python crate exposes `Client(proxy=...)`,
`AsyncClient(proxy=...)`, and per-request `proxy=` kwarg. The feature flag is
required for proxy functionality; it pulls in tunnel and proxy-protocol
dependencies.

### tracing

**Status:** implemented (optional `tracing` dependency gate).
Enables structured logging via the tracing ecosystem. This is opt-in to avoid pulling in logging dependencies for users who do not need them.

### test-util

**Status:** implemented.
Enables `tokio/test-util` for deterministic time testing. This feature is for internal use only and should not be enabled by downstream consumers. It allows tests to control time progression for timeout-related scenarios.

## Rules

- Do not add a feature just to silence a clippy lint.
- Do not enable optional behavior in `default` without discussion.
- Every feature must have a clear purpose and be documented here.
- Features that are not core to HTTP/1.1 client behavior stay optional.

## Validation matrix

The repository validates the following core combinations via Tier 2
(`./scripts/check.sh extended`: `tier2_feature_matrix` + `tier2_feature_tests`)
before a release. This list must match `scripts/check.sh` exactly — do not
add combinations here without updating the script (and vice versa):

```text
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,tls-native-roots
cargo check -p eggfetch-core --all-features
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-brotli
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-zstd
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-deflate
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
```

Manual (not Tier 2 gates) compile checks for other combinations:

```text
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,multipart,proxy
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
```
