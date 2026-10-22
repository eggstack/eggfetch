# Core Transport and Request Policy Roadmap

Status: active — M008, M009 and M010 closed

Long-term references:

- `plans/000-long-term-specification.md` §4.1, §4.5, §5, §8
- `plans/001-terminology-and-domain-model.md` §2, §4
- `plans/002-long-term-roadmap.md` Phase 0

Related ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0002-pipeline-route-finalize.md`
- `plans/adrs/ADR-0004-proxy-fallback-total-deadline.md`
- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`
- `plans/adrs/ADR-0006-native-transport-failure-classification.md`
- `plans/adrs/ADR-0008-uncompiled-protocol-request-semantics.md`
- `plans/adrs/ADR-0009-compat-policy-authority.md`

## 1. Purpose and ownership boundary

Owns the engine's request lifecycle: pipeline stages, route selection,
retry/redirect reconstruction, preparation, post-transport policy, pool
admission, body lifecycle, Hyper client construction and caching, resolved
routing, and the feature-profile boundaries that compile policy in or out.

Consumes: CONNECT wire bytes from `eggfetch-http-connect`; trust and proxy
mechanics owned with the TLS/proxy workstream. Must not own: adapter parsing,
facade semantics, publication process.

## 2. Work classification

### Invariants

- Single engine owns all I/O; exhaustive typed rebuilds, no silent drops.
- Total deadline owned by the outer dispatch across the `PoolGuard` lifecycle;
  never cached in reusable connectors.
- `ResponseBody` shapes frozen; `BodyTimeoutStream` the single high-level
  timeout owner.

### Capabilities

- Resolved-destination routing, custom dialer, SNI override, UDS route.
- Physical admission and established-I/O inactivity guardrails.
- Strict Hyper stale-idle retry control.

### Infrastructure

- Pipeline decomposition (`prepare`/`route`/`finalize`, `retry`/`redirect`/`lean`).
- Centralized Hyper construction; bounded resolved-route client cache.
- Private module decomposition behind stable public paths.

### Polish

- Route/client ownership documentation, trace-observer non-retention proof.

## 3. Non-goals

- Facade semantics, trust policy, H3 transport internals, publication.
- New public helpers beside contained surfaces; new transports.

## 4. Current state

Pipeline decomposed by responsibility; `select_route()` precedence
unit-tested; reusable connectors carry route policy only; default logical
admission proven inert by measurement; lean profiles compile out
policy/advanced-routing arms and fail closed. Evidence in the legacy plans
below; live behavior qualified on the Stage C freeze (see ledger).

M007 completed the native failure-classification seam accepted by ADR-0006.
High-level detailed sends retain RequestFailure / NetworkFailureKind connection
provenance, while execute_http_body() and NativeResponseBody continue returning
Error and now expose the generic TransportFailureKind classifier without
consumer Hyper/rustls downcasts. The subsystem is back at steady state;
publication is owned by release-verification.

## 5. Target architecture

The §2 invariants hold permanently. M007's evidence-backed, policy-neutral
classifier is closed and leaves Error, NativeResponseBody, timeout ownership,
route policy, and adapters unchanged. Future core-transport changes require
the normal corrective/ADR gates.

## 6. Dependency graph

```text
M001 pipeline decomposition + typed rebuilds (hard predecessor)
    |
    +--> M002 total-deadline lifecycle ownership
    |
    +--> M003 route cache + Hyper consolidation
    |
    +--> M004 feature-profile containment + private decomposition
    |
    `--> M005 embedded transport extensions (dialer/pinned/admission)
