# High-Level H1/H2 Request Ownership Fast Path

Planning baseline: `b6bcc33aa1ad04dfa22af219b82601d3c1377743`
Parent program: `plans/second-pass-performance-ownership-optimization-program.md`
Prerequisite: `plans/second-pass-performance-benchmark-and-guardrails.md`

## Objective

Preserve ownership of prepared request metadata through ordinary high-level H1/H2 transport dispatch so the default request path does not rebuild an already-owned HeaderMap or clone the logical URL solely because private helper signatures borrow those values.

This plan targets the standard route first, then UDS/custom/direct/SNI H1/H2 routes where the same move-based request construction can be used without changing route/fallback semantics.

## Part A — introduce one move-based high-level Hyper request builder

The existing private `build_http_request_owned()` can install an owned HeaderMap directly into an `http::Request`. Reuse or adapt that helper so high-level `RequestBody` construction can consume:

- `http::Method` when no later caller needs it;
- `http::Uri`;
- `http::Version`;
- owned `Headers`;
- owned `RequestBody`.

Do not expose a new public API.

Avoid maintaining parallel builder implementations that can diverge. After migration, the borrowed helper should remain only for code paths that genuinely need shared headers, or be removed if no caller requires it.

Correctness requirements:

- preserve duplicate header values and HeaderMap semantics;
- preserve all current request-size validation before ownership is moved;
- preserve H2 forbidden-header stripping, content-length validation, default user-agent and accept-encoding injection;
- preserve trace observer request method/target values;
- preserve write-timeout wrapping and body replayability behavior.

## Part B — move owned headers through selected ordinary H1/H2 routes

Refactor `send_single_request()` and the H1/H2 route helpers so mutually exclusive route arms consume the prepared `Headers` instead of borrowing and rebuilding them.

Initial required routes:

- Standard;
- UDS;
- Custom dialer;
- Direct/resolved-target;
- SNI direct.

The route match already consumes the body in exactly one arm; headers should follow the same ownership model.

Proxy and H3 are intentionally not required here because their fallback/overlay logic may still need shared inspection. Do not complicate those paths merely to make the ownership pattern uniform; Plan 3 owns their narrow cleanup.

## Part C — remove redundant standard-route Url clones

Refactor private dispatch/finalization ownership so the logical `url::Url` is moved into the successful response construction exactly once where practical.

Specific target:

- avoid cloning `url` in `send_single_request()` solely so `finalize_response()` can read it after route dispatch;
- avoid cloning again inside `send_hyper_request()` before `direct::send_request()`.

Prefer reading the final logical URL from the returned `Response` during finalization/Alt-Svc policy if that preserves all semantics.

Because `finalize_response()` mutates the response, handle Rust borrowing explicitly: gather any URL-derived facts before mutable transformations or use a narrow immutable borrow in the HTTP/3-gated block. Do not duplicate the URL back into a side variable just to satisfy the borrow checker.

## Part D — route and redirect/retry regression proof

The move-based path must not change:

- default redirect-disabled requests;
- redirect-enabled first and subsequent hops;
- retry replay of byte bodies;
- non-replayable streaming-body errors;
- same-origin/cross-origin credential behavior;
- cookie injection/update;
- target/SNI/resolved-target hints;
- logical URL attached to the final response/history;
- H1/H2 protocol selection;
- 101 upgrades and trailers.

Add focused tests that would fail if a moved value were accidentally unavailable to later policy.

## Performance acceptance

Compare to the baseline from Plan 1.

Required evidence:

- request construction/header-heavy microbench shows the HeaderMap rebuild is eliminated and a reproducible improvement exists at medium/large header counts;
- warm tiny request is non-regressed within normal measurement noise;
- no trace/timeout/redirect control regresses materially;
- URL clone removal is recorded as a private ownership win even if loopback latency impact is below noise.

If the owned-header change somehow performs worse reproducibly for ordinary requests, investigate before retaining it.

## Acceptance criteria

- [x] Prepared Headers move into ordinary H1/H2 outgoing requests without whole-map reinsertion.
- [x] Standard route performs no redundant Url clone solely for finalization.
- [x] Standard helper performs no second Url clone solely for response construction.
- [x] Duplicate/multi-value headers are unchanged.
- [x] Request-size, H2 header, Host/target, trace, timeout, retry and redirect tests remain green.
- [x] Public Rust API and ResponseBody shape are unchanged.
- [x] No new dependency or unsafe code.
- [x] Focused benchmark evidence is recorded.
- [x] Tier 1 is green.

## Stop conditions

Stop and split a corrective plan if eliminating a clone requires changing a public type, changing history/response URL semantics, storing borrowed request state across await, or weakening route/fallback behavior.

## Execution record — 2026-09-20

Implemented the ordinary H1/H2 ownership fast path. Prepared `Method`,
`Headers`, and the logical `Url` now move through the private standard-route
helpers; finalization reads the response URL instead of requiring a second
owned URL clone. Duplicate request-header coverage was added and confirms
that repeated values survive unchanged. H3 fallback retains its explicit
clones because that path needs the original request for an alternate
transport.

The request-ownership Criterion control measured owned-header versus
rebuild controls at approximately 360 vs 889 ns, 1.00 vs 3.81 us, and 3.23
vs 13.92 us for 8/50/200 headers. The optimized warm request controls were
approximately 339/879/2,818 ns versus 833/3,853/14,044 ns for the rebuild
path. These are private construction controls; routing, retry, redirect,
trace, timeout, protocol, and response-shape behavior remains covered by the
Tier 1 and extended suites. No public API, dependency, or unsafe code was
added.
