# Total Deadline Across Response Body Lifecycle Corrective

Planning baseline: `60a6e2b384519507e04cf296ffd484388e872e47` (`main`, 2026-09-18; eggfetch-core 0.1.6 plus the post-release lean route/policy feature work)
Status: active — correctness corrective required before downstream native-total consumers rely on the documented lifecycle contract
Scope: make native `Timeout.total` remain an absolute wall-clock deadline through response-body consumption for high-level and frame-preserving native APIs; no public timeout redesign, dependency addition, proxy-cache redesign, footprint program, or downstream-specific policy

## Objective

Make the existing eggfetch-native `Timeout.total` contract true in the part of the request lifecycle that currently escapes it: response-body streaming after response headers have been returned.

The public documentation and type-level contract already define `Timeout.total` as a wall-clock cap across the entire request lifecycle. The transport pipeline correctly shrinks that budget across pool acquisition, retries, redirects, proxy setup, connection establishment, and response-header acquisition. However, once a streaming response is returned to the caller, the absolute total deadline is dropped. Only the per-chunk `Timeout.read` wrapper remains attached to the response body.

This corrective should preserve the current API and timeout taxonomy while making the implementation match the documented contract:

```text
logical request begins
  -> pool / connect / TLS / proxy / request write / redirects / retries
  -> response headers
  -> response body chunks / decompression / trailers
  -> EOF or terminal body error

Timeout.total is one absolute deadline across that lifecycle.
Timeout.read remains a separate inactivity deadline that starts when body
consumption starts and resets after each successful body chunk/frame.
```

A response may still be dropped intentionally before EOF; dropping is not itself a timeout error. The body must nevertheless retain the original absolute total deadline so that a caller who starts or resumes polling after the deadline receives `TimeoutPhase::Total` rather than receiving fresh time.

## Defect statement

The current pipeline has nearly all of the machinery required for the correct behavior, but loses the deadline at response finalization.

### High-level request path

`pipeline/prepare.rs` computes both:

```text
remaining_total: Option<Duration>
deadline: Option<std::time::Instant>
```

after logical pool acquisition. The route dispatch uses `remaining_total` to bound the transport future. Proxy-specific multi-phase work also receives the request-local `deadline`.

After a response is received, `pipeline/mod.rs::send_single_request()` calls `finalize::finalize_response(...)`, but the absolute `deadline` is not forwarded. `finalize_response()` ultimately calls `apply_read_timeout_and_lease()`, which attaches only the per-chunk read timeout plus the logical pool lease.

Consequences:

- a server can return response headers before `Timeout.total` expires and then stall the body indefinitely when `Timeout.read` is unset;
- a server can trickle one chunk inside every read-timeout interval and keep the body alive beyond `Timeout.total`;
- `Response::bytes()`, `text()`, `json()`, `bytes_stream()`, `raw_bytes_stream()`, and trailer completion can all outlive the documented total cap;
- the current high-level total-timeout tests mostly delay response headers and therefore do not detect the post-header gap.

### Native frame-preserving path

`pipeline/mod.rs::send_native_http_body()` likewise computes the request's remaining total budget and bounds the transport future, but constructs `NativeResponseBody` with only `timeout.read`.

Therefore `Client::execute_http_body()` and the delegated `Client::native_service()` expose the same post-header gap.

### Why this belongs in eggfetch

This is not a downstream updater quirk. It is a mismatch between eggfetch's own public native timeout contract and its response-body implementation. Consumers should not need to wrap every body read in an application-level timer to make `Timeout.total` mean what eggfetch documents.

Comparable HTTP-client semantics also treat a configured total/request timeout as spanning body completion rather than stopping at response headers. Preserve eggfetch's existing native contract instead of weakening the documentation to match the defect.

## Existing behavior that must remain intact

Preserve all of the following:

- `Timeout.read` remains an inactivity timeout, not an aggregate duration;
- the read timer starts when the caller first consumes the body and resets after each successful chunk/frame;
- `Timeout.total` is native eggfetch policy only; HTTPX compatibility continues to map connect/read/write/pool and must not synthesize a total timeout;
- retry and redirect loops continue shrinking the original logical total budget rather than restarting it;
- proxy cached clients/connectors never store request-scoped total/deadline state;
- the legacy multi-target proxy fallback may continue using request-local deadline state because it is not reusable cached connector state;
- body-size and decompression limits remain independent resource controls;
- pool leases remain held through a streaming response and release on EOF, terminal error, or drop;
- existing `Error::Timeout { phase, elapsed }` and `Error::kind()` tokens remain unchanged;
- no public API, dependency, MSRV, default-feature, or compatibility-facade change is required.

## Part A — Reproduce the defect before implementation

Add at least one deterministic local regression that is red on the planning baseline for the intended reason.

Use the existing loopback test server support for chunked responses, inter-chunk delays, and post-chunk stalls. Do not add a production hook merely to create a test.

### A1. Immediate headers, body exceeds total

Configure a response that sends headers immediately, then emits body chunks slowly enough that:

- `send().await` succeeds before the total deadline;
- each individual chunk may arrive quickly enough that a read inactivity timeout would not need to fire;
- full body completion exceeds a short injected `Timeout.total`.

Consume the body and require:

```text
Error::Timeout {
    phase: TimeoutPhase::Total,
    ..
}
```

The baseline should incorrectly finish the body or continue waiting beyond the configured total.

### A2. Post-header stall with no read timeout

Send response headers and at least one body chunk, then stall longer than the total budget while `Timeout.read = None`.

The body must terminate with `TimeoutPhase::Total`. This proves total is independently authoritative rather than accidentally relying on the read timeout.

Record the exact baseline-red test name and observed failure in the closure record.

## Part B — Establish one crate-private absolute response deadline

Do not create a second public timeout model.

Reuse the request-local absolute deadline already computed by the preparation path. The response body needs an internal representation of the remaining native total deadline that can survive the handoff from transport completion to caller-driven body polling.

A small crate-private value is appropriate, for example conceptually:

```rust
struct ResponseDeadline {
    deadline: std::time::Instant,
    // optional bookkeeping needed only to preserve the existing
    // Error::Timeout elapsed convention
}
```

Exact naming and placement are implementation details.

### B1. Absolute, never reset

The total deadline is absolute.

It must not:

- start when the first response body chunk is polled;
- reset after a body chunk;
- restart when decoding is selected;
- restart when `bytes()` switches from response metadata to collection;
- restart after a redirect;
- restart after a retry;
- restart when a response is handed to another runtime by an adapter.

### B2. Runtime-safe timer ownership

The existing read-timeout stream deliberately creates its Tokio timer on first body poll because the synchronous Python adapter can receive response headers on one runtime and consume the body on another.

Preserve that property.

Preferred implementation:

1. retain a runtime-independent `std::time::Instant` absolute deadline in response-body state;
2. on the first poll in the consuming runtime:
   - if the absolute deadline is already expired, return `TimeoutPhase::Total` without polling the inner transport;
   - otherwise create/reset the Tokio sleep against that absolute instant in the current runtime;
3. never change the absolute instant after construction.

Do not construct a Tokio `Sleep` during response finalization and carry it across runtime boundaries.

### B3. Deterministic timeout precedence

When both read inactivity and total could expire, use one deterministic rule:

- an already-expired absolute total deadline wins before polling the inner body;
- otherwise, if the inner body is pending, whichever deadline becomes authoritative first should surface;
- if both are observably expired at the same poll boundary, prefer `TimeoutPhase::Total` because the absolute lifecycle deadline is the outer cap.

Do not let a ready chunk received after the absolute total deadline extend the request merely because the current read-timeout stream polls its inner source before checking its timer.

## Part C — Apply the total deadline to every high-level response consumption path

The correction should have one common ownership point rather than duplicating timers in `Response::bytes()`, `text()`, JSON helpers, CLI code, and adapters.

### C1. Response finalization

Expand/rename the current `apply_read_timeout_and_lease()` boundary so response finalization attaches:

- the absolute total deadline when configured;
- the existing read inactivity timeout;
- the existing pool lease.

