# Post-Audit Maintenance, Security, and Proxy Modernization Program

Planning baseline: `025b1a5a6a94b017b3f3b183c3d562e7ee9bcca7` (`main`, 2026-09-15; eggfetch 0.1.4)
Audit date: 2026-09-15
Reference compatibility contracts: `httpx==0.28.1`, `httpx2==2.12.0`, Python 3.10+
Normative verification policy: `docs/verification-policy.md`

## Objective

Close the remaining maintenance, dependency-boundary, security-process, and proxy-transport issues found by the post-maturation source audit without reopening already-completed architecture programs or adding speculative surface area.

The current engine is feature-complete enough that this program is primarily about making ownership truthful and cheaper to maintain:

1. make adapter Cargo features actually control the core features they claim to expose;
2. remove dependency residue that no longer owns behavior;
3. eliminate the remaining duplicated Python request-to-core dispatch policy;
4. reconcile security/release claims with what the repository really enforces and make publication fail closed;
5. move ordinary HTTP proxy traffic back under Hyper framing/pooling where this can be done without regressing eggfetch-specific proxy semantics;
6. freeze and requalify one executable SHA after the line of work is complete.

This is deliberately not another broad parity, HTTP/3, Node, or framework-integration program.

## Why this program exists

The September 8 architecture/surface maturation program already consolidated core request/transport state, hardened HTTP/3, made the Node prototype disposition explicit, and cleaned up protocol observability. Those plans are historical evidence and must not be duplicated.

The current audit found a narrower set of post-maturation defects and opportunities:

### Adapter feature ownership is not truthful

`eggfetch-ffi` declares features that appear to mirror `eggfetch-core`, but its dependency on `eggfetch-core` does not set `default-features = false`. Consequently, `cargo build -p eggfetch-ffi --no-default-features` still activates the core default H1 + Rustls + native-root profile. The Node crate depends on `eggfetch-ffi` with `default-features = false`, so it currently benefits from the same accidental leakage.

Correcting this requires preserving the *effective* existing default behavior rather than merely toggling one Cargo flag. In particular, the FFI surface needs an explicit native-root feature relationship, and the Node prototype must request the narrow transport/TLS features it actually requires.

### Python still duplicates runtime-independent dispatch policy

`crates/eggfetch-python/src/request_preparation.rs` now centralizes sync/async argument normalization, but the sync client, async client, and top-level request helper still independently apply much of the prepared state to `eggfetch_core::RequestBuilder`: headers, body, timeout, decompression, auth, proxy extras, redirect overrides, retries, and transport hints.

That is a semantic-drift seam. Runtime/GIL/coroutine mechanics should remain separate, but constructing the equivalent core request should have one authoritative implementation.

### Security/release truth has drifted

`SECURITY.md` says `cargo-deny` and `cargo-audit` run on every push, while the normative dependency and verification documents explicitly describe them as manual security-review tools and the current single automatic workflow does not execute them. It also says unsafe code is forbidden workspace-wide even though the deliberately bounded FFI and Node crates allow unsafe at their ABI boundaries.

`deny.toml` still carries PyO3 RustSec ignores whose stated upgrade targets have already been exceeded by the current lockfile. Those exceptions are stale and can hide a future accidental downgrade.

The manually dispatched PyPI workflow uses OIDC and SHA-pinned Actions, but its repository-side validation does not currently enforce the documented tag-only publication contract: `validate_release_versions.py` has strict `--tag ... --publish` handling, yet the workflow calls the non-publishing mode. Build-only dispatch from a branch should remain possible; publication should fail closed unless the selected ref is an exact version tag matching package metadata.

### HTTP forward/CONNECT proxy routes still bypass Hyper pooling

SOCKS already uses persistent Hyper clients. HTTP forwarding and HTTPS CONNECT, however, establish fresh proxy sockets and manually serialize/parse HTTP/1 responses. This costs connection reuse and leaves eggfetch maintaining response framing/body streaming code that Hyper already owns on ordinary routes.

