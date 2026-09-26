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

Pending release train: M001A and M002 are closed. M004 is the sole ready
release task: refresh release-facing documentation, freeze a docs-only final
0.2.1 candidate, renew the build-only wheel rehearsal on that exact SHA, then
publish crates.io, signed v0.2.1, PyPI, and the GitHub Release. The unexecuted
M001B plan is superseded.

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
| Core transport and request policy | closed | `plans/subsystems/core-transport-policy-roadmap.md` | M006/M006C1/M006C2 all closed; subsystem at steady state | New corrective requires a fresh milestone plan. |
| Python bindings and HTTPX compatibility | closed | `plans/subsystems/python-httpx-compat-roadmap.md` | M001-M003 closed on live Stage C binding | Facade work reopens only via gated roadmap Phase 4 trigger. |
| TLS, proxy, and protocols | closed | `plans/subsystems/tls-proxy-protocols-roadmap.md` | All milestones closed; H3 experimental retained | H3 graduation blocked on named external evidence. |
| Release and verification | active | `plans/subsystems/release-verification-roadmap.md` | M004 0.2.1 tagged release finalization | M001A/M002 closed; M004 is the sole ready task and supersedes unexecuted M001B. |
| Performance and footprint | closed | `plans/subsystems/performance-footprint-roadmap.md` | Campaigns closed; fixtures repaired | New optimization needs its own milestone plan. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| Release and verification | M001 coordinated 0.2.x publication umbrella | ready (finalization pending) | `plans/implementation/release-verification/001-release-publication.md` | M001A/M002 are closed; umbrella closes after M004 publication/closure. |
| Release and verification | M001A 0.2.1 release-candidate preparation | closed | `plans/implementation/release-verification/001a-0-2-1-release-candidate-preparation.md` | Closed on original candidate `41757569123c0b8038550b956d8b244ab55094a6`. |
| Release and verification | M002 Python 3.15 wheel rehearsal | closed | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | Run `36223505399` green: 18 wheels + 1 sdist, `publish=false`. |
| Release and verification | M001B 0.2.1 coordinated publication | superseded | `plans/implementation/release-verification/001b-0-2-1-coordinated-publication.md` | Superseded before execution by M004. |
| Release and verification | M004 0.2.1 tagged release finalization | **ready** | `plans/implementation/release-verification/004-0-2-1-tagged-release-finalization.md` | Sole ready task: docs truth refresh → renewed exact-SHA rehearsal → crates.io → signed tag → PyPI → GitHub Release → closure. |

## Current execution order and dependency gates

**Release execution gate:** M001A and M002 are closed. M004 is the sole
ready task. It first corrects the stale release-facing README/process docs,
freezes a docs-only final `0.2.1` candidate, and renews the build-only PyPI
rehearsal because the README is PyPI package metadata. It then publishes the
six crates, signed `v0.2.1` tag, PyPI release, and required GitHub Release.
Historical `v0.2.0` remains immutable.

**Corrective gate:** none. The M006 Windows response-completeness work chain
(M006/M006C1/M006C2) is closed. New correctives open through a fresh milestone
plan and a fresh entry in this registry.

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

M001A and M002 are closed historical release evidence. M002 run
`36223505399` proved the original candidate's 18-wheel + 1-sdist matrix with
`publish=false`; M004 renews that exact-SHA rehearsal only because the root
README/PyPI long description must be corrected before publication. All
M006-family work is also closed. Stage C remains bound to
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa` unless executable or
qualification inputs change.