`finalize_response()` should receive the request-local absolute deadline from `PreparedRequest`.

The total deadline must be route-neutral. H1, H2, proxy, direct/advanced routes, and H3 should not each implement their own response-total policy.

### C2. Final consumer stream boundary

Audit both `ResponseBody::Streaming` and `ResponseBody::EncodedStreaming`.

The final implementation must cover:

- decoded `bytes_stream()`;
- raw `raw_bytes_stream()`;
- buffered collection through `bytes()`;
- `text()`;
- `json()` when the JSON feature is enabled;
- compressed responses after the caller selects decoded mode;
- trailers that require advancing the body to EOF.

Do not assume that wrapping only the encoded transport source is sufficient without testing both raw and decoded paths. Prefer retaining deadline metadata with response-body state and applying the absolute deadline at the final stream boundary used by the chosen consumption mode, or otherwise prove an equivalent design with compressed-response tests.

Avoid parallel collection implementations. If a small internal refactor lets `bytes()` consume the same final stream path as `bytes_stream()`, prefer that over reproducing timeout logic in two places.

### C3. Terminal behavior and lease release

On total timeout:

- yield/return the existing `Error::Timeout { phase: Total, .. }`;
- fuse the body so repeated polling does not repeatedly emit timeout errors;
- release the logical pool lease when the terminal stream/body state releases it;
- leave trailers unavailable if EOF/trailers were not reached;
- do not attempt to continue draining a caller-visible timed-out body in the background.

Dropping the response/body remains ordinary cancellation and should release its lease without manufacturing an error.

## Part D — Apply the same contract to NativeResponseBody

`Client::execute_http_body()` is a first-class public Rust transport seam and must not retain weaker total semantics than the high-level client.

Extend the crate-private `NativeResponseBody` state so its `http_body::Body::poll_frame()` enforces both:

- the existing per-frame `Timeout.read`; and
- the same absolute `Timeout.total` deadline.

Requirements:

- total starts with the request lifecycle, not the first frame poll;
- read still starts at first frame poll;
- total does not reset on DATA or trailer frames;
- read resets after each successful frame;
- total wins when already expired;
- EOF/error/timeout releases the logical pool lease;
- `NativeHttpService` inherits the corrected behavior by delegation, without a second implementation.

Do not expose Hyper `Incoming`, Tokio timers, or the internal response-deadline type publicly.

## Part E — Preserve redirect and retry aggregate deadlines

Do not redesign the retry or redirect engines.

They already shrink `Timeout.total` across logical attempts/hops. The correction should carry the final hop/attempt's absolute deadline into the returned body rather than assigning a fresh total duration after headers.

### E1. Redirect proof

Add a deterministic test where:

1. the first hop consumes a measurable portion of the total budget;
2. the redirect reaches a final response before the deadline;
3. the final body trickles or stalls;
4. body consumption crosses the original logical deadline.

Require `TimeoutPhase::Total` near the original logical deadline, not one full fresh `total` after the final response headers.

### E2. Retry proof

Where the existing local retry fixture can do so without disproportionate complexity, add the analogous proof:

1. first logical attempt fails in a retryable way after consuming part of the total;
2. the retry returns response headers;
3. final response-body completion would exceed the original total.

The body must terminate under the remaining original budget.

If a deterministic retry-body fixture would materially duplicate existing retry infrastructure, record why and rely on the existing retry shrink tests plus the final-attempt body-deadline unit/integration proof. Do not add a production test seam just for this case.

## Part F — Preserve body-size controls; do not add a duplicate API

No new body-size API is required for this corrective.

Current `ClientBuilder::max_decoded_body_size()` and `RequestBuilder::max_decoded_body_size()` are already authoritative stream-level limits for ordinary identity and decoded compressed bodies. They remain effective when `Content-Length` is absent, incorrect, or describes encoded bytes.

Re-run the existing body-cap tests and add a focused unknown-length/chunked case only if current coverage does not already prove it.

Document in the closure/handoff note that downstream consumers needing bounded small metadata should apply `max_decoded_body_size` at request/client policy rather than buffering first and checking `bytes.len()` afterward.

