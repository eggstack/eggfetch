# TLS, Proxy, and Protocols Roadmap

Status: active — M004 caller-owned H3 datagram routing ready; H3 graduation remains gated

Long-term references:

- `plans/000-long-term-specification.md` §4.4, §5, §8
- `plans/001-terminology-and-domain-model.md` §6, §7
- `plans/002-long-term-roadmap.md` Phase 0 + Phase 4 item 2

Related ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0004-proxy-fallback-total-deadline.md`
- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`
- `plans/adrs/ADR-0007-caller-owned-h3-datagram-routing.md`

## 1. Purpose and ownership boundary

Owns transport security and connectivity: TLS configuration and trust,
CONNECT/SOCKS proxying and pooling, H1/H2 negotiation, experimental H3/QUIC,
Alt-Svc discovery, UDS and dialer transports shared with the core
workstream, and the ProxyOverride/`NoProxy` model.

Consumes: pipeline stages, pool/timeout ownership. Must not own: facade
semantics, publication, a parallel connection pool, downstream route policy.

## 2. Work classification

### Invariants

- `additional_ca_*` augments; `ca_certificate_*` replaces. Fallback to
  WebPKI on construction failure only; verification failure never retries
  another store.
- Typed proxy fallback; no request-total state in reusable connectors.
- Forward proxying H1-only; H3 never bypasses proxy selection.

### Capabilities

- HTTP forwarding, CONNECT tunneling, SOCKS5, proxy auth, per-request
  override, `NO_PROXY` bypass, resolved proxy peers/destinations.
- H1/H2 with three-layer `Http2Only`; experimental H3 behind `http3`.

### Infrastructure

- Hyper-owned forward/CONNECT reuse; per-route SOCKS pools; bounded H3
  per-origin cache; authenticated Alt-Svc cache with suppression/drain.

### Polish

- Diagnostics counter semantics; route/client ownership documentation.

## 3. Non-goals

- H3 graduation (gated); 0-RTT, WebTransport, H3 application datagrams,
  MASQUE, QUIC migration; automatic H3-by-default; environment-proxy
  activation; upstream proxy helpers where eggfetch contracts would regress.
- M004 permits only fixed-target transport datagrams beneath origin QUIC; it
  does not expose QUIC DATAGRAM or an application datagram API.

## 4. Current state

Proxy framing back under Hyper with safe reuse; typed fallback; route
pinning for proxy peers and ultimate destinations; H3 hardened (bounded
cache, multi-address fallback, phase-correct timeouts, keepalive idle,
GOAWAY drain) and qualified as retained-experimental. Graduation blockers
recorded in `docs/architecture/core-tls-proxy-protocols.md` and the
machine-readable evidence ledger. The direct-only H3 route boundary is now
reopened narrowly by ADR-0007/M004 so embedders can supply a fixed-target
physical datagram route without becoming QUIC/H3 owners.

## 5. Target architecture

Invariants permanent. H3 graduates only through the gated program with fresh
executable freeze + full requalification; until then every H3 change keeps
the experimental label and must not convert missing evidence into a pass.
M004 adds one client-scoped datagram-routing contract below QUIC while keeping
origin QUIC/TLS/H3 in Eggfetch and preserving the direct-H3 path when unset.

## 6. Dependency graph

```text
M001 proxy Hyper pooling + upstream reuse (hard predecessor for M002)
    |
    +--> M002 route pinning + egress interop (soft after M001)
    |
    `--> M003 H3 hardening + retained-experimental qualification
              (soft after M001; independent evidence)
              |
              `--> M004 caller-owned H3 datagram routing
                    (hard: M003; interface: ADR-0007 + Quinn 0.11.11 abstract socket)
```

M001–M003 are closed. M004 is ready by explicit maintainer direction and
accepted ADR-0007. H3 graduation remains a separate gated future and is not an
M004 exit condition.

## 7. Milestones

### Milestone 1 — Proxy Hyper pooling and upstream reuse

Class: infrastructure. Status: closed.

Legacy: `plans/proxy-hyper-pooling-and-upstream-reuse.md`,
`plans/post-audit-maintenance-security-and-proxy-modernization-program.md`
(proxy track), `plans/security-policy-release-and-supply-chain-hardening.md`.

