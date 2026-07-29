# Final Validator Closure and PyPI Multi-Platform Wheel Pipeline

Status: detailed implementation handoff plan

Audited baseline commit: `d04fb77319e88831bd8162992daa7f7608e7d235`

Audit date: 2026-07-29

Target repository: `eggstack/eggfetch`

Parent plans:

- `plans/ci-verification-and-manual-release-simplification.md`
- `plans/ci-validation-truthfulness-corrective-pass.md`
- `plans/ci-package-validation-final-closure.md`
- `plans/ci-final-portability-and-operational-verification-closure.md`

Primary implementation surfaces:

- `scripts/validate_publishable_internal_dependencies.py`
- `scripts/check.sh`
- `.github/workflows/ci.yml`
- new `.github/workflows/pypi.yml`
- `crates/eggfetch-python/Cargo.toml`
- `crates/eggfetch-python/pyproject.toml`
- `scripts/wheel_smoke.py`
- optionally one narrowly focused release-version validator under `scripts/`
- `docs/verification-policy.md`
- `docs/architecture/build-ci.md`
- `docs/releases/process.md`
- optionally `docs/releases/pypi.md`
- GitHub branch protection and rulesets
- GitHub Actions secrets and environments
- PyPI Trusted Publisher configuration for the `eggfetch` project

## 1. Purpose

The CI simplification work is structurally successful: ordinary push and pull-request validation remains a single Ubuntu job, crates.io publication is manual, local package validation is fail-closed, and wheel smoke/content validation uses one exact current-run wheel.

The remaining work now has two related but distinct goals:

1. close the last correctness gaps in the publishable-internal-dependency validator and canonical package command;
2. add a deliberately isolated, manually invoked GitHub Actions pipeline that builds, tests, assembles, and optionally publishes multi-platform Python wheels to PyPI.

The new PyPI workflow is explicitly authorized by this plan. It must not undo the earlier simplification:

- `.github/workflows/ci.yml` remains the only workflow triggered by pushes and pull requests;
- the PyPI workflow is release-only and manually invoked;
- crates.io remains fully manual and outside GitHub Actions;
- no workflow may contain a real `cargo publish` command;
- no PyPI API token should be stored in GitHub Actions when Trusted Publishing is available.

## 2. Current state and defects

At baseline `d04fb77319e88831bd8162992daa7f7608e7d235`:

- `scripts/validate_publishable_internal_dependencies.py` uses `cargo metadata` and the Python standard library, which correctly removes the earlier GNU/BSD grep portability defect;
- the helper rejects missing, empty, and exact `*` requirements;
- the helper does not enforce the exact expected dependency topology for every dependent crate;
- `eggfetch-ffi` and `eggfetch-node` may contain no internal dependency and still pass;
- a crate may depend on the wrong internal crate and still pass if that dependency belongs to the broad internal-dependency set;
- wildcard requirements such as `0.*` or `1.*` are not rejected;
- `scripts/check.sh package` permanently passes `--allow-dirty` to Cargo package commands, weakening the clean-release-worktree contract;
- no multi-platform release wheel workflow exists;
- `pyproject.toml` declares Python 3.10 through 3.13 support;
- the PyO3 dependency does not currently declare an ABI3 minimum-version feature, so this plan must build interpreter-specific wheels rather than pretending one wheel covers every supported Python version.

## 3. Scope

### 3.1 Included

This pass includes:

- enforcing the exact publishable internal dependency map;
- rejecting every wildcard version requirement;
- requiring local path plus concrete version requirements for internal dependencies;
- removing permanent `--allow-dirty` use from canonical package validation;
- preserving fail-closed package and exactly-one-wheel behavior;
- completing the previously required CI, branch-rule, secret, and environment verification;
- adding a separate manual PyPI workflow;
- building wheels for Python 3.10, 3.11, 3.12, and 3.13;
- building for Linux x86_64 and aarch64, macOS x86_64 and arm64, and Windows x86_64;
- testing each wheel on its native operating system and architecture where GitHub-hosted runners support it;
- assembling and validating the full distribution set before upload;
- publishing through PyPI Trusted Publishing with a protected GitHub environment;
- keeping build-only workflow rehearsals available without publishing;
- documenting the exact crates.io/PyPI release sequence.

### 3.2 Excluded

This pass does not include:

- automated crates.io publication;
- crates.io credentials in GitHub Actions;
- npm or Node package publication;
- GitHub Release creation or tag creation by CI;
- ordinary push/PR wheel matrices;
- musllinux wheels;
- 32-bit Linux or Windows wheels;
- Windows ARM64 wheels;
- PyPy, GraalPy, free-threaded CPython, Android, iOS, or WebAssembly wheels;
- Python 3.14 support until project metadata and tests explicitly adopt it;
- converting the bindings to ABI3 as part of this pass;
- automatic release cadence;
- restoring qualification/evidence workflow architecture;
- adding a release orchestrator or task-runner framework.

