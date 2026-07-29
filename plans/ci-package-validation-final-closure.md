# CI Package Validation and Operational Evidence Final Closure

Status: narrow implementation handoff plan

Audited baseline commit: `a85a34c3b3345d3466309524c623266874c0a2bd`

Audit date: 2026-07-29

Target repository: `eggstack/eggfetch`

Parent plans:

- `plans/ci-verification-and-manual-release-simplification.md`
- `plans/ci-validation-truthfulness-corrective-pass.md`

Primary implementation surfaces:

- `scripts/check.sh`
- `scripts/wheel_smoke.py`
- `docs/verification-policy.md`
- `docs/architecture/build-ci.md`
- `docs/releases/process.md`
- `AGENTS.md`
- GitHub branch protection or rulesets
- GitHub Actions secrets and environments
- the current `CI / ci` workflow run

## 1. Purpose

The CI simplification and validation-truthfulness work is substantially complete. The repository now has one automatic Ubuntu CI job, no matrix, no qualification or release automation, explicit virtual-environment handling, fail-closed routine and extended checks, fresh package artifacts, and manual release documentation.

This final closure pass addresses only the remaining discrepancies:

1. `./scripts/check.sh package` validates `eggfetch-core` but merely prints that the four dependent publishable crates are skipped.
2. The script then prints `All package checks passed`, although four crate packages were not examined.
3. Release and CI documentation claim those dependent crates are checked with `cargo package --no-verify`, but the script does not run that command.
4. Wheel selection does not explicitly reject zero or multiple current-run wheels before smoke and content validation.
5. `AGENTS.md` and the CI architecture document retain minor command drift from the canonical validation entry point.
6. There is no retained closure evidence for a successful current one-job CI run or for repository settings cleanup.

The purpose of this pass is to make package validation complete and mechanically truthful, reconcile the remaining documentation, and record operational closure. It must not alter the one-job CI architecture or add new automation.

## 2. Scope

### Included

- execute a local package-structure check for every publishable crate;
- retain `cargo publish --dry-run` for `eggfetch-core`;
- use `cargo package --no-verify` for dependent crates when full dry-run resolution is impossible before publication;
- fail package mode if any selected crate package check fails;
- remove the package-mode skip message;
- require exactly one wheel produced by the current package-mode invocation;
- pass the exact wheel to smoke and content validation, or otherwise make the one-wheel invariant explicit;
- reconcile release, verification, CI architecture, and agent documentation with actual commands;
- verify a successful `CI / ci` run for current `main`;
- inspect and correct branch-protection/ruleset check names;
- inspect and remove obsolete publication secrets and release environments where authorized;
- document any repository-setting action that cannot be completed with the implementing agent's permissions.

### Excluded

- adding CI jobs, matrices, operating systems, or Python versions;
- restoring qualification, release, benchmark, FFI, or security workflows;
- publishing any crate or Python package;
- creating a GitHub Release or tag;
- changing HTTP behavior or compatibility claims;
- adding a workflow validator or evidence schema;
- expanding the extended validation suite;
- redesigning `scripts/check.sh` beyond the package-mode corrections specified here;
- creating another roadmap or follow-up planning series.

## 3. Non-negotiable constraints

1. `.github/workflows/ci.yml` remains the only push/PR workflow.
2. CI remains one Ubuntu job with no matrix.
3. CI continues to invoke `./scripts/check.sh` routine mode only.
4. Package mode performs no real publication.
5. Package mode has no `SKIP` outcome.
6. All five publishable crates receive an executable local package check.
7. Any crate package-check failure returns nonzero.
8. Package mode prints success only after all five crate checks, wheel build, wheel smoke, and wheel-content validation pass.
9. Package mode uses only artifacts produced during the current invocation.
10. Zero current-run wheels is a failure.
11. More than one current-run wheel is a failure unless the command explicitly requested multiple wheels, which this pass must not introduce.
12. Documentation describes commands the repository actually runs.
13. No branch rule requires a deleted check name.
14. No GitHub Actions publication credential remains unless a current, documented non-publication use is proven.
15. No release environment remains solely for the deleted release workflow.
16. No code change in this pass may reintroduce release automation.

## 4. Required implementation

## Phase 1: Complete crate package validation

Update the package-mode crate function in `scripts/check.sh`.

### 1.1 Core crate

Retain the full dry run:

```sh
cargo publish --dry-run -p eggfetch-core
```

This command is required and fail-closed.

### 1.2 Dependent publishable crates