```

M002–M005 are soft/parallel after M001 and remain closed. M006 is a bounded
Phase-3 corrective discovered by downstream Windows qualification; it does not
reopen M001–M005 architecture. M007 follows the closed embedded-transport
surface and accepted ADR-0006; it is independent of publication but downstream
exact-pin use requires a later published release.

## 7. Milestones

### Milestone 1 — Pipeline decomposition and typed rebuilds

Class: infrastructure + invariant. Status: closed.

Objective: one staged pipeline with exhaustive retry/redirect reconstruction.

Legacy: `plans/pipeline-policy-and-transport-dispatch-decomposition.md`,
`plans/core-request-and-transport-consolidation.md`.

Exit conditions: all hops rebuild through typed helpers; one
post-transport policy; forward H1-only. Met.

### Milestone 2 — Total-deadline response-body lifecycle

Class: invariant. Status: closed.

Objective: total spans EOF/trailers; public shapes restored; native
lease-release proven.

Legacy: `plans/total-deadline-response-body-lifecycle-corrective.md`,
`plans/total-deadline-response-body-api-compatibility-corrective-pass.md`,
`plans/total-deadline-final-proof-qualification-release-closure.md`,
`plans/proxy-cached-total-deadline-corrective-pass.md`.

Exit conditions: freeze-qualified with timed-out-body-kept-alive proof. Met.

### Milestone 3 — Route cache and Hyper client consolidation

Class: infrastructure. Status: closed.

Objective: bounded route-keyed Hyper reuse; central construction; uniform
idle policy; opaque proxy-TLS identity; invariant matrices.

Legacy: `plans/resolved-target-route-cache-and-connection-reuse.md`,
`plans/reusable-route-cache-invariants-and-hardening.md`,
`plans/core-hyper-client-construction-and-cache-consolidation.md`,
`plans/hyper-idle-pool-policy-corrective-pass.md`,
`plans/proxy-tls-route-cache-identity-corrective-pass.md`,
`plans/post-core-integrity-current-head-requalification-corrective-closure.md`.

Exit conditions: reuse qualified; no RPS regression; Stage C renewed. Met.

### Milestone 4 — Feature-profile containment and private decomposition

Class: infrastructure + polish. Status: closed.

Objective: truthful feature ownership; private decomposition without moving
public declarations; standard/lean boundary; residual splits evidence-gated.

Legacy: `plans/core-feature-dependency-and-tls-boundary-hardening.md`,
`plans/standard-route-advanced-routing-feature-boundary.md`,
`plans/high-level-policy-footprint-feature-boundary.md`,
`plans/core-private-module-decomposition.md`,
`plans/core-client-proxy-private-decomposition-second-pass.md`,
`plans/adapter-feature-and-dependency-boundary-correction.md`.

Exit conditions: profiles compile and behave per the feature-flags matrix. Met.

### Milestone 5 — Embedded transport extensions

Class: capability. Status: closed.

Objective: caller-owned dialer, request-scoped static routing, Hyper attempt
control, physical admission/I-O guards — all fail-closed, transport-generic.

Legacy: `plans/custom-dialer-transport-extension.md`,
`plans/strict-underlying-transport-attempt-control.md`,
`plans/physical-connection-admission-and-io-inactivity-guardrails.md`,
`plans/static-resolution-and-pinned-destination-routing.md`,
`plans/native-pool-map-dependency-reduction.md`,
`plans/native-uri-dependency-separation.md`,
`plans/standard-route-dns-provenance-correction.md`,
`plans/post-extensible-transport-qualification-and-closure.md`.

Exit conditions: external-style fixtures green; no downstream-specific types. Met.

### Milestone 6 — Windows TLS response completeness corrective

Class: invariant / corrective. Status: closed.

Implementation plan:

- `plans/implementation/core-transport-policy/006-windows-tls-response-completeness-corrective.md`

Closure record:

- `plans/closure/core-transport-policy/006-windows-tls-response-completeness.md`

Objective: reproduce and isolate a deterministic Windows HTTPS response-body
truncation below EggReplay's interception layer, distinguish fixture/rustls/
Hyper/EggFetch ownership, fix only the proven owner, and renew the exact-SHA
qualification before release work resumes.

This corrective preserves the frozen `ResponseBody` shape, single Hyper
engine, timeout/pool ownership, and supported-platform truthfulness.

Exit conditions: Windows reproducer resolved or explicitly blocked on proven
external ownership; genuine premature EOF still errors; Linux/macOS controls
green; Stage C rebound to the corrected exact SHA; downstream EggReplay
Windows large-body reproduction green before publication resumes.

Disposition: the deterministic Windows truncation is a
fixture-teardown contract defect (origin `write_all` + drop without `close_notify`/
linger lets the Windows transport discard the unsent tail); the receiver
correctly surfaces a `body` error rather than a short success. No production
change; a 19-test hermetic reproducer pins graceful/abrupt/keep-alive/
truncated/close-delimited behavior plus raw tokio-rustls and minimal-Hyper
isolation probes. Stage C rebound to
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`.