## 4. Non-negotiable design decisions

1. `.github/workflows/ci.yml` remains the only push/PR workflow.
2. Routine CI remains one Ubuntu job named `ci` with no matrix.
3. The new PyPI workflow is stored separately as `.github/workflows/pypi.yml`.
4. The PyPI workflow is triggered only through `workflow_dispatch`.
5. A build-only dispatch may run from any selected ref.
6. A publishing dispatch must run from a version tag matching `v<SEMVER>`.
7. PyPI publication remains an explicit maintainer action; it is not triggered automatically by pushes, pull requests, or tag creation.
8. PyPI upload uses Trusted Publishing/OIDC, not a long-lived API token.
9. Only the final publish job receives `id-token: write`.
10. Build and test jobs receive read-only repository permissions.
11. The publish job uses a GitHub environment named `pypi` with required reviewers.
12. Crates.io publication remains manual and occurs outside GitHub Actions.
13. No workflow contains `cargo publish` without `--dry-run`; preferably no PyPI workflow contains any `cargo publish` command at all.
14. The Python release version must match the selected tag and coordinated Cargo versions.
15. Because ABI3 is not currently configured, each supported Python version receives a distinct native wheel.
16. Wheel builds and uploads use artifacts produced by the same workflow run.
17. The workflow never downloads or uploads an artifact from an unrelated run.
18. Existing local `./scripts/check.sh package` remains the canonical package correctness check.
19. The PyPI workflow supplements local package validation; it does not replace it.
20. Third-party GitHub Actions are pinned to immutable commit SHAs.

## 5. Phase 1: Correct the internal dependency validator

### 5.1 Replace broad sets with an exact topology map

Replace the current broad logic with an explicit expected dependency map:

```python
EXPECTED_DEPENDENCIES = {
    "eggfetch-cli": {"eggfetch-core"},
    "eggfetch-ffi": {"eggfetch-core"},
    "eggfetch-python": {"eggfetch-core"},
    "eggfetch-node": {"eggfetch-ffi"},
}
```

For each package:

1. require the package to exist in `cargo metadata` output;
2. identify internal dependencies by actual package name;
3. require the set of internal dependencies to equal the expected set exactly;
4. fail if an expected dependency is missing;
5. fail if the package depends on the wrong internal crate;
6. fail if an unexpected publishable internal dependency appears without updating the explicit map;
7. report the package, expected set, and observed set in the failure.

The validator must not infer that the absence of an internal dependency is acceptable for any listed package.

### 5.2 Require both local path and concrete registry requirement

For each expected internal dependency record, require:

- a non-null local `path`, preserving workspace development against local source;
- a non-null `req` version requirement;
- a non-empty requirement;
- a requirement containing no `*` character;
- a requirement that is compatible with the current local package version.

`cargo metadata` already fails when the path package version cannot satisfy the declared requirement. Preserve that property by treating metadata failure as fatal.

Do not add a third-party semantic-version parser merely to duplicate Cargo's own resolution.

### 5.3 Handle renamed dependencies deliberately

Cargo metadata dependency records may contain both package names and local rename information. The helper must validate the actual package dependency, not only the source-code alias.

A renamed internal dependency is acceptable only when:

- the actual package is the expected package;
- the path and concrete version requirement remain present;
- the rename is reported in the successful diagnostic output.

Do not silently treat an unrelated package renamed to `eggfetch-core` as the expected dependency.

### 5.4 Improve diagnostics

Successful output should identify each validated edge, for example:

```text
eggfetch-cli -> eggfetch-core req=^0.1.0 path=crates/eggfetch-core
eggfetch-ffi -> eggfetch-core req=^0.1.0 path=crates/eggfetch-core
eggfetch-python -> eggfetch-core req=^0.1.0 path=crates/eggfetch-core
eggfetch-node -> eggfetch-ffi req=^0.1.0 path=crates/eggfetch-ffi
```

Failure output must name:

- the dependent crate;
- the expected internal dependency;
- the observed dependency or missing field;
- the invalid requirement when safe to print.

### 5.5 Focused controlled-failure verification

Perform reversible local checks without committing broken manifests:

1. remove the `eggfetch-core` dependency from `eggfetch-ffi`; validator must fail;
2. replace `eggfetch-node -> eggfetch-ffi` with `eggfetch-node -> eggfetch-core`; validator must fail;
3. change one requirement to `*`; validator must fail;
4. change one requirement to `0.*`; validator must fail;
5. remove the `version` field but retain `path`; validator must fail;
6. remove the `path` field but retain `version`; validator must fail under the local-workspace contract;
7. restore all manifests and rerun successfully.

The focused helper may be run against the dirty temporary worktree. Canonical package mode must not be weakened to accommodate these tests.

