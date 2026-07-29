# Final CI Portability and Operational Verification Closure

Status: narrow implementation handoff plan

Audited baseline commit: `8489225108e858e788e50442b5b2eafb52e689b2`

Audit date: 2026-07-29

Target repository: `eggstack/eggfetch`

Parent plans:

- `plans/ci-verification-and-manual-release-simplification.md`
- `plans/ci-validation-truthfulness-corrective-pass.md`
- `plans/ci-package-validation-final-closure.md`

Primary surfaces:

- `scripts/check.sh`
- optionally one small focused validation helper under `scripts/`
- `docs/architecture/dependency-policy.md`
- GitHub Actions run state for the final implementation SHA
- GitHub branch protection and repository rulesets
- GitHub Actions secrets
- GitHub release environments

## 1. Purpose

The CI simplification and package-validation work is functionally complete. Routine CI remains one Ubuntu job, release remains manual, package mode performs a real local check for all publishable crates, and wheel validation now enforces a single current-run artifact.

This handoff closes only the final outstanding items:

1. replace the non-portable, line-oriented `grep -E` Cargo manifest inspection with a portable semantic check;
2. correct the inaccurate claim that `cargo-deny` and `cargo-audit` run through `./scripts/check.sh extended`;
3. obtain and record the required final CI and repository-settings evidence.

No additional CI design, package architecture, compatibility work, or release automation belongs in this pass.

## 2. Scope

### Included

- replace `verify_non_path_versions()` implementation with a portable structured check;
- ensure every publishable internal dependency has an explicit non-wildcard registry version requirement in addition to its local path;
- retain fail-closed package-mode behavior;
- correct `docs/architecture/dependency-policy.md` to describe security tools accurately;
- run routine and package validation after the correction;
- obtain a successful `CI / ci` run on the final implementation SHA;
- inspect branch protection and repository rulesets for stale required check names;
- inspect Actions secret names for obsolete publication credentials;
- inspect repository environments for obsolete release environments;
- remove stale settings where authorized, or report the exact permission limitation without claiming full operational closure.

### Excluded

- adding CI jobs, matrices, platforms, or Python versions;
- adding `cargo-deny` or `cargo-audit` to routine or extended validation;
- restoring security, qualification, evidence, or release workflows;
- publishing crates, wheels, tags, or GitHub Releases;
- changing HTTP behavior or HTTPX compatibility behavior;
- changing package-mode crate-selection or wheel-selection architecture;
- adding a general TOML framework or new build orchestrator;
- creating another roadmap or broad follow-up series.

## 3. Non-negotiable constraints

1. `.github/workflows/ci.yml` remains the only push/PR workflow.
2. CI remains one Ubuntu job with no matrix.
3. CI continues to run routine `./scripts/check.sh` only.
4. Release remains fully manual.
5. Package mode remains fail-closed and has no skip outcome.
6. All five publishable crates continue to receive an executable package check.
7. Manifest validation must be semantic rather than dependent on Cargo.toml line layout.
8. Manifest validation must work on Linux and macOS developer environments.
9. The correction must not require GNU grep.
10. The correction must not add a mandatory third-party Python dependency to routine or package validation.
11. Documentation must not claim a command is part of a validation tier when it is not invoked there.
12. Repository settings must not be declared clean without direct evidence.
13. No workflow receives publication secrets or write permissions.

## 4. Phase 1: Replace the non-portable manifest validator

Current package validation uses a regular expression equivalent to:

```sh
grep -E 'eggfetch-(core|ffi)\s*=\s*\{.*version\s*=' Cargo.toml
```

This is not an acceptable final implementation because:

- `\s` is not portable POSIX extended-regex syntax;
- inline-table formatting or multiline dependency declarations can change without changing TOML semantics;
- one match only proves that some internal dependency contains `version`, not that every publishable internal dependency is correctly versioned;
- the command uses `|| true` to turn the search result into data.

### 4.1 Preferred implementation: Cargo metadata plus standard-library JSON

Use Cargo's structured metadata as the primary source of dependency requirements. One narrow implementation is:

```sh
cargo metadata --format-version 1 --no-deps
```

Parse the JSON using the already-selected Python interpreter and the standard-library `json` module. The check must:

1. identify package records for:
   - `eggfetch-cli`;
   - `eggfetch-ffi`;
   - `eggfetch-python`;
   - `eggfetch-node`;
2. inspect dependencies whose names are publishable internal crates:
   - `eggfetch-core`;
   - `eggfetch-ffi`;
3. require each internal dependency to have a concrete version requirement;
4. reject missing, wildcard-only, or otherwise non-publishable requirements;
5. preserve the local path dependency used for workspace development;
6. fail if an expected package or expected internal dependency cannot be found.

A small focused helper such as:

```text
scripts/validate_publishable_internal_dependencies.py
```

is acceptable and preferable to embedding a large Python heredoc in `check.sh`. It must use only the Python standard library and consume either Cargo metadata JSON from stdin or invoke `cargo metadata` directly.

