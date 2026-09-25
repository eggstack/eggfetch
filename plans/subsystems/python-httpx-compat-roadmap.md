# Python Bindings and HTTPX Compatibility Roadmap

Status: closed; M001–M003 closed on the live Stage C binding

Long-term references:

- `plans/000-long-term-specification.md` §4.2, §4.6, §5, §6, §8
- `plans/001-terminology-and-domain-model.md` §5, §8
- `plans/002-long-term-roadmap.md` Phase 0

Related ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`

## 1. Purpose and ownership boundary

Owns the Python surface: sync/async adapters over the engine, shared request
preparation, native API/typing manifests, the neutral `_ssl_context`
interop, streaming bridge, SSE framing, wsproto-over-101 WebSocket, and the
two versioned compat facades with their Stage C evidence and residuals.

Consumes: engine behavior, trust translation rules, timeout/pool semantics.
Must not own: transport policy, publication, downstream migration code, Trio/
AnyIO support, Python trailer exposure.

## 2. Work classification

### Invariants

- Shared setup in `request_preparation.rs`; `content=` takes one iterator;
  async-only bodies rejected before sync dispatch.
- `_native` private; public surface = `__init__.py` + `__all__` + manifests
  in sync.
- Facades coexist without cross-mutation; generated manifests never
  hand-edited.
- Timeouts map only `connect`/`read`/`write`/`pool`; never synthesize native
  `total`. Compat `NO_PROXY` parser intentionally not unified with native.
- `ssl.SSLContext` translation fail-closed; proxy TLS only from
  `Proxy(ssl_context=...)`.
- `Http2Only` enforced at three layers; never fork a second engine.
- Only 101 responses own `network_stream`; CONNECT tunnels body-only.
- `aread()` builds final bytes in the GIL bridge; buffered iterators lazy.

### Capabilities

- Native sync/async clients, streaming iterators, network-stream wrappers.
- `eggfetch.compat.httpx` (0.28.1) and `eggfetch.compat.httpx2` (2.12.0)
  facades with SSE (+ optional WS on httpx2).

### Infrastructure

- Single runtime-neutral Python→core request mapping; versioned private
  SSLContext contract; relational mirror guardrails.

### Polish

- Typing surface, stub/member/return drift correction, manifest tooling.

## 3. Non-goals

- HTTPX 1.0 target (gated future); Trio/AnyIO; trailers; new auth schemes;
  downstream-specific types; raw sockets from Python.

## 4. Current state

Native API, neutral SSL boundary, async-body bridge, PyO3/Python matrix, PEP
561 surface, relational guardrails, and both facades qualified Stage C on the
live freeze. See `plans/httpx-parity-correction-status.md`,
`compat/*/profile.toml`, `docs/residual-differences.md`.

## 5. Target architecture

The §2 invariants hold permanently. Facade behavior changes only through new
milestone plans with renewed exact-SHA qualification; residuals stay
documented.

## 6. Dependency graph

```text
M001 HTTPX 0.28.1 Stage C (hard predecessor for M002 methodology)
    |
    +--> M002 httpx2 2.12.0 facade + SSE/WS (soft after M001)
    |
    `--> M003 native surface / typing / SSL hardening (soft after M001)
```

All closed.

## 7. Milestones

### Milestone 1 — HTTPX 0.28.1 Stage C facade

Class: capability + invariant. Status: closed.

Legacy: `plans/httpx-parity-correction-roadmap.md` and phase/corrective
passes (`httpx-parity-correction-phase-*.md`,
`httpx-parity-corrective-*.md`, final-closure passes), renewed through the
maintenance and private-architecture requalifications.

Exit conditions: pinned suites + API oracles green on the freeze; residuals
recorded. Met; binding renewed on the live freeze.

### Milestone 2 — httpx2 2.12.0 sibling facade

Class: capability. Status: closed.

Legacy: `plans/httpx2-2.12-profile-and-delta-baseline.md`,
`plans/httpx2-2.12-core-facade-parity.md`,
`plans/httpx2-2.12-sse-and-websocket-parity.md`.

Exit conditions: independent Stage C on the same freeze; FunctionAuth,
Origin/QUERY/operators/SSE/optional-WS covered. Met.

### Milestone 3 — Native surface, typing, and SSL hardening

Class: infrastructure + polish. Status: closed.

Legacy: `plans/python-interop-api-hygiene-program.md`,
`plans/python-interop-boundary-and-async-body.md`,
`plans/python-pep561-typing-surface.md`,
`plans/python-pep561-method-contract-corrective-pass.md`,
`plans/post-python-typing-corrective-qualification-and-closure.md`,
`plans/python-ssl-context-private-contract-hardening.md`,
`plans/python-request-dispatch-consolidation.md`,
`plans/python-native-surface-relational-guardrails.md`,
`plans/python-streaming-and-cli-private-decomposition.md`,
`plans/native-request-failure-introspection.md`,
`plans/native-tower-service-adapter.md` (+ tonic-0.14 corrective),
`plans/native-http-body-interoperability.md`,
`plans/tls-crypto-provider-extensibility.md`,
`plans/tls-additional-trust-anchors.md`.

Exit conditions: manifests/typing/SSL/dispatch hardened with zero semantic
drift; exact-SHA requalification green. Met.

## 8. Cross-cutting requirements

Secret redaction at every Python boundary; `TransportHints` wire-only and
redirect-cleared per §4 of the terminology; codec/jar semantics unchanged
without their own milestone.

## 9. Verification strategy

Native manifest + relational guardrails + typing surface (Tier 1); full
pinned compat suites with `EGGFETCH_COMPAT_REQUIRED=1` + oracles
(zero unexplained) + six-profile Rust oracle (Tier 2); three consecutive
passes for binding renewal per legacy practice.

## 10. Risks and decision points

Risk: facade drift from upstream HTTPX. Control: pinned references,
regenerated manifests, residuals file; HTTPX 1.0 work gated on the roadmap
trigger, never speculative.

## 11. Completion definition

Closed: both facades Stage C on the live binding with accepted closure
evidence and no unresolved medium-or-higher findings. Reopens only via a new
milestone plan (e.g. gated HTTPX 1.0 program).

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 httpx 0.28.1 Stage C | closed | legacy parity plans §7 | legacy + live ledger | — |
| M002 httpx2 2.12.0 facade | closed | legacy httpx2 plans §7 | legacy + live ledger | — |
| M003 native/typing/SSL | closed | legacy plans §7 | legacy closures | — |
