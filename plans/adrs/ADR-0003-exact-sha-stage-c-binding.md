# ADR-0003: Exact-SHA Stage C Qualification Binding

Status: accepted

Date: 2026-09-25

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md` §4.6, §6, §8
- `plans/002-long-term-roadmap.md` cross-phase rules

Affected subsystem roadmaps:

- `plans/subsystems/python-httpx-compat-roadmap.md`
- `plans/subsystems/release-verification-roadmap.md`

## Context

Compatibility claims ("drop-in for HTTPX 0.28.1 / httpx2 2.12.0") are only
meaningful against the exact code that was tested. Past practice bound claims
to branches or recomputed them after unrelated changes, producing stale or
overstated bindings the corrective history had to repair.

## Decision drivers

- Claims must match tested behavior.
- Requalification cost must fall only on changes that affect evidence.
- Live status must have exactly one authoritative location.

## Considered options

### Option A — Exact-SHA binding (selected)

Each Stage C qualification binds to one executable freeze SHA recorded in
`plans/httpx-parity-correction-status.md` + `compat/*/profile.toml`. Any
executable or qualification-input change invalidates the binding; docs-only
descendants do not. Live SHAs are never hardcoded outside the canonical
qualification records.

### Option B — Branch-bound claims

"Stage C on main" as a standing property. Rejected: every commit would
silently inherit or silently void the claim.

## Decision

Option A. Corrective and maintenance work that changes executable inputs
MUST renew qualification from a new freeze and rebind the live records,
preserving the old binding as history. Residuals live in
`docs/residual-differences.md` and MUST NOT be papered over to keep a binding.

## Consequences

### Positive

- Falsifiable, auditable compatibility claims.
- Docs/state-only passes stay cheap by construction.

### Negative

- Executable correctives always pay requalification cost (intended).

### Neutral or deferred

- None.

## Compatibility and migration

No consumer migration. Records the rule already enforced by
`.skills/verification-qualification.md` and the Tier 2 compat gates.

## Security and reliability implications

Prevents shipping security-relevant behavior changes under a stale
compatibility banner.

## Verification

Tier 2 full compat suites + both API oracles (zero unexplained) on the
freeze; descendant audit proving post-freeze changes are docs/profile-only.

## Supersession

None. Codifies the existing exact-SHA rule.
