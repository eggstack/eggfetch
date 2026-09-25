# eggfetch Planning System

> **Current execution gate:** M006's fixture-teardown diagnosis remains the
> technical baseline, but **M006C1 — Windows qualification and downstream
> closure corrective is ready**. Native-Windows EggFetch evidence and the
> corrected EggReplay M013F hosted-Windows 300 KiB rerun are still required.
> Issue #24 publication/tag/PyPI and the Python 3.15 wheel rehearsal are
> blocked until M006C1 closes. Stage C remains
> `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa` unless executable or
> qualification inputs change.
> **Live status for agents:** the exact-SHA Stage C binding is
> `plans/httpx-parity-correction-status.md` (+ `compat/*/profile.toml`).
> Completed sections and legacy plan files are historical records, not
> current gates (verification-policy principle 9). Validation tiers:
> `.skills/verification-qualification.md`.

This directory separates durable architectural direction from temporary
execution planning. Since the 2026-09-25 migration it follows the
convention below (adapted from the codegg planning style): canonical
long-term documents, ADRs, subsystem roadmaps, bounded milestone handoffs,
closure records, a compact registry, and a frozen legacy archive.

## Canonical long-term documents

The following files define the intended product and architecture and MUST
NOT be edited as part of ordinary implementation work:

- `000-long-term-specification.md` — normative end-state specification and
  invariants.
- `001-terminology-and-domain-model.md` — normative language and identity
  model (engine, profiles, routes, deadlines, facades, Stage C).
- `002-long-term-roadmap.md` — dependency-ordered forward roadmap (Phases
  0–4; milestones A–Z remain historical in `ROADMAP.md`).
- `003-planning-process.md` — rules for deriving and managing interim plans.

The first three documents are stable architectural references. Changes to
them require an explicit long-term architecture decision, not an
implementation convenience. Interim plans MUST reference them rather than
copying or silently revising their requirements.

## Planning hierarchy

```text
Long-term specification and terminology
        |
        v
Architecture decision records
        |
        v
Forward roadmap (002) + subsystem roadmaps
        |
        v
Milestone implementation plans
        |
        v
Implementation and verification
        |
        v
Closure records and archive
```

## Directory roles

- `adrs/` — durable architecture decisions. Accepted decisions are
  superseded, not rewritten.
- `subsystems/` — subsystem specifications and dependency-ordered roadmaps
  translating the long-term documents into coherent workstreams.
- `implementation/` — focused milestone plans handed to implementation
  agents. Operational; may evolve as code changes.
- `closure/` — verification, evidence, residual-risk, and completion records
  for implemented milestones.
- `archive/` — completed or superseded interim planning retained for
  traceability (plus the grandfathered flat-file archive below).
- `registry.md` — compact index of active roadmaps, implementation plans,
  closure work, gates, and blockers.

## Core rule

Long-term documents state **what eggfetch is becoming and what must remain
true**. Interim documents state **what an agent should implement next
against a specific repository baseline**.

Implementation agents MUST NOT add commit-specific steps, transient file
lists, current test counts, or short-lived corrective work to the canonical
long-term documents.

## Planning lifecycle

1. Identify the relevant long-term specification sections and invariants.
2. Record any unresolved architectural decision in `adrs/`.
3. Create or update a subsystem roadmap in `subsystems/`.
4. Select one dependency-ready milestone.
5. Write a bounded handoff plan under `implementation/`.
6. Implement and verify the milestone (Tier 1 always; Tier 2/3 + renewed
   exact-SHA qualification when executable inputs changed).
7. Write a closure record under `closure/`.
8. Update `registry.md` and the subsystem roadmap status.
9. Move completed or superseded interim documents to `archive/` when they no
   longer represent active work.

No milestone is complete merely because code landed. Completion requires the
closure evidence defined by its implementation plan and subsystem roadmap.

## Required classification

Every subsystem roadmap and implementation plan MUST distinguish:

- **Invariant** — a property that must always remain true.
- **Capability** — user- or operator-visible behavior.
- **Infrastructure** — internal machinery required by capabilities.
- **Polish** — ergonomics, diagnostics, performance tuning, cleanup, or
  documentation.

Infrastructure and polish MUST NOT be presented as completed user capability
unless the user-visible acceptance criteria are actually satisfied.

## Naming conventions

- ADR: `adrs/ADR-NNNN-short-title.md`
- Subsystem roadmap: `subsystems/<subsystem>-roadmap.md`
- Milestone implementation plan: `implementation/<subsystem>/NNN-short-title.md`
- Closure record: `closure/<subsystem>/NNN-status.md`
- Archived document: retain its original relative structure beneath `archive/`

Use stable subsystem names (`core-transport-policy`, `python-httpx-compat`,
`tls-proxy-protocols`, `release-verification`, `performance-footprint`). Do
not encode dates in filenames unless the document is inherently time-bound.

## Starting a new workstream

Begin with `subsystems/README.md`, then use the templates and rules in:

- `adrs/README.md`
- `implementation/README.md`
- `closure/README.md`

Register active work in `registry.md` before handing implementation plans to
agents.

## Normative live compatibility records

`httpx-parity-correction-status.md` remains the live exact-SHA status for
both facades. Profiles: `compat/httpx/0.28.1/profile.toml`,
`compat/httpx2/2.12.0/profile.toml`. Preview: `compat/httpx/1.0-preview/`
(unqualified by design; status in `preview-status.toml`).

### HTTPX 1.0 migration trigger

A future HTTPX 1.0 implementation/qualification program opens only when all
three hold (recorded here and in `plans/ROADMAP.md`; never by renaming the
preview plan into a Stage C plan):

1. upstream publishes an RC with an explicitly frozen public API, or a
   stable 1.0 release;
2. release notes indicate no further major compatibility reset before stable;
3. a fresh delta inventory shows the target is stable enough to justify
   implementation, pinned to the exact RC/stable release.

## Legacy archive (pre-migration flat plans)

The ~250 flat `plans/*.md` files (milestones A–Z, parity phases, programs,
correctives, closures) plus `ROADMAP.md` are frozen historical records kept
at their original paths, because qualification evidence, compatibility docs,
and profiles cite them by path. They remain valid evidence citations but
MUST NOT be extended with new active scope — new work uses the taxonomy
above. Each subsystem roadmap's milestone-status section links its
controlling legacy plans. This index and `registry.md` are the authoritative
navigation layers; legacy files stay in place.

Plan files stay in place; this index is the authoritative navigation layer.
