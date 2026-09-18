# Standard Route / Advanced Routing Feature Boundary

Planning baseline: current tree after `linked-byte-baseline-and-attribution.md`
Parent program: `plans/linked-binary-footprint-reduction-program.md`
Status: complete (2026-09-18; see Closure record)

## Objective

Allow a small Rust consumer to compile eggfetch's ordinary standard DNS -> TCP -> optional Rustls -> HTTP/1/H2 route without compiling advanced route construction and selection machinery it did not request.

Preserve every existing advanced routing capability and preserve the behavior/API selected by the current `native-http1`, `native-http2`, `http1`, `http2`, and default feature names.

This is a Cargo feature/ownership split over one transport engine. It is not a new light client implementation.

## Current problem

The current native HTTP feature boundary activates much more than the ordinary standard route. Under `native-http1` / `native-http2`, `ClientBuilder::build()`, `ClientInner`, pipeline route selection, and transport modules can own:

- standard DNS/Hyper client;
- custom caller `Dialer`;
- direct connector;
- resolved-target/pinned-address route cache;
- explicit SNI override cache;
- local-address binding and socket options;
- UDS on Unix;
- physical lifecycle/transport metrics shared across these routes.

These are valuable capabilities. They should remain available. But a consumer that never selects them should be able to choose a supported profile in which their constructors/caches/dispatch arms are not reachable or compiled.

## Compatibility-preserving feature shape

Exact names may be adjusted, but the additive relationship is normative.

A preferred shape is:

```toml
# Primitive Hyper protocol support used by every H1/H2 route.
transport-http1 = ["hyper/http1", "hyper-util/http1", "hyper-rustls?/http1"]
transport-http2 = ["dep:h2", "hyper/http2", "hyper-util/http2", "hyper-rustls?/http2"]

# Standard DNS -> TCP/TLS route only.
standard-route = []

# Advanced native route controls.
advanced-routing = [
  # custom Dialer
  # DirectConnector / local address / socket options
  # resolved-target routing/cache
  # SNI override routing/cache
  # UDS
]

# New supported lean high-level/native recipes may use transport + standard route.
standard-http1 = ["transport-http1", "standard-route", "high-level-url"]

# Existing names preserve existing behavior.
native-http1 = ["transport-http1", "standard-route", "advanced-routing"]
native-http2 = ["transport-http2", "standard-route", "advanced-routing"]
http1 = ["native-http1", "high-level-url", ...existing policy compatibility features...]
http2 = ["native-http2", "high-level-url", ...existing policy compatibility features...]
```

Do not adopt these exact names if they create confusing public semantics. Do preserve the capability graph.

Cargo features are additive. Never create a `minimal` feature that disables APIs activated by `native-http1` or `http1`.

## Required work

### 1. Inventory advanced-route ownership

Before applying cfgs, create a source table for:

- `ClientBuilder` fields/methods;
- `ClientConfig` fields;
- `ClientInner` clients/caches;
- pipeline route enum/selection inputs;
- transport modules;
- public re-exports.

Classify each as:

- required for standard route;
- required for all native routes;
- advanced-routing only;
- proxy/H3 owned by their existing features;
- observability/lifecycle shared and intentionally retained.

At minimum audit:

- `transport/dialer.rs`;
- `transport/direct_connector.rs`;
- `transport/direct.rs`;
- `transport/uds.rs`;
- SNI client caches;
- resolved-route cache;
- route selection;
- transport hints / resolved target;
- local address / socket options builder methods.

### 2. Keep one standard connector implementation

The lean route must use the existing standard resolver/Hyper/Rustls construction.

Do not:

- create `LiteClient`;
- bypass `ClientBuilder`;
- open sockets manually beside Hyper;
- implement a second timeout/pool/body/error stack.

The standard route still needs:

- typed DNS/refused/connect provenance;
- connect timeout;
- total deadline;
- read/write inactivity handling where applicable;
- logical pool admission;
- Hyper physical reuse;
- body caps in the high-level layer;
- cancellation safety;
- TLS verification and SNI using the logical origin.

### 3. Gate advanced builder state

For the new lean profile, advanced-only builder/client state should not exist in the compiled type where practical.

Candidates include:

- `dialer`;
- `direct_connector_config`;
- direct client;
- custom client;
- SNI client caches;
- resolved-target client cache;
- UDS path/client.

Do not merely leave all fields present and make their public setters unavailable if that keeps the same constructors/dispatch machinery linked.

### 4. Gate route selection arms

The standard-only profile should not compile branches for routes it cannot configure.

Keep the route model readable. Prefer cfg-gated enum variants and inputs over a duplicated route selector.

Proxy and H3 continue to use their existing feature ownership. If enabling `proxy` or `http3` necessarily re-enables parts of advanced routing, encode that dependency explicitly.

### 5. Preserve existing public feature surfaces

The following must compile with the same public capabilities they have before this plan:

```text
default
http1
http2
native-http1
native-http2
proxy combinations
http3 combinations
Python adapter feature set
CLI adapter feature set
FFI/Node feature sets
```

