# Milestone T Plan: TLS Configuration

## Objective

Expose secure, explicit TLS configuration through `eggfetch-core`, Python, and later the CLI without weakening current defaults. Native/system trust should remain preferred where available, packaged roots should remain a deliberate fallback, and all unsafe verification bypasses must be opt-in and clearly labeled.

## Scope

Implement:

- reusable `TlsConfig`/builder in core
- system roots and packaged-root fallback policy
- custom CA certificates/bundles
- optional verification disable escape hatch
- client certificate and private-key loading
- minimum/maximum TLS version policy
- SNI controls where safely supportable
- Python `verify=` and `cert=` compatibility
- direct and proxied-CONNECT TLS parity
- redacted errors and diagnostics

Do not implement certificate pinning, TOFU, OCSP policy, custom verification callbacks, or browser-grade certificate policy in this milestone.

## Core architecture

Create a dedicated module:

```text
src/tls.rs
transport/tls.rs if connector-specific code is substantial
```

Suggested types:

```rust
pub struct TlsConfig { /* immutable shared state */ }
pub struct TlsConfigBuilder { /* construction */ }
pub enum TrustStore {
    NativeWithWebPkiFallback,
    NativeOnly,
    WebPkiOnly,
    Custom(Vec<CertificateDer<'static>>),
}
pub enum ClientIdentity { Pem { cert_chain, private_key } }
```

`ClientBuilder` should accept a constructed TLS config. Request-level TLS override may be deferred unless transport architecture cleanly supports it.

## Verification policy

Defaults:

- hostname verification enabled
- certificate-chain verification enabled
- native roots preferred
- fallback to packaged roots only when native root loading is unavailable, never after certificate/hostname validation failure

`verify=False` should construct an explicitly unsafe verifier behind a clearly named internal type. Add a warning in Python docs and repr/config diagnostics. Do not silently disable SNI when verification is disabled.

## Custom CA bundles

Support PEM files and in-memory PEM bytes in core if practical. Python target:

```python
Client(verify=True)
Client(verify=False)
Client(verify="/path/to/ca-bundle.pem")
```

Rules:

- malformed PEM fails at client construction
- empty bundle fails
- custom bundle policy must be explicit: replace roots versus augment roots
- recommended initial behavior: custom path replaces default roots, matching common Python-client expectations

## Client certificates

Python target:

```python
Client(cert="client.pem")
Client(cert=("cert.pem", "key.pem"))
```

Support unencrypted PEM private keys initially. Encrypted-key support may be deferred with a clear error.

Validate certificate chain and key parsing at construction. Never include private-key contents or paths containing credentials in debug output.

## TLS versions

Expose Rust builder controls for minimum/maximum versions. Python exposure may wait unless a clean public type is added.

Initial supported versions:

- TLS 1.2
- TLS 1.3

Reject impossible ranges.

## SNI

Keep SNI enabled by default. If exposing disable controls, document that hostname verification and virtual-host routing can break. Prefer not exposing SNI disable in Python during this milestone unless required.

## Proxy integration

HTTPS through CONNECT must use the same `TlsConfig` as direct HTTPS. Remove any separate hard-coded trust-store construction from proxy transport.

Tests must cover custom CA and client cert through CONNECT where feasible using local generated test certificates.

## Error taxonomy

Add stable variants for:

- TLS configuration error
- CA bundle parse error
- client certificate error
- private-key error
- certificate verification failure
- hostname verification failure

Preserve sources but redact secrets.

## Tests

Required:

- default verified HTTPS succeeds against trusted local fixture
- untrusted cert fails
- hostname mismatch fails
- custom CA succeeds
- malformed/empty CA fails
- `verify=False` succeeds against self-signed fixture and is explicitly marked unsafe
- client certificate accepted/rejected by mTLS server
- TLS 1.2/1.3 policy tests
- direct and CONNECT paths use identical TLS settings
- no fallback after validation failure
- debug/error output contains no private key material

## Python bindings

Add `verify` and `cert` to Client, AsyncClient, top-level helpers, and request methods only if request-level overrides are supported. Otherwise expose client-level first and reject request-level kwargs clearly.

Sync/async behavior must match.

## Documentation

Document trust-store precedence, fallback semantics, custom CA replacement behavior, mTLS formats, unsafe verification bypass, and enterprise/private-CA implications.

## Acceptance criteria

- one TLS configuration path is shared by direct and proxied HTTPS
- secure verification remains default
- custom CA and mTLS work
- unsafe bypass is explicit and tested
- Python semantics are documented and parity-tested
- feature-matrix, clippy, tests, docs, and wheels remain green
