# Core Feature, Dependency, and TLS Boundary Hardening

Planning baseline: `445149bd2af3c3ce5c0d34daacbde1bf8de57297` (`main`, 2026-09-11; eggfetch 0.1.3)
Parent program: `plans/embedded-rust-client-footprint-and-routing-program.md`

## Objective

Make `eggfetch-core`'s Cargo feature declarations accurately control the dependencies and behaviors they claim to represent, while preserving the current default client experience and existing protocol/TLS capabilities. Centralize ordinary TLS trust-root construction through the existing `TlsConfig` policy so default, explicit and embedded profiles do not maintain divergent trust logic.

This is primarily dependency ownership and architecture hardening. It is not a mandate to minimize every dependency or split the crate into micro-features.

## Current problem

At the planning baseline, `eggfetch-core` declares:

- `default = ["http1", "tls-rustls"]`;
- `tls-rustls = []`;
- `json = []`;
- `multipart = []`.

However, key transport/TLS dependencies and their H1/TLS feature sets are unconditional in `Cargo.toml`. `hyper`, `hyper-util`, and `hyper-rustls` already enable H1 support directly in the dependency declaration. `rustls`, `tokio-rustls`, `rustls-native-certs`, `webpki-roots`, and PEM support are unconditional. The feature documentation explicitly says `--no-default-features` is a supported compile check rather than a no-network/minimal build.

This makes the feature surface less useful for embedded Rust consumers and weakens the repository's stated policy that optional capabilities should remain optional where practical.

There is also duplicated TLS trust-root policy. `TlsConfig` already models `NativeWithWebPkiFallback`, `NativeOnly`, `WebPkiOnly`, and custom roots, while the standard Hyper connector separately tries `hyper-rustls` native roots and falls back to WebPKI roots. Those two paths should not independently define trust semantics.

Finally, some dependencies are currently unconditional even though their principal use is behind a named feature. The clearest known example is `getrandom`, which is used by multipart boundary generation while `multipart` itself is optional.

## Design principles

1. Preserve ordinary defaults. A user depending on `eggfetch-core = "0.1.x"` without custom feature selection should continue to receive the documented ordinary H1 + Rustls client with secure verification and the existing trust behavior.
2. Make feature names semantic. A named feature should enable real capability/dependency behavior, not exist only as documentation metadata.
3. Prefer a few meaningful feature boundaries to a proliferation of tiny internal flags.
4. Do not feature-gate dependencies that are truly foundational to every supported network path merely to reduce a crate-count metric.
5. The same trust policy should build standard/direct/proxy/H3 TLS configurations wherever semantics are shared. Route-specific differences such as proxy endpoint identity remain explicit.
6. Do not weaken TLS verification, root handling, ALPN, SNI, client certificate, or custom CA behavior.

# 1. Inventory actual dependency ownership

Produce a source-backed table covering every direct dependency of `eggfetch-core` with:

- modules/use sites;
- whether the dependency is needed by ordinary H1 cleartext operation;
- whether it is needed by Rustls TLS;
- whether it is needed only by a named optional feature;
- whether a dependency feature is currently enabled unconditionally despite corresponding to an eggfetch feature;
- whether moving it behind a feature would cause unsupported combinations or excessive conditional-compilation complexity.

At minimum inspect:

- `hyper`, `hyper-util`, `hyper-rustls`;
- `rustls`, `tokio-rustls`, `rustls-native-certs`, `webpki-roots`, `pem-rfc7468`;
- Tokio feature selections (`rt`, `net`, `time`, `sync`, `macros`, `fs`, `io-util`);
- `getrandom`;
- `httparse`, `httpdate`, `base64`, `tower-service`;
- DashMap and futures dependencies;
- all proxy/compression/cookie/multipart/H2/H3-specific crates.

Do not infer removal merely because a use is sparse. Confirm runtime and test/build requirements.

Acceptance:

