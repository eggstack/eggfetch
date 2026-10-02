# eggfetch Active Planning Registry

This file is the compact control surface for active planning. Detailed
requirements and completed history remain in source roadmaps, implementation
plans, `plans/closure/`, legacy plan records, and Git history.

Canonical direction remains in:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`

Live qualification binding (authoritative, never duplicated here — see the
ledger for SHAs):

- `plans/httpx-parity-correction-status.md` (+ `compat/*/profile.toml`)

Release train: M001/M004 is closed across crates.io, signed `v0.2.1`, PyPI,
and the published GitHub Release. M005 is ready to publish the closed M007
capability as coordinated 0.2.2. M001B remains superseded history.

Validation tiers: `.skills/verification-qualification.md`. Normative policy:
`docs/verification-policy.md`.

## Status vocabulary

- **proposed** — roadmap or plan exists but is not approved for execution.
- **ready** — dependencies and interfaces are satisfied; plan may be handed off.
- **active** — implementation or closure work is in progress.
- **blocked** — a named dependency or evidence requirement prevents progress.
- **closing** — implementation landed and closure evidence is being gathered.
- **closed** — closure record accepted.
- **conditionally closed** — substantial work landed, but a named correctness
  or operational evidence condition remains.
- **superseded** — replaced by another document.
- **archived** — no longer active and retained for traceability.

## Active subsystem roadmaps

| Subsystem | Status | Roadmap | Current milestone | Dependencies or blockers |
|---|---|---|---|---|
| Core transport and request policy | closed | `plans/subsystems/core-transport-policy-roadmap.md` | M007 closed; subsystem at steady state | ADR-0006 implemented and qualified; publication owned by release-verification M005. |
| Python bindings and HTTPX compatibility | closed | `plans/subsystems/python-httpx-compat-roadmap.md` | M001-M003 closed on live Stage C binding | Facade work reopens only via gated roadmap Phase 4 trigger. |
| TLS, proxy, and protocols | closed | `plans/subsystems/tls-proxy-protocols-roadmap.md` | All milestones closed; H3 experimental retained | H3 graduation blocked on named external evidence. |
| Release and verification | active | `plans/subsystems/release-verification-roadmap.md` | M005 0.2.2 M007 publication ready | Core-transport M007 closed; final candidate must be versioned, requalified, rehearsed, then published. |
| Performance and footprint | closed | `plans/subsystems/performance-footprint-roadmap.md` | Campaigns closed; fixtures repaired | New optimization needs its own milestone plan. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| Release and verification | M005 0.2.2 M007 publication | ready | `plans/implementation/release-verification/005-0-2-2-m007-publication.md` | Core M007 closed. Target 0.2.2; requalify/rehearse exact candidate before any immutable publication. |
| Core transport and request policy | M007 native transport failure classification | closed | `plans/implementation/core-transport-policy/007-native-transport-failure-classification.md` | `plans/closure/core-transport-policy/007-native-transport-failure-classification.md`. Publication/version selection is a later release-verification M003 intake; exact-pin downstream adoption remains blocked until then. |
| Release and verification | M001 coordinated 0.2.x publication umbrella | closed | `plans/archive/implementation/release-verification/001-release-publication.md` | `plans/closure/release-verification/001-coordinated-0-2-1-publication.md` |
| Release and verification | M001A 0.2.1 release-candidate preparation | closed | `plans/implementation/release-verification/001a-0-2-1-release-candidate-preparation.md` | Closed on original candidate `41757569123c0b8038550b956d8b244ab55094a6`. |
| Release and verification | M002 Python 3.15 wheel rehearsal | closed | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | Run `36223505399` green: 18 wheels + 1 sdist, `publish=false`. |
| Release and verification | M001B 0.2.1 coordinated publication | superseded | `plans/implementation/release-verification/001b-0-2-1-coordinated-publication.md` | Superseded before execution by M004. |
| Release and verification | M004 0.2.1 tagged release finalization | closed | `plans/archive/implementation/release-verification/004-0-2-1-tagged-release-finalization.md` | `plans/closure/release-verification/004-0-2-1-tagged-release-finalization.md` |

## Current execution order and dependency gates

**Release execution gate:** M005 is ready. Historical v0.2.0/v0.2.1 public
identities remain immutable. M005 targets coordinated 0.2.2 for the closed
M007 capability and must create a new exact candidate after version/package
edits, renew Tier 2/Stage C, complete Tier 3/security, and pass the build-only
19-distribution rehearsal before any publication.

**Core transport gate:** M007 is closed under accepted ADR-0006 and the
subsystem is back at steady state. Publication is now explicitly owned by
release-verification M005; no further core-transport implementation is
required for that handoff.

**Gated futures:** HTTPX 1.0 (RC/stable + frozen API + fresh delta),
H3 graduation (independent interop/drain/impairment/upstream evidence), Node
maturation (explicit scope decision). None is registered; none unblocks
active work.

## Blocked / operational work

| Subsystem | Milestone | Blocker |
|---|---|---|
| TLS, proxy, protocols | H3 graduation | Independent non-Quinn interop, GOAWAY/drain, public-origin, impairment, upstream-risk evidence. |
| Python compat | HTTPX 1.0 program | Upstream RC/stable trigger has not fired. |

## Closure work and current control points

Core transport M007 is closed and the subsystem status is reconciled to
steady state. Release-verification M005 is dependency-ready and is the only
active publication handoff: exact-pin downstream adoption remains blocked
until coordinated 0.2.2 is publicly verified. The gated futures (HTTPX 1.0,
H3 graduation, Node maturation) remain unregistered and unaffected.

M001A and M002 are closed historical release evidence. M002 run
`36223505399` proved the original candidate's 18-wheel + 1-sdist matrix with
`publish=false`; M004 run `36253723990` renewed the rehearsal on the final
candidate before publication. M004 publish run `36255517733` completed
successfully. All M006-family work is also closed. Stage C remains bound to
`3fc58fbd99ecb496749b833ee7b27436fe3b412d` unless executable or
qualification inputs change.
