# Native Request Failure Introspection

Planning baseline: `c5a79cbf3f94741dc25e694329b1ab552976d6a2` (`main`, 2026-09-14; eggfetch 0.1.4)
Status: complete
Motivating downstream review: Gregg system-monitor client integration. Gregg is requirements evidence only; no Gregg type, endpoint, status model, feature flag, or adapter belongs in eggfetch.

## Objective

Give native Rust embedders an **optional, structured request-failure view** that can distinguish common connection-establishment failures such as DNS resolution failure and connection refusal without parsing error strings, while preserving every existing `eggfetch_core::Error` variant, `Error::kind()` value, existing `Display` behavior, and the return type/semantics of current `Client::send`, `RequestBuilder::send`, native-body, Python, CLI, HTTPX, and HTTPX2 paths.

The implementation must be additive. Existing callers that do not opt into the richer failure surface must not allocate or maintain request diagnostics merely because this API exists.

Also correct the documentation around `max_decoded_body_size`: the implementation already bounds plain/identity response bodies as well as decoded compressed bodies, so this work must document the existing invariant rather than create a second body-limiting mechanism.

## Why this is needed

The current public `Error` surface deliberately gives callers a stable coarse classifier through `Error::kind()`, but some native embedders need a little more provenance for operator-facing health/polling behavior. A monitor may reasonably distinguish:

- a request timeout;
- DNS resolution failure;
- a refused TCP connection;
- another connection-establishment failure;
- a successful HTTP response with an application/HTTP failure.

Today callers cannot do that reliably from the final public error without transport knowledge. `Error::Connect(String)` is a public tuple variant and is currently exhaustively matchable. The standard Hyper error mapping converts connection failures to that string form after inspecting the underlying `hyper_util` error, while the specialized direct connector also turns DNS/TCP failures into `Error::Connect(String)` at its own boundary. Once that conversion has happened, typed provenance is gone.

Do **not** solve this by searching `Display` strings. Hyper, OS resolver, libc, Winsock, and platform text are not a stable API, may be localized, and can change independently of eggfetch. Likewise, do not change `Connect(String)` into another field shape, add a new variant to the currently exhaustive `Error` enum, mark the existing enum `#[non_exhaustive]`, or change existing `kind()` values merely to attach metadata. Each of those would turn a small embedding improvement into an API regression.

The correct boundary is an additive detailed-result path that captures structured provenance **before** the legacy/public `Error` collapse, while keeping the legacy result path exactly as it is.

## Current repository truth to preserve

1. `eggfetch_core::Error` is a public enum. `Connect(String)` contains no typed source, while `Hyper`, `HyperClient`, `Io`, and `CustomTransport` can retain structured sources.
2. `Error::kind()` is the documented stable Rust classifier and is also relied on by binding/error translation code.
3. `transport::direct::map_send_error` currently unwraps nested eggfetch errors when present and otherwise maps `hyper_util` connection errors to `Error::Connect(error.to_string())`.
4. `transport::direct_connector` performs its own DNS resolution and TCP address fallback for resolved/SNI/socket-option paths; current DNS/TCP failures there are formatted into `Error::Connect(String)`.
5. Caller-owned `Dialer` errors already retain `DialError` and its `DialErrorKind` through `Error::CustomTransport`.
6. Request timeout phases are already modeled structurally through `Error::Timeout { phase, elapsed }`; established-I/O inactivity timeouts have their own `TransportIoTimeout` variant and helper.
7. `max_decoded_body_size` is already enforced on buffered and streaming unencoded bodies through `ResponseBody::limit_decoded_size`, and on compressed bodies through the decode limit. No second collection loop or response-cap helper is required.
8. The recent embedded-consumer qualification classified eggfetch as **not a binary-footprint win** versus an aligned reqwest/Rustls fixture. This plan is about maintenance ownership and native error ergonomics, not size marketing.

## Scope guardrails

This plan may add generic native Rust types/methods for request-failure introspection and the minimum private plumbing required to populate them.

It must not:

- add Gregg, EggPool, CodeGG, Synvoid, provider/account, health-check, monitoring, or application-specific types;
- add endpoint-specific status mappings or a reqwest-compatibility error facade;
- change the shape or exhaustiveness of the existing public `Error` enum;
- change existing `Error::kind()` tokens;
- require callers to parse error strings;
- make Python/CLI/HTTPX callers opt into or observe a new error model;
- change retry, redirect, timeout, proxy, TLS, H1/H2/H3, pooling, or body-consumption policy;
- add a new automatic CI job, evidence subsystem, benchmark dashboard, or scheduled qualification;
- claim a binary-size reduction.

