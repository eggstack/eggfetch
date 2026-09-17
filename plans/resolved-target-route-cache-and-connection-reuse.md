# Resolved-Target Route Cache and Connection Reuse

Status: Ready for handoff

Date: 2026-09-17

Planning baseline: `0e798ac4372fd1a6005eeb552e1c5d3395241c2c`

Related completed work:

- `plans/core-hyper-client-construction-and-cache-consolidation.md`
- `plans/core-hyper-client-construction-and-cache-consolidation-closure.md`
- proxy route-cache invariant hardening recorded in `plans/README.md`

## Objective

Retain the security properties of `RequestBuilder::resolved_addresses()` while allowing repeated requests for the same logical origin and the same caller-supplied physical route snapshot to reuse the same Hyper client and therefore Hyper-owned H1 keep-alive / H2 multiplexed connections.

This is an internal core optimization and route-identity hardening pass. It must not add authorization policy, change the public resolved-target API, add a second connection pool, or weaken the existing fail-closed static-routing behavior.

The motivating downstream use case is a security-sensitive client that resolves and authorizes a physical address set itself and then issues many requests through that stable approved route. That use case is requirements evidence only. The implementation must remain generic and must not introduce downstream-specific types, feature flags, callbacks, or policy concepts.

## Confirmed current state

### 1. Resolved targets preserve logical HTTP/TLS identity

`crates/eggfetch-core/src/request.rs` defines `ResolvedTarget` as an ordered, non-empty `Arc<[SocketAddr]>`. `TransportHints::resolved_target` controls only the physical TCP destination. The logical URL remains authoritative for HTTP authority, Host, TLS SNI/certificate verification, cookies/auth origin semantics, redirects, and other request policy.

`RequestBuilder::resolved_addresses()` validates that every supplied socket uses the logical URL's effective HTTP/HTTPS port. Direct static routing remains incompatible with proxy, UDS, and HTTP/3 routes and fails closed for unsupported combinations.

### 2. The direct connector already honors caller-supplied addresses exactly

`DirectConnector::with_resolved_target()` uses the supplied addresses as the physical dial candidates and does not replace them with system DNS. Address ordering is meaningful because it controls attempt order/failover.

The connector can also retain a request-selected SNI override. Consequently, any reusable client identity must distinguish the full physical route snapshot and SNI identity used to build the connector.

### 3. The current resolved-target client is intentionally request-scoped

At the planning baseline, `ClientInner::resolved_client()` constructs a new `DirectConnector`, wraps it through the shared `HyperClientPolicy::cached_route()` / `build_hyper_client()` path, and returns a newly built `TimeoutDirectClient` for each dispatch.

`pipeline/hyper_dispatch.rs::send_direct_route()` stores that result only in a stack local for the current send:

```text
resolved target present
  -> ClientInner::resolved_client(...)
  -> newly built Hyper client
  -> one dispatch
  -> local client dropped
```

Hyper is configured with the normal idle timeout and effective per-host idle cap, but the configured Hyper client that owns that pool does not survive for the next independent resolved-target request. Response-body lifetime remains correct, but a later request builds a different client and therefore cannot check out an idle connection or multiplex onto an H2 connection owned by the previous client.

This isolation originally prevented accidental reuse between ordinary DNS routing or a different address snapshot. That invariant is still required; the missing capability is reuse only when the complete route identity is equal.

### 4. The repository already has the correct cache owner and policy

`transport/hyper_client.rs` already owns:

- `HyperClientPolicy` for persistent/cached H1/H2 client builder policy;
- `build_hyper_client()` for connector -> connect-timeout -> lifecycle -> Hyper construction;
- `BoundedClientCache<K, V>` for bounded configured-client caches;
- explicit route-specific cache capacities;
- the locking rule that cache construction may occur under the async mutex only while construction remains CPU/local configuration with no network I/O.

The same module explicitly identifies resolved-target clients as the sole H1/H2 family that is still request-scoped. Do not create another cache implementation, an LRU dependency, or a parallel physical connection pool. Hyper remains the physical pool owner.

### 5. Redirect behavior already supplies the correct route boundary

`pipeline/redirect.rs` preserves the original `resolved_target` only for same-origin redirect hops. A cross-origin redirect while a resolved/proxied target is present returns `Error::ResolvedTargetRedirect` before second-origin dispatch.