Do not:

- add `Response::bytes_limited()` solely for this downstream;
- add an Eggsact-specific metadata helper;
- make a declared `Content-Length` the authoritative resource bound.

## Part G — Required regression matrix

Use deterministic loopback tests. No public internet is required.

### G1. High-level total body deadline

Required:

- headers arrive before total, body completes after total -> `Total`;
- first chunk arrives, subsequent body stalls past total with no read timeout -> `Total`;
- chunks continuously arrive within read timeout but aggregate duration exceeds total -> `Total`;
- no total configured -> existing streaming behavior unchanged.

### G2. Delayed first poll

Required:

1. `send().await` returns response headers;
2. test waits until the original total deadline has passed without polling the body;
3. first body poll returns `TimeoutPhase::Total` without accepting a ready inner chunk.

This prevents accidentally restarting total at body-consumption time.

### G3. Read-vs-total precedence

Required:

- shorter read inactivity budget -> `TimeoutPhase::Read`;
- shorter absolute total budget -> `TimeoutPhase::Total`;
- trickling data that continually resets read still cannot extend total.

Do not assert millisecond-perfect timing. Assert the phase and use generous upper/lower tolerances appropriate for CI scheduling.

### G4. Consumption modes

Cover at least:

- `bytes()`;
- `bytes_stream()`;
- one `text()` or `json()` buffered convenience path;
- `raw_bytes_stream()` for an encoded response under all-features;
- decoded compressed streaming under all-features.

The intent is one timeout mechanism used by all modes, not five separate timeout implementations.

### G5. Trailers

Use the existing trailer fixtures where practical. If trailer arrival is delayed beyond total after body data, body completion must fail with `Total` and `Response::trailers()` must remain `None`.

Read timeout behavior while waiting for trailers remains unchanged when it is the earlier deadline.

### G6. Pool lease

With the logical per-origin request limit set to one:

1. receive a streaming response;
2. let its body hit `TimeoutPhase::Total`;
3. verify a second same-origin request can acquire the released permit.

Also retain the existing drop and normal-EOF release tests.

### G7. Native frame body

Required for `Client::execute_http_body()`:

- headers succeed, body/frame stream exceeds total -> `Total`;
- delayed first frame poll after total -> `Total`;
- read shorter than total -> `Read`;
- total shorter than read -> `Total`;
- terminal timeout releases the pool lease.

Run the existing `NativeHttpService` tests and add one focused service-level inheritance regression only if delegation is not already structurally obvious from the tests.

### G8. Redirect/retry and route coverage

Required:

- redirect aggregate-deadline regression from Part E;
- lean `standard-http1` profile regression, because it bypasses redirect policy and is a supported small-consumer path;
- default/full `http1` profile;
- proxy focused timeout tests;
- deterministic H3 hardening/Alt-Svc suites because `finalize_response()` is route-neutral and H3 is one caller.

Do not turn HTTP/3 experimental qualification or external interop into a blocker for this H1/H2 correctness fix.

## Part H — Documentation truth pass

Update only documents whose timeout/body description changes or is currently inaccurate.

At minimum review:

- `crates/eggfetch-core/src/timeout.rs`;
- `crates/eggfetch-core/src/response.rs`;
- `crates/eggfetch-core/src/body.rs`;
- `docs/architecture/core-timeout-pool.md`;
- `docs/architecture/core-body-streaming.md`;
- `docs/architecture/core-engine.md`;
- `AGENTS.md`;
- `.skills/rust-development.md`;
- `CHANGELOG.md` for the eventual coordinated release.

Document this distinction precisely:

```text
Read timeout:
  inactivity timeout
  starts when body consumption begins
  resets after each successful chunk/frame

Total timeout:
  absolute request-lifecycle deadline
  starts with the logical request
  includes pool/transport/headers/body/trailers
  never resets
  can already be expired when the caller first polls the body
```

Do not claim that total forces an error into an unpolled response object asynchronously. The contract is that the body retains the absolute deadline and reports `Total` when polled after expiry; ordinary drop remains cancellation.

