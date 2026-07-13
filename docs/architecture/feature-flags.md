# Feature Flags

eggfetch-core uses feature flags to reserve optional capabilities and to keep
the public feature matrix explicit. The current transport implementation is
HTTP/1.1 over Rustls; its direct transport dependencies remain unconditional
so `--no-default-features` is a supported compile check, not a no-network
build.

## Current Features

The following features are declared in `crates/eggfetch-core/Cargo.toml`:

```toml
[features]
default = ["http1", "tls-rustls"]
http1 = []
http2 = []
tls-rustls = []
json = []
compression-gzip = []
compression-brotli = []
compression-zstd = []
cookies = []
multipart = []
proxy = []
tracing = []
```

## Default Features

```toml
default = ["http1", "tls-rustls"]
```

The defaults advertise HTTP/1.1 and Rustls TLS. Cookies are deliberately not
enabled by the core default feature set; the Python binding enables them for
its public cookie API.

## Feature Reference

### http1

**Status:** implemented (Milestone B).
Enables HTTP/1.1 support. This is the primary protocol for the MVP, backed by hyper.

### http2

**Status:** planned, not implemented.
**Milestone:** after MVP.
Enables HTTP/2 support. HTTP/2 is a later expansion, not an MVP requirement.

### tls-rustls

**Status:** implemented (Milestone B).
Enables the Rustls transport configuration. Native roots are preferred at
runtime and packaged WebPKI roots are used only when native roots are
unavailable. Verification failures never trigger the fallback. The current
Cargo dependency graph keeps the transport crates available in all core
feature combinations so disabled-feature checks remain buildable.

### json

**Status:** feature flag exists, not wired into eggfetch-core.
**Milestone:** I (request builder compatibility surface) delivered JSON body support in the Python crate via Python's `json.dumps()`, not through a Rust-side feature gate. The feature flag is reserved for future Rust-native JSON serialization (e.g., serde integration in `eggfetch-core`).

### compression-gzip

**Status:** planned, not implemented.
**Milestone:** not yet implemented (after E).
Enables gzip decompression of response bodies. This is behind a feature flag to avoid pulling in compression dependencies for users who do not need them.

### compression-brotli

**Status:** planned, not implemented.
**Milestone:** not yet implemented (after E).
Enables Brotli decompression of response bodies.

### compression-zstd

**Status:** planned, not implemented.
**Milestone:** not yet implemented (after E).
Enables Zstandard decompression of response bodies.

### cookies

**Status:** implemented (Milestone O).
Enables cookie jar support for persistent cookies across requests. Provides RFC 6265 cookie parsing, domain/path matching, cookie jar with thread-safe storage, and automatic Set-Cookie ingestion on responses. The Python crate exposes `client.cookies`, `response.cookies`, and a `cookies=` kwarg for initial cookies.

Python request-local `cookies=` values are serialized into the request header,
are not persisted in the client jar, and are removed on cross-origin redirects.

### multipart

**Status:** implemented (Milestone Q).
Enables streaming multipart/form-data request bodies. Provides `Multipart`, `Part`, `PartBody`, and `Boundary` types with a builder API, a streaming encoder backed by a state machine, known-length calculation when all parts have known sizes, boundary generation and validation, and per-part headers and content types. The Python crate exposes `files=` kwarg support including bytes, tuples, path-backed `File` wrapper, and mixed `data=` + `files=`.

Python `files=` accepts bytes, `(filename, data)` tuples, `(filename, data, content_type)` triples, `(filename, data, content_type, headers)` quads, and `eggfetch.File(path)` objects. Files are read via synchronous std::fs (blocking in GIL context) for path-backed parts. Cancellation safely drops file handles and streams.

### proxy

**Status:** planned, not implemented.
**Milestone:** deferred beyond I. Proxy support requires CONNECT tunneling, authentication, and PAC file handling, which are beyond the MVP scope. The feature flag is reserved.

### tracing

**Status:** planned, not implemented.
**Milestone:** after core engine is stable.
Enables structured logging via the tracing ecosystem. This is opt-in to avoid pulling in logging dependencies for users who do not need them.

## Rules

- Do not add a feature just to silence a clippy lint.
- Do not enable optional behavior in `default` without discussion.
- Every feature must have a clear purpose and be documented here.
- Features that are not core to HTTP/1.1 client behavior stay optional.

## Validation matrix

The repository validates the following core combinations in CI and before a
release:

```text
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --all-features
cargo test -p eggfetch-core --all-features
```
