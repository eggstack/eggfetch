# Hardening and Correctness Plan Before Python Bindings

## Objective

Perform a focused hardening pass after Milestones A-E and before Milestone F. The repository now has a real async Rust core with request/response modeling, pooling, timeouts, and streaming foundations. Before exposing these semantics through Python, verify that the Rust core behavior is honest, stable, documented, and sufficiently correct.

This pass should not expand product scope. It should tighten the foundation already present.

The main goal is to prevent accidental API and semantic debt from being frozen into the Python sync/async wrappers.

## Current state assumption

The current `main` branch is expected to have:

- workspace foundation complete
- `eggfetch-core` implemented as the only networking engine
- stub `eggfetch-cli` and `eggfetch-python` crates
- HTTP/1.1 client over hyper/hyper-util
- Rustls HTTPS support
- request/response types
- headers, query parameters, byte bodies
- connection pool policy and metrics
- phase-aware timeout configuration and errors
- response streaming foundation
- request streaming type shape
- local integration tests

This plan assumes Milestones F and G have not yet started.

## Non-goals

Do not implement the Python sync API in this pass.

Do not implement the Python async API in this pass.

Do not implement redirects, cookies, proxies, compression, multipart, retries, HTTP/2, or CLI behavior unless a tiny change is required to fix an existing correctness bug.

Do not add large new dependencies.

Do not rewrite the whole client stack.

## Priority 1: Validate real versus nominal streaming semantics

### Problem

The current streaming foundation appears to expose streaming response bodies, but request streaming may still be nominal rather than truly streaming. If `RequestBody::Stream` is collected into `Full<Bytes>` before sending, then streamed uploads are not backpressure-preserving and can unexpectedly buffer unbounded data.

That is acceptable as an interim limitation only if it is explicit and tested. It is not acceptable if public docs claim true streamed upload support.

### Tasks

Inspect `crates/eggfetch-core/src/body.rs`, `client.rs`, and request sending code.

Determine whether streamed request bodies are sent incrementally through hyper or collected first.

If streams are collected first, choose one of two corrective paths:

1. Preferred: implement true streaming request bodies using an appropriate hyper-compatible body type.
2. Conservative: rename/document the current behavior as buffered stream collection and mark true streamed uploads as not yet implemented.

Do not leave the API claiming chunked transfer or streamed upload behavior if the implementation buffers first.

### Preferred implementation direction

Use a boxed body abstraction compatible with hyper request bodies, such as `http_body_util::combinators::BoxBody` or a small project wrapper around `http_body::Body`.

The request body conversion should preserve:

- empty body
- known-size bytes body
- unknown-size stream body
- known-size stream body if supported
- body error mapping into `Error::Body`

When length is known, set or preserve `Content-Length` correctly.

When length is unknown for HTTP/1.1, allow hyper to use a safe transfer mode rather than collecting the stream.

### Tests

Add tests showing that streamed request bodies are not fully polled before the server starts reading.

Use a controlled stream that increments a counter when polled. The test should prove that `send()` does not eagerly drain all chunks into memory before establishing the request.

Add a large streamed upload test that would be unreasonable to buffer in normal use. Keep the test memory-safe and deterministic.

Add a streamed request cancellation test verifying no pool permit leak.

### Acceptance criteria

Either true streamed uploads work and are tested, or docs and type names honestly state that streamed request bodies are currently collected before send.

No public doc should claim chunked streamed upload semantics unless the implementation actually provides it.

## Priority 2: Tighten response body lifecycle and connection reuse safety

### Problem

Response streaming interacts directly with connection reuse. If a response body is dropped before consumption, the associated connection must not be reused unsafely. If a body times out or errors mid-stream, the pool state must remain consistent.

### Tasks

Audit response body ownership in `Response`, `ResponseBody`, and the hyper body adapter.

Verify behavior for:

- full body consumption
- partial body consumption then drop
- body timeout
- body stream error
- double consumption
- `bytes()` after `bytes_stream()`
- `text()` after partial byte streaming

Ensure the code either drains safely or lets hyper discard/close the connection. Do not rely on optimistic reuse after unread bytes.

### Tests

Add or tighten tests for:

- fully consumed response permits reuse
- partially consumed and dropped response does not corrupt the next response
- timed-out body read does not corrupt the next response
- body stream error maps to a stable error variant
- double-consume returns the intended error
- connection metrics remain sane after body drop and timeout

For local test server support, add endpoints that:

- send chunked data slowly
- send headers then stall
- close mid-body
- send two different bodies over reusable connections so corruption is visible

### Acceptance criteria

Partial body consumption and body errors are explicitly tested.

