# Rust Surface Containment

This inventory records the Rust surfaces that the repository must preserve
while keeping new implementation details private. It is a compatibility
record, not a proposal to hide or redesign existing public items.

## Surface categories

| Category | Examples | Rule |
| --- | --- | --- |
| Intended native API | `Client`, `ClientBuilder`, request/response types, policies, bodies, TLS and routing configuration | Public paths and signatures are deliberate and reviewed by the profile contracts and API oracle. |
| Historical public compatibility surface | `transport::alt_svc`, `direct_connector`, `lifecycle`, `metrics`, `dialer`, `Pool`, `PoolConfig`, `PoolMetrics`, `PoolGuard`, and their public records/snapshots | These items remain public because they already exist. Do not remove, move, rename, narrow, or add adjacent implementation helpers. |
| Test-only public surface | `test-util`-gated transport hooks and fixtures | Preserve the existing feature gate and symbols; do not make production-only hooks public for test convenience. |
| Private implementation | `client/routes.rs`, `client/connectors.rs`, `proxy/identity.rs`, private transport clients/caches, and `pub(crate)` helpers | Keep visibility private or crate-local. Add no public re-export to ease internal wiring. |

## Audited low-level areas

- `eggfetch_core::transport::alt_svc`: authenticated cache/state,
  suppression and routing types, and existing constants are frozen. HTTP/3
  remains experimental; this record does not add H3 parsers, route controls,
  connection drivers, or mutation/test-injection APIs.
- `eggfetch_core::transport::direct_connector`: the existing public
  caller-owned connector seam and its feature relationship remain unchanged;
  private Hyper adaptation stays private.
- `eggfetch_core::transport::lifecycle`: existing admission and established-I/O
  guard contracts remain unchanged.
- `eggfetch_core::transport::metrics`: existing metrics, snapshot records,
  counters, fields, and methods are compatibility debt and must not gain
  neighboring public mutators.
- `eggfetch_core::transport::dialer`: the caller-owned raw-stream seam and
  `Dialer`/`SocketOption` exposure remain feature-gated as before.
- `Pool`, `PoolConfig`, `PoolMetrics`, and `PoolGuard`: existing public types
  and lifecycle/metrics observations remain frozen; route/cache refactors do
  not introduce a second pool or expose its private keys.

Historical compatibility re-exports and feature-specific availability are
covered by the six profile snapshots in `compat/rust-public-api/`. The exact
public API oracle is the authoritative guard; no second source-level API
manifest is maintained here. The representative compile contracts remain a
fast check for the intended paths and feature boundaries.

## Adapter status boundaries

The Node package is still an experimental, unsupported prototype. Its npm
metadata is aligned with the coordinated `0.2.0` version and repository MIT
license, but its declaration-free `index.d.ts`, manual native-artifact
loading, and lack of npm publication remain intentional. No Node capability
or supported API is implied.

HTTP/3 remains experimental and subject to the separate graduation gate in
`core-tls-proxy-protocols.md`. Passing the private-decomposition checks does
not provide interoperability, impairment, lifecycle, or upstream-risk
evidence for graduation.
