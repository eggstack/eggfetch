# Release and Verification Roadmap

Status: active — M001/M002 blocked on core-transport M006 corrective

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
unexplained; wheel matrix builds 3.10–3.15 pending rehearsal dispatch;
issue #24 publication and the Python 3.15 rehearsal are temporarily blocked by
core-transport M006, a supported-Windows HTTPS response-completeness
corrective.

## 5. Target architecture

Publication and rehearsal complete with evidence; afterwards this workstream
holds the gates steady and processes corrective requalifications. No standing
automation growth.

## 6. Dependency graph

```text
core-transport M006 corrective + exact-SHA Stage C renewal
    |
    +--> M001 release publication (operational: maintainer)
    `--> M002 wheel rehearsal (operational: maintainer; must use corrected SHA)
M003 standing corrective intake (soft; ongoing)
```

## 7. Milestones

### Milestone 1 — 0.2.x coordinated publication

Class: infrastructure (operational). Status: blocked on core-transport M006.

Implementation plan:

- `plans/implementation/release-verification/001-release-publication.md`

Legacy context: `plans/issue-24-release-qualification-and-closure.md`
(implementation + qualification complete; publication pending),
`docs/releases/process.md`.

Objective: manual `cargo publish` in leaf order, tag, PyPI dispatch, with
Tier 2 + Tier 3 + live preflight from a trusted local environment.

Exit conditions: published versions verified; binding untouched unless
executable inputs changed (then requalify first).

### Milestone 2 — Python 3.15 wheel production rehearsal

Class: capability (packaging). Status: blocked on core-transport M006.

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

## 11. Completion definition

M001/M002 close with their evidence artifacts; M003 remains standing
process, never "complete."

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 publication | blocked | `plans/implementation/release-verification/001-release-publication.md` | — | Core-transport M006 closure + renewed Stage C binding |
| M002 wheel rehearsal | blocked | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | — | Core-transport M006 closure; run from corrected implementation SHA |
| M003 intake | proposed | — | — | — |
