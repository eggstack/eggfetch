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
| Core transport and request policy | closed | `plans/subsystems/core-transport-policy-roadmap.md` | M006C1 Windows qualification/downstream closure | EggFetch native-Windows matrix and corrected EggReplay M013F Windows proof green; see M006C1 closure. |
| Python bindings and HTTPX compatibility | closed | `plans/subsystems/python-httpx-compat-roadmap.md` | M001-M003 closed on live Stage C binding | Facade work reopens only via gated roadmap Phase 4 trigger. |
| TLS, proxy, and protocols | closed | `plans/subsystems/tls-proxy-protocols-roadmap.md` | All milestones closed; H3 experimental retained | H3 graduation blocked on named external evidence. |
| Release and verification | active | `plans/subsystems/release-verification-roadmap.md` | M001 publication + M002 wheel rehearsal ready | M006C1 closed; Stage C remains bound to `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`. |
| Performance and footprint | closed | `plans/subsystems/performance-footprint-roadmap.md` | Campaigns closed; fixtures repaired | New optimization needs its own milestone plan. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| Core transport and request policy | M006 Windows TLS response completeness corrective | closed | `plans/implementation/core-transport-policy/006-windows-tls-response-completeness-corrective.md` | Historical investigation/fixture-teardown disposition; closure evidence is supplemented by M006C1 before release. |
| Core transport and request policy | M006C1 Windows qualification/downstream closure corrective | **closed** | `plans/implementation/core-transport-policy/006c1-windows-qualification-and-downstream-closure.md` | EggFetch matrix green (run 36193582776); corrected EggReplay hosted-Windows 300 KiB proof green (run 36211265347). |
| Release and verification | M001 0.2.x publication/tag/PyPI | ready | `plans/implementation/release-verification/001-release-publication.md` | M006C1 closed; existing manual release gates apply. |
| Release and verification | M002 Python 3.15 wheel rehearsal | ready | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | M006C1 closed; use authoritative Stage C SHA unless executable inputs change. |

## Current execution order and dependency gates

**Corrective gate:** M006's fixture-teardown diagnosis remains the current
technical verdict and Stage C remains bound to
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa` (the manual-qualification head is a
CI-surface-only descendant: manual workflow file only, no
executable/test/validation-input change). The native-Windows EggFetch matrix
is green (run 36193582776, 19/19), and the corrected EggReplay M013F hosted-
Windows 300 KiB proof is green (run 36211265347, commit
`5efc6f9c892bb5b1c2e84330a0c40a38f1de0de7`). M006C1 is closed; release gates
M001 and M002 are reopened as ready.

**Release publication gate:** M001 is ready after M006C1 closure. Follow its
existing maintainer-run publication and verification procedure.

**Wheel rehearsal gate:** M002 is ready after M006C1 closure. The corrective
was evidence-only, so the current Stage C SHA remains authoritative; if any
executable or qualification input changes, apply normal exact-SHA
invalidation/rebinding rules before rehearsal.

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

M006 remains closed as historical investigation evidence through
`plans/closure/core-transport-policy/006-windows-tls-response-completeness.md`.
M006C1 is closed: the native-Windows EggFetch matrix is green
(run 36193582776, 19/19), and the corrected EggReplay M013F Windows proof is
green at 300 KiB (run 36211265347). See
`plans/closure/core-transport-policy/006c1-windows-qualification-and-downstream-closure.md`.
Stage C remains bound to `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`
(the manual-qualification head is a CI-surface-only descendant: manual
workflow file only, no executable/test/validation-input change).