### 4.2 Acceptable alternative: semantic TOML parsing

Direct TOML parsing is acceptable only when it does not add an undeclared runtime dependency to Python 3.10 package validation. Do not assume `tomllib` exists on Python 3.10 and do not silently install `tomli`.

If this route is chosen, the implementer must provide an explicit, already-satisfied parser contract. Otherwise use Cargo metadata.

### 4.3 `scripts/check.sh` integration

Replace `verify_non_path_versions()` with the structured helper or structured inline check. The package loop should remain conceptually:

```sh
for crate in eggfetch-cli eggfetch-ffi eggfetch-python eggfetch-node; do
    cargo package --list -p "$crate" >/dev/null
    validate_publishable_dependencies "$crate"
done
```

Do not change:

- the `eggfetch-core` `cargo publish --dry-run`;
- the exactly-one-wheel logic;
- wheel smoke behavior;
- package-content validation;
- final success semantics.

### 4.4 Controlled failure validation

Temporarily remove or replace the version requirement for one internal dependency, for example:

```toml
eggfetch-core = { path = "../eggfetch-core" }
```

Then run the focused validator or package mode and prove that it:

- exits nonzero;
- names the package and dependency;
- does not print `All package checks passed`.

Restore the manifest before committing.

### Phase 1 acceptance criteria

- no semantic Cargo.toml validation relies on `grep`;
- no `\s` regex remains in the package manifest validator;
- no validator branch uses `|| true` to suppress a failed semantic check;
- all expected dependent packages are checked;
- every expected internal publishable dependency is checked individually;
- a missing version requirement fails;
- a wildcard-only version requirement fails;
- missing package metadata fails;
- missing expected internal dependency metadata fails;
- the validator uses no new third-party Python package;
- the validator works with the repository's Python 3.10+ contract;
- `./scripts/check.sh package` remains fail-closed;
- package mode still validates exactly one current-run wheel.

## 5. Phase 2: Correct security-tool documentation

Update only the inaccurate paragraph in:

```text
docs/architecture/dependency-policy.md
```

The document currently claims that both `cargo-deny` and `cargo-audit` run through `./scripts/check.sh extended`. They do not.

Replace that claim with the actual policy. A suitable contract is:

- `cargo-deny` and `cargo-audit` are optional manual security-review tools;
- when installed, maintainers may run commands such as:

```sh
cargo deny check
cargo audit
```

- neither command is currently part of routine CI or `./scripts/check.sh extended`;
- findings should be addressed during security review, but these tools are not automatic merge or release gates under the simplified CI policy.

Do not add the tools to `scripts/check.sh` as a way to make the existing sentence true. That would expand scope and restore validation complexity.

### Phase 2 acceptance criteria

- `docs/architecture/dependency-policy.md` no longer says cargo-deny runs in extended validation;
- it no longer says cargo-audit runs in extended validation;
- the document distinguishes optional manual security review from canonical CI;
- no deleted security workflow is referenced;
- no new mandatory security gate is introduced;
- the one-job CI policy remains unchanged.

## 6. Phase 3: Validate the final code state

Use a prepared virtual environment and run:

```sh
./scripts/check.sh
./scripts/check.sh package
```

Also run focused checks for the new helper where useful. Do not add a permanent shell-test framework.

Record:

- final implementation SHA;
- routine validation exit status;
- package validation exit status;
- exact dependency records checked for each dependent crate;
- controlled missing-version failure result;
- final wheel path used by smoke and content validation.

### Phase 3 acceptance criteria

- routine validation exits zero;
- package validation exits zero;
- `eggfetch-core` publish dry-run executes;
- all four dependent package-list checks execute;
- structured dependency validation executes for all four dependent crates;
- controlled missing-version validation exits nonzero;
- no failed validation prints a success summary;
- the restored worktree is clean before push.

## 7. Phase 4: Obtain final CI evidence

Push the implementation and identify the GitHub Actions run associated with the final implementation SHA.

Suggested GitHub CLI commands:

```sh
gh run list \
  --repo eggstack/eggfetch \
  --workflow CI \
  --commit <FINAL_SHA>

gh run view \
  --repo eggstack/eggfetch \
  <RUN_ID>
```

Confirm and record:

- workflow name: `CI`;
- job name: `ci`;
- trigger corresponds to the final push or PR SHA;
- conclusion is `success`;
- exactly one job ran;
- no matrix expansion exists;
- no workflow artifact was uploaded;
- no other push-triggered workflow ran for that SHA.

A successful local run is not a substitute for this evidence.

### Phase 4 acceptance criteria

- the final implementation SHA has a retained successful Actions run;
- the run URL is recorded;
- the job name is recorded;
- the conclusion and duration are recorded;
- exactly one job is present;
- no other push workflow ran for the SHA;
- no artifact upload or publication step exists.

## 8. Phase 5: Verify repository operational settings

These checks close deleted-workflow residue. They must not produce new infrastructure.

### 8.1 Branch protection and rulesets