Current `hyper-util` releases provide several relevant primitives: connector-level HTTP CONNECT tunneling, SOCKS connectors, `Connected::proxy(true)` for absolute-form HTTP proxy requests, and composable pool utilities. These are not automatically drop-in replacements for eggfetch because eggfetch also owns distinct proxy TLS policy, route pinning, phase-aware timeouts, proxy-only headers/auth, failure taxonomy, metrics, and fail-closed DNS semantics. The correct target is therefore selective upstream reuse, not wholesale replacement.

## Architectural invariants

The implementation must preserve all of the following:

- `eggfetch-core` remains the sole owner of HTTP/network behavior.
- Rust remains async-first; Python adapters remain bindings over the same engine.
- Python sync and async APIs continue to expose their existing runtime semantics and HTTPX compatibility contracts.
- FFI and Node remain adapters; they do not grow independent HTTP policy.
- Node remains an explicitly experimental prototype unless a separate future program reopens its support decision.
- HTTP/3 remains experimental; this program does not attempt another graduation pass.
- Secure TLS verification and native/WebPKI trust behavior remain unchanged for ordinary users unless a change is explicitly justified and separately tested.
- Proxy auth, origin auth, proxy TLS, origin TLS, logical Host/SNI identity, and physical route selection stay distinct.
- Request-scoped pinned routes continue to fail closed rather than falling back to DNS.
- Existing `Error`/Python exception contracts and timeout classifications do not change merely to simplify implementation.
- The single automatic CI workflow and complexity budget in `docs/verification-policy.md` remain authoritative.
- Publication may become stricter, but routine push/PR CI must not be expanded into another workflow matrix.
- No compatibility or release claim is renewed by inference from historical evidence.

## Ordered implementation plans

Execute the following in order unless a child plan explicitly permits overlap.

### 1. `plans/adapter-feature-and-dependency-boundary-correction.md`

Correct FFI/core feature propagation, make Node's required transport/TLS features explicit, audit and remove redundant Python direct TLS dependencies where proven safe, delete the orphaned core config placeholder, and add bounded dependency/feature assertions.

This plan must preserve effective default FFI/Node behavior while making no-default/minimal feature builds truthful.

### 2. `plans/python-request-dispatch-consolidation.md`

Create one runtime-neutral prepared-request-to-core-builder path shared by sync, async, and top-level Python dispatch. Keep runtime/GIL/async bridging, trace callback handoff, and response adaptation outside that helper.

This is a structural refactor. Any behavior difference discovered while consolidating must be called out as a separate correctness defect and covered by a regression test rather than silently normalized.

### 3. `plans/security-policy-release-and-supply-chain-hardening.md`

Remove stale advisory ignores, create a fail-closed explicit security preflight that fits the local-first verification model, make security documentation truthful, enforce tag/version identity for actual PyPI publication, and reduce release-tooling drift without adding another automatic workflow.

### 4. `plans/proxy-hyper-pooling-and-upstream-reuse.md`

Move HTTP forward-proxy and HTTPS CONNECT origin HTTP framing into Hyper so successful proxy routes can reuse connections and shed custom parser/body-stream maintenance. Evaluate current `hyper-util` proxy connector primitives first; use them only where they preserve eggfetch semantics. Retain the minimum custom connector/handshake logic required for richer route pinning, proxy TLS, timeout, and error contracts.

SOCKS is already pooled and is not rewritten merely for symmetry.

### 5. `plans/post-maintenance-security-proxy-requalification-and-closure.md`

Freeze one executable/test/build SHA after Plans 1–4, run the repository's complete existing qualification process plus the new focused feature/security/proxy evidence, renew HTTPX/HTTPX2 exact-SHA records only if all gates pass, and then perform documentation/index closure as a docs-only descendant.

## Sequencing and freeze rules

Plans 1, 2, and 4 change executable/build behavior and invalidate prior exact-SHA compatibility evidence. Plan 3 primarily changes security/release policy and tooling but may touch lock/configuration files; treat it as pre-freeze work as well.

Do not run a full three-pass exact-SHA requalification after every child plan. Each child plan must keep Tier 1 green and run its focused regressions. Plan 5 owns the single final compatibility freeze.

After Plan 5 freezes the executable candidate, only documentation, compatibility-ledger binding, and plan-index changes may land without reopening the freeze. Any product, test, dependency, build, packaging, or workflow change that can alter produced artifacts or validation semantics requires a new freeze.