### M006C1 — Windows qualification and downstream closure corrective

Class: invariant / qualification corrective. Status: closed (native-Windows
EggFetch matrix green — run 36193582776, 19/19; corrected EggReplay M013F
Windows proof green — run 36211265347).

Implementation plan:

- `plans/implementation/core-transport-policy/006c1-windows-qualification-and-downstream-closure.md`

Objective: complete the two evidence conditions M006 advanced past: run the
new response-completeness matrix on native Windows x86_64 and retain the
corrected EggReplay M013F hosted-Windows 300 KiB direct/Eggress/MITM proof.

M006C1 does not reopen the fixture-teardown diagnosis or authorize production
transport changes. If either graceful Windows proof contradicts M006, the
technical investigation reopens and release remains blocked.

Exit conditions: direct EggFetch Windows matrix green (run 36193582776,
19/19 on the manual-qualification head), corrected EggReplay Windows M013F
green at the original 300 KiB envelope (run 36211265347), normal EggFetch
Tier 1 green, and closure evidence recorded. Met.

### M006C2 — Post-closure reconciliation and one-off workflow retirement

Class: corrective / polish. Status: closed.

Implementation plan:

- `plans/implementation/core-transport-policy/006c2-post-closure-reconciliation-and-workflow-retirement.md`

Closure record:

- `plans/closure/core-transport-policy/006c2-post-closure-reconciliation.md`

Objective: remove the one-off M006C1 Windows qualification workflow after its
evidence is retained, restore the steady-state verification-policy complexity
budget, and reconcile active registry/roadmap/release status.

Exit conditions: only `ci.yml` and `pypi.yml` remain under `.github/workflows/`,
Tier 1 and hosted CI are green, Stage C remains unchanged unless scope expands,
and M001/M002 return to ready. Met.

### Milestone 7 — Native transport failure classification

Class: capability / invariant. Status: closed.

Accepted ADR:

- `plans/adrs/ADR-0006-native-transport-failure-classification.md`

Implementation plan:

- `plans/implementation/core-transport-policy/007-native-transport-failure-classification.md`

Closure record:

- `plans/closure/core-transport-policy/007-native-transport-failure-classification.md`

Objective: extend the existing error domain with one non-exhaustive,
evidence-backed transport-failure classifier so native frame embedders can
distinguish connection, TLS, HTTP protocol/framing, and cancellation failures
without Hyper/rustls downcasts or display parsing.

Hard dependencies: M005 embedded transport extensions closed; ADR-0005
containment remains in force; ADR-0006 accepted.

Operational dependency: publication is outside M007. After implementation and
exact-SHA qualification close, hand the candidate to release-verification M003
standing intake.

Exit conditions: Error and NativeResponseBody shapes unchanged; existing
timeout/admission/DialErrorKind/NetworkFailureKind authorities preserved;
dispatch and body classification proven by deterministic fixtures; no message
parsing/dependency/feature growth; Tier 1/Tier 2 green with expected API delta
and renewed exact-SHA binding.

### Milestone 8 — Profile matrix completion

Class: invariant. Status: closed.

Implementation:
`plans/implementation/core-transport-policy/008-profile-matrix-completion.md`.

Closure record:
`plans/closure/core-transport-policy/008-profile-matrix-completion.md`.

Objective: make the supported feature-profile set fully covered by the
verification gates, so that no declared profile is compiled only as a side
effect of `--all-features`.

Dependencies:
- hard: none; the gap is a coverage gap, and both missing profiles already
  compile at the baseline;
- interface: the existing six-profile contract matrix in `scripts/check.sh`
  and the six-profile Rust public-surface oracle in
  `scripts/check_rust_public_api.py`.

Deliverable boundary: `standard-http2` and `native-http2` are added to the
stable compile-contract matrix and frozen in the public-surface oracle; the
lean profile's behavioral suites run under the lean profile in Tier 1. No
runtime behavior and no public surface change.

