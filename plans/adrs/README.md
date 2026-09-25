# Architecture Decision Records

This directory contains durable decisions that affect eggfetch architecture
across milestones or subsystems.

Use an ADR when a question cannot be answered safely inside one implementation
plan without establishing a reusable architectural contract.

## Naming

```text
ADR-NNNN-short-title.md
```

Numbers are monotonically increasing and never reused.

## Status lifecycle

```text
proposed -> accepted -> deprecated or superseded
         `-> rejected
```

Accepted ADRs are historical records. Do not rewrite an accepted ADR to make
a later decision appear original. Create a new ADR and mark the old one
superseded.

## ADR template

```markdown
# ADR-NNNN: Title

Status: proposed

Date: YYYY-MM-DD

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md#...`
- `plans/001-terminology-and-domain-model.md#...`

Affected subsystem roadmaps:

- `plans/subsystems/...`

## Context

## Decision drivers

## Considered options

### Option A — Name

### Option B — Name

## Decision

## Consequences

### Positive

### Negative

### Neutral or deferred

## Compatibility and migration

## Security and reliability implications

## Verification

## Supersession

None.
```

## ADR threshold

An ADR is normally required when a decision:

- changes engine/adapter ownership or the CONNECT-primitive boundary;
- changes pipeline, route selection, or retry/redirect authority;
- establishes or changes a compatibility contract (facades, Stage C binding);
- changes timeout/pool ownership or body-lifecycle semantics;
- changes TLS trust, proxy fallback, or transport security semantics;
- changes the feature-profile matrix or public-surface containment rule;
- changes verification tiers, publication order, or qualification binding.

An ADR is usually unnecessary for local refactors, internal naming cleanup,
implementation-specific data structures, or reversible optimizations that
preserve established contracts.
