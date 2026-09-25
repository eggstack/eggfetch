# eggfetch Long-Term Implementation Roadmap

Status: execution roadmap for `plans/000-long-term-specification.md`

Terminology: `plans/001-terminology-and-domain-model.md`

This roadmap orders the work needed to hold and extend the eggfetch end state.
Milestones A–Z, N–M, and production tracks A–D are complete; their record is
`plans/ROADMAP.md` (historical, non-normative). This document plans forward
from the qualified 0.2.x engine: it keeps every phase's exit evidence
explicit and treats missing evidence as a blocker, never a pass.

The roadmap is dependency-ordered, not calendar-ordered. Parallel work is
appropriate only where the dependency notes allow it.

## Cross-phase execution rules

Every phase MUST:

1. preserve the single-engine and scheduler-free adapter invariants (§8 of the
   specification);
2. route new request fields through the exhaustive typed rebuild helpers;
3. keep public `ResponseBody` variant shapes frozen;
4. invalidate exact-SHA Stage C bindings on any executable or
   qualification-input change, and requalify from a new freeze;
5. record residuals in `docs/residual-differences.md` rather than hiding them;
6. use the Tier 1/2/3 gates of `docs/verification-policy.md` with no new
   automatic checks except under that policy's budget rules;
7. update architecture documentation and static ownership guards with code;
8. leave lean profiles compiling and behaving per
   `docs/architecture/feature-flags.md`;
9. keep experimental surfaces (H3, Node, HTTPX 1.0 preview) explicitly labeled;
10. record explicit exit evidence in the milestone closure record before any
    dependent phase treats the work as available.

## Phase 0 — Qualified engine baseline (closed)

Objective: hold the 0.2.x engine as the stable foundation for all later work.

Deliverables (all landed): single-engine ownership, async-first adapters,
feature-profile containment, pipeline decomposition, centralized Hyper
construction, typed proxy fallback, total-deadline ownership, frozen
`ResponseBody` shapes, private-architecture containment, second-pass
performance ownership work, footprint qualification records.

Exit criteria: Tier 1 green; Stage C bound to the live freeze in
`plans/httpx-parity-correction-status.md`. Closed; evidence in the legacy
plan records indexed by `plans/registry.md`.

## Phase 1 — Release publication (active)

Objective: publish the qualified 0.2.x line through the manual release path
with no automation or policy change.

Deliverables:

- Coordinated crates.io publication in dependency-leaf order, then tag, per
  `docs/releases/process.md` (pending maintainer action, issue #24 line).
- PyPI publication only via manually dispatched `pypi.yml` with Trusted
  Publishing; live security preflight green at publication time.

Dependencies: Phase 0 closed; live Stage C binding current at publication.

Exit criteria: published versions verified (`cargo search`, installed-wheel
smoke); ledger and compatibility docs rebound only if publication changes
executable inputs (it SHOULD NOT).

Required gates: Tier 2 + Tier 3 + `check_security.sh` from a trusted local
environment. No CI change belongs in this phase.

## Phase 2 — Python 3.15 wheel production (active)

Objective: extend the wheel matrix to CPython 3.10–3.15 (18 wheels + 1 sdist)
with a build-only rehearsal before any 3.15 support claim.

Deliverables (per legacy plan `plans/python-3.15-pypi-wheel-production.md`):

- Matrix builds for Linux x86_64, macOS arm64, Windows x86_64.
- Coverage validator, package classifier, and release documentation match.
- Required `publish=false` 18-wheel rehearsal dispatched from the
  implementation commit before claiming 3.15 support.

Dependencies: Phase 0 closed. Independent of Phase 1 except that a published
3.15 claim requires both.

Exit criteria: rehearsal artifacts assemble (19 distributions) with Tier 1 +
package validation green locally; 3.15 support claimed only after rehearsal.

## Phase 3 — Corrective maintenance (standing)

Objective: absorb bounded corrective passes without reopening closed scope.

Deliverables: corrective implementation plans under
`plans/implementation/<subsystem>/` referencing the original milestone and
closure record, each with regression guards and renewed exact-SHA
qualification where executable inputs changed.

Dependencies: none beyond the affected subsystem roadmap. Repeated
correctives in one subsystem REQUIRE a roadmap or sizing revision.

Exit criteria per corrective: closure record with requirement-to-evidence
matrix, zero unexplained drift, gates green on the new freeze where
applicable.

Non-goals: public visibility cleanup, feature-graph simplification, new
transports, Node maturation, H3 graduation, Python trailer exposure,
Trio/AnyIO, new CLI features, new FFI symbols, compatibility waivers — none
belongs in a corrective unless its own milestone plan opens it.

## Phase 4 — Gated futures (not opened)

Each item opens only on its named trigger, as its own subsystem roadmap +
milestone plan. None is approved work today.

1. **HTTPX 1.0 compatibility program.** Trigger: upstream RC with frozen
   public API (or stable 1.0) + no further reset signal + fresh pinned delta
   inventory. Dev releases never open work. Never by renaming the preview
   plan into a Stage C plan.
2. **HTTP/3 production graduation.** Trigger: independent non-Quinn interop,
   independent GOAWAY/drain, public-origin, realistic impairment, and
   upstream-risk closure evidence (ledger:
   `plans/http3-independent-interop-and-impairment-qualification-evidence.json`).
   Graduation decision lives in
   `docs/architecture/core-tls-proxy-protocols.md`.
3. **Node binding maturation.** Trigger: explicit maintainer scope decision
   superseding `plans/node-binding-maturation.md` (prototype verdict). No
   silent capability growth.
4. **New transports / auth schemes / public helpers.** Trigger: accepted ADR
   proving the addition cannot live behind existing surfaces (see inventory
   rule in `docs/architecture/rust-surface-containment.md`).

## Recommended immediate execution sequence

```text
Phase 1  release publication (maintainer action)
Phase 2  Python 3.15 wheel rehearsal (then claim)
Phase 3  correctives as needed, each bounded and requalified
```

Phase 4 items remain gated; none unblocks or is unblocked by Phases 1–3.

## Roadmap governance

Implementation plans derived from this roadmap SHOULD cite the exact phase
and specification sections they satisfy. A phase is not complete because code
exists; it is complete only when its ownership model, gates, compatibility
evidence, documentation, and closure record are present.

When implementation reveals that a term or ownership boundary is wrong, update
the terminology document and long-term specification first, then adjust this
roadmap. Avoid accumulating incompatible local meanings in phase plans.

New scope SHOULD be evaluated against the non-goals in the specification.
Features that do not strengthen the engine, its adapters, its compatibility
evidence, or its release integrity SHOULD NOT displace the roadmap's core work.
