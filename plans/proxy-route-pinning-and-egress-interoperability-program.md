# Proxy Route Pinning and Egress Interoperability Program

Status: Complete

Date: 2026-09-14

Eggfetch baseline: `d681da848e36199846604b71f2162a1054d24aa6`

Egress research baseline: `eggstack/eggress@c82649d3992fb9757de94fed945f6710bbfef61d`

Motivating downstream evidence: EggSec's scope-aware transport work. The requirements are intentionally generalized; no EggSec type, policy object, feature flag, or adapter belongs in Eggfetch.

## Purpose

Close the remaining physical-route control gap in Eggfetch's native proxy transport without weakening the existing `ResolvedTarget` contract, importing a second routing policy engine, or specializing the HTTP client for one downstream.

Eggfetch already has first-class caller-supplied destination routing for direct requests through `ResolvedTarget` / `RequestBuilder::resolved_addresses()`. That path correctly keeps the logical URL authoritative for HTTP identity, TLS SNI/certificate verification, redirects, cookies, and auth while using caller-supplied `SocketAddr` values for the physical connection. It deliberately rejects proxy, UDS, and HTTP/3 combinations.

The proxy subsystem does not yet provide the corresponding physical-route controls. The proxy endpoint is still resolved internally in the proxy transport, and CONNECT/SOCKS routes may also delegate ultimate-target resolution below a caller that wants to validate physical destinations before I/O. This is the remaining generic blocker for security-sensitive embedders that need both an explicit proxy and a proof that no independent DNS decision occurs after caller policy.

The program adds two orthogonal controls in order:

1. pin the physical proxy peer while preserving the proxy's logical URI/TLS identity;
2. where the proxy protocol can represent it safely, pin the physical ultimate destination of a proxied tunnel while preserving the origin's logical HTTP/TLS identity.

The existing direct `resolved_addresses()` behavior remains unchanged and fail-closed with proxies.

## Confirmed current architecture

At the Eggfetch baseline:

- `ResolvedTarget` is request-scoped direct routing. Static mode performs no second DNS lookup, retries preserve the snapshot, same-origin redirects may retain it, cross-origin redirects fail closed, and proxy/UDS/H3 combinations are rejected.
- `Proxy` is the public native proxy configuration; `ProxyConfig` is the resolved internal configuration passed into transport selection.
- `transport/proxy.rs::connect_to_proxy()` owns proxy endpoint DNS via `tokio::net::lookup_host`, candidate dialing, proxy TLS setup, CONNECT, origin TLS, timeout budgeting, and proxy transport metrics.
- HTTPS proxy TLS already distinguishes logical proxy identity from the selected physical socket address. This is the correct seam for caller-pinned proxy peers.
- SOCKS5 and SOCKS5H remain semantically distinct in the native API: local-resolution SOCKS5 and proxy-resolution SOCKS5H must not be collapsed by the new native routing controls.
- `ClientBuilder::dialer()` is already a generic external raw-stream seam for callers that own an entire route. It intentionally conflicts with Eggfetch's built-in proxy route rather than creating two routing authorities.
- `TransportHints` is a public native type and has already grown fields during 0.1.x. New proxy route controls must not require downstream callers to exhaustively construct another expanded public struct literal.

## Why one physical address control is not enough

A proxied request has at least four distinct identities/locations:

```text
logical origin            service.example:443
physical origin target    203.0.113.10:443
logical proxy             proxy.example:8443
physical proxy peer       198.51.100.20:8443
```

For an HTTPS CONNECT route, TCP first connects to the physical proxy peer while proxy TLS (when the proxy is HTTPS) authenticates `proxy.example`. The proxy then receives a CONNECT target. After the tunnel is established, destination TLS authenticates `service.example`.

A security-sensitive embedder may need to validate both physical addresses independently. Pinning only the proxy peer still leaves the proxy free to resolve the CONNECT hostname. Pinning only the origin target does not prove which proxy socket Eggfetch connected to. The API and tests therefore must model these as separate optional constraints rather than overloading one `resolved_addresses()` value.

