# Implementation Hardening Plan: Streaming, Timeouts, and Pool Lifetimes

## Objective

Implement the next correctness pass after the documentation/test honesty pass. The previous hardening work clarified current limitations: request stream bodies are buffered before send, only pool and total timeouts are independently enforced, and pool connection lifecycle metrics are partly skeletal because hyper owns socket lifecycle visibility.

This pass should convert the most important limitations into stronger implementation behavior before Python sync/async bindings begin. The priority is to prevent weak Rust-core semantics from being frozen into Python API compatibility.

The pass focuses on three areas:

1. True streaming request uploads.
2. Correct pool/request permit lifetime with streaming responses.
3. Enforceable read timeout behavior for response headers and body chunks where feasible.

## Current state summary

The repository currently has:

- async Rust core using hyper/hyper-util/tokio/rustls
- request and response types
- response streaming support
- request stream body type shape
- connection pool permits and pool timeout
- total request timeout
- documented limitation that request streams are buffered into `Full<Bytes>`
- documented limitation that connect/write/read timeout fields are stored but not independently enforced
- documented limitation that some pool metrics are skeletal
- roughly 176 tests after the prior hardening pass

## Non-goals

Do not implement Python bindings in this pass.

Do not implement redirects, cookies, proxies, compression, multipart, retries, HTTP/2, or CLI behavior.

Do not replace hyper/hyper-util wholesale.

Do not add large dependencies unless they directly enable correct streaming body integration.

Do not add public API compatibility shims for requests/httpx yet.

## Design constraints

The Rust core remains the only networking implementation.

The API should remain Rust-idiomatic, not Python-shaped.

Any behavior implemented now should be usable by future Python sync and async layers without semantic reinterpretation.

Streaming behavior must preserve backpressure. Avoid unbounded buffering unless the caller explicitly asks for buffered content.

Timeout errors must not claim phase precision the implementation does not actually have.

Pool limits must remain meaningful while response bodies are still active.

## Priority 1: Implement true streaming request bodies

### Problem

`RequestBody::Stream` currently has a useful API shape but is buffered into a single `Full<Bytes>` before send. That defeats backpressure, risks unbounded memory growth, and is not acceptable to expose through Python as streamed upload behavior.

### Target behavior

A stream request body should be converted into a hyper-compatible body without collecting the entire stream first.

Expected behavior:

- `RequestBody::Empty` sends no body.
- `RequestBody::Bytes` sends a known-size full body.
- `RequestBody::Stream { stream, length }` pipes chunks incrementally.
- If `length` is `Some(n)`, set `Content-Length: n` unless the user already supplied a conflicting length. Decide conflict behavior explicitly.
- If `length` is `None`, do not set `Content-Length`; allow hyper's HTTP/1.1 machinery to choose safe transfer behavior.
- If a stream yields an error, map it to `Error::Body` or a more specific body-stream variant.
- Dropping the request future before completion must drop the stream and release permits.

### Implementation direction

Replace the current concrete hyper request body type with a boxed body abstraction.

Likely direction:

```rust
type BoxReqBody = http_body_util::combinators::BoxBody<bytes::Bytes, crate::Error>;
```

or use an internal wrapper if hyper requires a different error type. If the hyper client type requires a single request body type, make all request bodies convert into the same boxed body.

Investigate these building blocks:

- `http_body_util::Full`
- `http_body_util::Empty`
- `http_body_util::StreamBody`
- `http_body_util::BodyExt::boxed`
- `hyper::body::Frame`

The stream body likely needs to yield `Result<Frame<Bytes>, Error>` rather than raw `Bytes`, depending on exact `http-body-util` APIs. Keep the public `BoxBytesStream` if it is ergonomic, but adapt internally to the hyper body frame stream.

### API adjustments

Keep or add:

```rust
RequestBody::empty()
RequestBody::from_bytes(Bytes)
RequestBody::from_stream(stream, length)
RequestBody::known_len()
RequestBody::is_replayable()
```

Rename misleading methods:

- `into_hyper_body()` should not imply full buffering.
- Prefer `into_http_body()` or `into_request_body()`.

If full buffering remains useful internally for tests or compatibility, make that method explicit:

```rust
async fn collect_into_full_body(self) -> Result<Full<Bytes>>
```

### Header rules

Implement clear `Content-Length` / transfer behavior:

