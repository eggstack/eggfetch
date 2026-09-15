# Native Tower Service Adapter

Planning baseline: `9ed1d78cbbac80a9bd55481c672909aa76eb8446` (`main`, 2026-09-15; eggfetch-core 0.1.4)
Status: complete

## Closure record (2026-09-15)

Executable freeze: `490320f6e99fbb7916280d6bcdd21660bd74f858`

The implementation adds the small cloneable `NativeHttpService` wrapper and
`Client::native_service()` constructor. It implements
`tower_service::Service<http::Request<B>>` with the existing native body/error
bounds, returns `http::Response<NativeResponseBody>`, stays always ready for
request acceptance, and delegates the call future directly to
`Client::execute_http_body()`. No full Tower, Tonic, middleware, or
downstream-specific API was added to `eggfetch-core`.

Validation on the executable freeze:

- focused native Tower service tests: 7 passed;
- supported feature slices, dependency/tree checks, and core rustdoc: passed;
- external-style Tonic 0.12.3 fixture: compiled and ran successfully;
- Tier 1: passed;
- extended: passed, including the full 1,870-test compatibility suite,
  feature matrix, docs, FFI, lifecycle, resource, soak, and benchmark checks;
- package: passed, including crate dry-run, wheel smoke, and package-content
  validation. The initial package attempt was correctly rejected for a dirty
  tree; the committed rerun passed with an absolute venv interpreter;
- exact-SHA HTTPX 0.28.1/HTTPX2 2.12.0 compatibility evidence was renewed on
  this freeze under the current verification policy. The three existing
  optional local skips remain documented by the extended gate: missing Node
  artifact, unsupported Rust 1.80/Cargo resolution, and absent downstream
  artifact manifest.

The bounded embedded-footprint run at `/tmp/eggfetch-native-tower-footprint`
measured the minimal WebPKI profile at 3,654,432 stripped bytes versus
3,079,840 for aligned reqwest, and the JSON WebPKI profile at 3,780,016
versus 3,223,256. The adapter adds no core dependency and this remains **not a
footprint win**; no binary-size reduction is claimed.

All remaining plan changes after the executable freeze are documentation,
compatibility-profile, and plan-index ledger updates.

## Objective

Expose eggfetch's existing native frame-preserving HTTP execution surface through the standard `tower_service::Service<http::Request<B>>` interface so unrelated Tower-native Rust consumers can reuse eggfetch's HTTP/TLS/pooling/transport engine without writing a local adapter.

This is an ergonomics/interoperability change, not a second transport implementation and not a Tonic/gRPC feature. The implementation must delegate to the existing `Client::execute_http_body()` path and preserve its current semantics exactly.

The target public shape is intentionally small:

```rust
#[derive(Clone, Debug)]
pub struct NativeHttpService {
    // private
}

impl Client {
    pub fn native_service(&self) -> NativeHttpService;
}

impl NativeHttpService {
    pub fn new(client: Client) -> Self;
    pub fn with_options(self, options: NativeRequestOptions) -> Self;
}

impl<B> tower_service::Service<http::Request<B>> for NativeHttpService
where
    B: http_body::Body<Data = bytes::Bytes> + Send + 'static,
    B::Error: std::error::Error + Send + Sync + 'static,
{
    type Response = http::Response<NativeResponseBody>;
    type Error = eggfetch_core::Error;
    // implementation-selected Send future
}
```

Exact naming may be adjusted during implementation if a materially clearer name is found, but the semantic boundary below is normative.

## Why this belongs in eggfetch

On the planning baseline:

