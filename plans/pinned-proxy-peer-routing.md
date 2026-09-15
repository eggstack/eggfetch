# Pinned Proxy Peer Routing

Status: Complete

Date: 2026-09-14

Parent: [`proxy-route-pinning-and-egress-interoperability-program.md`](proxy-route-pinning-and-egress-interoperability-program.md)

Depends on: current native proxy subsystem and completed direct `ResolvedTarget` work

Baseline: `d681da848e36199846604b71f2162a1054d24aa6`

## Objective

Add a native, typed way for a caller to supply the already-resolved physical `SocketAddr` candidates for a configured proxy endpoint and require Eggfetch to connect only to those addresses, while preserving the proxy URL as the logical identity for proxy matching, authentication, HTTPS-proxy TLS SNI/certificate verification, error reporting, and user-visible configuration.

This is the proxy-peer analogue of direct `RequestBuilder::resolved_addresses()`. It is not a proxy policy engine and does not pin the ultimate origin; the second child plan owns that separate problem.

## Problem statement

Today `Proxy` / `ProxyConfig` carries the logical proxy URI, auth, proxy-only headers, routing rule, and optional proxy TLS configuration. The proxy transport then resolves the proxy host internally with `tokio::net::lookup_host` before dialing candidates.

That means a caller can pre-resolve and validate a direct origin, but cannot do the same for the first hop of a built-in proxy route. A security-sensitive caller can authorize `proxy.example:8443` as a hostname yet still has no API-level guarantee that the socket Eggfetch opens is one of the addresses it validated.

The desired invariant is:

```text
logical proxy URI:       https://proxy.example:8443
caller-approved peers:   [198.51.100.20:8443, 198.51.100.21:8443]
TCP remote:              one of the supplied peers only
proxy TLS SNI:           proxy.example
proxy cert validation:   proxy.example
proxy auth/rule identity: unchanged
DNS fallback:            forbidden while the pin is present
```

## API design

Prefer extending the existing public `Proxy` builder rather than adding a request `TransportHints` field.

Conceptual API:

```rust
let proxy = Proxy::all("https://proxy.example:8443")?
    .resolved_addresses([
        "198.51.100.20:8443".parse()?,
        "198.51.100.21:8443".parse()?,
    ])?;

let client = Client::builder()
    .proxy(proxy)
    .build();
```

Exact naming may change if another name is clearer (`proxy_addresses`, `resolved_proxy_addresses`), but it must be obvious that the addresses belong to the proxy endpoint rather than the origin.

### Validated address representation

Do not copy the direct connector's validation/candidate logic into a second independent model.

Preferred options, in order:

1. reuse the existing `ResolvedTarget` internally after broadening its documentation so it truthfully describes a validated physical destination set for one logical endpoint; or
2. extract a private `ResolvedEndpoint` / `StaticDestinations` representation shared by direct `ResolvedTarget` and the proxy model, retaining the public direct type unchanged.

Do not rename the public `ResolvedTarget` in 0.1.x merely for taxonomy. Do not add public fields.

### Construction errors

Reject before I/O:

- an empty supplied set;
- a `SocketAddr` whose port differs from the logical proxy URI's effective port;
- a proxy URI with no usable host/port;
- any unsupported scheme already rejected by normal proxy construction.

Preserve candidate order. A caller may intentionally order IPv6/IPv4 or service-discovery results.

## Internal state ownership

Add the pinned peer snapshot to `Proxy` and carry it into `ProxyConfig::config()` as private/internal state.

The snapshot must survive:

- client-level proxy configuration;
- per-request `ProxyOverride::Override` cloning;
- retry reconstruction;
- redirect hops when the same effective proxy still applies;
- proxy matching/routing rule evaluation.

It must not survive into a different proxy selected by another rule. It must not affect a request that matches `NO_PROXY` and takes the direct route.

`Debug` should report at most a count/presence summary, matching `ResolvedTarget`'s existing privacy posture. Socket addresses are not credentials, but there is no need to dump private topology into routine debug output.

## Connector implementation

Refactor `transport/proxy.rs::connect_to_proxy()` so candidate acquisition is one explicit step:

```text
if proxy has caller-supplied peers:
    candidates = exact snapshot
else:
    candidates = current lookup_host(proxy_host, proxy_port)

for candidate under the existing connect budget:
    apply current local-bind/socket behavior
    connect
    record existing metrics/failure provenance
```

The candidate loop, timeout budgeting, metrics, and failure handling should remain one implementation for pinned and system-resolved peers.

Do not implement pinning by rewriting the proxy URL host to an IP literal. That would corrupt HTTPS-proxy TLS identity and configuration semantics.

### HTTPS proxy identity

The physical socket and TLS server name must remain distinct:

```text
connect(198.51.100.20:8443)
TLS server name = proxy.example
certificate policy = proxy TLS config / existing roots
```

Existing `proxy_tls_config` replacement/augmentation semantics must be unchanged. Client identity and custom CA handling must continue to target the proxy TLS leg only.

## Timeouts and candidate fallback

All supplied peers share the existing proxy-connect phase budget. Do not give every candidate a fresh full connect timeout.

If the first candidate fails and the second succeeds, the request succeeds under one aggregate budget exactly as ordinary multi-address proxy DNS does today.

If all supplied candidates fail, return the ordinary proxy connection failure with the best available structured terminal provenance. Never fall back to DNS afterward.

## Retry semantics

Retries preserve the exact proxy-peer snapshot. A retry is another attempt under the same caller routing decision, not permission to re-resolve the proxy.

Regression test this directly: make the proxy hostname unresolvable or changeable, provide a working pinned local proxy listener, force a retry, and prove both attempts use the supplied peer without consulting DNS.

The program must not add automatic refreshing of caller-supplied peers. Callers that want new service-discovery results construct a new proxy/request.

## Redirect semantics

Proxy peer state belongs to the effective proxy, not the origin. Therefore a redirect may retain the peer snapshot only if the same `Proxy` remains effective for the redirected URL.

Required cases:

- same effective proxy: retain the pinned peer snapshot;
- `NO_PROXY` begins matching after redirect: drop the proxy route entirely and use normal direct semantics;
- another proxy rule selects a different proxy: use only that proxy's own peer snapshot;
- no effective proxy: never apply stale proxy peers to a direct connection.

Do not treat a redirect as permission to mutate a configured proxy object's physical route.

## Pool/client reuse and route identity

Audit every cached client/connection path used by HTTP, HTTPS CONNECT, and SOCKS proxies.

A request constrained to a supplied proxy-peer set must not reuse a pooled transport that was established to the same logical proxy through ordinary DNS or through a different supplied set unless the cache key proves physical-route equivalence.

If the current proxy paths do not retain reusable physical connections across requests, document and test that fact rather than adding unnecessary route-key machinery. If any path does cache by logical proxy only, extend the key with a stable physical-route identity derived from the ordered address snapshot.

Do not key logical request-concurrency policy solely by raw socket address; logical proxy identity and physical route identity are separate concerns.

## SOCKS interaction

Pinned proxy-peer routing applies equally to SOCKS5 and SOCKS5H because it controls the TCP connection **to the SOCKS server**, not the SOCKS target address.

Keep target-resolution semantics unchanged in this plan:

- SOCKS5 continues its existing local-target-resolution behavior;
- SOCKS5H continues delegating target resolution to the proxy.

The next child plan adds an explicit ultimate-target pin for SOCKS5 where enforceable.

Do not adopt `eggress-protocol-socks` as part of this change. The existing Eggfetch SOCKS path already integrates shared deadlines, metrics, connection metadata, local binding/socket options, and error handling. Swapping only the handshake is orthogonal and would obscure whether route pinning itself is correct.

## Custom Dialer and Egress interaction

A configured built-in proxy route and `ClientBuilder::dialer()` remain separate routing authorities. Do not make the dialer secretly implement or override proxy-peer pinning.

Applications that want Egress to own the whole chain should continue using the dialer path. Applications that want Eggfetch's native HTTP/SOCKS proxy implementation use this new built-in pinning control. There is no dependency or callback between them.

## Error classification

Use current `Error` and `send_detailed()` compatibility rules.

Where the proxy path exposes typed evidence, preserve route provenance so callers can distinguish at least:

- invalid pinned proxy configuration (pre-I/O validation);
- proxy peer DNS failure when no pin exists;
- proxy peer connection refusal/connect failure;
- proxy TLS failure;
- CONNECT/SOCKS/origin failure after the proxy peer was established.

