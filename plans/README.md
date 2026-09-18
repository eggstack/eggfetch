# eggfetch Plan Index

This directory contains active implementation plans, live qualification/status records, and historical implementation records. Completed plans are non-normative unless another current document explicitly says otherwise. Verification and release policy remain governed by `docs/verification-policy.md` and `docs/releases/process.md`.


## Active — Python 3.15 PyPI wheel production (2026-09-18)

Plan: `python-3.15-pypi-wheel-production.md`

Status: implementation complete, pending build-only qualification. The
matrix now builds CPython 3.10–3.15 on Linux x86_64, macOS arm64, and
Windows x86_64 (18 wheels + 1 sdist = 19 distributions); the coverage
validator, package classifier, and release documentation match. Tier 1
and package validation are green locally. The required `publish=false`
18-wheel rehearsal has not been dispatched yet — dispatch it from the
implementation commit before claiming 3.15 support in a published
release (see the plan's Closure record). Python 3.15 is still
prerelease at implementation time, so the workflow must use bounded 3.15-only prerelease fallback that naturally
selects stable 3.15.x after GA without changing the 3.10–3.14 behavior.

## Completed corrective — total deadline across response body lifecycle (2026-09-18)

Parent plan: `total-deadline-response-body-lifecycle-corrective.md`

API-compatibility corrective:
`total-deadline-response-body-api-compatibility-corrective-pass.md`

Final closure plan:
`total-deadline-final-proof-qualification-release-closure.md`

Original planning baseline:
`60a6e2b384519507e04cf296ffd484388e872e47`.

Behavioral implementation:
`dd52f8c4a8d54fbaa403a7995a41bf60b235f7ac`.

Public-API compatibility correction:
`2c68b44176b3a189e1f1fdc6586d8678a0945284`.

Final executable/test freeze:
`82f3f38631b44a9a5c5ec5b40790e5015aeb40f8`.

Status: implementation, release qualification, and coordinated crates.io
publication complete. The total deadline now spans response-body EOF/trailers
for high-level and native frame-preserving responses. Read remains
first-poll/per-chunk inactivity; Total never resets and wins ties.
`ResponseBody`'s published exhaustive variant shape is restored (no public
timeout fields, no `#[non_exhaustive]`), timeout metadata lives behind the
private `PoolGuard` response lifecycle, `BodyTimeoutStream` is the single
high-level owner, and redirect/retry remaining-budget plus the strengthened
native lease-release proof (timed-out body kept alive while the second request
reaches 200 OK) are green. Historical baseline-red (`60a6e2b3` returned
`Ok(b"")` past total) and three-state public-shape evidence (`60a6e2b3`
green / `dd52f8c4` E0027 red / freeze green) are recorded. Tier 1, extended
(incl. Rust 1.89.0 MSRV), package, security, both API oracles (71 / 79, zero
unexplained/stale/resolved-active), and three consecutive 1,871-test
compatibility passes renewed Stage C on the freeze for HTTPX 0.28.1 and HTTPX2
2.12.0. HTTP/3 and Node remain experimental.

No new bounded-body API was added. `max_decoded_body_size` remains the
authoritative stream-level bound for identity and decoded compressed bodies
when `Content-Length` is absent or false.

Coordinated version 0.1.7 (release commit `43c3b31`, tag `v0.1.7`) is published
to crates.io for all six crates (http-connect, core, cli, ffi, python, node —
each verified via `cargo search`); PyPI wheel publication remains a manual
`pypi.yml` dispatch on the tag and was not run in this pass. Remote CI is green
on both the docs closure head (`842a3c35`, run 35377996077) and the release
commit (`43c3b31`, run 35385440508). Downstream consumers should use
`eggfetch-core 0.1.7`: `Timeout.total` spans response-body EOF/trailers,
`max_decoded_body_size` remains the hard bound for unknown/false
`Content-Length` metadata bodies, the public `ResponseBody` field shape is
compatible with 0.1.6, there is no new HTTPX total semantic, and there are no
new dependency/MSRV/feature requirements beyond the release notes.

## Completed — linked binary footprint reduction (2026-09-17 → 2026-09-18)

Handoff program: `linked-binary-footprint-reduction-program.md`

Planning baseline: `6093a66959165f132f02102ffb727ac3e710917c`
(eggfetch-core 0.1.6 after the completed native pool-map and native URI
dependency-separation work).

