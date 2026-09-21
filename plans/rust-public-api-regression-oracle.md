# Rust Public API Regression Oracle

Planning baseline: 03ecba973010e2858bf16a2b5f84d51ce70adae4
Parent program: plans/api-preserving-maintenance-and-interop-hardening-program.md
Normative verification policy: docs/verification-policy.md

## Objective

Add reproducible mechanical protection for the supported eggfetch-core Rust API without changing that API.

This plan exists because the Rust surface has grown substantially while routine verification currently emphasizes compilation, tests, rustdoc, feature checks, and downstream behavior rather than exact public-surface equality. Python already has a reviewed manifest and API oracle; Rust should gain equivalent regression protection appropriate to Cargo features and pre-1.0 compatibility policy.

The implementation must detect accidental removals, moves, signature changes, field/variant changes, trait-bound tightening, and feature-profile exposure drift. Because this maintenance campaign requires zero public-surface change, the plan should also flag accidental public additions rather than treating every additive change as acceptable.

## Research constraint

Current cargo-public-api tooling provides exact public API listing/diffing but requires rustdoc JSON generated with a recent nightly toolchain. cargo-semver-checks works with supported stable toolchains and can compare a current crate with a baseline revision, but semver checking alone does not reject every additive public item.

Use that distinction deliberately:

- exact-surface equality is the primary campaign oracle;
- semver checking is a complementary safety check;
- routine Tier 1 must not become dependent on an unpinned moving nightly;
- no new GitHub Actions job is required.

Before committing tooling, verify the exact CLI syntax/version selected by the implementation against the pinned tool version. Do not copy historical commands blindly.

## Part A — freeze supported profile contracts

Capture the public surface of eggfetch-core at the planning baseline for these representative supported profiles:

1. default features;
2. no-default + http1 + tls-rustls;
3. no-default + http2 + tls-rustls;
4. no-default + standard-http1 + tls-rustls;
5. no-default + native-http1 + tls-rustls;
6. all features.

If two profiles produce provably identical public surfaces, the implementation may de-duplicate stored snapshots while retaining an explicit mapping that says which profiles are covered by that snapshot.

Do not include deliberately unsupported arbitrary feature combinations merely to maximize matrix size.

The profile inventory must explicitly include compatibility re-exports whose continued availability matters, including:

- eggfetch_core::request::{ResolvedTarget, TransportHints, NativeRequestOptions} when high-level-url is present;
- root re-exports from eggfetch_core::lib.rs;
- feature-gated Proxy/NoProxy, RetryPolicy, RedirectPolicy, BasicAuth, multipart and TLS items;
- advanced-routing Dialer/SocketOption exposure;
- native service/http_body entry points.

## Part B — add a fast compile-contract layer

Add focused compile-time public-contract coverage that exercises critical import paths and method signatures using only stable Rust/Cargo.

Preferred shape:

- one or more integration-test or compile-fixture sources owned by eggfetch-core;
- feature-gated use-sites for profile-specific APIs;
- explicit assignments/function-pointer/type assertions where they help catch signature drift;
- exhaustive compile controls for frozen public enum/variant shapes where repository policy already promises exhaustiveness.

The fast layer should cover representative contracts rather than attempting to encode every rustdoc item by hand.

At minimum cover:

- Client::new / Client::builder;
- ClientBuilder's currently public configuration methods across their feature gates;
- Client verb/request entry points;
- RequestBuilder build/send/send_detailed and request-scoped transport controls;
- Response status/version/headers/url/body/streaming/network-stream/trailer paths that are public in the selected profile;
- NativeHttpService and Client::execute_http_body;
- Timeout/TimeoutBuilder, Limits, pool and transport lifecycle types;
- auth/retry/redirect/proxy/TLS/multipart/cookie public roots under their features;
- canonical and historical transport-hint import paths.

Integrate the fast checks into existing Tier 1/feature checks only by extending current script commands. Do not add a new CI job.

## Part C — exact public-surface snapshots

Introduce a repository-owned exact API snapshot/diff step for the selected profiles.

Preferred implementation:

