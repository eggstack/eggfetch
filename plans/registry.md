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

Pending maintainer actions: issue #24 publication/tag/PyPI and the Python
3.15 wheel rehearsal (see Blocked/operational work below).

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
| Core transport and request policy | active corrective | `plans/subsystems/core-transport-policy-roadmap.md` | M006 Windows TLS response completeness | Windows HTTPS body truncation investigation/qualification; publication gates paused. |
| Python bindings and HTTPX compatibility | closed | `plans/subsystems/python-httpx-compat-roadmap.md` | M001-M003 closed on live Stage C binding | Facade work reopens only via gated roadmap Phase 4 trigger. |
| TLS, proxy, and protocols | closed | `plans/subsystems/tls-proxy-protocols-roadmap.md` | All milestones closed; H3 experimental retained | H3 graduation blocked on named external evidence. |
| Release and verification | blocked on corrective | `plans/subsystems/release-verification-roadmap.md` | M001 publication + M002 wheel rehearsal paused | M006 core transport corrective must close and Stage C must be rebound first. |
| Performance and footprint | closed | `plans/subsystems/performance-footprint-roadmap.md` | Campaigns closed; fixtures repaired | New optimization needs its own milestone plan. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| Core transport and request policy | M006 Windows TLS response completeness corrective | **ready** | `plans/implementation/core-transport-policy/006-windows-tls-response-completeness-corrective.md` | Reproduce/isolate Windows HTTPS truncation; fix only proven owner; renew exact-SHA qualification. |
| Release and verification | M001 0.2.x publication/tag/PyPI | blocked | `plans/implementation/release-verification/001-release-publication.md` | Blocked on M006 closure + renewed Stage C binding. |
| Release and verification | M002 Python 3.15 wheel rehearsal | blocked | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | Blocked on M006 closure; rehearsal must use the post-corrective implementation SHA. |

## Current execution order and dependency gates

**Corrective gate:** M006 is the sole dependency-ready implementation task.
It investigates the Windows HTTPS response-body truncation reproduced by
EggReplay without proxy/MITM involvement. Any executable fix invalidates the
live Stage C binding and requires exact-SHA renewal.

**Release publication gate:** M001 is blocked until M006 closes and the live
Stage C binding points at the post-corrective exact SHA. Do not publish the
older qualified candidate while a supported-Windows correctness defect is
open.

**Wheel rehearsal gate:** M002 is blocked until M006 closes. The
`publish=false` 18-wheel rehearsal must run from the post-corrective
implementation commit; Python 3.15 support remains unclaimed until that
rehearsal evidence lands.

**Gated futures:** HTTPX 1.0 (RC/stable + frozen API + fresh delta),
H3 graduation (independent interop/drain/impairment/upstream evidence), Node
maturation (explicit scope decision). None is registered; none unblocks
active work.

## Blocked / operational work

| Subsystem | Milestone | Blocker |
|---|---|---|
| Release and verification | M001 publication/tag/PyPI | M006 Windows TLS response completeness corrective + renewed Stage C binding. |
| Release and verification | M002 wheel rehearsal | M006 closure; rehearsal must target the corrected implementation SHA. |
| TLS, proxy, protocols | H3 graduation | Independent non-Quinn interop, GOAWAY/drain, public-origin, impairment, upstream-risk evidence. |
| Python compat | HTTPX 1.0 program | Upstream RC/stable trigger has not fired. |

## Closure work and current control points

M006 is open and will close through
`plans/closure/core-transport-policy/006-windows-tls-response-completeness.md`.
Legacy closure evidence lives in the grandfathered flat plan records (see
`plans/README.md` § Legacy archive); each subsystem roadmap's milestone-status
table links the controlling legacy plans.
