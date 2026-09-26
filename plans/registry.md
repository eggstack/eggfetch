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

Pending release train: M001A prepares a fresh coordinated 0.2.1 candidate;
M002 rehearses the Python 3.10–3.15 wheel/sdist set from that exact candidate;
M001B performs coordinated crates.io/tag/PyPI publication.

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
| Release and verification | active | `plans/subsystems/release-verification-roadmap.md` | M001A 0.2.1 candidate preparation | M001A is the sole ready task; M002 then M001B follow in order. |
| Performance and footprint | closed | `plans/subsystems/performance-footprint-roadmap.md` | Campaigns closed; fixtures repaired | New optimization needs its own milestone plan. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| Release and verification | M001 coordinated 0.2.x publication umbrella | ready (decomposed) | `plans/implementation/release-verification/001-release-publication.md` | Execute M001A → M002 → M001B; umbrella closes after publication. |
| Release and verification | M001A 0.2.1 release-candidate preparation | **ready** | `plans/implementation/release-verification/001a-0-2-1-release-candidate-preparation.md` | Bump coordinated identity to 0.2.1, audit candidate delta, run Tier 1/2/3, freeze exact candidate SHA. |
| Release and verification | M002 Python 3.15 wheel rehearsal | blocked | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | Blocked on M001A; `publish=false` from exact candidate, 18 wheels + 1 sdist. |
| Release and verification | M001B 0.2.1 coordinated publication | blocked | `plans/implementation/release-verification/001b-0-2-1-coordinated-publication.md` | Blocked on M002; publish six crates, signed v0.2.1 tag, PyPI `publish=true`, external smoke. |

## Current execution order and dependency gates

**Release execution gate:** M001A is the sole ready task. The existing
`v0.2.0` tag/release is historical and must not be moved or reused. M001A
prepares a fresh `0.2.1` candidate; M002 then proves the Python 3.15 matrix
from that exact SHA; M001B finally publishes/tag/releases that same candidate.

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

All M006-family work (M006, M006C1, M006C2) is closed historical evidence;
see `plans/closure/core-transport-policy/`. Stage C remains bound to
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa` unless a future corrective
changes executable or qualification inputs.