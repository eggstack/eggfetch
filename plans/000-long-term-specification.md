# eggfetch Long-Term Architecture and Product Specification

Status: canonical long-term implementation directive

Companion documents:

- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`

This document defines the intended end state for eggfetch. It is deliberately
broader than an implementation plan: it establishes product scope, crate
ownership, protocol expectations, security properties, compatibility
requirements, and acceptance criteria. The roadmap decomposes this
specification into ordered execution phases. The terminology document is
normative whenever older eggfetch plans or documentation use overlapping terms
such as engine, client, profile, route, total timeout, facade, or Stage C.

The keywords MUST, MUST NOT, REQUIRED, SHOULD, SHOULD NOT, and MAY are normative.

Architecture entry point: `docs/architecture/overview.md`. Normative
verification and release policy: `docs/verification-policy.md`.

## 1. Product definition

eggfetch is a Rust-native async HTTP client engine with thin adapters. One
async engine (`eggfetch-core` plus the small `eggfetch-http-connect` CONNECT
wire primitive it owns) serves a Python package (sync + asyncio + two
versioned compatibility facades), a CLI, a C ABI, and an experimental Node.js
prototype.

The same engine MUST support three consumer shapes without creating separate
products:

```text
Native Rust
    application -- await --> eggfetch-core (feature-selected profile)

Python
    sync/async code --> eggfetch (PyO3, blocks on async engine, GIL released)
                     --> eggfetch.compat.httpx  (HTTPX 0.28.1 facade)
                     --> eggfetch.compat.httpx2 (httpx2 2.12.0 facade)

Process boundary
    shell --> eggfetch-cli --> eggfetch-core
    C host --> eggfetch-ffi --> eggfetch-core --> eggfetch-node (prototype)
