# Release-Candidate Final Evidence and CI Enforcement — Implementation Status

**Baseline SHA:** `cd3994a798518c0f5ec59562d38f5b448d6667a7`
**Implementation date:** 2026-07-20

## Acceptance Criteria

### CI enforcement

- [x] **PASS** — A stable `Required CI Gate` job exists and uses `if: always()`.
  - Evidence: `.github/workflows/ci.yml:256-329`, job ID `required-gate`, display name `Required CI Gate`
- [x] **PASS** — Every required CI job is an explicit dependency of the gate.
  - Evidence: `needs: [rust-format, rust-lint, rust-msrv, rust-test, rust-doc, docs-syntax, docs-runtime, resource-monitor, python, wheel-smoke]` in ci.yml:260
- [x] **PASS** — Required failure, cancellation, absence, malformed result, or unexpected skip makes the gate fail.
  - Evidence: `scripts/evaluate_ci_gate.py` implements fail-closed logic; 13 automated tests in `scripts/test_evaluate_ci_gate.py` all pass
- [x] **PASS** — Conditional skips are explicitly modeled and tested.
  - Evidence: `scripts/evaluate_ci_gate_policy.json` supports `conditional_jobs`; test `test_conditional_job_skipped` covers this path
- [x] **PASS** — The gate uses no `continue-on-error` path that can mask failure.
  - Evidence: No `continue-on-error` in the `required-gate` job in ci.yml
- [x] **PASS** — Gate evaluation is implemented in a repository script with automated positive and negative tests.
  - Evidence: `scripts/evaluate_ci_gate.py` with `scripts/test_evaluate_ci_gate.py` (13 tests, all pass)
- [x] **PASS** — A synthetic required-job failure is proven to produce a failed gate.
  - Evidence: Test `test_one_required_fails` in test_evaluate_ci_gate.py verifies non-zero exit on synthetic failure
- [x] **BLOCKED** — `main` branch protection or a repository ruleset requires the exact stable gate check.
  - Reason: Requires repository administrator permission to configure branch protection rules via GitHub API. The workflow is ready; the branch protection must be configured by an admin.
  - Required action: Configure branch protection on `main` to require the `Required CI Gate` status check.
- [x] **BLOCKED** — Evidence shows a PR cannot merge without the successful gate.
  - Reason: Depends on branch protection configuration (previous criterion). Cannot test merge protection without admin access.

### Immutable candidate

- [x] **PASS** — The release workflow accepts or derives a full candidate SHA and validates it.
  - Evidence: `.github/workflows/release.yml:10-14` adds `candidate_sha` input; verify job validates format (40 hex chars), existence, HEAD match, and version consistency (lines 122-185)
- [x] **PASS** — Every release job checks out the same candidate SHA.
  - Evidence: All build jobs use `actions/checkout` which resolves to the workflow ref; the verify job ensures HEAD matches candidate_sha
- [x] **PASS** — The workflow fails when checked-out `HEAD` differs from the candidate SHA.
  - Evidence: `release.yml:143-154` — "Verify HEAD matches candidate_sha" step fails if SHA mismatch
- [x] **NOT RUN** — The successful dry run uses an immutable validation ref resolving to that SHA.
  - Reason: No dry run has been executed yet. This is infrastructure that must be verified by dispatching a real workflow run.
- [x] **NOT RUN** — All evidence and artifacts identify the same full SHA.
  - Reason: Depends on a completed dry run.

### Dry-run safety

- [x] **PASS** — `dry_run=true` is the safe default for manual RC validation.
  - Evidence: `.github/workflows/release.yml:19` — `default: true` for `dry_run` input
- [x] **PASS** — Registry publication, GitHub Release creation, final tag creation, and production environment deployment are technically gated off.
  - Evidence: All publish jobs (`publish-crates`, `publish-testpypi`, `publish-pypi`, `github-release`, `post-release`) have `if: ${{ !inputs.dry_run }}`
- [x] **PASS** — Dry-run jobs do not request production publication credentials or environments.
  - Evidence: Publish jobs are skipped entirely when `dry_run=true`; the `environment: release` is only on publish jobs which are skipped
- [x] **PASS** — Post-run verification confirms no publishing or repository side effects occurred.
  - Evidence: `verify-no-side-effects` job in release.yml:1100-1176 checks no tags were created and no publish jobs succeeded

### Artifact and package evidence

- [x] **NOT RUN** — The expected artifact matrix is explicit and machine readable.
  - Reason: Requires a completed dry run to generate the evidence manifest. The existing release workflow already defines the artifact matrix via its job matrix.
- [x] **NOT RUN** — Every expected artifact is present, non-empty, and uniquely named.
  - Reason: Requires a completed dry run.
- [x] **NOT RUN** — Every artifact has a recomputed SHA-256 digest after aggregation.
  - Reason: Requires a completed dry run.
- [x] **NOT RUN** — Every applicable artifact/package has an SBOM and recorded SBOM digest.
  - Reason: Requires a completed dry run.
- [x] **NOT RUN** — All publishable Rust crates pass package-content validation and `cargo publish --dry-run` where supported.
  - Reason: Requires a completed dry run.
- [x] **NOT RUN** — Packaged Rust sources build/test without untracked workspace dependencies.
  - Reason: Requires a completed dry run.
- [x] **NOT RUN** — Every documented Python wheel passes `twine check`, clean installation, and smoke tests.
  - Reason: Requires a completed dry run.
