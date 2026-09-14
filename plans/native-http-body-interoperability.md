# Native HTTP Body Interoperability

Planning baseline: `45c08e0e7587eb1e8f713d49e6f7902478c27a35` (`main`, 2026-09-14; eggfetch 0.1.4)
Parent program: `plans/native-http-body-and-tls-extensibility-program.md`
Status: planned; ready for implementation handoff

## Objective

Add a narrow Rust-native interoperability surface for `http_body::Body` so embedding applications can hand an existing HTTP request body to eggfetch and receive a frame-preserving HTTP response body while eggfetch continues to own destination routing, connection establishment, TLS, pooling, transport lifecycle controls, timeouts and protocol negotiation.

The new surface must be additive. It must not replace the existing high-level `Request`, `RequestBuilder`, `RequestBody`, `Response`, `ResponseBody`, Python, CLI or HTTPX-facing APIs, and it must not turn eggfetch into a reverse-proxy framework.

## Current-state findings

On the planning baseline:

- `RequestBody` is a public enum with `Empty`, `Bytes` and `Stream { BoxBytesStream, length }` variants.
- `RequestBody::into_http_body()` already converts the high-level model into eggfetch's internal `UnsyncBoxBody<Bytes, BoxError>` used by Hyper.
- `RequestBody::Stream` converts every byte chunk to `Frame::data`, so request trailers or other frame types cannot cross the public boundary.
- `HyperRequestBody` is already an internal boxed `http_body::Body`, meaning the transport engine itself is closer to the desired abstraction than the public request body model.
- Hyper H1/H2 responses arrive as `http::Response<hyper::body::Incoming>`.
- `finish_hyper_response()` immediately converts `Incoming` through `wrap_incoming()` into `BoxBytesStream` for the application-oriented `Response`.
- `wrap_incoming()` preserves data, copies trailers into `SharedTrailers`, and currently treats a non-data/non-trailer frame as end-of-stream.
- `ResponseBody` is also a public enum and therefore should not gain a new frame-body variant merely for this work.
- Existing pool permits are attached to high-level response streams and are released on EOF/error/drop. The new body form must retain the same lifecycle property.

The underlying `http-body` 1.x API uses `Body::poll_frame` specifically so DATA, trailers and future frame types can be represented without reducing HTTP to a stream of byte chunks. eggfetch already depends on `http-body` and `http-body-util`, so this work should require no new runtime dependency.

## Core design decision: add a sibling native execution surface

Do **not** add variants to `RequestBody` or `ResponseBody`.

Instead, add a transport-oriented sibling execution surface whose exact naming can be chosen during implementation. The semantic shape should be approximately:

```rust
pub struct NativeResponseBody { /* private */ }

impl http_body::Body for NativeResponseBody {
    type Data = bytes::Bytes;
    type Error = eggfetch_core::Error;
    // poll_frame delegates through eggfetch lifecycle wrappers
}

pub struct NativeRequestOptions {
    // only transport/request-lifecycle controls that are meaningful here
}

impl Client {
    pub async fn execute_http_body<B>(
        &self,
        request: http::Request<B>,
        options: NativeRequestOptions,
    ) -> Result<http::Response<NativeResponseBody>>
    where
        B: http_body::Body<Data = bytes::Bytes> + Send + 'static,
        B::Error: std::error::Error + Send + Sync + 'static;
}
```

Names are intentionally non-normative. The required semantics are normative:

- the request URI represents the logical HTTP(S) destination and must be validated as such;
- caller body frames pass through to the Hyper request body without flattening to `Stream<Bytes>`;
- the response body remains an eggfetch-owned `http_body::Body` rather than exposing `hyper::body::Incoming` directly;
- existing high-level clients keep using the existing `Response` and body helpers;
- the new method is clearly documented as the lower-level native Rust transport/body surface, not the preferred API for ordinary application requests.

If returning a plain `http::Response<NativeResponseBody>` cannot preserve an existing capability such as upgrade metadata cleanly, a small eggfetch-owned response wrapper may be used, provided it can losslessly expose/consume the underlying `http::Response` and does not reproduce the existing high-level response API. Do not publish Hyper response types to solve this.

## 1. Define an eggfetch-owned frame-preserving response body

Add a public opaque body type in `eggfetch-core` that implements `http_body::Body<Data = Bytes>`.

Requirements:

