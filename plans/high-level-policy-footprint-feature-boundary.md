# High-Level Policy Footprint Feature Boundary

Planning baseline: current tree after `standard-route-advanced-routing-feature-boundary.md`
Parent program: `plans/linked-binary-footprint-reduction-program.md`
Status: planned

## Objective

Allow an opt-in high-level Rust client to omit logical retry, redirect-following, and Basic-auth machinery while retaining the URL/request/response conveniences, Bearer auth, timeouts, body bounds, pooling, TLS, and typed failure reporting needed by small API clients.

Existing `http1`, `http2`, default, Python, CLI, HTTPX/HTTPX2, and other established feature surfaces must retain current behavior.

## Current problem

Today `high-level-url` is a broad ownership boundary. The ordinary high-level send path routes through retry/redirect orchestration even when:

- `ClientConfig.retry` is `None`;
- redirect following is disabled.

The retry implementation owns:

- retry policy types and reconstruction;
- backoff;
- jitter;
- `Retry-After` HTTP-date parsing;
- `getrandom` use.

Basic and Bearer auth share one `AuthScheme`; Basic auth keeps Base64 reachable even for Bearer-only applications.

For a scheduler-owned poller or simple API client, these policies may be intentionally absent. Their absence should be expressible at compile time in a new profile.

## Feature design

Prefer coarse capability features, not one flag per helper crate.

Conceptually:

```toml
logical-retry = ["dep:getrandom", "dep:httpdate"]
redirects = []
basic-auth = ["dep:base64"]

# Bearer auth remains available in the lean high-level client without base64.
high-level-url = ["dep:url", "dep:percent-encoding"]

# Existing compatibility alias retains historical behavior.
http1 = [
  "native-http1",
  "high-level-url",
  "logical-retry",
  "redirects",
  "basic-auth",
]
```

If `getrandom` also remains required by `multipart`, encode:

```toml
multipart = ["dep:getrandom"]
logical-retry = ["dep:getrandom", "dep:httpdate"]
```

Cargo feature unification then keeps randomness whenever either capability is selected.

Exact names may change, but existing aliases must continue to enable the old public surface.

## 1. Separate logical retry orchestration

Gate retry-specific state and orchestration behind a coarse feature.

When retry is absent:

- a high-level request is dispatched once;
- there is no retry backoff/jitter/status policy;
- there is no `Retry-After` parsing;
- request bodies do not need retry replay factories solely for retry;
- the outer total deadline still bounds the one attempt;
- Hyper's distinct canceled-idle-request retry setting remains governed by its existing transport policy and must not be conflated with logical `RetryPolicy`.

Do not change the existing behavior when retry is enabled.

### Public surface

For the new lean profile, retry types/methods may be absent because that profile explicitly did not select retry capability.

Existing `http1`/`http2`/default users continue to receive:

- `RetryPolicy`;
- `ClientBuilder::retry`;
- request retry overrides;
- existing Python/HTTPX retry behavior.

### Dependency ownership

Make `httpdate` optional and owned by logical retry.

Make `getrandom` optional only if all remaining non-retry owners are also feature-gated appropriately. Multipart remains a valid independent owner.

## 2. Separate redirect-following

Gate redirect-loop/reconstruction/history machinery behind a redirect capability.

When redirects are absent:

- responses with 3xx status are returned as ordinary responses;
- no second hop occurs;
- no redirect history is constructed;
- no cross-origin credential reconstruction logic is needed in the lean path;
- request timeout/body-limit/failure semantics remain unchanged.

This profile does not need a runtime `.follow_redirects(false)` call; absence of the capability is itself the contract.

Existing compatibility features retain the current `RedirectPolicy`, redirect builder methods, and behavior.

Do not remove URL parsing needed for the ordinary high-level request itself.

## 3. Split Basic auth from Bearer auth

Bearer auth does not require Base64. Make Basic auth the owner of the core `base64` dependency where practical.

A preferred shape:

```rust
pub enum AuthScheme {
    #[cfg(feature = "basic-auth")]
    Basic(BasicAuth),
    Bearer(BearerAuth),
}
```

Existing compatibility aliases include `basic-auth`, so current users retain `AuthScheme::basic`, `BasicAuth`, Python tuple/basic auth, proxy auth behavior owned by the proxy subsystem, and existing compatibility tests.

Do not weaken redaction or header validation.

Audit the separate `eggfetch-http-connect` crate before claiming Base64 has disappeared from the *full dependency graph*: proxy CONNECT has its own Basic-auth helper and should be handled by the residual-dependency plan.

## 4. Keep typed failure reporting in the lean path

The motivating use case depends on `send_detailed()` and `RequestFailure`.

The no-retry/no-redirect path must still preserve:

- DNS;
- connection refused;
- generic connect;
- phase-aware timeout;
- body-too-large;
- ordinary public `Error`.

Do not make detailed failure reporting depend on logical retry/redirect.

## 5. Keep high-level response/body semantics

The lean high-level path should continue to support:

- status/version/headers;
- bounded bytes;
- streaming body;
- request and client body limits;
- Bearer auth;
- default headers/user-agent;
- query/URL request construction if `high-level-url` is selected.

Compression/cookies/JSON/multipart remain independently feature-owned.

## Tests

### Lean policy profile

Compile and run a fixture with:

- standard H1 route;
- Rustls/WebPKI;
- high-level URL API;
- Bearer auth;
- no logical retry;
- no redirects;
- no Basic auth.

Prove:

- one failing connection produces one logical attempt;
- 3xx is returned without following;
- Bearer header is present and redacted in diagnostics;
- typed DNS/refused/timeout failure remains;
- body cap remains;
- total timeout remains.

### Compatibility profiles

Run existing tests proving:

- retry status/error/body replay semantics;
- redirect method rewrite/history/cross-origin auth stripping;
- Basic auth;
- Python auth;
- HTTPX/HTTPX2 redirect/retry behavior as required by current feature wiring.

## Footprint measurement

Re-run the Gregg-like fixture before and after this plan.

Record whether the lean profile removes or materially reduces:

- `getrandom`;
- `httpdate`;
- core `base64`;
- retry symbols;
- redirect symbols;
- request reconstruction/history symbols.

Use `cargo bloat` to distinguish dependency removal from actual linked-byte reduction.

## Validation

Run focused profile checks, then:

```sh
./scripts/check.sh
```

Run relevant extended feature checks because public feature availability in the new profile is changing. Full exact-SHA compatibility renewal remains deferred to program closure.

## Non-goals

- no semantic change to retries/redirects when enabled;
- no removal of Hyper's distinct stale/canceled connection retry behavior;
- no change to Python/CLI/HTTPX defaults;
- no Digest/OAuth/API-key auth redesign;
- no custom Base64 or HTTP-date implementation;
- no removal of Bearer redaction/validation;
- no attempt to eliminate every tiny dependency.

## Exit criteria

- [ ] Logical retry has an explicit feature owner and can be absent from a supported lean high-level profile.
- [ ] Redirect-following has an explicit feature owner and can be absent from that profile.
- [ ] Existing compatibility aliases retain retry/redirect APIs and behavior.
- [ ] Bearer auth works without core Basic-auth Base64 when `basic-auth` is omitted.
- [ ] Retry-only `httpdate` is absent when retry is omitted.
- [ ] `getrandom` ownership is truthful across retry/multipart.
- [ ] Typed failure, timeout, body-limit, TLS, pool, and high-level response semantics remain available.
- [ ] Linked-byte effect is measured.
- [ ] Tier 1 and focused compatibility tests pass.
