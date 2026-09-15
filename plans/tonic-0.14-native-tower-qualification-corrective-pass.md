# Tonic 0.14 native Tower qualification corrective pass

Planning baseline: `d112d06c4d0318ab34b53e0a4b6204702507b191` (`main`, 2026-09-15; eggfetch-core 0.1.4)

Status: ready for implementation

## Purpose

Correct the one stale piece of qualification evidence left after the native Tower service adapter landed.

The production adapter itself is not presently known to be defective. `NativeHttpService` and `Client::native_service()` already implement the intended generic `tower_service::Service<http::Request<B>>` seam over `Client::execute_http_body()`, preserve the native frame-preserving body path, and keep Tonic outside the eggfetch-core runtime dependency graph.

The defect is narrower: the standalone external-style qualification fixture under `qualification/native-tower-service/` still pins Tonic 0.12 and proves compatibility against the older request-body surface. The downstream consumer that motivated the re-check, Snip-it, currently uses Tonic 0.14. The closure record therefore overstates the relevance of the fixture to that consumer generation.

This corrective pass must update the qualification evidence to Tonic 0.14.x, preferably the exact current 0.14.6 release, prove the adapter against the request/response bounds used by Tonic 0.14 generated clients, and amend the closure record truthfully. It must not turn into a redesign of eggfetch-core, a Snip-it adapter, or a second transport stack.

## Current facts to preserve

On the planning baseline:

- `eggfetch-core` exports `NativeHttpService` and `Client::native_service()`.
- `NativeHttpService` delegates directly to `Client::execute_http_body()`.
- the adapter remains always-ready; origin-aware pool admission and physical transport backpressure occur in the returned call future.
- `NativeResponseBody` preserves HTTP DATA and trailer frames.
- the native path intentionally omits high-level redirects, logical retries, cookies, application authentication, decompression, and decoded-body limits.
- `eggfetch-core` already depends on `tower-service = "0.3"`; no full Tower dependency was added for the adapter.
- the current standalone fixture uses `tonic = { version = "0.12", default-features = false, features = ["codegen", "transport"] }` and asserts compatibility with the older Tonic body type.
- Tonic 0.14 generated clients use `tonic::body::Body` in their generic `GrpcService` bounds.
- Tonic 0.14 `Grpc::with_origin()` still accepts an arbitrary transport implementing the generic `GrpcService`/Tower service boundary.
- Tonic 0.14 code generation can build clients without the convenience `tonic::transport::Channel` connect implementation; the generic client transport does not intrinsically require Tonic's transport stack.
- Snip-it currently declares Tonic 0.14 and is the concrete downstream generation this qualification is intended to make relevant to.

Do not reinterpret this pass as proof that migrating Snip-it to eggfetch reduces binary size. The existing footprint qualification remains **not a footprint win** and this plan does not change that result.

## Scope firewall

This pass is qualification-first and production-code-averse.

Allowed changes by default:

- `qualification/native-tower-service/Cargo.toml`
- `qualification/native-tower-service/Cargo.lock`
- `qualification/native-tower-service/src/main.rs`
- `qualification/native-tower-service/README.md`
- `plans/native-tower-service-adapter.md` closure text
- `plans/README.md` plan/status index text
- this corrective plan's status/closure section

Production changes under `crates/eggfetch-core/src/`, ordinary core tests, public API, Python/CLI compatibility code, HTTPX facade code, CI workflow structure, release automation, or dependency ownership are **not allowed by default**.

A production change may be considered only if all of the following are true:

1. the Tonic 0.14 fixture fails against the current public adapter for a reproducible type/API incompatibility;
2. the failure is not caused by stale fixture code, an unnecessary Tonic feature, or an incorrect generic assertion;
3. the required correction is protocol-neutral and useful to unrelated Tower/native HTTP consumers;
4. the correction can be additive or otherwise preserve existing API/semantics;
5. the executor records the exact failing compiler/runtime evidence before changing production code.

If those conditions are not all satisfied, stop instead of widening eggfetch-core.

