# Subsystem Roadmaps

Subsystem roadmaps translate the canonical eggfetch direction into coherent,
dependency-aware workstreams. They are not direct coding-agent checklists.

Each roadmap should remain useful across several implementation milestones
and repository revisions. Commit-specific mechanics belong in
`plans/implementation/`.

## Naming

```text
<subsystem>-roadmap.md
```

Current subsystems: `core-transport-policy`, `python-httpx-compat`,
`tls-proxy-protocols`, `release-verification`, `performance-footprint`.

## Required roadmap structure

```markdown
# <Subsystem> Roadmap

Status: proposed | active | closing | closed | superseded

Long-term references:

- `plans/000-long-term-specification.md#...`
- `plans/001-terminology-and-domain-model.md#...`
- `plans/002-long-term-roadmap.md#...`

Related ADRs:

- `plans/adrs/ADR-NNNN-...md`

## 1. Purpose and ownership boundary

## 2. Work classification

### Invariants

### Capabilities

### Infrastructure

### Polish

## 3. Non-goals

## 4. Current state

## 5. Target architecture

## 6. Dependency graph

Classify each dependency as hard, interface, soft, or operational.

## 7. Milestones

### Milestone N — Title

Class: invariant | capability | infrastructure | polish
Status: ...
Objective:
Dependencies:
Deliverable boundary:
User or operator value:
Exit conditions:
Deferred work:

## 8. Cross-cutting requirements

Storage/migration; protocol/compatibility; security; cancellation/timeout/
pool semantics; observability; performance; documentation/operations.

## 9. Verification strategy

## 10. Risks and decision points

## 11. Completion definition

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
```

## Roadmap rules

A subsystem roadmap MUST link to canonical requirements rather than
duplicating them, define ownership before milestones, distinguish
infrastructure from completed capability, expose dependencies and decision
points, preserve completed milestone history, link each active milestone to
one implementation plan and later one closure record, state non-goals, and
remain at subsystem level. Material changes must record why the roadmap
changed. Create only roadmaps ready to be reasoned about.