Status: implementation complete (material linked-footprint improvement).
The lean `standard-http1` + `tls-rustls` profile closes the measured
stripped delta vs aligned reqwest to +32 KiB (+1.0%) on
x86_64/thin-LTO (full compat +590 KiB); `eggfetch_core` .text −49%.
Existing default/`http1`/`http2`/`native-http1`/`native-http2`/Python/CLI/FFI/
Node/HTTPX/proxy/retry/redirect/auth/TLS/H2/H3/cookies/compression/multipart
behavior unchanged. See the program file and child-plan closure records;
`docs/architecture/embedded-footprint.md` is the single authority for bytes.
Tier 1 green locally + CI; extended/package/security/exact-SHA compat renewal
remain release-time gates per `docs/verification-policy.md`.

Execution order:

1. `linked-byte-baseline-and-attribution.md` — remeasure current main using a
   real Gregg-like high-level request path and aligned reqwest/0.1.5/native
   controls; collect stripped bytes plus crate/symbol attribution before
   executable changes.
2. `standard-route-advanced-routing-feature-boundary.md` — add an opt-in
   standard DNS/TCP/TLS route profile that does not compile custom Dialer,
   resolved-target/SNI override, socket-option/local-address, or UDS machinery,
   while existing `native-http1`/`http1` aliases retain all current
   capabilities.
3. `high-level-policy-footprint-feature-boundary.md` — allow the lean
   high-level profile to omit logical retry, redirect-following, and Basic-auth
   Base64 while retaining URL/request/response ergonomics, Bearer auth,
   timeouts, body limits, pooling, TLS, and typed failures.
4. `conditional-tls-and-residual-dependency-footprint-tuning.md` — remeasure,
   then make only evidence-justified residual splits such as proxy-owned
   `eggfetch-http-connect`, TLS PEM/logging ownership, or metrics boundaries;
   explicitly stop rather than proliferating micro-features for negligible
   byte savings.
5. `post-footprint-reduction-requalification-and-closure.md` — freeze one
   final executable SHA, rerun the comparable footprint matrix, prove the
   Gregg-like lean path and full-capability regressions, run Tier 1/extended/
   package/security gates, renew exact-SHA HTTPX/HTTPX2 evidence, and update
   feature/dependency/footprint documentation.

The program is additive by design. Existing default, `http1`, `http2`,
`native-http1`, `native-http2`, Python, CLI, FFI, Node prototype,
HTTPX/HTTPX2, proxy, advanced routing, retry, redirect, Basic/Bearer auth, TLS,
H2/H3, cookies, compression, multipart and other established capabilities
remain available under their current compatibility/default profiles. No second
client, pool, HTTP engine, TLS backend, or downstream-specific Gregg API is
allowed. Binary size is measured under identical release settings; package
count alone is not success evidence.

## Completed — resolved-target route cache and connection reuse (2026-09-17)

Plan: `resolved-target-route-cache-and-connection-reuse.md`

Status: implementation complete. Same logical origin + same ordered physical
snapshot + same SNI reuses one bounded (64-entry) Hyper client via the new
crate-private `ResolvedRouteKey`; Hyper remains the only physical pool (H1
keep-alive / H2 multiplexing). Routing semantics unchanged (no DNS fallback,
same-origin retention, cross-origin fail-closed, proxy/UDS/H3 rejection). Key
matrix, bound, H1/H2 reuse, isolation, redirect, eviction, construction-
failure, and cancellation regressions are in `client.rs` unit tests and
`tests/resolved_route_cache_tests.rs`; loopback churn drops from request-count
to concurrency-level with no RPS regression. Tier 1 green locally; extended,
package, and exact-SHA compatibility renewal remain maintainer-controlled
release gates. Closure evidence is in the plan's implementation record.

## Corrective closure — current-head requalification (2026-09-16)

Plan: `post-core-integrity-current-head-requalification-corrective-closure.md`

