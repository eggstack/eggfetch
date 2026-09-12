# Embedded Rust Client Footprint and Routing Program

Planning baseline: `445149bd2af3c3ce5c0d34daacbde1bf8de57297` (`main`, 2026-09-11; eggfetch 0.1.3)
Program opened: 2026-09-12
Motivating downstream: CodeGG evaluation, but all work in this program must remain generally useful to Rust-native eggfetch consumers.

## Objective

Make `eggfetch-core` a cleaner embedded Rust HTTP engine with a truthful minimal feature/dependency profile, a first-class caller-supplied resolved-destination routing primitive, native JSON ergonomics, and empirical footprint evidence, while preserving eggfetch's default capabilities, Python/CLI behavior, HTTPX compatibility surfaces, and existing security/transport goals.

This is an eggfetch product-quality program, not a CodeGG compatibility shim. The implementation must improve or preserve eggfetch as a standalone HTTP client even if CodeGG never migrates.

## Why this program exists

The current 0.1.3 architecture is mature enough for broad use, but the audit identified four concrete gaps for embedded Rust consumers:

1. Cargo feature declarations do not yet correspond cleanly to dependency ownership. `--no-default-features` remains a compile-matrix state rather than a materially minimal transport build; H1/Rustls/root-store machinery is still largely unconditional, and the reserved `tls-rustls` feature owns little dependency behavior.
2. `DirectConnector` already owns DNS -> TCP -> TLS/SNI sequencing, but it always performs its own DNS lookup. There is no typed way for a caller that has already validated/resolved a destination to require connection exclusively to that address set while preserving the logical URL/Host/SNI identity.
3. The native Rust `json` feature is reserved but empty even though JSON request/response ergonomics are a common HTTP-client concern and are already identified in architecture docs as future native behavior.
4. Existing performance/resource benchmarks do not answer the embedded-consumer question: what dependency and stripped-binary cost does a minimal `eggfetch-core` client impose compared with a similarly configured alternative such as reqwest?

The audit also found two supporting cleanup opportunities that belong inside these tracks rather than separate speculative programs: centralize TLS trust-root policy instead of keeping parallel default-connector and `TlsConfig` root selection, and finish feature ownership for multipart-only/randomness and other dependencies found to be semantically optional.

## Architectural invariants

The program must preserve all existing repository invariants:

- `eggfetch-core` remains the sole owner of network I/O.
- Rust remains async-first; Python sync remains a bridge over the Rust async engine.
- Default eggfetch behavior must not lose HTTP/1.1, Rustls TLS, secure certificate verification, normal system-root behavior/fallback, redirects, retries, streaming, pooling, proxies, cookies, compression, multipart, or the protocol capabilities already advertised by the relevant feature sets.
- Python, CLI, FFI, Node prototype, HTTPX 0.28.1 facade and HTTPX2 2.12.0 facade must continue to use the same core engine; no second client stack is introduced.
- Optional capabilities remain feature-gated where the semantic boundary is real. Do not create dozens of micro-features solely to reduce crate counts.
- Security-sensitive routing constraints must fail closed. A caller-supplied resolved destination may never silently fall back to DNS, proxies, H3, or another route that violates the supplied constraint.
- TLS certificate/hostname verification remains tied to the logical server identity, not the selected remote socket address.
- Compatibility claims remain exact-SHA evidence claims. Executable/test changes in plans 1-4 invalidate the current compatibility qualification until plan 5 freezes and requalifies a final SHA.
- Verification remains proportional to the repository's existing policy; no new sprawling CI framework is part of this program.

## Ordered implementation plans

Execute the following plans in order unless a child plan explicitly permits overlap:

1. `plans/core-feature-dependency-and-tls-boundary-hardening.md`
   - make Cargo feature ownership truthful for H1/TLS/root-store/multipart-adjacent dependencies;
   - centralize default TLS root construction through the existing `TlsConfig` policy;
   - retain current default behavior while enabling a materially leaner explicit embedded profile;
   - audit, rather than blindly remove, unconditional Tokio/dependency features.

2. `plans/static-resolution-and-pinned-destination-routing.md`
   - add a typed caller-supplied resolved-destination primitive to the native core;
   - route it through `DirectConnector` without a second DNS lookup;
   - preserve logical URL, Host header, SNI and certificate verification;
   - define strict retry/redirect/proxy/H3 semantics and regression tests for DNS-rebinding/TOCTOU resistance.

3. `plans/native-rust-json-and-response-ergonomics.md`
   - implement the reserved native `json` feature with optional `serde`/`serde_json` dependencies;
   - add idiomatic request serialization and response deserialization helpers;
   - add only narrow response/request ergonomics already supported by core metadata/limits where they reduce downstream boilerplate without becoming a compatibility facade.