## Cross-program acceptance criteria

The program is complete only when all of the following are true:

- [ ] `eggfetch-ffi --no-default-features` no longer inherits core default H1/TLS/native-root features accidentally.
- [ ] The FFI default profile explicitly preserves its intended current transport/TLS trust behavior.
- [ ] The Node prototype explicitly requests the transport/TLS features it depends on rather than relying on transitive default leakage.
- [ ] Direct Python TLS-related dependencies have each been proven necessary or removed; no dependency is removed solely because source grep is empty.
- [ ] `crates/eggfetch-core/src/config.rs` is either given a real owner/use or deleted; stale milestone placeholder code does not remain.
- [ ] Sync, async, and top-level Python dispatch share one runtime-independent request-builder application path for semantically common state.
- [ ] Full native Python API/typing and HTTPX/HTTPX2 behavior remains unchanged unless an explicitly documented correctness fix is required.
- [ ] `SECURITY.md`, dependency policy, verification policy, and actual workflows no longer contradict one another.
- [ ] Stale RustSec exceptions are removed and any remaining advisory ignore has a current, source-backed justification and review trigger.
- [ ] A release security preflight checks the live dependency graph without adding a second automatic push/PR workflow.
- [ ] PyPI build-only workflow dispatch remains possible from non-tag refs, while publication is impossible unless the selected ref is an exact matching `v<SEMVER>` tag.
- [ ] Release tooling that can influence produced artifacts is version-controlled/pinned sufficiently to prevent unreviewed tool drift.
- [ ] HTTP forward proxy requests can reuse eligible proxy connections through Hyper-managed framing/pooling.
- [ ] HTTPS CONNECT requests can reuse eligible established tunnels for the same compatible route/origin through a Hyper-managed client.
- [ ] Proxy pooling never crosses incompatible proxy identity/auth/TLS/pinning/origin boundaries.
- [ ] Successful HTTP responses on modernized proxy routes no longer require eggfetch's independent full HTTP/1 response parser/body-stream implementation where Hyper can own it.
- [ ] Proxy TLS, CONNECT rejection behavior, timeout phases, route pinning, DNS fail-closed rules, streaming/cancellation, and HTTPX compatibility remain covered by focused regressions.
- [ ] Tier 1, extended, package validation, API oracles, and exact-SHA HTTPX/HTTPX2 qualification pass on the final frozen executable candidate.
- [ ] Current `main` CI is green on the closing documentation descendant.

## Explicit non-goals

Do not expand this program into:

- another HTTP/3 production-graduation attempt;
- Node supported-binding maturation;
- a new language binding;
- HTTPX 1.0 implementation before the repository's existing RC/stable trigger;
- OAuth/OIDC or browser-grade cookie behavior;
- a new general proxy framework or Eggress-specific adapter;
- cross-origin HTTP forward-proxy pooling if per-origin reuse delivers the maintenance/performance goal with materially simpler correctness reasoning;
- a custom connection pool if Hyper's existing pool can own connection lifecycle;
- a second automatic security workflow;
- SBOM/provenance/evidence infrastructure revival from the pre-simplification CI system;
- dependency churn unrelated to a concrete ownership, security, pooling, or maintenance requirement found here;
- binary-size claims without measurement.

## Validation policy

Use the existing repository tiers rather than inventing a new framework:

- every executable child-plan commit: `./scripts/check.sh` plus focused tests;
- major child-plan closure: `./scripts/check.sh extended` where prerequisites are available;
- packaging-affecting work: `./scripts/check.sh package`;
- final program closure: all repository-required qualification plus exact-SHA HTTPX/HTTPX2 evidence and the explicit security preflight.

Security database currency is inherently time-dependent. Do not mislabel a live RustSec scan as deterministic routine CI. Record the advisory database/check date at release/final qualification, keep the repository policy truthful about that distinction, and fail closed when the explicit security preflight is invoked.

## Handoff outcome

This program should leave eggfetch with fewer accidental dependencies and duplicated adapter decisions, a security/release story that matches actual enforcement, and proxy routes that reuse the existing Hyper transport machinery instead of maintaining a parallel HTTP/1 client. The measure of success is reduced long-term ownership burden with preserved semantics—not maximum code deletion at any cost.