# Release-Candidate Final Evidence and CI Enforcement Pass

Status: ready for implementation handoff

## Purpose

Close the final two release-candidate gaps before tagging `0.1.0-rc1`:

1. Produce a complete, immutable, non-publishing release dry run against one exact candidate commit and retain machine-verifiable evidence for every expected artifact and validation job.
2. Make the pull-request and `main` CI gate fail closed whenever any required matrix job fails, is cancelled, is missing, or is unexpectedly skipped.

This is an evidence and enforcement pass. It must not add product features, expand the public API, change supported-platform claims, or relax existing test, lint, security, packaging, or resource thresholds.

## Baseline

Implementation begins from the current `main` head. Record the starting SHA in the implementation status document before changing files.

Relevant existing infrastructure includes:

- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `docs/releases/process.md`
- `docs/releases/rc-checklist.md`
- `scripts/check_lint_suppressions.sh`
- `scripts/wheel_smoke.py`
- resource and benchmark validation under `crates/eggfetch-bench`

Do not assume the existing informational matrix summary is an enforceable release gate. Verify actual job dependencies and branch/ruleset configuration.

## Non-goals

- Publishing to crates.io, PyPI, npm, or GitHub Releases.
- Creating the final `0.1.0-rc1` tag.
- Changing versions except where an isolated dry-run fixture requires an RC-form version and the change is intentionally part of the candidate commit.
- Adding new transport, CLI, Python, FFI, or Node functionality.
- Weakening flaky tests, changing assertions solely to obtain green CI, or adding broad lint suppressions.
- Treating documentation or a manually written checklist as proof in place of retained workflow evidence.

## Deliverables

The pass is complete only when all of the following exist:

1. A stable fail-closed aggregate CI job in `.github/workflows/ci.yml`.
2. A repository script that evaluates required job results deterministically and has automated positive and negative tests.
3. Documented branch-protection or repository-ruleset configuration requiring the aggregate gate on `main`.
4. A release dry-run workflow that accepts and verifies an explicit candidate SHA and cannot publish when `dry_run=true`.
5. A machine-readable evidence manifest binding all outputs to the candidate SHA and workflow run.
6. A successful dry-run workflow execution against one immutable candidate SHA.
7. Retained release artifacts, checksums, SBOMs, smoke-test results, package-validation results, and job-status evidence from that run.
8. A committed evidence record under `docs/releases/evidence/` referencing the immutable SHA and successful run.
9. A final status file under `plans/` that maps every acceptance criterion to concrete evidence.

## Track A — Define the required CI contract

### A1. Inventory required jobs

Inspect `.github/workflows/ci.yml` and classify every job as one of:

- required for every pull request and `main` commit;
- conditionally required when its feature/platform is selected;
- informational only;
- scheduled/manual only.

At minimum, the required set must account for the repository's existing validation policy, including:

- formatting;
- linting and broad-suppression detection;
- Rust tests on every supported required host platform;
- feature-matrix and protocol builds required by project policy;
- Python build/tests on the supported matrix;
- CLI tests;
- FFI validation;
- documentation checks and runtime examples;
- security and package-content checks that are currently part of merge readiness.

Do not silently downgrade an existing required job to informational status. If a job cannot be required, document the reason and obtain an explicit policy decision in the status file.

### A2. Give the aggregate gate a stable identity

Add or replace the current summary job with one stable job whose display name and job ID are intended for branch protection, for example:

- job ID: `required-gate`
- display name: `Required CI Gate`

The gate must:

- use `if: ${{ always() }}`;
- list every required upstream job in `needs`;
- run even after an upstream failure or cancellation;
- fail when any required job result is `failure`, `cancelled`, or absent;
- fail when a required job is unexpectedly `skipped`;
- allow `skipped` only for jobs explicitly classified as conditional and only when the documented condition is false;
- print a concise table of job names and results before exiting;
- never use `continue-on-error: true`;
- never convert an upstream failure into a successful gate for reporting convenience.

Do not rely on GitHub's matrix-summary presentation alone. The aggregate gate must have executable fail-closed logic.

### A3. Put gate evaluation in a testable script

Create a small repository-owned evaluator, preferably under `scripts/`, rather than embedding untestable ad hoc shell expressions in YAML.

The evaluator must accept a machine-readable map of job results plus the required/conditional policy and return:

- exit `0` only when all required results satisfy policy;
- non-zero for failure, cancellation, missing entries, unknown result values, malformed input, or unexpected skips.

The workflow should pass `${{ toJSON(needs) }}` or an explicitly constructed equivalent to this script.

