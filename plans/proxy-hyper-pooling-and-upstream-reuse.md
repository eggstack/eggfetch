# Proxy Hyper Pooling and Upstream Reuse

Planning baseline: `025b1a5a6a94b017b3f3b183c3d562e7ee9bcca7` (`main`, 2026-09-15; eggfetch 0.1.4)
Parent program: `plans/post-audit-maintenance-security-and-proxy-modernization-program.md`

## Objective

Reduce eggfetch's custom proxy HTTP maintenance burden and add persistent connection reuse for ordinary HTTP forward-proxy and HTTPS CONNECT routes by moving successful HTTP framing/body handling back under Hyper wherever this preserves existing eggfetch semantics.

The target is **not** “replace all proxy code with hyper-util.” Eggfetch has requirements that generic upstream helpers may not preserve: separate proxy/origin TLS policy, pinned proxy and ultimate-destination routes, phase-aware timeouts, explicit proxy headers/auth, structured CONNECT rejection errors, transport metrics, logical identity preservation, and fail-closed DNS behavior.

The preferred end state is:

- Hyper owns request/response framing and physical connection pooling after a route-specific connector has established the correct transport;
- eggfetch retains only route selection, proxy policy, and the minimum handshake/connector logic needed for semantics that upstream helpers cannot represent;
- the manual full HTTP/1 response parser and custom proxy response body streams are no longer used for successful ordinary origin responses where Hyper can do the job;
- SOCKS remains on its already-persistent Hyper path unless upstream reuse produces a clear net maintenance win without semantic loss.

## Current implementation

The current proxy route has three distinct behaviors:

1. **SOCKS** — uses persistent Hyper clients keyed by a bounded `SocksRouteKey` cache.
2. **HTTP forward proxy** — calls `connect_to_proxy()`, manually writes an absolute-form HTTP request, manually parses the proxy/origin HTTP response, and returns a custom `ProxyResponseStream`. A fresh proxy socket is opened for each request.
3. **HTTPS CONNECT** — calls `connect_to_proxy()`, manually emits/parses CONNECT, performs origin TLS, manually emits the origin request, manually parses the origin response, and returns `TlsProxyResponseStream`. A fresh proxy socket/tunnel is opened for each request.

The manual parser is carefully bounded and has accumulated important compatibility/security behavior, but it duplicates functionality already provided by Hyper on every non-proxy route and makes connection reuse difficult.

## Relevant upstream capability

Current `hyper-util` 0.1.x releases provide primitives that did not exist when many custom proxy paths were originally built:

- `hyper_util::client::legacy::connect::Connected::proxy(true)` marks a connection as an HTTP proxy transport, causing Hyper's HTTP/1 client to emit absolute-form request targets;
- `hyper_util::client::legacy::connect::proxy::Tunnel` can establish HTTP CONNECT through a connector;
- upstream SOCKS4/SOCKS5 connector helpers exist;
- `hyper_util::client::legacy::Client` owns a reusable connection pool;
- newer `client::pool` primitives exist for composable pooling/key mapping if the legacy client's ordinary per-origin pool is insufficient.

These are candidate building blocks, not a mandate. The implementation must first prove feature parity against eggfetch's existing proxy contracts.

## Required implementation

# 1. Build a proxy behavior contract before changing transport ownership

Create a source/test-backed inventory of current proxy semantics for:

### Proxy endpoint connection

- `http://` proxy;
- `https://` proxy;
- native DNS proxy resolution;
- `Proxy::resolved_addresses()` route pinning;
- multi-address fallback;
- proxy connect timeout;
- proxy TLS timeout;
- nodelay/socket behavior;
- proxy transport metrics.

### Forward HTTP

- absolute-form target;
- custom target extension validation;
- Host behavior;
- proxy-only headers;
- Proxy-Authorization;
- duplicate/raw header preservation;
- known-length and chunked streaming request bodies;
- HTTP/1.0 vs HTTP/1.1 behavior;
- explicit rejection of HTTP/2-only forward-proxy policy;
- response reason phrase preservation where exposed;
- response streaming and body-length/truncation behavior.

### CONNECT

