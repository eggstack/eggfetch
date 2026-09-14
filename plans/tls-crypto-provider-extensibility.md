# TLS Crypto Provider Extensibility

Planning baseline: `45c08e0e7587eb1e8f713d49e6f7902478c27a35` (`main`, 2026-09-14; eggfetch 0.1.4)
Parent program: `plans/native-http-body-and-tls-extensibility-program.md`
Status: complete; implemented and frozen in `fdfe060`

## Objective

Make Rustls cryptographic-provider selection an explicit, per-`TlsConfig` native capability while preserving eggfetch's current ordinary defaults and public API behavior.

A native Rust caller should be able to supply an `Arc<rustls::crypto::CryptoProvider>` and have that provider govern the TLS configuration consistently, including certificate verification and client-certificate private-key loading, without installing a process-global provider and without requiring eggfetch to add a downstream-specific AWS-LC/FIPS/PQ mode.

The existing default behavior must remain valid: callers that do not select a provider continue to use the current process-default-if-present / ring fallback behavior supplied by the existing `tls-rustls` feature profile.

## Current-state findings

On the planning baseline:

- `eggfetch-core` depends on Rustls 0.23 with default features disabled and `std` enabled.
- `tls-rustls` enables `hyper-rustls/ring`; the ordinary build therefore has the ring backend available.
- `TlsConfig::build_rustls_config()` calls `process_crypto_provider()` and passes the resulting provider to `ClientConfig::builder_with_provider`.
- `process_crypto_provider()` first reuses `CryptoProvider::get_default()` if another component already installed one; otherwise it explicitly installs `rustls::crypto::ring::default_provider()` process-wide and returns it.
- `SingleCertResolver::resolve()` ignores that selected provider for key loading and directly calls `rustls::crypto::ring::sign::any_supported_type`.
- As a result, server-certificate verification can follow one selected process provider while mTLS signing remains hard-wired to ring.
- The provider decision is also process-global unless some other library happened to install a provider before eggfetch initializes TLS.

Rustls 0.23 already exposes the generic mechanisms needed to remove this mismatch: `ClientConfig::builder_with_provider(Arc<CryptoProvider>)` and `CryptoProvider::key_provider`, whose `KeyProvider::load_private_key()` converts `PrivateKeyDer` into the provider's `SigningKey`. The plan should use those contracts rather than switch on provider brand names inside eggfetch.

## Core design decision: explicit provider on `TlsConfig`

Add an optional explicit provider to the TLS configuration. The exact public naming can vary, but the semantic shape should be approximately:

```rust
pub struct TlsConfig {
    // existing fields...
    crypto_provider: Option<Arc<rustls::crypto::CryptoProvider>>,
}

impl TlsConfigBuilder {
    pub fn crypto_provider(
        self,
        provider: Arc<rustls::crypto::CryptoProvider>,
    ) -> Self;
}
```

Do not add a public enum such as `TlsBackend::{Ring, AwsLc, ...}` in this plan. That would make eggfetch responsible for enumerating every Rustls provider and would force new backend dependencies/features into eggfetch merely to express a generic Rustls concept.

Provider precedence must be explicit:

1. if this `TlsConfig` carries an explicit provider, use it;
2. otherwise retain the current process-default provider behavior;
3. if no process default exists, retain eggfetch's current ring fallback for the ordinary `tls-rustls` build.

An explicit provider must not be installed as the process-global Rustls default as a side effect of building or using a client.

## 1. Store provider identity safely and cheaply

`TlsConfig` is cloned and reused across transport constructors, so the selected provider should be stored as an `Arc<CryptoProvider>`.

Requirements:

- cloning `TlsConfig` must not clone provider internals;
- `Debug` must report only bounded non-secret metadata such as `has_explicit_crypto_provider: true`; do not dump the provider structure or implementation-specific key material;
- provider selection should not participate in any public equality/hash contract because `CryptoProvider` is not intended as a stable identity key;
- if client/pool caches are keyed from `TlsConfig` policy, audit whether separately configured clients could accidentally share a Hyper client built with a different provider. Pool/client construction must isolate distinct TLS configs naturally, without pointer-address hashing in public APIs.

If an internal cache genuinely requires stable differentiation, prefer constructing the TLS/Hyper client once per built client/config rather than inventing a public provider fingerprint.

## 2. Separate explicit-provider resolution from legacy/default resolution

Refactor `process_crypto_provider()` (or replace it with a clearer helper) so explicit selection never mutates global state.

Conceptually:

```rust
fn resolve_crypto_provider(
    explicit: Option<&Arc<CryptoProvider>>,
) -> Result<Arc<CryptoProvider>> {
    if let Some(provider) = explicit {
        return Ok(provider.clone());
    }
    legacy_or_default_provider_resolution()
}
```

Requirements:

