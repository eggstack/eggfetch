# Core Transport and Request Policy Roadmap

Status: closed — M006C1 native-Windows and downstream EggReplay qualification complete

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

M002–M005 are soft/parallel after M001 and remain closed. M006 is a bounded
Phase-3 corrective discovered by downstream Windows qualification; it does not
reopen M001–M005 architecture.

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
Tier 1 green, and closure evidence recorded. Met. M001/M002 release work is
ready to resume.

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

M001–M006 and M006C1 are closed. M006C1's evidence-only closure reopens the
M001/M002 release gates. The live Stage C binding continues to point at
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`; no EggFetch executable or
qualification input changed.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 pipeline + rebuilds | closed | legacy plans §7 | legacy closure records | — |
| M002 total-deadline lifecycle | closed | legacy plans §7 | legacy closure records | — |
| M003 route cache + Hyper | closed | legacy plans §7 | legacy closure records | — |
| M004 profiles + decomposition | closed | legacy plans §7 | legacy closure records | — |
| M005 embedded extensions | closed | legacy plans §7 | legacy closure records | — |
| M006 Windows TLS response completeness | closed | `plans/implementation/core-transport-policy/006-windows-tls-response-completeness-corrective.md` | `plans/closure/core-transport-policy/006-windows-tls-response-completeness.md` | Historical diagnosis; release evidence completed by M006C1 |
| M006C1 Windows qualification/downstream closure | **closed** | `plans/implementation/core-transport-policy/006c1-windows-qualification-and-downstream-closure.md` | `plans/closure/core-transport-policy/006c1-windows-qualification-and-downstream-closure.md` | EggFetch Windows matrix and EggReplay hosted Windows 300 KiB proof green |
