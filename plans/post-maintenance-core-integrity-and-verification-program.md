# Post-Maintenance Core Integrity and Verification Program

Planning baseline: `42f6de75360763a3301ed3bdf1ca532ce7f5c0ba` (`main`, 2026-09-16; eggfetch 0.1.4)
Audit date: 2026-09-16
Predecessor: completed `plans/post-audit-maintenance-security-and-proxy-modernization-program.md`
Normative verification policy: `docs/verification-policy.md`
Reference compatibility contracts: `httpx==0.28.1`, `httpx2==2.12.0`, Python 3.10+

## Objective

Perform a narrow post-maintenance integrity pass over the current engine after the proxy pooling/security program. The repository no longer needs another broad parity or feature-acquisition phase. This program instead corrects two source-proven policy defects discovered by the follow-up audit, formalizes reusable route-cache identity invariants, reduces repeated Hyper-client construction and request-pipeline orchestration, and tightens dependency/validation reproducibility without changing the repository's deliberately small CI model.

The program is intentionally ordered so correctness and security invariants are established before structural refactoring. Refactors must preserve proven behavior rather than defining it retroactively.

## Research conclusions

### Current architecture is mature enough that consolidation is now the highest-value work

The prior `core-request-and-transport-consolidation.md` work already established typed request reconstruction, a shared redirect first-hop path, `PreparedRequest`, declarative route selection, and common response handling. Do not repeat that work. The residual maintenance burden is now concentrated in two places:

- `client.rs` constructs many related Hyper clients independently: standard, direct, UDS, custom dialer, SNI, custom-SNI, SOCKS, forward proxy, and CONNECT proxy. The route connectors differ legitimately, but HTTP-version, stale-idle retry, idle-pool, lifecycle-wrapper, and bounded-cache policy are repeated.
- `pipeline.rs` still owns retry, redirect, preparation, route compatibility, proxy-context construction, H3 discovery/fallback/suppression, dispatch, and common post-response policy in one very large module. Its internal abstractions are sound; the remaining task is responsibility/module decomposition, not another transport framework.

### Corrective 1: proxy TLS cache identity can alias different security policies

`TlsConfig` is cloneable and contains multiple connection-affecting fields: trust policy, additional/custom roots, crypto provider, client identity, certificate/hostname verification flags, TLS version bounds, and SNI policy. Its current crate-private `connection_identity()` returns only the pointer identity of the shared `root_store` cache.

That identity is insufficient once a clone is modified. For example, `base.clone().danger_accept_invalid_certs(true)` changes certificate/hostname verification while retaining the same `root_store` `Arc`; the original secure clone and the modified insecure clone therefore produce the same current connection identity.

`ProxyConfig::connection_identity()` incorporates that TLS identity, and both `ForwardRouteKey` and `ConnectRouteKey` use the proxy identity to select a cached Hyper client. Because request-level proxy overrides are supported, one `Client` can legitimately alternate proxy configurations. A false-equal TLS identity can therefore allow a later request with stricter proxy-TLS policy to reuse a Hyper client/physical connection established under weaker policy.

False cache misses are acceptable here. False cache hits across distinct TLS security policy are not.

### Corrective 2: Hyper idle-pool policy is not fully enforced

Current `configure_hyper_builder_policy()` calls `pool_idle_timeout()` but never installs `hyper_util::rt::TokioTimer` with `pool_timer()`. In hyper-util 0.1.20, `pool_idle_timeout()` requires a timer and the builder defaults `pool_timer` to `None`. Consequently, configured `Limits::keepalive_expiry` / `PoolConfig::idle_timeout` does not currently drive Hyper idle eviction on these builders.

Policy propagation is also inconsistent among persistent route-specific clients. SNI/forward/CONNECT cached clients receive the idle duration but not the configured idle-per-host cap; SOCKS cached clients currently receive neither. This contradicts the documented contract that idle timeout/caps are physical Hyper-pool policy and makes the route-specific pools behave differently from standard/direct/UDS/custom paths.

### Route cache identity is now a first-class correctness boundary

