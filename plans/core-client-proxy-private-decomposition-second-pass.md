# Core Client/Proxy Private Decomposition — Second Pass

Planning baseline: `c53eebc47569279b61c0611d4523c5132bdbcaeb`
Parent program: `plans/api-preserving-private-architecture-containment-program.md`
Normative verification policy: `docs/verification-policy.md`

## Objective

Reduce remaining private change concentration in
`crates/eggfetch-core/src/client.rs` and
`crates/eggfetch-core/src/proxy.rs` without changing any public Rust path,
signature, field/variant shape, feature exposure, protocol behavior, cache
identity, accepted/rejected input, or compatibility claim.

This is a second decomposition pass. The prior maintenance campaign already
extracted `client/config.rs`, `proxy/no_proxy.rs`, and
`proxy/environment.rs`. This plan should complete only the private moves that
produce clear ownership wins; it must not chase file size for its own sake.

## Baseline constraints

Before moving code:

1. Run the stable public compile contracts for all supported profiles.
2. Run the exact Rust public API oracle from `compat/rust-public-api/`.
3. Record the current public surface hashes/diffs in the plan closure notes.
4. Run focused client/proxy tests so the before-state is known green.

The exact public API snapshots are evidence. Do not regenerate them as part of
this implementation.

## Part A — decompose private client route/cache ownership

Keep these public declarations and their public method implementations at
their current canonical paths in `client.rs`:

- `Client`;
- `ClientBuilder`;
- every existing public builder method;
- every existing public `Client` method;
- existing feature-gated public methods and trait impls.

Move only private/`pub(crate)` machinery.

### Candidate: `client/routes.rs`

Own private route/cache identities and route-specific client acquisition that
currently live beside the public builder.

Candidate contents:

- `ResolvedRouteKey` and its constructors;
- bounded resolved-route cache lookup/insert orchestration;
- SNI client lookup/construction orchestration;
- SNI custom-client lookup/construction orchestration;
- SOCKS route client lookup;
- forward-proxy route client lookup;
- CONNECT route client lookup.

Requirements:

- cache key equality/hash inputs remain exactly equivalent;
- cache capacities remain sourced from existing
  `transport::hyper_client::*_CACHE_MAX_ENTRIES` constants;
- TLS/SNI/proxy/resolved-address/http-version/physical-policy identity
  separation remains unchanged;
- no request-local total deadline enters persistent route identity;
- no global/shared cache is introduced;
- cache eviction behavior remains unchanged.

If a single `routes.rs` would hide too much, split private child modules such
as `client/routes/resolved.rs` and `client/routes/proxy.rs`, but do not
create public modules.

### Candidate: `client/connectors.rs`

Move private connector preparation helpers that are not themselves the
central Hyper-client policy owner.

Candidate contents:

- standard connector construction helpers;
- custom connector construction helpers;
- client-level ALPN preparation helper if dependency direction remains clear.

Requirements:

- `transport/hyper_client.rs` remains the single owner of Hyper client
  builder policy and bounded cache implementation;
- `transport/direct_connector.rs`, `transport/dialer.rs`, and proxy
  connector modules retain their transport-specific ownership;
- no connector path is duplicated merely to make imports convenient;
- all existing cfg expressions are preserved exactly;
- HTTP/1-only, HTTP/2-only, Auto, custom-dialer, SNI, resolved-target,
  local-address, socket-option and UDS behavior remains byte/semantic
  equivalent.

If moving a helper introduces circular dependencies or obscures transport
ownership, leave it in `client.rs`.

## Part B — preserve `ClientInner` as a private integration boundary

`ClientInner` may remain in `client.rs` if moving it would force broad
visibility expansion. Alternatively it may move to a private child module
only if:

- every field remains private or `pub(crate)` with no visibility increase;
- `Client` layout/traits/public behavior do not change;
- no public type aliases or re-exports are added;
- route/cache/transport ownership becomes clearer, not more indirect.

Do not introduce a public "engine", "context", "transport manager", or generic
client-internals abstraction.

## Part C — move private proxy parsing/matching mechanics

Keep these public types at their current canonical paths:

- `NoProxyRule`;
- `NoProxy`;
- `ProxyAuth`;
- `ProxyRule`;
- `ProxyDecision`;
- `ProxyConfig`;
- `Proxy`;
- `ProxyEnvironment`.

Their public methods remain available at the same paths and with identical
signatures.

Expand the existing private `proxy/no_proxy.rs` ownership to include private
matching/parsing primitives where this can be done without changing native vs
HTTPX semantics.

Candidate moves:

- `parse_entry`;
- private native/HTTPX entry classification helpers;
- domain suffix/host/exact-host matching helpers;
- IPv4/IPv6/CIDR matching helpers;
- default-port matching helpers.

Requirements:

- native `NoProxy::parse` and HTTPX-specific `parse_httpx` semantics remain
  intentionally distinct;
- localhost, leading-dot, host+port, CIDR-looking, bracketed IPv6 and
  malformed-entry behavior stays identical;
- bounded error rendering remains unchanged;
- no parsing rule is generalized to make the implementations "cleaner."

Public `NoProxy` methods may remain in the parent and delegate to private
helpers.

## Part D — expand private proxy environment ownership

Expand `proxy/environment.rs` only for private environment normalization and
selection helpers.

