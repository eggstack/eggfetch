# Corrective Integration Plan After Milestone S

## Objective

Perform a focused integration and hardening pass after Milestones Q–S. The repository now contains streaming multipart uploads, automatic response decompression, HTTP forward proxying, HTTPS CONNECT tunneling, proxy authentication, NO_PROXY matching, and expanded Python bindings. These capabilities are individually substantial, but they now intersect inside the central request pipeline.

This pass should reduce integration risk before TLS customization, retries, HTTP/2, or CLI expansion. The emphasis is on transport lifecycle, resource limits, feature isolation, dependency hygiene, and maintainable subsystem boundaries.

The pass is complete when the newly added capabilities behave correctly together under reuse, cancellation, redirects, timeouts, streaming, and feature-gated builds.

## Current state assumption

The repository currently provides:

- async Rust HTTP/1.1 engine over tokio, hyper/hyper-util, and rustls
- sync and asyncio Python APIs
- live Python response streaming
- request and response body streaming
- cookies and Basic/Bearer authentication
- redirect handling with replayability checks
- streaming multipart/form-data uploads
- gzip, deflate, brotli, and zstd response decompression
- HTTP forward proxying
- HTTPS CONNECT tunneling
- proxy authentication and NO_PROXY bypass rules
- proxy-aware pool permit keys
- CI across Linux, macOS, and Windows with Python 3.10–3.13

## Non-goals

Do not implement custom TLS verification controls, retries, HTTP/2, HTTP/3, SOCKS, PAC files, or broad CLI features in this pass.

Do not redesign public Python APIs unless a correctness issue requires it.

Do not replace hyper or tokio.

Do not combine optional features into default Rust-core dependencies without a clear justification.

# Track A: Split the request pipeline into reviewable modules

## Problem

`client.rs` now coordinates request defaults, cookies, auth, redirects, multipart bodies, proxy selection, direct transport, CONNECT tunneling, TLS, decompression, pool permits, and timeout deadlines. This concentration makes ordering and security review difficult.

## Target structure

Refactor without changing externally visible behavior.

Suggested module layout:

```text
src/client.rs
src/pipeline.rs
src/transport/mod.rs
src/transport/direct.rs
src/transport/proxy.rs
src/transport/connect.rs
src/response_decode.rs
```

Possible responsibilities:

- `client.rs`: public `Client`, `ClientBuilder`, configuration, top-level send entry point
- `pipeline.rs`: request normalization, defaults, cookie/auth application, redirect loop, deadline propagation
- `transport/direct.rs`: direct hyper client path
- `transport/proxy.rs`: HTTP forward proxy path
- `transport/connect.rs`: CONNECT handshake, tunnel TLS, tunneled HTTP execution
- `response_decode.rs`: content-encoding negotiation and decoder wrapping

Keep multipart encoding in `multipart.rs` and proxy configuration parsing in `proxy.rs`.

## Required invariants

The refactor must preserve:

- one logical total deadline across pool acquisition, transport setup, redirects, and buffering
- cookie and auth ordering
- cross-origin credential stripping
- body replayability semantics
- pool lease ownership through live response consumption
- decompression after transport response creation but before Python consumption
- identical error kinds and Python exception mapping

## Tests

Run the entire existing suite before and after extraction. Add focused pipeline-order tests if current behavior is not directly asserted.

## Acceptance criteria

- `client.rs` is reduced to public client/configuration orchestration.
- Transport-specific code no longer lives in the central client implementation.
- No public API behavior changes.
- Existing tests pass unchanged unless assertions are deliberately strengthened.

# Track B: Validate and improve proxy connection reuse

## Problem

Proxy-aware pool keys currently regulate logical concurrency, but it is not yet clear whether HTTP proxy connections and HTTPS CONNECT tunnels are actually reused across requests. The manual proxy send path may create a fresh TCP/TLS connection per request.

## Required investigation

Instrument local proxy test servers to count:

- accepted TCP connections
- CONNECT requests
- forwarded HTTP requests per connection
- tunneled HTTP requests per CONNECT tunnel
- connection closes

Determine behavior for:

1. repeated HTTP requests through one proxy to one destination
2. repeated HTTP requests through one proxy to multiple destinations
3. repeated HTTPS requests through one proxy to one destination
4. repeated HTTPS requests through one proxy to multiple destinations
5. cancellation during a proxied response
6. dropped live response bodies

## Preferred implementation

Where feasible, reuse proxy transports through a dedicated transport pool keyed by:

```text
proxy origin + destination origin + tunnel mode + relevant TLS configuration
```

For HTTP forwarding, a proxy connection may serve multiple destination origins if protocol semantics and authentication policy permit it. The initial implementation may remain more conservative and key by both proxy and destination.

For CONNECT, reuse a tunnel only for the same destination origin and compatible TLS configuration.

If persistent proxy connection reuse is not implemented in this pass, document that the current pool controls concurrency permits but does not imply socket/tunnel reuse.

## Cancellation and shutdown

Verify that cancellation during:

- proxy TCP connect
- CONNECT response wait
- tunneled TLS handshake
- request upload
- response streaming

closes or safely releases the underlying transport and pool permit.

No detached tasks or orphaned sockets should remain.

## Tests

Required:

- connection-count assertions for repeated proxied requests
- tunnel-count assertions for repeated CONNECT traffic
- cancellation followed by successful later request
- dropped response body followed by successful later request
- proxy auth does not leak between pooled routes
- direct and proxied requests remain isolated

## Acceptance criteria

- Actual proxy reuse behavior is measured and documented.
- Reuse is implemented or explicitly described as unavailable.
- Cancellation and drop paths release resources deterministically.

# Track C: Add decoded-body resource limits

## Problem

Streaming decompression controls instantaneous buffering, and nesting depth is bounded, but a single compressed response can expand to an arbitrarily large decoded body. Buffered Python APIs can therefore consume excessive memory.

## Required core policy

Introduce configurable decoded-body limits.

Suggested configuration:

```rust
Client::builder()
    .max_decoded_body_size(Some(bytes))
    .max_decompression_ratio(Some(ratio))
```

Request-level override may be added if useful.

Recommended initial behavior:

- `max_decoded_body_size`: hard limit on total decoded bytes
- `max_decompression_ratio`: optional limit comparing decoded bytes to compressed bytes once enough input has been observed
- `None`: unlimited

Choose conservative Python defaults only if compatibility permits. Otherwise expose limits while retaining unlimited behavior initially and document the security tradeoff.

## Streaming behavior

The decoder wrapper must count bytes incrementally and return a dedicated error as soon as a limit is exceeded.

The error must:

- close/discard the underlying response stream safely
- release the pool lease
- map to a specific Python exception
- avoid returning partial decoded content as a successful response

Suggested error variants:

- `DecodedBodyTooLarge`
- `DecompressionRatioExceeded`

## Buffered behavior

`response.content`, `read()`, and ordinary buffered request methods must enforce the same decoded-byte limit.

## Tests

Required for each enabled codec where practical:

- compressed response under limit succeeds
- output exceeding byte limit fails
- extreme expansion ratio fails if enabled
- streaming iterator fails at the boundary
- pool permit is released after limit error
- decompression disabled returns raw compressed bytes without decoded-size enforcement
- nested encodings count final decoded output correctly

## Acceptance criteria

- Decoded output can be bounded independently of compressed input size.
- Streaming and buffered paths enforce the same policy.
- Limit errors are specific and tested.

# Track D: Compression feature-matrix correctness

## Required checks

Verify that `Accept-Encoding` advertises exactly the compiled and enabled decoders.

Test configurations:

```sh
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-deflate
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-brotli
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-zstd
cargo test -p eggfetch-core --all-features
```

Required semantics:

- no compression feature: no automatic `Accept-Encoding`
- one feature: advertise only that encoding
- multiple features: deterministic preference/order
- unsupported response encoding with decompression enabled: documented error or raw pass-through policy
- decompression disabled: preserve `Content-Encoding` and raw bytes
- decoded response removes stale `Content-Length` and updates/removes `Content-Encoding` correctly

## Dependency audit

Confirm `flate2` is feature-gated appropriately. It should not remain an unconditional dependency if only gzip/deflate buffering uses it.

## Acceptance criteria

- All codec feature combinations compile independently.
- Negotiation and unsupported-encoding behavior are deterministic.
- Compression dependencies are optional where practical.

# Track E: Multipart correctness and compatibility edge cases

## Boundary generation

Replace or justify the custom PRNG used for multipart boundaries.

Preferred options:

- use `getrandom` directly
- use an already-present secure/random source with a minimal feature set

Multipart boundaries are not secrets, but collision-resistant generation should avoid custom randomness unless there is a strong dependency reason.

For buffered parts, optionally detect accidental boundary occurrence and regenerate. For streamed parts this cannot be guaranteed without buffering; document the statistical guarantee.

## File and stream behavior

Audit:

- file opened at request construction versus send time
- behavior if file changes between construction and send
- error mapping for missing/unreadable files
- cleanup on cancellation
- large file backpressure
- known-length calculation from file metadata
- redirect replayability for path-backed files

A path-backed file may be replayable by reopening it for each attempt, unlike an arbitrary live stream. Consider introducing a replayable file-body factory rather than treating all file parts as non-replayable.

## Python compatibility

Add or verify support for:

- repeated field names
- empty filenames
- `filename=None` semantics
- per-part headers
- bytes-like objects
- file paths
- binary file-like objects if deliberately supported
- Unicode field names and filenames under a documented encoding policy

Do not add arbitrary Python file-like streaming unless lifecycle and GIL behavior are safe.

## Injection and formatting tests

Test:

- CR/LF rejection in names, filenames, and custom headers
- quoting and escaping of quotes/backslashes
- empty field values
- zero-byte files
- exact terminal boundary formatting
- mixed known and unknown-length parts
- integer overflow in total-length calculation

## Acceptance criteria