- internal representation is private and may wrap `hyper::body::Incoming`, a boxed generic frame body, or existing proxy/H3 adapters;
- public `Error` is eggfetch's structured error type or another eggfetch-owned error that preserves useful source classification; do not expose `hyper::Error` as the stable API contract;
- `poll_frame()` forwards DATA frames without copying payload bytes;
- trailer frames are returned as trailer frames, not converted into data or hidden behind a side-channel on this native surface;
- future/unknown frame types must be forwarded where the `http-body` API permits. They must never be interpreted as EOF merely because eggfetch's high-level byte API does not consume them;
- `is_end_stream()` and `size_hint()` should delegate conservatively and accurately where possible;
- body drop/EOF/error must release the same eggfetch pool permit / logical in-flight lease attached to the request;
- established-transport physical connection permits remain owned by the connection/Hyper pool as today and must not be released early just because one response body is dropped when the connection remains pooled;
- cancellation remains drop-driven and must not spawn a detached drain task solely to make the API appear reusable;
- `Debug` is bounded and contains no body data.

The type should be useful independently of Synvoid and should appear in native Rust documentation as an interop primitive.

## 2. Accept arbitrary request `Body` values without byte-stream flattening

The native execution method should accept `B: http_body::Body<Data = Bytes>` and erase it internally to the existing `HyperRequestBody` shape (or a compatible internal alias) using `http-body-util`.

Requirements:

- preserve request DATA and trailer frames;
- preserve backpressure: poll the caller body only when the underlying transport polls it;
- preserve cancellation: dropping the request future or connection must drop the caller body;
- map caller body errors into eggfetch/Hyper without string-only loss where the current error architecture allows a source chain;
- do not buffer in order to discover a content length;
- honor the caller's explicit `Content-Length` / transfer semantics subject to ordinary HTTP validity rules; do not fabricate length from `size_hint()` unless the existing transport already does so safely;
- do not create a replay clone, body factory or seek abstraction in this plan.

A native body is one-shot unless a future API explicitly says otherwise.

## 3. Keep logical request policy intentionally narrow

This API exists for callers that already own HTTP-level policy and need eggfetch's transport/TLS/pool machinery. It should not silently run the entire high-level application-client transformation stack.

Initial semantic contract:

### Apply / inherit

The native execution path should reuse:

- client-level connector and route selection;
- logical URL authority and destination scheme validation;
- HTTP version policy;
- destination TLS configuration;
- custom dialer when configured;
- physical connection admission;
- established transport I/O inactivity timeout;
- Hyper stale-idle retry setting (`retry_canceled_requests`);
- Hyper connection pooling and idle configuration;
- client-level proxy routing only if the selected proxy route is implemented for generic bodies without flattening frames;
- transport metrics that are truly transport-level;
- request total/connect/read/write phase controls that can be represented without rebuilding the high-level pipeline.

### Do not apply implicitly

The native body API should not automatically apply:

- cookies;
- application authentication;
- redirect following;
- eggfetch logical `RetryPolicy`;
- automatic response decompression;
- decoded-body size/ratio limits;
- JSON/form/multipart body construction;
- compatibility-facade behavior.

Those policies either require body replay/transformation or intentionally change wire headers/body bytes. The frame-preserving surface should expose wire HTTP semantics and leave such policy to the caller.

Document this distinction explicitly. The existing high-level `Client::request()` path remains the application-client API for users who want those features.

## 4. Reuse current route selection instead of introducing a second client stack

The implementation should refactor at the lowest useful shared point so high-level and native-body dispatch use the same physical transport clients.

Preferred architecture:

```text
high-level Request/RequestBody
    -> high-level policy (auth/cookies/retry/redirect/decode)
    -> erase body to HyperRequestBody
                         \
                          -> shared single-attempt route dispatch
                         /
native http::Request<B>
    -> validate logical URI + native transport options
    -> erase B to HyperRequestBody

shared route dispatch
    -> standard/direct/resolved/SNI/custom-dialer/UDS/SOCKS/proxy as applicable
    -> raw/frame response

high-level caller -> adapt frames to existing ResponseBody + SharedTrailers
native caller     -> NativeResponseBody directly
```

Do not duplicate connector construction, pool configuration, route precedence, TLS setup or lifecycle wrappers in a `native_*` transport tree.

The existing `finish_hyper_response()` is a likely refactoring seam: split "Hyper response/upgrade acquisition" from "convert response body to high-level byte stream" so both surfaces share header/trace/upgrade correctness while choosing different body adapters.

## 5. Audit `wrap_incoming()` forward compatibility

The current high-level adapter should remain byte-oriented, but it must not accidentally terminate on an unknown future frame.

