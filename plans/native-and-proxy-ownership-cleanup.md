# Native and Proxy Ownership Cleanup

Planning baseline: `b6bcc33aa1ad04dfa22af219b82601d3c1377743`
Parent program: `plans/second-pass-performance-ownership-optimization-program.md`
Prerequisite: `plans/second-pass-performance-benchmark-and-guardrails.md`

## Objective

Finish the remaining move-based ownership cleanup in the native frame-preserving request path and eligible proxy/SOCKS Hyper paths without altering public native APIs, proxy routing, fallback behavior, authentication, timeout classification, or manual wire-framing semantics.

## Part A — consume native http::Request parts after validation

`send_native_http_body()` owns `http::Request<B>`. Keep all validations that need the original URI/reference first, then destructure the request with `into_parts()`.

Move rather than clone:

- method;
- HeaderMap;
- version;
- URI where the no-target path can use it directly;
- body.

Convert the owned HeaderMap to `Headers` via the existing `From<HeaderMap>` implementation.

Add or adapt a private owned form of native URI resolution if useful:

- no target override: return/move the original URI;
- target override: preserve scheme/authority and replace only path/query under the existing smuggling/UTF-8 validation.

Do not change native IDNA/userinfo/port validation or introduce `url::Url` into the native path.

## Part B — preserve native frame semantics

The native path is frame-preserving. Ownership cleanup must retain:

- `http_body::Body<Data = Bytes>` framing;
- trailers;
- streaming errors;
- write/read/total/pool timeout ownership;
- 101 upgrade behavior;
- resolved-target/SNI/direct routing;
- native response HeaderMap semantics.

Add focused tests with duplicate request headers and frame/trailer bodies so a move-based implementation cannot accidentally collapse values or buffer frames.

## Part C — remove SOCKS Hyper request HeaderMap rebuild where safe

`transport::proxy::send_socks_request()` currently rebuilds an `http::Request` by iterating borrowed `Headers`.

Change private proxy plumbing to transfer ownership into the chosen proxy sub-route when possible. For the SOCKS path, use the same owned request builder approach as ordinary H1/H2 dispatch.

Be careful that `send_proxy_request()` branches among SOCKS, forward proxy, CONNECT, and fallback/manual routes. Do not move headers before the branch that needs them is selected.

If ownership cannot be made single-owner without materially complicating typed fallback behavior, optimize only the branch where ownership is unambiguous and document the boundary.

## Part D — move SOCKS/eligible Hyper response headers

The SOCKS response converter currently reads status/version, clones `response.headers()`, then consumes the body.

Use `response.into_parts()` so response HeaderMap and Incoming body move together, matching the already-optimized ordinary direct path.

Inspect any other Hyper-backed proxy converter for the same pattern. Fix only exact equivalents; do not rewrite the manual proxy response parser merely for stylistic consistency.

## Part E — preserve proxy contracts

No ownership optimization may change:

- proxy route selection;
- SOCKS5 vs SOCKS5H DNS behavior;
- proxy authentication/conflicting-auth errors;
- proxy TLS policy and pinned addresses;
- forward-proxy absolute-form request target;
- CONNECT 502/504 typed fallback;
- local SOCKS destination-specific fallback;
- total/connect/proxy-TLS/read/write timeout classification;
- current-request deadline ownership;
- physical connection reuse/cache keys;
- raw obs-text header handling in manual H1 framing.

## Performance acceptance

Use Plan 1 native/proxy benchmarks and existing proxy reuse/route controls.

Required result:

- native 50/200-header request construction removes the whole-map clone and shows a reproducible allocation/time benefit;
- SOCKS request/response ownership removes redundant full-map work without regressing proxy throughput/reuse controls;
- ordinary standard-route controls remain unaffected.

## Acceptance criteria

- [ ] Native request HeaderMap is moved, not cloned, after validation.
- [ ] Native no-target URI can move through private plumbing without changing validation.
- [ ] Native frame/trailer/upgrade/timeout behavior remains unchanged.
- [ ] Eligible SOCKS request construction consumes owned Headers rather than rebuilding the HeaderMap.
- [ ] SOCKS response headers move via `into_parts()` rather than whole-map clone.
- [ ] Manual proxy framing and typed fallback semantics are unchanged.
- [ ] Proxy route-cache identity and current-request deadline invariants remain unchanged.
- [ ] No public API/dependency/feature changes.
- [ ] Focused tests and benchmark evidence are recorded.
- [ ] Tier 1 is green.