### Phase 1 acceptance criteria

- the exact four-edge dependency map is encoded explicitly;
- all four dependent crates are required to have their expected internal dependency;
- wrong-topology dependencies fail;
- missing internal dependencies fail for `eggfetch-ffi` and `eggfetch-node` as well as CLI/Python;
- unexpected internal dependency edges fail;
- exact `*` fails;
- partial wildcards such as `0.*` and `1.*` fail;
- empty requirements fail;
- path-only dependencies fail;
- registry-only dependencies fail under the current local-workspace contract;
- metadata failure is nonzero and visible;
- no new third-party Python dependency is introduced;
- the helper remains compatible with Python 3.10+;
- the successful output enumerates all four expected edges.

## 6. Phase 2: Restore the clean-worktree package contract

### 6.1 Remove permanent `--allow-dirty`

Remove `--allow-dirty` from canonical commands in `scripts/check.sh`:

```sh
cargo publish -p eggfetch-core --dry-run
cargo package --list -p "$crate"
```

The release documentation already requires a clean worktree. The script must enforce Cargo's default clean-worktree behavior rather than bypass it.

### 6.2 Implementation workflow for the agent

Because modifying the script itself makes the implementation worktree dirty, validate in this order:

1. run focused helper tests before committing;
2. restore all intentionally modified manifests;
3. commit the implementation changes;
4. confirm the worktree is clean;
5. run `./scripts/check.sh`;
6. run `./scripts/check.sh package`;
7. amend only if necessary, then repeat from a clean worktree.

Do not add a hidden environment variable that automatically enables dirty packaging.

### 6.3 Preserve existing package invariants

Do not change:

- `eggfetch-core` publish dry-run;
- dependent-crate `cargo package --list` checks;
- exact current-run wheel selection;
- wheel smoke against the exact selected wheel;
- package-content validation against the same wheel;
- temporary artifact cleanup;
- nonzero propagation under `set -euo pipefail`.

### Phase 2 acceptance criteria

- no canonical Cargo package command uses `--allow-dirty`;
- a dirty worktree causes package mode to fail before claiming success;
- a clean worktree allows package mode to proceed;
- no failed Cargo command is converted to a warning;
- package mode still checks all five publishable crates;
- package mode still requires exactly one wheel;
- package mode prints success only after every selected check passes.

## 7. Phase 3: Add release-version coherence validation

Before adding publication, create or extend one narrowly scoped version validator. A dedicated standard-library Python helper is preferred, for example:

```text
scripts/validate_release_versions.py
```

### 7.1 Inputs and sources

The helper must inspect:

- the selected Git ref/tag supplied by the workflow;
- workspace publishable Cargo package versions from `cargo metadata`;
- `crates/eggfetch-python/pyproject.toml` project version;
- `crates/eggfetch-python/Cargo.toml` package version.

Because the repository supports Python 3.10, do not require Python's built-in `tomllib` unless the workflow explicitly runs the helper under Python 3.11+. A small targeted parser may instead consume:

- `cargo metadata` for Cargo versions;
- `maturin` or build metadata for Python project version;
- a simple exact `[project] version =` extraction only if tested and isolated.

Alternatively, run the release-version helper under Python 3.12 in the release workflow and use `tomllib`. This does not alter the package's runtime Python support.

### 7.2 Required coherence

For a publishing run:

- the selected ref must be a tag;
- the tag must match `v<SEMVER>`;
- tag version must equal `pyproject.toml` project version;
- `eggfetch-python` Cargo version must equal the Python project version;
- all coordinated publishable crates must have the same version;
- the version must not contain an unreleasable placeholder;
- the workflow must fail before wheel builds on mismatch.

For a build-only rehearsal:

- branch and commit refs are allowed;
- version coherence among package files is still required;
- tag matching is not required.

### 7.3 PyPI immutability

Do not use `skip-existing` during the final PyPI upload. If a version already exists, publishing must fail visibly. The maintainer must bump the version and produce a fresh release.

### Phase 3 acceptance criteria

- version mismatch fails before matrix builds;
- publishing from a branch fails;
- publishing from a non-version tag fails;
- a matching `vX.Y.Z` tag passes;
- all coordinated crate versions are checked;
- Python and Cargo package versions are checked;
- no existing PyPI file is silently skipped.

## 8. Phase 4: Add the manual PyPI workflow

Create:

```text
.github/workflows/pypi.yml
```

Suggested workflow name:

```yaml
name: PyPI Wheels
```

### 8.1 Trigger contract

Use only:

```yaml
on:
  workflow_dispatch:
    inputs:
      publish:
        description: Publish validated distributions to PyPI
        required: true
        type: boolean
        default: false
```

Do not add `push`, `pull_request`, `release`, or automatic tag triggers.

