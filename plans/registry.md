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

Pending maintainer actions: issue #24 publication/tag/PyPI (M001) and the
Python 3.15 wheel rehearsal (M002), both ready against the live Stage C
freeze.

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
| Release and verification | active | `plans/subsystems/release-verification-roadmap.md` | M001 publication + M002 wheel rehearsal ready | Maintainer dispatch from authoritative Stage C SHA. |
| Performance and footprint | closed | `plans/subsystems/performance-footprint-roadmap.md` | Campaigns closed; fixtures repaired | New optimization needs its own milestone plan. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| Release and verification | M001 0.2.x publication/tag/PyPI | **ready** | `plans/implementation/release-verification/001-release-publication.md` | Maintainer dispatch from authoritative Stage C SHA; leaf order http-connect → core → cli → ffi → python → node, then tag, then `pypi.yml` with `publish=true`. |
| Release and verification | M002 Python 3.15 wheel rehearsal | **ready** | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | Dispatch `pypi.yml` with `publish=false`; claim 3.15 only after 19-distribution assembly + smoke. |

## Current execution order and dependency gates

**Release publication gate:** M001 is ready; M002 is ready. Both run
against the live Stage C freeze; no additional corrective work is pending.

**Corrective gate:** none. The M006 Windows response-completeness work
chain (M006/M006C1/M006C2) is closed. New correctives open through a
fresh milestone plan and a fresh entry in this registry.

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