# Static Resolution and Pinned Destination Routing

Planning baseline: `445149bd2af3c3ce5c0d34daacbde1bf8de57297` (`main`, 2026-09-11; eggfetch 0.1.3)
Parent program: `plans/embedded-rust-client-footprint-and-routing-program.md`
Depends on: `plans/core-feature-dependency-and-tls-boundary-hardening.md` reaching a stable direct/TLS connector shape

## Closure evidence — 2026-09-12

Implemented in the frozen executable commit
`6e65c5bfc2607b30af8062a9269fcd260ed96464`. The request-scoped
`ResolvedTarget`/`resolved_addresses()` API uses the existing direct connector
candidate loop, never calls DNS in static mode, preserves logical Host/TLS
identity, retains the snapshot for retries and same-origin redirects, and
rejects cross-origin redirects, proxy/UDS/H3 combinations before I/O. Focused
direct/TLS tests and the full Tier 1/extended/package gates passed. This v1
contract is intentionally fail-closed for cross-origin redirects; no separate
redirect destination map or SSRF policy engine was added.

## Objective

Add a native, typed way for callers to supply one or more already-resolved remote `SocketAddr` values for a logical HTTP(S) origin and require eggfetch to connect exclusively to those addresses without performing a second DNS lookup, while preserving logical URL semantics, HTTP Host behavior, TLS SNI, certificate verification, redirects/retries, timeouts, pooling, metrics and existing transport security rules.

This is a general-purpose transport capability for secure URL fetchers, custom resolvers, service discovery, split-horizon environments and deterministic tests. It must not be designed as a CodeGG-specific SSRF switch.

## Security requirement

The primary invariant is fail-closed destination control:

> When a request carries an explicit resolved destination set, no execution path may silently perform fresh DNS resolution or route the origin through a transport that does not honor that set.

The remote socket address controls only where TCP/QUIC connects. It must not replace the request's logical origin identity used for Host, SNI, certificate verification, auth/cookie origin policy, redirects or user-facing URL reporting.

## Current architecture

`DirectConnector` already owns the correct low-level sequence for this capability:

1. obtain URL host/port;
2. call `tokio::net::lookup_host`;
3. iterate candidate `SocketAddr` values;
4. create/configure `TcpSocket`;
5. optionally bind a local address/apply socket options;
6. connect;
7. wrap in TLS using the logical hostname or explicit SNI override.

`TransportHints` already carries destination-sensitive wire behavior (`target`, `sni_hostname`, trace observer), is explicitly preserved across retries, and is cleared according to redirect destination-change semantics. The recent request/transport consolidation gives this work a suitable typed state boundary.

The missing capability is replacing step 2 with a caller-supplied address set and ensuring every route selection path either honors it or rejects the request before network I/O.

# 1. Define the public/native model

Introduce a small typed model. Exact names may change, but the semantics should resemble one of:

```rust
pub struct ResolvedTarget {
    addresses: Arc<[SocketAddr]>,
}
```

or a route-specific enum if future extensibility materially improves correctness:

```rust
pub enum ResolutionPolicy {
    System,
    Static(Arc<[SocketAddr]>),
}
```

Prefer a type that cannot represent an ambiguous "empty static set means use DNS" state. Empty caller-supplied address sets must be rejected during request construction or preparation.

The API should be request-scoped first. A client-wide resolver override is explicitly not required by this plan. A future resolver interface can be considered separately if a concrete use case appears.

Expose the capability through an idiomatic native builder method rather than requiring callers to construct internal transport structures manually. If `TransportHints` remains the best storage boundary, provide a higher-level method such as `resolved_addresses(...)`/`connect_to(...)` that constructs the internal typed state safely.

Acceptance:

- [x] Public/native API clearly distinguishes normal system resolution from static caller-supplied routing.
- [x] Empty static destinations are rejected before I/O.
- [x] Debug output never leaks more destination detail than ordinary connection metadata policy allows; no secrets are involved.
- [x] Existing callers need no changes unless they opt into the new behavior.

# 2. Extend `DirectConnector` rather than adding a second transport stack

Implement static destination support inside the existing direct connector path.

The connector should choose candidate addresses as follows:

```text
if static resolved addresses exist:
    use exactly those addresses
else:
    perform existing system DNS lookup
```

Keep existing address-family filtering for local-address binding and existing socket-option application. Iterate supplied addresses under the same shared connect budget/multi-address fallback policy as system-resolved addresses.

