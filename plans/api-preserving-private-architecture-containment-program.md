# API-Preserving Private Architecture Containment Program

Planning baseline: `c53eebc47569279b61c0611d4523c5132bdbcaeb` (`main`, 2026-09-21)
Audit date: 2026-09-21
Normative verification policy: `docs/verification-policy.md`
Repository architecture guide: `AGENTS.md`
Primary compatibility contracts: `eggfetch-core` Rust API and supported feature profiles, native Python `eggfetch.__all__`/PEP 561 surface, HTTPX 0.28.1, HTTPX2 2.12.0, C ABI, CLI behavior

## Objective

Perform a second narrow architecture-maintenance campaign after the completed
API-preserving maintenance/interop hardening work. Reduce remaining private
change concentration and prevent accidental growth of historical public
implementation surfaces without changing any existing API surface,
capability, accepted/rejected input, default, protocol behavior, adapter
maturity level, release promise, or compatibility claim.

The current repository is not missing a broad transport/client architecture.
The HTTP engine is correctly centralized in `eggfetch-core`; Python, CLI,
FFI, and Node remain adapters; request normalization is shared across Python
sync/async paths; Hyper client construction is centralized; CONNECT wire
ownership is bounded; Rust and Python surface oracles now exist.

The remaining work is therefore containment rather than redesign:

1. finish decomposing private responsibilities still concentrated in
   `client.rs` and `proxy.rs`;
2. reduce private implementation concentration in Python streaming and the CLI
   while preserving literal public signatures and CLI behavior;
3. audit already-public low-level Rust surfaces so internal refactors do not
   accidentally expand them, and reconcile experimental adapter metadata
   without maturing those adapters;
4. requalify one exact executable SHA across existing Rust/Python/C/CLI and
   compatibility gates.

## Confirmed audit findings

### 1. Core ownership is sound

`eggfetch-core` remains the single HTTP/network behavior owner.
`eggfetch-python`, `eggfetch-cli`, `eggfetch-ffi`, and `eggfetch-node`
delegate to it. `eggfetch-http-connect` remains an intentionally narrow
wire-mechanics crate for caller-owned HTTP/1 CONNECT streams.

Do not merge crates or introduce another request engine.

### 2. `client.rs` still mixes broad public API declarations with substantial private machinery

At the planning baseline, `crates/eggfetch-core/src/client.rs` is roughly
2.6k lines / 100 KiB. The prior decomposition extracted private
`client/config.rs`, but the parent still owns:

- public `Client` and `ClientBuilder` declarations/methods;
- private `ClientInner`;
- resolved-route keying and bounded client cache access;
- SNI/resolved/SOCKS/forward/CONNECT client acquisition;
- standard/custom connector preparation;
- URL parsing and a large private unit-test block.

The public declarations should remain where they are. The remaining private
route/cache/connector mechanics can be split behind private modules.

### 3. `proxy.rs` has the same concentration pattern

`crates/eggfetch-core/src/proxy.rs` remains roughly 2.5k lines / 90 KiB.
Small `proxy/no_proxy.rs` and `proxy/environment.rs` modules now exist, but
the parent still owns most native/HTTPX NO_PROXY parsing/matching,
`ProxyAuth`, connection identity, URL normalization, environment resolution,
the hidden parser used by fuzz/testing, and a very large unit-test block.

The public proxy types and canonical paths must remain unchanged. Only private
implementation should move.

### 4. Python sync/async semantic ownership is already centralized

`request_preparation.rs` is the shared normalization and dispatch-mapping
boundary. The visible duplication in `Client`/`AsyncClient` and top-level
helpers is primarily public signature/API glue and should not be hidden behind
macro generation merely to reduce repetition.

The remaining Python maintenance concentration is
`crates/eggfetch-python/src/streaming.rs`, which owns streaming-response
state, sync bridge/backpressure, decoding/line splitting, and all sync/async
iterator classes in one large module.

### 5. CLI behavior is concentrated but not duplicated HTTP policy

`crates/eggfetch-cli/src/main.rs` combines CLI schema, parsing, request
assembly, output rendering, file handling, exit-code mapping, streaming
output, and tests. This is adapter logic, not a second HTTP implementation.
It can be decomposed privately while retaining clap names/defaults, stdout/
stderr behavior, exit codes, redaction, and request semantics.

### 6. Some low-level Rust implementation surfaces are already public

Examples include public `transport::alt_svc` types/constants and mutable
counter fields/methods on transport/pool metrics. These may be historical
overexposure, but they are now part of the mechanically captured Rust public
surface.

This campaign must not remove, hide, rename, move, deprecate, or broaden them.
Treat them as frozen compatibility debt and ensure new internal work does not
add adjacent public implementation APIs.

### 7. HTTP/3 and Node remain intentionally experimental

HTTP/3 still has explicit production-graduation blockers, including external
h3 lifecycle/corruption risk and incomplete independent interoperability/
impairment evidence. Node is deliberately a narrow prototype using the
blocking C ABI inside `spawn_blocking`.

Neither area should be matured here. Only metadata/support-status consistency
and API-containment checks belong in this program.

## Ordered implementation plans

Execute in this order:

1. `core-client-proxy-private-decomposition-second-pass.md`
   - move additional private client route/cache/connector helpers behind
     private child modules;
   - move additional private proxy parsing/environment/identity/test-only
     mechanics behind private child modules;
   - keep all existing public declarations and canonical import paths fixed.