## 1. Define an additive native detailed-failure API

Add a small opaque failure wrapper for callers that explicitly request richer native diagnostics. A representative shape is:

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetworkFailureKind {
    Dns,
    ConnectionRefused,
    Connect,
}

pub struct RequestFailure {
    error: Error,
    network_failure: Option<NetworkFailureKind>,
}
```

The exact naming may be adjusted to match existing repository conventions, but the semantic boundary is required:

- the new classification enum may be `#[non_exhaustive]` from birth so future generic categories can be added without repeating the existing `Error` constraint;
- the wrapper fields stay private;
- callers can borrow the original `Error`, consume the wrapper back into the original `Error`, and inspect optional network classification;
- `RequestFailure::Debug`/`Display` must preserve existing redaction guarantees and must not dump opaque source chains or secrets;
- the wrapper must implement `std::error::Error` only if its source relationship is clear and does not expose more sensitive text than the existing `Error` path.

Minimum useful accessors:

```rust
impl RequestFailure {
    pub fn error(&self) -> &Error;
    pub fn into_error(self) -> Error;
    pub fn network_failure_kind(&self) -> Option<NetworkFailureKind>;
    pub fn is_timeout(&self) -> bool;
    pub fn timeout_phase(&self) -> Option<TimeoutPhase>;
}
```

`is_timeout()` should be documented precisely. At minimum it must recognize ordinary `Error::Timeout`; if it also treats `TransportIoTimeout` or caller-dialer timeout as a timeout, that broader behavior must be explicit and covered by tests. `timeout_phase()` must never invent a `TimeoutPhase` for error families that do not carry one.

Expose opt-in send entry points for the ordinary high-level Rust request path, for example:

```rust
impl Client {
    pub async fn send_detailed(&self, request: Request)
        -> std::result::Result<Response, RequestFailure>;
}

impl RequestBuilder {
    pub async fn send_detailed(self)
        -> std::result::Result<Response, RequestFailure>;
}
```

Use names consistent with the final API review, but do not replace or change `send()`.

Acceptance:

- [x] Existing `Client::send` and `RequestBuilder::send` signatures are unchanged.
- [x] Existing `Error` variants, field shapes, exhaustiveness, `kind()` values and ordinary display behavior are unchanged.
- [x] The detailed API can always recover the original `Error` losslessly with `into_error()`.
- [x] The new classification enum is generic and evolvable from its first release.
- [x] No application/downstream terminology appears in the public API.

## 2. Keep diagnostics private to opted-in requests

Do not add diagnostic state to public `TransportHints` fields: it is a public struct and changing its construction contract would create avoidable downstream breakage.

Instead, carry an optional crate-private failure context through private request state. `Request` already owns internal state that is decomposed through `RequestParts`; extend that private path rather than widening the public transport-hint contract.

A suitable model is an optional `Arc` to a tiny request-local context that exists only for the detailed send path. Ordinary `send()` should pass/retain `None` and should not allocate an `Arc`, mutex, map, or diagnostics buffer.

The context needs only the terminal structured classification required by `RequestFailure`. Prefer a small synchronization primitive or mutex-protected `Option` over a general event log. Do not duplicate `TraceObserver` or `TransportMetrics`.

The context must survive the existing private request reconstruction paths so a detailed request remains detailed across:

- retry preparation;
- same-origin redirects;
- route selection;
- safe H3 fallback when applicable.

However, classification is about the **terminal request failure**, not every transient failure. A failed attempt that is retried successfully must not leak into a successful result. A failed H3 discovery route followed by successful H2/H1 fallback must not leave a stale failure. If a later terminal attempt fails, the later cause wins.

Acceptance:

- [x] Ordinary requests pay no diagnostics allocation/storage cost.
- [x] Detailed requests carry at most bounded O(1) failure metadata.
- [x] Retry/redirect/fallback cannot report stale earlier-attempt classification as the terminal failure.
- [x] Existing tracing and transport metrics remain separate; no second lifecycle/event subsystem is created.

## 3. Capture structured provenance before legacy error collapse

Introduce one crate-private connection-failure classification vocabulary at the transport boundary. Do not expose transport-library types in the public API.

### Standard Hyper connector path

