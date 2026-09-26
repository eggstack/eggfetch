# Release and Verification Roadmap

Status: active — M001A ready; M002 and M001B dependency-blocked

Long-term references:

- `plans/000-long-term-specification.md` §6, §7
- `plans/001-terminology-and-domain-model.md` §10, §11
- `plans/002-long-term-roadmap.md` Phases 1–3

Related ADRs:

- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`

Normative policy: `docs/verification-policy.md`. Workflow:
`.skills/verification-qualification.md`.

## 1. Purpose and ownership boundary

Owns validation and release integrity: Tier 1/2/3 gates, API oracles,
packaging checks, security preflight, publication order, wheel matrix, and
the live Stage C binding renewal process.

Consumes: engine/adapter evidence from other subsystems. Must not own:
product behavior changes, CI redesign (one automatic job; additions need
explicit approval + regression history), publish automation.

## 2. Work classification

### Invariants

- `scripts/check.sh` is the single validation source; CI repeats it.
- Completed plans non-normative; historical qualification never gates.
- Publication manual; no workflow publishes, tags, or authorizes a SHA.
- Live binding exact-SHA; docs-only descendants do not invalidate.

### Capabilities

- Reproducible crates.io publication order; 18-wheel + 1-sdist PyPI matrix
  with build-only rehearsal mode.

### Infrastructure

- Oracle tooling (pinned `cargo-public-api`/`cargo-semver-checks`/nightly),
  manifest generators, compat profiles, coverage validators.

### Polish

- Release notes, classifier/coverage alignment, operator docs.

## 3. Non-goals

- New CI jobs/matrices/evidence schemas without explicit request.
- Behavior changes disguised as release work; H3/Node graduation claims.

## 4. Current state

Tier 1/2/3 + security preflight defined and green on the live freeze;
six-profile oracle vs the oracle baseline; both API oracles zero
unexplained; wheel matrix builds 3.10–3.15 pending rehearsal dispatch.
Core-transport M006 and M006C1 are closed. The native-Windows EggFetch
matrix passed 19/19 (run 36193582776), and the corrected EggReplay M013F
Windows 300 KiB proof passed on commit `5efc6f9c892bb5b1c2e84330a0c40a38f1de0de7`
(run 36211265347). Stage C remains bound to
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`. The historical `v0.2.0` tag/release already exists at an older commit, so
release work is decomposed around a fresh coordinated `0.2.1` identity. M001A
prepares the exact candidate; M002 rehearses the Python matrix from that SHA;
M001B performs publication.

## 5. Target architecture

Publication and rehearsal complete with evidence; afterwards this workstream
holds the gates steady and processes corrective requalifications. No standing
automation growth.

## 6. Dependency graph

```text
core-transport M006C2 post-closure reconciliation (closed)
    |
    v
M001A 0.2.1 candidate preparation (ready)
    |
    v
M002 Python 3.15 wheel rehearsal (blocked on M001A)
    |
    v
M001B coordinated 0.2.1 publication (blocked on M002)

M001 umbrella closes after M001B.
M003 standing corrective intake remains soft/ongoing.
```

## 7. Milestones

### Milestone 1 — 0.2.x coordinated publication

Class: infrastructure (operational). Status: ready (decomposed).

Umbrella plan:

- `plans/implementation/release-verification/001-release-publication.md`

Child sequence:

1. `plans/implementation/release-verification/001a-0-2-1-release-candidate-preparation.md`
2. `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md`
3. `plans/implementation/release-verification/001b-0-2-1-coordinated-publication.md`

The existing `v0.2.0` tag points at an older release commit and is immutable
history. The next coordinated identity is `0.2.1`; do not move/reuse v0.2.0.

Exit conditions: one exact 0.2.1 candidate is prepared and qualified, M002
rehearses that same SHA, six crates are published in dependency order, signed
`v0.2.1` targets that candidate, PyPI publishes from that tag through OIDC,
and external registry smoke passes.

### Milestone 2 — Python 3.15 wheel production rehearsal

Class: capability (packaging). Status: blocked on M001A.

Implementation plan:

- `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md`

Legacy detail: `plans/python-3.15-pypi-wheel-production.md`
(implementation complete, qualification pending).

Objective: `publish=false` rehearsal from the exact M001A 0.2.1 candidate;
prove 18 wheels + 1 sdist, including Linux/macOS/Windows Python 3.15 rows,
before M001B publication.

Exit conditions: 19 distributions assemble; Tier 1 + package green; bounded
3.15-only prerelease fallback documented.

### Milestone 3 — Standing corrective intake

Class: process. Status: proposed (standing).

Objective: route each future corrective through a bounded plan + closure +
renewed binding where applicable. Repeated correctives in one subsystem
force a roadmap revision.

## 8. Cross-cutting requirements

Security preflight live before publication; Trusted Publishing (OIDC) for
PyPI; clean worktree for packaging; no `--allow-dirty`.

## 9. Verification strategy

Tier 2 + Tier 3 + `check_security.sh` for M001; Tier 1 + package + rehearsal
artifacts for M002; per-corrective gates for M003.

## 10. Risks and decision points

Risk: publication pressure skipping rehearsal. Control: M002 gate is
explicit — no 3.15 claim without rehearsal evidence.

Risk: publishing before the mandatory Windows qualification is complete.
Control: M006C1 now records both the direct native-Windows EggFetch matrix
and corrected EggReplay M013F hosted-Windows 300 KiB rerun. Continue to follow
the existing maintainer-run publication and wheel-rehearsal procedures.

## 11. Completion definition

M001/M002 close with their evidence artifacts; M003 remains standing
process, never "complete."

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 umbrella | ready (decomposed) | `plans/implementation/release-verification/001-release-publication.md` | `plans/closure/release-verification/001-coordinated-0-2-1-publication.md` | M001A → M002 → M001B |
| M001A 0.2.1 candidate prep | **ready** | `plans/implementation/release-verification/001a-0-2-1-release-candidate-preparation.md` | `plans/closure/release-verification/001a-0-2-1-release-candidate-preparation.md` | — |
| M002 Python 3.15 rehearsal | blocked | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | `plans/closure/release-verification/002-python-315-wheel-rehearsal.md` | M001A closure |
| M001B 0.2.1 publication | blocked | `plans/implementation/release-verification/001b-0-2-1-coordinated-publication.md` | `plans/closure/release-verification/001b-0-2-1-coordinated-publication.md` | M002 closure |
| M003 intake | proposed | — | — | — |