- no call to `install_default()` occurs for an explicitly supplied provider;
- the no-explicit-provider branch preserves current compatibility behavior unless separate evidence justifies changing it;
- failure to obtain a usable provider remains a TLS-configuration error before network I/O;
- do not silently replace an explicit provider with ring because one cipher/key type cannot be loaded;
- provider resolution must be centralized so origin TLS, proxy endpoint TLS, UDS TLS, direct/SNI/custom-dialer TLS and QUIC configuration cannot independently choose different providers from the same `TlsConfig`.

A later breaking-major release may reconsider the process-global fallback model; that is out of scope here.

## 3. Make mTLS key loading provider-neutral

Remove the direct call to `rustls::crypto::ring::sign::any_supported_type` from `SingleCertResolver`.

Preferred implementation order:

1. determine whether Rustls's ordinary `ClientConfig` builder can install the configured client certificate through `with_client_auth_cert(cert_chain, key_der)` while still preserving eggfetch's current dynamic resolver behavior; if yes, use that standard builder path and let Rustls invoke the selected provider;
2. if the custom resolver remains necessary, pass the resolved provider into `SingleCertResolver` and call `provider.key_provider.load_private_key(key_der)`;
3. preserve the resulting `CertifiedKey` behavior and certificate chain without backend-specific branches.

Requirements:

- PKCS#1, SEC1 and PKCS#8 input parsing semantics remain at least as capable as today;
- unsupported key/provider combinations fail deterministically and do not return `None` in a way that silently looks like "no client certificate configured";
- private key bytes remain redacted from all diagnostics;
- no provider-specific `match` on ring/AWS-LC type names enters the generic resolver;
- existing mTLS callers using the default ring profile continue to work unchanged.

If Rustls's `KeyProvider` returns an error that does not fit the current `ResolvesClientCert` callback well, validate keys eagerly at `TlsConfig`/client construction time and store a ready `CertifiedKey`. Prefer fail-fast configuration errors over handshake-time ambiguity.

## 4. Preserve public TLS policy independently of provider choice

The selected provider changes cryptographic implementation/capabilities, not eggfetch policy. Existing settings retain their meaning:

- `TrustStore` selection;
- hostname verification;
- certificate-chain verification;
- minimum/maximum TLS version;
- SNI enable/disable;
- custom/additional trust anchors;
- client identity;
- HTTP ALPN/version policy.

Provider capabilities may constrain whether a configuration can be built. For example, a caller-selected provider may omit algorithms needed by a certificate/key or may expose a different key-exchange set. Such incompatibilities must fail clearly during configuration/handshake and must not cause fallback to another provider.

Do not add a special `prefer_post_quantum` boolean in this plan. A caller that needs a specific key-exchange policy should construct/configure the Rustls provider according to Rustls's own API and pass that provider explicitly. eggfetch should transport that choice faithfully, not reinterpret it.

## 5. Audit every Rustls construction path

Create one inventory and prove that a `TlsConfig`'s resolved provider reaches every route that uses that configuration.

At minimum audit:

### Standard Hyper TLS

`hyper_rustls::HttpsConnectorBuilder` must receive a completed Rustls `ClientConfig` built with the selected provider. Do not let a convenience constructor rebuild TLS using Hyper-Rustls's compile-time default provider.

### Specialized direct / resolved-target / SNI override

`DirectConnector` TLS handshakes must use the same configured provider.

### Custom dialer

The caller supplies only the raw stream; destination TLS remains eggfetch-owned and must use the selected provider.

### UDS TLS

Where UDS + TLS is supported, the same `TlsConfig` provider must be used.

### Proxy origin TLS

After CONNECT/SOCKS tunnel establishment, origin TLS must use the origin `TlsConfig` provider.

### HTTPS proxy endpoint TLS

When an explicit proxy `TlsConfig` is provided, its own selected provider governs the proxy leg independently of the origin provider. If no explicit proxy TLS config is supplied, preserve existing default proxy TLS behavior.

### HTTP/3 / QUIC

`build_quic_rustls_config()` must use the same explicit provider when possible. Then audit eggfetch's pinned Quinn feature graph and adapter requirements.

Do not assume that enabling an external provider for TCP TLS automatically makes the current Quinn build compatible. The current manifest enables Quinn's `ring` feature. Implementation has two acceptable v1 outcomes:

1. adjust feature wiring so Quinn can accept the same externally configured Rustls provider without adding a mandatory AWS-LC dependency; or
2. reject explicit providers that cannot be represented on the H3 path before QUIC I/O, while H1/H2 remain supported.

Whichever outcome is chosen must be tested and documented. This plan does not require H3 provider parity at the cost of destabilizing an experimental protocol path.

## 6. Cargo-feature and dependency policy

The generic provider-injection API should not itself add an AWS-LC dependency to `eggfetch-core`.

A downstream that wants AWS-LC can depend on a Rustls configuration that exposes `rustls::crypto::aws_lc_rs::default_provider()` and pass that `CryptoProvider` to eggfetch. Cargo feature unification on the shared Rustls version makes the provider implementation available to that downstream build without eggfetch naming the downstream use case.