Status: complete. Executable freeze:
`1f52d846c186b061481ebb14a7be414f5c78ec7e`; the final profile/ledger/plan-index
closure is documentation-only. This is a qualification corrective only: the
post-freeze reusable route-cache invariant hardening pass (expanded
SOCKS/forward/CONNECT route-key matrices, route/client ownership
documentation, forward-proxy trace-observer non-retention regression) changed
test/source-comment inputs after the `bfda3889` freeze without changing
production behavior, invalidating that binding per the exact-SHA rule. Focused
invariant tests, Tier 1, extended (incl. Rust 1.89.0 MSRV), package, live
security preflight, both API oracles, and three consecutive 1,870-test
compatibility passes renewed Stage C on the new freeze for HTTPX 0.28.1 and
HTTPX2 2.12.0. HTTP/3 and Node remain experimental. Closure evidence is in
the plan and `httpx-parity-correction-status.md`.

## Completed program — post-maintenance core integrity and verification (2026-09-16)

Handoff program: `post-maintenance-core-integrity-and-verification-program.md`

Status: complete. Executable freeze: `bfda3889cbeff5f6fbd98bf3eee12f77fab301c4`;
the final profile/ledger/plan-index closure is documentation-only. All six
executable child plans landed before the freeze: proxy TLS route-cache
identity corrective (opaque per-build token), Hyper idle-pool policy
corrective (pool timer + uniform per-route idle policy), reusable route-cache
invariant hardening (key matrices, deadline ownership, checklist), Hyper
client construction/cache consolidation (central policy/cache owners,
hyper-util pool do-not-adopt recorded), pipeline decomposition
(responsibility-owned `pipeline/` modules), and dependency/validation
reproducibility hardening (cargo-deny all-features + Windows, pinned CI
tooling). Tier 1, extended (incl. Rust 1.89.0 MSRV), package, live security
preflight, both API oracles, and three consecutive 1,870-test compatibility
passes renewed Stage C on the freeze for HTTPX 0.28.1 and HTTPX2 2.12.0.
HTTP/3 and Node remain experimental. Closure evidence:
`post-core-integrity-requalification-and-closure.md` and
`httpx-parity-correction-status.md`. The `bfda3889` binding recorded here is
historical: it was superseded by the corrective closure above after the
post-freeze route-cache invariant hardening pass.

## Corrective — proxy cached total-deadline ownership (2026-09-16)

Plan: `proxy-cached-total-deadline-corrective-pass.md`

Status: implementation and executable validation complete on freeze
`de00479ef1161ec24c7f2c34a1cc95c7872e7643`; compatibility/profile and CI
closure evidence is recorded in that plan and the live parity ledger. The
correction removes request-total state from reusable Hyper forward/CONNECT
connectors while retaining the outer per-dispatch total deadline.

## Completed program — post-audit maintenance, security, and proxy modernization (2026-09-16)

Handoff program: `post-audit-maintenance-security-and-proxy-modernization-program.md`

Planning baseline: `025b1a5a6a94b017b3f3b183c3d562e7ee9bcca7`.

Status: complete. Executable freeze:
d87be1b780a41dc8ff5f3ba8a14f8d74de5814d0; the final plan/profile/index
closure is documentation-only. This program follows the completed post-audit maturation, embedded-consumer,
transport-extensibility, proxy-pinning, and Python interop lines. It does not
reopen Node maturation or HTTP/3 graduation. The new source audit found a
narrower set of ownership/maintenance defects: FFI feature leakage into core
defaults, probable adapter dependency residue, duplicated Python
request-to-core dispatch, stale security/release claims, and one-shot manual
HTTP forward/CONNECT proxy paths that duplicate Hyper framing and cannot reuse
connections.

Execution order:

1. `adapter-feature-and-dependency-boundary-correction.md` — make FFI/core
   feature forwarding truthful, preserve explicit Node transport/TLS behavior,
   audit Python TLS dependency ownership, and delete obsolete core placeholder
   configuration.
2. `python-request-dispatch-consolidation.md` — establish one runtime-neutral
   mapping from normalized Python request state to the core request builder
   while preserving sync/async/top-level runtime semantics.
3. `security-policy-release-and-supply-chain-hardening.md` — remove stale
   advisory ignores, add a fail-closed explicit/release security preflight,
   reconcile security policy with the simplified CI model, and make PyPI
   publication version-tag identity fail closed.
4. `proxy-hyper-pooling-and-upstream-reuse.md` — qualify current hyper-util
   proxy connector primitives, move successful forward/CONNECT HTTP framing
   back under Hyper, and add safe connection/tunnel reuse without regressing
   proxy TLS, pinning, timeout, auth, or error contracts.
