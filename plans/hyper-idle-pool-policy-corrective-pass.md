# Hyper Idle-Pool Policy Corrective Pass

Planning baseline: `42f6de75360763a3301ed3bdf1ca532ce7f5c0ba` (`main`, 2026-09-16; eggfetch 0.1.4)
Parent program: `plans/post-maintenance-core-integrity-and-verification-program.md`
Reference contracts: native Rust `Limits`/`PoolConfig`, HTTPX 0.28.1, HTTPX2 2.12.0

## Objective

Make the documented Hyper idle-connection policy executable and consistent across every persistent H1/H2 client family.

Two separate defects are in scope:

1. `pool_idle_timeout(...)` is configured without the timer Hyper requires to drive idle eviction.
2. route-specific persistent clients do not all receive the same configured idle timeout and per-host idle cap as the standard/direct/UDS/custom clients.

This is a bounded correctness/resource-lifecycle correction. It must not redefine logical request concurrency, introduce a second connection pool, or broaden timeout semantics.

## Current defect

`configure_hyper_builder_policy(...)` currently applies:

- `retry_canceled_requests(...)`;
- `pool_idle_timeout(...)` when configured;
- `pool_max_idle_per_host(...)` when a cap is supplied.

The builder never installs a Hyper timer. With the locked `hyper-util 0.1.20`, idle timeout requires a timer and the client builder does not provide one by default. Therefore `Limits::keepalive_expiry` / `PoolConfig::idle_timeout` is documented as physical idle-pool policy but is not actually sufficient to evict idle Hyper connections.

Policy propagation is also inconsistent:

- standard client: idle timeout + effective per-host cap;
- direct client: idle timeout + effective per-host cap;
- UDS client: idle timeout + effective per-host cap;
- custom-dialer client: idle timeout + effective per-host cap;
- resolved one-shot direct client: idle timeout, no cap;
- SNI client: idle timeout, no cap;
- custom-SNI client: idle timeout, no cap;
- forward-proxy cached client: idle timeout, no cap;
- CONNECT cached client: idle timeout, no cap;
- SOCKS cached client: neither idle timeout nor cap.

The route-specific omission is not documented as intentional and contradicts the current public architecture prose.

## Required design

One client-level physical idle policy should be resolved once from `PoolConfig` and applied to every persistent Hyper client unless a route has a concrete protocol reason not to support it.

The policy consists of:

- `idle_timeout` / `keepalive_expiry`;
- effective per-host idle cap: `max_idle_connections_per_host.or(max_idle_connections)` under the current API contract;
- a timer installed whenever Hyper idle timeout semantics require one.

Logical pool permits (`max_in_flight_requests*`) remain entirely separate.

## Required implementation

### 1. Add the required Hyper pool timer

Update the common Hyper-builder policy path so that when idle timeout is configured, the builder also receives the runtime timer required by `hyper-util`.

Preferred implementation:

```rust
builder.pool_timer(hyper_util::rt::TokioTimer::new());
builder.pool_idle_timeout(idle_timeout);
```

Use the exact API supported by the locked version. Do not add a dependency.

If Hyper requires the timer even when no timeout is set for another configured idle policy, follow upstream semantics and document why.

Acceptance:

- [ ] A configured idle timeout has an installed timer on every relevant Hyper builder.
- [ ] No timer is invented for H3; QUIC idle policy remains owned by the existing H3 connector.
- [ ] `Timeout.pool` and `Timeout.total` remain request acquisition/lifetime budgets and are not reused as idle policy.

### 2. Centralize effective idle policy lookup

Avoid repeatedly reconstructing the effective cap from raw `PoolConfig` fields in unrelated builder sites.

Add crate-private helpers equivalent to:

- `Pool::idle_timeout()`;
- `Pool::max_idle_per_host()` or a small immutable `HyperPoolPolicy` view.

The helper must preserve current API precedence exactly.

Acceptance:

- [ ] One crate-private source defines the effective Hyper idle timeout/cap.
- [ ] Standard and route-specific clients consume the same resolved values.