GitHub's workflow-dispatch UI allows the maintainer to choose the ref. Document:

- choose a branch or commit for a build-only rehearsal;
- choose a `vX.Y.Z` tag and set `publish=true` for publication.

### 8.2 Workflow permissions

Set a restrictive workflow-level default:

```yaml
permissions:
  contents: read
```

Use job-level permissions for publication:

```yaml
permissions:
  contents: read
  id-token: write
```

No build job receives `id-token: write`.

Use `actions/checkout` with credentials persistence disabled:

```yaml
with:
  persist-credentials: false
```

### 8.3 Validation job

Add one initial job, for example `validate-release`, on Ubuntu. It must:

1. check out the exact selected ref;
2. install the stable Rust toolchain;
3. install Python 3.12 or later for workflow tooling;
4. create and activate a virtual environment;
5. install only required release tools with pinned compatible versions;
6. run the release-version validator;
7. run `./scripts/check.sh`;
8. run the corrected internal-dependency validator;
9. optionally run `./scripts/check.sh package` when the additional local package build cost is acceptable.

Preferred contract: run full `./scripts/check.sh package` once in this job because the workflow is manual and release-oriented. This validates Cargo package structure and one native Linux wheel before the platform matrix begins.

All wheel build jobs depend on `validate-release`.

## 9. Phase 5: Define the supported wheel matrix

The initial PyPI support contract is:

| Operating system | Architecture | Python versions | Wheel policy |
|---|---|---|---|
| Linux manylinux2014 | x86_64 | 3.10, 3.11, 3.12, 3.13 | native/tested |
| Linux manylinux2014 | aarch64 | 3.10, 3.11, 3.12, 3.13 | native ARM runner/tested |
| macOS | x86_64 | 3.10, 3.11, 3.12, 3.13 | native/tested |
| macOS | arm64 | 3.10, 3.11, 3.12, 3.13 | native/tested |
| Windows | x86_64 | 3.10, 3.11, 3.12, 3.13 | native/tested |

Expected wheel count: 20.

### 9.1 Linux x86_64

Use an Ubuntu x86_64 runner with `PyO3/maturin-action`, configured for:

- target `x86_64` or the action's documented x86_64 Rust target;
- manylinux2014 compatibility;
- one selected interpreter per matrix job;
- manifest `crates/eggfetch-python/Cargo.toml`;
- release build;
- output to a job-local `dist/` directory.

Do not publish a wheel tagged only `linux_x86_64`; require a manylinux-compatible tag.

### 9.2 Linux aarch64

Prefer a native GitHub-hosted ARM runner such as the currently supported Ubuntu ARM runner. Build and test natively.

If the repository/account cannot access a native ARM runner:

- cross-build with maturin-action only as a temporary fallback;
- mark the wheel as built but not natively smoke-tested;
- do not claim full closure until a native or equivalent controlled ARM test is obtained.

The preferred implementation for this plan is native ARM.

### 9.3 macOS x86_64

Use a current Intel macOS GitHub-hosted runner. Set up the selected Python version and build the x86_64 wheel.

Do not build Intel wheels on an arm64 runner through ad hoc environment manipulation unless the action's documented cross-build mode is used and tested.

### 9.4 macOS arm64

Use a current Apple Silicon runner. Set up the selected Python version and build/test natively.

### 9.5 Windows x86_64

Use `windows-latest` with x64 Python. Build and test the `.whl` natively.

Do not add x86 or ARM64 Windows until explicitly adopted.

### 9.6 Python interpreter policy

Because the project does not currently enable ABI3:

- build one wheel per Python version per platform/architecture;
- do not label a wheel `abi3` unless the PyO3 feature contract is deliberately changed and separately validated;
- do not use `--find-interpreter` in a way that silently builds unexpected Python versions;
- make the interpreter version explicit in the matrix and artifact name.

An ABI3 conversion may be proposed later as a separate optimization after compatibility and performance testing. It is not part of this implementation.

### Phase 5 acceptance criteria

- exactly five platform/architecture combinations are defined;
- exactly four Python versions are defined;
- exactly 20 wheel jobs or equivalent deterministic wheel outputs are produced;
- all Linux wheels carry valid manylinux tags;
- macOS Intel and arm64 wheels are distinct;
- Windows wheels are x86_64 only;
- no unsupported Python or platform is built accidentally;
- no wheel claims ABI3 compatibility without ABI3 configuration.

## 10. Phase 6: Test every built wheel

Each wheel build job must test its own current-run wheel before artifact upload.

### 10.1 Exact artifact selection

Require exactly one wheel in the job's `dist/` directory. Reuse or generalize the existing exact-wheel selection semantics.

Zero or multiple wheels must fail.

### 10.2 Native smoke test

Invoke:

```sh
python scripts/wheel_smoke.py --wheel <exact-wheel-path>
```

