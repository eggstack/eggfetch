# eggfetch Canonical Terminology and Domain Model

Status: normative companion to `plans/000-long-term-specification.md`

This document defines the language eggfetch implementation plans,
compatibility records, architecture documents, tests, user labels, and
operator documentation MUST use. When legacy plans use a term differently, the
compatibility mapping in §13 describes the migration target.

## 1. Naming rules

1. A durable product concept MUST use the term defined here rather than a
   local synonym.
2. A profile name MUST refer to the exact feature set in
   `docs/architecture/feature-flags.md`, never to an assumed capability.
3. A timeout phase MUST use its `TimeoutPhase` name; `total` and `read` MUST
   NOT be interchanged.
4. Compatibility evidence MUST cite the exact executable SHA it was measured
   on, via the live ledger, never by copying a SHA into prose as current.
5. Legacy plan filenames remain valid historical references but MUST NOT be
   treated as the current planning taxonomy (§13).

## 2. Engine and adapter terms

### Engine

`eggfetch-core` plus the `eggfetch-http-connect` CONNECT wire primitive it
owns. The sole authority for HTTP behavior. Suggested reference: `engine`.

### Adapter

`eggfetch-cli`, `eggfetch-python`, `eggfetch-ffi`, or `eggfetch-node`. An
adapter parses input, calls the engine, and formats output. It MUST NOT own
transport, pooling, timeout, retry, or TLS policy.

### CONNECT wire primitive

`eggfetch-http-connect`: generic CONNECT target formatting, request
serialization, and bounded response-head parsing over caller-owned buffers.
No sockets, TLS, retry, or policy.

### Pipeline

The crate-private lifecycle `prepare.rs` → `route.rs` (`select_route()`) →
`finalize.rs` (one post-transport policy), entered via `retry.rs`
(`logical-retry`), `redirect.rs` (only `redirects`), or `lean.rs`
single-hop when both are absent.

## 3. Profile terms

### High-level profile

`http1`/`http2` aliases: `native-http1`/`native-http2` + `high-level-url` +
`logical-retry` + `redirects` + `basic-auth`. String-URL ergonomics with
full policy.

### Native profile

`native-http1`/`native-http2`: `http::Request`/`http::Uri` transport without
`url`/`idna`/ICU. Native callers own IDNA/punycode before constructing the URI.

### Lean profile

`standard-http1`/`standard-http2` (+ `tls-rustls`): Bearer-only
single-attempt standard-route client; 3xx returned without following.
`transport-http1` + `standard-route` without `advanced-routing` is the
leanest native transport.

### Advanced routing

Opt-in `advanced-routing` machinery: custom `Dialer`, resolved-target/SNI
override, socket-option/local-address, UDS. Absent from lean profiles, where
those arms fail closed.

## 4. Request lifecycle terms

### Route

The declarative transport selection for one hop: UDS → dialer →
static/direct → proxy/SOCKS → SNI → H3 → standard. Selected once per hop by
`select_route()`.

### Resolved target

Caller-supplied pinned destination (`RequestBuilder::proxy_target_addresses()`
/ `Proxy::resolved_addresses()` / `ResolvedTarget`). Preserves logical
URL/Host/TLS identity, never falls back to DNS, fails closed on incompatible
routes and cross-origin redirect hops (same-origin retains only the pin).

### TransportHints

`TransportHints` / `NativeRequestOptions`: protocol-neutral wire overrides
(SNI hostname, pinned destination, trace). They override wire behavior only
and survive retry; redirect hops clear them except same-origin resolved
destination.

### Total deadline

`Timeout.total`: one absolute per-dispatch deadline spanning response-body
EOF/trailers. Never resets, wins ties, enforced by the outer dispatch across
the `PoolGuard` body lifecycle. MUST NOT be cached in reusable connectors.

### Read timeout

`read`: first-poll/per-chunk inactivity timeout. Distinct from total; the two
MUST NOT be conflated in plans, facades, or documentation.

### PoolGuard lifecycle

Streaming bodies hold an `Arc<PoolGuard>` lease until EOF/drop; buffered
responses release immediately after reading. Origin key: `(scheme, host,
port)` + optional proxy route.

## 5. Compatibility terms

### Facade

`eggfetch.compat.httpx` (HTTPX 0.28.1) or `eggfetch.compat.httpx2` (httpx2
2.12.0). Versioned, independent, coexisting without cross-mutation.

### Stage C

A facade qualification verdict: the documented surface is drop-in compatible
within recorded residuals, proven by the pinned compat suites plus API
oracles on one frozen executable SHA. Stage C is always SHA-bound, never a
standing property of `main`.

### Executable freeze / binding

Executable freeze: the exact commit whose built artifacts produced
qualification evidence. Live binding: the freeze currently recorded in
`plans/httpx-parity-correction-status.md` plus `compat/*/profile.toml`. Any
executable or qualification-input change invalidates the binding.

### Residual

An intentional, documented compatibility difference
(`docs/residual-differences.md`). Residuals are classified, never papered over.

### Planning baseline vs oracle baseline

Planning baseline: the commit a plan was written against. Oracle baseline:
the fixed reference (`03ecba97…`) the six-profile Rust public-surface oracle
compares against. Distinct from the live Stage C SHA. MUST NOT be conflated.