- `Client::execute_http_body()` already accepts an arbitrary `http_body::Body<Data = Bytes>` and returns `http::Response<NativeResponseBody>`.
- `NativeResponseBody` preserves DATA and trailer frames and owns the existing pool-lease/read-timeout lifecycle.
- `NativeRequestOptions` already contains the request-scoped transport controls appropriate to this low-level API.
- the native execution path intentionally skips redirects, eggfetch logical retries, cookies, application authentication, response decompression and decoded-body limits.
- `eggfetch-core` already has a direct, unconditional `tower-service = "0.3"` dependency because connector implementations use `tower_service::Service` internally.
- Hyper-util clients and Tonic's client transport abstraction both use the same `tower_service::Service<http::Request<B>>` interoperability boundary.

Therefore the missing capability is only a small public adapter around a mature transport entry point. No new runtime dependency or feature flag is justified.

## Design constraints

The implementation must preserve all of the following:

1. `Client::execute_http_body()` remains the authoritative implementation.
2. The adapter must not duplicate route selection, pool acquisition, TLS setup, body wrapping, timeout logic or error mapping.
3. No `tonic`, `tower`, `tower-layer`, `tower-http`, gRPC, protobuf or downstream project dependency is added to `eggfetch-core`.
4. No Snip-it-, CodeGG-, Synvoid-, Eggpool-, Eggress- or provider-specific type, method, feature flag or policy is added.
5. Existing high-level `RequestBuilder` semantics remain unchanged.
6. Existing native body semantics remain unchanged.
7. No request body buffering or response-body flattening is introduced.
8. DATA/trailer frame preservation remains exactly the responsibility of the existing native body path.
9. The adapter does not create a second retry, redirect, authentication, decompression or timeout policy layer.
10. The adapter does not claim to be a lossless wrapper over every Hyper request extension or upgrade facility.

## 1. Add a narrow `NativeHttpService` type

Preferred location: `crates/eggfetch-core/src/service.rs` with a focused public module or a private module plus root re-export. Do not create a broad `tower` subsystem.

The service owns:

- a cloned `Client`;
- one `NativeRequestOptions` value used for every request sent through that service instance.

Requirements:

- `Clone` must be cheap and share the underlying `Client`/pool state rather than creating a new transport engine;
- `Debug` must remain bounded and must not expose body data, credentials or transport secrets;
- `NativeRequestOptions::default()` is used by `Client::native_service()` and `NativeHttpService::new()`;
- `with_options()` configures service-instance defaults by value;
- do not add interior mutability solely to make options dynamically reconfigurable;
- callers needing different options per request should continue to call `execute_http_body()` directly or use separately configured service clones;
- do not encode eggfetch options in arbitrary `http::Extensions`.

Do not add convenience methods for retries, auth, redirects, decompression, gRPC metadata or Tower middleware. Those belong either to the existing high-level eggfetch API or to the caller's Tower stack.

## 2. Implement `tower_service::Service<http::Request<B>>`

The adapter must accept the same generic body class as `execute_http_body()` and return the same response/error types.

`call()` should do no transport work itself. Its logical behavior should be:

```rust
let client = self.client.clone();
let options = self.options.clone();
Box::pin(async move {
    client.execute_http_body(request, options).await
})
```

A boxed future is acceptable if required to express the `Service` associated future type on stable Rust. Before finalizing, verify there is no simple allocation-free named-future option that avoids duplicating the native execution state machine. Do **not** refactor `execute_http_body()` into a hand-written future or duplicate pipeline logic merely to eliminate one service-adapter allocation.

If the implementation uses one allocation per `call()`, document that fact in code comments only where useful; do not add a micro-feature or unsafe machinery to avoid it.

The service future must be `Send` when the existing body/error bounds permit it.

## 3. Readiness semantics: always ready

`poll_ready()` should return `Poll::Ready(Ok(()))`.

This is intentional and must be documented.

Eggfetch's logical admission is origin-aware. `poll_ready()` receives no request and therefore does not know which origin pool would be acquired. Trying to reserve permits in readiness would either destroy current per-origin semantics or require a second stateful admission model.

Normative meaning:

> Tower readiness means that the adapter can accept a request. Eggfetch's origin-aware logical pool admission, physical connection admission and transport backpressure occur inside the future returned by `call()`.

