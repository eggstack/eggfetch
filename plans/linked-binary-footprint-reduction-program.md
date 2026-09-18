# Linked Binary Footprint Reduction Program

Planning baseline: `6093a66959165f132f02102ffb727ac3e710917c` (`main`, 2026-09-17; eggfetch-core 0.1.6)
Program opened: 2026-09-17
Motivating downstream evidence: Gregg's reqwest -> eggfetch-core migration increased its stripped release client binary by roughly 655 KiB while reducing dependency/maintenance surface. The work in this program must remain generally useful to Rust-native eggfetch consumers and must not add Gregg-specific APIs.
Normative verification policy: `docs/verification-policy.md`

## Objective

Reduce the linked binary cost paid by small Rust consumers of `eggfetch-core` without removing, weakening, or changing the default capabilities of eggfetch.

The current repository already completed two useful prerequisites:

- `native-pool-map-dependency-reduction.md` removed the unconditional DashMap closure while preserving pool semantics;
- `native-uri-dependency-separation.md` introduced `native-http1` / `native-http2` transport slices that can omit `url`, IDNA, ICU, and percent-encoding.

Those changes improve dependency ownership, but the native-URI closure evidence showed that removing dozens of resolved packages did not materially reduce a tiny linked artifact. The next work therefore targets *reachable engine code*, not package-count aesthetics.

The intended product result is a new explicit lean embedding profile. Existing `default`, `http1`, `http2`, Python, CLI, FFI, Node prototype, HTTPX compatibility, proxy, H2/H3, custom-dialer, pinned-routing, UDS, TLS, retry, redirect, cookie, compression, multipart, auth, metrics, and other existing capabilities must remain available under their existing compatibility/default profiles.

## Research conclusions

### 1. The remaining footprint delta is primarily linked engine code

Eggfetch's own embedded qualification found the minimal Rustls profile larger than an aligned reqwest profile even though eggfetch resolved fewer packages. Gregg independently reproduced the same order of magnitude after migration.

The prior footprint record attributes the difference to eggfetch-owned client/pipeline/retry/redirect/pool/metrics and TLS-policy machinery rather than one obvious accidental dependency. That diagnosis remains directionally correct, but two later upstream changes invalidate the old exact dependency baseline:

- DashMap has since been removed;
- native URI feature separation has since landed.

Re-measure current `main` before making new executable changes.

### 2. Runtime-optional advanced routes are compiled under the ordinary H1 transport boundary

`ClientBuilder::build()` currently owns standard Hyper construction together with direct/socket-option clients, UDS, custom dialer, SNI caches, resolved-target caches, physical lifecycle policy, and related route machinery under the broad native H1/H2 feature boundary.

A small consumer that only performs ordinary DNS -> TCP -> TLS -> HTTP/1 requests can leave those runtime options unset, but the invoked builder and route-selection code still has to remain capable of constructing and selecting them. Link-time optimization cannot be assumed to erase code that is reachable through runtime configuration.

This is the highest-value architectural boundary to separate.

### 3. High-level retry and redirect are runtime policy, not currently compile-time policy

The ordinary high-level send path routes through retry/redirect orchestration even when:

- retry policy is absent;
- redirect following is disabled.

For small service clients and polling agents, these behaviors may deliberately belong to an outer scheduler rather than the HTTP engine. A new lean profile should be able to omit them while the existing `http1` compatibility/default surface retains them.

### 4. Several smaller dependencies follow disabled behavior but remain foundational today

Candidates that should be measured after the large route/policy split include:

- `getrandom` and `httpdate` used by logical retry policy (and `getrandom` by multipart);
- `base64` used by Basic auth even when a consumer only needs Bearer auth;
- `eggfetch-http-connect` used by built-in proxy CONNECT but currently unconditional in core;
- `pem-rfc7468` / `base64ct` reachable through PEM custom-CA and mTLS convenience parsing;
- `hyper-rustls/logging` in the Rustls feature wiring.

These are not all expected to save meaningful final bytes. They should be split only where measurement and ownership justify the cfg complexity.

## Ordered implementation plans

Execute in this order:

1. `linked-byte-baseline-and-attribution.md`
   - re-run the downstream-style stripped binary comparison on current `main`;
   - reproduce a Gregg-like high-level H1 + Rustls/WebPKI + Bearer + typed-failure profile;
   - use `cargo bloat`/symbol attribution on unstripped companion builds;
   - establish a byte-attribution table before adding feature boundaries.

2. `standard-route-advanced-routing-feature-boundary.md`
   - create a compatibility-preserving feature boundary between ordinary standard DNS/TCP/TLS HTTP transport and advanced routing controls;
   - preserve existing `native-http1`, `native-http2`, `http1`, and `http2` behavior;
   - add a supported lean standard-route profile without a second client/transport stack.

3. `high-level-policy-footprint-feature-boundary.md`
   - make logical retries and redirect-following compile-time-selectable for the new lean high-level profile while retaining existing aliases/defaults;
   - allow Bearer-only auth without pulling Basic-auth Base64;
   - move retry-only dependency ownership behind the retry feature where truthful.

4. `conditional-tls-and-residual-dependency-footprint-tuning.md`
   - remeasure after plans 2-3;
   - make only measurement-justified residual dependency/TLS splits;
   - prioritize proxy CONNECT ownership and PEM/logging boundaries if they remain linked or resolved in the lean profile;
   - stop rather than proliferating micro-features for negligible byte savings.

