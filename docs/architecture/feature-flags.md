# Feature Flags

eggfetch-core uses feature flags to let users opt into capabilities they need. The default feature set is minimal.

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
proxy = []
tracing = []
```

## Default Features

```toml
default = ["http1", "tls-rustls"]
```

The defaults enable HTTP/1.1 and Rustls TLS. This is the minimal set required for a useful HTTPS client. Users who want HTTP/2, compression, cookies, proxy support, JSON, or tracing must opt in explicitly.

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
Enables TLS via Rustls. This is preferred over native TLS for portability and auditability. The feature gates the rustls, tokio-rustls, and hyper-rustls dependencies.

### json

**Status:** planned, not implemented.
**Milestone:** I (request builder compatibility surface).
Enables JSON request body serialization and response body deserialization via serde and serde_json. This is an optional convenience, not a core requirement.

### compression-gzip

**Status:** planned, not implemented.
**Milestone:** after streaming foundation (E).
Enables gzip decompression of response bodies. This is behind a feature flag to avoid pulling in compression dependencies for users who do not need them.

### compression-brotli

**Status:** planned, not implemented.
**Milestone:** after streaming foundation (E).
Enables Brotli decompression of response bodies.

### compression-zstd

**Status:** planned, not implemented.
**Milestone:** after streaming foundation (E).
Enables Zstandard decompression of response bodies.

### cookies

**Status:** planned, not implemented.
**Milestone:** H (response compatibility surface).
Enables cookie jar support for persistent cookies across requests.

### proxy

**Status:** planned, not implemented.
**Milestone:** I (request builder compatibility surface).
Enables HTTP and HTTPS proxy support. SOCKS proxy support is a later addition.

### tracing

**Status:** planned, not implemented.
**Milestone:** after core engine is stable.
Enables structured logging via the tracing ecosystem. This is opt-in to avoid pulling in logging dependencies for users who do not need them.

## Rules

- Do not add a feature just to silence a clippy lint.
- Do not enable optional behavior in `default` without discussion.
- Every feature must have a clear purpose and be documented here.
- Features that are not core to HTTP/1.1 client behavior stay optional.
