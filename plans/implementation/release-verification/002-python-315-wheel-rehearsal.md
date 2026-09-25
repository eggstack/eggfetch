# Release and Verification Milestone 002 — Python 3.15 Wheel Rehearsal

Status: ready

Repository baseline: implementation commit of
`plans/python-3.15-pypi-wheel-production.md` (see that plan for the SHA)

Source roadmap:

- `plans/subsystems/release-verification-roadmap.md` §7 Milestone 2

Long-term requirements:

- `plans/000-long-term-specification.md` §7
- `plans/001-terminology-and-domain-model.md` §11
- `plans/002-long-term-roadmap.md` Phase 2

Applicable ADRs:

- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`

Primary class: infrastructure (packaging)

## 1. Objective

Prove the CPython 3.10–3.15 wheel matrix (18 wheels + 1 sdist = 19
distributions, Linux x86_64 / macOS arm64 / Windows x86_64) assembles via a
build-only rehearsal before any 3.15 support is claimed.

## 2. Why this milestone is ready

Matrix, coverage validator, classifier, and release documentation
implementation complete per the legacy plan; Tier 1 and package validation
green locally. Only the rehearsal dispatch is outstanding.

## 3. Current implementation evidence

`plans/python-3.15-pypi-wheel-production.md`: full matrix definition,
bounded 3.15-only prerelease fallback (selects stable 3.15.x after GA
without changing 3.10–3.14 behavior), validator/classifier/docs alignment.

## 4. Invariants that must not regress

- Rehearsal is `publish=false`; nothing uploads.
- 3.10–3.14 behavior unchanged by the 3.15 fallback bound.
- No executable change inside this milestone (packaging-only).

## 5. Scope

### In scope

- Dispatch `pypi.yml` with `publish=false` from the implementation commit.
- Verify 19-distribution assembly + wheel smoke.
- Record run ID and artifacts in the closure record.

### Explicitly out of scope

- Publishing (`publish=true` belongs to M001's dispatch).
- Claiming 3.15 support before rehearsal evidence lands.
- Matrix expansion beyond the documented platform set.

## 6. Required production changes

None. Workflow dispatch + evidence recording only.

## 7. Ordered work packages

### Work package A — Rehearsal dispatch

Intent: run the full build-only matrix from the exact implementation commit.

Required changes: dispatch only.

Acceptance evidence: workflow run ID; all 19 distributions assembled.

### Work package B — Smoke and closure

Intent: prove the artifacts install and import.

Required changes: wheel smoke per `scripts/wheel_smoke.py` and package
validators (Tier 3).

Acceptance evidence: smoke outputs + closure record; 3.15 claim unblocked.

## 8. Failure, cancellation, timeout, and pool semantics

A red platform leg MUST be recorded per-leg with logs; do not re-scope the
matrix to hide it. Re-dispatch (not config weakening) is the remedy.

## 9. Compatibility and migration

No runtime compatibility change. Python support stays 3.10–3.14 claimed
until this milestone closes; 3.15 prerelease fallback bounded as designed.

## 10. Required tests

Package validators + wheel smoke (Tier 3). No new test code.

## 11. Required verification commands

```bash
./scripts/check.sh           # Tier 1, local
./scripts/check.sh package   # Tier 3, local
# then: pypi.yml workflow_dispatch with publish=false (maintainer)
```

## 12. Documentation updates

Record the rehearsal run ID in the closure record and (on success) lift the
3.15 pending note in `plans/README.md` + `plans/registry.md`.

## 13. Acceptance criteria

- `publish=false` rehearsal green from the implementation commit; 19
  distributions assembled and smoke-tested.
- 3.15 support claimed only after this evidence exists.

## 14. Stop conditions

- Rehearsal red → stop and report per-leg; open corrective work, do not
  claim 3.15.
- Any temptation to dispatch `publish=true` to "test publishing" → stop;
  publication belongs to M001.

## 15. Closure evidence required

Workflow run ID, per-platform build results, smoke outputs, Tier 1 + package
results, and the follow-up 3.15-claim update (or its deferral with reason).

## 16. Handoff notes

Maintainer dispatch required. Python 3.15 was prerelease at implementation
time; verify whether GA has since occurred and note which interpreter the
fallback resolved.