```

## 2. Primary product goals

eggfetch MUST provide:

1. A single audited async HTTP implementation shared across Rust, Python,
   CLI, and future language bindings.
2. Idiomatic native APIs per surface: the Rust `Client` feels like a Rust HTTP
   client, the Python API mirrors requests/HTTPX ergonomics without shaping
   the Rust API.
3. Feature-gated modularity so embedders pay only for what they use, from the
   lean `standard-http1` profile to the full Python/compat profile.
4. Phase-aware timeout, pooling, retry, redirect, proxy, and TLS semantics
   that are documented, tested, and stable across releases.
5. Versioned, independently qualified HTTPX compatibility facades with
   exact-SHA-bound Stage C evidence and documented residuals.
6. Security by default: memory-safe TLS, secret redaction, fail-closed
   translation, and explicit trust policy.
7. Manual, maintainer-controlled publication to crates.io and PyPI with
   packaging validation that never publishes by itself.

## 3. Non-goals

eggfetch is not initially:

- a second HTTP engine per adapter (no duplicate networking outside core);
- a general distributed workflow engine, proxy product, or observability platform;
- a Trio/AnyIO-native Python client (asyncio only);
- a supported Node.js binding (the N-API surface remains an experimental prototype);
- a production HTTP/3 stack (H3 remains experimental until the graduation
  evidence gate in `docs/architecture/core-tls-proxy-protocols.md` closes);
- an HTTPX 1.0 compatibility target before the upstream RC/stable trigger
  fires (preview reconnaissance only);
- a downstream-specific adapter host (no Gregg, Eggpool-provider, WAF, or
  site-policy types in this repository; downstream migration stays outside).

eggfetch MAY interoperate with those systems where they consume the engine as
ordinary clients. It MUST NOT absorb their product scope.

## 4. Architectural principles

### 4.1 One engine, thin adapters

All HTTP logic lives in `eggfetch-core` plus the `eggfetch-http-connect`
CONNECT wire primitive it owns. CLI, Python, FFI, and Node never touch the
network directly. If HTTP logic appears outside core, it MUST be refactored
inward (CONNECT wire bytes belong in `eggfetch-http-connect`).

### 4.2 Async-first

The Rust engine is async-only (tokio). Synchronous APIs are adapter-layer
concerns that block on the async engine (Python sync releases the GIL; the
Node prototype uses `spawn_blocking` over FFI).

### 4.3 Feature-gated modularity

Capabilities beyond HTTP/1.1 client behavior are feature-gated or isolated in
higher-level crates. Defaults stay minimal (`http1 + tls-rustls +
tls-native-roots`). Optional behavior MUST NOT enter `default` without an
explicit ownership decision.

### 4.4 Security by default

`unsafe_code = "forbid"` workspace-wide (only `eggfetch-ffi` and
`eggfetch-node` override to `"allow"` for their checked FFI/N-API
boundaries). Credentials are redacted at every Debug/Display/`__repr__`/error
boundary. TLS translation is fail-closed. Trust replacement and trust
augmentation are distinct APIs with distinct semantics.

### 4.5 Typed reconstruction, no silent drops

Request rebuilds for retry/redirect go through exhaustive typed helpers. A new
request field MUST fail to compile when omitted, never be silently dropped.

### 4.6 Compatibility claims must match tested behavior

Every compatibility claim is bound to the exact executable SHA that produced
its evidence. Residual differences are documented in
`docs/residual-differences.md`, never papered over.

## 5. Canonical crate ownership

- `eggfetch-http-connect` — generic CONNECT wire bytes only (target
  formatting, serialization, bounded head parsing). No sockets, TLS, retry,
  or policy. Owned by the `proxy` feature.
- `eggfetch-core` — the engine: client, pipeline (`prepare` → `route` →
  `finalize`, entered via `retry`, `redirect`, or `lean`), transport routes,
  timeouts, pool, TLS, proxy/SOCKS/UDS/dialer/SNI/H3 transports, auth,
  cookies, multipart, compression, streaming bodies, metrics, tracing.
- `eggfetch-cli` — argument parsing and output formatting only.
- `eggfetch-python` — PyO3 sync/async adapters, shared request preparation,
  neutral `_ssl_context` interop, streaming bridge, and the two compat
  facades. No raw sockets; SSE is Python framing over streams, WebSocket uses
  wsproto over the core 101 stream.
- `eggfetch-ffi` — opaque-handle C ABI over the engine.
- `eggfetch-node` — experimental N-API prototype over FFI (string-only
  bodies, buffered responses, unstructured errors).
- `eggfetch-bench` — Criterion harnesses and fixtures. Harness code, never
  published, never a proxy-behavior authority.

## 6. Compatibility contract

The two facades (`eggfetch.compat.httpx` for HTTPX 0.28.1,
`eggfetch.compat.httpx2` for httpx2 2.12.0) coexist without cross-mutation.
Each Stage C qualification is bound to one exact executable SHA recorded in
`plans/httpx-parity-correction-status.md` plus `compat/*/profile.toml`. Any
change to executable or qualification inputs invalidates the binding and
REQUIRES fresh exact-SHA requalification from a new freeze. Docs-only changes
do not invalidate it. Live SHAs MUST NOT be hardcoded outside the canonical
qualification records.

## 7. Verification and release contract

`docs/verification-policy.md` is normative. CI is one automatic job repeating
`scripts/check.sh` (Tier 1); extended (Tier 2) and package (Tier 3) validation
are maintainer-run gates, not merge gates. Publication is manual: crates.io
from a trusted local environment in dependency-leaf order, PyPI via the
manually dispatched wheel workflow. No new automatic check, matrix, evidence
schema, or publish automation may be added without explicit maintainer
approval and a concrete regression history.

## 8. System invariants

The implementation MUST preserve the following invariants:

1. Every network byte flows through `eggfetch-core` (CONNECT wire bytes
   excepted via `eggfetch-http-connect`).
2. The Rust engine remains async-only.
3. Python sync blocks on the async engine while releasing the GIL; Python
   async targets asyncio only.
4. `ResponseBody` public variant shapes stay frozen; `BodyTimeoutStream`
   stays the single high-level timeout owner.
5. `Timeout.total` is one absolute dispatch-to-EOF deadline that never resets
   and wins ties; `read` is first-poll/per-chunk inactivity.
6. Total-deadline state is never cached in reusable route connectors; the
   outer dispatch owns it across the `PoolGuard` body lifecycle.
7. Proxy fallback is typed only (CONNECT on 502/504; local SOCKS5 on
   destination-specific 0x03/0x04/0x05); auth/policy/protocol/malformed
   failures stop.
8. `additional_ca_*` augments the base trust store; `ca_certificate_*`
   replaces it. Verification failure MUST never retry with another store.
9. `crypto_provider()` is per-config, never process-global.
10. Only 101 responses own `response.extensions["network_stream"]`; CONNECT
    tunnels stay body-iterator only.
11. Secrets (`authorization`, `proxy-authorization`, `cookie`, `set-cookie`)
    never appear in Debug/Display/`__repr__`/errors.
12. Hyper client construction is centralized; forward proxying stays H1-only.
13. Compatibility claims match tested behavior on the bound SHA; residuals are
    documented, not hidden.
14. Completed plans are historical records, never active gates.

## 9. Completion criteria

This long-term directive is complete when:

- one async engine serves native Rust, Python sync/async, both compat
  facades, CLI, and C ABI with per-surface idiomatic APIs;
- every compatibility claim carries exact-SHA evidence and explicit residuals;
- feature profiles let embedders select lean-to-full capability without
  behavior drift between profiles for the shared subset;
- verification tiers, publication order, and wheel matrix run as documented
  with no automatic publication path;
- experimental surfaces (H3, Node, HTTPX 1.0 preview) remain explicitly
  labeled until their named evidence gates close.
