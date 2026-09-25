# ADR-0005: Frozen Body Shapes, Single Timeout Owner, Feature-Profile Containment

Status: accepted

Date: 2026-09-25

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md` §4.3, §5, §8

Affected subsystem roadmaps:

- `plans/subsystems/core-transport-policy-roadmap.md`
- `plans/subsystems/performance-footprint-roadmap.md`

## Context

The public `ResponseBody` shape, timeout ownership, and feature matrix are
the three structures most exposed to well-meaning but destabilizing edits:
new public timeout fields, parallel timeout owners, micro-features for
negligible savings, or public helpers beside contained surfaces.

## Decision drivers

- Exhaustive matching stability for native consumers.
- Exactly one high-level timeout owner.
- Embedders pay only for what they use, with truthful ownership.

## Considered options

### Option A — Freeze + contain (selected)

`ResponseBody` public variant shapes frozen (no new timeout fields/variants,
no `#[non_exhaustive]`); `BodyTimeoutStream` the single high-level timeout
owner. Core default `http1 + tls-rustls + tls-native-roots` (`http1` alone is
cleartext-only); canonical recipes in
`docs/architecture/feature-flags.md#supported-core-profiles` only. No new
public helpers beside existing Alt-Svc, lifecycle, metrics, dialer, or pool
surfaces. Residual footprint splits require evidence; stop rather than
proliferate micro-features.

### Option B — Evolve freely per need

Rejected: breaks exhaustive consumers, splits timeout authority, and
fragments the feature graph.

## Decision

Option A. `max_decoded_body_size` remains the authoritative stream-level
bound for unknown/false `Content-Length` metadata bodies. `crypto_provider()`
stays per-config, never process-global.

## Consequences

### Positive

- Stable public surface for embedders and facades.
- Bounded feature matrix the oracle can actually check.

### Negative

- Genuine new needs require an ADR and containment proof (intended).

### Neutral or deferred

- None.

## Compatibility and migration

Additive capability only via accepted ADR; existing profiles unchanged.

## Security and reliability implications

Single timeout owner prevents competing deadline enforcement; frozen shapes
prevent silent semantic smuggling through new variants.

## Verification

Six-profile public API oracle + semver cross-check (Tier 2), feature-matrix
builds, `rust-surface-containment` inventory review.

## Supersession

None. Codifies the existing surface/timeout/feature discipline.
