# Core Transport and Request Policy Roadmap

Status: closed; all milestones closed

Long-term references:

- `plans/000-long-term-specification.md` §4.1, §4.5, §5, §8
- `plans/001-terminology-and-domain-model.md` §2, §4
- `plans/002-long-term-roadmap.md` Phase 0

Related ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0002-pipeline-route-finalize.md`
- `plans/adrs/ADR-0004-proxy-fallback-total-deadline.md`
- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`

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

## 5. Target architecture

The §2 invariants hold permanently. Future changes in this subsystem arrive
as corrective passes (§7 of the planning process), each requalifying when
executable inputs change.

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

M002–M005 are soft/parallel after M001; all are closed.

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

## 8. Cross-cutting requirements

Timeout/pool ownership per ADR-0004; trust semantics per the TLS workstream;
no new public surface without ADR-0005 containment proof; docs updated with
each change.

## 9. Verification strategy

Route-selection unit tests; retry/redirect matrices; total-deadline
lifecycle and lease proofs; feature-matrix builds; Tier 1 always; Tier 2 +
exact-SHA requalification when executable inputs change.

## 10. Risks and decision points

Risk: future fields bypassing rebuild helpers. Control: helpers are
exhaustive by construction; review must reject inline hop construction.

## 11. Completion definition

Closed: all five milestones have accepted closure evidence and the live
Stage C binding covers the resulting engine. Standing intake continues via
corrective passes only.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 pipeline + rebuilds | closed | legacy plans §7 | legacy closure records | — |
| M002 total-deadline lifecycle | closed | legacy plans §7 | legacy closure records | — |
| M003 route cache + Hyper | closed | legacy plans §7 | legacy closure records | — |
| M004 profiles + decomposition | closed | legacy plans §7 | legacy closure records | — |
| M005 embedded extensions | closed | legacy plans §7 | legacy closure records | — |