## Egress reuse decision

### `eggress-routing`: do not depend on it from Eggfetch

`eggress-routing` is a policy/scheduler layer, not a neutral socket-address primitive. It brings route rules, direct/upstream/reject actions, regex/CIDR matching, upstream groups, health state, leases, Tokio state, and Egress-specific models. Importing it into `eggfetch-core` would give Eggfetch a second routing-policy language and invert the desired ownership boundary.

Eggfetch should remain an HTTP engine that executes an explicit route supplied by its caller. Egress may decide a route outside Eggfetch, but Eggfetch must not import Egress policy to decide whether a route is authorized or preferred.

### `eggress-uri`: do not adopt for this work

`eggress-uri` is lightweight, but it deliberately parses a much broader proxy/protocol vocabulary than Eggfetch needs (HTTP/SOCKS plus SSH, FTP, LDAP, SMTP, IMAP, POP3, custom schemes and chain-oriented metadata). Replacing Eggfetch's current proxy URL model with it would couple HTTP client semantics to Egress's cross-protocol grammar without deleting the transport work this program needs.

Re-evaluate only if a later measured change shows substantial duplicated parser/model code can actually be removed without broadening Eggfetch's accepted schemes or public API.

### `eggress-protocol-socks`: technically reusable, not useful for this program

The Egress SOCKS crate exposes a clean stream-level SOCKS4/5 implementation, but replacing Eggfetch's existing SOCKS handshake does not solve the route-pinning gap. Eggfetch would still own proxy TCP dialing, candidate fallback, shared deadlines, local bind/socket options, metrics, logical origin TLS, retry behavior, pool identity, and request error provenance. Adopting the crate now would add `eggress-core` and related dependencies while moving only handshake bytes.

Do not combine a SOCKS implementation swap with proxy route pinning. If a future independent maintenance audit demonstrates real code/dependency deletion, evaluate it as a separate project with before/after graph measurements.

### Supported relationship: Egress above Eggfetch

The existing Eggfetch `Dialer` seam is the correct composition point when a consumer wants Egress to own an entire route or proxy chain. Egress can establish a stream according to its own policy and hand that stream to Eggfetch; Eggfetch then owns HTTP and destination TLS according to the dialer contract. No dependency edge from Eggfetch to Egress is required.

This program must preserve that direction:

```text
application policy / optional Egress routing
                 |
                 v
         explicit route choice
                 |
        +--------+--------+
        |                 |
        v                 v
 Eggfetch built-in     Eggfetch Dialer
 proxy route           (external stream)
        |
        v
 HTTP / TLS engine
```

Do not add Egress-specific examples, feature flags, public types, or optional dependencies to `eggfetch-core` merely to prove this composition.

## Public API direction

The exact names may be adjusted during implementation, but the API must preserve these separations.

### Proxy peer pinning

Preferred native shape: extend the existing public `Proxy` builder with a caller-supplied resolved-address option, conceptually:

```rust
let proxy = Proxy::all("https://proxy.example:8443")?
    .resolved_addresses(["198.51.100.20:8443".parse()?])?;
```

The proxy URL remains logical identity. The supplied addresses control only the physical socket opened to the proxy.

Reuse the existing validated resolved-address representation internally where doing so does not make its public documentation false. If necessary, broaden `ResolvedTarget` documentation from "HTTP origin" to "logical endpoint" without changing its existing behavior, or introduce a private shared validated-address type beneath the public wrappers. Do not create two independent candidate-loop implementations.

Do not add a new required public `TransportHints` field for proxy-peer routing. `Proxy` fields are private and are the safer compatibility boundary.

### Proxied ultimate-destination pinning

This must be an explicit request-level opt-in separate from direct `resolved_addresses()`. The existing direct method currently fails closed when a proxy is effective; changing its meaning to silently become a CONNECT/SOCKS target would weaken an established security contract.

The implementation plan may select a concise native name such as `proxy_target_addresses(...)`, `proxied_addresses(...)`, or a small typed proxy-route hint, but the semantics must be unambiguous: these addresses describe the ultimate proxied target, not the proxy peer.

