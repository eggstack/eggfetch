# Core Hyper Client Construction and Cache Consolidation — Closure

Parent program: `plans/post-maintenance-core-integrity-and-verification-program.md`
Plan: `plans/core-hyper-client-construction-and-cache-consolidation.md`
Status: implementation complete, Tier 1 green. No compatibility renewal
(program rule: plan 7 owns the single final freeze).

## Baseline deviation (read first)

The plan assumes the tree already contains corrective/invariant plans 1–3
(proxy TLS cache identity, Hyper idle-pool timer propagation, route-cache
invariant hardening). That baseline does **not** hold: none of the three had
landed when this work started (`TlsConfig::connection_identity()` is still
pointer identity, no `pool_timer` is installed, SOCKS pools still receive no
idle tuning).

Per the parent program rule ("refactors must preserve proven behavior rather
than defining it retroactively"), this pass centralizes **current** behavior
exactly as-is and structures the new helpers so plans 1–3 land in one place
each:

- idle-pool timer installation belongs in `HyperClientPolicy::apply`;
- SOCKS idle propagation belongs in the `socks_client` policy call;
- TLS identity correction stays out of this module entirely.

## 1. Construction inventory (plan §1)

| Family | Connector | TLS ownership | H2/ALPN | Connect-timeout owner | Lifecycle | Idle policy (preserved) | Retry-canceled | Cache (preserved) | Exception |
|---|---|---|---|---|---|---|---|---|---|
| standard | hyper-rustls `HttpsConnector` / cleartext `HttpConnector` | authoritative `TlsConfig` built once in `build()` | `HttpsConnectorBuilder` enable + shared `http2_only` | client `timeout.connect` | yes | idle + per-host cap | shared | singleton `Option` | `Http3Only` / TLS error → `None` |
| direct (socket opts/local addr) | `DirectConnector` + metrics | per-build `TlsConfig` via `configure_tls_alpn` | shared `http2_only` | client `timeout.connect` | yes | idle + per-host cap | shared | singleton `Option` | built only when configured; TLS failure → cleartext-capable `None` connector (unchanged) |
| resolved target | `DirectConnector.with_resolved_target` + optional SNI | same as direct | shared `http2_only` | client `timeout.connect` | yes | idle, no cap | shared | isolated per request | no reuse by design |
| SNI | `DirectConnector.with_sni` | same as direct | shared `http2_only` | client `timeout.connect` | yes | idle, no cap | shared | `BoundedClientCache` keyed by hostname only, 256 | hostname-only key valid: version/TLS policy immutable at `ClientInner` scope |
| custom-SNI | dialer `CustomConnector` + fixed SNI resolver | same via `build_custom_connector` | shared `http2_only` | client `timeout.connect` | yes | idle, no cap | shared | same hostname cache, 256 | — |
| UDS | `UdsConnector` | per-build `TlsConfig` | shared `http2_only` | client `timeout.connect` | yes | idle + per-host cap | shared | singleton `Option` | Unix only |
| custom dialer | dialer `CustomConnector` | per-build `TlsConfig` | shared `http2_only` | client `timeout.connect` | yes | idle + per-host cap | shared | singleton `Option` | TLS failure → `None` (unchanged) |
| SOCKS | `SocksConnector` | per-call `TlsConfig` (client or default) | shared `http2_only` | client `timeout.connect` | yes | **none** | shared | `BoundedClientCache` keyed by `SocksRouteKey`, 64 | no idle tuning: current behavior, pending idle-pool corrective |
| forward proxy | `ForwardProxyConnector` | n/a (plaintext leg) | **never `http2_only`** | per-route connect timeout | yes | idle, no cap | shared | `BoundedClientCache` keyed by `ForwardRouteKey`, 64 | H1 absolute-form leg: intentional H1-only exception |
| CONNECT | `ConnectProxyConnector` | client `TlsConfig` + SNI hints | shared `http2_only` | per-route connect timeout | yes | idle, no cap | shared | `BoundedClientCache` keyed by `ConnectRouteKey`, 64 | multi-target fallback stays handshake-specific |
| H3 | QUIC (`H3Connector`) | QUIC stack | n/a | shared connect budget | no | QUIC idle from pool | n/a | separate H3 cache | out of scope (non-goal), untouched |

## 2–4. What was built

New crate-private module `crates/eggfetch-core/src/transport/hyper_client.rs`:

- `HyperClientPolicy` with `persistent()` / `cached_route()` /
  `forward_route()` constructors taking semantic inputs (retry flag, idle
  values, `HttpVersionPolicyEnabler`) and one `apply()` owning retry,
  H2-only, idle timeout, and per-host cap. No route family reimplements
  builder policy; `client.rs` shrank by ~150 lines with no public API change.
- `build_hyper_client()`: one generic helper for `route connector ->
  ConnectTimeout -> LifecycleConnector` + Tokio-executor build. Wrapping and
  building share one function because the generic bounds are identical; a
  separate wrap-only helper would not be clearer (plan §3 allowance).
  Monomorphized concrete types throughout — no boxed connectors.
- `BoundedClientCache<K,V>`: centralized get-or-build-and-evict with
  borrowed-key `get`, replace-without-evict `insert`, arbitrary eviction at
  capacity. Capacities unchanged and explicit (256 / 64 / 64 / 64).
  Construction under the cache lock remains CPU/local-only; failures return
  before insert and never poison the cache.
- Unit tests: bound/eviction/replace/hit semantics, H2-only selection per
  policy (feature-aware), forward-route H1-only exception.

## 5. `hyper-util::client::pool` qualification — do not adopt

Locked version 0.1.20, source-reviewed (`src/client/pool/{cache,map,
singleton}.rs`):

- `pool::cache`: single-destination service cache, unbounded `Vec`, types
  deliberately unnameable — cannot represent keyed route clients.
- `pool::map`: keyed but unbounded (`or_insert_with`, no eviction) with
  sealed/unnameable builder types; caches `MakeService` outputs, with no
  coexistence story for `legacy::Client` physical pooling, lifecycle
  wrappers, or secret-safe route keys.
- `pool::singleton`/`negotiate`: connection-level primitives, not route-keyed
  configured-client caches.

Adoption would add an abstraction layer without removing code and risks
route-key security, timeout, pinning, lifecycle, and MSRV behavior. No
throwaway fixture was needed: the unnameable-type design and missing
bounded-keyed semantics are conclusive at the source level. No new
dependency was added.

## 6–7. Key/capacity invariants (preserved, not redesigned)

- Immutable-client policy stays out of route keys (SNI hostname-only keying
  documented at the field); request-scoped proxy state stays out of reusable
  connectors (prior total-deadline corrective untouched).
- Capacities keep route-specific names in `transport::hyper_client`.

## 8. Validation

- `cargo test -p eggfetch-core --all-features client::` — 37 passed.
- New `hyper_client` unit tests — 5 passed.
- `direct_transport_tests` + `http2_tests` + `custom_dialer_tests` — 54 passed.
- `proxy_tests` + `pool_tests` + `integration` — 160 passed.
- `./scripts/check.sh` (Tier 1) — green locally (see commit CI).
- Feature-profile `cargo check`: no-default, `http1`, `http1,tls-rustls`,
  `http1,tls-rustls,tls-native-roots`, `--all-features` — all compile
  (minimal-profile dead-code warnings pre-existing, unrelated).
- Existing H2-only tests green; no H3 behavior touched (H3 suites rerun in
  Tier 1).

## Exit criteria

- [x] Common Hyper builder policy has one owner (`HyperClientPolicy::apply`).
- [x] Persistent route clients consume the same idle/retry/protocol policy.
- [x] Bounded cache mechanics centralized (`BoundedClientCache`).
- [x] No async network operation under route-cache mutexes (documented
  contract; construction sites unchanged in kind).
- [x] Route-specific connector/security behavior explicit (inventory above;
  forward-H1 and SOCKS-no-idle exceptions documented at constructor + field).
- [x] `client.rs` materially shorter/less repetitive, no public API growth.
- [x] hyper-util pool decision recorded with source rationale (above).
- [x] Tier 1 and focused route tests pass.

## Follow-ups (not this plan)

- Plans 1–3 still pending; their landing spots are marked in
  `transport::hyper_client` docs.
- `plans/README.md` does not index this program's children yet; recommend
  plan 7 (program closure) index them together.
- No compatibility renewal performed (program closure rule).
