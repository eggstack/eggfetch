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
| Core transport and request policy | active corrective | `plans/subsystems/core-transport-policy-roadmap.md` | M006 closed (Windows TLS fixture-teardown disposition) | Stage C rebound to `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`; downstream EggReplay must call TLS shutdown in its local origin helper. |
| Python bindings and HTTPX compatibility | closed | `plans/subsystems/python-httpx-compat-roadmap.md` | M001-M003 closed on live Stage C binding | Facade work reopens only via gated roadmap Phase 4 trigger. |
| TLS, proxy, and protocols | closed | `plans/subsystems/tls-proxy-protocols-roadmap.md` | All milestones closed; H3 experimental retained | H3 graduation blocked on named external evidence. |
| Release and verification | ready (operational) | `plans/subsystems/release-verification-roadmap.md` | M001 publication + M002 wheel rehearsal ready | Maintainer may run against corrected Stage C SHA `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`. |
| Performance and footprint | closed | `plans/subsystems/performance-footprint-roadmap.md` | Campaigns closed; fixtures repaired | New optimization needs its own milestone plan. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| Core transport and request policy | M006 Windows TLS response completeness corrective | **closed** | `plans/implementation/core-transport-policy/006-windows-tls-response-completeness-corrective.md` | `plans/closure/core-transport-policy/006-windows-tls-response-completeness.md` records fixture-teardown verdict; Stage C rebound to `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`. |
| Release and verification | M001 0.2.x publication/tag/PyPI | ready (operational) | `plans/implementation/release-verification/001-release-publication.md` | Maintainer publication against corrected Stage C SHA. |
| Release and verification | M002 Python 3.15 wheel rehearsal | ready (operational) | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | Maintainer dispatches the `publish=false` 18-wheel rehearsal from the corrected implementation SHA. |

## Current execution order and dependency gates

**Corrective gate:** M006 is **closed** (Windows TLS response completeness
fixture-teardown disposition, Stage C rebound to
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`). No further follow-up is
scheduled inside the core-transport workstream; the corrective is documented
and the engine is back in service.

**Release publication gate:** M001 is ready against the corrected Stage C
SHA. The maintainer may run the manual `cargo publish` sequence
(http-connect → core → cli → ffi → python → node → tag → PyPI dispatch)
from the documented SHA. Issue #24 publication continues to wait on
maintainer action; do not publish any older qualified candidate.

**Wheel rehearsal gate:** M002 is ready against the corrected SHA. The
`publish=false` 18-wheel rehearsal must still run from the
post-corrective implementation commit; Python 3.15 support remains
unclaimed until that rehearsal evidence lands.

**Gated futures:** HTTPX 1.0 (RC/stable + frozen API + fresh delta),
H3 graduation (independent interop/drain/impairment/upstream evidence), Node
maturation (explicit scope decision). None is registered; none unblocks
active work.

## Blocked / operational work

| Subsystem | Milestone | Blocker |
|---|---|---|
| TLS, proxy, protocols | H3 graduation | Independent non-Quinn interop, GOAWAY/drain, public-origin, impairment, upstream-risk evidence. |
| Python compat | HTTPX 1.0 program | Upstream RC/stable trigger has not fired. |
| Downstream (informational) | EggReplay M013F repro artifact | EggReplay local origin helper must call async TLS shutdown + brief linger after `flush()`; engine is contract-conformant. |

## Closure work and current control points

M006 closed through
`plans/closure/core-transport-policy/006-windows-tls-response-completeness.md`
on 2026-09-25; Stage C rebound to
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`. Legacy closure evidence lives in
the grandfathered flat plan records (see `plans/README.md` § Legacy archive);
each subsystem roadmap's milestone-status table links the controlling legacy
plans.