Use the same Python version represented by the wheel.

The smoke test already creates an isolated virtual environment and uses a local HTTP server. Preserve that network-isolated behavior.

### 10.3 Metadata and tag validation

Add a lightweight standard-library or release-tool check that confirms:

- distribution name is `eggfetch`;
- version equals the validated release version;
- Python tag matches the matrix interpreter;
- ABI tag is expected;
- platform tag matches the job target;
- the wheel contains `eggfetch._native` and required Python package files.

Do not merely trust the filename generated by the build tool.

### 10.4 Upload build artifacts

After tests pass, upload the wheel as a GitHub Actions artifact with a unique name, for example:

```text
wheel-linux-x86_64-py310
wheel-linux-aarch64-py313
wheel-macos-x86_64-py312
wheel-macos-arm64-py311
wheel-windows-x86_64-py310
```

Use short retention suitable for release handoff, such as 7 days.

### Phase 6 acceptance criteria

- every wheel is smoke-tested before upload;
- smoke testing uses the exact current-run wheel;
- smoke testing uses the matching Python version;
- zero or multiple wheels fail the job;
- metadata/version/tag mismatch fails;
- artifact names are unique and deterministic;
- failed wheels are not uploaded as successful release artifacts.

## 11. Phase 7: Build and validate the source distribution

Although the primary request is multi-platform wheels, a PyPI source distribution is valuable only if it is self-contained and buildable.

Add one Ubuntu `sdist` job depending on `validate-release`.

### 11.1 Build

Use maturin to build exactly one source distribution from the Python binding manifest into `dist/`.

Require exactly one `.tar.gz` source distribution.

### 11.2 Self-contained build test

In a clean temporary directory:

1. create an isolated virtual environment;
2. preinstall the build backend required by `pyproject.toml`;
3. run `python -m pip wheel --no-deps --no-build-isolation <sdist>`;
4. ensure the build succeeds without depending on the repository checkout;
5. install the resulting wheel;
6. run the compact smoke test.

This proves the sdist includes or can resolve every required Rust source dependency.

### 11.3 Failure policy

A broken sdist must never be published.

If maturin cannot produce a self-contained sdist because of workspace path dependencies:

- treat the sdist job as a release blocker for this plan;
- correct the sdist packaging inputs narrowly;
- do not replace the local dependency with an automated crates.io publication step;
- do not upload a known-broken sdist;
- do not silently omit the sdist without documenting and obtaining explicit maintainer approval.

### Phase 7 acceptance criteria

- exactly one sdist is produced;
- the sdist builds outside the repository checkout;
- the resulting wheel installs and passes smoke testing;
- the sdist version matches the wheel version;
- a broken sdist blocks publication;
- no GitHub Actions crates.io publication is introduced.

## 12. Phase 8: Assemble and validate the release set

Add an `assemble` job depending on all wheel jobs and the sdist job.

### 12.1 Download artifacts from the same run

Download artifacts produced by the current workflow run and merge them into one clean `dist/` directory.

Do not use release assets, prior-run artifacts, repository `dist/`, or external storage.

### 12.2 Cardinality

Require:

- 20 wheels;
- 1 sdist;
- 21 total distributions;
- no duplicate filenames;
- no duplicate platform/Python coverage tuples;
- no unexpected distribution type.

### 12.3 Package validation

Run:

```sh
python -m twine check dist/*
```

Also run a focused coverage verifier that compares observed wheel tags with the expected matrix.

The verifier must report missing and unexpected tuples explicitly.

### 12.4 Re-upload assembled release set

Upload one assembled artifact, for example:

```text
pypi-distributions
```

This artifact is the only input to the publish job.

### Phase 8 acceptance criteria

- only current-run artifacts are assembled;
- exactly 20 wheels and one sdist are present;
- duplicate names fail;
- missing matrix coverage fails;
- unexpected matrix coverage fails;
- `twine check` passes for every distribution;
- one assembled artifact is produced for the publish job.

## 13. Phase 9: Publish through PyPI Trusted Publishing

### 13.1 PyPI project setup

Configure a Trusted Publisher for the PyPI project `eggfetch` with:

- owner: `eggstack`;
- repository: `eggfetch`;
- workflow filename: `pypi.yml`;
- GitHub environment: `pypi`.

Use the exact filename registered with PyPI. Renaming the workflow later requires updating the Trusted Publisher configuration.

### 13.2 GitHub environment

Create or retain the `pypi` environment with:

- required maintainer reviewers;
- deployment branch/tag restrictions permitting only version tags used by the release process;
- no long-lived PyPI token secret;
- no crates.io credential;
- a concise environment description stating that it authorizes PyPI OIDC publication only.

This environment is intentionally retained and is no longer considered obsolete release-workflow residue.

### 13.3 Publish job conditions

The publish job must run only when all are true:

- `inputs.publish == true`;
- the selected ref is a tag;
- the tag matches `v<SEMVER>`;
- release-version validation passed;
- all wheel jobs passed;
- sdist validation passed;
- assembly passed;
- environment approval was granted.

### 13.4 Upload action

Use the official PyPA publication action with Trusted Publishing. Pin the action to an immutable commit SHA corresponding to the reviewed `release/v1` version.

The job must:

1. download only the assembled `pypi-distributions` artifact;
2. verify expected cardinality again;
3. publish without username, password, or token inputs;
4. avoid `skip-existing`;
5. fail if PyPI rejects any file;
6. print the final PyPI project/version URL in the handoff output without embedding credentials.

### 13.5 Build-only runs

When `publish=false`:

- all validation, build, smoke, sdist, and assembly jobs run;
- the publish job is skipped by condition;
- the assembled artifact remains available for inspection;
- the workflow conclusion should be successful when all build checks pass.

### Phase 9 acceptance criteria

- PyPI uses Trusted Publishing/OIDC;
- no `PYPI_TOKEN`, password, or username is required;
- only the publish job has `id-token: write`;
- the `pypi` environment requires approval;
- branch/commit dispatches cannot publish;
- non-version tags cannot publish;
- build-only rehearsals complete without upload;
- publication consumes only the assembled current-run artifact;
- existing versions fail rather than skip;
- no crates.io command or credential appears in the workflow.

## 14. Phase 10: Update release and CI documentation

### 14.1 Verification policy

Update `docs/verification-policy.md` to distinguish:

- automatic routine CI: `.github/workflows/ci.yml`, push/PR, one Ubuntu job;
- manual PyPI release CI: `.github/workflows/pypi.yml`, workflow dispatch only;
- local canonical checks: `./scripts/check.sh`, `extended`, and `package`;
- manual crates.io publication.

Do not describe the PyPI workflow as a required merge check.

### 14.2 CI architecture

Update `docs/architecture/build-ci.md` with a compact architecture description:

```text
Push/PR -> CI / ci -> routine validation
Manual dispatch -> PyPI Wheels -> release validation -> wheel/sdist matrix -> assembly -> optional approved PyPI upload
Local maintainer -> manual crates.io publication
```

Document the supported wheel matrix and why it is release-only.

### 14.3 Release process

Update `docs/releases/process.md` with the authoritative sequence:

1. bump all coordinated versions;
2. update changelog;
3. run routine, extended as appropriate, and package validation locally;
4. manually publish crates.io packages in dependency order;
5. verify crates.io propagation;
6. create and push signed `vX.Y.Z` tag;
7. dispatch `PyPI Wheels` from that tag with `publish=false` for final rehearsal when desired;
8. inspect the assembled artifacts;
9. dispatch from the same tag with `publish=true`;
10. approve the `pypi` environment deployment;
11. verify the PyPI release and installation on representative platforms.

Crates.io remains manual even if PyPI uses GitHub Actions.

### 14.4 Dedicated PyPI guide

A small `docs/releases/pypi.md` is acceptable when it reduces clutter. It should cover:

- supported wheel matrix;
- workflow-dispatch procedure;
- Trusted Publisher configuration fields;
- environment approval;
- build-only versus publish runs;
- immutable-version correction procedure;
- how to inspect artifacts and run post-publish installation checks.

Do not add an evidence schema, candidate manifest, or release-authorization framework.

### Phase 10 acceptance criteria

- docs accurately distinguish automatic CI and manual release CI;
- the one-job push/PR policy remains explicit;
- crates.io publication remains explicitly manual;
- PyPI publication is explicitly manually dispatched and environment-approved;
- supported Python/platform coverage matches workflow behavior;
- no docs reference a PyPI API-token secret;
- no docs imply tag creation automatically publishes;
- immutable-version handling is explicit.

## 15. Phase 11: Complete operational verification

Perform these checks after all workflow changes land.

### 15.1 Routine CI

For final implementation SHA:

- confirm `CI / ci` ran successfully;
- confirm exactly one job exists;
- confirm no matrix exists;
- confirm the new PyPI workflow did not run automatically;
- record run URL, job name, conclusion, and duration.

### 15.2 Build-only PyPI rehearsal

Dispatch `.github/workflows/pypi.yml` with `publish=false` from the final implementation branch or commit.

Confirm:

- validation passed;
- all 20 wheels were produced;
- all native wheel smoke tests passed;
- sdist validation passed;
- assembly found exactly 21 distributions;
- publish job was skipped;
- assembled artifact is downloadable;
- no OIDC permission was granted to build jobs.

### 15.3 Branch protection and rulesets

Inspect both legacy branch protection and repository rulesets.

Confirm:

- no deleted check context remains;
- only `CI / ci`, if any status check is required, gates main;
- the PyPI workflow is not a branch-protection requirement;
- no release matrix job is required for pull requests.

