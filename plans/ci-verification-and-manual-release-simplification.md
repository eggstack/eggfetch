# CI, Verification, and Manual Release Simplification

Status: implementation handoff plan

Audited baseline commit: `b4415c55fb355990619b4161b2d20c6147ec03f3`

Audit date: 2026-07-28

Target repository: `eggstack/eggfetch`

Primary implementation surfaces:

- `.github/workflows/ci.yml`
- `.github/workflows/security.yml`
- `.github/workflows/benchmarks.yml`
- `.github/workflows/ffi.yml`
- `.github/workflows/qualification.yml`
- `.github/workflows/release.yml`
- `scripts/`
- `scripts/tests/`
- `README.md`
- `CONTRIBUTING.md`
- `AGENTS.md`
- `.skills/release-process.md`
- `docs/architecture/build-ci.md`
- `docs/releases/process.md`
- `docs/releases/rc-checklist.md`
- other documentation that describes CI gates, candidate identity, qualification evidence, automated publication, or release workflow behavior

## 1. Purpose

The repository's CI, verification, qualification, and release apparatus has become substantially more complex than the project it is intended to protect. Routine changes currently trigger broad operating-system and Python-version matrices, duplicate builds, package construction, wheel smoke tests, documentation execution, resource checks, security scanners, FFI-specific duplication, and a synthetic aggregation gate. Separate qualification and release workflows add candidate-SHA verification, candidate identity, artifact normalization, evidence schemas, downstream portfolio orchestration, release manifests, SBOM generation, registry publication, provenance attestations, post-publication verification, and workflow self-validation.

This infrastructure now consumes significant implementation effort and creates repeated false blockers. Recent work has repeatedly modified timeout limits, virtual-environment behavior, pytest invocation, artifact paths, matrix keys, gate evaluation, cancellation handling, and workflow validators instead of improving eggfetch behavior. The verification system is producing its own defects and materially reducing iteration speed.

This plan performs a deletion-oriented simplification. The target is a local-first development model with one small automatic CI job, behavior-focused tests, explicitly optional extended validation, and entirely manual releases. GitHub Actions must not publish packages, create releases, determine release cadence, or act as a release-qualification authority.

The central design rule is:

> Keep checks that fail because eggfetch is wrong. Remove machinery that fails only because the verification apparatus is arranged differently.

This is a planning-only change. It does not itself alter workflows, product code, release state, compatibility claims, branch protection, or published artifacts.

## 2. Audited current state

### 2.1 Routine CI is a large fan-out graph

At the audited baseline, `.github/workflows/ci.yml` contains separate jobs for:

- Rust formatting;
- Rust clippy plus a lint-suppression policy script;
- Rust 1.80 MSRV compilation;
- Rust build, feature checks, and tests on Ubuntu, macOS, and Windows;
- Rust documentation and doctests;
- documentation syntax, links, CLI help, and API-surface checks;
- Python documentation runtime examples;
- resource-regression monitoring;
- Python tests across Ubuntu, macOS, and Windows for Python 3.10, 3.11, 3.12, and 3.13;
- wheel construction and clean-environment smoke testing across the same 12 OS/Python combinations;
- HTTPX compatibility testing across four Python versions;
- a final `Required CI Gate` job that converts upstream job conclusions into JSON and reevaluates them through a custom script.

The workflow therefore starts approximately 39 runner instances for an ordinary push or pull request before considering the additional workflows below. Several jobs build the same Rust and Python components independently.

### 2.2 Adjunct workflows duplicate routine validation

`.github/workflows/security.yml` runs on every push and pull request and separately executes:

- cargo-deny advisories;
- cargo-deny licenses;
- cargo-deny bans;
- cargo-deny sources;
- cargo-audit installation and execution;
- cargo-geiger installation and informational execution.

`.github/workflows/benchmarks.yml` also runs on every push and pull request. It attempts artifact-backed Criterion baselines, suppresses benchmark command failures with `|| true`, parses textual regression output, and uploads result and baseline artifacts.

`.github/workflows/ffi.yml` runs a three-OS matrix plus a separate Ubuntu integration job when core or FFI paths change. It repeats format, clippy, feature checks, and FFI tests that are already reachable through workspace validation.

These workflows add cost and failure surface without providing a proportional independent correctness signal for routine iteration.

### 2.3 Qualification has become an independent software system

`.github/workflows/qualification.yml` is manually dispatchable and scheduled weekly. It includes or coordinates:

- candidate SHA validation;
- lookup of a green `Required CI Gate` for the same SHA;
- ordinary and controlled-replacement wheel builds;
- multi-platform wheel and sdist builds;
- candidate artifact normalization;
- artifact manifests;
- candidate identity generation and digest binding;
- bundle indexes;
- package-content validation;
- wheel smoke matrices;
- HTTPX compatibility suites;
- API manifest generation and comparison;
- manifest-generated downstream matrices;
- isolated downstream substitution environments;
- downstream result aggregation;
- shim substitution;
- timeout, proxy, TLS, shutdown, resource, and soak jobs;
- workflow structure validation;
- evidence generation from retained artifacts;
- independent evidence validation;
- a fail-closed qualification gate;
- status-document generation.

This is not required to establish ordinary regression confidence. The same behavior can remain covered by direct tests without candidate identity, artifact choreography, evidence envelopes, or workflow meta-validation.

### 2.4 Release automation duplicates CI and owns publication

`.github/workflows/release.yml` currently:

- validates versions, tags, changelog state, and candidate SHAs;
- repeats Rust and Python CI matrices;
- builds wheels, sdists, and CLI binaries across targets;
- builds and verifies archives and checksums;
- generates release manifests and SBOMs;
- runs package dry runs and package-content scans;
- publishes five crates to crates.io with hard-coded index propagation waits;
- publishes to TestPyPI and PyPI;
- creates a GitHub Release;
- attaches assets;
- creates provenance attestations;
- waits for registry propagation;
- installs published packages;
- generates release summaries;
- verifies that dry-run execution produced no side effects.

This workflow makes GitHub Actions part of release cadence, release authorization, credential custody, and publication. That is explicitly contrary to the target operating model.

### 2.5 Documentation contains contradictory policy

The repository currently contains both of these claims:

- CI is informational and is not a merge gate.
- `Required CI Gate` is a mandatory merge prerequisite.

