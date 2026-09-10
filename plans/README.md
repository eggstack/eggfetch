# eggfetch Plan Index

This directory contains active implementation plans, live qualification/status records, and historical implementation records. Completed plans are non-normative unless another current document explicitly says otherwise. Verification and release policy remain governed by `docs/verification-policy.md` and `docs/releases/process.md`.

## Active program — HTTP/3 graduation and next HTTPX compatibility (2026-09-10)

Handoff program: `http3-and-next-httpx-compatibility-program.md`

Planning baseline: `4456680361dbfdc2d15b0d43a18cdcb20704f667`.
The existing HTTPX 0.28.1 Stage C qualification remains bound to executable SHA `d034a1005857a7f403222dda4bda5f2f204a44fe`. These planning/index commits are documentation-only and do not invalidate it. Once any qualification-sensitive implementation/test/validation work below begins, the resulting executable tree is unqualified until the final closure plan freezes and requalifies it.

Execute in order:

1. `http3-alt-svc-discovery-fallback-and-draining.md` — add authenticated Alt-Svc discovery, safe fallback/suppression, and H3 drain/reconnect semantics.
2. `http3-interoperability-and-production-graduation.md` — dependency-risk review, cross-implementation/impairment/resource evidence, and objective supported-vs-experimental decision.
3. `httpx2-2.12-profile-and-delta-baseline.md` — create the independent HTTPX2 2.12.0 reference/profile and generalize existing oracle machinery only where required.
4. `httpx2-2.12-core-facade-parity.md` — implement non-SSE/WebSocket HTTPX2 deltas while preserving HTTPX 0.28.1 semantics.
5. `httpx2-2.12-sse-and-websocket-parity.md` — implement SSE over streamed responses and optional WebSockets over the existing 101 network-stream path.
6. `httpx-1.0-preview-tracking.md` — maintain a non-qualified preview/delta record for original HTTPX 1.0 dev releases; no implementation promise until RC/stable.
7. `post-next-scope-compatibility-requalification-and-closure.md` — freeze one executable SHA; requalify HTTPX 0.28.1; independently qualify HTTPX2 2.12.0 to the earned stage; record H3 graduation outcome.
8. `post-next-scope-documentation-and-plan-hygiene.md` — documentation-only truth pass and closure/index cleanup after qualification.

Plans 1–6 may change source/tests/dependencies/qualification tooling and must finish before the freeze. Plan 7 is the exact-SHA evidence boundary. Plan 8 must remain documentation/profile/ledger-only.

Key scope decisions:

- HTTP/3 graduation does not require 0-RTT, WebTransport, H3 datagrams, MASQUE, or connection migration.
- `eggfetch.compat.httpx` remains the independently pinned HTTPX 0.28.1 facade.
- `httpx2==2.12.0` is the stable next-family compatibility target and should receive a sibling versioned facade/profile rather than mutating 0.28.1 behavior.
- original `httpx==1.0.dev*` is preview/reconnaissance only until an RC/stable public contract exists.
- no new routine CI workflow/matrix/evidence system is authorized by these plans; reuse Tier 1/2/3 and existing qualification infrastructure.

## Normative live HTTPX 0.28.1 compatibility record

`httpx-parity-correction-status.md` remains the live exact-SHA HTTPX 0.28.1 compatibility status/evidence ledger until the new program's final closure updates it. The current profile is `compat/httpx/0.28.1/profile.toml`.

The HTTPX2 baseline plan will establish its own versioned profile/status record. Do not treat HTTPX2 evidence as automatically modifying the HTTPX 0.28.1 claim.

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