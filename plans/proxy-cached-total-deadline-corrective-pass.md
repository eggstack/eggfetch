# Proxy Cached Total-Deadline Corrective Pass

Planning baseline: `8eccbede083f3ea0fb404c7c96e644dd820fb144` (`main`, 2026-09-16; documentation descendant of executable freeze `d87be1b780a41dc8ff5f3ba8a14f8d74de5814d0`)
Parent completed program: `plans/post-audit-maintenance-security-and-proxy-modernization-program.md`
Reference compatibility contracts: `httpx==0.28.1`, `httpx2==2.12.0`, Python 3.10+

## Objective

Correct one post-closure timeout-ownership defect in the new Hyper-managed HTTP forward-proxy and HTTPS CONNECT client caches.

Reusable proxy clients/connectors must contain only connection-scoped policy. A logical request's shrinking `Timeout.total` budget must remain request-scoped and must never be captured by a cached connector and reused by a later request.

This is a narrow correctness corrective. It must not reopen the proxy architecture program, redesign timeout APIs, alter public proxy semantics, add a dependency, broaden Node/HTTP/3 scope, or replace Hyper's pool.

## Defect statement

The completed proxy-modernization work correctly introduced bounded Hyper client caches for ordinary forward-proxy and compatible single-target CONNECT routes. The route keys intentionally describe connection-affecting state and intentionally do **not** include a logical request's total timeout.

However, current `ClientInner::forward_client(...)` and `ClientInner::connect_client(...)` also receive the current request's `remaining_total`. That value is passed into the route connector as `setup_timeout` and stored in the cached connector instance:

- `ForwardProxyConnector::setup_timeout`;
- `ConnectProxyConnector::setup_timeout`.

When Hyper later has to establish a new physical proxy connection/tunnel through an already-cached client, the connector can therefore use the total-budget remainder from the request that originally created the cache entry rather than the total budget of the request that triggered the new connection.

The cache keys include stable connection policy such as proxy identity, origin, TLS/SNI/pinning state, HTTP-version policy, and configured connect/proxy-TLS timeout where applicable, but do not include `remaining_total`. This is correct for pooling, but makes retaining `setup_timeout` in the connector incorrect.

### Concrete failure mode

A representative sequence is:

1. Request A uses a short total timeout and creates a cached CONNECT client.
2. Request A completes successfully and the route/client remains cached.
3. The pooled tunnel becomes unusable or is closed, so a later request must invoke the cached connector again.
4. Request B uses a materially longer total timeout on the same proxy/origin route.
5. The cached connector still carries Request A's short `setup_timeout` and can abort Request B's new connection/tunnel establishment prematurely.

The inverse sequence is also important: if Request A creates the client with a long total timeout and Request B later has a short total timeout, removing connector-captured total state must **not** permit Request B to overrun its own total budget. The per-dispatch outer timeout must remain authoritative.

## Existing ownership that should be preserved

Current code already has the two correct timeout ownership mechanisms needed after this correction:

1. `ConnectTimeout<C>` wraps a connector and bounds the complete connection-establishment future with the configured **connect-phase** timeout. That is connection/connector policy and reports `TimeoutPhase::Connect`.
2. The proxy request path wraps the current logical request future in `send_with_total_timeout(proxy_future, remaining_total)`. That is request policy and reports `TimeoutPhase::Total` for the current request.

The manual/multi-target proxy fallback also receives the current request `deadline` through `ProxyRequestContext`; it is not a reusable cached connector and may continue using request-scoped deadline state.

The correction should therefore remove the logical total timeout from cached connector state rather than inventing another timeout mechanism.

## Required implementation

### 1. Prove the current stale-state path before editing

Trace the exact current call path and record it in the closure notes:

- `pipeline.rs` computes the current hop's `remaining_total`;
- `ClientInner::forward_client(...)` / `connect_client(...)` receive that value;
- the factories construct a cached Hyper client and route connector;
- `ForwardProxyConnector` / `ConnectProxyConnector` retain the value as `setup_timeout`;
- a later connector invocation reconstructs a deadline from that retained duration.