Run a package-archive and manifest/content check for each dependent crate:

```sh
cargo package --no-verify -p eggfetch-cli
cargo package --no-verify -p eggfetch-ffi
cargo package --no-verify -p eggfetch-python
cargo package --no-verify -p eggfetch-node
```

These commands are required and fail-closed. They validate that Cargo can assemble each publishable package without requiring the unpublished coordinated dependency version to resolve from crates.io.

Before committing, run each command independently at the audited baseline and record the actual result. If Cargo still refuses a dependent package solely because an internal dependency version is not present on crates.io, do not silently skip it. Use the narrowest truthful local fallback:

1. run `cargo package --list -p <crate>` to validate manifest/package inclusion;
2. verify the package manifest contains a non-path version for every publishable dependency;
3. build/test the workspace crate through existing Tier 1 validation;
4. document the registry-resolution limitation in the release runbook;
5. retain the mandatory full `cargo publish --dry-run -p <crate>` immediately before publication after dependencies are visible.

The fallback may be used only for a specifically observed Cargo resolution limitation. It must remain executable and fail-closed; an informational print statement is not a package check.

### 1.3 Naming and summary

Rename the function if necessary so its name reflects mixed validation, for example:

```text
tier3_crate_packages
```

Do not call all five operations “publish dry runs” when four use package assembly.

Remove output equivalent to:

```text
Skipped: internal dependencies not yet on crates.io
```

Package mode must not record or print a skip.

### Acceptance criteria

- `eggfetch-core` executes `cargo publish --dry-run`;
- `eggfetch-cli` executes a fail-closed package check;
- `eggfetch-ffi` executes a fail-closed package check;
- `eggfetch-python` executes a fail-closed package check;
- `eggfetch-node` executes a fail-closed package check;
- there is no package-mode skip message;
- corrupting a dependent crate manifest causes package mode to return nonzero;
- no `cargo publish` command without `--dry-run` exists in `scripts/check.sh`;
- the final success message is unreachable after any crate package-check failure.

## Phase 2: Enforce exactly one current-run wheel

Package mode already builds into a fresh temporary directory. Complete the invariant by resolving the wheel once and passing that exact artifact through the remaining checks.

### 2.1 Shell-side wheel resolution

After `maturin build`, collect wheels with null-glob semantics. One acceptable implementation is:

```bash
find_single_wheel() {
    local wheel_dir="$1"
    local wheels=()

    shopt -s nullglob
    wheels=("$wheel_dir"/*.whl)
    shopt -u nullglob

    if [[ ${#wheels[@]} -ne 1 ]]; then
        fail "Expected exactly one wheel in $wheel_dir; found ${#wheels[@]}"
    fi

    printf '%s\n' "${wheels[0]}"
}
```

Resolve once:

```bash
PACKAGE_WHEEL="$(find_single_wheel "$PACKAGE_TMP/wheels")"
```

Do not repeat independent globs in smoke and content functions.

### 2.2 Wheel smoke interface

Preferred implementation: update `scripts/wheel_smoke.py` to accept an exact wheel path:

```sh
python scripts/wheel_smoke.py --wheel "$PACKAGE_WHEEL"
```

The script must verify that the path:

- exists;
- is a regular `.whl` file;
- is compatible with the active Python interpreter or ABI policy;
- is the only artifact selected for installation.

Remove first-match behavior. The script must not silently choose the first wheel from a directory containing multiple candidates.

An acceptable smaller alternative is to preserve `--wheel-dir` but make `wheel_smoke.py` require exactly one compatible wheel and fail on ambiguity. The shell must still establish the exactly-one-current-run-wheel invariant before invoking it.

### 2.3 Content validation

Pass the exact wheel path to `validate_package_content.py`:

```sh
python scripts/validate_package_content.py "$PACKAGE_WHEEL"
```

Do not loop over an ambiguous glob.

### Acceptance criteria

- no wheel produces a clear nonzero failure;
- two wheels produce a clear nonzero failure;
- one wheel is selected deterministically;
- smoke and content validation inspect the same exact wheel;
- `wheel_smoke.py` no longer selects `matched[0]` from multiple candidates;
- stale repository `dist/` contents cannot affect package mode;
- temporary package artifacts are removed on success and failure.

## Phase 3: Reconcile documentation with executable behavior

Update only the documents containing current drift.

### 3.1 Release process

`docs/releases/process.md` must state the exact package-mode contract:

- full `cargo publish --dry-run` for `eggfetch-core`;
- fail-closed local package assembly/check for each dependent crate;
- full dependent-crate dry run immediately before each publication after internal dependencies become visible;
- one fresh wheel is built, smoke-tested, and content-validated;
- package mode performs no publication and is not release authorization.

Do not claim `cargo package --no-verify` is executed unless the final script actually executes it.

### 3.2 Verification policy

Change generic wording such as “crate dry-runs” to the precise contract:

```text
core publish dry-run plus dependent-crate package checks
```

Preserve manual-release policy and the one-job complexity budget.

### 3.3 CI architecture

Update `docs/architecture/build-ci.md` so:

- Tier 1 Python test documentation no longer names the deleted `soak_test.py` ignore path;
- package-mode commands match `scripts/check.sh` exactly;
- dependent crates are not described as checked by a command the script does not run;
- routine CI remains documented as one Ubuntu job.

### 3.4 Agent guide

Reduce `AGENTS.md` Quick Commands to the three canonical commands plus only a minimal number of verified focused commands:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Where a focused Python command remains, use:

```sh
python -m pytest
```

Do not use bare `pytest` in canonical examples. Do not recommend `cargo test --workspace --all-features` as equivalent to Tier 1 when Tier 1 intentionally excludes `eggfetch-python` from direct workspace execution.

The resolved-differences ledger may remain documented as historical context, but no command may imply that the comparator consumes it after removal of `--resolved`.

### Acceptance criteria

- release documentation matches the final crate commands;
- verification policy uses precise package terminology;
- CI architecture has no deleted `soak_test.py` path;
- `AGENTS.md` centers the canonical script instead of duplicating the validation graph;
- active examples use `python -m pytest` and `python -m pip`;
- no active documentation references deleted workflows or ignored comparator options;
- manual crates.io publication remains explicit.

## Phase 4: Validate package-mode truthfulness

Run in a prepared virtual environment:

```sh
./scripts/check.sh package
```

Record the command and exit status for each stage:

1. Tier 1 validation;
2. `eggfetch-core` publish dry run;
3. `eggfetch-cli` package check;
4. `eggfetch-ffi` package check;
5. `eggfetch-python` package check;
6. `eggfetch-node` package check;
7. wheel build;
8. one-wheel resolution;
9. wheel smoke;
10. package-content validation.

Perform temporary controlled failures without committing the broken state:

- introduce a reversible invalid package metadata change in one dependent crate and prove package mode fails;
- temporarily place a second dummy `.whl` in the current-run wheel directory at the cardinality-check boundary and prove it fails;
- invoke wheel smoke with a missing wheel and prove it fails;
- restore the repository and rerun package mode successfully.

Do not add a permanent shell-test framework for these checks.

### Acceptance criteria

- the successful package run exits zero;
- every required stage is visibly executed;
- the dependent-crate failure injection exits nonzero;
- the multiple-wheel injection exits nonzero;
- the missing-wheel case exits nonzero;
- no failed case prints `All package checks passed`;
- the restored final run prints `All package checks passed` only after all stages pass.

## Phase 5: Record CI and repository-settings closure

These are operational checks, not new automation.

### 5.1 Current CI run

Push the implementation and obtain the `CI` workflow run associated with the final implementation SHA.

Using GitHub CLI where available:

```sh
gh run list --repo eggstack/eggfetch --workflow CI --commit <FINAL_SHA>
gh run view --repo eggstack/eggfetch <RUN_ID>
```

Confirm:

- exactly one job named `ci` ran;
- the run completed successfully;
- no matrix jobs exist;
- no artifacts were uploaded;
- no other push-triggered workflow ran for the commit.

A screenshot is not required. The final handoff must include the run URL, final SHA, job name, conclusion, and duration.

### 5.2 Branch protection and rulesets

Inspect both legacy branch protection and repository rulesets:

```sh
gh api repos/eggstack/eggfetch/branches/main/protection
gh api repos/eggstack/eggfetch/rulesets
```

A 404 from the legacy protection endpoint may mean no legacy protection is configured; it is not by itself a failure.

Confirm no rule requires:

- `Required CI Gate`;
- deleted matrix job names;
- qualification jobs;
- release jobs;
- FFI, benchmark, or security workflow jobs.

If a status check is required, it must be only the current `CI / ci` context, using the exact context name GitHub reports.

### 5.3 Actions secrets

List names only; never expose values:

```sh
gh secret list --repo eggstack/eggfetch --app actions
```

