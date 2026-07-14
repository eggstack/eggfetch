# Milestone W Plan: HTTP/3

## Objective

Add optional HTTP/3 support through an isolated QUIC transport without destabilizing HTTP/1.1 or HTTP/2. HTTP/3 is post-MVP and should remain feature-gated, explicitly selected, and independently testable until ecosystem maturity justifies broader defaults.

## Scope

Implement:

- separately gated QUIC/H3 transport, likely Quinn plus `h3`
- protocol selection and fallback policy
- shared TLS trust/client identity configuration where possible
- QUIC connection lifecycle and stream multiplexing
- timeout/cancellation integration
- request/response streaming and backpressure
- protocol-version reporting
- Python advanced opt-in configuration
- interoperability and adverse-network tests

Do not enable 0-RTT by default. Do not silently fall back after security/protocol failures unless the configured policy explicitly permits it.

## Architecture

Create isolated modules/crate if dependency containment warrants it:

```text
transport/http3.rs
# or crates/eggfetch-http3
```

Expose a transport trait/internal interface sufficient for pipeline use without forcing HTTP/3 dependencies into minimal builds.

Suggested policy:

```rust
pub enum ProtocolPolicy {
    Http1,
    Http2,
    Http3,
    Auto { allow_http3: bool },
}
```

Initially require explicit HTTP/3 opt-in or strict selection.

## QUIC lifecycle

Define:

- endpoint ownership
- per-origin connection cache
- idle timeout
- maximum concurrent bidirectional streams
- connection migration policy
- stateless reset handling
- DNS/address-family behavior

Pool documentation must distinguish logical permits, H3 streams, and QUIC connections.

## TLS and certificates

Reuse trust roots, custom CAs, client identities, and verification policy from Milestone T where the QUIC stack allows. H3 uses TLS 1.3 only. Keep ALPN `h3` configuration isolated.

## 0-RTT

Keep disabled initially. A later opt-in must enforce method/idempotency/body-replay rules and document replay risk. Never send auth/cookies or unsafe methods in early data without an explicit expert policy.

## Fallback

Define when Auto may fall back to h2/h1:

- unsupported protocol/Alt-Svc absence may fall back
- certificate/hostname validation failures must not
- malformed H3/protocol errors should not be hidden by fallback unless policy explicitly requests best-effort behavior

Alt-Svc discovery/caching may be deferred. Explicit H3 endpoints are sufficient initially.

## Tests

Required:

- local H3 server request/response
- large streaming upload/download
- concurrent streams
- cancellation and reset
- idle connection cleanup
- TLS custom CA and verification failures
- timeout phases
- multipart/compression/cookies/auth semantics
- fallback policy tests
- malformed frames/connection-close handling
- packet loss/latency simulations where practical
- interop with at least one external implementation in opt-in CI

## Python API

Expose advanced client options rather than overloading `http2`:

```python
Client(http3=True)
```

or a protocol enum. Mark experimental until interoperability and packaging are stable. Ensure optional native dependencies do not complicate default wheels unless deliberately enabled.

## Acceptance criteria

- H3 is isolated behind a feature
- h1/h2 minimal builds remain unchanged
- streaming, deadlines, cancellation, and security semantics match the shared pipeline
- fallback does not mask security failures
- 0-RTT remains disabled or tightly controlled
- protocol support is documented as experimental until release criteria are met
