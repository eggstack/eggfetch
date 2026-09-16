# Proxy TLS Route-Cache Identity Corrective Pass

Planning baseline: `42f6de75360763a3301ed3bdf1ca532ce7f5c0ba` (`main`, 2026-09-16; eggfetch 0.1.4)
Parent program: `plans/post-maintenance-core-integrity-and-verification-program.md`
Reference contracts: native Rust proxy/TLS API, HTTPX 0.28.1, HTTPX2 2.12.0

## Objective

Correct a reusable-proxy-client security/correctness defect: two `TlsConfig` values with different connection-affecting TLS policy must never compare as the same route-cache identity merely because they share the same lazily initialized root-store cache.

The correction must preserve safe connection reuse for unchanged clones, keep cache identity opaque/redaction-safe, and avoid broad public API changes.

## Defect statement

Current `TlsConfig` is `Clone` and contains connection-affecting state including trust-store selection/root material, additive CA roots, explicit crypto provider, client identity, certificate/hostname verification flags, TLS version bounds, and SNI policy. The current crate-private cache identity is only:

```rust
pub(crate) fn connection_identity(&self) -> usize {
    Arc::as_ptr(&self.root_store) as usize
}
```

The root-store `Arc` is an implementation cache, not a complete statement of TLS connection compatibility. A deterministic false equality exists:

```rust
let base = TlsConfig::default();
let strict = base.clone();
let weak = base.clone().danger_accept_invalid_certs(true);
```

`strict` and `weak` retain the same `root_store` `Arc`, while `weak` changes certificate and hostname verification. `ProxyConfig::connection_identity()` incorporates that incomplete TLS identity, and both forward-proxy and CONNECT route keys use proxy identity when selecting cached Hyper clients. Request-scoped proxy overrides make it possible for one `Client` to alternate these policies.

A safe cache may miss reuse. It must never falsely reuse a connection/client established under a different TLS security policy.

## Required implementation

### 1. Reproduce and inventory the defect

Before changing implementation, add a regression proving an unchanged clone and a clone modified with `danger_accept_invalid_certs(true)` currently have the same connection identity.

Audit every `TlsConfigBuilder` input and every consuming modifier. Classify each field as connection-affecting or not. Do not patch only the demonstrated verification toggle while leaving another mutation path with the same aliasing class.

Acceptance:

- [ ] Closure notes contain the field/mutator inventory.
- [ ] At least one baseline-red regression is recorded.

### 2. Replace root-store pointer identity with a complete opaque policy identity

Preferred design: add a crate-private opaque policy token shared by semantically unchanged clones. Any operation that creates a `TlsConfig` with changed connection-affecting semantics mints a new token.

Required properties:

- ordinary `Clone` retains identity;
- each independent builder `build()` may create a fresh identity even for structurally identical policy;
- consuming modifiers that alter connection semantics mint a new identity;
- equality is collision-free for live policy objects; do not reduce security to a non-cryptographic digest if an opaque shared token is simpler;
- token has no public API and no secret-bearing `Debug`/`Display`.

A structural key is acceptable only if it completely represents connection-affecting policy without exposing private key/certificate/provider material.

### 3. Cover all current policy-changing paths

At minimum verify identity behavior for:

- builder construction;
- ordinary clone;
- `danger_accept_invalid_certs`;
- trust-store/custom/additional CA policy;
- explicit crypto provider;
- mTLS client identity;
- min/max TLS versions;
- SNI enablement;
- any other current consuming setter.

Unchanged clones must remain equal; changed connection-affecting policy must not remain equal merely because it shares internal caches.

### 4. Apply the corrected identity consistently

Audit every use of `TlsConfig::connection_identity()`.

At minimum:

- `ProxyConfig::connection_identity()` uses the corrected proxy TLS identity;
- `ConnectRouteKey::origin_tls_identity` uses the same primitive;
- no other cache invents a second pointer shortcut.

Do not add request `total`, read/write timeout, redirect, retry, body, or auth state to TLS identity unless it truly changes physical connection compatibility.

### 5. Add wire-level forward-proxy and CONNECT regressions

Use the existing local TLS/proxy fixtures and one eggfetch `Client`.

For HTTPS-proxy HTTP forwarding:

1. create a request-scoped proxy using a deliberately weak proxy TLS policy and establish a reusable connection;
2. send a later same-route request using a strict proxy TLS policy cloned from the same base;
3. the later request must perform policy-compatible proxy TLS and fail certificate verification against the intentionally untrusted fixture rather than silently reusing the weak route.

Repeat the same security boundary for a compatible HTTPS CONNECT route.

Also test strict -> weak ordering and unchanged-clone reuse. Where practical count physical proxy TCP/TLS connections so the tests prove route compatibility rather than passing for an unrelated reason.

### 6. Add representative field-level identity tests

Assert non-equality across practical connection-policy dimensions:

- verification flags;
- min/max TLS version;
- SNI enabled/disabled;
- different custom/additional roots;
- different client identity when fixtures exist;
- different explicit provider when existing qualification support allows it without changing default dependencies.

## Files expected to change

Likely:

- `crates/eggfetch-core/src/tls.rs`;
- `crates/eggfetch-core/src/proxy.rs`;
- `crates/eggfetch-core/src/transport/connect.rs`;
- `crates/eggfetch-core/tests/proxy_tests.rs`;
- existing TLS tests.

Unexpected public API, Python/CLI/FFI/Node, manifest, or dependency changes require explicit scope justification.

## Validation

Run focused TLS/proxy tests, then:

```sh
./scripts/check.sh
./scripts/check_security.sh
```

Run directly affected HTTPX and HTTPX2 proxy/TLS compatibility tests. Do not renew final Stage C here; the parent closure owns exact-SHA requalification.

## Non-goals

No public cache-key API, global TLS session cache, certificate-pinning expansion, secure-default change, proxy redesign, logical-timeout cache keying, new hashing/UUID dependency, or Node/H3 work.

## Exit criteria

- [ ] Root-store pointer identity is no longer treated as complete TLS policy identity.
- [ ] Unchanged clones remain compatibility-equivalent.
- [ ] Changed connection-affecting policy cannot alias through clone ancestry.
- [ ] Weak proxy TLS followed by strict proxy TLS cannot reuse the weak route as compatible.
- [ ] Forward and CONNECT cached paths are covered.
- [ ] Identity remains opaque and non-renderable.
- [ ] Tier 1 plus focused compatibility/security checks pass.
