# ADR-0004: Typed Proxy Fallback and Outer Total-Deadline Ownership

Status: accepted

Date: 2026-09-25

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md` §5, §8

Affected subsystem roadmaps:

- `plans/subsystems/core-transport-policy-roadmap.md`
- `plans/subsystems/tls-proxy-protocols-roadmap.md`

## Context

Proxy fallback and timeout ownership interact dangerously: an over-eager
fallback masks auth/policy failures, and caching a request's shrinking total
deadline inside a reusable connector lets one request's budget leak into
another's (the defect class repaired by the proxy total-deadline corrective).

## Decision drivers

- Fallback must advance only on evidence the alternate path can succeed.
- One request's timeout state must never inhabit shared connectors.
- Total must span the full body lifecycle including EOF/trailers.

## Considered options

### Option A — Typed fallback + outer deadline (selected)

CONNECT advances on 502/504; local SOCKS5 advances on destination-specific
0x03/0x04/0x05; auth/policy/protocol/malformed failures stop. Total is
enforced by the outer dispatch across the `PoolGuard` body lifecycle;
reusable forward/CONNECT connectors carry no request-total state. Read stays
first-poll/per-chunk inactivity; total never resets and wins ties.

### Option B — String-matched fallback with connector-local totals

Rejected: masks configuration errors as transport flakes and leaks budgets
across requests.

## Decision

Option A. Compatible forward-proxy and single-target CONNECT clients are
cached by connection-affecting route policy only.

## Consequences

### Positive

- Predictable proxy behavior under failure.
- No cross-request timeout contamination.

### Negative

- New fallback-worthy signals require explicit typed evidence (intended).

### Neutral or deferred

- None.

## Compatibility and migration

Internal. Public proxy/route configuration unchanged.

## Security and reliability implications

Auth failures never trigger silent alternate-path retries; total deadlines
cannot be stretched by connector reuse.

## Verification

Proxy fallback matrices, total-deadline lifecycle tests (incl. native
lease-release proof), Tier 1 + Tier 2 lifecycle gates.

## Supersession

None. Codifies the existing proxy timeout/fallback ownership.