Remove obsolete publication secrets where authorized, including names corresponding to:

- crates.io tokens;
- PyPI or TestPyPI tokens;
- npm publication tokens;
- legacy release automation.

Do not remove a secret with a current documented non-publication purpose. Record any retained secret name and its purpose without exposing its value.

### 5.4 Environments

Inspect repository environments:

```sh
gh api repos/eggstack/eggfetch/environments
```

Remove obsolete environments used solely by the deleted automated release workflow. Retain an environment only when it has a current manual or non-release function, and document that function.

### 5.5 Permission limitation

If the implementing agent cannot inspect or modify settings, the code pass may still complete, but the final handoff must state precisely which checks remain unverified and provide the exact maintainer commands above. Do not claim operational closure without evidence.

### Acceptance criteria

- a successful current `CI / ci` run is linked;
- exactly one CI job ran for the final SHA;
- no branch rule requires deleted checks;
- Actions publication secrets are removed or each retained secret has a documented current purpose;
- obsolete release environments are removed or each retained environment has a documented current purpose;
- no secret values appear in commits, logs, plans, or handoff text.

## 5. Final closure criteria

This line of work is closed only when all criteria below are true.

### Package validation

1. Package mode executes a check for all five publishable crates.
2. `eggfetch-core` receives a full publish dry run.
3. Each dependent crate receives a real local package check.
4. No dependent crate is represented by an informational skip.
5. Every crate check is fail-closed.
6. No real publish command exists in the script.
7. Package mode succeeds only after all crate checks pass.
8. Exactly one current-run wheel is required.
9. Zero wheels fails.
10. Multiple wheels fail.
11. Smoke and content validation consume the same exact wheel.
12. Stale artifacts cannot satisfy package mode.
13. Temporary artifacts are cleaned on success and failure.

### Documentation

14. Release documentation matches the script.
15. Verification policy distinguishes core dry-run from dependent package checks.
16. CI architecture contains no deleted soak-test path.
17. Agent instructions center the three canonical commands.
18. Canonical Python examples use `python -m pytest` and `python -m pip`.
19. No active documentation references deleted workflows or ignored options.
20. Manual release and crates.io dependency order remain explicit.

### CI and settings

21. The final implementation SHA has a successful `CI / ci` run.
22. Exactly one CI job ran for that SHA.
23. No other push workflow ran.
24. Branch protection or rulesets require no deleted check names.
25. Any required check is the current `CI / ci` context only.
26. Obsolete Actions publication secrets are removed.
27. Obsolete release environments are removed.
28. Any unverified setting is explicitly reported rather than assumed.

### Scope control

29. CI remains one Ubuntu job with no matrix.
30. No workflow, evidence schema, artifact upload, or publication automation is added.
31. The implementation is limited to package truthfulness, documentation parity, and operational closure evidence.
32. No further corrective plan is required for this CI simplification line of work.

## 6. Rejection conditions

Reject the implementation if any of the following is true:

- package mode still prints that dependent crates are skipped;
- package mode claims success without executing a check for all five crates;
- a failing crate check is converted into a warning;
- wheel smoke silently selects the first of multiple wheels;
- documentation claims a command is executed when it is not;
- the deleted `soak_test.py` path remains in active CI documentation;
- CI gains another job or matrix;
- a release workflow, publication credential reference, or publication permission returns;
- operational settings are declared clean without evidence;
- the implementation expands beyond this narrow closure scope.

## 7. Handoff requirements

The implementing agent must provide:

- baseline SHA `a85a34c3b3345d3466309524c623266874c0a2bd`;
- final implementation SHA;
- exact package command used for each publishable crate;
- successful `./scripts/check.sh package` output summary;
- results of dependent-crate failure injection;
- results of zero/multiple-wheel validation;
- final wheel path used by smoke and content checks;
- documentation files changed;
- final CI run URL, job name, conclusion, and duration;
- branch-protection/ruleset check-name findings;
- Actions secret names removed or retained-purpose summary;
- environments removed or retained-purpose summary;
- explicit statement that CI remains one job and release remains manual;
- explicit statement that this CI simplification line is closed, or a precise list of settings that remain inaccessible to the agent.

## 8. Final decision rule

Close this work when package mode truthfully validates every publishable crate, accepts exactly one current-run wheel, documentation exactly matches executable behavior, one current CI run is green, and repository settings no longer reference deleted release or verification infrastructure.

Do not create another follow-up plan for this line of work after these criteria are satisfied.