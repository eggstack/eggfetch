# TLS, Proxy, and Protocols Roadmap

Status: closed with H3 experimental retained; graduation gated

Long-term references:

- `plans/000-long-term-specification.md` §4.4, §5, §8
- `plans/001-terminology-and-domain-model.md` §6, §7
- `plans/002-long-term-roadmap.md` Phase 0 + Phase 4 item 2

Related ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0004-proxy-fallback-total-deadline.md`
- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`

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

- H3 graduation (gated); 0-RTT, WebTransport, datagrams, MASQUE, migration;
  automatic H3-by-default; environment-proxy activation; upstream proxy
  helpers where eggfetch contracts would regress.

## 4. Current state

Proxy framing back under Hyper with safe reuse; typed fallback; route
pinning for proxy peers and ultimate destinations; H3 hardened (bounded
cache, multi-address fallback, phase-correct timeouts, keepalive idle,
GOAWAY drain) and qualified as retained-experimental. Graduation blockers
recorded in `docs/architecture/core-tls-proxy-protocols.md` and the
machine-readable evidence ledger.

## 5. Target architecture

Invariants permanent. H3 graduates only through the gated program with fresh
executable freeze + full requalification; until then every H3 change keeps
the experimental label and must not convert missing evidence into a pass.

## 6. Dependency graph

```text
M001 proxy Hyper pooling + upstream reuse (hard predecessor for M002)
    |
    +--> M002 route pinning + egress interop (soft after M001)
    |
    `--> M003 H3 hardening + retained-experimental qualification
              (soft after M001; independent evidence)
```

All closed; graduation is a gated future, not M004.

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

Closed: M001–M003 evidence accepted; H3 experimental with named blockers;
live Stage C binding unaffected. Graduation reopens only via the roadmap
Phase 4 trigger.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 proxy pooling/reuse | closed | legacy plans §7 | legacy closures | — |
| M002 route pinning/egress | closed | legacy program §7 | legacy closure | — |
| M003 H3 retained-experimental | closed | legacy program §7 | legacy + evidence ledger | Graduation evidence (gated future) |