Exit conditions: framing under Hyper with reuse; TLS/pinning/timeout/auth/
error contracts preserved; no parallel pool. Met.

### Milestone 2 — Proxy route pinning and egress interoperability

Class: capability. Status: closed.

Legacy: `plans/proxy-route-pinning-and-egress-interoperability-program.md`,
`plans/pinned-proxy-peer-routing.md`,
`plans/pinned-proxy-destination-routing.md`,
`plans/post-proxy-route-pinning-qualification-and-closure.md`.

Exit conditions: `Proxy::resolved_addresses()` +
`RequestBuilder::proxy_target_addresses()`; no DNS fallback; no Egress
dependency; exact-SHA requalification. Met.

### Milestone 3 — H3 hardening and retained-experimental qualification

Class: capability (experimental) + infrastructure. Status: closed with
retained-experimental verdict.

Legacy: `plans/http3-production-qualification-program.md`,
`plans/http3-post-freeze-diagnostics-requalification-corrective-closure.md`,
`plans/http3-independent-interop-and-impairment-qualification.md`,
`plans/http3-upstream-risk-resource-and-observability-hardening.md`,
`plans/http3-production-graduation-and-compatibility-requalification.md`,
`plans/http3-alt-svc-discovery-fallback-and-draining.md`,
`plans/http3-lifecycle-and-policy-hardening.md`,
`plans/http3-interoperability-and-production-graduation.md`.

Exit conditions: hardening landed; qualification attempted with truthful
retained-experimental outcome; blockers named, not waived. Met as a valid
negative graduation verdict.

### Milestone 4 — Caller-owned H3 datagram routing

Class: capability. Status: ready.

Implementation:
`plans/implementation/tls-proxy-protocols/004-caller-owned-h3-datagram-routing.md`.

Objective: add one Rust-only client-scoped fixed-target datagram route beneath
experimental H3, privately adapted to Quinn, without exposing Quinn or
downstream route-product types.

Dependencies:

- hard: M003 H3 hardening/retained-experimental closure;
- interface: accepted ADR-0007;
- interface: Quinn 0.11.11 abstract-socket API already present in the H3
  dependency line;
- downstream Eggress is not a dependency.

Deliverable boundary: custom-routed H3 works through an arbitrary fixed-target
datagram route; direct H3 remains unchanged; H3 remains experimental.

User/integration value: embedders can route origin HTTP/3 over a listener-free
UDP provider (for example a SOCKS5 UDP association) while Eggfetch retains TLS,
SNI, H3 and retry/fallback authority.

Exit conditions: bounded/cancel-safe bridge, no direct-UDP fallback when the
route is configured, direct-H3 non-regression, public API oracle update,
Tier 1/2/3 green, exact-SHA Stage C renewal, closure record accepted.

Deferred: per-request route override, MASQUE/CONNECT-UDP, QUIC migration,
0-RTT, H3 application datagrams and graduation.

## 8. Cross-cutting requirements

Trust construction explicit per profile; `Proxy(headers=…)` rides the proxy
leg only; proxy setup uses one monotonic deadline; redaction at every
boundary; `crypto_provider()` per-config.

## 9. Verification strategy

Proxy/TLS/route matrices; H3 deterministic loopback controls in the normal
suite; independent-server/impairment/public-origin evidence manual and
opt-in, never routine gates; Tier 2 + oracles on any executable change.

## 10. Risks and decision points

Risk: silent H3 capability growth. Control: graduation gate + experimental
label rule; any promotion attempt is a new gated milestone, not a corrective.

## 11. Completion definition

M001–M003 evidence is accepted. While M004 is open, the subsystem is active.
M004 completion requires a renewed exact-SHA Stage C binding because it changes
executable/public Rust inputs. After M004 closure the subsystem may return to
closed/steady state with H3 still experimental. Graduation remains controlled
by the separate Phase 4 evidence trigger.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 proxy pooling/reuse | closed | legacy plans §7 | legacy closures | — |
| M002 route pinning/egress | closed | legacy program §7 | legacy closure | — |
| M003 H3 retained-experimental | closed | legacy program §7 | legacy + evidence ledger | Graduation evidence (gated future) |
| M004 caller-owned H3 datagram routing | **ready** | `implementation/tls-proxy-protocols/004-caller-owned-h3-datagram-routing.md` | pending | ADR-0007 accepted; no external blocker |