2. `python-streaming-and-cli-private-decomposition.md`
   - split Python streaming internals by responsibility without changing any
     Python class/method/signature/return/lifecycle semantics;
   - split CLI parsing/request/output/error internals without changing command
     syntax, defaults, output contract, exit codes, redaction, or HTTP behavior.

3. `rust-surface-containment-and-experimental-adapter-hygiene.md`
   - inventory/freeze historical low-level Rust public exposure;
   - add containment checks where current exact-surface snapshots are too
     coarse to prevent accidental adjacent `pub` growth;
   - reconcile Node package metadata/support-status truth without adding Node
     capability or publication support;
   - confirm HTTP/3 remains experimental and no maintenance refactor silently
     broadens its public promise.

4. `post-private-architecture-api-requalification-and-closure.md`
   - freeze one executable SHA after Plans 1-3;
   - run existing Tier 1/extended/package/security/MSRV/API/compatibility gates;
   - renew exact-SHA compatibility records only if required by normative
     policy;
   - reconcile plan/index documentation truthfully.

## Cross-program invariants

- No existing Rust public item may be removed, renamed, moved to a different
  canonical path, have its signature/bounds/fields/variants changed, or have
  its feature/default exposure changed.
- Do not add new public Rust helpers to make private decomposition easier.
  Prefer private or `pub(crate)` modules/functions/types.
- Do not regenerate the Rust public API snapshots merely to accept drift.
- No Python root export, PyO3 signature, PEP 561 declaration, class/property/
  method shape, exception hierarchy, iterator protocol, sync/async return
  shape, or accepted/rejected argument behavior may change.
- Do not replace explicit Python public signatures with opaque macro-generated
  signatures.
- Python sync and async runtime ownership stays distinct while shared
  normalization remains in `request_preparation.rs`.
- No CLI option/alias/default/env behavior, exit code, stdout/stderr routing,
  redaction rule, machine-output schema, filename safety rule, or streaming
  behavior may change.
- C ABI symbols, ownership rules, sentinels, error handling, and feature
  exposure remain unchanged.
- CONNECT wire behavior remains owned by `eggfetch-http-connect`; retained
  duplicate test/fuzz logic should be conformance-tested rather than forced
  into a single stricter/looser parser.
- Hyper remains the H1/H2 engine and connection pool.
- HTTP/3 remains experimental. No 0-RTT, datagram, WebTransport, MASQUE,
  migration, graduation, or new public H3 control surface belongs here.
- Node remains an experimental prototype. No async-core rewrite, streaming,
  cancellation, structured-error redesign, generated declarations, or npm
  publication pipeline belongs here.
- No new language binding, transport trait, global runtime, custom pool, or
  public internal-helper crate may be introduced.
- No compatibility waiver may be added to permit a refactor.
- Existing verification tiers remain authoritative; do not create a parallel
  evidence framework.

## Program completion criteria

This program is complete only when:

- public `Client`/`ClientBuilder` declarations remain at their existing
  canonical paths while substantial private route/cache/connector mechanics
  are no longer concentrated in the same parent module;
- public proxy declarations remain at their existing canonical paths while
  private parsing/environment/identity/test-only mechanics have clearer
  private ownership;
- Python streaming public classes and iterator behavior are unchanged but
  private state/bridge/decoding/iterator responsibilities are split into
  auditable modules;
- CLI behavior is unchanged but private parsing/output/file/error/request
  responsibilities are separated enough that ordinary maintenance need not
  edit one monolithic source file;
- no new public Rust item appears except where independently required by an
  already-existing public contract (the default expectation is zero);
- historical low-level public Rust surfaces are explicitly inventoried and
  guarded against accidental adjacent exposure;
- Node metadata is internally consistent with its explicit experimental
  support status without creating a supported Node API;
- HTTP/3 remains truthfully experimental with unchanged capability/status;
- native Python manifest/typing checks and Rust exact-surface checks report
  zero unexplained drift;
- C ABI and CLI compatibility tests remain green;
- Tier 1, extended, applicable package, security, exact Rust 1.89.0 MSRV,
  docs/doctests, API-oracle, lifecycle/resource and compatibility gates pass
  on the final executable freeze;
- final plan/index/status documents distinguish the executable freeze from
  later documentation-only descendants.

## Non-goals

Do not use this program to add HTTPX APIs, Python trailers, Trio/AnyIO,
coroutine trace callbacks, new auth schemes, H3 features, Node capabilities,
new FFI functions, new CLI features, new package targets, performance
optimizations unrelated to decomposition, or release/publication work for
issue #24/Python 3.15.

Issue #24 publication and Python 3.15 wheel rehearsal remain independent
maintainer/release work.

## Stop conditions

Stop and split a separate corrective if a proposed decomposition:

- requires a public API or feature-graph change;
- changes accepted/rejected input or error classification;
- changes protocol selection, connection identity, cache key semantics,
  timeout/redirect/retry/proxy/TLS behavior, streaming backpressure, or
  lifecycle ownership;
- weakens redaction or fail-closed behavior;
- requires an API/compatibility waiver;
- needs executable changes after the final qualification freeze.

The correct implementation may retain a locally duplicated helper when
deduplication would cross a public/semantic boundary.