Keep the evaluator dependency-light. Python from the GitHub-hosted runner is acceptable; a POSIX shell implementation is acceptable only if its JSON/result handling is deterministic and portable.

### A4. Add evaluator tests

Add automated tests covering at least:

1. all required jobs succeed;
2. one required job fails;
3. one required job is cancelled;
4. one required job is missing;
5. one required job is unexpectedly skipped;
6. an explicitly conditional job is skipped because its condition is false;
7. an unknown result value is supplied;
8. malformed input is supplied;
9. multiple simultaneous failures are all reported;
10. the evaluator itself cannot find its policy/configuration file.

The negative cases must assert a non-zero exit. Tests that only inspect printed text are insufficient.

### A5. Verify actual branch/ruleset enforcement

Configure `main` so the stable aggregate gate is required before merge. Prefer requiring the single fail-closed aggregate gate rather than an unstable list of matrix-generated check names.

Record the configuration with one of the following:

- repository ruleset export or API response;
- branch-protection API response;
- an administrator-authenticated `gh api` command and captured output;
- a screenshot only as supplemental evidence, never as the sole evidence when an API response is available.

The evidence must show:

- the protected branch or ruleset target includes `main`;
- `Required CI Gate` is a required status check using the exact emitted check name;
- administrators are not silently exempt unless that exemption is an explicit documented project policy;
- force pushes and branch deletion are disabled or explicitly justified;
- required checks must pass on the latest commit before merge.

If the implementing agent lacks repository-administration permission, it must not mark this track complete. It must provide the exact command/configuration needed and mark the criterion blocked pending an administrator action.

## Track B — Bind release dry runs to an immutable candidate

### B1. Add explicit candidate identity

The manually dispatched release workflow must accept or derive an explicit candidate commit SHA.

Preferred contract:

- input: `candidate_sha` containing a full 40-character Git commit SHA;
- input: `version`, such as `0.1.0-rc1`;
- input: `dry_run`, defaulting to `true` for manual validation;
- workflow dispatched from a ref that contains the candidate commit.

The verify job must fail before building anything unless:

- `candidate_sha` is exactly 40 lowercase or uppercase hexadecimal characters;
- the commit exists in the fetched repository;
- the checked-out `HEAD` equals `candidate_sha` after resolution;
- the candidate is reachable from the intended release branch according to project policy;
- the working tree is clean;
- all package versions and the requested release version satisfy the repository's version policy.

Every build job must check out the same SHA. Do not allow individual jobs to resolve a moving branch independently.

### B2. Prevent all dry-run side effects

When `dry_run=true`, the workflow must be technically incapable of:

- publishing crates;
- publishing wheels or source distributions;
- publishing npm packages;
- creating or mutating Git tags;
- creating a GitHub Release;
- uploading to TestPyPI unless a separate explicit non-default test-publication input is supplied;
- writing release metadata back to the repository;
- using production publishing environments or credentials.

Enforcement must occur in job-level and step-level conditions. A warning message or convention is insufficient.

The dry-run path should not request registry publication environments or secrets. Where GitHub environment protections are already configured, retain them for the real publishing path.

### B3. Preserve the exact build context

Generate a build-context record containing at least:

- repository;
- full candidate SHA;
- requested version;
- workflow name;
- workflow run ID and attempt;
- event type;
- dry-run flag;
- runner OS and architecture per artifact;
- Rust toolchain and target;
- Python and maturin versions for Python artifacts;
- Cargo.lock digest;
- release workflow file digest;
- UTC build timestamp.

This record must be included in the evidence manifest and retained as an artifact.

## Track C — Produce complete release evidence

### C1. Define the expected artifact matrix

Derive the expected matrix from `.github/workflows/release.yml` and the documented support policy. Put the expected set in a machine-readable configuration or in the evidence-generation script.

The matrix must explicitly name every artifact category expected from the current release process, including as applicable:

- Rust `.crate` package archives or package-validation outputs;
- Python wheels for each supported OS/architecture/Python ABI combination;
- Python source distribution;
- CLI binaries/archives for each supported release target;
- FFI libraries and public headers for each supported release target;
- Node artifacts only if the repository currently claims they are part of the RC release surface;
- SBOM files;
- checksum files;
- release manifest and validation reports.

Do not claim completion when a configured target is absent. If a target is intentionally deferred, update the support documentation and RC checklist in the same change and explain the decision.

### C2. Generate a machine-readable evidence manifest

Produce a final `release-evidence.json` or equivalently named artifact that contains, at minimum:

- schema version;
- candidate SHA;
- requested release version;
- workflow run ID and attempt;
- dry-run state;
- expected artifact entries;
- actual artifact entries;
- artifact filename, category, target, size, and SHA-256 digest;
- SBOM filename and digest for each applicable package/artifact;
- package validation result;
- smoke-test result;
- CI job/check result summary;
- generation timestamp;
- an overall `pass` field that is true only when every required criterion passed.

The manifest generator must fail on duplicate filenames, missing expected artifacts, unexpected empty artifacts, missing checksums, digest mismatches, or failed validation records.

### C3. Validate Rust packages

For every publishable Rust crate:

- run `cargo package --list`;
- reject forbidden files, generated corpora, secrets, local-only fixtures, build outputs, and unintended large assets;
- run `cargo package`;
- inspect the resulting `.crate` contents;
- run `cargo publish --dry-run` in dependency order where registry behavior permits;
- verify packaged sources build and test without relying on untracked workspace files;
- record the commands, crate versions, package archive hashes, and results in the evidence manifest.

A package passing in the workspace but failing from the packaged archive is a release blocker.

### C4. Validate Python artifacts

For every wheel produced:

- run `twine check`;
- install it into a new isolated environment appropriate for the target;
- run the full wheel smoke suite;
- verify reported package version;
- verify sync and async client import/construction;
- verify local HTTP request, headers, JSON, streaming, auth, retry, multipart, timeout/error mapping, and clean interpreter shutdown according to the existing smoke policy;
- record the exact wheel hash and smoke result.

For the source distribution:

- run `twine check`;
- install it into a clean build environment;
- confirm the resulting package imports and passes the required smoke subset;
- prove the build did not consume files absent from the sdist.

A single missing or untested wheel in the documented matrix is a release blocker.

### C5. Validate native CLI and FFI artifacts

For each native target produced by the workflow:

- extract the archive in a clean directory;
- verify archive contents and executable/library names;
- run `eggfetch --version` and `eggfetch --help`;
- run local GET, JSON-output, download/output, and deterministic exit-code smoke tests supported on that runner;
- confirm no unexpected dynamic-library dependency is introduced relative to documented policy;
- for FFI, verify the public header is present and compile/link a minimal C consumer where the target runner supports it;
- record artifact hashes and smoke results.

Cross-compilation without target-side execution does not satisfy runtime-smoke acceptance. Mark such targets separately and require an explicit policy decision if no native runner is available.

### C6. Validate SBOMs and checksums

For every release artifact:

- produce SHA-256 checksums;
- verify checksums after artifact aggregation, not only before upload;
- generate the configured SBOM format;
- ensure the SBOM identifies the candidate version and relevant package dependencies;
- include checksum and SBOM digests in the evidence manifest;
- fail if an expected SBOM or checksum is absent.

The final aggregation job must re-read all downloaded artifacts and independently recompute hashes.

## Track D — Execute the immutable dry run

### D1. Select and freeze the candidate

After all implementation changes are merged and CI is green:

1. identify the exact candidate SHA;
2. create an immutable validation ref according to project policy, preferably a non-release tag such as `rc-dry-run-<short-sha>` that cannot trigger publishing;
3. confirm the ref resolves to the full candidate SHA;
4. do not advance or recreate that ref during the validation run.

If repository policy prohibits temporary tags, use another immutable mechanism and document how immutability is enforced. A moving `main` branch name by itself is not sufficient evidence.

### D2. Run the release workflow

Dispatch `.github/workflows/release.yml` with:

- the exact immutable validation ref;
- `candidate_sha=<full SHA>`;
- `version=0.1.0-rc1` or the approved RC version;
- `dry_run=true`;
- every optional publication input disabled.

The run must complete without rerunning failed jobs against a different commit. A rerun is acceptable only when it uses the same workflow definition, run attempt lineage, and candidate SHA; record all attempts.

### D3. Retain evidence

Retain, at minimum:

- all produced release artifacts;
- release evidence manifest;
- checksums;
- SBOMs;
- Rust package-validation logs;
- wheel and sdist validation logs;
- CLI/FFI smoke logs;
- required-gate result;
- workflow job summary;
- run URL and run ID;
- candidate SHA and immutable validation ref.

Set artifact retention long enough to support release review and rollback analysis. Thirty days is the minimum unless repository policy specifies longer.

### D4. Prove absence of publishing side effects

After the dry run, verify and record that:

- no new crates.io version exists;
- no new PyPI/TestPyPI version exists unless explicitly authorized;
- no npm publication exists;
- no GitHub Release was created;
- no final RC tag was created;
- no production publishing environment deployment occurred;
- no repository files were mutated by the workflow.