Explicitly prohibited in this pass:

- any `snip-it`, Snip, sync, protobuf-service, provider, repository, or application-specific type in eggfetch;
- adding `tonic`, `prost`, `tonic-build`, `tonic-prost-build`, full `tower`, `tower-http`, or `tower-layer` to eggfetch-core runtime dependencies;
- implementing gRPC metadata, `grpc-status`, retry, deadline, authentication, compression, or protobuf behavior inside eggfetch;
- implementing `tower_service::Service` directly on the high-level `Client` merely to satisfy the fixture;
- adding `Default`/`empty()` to `NativeResponseBody` solely to satisfy an interceptor convenience path;
- broadening native body error bounds solely to silence a fixture without demonstrating a generic need;
- adding Tonic's `channel`, `transport`, or TLS stack to the fixture as the mechanism under qualification;
- code generation/protoc setup merely to prove generic transport compatibility;
- changing the measured footprint conclusion;
- rerunning unrelated HTTP/3 graduation work or opening new compatibility programs.

## Expected end state

The corrective pass is complete when the standalone fixture proves that current `NativeHttpService` satisfies the Tonic 0.14 generic client transport bounds using Tonic's current `tonic::body::Body` request body, without enabling or using Tonic's own Channel/transport stack, and all existing focused/core gates remain green.

The preferred outcome is documentation/fixture-only: **zero production-code changes**.

## Track 1 — capture the baseline before editing

Before making any change, record the exact repository state and current fixture resolution.

Run:

```bash
git rev-parse HEAD
git status --short
cargo metadata --manifest-path qualification/native-tower-service/Cargo.toml --format-version 1 > /tmp/eggfetch-native-tower-before.json
cargo tree --manifest-path qualification/native-tower-service/Cargo.toml -e features
cargo run --locked --manifest-path qualification/native-tower-service/Cargo.toml
```

Expected baseline:

- repository begins clean;
- the fixture resolves Tonic 0.12.x;
- the current fixture compiles/runs;
- the current dependency tree includes Tonic's `transport` feature because the fixture explicitly enables it.

Record this only as historical baseline evidence. Passing the old fixture does not satisfy this corrective pass.

## Track 2 — move the fixture to Tonic 0.14 without Tonic transport ownership

Edit `qualification/native-tower-service/Cargo.toml` first.

Preferred dependency shape:

```toml
tonic = { version = "=0.14.6", default-features = false, features = ["codegen"] }
```

Rationale:

- pin the exact version used for closure evidence rather than allowing a future 0.14 patch to silently change the recorded qualification;
- retain `codegen` because the fixture should mirror the generic bounds emitted for generated clients;
- remove `transport` because the object being qualified is eggfetch's custom Tower/native HTTP transport, not `tonic::transport::Channel`;
- do not enable Tonic TLS or server features;
- keep Tonic isolated to the standalone fixture, outside the eggfetch workspace/runtime dependency graph.

If Tonic 0.14.6 cannot compile the intended generic client assertions with only `codegen`, determine the minimum non-transport feature actually required. Do not reflexively restore `transport`. Document the concrete requirement if an additional feature is unavoidable.

Regenerate only the fixture lockfile:

```bash
cargo update --manifest-path qualification/native-tower-service/Cargo.toml -p tonic --precise 0.14.6
```

Then inspect:

```bash
cargo tree --locked --manifest-path qualification/native-tower-service/Cargo.toml -e features
```

Acceptance for this track:

- Tonic resolves exactly to 0.14.6;
- the fixture does not activate Tonic `channel`, `transport`, server, or TLS features;
- `eggfetch-core` remains the only HTTP/TLS transport implementation being exercised by the fixture;
- no eggfetch workspace manifest changes are required.

## Track 3 — update the compile qualification to the Tonic 0.14 generated-client shape

Update `qualification/native-tower-service/src/main.rs` to assert the modern generic transport contract.

The fixture should use:

- `tonic::body::Body` as the request body type;
- `tonic::client::{Grpc, GrpcService}`;
- the same response-body conditions generated Tonic 0.14 clients rely on: response body DATA is `bytes::Bytes`, response body is `Send + 'static`, and its error can convert into Tonic's generated-client standard error type;
- `Grpc::with_origin(...)` to prove an explicit origin can be attached to the eggfetch service without `Channel`.

A representative shape is:

```rust
use bytes::Bytes;
use eggfetch_core::{Client, NativeHttpService};
use http_body::Body as HttpBody;
use tonic::body::Body as TonicBody;
use tonic::client::{Grpc, GrpcService};
use tonic::codegen::StdError;

fn assert_tonic_014_transport<T>(service: T)
where
    T: GrpcService<TonicBody>,
    T::Error: Into<StdError>,
    T::ResponseBody: HttpBody<Data = Bytes> + Send + 'static,
    <T::ResponseBody as HttpBody>::Error: Into<StdError> + Send,
{
    let _client = Grpc::with_origin(
        service,
        http::Uri::from_static("http://127.0.0.1:50051"),
    );
}

fn main() {
    let service: NativeHttpService = Client::new().native_service();
    assert_tonic_014_transport(service);
    println!("native tower service satisfies tonic 0.14 transport bounds");
}
```

Treat this as guidance, not text that must be copied blindly. Compile against Tonic 0.14.6 and adapt only to the actual public API/bounds.

Important distinctions:

- ordinary generated client construction is the target;
- `with_interceptor` is **not** required acceptance for this pass because Tonic's interceptor helper may impose `ResponseBody: Default`, which is a separate convenience constraint and was explicitly excluded from the original adapter plan;
- do not add `Default` to `NativeResponseBody` merely to make interceptor construction compile;
- do not add protobuf code generation or a checked-in generated service solely for this correction unless the simple generic assertion proves insufficient to represent generated-client bounds.

Acceptance for this track:

```bash
cargo check --locked --manifest-path qualification/native-tower-service/Cargo.toml
cargo run --locked --manifest-path qualification/native-tower-service/Cargo.toml
```

must both pass with Tonic 0.14.6 and no Tonic transport feature.

## Track 4 — prove HTTP/2 and trailer assumptions remain owned by the existing native path

The fixture is primarily a compile/interoperability oracle. Do not duplicate the whole `native_tower_service_tests.rs` suite inside it.

Instead, re-run the existing adapter-specific tests and the native HTTP/2/trailer tests that establish the properties gRPC depends on:

```bash
cargo test -p eggfetch-core --test native_tower_service_tests
cargo test -p eggfetch-core --test native_http_body_tests
cargo test -p eggfetch-core --test trailer_tests
cargo test -p eggfetch-core --test http2_tests --features http2,tls-rustls
```

If repository scripts provide narrower canonical commands for the same tests, use those instead and record them.

Confirm explicitly:

- request and response trailer frames remain preserved;
- response-body errors still meet the generic Tonic conversion bound;
- HTTP/2 is configured through ordinary eggfetch feature/policy ownership;
- no eggfetch logical retry/redirect/auth layer is introduced by the service wrapper;
- no behavior changed merely because the fixture moved from Tonic 0.12 to 0.14.

Do **not** add a gRPC-specific retry test to eggfetch. RPC retry semantics belong to the downstream application/Tonic layer.

## Track 5 — dependency and scope audit

After the fixture passes, verify the correction did not smuggle another transport stack into eggfetch-core.

Run at minimum:

```bash
cargo tree -p eggfetch-core -e features
cargo tree -p eggfetch-core -d
cargo tree --locked --manifest-path qualification/native-tower-service/Cargo.toml -e features
```

Inspect the diff:

```bash
git diff --check
git diff --stat
git diff -- crates/eggfetch-core qualification/native-tower-service plans
```

Required conclusions:

- `crates/eggfetch-core/Cargo.toml` has no new Tonic/Tower-framework dependency;
- production source is unchanged in the preferred path;
- the standalone fixture does not enable Tonic `transport`/`channel`;
- Tonic remains qualification-only;
- the fixture still uses eggfetch `http2` + Rustls features rather than Tonic's transport stack;
- no Snip-it-specific concept entered eggfetch.