Do not implement static routing by rewriting the URL host to an IP address. That would corrupt Host/SNI/cert identity and origin semantics.

Acceptance:

- [x] No duplicate connector implementation is created.
- [x] Static and DNS-derived candidate sets share connect/fallback/socket-option logic.
- [x] Static mode performs no `lookup_host` call.
- [x] All supplied candidates obey the existing connect timeout budget rather than receiving a fresh full timeout each.

# 3. Preserve HTTP and TLS logical identity

For a request such as:

```text
https://service.example/api
static destinations = [203.0.113.10:443]
```

required behavior is:

```text
TCP remote:              203.0.113.10:443
HTTP Host / authority:   service.example
TLS SNI:                 service.example
certificate validation:  service.example
Response::url():          https://service.example/api
origin policy:            service.example
```

An explicit existing `sni_hostname` override may still override TLS identity according to current eggfetch semantics; static routing must not invent one automatically beyond normal logical-host behavior.

Add H1 Host-header regressions and HTTPS tests using a deterministic local TLS server/certificate fixture. Prefer local certificates/test helpers already used by eggfetch; do not depend on public DNS or external hosts.

Acceptance:

- [x] HTTP Host remains the logical authority.
- [x] HTTPS SNI/certificate verification uses logical identity unless the caller explicitly requested an existing SNI override.
- [x] IP literals continue to work according to existing rustls IP-server-name semantics.

# 4. Define retry semantics

Retries of the same logical request must preserve the exact static destination snapshot by default.

Rationale: a caller that resolved/validated addresses before dispatch expects retry not to silently re-resolve or change the trust/routing decision.

Required behavior:

- body replay rules remain unchanged;
- retry timing/backoff remains unchanged;
- candidate addresses may be attempted again according to existing connector behavior;
- no DNS lookup is introduced on later attempts;
- error taxonomy should distinguish invalid static routing configuration from ordinary connection failure where useful, without unnecessary variant proliferation.

Acceptance:

- [x] Retry reconstruction preserves the static destination set.
- [x] Regression test makes system resolution fail/change after first attempt and proves retry still uses the supplied set.

# 5. Define redirect semantics conservatively

Static destination state is destination-specific. Redirect handling must therefore be explicit.

Required default policy:

- same-origin redirects may preserve the static destination set because logical `(scheme, host, effective port)` remains unchanged;
- cross-origin redirects must clear the old static destination set and either use normal resolution for the new origin only if that behavior is explicitly documented/selected, or fail closed if the request was marked as requiring fully pinned routing across the request lifecycle;
- redirects that change effective port must not reuse addresses whose port no longer matches unless the static model explicitly supports per-origin/per-port mappings;
- HTTPS -> HTTP or HTTP -> HTTPS changes are not considered same-origin for routing preservation.

Prefer a simple v1 contract. Do not build a general redirect destination map unless a concrete caller requires it.

For security-sensitive use, provide a strict mode or semantics in which any redirect requiring a different origin fails before network I/O rather than silently returning to DNS. If the existing redirect policy already supports disabling redirects, document how it composes with static routing.

Acceptance:

- [x] Same-origin redirect behavior is directly tested.
- [x] Cross-origin redirect cannot accidentally connect the new host to the old pinned address.
- [x] Strict pinned mode never returns to DNS through redirect handling.

# 6. Define proxy interaction

A direct-origin static destination set and a proxy route are different routing authorities. Do not silently ignore either.

Initial supported contract should be conservative:

- request-level static origin routing is supported for direct requests;
- if an effective HTTP/SOCKS proxy would be used, reject the incompatible combination before opening the proxy connection unless an existing explicit direct/no-proxy override is active;
- do not reinterpret the origin destination set as proxy addresses;
- do not add proxy-address pinning in this plan.

If implementation can safely route an HTTPS CONNECT tunnel to a pinned destination while still going through a proxy, that is a separate capability and should not be inferred here because the proxy performs or influences destination resolution depending on mode.

Acceptance:

- [x] Effective proxy + static origin routing has deterministic documented behavior.
- [x] No request silently bypasses a configured proxy unless the caller explicitly selected direct routing through existing API.
- [x] No proxy path silently discards the static destination constraint.

# 7. Define HTTP/3 interaction

HTTP/3 has an independent Quinn transport and multi-address path. Do not claim static routing support until that path can enforce the same constraint.

