# Second-Pass Performance Ownership Optimization Program

Planning baseline: `b6bcc33aa1ad04dfa22af219b82601d3c1377743` (`main`, 2026-09-20; documentation/evidence head after the completed first performance campaign)
Audit date: 2026-09-20
Predecessor program: `plans/performance-optimization-no-api-regression-program.md`
Normative verification policy: `docs/verification-policy.md`
Primary compatibility contracts: native Rust public API, Python native manifest/typing surface, HTTPX 0.28.1, HTTPX2 2.12.0, C ABI, existing CLI behavior

## Objective

Perform a narrow second-pass optimization campaign focused on ownership continuity across the request/response pipeline and Python adapter boundaries, without changing the established public API surface or protocol semantics.

The first performance campaign is complete and remains historical evidence. It already removed default logical-pool bookkeeping, response-header cloning in the ordinary Hyper converter, trace-only URI formatting, blocking sync-stream backpressure, repeated Python streaming remainder copies, unconditional cookie read-side expiry pruning, and eager buffered Python text decoding. Do not reopen those areas unless this campaign produces independent evidence of a new defect.

This follow-up exists because the post-campaign audit found several remaining places where an owned value is converted back into a borrowed interface and then cloned/rebuilt one layer later. The highest-value targets are request HeaderMap ownership in ordinary H1/H2 dispatch, redundant Url ownership in the standard route, native request decomposition, Python response-header transfer, buffered Python iterator eagerness, and the final full-body copy in async Python `aread()`.

Every executable optimization must either remove an objectively redundant ownership conversion/copy or be supported by comparable before/after evidence. Optional residual tuning must close as "no change justified" if measurement does not show a reproducible benefit.

## Confirmed source findings at planning baseline

### 1. Ordinary high-level H1/H2 dispatch rebuilds an already-owned header map

`prepare_single_request()` owns the final `Headers`, but the standard, UDS, custom, direct, and SNI H1/H2 route helpers accept `&Headers`. `build_hyper_request()` then iterates every header through `http::Request::builder().header(...)`.

The repository already has `build_http_request_owned()`, which moves an owned `HeaderMap` directly into the outgoing request. The normal no-redirect path therefore still pays a complete header-map rebuild after the request pipeline has already established sole ownership.

### 2. Standard-route Url ownership crosses avoidable clone boundaries

`send_single_request()` retains the prepared `url::Url`, clones it into route dispatch, and `send_hyper_request()` clones it again before the response converter takes ownership. Finalization keeps the original mainly for route-neutral metadata/Alt-Svc policy even though the returned core `Response` already owns the final logical URL.

This is private plumbing and can be tightened without changing `Response::url()`, redirect identity, cookies, auth, proxy identity, SNI, or Alt-Svc semantics.

### 3. Native `http::Request<B>` dispatch clones owned request metadata

`send_native_http_body()` receives an owned `http::Request<B>`, validates through references, then clones the method and full HeaderMap before finally consuming the request body. The request can be decomposed with `into_parts()` after validation so method, headers, version, URI, and body continue by ownership.

### 4. Python response conversion clones core HeaderMap state

Buffered and streaming Python response constructors clone the core `HeaderMap` into `PyHeaders`. The buffered constructor receives `&mut Response`, and the streaming constructor owns `Response`; after charset/wire metadata/cookie extraction the ordinary headers can be moved into `PyHeaders`.

The later Python streaming body-state code does not require the core response's ordinary HeaderMap.

### 5. Buffered Python iterators are API-lazy but implementation-eager

`PyResponse::iter_bytes()`, `iter_text()`, and `iter_lines()` materialize every Python bytes/string object into a Vec/List before returning `iter(list)`.

For large buffered bodies this increases peak RSS and delays the first item even though the public methods are iterator-shaped. Private PyO3 iterator objects can yield one chunk/line at a time while preserving signatures and exact chunk/line semantics.

### 6. Async Python `aread()` retains one avoidable full-body Rust copy

The streaming response `aread()` path collects into `BytesMut`, freezes into `Bytes`, stores a cheap cache clone, then converts the full body to `Vec<u8>` for Python conversion. Returning a Python bytes object directly from the async bridge can remove that intermediate full-size Vec while preserving the public return type.

### 7. Residual cookie-write and body-collection tuning is plausible but not yet justified

The new cookie expiry watermark eliminates the common read-side writer scan, but single-cookie mutation still recomputes the minimum expiry by scanning the jar. Streaming body collection also starts without a bounded capacity hint even when safe decoded-length information may be available.

Both are secondary and measurement-gated. Do not increase state complexity or preallocate from untrusted wire metadata merely to produce a microbenchmark win.

## Ordered implementation plans

Execute in this order:

1. `second-pass-performance-benchmark-and-guardrails.md`
   - freeze current post-campaign measurements for the newly identified paths;
   - add request-header ownership, Python first-item/RSS, native request, async aread, and optional mutation/body collection controls;
   - capture public/API baselines before executable changes.

2. `high-level-h1-h2-request-ownership-fast-path.md`
   - preserve ownership of prepared headers through ordinary H1/H2 route dispatch;
   - remove redundant standard-route Url clones where response-owned URL state is sufficient;
   - keep proxy/H3 fallback semantics out of this plan unless ownership can be moved without duplicating state.