If any of those statements are false, the pass is not complete.

## Track 6 — repository regression gates

Because the preferred correction is fixture/docs only, use proportionate validation. Do not manufacture a new full-program qualification campaign merely because an external fixture version changed.

Required gates:

```bash
cargo fmt --all -- --check
cargo test -p eggfetch-core --test native_tower_service_tests
cargo run --locked --manifest-path qualification/native-tower-service/Cargo.toml
```

Then run the repository's existing Tier 1 validation command exactly as documented on the implementation tree.

If the implementation changes production Rust code despite the stop conditions, escalate validation: run the normal extended/package gates required by `docs/verification-policy.md` and treat existing exact-SHA compatibility evidence as invalidated until renewed. Do not claim fixture-only validation is sufficient for a production-code correction.

For a fixture/docs-only correction, HTTPX/HTTPX2 exact-SHA compatibility requalification is not required unless repository policy explicitly classifies qualification fixture changes as invalidating those records. Preserve the existing executable freeze semantics truthfully.

## Track 7 — documentation and closure correction

Once Tonic 0.14.6 qualification passes, amend the existing records without rewriting history.

### `qualification/native-tower-service/README.md`

Update it to state:

- the fixture is pinned to Tonic 0.14.6;
- it tests generic Tonic generated-client transport bounds over `NativeHttpService`;
- Tonic `transport`/`Channel` is intentionally disabled;
- the fixture is external/manual qualification, not a runtime eggfetch dependency or routine compatibility suite;
- interceptor helper compatibility is not implied.

### `plans/native-tower-service-adapter.md`

Add a dated corrective closure note rather than deleting the original 0.12 evidence.

The note should say approximately:

- the original executable adapter freeze remains `490320f6...` unless production code changed;
- a later audit found the external Tonic fixture was still on 0.12 while Snip-it uses 0.14;
- the fixture was corrected to exact Tonic 0.14.6 and to the `tonic::body::Body` generated-client transport shape;
- Tonic transport/Channel remains disabled in the fixture;
- focused adapter/native H2/trailer tests and required repository gates passed;
- no binary-size claim changed;
- list the exact corrective commit SHA and validation commands/results.

Do not rewrite the old 0.12 run as though it had been 0.14 all along.

### `plans/README.md`

Update the native Tower adapter entry so it no longer ambiguously says only that "the external Tonic fixture" passed. State the current qualified Tonic version explicitly and, once complete, mark this corrective plan complete.

### this plan

Replace `Status: ready for implementation` with a concise closure record containing:

- final commit SHA;
- exact Tonic version;
- whether production code changed;
- fixture compile/run result;
- focused test results;
- Tier 1/other required validation result;
- dependency-tree conclusion;
- any documented skips/failures;
- final disposition: closed, or blocked with exact reason.

## Failure handling and stop conditions

### Case A — fixture compiles cleanly on Tonic 0.14.6

This is the expected result.

Do not change production code. Finish tests/docs and close the pass.

### Case B — compile failure is only an obsolete fixture type/import

Update the fixture to the current public Tonic API. Do not change eggfetch-core.

Examples include:

- `BoxBody` -> `tonic::body::Body` evolution;
- generated-client bound spelling changes;
- private/public error alias differences that can be expressed through `tonic::codegen::StdError`.

### Case C — fixture requires Tonic `transport` merely because of a convenience API

Do not enable it. Use the generic client constructor/bounds instead. The purpose is to qualify eggfetch in place of Channel.

### Case D — ordinary generated-client transport requires `NativeResponseBody: Default`

Verify this carefully. If the requirement appears only in `with_interceptor`, it is out of scope and no production change is justified. If ordinary generated-client construction itself requires it, record compiler evidence and stop for a separate generic API decision.

### Case E — Tonic 0.14 request-body error bounds do not satisfy eggfetch's current native body bound