Do not require an existing consumer to add `advanced-routing` manually.

### 6. Add one documented lean recipe

Document a supported recipe for ordinary embedded H1 + Rustls/WebPKI clients.

The exact final feature list depends on the policy-boundary plan, but after this plan alone it should be possible to select the standard route without advanced route machinery.

Document what the lean route intentionally omits:

- custom `Dialer`;
- caller-supplied resolved addresses;
- SNI override;
- local-address/socket-option route;
- UDS.

Omission in a new opt-in profile is not deprecation of those features.

## Tests

Add compile/runtime coverage proving both absence and preservation.

### Lean route

- HTTP loopback request;
- HTTPS loopback request with test CA;
- DNS failure typed through `send_detailed` or equivalent selected surface;
- connection refused;
- connect/total/read timeout;
- keep-alive reuse;
- body limit;
- cancellation;
- no advanced route cache/client is constructible through the selected profile.

### Existing advanced profiles

Retain focused tests for:

- custom dialer;
- resolved target;
- SNI override;
- local address/socket options;
- UDS;
- H2 where applicable;
- proxy/H3 feature interactions.

Feature compile fixtures should prove current public aliases did not lose methods/types.

## Footprint evidence

Re-run the Gregg-like fixture with:

1. current high-level `http1,tls-rustls`;
2. new standard-route equivalent with otherwise identical high-level behavior.

Record:

- stripped bytes;
- `cargo bloat --crates`;
- top symbols;
- dependency count/tree;
- whether advanced-route modules disappeared from linked attribution.

A meaningful linked-byte reduction is expected. No arbitrary threshold is required.

If the new profile does not materially reduce linked size, inspect why before adding more cfg complexity.

## Validation

Run focused feature builds/tests during implementation and then:

```sh
./scripts/check.sh
```

Also run the relevant feature-slice checks from the repository's extended matrix. Do not renew exact-SHA HTTPX profiles yet; final closure owns that.

## Non-goals

- no second client type;
- no second pool;
- no alternate TLS stack;
- no behavior change to advanced routing;
- no removal of transport metrics unless later attribution separately justifies a feature boundary;
- no proxy/H3 redesign;
- no downstream-specific Gregg API;
- no new production dependency.

## Exit criteria

- [ ] Standard route has an explicit compile-time owner below existing compatibility aliases.
- [ ] A supported standard-only H1 profile exists.
- [ ] Advanced route state/dispatch is absent from that profile where technically practical.
- [ ] Existing `native-http1` / `http1` and other established profiles retain current APIs and semantics.
- [ ] Standard DNS/TLS/pool/timeout/failure/body behavior uses the same engine implementation.
- [ ] Advanced routing tests remain green.
- [ ] Measured linked-byte effect is recorded.
- [ ] Tier 1 and focused feature checks pass.

## Closure record (2026-09-18)

### Feature design (as implemented)

```toml
transport-http1 = ["hyper/http1", "hyper-util/http1", "hyper-rustls?/http1"]
transport-http2 = ["dep:h2", "hyper/http2", "hyper-util/http2", "hyper-rustls?/http2"]
standard-route = []
advanced-routing = []
native-http1 = ["transport-http1", "standard-route", "advanced-routing"]
native-http2 = ["transport-http2", "standard-route", "advanced-routing"]
standard-http1 = ["transport-http1", "standard-route", "high-level-url"]
standard-http2 = ["transport-http2", "standard-route", "high-level-url"]
http1 = ["native-http1", "high-level-url", "logical-retry", "redirects", "basic-auth"]
http2 = ["native-http2", "high-level-url", "logical-retry", "redirects", "basic-auth"]
```

Existing `native-http1`/`native-http2`/`http1`/`http2`/default behavior
preserved (they retain transport + both route capabilities + policy bundle
where applicable). New opt-in lean recipes: `standard-http1`/`standard-http2`
(+ `tls-rustls`) for Bearer-only single-attempt standard-route clients, and
`transport-http1`/`transport-http2` + `standard-route` for leanest native
`execute_http_body` transport. No `minimal` subtractive feature; no second
`Client`/pool/engine; Hyper owns physical reuse; `proxy`/`http3` re-enable
advanced routing transitively via `http1`.

### Code boundaries

- `transport/mod.rs`: `TimeoutHyperClient` + `direct` module on
  `transport-http1`/`transport-http2`; `TimeoutCustomClient`/
  `TimeoutDirectClient`/`TimeoutUdsClient`/`custom_connector`/`uds` on
  `advanced-routing`; `hyper_client` on transport/advanced/proxy.
- `client.rs`: `hyper_client`/`lifecycle`/`enabler`/`persistent_policy` on
  transport-or-advanced; `custom_client`/`direct_client`/
  `direct_connector_config`/`sni_*`/`resolved_*`/`uds_client`/`dialer`/
  `uds_configured`/`uds_path`/`direct_connector_config` builder fields and
  `dialer`/`local_address`/`socket_options`/`uds_path` setters plus
  `Dialer`/`SocketOption` re-exports on `advanced-routing`;
  `RequestBuilder::resolved_addresses` on `advanced-routing`. Lean omissions
  fail closed (`Unsupported` mentioning `advanced-routing`) where hints could
  otherwise reach the standard path.
