# Milestone V Plan: HTTP/2

## Objective

Enable production-quality HTTP/2 support without regressing HTTP/1.1. HTTP/2 must share the same request pipeline, TLS, timeout, redirect, cookie, auth, proxy, compression, and Python APIs while using protocol-appropriate connection and concurrency semantics.

## Scope

Implement:

- HTTP/2 feature wiring in hyper/hyper-util/rustls
- ALPN negotiation for HTTPS
- optional prior-knowledge h2c support only if deliberately scoped
- client protocol preference/policy
- protocol version reporting
- multiplexed connection semantics
- stream concurrency limits and pool model adjustment
- HTTP/2 error mapping
- flow-control, cancellation, and load tests
- direct and CONNECT-proxy HTTP/2 where transport permits
- Python configuration and parity tests

Do not implement server push consumption unless a compelling use case exists. Do not expose HTTP/2 priority APIs initially.

## Protocol policy

Suggested configuration:

```rust
pub enum HttpVersionPolicy {
    Http1Only,
    Http2Only,
    Auto,
}
```

Default may remain HTTP/1.1 initially until HTTP/2 validation is complete, then switch to `Auto` deliberately. Python target:

```python
Client(http2=True)
```

`http2=True` should enable negotiation, not necessarily force HTTP/2 unless a separate strict option is provided.

## Pool/concurrency redesign

Current permits model active logical requests and proxy routes. HTTP/2 multiplexes many streams on one connection, so document and separate:

- logical request concurrency
- per-origin HTTP/2 stream limit
- physical connection count
- server-advertised `SETTINGS_MAX_CONCURRENT_STREAMS`

Do not claim physical connection metrics unless measured. A first implementation may rely on hyper's h2 pool while eggfetch continues enforcing logical request permits.

## TLS/ALPN

Use Milestone T TLS config. ALPN should advertise `h2` and `http/1.1` according to policy. Custom CA, mTLS, and CONNECT paths must preserve ALPN settings.

## Request/response semantics

Ensure HTTP/2-specific rules:

- no HTTP/1 transfer-encoding semantics
- pseudo-header generation remains inside hyper
- reject/strip forbidden connection-specific headers (`Connection`, `Keep-Alive`, `Transfer-Encoding`, `Upgrade`, inappropriate `TE`)
- trailers are either supported explicitly or documented as unavailable
- streaming request/response bodies preserve backpressure

## Error taxonomy

Map connection errors, stream resets, refused streams, flow-control errors, and protocol errors without exposing unstable hyper internals. Preserve source errors.

Retry policy may later classify `REFUSED_STREAM` as retryable only for replayable requests.

## Tests

Required:

- ALPN selects h2 against local TLS h2 server
- Auto falls back to HTTP/1.1
- HTTP2-only fails clearly against h1-only server
- many concurrent requests multiplex correctly
- server stream limit is respected
- cancellation/reset releases permits
- large upload/download flow-control works
- streaming/decompression/multipart work over h2
- cookies/auth/redirects retain semantics
- forbidden headers handled correctly
- direct and CONNECT paths tested if proxy supports h2 tunnel destination
- protocol version exposed in Rust/Python

Add load tests that compare connection counts for h1 versus h2.

## Python API

Add `http2` client option and possibly strict version policy via an advanced enum later. Sync and async APIs must match. Response `http_version` should report `HTTP/2` accurately.

## Feature gating

The `http2` feature should enable only necessary hyper/h2 features. Validate:

- HTTP/1-only minimal build
- HTTP/2 build without optional cookies/proxy/compression
- all-features build

## Acceptance criteria

- HTTP/2 negotiates and multiplexes correctly
- HTTP/1.1 remains green
- pool/concurrency semantics are documented honestly
- protocol errors/cancellation do not leak permits
- all existing features work over h2 or have explicit limitations
- Python wheels and CI cover h2-enabled behavior
