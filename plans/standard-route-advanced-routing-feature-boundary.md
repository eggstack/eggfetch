# Standard Route / Advanced Routing Feature Boundary

Planning baseline: current tree after `linked-byte-baseline-and-attribution.md`
Parent program: `plans/linked-binary-footprint-reduction-program.md`
Status: planned

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