- [ ] Every direct core dependency has a documented owner/use rationale.
- [ ] Known optional-only dependencies are identified explicitly.
- [ ] No dependency is proposed for gating if doing so would silently break a supported default path.

# 2. Define a truthful coarse feature model

Refine features around capabilities, not individual crates. The target shape should be equivalent to the following concepts, though exact names/wiring may change if current public compatibility requires it:

- `http1` — owns H1 support in Hyper/Hyper-util/Hyper-rustls where applicable;
- `http2` — remains opt-in and owns H2-specific dependencies/features;
- `tls-rustls` — owns Rustls-based HTTPS support and the dependencies required for Rustls transport/configuration;
- `tls-native-roots` (or equivalently clear naming) — owns native/system trust-store loading while depending on `tls-rustls`;
- bundled/WebPKI roots remain available to `tls-rustls` so a deterministic Rustls-only embedded profile can operate without native-root loading;
- `multipart` owns boundary-generation randomness if no other unconditional use requires it;
- existing proxy/cookie/compression/tracing/H3 gates remain capability-oriented;
- `json` remains reserved for the next child plan rather than being partially implemented here.

The ordinary default feature set should retain the current trust behavior. If the project chooses `default = ["http1", "tls-rustls", "tls-native-roots"]`, document why native roots stay default and WebPKI stays available as fallback/explicit policy.

`--no-default-features` does not need to become a useful no-network utility library if doing so would distort the crate, but a consumer selecting `http1` only or `http1,tls-rustls` should pay only for the corresponding supported network capabilities as far as practical.

Acceptance:

- [ ] Feature names map to real dependency or behavior boundaries.
- [ ] Default behavior remains secure and compatible.
- [ ] Unsupported combinations fail clearly at compile time or construction rather than silently downgrading.
- [ ] Feature relationships are encoded explicitly (for example H3 -> TLS if required), not only documented in prose.

# 3. Centralize TLS root policy

Use `TlsConfig`/`TrustStore` as the authoritative trust-policy model for ordinary origin TLS as well as explicit configurations.

Refactor standard connector creation so it does not maintain an independent `with_native_roots()` -> `with_webpki_roots()` policy separate from `TlsConfig`.

Required invariants:

- default still prefers native roots and falls back to packaged WebPKI roots only when the native store cannot be constructed/populated;
- certificate-chain or hostname verification failure never triggers fallback to a different root store;
- `NativeOnly`, `WebPkiOnly`, custom CA, mTLS, version bounds and verification settings retain their documented semantics;
- the standard Hyper connector and specialized direct connector receive equivalent origin TLS policy;
- proxy endpoint TLS remains logically separate from destination/origin TLS but should reuse the same root-building primitive when policy is equivalent;
- H3/QUIC keeps TLS 1.3/ALPN requirements while sharing trust roots and verification semantics where appropriate.

Avoid creating a generic TLS abstraction layer larger than needed. Reuse existing `TlsConfig` builders/helpers.

Acceptance:

- [ ] There is one authoritative implementation for native/WebPKI/custom root-store construction.
- [ ] The standard connector no longer defines a separate fallback policy.
- [ ] Existing TLS tests remain green and new tests compare default standard/direct trust semantics.
- [ ] Native-root absence is distinguished from certificate verification failure.

# 4. Gate genuinely optional dependencies

Move dependencies behind existing/new coarse features where ownership is unambiguous.

Required known case:

- `getrandom` should be enabled by `multipart` if multipart remains its only production use.

Audit other candidates found in step 1. Examples to evaluate rather than assume:

- `rustls-native-certs` behind the native-root feature;
- PEM parsing if custom CA/client identity support can be gated without fragmenting core TLS API excessively;
- Tokio `fs` if no core default path requires it;
- proxy-specific parsing/helpers if they are currently unconditional despite proxy-only use.