- Boundary generation uses a justifiable source.
- File resource lifecycle is deterministic.
- Replayability distinctions are explicit.
- Multipart wire-format edge cases are covered.

# Track F: Cross-feature integration tests

Add a dedicated matrix of subsystem combinations rather than testing each feature only in isolation.

Required scenarios:

1. multipart upload through HTTP proxy
2. multipart upload through HTTPS CONNECT
3. proxy-authenticated multipart upload
4. compressed response through HTTP proxy
5. compressed response through CONNECT tunnel
6. redirect from proxied request to same origin
7. redirect from proxied request to cross origin
8. cookies set on proxied redirect hop
9. auth stripping on cross-origin proxied redirect
10. live streamed compressed response through proxy
11. cancellation during streamed compressed proxied response
12. total timeout during CONNECT plus response decompression

Each scenario should verify headers, body integrity, credentials, pool permits, and error classification.

## Acceptance criteria

- Critical combinations have local deterministic integration tests.
- Failures identify the responsible subsystem clearly.

# Track G: Feature-gated API hygiene

## Required builds

Validate at least:

```sh
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,multipart
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip
cargo check -p eggfetch-core --all-features
cargo test -p eggfetch-core --all-features
```

Verify public re-exports disappear cleanly when features are disabled.

Python bindings should explicitly enable the full intended feature set rather than depending on core defaults accidentally.

## Acceptance criteria

- No optional module leaks unguarded imports or public types.
- Minimal core builds remain small and clean.
- Python feature selection is explicit.

# Track H: Dependency duplication and auditability

## Required audit

Run and inspect:

```sh
cargo tree -d
cargo tree -p eggfetch-core --all-features
```

Focus on:

- duplicate `webpki-roots` versions
- compression-native/build dependencies
- unconditional `flate2`
- `httparse` scope
- random-number dependency for multipart boundaries
- optional dependencies that appear in minimal builds

Where possible, align direct dependency versions with transitive versions to avoid duplicate crates.

Do not force version unification if it requires unsafe downgrades or incompatible APIs; document justified duplicates.

Update `docs/architecture/dependency-policy.md` with current direct dependencies and feature ownership.

## Acceptance criteria

- Duplicate dependencies are reduced or justified.
- Minimal and all-feature dependency trees are documented.

# Track I: Proxy security review

Audit:

- `Proxy-Authorization` redaction in Debug, errors, and Python reprs
- CONNECT authority validation
- malformed proxy response size limits
- maximum proxy response header size
- timeout coverage for connect, CONNECT response, tunnel TLS, upload, and response read
- rejection of proxy URL userinfo if credentials must be configured separately
- NO_PROXY matching for IPv4, IPv6, ports, suffixes, and localhost
- prevention of destination credentials leaking into proxy requests
- correct absolute-form URI generation for HTTP forwarding

Add explicit bounds to proxy response parsing so a malicious proxy cannot send unbounded headers before termination.

## Acceptance criteria

- Proxy parser inputs are bounded.
- Credentials are redacted and scoped correctly.
- NO_PROXY behavior is exhaustively tested.

# Track J: Documentation and observability

Document:

- whether proxy sockets/tunnels are reused
- pool key semantics versus actual transport reuse
- decoded-body limits and defaults
- multipart replayability
- supported feature combinations
- environment-variable proxy policy
- native versus webpki trust-store behavior

Consider adding optional tracing spans behind the existing `tracing` feature for:

- logical request ID
- redirect hop
- direct/proxy route
- pool wait duration
- connect/CONNECT/TLS phases
- bytes uploaded/downloaded
- decompression codec and decoded byte count

Never include credentials, cookies, auth headers, or sensitive query data by default.

# Suggested implementation order

1. Add cross-feature integration tests to establish the current baseline.
2. Extract transport and pipeline modules without behavior changes.
3. Measure proxy connection/tunnel reuse.
4. Fix or document proxy reuse and cancellation behavior.
5. Add decoded-body limits.
6. Tighten compression feature combinations and dependency gating.
7. Harden multipart boundary/file/replay behavior.
8. Complete proxy security bounds and NO_PROXY edge tests.
9. Audit and reduce duplicate dependencies.
10. Update documentation and validation commands.

# Final acceptance criteria

This corrective integration pass is complete when:

- the central client pipeline is split into reviewable modules
- proxy connection and tunnel reuse behavior is measured, tested, and documented
- proxy cancellation/drop paths release all resources
- decompressed output can be bounded
- multipart boundary generation and file lifecycle are hardened
- Q–S cross-feature combinations have deterministic integration coverage
- optional feature combinations compile independently
- dependency duplication is reduced or justified
- proxy parsing and credential handling have explicit security bounds
- documentation accurately describes transport and resource behavior

## Required validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
cargo tree -d

cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,multipart
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip
cargo check -p eggfetch-core --all-features

cd crates/eggfetch-python
maturin develop
python -m pytest
maturin build
```

## Handoff note

Do not begin TLS customization, retries, or HTTP/2 until this pass is complete. Those features will further multiply transport variants and lifecycle states; the current pipeline and transport boundaries should be clean first.
