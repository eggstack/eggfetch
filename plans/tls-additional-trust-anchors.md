# TLS Additional Trust Anchors

Planning baseline: `45c08e0e7587eb1e8f713d49e6f7902478c27a35` (`main`, 2026-09-14; eggfetch 0.1.4)
Parent program: `plans/native-http-body-and-tls-extensibility-program.md`
Status: complete; implemented and frozen in `fdfe060`

## Objective

Add an explicit native Rust API for augmenting the selected TLS trust store with additional CA certificates while preserving every existing replacement-style custom-CA behavior.

This addresses a common private-PKI requirement: trust the ordinary native/WebPKI public roots *and* one or more private enterprise/service roots. Callers should not need to reconstruct eggfetch's native-root loading, WebPKI fallback and route-specific TLS wiring merely to add a private anchor.

The change must be additive and generic. Existing `TrustStore::Custom`, `ca_certificate_path`, `ca_certificate_pem`, `ca_certificate_der`, and `add_ca_certificate_path` semantics remain replacement-style and continue to mean that the configured custom set is the base trust store.

## Current-state findings

On the planning baseline:

- `TrustStore` is a public enum with `NativeWithWebPkiFallback`, `NativeOnly`, `WebPkiOnly`, and `Custom(Vec<CertificateDer<'static>>)`. Adding a new enum variant would create avoidable source compatibility churn for downstream exhaustive matches.
- `TlsConfig` currently owns `trust_store` and `custom_ca_roots` plus a lazily cached `RootCertStore`.
- `ca_certificate_path`, `ca_certificate_pem`, and `ca_certificate_der` replace the selected trust store by switching to `TrustStore::Custom`.
- `add_ca_certificate_path` appends to the replacement custom set; despite the name, it does not augment native/WebPKI roots.
- `build_root_store_uncached()` first selects exactly one base policy and returns that root store.
- Native root policy is fail-closed for `NativeOnly`; `NativeWithWebPkiFallback` falls back to packaged WebPKI only when native roots are unavailable/empty.
- The completed TLS boundary hardening centralized these semantics so the new capability should extend this one root-store construction path rather than add route-local certificate logic.

## Core design decision: separate base trust policy from additional anchors

Do not change existing methods' meaning and do not add a `TrustStore` variant.

Add a separate private collection on `TlsConfig`, for example:

```rust
pub struct TlsConfig {
    trust_store: TrustStore,
    additional_ca_roots: Vec<CertificateDer<'static>>,
    // existing fields...
}
```

and additive builder methods with semantics clearly distinct from replacement CA methods. Candidate names:

```rust
TlsConfigBuilder::additional_ca_certificate_path(...)
TlsConfigBuilder::additional_ca_certificate_pem(...)
TlsConfigBuilder::additional_ca_certificate_der(...)
```

The exact naming can be adjusted for consistency, but avoid overloading `add_ca_certificate_path` with a new meaning: existing users may rely on its current custom-replacement behavior.

Root construction becomes conceptually:

```text
select/build base TrustStore exactly as today
                |
                v
      overlay additional roots
                |
                v
       final RootCertStore
```

## 1. Preserve base trust-store semantics exactly

Additional anchors must not redefine the selected base policy.

### `NativeWithWebPkiFallback`

Attempt native roots exactly as today. If native roots are unavailable or empty, select WebPKI fallback exactly as today. Only after that selection succeeds, overlay additional anchors.

### `NativeOnly`

Native roots must still be available. If native loading fails or produces no usable roots, return the existing TLS configuration error even when additional anchors were provided. "NativeOnly + extras" means native roots are mandatory plus extras; it does not mean "prefer native but private roots are enough."

### `WebPkiOnly`

Start with packaged WebPKI roots, then overlay additional anchors.

### `Custom`

Start with the existing replacement custom root set, then overlay explicit additional anchors. This permits callers to compose two intentional private sources without changing what `Custom` means.

An empty custom base remains invalid under the current policy; additional roots must not silently rescue an invalid `TrustStore::Custom([])` configuration.

## 2. Add path, PEM and DER forms consistently

Mirror the useful input forms already available for replacement CAs so callers do not need to parse certificates themselves.

Requirements:

- path input supports the same file/directory behavior and deterministic directory ordering as existing CA loading;
- PEM input accepts certificate sections and rejects malformed/unterminated PEM according to existing parsing policy;
- DER input accepts one or more raw X.509 certificates;
- empty explicit inputs fail with clear `CaBundle`/TLS configuration errors unless an existing directory policy deliberately treats an empty directory as no-op;
- do not add environment-variable or automatic filesystem discovery to this API;
- do not expose certificate contents through `Debug`/logs.

Prefer factoring common certificate-loading helpers so replacement and additional APIs share parsing behavior rather than maintaining two PEM implementations.

## 3. Deduplicate without changing trust meaning

When additional anchors duplicate certificates already present in the base or earlier additional sources, ignore the duplicate rather than treating it as an error.

Use DER byte equality for deterministic deduplication. Do not introduce X.509 semantic parsing solely to detect equivalent-but-differently-encoded certificates.

The number of roots is normally small enough that a straightforward bounded comparison is acceptable; do not add a hashing dependency solely for this.

If Rustls `RootCertStore` does not expose existing DER values needed for cross-base deduplication, duplicate insertion may be allowed when Rustls safely tolerates it. In that case, deduplicate the explicit additional collection itself and document that base-vs-additional duplicate elimination is best-effort. Correct trust semantics matter more than introducing invasive root-store introspection.

## 4. Fail closed on invalid additional anchors

Additional trust is security-sensitive configuration and must be validated before network I/O where possible.

Requirements:

- malformed PEM/DER input returns an error rather than being silently skipped;
- if one input bundle contains both valid and invalid certificate blocks, do not silently install only the valid subset unless that is already the documented replacement-CA policy; keep both paths consistent;
- unsupported non-certificate PEM blocks may follow existing warning/ignore behavior, but a bundle that yields zero certificates is an error except for an explicitly documented empty-directory no-op;
- duplicate certificates are not errors;
- configuration errors must identify the source path/type sufficiently for diagnosis without printing certificate bytes;
- no fallback may drop the additional-anchor requirement and retry with only public roots after verification failure.

## 5. Compose with hostname-verification policy

Additional roots change chain trust only. They do not modify logical server identity.

The following must remain independent:

- `verify_certificate = true, verify_hostname = true`: validate chain against base + additional roots and hostname normally;
- chain verification on / hostname verification off: use base + additional roots in the existing hostname-skipping verifier;
- full invalid-cert escape hatch: retain current semantics; additional roots are configured but not consulted while verification is intentionally disabled;
- SNI override: verification uses the configured logical/SNI name according to existing policy, not any certificate metadata inferred from the additional root source.

Do not add "trust this certificate regardless of hostname" convenience behavior to the additional-anchor API.

## 6. Compose with explicit crypto-provider selection

The sibling `tls-crypto-provider-extensibility.md` plan may land before or alongside this one. Root-store composition must be provider-independent.

Requirements:

- root loading/parsing does not assume ring or AWS-LC;
- the final `RootCertStore` is passed into the certificate verifier created with the `TlsConfig`'s selected provider;
- hostname-skipping verification uses the same final composed roots and selected provider;
- changing provider does not change which certificates are trusted, except where a provider rejects an otherwise unsupported signature algorithm through normal verification semantics.

Avoid implementation order coupling beyond the shared `TlsConfig` fields/build path.

## 7. Route consistency

Because `TlsConfig::build_rustls_config()` is intended to be authoritative, additional anchors should require no route-specific API.

Audit at minimum:

- standard Hyper HTTPS;
- specialized direct / resolved-target / SNI routes;
- custom dialer destination TLS;
- UDS TLS where supported;
- SOCKS/CONNECT origin TLS;
- explicit HTTPS proxy endpoint TLS when a proxy `TlsConfig` is supplied;
- HTTP/3's Rustls config builder.

Every route using the same `TlsConfig` must observe the same final root store. If a route bypasses the authoritative builder, fix that generic divergence rather than copying additional-root logic into the route.

Proxy endpoint and origin trust remain intentionally independent. Supplying extra origin roots must not cause a private origin CA to be trusted for an HTTPS proxy endpoint unless that proxy leg receives the same/appropriate TLS config explicitly.

## 8. Keep compatibility facades stable

Do not reinterpret Python/HTTPX `verify`, `SSLContext`, `certifi`, truststore, proxy SSL context or existing custom CA behavior through this new native API.