Do not parse error strings. Do not change existing `Error::kind()` tokens unless a separately justified compatibility change is required.

## Tests

Add deterministic local tests covering at minimum:

1. proxy hostname has no DNS answer, supplied local proxy `SocketAddr` succeeds;
2. instrumented resolver/system-DNS failure proves static proxy-peer mode does not fall back to DNS;
3. empty peer set fails before I/O;
4. peer port mismatch fails before I/O;
5. first peer fails and second succeeds under one connect budget;
6. every peer fails -> connection error, no DNS fallback;
7. HTTP proxy peer pin works for an HTTP origin;
8. HTTP proxy peer pin works for HTTPS CONNECT;
9. HTTPS proxy connects to supplied IP while SNI/cert verification uses logical proxy hostname;
10. custom proxy CA / proxy TLS config behavior remains correct;
11. SOCKS5 server peer pin works;
12. SOCKS5H server peer pin works while target DNS remains remote;
13. retry preserves the same peer snapshot;
14. same-proxy redirect preserves the snapshot;
15. redirect into `NO_PROXY` does not apply stale proxy peers to the direct route;
16. two different peer snapshots for one logical proxy cannot cross-reuse an incompatible pooled transport;
17. unpinned proxy behavior remains unchanged;
18. `Debug`/error output does not expose credentials and does not gratuitously dump route topology.

Prefer local listeners and deterministic fixtures. Do not depend on public DNS or an external proxy service.

## Expected files

Likely implementation surface:

```text
crates/eggfetch-core/src/proxy.rs
crates/eggfetch-core/src/transport/proxy.rs
crates/eggfetch-core/src/pipeline.rs          # only if route persistence/keying needs it
crates/eggfetch-core/src/client.rs            # only if proxy client cache identity lives here
crates/eggfetch-core/src/error.rs             # only if typed pre-I/O validation needs an existing-compatible variant
crates/eggfetch-core/tests/*proxy*
docs/architecture/core-tls-proxy-protocols.md
docs/rust/guide.md
```

Do not touch Python/CLI compatibility code unless an existing shared construction path mechanically requires carrying the native field without exposing it.

## Required validation

Focused first:

```sh
cargo fmt --all -- --check
cargo test -p eggfetch-core --features proxy
cargo clippy -p eggfetch-core --all-targets --features proxy -- -D warnings
cargo tree -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
```

Then the repository's normative Tier 1 check:

```sh
./scripts/check.sh
```

Before marking the child plan complete, also run affected proxy, TLS, retry, redirect, detailed-error, direct-static-routing, and custom-dialer regressions explicitly.

## Acceptance criteria

- [x] A native caller can pin one or more physical proxy peers without changing the logical proxy URI.
- [x] Pinned mode performs no proxy-host DNS lookup and never falls back to one.
- [x] HTTP, HTTPS CONNECT, HTTPS-proxy TLS, SOCKS5, and SOCKS5H proxy-server connections use the supplied peer snapshot where applicable.
- [x] HTTPS proxy SNI/certificate verification remains logical-host based.
- [x] Candidate fallback shares the existing connect budget.
- [x] Retry preserves the exact snapshot.
- [x] Redirect/proxy-rule transitions cannot leak a stale peer constraint into a different route.
- [x] Pool/client reuse cannot violate the physical route constraint.
- [x] Existing unpinned proxy and direct `resolved_addresses()` behavior is unchanged.
- [x] No new production dependency is introduced.
- [x] No Egress dependency or EggSec-specific policy surface is introduced.

## Implementation record

Implemented on executable freeze
`13c4ab4`. The public `Proxy` builder now
accepts an ordered, validated peer snapshot; pinned dialing skips proxy-host
DNS and preserves logical proxy identity. HTTP, HTTPS CONNECT, HTTPS-proxy,
SOCKS5, and SOCKS5H proxy-peer paths consume the snapshot, while route-aware
SOCKS cache keys prevent incompatible reuse. Local proxy/TLS fixtures cover
unresolvable logical proxy names, identity separation, pre-I/O validation, and
topology-safe debug output. No production dependency or Egress policy surface
was added. The broader program closure record documents the shared validation
evidence and its explicit optional skips.