Connection reuse after abnormal body lifecycle is either safe or deliberately prevented.

Documentation explains the behavior.

## Priority 3: Make timeout phase semantics honest and test-backed

### Problem

The roadmap wanted pool, connect, write, read, and total timeout phases. Hyper may hide some phase boundaries, so the implementation may only precisely support pool and total, with less precise connect/write/read behavior. That is acceptable only if errors and docs are honest.

### Tasks

Audit `timeout.rs`, `client.rs`, and timeout tests.

Create a matrix documenting which phases are actually enforced and where:

```text
pool    precise / enforced around pool acquisition
connect precise or approximate / explain DNS and TLS inclusion
write   precise or approximate / explain current limitation
read    precise or approximate / response headers versus body chunks
total   precise wall-clock cap / scope documented
```

Update docs and tests to match reality.

If read timeouts are not applied per response body chunk after streaming, implement or document the gap. Prefer implementation before Python bindings, because Python streaming APIs will rely on this.

Do not report `TimeoutPhase::Connect`, `Write`, or `Read` unless the code can honestly identify that phase.

### Tests

Required:

- pool timeout under saturated permits
- total timeout over a slow request
- read timeout while waiting for response headers if supported
- read timeout while waiting for next body chunk if supported
- no read timeout when chunks arrive under the threshold
- cancellation during timeout wait does not leak pool permit

If connect/write tests are flaky or not practically enforceable through current hyper integration, document them as implementation limitations rather than pretending full coverage.

### Acceptance criteria

Timeout docs exactly match implementation.

Timeout tests cover every phase that the implementation claims to enforce.

Unsupported or approximate phases are explicitly described as current limitations.

## Priority 4: Validate connection pool limits and metrics against actual hyper behavior

### Problem

eggfetch has an outer pool/permit system plus hyper-util's internal pool. This is a reasonable architecture, but metrics can become misleading if they count permits rather than actual opened/reused sockets.

### Tasks

Audit `pool.rs` and how `Client` acquires/releases permits.

Clarify whether `PoolMetrics` means:

- logical request permits
- active in-flight requests
- actual TCP connections
- hyper idle connections
- waiters

Rename metrics or docs if needed. Avoid implying exact socket counts unless measured at the transport layer.

Verify per-host keying. Include scheme, host, and port as needed. Do not pool-limit `http://example.com:80` and `https://example.com:443` incorrectly under a bare hostname key if that causes wrong behavior.

Check permit release timing with streaming responses. If the pool guard is released when response headers are returned while the body is still streaming, then max connection/request concurrency semantics may be wrong. The guard likely needs to live until body consumption/drop if it represents an in-use connection or request slot.

### Tests

Add tests for:

- max connections or permits under long-lived streaming responses
- same host different ports are limited independently if intended
- same hostname different scheme does not incorrectly share a limit if unintended
- many concurrent cancelled requests do not leave stale waiters
- metrics return to baseline after cancellations and body drops

### Acceptance criteria

Pool metrics are named and documented accurately.

Pool keys include enough origin information for correct behavior.

Pool permit lifetime is correct for streaming responses.

## Priority 5: Verify header and URI correctness before Python API freezes expectations

### Problem

Requests/httpx users rely heavily on header and URL edge-case behavior. Early mistakes here become hard to correct after Python API release.

### Tasks

Audit `Headers` and request builder behavior.

Verify:

- duplicate headers are preserved where semantically required
- `set-cookie` is not collapsed
- invalid header names and values are rejected
- case-insensitive lookup works
- insertion order is not promised unless explicitly provided
- user-supplied `Host` behavior is documented
- `Content-Length` and `Transfer-Encoding` behavior is safe

Audit URL and query handling.

Verify:

- existing query strings are preserved
- appended params percent-encode correctly
- repeated query keys are preserved
- unsupported schemes fail clearly
- username/password in URLs are handled or rejected deliberately
- fragment is not sent on the wire
- default ports and explicit ports are handled consistently

### Tests

Add tests for:

- repeated query keys
- percent-encoded params
- existing query plus appended query
- invalid URL
- unsupported scheme
- duplicate response headers
- duplicate request headers if exposed
- invalid newline in header value
- fragment stripping or preservation only in metadata, not wire request

### Acceptance criteria

Headers and URL behavior are deterministic and documented.

No Python-facing API work starts with ambiguous URL/header semantics.

## Priority 6: CI and local validation visibility

### Problem

Commits claim successful local validation, but workflow runs may not be visible or may not be triggering. Before the project accumulates bindings and packaging complexity, CI should be verifiably active.

### Tasks

