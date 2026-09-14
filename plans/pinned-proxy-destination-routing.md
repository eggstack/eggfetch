# Pinned Proxied Destination Routing

Status: Ready for handoff

Date: 2026-09-14

Parent: [`proxy-route-pinning-and-egress-interoperability-program.md`](proxy-route-pinning-and-egress-interoperability-program.md)

Depends on: [`pinned-proxy-peer-routing.md`](pinned-proxy-peer-routing.md)

Baseline: `d681da848e36199846604b71f2162a1054d24aa6`

## Objective

Add an explicit native request-level mechanism for a caller to pin the physical **ultimate destination** of a proxied request where Eggfetch can enforce that choice end to end, without changing the existing direct `RequestBuilder::resolved_addresses()` contract and without conflating the ultimate destination with the physical proxy peer.

The feature must preserve logical origin semantics:

```text
logical origin:          https://service.example/api
approved origin socket:  203.0.113.10:443
logical proxy:           https://proxy.example:8443
approved proxy socket:   198.51.100.20:8443

TCP #1:                  198.51.100.20:8443
proxy TLS identity:      proxy.example
CONNECT / SOCKS target:  203.0.113.10:443
origin TLS identity:     service.example
HTTP Host / URL:         service.example / logical URL
```

This lets callers that already own DNS/service-discovery/authorization decisions prove that neither Eggfetch nor the selected proxy route silently substitutes a different destination.

## Why this must be a distinct API

`RequestBuilder::resolved_addresses()` has an established security contract: it is direct-only and rejects effective proxy routes. Reinterpreting that existing option as a proxy tunnel target would silently broaden an API that currently fails closed.

The new control must therefore be explicitly proxy-oriented. Conceptual native shapes include:

```rust
client
    .get("https://service.example/api")?
    .proxy_target_addresses(["203.0.113.10:443".parse()?])
    .send()
    .await?;
```

or an opaque validated proxy-route hint carried through a fluent request builder.

Exact naming may change, but it must make the two address sets impossible to confuse:

- proxy peer addresses belong to `Proxy`;
- proxied target addresses belong to the request/logical origin.

Do not add another required field to the publicly constructible `TransportHints` struct. Prefer a builder method backed by private request state or an opaque validated type. If implementation reuses internal `TransportHints`, expose construction through methods and preserve source compatibility.

## Core semantics

The caller-supplied target snapshot controls only the physical target communicated to a proxy protocol. It does **not** replace:

- the request URL;
- origin comparison for redirects/auth/cookies;
- HTTP Host;
- destination TLS SNI;
- destination certificate verification;
- response URL/history;
- proxy selection;
- proxy peer pinning.

Empty sets and port mismatches fail before I/O. The effective target port must match the logical origin's effective port for v1.

Retries preserve the exact snapshot. Same-origin redirects may preserve it. Cross-origin redirects must fail closed or clear it only into an explicitly unpinned behavior chosen by the caller; the preferred v1 behavior is fail closed, matching direct static routing.

## HTTP CONNECT implementation

### HTTPS origin through HTTP/HTTPS proxy

For HTTPS origins, the proxy transport already establishes a CONNECT tunnel before destination TLS. This is the cleanest enforceable path.

Without a pin, keep existing behavior:

```text
CONNECT service.example:443
```

With an explicit proxied target pin, the wire CONNECT authority may use the approved physical address:

```text
CONNECT 203.0.113.10:443
```

After the tunnel is established, destination TLS must still use the logical origin host (`service.example`) for SNI and certificate validation, and HTTP inside the tunnel must retain the logical Host/URL.

For IPv6, use correct bracketed CONNECT authority syntax and test it directly.

### Multiple approved target candidates

A multi-address target set requires explicit retry/fallback semantics because an HTTP CONNECT proxy receives one authority per tunnel attempt.

Use the existing shared connect/total deadline. For each approved candidate, Eggfetch may establish/reuse the proxy leg as safe and attempt CONNECT to that candidate. A failed CONNECT to one approved target may move to the next candidate only while the common budget remains.

Do not perform a fresh origin DNS lookup after all supplied candidates fail.

Be careful with HTTP status semantics. Proxy CONNECT rejection (for example 403/407/502) is not always evidence that trying another physical destination is appropriate. Define a conservative retry/fallback table from typed conditions rather than blindly cycling candidates on every non-2xx CONNECT response.

Suggested initial rule:

- connection-level failure specifically attributable to the requested tunnel target: candidate fallback may continue;
- proxy authentication/policy response: stop, because the result is proxy-wide rather than address-specific;
- malformed/protocol/TLS failure on the proxy leg: stop;
- ambiguous CONNECT status: stop unless existing typed evidence proves candidate-specific failure.

## Plain HTTP forward-proxy limitation

For a normal HTTP origin through an HTTP forward proxy, Eggfetch sends an absolute-form logical URL and the proxy itself performs the origin connection. Standard forwarding does not provide a separate wire field where Eggfetch can safely tell an arbitrary proxy "connect this logical host to this exact IP" while preserving ordinary proxy semantics.