Add at least one regression that fails against the planning baseline for the stale-deadline behavior before completing the implementation. Prefer a deterministic local proxy fixture rather than a timing-only unit assertion.

Acceptance:

- [ ] The closure record identifies one test that is red on the baseline for the intended reason and green after the fix.
- [ ] The reproduced failure requires reuse of the same cached client followed by a fresh physical connection/tunnel attempt; a test that merely creates two independent clients is insufficient.

### 2. Remove request-total state from reusable proxy client factories

The preferred implementation is to stop passing `remaining_total` / request-total setup budget into the reusable client factories.

Expected shape:

- remove the request-total/setup argument from `ClientInner::forward_client(...)`;
- remove it from `ClientInner::connect_client(...)`;
- update their call sites in `pipeline.rs`;
- keep only connection-scoped timeout policy in the cached-client construction path.

Do **not** add total timeout to `ForwardRouteKey` or `ConnectRouteKey`. Logical total budget is not connection identity.

Do **not** create a second client cache keyed by timeout buckets.

Acceptance:

- [ ] No cached proxy-client factory accepts a logical request's `remaining_total`, absolute deadline, or equivalent request-lifetime budget.
- [ ] Different `Timeout.total` values for otherwise compatible requests do not create distinct route-cache entries.
- [ ] Existing connection-scoped `connect` timeout distinctions remain explicit where the connector requires them.

### 3. Remove request-total state from the cached connectors themselves

Remove `setup_timeout` (or any equivalent logical-request total/deadline field) from:

- `ForwardProxyConnector`;
- `ConnectProxyConnector`.

A connector created for a reusable Hyper client must be valid for every later request whose connection-affecting route key is compatible.

For a fresh physical forward-proxy connection, continue to preserve:

- pinned proxy address behavior;
- proxy DNS ownership;
- TCP connect timeout;
- HTTPS-proxy TLS policy and timeout;
- transport metrics;
- lifecycle/physical-admission wrappers.

For a fresh CONNECT tunnel, continue to preserve:

- proxy endpoint routing/pinning;
- proxy TLS versus origin TLS separation;
- proxy auth and proxy-only headers;
- typed CONNECT rejection semantics;
- single-target proxied pinning and logical Host/SNI identity;
- origin TLS/ALPN policy;
- connection-phase timeout classification;
- transport metrics and lifecycle policy.

If implementation discovers that some connection-establishment subphase is currently bounded **only** because `setup_timeout` was copied from the logical total budget, give that phase an explicit connection-scoped owner using existing timeout policy. Do not retain request-total state in the connector as a shortcut.

Acceptance:

- [ ] Neither reusable proxy connector stores a logical request total timeout or derived absolute deadline.
- [ ] Connector establishment remains bounded by existing connect/phase policy where configured.
- [ ] No public timeout type or semantics are broadened to accomplish this correction.

### 4. Keep the current request total deadline authoritative at dispatch

Retain per-request enforcement through the existing outer proxy dispatch boundary:

```text
send_with_total_timeout(proxy_future, remaining_total)
```

A request with a short remaining total budget must still cancel an in-progress connection/tunnel establishment even if the cached connector was originally created by a long-budget request.

Do not move the total timeout outside the logical request/redirect/retry machinery. The existing redirect/retry code intentionally shrinks total budget across attempts/hops.

Acceptance:

- [ ] A short-budget request cannot inherit a longer predecessor's setup allowance.
- [ ] A long-budget request cannot inherit a shorter predecessor's setup allowance.
- [ ] `TimeoutPhase::Total` remains the result when the current logical total budget is what expires.
- [ ] `TimeoutPhase::Connect` remains the result when the configured connection-establishment budget expires first.

### 5. Preserve the legacy multi-target/fallback path

The Hyper CONNECT cache is deliberately used only for compatible single-target routes. Multi-address proxied-target fallback retains handshake-specific behavior because 502/504 candidate fallback and replay rules require request-local state.

Do not remove request `deadline` handling from the non-cached fallback merely for symmetry.

Explicitly confirm that:

- multi-target 502/504 fallback still uses the current request's shrinking deadline;
- a candidate attempt cannot reset `Timeout.total`;
- request bodies retain the current replayability rules;
- pinned routes remain fail closed and never fall back to DNS.

Acceptance:

- [ ] Existing multi-target CONNECT tests remain green.
- [ ] No cached-client optimization is extended to a route whose semantics require per-attempt request state.

## Required regression tests

Add focused deterministic coverage in `crates/eggfetch-core/tests/proxy_tests.rs` or the narrowest existing proxy test owner.

### A. CONNECT short -> long budget with forced reconnect

Use one `Client` and the same proxy/origin route.

1. Request A has a short `total` budget but enough time for an immediate CONNECT and response.
2. Confirm A creates/uses the route and completes.
3. Force the established tunnel/physical connection to become unusable while leaving the cached Hyper client entry alive.
4. Configure the next CONNECT establishment to take longer than A's old total budget but less than:
   - Request B's total budget; and
   - Request B's configured connect budget.
5. Request B uses a longer total budget and must succeed.

The fixture must count physical proxy connections and/or CONNECT handshakes so the test proves the cached client was reused while the connector was invoked again.

This is the primary stale-short-deadline regression and should fail on the planning baseline if practical.

### B. CONNECT long -> short budget with forced reconnect

Repeat the same route in the opposite direction:

1. Request A creates the cached client using a long total budget.
2. Force a fresh tunnel on Request B.
3. Delay establishment beyond B's short total budget while keeping the configured connect timeout longer.
4. Request B must fail with the existing total-timeout classification near B's own budget, not continue under A's historical budget.

This proves that removing connector-captured total state does not weaken current-request deadline enforcement.

### C. Varying total budgets must not fragment CONNECT reuse

With a healthy reusable tunnel:

- send sequential same-route requests with materially different `Timeout.total` values;
- verify one compatible tunnel/physical proxy connection remains sufficient where protocol semantics allow.

This test explicitly rejects the tempting but incorrect implementation of adding `Timeout.total` to `ConnectRouteKey`.

### D. Varying total budgets must not fragment forward-proxy reuse

For ordinary HTTP forwarding:

- send sequential same-proxy/same-origin requests with different total budgets;
- verify the existing Hyper forward client/physical proxy connection remains reusable.

This explicitly rejects adding logical total budget to `ForwardRouteKey`.

### E. Forward connector cannot retain stale logical total state

Because local plaintext TCP establishment may be too fast to expose the old forward-path deadline behavior reliably, use one of these approaches in priority order:

1. a deterministic HTTPS-proxy fixture whose second proxy TLS establishment can be delayed after the cached forward client has already been created;
2. an existing connector test seam that forces a delayed second physical establishment;
3. if neither can be implemented without disproportionate fixture complexity, a source-level/unit invariant that the reusable `ForwardProxyConnector` has no request-total/setup-deadline field, combined with regression D proving differing totals still share the cached route.

Do not add production hooks solely for this test.

## Timeout matrix to recheck

The focused pass must also re-run existing coverage for:

- unreachable proxy with `Timeout.total` shorter than connect timeout;
- connect timeout shorter than total timeout;
- delayed CONNECT response;
- proxy TLS timeout where covered;
- origin TLS timeout through CONNECT where covered;
- read/write timeout after a reused tunnel;
- stale-idle connection recovery;
- redirect/retry total-budget shrinking through a proxy;
- request cancellation/drop behavior on pooled proxy responses.

Any changed timeout classification must be treated as a regression unless source review proves the previous behavior was itself incorrect and the public compatibility contract permits the correction.

## Files expected to change

Likely executable/test scope:

- `crates/eggfetch-core/src/client.rs`;
- `crates/eggfetch-core/src/pipeline.rs`;
- `crates/eggfetch-core/src/transport/proxy.rs`;
- `crates/eggfetch-core/src/transport/connect.rs`;
- `crates/eggfetch-core/tests/proxy_tests.rs`.

Documentation/profile closure may additionally touch:

- this plan;
- `plans/README.md`;
- `plans/httpx-parity-correction-status.md`;
- `compat/httpx/0.28.1/profile.toml`;
- `compat/httpx2/2.12.0/profile.toml`;
- narrowly affected timeout/proxy architecture prose if implementation changes an existing description.

Unexpected manifest, lockfile, public API, Python API, FFI, Node, or HTTP/3 changes are a scope-expansion signal and require explicit justification before continuing.

## Validation and exact-SHA requalification

This correction changes executable core transport behavior and tests. The current Stage C compatibility binding to `d87be1b780a41dc8ff5f3ba8a14f8d74de5814d0` therefore becomes historical once implementation lands.

### During implementation

Run at minimum:

```sh
cargo test -p eggfetch-core --all-features --test proxy_tests -- --test-threads=1
./scripts/check.sh
```

Run relevant focused timeout tests directly while iterating.

### Corrective freeze

After the implementation and regressions are green, freeze one clean executable/test SHA. Then run the repository's canonical qualification applicable to an executable core correction:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Also run:

- the full focused proxy suite;
- native API/type oracles through the existing repository commands;
- the exact HTTPX 0.28.1 and HTTPX2 2.12.0 Stage C process required by the live compatibility policy, including the repository's repeated full-run convention;
- exact Rust 1.89 MSRV checks through extended validation.

A fresh live security preflight is not required solely for this no-dependency timeout correction, but **is required** if the implementation unexpectedly changes manifests, lockfiles, dependency features, release tooling, or security configuration.

Any executable/test/build change after the freeze invalidates it and requires a new freeze before rebinding compatibility profiles.

## Closure record requirements

Before marking this plan complete, append a closure section containing at least:

```text
Executable freeze SHA:
Implementation summary:
Baseline red regression:
CONNECT short -> long forced-reconnect proof:
CONNECT long -> short forced-reconnect proof:
CONNECT differing-total reuse proof:
Forward differing-total reuse proof:
Legacy multi-target fallback proof:
Focused proxy suite:
Tier 1:
Extended:
Package:
HTTPX 0.28.1:
HTTPX2 2.12.0:
API/type oracles:
MSRV:
Known optional skips:
Residual limitations:
Documentation-only descendant SHA:
```

Do not mark an item passed by inference from the previous proxy-modernization freeze.

## Explicit non-goals

Do not expand this corrective into:

- adding `Timeout.total`/`remaining_total` to proxy cache keys;
- changing the public timeout model;
- changing proxy retry policy;
- broad proxy cache redesign or LRU work;
- cross-origin forward-proxy pooling;
- rewriting SOCKS;
- replacing the narrow CONNECT handshake with the upstream generic `Tunnel` helper;
- HTTP/3 proxy support or HTTP/3 graduation;
- Node maturation;
- HTTPX 1.0 work;
- new dependencies;
- performance claims unrelated to preserving existing pooling.

## Final acceptance criteria

This corrective pass is complete only when:

- [x] Reusable forward/CONNECT connectors contain no logical request total/deadline state.
- [x] `ForwardRouteKey` and `ConnectRouteKey` remain independent of logical `Timeout.total`.
- [x] Differing per-request total budgets still reuse an otherwise compatible cached route.
- [x] A forced reconnect cannot inherit a previous request's shorter total budget.
- [x] A forced reconnect cannot escape the current request's shorter total budget because an earlier request had a longer one.
- [x] Current request total expiry remains `TimeoutPhase::Total`; connection-phase expiry remains `TimeoutPhase::Connect` where that phase is authoritative.
- [x] Multi-target CONNECT fallback retains current request-local deadline/replay/fail-closed semantics.
- [x] Proxy auth/TLS/pinning/origin isolation and pooling behavior remain unchanged.
- [x] No dependency or public API change is introduced.
- [x] Focused proxy tests, Tier 1, extended, package, API/type oracles, MSRV, and exact-SHA HTTPX 0.28.1 / HTTPX2 2.12.0 qualification all pass on one final executable freeze.
- [x] Compatibility profiles/ledger are rebound only to that exact corrected freeze.
- [x] Any final plan/index/profile edits after freeze are documentation-only.