Consequences to document:

- Tower `LoadShed` wrapped directly around this service will not observe eggfetch pool saturation through readiness;
- callers that want Tower-level global concurrency/load-shed policy may wrap the service in their own Tower layers;
- eggfetch's existing pool/connection limits remain authoritative for eggfetch transport behavior;
- do not try to map an unknown future request to a pool permit in `poll_ready()`.

Add a deterministic readiness test that repeatedly polls a newly created and a cloned service and proves readiness does not mutate or consume eggfetch pool state.

## 4. Preserve native execution semantics exactly

The adapter must delegate to `execute_http_body()` rather than reproducing any part of `send_native_http_body()`.

Verify that service calls preserve:

- absolute HTTP/HTTPS URI validation;
- URL-userinfo rejection;
- HTTP version policy;
- TLS configuration and certificate validation;
- resolved-target and SNI transport hints;
- custom dialer selection;
- UDS behavior where supported;
- logical pool admission;
- physical connection policy;
- connect/write/read/total timeout semantics;
- established transport I/O inactivity policy;
- Hyper canceled-request retry setting;
- frame-preserving request and response bodies;
- response trailer delivery;
- response-body lease release on EOF/error/drop;
- existing fail-closed behavior for unsupported built-in proxy, HTTP/3 and upgrade routes on the native frame API.

Also prove what is intentionally **not** applied:

- eggfetch logical `RetryPolicy`;
- redirect following;
- cookies;
- application auth;
- automatic decompression;
- decoded-body size/ratio limits;
- high-level request construction helpers.

The service is only another invocation shape for the native transport path.

## 5. Keep request-extension semantics bounded

Do not claim that `NativeHttpService` is a fully transparent substitute for `hyper_util::Client`.

The current native execution path reconstructs the wire request from method, URI, version, headers and body; arbitrary caller request extensions are not a guaranteed transport contract. Some Hyper extensions have transport/upgrade semantics that eggfetch intentionally does not expose through the native frame API.

For this plan:

- do not blindly forward all `http::Extensions` into the internal Hyper request;
- do not add special handling for Hyper upgrade/capture extensions;
- do not add an extension-based eggfetch configuration mechanism;
- document that middleware-local extensions remain available to middleware before the request reaches eggfetch, but arbitrary extensions are not guaranteed to reach the physical Hyper request;
- if implementation discovers one extension required by a broad protocol-neutral consumer, record it as a separate follow-up instead of silently widening this plan.

## 6. Do not add the full Tower framework

`eggfetch-core` already depends on `tower-service`; keep that dependency as the entire runtime Tower surface.

Do **not** add runtime dependencies on:

- `tower`;
- `tower-layer`;
- `tower-http`;
- `tonic`;
- any service-erasure helper solely for this adapter.

Do not expose eggfetch-owned wrappers for:

- `ServiceBuilder`;
- `TimeoutLayer`;
- `ConcurrencyLimitLayer`;
- `LoadShedLayer`;
- Tower retry;
- Tower buffer;
- `BoxService` / `BoxCloneService`;
- `MakeService`.

Callers can opt into those crates/layers independently. Keeping only `tower-service` avoids conflating eggfetch's transport-aware timeout/retry/concurrency semantics with generic outer middleware semantics.

## 7. Keep `Client` and the service wrapper semantically distinct

Do not implement `tower_service::Service` directly for the ordinary `Client` in this pass.

`Client` exposes two intentionally different APIs:

- high-level application requests (`RequestBuilder`) with redirects/retries/auth/cookies/decompression;
- low-level native frame execution (`execute_http_body`) with transport-only policy.

A direct `Service` implementation on `Client` would make it less obvious which semantic path `call()` selects. `NativeHttpService` makes that selection explicit and leaves room for future high-level ergonomics without source ambiguity.

## 8. Tonic qualification without Tonic coupling

