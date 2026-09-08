# eggfetch Plan Index

This directory contains active implementation plans, live qualification/status records, and historical implementation records. Completed plans are non-normative unless another current document explicitly says otherwise. Verification and release policy remain governed by `docs/verification-policy.md` and `docs/releases/process.md`.

## Active post-audit maturation program

Current handoff program: `post-audit-architecture-and-surface-maturation-program.md`

Execute in this order:

1. `core-request-and-transport-consolidation.md`
2. `http3-lifecycle-and-policy-hardening.md`
3. `node-binding-maturation.md`
4. `native-protocol-observability-and-api-cleanup.md`
5. `post-maturation-httpx-requalification-and-closure.md`
6. `post-maturation-documentation-and-plan-hygiene.md`

The first four plans are executable work. Do not repeatedly renew the exact-SHA HTTPX qualification while those plans are still changing executable/test/build/validation/package state. The fifth plan freezes and qualifies one final executable SHA using the existing HTTPX 0.28.1 Stage C procedure. The sixth plan must remain documentation/ledger/plan-hygiene only.

## Normative live compatibility record

`httpx-parity-correction-status.md` is the live exact-SHA compatibility status/evidence ledger referenced by repository guidance. The compatibility profile is stored under `compat/httpx/0.28.1/`.

## Recently completed closure work

The prior qualification/documentation sequence is retained as historical evidence:

- `httpx-parity-corrective-08-post-hardening-requalification-and-closure.md`
- `documentation-broad-truth-refresh-after-requalification.md`

Those plans qualified/documented the earlier executable state; later executable changes and the active maturation program supersede them for current handoff sequencing.

## Historical plans

Other plan files in this directory record prior milestones, corrective passes, validation work and release preparation. They should be consulted for design history and prior acceptance criteria, but they are not automatically current requirements.

The final plan in the active maturation program includes a documentation-only task to improve historical plan indexing/archive hygiene after the executable tree has been requalified.
