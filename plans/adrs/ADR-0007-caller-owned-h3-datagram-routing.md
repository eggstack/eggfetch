# ADR-0007: Caller-Owned Fixed-Target Datagram Routing for Experimental HTTP/3

Status: accepted

Date: 2026-10-05

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md` §3, §4.1, §4.3, §5, §8
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 items 2 and 4

Affected subsystem roadmaps:

- `plans/subsystems/tls-proxy-protocols-roadmap.md`

## Context

Eggfetch's experimental HTTP/3 transport correctly owns origin QUIC and H3:
TLS trust, SNI, ALPN, QUIC endpoint/session lifecycle, origin caching,
Alt-Svc selection, GOAWAY/draining, retry/fallback commit semantics, and H3
request/response framing all live in `eggfetch-core`.

The existing public `Dialer` is a byte-stream seam for H1/H2 and cannot
carry QUIC. QUIC requires a datagram transport, and tunneling origin QUIC
through a TCP-like `Dialer` would be architecturally invalid.

A downstream route provider now exposes a listener-free fixed-target UDP
association with bounded send/receive and explicit lifecycle. The motivating
example is Eggress v1.0.11's `OutboundConnector::associate_udp`, which can
provide direct or single-hop SOCKS5 UDP routing while hiding proxy framing
from its caller. Eggfetch MUST NOT depend on Eggress or absorb its route
policy; the API must remain generic enough for any caller-owned datagram
transport.

Quinn 0.11.11 exposes `Endpoint::new_with_abstract_socket` and the
`AsyncUdpSocket` runtime boundary. Exposing Quinn's trait directly would,
however, leak Eggfetch's current QUIC implementation into the Rust public API
and force embedders to implement Quinn-specific batching/readiness metadata.

The existing public-surface containment policy forbids adjacent transport
helpers without an explicit architecture decision. This ADR is that decision.

## Decision drivers

- Keep Eggfetch the sole owner of origin QUIC/H3/TLS semantics.
- Let embedders control only the physical datagram route.
- Preserve fixed-target/proxy-side-DNS use cases without requiring a local DNS
  result from Eggfetch.
- Never route QUIC through the byte-stream `Dialer`.
- Do not expose Quinn, `h3`, `quinn-udp`, SOCKS, Eggress, or MASQUE types in
  the new public contract.
- Preserve strict `Http3Only` and the existing safe pre-commit
  `Auto { allow_http3: true }` fallback rules.
- Preserve direct H3 as the unchanged fast/default experimental path when no
  datagram route is installed.
- Keep all queues, worker tasks, packet metadata adaptation, and Quinn socket
  implementation private and bounded.
- Do not change HTTPX/Python/CLI/FFI/Node surfaces merely to expose the Rust
  transport hook.

## Considered options

### Option A — Expose a caller-owned fixed-target datagram route (selected)

Add a narrow Rust-only route contract alongside `Dialer`:

- a client-scoped datagram dialer receives a logical/physical UDP target
  expressed with the existing `DialTarget` host/port shape;
- it returns one fixed-target datagram route;
- the route exposes bounded asynchronous send/receive plus truthful local and
  physical-peer socket addresses;
- Eggfetch privately adapts that route to Quinn's `AsyncUdpSocket`;
- Eggfetch continues to own QUIC configuration, TLS, H3, connection/session
  caching and HTTP policy.

The exact public names may follow the existing `Dialer` family, but the
semantic contract above is normative.

### Option B — Expose Quinn's `AsyncUdpSocket` publicly

Rejected. It couples Eggfetch's public API to a specific QUIC implementation,
leaks Quinn's batching/ECN/readiness types, and makes a future QUIC-engine
change an application API migration.

### Option C — Add an Eggress/QUIC-specific connector

Rejected. Eggfetch cannot depend on a downstream route product, and
Eggress's `quic://` / `h3://` transports are proxy-hop byte-stream
transports rather than origin-H3 datagram routes.

### Option D — Extend the existing byte-stream `Dialer`

Rejected. QUIC packet boundaries, loss semantics and path state cannot be
represented as an `AsyncRead + AsyncWrite` byte stream.

### Option E — Implement CONNECT-UDP/MASQUE inside Eggfetch

Rejected. RFC 9298 is a proxy protocol, not an origin-H3 transport primitive.
If a route provider later supports CONNECT-UDP it may implement this generic
datagram contract without moving MASQUE policy into Eggfetch.

## Decision

Select Option A.

The new route is **fixed-target**. The provider chooses how to reach the host
and port and may resolve the hostname locally, remotely, through a proxy, or
through another overlay. After establishment, each send carries exactly one
UDP payload to that fixed target and each receive returns one UDP payload from
that route. No per-send destination API is exposed.

Eggfetch receives two address facts from the established route:

- a local socket address used for Quinn endpoint/path bookkeeping;
- a physical peer address used as the stable Quinn path peer.

The physical peer may be a proxy relay rather than the logical origin and MUST
NOT be reused as SNI, Host, cookie/auth origin, redirect identity, or
certificate-verification identity. Eggfetch retains those logical-origin
authorities.

