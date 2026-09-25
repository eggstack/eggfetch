# Milestone Implementation Plans

Bounded plans handed directly to implementation agents. Operational documents
tied to the current repository state; they may be corrected, superseded, or
archived without modifying canonical long-term documents.

## Layout and naming

```text
implementation/<subsystem>/NNN-short-title.md
```

Milestone numbering is local to the subsystem roadmap.

## Required implementation-plan template

```markdown
# <Subsystem> Milestone NNN — <Title>

Status: ready | active | blocked | closing | closed | superseded

Repository baseline: `<commit SHA or branch state>`

Source roadmap:

- `plans/subsystems/<subsystem>-roadmap.md#...`

Long-term requirements:

- `plans/000-long-term-specification.md#...`
- `plans/001-terminology-and-domain-model.md#...`

Applicable ADRs:

- `plans/adrs/ADR-NNNN-...md`

Primary class: invariant | capability | infrastructure | polish

## 1. Objective

## 2. Why this milestone is ready

## 3. Current implementation evidence

## 4. Invariants that must not regress

## 5. Scope

### In scope

### Explicitly out of scope

## 6. Required production changes

## 7. Ordered work packages

## 8. Failure, cancellation, timeout, and pool semantics

## 9. Compatibility and migration (incl. feature-profile effects)

## 10. Required tests

## 11. Required verification commands

```bash
# Tier 1 first; Tier 2/3 only where the roadmap phase requires them
```

## 12. Documentation updates

## 13. Acceptance criteria

## 14. Stop conditions

## 15. Closure evidence required

## 16. Handoff notes
```

## Handoff rules

Before assigning a plan: confirm the baseline is current; confirm hard
dependencies are closed; confirm unresolved decisions have ADRs or are out of
scope; ensure one-coherent-pass sizing; ensure tests and closure evidence are
specific; register the plan in `plans/registry.md`.

The agent may adjust file-level mechanics based on current code. It may not
weaken canonical invariants, change the Stage C binding implicitly, or
silently enlarge scope.

## Corrective plans

Corrective work receives a new plan in the same subsystem directory. It must
reference the original plan and closure record, enumerate unclosed findings,
and add regression evidence that would have caught them. When executable
inputs changed, it must renew exact-SHA qualification.