The immediately preceding proxy modernization already produced one stale-state regression: a logical request total timeout was accidentally captured in a reusable connector and then corrected in `de00479ef1161ec24c7f2c34a1cc95c7872e7643`. The TLS identity issue is the second example in the same architectural boundary.

The durable correction is not a larger universal cache key. It is an explicit invariant: reusable client/cache state may contain only connection-scoped policy, every connection-affecting policy must participate in compatibility identity, and request-scoped policy must never be captured by reusable connectors.

### Upstream Hyper offers new composable pool primitives, but adoption is not assumed

The locked workspace uses `hyper-util 0.1.20`. Its newer `client::pool` module provides composable service-pool/cache primitives. This is worth a bounded source/fixture qualification during the client-construction consolidation pass, but it is not automatically a replacement for eggfetch's outer route-policy caches. The legacy Hyper client already owns physical HTTP connection reuse; eggfetch's maps primarily cache configured clients/connectors keyed by route policy. A migration is justified only if it materially simplifies ownership without losing route-key security, timeout, pinning, lifecycle, or MSRV behavior.

### Do not reopen HTTP/3 or Node maturation in this program

HTTP/3 remains explicitly experimental in the repository after a completed production-qualification attempt. This program may run deterministic H3 regressions after pipeline movement, but it must not claim graduation or add H3 features.

The Node binding is also deliberately documented as an experimental prototype. There is no new product requirement in this audit that justifies reopening its maturation program. Tier 1's existing truthful native-artifact skip remains unchanged.

### Preserve the simplified verification model

`docs/verification-policy.md` intentionally limits routine push/PR CI to one automatic workflow, one Ubuntu job, and no matrices. The completed security program intentionally keeps live RustSec intelligence in `./scripts/check_security.sh` and publication/release paths instead of routine CI. The current audit does not provide sufficient regression history to overturn that policy.

Security follow-up therefore targets the coverage of the existing explicit security command and reproducibility of the existing validation job, not a second workflow, scheduled scanner, or OS/Python matrix.

## Ordered implementation plans

Execute in this order unless a child plan explicitly permits overlap:

1. `proxy-tls-route-cache-identity-corrective-pass.md`
   - eliminate false cache compatibility across distinct proxy TLS policy;
   - add wire-level regressions proving strict policy cannot inherit a weaker pooled proxy connection;
   - establish an opaque complete TLS connection-policy identity with no secret-bearing diagnostics.

2. `hyper-idle-pool-policy-corrective-pass.md`
   - install the required Hyper pool timer when an idle timeout is configured;
   - propagate idle timeout and idle-per-host cap consistently to every persistent Hyper client family;
   - add deterministic connection-count/expiry tests for representative standard and cached routes.

3. `reusable-route-cache-invariants-and-hardening.md`
   - classify connection-scoped versus request-scoped policy for every reusable client cache;
   - add table/property and wire tests for equality/isolation/reuse invariants;
   - preserve the prior total-deadline corrective as a permanent invariant rather than a one-off regression.

4. `core-hyper-client-construction-and-cache-consolidation.md`
   - centralize common Hyper builder/lifecycle/pool policy after plans 1-3 define the required behavior;
   - centralize bounded client-cache mechanics without adding a dependency merely for LRU semantics;
   - qualify hyper-util's composable `client::pool` primitives and adopt them only if they are a clear simplification with no semantic regression.

5. `pipeline-policy-and-transport-dispatch-decomposition.md`
   - retain existing typed request/preparation/route abstractions;
   - move retry, redirect, preparation, proxy dispatch, H3 dispatch/fallback, and common response finalization into narrow internal modules;
   - make the top-level pipeline an orchestration boundary rather than a second transport framework.

6. `dependency-graph-and-validation-reproducibility-hardening.md`
   - make cargo-deny inspect the supported optional-feature dependency graph and release target families, including Windows;
   - pin routine Python validation tooling in repository-controlled requirements rather than unconstrained `pip install`;
   - preserve one automatic workflow/job and the separate live security command.

7. `post-core-integrity-requalification-and-closure.md`
   - freeze one final executable SHA;
   - run focused corrective/invariant checks plus Tier 1, extended, package, and live security gates;
   - renew exact-SHA HTTPX 0.28.1 / HTTPX2 2.12.0 qualification only after all executable work is complete;
   - retain HTTP/3 and Node experimental labels.