- authority-form IPv4/IPv6;
- proxy auth and proxy-only CONNECT headers;
- structured non-200 rejection status/body sanitization;
- pinned ultimate target address while preserving logical Host/SNI identity;
- origin TLS config independent from proxy TLS config;
- origin SNI override;
- H1/H2 ALPN after tunnel establishment;
- phase-aware total/connect/read/write budgets;
- multi-pinned-target retry rules;
- response streaming/cancellation;
- no proxy header/auth leakage inside the tunnel.

### Routing/security

- client proxy vs environment proxy vs request override;
- `without_proxy()`;
- NO_PROXY behavior;
- redirect route re-evaluation;
- SOCKS5 vs SOCKS5H DNS ownership;
- no fallback to DNS where a pinned route promises fail-closed behavior;
- no H3 bypass of proxy rules.

This contract is the acceptance baseline. An upstream helper may replace custom code only if the behavior remains representable or an explicitly approved public behavior change is separately justified.

# 2. Run an upstream-reuse feasibility spike

Before implementing a new custom pool, prototype the relevant current `hyper-util` primitives in focused tests/examples.

## Forward proxy candidate

Prove that a connector that always establishes the selected proxy transport and returns a connection whose `Connection::connected()` reports `Connected::proxy(true)` causes Hyper to:

- emit absolute-form HTTP/1 targets;
- retain request streaming/backpressure;
- parse/stream responses correctly;
- reuse eligible connections on repeated requests;
- drop/replace dead pooled connections correctly.

The connector must be able to represent `http://` and `https://` proxy endpoints. For HTTPS proxies, proxy TLS must use the existing proxy TLS policy rather than the origin TLS config.

## CONNECT candidate

Evaluate `hyper_util::client::legacy::connect::proxy::Tunnel` against eggfetch requirements. Specifically determine whether it can preserve:

- custom proxy headers/auth;
- IPv6 authority formatting;
- pinned ultimate target differing from logical destination;
- rich rejection status/body needed by `Error::ProxyConnectRejected`;
- separate proxy TLS;
- eggfetch timeout phase classification/total deadline;
- metrics/failure provenance.

If upstream `Tunnel` cannot preserve these contracts, do **not** wrap it with enough compensating code to create a more complex stack. Instead implement/retain a narrow eggfetch `Service<Uri>` CONNECT connector that performs the required handshake and returns the established tunnel stream to Hyper.

## Pool primitives

Use the legacy Hyper client's built-in pool if its per-origin keying is sufficient. Do not adopt `hyper-util::client::pool` solely because it is newer.

Composable key remapping is justified only if a concrete required route cannot be safely pooled under legacy-client keying.

Acceptance:

- [ ] A recorded go/no-go decision exists for `Connected::proxy(true)` forward routing.
- [ ] A recorded go/no-go decision exists for upstream `Tunnel`.
- [ ] No custom connection pool is introduced before proving Hyper's existing pool insufficient.

# 3. Define connection-affecting proxy route identity

Pooling requires a key that prevents unsafe reuse across incompatible routes.

Define an internal route/client key that distinguishes every policy capable of changing the established connection or tunnel, including as applicable:

- proxy scheme/host/port;
- pinned proxy addresses;
- proxy TLS configuration identity/policy;
- CONNECT destination/origin for tunnels;
- pinned proxied target;
- origin TLS/SNI policy for tunneled HTTPS;
- HTTP version/ALPN policy;
- local-address/socket/physical lifecycle policy if it applies to proxy connectors;
- any other connection-scoped state discovered during implementation.

Credentials require special treatment. Different CONNECT credentials must never share a tunnel established under another credential identity. Forward-proxy connections can carry per-request Proxy-Authorization, but do not pool across auth configurations unless correctness is proven and no connection-scoped proxy authentication assumption is introduced.

Do not expose credentials in `Debug`, metric labels, cache keys rendered to logs, or error strings. An internal opaque/fingerprint/identity representation is acceptable where raw secret material need not be retained in a printable key.

Prefer a bounded cache using the existing SNI/SOCKS client-cache pattern rather than introducing an LRU dependency.

Acceptance:

- [ ] No pooled client/tunnel can cross incompatible proxy TLS/auth/pinning/origin policy.
- [ ] Cache growth is bounded for request-scoped arbitrary proxies.
- [ ] Key `Debug`/logging cannot disclose proxy credentials.

# 4. Move HTTP forward proxy framing under Hyper