Release documentation further treats immutable candidate SHAs, qualification evidence, full cross-platform matrices, artifact manifests, SBOMs, attestations, workflow dry runs, and automated publication as mandatory release procedure.

The implementation must remove these contradictions and establish one normative policy.

## 3. Target operating model

The final repository must use the following model.

### 3.1 Routine development

A contributor or implementation agent runs one local command that performs the same checks as automatic CI:

```sh
./scripts/check.sh
```

The command must be deterministic, behavior-focused, and small enough to run during normal iteration. It must stop on the first failure and must not generate retained evidence or publication artifacts.

### 3.2 Automatic GitHub CI

Only one workflow runs automatically on pushes and pull requests to `main`:

```text
.github/workflows/ci.yml
  -> one Ubuntu job
  -> calls ./scripts/check.sh
```

The workflow is a regression safety net, not a release authority. It has no matrix, no aggregation gate, no artifact exchange, no evidence generation, and no publishing permissions.

### 3.3 Extended validation

Slower or less frequently useful checks remain available through an explicit local command:

```sh
./scripts/check.sh extended
```

Extended validation may include full HTTPX compatibility, feature combinations, docs runtime, package smoke checks, downstream compatibility, lifecycle tests, resource monitoring, or soak tests. These checks do not run on every push or pull request.

Extended validation is advisory unless a maintainer explicitly decides to use it for a particular change. Its output is ordinary command output, not a versioned evidence contract.

### 3.4 Packaging validation

Packaging checks are local and dry-run only:

```sh
./scripts/check.sh package
```

This mode may run `cargo package`, `cargo publish --dry-run`, wheel or sdist builds, and local install/import smoke checks. It must never publish.

### 3.5 Security maintenance

Dependency policy and advisory scans run either:

- manually; or
- in one small weekly/manual GitHub Actions job.

They do not run for every source change. cargo-deny is sufficient as the canonical automated dependency-policy tool unless a concrete, documented gap requires an additional scanner. cargo-geiger must not be retained as unattended informational automation.

### 3.6 Release

Release timing and publication are maintainer decisions performed from a trusted local environment. GitHub Actions does not:

- hold crates.io, PyPI, npm, or other publication credentials;
- publish any crate or package;
- create or move tags;
- create GitHub Releases;
- build mandatory release artifacts;
- attest artifacts;
- authorize a candidate SHA;
- determine whether a release may occur;
- automatically release in response to a tag.

crates.io is the primary required release channel. Any PyPI or GitHub Release activity that remains desired is also performed manually and is not part of CI.

## 4. Scope

### 4.1 Included

This pass includes:

- replacing the current routine CI graph with one automatic Ubuntu job;
- creating one local validation entry point used by CI;
- removing the custom `Required CI Gate` and gate-evaluation infrastructure;
- removing the qualification workflow;
- removing the release workflow;
- removing automatic benchmark and FFI workflow duplication;
- reducing security automation to weekly/manual execution or removing it from GitHub Actions;
- separating routine, extended, and package validation;
- retaining direct behavioral tests while removing evidence and workflow meta-testing;
- deleting qualification-only scripts, schemas, fixtures, and tests after reference analysis;
- rewriting release procedure as a manual crates.io runbook;
- correcting all normative documentation and agent instructions;
- updating branch protection so it does not require deleted check names;
- validating the simplified workflows and local command;
- recording residual manual checks without converting them back into CI gates.

### 4.2 Excluded

This pass does not include:

- adding new HTTP features;
- expanding HTTPX API compatibility;
- changing networking semantics merely to make tests faster;
- rewriting the Rust engine;
- changing public APIs without a correctness reason discovered during implementation;
- adding a new CI framework, task runner, build service, or release bot;
- replacing GitHub Actions release automation with another hosted release automation service;
- adding Docker, Nix, Bazel, Earthly, reusable workflow frameworks, or custom action repositories;
- producing new evidence schemas or release attestations;
- redesigning the planning system;
- requiring old qualification plans to be executed before this simplification;
- deleting useful product-level tests solely because they are currently called by qualification.

## 5. Non-negotiable constraints

1. There must be exactly one push/PR CI workflow after this pass.
2. That workflow must contain exactly one required job.
3. The required job must run on Ubuntu only.
4. The required job must not use a strategy matrix.
5. The required job must call the same checked-in validation entry point used locally.
6. CI must not publish, tag, create a release, attest, or mutate repository contents.
7. No workflow may reference registry publication secrets.
8. Release cadence must be explicitly manual.
9. crates.io publication must be performed locally by a maintainer.
10. Packaging validation must not imply publication authorization.
11. A full HTTPX compatibility suite may remain available, but it must not require candidate identity or evidence artifacts.
12. Slow lifecycle, downstream, resource, and soak checks must not run on every push or pull request.
13. Cross-platform matrices must not be retained merely because platforms are theoretically supported.
14. FFI tests must remain runnable, but a separate three-OS FFI CI matrix is not required.
15. Benchmarks must remain runnable locally, but baseline artifact management is not part of routine CI.
16. Security automation must not duplicate advisory scans without a documented gap.
17. The implementation must prefer deleting code over introducing wrappers around obsolete code.
18. Completed historical plans must not be allowed to create permanent CI jobs, markers, evidence formats, or release gates.
19. No new workflow validator may be added to validate the simplified workflow.
20. No final JSON aggregation job may replace the deleted `Required CI Gate`.
21. Branch protection must not require a check name that no longer exists.
22. Documentation must not describe CI as both informational and mandatory.
23. Product tests must assert observable behavior, not source layout, workflow shape, or evidence serialization unless the serialized format is itself a supported product interface.
24. A test that exists solely to validate qualification/release orchestration must be deleted with that orchestration.
25. This pass must not become a new multi-stage release-qualification initiative.

## 6. Validation tiers

The implementation must codify three tiers and no more than three tiers.

### 6.1 Tier 1: routine validation

Command:

```sh
./scripts/check.sh
```

Required contents, in order:

1. Rust formatting check:

   ```sh
   cargo fmt --all -- --check
   ```

2. Existing lint-suppression policy, if retained after review:

   ```sh
   bash scripts/check_lint_suppressions.sh
   ```

3. Rust clippy:

   ```sh
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   ```

4. Rust workspace tests excluding the PyO3 crate from direct workspace execution when required by current build constraints:

   ```sh
   cargo test --workspace --exclude eggfetch-python --all-features
   ```