For routed H3, Eggfetch creates a private Quinn abstract socket over the route.
The first implementation may use bounded send/receive worker queues to bridge
the caller's asynchronous datagram API to Quinn's poll/try socket contract.
Those queues and Quinn types remain private. Backpressure MUST surface as
`WouldBlock`/readiness, not unbounded buffering.

The routed path advertises conservative UDP behavior:

- no GSO/GRO contract;
- no ECN contract;
- one datagram per route send/receive;
- QUIC minimum/initial MTU 1200;
- path MTU discovery disabled initially for the opaque routed path;
- direct H3 retains the existing Quinn socket and existing MTU discovery.

A route that cannot carry at least the QUIC minimum 1200-byte payload must
fail closed before the QUIC handshake.

The custom datagram route is client-scoped and immutable after client
construction, matching the existing `Dialer` ownership model. The H3 cache
must never silently reuse a direct QUIC connection for a client configured
with a datagram route, or vice versa.

For `Http3Only`, route establishment or route I/O failure is returned; there
is no direct-UDP fallback.

For `Auto { allow_http3: true }`, only the existing pre-commit/replayability
rules may fall back from H3 to H1/H2. H1/H2 routing remains independently
owned by the byte-stream `Dialer`/proxy/direct configuration. Installing only
a datagram dialer does not imply a global all-protocol routing policy.
Callers that require one physical route for every protocol must configure
both stream and datagram routing.

## Consequences

### Positive

- Eggfetch gains a transport-neutral H3 route injection seam without leaking
  Quinn.
- Existing listener-free UDP providers can integrate without becoming QUIC or
  H3 owners.
- Proxy-side DNS remains possible because a custom route receives a hostname
  and Eggfetch need not locally resolve it before route establishment.
- Origin TLS/H3 policy remains centralized in Eggfetch.
- Future CONNECT-UDP/MASQUE, VPN, overlay, or test transports can implement
  the same fixed-target datagram contract.

### Negative

- The Rust public API oracle changes and exact-SHA compatibility
  requalification is required.
- The private Quinn adapter adds bounded queue/task lifecycle that requires
  explicit cancellation/backpressure tests.
- Routed H3 diagnostics need careful wording because Quinn's path peer may be
  a proxy relay, not the origin.
- The first routed path deliberately gives up ECN/GSO/GRO and PMTU discovery,
  trading performance for a small, auditable contract.

### Neutral or deferred

- H3 remains experimental. This ADR does not satisfy the independent interop,
  impairment, public-origin, or upstream-risk graduation gate.
- QUIC migration/rebinding, 0-RTT, WebTransport, H3 application datagrams and
  MASQUE remain outside Eggfetch scope.
- Per-request datagram-route overrides are deferred; the first contract is
  client-scoped only.
- Downstream route-provider publication/versioning remains downstream work.

## Compatibility and migration

No existing caller changes unless it opts into the new Rust-only hook. Direct
H3, H1/H2, proxy, SOCKS, Python, CLI, FFI and Node behavior remain unchanged.

The new symbols are feature-gated with experimental H3/advanced routing and
must be recorded by the Rust public API oracle. No new default feature is
authorized.

Because executable/public API inputs change, the live HTTPX Stage C binding
must be renewed on the final implementation SHA even though the compatibility
facades do not expose this hook.

## Security and reliability implications

- Route errors must be bounded/redacted; Eggfetch must never log arbitrary
  route configuration or credentials.
- Route failure must never silently instantiate a direct UDP socket.
- Route send/receive queues are bounded.
- Dropping/closing a connection must cancel bridge workers and release the
  caller route deterministically.
- The physical peer address is diagnostic/path metadata only and must not
  influence origin identity or trust.
- A route provider's send/receive futures must be cancellation-safe under
  ordinary future drop; Eggfetch must document this contract.
- Direct and routed H3 cache generations must not cross-contaminate.
- Routed H3 must preserve total/connect/read/write deadline ownership and must
  not create a second retry engine.

## Verification

Required evidence:

- compile-time/public API tests for the new route contract;
- a deterministic in-repo fixed-target datagram-route test double with no
  external dependency;
- H3 GET/POST/streaming/trailers/multiplexing over the custom route;
- route-side DNS ownership proof (Eggfetch does not call local DNS for a
  custom-routed H3 target);
- `Http3Only` fail-closed/no-direct-fallback proof;
- `Auto` pre-commit fallback proof with an independently configured stream
  dialer;
- route cancellation, queue saturation/backpressure, connection eviction,
  GOAWAY/reconnect, client drop and route-drop accounting;
- direct-H3 non-regression including existing multi-address fallback;
- MTU=1200 / no-PMTUD routed-path assertions;
- diagnostic proof that route peer metadata cannot replace logical-origin
  identity;
- Tier 1 + Tier 2/MSRV + Rust API oracle + exact-SHA Stage C rebinding;
- Tier 3 package validation because the Rust public surface changes.

## Supersession

None. This is a narrow public-surface and route-ownership exception under
ADR-0005. It does not authorize unrelated transport helpers or H3 graduation.