Tonic is useful as an external interoperability oracle because its `GrpcService` abstraction accepts Tower `Service<http::Request<B>>` transports and gRPC requires HTTP/2/trailer preservation. It must not become an eggfetch runtime dependency.

Add an isolated external-style qualification fixture under a path such as:

`qualification/native-tower-service/`

The fixture should have its own `Cargo.toml`, depend on `eggfetch-core` by path, and may use `tonic` as a fixture-only dependency. Do not add Tonic to `eggfetch-core` dependencies or normal workspace feature ownership merely for this check.

The fixture should prove at minimum:

1. `NativeHttpService` satisfies the relevant Tonic transport/`GrpcService` generic bounds for a Tonic request body;
2. a Tonic client can be constructed with an explicit origin over the eggfetch service without an eggfetch-specific adapter;
3. HTTP/2 is configured through ordinary eggfetch `HttpVersionPolicy`, not a gRPC-specific eggfetch switch;
4. no Tonic type appears in eggfetch public APIs;
5. no special gRPC retry/status/metadata logic is required in eggfetch.

Prefer a compile + local deterministic qualification over code generation or an external service. If a small checked-in fixture can exercise one unary exchange and trailer-delivered gRPC status without `protoc`, that is useful; do not add a code-generation pipeline simply to make this plan look more complete.

This fixture is qualification evidence, not a permanent routine compatibility suite unless later maintenance experience demonstrates that value.

## 9. Do not add `NativeResponseBody: Default` speculatively

Some generated Tonic interceptor helpers may require a default response body, but ordinary transport interoperability does not establish that this belongs in eggfetch.

For this plan:

- first compile the isolated Tonic qualification against the existing `NativeResponseBody` API;
- if the basic client transport works, do not add `Default` merely for an optional helper;
- if an unrelated broad Tower/Tonic interoperability requirement demonstrably needs an empty body, record that as a separate generic API decision;
- any future `Default`/`empty()` addition must define EOF, size hint, timeout and pool-lease semantics explicitly and cannot exist only as a Snip-it workaround.

## 10. Do not loosen request-body error bounds speculatively

The native API currently requires:

```rust
B::Error: std::error::Error + Send + Sync + 'static
```

Some generic clients use the broader `Into<Box<dyn Error + Send + Sync>>` convention. Keep the existing bound in this implementation unless a concrete external-style qualification fails because of it.

If a broader bound is genuinely required:

- treat it as an additive generalization of `execute_http_body()` itself, not a service-only special case;
- preserve source error information;
- add focused compile/runtime coverage;
- document the compatibility rationale in the plan closure record.

Do not broaden public generic bounds solely for aesthetic parity with Hyper-util.

## 11. Focused tests

Add deterministic tests with no external network dependency.

At minimum cover:

1. `Client::native_service()` constructs a cloneable service with default options.
2. `NativeHttpService::new()` and `Client::native_service()` are behaviorally equivalent.
3. `with_options()` propagates an observable native option such as timeout or a transport hint.
4. `poll_ready()` is immediately ready before and after requests and does not acquire pool permits.
5. a simple native DATA request succeeds through the service.
6. request trailers survive the service path.
7. response DATA frames survive without buffering.
8. response trailers survive through `NativeResponseBody`.
9. dropping the response body releases the logical pool lease exactly as direct `execute_http_body()` does.
10. a native body error surfaces as the existing eggfetch error behavior.
11. unsupported native routes remain unsupported through the service rather than falling into a different path.
12. a configured eggfetch logical retry/redirect policy is not applied by the service.
13. cloned service values share the underlying client/pool state.
14. no high-level headers/auth/cookies/decompression are injected solely by using the service adapter.
15. no-default/minimal protocol feature profiles still compile with the public type; unsupported runtime protocol configurations retain the existing `execute_http_body()` error behavior.

Where possible, reuse native-body test helpers instead of creating another local HTTP server harness.

## 12. Documentation

