# eggfetch Plan Index

This directory contains active implementation plans, live qualification/status records, and historical implementation records. Completed plans are non-normative unless another current document explicitly says otherwise. Verification and release policy remain governed by `docs/verification-policy.md` and `docs/releases/process.md`.

## Completed program — HTTP/3 graduation and next HTTPX compatibility (2026-09-11)

Handoff program: `http3-and-next-httpx-compatibility-program.md`
(completed 2026-09-11 on frozen executable SHA
`65beb675a5380d3ff4291da6833b91ebf12c769a`).

HTTPX 0.28.1 renewed Stage C and HTTPX2 2.12.0 earned Stage C on that
SHA (evidence: `httpx-parity-correction-status.md`, the live ledger).
HTTP/3 retained experimental with blockers; HTTPX 1.0 remains
preview-only. Child plans below are historical records:

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