### 15.4 Actions secrets

List secret names only. Remove obsolete:

- PyPI API tokens;
- TestPyPI tokens not actively used;
- crates.io tokens;
- npm release tokens;
- credentials used only by deleted workflows.

Trusted Publishing requires no PyPI token secret.

### 15.5 Environments

Inspect repository environments:

- remove obsolete release environments;
- retain or create only the intentional `pypi` environment for this workflow;
- verify reviewer and tag restrictions;
- document retained purpose.

### 15.6 Initial publishing verification

For the first real release through this workflow:

- dispatch from a matching signed version tag with `publish=true`;
- approve the environment;
- verify PyPI contains exactly the expected files;
- install from PyPI on representative Linux, macOS, and Windows environments;
- verify `eggfetch.__version__` matches the release;
- run one compact local HTTP request smoke test.

### Phase 11 acceptance criteria

- final routine CI is green;
- routine CI still has one job;
- PyPI workflow does not run on push/PR;
- build-only release rehearsal is green;
- 20 wheel artifacts and one sdist are assembled;
- publish job is skipped during rehearsal;
- branch rules contain no stale contexts;
- PyPI workflow is not a merge gate;
- obsolete publication secrets are absent;
- `pypi` environment is deliberately configured and protected;
- any inaccessible setting is reported precisely rather than assumed clean.

## 16. Security requirements

1. Pin every third-party action to a full commit SHA.
2. Keep workflow-level permissions read-only.
3. Grant `id-token: write` only to the final publish job.
4. Use `persist-credentials: false` for checkout.
5. Never execute publication from pull-request code.
6. Require a trusted version tag and protected environment approval.
7. Do not use long-lived PyPI API tokens.
8. Do not store crates.io tokens in Actions.
9. Do not print OIDC tokens or minted credentials.
10. Do not use `pull_request_target`.
11. Do not use artifacts from another workflow run.
12. Do not allow arbitrary user-supplied shell arguments in workflow inputs.
13. Keep the only input a typed boolean unless a future requirement justifies more.
14. Do not use `skip-existing` to conceal immutable-version mistakes.

## 17. Suggested implementation commits

Keep the implementation reviewable with approximately these commits:

1. `fix: enforce exact publishable dependency topology`
   - exact map;
   - wildcard rejection;
   - path/version checks;
   - controlled validator verification.

2. `fix: restore clean package validation contract`
   - remove `--allow-dirty`;
   - preserve package/wheel invariants.

3. `ci: add manual multi-platform PyPI wheel workflow`
   - version validator;
   - validation job;
   - 20-wheel matrix;
   - sdist;
   - assembly;
   - build-only behavior;
   - OIDC publish job.

4. `docs: document manual crates and approved PyPI release flow`
   - verification policy;
   - CI architecture;
   - release process;
   - optional focused PyPI guide.

Repository-settings changes are operational and may not produce commits.

## 18. Rejection conditions

Reject the implementation if any of the following is true:

- `eggfetch-ffi` or `eggfetch-node` can omit their expected internal dependency and still pass;
- the validator accepts the wrong internal dependency topology;
- a requirement containing `*` passes;
- canonical package validation retains `--allow-dirty`;
- the new wheel workflow runs on every push or pull request;
- routine CI gains a matrix or additional jobs;
- crates.io publication is added to GitHub Actions;
- any crates.io credential is added to Actions;
- PyPI publication uses a long-lived token despite Trusted Publishing availability;
- build jobs receive `id-token: write`;
- publishing is possible from a branch or arbitrary tag;
- publication can occur without environment approval;
- wheels are uploaded without native smoke testing where native runners are available;
- Linux wheels are published with non-portable `linux_*` tags;
- the release set lacks one of the supported platform/Python combinations;
- multiple or stale wheels can satisfy a job;
- a broken sdist is uploaded;
- an existing PyPI version is silently skipped;
- the PyPI workflow becomes a branch-protection requirement;
- documentation implies crates.io is automated;
- operational settings are declared clean without direct evidence.

## 19. Final closure criteria

This line of work is complete only when all criteria below are satisfied.

### Validator correctness

1. Exact internal dependency topology is enforced.
2. All four dependent crates require the correct internal crate.
3. Wrong-topology dependencies fail.
4. Missing internal dependencies fail for every dependent crate.
5. Unexpected internal dependency edges fail.
6. Every expected dependency has a local path.
7. Every expected dependency has a concrete version requirement.
8. Exact and partial wildcard requirements fail.
9. Metadata errors fail visibly.
10. The validator uses only standard-library Python plus Cargo metadata.
11. Successful output enumerates all four validated edges.

### Local package integrity