Where external registry access is unavailable to the implementing agent, capture the workflow's skipped job evidence and require a maintainer to verify the registries before closure.

## Track E — Negative enforcement proof

### E1. Test the gate logic independently

Run the evaluator's negative tests in CI and locally. Preserve the test output as part of the implementation evidence.

### E2. Demonstrate a failed required job cannot yield a green gate

Use a safe, non-main test branch or dedicated workflow test that supplies synthetic failed job results to the evaluator. Do not intentionally weaken or break production code solely to test branch protection.

Required proof:

- synthetic required failure produces non-zero evaluator exit;
- the corresponding test job fails;
- the aggregate gate cannot report success when its real required dependency fails or is cancelled.

If a temporary pull request is used, close it after capturing the run and link it in the status document.

### E3. Verify merge protection

With branch protection/ruleset enabled, confirm that a pull request lacking a successful `Required CI Gate` cannot merge.

Acceptable evidence is an API/ruleset response plus a test pull request or repository ruleset evaluation showing merge blocked. Do not merge a deliberately failing PR.

## Track F — Documentation and handoff evidence

### F1. Update release process documentation

Update `docs/releases/process.md` and `docs/releases/rc-checklist.md` to specify:

- the stable required-gate name;
- the immutable candidate-SHA requirement;
- the exact dry-run dispatch procedure;
- expected artifact categories;
- evidence manifest location and schema;
- required post-run side-effect checks;
- the rule that an RC tag may only point to the successfully validated candidate SHA.

### F2. Add an evidence record

After the successful dry run, create:

`docs/releases/evidence/0.1.0-rc1-dry-run.md`

It must contain:

- candidate SHA;
- immutable validation ref;
- workflow run URL, ID, and attempt;
- date in UTC;
- required-gate result;
- artifact/evidence-manifest filename and digest;
- concise matrix result summary;
- side-effect verification result;
- any accepted limitations;
- reviewer/maintainer sign-off field.

Do not pre-populate this file with fabricated run IDs, hashes, or success claims. Create it only from actual retained workflow evidence.

### F3. Add implementation status

Create:

`plans/release-candidate-final-evidence-and-enforcement-status.md`

Map every acceptance criterion below to one of:

- PASS — with file, commit, workflow run, or artifact evidence;
- FAIL — with the observed failure;
- BLOCKED — with the external permission or infrastructure dependency;
- NOT RUN.

No criterion may be marked PASS based only on an implementation commit message.

## Required implementation sequence

1. Record baseline SHA and inventory the current CI/release job graph.
2. Implement and test the result evaluator.
3. Replace the informational-only aggregate behavior with the fail-closed gate.
4. Update documentation for the stable gate name.
5. Configure or request branch/ruleset enforcement and capture proof.
6. Add candidate-SHA verification to the release workflow.
7. Harden dry-run side-effect prevention.
8. Implement expected-artifact policy and evidence-manifest generation.
9. Add/complete package, wheel, sdist, CLI, FFI, checksum, and SBOM aggregation checks.
10. Run normal CI until the implementation commit is fully green without weakening tests.
11. Freeze the candidate SHA with an immutable validation ref.
12. Run the full release workflow with `dry_run=true`.
13. Validate retained artifacts and absence of publishing side effects.
14. Commit the evidence and status documents.
15. Re-run normal CI on the documentation/evidence commit if those files affect checks.
16. Only then recommend creation of the actual `0.1.0-rc1` tag at the validated candidate SHA or decide whether a second validation run is required because the evidence commit changed release-relevant files.

## Acceptance criteria

### CI enforcement

- [ ] A stable `Required CI Gate` job exists and uses `if: always()`.
- [ ] Every required CI job is an explicit dependency of the gate.
- [ ] Required failure, cancellation, absence, malformed result, or unexpected skip makes the gate fail.
- [ ] Conditional skips are explicitly modeled and tested.
- [ ] The gate uses no `continue-on-error` path that can mask failure.
- [ ] Gate evaluation is implemented in a repository script with automated positive and negative tests.
- [ ] A synthetic required-job failure is proven to produce a failed gate.
- [ ] `main` branch protection or a repository ruleset requires the exact stable gate check.
- [ ] Evidence shows a PR cannot merge without the successful gate.

### Immutable candidate

- [ ] The release workflow accepts or derives a full candidate SHA and validates it.
- [ ] Every release job checks out the same candidate SHA.
- [ ] The workflow fails when checked-out `HEAD` differs from the candidate SHA.
- [ ] The successful dry run uses an immutable validation ref resolving to that SHA.
- [ ] All evidence and artifacts identify the same full SHA.