4. `plans/embedded-consumer-footprint-qualification.md`
   - create a tiny downstream-style Rust fixture or qualification harness;
   - record dependency/feature shape and stripped release artifact size for representative minimal eggfetch and reqwest configurations;
   - use results to correct feature/dependency choices before claiming footprint benefit;
   - keep footprint qualification manual/bounded rather than a brittle routine CI gate.

5. `plans/post-embedded-engine-compatibility-requalification-and-closure.md`
   - audit closure of plans 1-4;
   - freeze one executable/test SHA;
   - run existing repository/package and HTTPX/HTTPX2 qualification procedures on that SHA;
   - renew compatibility claims only if existing gates pass without semantic drift.

6. `plans/post-embedded-engine-documentation-and-plan-hygiene.md`
   - documentation/registry-only descendant after the executable freeze;
   - reconcile feature/dependency/TLS/routing docs with actual behavior;
   - document the minimal embedded profile and footprint evidence truthfully;
   - correct stale connection-reuse language and register final program status.

## Sequencing rules

Plan 1 establishes the dependency/feature boundary that later work should target. Plan 2 may begin once the direct/TLS connector structure from plan 1 is stable. Plan 3 may overlap late plan 2 work if files do not conflict, but both must close before footprint qualification so the measured profile reflects the intended consumer surface.

Plan 4 is measurement and bounded corrective tuning, not permission to redesign the engine solely to win a byte-count comparison. Any corrective executable change discovered by the measurement must be completed before plan 5 freezes the tree.

Plan 5 is the only plan allowed to renew exact-SHA HTTPX/HTTPX2 compatibility status for this program. Plan 6 must remain documentation/registry-only; if it discovers an executable defect, fix the defect and repeat plan 5 rather than changing code after the freeze.

## Cross-program acceptance criteria

The program is complete only when all of the following are true:

- [ ] The default eggfetch-core feature set preserves the documented 0.1.3 ordinary-client behavior unless an intentional, documented pre-1.0 API correction is required.
- [ ] `tls-rustls` and related root-store features own real optional dependency behavior rather than being mostly declarative aliases.
- [ ] A no-default-features embedded configuration can select only the protocol/TLS/JSON capabilities it actually needs without pulling unrelated proxy/cookie/compression/multipart/H2/H3 dependencies.
- [ ] Multipart-only randomness/dependencies are not unconditional unless another core use independently justifies them.
- [ ] TLS trust-root selection has one authoritative policy path for standard/direct/proxy/H3 contexts where the semantics are shared; intentional leg-specific behavior remains explicit.
- [ ] A caller can provide one or more validated `SocketAddr` destinations for a logical origin and require that request to connect only to those addresses, without a second DNS lookup.
- [ ] Pinned routing preserves logical HTTP Host and HTTPS SNI/certificate identity and fails closed when incompatible with route selection.
- [ ] Retry and redirect transformations preserve or clear resolved-destination state according to explicit tests; cross-origin redirects never accidentally reuse the old destination snapshot.
- [ ] The native `json` feature provides request and response helpers backed by optional Serde dependencies and does not enter the default dependency graph unless intentionally selected.
- [ ] Footprint qualification records actual stripped artifact size and dependency/feature evidence for an embedded eggfetch profile and an equivalently scoped comparison client; documentation does not claim a win that measurements do not support.
- [ ] Existing Tier 1 checks remain green during implementation and affected compatibility tests run with each compatibility-sensitive change.
- [ ] One final SHA passes the repository's existing package/compatibility qualification before compatibility ledgers are renewed.
- [ ] Documentation, feature flags, dependency policy, roadmap and plan index describe the final implementation accurately.

## Explicit non-goals

Do not expand this program into:

- migrating CodeGG or any other downstream to eggfetch;
- a Rust reqwest-compatibility facade;
- provider-specific OpenAI/Anthropic/SSE APIs;
- a public general-purpose transport trait solely for abstraction aesthetics;
- removing DashMap or changing its major version only to align with a downstream dependency graph;
- removing native trust-store support from the default Python/CLI experience;
- changing HTTP/3 from experimental to supported;
- adding a new protocol family, DNS client, resolver cache, DoH/DoT implementation, or service-discovery framework;
- optimizing solely for synthetic binary-size rankings at the expense of correctness, maintainability or existing eggfetch goals;
- new routine CI matrices for byte-size or compile-time measurements.

## Validation policy

Use existing verification conventions. `./scripts/check.sh` is mandatory after executable child-plan changes. Use focused tests for affected feature combinations and transport/TLS/security semantics throughout. `./scripts/check.sh extended` and package validation belong at meaningful closure points rather than after every small edit.

The final exact-SHA compatibility qualification is intentionally deferred until all executable/test work is complete. Missing external prerequisites or unsupported environments must be recorded as such rather than converted into a pass.