12. Canonical package mode contains no `--allow-dirty`.
13. Dirty package worktrees fail.
14. Clean package validation passes.
15. All five publishable crates receive a real check.
16. Exactly-one-wheel validation remains intact.
17. Smoke and content validation inspect the same wheel.
18. No package failure is converted to success.

### Routine CI simplicity

19. `.github/workflows/ci.yml` remains the only push/PR workflow.
20. Routine CI remains one Ubuntu job named `ci`.
21. Routine CI has no matrix.
22. Routine CI calls `./scripts/check.sh`.
23. Routine CI has no publication permissions or credentials.

### PyPI wheel generation

24. `.github/workflows/pypi.yml` is manual-dispatch only.
25. Build-only mode is supported.
26. Publishing requires `publish=true` and a matching version tag.
27. Python 3.10–3.13 are built explicitly.
28. Linux x86_64 and aarch64 manylinux wheels are built.
29. macOS x86_64 and arm64 wheels are built.
30. Windows x86_64 wheels are built.
31. Exactly 20 wheels are produced.
32. Every wheel is natively smoke-tested where supported.
33. Wheel metadata and tags are validated.
34. Exactly one self-contained sdist is produced and tested.
35. Assembly contains exactly 21 distributions.
36. `twine check` passes for all distributions.
37. Missing, duplicate, or unexpected coverage fails.

### PyPI publication security

38. PyPI uses Trusted Publishing/OIDC.
39. No long-lived PyPI token is stored.
40. Only the publish job has `id-token: write`.
41. The publish job uses the protected `pypi` environment.
42. Environment approval is required.
43. Publishing from branches is impossible.
44. Publishing from non-version tags is impossible.
45. Existing versions are not skipped.
46. Publication consumes only current-run assembled artifacts.

### Crates.io policy

47. Crates.io remains manually published by a maintainer.
48. No GitHub workflow performs crates.io publication.
49. No crates.io credential is present in Actions.
50. Manual dependency-order publication remains documented.

### Operational closure

51. Final `CI / ci` run is green and recorded.
52. Exactly one routine CI job ran.
53. Build-only PyPI rehearsal is green and recorded.
54. All expected artifacts are present in rehearsal.
55. Branch rules contain no deleted contexts.
56. PyPI jobs are not branch-protection requirements.
57. Obsolete publication secrets are absent.
58. Obsolete release environments are removed.
59. The intentional `pypi` environment is configured and documented.
60. Any inaccessible repository setting is reported precisely.

### Scope control

61. No automatic release cadence is introduced.
62. No push/PR wheel matrix is introduced.
63. No unsupported platform claim is added.
64. ABI3 is not claimed without explicit configuration.
65. No evidence schema, release orchestrator, or qualification framework is added.
66. Documentation matches executable behavior exactly.

## 20. Required handoff report

The implementing agent's final handoff must include:

- baseline SHA `d04fb77319e88831bd8162992daa7f7608e7d235`;
- final implementation SHA;
- exact validator dependency map;
- controlled failure results for missing, wrong, wildcard, path-only, and registry-only dependencies;
- successful clean `./scripts/check.sh package` summary;
- final routine CI run URL, job name, conclusion, and duration;
- build-only PyPI workflow run URL;
- wheel list grouped by OS, architecture, and Python version;
- sdist filename and isolated-build result;
- assembled artifact count and `twine check` result;
- action SHAs used in `pypi.yml`;
- PyPI Trusted Publisher workflow/environment fields;
- branch protection/ruleset findings;
- Actions secret names removed or retained-purpose summary, without values;
- environment findings and reviewer/tag restriction summary;
- explicit statement that crates.io remains manual;
- explicit statement that ordinary CI remains one Ubuntu job;
- any blocked operational setting with the exact permission failure and maintainer action required.

## 21. References for implementation

Use current official documentation during implementation and pin reviewed action revisions:

- maturin project and distribution guidance: `https://github.com/PyO3/maturin`
- maturin GitHub Action: `https://github.com/PyO3/maturin-action`
- PyPI Trusted Publishing: `https://docs.pypi.org/trusted-publishers/`
- configuring a GitHub Actions Trusted Publisher: `https://docs.pypi.org/trusted-publishers/adding-a-publisher/`
- publishing with the official PyPA action: `https://docs.pypi.org/trusted-publishers/using-a-publisher/`
- PyPI Trusted Publishing security model: `https://docs.pypi.org/trusted-publishers/security-model/`
- official PyPI publish action: `https://github.com/pypa/gh-action-pypi-publish`

## 22. Final decision rule

Close this work only when the internal dependency validator enforces the exact crate topology, canonical package validation requires a clean worktree, ordinary CI remains one lightweight job, the manual PyPI workflow produces and tests the complete 20-wheel plus sdist release set, PyPI upload is protected by tag validation, environment approval, and OIDC, crates.io remains manual, and all repository settings are directly verified or precisely reported as inaccessible.