Before `map_send_error` converts a `hyper_util::client::legacy::Error` into `Error::Connect(String)`, inspect the typed source chain while it still exists. Classify only evidence that can be established structurally.

At minimum:

- `std::io::ErrorKind::ConnectionRefused` -> `NetworkFailureKind::ConnectionRefused`;
- a positively identified resolver/DNS failure -> `NetworkFailureKind::Dns`;
- another error for which Hyper reports `is_connect()` -> `NetworkFailureKind::Connect`.

Use a bounded source-chain walk and guard against pathological/cyclic custom source chains. Do not recurse without a bound.

### Direct/resolved/SNI connector path

The direct connector currently formats resolver/TCP errors directly into `Error::Connect(String)`. Replace that **internal creation seam**, not the public `Error`, with a crate-private structured connector error that carries:

- a private connect-failure category;
- a safe human-readable message capable of reproducing the existing public `Error::Connect` text closely enough to avoid a user-visible regression;
- the underlying `std::io::Error` as a source when available.

Let the common mapping boundary read that private category for detailed requests and then continue producing the same public `Error::Connect(String)` for both ordinary and detailed requests. The private structured type must not escape `eggfetch-core`.

This also applies to address fallback: if several candidate addresses fail, use a deterministic terminal classification. A sensible rule is:

1. DNS failure only when resolution itself failed before any address was attempted;
2. `ConnectionRefused` when every attempted compatible address failed with refusal;
3. generic `Connect` for mixed or other connect failures.

Do not label a local bind/socket-option/configuration failure as DNS or refusal merely because a later message contains similar text.

### Custom dialer

Reuse `DialErrorKind` where it truthfully maps. Do not reinterpret authentication/rejection failures as DNS/refusal. The existing `custom_transport_error()` access remains valid and unchanged.

### Proxy, UDS and HTTP/3 routes

Classify only where existing structured evidence supports the claim. It is acceptable for `network_failure_kind()` to return a generic `Connect` or `None` rather than fabricate a DNS/refusal subtype for a route whose current abstraction does not expose that distinction.

Document route coverage. This API is an evidence-backed classifier, not a promise that every transport can report every subtype.

Acceptance:

- [x] No classifier searches `Display`, `Debug`, localized OS text, Hyper text, or prefix/suffix strings.
- [x] DNS/refusal is derived from typed/private provenance before public error collapse.
- [x] Existing public `Error` returned by ordinary `send()` remains compatible.
- [x] Direct-connector address fallback has deterministic, tested classification rules.
- [x] Routes without sufficient evidence degrade to generic/unknown rather than guessing.

## 4. Centralize cross-platform resolver/refusal detection

Keep the platform-specific detection logic narrow and private. Prefer `std::io::ErrorKind` where sufficient. If resolver APIs surface platform-specific raw codes, recognize only documented codes that can be tested on the supported platforms.

Do not use broad rules such as “all `Other` errors during connect are DNS.” That would misclassify unreachable routes, local interface failures, TLS-adjacent failures, and socket setup problems.

Unit-test the classifier using synthetic `std::io::Error` values and private connector errors so routine tests do not depend on public DNS, external network access, resolver configuration, or localized messages.

Where a platform does not expose enough structured information to prove DNS failure, return generic `Connect`. Accuracy is more important than maximizing subtype coverage.

Acceptance:

- [x] Linux, macOS and Windows logic is explicit where platform raw codes are needed.
- [x] No test depends on internet reachability or a public resolver.
- [x] Unknown resolver/connect states fail closed to generic classification.

## 5. Preserve timeout semantics exactly

Do not alter `Timeout`, `TimeoutPhase`, total-deadline behavior, per-chunk read/write timeout behavior, custom-dialer timeout semantics, or transport-I/O inactivity policy.

The new detailed wrapper may expose convenience timeout inspection, but it must be derived from the error that the request already returned. In particular:

- scalar `Timeout::from_secs` continues to configure pool/connect/write/read without implicitly adding `total`;
- explicit `total` remains the request-wide wall-clock cap;
- `TransportIoTimeout` remains distinct from ordinary request-phase timeout;
- no retry behavior changes merely because a caller uses `send_detailed()`.

Add regression tests showing `send()` and `send_detailed()` return equivalent underlying timeout errors for identical requests/configuration.

## 6. Clarify the existing response-size limit; do not add another one

Update Rustdoc and the Rust guide/reference so `max_decoded_body_size` is described truthfully:

- it caps the bytes made available to the caller after decoding when compression is used;
- it also caps ordinary unencoded/identity buffered and streaming response bodies;
- a wire `Content-Length` can be used by callers as an early metadata check, but the streaming/body limit remains authoritative when the header is absent, incorrect, or describes encoded bytes;
- exceeding the configured limit yields `Error::DecodedBodyTooLarge` without requiring the caller to maintain a second manual accumulation loop.

Do not rename the existing method in this plan. Do not add a second `max_response_body_size` setting that aliases the same mechanism unless a separate API review establishes a real semantic distinction.

Where existing tests already prove the identity/unencoded path, reference them in the implementation record. Add a focused regression only if the current suite does not make the invariant explicit.

Acceptance:

- [x] Public documentation no longer implies the limit applies only to compressed/decompressed bodies.
- [x] No duplicate body-limit implementation is added.
- [x] Existing `DecodedBodyTooLarge` semantics remain unchanged.

## 7. Public API and regression tests

Add focused tests at the lowest useful layer plus a small number of external-style integration tests.

Required coverage:

1. detailed success returns exactly the ordinary response semantics and no failure object;
2. detailed failure can be converted back to the same public `Error` category as ordinary `send()`;
3. request timeout is reported as timeout without changing `TimeoutPhase`;
4. local loopback connection refusal is classified as refusal where the platform exposes it deterministically;
5. private/synthetic resolver failure is classified as DNS without external DNS access;
6. another connect failure degrades to generic `Connect`;
7. direct/resolved connector classification follows the all-refused/mixed-failure rule;
8. caller `Dialer` errors retain the existing `DialError` source and are not overclassified;
9. a retry that later succeeds returns success and no stale failure;
10. a fallback route that later succeeds returns success and no stale failure;
11. existing `Error::kind()` assertions remain unchanged;
12. old `send()` does not allocate/create the detailed failure context (prove structurally or with a narrow test hook rather than a benchmark).

Avoid network-flaky DNS integration tests. Use loopback/local fixtures and synthetic private errors.

## 8. Documentation

Update at least:

- `docs/reference/errors.md` — distinguish stable coarse `Error::kind()` from opt-in detailed request-failure classification and document route coverage/fallback behavior;
- `docs/rust/guide.md` — include one concise native example showing how an embedded client can map timeout/DNS/refused/generic failures without matching strings;
- relevant `ClientBuilder::max_decoded_body_size` and `RequestBuilder::max_decoded_body_size` Rustdoc;
- any architecture/error-boundary document that currently claims all connection errors are represented only by the public `Error` taxonomy.

Keep the README concise. Add the detailed API there only if it materially improves the ordinary Rust quickstart; otherwise link to the Rust guide.

Do not document the motivating Gregg integration as a supported adapter. At most, the implementation/plan record may say that health checkers, monitors, service clients, agents, and other embedders motivated the generic capability.

## 9. Compatibility and validation

Because this touches core request/error plumbing, validate more broadly than a documentation-only change, but do not create new CI infrastructure.

During implementation run focused tests for each modified module. Before closure run:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Follow the repository's existing compatibility tooling inside `extended`; do not add a new matrix or workflow. If the live HTTPX/HTTPX2 compatibility records require an exact executable SHA after this core change, perform the existing one-time requalification/update on the final executable commit, then keep any descendant closure edits documentation/profile-only.

Also inspect:

```sh
cargo tree -p eggfetch-core
cargo tree -p eggfetch-core -e features
cargo tree -p eggfetch-core -d
```

The new API should not introduce a dependency. In particular, do not add a diagnostics/event crate, error-reporting framework, resolver crate, or synchronization dependency for this feature unless the standard library/current dependency graph genuinely cannot express the bounded state.

Run the MSRV check through the existing extended tooling. Do not raise the Rust 1.80 floor as a side effect of this plan.

## 10. Closure record

When implementation is complete, amend this plan with:

- final executable SHA;
- exact public names chosen for the detailed failure API;
- route-coverage table for DNS/refusal/generic classification;
- validation results and any truthful optional skips;
- dependency-tree delta (expected: no new dependency);
- compatibility-profile SHA update if required by the current live compatibility process;
- confirmation that body-limit work was documentation/test clarification only.

Do not turn this plan into a permanent evidence subsystem. Once closed, it becomes a historical implementation record under the normal plan-index rules.

## Closure record