5. `post-maintenance-security-proxy-requalification-and-closure.md` — freeze
   one final executable SHA, run feature/dependency/security/proxy proofs plus
   Tier 1/extended/package gates, renew exact-SHA HTTPX 0.28.1 / HTTPX2 2.12.0
   qualification, then perform documentation/index closure.

The proxy plan did not introduce a parallel custom connection pool: Hyper owns
eligible forward/CONNECT reuse and narrow handshake fallbacks preserve the
remaining contracts. The proxy plan must not introduce a parallel custom connection pool before
proving Hyper's existing pool insufficient. Upstream proxy helpers are reused
only where they preserve eggfetch's richer route/security contracts. The
security plan must preserve the single automatic push/PR workflow required by
`docs/verification-policy.md`; live advisory currency belongs to the explicit
security/release path rather than being mislabeled deterministic routine CI.

## Completed corrective — Python PEP 561 method contract (2026-09-16)

The completed Python interop/API-hygiene program remains the historical baseline on executable/package freeze `2281345f3eaf636c62ec21d2c963d6f90ea764a8`, but a follow-up audit found that the typing gate proved exports and exception bases without fully proving public class methods, properties, and semantic return annotations. The corrective line was intentionally narrow and did not reopen the Python runtime architecture. Its executable/package freeze is `c28bbcad6bf9c420721731e8b7a18c2ec1707dd1`.

Execution order:

1. `python-pep561-method-contract-corrective-pass.md` — completed the native
   `_native.pyi` and runtime↔stub member/return drift correction.
2. `post-python-typing-corrective-qualification-and-closure.md` — completed
   source and installed-wheel proof, canonical repository gates, and exact-SHA
   HTTPX 0.28.1 / HTTPX2 2.12.0 qualification renewal.

Do not change runtime behavior merely to fit existing stubs. The stubs must describe the actual native API, and any runtime defect discovered during implementation must be called out explicitly before expanding scope.

## Completed — Python interop/API hygiene closure (2026-09-15)

The native API, neutral SSL boundary, async-body bridge, PyO3/Python matrix,
and PEP 561 typing surface are qualified on executable candidate
`2281345f3eaf636c62ec21d2c963d6f90ea764a8`. HTTPX 0.28.1 and HTTPX2 2.12.0
profiles were renewed after Tier 1, Tier 2, package validation, API oracles,
and three consecutive 1,870-test compatibility passes. See
`post-python-interop-api-qualification-and-closure.md`.

## Completed corrective closure — standard-route DNS provenance (2026-09-15)

Plan: `standard-route-dns-provenance-correction.md`

Status: complete. Executable freeze:
`312cd4402ea2b4b27bf54cf7dc36924a1d449adc`. This is a narrow correction to the completed
native request-failure introspection work. The standard Hyper HTTP/HTTPS route
should preserve typed resolver-failure provenance through a crate-private
resolver wrapper so `send_detailed()` can report `NetworkFailureKind::Dns`
without error-string matching, a new public resolver API, or a transport
rewrite. Hyper-util must continue to own standard TCP/address-selection
behavior; `DirectConnector` must not become the default route merely to obtain
diagnostics.

The implementation should add no dependency or MSRV increase, preserve the
existing public `Error`/`Error::kind()` and ordinary `send()` semantics, and
re-run the repository's existing exact-SHA compatibility process because the
work changes executable core connector/error plumbing. Gregg is motivating
requirements evidence only; no downstream-specific type or adapter belongs in
eggfetch.

Tier 1, extended, clean package validation, dependency/feature checks, focused
resolver/refusal/timeout tests, both API oracles, and three consecutive exact-
SHA 1,870-test compatibility runs passed. The plan contains the implementation
and closure evidence; the live profiles and ledger were renewed on the exact
freeze SHA. The three documented optional local skips remain: unbuilt Node JS
artifact, Rust 1.80/Cargo resolution incompatibility, and absent downstream
artifact manifest.

## Completed — native Tower service adapter (2026-09-15)

Plan: `native-tower-service-adapter.md`

Status: complete. Executable freeze:
`490320f6e99fbb7916280d6bcdd21660bd74f858`. `eggfetch-core` now exports the
cloneable `NativeHttpService` and `Client::native_service()` over the existing
frame-preserving native execution path. The adapter is always ready to accept
requests; origin-pool admission and transport backpressure remain in the
delegated future. Full Tower/Tonic remain outside the core dependency graph.
The standalone fixture now qualifies Tonic 0.14.6 generated-client bounds with
`codegen` only; Tonic `transport`/`Channel` remains disabled.

