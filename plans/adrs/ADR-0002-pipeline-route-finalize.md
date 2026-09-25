# ADR-0002: Pipeline Route/Finalize Shape with Exhaustive Rebuilds

Status: accepted

Date: 2026-09-25

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md` §4.5, §5, §8

Affected subsystem roadmaps:

- `plans/subsystems/core-transport-policy-roadmap.md`

## Context

Retry, redirect, preparation, transport selection, and post-transport policy
interact on every request. Ad-hoc hop construction risks silently dropping
fields (auth, cookies, hints, pins, budgets) on retry/redirect hops.

## Decision drivers

- No silent field drops across hops.
- One post-transport policy for all routes.
- Lean profiles must compile out policy machinery, not stub it at runtime.

## Considered options

### Option A — Staged pipeline with typed rebuilds (selected)

`prepare` → `route` (`select_route()`) → `finalize`, entered via `retry`,
`redirect`, or `lean` single-hop. Retry/redirect hops rebuild through
exhaustive helpers (`retry_request` / `advance_redirect_hop`). Hyper client
construction centralized in `transport/hyper_client.rs`. Forward stays
H1-only.

### Option B — Per-route pipelines

Each transport owns its full lifecycle. Rejected: policy duplication across
UDS/dialer/direct/proxy/SNI/H3/standard routes.

## Decision

Option A. UDS/dialer/pinning/SNI arms require `advanced-routing` and fail
closed in lean profiles. `TransportHints` survive retry; redirect hops clear
them except same-origin resolved destination. No public helpers beside the
existing Alt-Svc, lifecycle, metrics, dialer, or pool surfaces (inventory:
`docs/architecture/rust-surface-containment.md`).

## Consequences

### Positive

- Compile-time guarantee against hop field drops.
- Uniform Alt-Svc learning, decompression, and lease handling.

### Negative

- New fields require touching the rebuild helpers (intended friction).

### Neutral or deferred

- None.

## Compatibility and migration

Internal only. Public client/builder/proxy declarations stay at canonical
paths.

## Security and reliability implications

Cross-origin redirect stripping, replayability checks, and typed proxy
fallback all execute at defined pipeline stages; a second pipeline would
need independent qualification.

## Verification

Route-selection unit tests, redirect/retry matrices, feature-profile
containment checks (Tier 1 + Tier 2 matrix).

## Supersession

None. Codifies the existing pipeline decomposition.