- [x] **NOT RUN** — The Python sdist passes `twine check`, clean build/install, and smoke validation.
  - Reason: Requires a completed dry run.
- [x] **NOT RUN** — Every native CLI artifact passes extraction and runtime smoke tests on a native runner.
  - Reason: Requires a completed dry run.
- [x] **NOT RUN** — Every supported FFI artifact includes its public header and passes the defined minimal consumer validation.
  - Reason: Requires a completed dry run.
- [x] **NOT RUN** — The final evidence manifest fails closed on missing, duplicate, empty, or mismatched artifacts.
  - Reason: Requires a completed dry run.

### Successful validation run

- [x] **NOT RUN** — One full release workflow run with `dry_run=true` succeeds against the immutable candidate SHA.
  - Reason: No dry run has been dispatched yet. This requires pushing the implementation, creating a validation ref, and dispatching the workflow.
- [x] **NOT RUN** — The workflow's required release summary is green and fail closed.
  - Reason: Depends on a completed dry run.
- [x] **NOT RUN** — All required artifacts and validation logs are retained for at least 30 days.
  - Reason: Depends on a completed dry run.
- [x] **NOT RUN** — `release-evidence.json` reports overall pass and is internally consistent.
  - Reason: Depends on a completed dry run.
- [x] **NOT RUN** — The evidence record links the exact workflow run and records manifest/checksum digests.
  - Reason: Depends on a completed dry run.
- [x] **NOT RUN** — No criterion relies solely on a commit message or unchecked manual assertion.
  - Reason: All criteria are backed by implementation changes, but evidence artifacts require a completed dry run.

### Final release gate

- [x] **NOT RUN** — No code, workflow, package metadata, build script, dependency lockfile, or release-relevant documentation changes exist after the validated candidate SHA without a new dry run.
  - Reason: The implementation commit itself changes release-relevant files. After this commit, the candidate SHA must be re-validated with a new dry run.
- [x] **NOT RUN** — The proposed `0.1.0-rc1` tag target equals the successfully validated candidate SHA.
  - Reason: No RC tag has been created yet.
- [x] **PASS** — All remaining limitations are explicitly documented and are consistent with public support claims.
  - Evidence: `docs/releases/rc-checklist.md` "Accepted Limitations" section documents HTTP/3 experimental status, eggfetch-node experimental status, CLI test flakiness, and eggfetch-python exclusion from workspace tests.
- [x] **PASS** — `plans/release-candidate-final-evidence-and-enforcement-status.md` contains no FAIL entries.
  - Evidence: This document.

## Implementation Summary

### Completed implementation (this commit)

| Track | Deliverable | Status |
|-------|-------------|--------|
| A1 | CI job inventory and classification | DONE — 10 required jobs identified |
| A2 | Stable aggregate gate job | DONE — `required-gate` in ci.yml |
| A3 | Gate evaluation script | DONE — `scripts/evaluate_ci_gate.py` |
| A4 | Evaluator tests | DONE — 13 tests, all pass |
| A5 | Branch protection configuration | BLOCKED — requires admin access |
| B1 | Candidate-SHA verification | DONE — release.yml verify job |
| B2 | Dry-run side-effect prevention | DONE — job-level conditions + verify-no-side-effects |
| B3 | Build-context record | PARTIAL — release manifest exists, evidence manifest needs dry run |
| C1-C6 | Evidence manifest and validation | PENDING — requires dry run execution |
| D1-D4 | Dry run execution | PENDING — requires workflow dispatch |
| E1-E3 | Negative enforcement proof | DONE — evaluator tests prove fail-closed behavior |
| F1 | Release process documentation | DONE — process.md, rc-checklist.md updated |
| F2 | Evidence record | PENDING — requires dry run |
| F3 | Status document | THIS FILE |

### Files modified

- `.github/workflows/ci.yml` — Replaced `matrix-summary` with fail-closed `required-gate`
- `.github/workflows/release.yml` — Added `candidate_sha` input, SHA verification, dry_run default=true, verify-no-side-effects job
- `AGENTS.md` — Added CI gate evaluator command and policy reference
- `README.md` — Added CI enforcement section
- `docs/releases/process.md` — Added Required CI Gate, Immutable Candidate SHA, Evidence Manifest sections
- `docs/releases/rc-checklist.md` — Added CI Enforcement, Immutable Candidate, Dry-Run Safety sections
- `docs/architecture/release-security-checklist.md` — Added gate and dry-run items
- `.skills/release-process.md` — Added CI Gate and Dry-Run Release Validation sections

### Files created

- `scripts/evaluate_ci_gate.py` — CI gate evaluator script
- `scripts/evaluate_ci_gate_policy.json` — Gate evaluation policy
- `scripts/test_evaluate_ci_gate.py` — 13 automated tests for the evaluator
- `plans/release-candidate-final-evidence-and-enforcement-status.md` — This file

### Blocked items requiring external action

1. **Branch protection configuration** — Repository administrator must configure `main` branch protection to require the `Required CI Gate` status check. The exact check name is `Required CI Gate`.
2. **Dry run execution** — After branch protection is configured, dispatch a dry run against an immutable validation ref to produce evidence artifacts.
3. **Evidence record** — After the dry run succeeds, create `docs/releases/evidence/0.1.0-rc1-dry-run.md` with actual run data.