- For empty bodies, no body-specific header should be added unless already provided.
- For byte bodies, set `Content-Length` when absent.
- For known-length stream bodies, set `Content-Length` when absent.
- For unknown-length stream bodies, do not set `Content-Length`.
- If the user supplied `Content-Length`, decide whether to trust it or reject mismatches for known-length bodies.

Recommendation: reject a mismatched user-supplied `Content-Length` for known-size bodies to prevent protocol bugs. For unknown streams, trust user-supplied length only if documented; otherwise avoid supporting it until a counting wrapper exists.

### Tests

Add tests proving real streaming behavior:

- A stream body is not fully polled before the server starts reading.
- A slow stream upload reaches the server chunk by chunk.
- An unknown-length stream upload succeeds.
- A known-length stream upload sets/sends the correct content length.
- A byte body sets correct content length.
- A mismatched content length for byte/known stream errors clearly if that policy is chosen.
- Stream body error maps to body error.
- Cancelling an in-flight streamed upload releases the pool permit.
- A large generated stream uploads without materializing a large buffer.

Use a controlled test stream with counters and channels to prove polling behavior. The server should block on reading and release signals so the test can distinguish incremental streaming from eager collection.

### Documentation updates

Update all docs that currently say request streams are buffered. Replace with true current behavior after implementation.

If there are remaining limitations, document them precisely.

### Acceptance criteria

Request stream bodies are no longer collected into one buffer before send.

At least one test would fail under the old eager-collection implementation.

Content-length behavior is explicit and tested.

Docs no longer understate or overstate upload streaming behavior.

## Priority 2: Tie pool/request permits to the full response body lifecycle

### Problem

The current pool guard may be held only until response headers are received, depending on how `Client::send` is structured. With streaming responses, this can allow concurrency permits to be released while the response body is still active. That weakens pool limits and may allow more in-flight connections/streams than configured.

### Target behavior

If a pool permit represents an in-flight request/connection slot, it must remain held until the response body is fully consumed, explicitly discarded, or dropped.

For buffered responses, this naturally ends when buffering completes.

For streaming responses, the guard must move into the response body or another lifecycle handle that drops only when the body is consumed or dropped.

### Implementation direction

Introduce an internal lifecycle guard carried by response bodies.

Possible shape:

```rust
pub(crate) struct ResponseLease {
    _pool_guard: PoolGuard,
}

pub enum ResponseBody {
    Buffered { bytes: Bytes },
    Streaming { stream: BoxBytesStream, lease: Option<ResponseLease> },
    Consumed,
}
```

However, buffered responses should probably drop the lease once body collection is complete. Streaming responses should retain it.

If `ResponseBody::bytes_stream()` moves the stream out, ensure the lease is retained until the returned stream is dropped. That may require a wrapper stream type rather than returning the raw `BoxBytesStream` directly.

Suggested approach:

- Create `LeasedBytesStream` wrapping the inner stream and an optional lease.
- Its `Drop` releases the lease by normal ownership drop.
- `ResponseBody::bytes_stream()` returns a boxed stream that owns the lease.
- `ResponseBody::bytes()` consumes stream and drops lease at the end.
- On stream error or timeout, lease is still dropped.

### API considerations

Avoid exposing pool guards publicly.

Preserve existing public response body API if possible.

If existing `bytes_stream()` currently borrows or mutates `ResponseBody`, ensure ownership semantics remain clear and compile safely.

### Tests

Add tests for permit lifetime:

- Configure `max_connections_per_host = 1`.
- Start one request that receives headers and then holds a streaming body open.
- Start a second request to the same origin.
- Verify the second request waits until the first body is consumed or dropped.
- Drop the first body without consuming it and verify the second proceeds.
- Fully consume the first body and verify the second proceeds.
- Trigger a body read error and verify the second proceeds after error/drop.
- Trigger a body read timeout if read timeout is implemented and verify the second proceeds.

Also test cross-host behavior:

- A long streaming body from host A should not block host B if per-host limits are configured.

### Metrics clarification

If pool metrics track logical permits rather than sockets, expose and document that clearly.

Consider renaming or adding metrics:

- `permits_in_use`
- `acquisition_waits`
- `acquisition_cancellations`

Avoid exposing or emphasizing `connections_opened/reused/closed` until actual socket lifecycle instrumentation exists.

If changing public metrics is too disruptive, mark skeletal fields deprecated or doc-hidden until implemented.