Tier 1, extended, package, focused feature/dependency checks, the external
Tonic 0.14.6 fixture, and the exact-SHA compatibility validation passed. The bounded
footprint measurement remains **not a footprint win** and no binary-size
reduction is claimed. The plan contains the complete closure record; the Tonic
0.14 corrective closure is recorded in
`tonic-0.14-native-tower-qualification-corrective-pass.md`. This
entry and the compatibility profiles are documentation-only descendants of
the executable freeze.

## Completed program — proxy route pinning and egress interoperability (2026-09-15)

The proxy route-pinning program is complete on the executable freeze recorded
in `proxy-route-pinning-and-egress-interoperability-program.md`. Native Rust
callers can pin proxy peers with `Proxy::resolved_addresses()` and supported
proxied ultimate destinations with
`RequestBuilder::proxy_target_addresses()`. HTTPS CONNECT and local-resolution
SOCKS5 preserve logical URL/Host/TLS identity and never fall back to DNS;
SOCKS5H and plaintext HTTP forward-proxy target pins fail closed. No Egress
dependency was added; external route policy remains above eggfetch and can
use the generic `Dialer` seam.

Executable freeze: `13c4ab4`; the final plan
status/validation closure is a documentation-only descendant.

## Completed — native request failure introspection (2026-09-14)

Plan: `native-request-failure-introspection.md`

Status: complete. The executable implementation is committed at
`e10c4efdff6af9a73a89d015584df15fa6e2900c`; the plan contains the API,
route-coverage, compatibility, and validation closure record. Tier 1,
extended, package validation, and three consecutive exact-SHA 1,870-test
compatibility runs passed. The compatibility profiles were renewed on this
SHA; the closing documentation/profile commit is a docs-only descendant.

Objective: add an opt-in, transport-generic native Rust request-failure surface that can preserve structured DNS/refusal/connect provenance before the existing public `Error::Connect(String)` collapse, without changing the existing public `Error` enum, `Error::kind()` tokens, ordinary `send()` APIs, Python/CLI/HTTPX behavior, or transport policy. The same plan clarifies that `max_decoded_body_size` already bounds unencoded/identity responses as well as decoded compressed bodies; it does not add another body-limit implementation.

The motivating Gregg review is requirements evidence only. No Gregg, EggPool, monitoring, endpoint-status, provider, or other downstream-specific type or adapter belongs in eggfetch. This is a maintenance/ergonomics improvement for unrelated native embedders, not a binary-footprint claim.

## Completed program — native HTTP body and TLS extensibility (2026-09-14)

Handoff program: `native-http-body-and-tls-extensibility-program.md`

Objective: extend `eggfetch-core` for transport-oriented native Rust consumers without changing existing high-level client semantics or adding downstream-specific adapters. Implementation completed in `fdfe060cd7035ddf936d6e0817cb2459f9d3fc0c`; final qualification-only provider/mTLS test hardening completed in `1ea63ba1a4ea81ab548c2a7c9af5563d975793cd`. The program adds a frame-preserving `http_body::Body` interoperability surface, explicit per-`TlsConfig` Rustls `CryptoProvider` selection with provider-neutral mTLS key loading, and explicit additional trust anchors layered on top of the existing base trust policy. Existing `RequestBody`, `ResponseBody`, `TrustStore`, Python/CLI, HTTPX/HTTPX2 and replacement custom-CA semantics remain compatible.

Execution order:

1. `native-http-body-interoperability.md` — add an additive native frame-preserving request/response body surface that reuses the existing transport/TLS/pool engine without exposing Hyper internals or changing public body enums.
2. `tls-crypto-provider-extensibility.md` — allow caller-supplied Rustls `CryptoProvider` policy per `TlsConfig`, remove hard-coded ring key loading from mTLS, and retain current ring/default behavior for callers that do not opt in.
3. `tls-additional-trust-anchors.md` — add a distinct native API for augmenting native/WebPKI/custom base trust with private roots while preserving all existing replacement-style CA APIs.
4. `post-native-http-body-and-tls-extensibility-qualification-and-closure.md` — completed the integration/API-boundedness audit, external-style qualification, dependency/feature checks, exact-SHA HTTPX/HTTPX2 requalification, and documentation/plan closure. Tier 1, extended, package, and three consecutive 1,870-test compatibility runs passed on final qualification tree `1ea63ba1a4ea81ab548c2a7c9af5563d975793cd`. Known optional skips remain explicitly recorded in the program closure record.