The additional-root methods are native Rust functionality initially. A later compatibility/API program may expose an equivalent higher-level concept if its reference library has matching semantics.

Requirements:

- no existing Python default begins trusting a private CA merely because an environment variable happens to point to one;
- HTTPX 0.28.1 and HTTPX2 2.12.0 compatibility root behavior remains unchanged;
- existing replacement CA tests remain unchanged except for internal refactoring;
- current default root loading/fallback remains unchanged when no additional roots are configured.

## 9. Focused deterministic tests

Use local TLS fixtures with independently generated public-like and private roots. At minimum prove:

1. default/no-additional configuration is behaviorally unchanged;
2. `WebPkiOnly + private additional CA` retains WebPKI roots and trusts the private server certificate;
3. `NativeWithWebPkiFallback + private additional CA` trusts both the selected base and the private root;
4. `NativeOnly + extras` still fails if native roots cannot be loaded in a controllable test seam;
5. `Custom(base) + additional(extra)` trusts certificates rooted in both explicit sets;
6. `ca_certificate_path` remains replacement-style rather than additive;
7. `add_ca_certificate_path` retains its existing replacement-custom-set behavior;
8. additional PEM, path and DER input forms produce equivalent trust outcomes;
9. multiple additional sources compose;
10. duplicate additional certificates do not fail;
11. malformed additional input fails before network I/O;
12. hostname mismatch is still rejected even when the signing root is an additional anchor;
13. hostname-skipping mode validates a chain against an additional root while intentionally skipping name matching;
14. explicit crypto provider + additional roots works together;
15. custom dialer/resolved-target HTTPS sees the same additional trust roots;
16. origin additional roots are not implicitly reused for an HTTPS proxy endpoint;
17. H2 and, where supported, H3 use the same composed trust store;
18. `Debug` output reports only bounded counts/flags, never certificate bytes.

Where native-root availability is host-dependent, introduce a small internal root-source seam for deterministic unit tests rather than asserting properties of the CI machine's real trust store.

## 10. Documentation

Update at least:

- `docs/architecture/core-tls-proxy-protocols.md` — base vs additional trust model and route isolation;
- `docs/rust/guide.md` — examples for replacement CA versus additional private root;
- `docs/architecture/dependency-policy.md` only if parsing/dependency behavior changes;
- public rustdoc for all new methods, with explicit statements that existing CA methods replace defaults while `additional_*` augments the selected base.

Include a short migration note explaining why `add_ca_certificate_path` was not repurposed: compatibility and semantic clarity.

## Dependency policy

No new runtime dependency should be necessary. Certificate parsing, Rustls root stores and PEM support are already present under `tls-rustls`.

Do not add a full X.509 parser merely to provide richer diagnostics or semantic deduplication.

## Required validation

Run focused TLS/root-store/route tests, then at minimum:

```sh
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,tls-native-roots
cargo check -p eggfetch-core --no-default-features --features http1,http2,tls-rustls,tls-native-roots
cargo test -p eggfetch-core
./scripts/check.sh
```

Do not renew exact-SHA HTTPX/HTTPX2 compatibility evidence in this child plan. The parent closure plan owns the final executable freeze.

## Acceptance criteria

- [x] Existing `TrustStore` enum shape is unchanged.
- [x] Existing replacement-style CA methods preserve their current semantics.
- [x] A distinct native API augments the selected base trust store with additional certificate roots.
- [x] Base policy selection completes before extras are overlaid.
- [x] `NativeOnly` remains truly native-required.
- [x] Invalid additional CA material fails closed.
- [x] Additional roots do not alter hostname/SNI semantics.
- [x] Provider selection and additional roots compose without provider-specific trust logic.
- [x] All TLS routes observe one authoritative composed root-store policy.
- [x] Origin and HTTPS-proxy trust remain isolated unless configured explicitly.
- [x] Compatibility-facade behavior is unchanged by default.

## Non-goals

- no new `TrustStore` variant;
- no semantic change to `add_ca_certificate_path`;
- no environment-driven enterprise CA discovery;
- no certificate pinning/TOFU system;
- no hostname bypass tied to private roots;
- no automatic OS keychain mutation;
- no browser trust-policy implementation;
- no provider-specific certificate store;
- no HTTPX/Python API expansion in this plan.