Update only the documentation needed to make the boundary clear:

- `docs/rust/guide.md`: add a short `NativeHttpService` section with a generic Tower service example and readiness semantics;
- native-body interoperability documentation: state that `native_service()` is an ergonomic wrapper over `execute_http_body()`, not a separate policy path;
- rustdoc on `NativeHttpService`, `Client::native_service()`, constructor/options methods and `Service` behavior;
- `README.md` only if the native Rust feature list benefits from a brief mention; do not promote Tower as a new top-level framework dependency;
- `plans/README.md` at closure: record completion and executable SHA in the existing plan ledger style.

Explicitly document:

- no full `tower` dependency;
- always-ready `poll_ready()` semantics;
- pool/transport backpressure occurs inside `call()`'s future;
- native request extensions are not a guaranteed Hyper passthrough contract;
- high-level eggfetch request policy is not applied;
- Tonic/gRPC is an external consumer example, not an eggfetch feature.

## 13. Dependency and footprint checks

This change should add no runtime dependency because `tower-service` already exists in `eggfetch-core`.

Verification must include:

```sh
cargo tree -p eggfetch-core -e features
cargo tree -p eggfetch-core -d
```

Compare against the planning baseline and confirm:

- no `tower`, `tower-layer`, `tower-http` or `tonic` runtime dependency was added;
- no new default Cargo feature was added;
- minimal HTTP/1 and HTTP/2 profiles remain valid;
- any Tonic dependency exists only inside the isolated qualification fixture, if that fixture is implemented;
- the adapter does not materially increase a minimal stripped consumer binary beyond ordinary codegen noise. Measure rather than infer; binary-size reduction is not an acceptance criterion.

If the boxed service future produces a measurable hot-path regression in a focused local benchmark, document it and evaluate whether a simple named-future design is practical. Do not introduce unsafe code, a new dependency or a duplicated request state machine to eliminate a tiny allocation without evidence.

## 14. Validation sequence

Use the repository's existing verification policy rather than creating another CI workflow.

Implementation sequence:

1. add the adapter and focused unit/integration tests;
2. run the focused native-body/service tests;
3. compile the supported feature slices, especially no-default, HTTP/1-only, HTTP/2-only/HTTP2+TLS profiles already represented by repository validation;
4. run the isolated native Tower qualification fixture;
5. run the canonical Tier 1 gate;
6. run extended/package validation as required by `docs/verification-policy.md` for a public Rust API change;
7. inspect dependency/feature diffs and minimal-consumer footprint;
8. freeze one executable SHA after code/tests/fixture stabilize;
9. renew exact-SHA compatibility evidence only to the extent required by the repository's current verification policy for an executable public-core change;
10. make only documentation/plan-ledger changes after the executable freeze unless a defect is found, in which case freeze and qualify again.

Do not add a dedicated Tower CI matrix or continuously test multiple Tonic versions. The public contract is `tower-service` + `http` + `http-body`; the isolated Tonic fixture is supporting evidence for one important consumer class.

## Suggested implementation order for a small model

### Step A — public adapter skeleton

- create the service module;
- define `NativeHttpService { client, options }`;
- derive/implement bounded `Clone` and `Debug`;
- add `NativeHttpService::new()`;
- add `NativeHttpService::with_options()`;
- add `Client::native_service()`;
- re-export the type from `eggfetch_core` root.

Acceptance: `cargo check -p eggfetch-core` passes and rustdoc has no missing-public-doc failure.

### Step B — `Service` implementation

- implement `Service<http::Request<B>>` with the same bounds as `execute_http_body()`;
- make `poll_ready()` unconditionally ready;
- make `call()` delegate once to `execute_http_body()`;
- do not inspect/modify request headers/body/extensions in the adapter.

Acceptance: a direct service call reaches a deterministic local upstream and returns a native response body.

### Step C — semantic regression tests