5. `post-footprint-reduction-requalification-and-closure.md`
   - freeze one final executable/test SHA;
   - compare the final lean profile with current-main and aligned reqwest baselines under identical release settings;
   - run repository Tier 1 / extended / package gates and exact-SHA compatibility renewal required by the affected public feature surface;
   - update footprint documentation truthfully.

## Required feature-design invariants

- Existing feature names remain additive compatibility surfaces. A current consumer using `http1`, `http2`, `native-http1`, `native-http2`, or defaults must not lose APIs or behavior.
- The new lean profile is opt-in. Capability reduction is allowed only because that profile explicitly chooses not to compile the omitted capability.
- Do not add a subtractive `minimal` feature whose presence disables APIs selected by another feature.
- Do not create a second `Client`, second connection pool, or second HTTP/TLS engine.
- Standard H1/H2 physical reuse remains owned by Hyper.
- DNS, Host, SNI, certificate verification, timeout, body-size, typed failure, pooling, and cancellation semantics used by the lean profile remain the same audited implementations used by the full client.
- Existing advanced routes remain available: custom Dialer, resolved-target pinning, SNI override, local-address/socket options, UDS, proxy, H2/H3, and route caches.
- Existing logical retry and redirect semantics remain unchanged when their compatibility features are enabled.
- Default Python/CLI/HTTPX behavior is unchanged.
- Secure TLS verification and the existing default native-root/WebPKI fallback remain unchanged.
- No dependency is replaced by handwritten crypto, URL parsing, Base64, HTTP-date, PEM, or protocol code merely to reduce bytes.
- No new production dependency may be added for footprint reduction.

## Measurement policy

Binary size is an empirical output, not a package-count proxy.

Every executable child plan must record comparable measurements using:

- the same Rust toolchain;
- the same target;
- the same release profile;
- the same LTO/codegen-unit/panic settings;
- the same strip procedure;
- equivalent request/TLS behavior.

For attribution, use an unstripped companion build and tools such as `cargo bloat --crates` / `cargo bloat -n`. These are manual qualification tools, not production dependencies or routine CI gates.

The Gregg-like fixture should cover at least:

- HTTP/1;
- Rustls + WebPKI roots;
- ordinary DNS/standard route;
- redirects disabled;
- no logical retry;
- Bearer auth;
- explicit connect/read/write/pool/total timeouts;
- bounded response body;
- typed DNS / refused / timeout failure inspection;
- streaming or bounded bytes consumption sufficient to keep the production path linked.

Do not claim a reduction based on a construction-only fixture that allows the linker to discard the actual request path.

## Program acceptance criteria

- [x] Current-main linked-byte baseline is recorded after the DashMap and native-URI changes.
- [x] Attribution identifies which eggfetch-owned modules/dependencies materially contribute to a small real request path.
- [x] A new additive standard-route feature profile exists without changing existing feature behavior.
- [x] The lean profile does not compile custom-dialer, pinned/resolved-route, SNI-override, local-address/socket-option, or UDS machinery unless explicitly selected.
- [x] Existing advanced-routing profiles continue to pass their route/security tests.
- [x] A lean high-level profile can omit logical retry and redirect-following engines.
- [x] Retry-only dependencies are absent when both retry and their other legitimate owners are disabled.
- [x] Bearer-only clients can avoid Basic-auth Base64 if measurement justifies that split.
- [x] Proxy CONNECT and TLS convenience dependencies are feature-owned truthfully where practical.
- [x] Gregg-like stripped binary size is materially lower than the 0.1.5/high-level profile baseline or the closure record explains why further safe reduction is not justified.
- [x] No default capability, security property, compatibility claim, or supported adapter behavior regresses.
- [x] One final SHA passes the repository's required qualification gates before compatibility/footprint documentation is renewed.

## Non-goals

Do not:

- optimize solely to beat reqwest by an arbitrary byte threshold;
- remove or weaken existing default features;
- change TLS roots or verification semantics for size;
- remove typed failure provenance;
- remove pool/deadline/body-limit semantics used by small clients;
- replace Hyper, Tokio, or Rustls;
- rewrite advanced routing into a separate transport engine;
- add downstream-specific Gregg/EggPool concepts;
- add routine binary-size CI gates, dashboards, scheduled measurements, or matrices;
- reopen HTTP/3 graduation or Node maturation;
- perform unrelated dependency upgrades.

## Program closure rule

Plans 2-4 may change public feature availability in *new* profiles and executable core code, so they invalidate exact-SHA compatibility evidence. Keep Tier 1 and focused tests green while implementing, but do not repeatedly renew full compatibility profiles after each child plan.

Plan 5 owns the final executable freeze and qualification renewal. After that freeze, only documentation/profile/index changes may land without re-opening the affected gates.
## Program closure (2026-09-18)

Outcome: **material linked-footprint improvement** (see
`post-footprint-reduction-requalification-and-closure.md` closure record and
`docs/architecture/embedded-footprint.md` 2026-09-18 section).

- Baseline + attribution: `linked-byte-baseline-and-attribution.md` (complete).
- Standard-route boundary: `standard-route-advanced-routing-feature-boundary.md`
  (complete; `standard-http1` lean +32 KiB vs reqwest, `eggfetch_core` −49%).
- Policy boundary: `high-level-policy-footprint-feature-boundary.md`
  (complete 2026-09-18; −69 KiB).
- Residual tuning: `conditional-tls-and-residual-dependency-footprint-tuning.md`
  (complete 2026-09-18; proxy-owned CONNECT retained, B–E skipped on evidence).
- Requalification/closure: `post-footprint-reduction-requalification-and-closure.md`
  (complete; Tier 1 + focused checks green, extended/package/full-compat
  renewal deferred to release per `docs/verification-policy.md`).