5. Build/install the Python extension once in the active virtual environment:

   ```sh
   maturin develop -m crates/eggfetch-python/Cargo.toml
   ```

6. Run the ordinary Python behavior suite while excluding explicitly slow qualification/lifecycle/soak collections:

   ```sh
   python -m pytest crates/eggfetch-python/tests/ -q \
     --ignore=crates/eggfetch-python/tests/compat \
     --ignore=crates/eggfetch-python/tests/soak_test.py
   ```

   The implementer must adjust exact ignore paths to files that actually exist at implementation time. The intent is to retain the ordinary Python API and integration tests while excluding the large HTTPX qualification portfolio and soak tests from routine CI.

7. Run a compact HTTPX compatibility smoke kernel selected from existing deterministic tests.

The smoke kernel must satisfy all of these constraints:

- no more than three existing test files;
- local deterministic servers or in-memory transports only;
- no public network access;
- no downstream package installation;
- no candidate wheel or identity construction;
- no soak or resource thresholds;
- no workflow/evidence tests;
- covers at minimum facade import, one synchronous request, one asynchronous request, response decoding, error mapping, and client shutdown;
- measured runtime under two minutes after the extension is built.

Do not introduce a large new marker taxonomy merely to define this kernel. Prefer an explicit list of stable existing test files or a small dedicated smoke file containing direct behavioral tests.

### 6.2 Tier 2: extended validation

Command:

```sh
./scripts/check.sh extended
```

This mode runs Tier 1 first and may then run the following direct checks where they remain useful:

- complete HTTPX compatibility suite;
- API manifest/profile comparison, if the manifest remains a maintained compatibility tool rather than release evidence;
- deterministic downstream compatibility suites;
- timeout classification;
- proxy and TLS behavior;
- shutdown lifecycle behavior;
- docs syntax and runtime examples;
- MSRV compilation;
- selected feature combinations;
- resource monitoring;
- soak tests;
- FFI library build and tests;
- Node binding checks if the Node crate remains maintained.

Extended mode must not:

- create candidate identities;
- build qualification bundles;
- normalize test output into release schemas;
- aggregate evidence;
- require a GitHub run ID or candidate SHA;
- query GitHub check runs;
- upload artifacts;
- infer release readiness;
- fail because a release-only package or external downstream dependency is unavailable unless that check was explicitly selected.

Where extended validation is too slow or environment-specific to run as one command, it may print clearly named opt-in subcommands. Do not recreate the old orchestration graph inside `check.sh`.

### 6.3 Tier 3: package validation

Command:

```sh
./scripts/check.sh package
```

Package mode runs Tier 1 and then performs local, side-effect-free packaging checks:

- verify coordinated crate and Python package versions;
- run `cargo package` or `cargo publish --dry-run` in dependency order for publishable crates;
- build the Python wheel or sdist if the Python package remains a release target;
- install a locally built wheel into a clean temporary virtual environment and perform a compact import/request smoke test;
- run `twine check` only if PyPI publication remains a maintained manual channel;
- list package contents when diagnosing packaging changes.

Package mode must not:

- accept a registry token;
- run `cargo publish` without `--dry-run`;
- upload to TestPyPI or PyPI;
- create a Git tag;
- create a GitHub Release;
- sleep for registry propagation;
- generate a release authorization result;
- claim that a successful dry run guarantees publication success.

## 7. Required implementation phases

## Phase 0: Freeze and inventory the current apparatus

### Deliverables

1. Record the audited baseline SHA in the implementation PR or commit message.
2. Enumerate every file under `.github/workflows/`.
3. For each workflow, record:
   - triggers;
   - job count;
   - matrix expansion count;
   - artifact upload/download use;
   - secrets and permissions;
   - publication or repository mutation capability;
   - overlap with another workflow.
4. Enumerate scripts and tests referenced only by qualification, release, evidence, gate, or artifact orchestration.
5. Record current branch-protection required check names before deleting workflows.
6. Measure the current routine CI fan-out from YAML, not from an incomplete run.
7. Identify the ordinary Python tests, compatibility tests, slow lifecycle tests, soak tests, downstream tests, and packaging tests by path.
8. Identify documentation that refers to:
   - `Required CI Gate`;
   - qualification;
   - candidate SHA;
   - candidate identity;
   - evidence manifests;
   - release workflow dispatch;
   - automated crates.io/PyPI publication;
   - automated GitHub Releases;
   - SBOM/provenance as mandatory gates.

### Acceptance criteria

- The inventory covers every checked-in workflow.
- Every planned deletion has at least one reference-search result explaining why it is workflow-only or release-only.
- Product-level scripts are not deleted merely because a workflow calls them.
- Current required branch-protection checks are known before workflow removal.
- The implementation does not begin by adding a replacement framework.

## Phase 1: Establish one normative verification and release policy

### Deliverables

Create `docs/verification-policy.md` as the normative policy. It must state, in direct language:

- CI is a fast regression safety net.
- CI is not a release authority.
- CI does not determine release cadence.
- CI does not publish packages or create releases.
- ordinary CI uses one Ubuntu job;
- local validation is canonical;
- extended checks are opt-in;
- packaging checks are local dry runs;
- crates.io publication is manual;
- historical qualification plans are non-normative;
- verification infrastructure must remain materially simpler than the behavior it verifies.

Include an explicit budget:

- one automatic workflow;
- one required runner job per push/PR;
- no matrix in routine CI;
- no artifact exchange in routine CI;
- no evidence schemas;
- no workflow meta-validation;
- warm-cache target under 10 minutes;
- cold-cache target under 20 minutes;
- any addition exceeding this budget requires a concrete regression history and explicit maintainer approval.

Add a short rule for future checks:

A new automatic check is permitted only when all of the following are true:

1. it catches a plausible product regression;
2. the regression cannot be covered by an existing test in the routine job;
3. the check is deterministic;
4. the check adds less than two minutes of expected runtime or replaces an equivalent-cost check;
5. it does not require artifact choreography or external services;
6. it does not duplicate another job;
7. its ongoing maintenance cost is documented.

### Acceptance criteria

- `docs/verification-policy.md` is the sole normative statement of CI/release policy.
- The policy explicitly says releases are manual.
- The policy explicitly says crates.io publication occurs outside GitHub Actions.
- The policy contains quantitative complexity limits.
- The policy prohibits candidate identity/evidence systems from returning as ordinary CI requirements.