The motivating Synvoid evaluation is requirements evidence only. No Synvoid type, feature flag, routing/WAF/site/backend policy, provider-brand product mode, or downstream migration code belongs in eggfetch. The public additions must remain useful to unrelated gateways, service meshes, private-PKI clients, custom-network clients and other native Rust embedders.

## Completed program — extensible embedded transport consumers (2026-09-14)

Handoff program: `extensible-embedded-transport-consumer-program.md`

Objective: let native Rust consumers reuse eggfetch as the single HTTP/TLS engine when they already own the underlying network route, without adding downstream-specific adapters or changing existing retry, pooling, timeout, Python/CLI, HTTPX, or HTTP/3 defaults. The implementation landed in `af03f006f377979550ee6cb96a30b28193c8708d`; qualification and documentation closure completed on executable freeze `43c68bd1bcff45301fc8b6b163b6b6e06d98a786`. This is an ownership/control program, not a binary-size claim; the latest embedded record remains **not a footprint win** against aligned reqwest profiles.

Execution order:

1. `custom-dialer-transport-extension.md` — add a general caller-supplied raw-stream dialer below eggfetch-owned HTTP/TLS, with fail-closed route-combination semantics and no protocol-specific coupling.
2. `strict-underlying-transport-attempt-control.md` — expose Hyper's canceled-request retry policy separately from eggfetch `RetryPolicy` and apply it consistently to every Hyper client route.
3. `physical-connection-admission-and-io-inactivity-guardrails.md` — add opt-in physical live-connection admission and established-transport read/write inactivity controls without changing logical `PoolConfig` or existing `Timeout` meanings.
4. `post-extensible-transport-qualification-and-closure.md` — completed the synthetic external-style consumer, dependency/footprint impact, repository gates and exact-SHA compatibility, then performed documentation/plan closure.

The public API must remain transport-generic. No Eggpool, Eggress, provider/account, SSH, pproxy, Trojan, Shadowsocks or other downstream protocol model becomes part of eggfetch. Downstream migration remains outside this repository.

## Completed program — embedded Rust client footprint and routing (2026-09-12)

Handoff program: `embedded-rust-client-footprint-and-routing-program.md`

Objective: make `eggfetch-core` a cleaner embedded Rust HTTP engine with truthful feature/dependency ownership, first-class caller-supplied resolved-destination routing, native JSON ergonomics, and measured footprint evidence, without compromising default eggfetch capabilities, Python/CLI behavior, HTTPX compatibility, or existing transport/security goals.

Execution order and outcomes:

1. `core-feature-dependency-and-tls-boundary-hardening.md` — done; feature
   ownership and TLS trust construction are explicit while secure defaults
   remain unchanged.
2. `static-resolution-and-pinned-destination-routing.md` — done; typed
   request-scoped static routing preserves logical identity and fails closed
   for incompatible redirects/routes.
3. `native-rust-json-and-response-ergonomics.md` — done; native request and
   response JSON helpers are owned by the opt-in `json` feature.
4. `embedded-consumer-footprint-qualification.md` — done; bounded evidence
   classifies the result as **not a footprint win** against aligned reqwest
   Rustls profiles.
5. `post-embedded-engine-compatibility-requalification-and-closure.md` —
   done on frozen executable/test/fixture SHA
   `22a6f5c0dc0207c1356b6143c0eda3d4075063b0`; Tier 1, extended, package,
   API oracles, and three consecutive full compatibility runs passed.
6. `post-embedded-engine-documentation-and-plan-hygiene.md` — done as the
   documentation/profile/plan-index-only descendant of that SHA. The final
   documentation commit `a7df01df31fd624e3b6fff1db48a0120fd8b9779` passed
   routine remote CI in run `34741393283`.

Plans 1–4 were executable/test/qualification work and invalidated the prior
exact-SHA compatibility evidence. Plan 5 owned the single post-program freeze;
Plan 6 remained documentation/profile/ledger-only. The final exact-SHA
compatibility records are the two versioned profiles under `compat/`.

