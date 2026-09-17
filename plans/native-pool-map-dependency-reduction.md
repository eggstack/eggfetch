# Native Pool Map Dependency Reduction

Planning baseline: `b0a09eed95b88199db2d0188ea68bf100c43b50a` (`main`, 2026-09-17; `eggfetch-core` 0.1.6)
Status: implemented

## Objective

Remove the unconditional `dashmap` dependency from `eggfetch-core` without changing logical pool admission semantics, cancellation safety, per-origin isolation, or Hyper's ownership of physical connection reuse.

This is a general dependency/maintenance reduction for every eggfetch consumer. It is not an Eggpool-specific optimization and must not add downstream-specific APIs or behavior.

This plan is intentionally separate from URL/IDNA dependency separation. Removing `dashmap` is a small, self-contained pool-internal change that should land and qualify independently before the wider native-URI feature-boundary work.

## Current state on the planning baseline

`crates/eggfetch-core/Cargo.toml` declares `dashmap = "6"` unconditionally.

`crates/eggfetch-core/src/pool.rs` uses `DashMap<OriginKey, Arc<PerOriginSemaphore>>` for the lazily created per-origin logical-concurrency semaphores. A separate async `tokio::sync::Mutex<()>` serializes table eviction/insertion. Hyper still owns the actual HTTP connection pool; this map only tracks logical request-admission semaphores.

The current algorithm has three important properties that must survive:

1. global admission is acquired before per-origin admission;
2. one logical origin maps to one semaphore while requests are using or waiting on it;
3. idle per-origin entries can be evicted so arbitrary-origin traffic does not grow the table forever.

The current code also contains a narrow race worth correcting during this change. The existing-origin fast path clones an `Arc<PerOriginSemaphore>` from `DashMap`, drops the map guard, and only then increments the entry's `waiters` count through `WaiterGuard`. An insertion/eviction path can therefore theoretically remove an otherwise-idle entry in the gap between the clone and waiter registration, then install a second semaphore for the same origin. This is difficult to hit, but it defeats the invariant the waiter counter is intended to protect. The replacement should make "lookup/create + register waiter" one table-locked operation.

## Non-goals

Do not:

- replace Hyper's connection pool;
- change `PoolConfig` public fields or the logical-vs-physical semantics already documented;
- change `max_connections`/`max_in_flight_requests` precedence;
- add `parking_lot`, `scc`, `moka`, another concurrent-map crate, or a custom lock-free table;
- add sharding solely to reproduce `DashMap`'s implementation;
- introduce a background eviction task;
- add unsafe code;
- make binary size a hard acceptance threshold;
- refactor unrelated request, route-cache, or transport code.

The desired result is less machinery and one fewer direct dependency.

## Target design

Use a standard-library map protected by a short-lived synchronous lock:

```rust
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

struct PoolInner {
    global_semaphore: Option<Arc<Semaphore>>,
    per_origin: RwLock<HashMap<OriginKey, Arc<PerOriginSemaphore>>>,
    config: PoolConfig,
    metrics: PoolMetrics,
}
```

`Mutex<HashMap<...>>` is acceptable instead if measurement shows no meaningful benefit from an `RwLock`, but do not add another dependency for this table.

The lock must never be held across `.await`, semaphore acquisition, network I/O, timeout futures, or response-body lifetime.

### Why a standard lock is sufficient

The protected operation is only an in-process map lookup/clone or bounded idle-entry sweep/insert. The expensive operation is waiting on the semaphore, and that remains outside the map lock. Existing-origin lookups should take only a read lock long enough to clone the entry and register the waiter marker. New-origin creation/eviction takes a write lock.

This is simpler than the current `DashMap` plus a second async table lock and removes the `dashmap` closure from consumers that otherwise do not need it.

## Required implementation

### 1. Make waiter registration atomic with table membership

Create a small crate-private helper that returns the entry together with a registered waiter guard. The important ordering is normative:

For an existing origin:

1. acquire the table read lock;
2. locate and clone the entry;
3. increment `entry.waiters` while the read lock is still held;
4. release the table read lock;
5. attempt/acquire the semaphore permit;
6. drop/deactivate the waiter guard after permit acquisition or on cancellation.

For a missing origin:

1. acquire the table write lock;
2. re-check the key;
3. if still missing, evict only entries that are both fully idle and have zero registered waiters;
4. insert the new entry;
5. increment its waiter count before releasing the write lock;
6. release the table lock and acquire the semaphore normally.

Because eviction needs the write lock, it cannot remove an entry between table lookup and waiter registration.

Do not increment a waiter only after dropping the map lock; that recreates the current race.

### 2. Preserve cancellation safety

Keep an RAII waiter marker. Cancellation may occur:

- after table lookup;
- after waiter registration;
- during `try_acquire_owned` fallback;
- while awaiting `acquire_owned`;
- while an outer pool/total timeout is cancelling the acquire future.

Every path must decrement `waiters` exactly once. Do not require callers to remember an explicit cleanup call.

The existing `WaiterGuard` can be retained or adjusted. Prefer changing as little public/internal structure as possible.

### 3. Preserve eviction semantics

An entry is evictable only when:

```text
available_permits == max_per_origin
AND
waiters == 0
```

Do not evict an entry with a held logical permit or a task registered to acquire it.

The sweep may remain opportunistic on missing-origin insertion. No periodic task is needed.

Do not introduce a fixed LRU capacity in this plan; current semantics bound stale entries by sweeping idle entries on churn, and changing policy would be unrelated behavior.

### 4. Handle lock poisoning without process panic