Correct semantics for the high-level adapter are:

- DATA -> yield bytes;
- trailers -> store trailers and continue/finish according to Body semantics;
- unknown/non-data/non-trailer frame -> skip or otherwise handle without interpreting it as EOF, then continue polling;
- body error -> surface an eggfetch body error;
- actual body EOF -> end the byte stream.

Implement this as an iterative/poll-safe state machine; do not recursively call `poll_frame()` without a bounded strategy that could recurse indefinitely if an implementation produces many ignorable frames.

Add a synthetic Body test that emits an ignorable/future-like frame if the public `Frame` API permits construction; if it does not, structure the adapter so exhaustive assumptions are absent and document the limitation.

## 6. Pool lease, timeout and backpressure lifecycle

The native response body must preserve eggfetch's logical concurrency contract.

Today a `PoolGuard` remains held while a streaming high-level response is in flight. The native body needs an equivalent lease wrapper at the `Body`/frame layer:

- hold the logical lease until EOF, terminal error or body drop;
- release immediately on terminal state rather than waiting for wrapper drop if that matches current high-level behavior;
- never hold an extra permit after converting the native body into the high-level byte adapter;
- cancellation before response headers releases request-scoped state;
- cancellation after headers releases the response lease when the body is dropped;
- H2 multiplexing must continue to distinguish logical request limits from physical connection limits.

Read timeout semantics should apply while polling frames, including while waiting for trailers. If existing read-timeout code is byte-stream-specific, extract a frame/body wrapper rather than teaching the public body to duplicate timer logic.

Transport I/O inactivity timeout remains below Hyper and should continue to reset on real byte progress as already implemented; do not conflate it with response read timeout.

## 7. Route-support matrix

Audit each existing route and either support the native frame path or reject it explicitly before I/O. The preferred outcome is support for all Hyper-based H1/H2 routes because they already converge on `HyperRequestBody` / `Incoming`:

- standard direct;
- specialized direct (resolved target, local address/socket options);
- SNI override;
- custom dialer;
- UDS;
- SOCKS Hyper client.

Built-in HTTP forward-proxy / hand-rolled CONNECT paths need explicit analysis because parts of those implementations use custom stream parsers rather than the common Hyper `Incoming` lifecycle. Do not silently flatten the body to make the feature matrix look complete. Either adapt those parsers to an eggfetch-owned frame body without buffering, or return a documented unsupported-route error on the native interop API in v1.

HTTP/3 is similarly separate. If the existing H3 transport can naturally emit an eggfetch-owned `Body` with DATA/trailers while preserving cancellation, add it. Otherwise fail closed for native frame execution with H3-only policy in v1 and record the bounded difference. Do not expand this plan into H3 graduation or a second QUIC body engine.

The high-level APIs must retain all current routes regardless of native-surface support.

## 8. Upgrade / CONNECT behavior

Audit 101 Switching Protocols and successful CONNECT carefully before selecting the final response type.

Requirements:

- do not regress the existing high-level `NetworkStream::Upgraded` behavior;
- native response headers must not deadlock by attempting to consume `Incoming` for 101 responses;
- if the first native API intentionally excludes upgraded/tunneled responses, reject/document that limitation explicitly and keep the existing high-level path authoritative;
- do not expose `hyper::upgrade::Upgraded` publicly;
- a future native upgrade capability should use the existing eggfetch `UpgradedStream` abstraction, not a new Hyper-specific escape hatch.

## 9. Native request options

Keep any new options type small and composed from existing public concepts where possible.

Likely useful fields are:

- `Timeout` or a narrowly scoped request timeout override;
- existing `TransportHints` (resolved target, SNI/target override, trace) where semantically valid;
- per-request proxy override only if generic-body proxy routing supports it cleanly.

Do not duplicate `ClientBuilder` settings such as pool config, physical connection policy, transport I/O timeout or TLS config in per-request options.

Do not use arbitrary `http::Extensions` as the primary configuration API for eggfetch policy; typed public options are easier to validate, document and preserve across refactors.

## 10. External-style qualification fixture

Add a small fixture under `qualification/` that depends on `eggfetch-core` through the public API exactly as an independent Rust crate would. It must not import Synvoid.

The fixture should implement a generic local gateway-style flow:

1. create an inbound synthetic `http_body::Body` that yields multiple DATA frames and request trailers;
2. pass it to the native eggfetch execution surface;
3. local upstream server records body frames/trailers and sends a streaming response with multiple DATA frames plus trailers;
4. consume the response as `http_body::Body` and prove frame order/content/trailers;
5. repeat with a slow producer/consumer to prove incremental polling rather than eager collection;
6. drop a response early and prove pool/logical permits are not leaked;
7. exercise a custom dialer or resolved target so the fixture proves the native body API composes with the completed embedded transport work.