Plans 1 and 2 are independent correctness corrections and may be implemented in parallel only if their edits to `client.rs`/tests do not conflict. Plan 3 should land before Plan 4. Plan 5 should follow Plan 4 so it moves already-consolidated client-construction responsibilities rather than duplicating them. Plan 6 may overlap Plans 3-5 after both corrective plans are complete.

## Architectural invariants

The following are non-negotiable throughout this program:

- `eggfetch-core` remains the only networking engine.
- Rust remains async-first; Python/CLI/FFI/Node remain adapters.
- No new public transport framework or universal `Transport` trait is introduced for refactor aesthetics.
- Logical request policy (retry, redirect, auth, cookies, decompression, total/read/write deadlines) must not leak into reusable connection/cache identity unless it truly changes connection establishment.
- Any policy capable of changing the security or wire compatibility of a reused physical connection must be represented in reusable-client compatibility identity.
- A false-negative reuse decision is preferable to a false-positive reuse decision across incompatible security policy.
- Hyper continues to own eligible H1/H2 physical connection pooling; eggfetch must not build a second socket pool.
- Multi-target proxy fallback remains handshake-specific where candidate replay/error semantics require it.
- HTTP/3 remains experimental and does not bypass proxy/UDS/custom-routing precedence.
- The single automatic CI workflow/job and no-matrix complexity budget remain unchanged.
- Live advisory databases remain outside deterministic routine CI.
- No new production dependency is added unless a child plan proves a concrete maintenance/correctness benefit.

## Cross-program acceptance criteria

- [ ] Distinct connection-affecting `TlsConfig` values cannot alias in proxy route caches merely because they were cloned from the same root-store cache.
- [ ] Reusing a weaker proxy-TLS route can never satisfy a later stricter proxy-TLS request without a compatible TLS policy.
- [ ] Configured Hyper idle timeout has an actual timer and is executable behavior, not documentation-only policy.
- [ ] Persistent standard/direct/UDS/custom/SNI/SOCKS/forward/CONNECT Hyper client families receive the intended idle timeout and idle-per-host cap, with intentional exceptions explicitly documented.
- [ ] Reusable client cache keys have direct invariants proving what changes compatibility and what must remain request-scoped.
- [ ] Varying `Timeout.total` cannot fragment compatible route caches or poison later requests.
- [ ] `client.rs` no longer repeats Hyper builder/lifecycle/cache plumbing for each route without a route-specific reason.
- [ ] `pipeline.rs` is decomposed by responsibility while preserving the existing `RequestParts`/`PreparedRequest`/`TransportRoute` behavioral boundaries.
- [ ] H3 fallback/suppression semantics remain directly tested after code movement; no graduation claim is made.
- [ ] cargo-deny's explicit security scan covers optional dependency families and the supported publication platform set rather than only the default graph.
- [ ] Routine Python validation tooling is version-controlled/reproducible at a repository SHA.
- [ ] No additional automatic workflow, routine matrix, or publication authority is introduced.
- [ ] Tier 1, extended, package, explicit security preflight, API oracles, and exact-SHA compatibility qualification pass on the final frozen candidate.

## Non-goals

Do not expand this program into new HTTPX parity work, HTTP/3 promotion/features, Node graduation/npm publication, new proxy protocols, a new async runtime, browser-grade cookie policy, a new public resolver/transport trait, automatic crates.io publication, a scheduled advisory workflow, a routine CI matrix, wholesale unrelated dependency upgrades, or speculative performance rewrites.

## Program closure rule

Plans 1-6 modify executable/test/build/validation behavior and therefore invalidate the current exact-SHA compatibility evidence. Do not repeatedly renew Stage C after each child plan. Keep Tier 1 and focused compatibility tests green while working.

Plan 7 owns the single final executable freeze and compatibility renewal. After that freeze, only documentation/profile/plan-index changes may land without reopening qualification. Any executable, test, build, dependency, or validation-script change after the freeze requires a new frozen candidate and rerun of the affected closure gates.