Avoid adding another publicly exhaustible bag-of-options struct. Prefer a fluent request builder or an opaque/validated type with accessor methods.

## Protocol support matrix

The initial support contract is intentionally conservative.

| Route | Pinned proxy peer | Pinned ultimate target | Required behavior |
|---|---|---|---|
| HTTP proxy, HTTP origin (forward proxy) | yes | not in v1 | Forward proxy resolves/forwards the absolute logical URL; fail closed if an ultimate pin is requested unless an explicit CONNECT-for-HTTP design is separately justified |
| HTTP proxy, HTTPS origin (CONNECT) | yes | yes | CONNECT may use the approved physical destination while origin Host/SNI/certificate identity stays logical |
| HTTPS proxy, HTTPS origin (TLS-to-proxy + CONNECT) | yes | yes | Proxy TLS identity stays logical proxy host; destination TLS identity stays logical origin host |
| SOCKS5 | yes | yes | Send an approved IP target without a second destination DNS lookup; preserve existing timeout/candidate semantics |
| SOCKS5H | yes | no | Remote DNS is the protocol contract; explicit ultimate-target pinning must fail closed rather than claim local validation |
| HTTP/3 | n/a | n/a | Built-in proxy route remains incompatible; no H3/MASQUE work in this program |
| UDS/custom dialer | n/a | n/a | Remain separate routing authorities; do not merge with built-in proxy pinning |

If implementation evidence shows a row cannot preserve the stated invariants cleanly, reject that row before I/O and document it rather than creating a weaker fallback.

## Security invariants

1. Caller-supplied physical routes never silently fall back to system DNS.
2. Logical proxy identity and physical proxy destination remain separate.
3. Logical origin identity and physical origin destination remain separate.
4. HTTPS proxy certificate verification/SNI use the logical proxy hostname unless the caller explicitly uses existing proxy TLS overrides.
5. Destination TLS certificate verification/SNI and HTTP Host use the logical origin hostname unless an existing explicit override applies.
6. A supplied address set is immutable for the logical attempt/retry snapshot; retry cannot silently re-resolve.
7. Pool/client reuse may not cross incompatible physical-route constraints.
8. Redirects may not carry a destination-specific physical route to a different origin. Same-origin retention is allowed only when the effective proxy and ports remain compatible.
9. SOCKS5H is never presented as locally destination-pinned.
10. Proxy credentials remain proxy-leg-only and retain existing redaction guarantees.
11. Existing requests that do not opt into route pinning behave exactly as before.
12. The feature adds no authorization, CIDR, SSRF, allow/deny, or downstream policy engine to Eggfetch; validation remains the caller's responsibility.

## Execution order

1. [`pinned-proxy-peer-routing.md`](pinned-proxy-peer-routing.md) — add caller-pinned physical proxy endpoints to the existing proxy transport and prove DNS-free proxy dialing, identity preservation, candidate fallback, retry behavior, and route-aware reuse.
2. [`pinned-proxy-destination-routing.md`](pinned-proxy-destination-routing.md) — add explicit physical ultimate-target pinning only to proxy protocols where Eggfetch can enforce it end-to-end without weakening logical Host/TLS semantics.
3. [`post-proxy-route-pinning-qualification-and-closure.md`](post-proxy-route-pinning-qualification-and-closure.md) — audit the final API/dependency boundary, run the repository's current validation tiers and proxy security regressions, re-check compatibility evidence where required, and record the Egress non-dependency disposition.

Plan 2 depends on Plan 1 because an end-to-end scope-controlled proxy route is incomplete if Eggfetch can still independently resolve the proxy peer. Plan 3 is last.

## Dependency policy

Expected production dependency delta for `eggfetch-core`: **zero**.

The existing proxy transport already has the networking/TLS primitives required. Prefer extending the current candidate-dial and proxy handshake paths rather than importing Egress, Tower routing, another resolver, or a second proxy client.