Do not feature-gate tiny ubiquitous dependencies if the resulting `cfg` surface makes correctness harder to audit than the dependency cost justifies.

Acceptance:

- [ ] Every changed dependency gate has a corresponding feature-matrix compile/test case.
- [ ] Optional feature disablement removes the corresponding dependency from `cargo tree` where technically practical.
- [ ] No default public behavior disappears accidentally.

# 5. Keep the feature matrix bounded and useful

Update the repository's existing feature validation rather than creating an exhaustive combinatorial matrix.

At minimum validate:

```text
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,tls-native-roots
cargo check -p eggfetch-core --all-features
```

Adjust exact combinations if cleartext-only operation without `tls-rustls` is intentionally unsupported; if so, encode/document that contract rather than allowing misleading compilation.

Retain existing targeted proxy/compression/H3 checks where those features are supported.

Do not add a CI job for every possible cross-product.

Acceptance:

- [ ] Existing Tier 2 feature validation reflects the supported matrix truthfully.
- [ ] `docs/architecture/feature-flags.md` can describe each combination without caveats that contradict Cargo behavior.

# 6. Dependency duplication and MSRV audit

After refactoring, inspect `cargo tree -d` and `cargo tree -e features` for the core profiles.

Do not alter dependencies merely to eliminate every duplicate. Correct duplicates that are directly caused by avoidable feature wiring or obsolete dependency ownership; leave justified major-version divergence documented.

Confirm the refactor does not unintentionally raise the workspace MSRV. If a dependency update would be desirable but requires an MSRV decision, separate that decision from this plan unless required to implement the feature boundary safely.

Acceptance:

- [ ] No new unexplained major-version duplicate is introduced.
- [ ] Core minimal/default feature trees are captured for the later footprint plan.
- [ ] Workspace MSRV remains truthful.

# 7. Focused regression tests

Add tests for:

- default trust policy constructing with current secure semantics;
- explicit WebPKI-only policy;
- native-only behavior when the native store is unavailable (where deterministically testable by helper injection/unit boundary rather than depending on host configuration);
- custom CA and client identity paths after feature refactor;
- standard and specialized direct connectors using equivalent origin trust policy;
- H1 without H2 does not advertise H2;
- H2 feature behavior remains unchanged;
- multipart build/tests compile only when multipart dependencies are enabled;
- default Python/CLI builds retain the features they require.

Avoid host-dependent assertions that make CI flaky.

# 8. Documentation touched by this plan

Update only the architecture/docs necessary to keep executable changes truthful during implementation:

- `docs/architecture/feature-flags.md`;
- `docs/architecture/dependency-policy.md`;
- TLS architecture documentation where root-policy ownership changes.

The final broad documentation and plan-index reconciliation belongs to the program's docs-only closure plan.

## Non-goals

- no CodeGG changes;
- no native JSON implementation in this plan;
- no pinned/static resolution implementation in this plan;
- no removal of system roots from default users;
- no TLS backend other than Rustls;
- no OpenSSL/native-tls addition;
- no generic resolver framework;
- no dependency rewrite solely to match another repository's versions;
- no large CI expansion.

## Required validation

During implementation:

```sh
./scripts/check.sh
```

Run affected feature checks and targeted TLS tests directly. At plan closure, run `./scripts/check.sh extended` if existing prerequisites are available. Do not renew HTTPX exact-SHA qualification here; later executable child plans intentionally follow.

## Exit criteria

- [ ] Cargo features own meaningful capability/dependency boundaries.
- [ ] Default users retain current documented behavior.
- [ ] TLS trust-root policy is centralized through one authoritative core path.
- [ ] Native roots are independently selectable from Rustls/WebPKI operation where practical.
- [ ] Multipart-only randomness is not unconditional without justification.
- [ ] Supported minimal/default profiles compile and pass targeted tests.
- [ ] Feature/dependency docs match the implementation.
- [ ] No compatibility ledger is advanced yet.
