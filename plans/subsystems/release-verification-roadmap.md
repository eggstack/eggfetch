# Release and Verification Roadmap

Status: active — M001A/M002 closed; M004 tagged release finalization ready

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

Tier 1/2/3 + security preflight are defined on the live freeze; six-profile
oracle vs the oracle baseline and both API oracles have zero unexplained
delta. M001A closed on candidate `41757569123c0b8038550b956d8b244ab55094a6`.
M002 run `36223505399` then proved all 18 Python 3.10–3.15 wheels plus one
sdist with `publish=false` on that exact candidate.
Core-transport M006 and M006C1 are closed. The native-Windows EggFetch
matrix passed 19/19 (run 36193582776), and the corrected EggReplay M013F
Windows 300 KiB proof passed on commit `5efc6f9c892bb5b1c2e84330a0c40a38f1de0de7`
(run 36211265347). Stage C remains bound to
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`. The historical `v0.2.0` tag/release already exists at an older commit and
remains immutable. The unexecuted M001B publication plan is superseded by
M004 because a GitHub Release is now a required output and the candidate README
still carries a now-stale "3.15 wheels pending rehearsal" statement. M004
refreshes only release-facing docs, freezes a final docs-only candidate,
renews the exact-SHA build-only rehearsal, then performs publication and
closure.

## 5. Target architecture

Publication and rehearsal complete with evidence; afterwards this workstream
holds the gates steady and processes corrective requalifications. No standing
automation growth.

## 6. Dependency graph

```text
core-transport M006C2 post-closure reconciliation (closed)
    |
    v
M001A 0.2.1 candidate preparation (closed)
    |
    v
M002 Python 3.15 wheel rehearsal (closed)
    |
    v
M004 tagged 0.2.1 release finalization (ready; supersedes unexecuted M001B)

M001 umbrella closes after M004.
M003 standing corrective intake remains soft/ongoing.
```

## 7. Milestones

### Milestone 1 — 0.2.x coordinated publication

Class: infrastructure (operational). Status: ready (decomposed).

Umbrella plan:

- `plans/implementation/release-verification/001-release-publication.md`

Execution history / sequence:

1. `plans/implementation/release-verification/001a-0-2-1-release-candidate-preparation.md` — closed
2. `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` — closed
3. `plans/implementation/release-verification/001b-0-2-1-coordinated-publication.md` — superseded before execution
4. `plans/implementation/release-verification/004-0-2-1-tagged-release-finalization.md` — ready

The existing `v0.2.0` tag points at an older release commit and is immutable
history. The next coordinated identity is `0.2.1`; do not move/reuse v0.2.0.

Exit conditions: a truthful docs-clean final 0.2.1 candidate is frozen and
rehearsed on its exact SHA, six crates are published in dependency order,
signed `v0.2.1` targets that candidate, PyPI publishes 18 wheels + 1 sdist
from that tag through OIDC, a non-draft/non-prerelease GitHub Release exists
for the same tag, and external registry/install smoke passes.

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

### Milestone 4 — 0.2.1 tagged release finalization

Class: infrastructure / operational release. Status: ready.

Implementation plan:

- `plans/implementation/release-verification/004-0-2-1-tagged-release-finalization.md`

Objective: reconcile stale release-facing documentation, freeze a docs-only
final candidate, renew the exact-SHA wheel rehearsal, and complete coordinated
crates.io, signed tag, PyPI, GitHub Release, external smoke, and planning
closure. This supersedes the unexecuted M001B plan.

Exit conditions: all acceptance criteria in M004 pass and the M001 umbrella
closure is written.

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
| M001 umbrella | ready (finalization pending) | `plans/implementation/release-verification/001-release-publication.md` | `plans/closure/release-verification/001-coordinated-0-2-1-publication.md` | M004 |
| M001A 0.2.1 candidate prep | closed | `plans/implementation/release-verification/001a-0-2-1-release-candidate-preparation.md` | `plans/closure/release-verification/001a-0-2-1-release-candidate-preparation.md` | — |
| M002 Python 3.15 rehearsal | closed | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | `plans/closure/release-verification/002-python-315-wheel-rehearsal.md` | — |
| M001B 0.2.1 publication | superseded | `plans/implementation/release-verification/001b-0-2-1-coordinated-publication.md` | — | Replaced before execution by M004 |
| M004 0.2.1 tagged release finalization | **ready** | `plans/implementation/release-verification/004-0-2-1-tagged-release-finalization.md` | `plans/closure/release-verification/004-0-2-1-tagged-release-finalization.md` | — |
| M003 intake | proposed | — | — | — |