## Exit criterion

The defect is closed when cached proxy clients are reusable across compatible requests regardless of each request's `Timeout.total`, while every dispatch remains bounded by its own shrinking total deadline and all prior proxy route, security, pooling, and compatibility contracts remain qualified on the corrected exact SHA.

## Closure record — 2026-09-16

Executable freeze SHA: `de00479ef1161ec24c7f2c34a1cc95c7872e7643`.

Implementation summary: removed `remaining_total`/`setup_timeout` from the
reusable forward and compatible single-target CONNECT client factories and
connectors. The existing outer `send_with_total_timeout(proxy_future,
remaining_total)` remains authoritative; route keys and the legacy
multi-target fallback were unchanged. No dependency, public API, Python,
FFI, Node, or HTTP/3 changes were introduced.

Baseline red regression: `hyper_connect_proxy_does_not_reuse_short_total_on_reconnect`
failed on planning baseline `753d6931` with a stale predecessor-budget
`TimeoutPhase::Read` timeout at approximately 500 ms during the forced second
CONNECT. It passed after the correction with the same cached client and a
fresh physical CONNECT attempt.

CONNECT short -> long forced-reconnect proof: `hyper_connect_proxy_does_not_reuse_short_total_on_reconnect`
passed; Request A total 500 ms, Request B total 2 s, delayed second CONNECT
700 ms, two proxy connections and two CONNECT handshakes.

CONNECT long -> short forced-reconnect proof: `hyper_connect_proxy_reconnect_honors_short_current_total`
passed; Request A total 2 s, Request B total 100 ms, delayed second CONNECT
400 ms, resulting error remained `TimeoutPhase::Total`.

CONNECT differing-total reuse proof: `hyper_connect_proxy_reuses_keep_alive_tunnel`
passed with sequential 500 ms and 5 s totals on one reusable tunnel.

Forward differing-total reuse proof: `hyper_forward_proxy_reuses_keep_alive_connection`
passed with sequential 500 ms and 5 s totals on one reusable proxy connection.

Legacy multi-target fallback proof: the existing multi-target, candidate-reply,
replay, pinning, and SOCKS coverage remained green in the 48-test focused
proxy suite.

Focused proxy suite: `cargo test -p eggfetch-core --all-features --test
proxy_tests -- --test-threads=1` — 48 passed.

Tier 1: `./scripts/check.sh` — passed.

Extended: `./scripts/check.sh extended` — passed; Rust 1.89.0 MSRV,
feature matrix, docs, FFI, lifecycle, resource, soak, and benchmark checks
passed.

Package: `./scripts/check.sh package` — passed on the clean documentation
closure tree, including crate dry-run, wheel build/smoke, package contents,
and installed-wheel typing.

HTTPX 0.28.1: three consecutive exact-SHA full pinned runs passed 1,870
tests (245.12 s, 246.48 s, 242.26 s), with 26 existing non-failing warnings
per run.

HTTPX2 2.12.0: included in the same exact-SHA dual-facade compatibility
suite and repeated three-run qualification process; no unexplained,
stale, or resolved-active differences.

API/type oracles: native API/type gates passed (66 exports, 32 exception
bases, 24 reviewed member contracts). The HTTPX 0.28.1 oracle reported 71
allowed matches and HTTPX2 2.12.0 reported 79; both reported zero unexplained,
stale, or resolved-active differences.

MSRV: Rust 1.89.0 check passed through extended validation.

CI: GitHub `CI` run `35060621229` for pushed head `f1e7ba04` passed; the
single `ci` job passed in 11m23s. The only annotation was the existing
platform warning that actions currently targeting Node.js 20 are forced to
Node.js 24.

Known optional skips: Node JS surface (native artifact absent) and downstream
behavioral fixtures (artifact manifest absent), both existing policy skips.

Residual limitations: HTTP/3 proxy support, Node JS artifact qualification,
and downstream artifact qualification remain outside this corrective's scope.

Documentation-only descendant SHA: `0da37ab` (the documentation-only closure
commit descended from the executable freeze; subsequent profile/ledger edits
are also documentation-only).