### Acceptance criteria

Pool/request permits are held through streaming response body lifetime.

Tests prove that open streaming bodies enforce configured per-host limits.

Dropping or consuming the body releases the permit.

Metrics/docs do not imply false socket-level observability.

## Priority 3: Enforce read timeout for response headers and body chunks if feasible

### Problem

Current docs state only pool and total timeouts are individually enforced. That is honest, but Python users will expect read timeout semantics similar to requests/httpx. Implementing per-read timeout before Python bindings will make the compatibility layer substantially cleaner.

### Target behavior

Read timeout should apply to:

- waiting for response headers after request dispatch, if feasible
- waiting for each response body chunk

A body that sends a chunk before the read timeout elapses should continue. A body that stalls longer than the read timeout should fail with `TimeoutPhase::Read`.

### Implementation direction

Response header read may be hard to isolate through hyper-util legacy client. If so, do not force it. Body chunk read timeout is likely more feasible because eggfetch owns the response stream wrapper.

Implement a stream wrapper:

```rust
struct ReadTimeoutStream<S> {
    inner: S,
    timeout: Option<Duration>,
}
```

On each `poll_next`, enforce timeout carefully. A simple `tokio::time::timeout` is easier in async code than in manual `poll_next`, so a wrapper using `async_stream` would be tempting but adds dependency. Prefer a manual state machine or an existing minimal combinator if already available.

Alternative: wrap body chunk retrieval at the method level, e.g. provide an async `next_chunk()` API internally and use timeout there. But Python async iteration will eventually need stream-like behavior, so a stream wrapper is better.

For hyper body adaptation, map elapsed read timeout to:

```rust
Error::Timeout {
    phase: TimeoutPhase::Read,
    elapsed: duration,
}
```

Ensure this error is observable when consuming `bytes()`, `text()`, or `bytes_stream()`.

### Total timeout interaction

If total timeout is enforced around initial send only, document it.

If total timeout should apply across streaming body consumption, that requires carrying a deadline into the response body stream. This is useful but more complex.

Recommended sequence:

1. Implement read timeout per body chunk first.
2. Keep total timeout scoped to request dispatch/initial response unless current code already buffers under total.
3. Document streaming total timeout behavior explicitly.

### Tests

Add tests for:

- delayed body chunk triggers `TimeoutPhase::Read`.
- multiple chunks under read timeout succeed.
- stall after first chunk triggers read timeout on next chunk.
- `Response::bytes().await` returns read timeout for stalled body.
- `bytes_stream()` surfaces read timeout.
- read timeout releases pool lease when the body is dropped after error.

If response-header read timeout is implemented:

- server accepts request but delays response headers beyond read timeout.
- error phase is `Read`.

If header read timeout is not implemented:

- docs state body chunk read timeout is implemented, response-header read timeout remains under total timeout/hyper behavior.

### Acceptance criteria

At minimum, response body chunk read timeout is implemented and tested.

Docs precisely state whether response-header read timeout is independently enforced.

No timeout phase is reported unless the code actually enforces that phase.

## Priority 4: Consider write timeout after true upload streaming

### Problem

Write timeout cannot be meaningful while streamed request bodies are buffered before send. After true upload streaming exists, write timeout may be enforceable around body chunk production or send backpressure, but hyper may still hide actual socket write boundaries.

### Target behavior

Do not overclaim write timeout precision.

If feasible, write timeout should apply to waiting for the next request body chunk from the caller's stream. That is not exactly socket write timeout, but it prevents a stalled upload producer from hanging forever.

### Implementation direction

After request body streaming is implemented, wrap the outgoing body stream with a per-chunk timeout.

Semantics:

- If caller's stream does not yield the next upload chunk within `timeout.write`, return `TimeoutPhase::Write`.
- This does not guarantee a timeout on OS socket write stalls unless hyper exposes that boundary.
- Document the limitation precisely.

### Tests

Add tests for:

- upload stream stalls before first chunk and errors with `TimeoutPhase::Write`.
- upload stream stalls after first chunk and errors with `TimeoutPhase::Write`.
- chunks produced under write timeout upload successfully.

### Acceptance criteria

Either upload-producer write timeout is implemented and documented, or write timeout remains documented as configured but not independently enforced.

Do not block Priority 1-3 completion on socket-level write timeout if hyper does not expose it.

