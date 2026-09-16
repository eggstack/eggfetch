# Core Hyper Client Construction and Cache Consolidation

Planning baseline: tree after corrective/invariant plans 1–3
Parent program: `plans/post-maintenance-core-integrity-and-verification-program.md`

## Objective

Reduce the repeated Hyper-client construction and bounded route-cache mechanics in `client.rs` while preserving the already-proven route-specific connector behavior.

This is an internal maintainability refactor. It must not create a second transport stack, alter public API, or erase route-specific correctness distinctions merely to reduce lines of code.

## Current maintenance problem

`ClientBuilder::build()` and `ClientInner` independently construct standard, direct, UDS, custom-dialer, resolved-target, SNI, custom-SNI, SOCKS, forward-proxy, and CONNECT Hyper clients. Repeated policy includes:

- `hyper_util::client::legacy::Client::builder(TokioExecutor::new())`;
- H2-only selection;
- `retry_canceled_requests`;
- idle timeout/timer/cap;
- `ConnectTimeout` wrapping;
- `LifecycleConnector` wrapping;
- cache insertion/eviction;
- cloning returned Hyper clients.

The connectors and route keys legitimately differ. The duplicated builder/lifecycle/cache protocol does not.

## Required design direction

Prefer a few crate-private helpers over a generic framework. Likely concepts:

- `HyperClientPolicy`: immutable H1/H2 builder policy resolved from client configuration;
- one helper that configures a legacy Hyper builder from that policy;
- one helper that wraps connectors in the common connect-timeout + lifecycle layers where type constraints permit;
- `BoundedClientCache<K,V>` or small free functions for get/insert/evict semantics.

Keep route-specific connector construction in route modules/client methods.

## Required work

### 1. Inventory common versus route-specific construction

Before refactoring, create a short source/closure table identifying for each client family:

- connector type;
- TLS ownership;
- H2/ALPN handling;
- connect timeout owner;
- lifecycle wrapper applicability;
- idle policy;
- retry-canceled policy;
- cache lifetime/key;
- known intentional exception.

Use the table to decide what may be shared. Do not infer identical semantics from similar-looking code.

### 2. Centralize Hyper builder policy

Create one crate-private policy application helper that owns all common Hyper builder knobs currently intended to be identical:

- canceled-request retry;
- H2-only mode;
- idle timeout plus required timer;
- per-host idle cap;
- any currently shared executor/runtime configuration.

The helper should take semantic inputs rather than an entire `ClientConfig` when that makes ownership clearer.

Acceptance:

- [ ] No persistent client family independently reimplements common builder policy.
- [ ] H1/H2 feature-gating remains compile-correct across supported profiles.
- [ ] Existing HTTP2-only tests remain green.

### 3. Consolidate connector lifecycle wrapping where practical

Most custom connectors are wrapped as:

`route connector -> ConnectTimeout -> LifecycleConnector -> Hyper client`.

Extract a narrow helper/type alias only if Rust's concrete generic types make the result clearer. Do not introduce boxed dynamic connectors merely to make every route share one function; monomorphized concrete connector types are acceptable.

If a generic helper becomes harder to read than repeated two-line wrapping, keep wrapping local and consolidate only the policy data.

### 4. Centralize bounded route-cache mechanics

SNI, custom-SNI, SOCKS, forward, and CONNECT caches repeat:

- lock map;
- return cloned existing value;
- build value;
- evict one if capacity reached;
- insert;
- return clone.

Introduce a tiny internal cache abstraction or helper set that preserves:

- bounded size;
- no network I/O while holding the lock;
- secret-safe key behavior;
- arbitrary eviction unless measurements justify something stronger;
- route-specific capacities.

Do not add an LRU crate for this work.

If construction can fail, design the API so failures do not poison the cache. If construction ever becomes async/networked, do not await it under the map lock.

### 5. Qualify `hyper-util::client::pool` before adopting it

The locked hyper-util version exposes newer composable pool/cache primitives. Perform a bounded source and throwaway-fixture evaluation:

- Does it cache configured services/clients or only physical connection-ready services?
- Can it represent eggfetch's route keys without exposing secrets?
- Does it coexist cleanly with `legacy::Client` physical pooling?
- Does it preserve custom connector/lifecycle wrappers?
- What MSRV/feature impact does it have?
- Would adoption reduce code, or simply add another abstraction layer?

Default outcome is **do not adopt** unless the answer is clearly simpler than the small internal bounded map. Record the decision in closure notes.

### 6. Keep immutable-client policy out of route keys

Where a cache is owned by one immutable `ClientInner`, do not copy global immutable policy into every key unless compatibility can differ within that client. For example, SNI cache may remain keyed by hostname if TLS/HTTP-version policy is immutable and shared by the cache owner.

Conversely, request-scoped proxy configuration must retain complete connection-affecting identity because it can vary within one client.

### 7. Keep cache capacities explicit

Retain current bounded capacities unless evidence justifies a change:

- SNI/custom-SNI: 256;
- SOCKS/forward/CONNECT: 64 each.

If consolidating constants, preserve route-specific names or a configuration table so the limits remain reviewable.

### 8. Regression tests

After refactoring, rerun/extend tests for:

- direct H1/H2/H2-only;
- advanced direct/local-address/socket options;
- SNI and custom-SNI;
- custom dialer;
- UDS;
- SOCKS;
- HTTP forward proxy;
- CONNECT;
- route cache bounds;
- idle timeout/cap;
- canceled-request retry toggle;
- physical admission and I/O inactivity policy;
- TLS route isolation from the corrective plan.

## Files expected to change

Primarily `crates/eggfetch-core/src/client.rs`; small private modules may be introduced under `src/client/` or `src/transport/` if that materially improves ownership. Tests may move only when preserving discoverability.

## Validation

Run:

```sh
./scripts/check.sh
```

At plan closure also run relevant feature-combination tests from `./scripts/check.sh extended` or the narrower equivalent. Do not renew compatibility profiles yet.

## Non-goals

- no public `Transport`/`ConnectorFactory` framework;
- no second connection pool;
- no route-key semantic redesign after plan 3 except bugs discovered by tests;
- no performance tuning unsupported by measurement;
- no production dependency addition for caching;
- no H3 client-cache migration.

## Exit criteria

- [ ] Common Hyper builder policy has one owner.
- [ ] Persistent route clients consume the same tested idle/retry/protocol policy.
- [ ] Bounded cache mechanics are centralized or demonstrably clearer left as small route-specific wrappers.
- [ ] No async network operation occurs under route-cache mutexes.
- [ ] Route-specific connector/security behavior remains explicit.
- [ ] `client.rs` is materially shorter/less repetitive without public API growth.
- [ ] Hyper-util pool adoption decision is recorded with source/fixture rationale.
- [ ] Tier 1 and focused route tests pass.