## Phase 2: Add the local validation entry point

### Deliverables

Create `scripts/check.sh` with:

```bash
#!/usr/bin/env bash
set -euo pipefail
```

Supported invocations must be exactly:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Implementation requirements:

- default mode is routine Tier 1 validation;
- unknown arguments print usage and exit nonzero;
- commands are grouped in small named shell functions;
- no JSON result schemas;
- no timestamps, candidate identities, or GitHub-specific environment requirements;
- no implicit global package installation;
- check for required tools and provide direct setup guidance;
- use the active Python environment;
- use `python -m pytest` rather than relying on a potentially unrelated `pytest` executable;
- use temporary directories for package smoke environments and clean them with `trap`;
- avoid platform-specific shell complexity because routine CI is Ubuntu and primary local development is Unix-like;
- do not add a task-runner dependency.

The script must not silently skip a requested check. Environment-specific checks in `extended` should either run or clearly report that the prerequisite is absent and return an appropriate status. Optional downstream integrations may be individually opt-in rather than making the entire extended mode unusable.

### Acceptance criteria

- running `./scripts/check.sh` locally executes the same command sequence CI executes;
- the script exits on the first failing required command;
- no mode publishes or mutates the repository;
- no mode requires GitHub Actions metadata;
- default mode does not run soak, resource, downstream, cross-platform, wheel-matrix, workflow-validator, or release-evidence checks;
- `shellcheck` issues are corrected if shellcheck is already available, but shellcheck is not added as another mandatory CI dependency solely for this script;
- help/usage text accurately describes all three modes.

## Phase 3: Replace routine CI with one job

### Deliverables

Rewrite `.github/workflows/ci.yml` to the following conceptual structure:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