User or operator value: a regression that breaks HTTP/2 in the URL-less
native profile, or in the lean H2 profile, becomes visible in continuous
integration instead of being masked by `--all-features`.

Exit conditions: eight profiles compiled by the contract matrix; eight
profiles frozen in the oracle with no existing snapshot changed; the lean
behavioral suites green under the lean profile; Tier 1 and Tier 2 green with
renewed exact-SHA binding.

Deferred: whole-suite lean-profile coverage (M009); the lint-guard and
downstream-gate defects (release-verification M006). Both deferred items were
subsequently delivered by M009 and release-verification M006 respectively.

### Milestone 9 — Lean path single-sourcing and full-suite profile coverage

Class: infrastructure. Status: closed.

Implementation:
`plans/implementation/core-transport-policy/009-lean-path-single-sourcing.md`.

Objective: remove the request-preparation logic duplicated between the lean
path and the redirect path, and make the whole test suite compile under the
lean profile.

Dependencies:
- hard: M008, because the lean behavioral gate this milestone extends must
  land first;
- interface: ADR-0002 pipeline phase ownership; the lean path stays a
  deliberate footprint trade-off recorded in
  `plans/high-level-policy-footprint-feature-boundary.md`.

Deliverable boundary: one shared, feature-independent preparation builder
serves both paths; the exhaustive `Request::into_parts()` destructuring
exists in exactly one place; thirteen test targets gain precise file-level
feature gates so the whole suite compiles under the lean profile. The lean
profile's behavior and footprint are unchanged.

User or operator value: a change to how headers, timeouts, cookies or
credentials are prepared can no longer be applied in one copy and missed in
another, and the lean profile can be gated as a whole.

Exit conditions: exactly one exhaustive destructuring; identical prepared
hops from both paths; whole suite compiles under `standard-http1,tls-rustls`;
lean footprint unchanged; Tier 1 and Tier 2 green with renewed exact-SHA
binding.

Deferred: the remaining internal duplication clusters (M010), which M010
delivered.

### Milestone 10 — Core duplicate-helper consolidation

Class: infrastructure + polish. Status: closed.

Implementation:
`plans/implementation/core-transport-policy/010-core-duplicate-helper-consolidation.md`.

Objective: consolidate the seven confirmed clusters of duplicated helper
logic in `eggfetch-core` so that each rule is implemented once.

Dependencies:
- hard: M009, so that the remaining duplication inventory is accurate;
- interface: ADR-0002 phase ownership; ADR-0006 failure classification
  preserved; ADR-0005 containment preserved.

Deliverable boundary: one resolved-target/SNI client builder; one
effective-port validation; one custom-dialing incompatibility ladder; one
consistently-validating origin normalization; one post-handshake ALPN `h2`
check; one `HyperClientPolicy` constructor; one shared UTF-8 target-extension
error. All private; no public API change.

User or operator value: the engine's helper inventory becomes auditable, and
a rule change can no longer land in one copy and not another. Cluster D also
resolves a live divergence in which URLs are accepted as origins.

Exit conditions: each cluster implemented exactly once; pool keying unchanged
for every supported URL shape; error kinds and shared message text
unchanged; Tier 1 and Tier 2 green with renewed exact-SHA binding.

Deferred: nothing within the engine; the redirect-policy authority question
that crosses the facade boundary is owned by python-httpx-compat M005.

## 8. Cross-cutting requirements

Timeout/pool ownership per ADR-0004; trust semantics per the TLS workstream;
no new public surface without ADR-0005 containment proof. M007 is the narrow
accepted exception recorded by ADR-0006 and is contained to the existing error
domain; it does not authorize adjacent transport helpers. Docs update with
each executable change.

## 9. Verification strategy

Route-selection unit tests; retry/redirect matrices; total-deadline
lifecycle and lease proofs; feature-matrix builds; Tier 1 always; Tier 2 +
exact-SHA requalification when executable inputs change. M007 additionally
requires typed-source unit tests plus deterministic native dispatch and
NativeResponseBody protocol/TLS/cancellation fixtures under the native HTTP/1
profile.