Candidate moves:

- proxy URL normalization;
- environment key precedence helpers;
- `NO_PROXY` extraction/normalization;
- scheme fallback selection helpers;
- private parsing glue used only by `ProxyEnvironment`.

Requirements:

- lowercase/uppercase precedence remains unchanged;
- scheme-specific proxy beats fallback exactly as today;
- environment snapshots remain stable after source mutation;
- invalid proxy values continue to fail closed with redacted errors;
- no process-global mutable environment cache is introduced.

Public `ProxyEnvironment::{new,from_map,from_env,resolve,client_proxies}`
remain at the same canonical path.

## Part E — isolate private proxy connection identity if useful

`ProxyConfig::connection_identity()` and related private identity encoding
may move to a private `proxy/identity.rs` only if doing so makes the fields
that fragment transport reuse more auditable.

Acceptance requirements:

- byte output is identical for all existing test vectors;
- strict/weak TLS identities remain isolated;
- proxy headers that do not affect connection identity remain treated
  identically;
- resolved-address ordering and route identity remain unchanged;
- no hash-only replacement is introduced where current bytes are observable
  internally in tests/debugging.

## Part F — contain test/fuzz-only CONNECT response parsing

The hidden `parse_proxy_response_bytes` test/fuzz helper at the end of
`proxy.rs` may move to a private test-support child module if that improves
ownership.

Do not make it a production parser and do not replace
`eggfetch-http-connect` as the production CONNECT wire owner.

Keep/extend conformance tests proving overlapping behavior agrees with
`eggfetch-http-connect::read_connect_response_head`, while preserving
intentional differences in input domain and error type.

## Part G — move unit tests with private ownership

Large in-module test blocks should be split only when the corresponding
private implementation moves.

Maintain focused coverage for:

- client construction/defaults;
- timeout merging;
- TLS provider/root behavior;
- HTTP version policy;
- custom/direct connector ALPN;
- route-cache isolation and bounds;
- content-length rules;
- URL credential rejection/redaction;
- proxy URL parsing/redaction;
- proxy auth validation and CONNECT shared subset conformance;
- native vs HTTPX NO_PROXY behavior;
- environment precedence/snapshots;
- resolved proxy addresses;
- proxy connection identity;
- hidden CONNECT response parser bounds.

Do not convert meaningful unit tests into broad integration tests solely to
reduce source-file length.

## Validation

At minimum run:

```sh
./scripts/check.sh
./scripts/check.sh extended
```

Also run focused supported-profile contracts and exact API comparison:

```sh
cargo test -p eggfetch-core --test public_api_contracts
cargo test -p eggfetch-core --test public_api_contracts --all-features
cargo test -p eggfetch-core --test proxy_tests --all-features -- --test-threads=1
```

Use the repository's documented exact Rust API-oracle invocation from
`compat/rust-public-api/README.md`.

Run affected compatibility tests for proxy, transport, HTTP/2, TLS, redirect,
retry and downstream behavior when touched.

## Acceptance criteria

- [ ] Public `Client`/`ClientBuilder` declarations and all public methods
      remain at the same canonical paths and exact surface.
- [ ] Public proxy declarations and methods remain at the same canonical paths
      and exact surface.
- [ ] Exact Rust public API oracle reports zero unexplained drift for every
      supported profile.
- [ ] No new public module/type/function/constant is introduced.
- [ ] Substantial private route/cache/connector responsibility is removed from
      `client.rs` where dependency direction remains clear.
- [ ] Substantial private NO_PROXY/environment/identity/test-support
      responsibility is removed from `proxy.rs` where semantics remain exact.
- [ ] Cache identity, capacity and reuse/fragmentation tests remain green.
- [ ] Native and HTTPX NO_PROXY differential behavior remains green.
- [ ] CONNECT production ownership remains in `eggfetch-http-connect`.
- [ ] No behavior change is accepted through a compatibility waiver.
- [ ] Tier 1 and extended validation pass.

## Stop conditions

Stop a proposed move if it requires:

- increasing visibility to `pub`;
- changing a cfg/feature relationship;
- moving a public declaration and re-exporting it from the old location;
- introducing a circular dependency that obscures ownership;
- changing cache identity or connector semantics;
- changing proxy/NO_PROXY accepted or rejected inputs;
- making the hidden test/fuzz parser a production authority;
- changing error/redaction behavior.

A smaller decomposition is preferred over a mechanically larger but less
coherent module graph.

## Implementation record

Completed in the current qualification candidate:

- private route/cache ownership moved to `client/routes.rs`;
- private connector preparation moved to `client/connectors.rs`;
- proxy environment, NO_PROXY, and connection-identity mechanics are owned by
  `proxy/environment.rs`, `proxy/no_proxy.rs`, and `proxy/identity.rs`;
- `parse_proxy_response_bytes` remains in `proxy.rs` because it is an existing
  public/test-facing contract used by integration and fuzz consumers, so moving
  it would be public API drift.

The public `Client`/`ClientBuilder` and proxy declarations remain at their
canonical paths. Stable public contracts, the six-profile exact API oracle,
proxy/adverse focused tests, Tier 1, and the extended qualification candidate
reported zero semantic or surface drift. The final executable freeze SHA and
remote CI result are recorded by the closure plan after the qualification
commit.