Inspect `.github/workflows/ci.yml`.

Verify it triggers on push and pull request to `main`.

Check whether repository Actions are enabled. If no workflow runs appear after pushes, investigate whether the workflow path, organization settings, or branch restrictions are preventing execution.

Add CI jobs only if needed. Keep them lightweight.

Recommended validation commands:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
```

Optional but useful:

```sh
cargo tree -p eggfetch-core
cargo test -p eggfetch-core --all-features -- --nocapture
```

### Acceptance criteria

CI runs are visible on new commits or the repository documents why CI is not currently active.

The README or AGENTS file includes the exact local validation command sequence.

## Priority 7: Dependency and feature audit

### Problem

The dependency tree has grown from the skeletal plan. That is expected, but the project goal remains minimal and auditable.

### Tasks

Review `crates/eggfetch-core/Cargo.toml` and `Cargo.lock`.

For each direct dependency, confirm:

- why it is needed
- whether default features are disabled where appropriate
- whether it should be optional
- whether it belongs in `core`, `cli`, or `python`

Specific checks:

- `dashmap`: verify it is necessary rather than simpler `Mutex<HashMap>`/`RwLock<HashMap>` for the current pool metrics/waiters.
- `futures-util`: verify selected features are minimal.
- `pin-project-lite`: verify it is actually used.
- `rustls-pemfile` and `tokio-rustls`: verify direct dependency is needed and not only transitive through hyper-rustls.
- `hyper-rustls` native roots feature: document portability/security implications.

### Acceptance criteria

Dependency policy docs match actual dependencies.

Unused or premature dependencies are removed.

Optional future capabilities remain feature-gated or absent.

## Priority 8: Public API consistency and pre-Python freeze review

### Problem

The Rust API is already becoming public. Before Python bindings map onto it, check naming, ownership, and mutability for consistency.

### Tasks

Review public exports in `lib.rs`.

Check whether public types should be stable now or remain crate-private until needed.

Review:

- `Client`
- `ClientBuilder`
- `Request`
- `RequestBuilder`
- `Response`
- `RequestBody`
- `ResponseBody`
- `Headers`
- `Timeout`
- `Pool`, `PoolConfig`, `PoolMetrics`
- `Error`

Questions to answer:

- Are mutable response methods necessary and clear?
- Should response body consumption methods live on `Response` or `ResponseBody`?
- Are body consumption errors specific enough?
- Are builder methods returning `Self` consistently?
- Are fallible builder methods storing deferred errors or returning `Result` consistently?
- Should pool internals be public or only metrics/config?

### Acceptance criteria

Public API docs are internally consistent.

No obviously internal type is accidentally exported.

The Rust API remains idiomatic and does not contort itself for future Python compatibility.

## Priority 9: Documentation correction pass

### Problem

Docs have been updated milestone by milestone. They may now contain outdated status language or overclaims.

### Tasks

Review:

- README.md
- AGENTS.md
- CONTRIBUTING.md
- docs/architecture/overview.md
- docs/architecture/dependency-policy.md
- docs/architecture/feature-flags.md
- plans/milestone-e-streaming-foundation.md

Correct:

- claims about true streamed uploads if not implemented
- claims about timeout phase precision if approximate
- claims about connection metrics if they are logical permits rather than sockets
- stale milestone status
- examples using old sync body access if `bytes()` is now async

### Acceptance criteria

Docs match code behavior.

Known limitations are explicit and not buried.

## Implementation order

Recommended sequence:

1. Run current validation locally and record baseline failures.
2. Audit request streaming implementation and fix or document it.
3. Audit pool guard lifetime with streaming responses.
4. Tighten timeout phase docs/tests.
5. Add response lifecycle tests.
6. Add URL/header edge tests.
7. Audit dependency tree and remove unnecessary dependencies.
8. Fix docs and examples.
9. Verify CI visibility.
10. Run full validation.

## Final acceptance criteria

This hardening pass is complete when:

- the repo clearly distinguishes implemented behavior from planned behavior
- request streaming is either truly streaming or explicitly documented as buffered collection
- response body drop, timeout, and partial-consumption behavior is tested
- pool permit lifetime is correct with streaming responses
- timeout phase claims match tests
- header and URL edge cases have coverage
- dependency docs match actual dependencies
- CI is visible or the lack of visible CI is documented
- full local validation passes

Required final commands:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
```

## Handoff note

Do not start Milestone F until this pass is complete. Python sync bindings will freeze blocking behavior, runtime ownership expectations, body consumption semantics, timeout exception mapping, and response caching rules. Any ambiguity in the Rust core should be resolved first.