Initial acceptable behavior:

- when `Http3Only` or H3 dispatch would be selected for a request carrying static destinations, either implement the same address-set constraint end to end or reject that combination before H3 network I/O;
- Auto/H3 fallback must never cause a request to escape the static destination set through ordinary DNS;
- Alt-Svc must not redirect the transport to an unvalidated alternative endpoint when strict static routing is active unless explicit validated semantics are designed and tested.

Given HTTP/3 remains experimental, rejecting unsupported static-routing combinations is preferred over adding complex special cases in this plan.

Acceptance:

- [x] H3 cannot bypass static routing.
- [x] Error/documentation clearly states unsupported combinations.

# 8. Pooling, connection reuse, and route identity

Review Hyper connection-pool reuse with static destinations carefully.

A client must not reuse an existing connection for the same logical origin if doing so violates the caller's newly supplied destination constraint. Conversely, requests with the same logical origin and equivalent static route may safely reuse a compatible connection if the connector/client cache key proves route equivalence.

Determine whether the existing specialized-direct client caching key includes enough route identity. If not, add a route identity to the appropriate client/connector cache key or disable unsafe cross-route reuse for static-routed requests.

Do not key logical concurrency permits by raw address unless necessary; the pool's logical origin semantics should remain distinct from physical route identity.

Acceptance:

- [x] A static-routed request cannot accidentally ride a pooled connection established through ordinary DNS to another address.
- [x] Two different static address sets for the same logical origin do not share an incompatible physical connection cache.
- [x] Logical per-origin concurrency semantics remain unchanged unless documented.

# 9. Security regression suite

Add deterministic tests covering at minimum:

1. hostname has no resolvable DNS answer, supplied local `SocketAddr` succeeds;
2. Host header remains logical hostname;
3. HTTPS verifies/SNI against logical hostname while connecting to supplied IP;
4. empty static set is rejected;
5. all supplied addresses fail -> connection error, no DNS fallback;
6. first supplied address fails and second succeeds under one connect budget;
7. retry preserves supplied set;
8. same-origin redirect preserves where allowed;
9. cross-origin redirect never reuses old set;
10. strict pinned redirect mode never falls back to DNS;
11. effective proxy combination rejects before I/O unless explicitly supported;
12. H3/Alt-Svc cannot bypass pinning;
13. pooled connection/client cache cannot cross incompatible route constraints;
14. local-address binding and socket options still work with static candidates.

Where possible, instrument a test resolver seam/counter so tests prove DNS was not called instead of inferring that fact only from request success.

# 10. Public documentation/API naming

Document the capability in native Rust terms such as caller-supplied/static resolution or resolved destination routing. Explain security properties without marketing it as automatic SSRF protection: the caller remains responsible for validating addresses and redirect policy.

Document explicitly:

- logical identity vs physical destination;
- no DNS fallback guarantee;
- retry semantics;
- redirect semantics;
- proxy/H3 limitations;
- pool/reuse considerations;
- difference from local source-address binding and SNI override.

## Non-goals

- no built-in SSRF validator/IP policy engine;
- no CIDR/private-address blocklist;
- no custom DNS resolver trait in this plan;
- no DNS cache;
- no DoH/DoT;
- no service registry;
- no proxy endpoint pinning;
- no H3 expansion beyond what is required to fail closed;
- no CodeGG migration;
- no URL-host rewriting workaround.

## Required validation

Run focused static-routing tests plus:

```sh
./scripts/check.sh
```

Run affected retry/redirect/proxy/TLS/H3 route-selection tests directly. At closure, run `./scripts/check.sh extended` when existing prerequisites are available.

Do not renew HTTPX/HTTPX2 exact-SHA qualification in this plan. The new request/transport state is executable behavior and intentionally invalidates the prior freeze until the parent program's requalification plan.

## Exit criteria

- [x] Native callers can supply validated remote address sets for a logical origin.
- [x] Static mode performs no second DNS lookup and never silently falls back to one.
- [x] Host/SNI/certificate/origin semantics remain logical-host based.
- [x] Retry/redirect behavior is explicit and tested.
- [x] Proxy/H3 incompatible routes fail closed.
- [x] Physical connection reuse cannot violate route constraints.
- [x] Existing ordinary-resolution behavior remains unchanged for requests not using the feature.
- [x] Documentation makes caller responsibilities and guarantees precise.