This is a qualification fixture, not a new routine CI framework.

## 11. Focused deterministic tests

At minimum add tests for:

1. arbitrary request `Body` DATA frames reach a local H1 server in order;
2. request trailers are preserved;
3. request body errors surface and do not trigger fallback/replay;
4. request backpressure is incremental (producer is not drained before transport demand);
5. response DATA frames are preserved without buffering;
6. response trailers are returned as `Frame::trailers`;
7. dropping the native response body releases the logical pool lease;
8. body terminal error releases the lease;
9. read timeout can expire while waiting for the next frame/trailer;
10. transport I/O timeout remains independent of read timeout;
11. H2 response trailers survive the native frame path when `http2` is enabled;
12. custom dialer + native body uses the custom route and logical Host/SNI remains unchanged;
13. resolved target + native body performs no DNS fallback;
14. `retry_canceled_requests(false)` remains effective for strict physical-attempt callers;
15. eggfetch logical `RetryPolicy` and redirect policy are not silently applied by the native API;
16. high-level `bytes_stream()` and `trailers()` behavior remains unchanged;
17. `wrap_incoming()` does not treat an ignorable/future frame as EOF where test construction permits;
18. no new public `RequestBody` / `ResponseBody` enum variant exists.

Use local deterministic servers. No external network is needed for routine tests.

## 12. Documentation

During implementation update at least:

- `docs/architecture/core-body-streaming.md` — distinguish byte-oriented high-level bodies from the native frame-preserving surface;
- `docs/architecture/core-engine.md` — show shared route dispatch rather than a second engine;
- `docs/rust/guide.md` — native `http_body` interop example and explicit policy differences;
- `docs/architecture/feature-flags.md` only if implementation adds a real feature boundary;
- rustdoc for every new native type/method.

Document the intended audience as transport-oriented native embedders, not as a Synvoid/reverse-proxy API.

## Dependency and feature policy

Prefer no new Cargo feature. `http-body`, `http-body-util`, `bytes`, Hyper and futures utilities are already direct dependencies in `eggfetch-core`.

A feature flag is justified only if the new public API would otherwise force a new optional dependency or material code/data cost into users that cannot reference the types. Since the dependencies already exist, a `gateway`, `proxy-server`, `synvoid`, or `raw-body` feature is not justified.

No new runtime dependency should be necessary.

## Required validation

Run focused body/transport tests and existing H1/H2/route/timeout/pool tests, then at minimum:

```sh
cargo check -p eggfetch-core --no-default-features --features http1
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --no-default-features --features http1,http2,tls-rustls
cargo test -p eggfetch-core
./scripts/check.sh
```

Run the external-style fixture manually or through the existing bounded qualification mechanism. Do not add a CI matrix.

Do not renew exact-SHA HTTPX/HTTPX2 compatibility evidence in this child plan. The parent closure plan owns the final executable freeze.

## Acceptance criteria

- [ ] Existing high-level public body enum shapes and behavior are unchanged.
- [ ] Arbitrary `http_body::Body<Data = Bytes>` requests can be dispatched without conversion to `Stream<Bytes>`.
- [ ] Native responses expose an eggfetch-owned `Body<Data = Bytes>` with DATA/trailer fidelity.
- [ ] The public API does not expose Hyper `Incoming`, connector or client types.
- [ ] The implementation shares route/connector/pool/TLS lifecycle code with the existing client.
- [ ] Native bodies are one-shot and are never logically retried/redirected by hidden eggfetch policy.
- [ ] Hyper stale-idle retry remains separately controlled by the existing strict transport setting.
- [ ] Pool leases, cancellation and timeouts are correct at the frame boundary.
- [ ] High-level trailer/streaming behavior remains intact.
- [ ] Unsupported native routes, if any, fail explicitly before I/O and are documented.
- [ ] External-style qualification proves public API usability without downstream source dependencies.

## Non-goals

- no server/listener API;
- no reverse-proxy routing, WAF, cache or header-filter policy;
- no new body replay factory;
- no automatic redirects/retries/decompression on the native frame path;
- no replacement of high-level `Response` with `http::Response`;
- no public Hyper-specific body type;
- no new protocol family;
- no HTTP/3 graduation;
- no binary-size or throughput claim without measurement.