The provider/router use cases for eggfetch value failure isolation. Do not use an unconditional `unwrap()`/`expect()` on the new standard-library table lock in production paths.

Preferred options, in order:

1. convert lock poisoning into an existing `Error::Pool(...)` failure where the call surface already returns `Result`;
2. if a helper cannot conveniently return `Result`, restructure it rather than adding a panic;
3. recover a poisoned lock only if the implementation can demonstrate the table remains structurally valid and document why recovery is safe.

Do not add a new public error variant solely for lock poisoning.

### 5. Remove `dashmap`

After the replacement is complete:

- remove `dashmap = "6"` from `crates/eggfetch-core/Cargo.toml`;
- update `Cargo.lock`;
- run `cargo tree`/`cargo metadata` to determine whether `dashmap` and its unique transitive closure disappear from the resolved graph;
- do not claim dependency reduction based only on the manifest line if another crate still resolves it.

## Tests

Keep all existing pool tests. Add focused regressions for the invariants affected by the table replacement.

Required cases:

1. same-origin concurrency never exceeds the configured per-origin limit under many concurrent tasks;
2. different origins retain independent permits;
3. cancellation while waiting releases both global state and waiter registration;
4. repeated cancellation followed by valid requests does not wedge an origin;
5. idle entries are evicted on origin churn;
6. an entry with a live permit is not evicted;
7. an entry with a registered waiter is not evicted;
8. concurrent missing-origin creation never creates two independently active semaphores for one key;
9. high origin churn plus cancellation does not exceed the configured per-origin limit;
10. pool metrics (`acquisition_waits`, `acquisition_cancellations`) retain their existing meanings.

For the duplicate-semaphore race, prefer a deterministic unit-test seam around the table helper rather than a flaky timing-only test. A `#[cfg(test)]` barrier/hook localized to the pool module is acceptable if necessary, but do not expose public synchronization hooks.

## Performance qualification

The replacement trades a sharded concurrent map for a standard lock around very small operations. Verify that this simplification does not create an obvious throughput regression.

Use a bounded local comparison for at least:

- one hot origin with high concurrency;
- many origins with low contention;
- high origin churn that exercises eviction.

This can be an existing benchmark or a temporary measurement harness. Do not add a permanent benchmark CI threshold for this plan.

If the standard `RwLock<HashMap<...>>` shows a material regression under realistic eggfetch workloads, first try the simpler `Mutex<HashMap<...>>`/`RwLock` alternative and inspect lock scope. Do not immediately reintroduce a concurrent-map dependency.

## Validation

Run the repository's current verification policy rather than inventing a migration-specific CI system. At minimum:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo tree -p eggfetch-core -e features
```

Also run the feature slices and Tier 1/extended/package checks required by the current repository policy when closing the executable change.

Explicitly verify a no-default-features native slice because that is where dependency reduction matters most.

## Closure evidence

- Implementation commit: `196046f821415ea38a1eb3dd53d5df498c3bdcd6`.
- Minimal native slice (`cargo tree -p eggfetch-core
  --no-default-features --features http1 -e normal --prefix none | sort -u |
  wc -l`): **92 packages before → 86 after**. Removed closure:
  `dashmap`, `crossbeam-utils`, `hashbrown 0.14.5`, `lock_api`,
  `scopeguard`, `parking_lot_core`. (`hashbrown 0.17.1` remains in some
  wider profiles via unrelated dependencies, not via `dashmap`.)
- `dashmap` in the workspace graph: **gone**. `cargo tree --workspace -i
  dashmap` no longer matches any package and the workspace `Cargo.lock`
  contains no `dashmap` entry. Isolated non-workspace lockfiles (`fuzz/`,
  `qualification/*/`) still pin `dashmap` until those manual fixtures are
  next re-resolved; they are not part of the routine CI graph.
- Bounded smoke (temporary in-crate harness, removed before commit, no CI
  threshold added): hot origin 32×500 acquires ≈ 15.6 ms; 64 origins × 100
  acquires ≈ 7.1 ms; 5000-origin churn ≈ 15.2 ms with the table left at 1
  entry. No material regression; `RwLock` read fast path kept (no `Mutex`
  fallback needed).
- Focused pool tests: `cargo test -p eggfetch-core --all-features pool --
  --test-threads=1` → 73 passed. New regressions cover same-origin limit,
  waiter-release on cancellation, repeated-cancel recovery, live-permit and
  registered-waiter eviction immunity, barrier-seamed concurrent creation
  sharing one semaphore, churn-plus-cancellation limit preservation, and
  metrics meanings. Existing integration tests
  (`tests/pool_tests.rs`, incl. waiter-eviction) remain green.
- Standard qualification: `./scripts/check.sh` (Tier 1) green locally;
  feature-slice `cargo check` matrix green (`--no-default-features`;
  `http1`; `http1,tls-rustls`; `http1,tls-rustls,tls-native-roots`;
  `--all-features`); proxy subset
  (`--no-default-features --features http1,tls-rustls,proxy`) 621 lib tests
  green.

No binary-size claim is made (not measured under identical
toolchain/target/profile/strip settings).

## Completion criteria

This plan is complete when:

- `eggfetch-core` no longer directly depends on `dashmap`;
- the per-origin table uses only standard-library synchronization/collections;
- no table lock is held across an await point;
- lookup/create and waiter registration cannot race with idle eviction;
- cancellation cannot leak waiter state or permits;
- existing logical pool semantics and metrics are preserved;
- focused concurrency/eviction tests are green;
- standard repository qualification is green;
- dependency evidence is recorded.

The follow-on native URI/dependency-separation plan may then measure the remaining `url`/IDNA/ICU closure without `dashmap` confounding the result.