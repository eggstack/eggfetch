# eggfetch Plan Index

This directory contains active implementation plans, live qualification/status records, and historical implementation records. Completed plans are non-normative unless another current document explicitly says otherwise. Verification and release policy remain governed by `docs/verification-policy.md` and `docs/releases/process.md`.

## Completed qualification program — HTTP/3 production qualification (2026-09-11)

Handoff program: `http3-production-qualification-program.md`

Objective: close the blockers left by the prior HTTP/3 graduation attempt and make a new evidence-based decision on whether ordinary H3 operation can move from **experimental** to **supported**. This was a qualification/hardening program, not a feature-expansion program.

Execution is complete with a truthful retained-**experimental** outcome. The
child plans remain evidence records, but their unchecked external acceptance
items are not silently treated as satisfied. A future promotion attempt must
supply the missing evidence and perform a new executable freeze and
compatibility requalification.

Execution order:

1. `http3-independent-interop-and-impairment-qualification.md` — build/reuse one implementation-neutral H3 corpus; qualify against at least two independent non-Quinn servers; record public-origin evidence; run realistic loss/latency/reordering/UDP-block/MTU/address-family impairment; prove replay-safe fallback and independent drain/reconnect behavior.
2. `http3-upstream-risk-resource-and-observability-hardening.md` — audit the exact Quinn/h3/h3-quinn/rustls stack and current ordinary-client correctness issues; upgrade/work around only where justified; add targeted regressions; run lifecycle/resource soak; characterize cache pressure; improve real QUIC diagnostics without fabricated response metadata.
3. `http3-production-graduation-and-compatibility-requalification.md` — audit child-plan closure, freeze one exact executable SHA, run full repository/compatibility qualification, make the literal H3 graduation decision, renew HTTPX 0.28.1 and HTTPX2 2.12.0 Stage C on the new executable tree, and perform the documentation/descendant truth pass.

Plans 1 and 2 may overlap where implementation paths do not conflict, but both must close before plan 3 freezes the tree.

Final decision: HTTP/3 remains **experimental**. Deterministic controls and
repository gates are green, but independent non-Quinn interop, independent
GOAWAY/drain, public-origin, realistic impairment, and upstream-risk closure
evidence remain blockers. Missing evidence remains a blocker; it is never
converted into a pass.

The prior HTTPX 0.28.1 and HTTPX2 2.12.0 Stage C profiles were bound to
executable SHA `65beb675a5380d3ff4291da6833b91ebf12c769a`; that binding is
now historical because this program changed tests and qualification tooling.
The profiles were renewed on frozen executable SHA
`639bf186a71c054e11278d1b160ffe7a6f172c02` after three consecutive full
compatibility passes and clean API oracles.

Explicitly out of scope for this program: 0-RTT, WebTransport, H3 datagrams, MASQUE/CONNECT-UDP, connection migration, and automatic H3-by-default policy changes.

The current machine-readable evidence ledger is
`http3-independent-interop-and-impairment-qualification-evidence.json`.
The corpus and impairment contracts are under `../qualification/http3/`;
they are qualification inputs, not routine CI gates. The ledger retains the
experimental label until external evidence satisfies the parent gate.

## Completed program — HTTP/3 graduation and next HTTPX compatibility (2026-09-11)

Handoff program: `http3-and-next-httpx-compatibility-program.md`
(completed 2026-09-11 on frozen executable SHA
`65beb675a5380d3ff4291da6833b91ebf12c769a`).

HTTPX 0.28.1 and HTTPX2 2.12.0 had renewed Stage C results on that SHA
(evidence: `httpx-parity-correction-status.md`), but those results are now
historical because the active qualification program changed executable tests
and validation tooling. They were superseded by the current qualification on
`639bf186a71c054e11278d1b160ffe7a6f172c02`. HTTP/3 retained experimental with
blockers; HTTPX 1.0 remains preview-only. Child plans below are historical
records:

1. `http3-alt-svc-discovery-fallback-and-draining.md` — done.
2. `http3-interoperability-and-production-graduation.md` — done
   (experimental retained with blockers; valid outcome).
3. `httpx2-2.12-profile-and-delta-baseline.md` — done.
4. `httpx2-2.12-core-facade-parity.md` — done.
5. `httpx2-2.12-sse-and-websocket-parity.md` — done.
6. `httpx-1.0-preview-tracking.md` — done (preview-only).
7. `post-next-scope-compatibility-requalification-and-closure.md` — done.
8. `post-next-scope-documentation-and-plan-hygiene.md` — done.

## Normative live compatibility records

`httpx-parity-correction-status.md` remains the live exact-SHA status for
both facades until a future program updates it. Profiles:
`compat/httpx/0.28.1/profile.toml`, `compat/httpx2/2.12.0/profile.toml`.
Preview: `compat/httpx/1.0-preview/` (unqualified by design; status in
`preview-status.toml`, delta in `preview-delta-0.28.1-vs-1.0.dev6.json`,
notes in `redesign-notes.md`).

### HTTPX 1.0 migration trigger

A future HTTPX 1.0 implementation/qualification program opens only when all
three hold (recorded here and in `plans/ROADMAP.md`; never by renaming the
preview plan into a Stage C plan):

1. upstream publishes an RC with an explicitly frozen public API, or a
   stable 1.0 release;
2. release notes indicate no further major compatibility reset before stable;
3. a fresh delta inventory shows the target is stable enough to justify
   implementation, pinned to the exact RC/stable release.

## Recently completed program — post-audit maturation (2026-09-09)

`post-audit-architecture-and-surface-maturation-program.md` executed and closed:

1. `core-request-and-transport-consolidation.md` — done (`0477d35`)
2. `http3-lifecycle-and-policy-hardening.md` — done (`a505b6c`)
3. `node-binding-maturation.md` — done, experimental outcome (`8139e10`)
4. `native-protocol-observability-and-api-cleanup.md` — done (`363f2e3`)
5. `post-maturation-httpx-requalification-and-closure.md` — done, Stage C renewed on `d034a1005857a7f403222dda4bda5f2f204a44fe`
6. `post-maturation-documentation-and-plan-hygiene.md` — done, documentation-only

The prior Corrective 08 and broad truth-refresh plans remain historical evidence for earlier executable states.

## Historical plans

Other plan files in this directory record prior milestones, corrective passes, validation work and release preparation. Consult them for design history and prior acceptance criteria, but they are not automatically current requirements.

Plan files stay in place; this index is the authoritative navigation layer.