### 3. Apply policy to all persistent Hyper client families

Audit and correct:

- standard H1/H2 client;
- advanced direct client;
- UDS client;
- custom-dialer client;
- SNI client;
- custom-dialer SNI client;
- SOCKS client;
- HTTP forward-proxy client;
- HTTPS CONNECT client.

For request-scoped isolated clients such as resolved-target clients, applying the timeout is harmless but an idle cap is less material because the client is not retained. Keep behavior simple and consistent where practical, but prioritize persistent clients.

No route should silently omit configured policy merely because it owns a custom connector.

Acceptance:

- [ ] Every persistent Hyper client applies the resolved idle timeout and per-host cap.
- [ ] Any deliberate exception is documented at the call site and tested.
- [ ] H2-only protocol policy and canceled-request retry policy remain unchanged.

### 4. Add deterministic idle-expiry integration tests

Do not rely on sleeping for long production defaults. Use a short local timeout and a local server that counts accepted physical connections.

Representative direct test:

1. create a client with idle timeout around 50–150 ms;
2. issue request A and fully consume/close its response;
3. wait materially beyond the configured idle timeout;
4. issue request B to the same origin;
5. assert a new server-side TCP connection was accepted.

Control case:

- repeat with no idle timeout or with a delay below the timeout and prove compatible reuse occurs where Hyper behavior permits.

Avoid brittle exact scheduling: use generous margins and connection counters rather than elapsed-time equality.

### 5. Cover at least one cached route-specific client

Add a deterministic test for one route whose policy was omitted on the baseline, preferably SOCKS because it currently receives neither timeout nor cap.

The test should prove:

- one cached eggfetch route client remains present;
- the physical idle connection expires under configured policy;
- the subsequent request reconnects through the same cached client rather than requiring a new eggfetch route-cache entry.

If a SOCKS fixture cannot expose the connection count cleanly without disproportionate work, use SNI or forward proxy and add a direct unit assertion that SOCKS construction receives the same policy helper.

### 6. Cover idle-per-host cap propagation

Use a route where multiple physical H1 connections can be created concurrently, then release them idle and verify the configured cap prevents retaining more than intended.

Because Hyper does not expose its idle pool directly, server-side connection-close/reconnect observation is acceptable. Do not add production metrics solely for this test.

At minimum add source/unit tests proving all builder families receive the effective cap, plus one wire-level cap regression on a representative route.

### 7. Reconcile documentation

After executable behavior is fixed, update only affected prose:

- `docs/architecture/core-timeout-pool.md`;
- `docs/architecture/dependency-policy.md` if it mentions route-specific pool behavior;
- public rustdoc for `Limits`/`PoolConfig` if needed.

Do not claim a global idle-connection cap if Hyper only supports per-host capping. Preserve the current documentation distinction.

## Files expected to change

Likely:

- `crates/eggfetch-core/src/client.rs`;
- `crates/eggfetch-core/src/pool.rs`;
- representative transport/integration tests;
- narrow timeout/pool documentation.

No manifest/lockfile change is expected.

## Validation

Run focused tests while implementing, then:

```sh
./scripts/check.sh
```

Also run affected proxy/SOCKS/SNI and HTTPX limits/keepalive compatibility tests.

Do not renew Stage C here; final requalification belongs to the parent program closure.

## Non-goals

- no custom socket pool;
- no LRU dependency;
- no H3 idle-policy redesign;
- no logical concurrency semantic change;
- no new public timeout type;
- no connection-reuse metric API;
- no routine CI expansion.

## Exit criteria

- [ ] Hyper idle timeout has a functioning timer.
- [ ] Every persistent Hyper client family receives the same intended idle timeout and per-host cap.
- [ ] Direct idle expiry is proven at the physical-connection level.
- [ ] At least one route-specific cached client is proven to obey the same policy.
- [ ] `Limits::compat()` 5-second keepalive contract is executable behavior rather than documentation-only configuration.
- [ ] Tier 1 and focused compatibility tests pass.
