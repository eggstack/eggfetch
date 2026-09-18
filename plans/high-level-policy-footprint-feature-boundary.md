# High-Level Policy Footprint Feature Boundary

Planning baseline: current tree after `standard-route-advanced-routing-feature-boundary.md`
Parent program: `plans/linked-binary-footprint-reduction-program.md`
Status: complete (2026-09-18; standard-route plan still `planned`, implemented independently)

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

- [x] Logical retry has an explicit feature owner and can be absent from a supported lean high-level profile.
- [x] Redirect-following has an explicit feature owner and can be absent from that profile.
- [x] Existing compatibility aliases retain retry/redirect APIs and behavior.
- [x] Bearer auth works without core Basic-auth Base64 when `basic-auth` is omitted.
- [x] Retry-only `httpdate` is absent when retry is omitted.
- [x] `getrandom` ownership is truthful across retry/multipart.
- [x] Typed failure, timeout, body-limit, TLS, pool, and high-level response semantics remain available.
- [x] Linked-byte effect is measured.
- [x] Tier 1 and focused compatibility tests pass.

## Closure record (2026-09-18)

Parent plan 2 (`standard-route-advanced-routing-feature-boundary.md`) remains
`planned`, so this plan was implemented independently against current `main`
(no standard-route delta to build on). The lean profile below is the
`native-http1` + `high-level-url` + `tls-rustls` high-level recipe without
the three new policy features.

### Feature design (as implemented)

```toml
logical-retry = ["dep:getrandom", "dep:httpdate"]
redirects = []
basic-auth = ["dep:base64"]

http1 = ["native-http1", "high-level-url", "logical-retry", "redirects", "basic-auth"]
http2 = ["native-http2", "high-level-url", "logical-retry", "redirects", "basic-auth"]
multipart = ["dep:getrandom"]
proxy = ["http1", "tls-rustls", "tokio/io-util", "high-level-url", "dep:eggfetch-http-connect", "dep:base64"]
```

`base64`/`getrandom`/`httpdate` are now optional. `proxy` carries its own
`dep:base64` edge for `Proxy-Authorization` (independent of `basic-auth`);
`multipart` jointly owns `dep:getrandom` with `logical-retry` (unification
keeps it when either is selected). `httpdate` is owned solely by
`logical-retry`.

### Code boundaries

- `auth.rs`: `BasicAuth`, `AuthScheme::Basic`, `AuthScheme::basic()` gated
  on `basic-auth`; `BearerAuth` always available; redaction/validation
  unchanged.
- `retry.rs` / `pipeline/retry.rs` (`send_with_retry`): gated on
  `logical-retry`. Shared discard drain moved to `pipeline::mod`
  (`any(logical-retry, redirects)`).
- `redirect.rs` / `pipeline/redirect.rs` (`send_with_redirects`,
  `advance_redirect_hop`): gated on `redirects`.
- New `pipeline/lean.rs` (`send_lean`): single-hop dispatch used when
  `redirects` is absent (as the retry-loop inner when only retry is
  enabled, and directly from `Client::{send, send_detailed}` when both
  policy features are absent). Returns 3xx without following, empty
  history, no cross-origin reconstruction.
- `Request`/`RequestBuilder`/`RequestParts`/`ClientConfig`/`ClientBuilder`:
  `redirect`/`retry` fields and builder methods gated on their features;
  exhaustive destructuring preserved (new fields still fail to compile).
  `#[cfg]` on assignment statements is rejected by rustc (E0658), so
  cfg'd fields use the `set_*` setters.
- `body.rs::try_clone_for_retry` gated on `any(logical-retry, proxy)`
  (CONNECT multi-address fallback reuses it);
  `RequestParts::shrink_total_deadline` on `logical-retry`;
  `HistoryEntry::from_response`/`Response::set_history` on `redirects`.
- `lib.rs` re-exports gated accordingly. Python/CLI/FFI/Node adapters
  unchanged: they resolve `http1`/`http2`, which retain the full policy
  bundle.

### Tests

- New `crates/eggfetch-core/tests/lean_policy_tests.rs` (6 tests, stable
  APIs only so they pass under both profiles): 503 single attempt, 302
  passthrough with empty history, Bearer header + redaction, typed
  refused/timeout/body-cap failures.
  - `--all-features`: 6 passed.
  - `--no-default-features --features native-http1,high-level-url,tls-rustls`:
    6 passed.
- Compatibility: `cargo test -p eggfetch-core --all-features` 1320 passed;
  full Tier 1 (`./scripts/check.sh`) green, including workspace tests,
  Python suite, compat smoke kernel, and Node check.
- Focused feature checks: `cargo check` clean for `--no-default-features`,
  `http1`, `http1,tls-rustls`, `http1,tls-rustls,tls-native-roots`,
  `--all-features`, lean, lean+each-policy-feature, lean+all-three,
  and `http1,tls-rustls,proxy` / `multipart,proxy` combos.

### Footprint measurement

Same toolchain/profile/fixture for both (`rustc 1.98.1`,
`x86_64-unknown-linux-gnu`, `qualification/embedded/eggfetch-min` release
shape `lto="thin"`, `codegen-units=1`, `panic="unwind"`, `strip` copy;
lean fixture is the same streaming-GET source with
`native-http1,high-level-url,tls-rustls`):

- Unstripped: full 8,683,352 B → lean 8,605,056 B (−78,296 B).
- Stripped: full 3,669,840 B → lean 3,600,816 B (−69,024 B, −1.9%).
- `cargo bloat --crates`: `eggfetch_core` .text 262.0 KiB → 236.9 KiB
  (−25.1 KiB eggfetch-owned). `httpdate`/`base64` absent from the lean
  link; `getrandom` 748 B remains via ring/Rustls TLS crypto (transitive,
  not core's direct edge).
- `cargo tree --depth 1`: lean drops core's direct `base64`, `getrandom`,
  and `httpdate` edges (full fixture tree 189 lines → lean 183).
- `nm --size-sort`: zero `eggfetch_core::{retry,redirect,pipeline::retry,
  pipeline::redirect}` symbols in lean (remaining "retry" hits are
  rustls `HelloRetryRequest` only); full profile retains
  `send_with_retry`/`send_with_redirects`/`RetryPolicy`/`redirect_method`/etc.

### Security posture

No change to roots, verification, SNI, TLS versions, crypto provider, proxy
auth validation/redaction, or typed failures. Bearer redaction/validation
unchanged; Basic redaction unchanged where enabled. Exact-SHA compatibility
renewal remains deferred to program closure per the program closure rule.