Inspect both legacy branch protection and repository rulesets:

```sh
gh api repos/eggstack/eggfetch/branches/main/protection
gh api repos/eggstack/eggfetch/rulesets
```

A 404 from legacy branch protection can mean that endpoint is not configured. Record the result rather than treating it as automatic success.

Confirm no rule requires deleted contexts such as:

- `Required CI Gate`;
- qualification or evidence jobs;
- release jobs;
- deleted matrix jobs;
- deleted security, benchmark, or FFI workflow jobs.

If a status check is required, it must use the exact current context GitHub reports for `CI / ci`.

### 8.2 Actions secrets

List secret names only:

```sh
gh secret list --repo eggstack/eggfetch --app actions
```

Never expose values.

Remove obsolete publication or deleted-workflow secrets where authorized, including credentials whose only purpose was:

- crates.io publication;
- PyPI or TestPyPI publication;
- npm publication;
- deleted release automation.

Do not remove a secret with a verified current non-publication purpose. Record retained names and purposes without disclosing secret content.

### 8.3 Environments

Inspect repository environments:

```sh
gh api repos/eggstack/eggfetch/environments
```

Remove environments whose only purpose was the deleted release workflow, where authorized. Record any retained environment and its active purpose.

### 8.4 Permission limitations

If the implementing agent cannot inspect or modify a setting:

- record the exact API or CLI command attempted;
- record the permission or access error;
- identify the specific maintainer action still required;
- do not state that operational closure is complete.

Do not create another code change solely to compensate for inaccessible repository settings.

### Phase 5 acceptance criteria

- no branch rule requires a deleted check name;
- any required status context matches current `CI / ci` exactly;
- obsolete Actions publication secrets are removed or their absence is directly verified;
- obsolete release environments are removed or their absence is directly verified;
- retained settings have a documented current purpose;
- secret values are never printed or committed;
- inaccessible settings are explicitly reported rather than assumed clean.

## 9. Final closure criteria

This line of work is closed only when all of the following are true.

### Code portability

1. The grep-based Cargo manifest validator is removed.
2. Package manifest validation is semantic and structured.
3. The validator works without GNU grep behavior.
4. The validator introduces no new mandatory third-party Python dependency.
5. Every expected publishable internal dependency is checked individually.
6. Missing or wildcard-only version requirements fail.
7. Package mode remains fail-closed.
8. All five publishable crates still receive a real package check.
9. Exactly-one-wheel validation remains intact.

### Documentation

10. Dependency-policy documentation no longer claims cargo-deny runs in extended validation.
11. Dependency-policy documentation no longer claims cargo-audit runs in extended validation.
12. Manual security commands are described as optional review tools, not CI gates.
13. No deleted security workflow is referenced.
14. CI remains documented as one Ubuntu job.
15. Release remains documented as manual.

### Validation evidence

16. Routine validation passes on the final worktree.
17. Package validation passes on the final worktree.
18. Controlled missing-version validation fails as intended.
19. No failed validation prints a success summary.
20. The final implementation SHA is recorded.

### GitHub operational closure

21. The final SHA has a successful `CI / ci` run.
22. Exactly one CI job ran.
23. No other push workflow ran for that SHA.
24. No stale required check name remains in branch protection or rulesets.
25. Any required context is the exact current `CI / ci` context.
26. Obsolete publication secrets are absent or removed.
27. Obsolete release environments are absent or removed.
28. Any inaccessible setting is reported precisely and is not represented as verified.

### Scope control

29. No CI job, matrix, workflow, artifact upload, or publication automation is added.
30. No new mandatory security or release gate is added.
31. The implementation remains confined to portability, documentation accuracy, and operational evidence.
32. No further implementation plan is needed once these criteria are met.

## 10. Rejection conditions

Reject the implementation if any of the following is true:

- semantic manifest validation still relies on `grep` or line formatting;
- the validator accepts a missing internal dependency version;
- the validator checks only one arbitrary matching dependency;
- the correction adds an undeclared Python parser dependency;
- package mode regains a skip or warning path;
- wheel cardinality behavior is weakened;
- documentation claims cargo-deny or cargo-audit run automatically when they do not;
- cargo-deny or cargo-audit are added to CI merely to preserve the false documentation claim;
- CI gains another job, matrix, or workflow;
- operational settings are declared clean without direct evidence;
- secret values are exposed;
- release automation returns.

## 11. Required handoff report

The implementing agent must report:

- baseline SHA `8489225108e858e788e50442b5b2eafb52e689b2`;
- final implementation SHA;
- manifest-validation mechanism chosen;
- dependencies checked for each publishable dependent crate;
- routine validation result;
- package validation result;
- controlled missing-version failure result;
- final CI run URL, job name, conclusion, and duration;
- branch-protection and ruleset findings;
- Actions secret-name cleanup summary;
- environment cleanup summary;
- any permission limitations;
- explicit confirmation that CI remains one job and release remains manual;
- explicit decision: fully closed, or code closed with a precise remaining operational owner action.
