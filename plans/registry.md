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
Python 3.15 wheel rehearsal (M002), both ready after M006C1 closure.

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
| Core transport and request policy | active corrective | `plans/subsystems/core-transport-policy-roadmap.md` | M006C2 post-closure reconciliation | Retire one-off Windows workflow and return verification topology to policy budget. |
| Python bindings and HTTPX compatibility | closed | `plans/subsystems/python-httpx-compat-roadmap.md` | M001-M003 closed on live Stage C binding | Facade work reopens only via gated roadmap Phase 4 trigger. |
| TLS, proxy, and protocols | closed | `plans/subsystems/tls-proxy-protocols-roadmap.md` | All milestones closed; H3 experimental retained | H3 graduation blocked on named external evidence. |
| Release and verification | blocked on M006C2 | `plans/subsystems/release-verification-roadmap.md` | M001 publication + M002 wheel rehearsal paused | One-off Windows qualification workflow must be retired and planning reconciled first. |
| Performance and footprint | closed | `plans/subsystems/performance-footprint-roadmap.md` | Campaigns closed; fixtures repaired | New optimization needs its own milestone plan. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| Core transport and request policy | M006 Windows TLS response completeness corrective | closed | `plans/implementation/core-transport-policy/006-windows-tls-response-completeness-corrective.md` | Historical investigation/fixture-teardown disposition; closure evidence is supplemented by M006C1 before release. |
| Core transport and request policy | M006C1 Windows qualification/downstream closure corrective | closed | `plans/implementation/core-transport-policy/006c1-windows-qualification-and-downstream-closure.md` | EggFetch matrix green (run 36193582776); corrected EggReplay hosted-Windows 300 KiB proof green (run 36211265347). |
| Core transport and request policy | M006C2 post-closure reconciliation/workflow retirement | **ready** | `plans/implementation/core-transport-policy/006c2-post-closure-reconciliation-and-workflow-retirement.md` | Remove one-off Windows workflow; restore steady-state CI topology; reconcile status docs. |
| Release and verification | M001 0.2.x publication/tag/PyPI | blocked | `plans/implementation/release-verification/001-release-publication.md` | Blocked on M006C2 closure. |
| Release and verification | M002 Python 3.15 wheel rehearsal | blocked | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | Blocked on M006C2 closure. |

## Current execution order and dependency gates

**Corrective gate:** M006/M006C1 technical and Windows evidence are complete.
M006C2 is the sole active corrective: remove the temporary
`.github/workflows/windows-qualification.yml` surface, restore the
verification-policy workflow budget, and reconcile active planning. Stage C
remains bound to `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa` unless the cleanup
unexpectedly changes executable or qualification inputs.

**Release publication gate:** M001 is blocked until M006C2 closes.

**Wheel rehearsal gate:** M002 is blocked until M006C2 closes.

**Gated futures:** HTTPX 1.0 (RC/stable + frozen API + fresh delta),
H3 graduation (independent interop/drain/impairment/upstream evidence), Node
maturation (explicit scope decision). None is registered; none unblocks
active work.

## Blocked / operational work

| Subsystem | Milestone | Blocker |
|---|---|---|
| Release and verification | M001 publication/tag/PyPI | M006C2 workflow-retirement/reconciliation closure. |
| Release and verification | M002 wheel rehearsal | M006C2 closure. |
| TLS, proxy, protocols | H3 graduation | Independent non-Quinn interop, GOAWAY/drain, public-origin, impairment, upstream-risk evidence. |
| Python compat | HTTPX 1.0 program | Upstream RC/stable trigger has not fired. |

## Closure work and current control points

M006 and M006C1 remain closed historical evidence. M006C2 is open and will
close through
`plans/closure/core-transport-policy/006c2-post-closure-reconciliation.md`.
Stage C remains bound to `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa` unless
M006C2 changes executable or qualification inputs.