First prove the exact concrete type and conversion relationship. Do not weaken `execute_http_body()` only in the service adapter. If a broader generic bound is genuinely required, it must be designed as a protocol-neutral generalization of the native execution API and should be handled as a separately reviewed production correction.

### Case F — runtime HTTP/2/trailer test fails after no production change

Treat that as either a pre-existing regression or a test/environment issue. Isolate it before modifying the adapter. The fixture version bump itself cannot logically change eggfetch production behavior.

## Acceptance criteria

All of the following are required for closure:

1. `qualification/native-tower-service/Cargo.toml` pins Tonic 0.14.6 (or a documented newer 0.14 patch intentionally selected during execution).
2. Tonic `transport`, `channel`, server, and Tonic TLS features are not enabled by the qualification fixture.
3. `Cargo.lock` reflects the new fixture dependency graph.
4. the fixture uses `tonic::body::Body`/the actual current generated-client request-body shape.
5. `NativeHttpService` satisfies the Tonic 0.14 ordinary generated-client `GrpcService` bounds.
6. `Grpc::with_origin()` can be constructed over the eggfetch service without an eggfetch-specific adapter.
7. the fixture compiles and runs with `--locked`.
8. focused `native_tower_service_tests` pass.
9. native body/trailer/HTTP2 regression coverage required above passes.
10. Tier 1 passes on the final implementation tree.
11. no Tonic dependency is added to eggfetch-core.
12. no full Tower dependency is added to eggfetch-core.
13. no Snip-it/gRPC-specific policy or type is added to eggfetch.
14. no speculative `NativeResponseBody: Default` or body-bound broadening is added.
15. the existing footprint result remains described as **not a footprint win**.
16. historical 0.12 evidence is preserved as historical rather than silently rewritten.
17. closure records name the exact corrective commit and Tonic version.
18. `git diff --check` is clean and the repository is clean after commit.

## Rejection criteria

Reject the implementation if any of the following occurs:

- it adds Tonic or full Tower as an eggfetch-core runtime dependency;
- it re-enables Tonic Channel/transport in the fixture to make qualification easy;
- it introduces gRPC-specific behavior into eggfetch;
- it modifies retry/auth/deadline/metadata semantics to accommodate Snip-it;
- it changes production code without first preserving exact evidence that the generic adapter is actually incompatible with Tonic 0.14;
- it adds an interceptor-only `Default` workaround to `NativeResponseBody`;
- it claims binary-size reduction from this pass;
- it reruns unrelated broad programs and changes unrelated code;
- it marks the pass complete while the fixture still exercises Tonic 0.12 or while the Tonic version is not recorded exactly.

## Suggested commit decomposition

Prefer two small commits after this planning commit:

1. `test(qualification): update native tower fixture to tonic 0.14`
   - `qualification/native-tower-service/Cargo.toml`
   - `qualification/native-tower-service/Cargo.lock`
   - `qualification/native-tower-service/src/main.rs`
   - `qualification/native-tower-service/README.md`

2. `docs: close tonic 0.14 tower qualification correction`
   - `plans/native-tower-service-adapter.md`
   - `plans/README.md`
   - this plan's closure record

Do not split production changes into these commits because production changes are not expected. If a genuine production incompatibility is found, stop and document it before starting a separate implementation commit.

## Handoff summary

This is a narrow evidence-correction pass. The native Tower service implementation is currently considered sound; the stale qualification version is the issue. Update the isolated fixture from Tonic 0.12 to exact Tonic 0.14.6, remove Tonic `transport` ownership from that fixture, assert the modern generated-client `tonic::body::Body` bounds, rerun focused native HTTP/2/trailer/service tests and Tier 1, then amend the closure records without rewriting history.

The desired result is no production-code change at all. Any pressure to add gRPC/Snip-specific APIs, Tonic runtime dependencies, `NativeResponseBody: Default`, relaxed native body bounds, or Tonic Channel is a signal to stop and reassess rather than broaden the corrective pass.