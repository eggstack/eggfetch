# eggfetch Active Planning Registry

This is the control surface for current planning. Detailed requirements and
completed evidence remain in subsystem roadmaps, closure records, and the
historical plan archive.

Canonical direction:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`

Live compatibility binding:

- `plans/httpx-parity-correction-status.md`
- `compat/httpx/0.28.1/profile.toml`
- `compat/httpx2/2.12.0/profile.toml`

## Current release

The current public version is 0.3.0 across the six publishable crates and the
Python distribution. No release milestone is active. The publication evidence
is in `plans/closure/release-verification/008-0-3-0-publication.md`.

Validation tiers are defined in `.skills/verification-qualification.md`; the
normative policy is `docs/verification-policy.md`.

## Status vocabulary

- **proposed:** not approved for execution.
- **ready:** dependencies and interfaces are satisfied; the plan may be handed off.
- **active:** implementation or closure work is in progress.
- **blocked:** a named dependency or evidence requirement prevents progress.
- **closing:** implementation landed and closure evidence is being gathered.
- **closed:** the closure record is accepted.
- **conditionally closed:** implementation is complete, but a named condition remains.
- **superseded:** replaced by another document.
- **archived:** no longer active and retained for traceability.

## Subsystem roadmaps

| Subsystem | Roadmap status | Roadmap | Current state |
|---|---|---|---|
| Core transport and request policy | active | `plans/subsystems/core-transport-policy-roadmap.md` | No implementation milestone is registered. |
| Python bindings and HTTPX compatibility | active | `plans/subsystems/python-httpx-compat-roadmap.md` | No implementation milestone is registered. HTTPX 1.0 remains gated on its upstream trigger. |
| TLS, proxy, and protocols | active | `plans/subsystems/tls-proxy-protocols-roadmap.md` | No implementation milestone is registered. H3 graduation remains separately gated. |
| Release and verification | active | `plans/subsystems/release-verification-roadmap.md` | 0.3.0 publication is closed. |
| Performance and footprint | closed | `plans/subsystems/performance-footprint-roadmap.md` | Completed campaigns and repaired fixtures. |

No implementation plan is ready or active. Register new work in this file and
its subsystem roadmap before handing off a plan. Closed plans are historical
evidence, not active requirements or release gates.

## Gated futures

- **HTTPX 1.0:** wait for an upstream release candidate or stable release with a
  frozen API, then perform a fresh delta review.
- **H3 graduation:** requires independent interoperability, drain, impairment,
  public-origin, and upstream-risk evidence. H3 remains experimental.
- **Node.js maturation:** requires an explicit scope decision.

None of these is unblocked by the closed milestones.

## Pending maintainer actions

| # | Action | Owner | Reference |
|---|---|---|---|
| 2 | Decide whether CLI work gets its own subsystem roadmap or belongs under an existing roadmap. | Project maintainers | `plans/implementation/release-verification/007-planning-and-documentation-hygiene.md` §5 |
| 6 | Decide whether the IPv6 pool-key divergence needs its own milestone. The high-level and native paths currently produce different keys for the same IPv6 origin. | Project maintainers | `plans/closure/core-transport-policy/010-core-duplicate-helper-consolidation.md` §10 |

## Operational blockers

| Area | Blocker |
|---|---|
| HTTPX qualification | HTTPX 1.0 has not met its upstream trigger. |
| H3 | Graduation evidence is independent of the existing H3 qualification and remains outstanding. |
| Tier 2 downstream fixtures | The gate requires `target/downstream-qualification/artifact-manifest.json` and fails closed if it is absent. |

Closure records hold milestone-specific evidence and decisions. The live
binding is recorded in the ledger, profiles, and canonical compatibility pages:
`docs/residual-differences.md`, `docs/reference/compatibility.md`, and
`docs/reference/compatibility-stage-decision.md`. Other planning documents
should link to these records rather than duplicate SHA values.
