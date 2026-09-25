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
| Core transport and request policy | closed | `plans/subsystems/core-transport-policy-roadmap.md` | All milestones closed | Standing corrective intake only (`plans/003-planning-process.md` §7). |
| Python bindings and HTTPX compatibility | closed | `plans/subsystems/python-httpx-compat-roadmap.md` | M001-M003 closed on live Stage C binding | Facade work reopens only via gated roadmap Phase 4 trigger. |
| TLS, proxy, and protocols | closed | `plans/subsystems/tls-proxy-protocols-roadmap.md` | All milestones closed; H3 experimental retained | H3 graduation blocked on named external evidence. |
| Release and verification | active | `plans/subsystems/release-verification-roadmap.md` | M001 publication + M002 wheel rehearsal open | Operational: maintainer dispatch/publication. |
| Performance and footprint | closed | `plans/subsystems/performance-footprint-roadmap.md` | Campaigns closed; fixtures repaired | New optimization needs its own milestone plan. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| Release and verification | M001 0.2.x publication/tag/PyPI | ready | `plans/implementation/release-verification/001-release-publication.md` | Operational: maintainer runs Tier 2 + Tier 3 + security preflight from a trusted local environment, publishes in leaf order, tags, dispatches PyPI. |
| Release and verification | M002 Python 3.15 wheel rehearsal | ready | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | Operational: maintainer dispatches `publish=false` rehearsal from the implementation commit; legacy detail in `plans/python-3.15-pypi-wheel-production.md`. |

## Current execution order and dependency gates

**Release publication gate:** M001 is ready for maintainer execution. It
changes no executable inputs by design (version bumps and tags only); if
publication preparation changes executable inputs, stop and open a
requalification milestone first — the live Stage C binding MUST NOT be
carried across silently.

**Wheel rehearsal gate:** M002 is ready. The `publish=false` 18-wheel
rehearsal MUST be dispatched from the implementation commit before any 3.15
support is claimed in a published release. Python 3.15 support remains
unclaimed until rehearsal evidence lands.

**Corrective gate:** Closed subsystems accept corrective passes as new
implementation plans under their own directory, each referencing the original
milestone and closure record with regression guards. Repeated correctives in
one subsystem require a roadmap revision.

**Gated futures:** HTTPX 1.0 (RC/stable + frozen API + fresh delta),
H3 graduation (independent interop/drain/impairment/upstream evidence), Node
maturation (explicit scope decision). None is registered; none unblocks
active work.

## Blocked / operational work

| Subsystem | Milestone | Blocker |
|---|---|---|
| Release and verification | M001 publication/tag/PyPI | Maintainer action (issue #24 line): manual `cargo publish` in order, `v0.x` tag, PyPI dispatch. |
| Release and verification | M002 wheel rehearsal | Maintainer action: `publish=false` rehearsal dispatch not yet run. |
| TLS, proxy, protocols | H3 graduation | Independent non-Quinn interop, GOAWAY/drain, public-origin, impairment, upstream-risk evidence. |
| Python compat | HTTPX 1.0 program | Upstream RC/stable trigger has not fired. |

## Closure work and current control points

No new closure records are open. Legacy closure evidence lives in the
grandfathered flat plan records (see `plans/README.md` § Legacy archive);
each subsystem roadmap's milestone-status table links the controlling legacy
plans. New milestones will record closure under `plans/closure/<subsystem>/`
per `plans/closure/README.md`.
