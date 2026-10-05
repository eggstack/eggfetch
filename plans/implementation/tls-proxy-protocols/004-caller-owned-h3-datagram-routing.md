# TLS, Proxy, and Protocols Milestone 004 — Caller-Owned H3 Datagram Routing

Status: ready

Repository baseline: `3993e6b447606d93a102320be901c08d012b0c34`

Source roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-4--caller-owned-h3-datagram-routing`

Long-term requirements:

- `plans/000-long-term-specification.md` §3, §4.1, §4.3, §5, §8
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 items 2 and 4

Applicable ADRs:

- `plans/adrs/ADR-0002-pipeline-route-finalize.md`
- `plans/adrs/ADR-0004-proxy-fallback-total-deadline.md`
- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`
- `plans/adrs/ADR-0007-caller-owned-h3-datagram-routing.md`

Primary class: capability

## 1. Objective

Add one Rust-only, client-scoped fixed-target datagram routing seam for
experimental HTTP/3 while preserving Eggfetch ownership of origin QUIC, TLS,
H3, Alt-Svc, timeout, retry/fallback, cache, and diagnostics semantics.

A downstream route implementation must be able to carry H3 UDP payloads
through an arbitrary fixed-target datagram path without implementing Quinn or
H3. The motivating published consumer seam is Eggress v1.0.11
`OutboundConnector::associate_udp`, but Eggfetch must not depend on Eggress
or mention Eggress types in production APIs.

This milestone does **not** graduate H3.

## 2. Why this milestone is ready

The architectural questions are closed by ADR-0007:

- Quinn 0.11.11 provides `Endpoint::new_with_abstract_socket` and an
  `AsyncUdpSocket` boundary suitable for a private adapter;
- the current H3 implementation already centralizes QUIC/TLS/session ownership
  in Eggfetch;
- direct H3 has bounded cache, deadline, drain and failure semantics that can
  remain authoritative;
- a fixed-target asynchronous datagram route is sufficient for downstream
  direct/SOCKS-style UDP routing;
- no Eggress code change is required for the first downstream route: published
  v1.0.11 already supplies fixed-target send/recv/lifecycle/local+relay address
  primitives;
- H3 graduation remains separately blocked and is not a dependency for adding
  this experimental routing seam.

## 3. Current implementation evidence

At the baseline:

- `H3Connector` owns one direct `quinn::Endpoint` and a bounded per-origin
  sender cache;
- direct H3 resolves every origin/Alt-Svc target locally and performs
  sequential multi-address fallback under one connect deadline;
- `ClientBuilder::dialer` controls only raw byte-stream H1/H2 routing;
- no caller-owned datagram route reaches H3;
- `Http3Only` therefore always uses direct UDP;
- `Auto { allow_http3: true }` discovers/uses H3 only through the direct
  Quinn endpoint;
- H3 diagnostics currently report Quinn's selected UDP peer as a
  `SocketAddr`;
- H3 remains experimental with the existing graduation blockers.

## 4. Invariants that must not regress

- Eggfetch-core remains the sole networking implementation.
- No Quinn/h3 type enters the public datagram-route API.
- No Eggress/downstream dependency is added.
- The existing byte-stream `Dialer` contract is unchanged.
- Direct H3 remains behaviorally unchanged when no datagram route is set.
- A configured datagram route never fails over to direct UDP.
- `Http3Only` stays strict.
- `Auto` fallback remains pre-commit + replayability gated and uses the
  existing stream route for H1/H2.
- Logical origin controls SNI, certificate verification, Host, cookies, auth,
  redirects and Alt-Svc trust. A route peer address never becomes origin
  identity.
- One-shot request bodies are never duplicated.
- Existing total/connect/write/read deadlines remain monotonic and owned by
  the existing pipeline.
- H3 sender/cache generation eviction remains pointer/generation safe.
- Default/lean profiles stay free of H3/datagram-route code.
- H3 remains experimental after this milestone.

## 5. Scope

### In scope

- one client-scoped Rust public datagram dialer/route contract;
- existing `DialTarget`, `DialError`, and `DialErrorKind` reused where
  semantically correct rather than inventing downstream-specific errors;
- private bounded adaptation to Quinn `AsyncUdpSocket`;
- private route endpoint/worker lifetime tied to an H3 connection generation;
- custom-route H3 for explicit `Http3Only`;
- custom-route H3 for authenticated Alt-Svc under `Auto`;
- route-peer/local address mapping into Quinn path bookkeeping;
- conservative routed-path MTU/readiness behavior;
- transport metrics/diagnostic truthfulness;
- focused route tests, public API oracle updates and exact-SHA requalification.