Therefore the route cache does not need to invent redirect semantics. A same-origin redirect can legitimately hit the same resolved-route cache entry; a cross-origin redirect never receives the old snapshot.

## Problem statement

The current implementation makes a safe route artificially one-shot at the configured Hyper-client level. This has two costs:

1. H1 requests to a stable pinned origin cannot reuse an idle keep-alive connection across independent requests.
2. H2 requests to a stable pinned origin cannot share/multiplex through the same retained H2 client/pool across independent requests.

The desired behavior is:

```text
same logical origin
+ same ordered physical address snapshot
+ same SNI override
= same cached configured Hyper client may be reused
```

Any change in those route dimensions must select a different configured client/pool.

False cache hits are security/correctness defects. False cache misses are only performance defects. Key design must therefore prefer conservative separation.

## Security and correctness invariants

Implementation must preserve all of the following:

1. A resolved-target request never falls back to system DNS for the origin.
2. Ordinary DNS/direct clients and resolved-target clients never share a configured Hyper client or physical pool.
3. Different logical origins never share a resolved-route cache entry, even when the supplied physical addresses are identical.
4. The full ordered `SocketAddr` snapshot participates in route identity. Reordering candidates creates a distinct key because attempt order is observable route policy.
5. Any address or port change creates a distinct key.
6. The exact optional SNI override participates in route identity. Do not normalize two caller-supplied overrides into one cache entry merely for hit rate.
7. Path, query, fragment, method, headers, body, cookies, request auth, redirect/retry state, trace observer, failure context, decompression policy, and total/read/write/pool request budgets never participate in the reusable connection key and are never retained by the connector.
8. Client-wide connection policy that is immutable after `ClientBuilder::build()` remains outside the per-route key: TLS config/provider/trust policy, HTTP version/ALPN policy, direct socket/local-address configuration, connect timeout, lifecycle policy, canceled-request retry behavior, and idle-pool policy are already scoped to one immutable `ClientInner`.
9. If any currently client-wide connection-affecting field becomes request-scoped in the future, the resolved-route key must be expanded before that new variability may use the cache.
10. Same-origin redirects may reuse the entry because they retain the same route snapshot. Cross-origin redirects continue to fail closed before dispatch.
11. Proxy resolved routing remains owned by the existing SOCKS/forward/CONNECT route caches. This plan does not merge direct resolved routes with proxy caches.
12. HTTP/3 remains incompatible with `resolved_target`; no QUIC caching or static-route behavior changes.
13. Cache growth is bounded. Eviction of an entry may drop its idle pool, but must not cancel an in-flight request that already holds a cloned Hyper client.
14. A cache miss/build failure is returned and never inserts a partial/invalid entry.
15. Reuse is scoped to one `ClientInner`; no process-global or cross-`Client` cache is introduced.

## Required design

### 1. Add a private `ResolvedRouteKey`

Introduce a crate-private key near the direct/resolved client owner. A representative shape is:

```text
ResolvedRouteKey {
    origin: String,
    addresses: Arc<[SocketAddr]>,
    sni_hostname: Option<String>,
}
```

Exact ownership/types may vary, but the semantics must not.

`origin` should be derived from the normalized logical HTTP origin rather than string slicing the raw URL. Prefer the `url` crate's origin representation / ASCII serialization so scheme, normalized host (including IPv6 formatting), and effective port match existing origin semantics. Do not include path/query/fragment.

The address component must retain the caller's supplied order. Do not sort, deduplicate, convert to an unordered set, or collapse IPv4/IPv6 candidates.

The SNI component should use the exact request override value. A semantically redundant false miss is preferable to normalizing two distinct caller values into a false hit.

Avoid expanding the public `ResolvedTarget` API merely to make HashMap keying convenient. Preferred options are either:

- a crate-private cheap clone of its existing `Arc<[SocketAddr]>`; or
- a private key type with manual `Eq`/`Hash` over `ResolvedTarget::addresses()`.

Adding a public trait/API solely for the cache is unnecessary.

The key should not implement `Display`. `Debug` is not needed for cache operation; if added for tests, it must never grow to include request headers, credentials, or other request state.

### 2. Add one bounded resolved-route client cache