3. `native-and-proxy-ownership-cleanup.md`
   - decompose owned native `http::Request<B>` without cloning its HeaderMap;
   - carry the same move-based response/request ownership into SOCKS/eligible proxy Hyper paths where semantics permit;
   - keep manual proxy framing, CONNECT fallback, and error taxonomy unchanged.

4. `python-buffered-response-and-adapter-ownership-optimization.md`
   - move core response headers into Python wrappers after metadata extraction;
   - make buffered Python iterators lazy without changing returned values or exceptions;
   - remove the intermediate full-body Vec from async `aread()` when PyO3 conversion semantics allow it.

5. `benchmark-gated-cookie-and-body-buffer-tuning.md`
   - optimize cookie mutation watermark maintenance only if large-jar mutation evidence justifies it;
   - add a bounded safe body-collection capacity hint only if comparable evidence shows material allocation/RSS benefit;
   - explicitly close either candidate unchanged when evidence is weak.

6. `second-pass-performance-requalification-and-closure.md`
   - freeze one final executable SHA;
   - rerun comparable before/after performance evidence;
   - prove no Rust/Python/C/CLI API drift or HTTPX/HTTPX2 compatibility regression;
   - run repository-required Tier 1, extended, package, security, MSRV, lifecycle/resource, and exact-SHA qualification as required by current policy.

Plans 2 and 3 may be developed independently after Plan 1. Plan 4 can overlap them if touched files do not conflict. Plan 5 is optional-by-evidence and may close with no executable change. Plan 6 is the sole final qualification owner.

## Cross-program invariants

- No public Rust type, method, field, variant shape, feature/default, Python export/signature/property, C ABI symbol/signature, or established CLI surface may be removed, renamed, or semantically broadened.
- `ResponseBody` public variant shapes remain frozen.
- Hyper remains the sole H1/H2 physical connection pool.
- Request/response ownership changes must preserve duplicate headers, header ordering semantics required by current tests, exact target/Host behavior, trace lifecycle events, retry/redirect replayability, and timeout ownership.
- Moving a HeaderMap is permitted only after every caller that still needs to inspect it has completed that inspection.
- URL ownership changes must preserve the logical URL attached to every returned response and history entry.
- Native frame-preserving dispatch must retain `http_body::Body` framing/trailers/upgrades and must not acquire high-level URL policy.
- Python iterator changes must preserve chunk_size validation, byte-for-byte output, Unicode character chunking, line splitting/CRLF behavior, repeated iterator creation, and exception classes.
- Python `aread()` must still return a real Python `bytes` object, retain content caching semantics, and preserve cancellation/close/body-state transitions.
- No unbounded queue, speculative whole-body buffering, unsafe Python buffer sharing, or reservation directly proportional to untrusted encoded `Content-Length` may be introduced.
- No new production dependency is justified by this campaign.
- Routine CI topology remains unchanged.
- HTTP/3 and Node remain experimental and are not graduated by this work.

## Program completion criteria

This program is complete when:

- the new benchmark baseline is recorded before executable changes;
- ordinary high-level H1/H2 dispatch no longer rebuilds an owned prepared HeaderMap solely because a private helper takes `&Headers`;
- default/standard URL ownership has no redundant clone that exists only to feed finalization;
- native request dispatch no longer clones the caller-owned HeaderMap before consuming the request;
- eligible SOCKS/proxy Hyper response ownership no longer clones response headers before body consumption where `into_parts()` is sufficient;
- Python response conversion avoids a complete core HeaderMap clone where ownership can be transferred safely;
- buffered Python `iter_bytes`/`iter_text`/`iter_lines` produce the first item without prebuilding all remaining Python objects;
- async Python `aread()` avoids an unnecessary full-body intermediate Rust allocation if the benchmarked implementation is compatibility-safe;
- optional cookie/body tuning is either supported by reproducible evidence or explicitly rejected;
- representative performance controls show no material regression in ordinary warm requests, streaming, proxy/native, Python buffered/streamed responses, cookies, or resource behavior;
- final qualification records zero new public/API/compatibility drift.

## Non-goals

Do not redesign the request/response API, replace Hyper, introduce custom pooling, rewrite redirect/retry policy, change cookie semantics, make Python response content zero-copy across the Python ABI, redesign proxy framing, graduate HTTP/3/Node, add unsafe code, add a new allocator, or turn timing measurements into routine CI pass/fail thresholds.

## Execution record — implementation and qualification complete 2026-09-20

Plans 1 through 5 were executed in order. The benchmark/guardrail plan froze
the pre-change evidence and added deterministic correctness assertions; the
request, native/proxy, Python adapter, and cookie changes are recorded in the
child plans. The body-capacity candidate was rejected on decoded-length
provenance grounds. Final executable-SHA qualification and remote CI status
are recorded by `second-pass-performance-requalification-and-closure.md`
after the clean corrective commit `bc4800ee9428f0fd11d7d0b914c489b444fe93fc`
and its documentation-only qualification renewal are pushed. The preceding
`1153d40c9a8a3bb380e63add1dee7601469d91f3` freeze remains historical.