This program does **not** migrate CodeGG or add a CodeGG-specific facade. The
engine is ready for downstream migration evaluation on ownership/control/API
grounds, but any downstream migration remains outside eggfetch scope.

## Completed corrective closure — H3 post-freeze diagnostics requalification (2026-09-11)

Handoff plan: `http3-post-freeze-diagnostics-requalification-corrective-closure.md`

Trigger: executable/native HTTP/3 diagnostics landed in
`6a0cfd87551b7c634593e7b39cd2fd35d128f727` after the prior qualification
freeze `639bf186a71c054e11278d1b160ffe7a6f172c02`. The prior binding is now
historical. The corrected executable tree was frozen at
`78a77ea153aae239ce7b722aeb9909a87df3bbb5`.

This was a narrow corrective closure, not another H3 feature or graduation
program. It completed with the following sequence:

1. audited the diagnostics delta for boundedness, truthfulness, privacy,
   lifecycle safety, and feature gating;
2. closed the directly-found counter-semantics defect and ran focused H3/
   diagnostics regression gates;
3. froze one clean executable SHA;
4. ran Tier 1, extended, package validation, three consecutive full pinned
   compatibility runs, both API oracles, required downstreams, and existing
   remote CI;
5. rebound both compatibility profiles and the live ledger to that exact SHA;
6. performed a final descendant audit proving all post-freeze changes are
  documentation/profile/ledger-only.

The audit corrected the native diagnostics field to report received UDP
datagrams rather than a path-packet counter, documented the bounded copied
snapshot contract, and added explicit-route and sanitized-close regression
coverage. HTTP/3 remains **experimental**; diagnostics do not satisfy the
missing independent non-Quinn interoperability, independent GOAWAY/drain,
public-origin, realistic impairment, or upstream-risk evidence required for
production graduation. The exact evidence is in
`plans/http3-post-freeze-diagnostics-requalification-corrective-closure.md`
and the live HTTPX ledger.

HTTP/3 remains **experimental** throughout this corrective pass. The new
transport diagnostics improve observability but do not satisfy the missing
independent non-Quinn interoperability, independent GOAWAY/drain, public-origin,
realistic impairment, or upstream-risk evidence required for production
graduation.

## Completed qualification program — HTTP/3 production qualification (2026-09-11)

Handoff program: `http3-production-qualification-program.md`

Objective: close the blockers left by the prior HTTP/3 graduation attempt and make a new evidence-based decision on whether ordinary H3 operation can move from **experimental** to **supported**. This was a qualification/hardening program, not a feature-expansion program.

Execution is complete with a truthful retained-**experimental** outcome. The
child plans remain evidence records, but their unchecked external acceptance
items are not silently treated as satisfied. A future promotion attempt must
supply the missing evidence and perform a new executable freeze and
compatibility requalification.

Execution order:

1. `http3-independent-interop-and-impairment-qualification.md` — build/reuse one implementation-neutral H3 corpus; qualify against at least two independent non-Quinn servers; record public-origin evidence; run realistic loss/latency/reordering/UDP-block/MTU/address-family impairment; prove replay-safe fallback and independent drain/reconnect behavior.
2. `http3-upstream-risk-resource-and-observability-hardening.md` — audit the exact Quinn/h3/h3-quinn/rustls stack and current ordinary-client correctness issues; upgrade/work around only where justified; add targeted regressions; run lifecycle/resource soak; characterize cache pressure; improve real QUIC diagnostics without fabricated response metadata.
3. `http3-production-graduation-and-compatibility-requalification.md` — audit child-plan closure, freeze one exact executable SHA, run full repository/compatibility qualification, make the literal H3 graduation decision, renew HTTPX 0.28.1 and HTTPX2 2.12.0 Stage C on the new executable tree, and perform the documentation/descendant truth pass.

Plans 1 and 2 may overlap where implementation paths do not conflict, but both must close before plan 3 freezes the tree.

Final decision: HTTP/3 remains **experimental**. Deterministic controls and
repository gates are green, but independent non-Quinn interop, independent
GOAWAY/drain, public-origin, realistic impairment, and upstream-risk closure
evidence remain blockers. Missing evidence remains a blocker; it is never
converted into a pass.

