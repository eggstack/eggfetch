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

- [x] Native request HeaderMap is moved, not cloned, after validation.
- [x] Native no-target URI can move through private plumbing without changing validation.
- [x] Native frame/trailer/upgrade/timeout behavior remains unchanged.
- [x] Eligible SOCKS request construction consumes owned Headers rather than rebuilding the HeaderMap.
- [x] SOCKS response headers move via `into_parts()` rather than whole-map clone.
- [x] Manual proxy framing and typed fallback semantics are unchanged.
- [x] Proxy route-cache identity and current-request deadline invariants remain unchanged.
- [x] No public API/dependency/feature changes.
- [x] Focused tests and benchmark evidence are recorded.
- [x] Tier 1 is green.

## Execution record — 2026-09-20

Native dispatch now validates the request before decomposing it and moves the
owned method/version/headers/body into the private Hyper request. The SOCKS
request path consumes owned headers, and the SOCKS response path uses
`Response::into_parts()` so response headers and body move together. Manual
proxy framing, typed CONNECT/SOCKS fallback, route identity, and deadline
ownership were left unchanged.

The native request controls measured owned versus rebuild medians of
approximately 363 vs 625 ns, 1.03 vs 2.53 us, and 3.27 vs 9.44 us at 8/50/200
headers in the initial control run. The full workspace, proxy/native tests,
feature matrix, lifecycle/resource, and compatibility checks passed in the
canonical Tier 1/extended runs. No public API, dependency, or unsafe code was
added.

### Corrective addendum — native URI ownership

The prior record overstated the native URI optimization: the private resolver
still accepted `&http::Uri` and cloned it. It now consumes `http::Uri` by
value, returns it directly on the no-target path, and consumes its parts only
after target validation for an override. Native target, `*`, invalid UTF-8,
duplicate-header, frame/trailer, resolved-target, and routing checks remain
covered without changing the public request API.