Keep `tls-rustls`'s existing ring-backed ordinary profile intact unless implementation evidence shows a smaller feature correction is required.

Do not add `tls-aws-lc`, `synvoid-tls`, `fips`, or `post-quantum` feature flags merely for this handoff. Such product-level feature flags are justified only by an independent eggfetch distribution/support requirement.

If Hyper-Rustls or Quinn compile-time feature constraints prevent generic provider injection in a route, document and solve the minimum real constraint rather than broadening feature defaults.

## 7. External-provider qualification fixture

Add a small native fixture or test crate that proves the API using a non-ring provider available in the development environment. AWS-LC is the preferred qualification provider because it exercises a materially distinct built-in Rustls backend and is relevant to real consumers, but the test must remain framed as generic provider qualification.

The fixture should:

1. construct an explicit `Arc<CryptoProvider>` outside eggfetch;
2. pass it through `TlsConfigBuilder::crypto_provider`;
3. connect to a deterministic local TLS server whose trust anchor is configured explicitly;
4. assert successful H1 TLS with the explicit provider;
5. exercise mTLS with a client certificate/private key loaded through the selected provider;
6. prove that the process-global provider was not changed solely by using the explicit provider API;
7. exercise H2 when enabled;
8. exercise custom-dialer or resolved-target TLS to prove route propagation;
9. attempt H3 only if the selected provider is supported by the current QUIC feature graph, otherwise prove the documented fail-before-I/O behavior.

The fixture may carry dev/qualification-only provider dependencies. Do not add them to the ordinary default runtime graph solely to run the fixture.

## 8. Focused deterministic tests

At minimum prove:

1. explicit provider is retained by cloned `TlsConfig`;
2. explicit provider does not call/install a global default as a side effect;
3. no-explicit-provider path retains current ordinary ring behavior;
4. standard HTTPS succeeds with an explicit alternate provider;
5. hostname-verification-disabled / chain-verification-enabled mode uses the selected provider's verification algorithms;
6. full verification-disabled mode reports supported schemes from the selected provider rather than ring;
7. mTLS private key is loaded through the selected provider;
8. a key unsupported by the selected provider fails clearly and never falls back to ring;
9. direct/resolved/SNI routes use the selected provider;
10. custom-dialer HTTPS uses the selected provider after dialing;
11. UDS TLS, where enabled, uses the selected provider;
12. HTTPS proxy endpoint and tunneled origin can use different explicit providers when supplied separate TLS configs;
13. H3 either uses the selected provider correctly or rejects the unsupported combination before QUIC network I/O;
14. existing default TLS tests remain green;
15. `Debug` output does not expose provider internals or key material.

Provider identity assertions should be behavioral where possible. Avoid tests that depend on undocumented pointer layout or debug strings.

## 9. Documentation

Update at least:

- `docs/architecture/core-tls-proxy-protocols.md` — provider selection/precedence and route propagation;
- `docs/architecture/dependency-policy.md` — explain why provider injection does not add every backend to eggfetch;
- `docs/architecture/feature-flags.md` — only if actual feature ownership changes;
- `docs/rust/guide.md` — example constructing an external provider and passing it to `TlsConfig`;
- TLS public rustdoc — explicit provider locality, defaults and security caveats.

Documentation must not claim that eggfetch itself is FIPS validated or post-quantum merely because a caller can provide an AWS-LC or customized provider. Those properties depend on the exact provider build/configuration and are outside this generic API contract.

## Required validation

Run focused TLS/mTLS/provider/route tests, then at minimum:

```sh
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --no-default-features --features http1,http2,tls-rustls
cargo test -p eggfetch-core
./scripts/check.sh
```

Also compile/run the explicit alternate-provider fixture in isolation and inspect `cargo tree -e features` for both the ordinary default profile and the qualification fixture so the ordinary build does not accidentally acquire a new crypto backend.

Do not renew exact-SHA HTTPX/HTTPX2 compatibility evidence in this child plan. The parent closure plan owns the final executable freeze.

## Acceptance criteria

- [x] `TlsConfig` can carry an explicit `Arc<CryptoProvider>` through an additive builder API.
- [x] Explicit provider selection does not install or overwrite the process-global provider.
- [x] Existing no-explicit-provider behavior remains compatible.
- [x] mTLS key loading uses the selected provider's key loader rather than ring directly.
- [x] Verification and signing use one coherent provider per TLS config.
- [x] Every TLS route is audited and either propagates the provider or fails explicitly before I/O.
- [x] No mandatory AWS-LC or downstream-specific dependency enters the ordinary profile.
- [x] An external alternate-provider fixture passes without Synvoid source dependencies.
- [x] Documentation avoids unsupported FIPS/PQ claims.

## Non-goals

- no provider-brand enum in the public API;
- no mandatory AWS-LC dependency;
- no change to the default crypto backend solely for Synvoid;
- no FIPS certification claim;
- no eggfetch-owned PQ policy toggle;
- no process-global provider management API;
- no removal of ring support;
- no H3 graduation;
- no TLS server implementation.