## Priority 5: Make pool origin keys correct

### Problem

Per-host pool limits need clear keying. Bare hostnames can incorrectly conflate different origins: `http://example.com:80`, `https://example.com:443`, and `https://example.com:8443` should not necessarily share limits unless the policy explicitly chooses host-only limits.

### Target behavior

Pool acquisition should key by origin, not just host, unless host-only policy is intentional and documented.

Recommended key:

```text
scheme://host:port
```

Use effective port where no explicit port is provided:

- HTTP default 80
- HTTPS default 443

### Tasks

Audit the current host extraction in `Client::send`.

Introduce an internal `OriginKey` type or helper:

```rust
struct OriginKey {
    scheme: Scheme,
    host: String,
    port: u16,
}
```

It can stringify for map keys if needed.

### Tests

Add tests for:

- same host different ports do not share per-origin permits.
- same host same port shares per-origin permits.
- HTTP and HTTPS same host do not accidentally share if origin-key policy is used.
- invalid/missing host still maps to global permit behavior or errors before acquisition.

### Acceptance criteria

Per-origin pool keying is deterministic and documented.

Tests cover port and scheme distinctions.

## Priority 6: Decide what to do with skeletal socket metrics

### Problem

The current metrics include connection-open/reuse/closed counters that are not wired. Even documented as skeletal, public counters can mislead users and later Python consumers.

### Recommended correction

Before Python bindings, either remove skeletal socket lifecycle counters from the public metrics surface or make them explicitly unstable/test-only.

Options:

1. Remove public skeletal counters now and keep only metrics that are true.
2. Rename the metrics struct to `PoolPermitMetrics` and expose only acquisition/permit counters.
3. Keep socket counters private until a custom connector/pool layer can populate them.

Preferred: expose only truthful metrics. If actual socket lifecycle visibility is not available, do not expose socket lifecycle counters publicly.

### Tests

Update tests to assert truthful metrics only.

Remove tests that merely assert skeletal fields stay zero unless they are intentionally private implementation details.

### Acceptance criteria

Public metrics contain no known-unimplemented counters unless explicitly marked unstable/private.

Docs reflect the final metrics semantics.

## Priority 7: Full validation and docs

### Required local commands

Run:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
```

Also run targeted tests:

```sh
cargo test -p eggfetch-core stream --all-features
cargo test -p eggfetch-core timeout --all-features
cargo test -p eggfetch-core pool --all-features
```

### Documentation updates

Update:

- README.md
- AGENTS.md
- docs/architecture/overview.md
- docs/architecture/dependency-policy.md if dependencies change
- docs/architecture/feature-flags.md if body/timeout behavior affects features
- plans/hardening-correctness-before-python.md with completion notes if desired

Docs must state:

- request streaming is true pipe-through if implemented
- exact timeout phases enforced
- pool permit lifetime with streaming bodies
- metrics semantics
- remaining limitations before Python bindings

## Suggested implementation order

1. Refactor request body type to a boxed hyper-compatible body.
2. Implement true request streaming and update content-length handling.
3. Add upload streaming tests.
4. Move pool guard/lease into streaming response body lifetime.
5. Add pool lifetime tests with long-held response bodies.
6. Implement body chunk read timeout wrapper.
7. Add read timeout tests.
8. Optionally implement upload-producer write timeout.
9. Fix origin-key pool keying.
10. Remove or privatize skeletal socket metrics.
11. Update docs.
12. Run full validation.

## Final acceptance criteria

This pass is complete when:

- `RequestBody::Stream` does not eagerly buffer before network send.
- At least one test proves lazy upload stream polling.
- Streaming response bodies hold pool permits until consumed or dropped.
- A test with `max_connections_per_host = 1` proves a second same-origin request waits while the first response body is still active.
- Response body read timeout is enforced per chunk or explicitly deferred with precise documentation.
- Write timeout is either implemented for upload producer stalls or honestly documented as not independently enforced.
- Pool origin keys include scheme/host/port or docs explicitly justify host-only policy.
- Public metrics no longer expose known-unimplemented socket counters as normal stable metrics.
- Full cargo validation passes.

## Handoff note

Only after this pass should Milestone F begin. Python sync wrappers will need stable decisions for runtime blocking, response body caching, iterator behavior, timeout exception mapping, and upload streaming semantics. These should be settled in the Rust core first.