- `pipeline/route.rs`: `Uds`/`Custom`/`Direct`/`SniDirect` variants and arms
  on `advanced-routing`; lean `select_route` ignores advanced flags (always
  `false` from absent state) with explicit pre-selection rejection.
- `pipeline/hyper_dispatch.rs` + `pipeline/mod.rs` (`send_single_request` +
  `send_native_http_body`): UDS/Custom/Direct/SNI sends on
  `advanced-routing`; standard Hyper send on transport; lean guards reject
  `resolved_target`/`sni_hostname` with `Unsupported`.
- `pipeline/prepare.rs`: resolved/SNI/dialer/UDS compatibility checks gated
  accordingly, with lean `Unsupported` guards.
- Protocol gates (`http_version.rs`, `tls.rs`, `direct.rs` H2 paths,
  `hyper_client.rs` `http2_only`, `error.rs`/`retry.rs` `HyperClient`) moved
  from `native-http*` to `transport-http*` (native implies transport, so full
  profiles unchanged).
- `direct.rs` UDS upgrade downcasts on `all(unix, advanced-routing)` with a
  lean opaque fallback; `finalize.rs` Alt-Svc learnable-route match unchanged
  (safe: `http3` implies advanced, so `Direct` exists whenever the `http3`
  block compiles).

### Tests

- New `crates/eggfetch-core/tests/lean_route_tests.rs` (9 tests, stable APIs
  only + cfg-branched advanced-hint rejection): HTTP/HTTPS loopback,
  keep-alive reuse, typed DNS/refused, total timeout, body cap,
  cancellation, 3xx policy passthrough covered via `lean_policy_tests`,
  and lean `Unsupported` for pinned/SNI hints (full profiles take the
  advanced direct/SNI routes). Passes under both `--all-features` and
  `--no-default-features --features standard-http1,tls-rustls`.
- Existing `lean_policy_tests.rs` (6 tests) still passes under both the old
  policy-lean (`native-http1,high-level-url,tls-rustls`) and the new
  route+policy lean (`standard-http1,tls-rustls`).
- Compatibility: `cargo test -p eggfetch-core --all-features` 1335 passed
  (1320 prior + 6 policy + 9 route); `cargo test --lib` 788 passed.
- Focused feature checks: `cargo check` clean for `--no-default-features`,
  `http1`, `http1,tls-rustls`, `http1,tls-rustls,tls-native-roots`,
  `--all-features`, `standard-http1,tls-rustls`,
  `transport-http1,standard-route,high-level-url,tls-rustls`,
  `transport-http1,standard-route,tls-rustls`, and
  `http1,tls-rustls,proxy` / `multipart,proxy` / `standard-http1,tls-rustls,proxy`
  (proxy re-enables advanced via `http1`).

### Footprint measurement (same toolchain/profile/fixture as baseline)

- Full (`http1,tls-rustls`): unstripped 8,683,256 B, stripped 3,669,840 B.
- Policy-lean (`native-http1,high-level-url,tls-rustls`): unstripped
  8,609,648 B, stripped 3,600,816 B (−69,024 vs full).
- Lean standard (`standard-http1,tls-rustls`): unstripped 7,967,632 B,
  stripped 3,111,904 B (−557,936 vs full, −15.2%; −488,912 vs policy-lean).
- Reqwest aligned: stripped 3,079,840 B. Lean-vs-reqwest: +32,064 (+1.0%).
- `cargo bloat --crates`: `eggfetch_core` .text 262.0 KiB → 133.9 KiB
  (−128.1 KiB, −49%); `hyper` 124.5 → 78.6 KiB, `hyper-util` 132.7 → 68.3 KiB.
- `cargo bloat -n`: full top `send_single_request` 55.8 KiB +
  `send_with_redirects` 19.0 KiB + four `send_request` monos (standard/Dialer/
  Direct/UDS ~18 KiB each) + `DirectConnector::call` 14.2 KiB +
  `ClientBuilder::build` 13.1 KiB; lean top `send_lean` 26.1 KiB + exactly one
  standard `send_request` 21.7 KiB + `ClientBuilder::build` 7.8 KiB, zero
  retry/redirect/UDS/Dialer/Direct/SNI/resolved symbols.
- `cargo tree`: full 193 lines / 110 unique → lean 187 / 105; lean drops
  core's direct `base64`/`httpdate`/`getrandom` edges (transitive ring/TLS
  owners remain).

### Security posture

No change to roots, verification, SNI (logical-origin SNI retained in lean),
TLS versions, crypto provider, proxy auth validation/redaction, PEM parsing,
or typed failures. Pinned/SNI misuse fails closed; no silent standard-path
fallback.