Add to `ClientInner`:

```text
resolved_clients: Mutex<BoundedClientCache<ResolvedRouteKey, TimeoutDirectClient>>
```

under the existing H1/H2 feature gate.

Add an explicit `RESOLVED_CLIENT_CACHE_MAX_ENTRIES` constant in `transport/hyper_client.rs`. Use **64 entries** initially, matching the existing multidimensional SOCKS/forward/CONNECT route caches rather than the 256-entry hostname-only SNI caches. Each resolved-route entry can own its own Hyper idle pool, and physical snapshots may have higher cardinality than a simple hostname override; a conservative bound is preferable until measurements justify a larger value.

Do not expose a public cache-size knob in this pass. If real consumers later demonstrate a need, configuration can be considered separately with measurements.

### 3. Split route lookup from client construction

Refactor current `resolved_client()` into the same shape as the existing SNI/proxy cache helpers:

```text
resolved_client(origin, target, sni)
  -> build ResolvedRouteKey
  -> lock resolved_clients
  -> hit: clone and return stored Hyper client
  -> miss: build configured client locally
  -> insert
  -> clone/return
```

It is acceptable to keep a private synchronous `build_resolved_client(...)` helper for construction clarity.

Construction may remain under the cache mutex because current TLS/config/connector/Hyper client construction is CPU/local configuration only and performs no network I/O, matching the documented route-cache locking contract. Do not await connection establishment while holding the mutex. If implementation discovers an async/network operation in construction, restructure to build outside the lock and use an appropriate double-check/single-flight pattern rather than awaiting under the cache lock.

The lookup method will become async because the cache mutex is async. Update `pipeline/hyper_dispatch.rs::send_direct_route()` to await the lookup and use the returned cloned client exactly as other cached route paths do.

### 4. Keep Hyper as the only physical pool

Do not add socket pooling, connection objects, manual keep-alive state, semaphore-backed per-route connection stores, or another pool abstraction.

The resolved-route cache stores configured Hyper clients/connectors. Hyper continues to own H1 idle connection reuse and H2 connection/multiplexing behavior inside each correctly keyed client.

The existing `HyperClientPolicy::cached_route()` remains authoritative for idle timeout/timer, per-host idle cap, H2-only selection, and canceled-request retry behavior.

## Workstream 0 — Freeze baseline and add failing reuse evidence first

Before changing production code:

1. Record baseline SHA and resolved-target client/cache inventory.
2. Add a deterministic local H1 fixture that keeps one connection alive and counts accepted TCP connections.
3. Send multiple sequential requests through one `Client` with an identical `resolved_addresses()` route.
4. Demonstrate the current baseline establishes more than one configured-route connection / cannot reuse the first request's pool.
5. Add an H2 fixture where practical using the repository's existing local H2/TLS helpers and demonstrate that independent requests currently do not share the same resolved-route client/pool.

The regression should prove physical behavior, not merely count calls to `resolved_client()`.

Do not add a new CI job or external-network benchmark. Local deterministic fixtures belong in existing tests.

## Workstream 1 — Route key and cache plumbing

Implement `ResolvedRouteKey`, `RESOLVED_CLIENT_CACHE_MAX_ENTRIES`, the `ClientInner::resolved_clients` field, builder initialization, and focused key/cache unit tests.

Required key matrix:

| Change | Same cache key? | Reason |
|---|---:|---|
| same origin + same ordered addresses + same SNI | yes | intended reuse |
| path/query/fragment only | yes | request state, not connection identity |
| HTTP vs HTTPS | no | logical/TLS origin differs |
| host differs | no | logical Host/TLS identity differs |
| effective port differs | no | origin/socket identity differs |
| one address differs | no | physical route differs |
| same addresses, different order | no | attempt/failover order differs |
| SNI `None` vs override | no | TLS identity differs |
| SNI override A vs B | no | TLS identity differs |

Add a bound regression proving insertion beyond 64 entries never grows the resolved-route cache beyond its configured maximum.

## Workstream 2 — Cached dispatch integration

Convert resolved-target lookup to the cached async path and update `send_direct_route()`.

Preserve:

- current request timeout wrapping via `send_with_total_timeout`;
- current trace observer and failure context passed at dispatch, never cached;
- existing connector metadata / peer address behavior;
- direct socket/local-address options inherited from immutable `direct_connector_config`;
- current rustls config/provider/trust/ALPN behavior;
- current route error taxonomy.

Do not change standard direct, SNI-only, custom-dialer, UDS, or proxy dispatch except for imports/shared helper plumbing required by the new cache.

## Workstream 3 — Security/correctness regression suite

Add deterministic regressions for at least:

1. **H1 same-key reuse** — repeated sequential requests with one logical origin and identical resolved snapshot reuse a persistent TCP connection when the server keeps it alive.
2. **Different snapshot isolation** — same logical origin but target snapshot A then B reaches the correct distinct listener; an idle A connection must never serve B.
3. **Address-order isolation** — `[A, B]` and `[B, A]` produce distinct route identity; no cache false hit.
4. **Origin isolation** — two logical origins mapped to the same physical address do not share the configured resolved-route client.
5. **SNI isolation** — identical origin/addresses with distinct SNI overrides do not share the client/pool; TLS fixture observes the requested SNI/certificate identity.
6. **No DNS fallback** — cached resolved-target hits still perform no system/origin DNS lookup.
7. **Same-origin redirect** — retained snapshot remains usable and may hit the same route cache.
8. **Cross-origin redirect** — existing `ResolvedTargetRedirect` failure remains before second-origin dispatch.
9. **Eviction safety** — eviction does not make a later different-key request reuse an evicted route's physical connection; in-flight clones remain valid.
10. **Construction failure** — invalid TLS/config construction is not inserted/poisoned and can succeed later after using a valid independently built client configuration.
11. **Cancellation / response drop** — canceling or dropping one request does not corrupt a cached client for subsequent requests, subject to existing Hyper stale-idle retry policy.
12. **H2 reuse/multiplexing** when `http2` is compiled — same-key requests use the retained H2 client and do not create a fresh route client/pool per request.

Where existing transport metrics or connector metadata can prove the physical route, use them instead of adding test-only production instrumentation.

## Workstream 4 — Performance qualification

This work is justified by connection reuse, so record a bounded before/after local measurement. Use deterministic loopback fixtures; do not add an external benchmark dependency or an automatic performance CI gate.

Measure the old isolated resolved-target path versus the cached path at representative concurrency such as 1, 10, 50, and 100 where stable. Record at minimum:

```text
protocol (H1/H2)
request count
concurrency
requests/sec
p50/p95/p99 client-observed latency
accepted TCP connections
```

CPU/memory may be recorded if readily available but are not required acceptance metrics.

For H1, a stable same-key route should reduce accepted connections from approximately request-count behavior toward concurrency/keep-alive behavior. For H2, a stable same-key route should demonstrate retained-client multiplexing/reuse rather than one client/pool per request.

Do not set a universal percentage throughput threshold: local hardware variance makes that brittle. The acceptance proof is materially lower connection churn with no meaningful throughput/latency regression attributable to the cache. Record exact fixture parameters and results in this plan's completion record or a sibling closure record.

If connection churn does not improve, stop and diagnose Hyper route/pool keying rather than layering another pool over Hyper.

## Workstream 5 — Documentation and architecture reconciliation

Update the route/client inventory in `transport/hyper_client.rs` from:

```text
resolved-target client | request scoped / isolated
```

to the new bounded cache ownership and key dimensions.

Document at the `ClientInner::resolved_clients` field why the key contains origin + ordered target + SNI and why immutable client-wide TLS/ALPN/socket/connect policy is intentionally not duplicated into every key.

Update architecture docs only where they currently state that direct resolved clients are necessarily request-scoped. Do not advertise this as a new public capability; the public behavior is still `resolved_addresses()`, with improved connection reuse under identical route identity.

If a release note/changelog entry is appropriate, describe it as resolved-target connection reuse/performance with unchanged routing semantics.

## Files expected to change

Primary:

- `crates/eggfetch-core/src/client.rs`
- `crates/eggfetch-core/src/request.rs` only if a crate-private address-Arc helper is the cleanest key construction
- `crates/eggfetch-core/src/pipeline/hyper_dispatch.rs`
- `crates/eggfetch-core/src/transport/hyper_client.rs`
- resolved-target/direct transport tests or a focused new local fixture test

