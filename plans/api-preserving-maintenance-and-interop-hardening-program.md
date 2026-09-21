# API-Preserving Maintenance and Interop Hardening Program

Planning baseline: 03ecba973010e2858bf16a2b5f84d51ce70adae4 (main, 2026-09-21)
Audit date: 2026-09-21
Normative verification policy: docs/verification-policy.md
Repository architecture guide: AGENTS.md
Primary compatibility contracts: eggfetch-core Rust API and feature profiles, native Python eggfetch.__all__/PEP 561 surface, HTTPX 0.28.1, HTTPX2 2.12.0, C ABI, CLI behavior

## Objective

Perform a narrow maintenance campaign that reduces internal drift risk and clarifies ownership without changing any existing public API surface, supported capability, default, protocol behavior, compatibility claim, or language-binding maturity level.

The repository is no longer in a feature-acquisition phase for this line of work. The audit found a sound core/adapters architecture and no broad missing implementation requiring redesign. The remaining high-value work is preventive: make Rust API stability mechanically enforceable, make the Python native surface relationally self-consistent, formalize the private Python-to-Rust SSLContext translation contract, decompose oversized private core implementation regions without moving public items, and keep the remaining CONNECT-wire overlap conformant without inventing new public helpers.

This program must not become a vehicle for API cleanup by removal, signature redesign, behavior broadening, or capability expansion. Internal refactoring is acceptable only when public and compatibility oracles prove zero drift.

## Closure status — local qualification complete (2026-09-21)

The ordered child plans are implemented and locally qualified on executable
freeze `df2549f7c64ebfccde61ed36fef785d39e83b38d`. No public Rust/Python/C/CLI
surface, feature/default exposure, supported behavior, or maturity label
changed. The post-maintenance closure plan owns the final evidence; remote CI
run `35606617959` passed on the documented head. Issue #24 publication and the separate
Python 3.15 wheel rehearsal remain outside this program's closure.

## Confirmed findings at the planning baseline

### 1. Core ownership is healthy

eggfetch-core remains the owner of HTTP behavior. eggfetch-python, eggfetch-cli, eggfetch-ffi, and eggfetch-node are adapters. eggfetch-http-connect is the intentionally narrow exception for generic HTTP/1 CONNECT wire mechanics.

Do not merge these crates or introduce a second transport implementation.

### 2. Rust API breadth now exceeds its mechanical regression protection

The repository documents patch-level compatibility and runs feature checks, rustdoc, doctests, downstream tests, and Python API oracles. It does not currently have an exact Rust public-surface oracle comparable to the native Python manifest.

The Rust surface now includes Client/ClientBuilder, RequestBuilder/Response, native http_body execution, NativeHttpService, transport hints, caller-owned Dialer, resolved destinations, socket controls, TLS/proxy/retry policy, metrics, and compatibility re-export paths. Accidental re-export, feature, method, or type-shape drift therefore has increasing downstream cost.

### 3. Python runtime semantics are centralized, but declarations remain duplicated

prepare_client_config/apply_client_config and prepare_request/prepare_core_dispatch already centralize the dangerous semantic work. The remaining duplication is largely declarative across sync Client, AsyncClient, top-level helpers, PyO3 registration, native_api_manifest.json, __init__.py, __init__.pyi, and _native.pyi.

The next step is stronger relational validation, not a second Python request engine and not a macro-heavy rewrite that hides public signatures.

### 4. SSLContext translation is one logical private protocol split across Python and Rust

python/eggfetch/_ssl_context.py owns snapshotting, conservative representability classification, helper provenance, and mutation fingerprints. src/tls.rs independently imports several private Python objects/attributes and maps them into TlsConfigBuilder.

The fail-closed behavior is correct and must remain unchanged, but the cross-language contract should be explicit, versioned, bounded, and tested as one private protocol.

### 5. client.rs and proxy.rs still carry too many private responsibilities

client.rs contains public Client/ClientBuilder plus private config, route-cache identities, client construction/cache machinery, connector construction, and tests. proxy.rs contains public proxy models plus NO_PROXY parsing/matching, environment normalization, auth encoding, connection identity, URL normalization, and a hidden testing/fuzz parser.

The public types and paths are stable. Private helpers can be separated behind pub(crate)/private modules without changing canonical public paths.

### 6. CONNECT production ownership is already mostly correct, but test/fuzz/auth overlap can drift

Production CONNECT request serialization and bounded response-head parsing live in eggfetch-http-connect and are used by eggfetch-core transport/connect.rs. eggfetch-core still has a hidden byte parser used for testing/fuzzing, and ProxyAuth has validation/encoding logic overlapping conceptually with eggfetch-http-connect::basic_auth_value.

The input domains are not automatically identical. Do not unify them by making validation stricter or looser. Prefer conformance tests and only remove duplication when exact behavior is proven equivalent.

### 7. Repository state has minor stale bookkeeping

Recent CI is green on the planning baseline. Some plans/README.md headings still say Active while their own Status says complete. Issue #24 is recorded in plans as implemented and qualified for 0.1.9 while the GitHub issue remains open; publication state must determine whether it is closed or explicitly retained as fixed-but-unpublished. Python 3.15 wheel rehearsal remains a separate pending qualification item and must not be silently closed by this program.

## Ordered child plans

Execute in this order unless a child plan explicitly allows overlap:

1. rust-public-api-regression-oracle.md
   - establish exact public-surface baselines for supported Rust profiles;
   - add a fast compile-contract layer for critical paths;
   - add an extended exact-surface/semver check without changing CI topology or public features.

2. python-native-surface-relational-guardrails.md
   - extend existing native manifest/typing checks with sync/async/top-level mirror invariants;
   - preserve concrete readable PyO3 functions;
   - avoid introducing a second manifest unless the existing reviewed manifest cannot represent the invariant.

3. python-ssl-context-private-contract-hardening.md
   - replace scattered private attribute imports with one versioned internal export contract;
   - retain conservative fail-closed representability, mutation detection, mTLS provenance, CA bounds, and TLS version semantics.

4. core-private-module-decomposition.md
   - extract private ClientInner/config/cache/connector and proxy parsing/environment helpers into internal modules;
   - keep public Client/ClientBuilder/Proxy/NoProxy/ProxyConfig/etc. at their existing paths and with identical cfg exposure.

5. connect-wire-overlap-conformance.md
   - make production/test/fuzz CONNECT parsers and auth rules demonstrably conformant where their contracts overlap;
   - reuse existing public eggfetch-http-connect primitives only;
   - do not add a new public cross-crate helper merely to remove a few duplicated lines.

6. post-maintenance-api-requalification-and-state-closure.md
   - freeze one executable SHA after Plans 1-5;
   - run native Rust exact-surface proof plus Python/HTTPX/C/CLI/feature/MSRV/security gates;
   - reconcile plan-index headings and issue/status bookkeeping truthfully after qualification.

Plans 2 and 3 may overlap after Plan 1 has frozen the Rust baseline because they primarily touch Python binding/tooling files. Plan 5 should follow Plan 4 so tests target the final private module layout. Plan 6 is the sole final exact-SHA qualification owner.

## Cross-program invariants

- No existing public Rust item may be removed, renamed, moved to a different canonical import path, have its signature/bounds/field/variant shape changed, or become exposed under a different feature/default profile.
- No new public Rust API is required by this campaign. New helpers should be private or pub(crate).
- Historical compatibility re-exports such as crate::request::{ResolvedTarget, TransportHints, NativeRequestOptions} remain valid.
- No Python root export, constructor/method signature, property, exception hierarchy, sync/async return shape, PEP 561 annotation, or accepted/rejected argument semantics may change.
- eggfetch._native and eggfetch._ssl_context remain private implementation modules; no private helper added here becomes a support promise.
- SSLContext translation must remain fail-closed. Any state that cannot be faithfully represented by rustls remains rejected before dispatch.
- No certificate verification, hostname verification, CA replacement/addition, mTLS identity, ALPN, TLS-version, proxy-TLS, or trust_env semantic may weaken.
- eggfetch-http-connect remains wire-only: no socket dialing, TLS, retry, timeout policy, proxy selection, metrics, or client state enters that crate.
- Hyper remains the H1/H2 pool. No custom pool or parallel request engine is introduced.
- HTTP/3 remains experimental and is not graduated.
- Node remains an experimental prototype and is not matured.
- No Python trailers, Trio/AnyIO, coroutine trace callbacks, new auth schemes, or other capability additions belong to this program.
- No new GitHub Actions job or matrix is required. Integrate validation through the existing scripts/check.sh tiers when a permanent gate is justified.
- The existing API/compatibility ledgers may not gain a waiver merely to permit a refactor.

## Program completion criteria

The program is complete only when all of the following are true:

- a reproducible Rust public-surface baseline exists for the supported profiles selected by the Rust oracle plan;
- critical Rust import/method/re-export contracts compile in routine validation and exact surface equality is checked in extended/requalification validation;
- the final Rust surface has zero unexplained difference from the planning baseline;
- native Python sync/async/top-level signatures have explicit relational guardrails in addition to per-symbol snapshots;
- the SSLContext Python-to-Rust bridge has one documented private versioned contract and Rust no longer depends on scattered private implementation details that are outside that contract;
- all existing SSLContext acceptance/rejection and security tests remain green, with new cross-version/schema-failure tests;
- private client/proxy implementation responsibilities are decomposed without moving or re-exporting public types through new canonical paths;
- CONNECT production behavior still has one wire owner and any retained duplicate test/fuzz logic has differential/conformance coverage;
- no public API, compatibility facade, C ABI, CLI contract, Node status, or H3 status changed;
- final Tier 1, extended, package-if-required, security, exact Rust 1.89.0 MSRV, docs/doctests, FFI, lifecycle/resource, compatibility, and applicable downstream checks pass or are truthfully skipped under current policy;
- plans/README.md and related issue/status records describe actual state rather than historical labels.

## Non-goals

Do not redesign Client/Request/Response, remove compatibility aliases, change feature recipes, alter wire semantics, optimize performance without independent evidence, replace rustls/Hyper, add a new transport trait, create a public internal-helper crate, expose private Python snapshots, graduate HTTP/3, mature Node, or expand HTTPX/HTTPX2 parity.

## Stop conditions

Stop and split a separate corrective if any proposed internal change requires a public Rust/Python/C/CLI API change, changes a documented accepted/rejected input, changes transport selection, weakens security/fail-closed behavior, needs a new compatibility waiver, or requires a new feature/default to preserve compilation.

The correct result for a subtask may be "retain the duplication and add conformance coverage" when deduplication would change behavior or public exposure.
