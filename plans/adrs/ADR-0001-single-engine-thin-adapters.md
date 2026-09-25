# ADR-0001: Single Async Engine with Thin Adapters

Status: accepted

Date: 2026-09-25

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md` §4.1, §4.2, §5, §8

Affected subsystem roadmaps:

- `plans/subsystems/core-transport-policy-roadmap.md`
- `plans/subsystems/python-httpx-compat-roadmap.md`
- `plans/subsystems/tls-proxy-protocols-roadmap.md`

## Context

eggfetch serves native Rust, Python sync/async, two compat facades, a CLI, a
C ABI, and a Node prototype. Without an ownership rule, each adapter grows
its own transport, timeout, or retry logic and the surfaces drift apart (the
failure mode this repository's corrective history repeatedly repairs).

## Decision drivers

- One audited behavior across all surfaces.
- Adapter ergonomics must stay idiomatic per surface.
- The CONNECT wire format is shared but has no transport policy.

## Considered options

### Option A — Single engine, thin adapters

All HTTP logic in `eggfetch-core`; adapters parse input, call the engine,
format output. Python sync blocks on the async engine with GIL released.

### Option B — Per-adapter transports

Each surface owns its networking for maximal API freedom. Rejected: behavior
drift, duplicated security surface, unqualifiable compatibility claims.

## Decision

Option A. All network I/O lives in `eggfetch-core`. The sole exception is
`eggfetch-http-connect`, which owns generic CONNECT wire bytes only (target
formatting, serialization, bounded head parsing) with no sockets, TLS,
retry, or policy, under the `proxy` feature. `eggfetch-cli` and
`eggfetch-python` perform no direct hyper/tokio TCP. New request fields go
through the exhaustive typed rebuild helpers so omission fails to compile.

## Consequences

### Positive

- One behavior to test, qualify, and secure.
- Compatibility evidence transfers across adapters sharing the engine.

### Negative

- Engine changes require cross-adapter awareness.
- Adapter-specific needs must be expressed as engine capabilities, not forks.

### Neutral or deferred

- None.

## Compatibility and migration

No migration: this records the existing enforced boundary (see
`scripts/check_adapter_features.py` and `docs/architecture/overview.md`).

## Security and reliability implications

Single policy enforcement point for timeouts, TLS, proxy auth, and secret
redaction. A second networking path would bypass every ownership guard.

## Verification

Adapter feature-ownership check in Tier 1; clippy/deny gates; architecture
review of any new transport-adjacent code.

## Supersession

None. First recorded ADR under the new planning convention; codifies the
pre-existing enforced rule.