The prior HTTPX 0.28.1 and HTTPX2 2.12.0 Stage C profiles were bound to
executable SHA `65beb675a5380d3ff4291da6833b91ebf12c769a`; that binding is
now historical because this program changed tests and qualification tooling.
The profiles were renewed on frozen executable SHA
`639bf186a71c054e11278d1b160ffe7a6f172c02` after three consecutive full
compatibility passes and clean API oracles. That binding is now historical for
current `main` because the later diagnostics commit changed executable code;
the active corrective closure above owns requalification.

Explicitly out of scope for this program: 0-RTT, WebTransport, H3 datagrams, MASQUE/CONNECT-UDP, connection migration, and automatic H3-by-default policy changes.

The current machine-readable evidence ledger is
`http3-independent-interop-and-impairment-qualification-evidence.json`.
The corpus and impairment contracts are under `../qualification/http3/`;
they are qualification inputs, not routine CI gates. The ledger retains the
experimental label until external evidence satisfies the parent gate.

## Completed program — HTTP/3 graduation and next HTTPX compatibility (2026-09-11)

Handoff program: `http3-and-next-httpx-compatibility-program.md`
(completed 2026-09-11 on frozen executable SHA
`65beb675a5380d3ff4291da6833b91ebf12c769a`).

HTTPX 0.28.1 and HTTPX2 2.12.0 had renewed Stage C results on that SHA
(evidence: `httpx-parity-correction-status.md`), but those results are now
historical because the active qualification program changed executable tests
and validation tooling. They were superseded by the qualification on
`639bf186a71c054e11278d1b160ffe7a6f172c02`, which is itself now historical
for current `main` pending the active corrective closure. HTTP/3 retained
experimental with blockers; HTTPX 1.0 remains preview-only. Child plans below
are historical records:

1. `http3-alt-svc-discovery-fallback-and-draining.md` — done.
2. `http3-interoperability-and-production-graduation.md` — done
   (experimental retained with blockers; valid outcome).
3. `httpx2-2.12-profile-and-delta-baseline.md` — done.
4. `httpx2-2.12-core-facade-parity.md` — done.
5. `httpx2-2.12-sse-and-websocket-parity.md` — done.
6. `httpx-1.0-preview-tracking.md` — done (preview-only).
7. `post-next-scope-compatibility-requalification-and-closure.md` — done.
8. `post-next-scope-documentation-and-plan-hygiene.md` — done.

## Normative live compatibility records

`httpx-parity-correction-status.md` remains the live exact-SHA status for
both facades until the active corrective closure updates it. Profiles:
`compat/httpx/0.28.1/profile.toml`, `compat/httpx2/2.12.0/profile.toml`.
Preview: `compat/httpx/1.0-preview/` (unqualified by design; status in
`preview-status.toml`, delta in `preview-delta-0.28.1-vs-1.0.dev6.json`,
notes in `redesign-notes.md`).

### HTTPX 1.0 migration trigger

A future HTTPX 1.0 implementation/qualification program opens only when all
three hold (recorded here and in `plans/ROADMAP.md`; never by renaming the
preview plan into a Stage C plan):

1. upstream publishes an RC with an explicitly frozen public API, or a
   stable 1.0 release;
2. release notes indicate no further major compatibility reset before stable;
3. a fresh delta inventory shows the target is stable enough to justify
   implementation, pinned to the exact RC/stable release.

## Recently completed program — post-audit maturation (2026-09-09)

`post-audit-architecture-and-surface-maturation-program.md` executed and closed:

1. `core-request-and-transport-consolidation.md` — done (`0477d35`)
2. `http3-lifecycle-and-policy-hardening.md` — done (`a505b6c`)
3. `node-binding-maturation.md` — done, experimental outcome (`8139e10`)
4. `native-protocol-observability-and-api-cleanup.md` — done (`363f2e3`)
5. `post-maturation-httpx-requalification-and-closure.md` — done, Stage C renewed on `d034a1005857a7f403222dda4bda5f2f204a44fe`
6. `post-maturation-documentation-and-plan-hygiene.md` — done, documentation-only

The prior Corrective 08 and broad truth-refresh plans remain historical evidence for earlier executable states.

## Historical plans

Other plan files in this directory record prior milestones, corrective passes, validation work and release preparation. Consult them for design history and prior acceptance criteria, but they are not automatically current requirements.

Plan files stay in place; this index is the authoritative navigation layer.