## 10. Risks and decision points

Risk: future fields bypassing rebuild helpers. Control: helpers are
exhaustive by construction; review must reject inline hop construction.

## 11. Completion definition

M001–M007, M006C1, and M006C2 are closed. M007 added the contained ADR-0006
error-classification capability with Tier 2/public-API/exact-SHA evidence
and handed publication to release-verification. Live Stage C truth remains
owned by the canonical qualification ledger rather than this roadmap.

The subsystem reopens for M008–M010, registered from a repository audit that
found the supported-profile set under-covered by the gates, the lean path
carrying an unexercised duplicate of the redirect preparation, and seven
further internal duplication clusters. M008 and M009 are sequenced, and M010
follows M009. None of the three changes runtime behavior.

M008 was implemented and is **closed** on 2026-10-22: the contract matrix
now covers eight profiles, the lean profile's behavioral
suites run in Tier 1, and the Rust public-surface oracle freezes eight
profiles with two new tool-generated snapshots that were measured to be
byte-identical to their H1 counterparts. It was first recorded as
conditionally closed because the milestone changes validation scripts and API
snapshots — both executable inputs — which invalidated the live Stage C
binding and left the frozen-SHA Tier 1/2/3 gates outstanding. That condition
was met on bound SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, where the
Stage C binding was renewed and the tier gates ran green; M008 moved from
`conditionally closed` to `closed` there.

M009 and M010 were implemented on the same executable freeze and are closed on
that same bound SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, with the Stage
C binding renewed there and the frozen-SHA Tier 1/2/3 gates green. M009 makes
the whole core test suite compile under the lean profile behind one shared
first-hop preparation builder; M010 consolidates the seven duplication
clusters with no runtime and no public-API change, and the Rust public-surface
oracle is byte-identical to the frozen snapshots in all eight profiles.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 pipeline + rebuilds | closed | legacy plans §7 | legacy closure records | — |
| M002 total-deadline lifecycle | closed | legacy plans §7 | legacy closure records | — |
| M003 route cache + Hyper | closed | legacy plans §7 | legacy closure records | — |
| M004 profiles + decomposition | closed | legacy plans §7 | legacy closure records | — |
| M005 embedded extensions | closed | legacy plans §7 | legacy closure records | — |
| M006 Windows TLS response completeness | closed | `plans/implementation/core-transport-policy/006-windows-tls-response-completeness-corrective.md` | `plans/closure/core-transport-policy/006-windows-tls-response-completeness.md` | Historical diagnosis; release evidence completed by M006C1 |
| M006C1 Windows qualification/downstream closure | closed | `plans/implementation/core-transport-policy/006c1-windows-qualification-and-downstream-closure.md` | `plans/closure/core-transport-policy/006c1-windows-qualification-and-downstream-closure.md` | — |
| M006C2 post-closure reconciliation/workflow retirement | closed | `plans/implementation/core-transport-policy/006c2-post-closure-reconciliation-and-workflow-retirement.md` | `plans/closure/core-transport-policy/006c2-post-closure-reconciliation.md` | — |
| M007 native transport failure classification | closed | `plans/implementation/core-transport-policy/007-native-transport-failure-classification.md` | `plans/closure/core-transport-policy/007-native-transport-failure-classification.md` | Publication deferred to release-verification M003 intake (exact-pin downstream adoption blocked until then) |
| M008 profile matrix completion | closed | `plans/implementation/core-transport-policy/008-profile-matrix-completion.md` | `plans/closure/core-transport-policy/008-profile-matrix-completion.md` | Closed on bound SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c` with the Stage C binding renewed there. |
| M009 lean path single-sourcing | closed | `plans/implementation/core-transport-policy/009-lean-path-single-sourcing.md` | `plans/closure/core-transport-policy/009-lean-path-single-sourcing.md` | Closed on bound SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c` with the Stage C binding renewed there. |
| M010 core duplicate-helper consolidation | closed | `plans/implementation/core-transport-policy/010-core-duplicate-helper-consolidation.md` | `plans/closure/core-transport-policy/010-core-duplicate-helper-consolidation.md` | Closed on bound SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c` with the Stage C binding renewed there. |