- Final executable SHA: `e10c4efdff6af9a73a89d015584df15fa6e2900c` (`feat(core): add native request failure introspection`).
- Public API: `NetworkFailureKind`, `RequestFailure`, `Client::send_detailed`, and `RequestBuilder::send_detailed`; the wrapper exposes `error()`, `into_error()`, `network_failure_kind()`, `is_timeout()`, and `timeout_phase()`.
- The existing `Error` enum, `Error::kind()`, ordinary send methods, and binding/facade paths are unchanged. Detailed requests alone allocate the private bounded context.

Route coverage is evidence-backed and intentionally conservative:

| Route | DNS | `ConnectionRefused` | generic `Connect` | Notes |
|---|---|---|---|---|
| Standard Hyper | unknown/`None` | yes, when a typed refusal is present in the source chain | yes, for Hyper connect failures | The standard resolver’s concrete error is opaque; no message inference is used. The legacy public error kind remains `hyper_client` for this route. |
| Direct/resolved/SNI | yes, on direct resolver failure | yes, only when every attempted compatible address refused | yes, for mixed/other connect and setup failures | Private connector provenance is converted back to the existing public `Error::Connect(String)`. |
| Custom `Dialer` | `None` | `None` | `None` | Existing `DialError` and `CustomTransport` source semantics are preserved; caller errors are not reinterpreted. |
| Proxy/UDS/HTTP/3 | `None` | `None` | `None` | No current abstraction exposes sufficient typed evidence at the common boundary. |

Validation on the frozen executable tree:

- Focused native request-failure tests: 4/4; direct connector classifier tests: 9/9; custom-dialer provenance regression: passed.
- `cargo check -p eggfetch-core --all-features --locked`, focused all-target clippy with `-D warnings`, `cargo fmt --all -- --check`, doc tests, and documentation example/link checks: passed.
- `./scripts/check.sh`: passed (Rust, Python behavior 542/542, HTTPX smoke 133/133, and Rust Node tests; Node JS artifact surface skipped because the optional artifact is not built).
- `./scripts/check.sh extended`: passed; full compatibility 1,870/1,870, with the existing optional skips for the unbuilt Node artifact, Rust 1.80/Cargo resolution, and absent downstream artifact manifest.
- `./scripts/check.sh package`: passed, including crate dry-run, wheel smoke, and package-content validation.
- Three exact-SHA full compatibility runs: 1,870/1,870 each, in 245.62s, 247.55s, and 250.15s; 26 known non-failing warnings per run.
- Remote `CI` run `34903075007` passed on closure head `032f56d933a1d27c595c4730aa9f906083ab3d36` in 8m48s; its only annotation was the existing GitHub Actions Node.js 20 deprecation notice.
- Dependency trees (`cargo tree -p eggfetch-core`, feature tree, and duplicate tree) show no new dependency. MSRV was exercised through the extended gate; its documented local toolchain limitation remains an optional skip.
- The active HTTPX 0.28.1 and HTTPX2 2.12.0 profile bindings are renewed to this SHA on 2026-09-14. The subsequent closure commit is documentation/profile/plan-only.
- `max_decoded_body_size` work was documentation clarification only; existing identity-body coverage (`response_body_unencoded_size_limit_is_enforced`) and `DecodedBodyTooLarge` behavior remain authoritative, with no second limiter.

## Non-goals

- no Gregg migration code;
- no downstream-specific facade;
- no replacement of `Error`;
- no change to existing `Error` enum exhaustiveness;
- no string-based classifier;
- no public resolver abstraction;
- no new retry policy;
- no new timeout policy;
- no new body-buffering/body-limit mechanism;
- no proxy/H3 feature expansion;
- no binary-size claim;
- no release/publish automation.

## Exit criteria

- [x] Native Rust callers can opt into structured terminal request-failure metadata without parsing strings.
- [x] DNS and connection refusal are reported only when supported by structured evidence.
- [x] Existing public `Error` enum shape, `kind()` values, and ordinary send APIs remain compatible.
- [x] Existing non-detailed callers pay no diagnostics allocation/state cost.
- [x] Retry/redirect/fallback cannot leak stale failure classifications.
- [x] Direct-connector failure provenance is structured internally before conversion to the legacy public error.
- [x] Body-size documentation accurately describes the already-existing unencoded/decoded cap semantics.
- [x] No downstream-specific code or dependency is added.
- [x] Tier 1, extended, package, MSRV, and applicable existing compatibility checks pass or record only already-supported truthful optional skips.
- [x] The plan contains final closure evidence and `plans/README.md` points to the current status.