### Explicitly out of scope

- H3 graduation or support-tier promotion;
- changing direct H3 DNS/multi-address behavior;
- QUIC 0-RTT, migration/rebinding, WebTransport;
- H3 application datagrams or a public QUIC datagram API;
- CONNECT-UDP/MASQUE implementation;
- per-request datagram-route overrides;
- Python/HTTPX/CLI/FFI/Node exposure;
- a new retry engine or route-health policy;
- an Eggress dependency or built-in Eggress adapter;
- automatic CI/workflow expansion;
- publication/version bump.

## 6. Required production changes

### 6.1 Public route contract

Add the smallest object-safe Rust surface consistent with ADR-0007, preferably
alongside `transport::dialer`.

The contract must provide:

1. a client-scoped datagram dialer receiving a `DialTarget`;
2. an established fixed-target route object;
3. one-datagram asynchronous send;
4. one-datagram asynchronous receive into caller-provided bounded storage;
5. a local `SocketAddr`;
6. a stable physical-peer `SocketAddr`.

The physical peer may be a proxy relay. Documentation must explicitly forbid
treating it as the logical HTTP origin.

Prefer reusing `DialError`/`DialErrorKind` for route establishment and
route I/O unless implementation evidence proves a distinct error type is
required. Do not widen the public surface merely for internal convenience.

The public trait must not expose destination-per-send operations, Quinn
`Transmit`/`RecvMeta`, ECN, GSO/GRO or proxy-specific configuration.

Add one builder hook, representative shape
`ClientBuilder::datagram_dialer(...)`, gated so default and non-H3 profiles
do not acquire the API/dependencies.

### 6.2 Private Quinn socket bridge

Implement a crate-private adapter satisfying Quinn 0.11.11
`AsyncUdpSocket` over the asynchronous fixed-target route.

Requirements:

- bounded TX and RX queues;
- no allocation growth proportional to peer traffic;
- no unbounded spawned task count;
- `try_send` returns `WouldBlock` when the bounded route queue is full;
- the Quinn poller is woken when send capacity returns;
- `poll_recv` wakes only for queued data/terminal failure;
- route I/O errors terminate the corresponding H3 generation and remain
  classifiable without secret-bearing text;
- exactly one fixed peer is accepted; a Quinn transmit to another peer fails
  closed;
- one datagram per send/receive; advertise one segment for transmit/receive;
- ECN is not claimed; receive metadata uses no ECN;
- bridge `Debug` must not expose the route implementation/configuration;
- cancellation/drop aborts workers and releases the route.

The bridge may copy each datagram for this first experimental seam. Record the
copy/queue cost rather than introducing a public zero-copy abstraction.

### 6.3 Routed endpoint ownership

Do not replace the existing direct shared endpoint.

For a custom datagram route, create a route-owned abstract Quinn endpoint at
the H3 sender-generation boundary. The route and endpoint must live exactly as
long as the connection/driver generation requires and be released after
eviction/terminal close/drop.

A fresh reconnect after terminal failure or GOAWAY must establish a fresh
route through the datagram dialer. Do not reuse a stale route silently.

The existing logical-origin cache remains authoritative. If the current cache
key/generation fields cannot distinguish the direct/custom route state
safely, extend private identity explicitly; do not key by Display text or
physical relay address.

### 6.4 DNS and Alt-Svc routing

Direct H3 retains current local DNS + fair-share address fallback.

Custom-routed H3 must call the datagram dialer with the selected physical H3
target as host + port and must not pre-resolve that target merely to satisfy
Quinn. The route provider owns route resolution.

For Alt-Svc:

- the datagram target is the authenticated alternative host/port;
- SNI and certificate verification remain the logical origin exactly as today;
- Host/cookies/auth/redirect policy remain origin-bound;
- Alt-Svc generation/suppression/draining semantics remain unchanged.

### 6.5 Conservative routed QUIC policy

For the custom-routed path only:

- initial and minimum QUIC MTU = 1200;
- disable Quinn path-MTU discovery for the first implementation;
- configure endpoint accepted UDP payload size consistently with the bounded
  receive buffer;
- no ECN/GSO/GRO capability claims;
- no connection migration/rebinding.

If the route contract cannot carry a 1200-byte QUIC payload, fail route setup
before beginning the handshake.

Direct H3 keeps the current Quinn configuration.

### 6.6 Error and timeout mapping