concurrency:
  group: ci-${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

permissions:
  contents: read

jobs:
  ci:
    name: ci
    runs-on: ubuntu-latest
    timeout-minutes: 20
    steps:
      - checkout
      - install stable Rust
      - set up one supported Python version
      - cache Cargo data if the cache remains simple and reliable
      - install local test/build dependencies
      - run ./scripts/check.sh
```

Specific requirements:

- use Python 3.12 unless implementation-time support policy names a different single canonical version;
- install only the dependencies required by routine validation;
- one Cargo cache step is permitted;
- do not cache virtual environments or wheels;
- do not upload test reports;
- do not build release wheels;
- do not run `cargo publish --dry-run`;
- do not run resource monitoring;
- do not run benchmark baselines;
- do not run a matrix summary or required gate evaluator;
- do not use `if: always()` to construct a second gate;
- do not set permissions beyond `contents: read`;
- retain `RUSTFLAGS=-D warnings` only if it remains necessary in addition to explicit clippy flags;
- retain `PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1` only if required by the selected Python/Rust environment.

Delete the following jobs rather than renaming or wrapping them:

- `rust-format`;
- `rust-lint`;
- `rust-msrv`;
- matrix `rust-test`;
- `rust-doc`;
- `docs-syntax`;
- `docs-runtime`;
- `resource-monitor`;
- matrix `python`;
- matrix `wheel-smoke`;
- matrix `compat-httpx`;
- `required-gate` or `matrix-summary` variants.

Their high-value direct checks either move into `scripts/check.sh` routine mode or remain in extended mode. Their orchestration and fan-out are deleted.

### Acceptance criteria

- a push to `main` starts one CI runner job;
- a pull request to `main` starts one CI runner job;
- there is no `strategy.matrix` in `ci.yml`;
- there is no `needs:` graph in `ci.yml`;
- there is no artifact upload or download action in `ci.yml`;
- there is no custom final gate job;
- superseded branch runs are cancelled;
- permissions are read-only;
- the job calls `./scripts/check.sh` rather than duplicating its commands in YAML;
- the workflow has a bounded timeout;
- the workflow can be understood without consulting a schema validator.

## Phase 4: Remove per-change adjunct workflows

### 4.1 FFI workflow

Delete `.github/workflows/ffi.yml`.

Retain FFI correctness through:

- workspace clippy and tests in routine validation where buildable;
- explicit FFI tests/builds in `extended` mode;
- local target-specific validation when making platform-specific FFI changes.

Do not replace it with another path-filtered matrix.

### 4.2 Benchmark workflow

Delete `.github/workflows/benchmarks.yml`.

Retain benchmarks as local developer tools. Document direct `cargo bench` commands in an appropriate benchmark document or `scripts/check.sh extended` output.

Do not retain GitHub artifact baselines. Hosted shared runners are not a stable enough environment to justify pull-request performance gating for this project.

### 4.3 Security workflow

Choose one of these two implementations, preferring the simpler option:

Option A, preferred when maintainers are comfortable running dependency policy locally:

- delete `.github/workflows/security.yml`;
- document `cargo deny check` in extended/local maintenance instructions.

Option B, acceptable:

- retain `.github/workflows/security.yml` with `workflow_dispatch` and one weekly schedule;
- one Ubuntu job;
- one checkout;
- one Rust setup;
- one cargo-deny invocation covering the configured policy;
- no cargo-audit duplication;
- no cargo-geiger;
- no artifacts;
- read-only permissions.

Security must not run on push or pull request.

### Acceptance criteria

- FFI changes no longer create a separate matrix;
- benchmark jobs no longer run on push or pull request;
- benchmark artifact baselines are removed;
- security scans no longer run for every source change;
- cargo-geiger automation is removed;
- cargo-audit duplication is removed unless a documented cargo-deny coverage gap is proven;
- no replacement workflow increases routine runner fan-out above one.

## Phase 5: Delete qualification and release workflows

### 5.1 Qualification workflow

Delete `.github/workflows/qualification.yml` in full.

Do not retain a smaller qualification workflow. Direct extended tests are sufficient. There must be no scheduled or manually dispatched candidate qualification graph.

### 5.2 Release workflow

Delete `.github/workflows/release.yml` in full.

Do not retain a dry-run-only workflow. Package dry runs belong in `./scripts/check.sh package` and are executed locally.

### 5.3 Workflow credential and permission audit

After deletion, search all remaining workflows for:

```text
CARGO_REGISTRY_TOKEN
PYPI_TOKEN
TESTPYPI_TOKEN
NPM_TOKEN
id-token: write
attestations: write
contents: write
cargo publish
maturin publish
twine upload
gh release
softprops/action-gh-release
pypa/gh-action-pypi-publish
actions/attest-build-provenance
```

Every match must be absent unless it is clearly non-executable documentation outside workflows.

### Acceptance criteria

- no qualification workflow exists;
- no release workflow exists;
- tag pushes do not trigger publication;
- workflow dispatch cannot publish;
- no workflow contains registry credentials;
- no workflow has write or OIDC publication permissions;
- no workflow creates GitHub Releases;
- no workflow generates mandatory release artifacts, SBOMs, provenance, candidate identities, or evidence;
- release cadence cannot be initiated accidentally by a Git tag.

## Phase 6: Remove gate, qualification, evidence, and workflow meta-testing code

This phase requires careful reference analysis. The implementer must delete orchestration-only code while retaining direct product-level tests and reusable compatibility tools.

### 6.1 Delete custom CI-gate infrastructure

Delete, where present:

- `scripts/evaluate_ci_gate.py`;
- its policy JSON file;
- tests for the evaluator;
- generated gate-result fixtures;
- documentation describing `Required CI Gate` evaluation.

No equivalent evaluator is required because GitHub already reports the one job's result.

### 6.2 Delete qualification workflow validators

Delete, where present:

- `scripts/validate_qualification_workflow.py`;
- `scripts/tests/test_qualification_workflow.py`;
- fixtures for malformed workflow cases;
- tests for matrix-key naming, output dependency graphs, obsolete workflow CLI arguments, failure-suppression tokens, job naming, artifact paths, or evidence wiring.

Do not replace these tests with tests for the simplified YAML shape. The policy and code review are sufficient.

### 6.3 Delete candidate identity and bundle machinery

Delete qualification-only implementations and tests for:

- candidate identity generation and validation;
- artifact-manifest-to-candidate identity digest binding;
- bundle indexes;
- candidate bundle validation;
- exact-SHA workflow metadata propagation;
- GitHub run ID and attempt binding;
- producer-job identity;
- qualification result envelopes.

Likely files include, where present and not used by a maintained product interface:

- `scripts/candidate_identity.py`;
- `scripts/validate_bundle.py`;
- qualification-specific portions of `scripts/generate_artifact_manifest.py`;
- associated schema fixtures and tests.

If `generate_artifact_manifest.py` has a simple independent use for local package inspection, reduce it to that use rather than preserving candidate identity and workflow metadata.

### 6.4 Delete evidence generation and validation

Delete qualification/release-only implementations and tests for:

- normalized pytest release result schemas;
- evidence aggregation;
- candidate-identity propagation through result artifacts;
- compatibility evidence generation;
- independent evidence validation;
- mechanically generated qualification status documents;
- retained artifact completeness;
- `overall_pass` release calculations.

Likely files include, where present and not used by a supported product interface:

- `scripts/normalize_pytest_result.py`;
- `scripts/generate_compatibility_evidence.py`;
- `scripts/validate_compatibility_evidence.py`;
- downstream aggregation scripts used only to assemble qualification evidence;
- associated scripts tests, fixtures, and generated status/evidence documents.

### 6.5 Retain direct compatibility assets

Do not delete solely because qualification used them:

- `compat/httpx/0.28.1/` profile data that directly documents tested compatibility;
- `crates/eggfetch-python/tests/compat/` behavior tests;
- deterministic proxy, TLS, timeout, shutdown, and streaming fixtures;
- behavioral downstream fixtures that can be run directly and remain maintained;
- API manifest comparison if it is a direct compatibility-development tool;
- `scripts/check_compatibility_claims.py` if it directly prevents unsupported public claims without evidence orchestration;
- wheel smoke logic if simplified and used by local package validation.

The test is whether a file helps a developer determine product behavior without GitHub-specific candidate/evidence context.

### 6.6 Remove stale generated evidence

Review checked-in documents under release/evidence/status locations. Delete or archive generated qualification evidence that is presented as current release authority. Historical records may remain only when clearly marked historical and non-normative.

### Acceptance criteria

- no code queries GitHub check runs to authorize qualification;
- no candidate identity schema remains;
- no candidate bundle schema remains;
- no workflow validator remains;
- no test exists solely to assert GitHub Actions topology;
- no test exists solely to assert evidence completeness;
- no release-blocking result envelope remains;
- direct behavior tests remain runnable without a candidate SHA;
- compatibility tests do not require a GitHub run ID or artifact digest;
- removing the infrastructure produces a net deletion of scripts and tests;
- no deleted script remains referenced in docs, workflows, skills, or agent instructions.

## Phase 7: Simplify the test apparatus without reducing correctness

The purpose is not to reduce test count indiscriminately. It is to remove duplication, false precision, and orchestration coupling.

### 7.1 Classify tests by behavior and cost

Classify current tests into:

1. routine deterministic behavior;
2. extended compatibility behavior;
3. environment- or timing-sensitive lifecycle behavior;
4. soak/resource/performance behavior;
5. packaging behavior;
6. workflow/evidence meta-behavior.

Map categories 1 and a compact subset of 2 into routine CI. Keep categories 2 through 5 directly runnable locally. Delete category 6.

### 7.2 Remove duplicate execution, not unique assertions

A test should not be removed merely because it is slow. First determine whether the same assertion is already covered at a lower, deterministic layer.

Examples:

- retain one direct timeout classification test; move prolonged real-stall variants to extended mode;
- retain direct proxy CONNECT behavior; remove repeated candidate-wheel executions of the same proxy test;
- retain shutdown resource-release tests; move subprocess/stalled-request stress variants to extended mode;
- retain basic wheel installation smoke; remove 12-way per-commit wheel construction;
- retain FFI unit and integration assertions; remove repeated OS matrices unless a concrete platform-specific defect requires targeted manual validation;
- retain one API compatibility comparison; remove duplicate facade/controlled-replacement evidence envelopes if they test the same public surface.

### 7.3 Eliminate false-green patterns

Simplification must not preserve weak tests merely because they are cheap. Correct or remove tests that:

- catch arbitrary exceptions and pass;
- use `|| true` around a required behavioral command;
- assert only that a package imports when a behavioral contract is intended;
- use large timeout increases to hide nondeterminism;
- accept cancellation or infrastructure failure as product success;
- depend on textual pytest output parsing when pytest exit status is sufficient;
- verify source-code layout rather than runtime behavior;
- assert exact internal implementation details without a public contract.

### 7.4 Keep external network access out of routine CI

Routine tests must use local servers, deterministic fixtures, or in-memory transports. Downstream packages and public registries are extended/manual concerns.

### 7.5 Do not create a new marker bureaucracy

Use existing pytest markers only where they already communicate stable test semantics. A small number of simple markers such as `slow`, `soak`, or `external` is acceptable if already present or clearly useful. Do not create plan-numbered, evidence, stage, candidate, or release-gate markers.

### Acceptance criteria

- routine CI covers Rust behavior, Python behavior, and a compact HTTPX compatibility kernel;
- full compatibility remains directly runnable;
- timeout/proxy/TLS/shutdown behavior is not deleted wholesale;
- soak/resource/performance tests are not automatic push/PR gates;
- no required routine test accesses the public internet;
- no required routine test installs arbitrary downstream packages;
- no routine test relies on parsing human-oriented pytest summary text;
- tests fail on unexpected exceptions rather than accepting them as proof;
- the simplified suite has fewer execution paths but retains unique behavior assertions;
- test documentation clearly identifies routine versus extended checks.

## Phase 8: Replace release automation with a manual crates.io runbook

Rewrite `docs/releases/process.md` as a concise manual procedure.

### 8.1 Required pre-publication steps

1. Select the version deliberately.
2. Update every coordinated publishable crate version.
3. Update Python package metadata if maintained.
4. Move changelog entries into the release version section.
5. Ensure the worktree is clean.
6. Run:

   ```sh
   ./scripts/check.sh
   ./scripts/check.sh package
   ```

7. Review package contents for changed packaging surfaces.
8. Confirm credentials exist only in the maintainer's local Cargo configuration or temporary environment.

### 8.2 crates.io publication order

Publish manually in dependency order:

```text
1. eggfetch-core
2. eggfetch-cli
3. eggfetch-ffi
4. eggfetch-python
5. eggfetch-node
```

Before publishing a dependent crate, verify the preceding crate/version is visible to crates.io resolution. Do not encode fixed sleeps as policy. The maintainer should inspect actual registry availability.

Use explicit commands, for example:

```sh
cargo publish -p eggfetch-core
cargo publish -p eggfetch-cli
cargo publish -p eggfetch-ffi
cargo publish -p eggfetch-python
cargo publish -p eggfetch-node
```

The runbook must remind the maintainer that crates.io versions are immutable. If a published version is incorrect or publication is partial, correct the defect, bump the version where required, and publish a new version. Do not attempt to overwrite an existing version.

### 8.3 Tagging and GitHub Releases

Tagging is manual and separate from publication. The runbook may recommend creating and pushing a signed version tag after successful required publication.

A GitHub Release is optional and manual. It must not be described as an automated or required CI output.

### 8.4 PyPI or other channels

If PyPI remains supported, document its local manual build and publish commands separately. It must not be coupled to crates.io in a single supposedly atomic workflow.

A successful publication to one registry must not be deleted because another channel failed. Correct and issue a new version according to each registry's immutability rules.

### 8.5 Remove obsolete release requirements

The new runbook must not require:

- a GitHub Actions dry run;
- a candidate SHA input;
- an immutable validation tag;
- a green qualification workflow;
- an evidence manifest;
- candidate identity;
- a release manifest;
- a CI matrix summary;
- an SBOM as a publication gate;
- provenance attestations;
- automated post-publication sleeps;
- automated install verification;
- release environment approval in GitHub.

These may be performed manually when useful, but they are not part of the required release contract.

### Acceptance criteria

- release documentation begins by stating that release cadence is manual;
- crates.io publication commands are local commands;
- no release step tells the maintainer to run a GitHub Actions workflow;
- no GitHub secret is required;
- publication order is explicit;
- immutability and partial-publication recovery are explicit;
- optional PyPI/GitHub activity is not coupled to required crates.io publication;
- package dry-run validation is clearly distinct from publishing;
- no document claims CI authorizes release.

## Phase 9: Reconcile all documentation and agent instructions

Update at minimum:

- `README.md`;
- `CONTRIBUTING.md`;
- `AGENTS.md`;
- `.skills/release-process.md`;
- `docs/architecture/build-ci.md`;
- `docs/releases/process.md`;
- `docs/releases/rc-checklist.md`.

### 9.1 README

Replace detailed qualification/evidence claims with a concise description of direct HTTPX compatibility tests. Remove statements that present candidate identity, evidence validation, or fail-closed qualification as product capabilities.

The CI badge may remain if it points to the simplified workflow.

Installation text must not claim automatically produced GitHub Release binaries unless that distribution channel is actually maintained manually.

### 9.2 CONTRIBUTING

Document:

```sh
./scripts/check.sh
```

as the expected pre-commit validation command. Explain that CI repeats this command on Ubuntu and is intentionally small.

Remove large mandatory pre-release matrices from contributor guidance.

### 9.3 AGENTS

Remove:

- `Required CI Gate` requirements;
- qualification commands;
- candidate identity commands;
- evidence generation/validation commands;
- artifact normalization commands;
- instructions to preserve release-blocking result schemas.

Retain direct product-development commands and the three validation tiers.

Add an explicit agent constraint:

> Do not add CI jobs, matrices, evidence formats, release workflows, or publication automation without an explicit user request. Prefer direct tests in the existing local check path.

### 9.4 Release skill

Rewrite `.skills/release-process.md` as a manual local runbook or remove the skill if it only duplicates `docs/releases/process.md`.

It must not instruct agents to dispatch a release workflow.

### 9.5 Build/CI architecture

Replace the current job table with the one-job architecture and explain why extended checks remain local/manual.

### 9.6 RC checklist

Either delete `docs/releases/rc-checklist.md` or replace it with a compact optional maintainer checklist. It must not preserve the old evidence, full matrix, immutable candidate, dry-run workflow, or sign-off bureaucracy.

### 9.7 Historical plans

Add a clear statement to the plans index or relevant planning documentation that completed and superseded plan files are historical records, not active CI/release requirements. Do not edit every old plan to retrofit the new policy unless references make that necessary.

### Acceptance criteria

- repository-wide search finds no current normative claim that `Required CI Gate` is mandatory;
- repository-wide search finds no current instruction to dispatch `release.yml` or `qualification.yml`;
- no current documentation requires candidate identity or release evidence;
- `README`, `CONTRIBUTING`, `AGENTS`, CI docs, release docs, and the release skill agree;
- old plans are clearly non-normative;
- contributor instructions fit on a small number of commands;
- release documentation is substantially shorter than the workflow it replaces.

## Phase 10: Update repository settings

This phase may require GitHub repository administration outside a code commit.

### Deliverables

1. Inspect branch protection or rulesets for required check names.
2. Remove requirements for:
   - `Required CI Gate`;
   - old matrix job names;
   - security jobs;
   - FFI jobs;
   - qualification jobs.
3. If a required check is desired, require only the simplified `CI / ci` check.
4. Remove GitHub Actions release environments and registry secrets if they are no longer used:
   - crates.io token;
   - PyPI token;
   - TestPyPI token;
   - other publication credentials.
5. Verify Actions default permissions are read-only where practical.
6. Disable obsolete scheduled workflows if GitHub retains them after file deletion.

### Acceptance criteria

- merging is not blocked by deleted check names;
- only the simplified CI check is required, if any check is required;
- no registry publication credential remains available to workflows;
- no release environment is needed;
- repository settings match checked-in policy.

## Phase 11: Validation and closure

### 11.1 Static repository checks

Run repository-wide searches proving absence of obsolete machinery:

```sh
git grep -n "Required CI Gate" -- ':!plans/**'
git grep -n "candidate_sha" -- ':!plans/**'
git grep -n "candidate identity" -- ':!plans/**'
git grep -n "qualification.yml" -- ':!plans/**'
git grep -n "release.yml" -- ':!plans/**'
git grep -n "CARGO_REGISTRY_TOKEN" .github scripts docs README.md CONTRIBUTING.md AGENTS.md
git grep -n "PYPI_TOKEN" .github scripts docs README.md CONTRIBUTING.md AGENTS.md
git grep -n "cargo publish" .github/workflows
git grep -n "action-gh-release\|gh-action-pypi-publish\|attest-build-provenance" .github/workflows
```

Expected results outside historical plans are empty except for manual local release documentation where `cargo publish` is intentionally documented.

### 11.2 Workflow shape checks

Validate manually or with a generic YAML parser:

- only `ci.yml` triggers on push/PR;
- `ci.yml` has one job;
- `ci.yml` has no matrix;
- `ci.yml` has read-only permissions;
- security, if retained, is weekly/manual only;
- no workflow publishes or writes repository contents.

Do not create a project-specific workflow validator to perform these checks permanently.

### 11.3 Local commands

Run:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

If extended mode includes intentionally environment-specific optional checks, run the deterministic supported subset and document any intentionally manual SBC/platform validation.

### 11.4 CI execution

Push the implementation and verify:

- exactly one routine CI job starts;
- it executes `./scripts/check.sh`;
- it completes successfully;
- no other push/PR workflow starts;
- a superseding push cancels the prior in-progress run;
- no artifacts are uploaded;
- no qualification or release workflow is available.

### 11.5 Complexity comparison

Record before/after counts in the implementation summary:

- workflow files;
- workflows triggered per push/PR;
- expanded runner jobs per push/PR;
- matrix definitions;
- artifact upload/download steps;
- workflow-specific scripts/tests deleted;
- lines of workflow YAML deleted;
- publication-capable workflow steps before/after.

The expected result is approximately:

```text
Before:
- 6 major workflow files involved in CI/verification/release
- ~43+ runner jobs possible on an ordinary relevant change
- multiple matrices and aggregation gates
- publication-capable release workflow
- qualification/evidence subsystem

After:
- 1 push/PR workflow
- 1 push/PR runner job
- 0 routine matrices
- 0 routine artifacts
- 0 publication-capable workflows
- 0 qualification/evidence subsystem
```

### Acceptance criteria

- Tier 1 passes locally and in CI;
- package dry runs pass locally for publishable crates or any genuine package defect is reported separately rather than hidden by infrastructure work;
- one push creates one job;
- no release or qualification workflow runs;
- no publication secret is referenced;
- static searches show no active stale policy;
- branch protection is updated;
- before/after complexity is documented;
- the implementation is a net deletion by lines and files across workflows and verification-only code.

## 8. Explicit file disposition

The implementer must use this as the starting disposition, adjusting only for implementation-time reference findings.

### Delete

- `.github/workflows/qualification.yml`
- `.github/workflows/release.yml`
- `.github/workflows/benchmarks.yml`
- `.github/workflows/ffi.yml`
- custom CI-gate evaluator and policy files
- qualification workflow validator and tests
- candidate identity and bundle validation code used only for qualification
- release/compatibility evidence aggregation and validation code used only for qualification
- generated qualification status/evidence presented as current authority
- dry-run side-effect verification machinery
- tests for workflow topology, candidate identity propagation, artifact job wiring, or evidence completeness

### Replace

- `.github/workflows/ci.yml`
- `.github/workflows/security.yml` if Option B is selected
- `docs/releases/process.md`
- `docs/releases/rc-checklist.md`
- `.skills/release-process.md`
- `docs/architecture/build-ci.md`

### Add

- `scripts/check.sh`
- `docs/verification-policy.md`

### Update

- `README.md`
- `CONTRIBUTING.md`
- `AGENTS.md`
- plans index or planning guidance, if one exists
- any security/release/incident documentation that names deleted workflows or gates

### Retain unless direct review proves obsolete

- Rust unit and integration tests
- Python unit and integration tests
- direct HTTPX compatibility profile and behavior tests
- deterministic local test servers and transport fixtures
- direct timeout, proxy, TLS, streaming, and shutdown tests
- FFI tests
- benchmark source code
- cargo-deny configuration
- simple lint-suppression policy
- wheel smoke behavior useful for local package validation
- downstream behavioral fixtures useful for explicit compatibility development

## 9. Suggested implementation commit sequence

Use a small sequence that keeps review intelligible. Do not split this into a large roadmap.

### Commit 1: policy and local command

- add `docs/verification-policy.md`;
- add `scripts/check.sh`;
- establish routine/extended/package tiers;
- verify locally.

Suggested message:

```text
build: codify local-first verification policy
```

### Commit 2: collapse automatic CI

- replace `ci.yml`;
- delete FFI and benchmark workflows;
- reduce or delete security workflow;
- verify one-job workflow syntax.

Suggested message:

```text
ci: collapse push validation to one job
```

### Commit 3: remove qualification and release automation

- delete qualification and release workflows;
- delete gate/evidence/candidate/workflow-validator code and tests;
- retain direct behavior tests;
- run reference searches.

Suggested message:

```text
ci: remove qualification and release orchestration
```

### Commit 4: manual release and documentation reconciliation

- rewrite release process;
- update README, CONTRIBUTING, AGENTS, CI architecture, RC checklist, and release skill;
- mark old plans historical/non-normative.

Suggested message:

```text
docs: make release cadence and publication manual
```

### Commit 5: closure cleanup, only if needed

- remove stale references discovered by searches;
- correct test categorization;
- record before/after metrics;
- no new feature work.

Suggested message:

```text
chore: close verification simplification references
```

## 10. Global acceptance criteria

The pass is complete only when every criterion below is satisfied.

### CI topology

1. Exactly one workflow triggers on push to `main`.
2. Exactly one workflow triggers on pull requests to `main`.
3. The automatic workflow is `.github/workflows/ci.yml`.
4. `ci.yml` contains exactly one job.
5. The job runs on Ubuntu.
6. There is no routine matrix.
7. There is no final aggregation gate.
8. There is no cross-job artifact exchange.
9. Superseded runs are cancelled.
10. CI permissions are read-only.
11. CI calls the checked-in local validation command.
12. Expected cold runtime is bounded at 20 minutes.

### Verification behavior

13. Routine validation includes format, clippy, Rust tests, ordinary Python tests, and a compact HTTPX compatibility smoke kernel.
14. Routine validation does not include full downstream, soak, resource, benchmark, wheel matrix, or release checks.
15. Full compatibility remains directly runnable.
16. FFI tests remain directly runnable.
17. Benchmarks remain directly runnable.
18. Package dry runs remain directly runnable.
19. Direct proxy/TLS/timeout/shutdown correctness coverage remains.
20. Routine tests do not require public network access.
21. Routine tests do not require GitHub metadata.
22. Unexpected exceptions cannot be interpreted as passing behavior.
23. Workflow/evidence meta-tests are removed.
24. Verification-only code is a net deletion.

### Qualification and evidence removal

25. `.github/workflows/qualification.yml` is deleted.
26. No weekly candidate qualification remains.
27. No candidate SHA is used as verification authority.
28. No candidate identity schema remains active.
29. No candidate bundle index remains active.
30. No release evidence aggregation remains active.
31. No independent evidence validator remains active.
32. No generated status document determines compatibility/release state.
33. No custom workflow validator remains.
34. Direct compatibility tests run without qualification artifacts.

### Release isolation

35. `.github/workflows/release.yml` is deleted.
36. No workflow publishes to crates.io.
37. No workflow publishes to PyPI or TestPyPI.
38. No workflow creates GitHub Releases.
39. No workflow creates tags.
40. No workflow uses publication OIDC or attestation permissions.
41. No workflow references registry publication secrets.
42. Release cadence is documented as manual.
43. crates.io publication order is documented.
44. Local package dry runs are documented.
45. Registry immutability and partial-release recovery are documented.
46. Optional additional channels remain manual and decoupled.

### Documentation and governance

47. `docs/verification-policy.md` exists and is normative.
48. README accurately describes the simplified system.
49. CONTRIBUTING uses `./scripts/check.sh`.
50. AGENTS prohibits reintroducing CI/release complexity without explicit request.
51. CI architecture documentation shows one job.
52. Release documentation contains no workflow dispatch step.
53. RC checklist is deleted or reduced to a compact manual checklist.
54. Release skill is manual or removed.
55. No active document says `Required CI Gate` is mandatory.
56. No active document says CI is both informational and mandatory.
57. Historical plans are marked non-normative.
58. Branch protection does not require deleted checks.
59. Publication credentials are removed from Actions.
60. Before/after complexity metrics are recorded.

## 11. Rejection conditions

Reject the implementation as incomplete or contrary to this plan if any of the following occurs:

- the old workflow is moved into a reusable workflow rather than deleted;
- one large job is replaced by several nominally optional push/PR jobs;
- a matrix remains in routine CI;
- release publication remains possible from GitHub Actions;
- a dry-run release workflow remains as a prerequisite;
- candidate identity or evidence generation remains required for compatibility tests;
- a new workflow-shape validator is added;
- test results are normalized into a new schema merely to preserve evidence concepts;
- benchmarks continue to run on every push or pull request;
- FFI retains a separate three-OS automatic matrix;
- security scans continue on every push/PR without concrete justification;
- cargo-audit and cargo-deny advisories remain duplicated without a documented gap;
- the implementation deletes direct product behavior tests instead of removing duplicate execution;
- slow tests are hidden with longer timeouts instead of moved to extended validation or made deterministic;
- `continue-on-error` or `|| true` is used to manufacture a green required result;
- docs continue to require a deleted workflow;
- branch protection remains tied to deleted check names;
- the change adds more workflow or verification code than it deletes;
- implementation is divided into another sequence of qualification closure plans instead of completing the simplification.

## 12. Handoff checklist

The implementing agent must provide the following in its final handoff:

- baseline SHA;
- final implementation SHA;
- workflow files before and after;
- expanded push/PR runner count before and after;
- deleted workflow list;
- deleted gate/qualification/evidence script and test list;
- retained direct behavior test inventory;
- exact contents of routine validation;
- exact contents of extended validation;
- exact contents of package validation;
- local command results;
- simplified CI run link and job count;
- confirmation that no other push/PR workflow ran;
- branch-protection changes;
- publication-secret removal confirmation;
- repository-wide stale-reference search results;
- residual platform/SBC validation that remains intentionally manual;
- any genuine product defect discovered while removing infrastructure, clearly separated from infrastructure work;
- explicit statement that release cadence and crates.io publication are manual.

## 13. Final decision rule

This line of work is closed when eggfetch has one understandable automatic CI job, one shared local validation command, optional direct extended/package checks, no qualification/evidence subsystem, and no publication-capable GitHub workflow.

A green CI result must mean only that the current code passed the project's compact regression checks. It must not imply that a release has been qualified, authorized, scheduled, constructed, or published.

Release remains a deliberate maintainer action performed locally through crates.io.