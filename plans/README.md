# eggfetch Plan Index

This directory contains active implementation plans, live qualification/status records, and historical implementation records. Completed plans are non-normative unless another current document explicitly says otherwise. Verification and release policy remain governed by `docs/verification-policy.md` and `docs/releases/process.md`.

## Post-audit maturation program (complete, 2026-09-09)

Handoff program `post-audit-architecture-and-surface-maturation-program.md`
executed in order and closed:

1. `core-request-and-transport-consolidation.md` — done (`0477d35`)
2. `http3-lifecycle-and-policy-hardening.md` — done (`a505b6c`)
3. `node-binding-maturation.md` — done, experimental outcome (`8139e10`)
4. `native-protocol-observability-and-api-cleanup.md` — done (`363f2e3`)
5. `post-maturation-httpx-requalification-and-closure.md` — done, Stage C
   renewed on `d034a1005857a7f403222dda4bda5f2f204a44fe`
6. `post-maturation-documentation-and-plan-hygiene.md` — this index,
   roadmap, and guide refresh (documentation-only)

No active implementation plan remains. Future work is triggered by a new
pinned HTTPX version, a newly discovered concrete compatibility defect,
or an intentionally expanded scope.

## Normative live compatibility record

`httpx-parity-correction-status.md` is the live exact-SHA compatibility status/evidence ledger referenced by repository guidance. The compatibility profile is stored under `compat/httpx/0.28.1/`.

## Recently completed closure work

The prior qualification/documentation sequence is retained as historical evidence:

- `httpx-parity-corrective-08-post-hardening-requalification-and-closure.md`
- `documentation-broad-truth-refresh-after-requalification.md`

Those plans qualified/documented the earlier executable state; the
post-audit maturation program above supersedes them for the current tree.

## Historical plans

Other plan files in this directory record prior milestones, corrective passes, validation work and release preparation. They should be consulted for design history and prior acceptance criteria, but they are not automatically current requirements.

Plan files stay in place; this index is the authoritative navigation layer (no archival moves, no broken links).