Therefore v1 must reject:

```text
HTTP origin + forward proxy + explicit proxied target pin
```

before network I/O.

Do not silently rewrite the absolute URI host to an IP and restore only the Host header: proxies may route/cache/authenticate based on the absolute URI authority, so that would alter logical origin semantics.

If a future concrete consumer needs pinned plaintext HTTP through a proxy, evaluate an explicit CONNECT-for-HTTP mode as a separate design. Do not smuggle it into this plan.

## SOCKS5 implementation

Native `socks5://` represents local target resolution semantics. With an explicit proxied target snapshot, skip local DNS and issue the SOCKS5 CONNECT command using one of the supplied IP addresses.

Required properties:

- no origin DNS lookup in pinned mode;
- destination port must match the logical origin effective port;
- candidate order preserved;
- fallback attempts share one request/connect budget;
- destination TLS, if the logical origin is HTTPS, uses the logical origin hostname after SOCKS connection establishment;
- HTTP Host remains logical.

The existing Eggfetch SOCKS client may be extended to accept an already-resolved target rather than being replaced. Keep timeout, metrics, connection metadata, local bind/socket options, and error taxonomy in one authoritative implementation.

## SOCKS5H limitation

Native `socks5h://` means the proxy resolves the target hostname. An explicit local physical target constraint conflicts with that contract.

For v1:

```text
SOCKS5H + explicit proxied target pin => pre-I/O error
```

Do not downgrade SOCKS5H to SOCKS5 silently. Do not send the IP while continuing to report remote-resolution semantics. Do not claim that authorizing a hostname proves the address selected by a remote resolver.

Callers that require local destination validation use SOCKS5 plus this feature, or own the entire route through an external dialer.

## Proxy peer pinning is independently required

An ultimate target pin does not imply a trusted proxy socket. If the effective `Proxy` has no pinned peer, Eggfetch may still resolve the proxy endpoint normally. That is legitimate general client behavior, but callers that require complete physical-route control must configure both:

```text
Proxy::... + pinned proxy peer addresses
RequestBuilder::... + pinned proxied target addresses
```

Document this explicitly. Do not make ultimate-target pinning automatically require proxy-peer pinning for ordinary users; instead expose enough connection metadata/diagnostics that security-sensitive callers can verify their configuration. A stricter combined convenience constructor may be considered only if it stays generic and does not create policy concepts.

## Redirect semantics

The proxied target snapshot is origin-specific.

Required v1 behavior:

- same `(scheme, host, effective port)` redirect under the same effective proxy may preserve the snapshot;
- a redirect changing host, scheme, or effective port must not reuse it;
- a cross-origin redirect from a pinned proxied request should fail closed before dispatch unless the caller explicitly supplied a new route for the new origin through a future API;
- a redirect that changes proxy selection must re-evaluate both proxy-peer and target routing state;
- auth/cookie/proxy-auth stripping rules remain unchanged and operate on logical origins, not physical IPs.

Avoid building a general per-origin route map in v1.

## Retry semantics

Retry reconstructs the same logical request with the same immutable target snapshot and the same proxy configuration snapshot. No origin DNS lookup is introduced on later attempts.

If the request body is not replayable, existing retry rules remain authoritative. Route pinning does not make a body replayable.

Total timeout continues shrinking across retries/redirects; address fallback does not restart it.

## Pool and route identity

Audit connection reuse particularly carefully for CONNECT and SOCKS tunnels.

A request for logical `service.example:443` pinned to `203.0.113.10:443` must not reuse a tunnel/connection that was established for the same logical origin through ordinary proxy DNS or through `203.0.113.11:443` unless route equivalence is proven by the cache key.

At minimum the physical route identity for a reusable proxied connection must distinguish:

```text
logical proxy identity
physical proxy peer snapshot (if supplied)
logical origin identity
physical proxied target snapshot (if supplied)
proxy protocol / resolution mode
relevant TLS route state
```

Do not over-key logical concurrency permits if the current pool intentionally limits by origin; physical connection compatibility and logical request admission are separate concerns.

If the current proxy path does not pool tunnels, retain that simpler behavior and document/test it instead of introducing a new pool solely for this feature.

## SNI, Host, and explicit override interaction

The feature must compose with existing logical identity behavior.

Default:

```text
physical CONNECT/SOCKS target = caller-supplied IP
TLS SNI/cert name              = logical origin hostname
HTTP Host                      = logical origin authority
```

If Eggfetch already supports an explicit SNI override on this route, that explicit override remains authoritative; route pinning does not invent or overwrite it.

Host/request-target overrides must not be inferred from the physical IP. Any existing override incompatibility should fail using current rules rather than being silently normalized.

## Error and observability behavior

Use existing native detailed failure introspection where typed evidence exists. Route-sensitive diagnostics should distinguish the stage of failure without exposing downstream policy concepts:

- proxy peer connect/TLS;
- proxy auth/CONNECT protocol;
- specific pinned tunnel target attempt;
- SOCKS negotiation/target refusal;
- destination TLS;
- request/response I/O.