Route establishment is part of the existing connect phase and consumes the
same monotonic connect deadline as QUIC + H3 initialization.

Do not create independent route retry/backoff.

Map route failures using typed caller evidence where available:

- route establishment/DNS/connect -> connect-class failure;
- explicit provider rejection/auth -> preserve available broad kind;
- send/receive terminal I/O -> H3 transport/stream failure without Display
  parsing;
- route timeout must not restart `Timeout.total` or `Timeout.connect`.

`TransportFailureKind` must remain evidence-based and unknown evidence must
remain unknown.

### 6.7 Auto fallback and route separation

Add explicit tests for:

- `Http3Only` + datagram route failure -> returned failure, zero direct UDP;
- `Auto` + datagram route + stream `Dialer`: safe pre-commit H3 failure may
  fall back through the stream dialer;
- one-shot body -> no H3-to-H1/H2 replay;
- datagram dialer only -> H1/H2 behavior is unchanged and documented as
  separately routed, not implicitly forced through the datagram provider.

## 7. Ordered work packages

1. Add focused compile tests for the intended public trait shape before
   modifying H3 internals.
2. Implement the public fixed-target datagram contract and builder ownership.
3. Build the private bounded Quinn socket bridge with synthetic/test route
   coverage.
4. Refactor H3 connector initialization just enough to support direct shared
   endpoint vs route-owned endpoint generations.
5. Wire explicit H3 and Alt-Svc H3 through the custom route.
6. Add failure/deadline/fallback/cache/drop/diagnostic tests.
7. Re-run all existing direct H3 qualification controls.
8. Update public API oracle and feature-profile fixtures.
9. Update architecture/docs and the H3 evidence narrative without changing
   the experimental graduation verdict.
10. Freeze the executable candidate and renew exact-SHA Stage C qualification.
11. Write `plans/closure/tls-proxy-protocols/004-status.md` and return the
    subsystem to steady state if evidence is green.

## 8. Failure, cancellation, timeout, and pool semantics

- Route dial + QUIC handshake + h3 initialization share one connect budget.
- Dropping a pending request must not leak route workers or a half-created
  endpoint.
- Connection-level route failure evicts only the current H3 generation.
- In-flight H3 streams retain their own connection/driver lifetime exactly as
  direct H3 does.
- Send-queue saturation is backpressure, not a retry signal.
- Receive-queue saturation must be bounded; choose a policy that cannot
  silently drop authenticated QUIC packets. Prefer pausing route receives
  until capacity exists.
- Worker panic/exit becomes terminal route failure and must wake Quinn.
- Client drop eventually releases every custom route and worker.
- Existing pool permits remain the logical concurrency authority; no second
  application connection pool is introduced.

## 9. Compatibility and migration

The API is additive and Rust-only but executable, so the current Stage C
binding is invalidated once implementation lands.

Required compatibility actions:

- update all affected Rust public API snapshots/oracle expectations;
- prove default, standard-http1, native-http1 and non-H3 profiles unchanged;
- prove existing `Dialer` source compatibility;
- run full HTTPX 0.28.1 + HTTPX2 2.12.0 qualification on one final executable
  SHA and renew the live binding;
- do not expose or emulate this API through Python/HTTPX facades.

No downstream migration is claimed until a released Eggfetch version contains
the hook.

## 10. Required tests

At minimum:

### Public/feature boundary

- trait is available only in intended H3/advanced profile;
- default/lean API snapshots unchanged;
- existing Dialer compile fixtures unchanged;
- a tiny external-style Rust fixture implements the new route without Quinn
  types.

### Quinn bridge

- one-datagram send/receive round trip;
- ordered queue mechanics without assuming UDP network ordering;
- send backpressure / wake after capacity;
- bounded receive queue with no silent packet drop;
- route terminal error wakes receive/send sides;
- wrong Quinn peer rejected;
- local/peer metadata exact;
- route/worker drop accounting;
- current-thread Tokio runtime coverage.

### H3 behavior

- explicit Http3Only GET;
- buffered POST;
- streaming upload/download;
- trailers;
- multiplexed concurrency;
- cancellation;
- GOAWAY/draining -> fresh route generation;
- server restart/reconnect;
- route failure then recovery;
- client drop and many-origin cache boundedness;
- Alt-Svc alternative target passed to datagram route while SNI stays origin;
- no local DNS lookup for custom-routed target;
- direct H3 multi-address fallback unchanged;
- route failure never opens direct UDP;
- Auto safe fallback uses configured stream dialer;
- one-shot body never duplicated.