Possible documentation-only follow-up:

- architecture transport/pooling documentation
- this plan completion record
- `plans/README.md`

No new production dependency should be required.

## Validation

Run focused core tests during implementation, then the repository's canonical gates rather than adding new CI machinery.

At minimum:

```sh
cargo test -p eggfetch-core --no-default-features --features http1
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo test -p eggfetch-core --no-default-features --features http1,http2,tls-rustls
cargo test -p eggfetch-core --all-features
./scripts/check.sh
```

Before final closure of the executable change, also run:

```sh
./scripts/check.sh extended
./scripts/check.sh package
```

Follow the repository's current exact-SHA compatibility/profile renewal rules for an executable core transport change. Do not create a new workflow, matrix, evidence schema, or publication step. Release cadence and publication remain maintainer-controlled.

## Non-goals

- no Eggsec-specific API, authority callback, scope type, or security-policy engine;
- no DNS cache, DNS TTL/revalidation policy, or resolver redesign;
- no global cache shared between independent `Client` instances;
- no cache reuse across distinct logical origins;
- no sorting/canonicalization of caller-supplied address order;
- no new physical connection pool outside Hyper;
- no LRU dependency or cache replacement framework;
- no proxy route-key redesign;
- no HTTP/3 resolved-target support;
- no public resolved-route cache-size/configuration knob without later measurements;
- no change to retry, redirect, auth, cookies, decompression, or body replay semantics;
- no automatic fallback from a resolved route to ordinary DNS.

## Exit criteria

- [ ] The baseline per-request resolved-target client lifetime is captured by a failing physical-reuse regression.
- [ ] `ResolvedRouteKey` distinguishes logical origin, full ordered physical snapshot, and exact optional SNI override.
- [ ] A 64-entry bounded resolved-route client cache exists under `ClientInner` and reuses `BoundedClientCache` / `HyperClientPolicy` rather than adding another cache/pool abstraction.
- [ ] Identical route identity reuses the configured Hyper client across independent requests.
- [ ] Different origin/address/order/SNI identities cannot false-hit the same entry.
- [ ] H1 connection reuse is proven with local accept-count evidence.
- [ ] H2 retained-client reuse/multiplexing is proven under the `http2` profile.
- [ ] Resolved-target requests continue to bypass origin DNS and preserve logical Host/TLS identity.
- [ ] Same-origin redirect retention and cross-origin fail-closed behavior remain green.
- [ ] Current-request timeout/trace/failure/body/auth state is not retained by cached connectors.
- [ ] Cache eviction is bounded and safe for in-flight cloned clients.
- [ ] No production dependency, public API, new CI job, or second physical pool is introduced.
- [ ] Focused tests, Tier 1, extended validation, package validation, and required exact-SHA compatibility qualification are green.
- [ ] Before/after loopback measurements show materially reduced physical connection churn without a meaningful regression.

## Completion record

Implementation owner should append:

- implementation commit SHA / executable freeze;
- exact route-key fields and cache capacity used;
- focused key/isolation/reuse test results;
- H1/H2 before/after connection-count and latency/RPS measurements;
- Tier 1 / extended / package outcomes;
- exact-SHA compatibility/profile renewal outcome;
- any deviation from this plan and rationale.

### Implementation record (2026-09-17)

- Baseline: planning baseline `0e798ac4372fd1a6005eeb552e1c5d3395241c2c`;
  work started at `4212cdb6159ce1df6927cb7914609b6b13d3c1c1` (plan handoff
  commit). Implementation commit: the commit containing this record
  (`feat(core): resolved-target route cache and connection reuse`).
- Route key (`crates/eggfetch-core/src/client.rs::ResolvedRouteKey`,
  crate-private, no `Debug`/`Display`):
  `origin: String` (`url.origin().ascii_serialization()`, no
  path/query/fragment), `addresses: Arc<[SocketAddr]>` (caller's ordered
  snapshot via new `ResolvedTarget::addresses_shared()` helper, never
  sorted/deduped), `sni_hostname: Option<String>` (exact override).