Do not add an EggSec-style authorization checkpoint enum. Do not make transient failed candidates survive as the terminal `RequestFailure` classification after a later candidate succeeds.

Connection metadata should continue reporting truthful logical/wire state. If the existing response metadata cannot expose the physical ultimate peer for proxied tunnels, document that limitation rather than fabricating it; adding an optional generic physical-route snapshot may be considered if it is useful to more than this feature and does not leak credentials.

## Egress interaction

Do not import Egress for this work.

`eggress-routing` can decide which route should be used at the application layer, but its rule/upstream/health model must not become Eggfetch request state. `eggress-protocol-socks` can establish SOCKS streams, but adopting it does not remove the need for Eggfetch's proxy-peer/tunnel identity, timeout, retry, TLS, and pool integration.

When an application already wants Egress to own the complete tunnel or chain, it can use Eggfetch's custom `Dialer` seam instead of the built-in proxy path. This plan improves Eggfetch's own proxy route for callers that want Eggfetch to own proxy HTTP/SOCKS semantics.

## Tests

Add deterministic local coverage for at least:

1. HTTPS CONNECT to an unresolvable logical origin succeeds through a local proxy when a pinned target IP is supplied;
2. proxy observes CONNECT to the supplied IP/port rather than the logical hostname;
3. TLS inside the tunnel uses logical origin SNI and validates the logical certificate name;
4. HTTP Host and `Response::url()` remain logical;
5. empty target set fails before I/O;
6. target port mismatch fails before I/O;
7. IPv6 CONNECT authority formatting is correct;
8. first target fails and second succeeds only for typed candidate-specific failures under one budget;
9. no origin DNS fallback after all pinned CONNECT candidates fail;
10. HTTPS-proxy + pinned proxy peer + pinned CONNECT target preserves both independent TLS identities;
11. SOCKS5 uses supplied target IP without origin DNS;
12. SOCKS5 multiple-target fallback follows one deadline;
13. SOCKS5H + target pin fails before I/O;
14. HTTP forward-proxy + HTTP origin + target pin fails before I/O;
15. same-origin redirect preserves the target snapshot;
16. cross-origin redirect fails before dispatch rather than using stale target state;
17. retry preserves the exact target snapshot;
18. incompatible target snapshots cannot cross-reuse a tunnel/connection;
19. ordinary unpinned HTTP proxy, CONNECT, SOCKS5, and SOCKS5H behavior remains unchanged;
20. direct `RequestBuilder::resolved_addresses()` continues rejecting effective proxy combinations exactly as before.

Where practical, instrument local proxy fixtures to record the exact CONNECT/SOCKS destination. Do not infer physical binding solely from a successful response.

## Expected files

Likely implementation surface:

```text
crates/eggfetch-core/src/request.rs
crates/eggfetch-core/src/proxy.rs
crates/eggfetch-core/src/pipeline.rs
crates/eggfetch-core/src/transport/proxy.rs
crates/eggfetch-core/src/transport/socks.rs
crates/eggfetch-core/src/client.rs            # only if cache/route identity requires it
crates/eggfetch-core/src/error.rs             # only for compatible typed validation/provenance
crates/eggfetch-core/tests/*proxy*
crates/eggfetch-core/tests/*socks*
docs/architecture/core-tls-proxy-protocols.md
docs/rust/guide.md
docs/reference/errors.md                      # if structured failure docs change
```

Avoid Python/CLI/HTTPX changes unless shared native plumbing requires internal propagation with no public exposure.

## Required validation

Focused checks:

```sh
cargo fmt --all -- --check
cargo test -p eggfetch-core --features proxy
cargo clippy -p eggfetch-core --all-targets --features proxy -- -D warnings
cargo tree -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
```

Then:

```sh
./scripts/check.sh
```

Before child-plan completion, also run all existing direct resolved-target, custom dialer, proxy TLS, SOCKS, retry, redirect, timeout, detailed-error, and connection-reuse tests affected by the route selection changes.

## Acceptance criteria

- [ ] A native caller can explicitly pin the physical ultimate target of an HTTPS CONNECT request while preserving the logical origin URL/Host/TLS identity.
- [ ] A native caller can explicitly pin the physical ultimate target of a SOCKS5 request without origin DNS fallback.
- [ ] Existing direct `resolved_addresses()` remains direct-only and unchanged.
- [ ] SOCKS5H ultimate-target pinning fails closed before I/O.
- [ ] Plain HTTP forward-proxy ultimate-target pinning fails closed before I/O in v1.
- [ ] Multiple supplied targets use a conservative typed fallback policy under one shared deadline.
- [ ] Retry and same-origin redirects preserve the target snapshot; cross-origin redirects cannot reuse it.
- [ ] Pool/tunnel reuse cannot violate physical route constraints.
- [ ] Proxy and destination TLS identities remain logically correct when both proxy peer and ultimate target are pinned.
- [ ] Existing unpinned proxy/SOCKS behavior and compatibility facades are unchanged.
- [ ] No new production dependency and no Egress dependency is introduced.