### Dry-run safety

- [ ] `dry_run=true` is the safe default for manual RC validation.
- [ ] Registry publication, GitHub Release creation, final tag creation, and production environment deployment are technically gated off.
- [ ] Dry-run jobs do not request production publication credentials or environments.
- [ ] Post-run verification confirms no publishing or repository side effects occurred.

### Artifact and package evidence

- [ ] The expected artifact matrix is explicit and machine readable.
- [ ] Every expected artifact is present, non-empty, and uniquely named.
- [ ] Every artifact has a recomputed SHA-256 digest after aggregation.
- [ ] Every applicable artifact/package has an SBOM and recorded SBOM digest.
- [ ] All publishable Rust crates pass package-content validation and `cargo publish --dry-run` where supported.
- [ ] Packaged Rust sources build/test without untracked workspace dependencies.
- [ ] Every documented Python wheel passes `twine check`, clean installation, and smoke tests.
- [ ] The Python sdist passes `twine check`, clean build/install, and smoke validation.
- [ ] Every native CLI artifact passes extraction and runtime smoke tests on a native runner.
- [ ] Every supported FFI artifact includes its public header and passes the defined minimal consumer validation.
- [ ] The final evidence manifest fails closed on missing, duplicate, empty, or mismatched artifacts.

### Successful validation run

- [ ] One full release workflow run with `dry_run=true` succeeds against the immutable candidate SHA.
- [ ] The workflow's required release summary is green and fail closed.
- [ ] All required artifacts and validation logs are retained for at least 30 days.
- [ ] `release-evidence.json` reports overall pass and is internally consistent.
- [ ] The evidence record links the exact workflow run and records manifest/checksum digests.
- [ ] No criterion relies solely on a commit message or unchecked manual assertion.

### Final release gate

- [ ] No code, workflow, package metadata, build script, dependency lockfile, or release-relevant documentation changes exist after the validated candidate SHA without a new dry run.
- [ ] The proposed `0.1.0-rc1` tag target equals the successfully validated candidate SHA.
- [ ] All remaining limitations are explicitly documented and are consistent with public support claims.
- [ ] `plans/release-candidate-final-evidence-and-enforcement-status.md` contains no FAIL, BLOCKED, or NOT RUN entries.

## Stop conditions

Stop and do not recommend an RC tag when any of the following occurs:

- the aggregate gate can succeed after a required job fails or is cancelled;
- branch/ruleset enforcement cannot be proven;
- the dry-run workflow publishes or mutates release state;
- jobs build from different SHAs;
- any expected artifact is absent or untested;
- an artifact checksum or SBOM association is inconsistent;
- packaged code depends on workspace-only or untracked files;
- a supported wheel, CLI binary, or FFI artifact cannot pass its smoke test;
- the candidate changes after the successful validation run;
- evidence is unavailable, expired, or cannot be tied to the candidate SHA.

## Handoff commands and evidence guidance

Exact commands may be adapted to the implemented workflow inputs, but the handoff should provide equivalents of:

```bash
# Record candidate identity.
git rev-parse HEAD
git status --short

# Run repository validation before freezing the candidate.
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --exclude eggfetch-python
bash scripts/check_lint_suppressions.sh

# Create an immutable non-publishing validation ref after CI is green.
git tag rc-dry-run-$(git rev-parse --short=12 HEAD) $(git rev-parse HEAD)
git push origin rc-dry-run-$(git rev-parse --short=12 HEAD)

# Dispatch the manual dry run; adjust input names to the final workflow contract.
gh workflow run release.yml \
  --ref rc-dry-run-$(git rev-parse --short=12 HEAD) \
  -f candidate_sha=$(git rev-parse HEAD) \
  -f version=0.1.0-rc1 \
  -f dry_run=true

# Inspect and retain the run.
gh run list --workflow release.yml --limit 5
gh run view <run-id> --json databaseId,headSha,conclusion,jobs,url
gh run download <run-id> --dir release-dry-run-evidence

# Verify branch protection/ruleset configuration when permissions allow.
gh api repos/eggstack/eggfetch/branches/main/protection
# or inspect the applicable repository ruleset API response.
```

Never place registry tokens, signing secrets, private keys, or other credentials in the evidence directory or committed status document.

## Completion definition

This pass is complete only when the repository has both:

1. enforceable, proven fail-closed merge protection; and
2. a successful, retained, SHA-bound, non-publishing release dry run whose artifacts and validation results satisfy every criterion above.

Implementation alone is not closure. The retained workflow evidence and branch-protection proof are required outputs.