- Cache: `ClientInner::resolved_clients:
  Mutex<BoundedClientCache<ResolvedRouteKey, TimeoutDirectClient>>` under
  `http1`/`http2`, capacity `RESOLVED_CLIENT_CACHE_MAX_ENTRIES = 64` in
  `transport/hyper_client.rs` (conservative, matches SOCKS/forward/CONNECT).
  Lookup is async cached (`resolved_client(origin, target, sni)`); construction
  is a sync `build_resolved_client()` helper (CPU/local only, no I/O under
  lock, failures return before insert). `send_direct_route()` and the native
  `send_native_http_body` direct path await the lookup. Hyper remains the only
  physical pool via `HyperClientPolicy::cached_route()`.
- Focused tests:
  - `client.rs`: `resolved_route_key_matrix` (same-key reuse; path/query/
    fragment + explicit-default-port reuse; HTTP-vs-HTTPS, host, effective
    port, address, order, SNI-None-vs-override, SNI-A-vs-B fragmentation) and
    `resolved_route_cache_is_bounded` (64-entry bound) — 2 passed.
  - `tests/resolved_route_cache_tests.rs` (14 tests, all-features): H1
    same-key reuse (3 reqs, 1 accept), path/query reuse (1 accept),
    snapshot isolation (`[good]` vs `[good, good]`, 2 accepts), order
    isolation (`[good,bad]` vs `[bad,good]`, 2 accepts), origin isolation
    (2 origins, 2 accepts), SNI fragmentation (None vs override, 2 accepts),
    TLS SNI correctness (matching SAN succeeds, mismatch fails closed),
    no-DNS-fallback, same-origin redirect reuse (2 hops, 1 accept),
    cross-origin `ResolvedTargetRedirect`, eviction safety (80-entry pressure,
    both servers still correct), construction-failure non-poisoning (invalid
    TLS version range fails, valid client succeeds), cancellation/drop safety,
    H2 retained-client reuse (3 streams, 1 accept). Feature profiles:
    `http1` 11 passed, `http1,tls-rustls` 13 passed,
    `http1,http2,tls-rustls` 14 passed, `--all-features` 14 passed.
  - Full `cargo test -p eggfetch-core --all-features -- --test-threads=1`:
    1287 passed (31 suites).
- Workstream 0 failing evidence: the new H1 reuse test FAILED on the baseline
  (3 accepts vs 1 expected), PASSED after the cache (1 accept).
- Performance (loopback `TestServer`, keep-alive, ~40ms delayed-ACK floor):
  - Isolated baseline (fresh `Client` per request, approximates old
    per-request Hyper client): 20 reqs sequential, 20 accepts, 24.0 RPS.
  - Cached: 200 reqs conc=1, 1 accept, 24.3 RPS (no regression); conc=10,
    20 accepts; conc=50, 54 accepts; conc=100, 100 accepts — churn reduced
    from request-count to concurrency-level. H2: 3 streams over 1 TCP
    connection (multiplexed, retained client).
- Tier 1: `./scripts/check.sh` green locally (see commit CI); `cargo fmt`,
  `check_lint_suppressions.sh`, `cargo clippy --workspace --all-targets
  --all-features -- -D warnings` clean.
- Extended (`check.sh extended`), package (`check.sh package`), and exact-SHA
  HTTPX 0.28.1 / HTTPX2 2.12.0 compatibility renewal were not run as part of
  this change; they remain maintainer-controlled release gates per
  `docs/verification-policy.md`. No new CI job, matrix, evidence schema, or
  publication step was added.
- Docs: `transport/hyper_client.rs` inventory (resolved row now bounded-64
  with `ResolvedRouteKey`), `ClientInner::resolved_clients` field docs,
  `docs/architecture/core-engine.md` inventory, `core-timeout-pool.md`
  cached-route policy line, `core-tls-proxy-protocols.md` resolved section,
  `docs/rust/guide.md` static-routing paragraph, `.skills/rust-development.md`
  route-cache list, `.skills/security-review.md` pool note, `CHANGELOG.md`
  Unreleased entry. README and AGENTS.md needed no change (no stale lifetime
  claim). No public API, feature flag, dependency, or second pool added.
- Deviation: none material. Origin uses `url.origin().ascii_serialization()`
  per the plan's preference (rather than manual `scheme://host:port`
  formatting) so IPv6/host normalization matches `url` semantics; effective
  ports remain distinct via the origin (default-port omission still denotes
  the same origin, which is correct).