### Policy/diagnostics

- routed transport config fixes min/initial MTU at 1200 and disables PMTUD;
- direct H3 retains existing MTU behavior;
- routed diagnostics report the physical route peer as path metadata while
  origin identity remains separate;
- redaction tests prove no route configuration/source Debug text escapes.

## 11. Required verification commands

Follow `docs/verification-policy.md` and
`.skills/verification-qualification.md`.

Focused first:

```bash
cargo test -p eggfetch-core --features http3
cargo test -p eggfetch-core --test h3_hardening --features http3
cargo test -p eggfetch-core --test h3_alt_svc_discovery --features http3
cargo test -p eggfetch-core --test h3_interop_qualification --features http3
```

Then:

```bash
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
./scripts/check_security.sh
```

Use the existing Tier 2 profile/API/Stage-C mechanisms. Do not create a new
automatic workflow or permanent network-dependent CI job.

External H3 graduation evidence remains optional/manual for this milestone and
cannot be claimed as passed unless actually run.

## 12. Documentation updates

Implementation must update:

- `docs/architecture/core-tls-proxy-protocols.md`;
- `docs/architecture/rust-surface-containment.md`;
- feature/profile documentation;
- native Rust client/transport documentation;
- H3 qualification/evidence narrative to distinguish direct vs custom-routed
  experimental H3;
- `plans/registry.md`, subsystem roadmap and `plans/README.md` at closure.

Documentation must state that a datagram route controls only H3 physical
transport and does not imply H1/H2 route policy.

## 13. Acceptance criteria

M004 closes only when:

- the public route API contains no Quinn/Eggress/proxy-specific types;
- the custom route can establish real H3 request/response traffic through the
  private Quinn abstract-socket bridge;
- all route queues/tasks are bounded and cancel/drop cleanly;
- custom routing does not perform mandatory local DNS for the target;
- route peer metadata never replaces origin TLS/HTTP identity;
- Http3Only is fail-closed with zero direct UDP fallback;
- Auto fallback remains commit/replayability safe and composes with the
  existing stream Dialer;
- direct H3 remains green and behaviorally unchanged;
- routed MTU policy is conservative and explicit;
- existing H3 graduation verdict remains experimental;
- Rust public API oracle/semver checks pass;
- Tier 1, Tier 2/MSRV and Tier 3 pass on the frozen executable SHA;
- both HTTPX compatibility profiles are rebound to that exact SHA;
- closure evidence records test counts, public API delta, feature graph,
  known limitations and downstream handoff.

## 14. Stop conditions

Stop and revise ADR-0007 rather than improvising if:

- Quinn's abstract socket cannot be driven correctly without exposing Quinn
  types publicly;
- a bounded worker bridge necessarily drops packets or creates unbounded
  allocation/task growth;
- Quinn requires the logical origin SocketAddr to equal the route's physical
  peer in a way that breaks proxy-side DNS/SNI separation;
- preserving direct-H3 behavior would require a second H3 implementation;
- the route hook requires Eggfetch to depend on Eggress or another downstream
  routing product;
- a public per-request policy layer is required to make the client-scoped
  contract correct.

## 15. Closure evidence required

Create `plans/closure/tls-proxy-protocols/004-status.md` containing:

- implementation commit(s) and final frozen SHA;
- final public symbol/API-oracle delta;
- feature graph before/after;
- requirement-to-test evidence matrix;
- routed-vs-direct H3 test results;
- queue/task/resource/cancellation evidence;
- timeout/fallback/error-classification evidence;
- exact Tier 1/2/3 outcomes;
- renewed HTTPX Stage C binding evidence;
- known limitations;
- explicit confirmation H3 is still experimental;
- downstream handoff stating only that a published future version may be
  adopted by route providers after their own qualification.

## 16. Handoff notes

The implementation should not add Eggress as a dev or production dependency.
Use an in-repo fixed-target datagram test double to prove the contract.

For the motivating Eggress v1.0.11 consumer, the intended downstream mapping
is straightforward but downstream-owned:

- datagram dial -> `OutboundConnector::associate_udp(host, port)`;
- route send/recv -> `UdpAssociation::send/recv`;
- local address -> `UdpAssociation::local_addr`;
- physical peer -> SOCKS `relay_addr` for proxied UDP;
- dropping/closing the route -> `UdpAssociation::close`.

Eggress direct UDP is not needed for Eggfetch's direct H3 fast path; downstream
consumers may use it only when they intentionally want all H3 traffic to pass
through the same route abstraction.