## Part I — Validation

### I1. Focused development checks

Run the narrowest relevant tests while iterating, then at minimum:

```sh
cargo test -p eggfetch-core --all-features --test timeout_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features --test native_http_body_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features --test native_tower_service_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features --test trailer_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features --test pool_tests -- --test-threads=1
cargo test -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --test lean_route_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features -- --test-threads=1
```

Run the existing deterministic proxy and H3 suites affected by the common finalizer:

```sh
cargo test -p eggfetch-core --all-features --test proxy_tests -- --test-threads=1
cargo test -p eggfetch-core --all-features --test h3_hardening -- --test-threads=1
cargo test -p eggfetch-core --all-features --test h3_alt_svc_discovery -- --test-threads=1
```

If an exact test target name has moved, use the current repository equivalent; do not create duplicate suites merely to preserve this command text.

### I2. External-style native consumer

Because this changes the frame-preserving response-body lifecycle, run:

```sh
cargo run --manifest-path qualification/native-http-body-tls/Cargo.toml
```

No AWS-LC or alternate provider should enter the normal workspace graph; this remains an external-style qualification fixture.

### I3. Repository gate

Before the implementation commit:

```sh
./scripts/check.sh
```

Tests remain single-threaded per repository policy.

### I4. Release qualification

This is executable core behavior and invalidates prior exact-SHA compatibility evidence for the current executable tree. Before publishing the corrective release, freeze one clean executable SHA and run the repository's normal release qualification:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
./scripts/check_security.sh
```

Renew the current exact-SHA HTTPX 0.28.1 and HTTPX2 2.12.0 compatibility records according to the live compatibility policy. Compatibility facades must retain their existing semantics; they do not gain a synthesized `total` timeout.

Do not add a new CI workflow or matrix for this corrective. The checked-in local gates remain authoritative.

## Part J — Coordinated patch release and downstream handoff

The fix must be available as a normal crates.io release before downstream consumers are told to rely on it.

eggfetch uses coordinated workspace versions. Publish through the normal manual release process rather than publishing a one-off core version with inconsistent workspace metadata.

If no intervening coordinated release occurs, the expected corrective version is the next patch after 0.1.6. If another release lands first, use the next appropriate coordinated version; do not hard-code a stale number in downstream plans.

After publication, record:

```text
corrected eggfetch-core version:
executable freeze SHA:
crates.io publication:
Timeout.total post-header/body proof:
native frame-body proof:
HTTPX/HTTPX2 qualification:
known limitations:
downstream consumption note:
```

Downstream applications may then upgrade to the published version and remove any temporary application-side whole-body deadline workaround.

## Files expected to change

Likely production/test scope:

- `crates/eggfetch-core/src/pipeline/prepare.rs` only if deadline bookkeeping needs a small representation adjustment;
- `crates/eggfetch-core/src/pipeline/mod.rs`;
- `crates/eggfetch-core/src/pipeline/finalize.rs`;
- `crates/eggfetch-core/src/body.rs`;
- `crates/eggfetch-core/src/stream/` (preferred owner for a high-level total-deadline stream wrapper);
- `crates/eggfetch-core/src/timeout.rs`;
- `crates/eggfetch-core/tests/timeout_tests.rs`;
- `crates/eggfetch-core/tests/native_http_body_tests.rs`;
- focused trailer/pool/redirect/proxy/H3 tests only where required.

Documentation/closure may touch the files listed in Part H plus the live compatibility ledger/profile metadata required by the repository's normal exact-SHA process.

Unexpected manifest, lockfile, dependency, public Rust API, Python API, FFI, Node, or release-workflow changes are a scope-expansion signal. Stop and justify them before continuing.

## Explicit non-goals

Do not expand this corrective into:

- a new public timeout type or cancellation API;
- a new `Error` variant or `Error::kind()` token;
- an HTTPX total-timeout compatibility feature;
- storing request deadlines in reusable Hyper/proxy route clients or cache keys;
- retry/backoff or redirect policy redesign;
- proxy route-cache redesign;
- a lean-proxy feature or any continuation of the linked-footprint program;
- changing `proxy` feature ownership even though it currently re-enables the compatibility `http1` bundle;
- another decoded/body-size API;
- an Eggsact-specific updater helper;
- a second networking engine or manual socket path;
- HTTP/3 graduation, 0-RTT, WebTransport, MASQUE, or external H3 interop work;
- new dependencies;
- new automatic CI jobs, matrices, evidence schemas, or publication automation.

## Completion criteria

This corrective is complete only when all are true:

- [ ] A deterministic regression is recorded red on the planning baseline for a response whose headers arrive before total but body exceeds total.
- [ ] `Timeout.total` remains an absolute deadline through high-level response-body EOF/error and does not restart on first poll or chunk arrival.
- [ ] A caller that waits until after total before first body poll receives `TimeoutPhase::Total`.
- [ ] Continuous body progress cannot extend `Timeout.total`.
- [ ] `Timeout.read` retains its per-chunk inactivity semantics and phase classification.
- [ ] Deterministic read-vs-total precedence is tested.
- [ ] Buffered and streaming high-level consumption paths share the corrected semantics.
- [ ] Raw and decoded compressed response paths are covered.
- [ ] Trailer completion cannot outlive total without surfacing `Total`.
- [ ] Total-timeout termination releases the logical pool lease.
- [ ] `NativeResponseBody` and `Client::execute_http_body()` enforce the same absolute total deadline.
- [ ] `NativeHttpService` inherits the behavior without a duplicate timeout engine.
- [ ] Redirect final bodies use the remaining original logical deadline rather than receiving a fresh total.
- [ ] Retry aggregate total semantics remain unchanged and are re-proven where practical.
- [ ] Proxy cached clients/connectors remain free of request-total/deadline state.
- [ ] Existing decoded-body limits remain authoritative and no duplicate limit API is added.
- [ ] Lean standard-route, full/default, proxy, and deterministic H3 regressions remain green.
- [ ] No dependency, MSRV, public API, Python/CLI/FFI/Node default, or feature-graph change is introduced unless separately justified.
- [ ] The external native HTTP body/TLS fixture passes.
- [ ] Tier 1 passes before implementation commit.
- [ ] Extended/package/security and exact-SHA HTTPX 0.28.1 / HTTPX2 2.12.0 qualification pass on the final release freeze.
- [ ] Timeout/body documentation describes the absolute-total versus read-inactivity distinction accurately.
- [ ] A coordinated published crates.io version containing the fix is recorded before downstream handoff is called complete.

## Closure record template

Append a closure record before marking this plan complete:

```text
Planning baseline:
Baseline-red regression:
Implementation commit(s):
Executable freeze SHA:
Published coordinated version:

High-level immediate-headers/body-total proof:
Post-first-chunk stall proof:
Continuous-progress aggregate-total proof:
Delayed-first-poll proof:
Read-wins proof:
Total-wins proof:
bytes()/stream/raw/decoded proof:
Trailer deadline proof:
Pool-lease release proof:
Redirect remaining-budget proof:
Retry remaining-budget proof/justified existing evidence:
NativeResponseBody proof:
NativeHttpService proof:

Decoded-body-limit regression:
Lean standard-http1:
Default/full core:
Proxy focused:
H3 deterministic:
Native HTTP body/TLS fixture:

Tier 1:
Extended:
Package:
Security:
HTTPX 0.28.1:
HTTPX2 2.12.0:
MSRV:

Dependency/feature/public-API delta:
Known limitations:
Downstream handoff note:
Documentation-only descendant SHA:
```

Do not mark a gate passed by inference from older qualification evidence. If external/release qualification cannot be performed, leave the plan active with that item explicitly blocked rather than converting missing evidence into closure.

## Exit criterion

The defect is closed when eggfetch's documented native `Timeout.total` once again means one request-lifecycle wall-clock deadline from logical request start through response-body completion, across the high-level and frame-preserving native surfaces, without weakening read-timeout semantics, pooling, redirects/retries, proxy deadline ownership, compatibility behavior, or the recently established lean feature boundaries.