At closure record:

```text
cargo tree -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo tree -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
cargo tree -p eggfetch-core -d
```

The minimal non-proxy graph must remain unaffected by this program. Any new dependency requires a written explanation and before/after graph evidence; an Egress dependency is not expected or approved by this plan.

## Compatibility policy

This is additive native-Rust behavior. Existing Python, CLI, HTTPX, HTTPX2, direct resolved routing, custom dialer, UDS, proxy, TLS, redirect, retry, and error surfaces retain their current semantics unless a documented bug is found.

Do not expose the new native route pinning through Python/HTTPX compatibility APIs merely for symmetry. Those facades should change only when their reference APIs require a corresponding capability.

Do not make `TransportHints` more source-fragile during 0.1.x. Downstream exhaustive struct literals already make public-field growth costly. Prefer fluent methods/private fields for this program; consider an opaque or deliberately `#[non_exhaustive]` transport-hints boundary only as a separately reviewed compatibility change.

## Non-goals

- No EggSec, CodeGG, Synvoid, Eggpool, Egress, or other downstream adapter.
- No built-in SSRF validator, CIDR blocklist, scope engine, authorization callback, or DNS policy trait.
- No automatic proxy-chain selection.
- No migration of Eggfetch's SOCKS implementation to Egress.
- No Egress dependency in Eggfetch.
- No change to `RequestBuilder::resolved_addresses()` direct-only fail-closed semantics.
- No SOCKS5H local-validation claim.
- No CONNECT-UDP, MASQUE, HTTP/3 proxying, WebTransport, or QUIC proxy work.
- No forced CONNECT-for-plaintext-HTTP behavior without a separate protocol/design justification.
- No binary-size claim. The retained embedded evidence classifies Eggfetch as larger than aligned minimal Reqwest profiles; this program is about route control and ownership.

## Program exit criteria

The program closes only when all are true:

1. Native callers can pin the physical proxy peer without system DNS fallback while preserving logical proxy TLS identity.
2. Native callers can pin the physical ultimate destination for HTTPS CONNECT and local-resolution SOCKS5 without corrupting logical origin Host/SNI/certificate semantics.
3. Unsupported combinations, including SOCKS5H ultimate pinning and plaintext forward-proxy ultimate pinning, fail before network I/O.
4. Retry, redirect, timeout, candidate fallback, proxy auth, proxy TLS, destination TLS, and structured failure semantics are tested under pinned routes.
5. Physical connection reuse cannot cross incompatible pinned route snapshots.
6. Ordinary unpinned proxy behavior and existing direct `resolved_addresses()` behavior remain unchanged.
7. Minimal non-proxy feature/dependency graphs do not grow.
8. `eggfetch-core` has no Egress dependency; the retained decision explains why Egress remains an external policy/dialer layer.
9. Current repository verification and compatibility obligations are green on the final executable tree.
10. Documentation clearly distinguishes proxy peer, ultimate target, logical proxy identity, logical origin identity, SOCKS5, and SOCKS5H semantics.

## Closure record

The executable/test/fixture freeze is
`13c4ab4`, based on the baseline
`d681da848e36199846604b71f2162a1054d24aa6`. It adds the native
`Proxy::resolved_addresses()` peer pin and
`RequestBuilder::proxy_target_addresses()` proxied-target pin, with no new
production dependency or feature. Direct `resolved_addresses()` remains
direct-only; unsupported SOCKS5H and plaintext forward-proxy target pins fail
closed before proxy I/O; proxy/origin logical TLS and HTTP identities remain
separate from their physical destinations; route snapshots participate in
retry, redirect, and SOCKS route-cache state.

The local evidence bound to this freeze is Tier 1, extended, package, focused
proxy/propagation tests, documentation examples/links, and the existing
compatibility/lifecycle suites. Extended validation retained only the
documented optional skips for the unbuilt Node artifact, the unavailable
Rust-1.80/MSRV parser, and the absent downstream artifact manifest. The
documentation-only closure update is intentionally a descendant of this
freeze.