## 6. Trust and identity terms

### Base store

Native roots or WebPKI roots selected by feature and configuration. Native-root
construction failure MAY fall back to WebPKI roots.

### Additional vs replacement CA

`additional_ca_*` augments the base store; `ca_certificate_*` replaces it.
Plans MUST use these exact verbs.

### Crypto provider

Per-`TlsConfig` Rustls `CryptoProvider` selection (`crypto_provider()`).
Per-config, never process-global.

## 7. Proxy terms

### Typed fallback

CONNECT advances on 502/504; local SOCKS5 advances on destination-specific
0x03/0x04/0x05. Auth/policy/protocol/malformed failures stop. No string
matching, no untyped retry.

### Forward vs CONNECT

Forward: plaintext HTTP proxying under Hyper (H1-only). CONNECT: TLS tunnel
establishment for HTTPS/SOCKS destinations. CONNECT tunnels stay
body-iterator only and are never surfaced as `network_stream`.

## 8. Streaming and upgrade terms

### Network stream

Writable IO owned exclusively by 101 (Switching Protocols) responses,
surfaced as `response.extensions["network_stream"]`. Pooled responses map to
`None`.

### SSE / WebSocket

SSE: Python framing over streamed responses. WebSocket: wsproto over the core
101 stream. Never raw sockets from Python.

## 9. Planning terms (new taxonomy)

### Canonical long-term document

`plans/000`–`003`. Stable product direction; amended only for intentional
direction change, contradiction, accepted ADR, or explicit maintainer
direction.

### Subsystem roadmap

`plans/subsystems/<subsystem>-roadmap.md`. One coherent workstream: purpose,
ownership boundary, classification, milestones, dependencies, exit
conditions. Longer-lived than a milestone plan, more adaptable than the
canonical roadmap.

### Milestone implementation plan

`plans/implementation/<subsystem>/NNN-short-title.md`. The primary handoff
artifact: bounded, baseline-tied, independently executable. Status: `ready` |
`active` | `blocked` | `closing` | `closed` | `superseded`.

### Closure record

`plans/closure/<subsystem>/NNN-status.md`. The evidence gate deciding whether
a milestone is complete. A landed commit alone is not closure.

### Corrective pass

A new implementation plan referencing the original milestone and closure
record, enumerating unclosed requirements, explaining the verification gap,
and adding regression guards. Never an edit pretending the original succeeded.

### Program / handoff (legacy)

Older planning vocabulary for a multi-plan campaign (`*-program.md`) and its
entry plan. Retained as historical references; new campaigns use subsystem
roadmaps plus numbered milestone plans.

### Registry

`plans/registry.md`. The compact control surface: active roadmaps,
dependency-ready plans, gates, blockers, latest closure. Links to source
documents, never duplicates them.

## 10. Verification terms

### Tier 1 / Tier 2 / Tier 3

Routine (`scripts/check.sh`), extended (`check.sh extended`), package
(`check.sh package`) validation. Normative: `docs/verification-policy.md`.
Qualification fixtures and H3 are manual, never Tier 1 gates.

### API oracle

The six-profile Rust public-surface check (`cargo-public-api` +
`cargo-semver-checks`) against the oracle baseline, plus the native Python
manifest/typing guards. Tier 2 gates.

### Security preflight

Live `scripts/check_security.sh` RustSec/license/source scan before
publication. Release-time intelligence, not a merge gate.

## 11. Release terms

### Publication order

Manual crates.io publication in dependency-leaf order (http-connect → core →
cli → ffi → python → node, then tag), then PyPI via manually dispatched
`pypi.yml` with Trusted Publishing. `eggfetch-bench` and `fuzz/` are never
published.

### Wheel rehearsal

Build-only (`publish=false`) 18-wheel + 1-sdist matrix run proving the
release set assembles before any publish claim.

## 12. Prohibited ambiguous usage

- "the engine" for an adapter, or "the client" when engine/adapter/facade
  scope matters;
- "total timeout" for read/inactivity behavior, or vice versa;
- "compatible" without naming the facade version, SHA binding, and residuals;
- "qualified" without the exact executable SHA and gate set;
- "proxy support" without forward/CONNECT/SOCKS and typed-fallback scope;
- "H3 support" without the experimental label and graduation blockers;
- "closed" for a milestone with only a landed commit and no closure record;
- "current SHA" for any SHA copied outside the canonical qualification records.

## 13. Compatibility mapping from legacy plans

- Legacy flat `plans/*.md` files are frozen historical records
  (grandfathered archive-in-place). They remain valid evidence citations but
  MUST NOT be extended with new active scope; new work uses the §9 taxonomy.
- Legacy `*-program.md` ≈ subsystem roadmap + milestone set. Legacy
  `post-*-requalification-and-closure.md` ≈ closure record. Legacy
  `*-corrective*.md` ≈ corrective pass.
- `plans/ROADMAP.md` (A–Z milestones, N–M, production tracks A–D) is the
  historical construction record. `plans/002-long-term-roadmap.md` is the
  current forward-looking roadmap; the two MUST NOT be merged by editing
  history into the new file.
- Legacy "planning baseline" SHAs stay historical. The live binding is owned
  solely by `plans/httpx-parity-correction-status.md` + `compat/*/profile.toml`.