- use cargo-public-api or an equivalently mature rustdoc-based exact-surface tool;
- pin the cargo-public-api version;
- pin the nightly toolchain used only for this oracle;
- store normalized text snapshots under a clearly named compatibility/tooling directory such as compat/rust-public-api/;
- record the generating tool version, nightly version, crate/profile, and planning baseline SHA in a small README or snapshot header;
- normalize only known nondeterministic/tool-noise content, never Rust API items.

The snapshot check must fail on both additions and removals during this campaign.

Do not hand-edit snapshots to make a refactor pass. Regeneration is allowed only when a separately approved public API change intentionally changes the contract.

If cargo-public-api proves materially unstable or cannot represent one of the required feature profiles reproducibly, stop and document the blocker before substituting a custom parser. Do not invent a fragile rustdoc-HTML scraper.

## Part D — semver cross-check

Add or document a reproducible cargo-semver-checks comparison against the planning baseline or other explicit baseline revision for eggfetch-core.

Requirements:

- pin the cargo-semver-checks version used by repository validation;
- run on a normal supported stable toolchain, not the exact 1.89 MSRV toolchain unless the selected cargo-semver-checks release explicitly supports it;
- use explicit features/profile arguments where supported;
- keep the exact-surface snapshot as the stronger zero-drift gate for this campaign;
- do not treat cargo-semver-checks silence as proof that no additive API drift occurred.

This may live in extended validation if tool installation/runtime cost is inappropriate for Tier 1.

## Part E — feature/default drift protection

Add assertions that the Cargo feature graph relevant to public exposure is unchanged from the planning baseline.

Protect at least:

- default = http1 + tls-rustls + tls-native-roots;
- meanings of http1/http2 aliases;
- native-http1/native-http2;
- standard-http1/standard-http2;
- high-level-url/logical-retry/redirects/basic-auth;
- advanced-routing and standard-route;
- public optional capability gates currently documented in docs/architecture/feature-flags.md.

Use Cargo metadata or existing adapter/feature tooling where practical. Do not create a second feature-description source of truth.

## Part F — documentation

Update only the documents that describe API/version verification:

- docs/reference/versioning.md;
- docs/verification-policy.md if the new gate becomes normative;
- docs/architecture/build-ci.md if validation behavior changes;
- AGENTS.md only if agents need a new required command.

Do not describe the snapshots as a new supported API. They are verification artifacts for the existing one.

## Focused validation

Before this plan closes, run at least:

- cargo fmt --all -- --check;
- cargo clippy --workspace --all-targets --all-features -- -D warnings;
- the new fast public-contract checks for every selected profile;
- the exact public-api snapshot verifier;
- the pinned semver comparison;
- cargo doc --workspace --all-features --no-deps;
- ./scripts/check.sh;
- ./scripts/check.sh extended if the permanent oracle is integrated there.

Also run the exact Rust 1.89.0 MSRV checks already owned by extended validation. The new API tooling may use a different pinned toolchain when technically required, but it must not weaken the existing MSRV guarantee.

## Acceptance criteria

- [ ] Every selected supported profile has an explicit reviewed public-surface baseline.
- [ ] Routine stable compile-contract coverage catches removal/signature/path drift for critical API items.
- [ ] Exact public-surface validation fails on both additions and removals.
- [ ] The final candidate has zero exact-surface difference from 03ecba973010e2858bf16a2b5f84d51ce70adae4.
- [ ] Existing feature/default meanings are unchanged.
- [ ] Historical compatibility re-export paths compile.
- [ ] No new public item was added merely to make testing or cross-crate sharing easier.
- [ ] No new CI job or matrix was introduced.
- [ ] Tier 1 and extended validation remain green.

## Stop conditions

Do not update a snapshot to bless an accidental API difference.

If an internal refactor genuinely requires changing a public item, stop this maintenance program and open a separately approved API-change plan.

If exact API tooling requires an unbounded nightly dependency or produces unstable output that cannot be pinned reproducibly, keep the fast compile-contract layer, record the tooling blocker, and do not substitute an ad-hoc parser without a separate decision.
