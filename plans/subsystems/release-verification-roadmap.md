# Release and Verification Roadmap

Status: active — M001 publication and M002 wheel rehearsal ready
post-M006C2

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
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`. Issue #24 publication (M001)
and the Python 3.15 wheel rehearsal (M002) are ready to resume under their
existing release procedures.

## 5. Target architecture

Publication and rehearsal complete with evidence; afterwards this workstream
holds the gates steady and processes corrective requalifications. No standing
automation growth.

## 6. Dependency graph

```text
core-transport M006C2 post-closure reconciliation (closed)
    |
    +--> M001 release publication (ready)
    `--> M002 wheel rehearsal (ready)
M003 standing corrective intake (soft; ongoing)
```

## 7. Milestones

### Milestone 1 — 0.2.x coordinated publication

Class: infrastructure (operational). Status: ready; M006C2 is closed.

Implementation plan:

- `plans/implementation/release-verification/001-release-publication.md`

Legacy context: `plans/issue-24-release-qualification-and-closure.md`
(implementation + qualification complete; publication pending against
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`),
`docs/releases/process.md`.

Objective: manual `cargo publish` in leaf order, tag, PyPI dispatch, with
Tier 2 + Tier 3 + live preflight from a trusted local environment.

Exit conditions: published versions verified; binding untouched unless
executable inputs changed (then requalify first).

### Milestone 2 — Python 3.15 wheel production rehearsal

Class: capability (packaging). Status: ready; M006C2 is closed.

Implementation plan:

- `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md`

Legacy detail: `plans/python-3.15-pypi-wheel-production.md`
(implementation complete, qualification pending).

Objective: `publish=false` 18-wheel rehearsal from the implementation
commit; claim 3.15 support only after.

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
| M001 publication | ready | `plans/implementation/release-verification/001-release-publication.md` | — | — |
| M002 wheel rehearsal | ready | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | — | Dispatch from authoritative Stage C SHA |
| M003 intake | proposed | — | — | — |