Replace the successful forward-proxy request/response path with a Hyper client backed by a proxy connector.

The connector owns only connection establishment:

- resolve/use pinned proxy addresses;
- TCP connect;
- optional TLS-to-proxy handshake;
- return connection metadata indicating proxy mode.

Hyper should own:

- request line/framing;
- request streaming/chunking;
- response head parsing;
- response body framing;
- keep-alive/connection-close handling;
- connection reuse and stale-idle recovery.

Request policy remains in eggfetch:

- proxy auth/header injection;
- target override validation;
- HTTP/2-only forward-proxy rejection;
- timeout/deadline integration;
- decompression/body limits above the transport;
- logical pool permits;
- failure mapping/redaction.

Be especially careful with proxy-only headers. They must be present on the proxy request but must never leak onto a later direct route due to shared mutable headers.

The initial reuse target may be per destination origin even though one HTTP proxy TCP connection can theoretically serve multiple origins. Cross-origin forward-proxy pooling is not required if it complicates Hyper pool keys or security reasoning.

Acceptance:

- [ ] Two sequential eligible requests through the same forward proxy/origin can complete using one accepted proxy connection.
- [ ] Response parsing/body streaming for the modernized route is owned by Hyper.
- [ ] Chunked streaming upload/download compatibility remains green.
- [ ] `Connection: close`, truncated bodies, stale idle sockets, cancellation, and dropped responses do not return unsafe connections to the pool.

# 5. Move CONNECT origin HTTP framing under Hyper

For HTTPS destinations, create a connector stack that returns an origin-ready transport to a Hyper client.

The stack conceptually owns:

1. connect to proxy (including optional TLS to HTTPS proxy);
2. perform CONNECT with proxy auth/headers and the correct physical/logical target distinction;
3. validate/sanitize non-200 rejection;
4. establish TLS to the logical origin over the tunnel with origin TLS/SNI/ALPN policy;
5. return the resulting transport and metadata to Hyper.

Hyper then owns the origin HTTP request/response protocol and connection/tunnel reuse.

If layering `hyper-rustls` around an upstream/custom Tunnel connector cleanly preserves TLS policy, use it. If not, the eggfetch connector may perform origin TLS itself before returning the stream. Do not maintain two competing origin TLS implementations without a concrete reason.

For HTTP/2 through CONNECT, verify that Hyper recognizes negotiated H2 metadata and multiplexes safely. A tunnel pinned to an origin must never be reused for another origin.

Acceptance:

- [ ] Two sequential compatible HTTPS requests through the same proxy to the same origin can reuse an established tunnel/connection where protocol semantics allow.
- [ ] H1 and existing H2-through-CONNECT behavior remain correct.
- [ ] Proxy and origin TLS policies remain independent.
- [ ] Pinned ultimate destinations preserve logical Host/SNI and remain fail closed.
- [ ] CONNECT rejection retains the documented structured error information or an explicitly approved equivalent contract.

# 6. Retire only genuinely redundant manual HTTP code

After Hyper-backed forward/CONNECT paths are proven, identify manual helpers that no longer own a required handshake function.

Candidates include parts of:

- `write_proxy_request`;
- `write_proxy_body`;
- full successful-response use of `read_proxy_response`;
- `ProxyResponseStream`;
- `TlsProxyResponseStream`;
- duplicated response-header conversion/reason/body-length handling.

Do not delete `read_proxy_response` merely because successful origin responses moved to Hyper if a bounded version is still required for CONNECT handshake/rejection parsing. Narrow/rename it to its actual handshake role instead.

The desired maintenance result is fewer independent HTTP parsers, not the smallest line count.

Acceptance:

- [ ] There is no second full origin HTTP/1 response implementation on proxy routes unless a documented Hyper limitation requires it.
- [ ] Remaining manual parsing has a narrow handshake-specific owner and fuzz/unit coverage.

# 7. Preserve timeout and lifecycle semantics

Connection reuse changes where time is spent. Re-audit timeout mapping carefully.

Required invariants:

- a reused connection must not consume a fresh proxy-connect/proxy-TLS budget because those phases do not run;
- creating a new proxy connection still reports ProxyConnect/ProxyTls appropriately;
- CONNECT/origin TLS remains bounded by the existing total deadline and applicable phase budgets;
- response read/write inactivity remains governed by the established transport lifecycle policy;
- logical request permits are held until the response body is consumed/closed/dropped exactly as on direct routes;
- physical connection admission applies to newly opened proxy transports rather than each logical request;
- stale pooled connection retries remain bounded by the existing Hyper canceled-request policy and eggfetch retry policy does not double-retry unsafely.

Add deterministic paused-time/fake-proxy tests where possible instead of sleep-heavy integration tests.

# 8. Preserve observability without inventing reuse metrics

A connector invocation is truthful evidence that a physical connection/tunnel establishment was attempted. Hyper pool reuse may not expose a stable public callback for every reuse event.

Do not fabricate exact “pool hit” counts from request counts minus connector attempts unless the semantics are documented and race-safe.

It is acceptable to expose/retain connection-attempt metrics and prove reuse in tests by counting accepted proxy sockets.

Any new public metric requires a separate API justification; it is not required for this plan.

# 9. Test matrix

Focused native tests must cover at least:

### Reuse

- repeated HTTP forward requests reuse a connection;
- repeated HTTPS CONNECT requests to same origin reuse a compatible tunnel;
- concurrent H2-over-CONNECT requests multiplex if supported by existing policy;
- `Connection: close` forces reconnection;
- server/proxy-closes-idle causes safe reconnection;
- dropped/incomplete body does not make a poisoned connection reusable.

### Isolation

- different proxy endpoints do not share;
- different proxy credentials/config identities do not share in unsafe ways;
- different proxy TLS policy does not share;
- different CONNECT origins do not share;
- different pinned proxy/target routes do not share;
- proxy route never shares direct-route connections.

### Compatibility/security

- HTTPS proxy endpoint;
- proxy custom CA;
- origin custom CA/mTLS where already supported;
- proxy auth and proxy-only headers;
- no auth/header leakage inside CONNECT origin request;
- NO_PROXY and request `without_proxy()`;
- redirect route changes;
- IPv6 CONNECT authority;
- pinned destination/SNI identity;
- streaming request/response;
- large/chunked bodies;
- cancellation and close;
- malformed/bounded CONNECT response;
- rejection body sanitization;
- timeout classification.

Run the existing Python HTTPX proxy/TLS compatibility tests after every meaningful transport change.

# 10. Dependency and upstream-version discipline

The core already depends on `hyper-util = "0.1"`. Determine the exact locked version and whether the required proxy APIs are available without a version bump.

If an upgrade within 0.1 is needed:

- review the complete changelog between locked and target versions;
- confirm Rust 1.89 compatibility;
- rerun direct/H1/H2/custom-dialer/UDS/SOCKS tests, not only proxy tests;
- record any newly activated `hyper-util` features (`client-proxy`, `client-pool`, etc.);
- do not enable `full` merely for convenience.

No new third-party proxy crate should be added unless the upstream Hyper path proves insufficient and the dependency yields a demonstrated net reduction in maintenance/security burden.

## Required validation

During implementation:

```sh
./scripts/check.sh
```

At plan closure, run at minimum:

```sh
./scripts/check.sh extended
```

plus focused proxy reuse/isolation tests and the relevant HTTPX/HTTPX2 proxy suites.

If `hyper-util` or other dependencies change, run the explicit security preflight from the sibling security plan before declaring this plan ready for final freeze.

The parent program's final plan owns the exact-SHA compatibility renewal.

## Non-goals

- no HTTP/3-over-proxy implementation;
- no generic service-mesh framework;
- no Eggress-specific API or dependency;
- no cross-origin forward-proxy pooling requirement;
- no rewrite of already-pooled SOCKS solely for code symmetry;
- no new public resolver API;
- no custom LRU dependency solely for client cache eviction;
- no weakening of structured proxy errors just to fit an upstream helper;
- no proxy auto-discovery expansion beyond existing behavior.

## Exit criteria

This plan is complete when ordinary HTTP forward/CONNECT proxy traffic uses Hyper for successful HTTP framing and connection lifecycle, eligible routes demonstrably reuse physical connections/tunnels, connection-affecting policy is safely isolated, existing proxy/TLS/pinning/timeout semantics remain qualified, and the amount of eggfetch-owned generic HTTP parsing is materially reduced rather than merely moved.