- add readiness test;
- add options propagation test;
- add frame/trailer test using existing native-body helpers;
- add clone/shared-pool lifecycle test;
- add policy-isolation test proving high-level retry/redirect/auth/decode do not activate.

Acceptance: all new focused tests pass under the relevant HTTP/1 and HTTP/2 feature slices.

### Step D — external-style Tower/Tonic qualification

- create the isolated fixture;
- prove generic Tower service use;
- prove Tonic transport bounds/client construction without an eggfetch-specific adapter;
- if practical without codegen infrastructure, exercise one local H2 unary/trailer flow;
- do not modify eggfetch API just to satisfy optional generated interceptor helpers.

Acceptance: fixture compiles/runs using only public eggfetch exports and contains no patching of eggfetch internals.

### Step E — docs, dependency audit and closure

- update Rust guide/rustdoc;
- run dependency/feature comparison;
- run repository gates required for a public core API addition;
- record executable freeze and closure in this file and `plans/README.md`.

Acceptance: no new runtime dependency/feature, no API regression, all required gates green, exact-SHA evidence current.

## Non-goals

This plan does not:

- add gRPC support to eggfetch;
- add a Tonic adapter type;
- add a Snip-it adapter type;
- replace Tonic's codec/status/metadata behavior;
- add the full Tower framework;
- add Tower middleware implementations;
- expose eggfetch's pool as Tower readiness;
- redesign eggfetch retries/timeouts/concurrency around Tower layers;
- guarantee arbitrary request-extension passthrough;
- add native proxy/H3/upgrade support not already present in `execute_http_body()`;
- add `NativeResponseBody::Default` without independent justification;
- loosen body-error bounds without a demonstrated interoperability failure;
- change the high-level Rust/Python/CLI/HTTPX APIs;
- migrate any downstream repository;
- claim a binary-size reduction.

## Completion criteria

The plan is complete when all of the following are true:

- [x] `eggfetch-core` exports one small cloneable native HTTP service wrapper.
- [x] `Client::native_service()` creates the wrapper with default `NativeRequestOptions`.
- [x] the wrapper implements `tower_service::Service<http::Request<B>>` for the same body class accepted by `execute_http_body()`.
- [x] `Service::Response` is `http::Response<NativeResponseBody>` and `Service::Error` remains eggfetch's public `Error`.
- [x] `poll_ready()` is intentionally always ready and documented as request-acceptance readiness rather than origin-pool availability.
- [x] `call()` delegates to `execute_http_body()` and does not duplicate native dispatch/policy logic.
- [x] request/response DATA frames and trailers remain preserved.
- [x] pool lease, timeout, cancellation and transport lifecycle behavior matches direct native execution.
- [x] high-level redirect/retry/auth/cookie/decompression policy remains absent from the service path.
- [x] no full Tower or Tonic runtime dependency is added to `eggfetch-core`.
- [x] no downstream-specific public API is added.
- [x] arbitrary request extensions are not silently promoted into a new transport contract.
- [x] an isolated external-style fixture proves ordinary Tower use and Tonic-compatible transport bounds without an eggfetch-specific Tonic adapter.
- [x] supported feature slices compile and focused deterministic tests pass.
- [x] dependency/feature and minimal-consumer footprint effects are measured and recorded.
- [x] required Tier 1/extended/package/compatibility validation passes under the repository's current verification policy.
- [x] one executable SHA is frozen after implementation and tests stabilize.
- [x] documentation and `plans/README.md` record the final supported boundary and closure SHA.

## Handoff summary

Implement the smallest useful bridge between two APIs eggfetch already owns/dependencies it already carries: `tower_service::Service` and `Client::execute_http_body()`.

The preferred result is approximately one public adapter type plus tests/docs. If implementation begins creating Tower middleware, Tonic concepts, request-extension policy, service factories, retry layers, or a second transport path, stop and reduce scope. The value of this work is interoperability through a standard trait with near-zero architectural expansion.
