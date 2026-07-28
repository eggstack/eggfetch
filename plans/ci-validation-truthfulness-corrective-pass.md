# CI Validation Truthfulness and Manual-Release Closure Corrective Pass

Status: implementation handoff plan

Audited baseline commit: `246215cc88c1f13c5c65f7c8bfe0c7ffdee6bd03`

Audit date: 2026-07-28

Target repository: `eggstack/eggfetch`

Parent plan: `plans/ci-verification-and-manual-release-simplification.md`

Primary implementation surfaces:

- `scripts/check.sh`
- `.github/workflows/ci.yml`
- `AGENTS.md`
- `CONTRIBUTING.md`
- `README.md`
- `.skills/python-bindings.md`
- `.skills/release-process.md`
- `.skills/rust-development.md`
- `docs/verification-policy.md`
- `docs/architecture/build-ci.md`
- `docs/releases/process.md`
- `docs/releases/rc-checklist.md`
- `compat/httpx/0.28.1/README.md`
- `scripts/compare_httpx_api_manifest.py`
- repository branch-protection, Actions-secret, and environment settings

## 1. Purpose

The large CI, qualification, evidence, and release simplification landed structurally as intended. The repository now has one automatic Ubuntu CI job, one local validation entry point, no automatic release workflow, no qualification workflow, no routine matrices, and no publication-capable GitHub Actions path.

The remaining defects are narrower but materially important. The new `scripts/check.sh` currently converts several real validation failures into warnings, then exits successfully and prints `All checks passed.` Package dry-run failures, package-content failures, lifecycle failures, soak failures, downstream failures, merge failures, benchmark failures, and some resource or MSRV failures can therefore be misrepresented as successful validation. The script also installs `maturin` implicitly through an unqualified `pip` command, which can mutate an unrelated or global Python environment. Documentation retains at least one command for a deleted script and contains duplicated raw commands that have drifted from the canonical local entry point. The API-manifest comparator exposes a `--resolved` option that is accepted but ignored.

This corrective pass closes those truthfulness and cleanup gaps without re-expanding CI. It must preserve the one-workflow, one-job architecture and manual release policy. The objective is not to add more checks. The objective is to make the retained checks report their actual outcomes, make environment prerequisites explicit, remove stale interfaces, and produce the small amount of operational evidence needed to close the simplification safely.

The central rule is:

> A selected check may pass, fail, or be explicitly skipped before execution because a documented optional prerequisite is unavailable. A check that starts and fails may never be relabeled as skipped or successful.

## 2. Audited current state

### 2.1 Structural simplification is complete

At the baseline commit:

- `.github/workflows/ci.yml` is the only checked-in workflow;
- it triggers on pushes and pull requests to `main`;
- it has one job named `ci`;
- it runs on `ubuntu-latest`;
- it has no matrix;
- it has no job graph;
- it has read-only contents permission;
- it creates a Python virtual environment;
- it invokes `./scripts/check.sh`;
- qualification, release, benchmark, FFI, and security workflows are deleted;
- candidate identity, qualification evidence, result normalization, workflow validation, and release orchestration code are deleted;
- `docs/verification-policy.md` correctly defines local-first validation and manual release;
- `docs/releases/process.md` correctly states that crates.io publication is manual and outside GitHub Actions.

This corrective pass must not reverse those decisions.

### 2.2 Package mode is fail-open

Current package dry-run behavior is equivalent to:

```sh
cargo publish -p "$crate" --dry-run 2>&1 || warn "Dry-run for $crate had issues"
```

A failed dry run therefore does not fail `./scripts/check.sh package`.

Current package-content behavior is equivalent to:

```sh
python scripts/validate_package_content.py "$wheel" 2>/dev/null \
    || warn "Package content check had issues"
```

A failed package-content check also does not fail package mode.

The script then unconditionally prints:

```text
All checks passed.
```

Package mode is used as a required pre-publication command in `docs/releases/process.md`. Its current exit semantics therefore provide false release confidence.

### 2.3 Extended mode masks executed test failures

The following checks currently use broad `|| warn` handling:

- resource monitor;
- timeout/proxy/TLS/shutdown lifecycle tests;
- soak tests;
- downstream behavioral fixtures;
- lossless merge tests;
- benchmarks.

For these checks, a missing command, missing path, collection error, test failure, assertion failure, crash, timeout, build failure, or actual regression is collapsed into a warning such as `skipped`. Stderr is also redirected for several commands, removing the diagnostic information needed to distinguish setup problems from product defects.

The result is a false-green extended run.

### 2.4 Compatibility dependency setup is fail-open

Extended compatibility currently performs:

```sh
pip install -r compat/httpx/0.28.1/requirements.txt 2>/dev/null || true
```

Dependency installation failures are hidden. The compatibility suite may then run with missing or inconsistent dependencies, or may skip tests for reasons that are not reported.

### 2.5 Local environment handling is unsafe and inconsistent

Routine mode checks for `cargo`, `rustc`, and `python3`, but later invokes:

- `python`;
- `pip`;
- `maturin`;
- `pytest` through `python -m pytest`.

If `maturin` is missing, the script runs `pip install maturin` without confirming:

- that a virtual environment is active;
- that `pip` belongs to the selected Python interpreter;
- that the user authorized environment mutation;
- that global installation is safe or permitted.

CI happens to activate `.venv`, but the local command is presented as canonical and does not enforce the same environment contract.

### 2.6 Documentation contains stale and divergent commands

`AGENTS.md` still references:

```sh
python scripts/validate_httpx_compat_profile.py compat/httpx/0.28.1
```

The referenced script was deleted during the simplification.

`AGENTS.md` also duplicates numerous direct compatibility, lifecycle, timeout, and release-oriented command sequences. Some include `--timeout` options, while the canonical script removed those options because the routine CI environment does not install `pytest-timeout`.

Duplicating the command graph in agent instructions recreates the drift the simplification was intended to prevent.

### 2.7 API comparator exposes an ignored option

`scripts/compare_httpx_api_manifest.py` accepts:

```text
--resolved <path>
```

but `main()` does not load or use the path. Documentation continues to show the option in some examples. A command-line option that is accepted but ignored is a correctness defect because it implies validation that does not occur.

The simplest correction is to remove the dead option and its documentation references unless there is a current, direct behavioral requirement for a separate resolved ledger. This pass must not recreate evidence or qualification semantics to preserve the option.

### 2.8 Operational closure is unverified

Code review cannot confirm whether repository settings were updated after workflow deletion. The following remain unverified:

- branch protection or rulesets no longer require `Required CI Gate` or deleted job names;
- only `CI / ci`, if any check, is required;
- crates.io, PyPI, TestPyPI, npm, or other publication secrets were removed from Actions;
- obsolete release environments were removed;
- the final one-job workflow has a retained successful run on current `main`.

These are closure tasks, not reasons to add more code.

## 3. Scope

### 3.1 Included

This pass includes:

- making routine, extended, and package modes truthful about pass, fail, and skip outcomes;
- making all required package checks fail closed;
- replacing broad `|| warn`, `|| true`, and hidden-stderr patterns;
- distinguishing prerequisite-based skips from executed-check failures;
- removing implicit Python package installation from `scripts/check.sh`;
- making the Python interpreter and virtual-environment contract explicit;
- ensuring CI installs all Tier 1 prerequisites before calling the script;
- making package builds use fresh temporary output rather than persistent stale wheel directories;
- ensuring package smoke and content validation operate on the exact wheel built by the current invocation;
- correcting stale commands and deleted-script references;
- reducing duplicated command documentation in favor of the canonical script;
- removing or correctly implementing the ignored `--resolved` comparator option, with deletion preferred;
- validating all three script modes;
- validating one current one-job CI run;
- verifying branch protection and publication-secret cleanup;
- documenting any setting that cannot be changed by the implementing agent.

### 3.2 Excluded

This pass does not include:

- adding CI jobs;
- adding operating-system or Python-version matrices;
- restoring qualification, evidence, release, benchmark, FFI, or security workflows;
- adding artifact uploads;
- adding a shell-script testing framework;
- adding a workflow validator;
- adding candidate identity, result schemas, or release authorization;
- changing HTTPX compatibility stage claims;
- expanding compatibility surface;
- changing networking behavior unrelated to a test defect exposed by this work;
- publishing crates or Python packages;
- creating a release;
- introducing a task runner or build orchestration framework;
- converting every optional local diagnostic into a release gate;
- adding automatic dependency installation to the canonical script;
- suppressing failures merely to keep extended mode green.

## 4. Non-negotiable invariants

1. `.github/workflows/ci.yml` remains the only push/PR workflow.
2. CI remains one Ubuntu job with no matrix.
3. CI continues to call `./scripts/check.sh`.
4. No workflow gains publication permissions or secrets.
5. Release cadence remains manual.
6. `scripts/check.sh` never runs a non-dry-run publish command.
7. A required check failure produces a nonzero script exit.
8. A check that begins execution and fails cannot be reported as skipped.
9. A skip is permitted only after a specific preflight proves that an explicitly optional prerequisite is unavailable.
10. A skip message must name the missing prerequisite.
11. Stderr from failed checks must remain visible.
12. Routine mode has no skips.
13. Package mode has no skips for required package checks.
14. Extended mode may have a narrowly defined MSRV skip when the pinned toolchain is not installed.
15. Missing repository files are failures, not optional-prerequisite skips.
16. Missing test dependencies are setup failures with actionable guidance, not automatic installations.
17. The script must not invoke bare `pip install`.
18. The script must not mutate a global Python environment.
19. Package validation must use artifacts built during the same invocation.
20. A successful final message may be printed only after all required selected checks pass.
21. Documentation must not reference deleted scripts.
22. Documentation must not advertise ignored CLI options.
23. This pass must result in little or no increase in CI YAML.
24. No permanent test harness may be added solely to test workflow or shell topology.
25. The implementation must remain a narrow corrective pass.

## 5. Target command semantics

The three public invocations remain exactly:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Unknown arguments continue to print usage and return nonzero.

### 5.1 Outcome model

Every check has one of three outcomes:

```text
PASS    command executed and returned success
FAIL    command executed and returned nonzero, or a required prerequisite/path was absent
SKIP    command did not execute because a documented optional prerequisite was absent
```

Only extended mode may produce `SKIP`, and only for checks explicitly designated optional by this plan.

The final summary must be truthful. Acceptable examples:

```text
All routine checks passed.
```

```text
All package checks passed.
```

```text
Extended validation passed with 1 optional check skipped:
- MSRV: Rust 1.80 toolchain is not installed
```

The script must never print `All checks passed` after a failed command was converted to a warning.

### 5.2 Failure propagation

Use ordinary shell failure propagation under:

```bash
set -euo pipefail
```

Required commands should normally be invoked directly without `||` handling.

Where an optional prerequisite must be detected, test the prerequisite before invoking the command:

```bash
if rustup toolchain list | grep -Eq '^1\.80([.-]|$)'; then
    rustup run 1.80 cargo check ...
else
    record_skip "MSRV" "Rust 1.80 toolchain is not installed"
fi
```

Do not write:

```bash
rustup run 1.80 cargo check ... || warn "MSRV skipped"
```

because that converts compiler failures into skips.

### 5.3 Python interpreter contract

Choose one interpreter once:

```bash
PYTHON_BIN="${PYTHON:-python3}"
```

Use it consistently:

```bash
"$PYTHON_BIN" -m pytest
```

Do not mix `python`, `python3`, and unqualified `pip`.

Before Python build/test work, verify:

- the interpreter exists;
- it is Python 3.10 or newer;
- a virtual environment is active, using interpreter state rather than only trusting `$VIRTUAL_ENV`;
- `pytest` is importable;
- `pytest_asyncio` is importable;
- `maturin` is available as the expected executable for that environment.

A robust virtual-environment check may use:

```sh
"$PYTHON_BIN" -c 'import sys; raise SystemExit(0 if sys.prefix != sys.base_prefix else 1)'
```

If the environment is not ready, fail with setup guidance such as:

```text
Python validation requires an active virtual environment.
Create one and install test tooling:
  python3 -m venv .venv
  source .venv/bin/activate
  python -m pip install maturin pytest pytest-asyncio
```

The script must not perform those installation commands itself.

CI may continue creating `.venv` and installing Tier 1 dependencies before invoking the script.

### 5.4 Optional dependency contract for extended mode

Extended mode must not run `pip install` implicitly.

Before the full HTTPX compatibility suite, verify the required modules are importable. The current compatibility requirements include at minimum:

- `httpx==0.28.1`;
- `requests`;
- `pytest`;
- `pytest_asyncio`;
- `pytest_timeout` when timeout options are used.

Choose one consistent design:

Preferred design:

- extended mode requires the caller to install `compat/httpx/0.28.1/requirements.txt`;
- preflight verifies the exact HTTPX version and required modules;
- missing or wrong dependencies fail with the exact installation command;
- no dependency installation occurs inside the script.

Acceptable alternative:

- remove pytest timeout options from all canonical extended commands;
- require only modules actually used by the resulting command set;
- still fail with setup guidance when dependencies are absent.

Do not retain hidden `pip install ... || true` behavior.

## 6. Required implementation phases

## Phase 0: Baseline and reference inventory

### Deliverables

1. Record baseline SHA `246215cc88c1f13c5c65f7c8bfe0c7ffdee6bd03`.
2. Inspect the complete current `scripts/check.sh`.
3. Enumerate every occurrence in active files, excluding historical `plans/**`, of:

   ```text
   || true
   || warn
   2>/dev/null
   pip install
   validate_httpx_compat_profile.py
   --resolved
   --timeout
   Required CI Gate
   qualification.yml
   release.yml
   candidate identity
   ```

4. Classify each occurrence as:
   - required failure propagation;
   - legitimate preflight handling;
   - stale documentation;
   - historical/non-normative plan text.
5. Confirm the current workflow count and topology before changes.
6. Confirm which Python dependencies Tier 1 actually needs.
7. Confirm which Python dependencies extended compatibility actually needs.
8. Confirm the exact current locations of lifecycle, soak, downstream, merge, package-content, and wheel-smoke tests.
9. Confirm the actual Cargo behavior of coordinated unpublished internal dependencies during `cargo publish --dry-run`.

### Acceptance criteria

- No fail-open occurrence is missed because it appears outside `scripts/check.sh`.
- Historical plans are not edited merely to remove old terminology.
- The implementation distinguishes a real optional prerequisite from a masked product failure.
- Package dry-run design is based on observed Cargo behavior rather than assumptions.

## Phase 1: Refactor common environment and outcome helpers

### Deliverables

Refactor `scripts/check.sh` minimally to add reusable helpers for:

- command presence;
- file/directory presence;
- Python version;
- active virtual environment;
- Python module importability;
- optional skip recording;
- final mode-specific summary.

Suggested conceptual interface:

```bash
require_command cargo
require_command rustc
require_command "$PYTHON_BIN"
require_file "$REPO_ROOT/crates/eggfetch-python/Cargo.toml"
require_python_module pytest
require_python_module pytest_asyncio
record_skip "MSRV" "Rust 1.80 toolchain is not installed"
```

Implementation constraints:

- helpers must remain in `scripts/check.sh` unless separation clearly reduces complexity;
- do not add a shell library dependency;
- do not add JSON output;
- do not add timestamps, run IDs, candidate IDs, or evidence files;
- do not add a test framework for the helper functions;
- preserve `set -euo pipefail`;
- quote all path and interpreter variables;
- use one Python interpreter variable consistently;
- avoid ANSI color output when stdout is not a terminal, or retain current colors only if they do not obscure logs;
- no helper may convert nonzero status from an executed required command into success.

### Acceptance criteria

- the script fails before build/test execution when the Python environment is not suitable;
- the error message contains exact setup guidance;
- no bare `pip` invocation remains;
- no implicit installation remains;
- `python` and `python3` are not mixed unpredictably;
- optional skips are recorded separately from warnings;
- a final success summary is emitted only by the mode dispatcher after all selected required commands return successfully.

## Phase 2: Make Tier 1 deterministic and strict

### Deliverables

Retain the current Tier 1 content:

1. `cargo fmt --all -- --check`;
2. lint-suppression policy;
3. workspace clippy;
4. Rust workspace tests excluding `eggfetch-python` as required by the current PyO3 setup;
5. Python extension build through `maturin develop`;
6. ordinary Python behavior tests excluding the compatibility directory;
7. compact HTTPX compatibility smoke kernel.

Correct the environment behavior:

- require an active virtual environment before `maturin develop`;
- require `maturin`, `pytest`, and `pytest_asyncio` rather than installing them;
- use the selected Python interpreter for every pytest invocation;
- retain no skip path in Tier 1;
- do not suppress output from any Tier 1 command;
- keep the compact compatibility kernel at no more than three test files unless a direct correctness gap requires replacing, not expanding, one file.

Review the ignored soak path:

```text
crates/eggfetch-python/tests/soak_test.py
```

If that path does not exist, remove the stale ignore rather than retaining decorative configuration. The compatibility directory exclusion already separates the current `test_soak.py` location.

### CI integration

Keep `.github/workflows/ci.yml` structurally unchanged except for prerequisite corrections necessary to satisfy the new script contract.

The CI dependency setup should use the activated environment's interpreter:

```sh
python -m pip install maturin pytest pytest-asyncio
```

Do not use unqualified `pip`.

Do not add extended compatibility dependencies to routine CI unless one of the three smoke files actually imports them. Prefer keeping Tier 1 dependency scope small.

### Acceptance criteria

- Tier 1 has no `warn`, skip, or failure-suppression path;
- Tier 1 never installs packages;
- Tier 1 fails when no virtual environment is active;
- Tier 1 succeeds in the CI-created `.venv`;
- Tier 1 uses one Python interpreter consistently;
- all selected tests execute rather than silently skip from missing dependencies;
- `.github/workflows/ci.yml` remains one job, one OS, one Python version, no matrix;
- routine CI dependency installation uses `python -m pip`;
- CI permissions remain read-only;
- no artifact actions are added.

## Phase 3: Make extended mode truthful

### 3.1 Full compatibility

Replace hidden dependency installation with preflight.

Required behavior:

- verify `httpx.__version__ == "0.28.1"`;
- verify required modules are importable;
- if prerequisites are absent, fail before tests with:

  ```text
  Extended HTTPX compatibility dependencies are not installed.
  Install them in the active environment:
    python -m pip install -r compat/httpx/0.28.1/requirements.txt
  ```

- run the full suite without `|| true`;
- preserve pytest output and stderr;
- use `EGGFETCH_COMPAT_REQUIRED=1`;
- use timeout options only when `pytest-timeout` is a verified prerequisite.

### 3.2 API manifest comparison

Keep direct API-manifest generation and comparison if both scripts remain supported and useful.

Required behavior:

- missing script or manifest file is a failure;
- reference and candidate generation failures propagate;
- comparison failure propagates;
- temporary manifest paths are cleaned with `trap` or placed in one temporary directory;
- do not write fixed global `/tmp/eggfetch-manifest.json` paths that can collide across concurrent local runs.

Use:

```bash
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
```

or an equivalent small mechanism.

### 3.3 Feature and documentation checks

Feature checks, feature tests, documentation build, doctests, doc example syntax, and link checks are required once extended mode is selected. Their failures must propagate.

No stderr redirection or warning conversion is permitted.

### 3.4 MSRV

MSRV is the only check permitted to skip based on a specifically absent optional toolchain.

Required behavior:

- if `rustup` is absent, record one explicit MSRV skip before execution;
- if Rust 1.80 is not installed, record one explicit MSRV skip before execution;
- if the toolchain is installed, run the check and propagate all failures;
- do not redirect compiler errors;
- do not report compiler failure as missing toolchain.

### 3.5 FFI

FFI tests are required in extended mode and must fail on build or test failure.

No skip path is needed because the repository and Cargo toolchain are already required prerequisites.

### 3.6 Resource monitor

The resource monitor source is checked into the repository. In extended mode:

- build failure is a failure;
- execution failure is a failure;
- threshold failure is a failure;
- missing expected binary or malformed output is a failure;
- no broad warning conversion is permitted.

If the resource monitor is intentionally non-gating and its output cannot be interpreted deterministically, remove it from `extended` and document its direct manual command. Do not retain a command that always reports success regardless of result.

### 3.7 Lifecycle, soak, downstream, and merge tests

These test files and fixture directories are part of the repository. Therefore:

- missing paths are repository defects and must fail;
- collection failures must fail;
- test failures must fail;
- crashes must fail;
- no stderr redirection;
- no `|| warn`;
- no `|| true`.

Run them directly with the selected Python interpreter.

### 3.8 Benchmarks

Choose one explicit behavior:

Preferred:

- keep a direct benchmark command in extended mode;
- benchmark compilation or execution failures propagate;
- do not perform hosted-runner baseline comparisons;
- do not interpret ordinary variance as pass/fail unless Criterion itself exits nonzero.

Acceptable simplification:

- remove benchmarks from the aggregate extended command;
- print or document the direct `cargo bench` command under a manual diagnostics section.

Do not retain `cargo bench ... || warn` followed by a success summary.

### Acceptance criteria

- `git grep` finds no `|| true` in active validation code;
- `git grep` finds no broad `|| warn` around executed checks;
- extended compatibility dependency failure is visible and nonzero;
- API comparison uses collision-safe temporary paths;
- installed MSRV toolchain failures propagate;
- lifecycle, soak, downstream, merge, resource, and FFI failures propagate;
- stderr remains available;
- extended mode reports skips separately and only for allowed preflight conditions;
- an extended run with one failed test exits nonzero and does not print a success summary.

## Phase 4: Make package mode release-grade without adding release automation

### 4.1 Fresh artifact workspace

Do not build into a persistent repository `dist/` directory for validation.

Create a temporary package workspace:

```bash
PACKAGE_TMP="$(mktemp -d)"
trap 'rm -rf "$PACKAGE_TMP"' EXIT
```

Build the wheel into a dedicated current-run directory, for example:

```sh
maturin build --release \
  -m crates/eggfetch-python/Cargo.toml \
  --out "$PACKAGE_TMP/wheels"
```

Require exactly the intended wheel artifact for the current platform. Do not select an arbitrary wheel from stale prior output.

Pass that exact wheel path to smoke and content validation where supported. If existing scripts accept only a directory, require the directory to contain exactly one candidate wheel.

### 4.2 Crate packaging and dry runs

Every selected crate package check must fail closed.

The implementer must first observe Cargo behavior for unpublished coordinated internal versions. Then select the simplest truthful contract.

Preferred contract when all dry runs are independently valid:

```sh
cargo publish -p eggfetch-core --dry-run
cargo publish -p eggfetch-cli --dry-run
cargo publish -p eggfetch-ffi --dry-run
cargo publish -p eggfetch-python --dry-run
cargo publish -p eggfetch-node --dry-run
```

All failures propagate.

If dependent dry runs cannot succeed before the new internal dependency version exists on crates.io, use a staged contract rather than warning suppression:

- `./scripts/check.sh package` runs `cargo package` or an equivalent local package-content/build verification for every publishable crate;
- it runs `cargo publish --dry-run` for crates whose dependencies permit a truthful pre-publication dry run;
- `docs/releases/process.md` requires `cargo publish --dry-run -p <crate>` immediately before each dependent crate's real publication, after its new internal dependencies are visible in the registry;
- any such staged limitation is explicitly documented;
- no failing dry run is converted to success.

Do not use `--allow-dirty` in the normal clean-worktree release path unless there is a documented reason.

### 4.3 Wheel smoke

The wheel smoke test is required and fail-closed.

It must:

- install the exact current-run wheel into a clean temporary environment;
- verify import;
- perform the existing compact behavioral smoke;
- fail on installation, import, request, or cleanup error;
- avoid public network access if the existing smoke can use a local server;
- leave no persistent environment in the repository.

### 4.4 Package-content validation

Package-content validation is required and fail-closed.

Remove:

- stderr suppression;
- warning conversion;
- loops that silently succeed when no wheel matched.

Before iterating, verify exactly one wheel exists. Zero or multiple unexpected wheels must fail with a clear message.

### 4.5 Package-mode final result

Package mode may print `All package checks passed` only after:

- Tier 1 passes;
- every selected crate package/dry-run check passes;
- wheel build passes;
- wheel smoke passes;
- package-content validation passes.

### Acceptance criteria

- package mode contains no failure-to-warning conversion;
- package mode uses fresh temporary output;
- stale repository wheels cannot satisfy the check;
- zero current-run wheels fails;
- multiple ambiguous current-run wheels fails;
- every actual dry-run failure returns nonzero;
- package-content failure returns nonzero;
- wheel smoke failure returns nonzero;
- no real publication command exists in the script;
- temporary package artifacts are removed on success and failure;
- release documentation matches the observed Cargo dependency-order limitation.

## Phase 5: Remove stale and duplicated documentation

### 5.1 AGENTS.md

Delete the command referencing the removed script:

```sh
python scripts/validate_httpx_compat_profile.py ...
```

Reduce the Quick Commands section to canonical commands:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Retain a small number of direct commands only when they are useful for focused development and are verified against current paths and dependencies.

Do not duplicate the entire extended command graph in `AGENTS.md`.

Remove or correct timeout-bearing direct commands. If retained, they must be preceded by the exact requirement installation that provides `pytest-timeout`, or they must omit timeout options.

Preserve the explicit prohibition on adding CI/release complexity without user direction.

### 5.2 CONTRIBUTING.md and skills

Ensure contributor and skill documents:

- identify the active virtual-environment prerequisite;
- identify exact setup commands;
- use `python -m pip`;
- do not claim `scripts/check.sh` installs tooling;
- do not reference deleted scripts;
- do not instruct agents to dispatch deleted workflows;
- do not reintroduce large mandatory matrices.

### 5.3 Verification and CI docs

Update `docs/verification-policy.md` and `docs/architecture/build-ci.md` only as needed to state:

- required selected checks fail closed;
- optional skips are preflight-only and named;
- local setup is explicit;
- package mode is a truthful pre-publication check, not release authorization.

Do not expand these documents into another evidence specification.

### 5.4 Release docs

Verify every command in `docs/releases/process.md` against current output paths and script semantics.

In particular:

- package validation must be described as fail-closed;
- staged dependent-crate dry runs, if required by Cargo registry resolution, must be explicit;
- optional PyPI build output must match the upload path, for example by using `maturin build --release --out dist` before `twine upload dist/*`;
- no workflow dispatch step may return;
- no CI result may be described as release authorization.

### 5.5 Compatibility docs

Remove references to:

- deleted profile validators;
- candidate identity;
- qualification evidence;
- ignored `--resolved` behavior if the option is removed.

Retain direct compatibility-test and API-manifest commands that actually work.

### Acceptance criteria

- no active documentation references `validate_httpx_compat_profile.py`;
- no active documentation instructs use of deleted workflows;
- direct timeout commands either have verified dependencies or omit timeout flags;
- canonical validation commands are not duplicated inconsistently;
- Python setup uses `python -m pip`;
- release output paths are correct;
- manual crates.io publication remains explicit;
- documentation changes reduce rather than expand operational complexity.

## Phase 6: Correct the API-manifest comparator interface

### Preferred implementation: remove the dead option

Remove from `scripts/compare_httpx_api_manifest.py`:

```text
--resolved
```

Remove all active documentation examples that pass it.

Update the parser docstring, help text, and any direct tests that still describe workflow validation rather than API comparison.

The active `allowed-differences.toml` logic already rejects `resolved` entries in the active allowlist. A separate ignored path adds no correctness.

### Alternative implementation: wire it directly and minimally

Use this alternative only if current direct compatibility behavior genuinely depends on a separate resolved ledger.

Requirements:

- load the file;
- validate its schema;
- ensure resolved entries cannot waive active differences;
- report only useful historical consistency findings;
- add no candidate identity, evidence, workflow, or release coupling;
- add focused unit coverage for the direct comparator behavior.

Do not preserve the option merely for compatibility with deleted qualification commands.

### Acceptance criteria

- no accepted CLI option is silently ignored;
- comparator help text describes only implemented behavior;
- direct API comparison still fails on unexplained and stale active differences;
- no evidence or qualification concepts return;
- active docs use the final supported command line.

## Phase 7: Validate the corrected script by controlled failure injection

Do not add a permanent shell-test framework. Perform targeted, documented validation during implementation.

### 7.1 Routine mode

Run in a correctly prepared environment:

```sh
./scripts/check.sh
```

Then verify setup failure behavior in a clean shell without an active virtual environment:

```sh
env -u VIRTUAL_ENV <appropriate clean-shell invocation> ./scripts/check.sh
```

The exact invocation may vary by local environment, but the result must be:

- nonzero exit;
- no package installation;
- actionable setup guidance;
- no `All routine checks passed` message.

### 7.2 Extended prerequisite failure

In an environment without the exact compatibility dependencies, run:

```sh
./scripts/check.sh extended
```

Expected:

- nonzero exit before compatibility tests;
- exact dependency installation guidance;
- no hidden installation;
- no success summary.

Then install the documented requirements into the active environment and rerun.

### 7.3 Executed-test failure

Temporarily introduce or select a known failing test in a disposable worktree/branch. Run the relevant extended path.

Expected:

- pytest failure is visible;
- script exits nonzero;
- failure is not called skipped;
- temporary test modification is reverted and not committed.

### 7.4 Package failure

Use a disposable worktree/branch to create a reversible package defect, such as an invalid package metadata field or content-validation mismatch.

Run:

```sh
./scripts/check.sh package
```

Expected:

- nonzero exit;
- exact failing command visible;
- no success summary;
- no registry mutation;
- temporary defect reverted and not committed.

### 7.5 Stale artifact isolation

Place an unrelated stale wheel in the repository's historical `dist/` path, then run package mode.

Expected:

- package mode ignores the stale wheel;
- it validates only the current-run temporary artifact;
- stale wheel is not selected accidentally.

Remove the stale artifact after the check.

### 7.6 Optional MSRV skip

Run extended mode without Rust 1.80 installed, if practical.

Expected:

- one explicit MSRV skip recorded before command execution;
- other checks continue;
- final summary reports the skip;
- no compiler failure is hidden.

When Rust 1.80 is installed, introduce no skip and propagate compiler failure normally.

### Acceptance criteria

- each failure-injection case produces the expected nonzero exit;
- no temporary defect is committed;
- no package is published;
- no validation dependency is installed implicitly;
- success summaries are absent on failure;
- skip summaries are specific and truthful.

## Phase 8: Validate current CI execution

Push the corrective implementation and inspect the resulting Actions run.

Required evidence:

- one workflow run named `CI`;
- one job named `ci`;
- no other push-triggered workflow;
- no matrix children;
- no artifact uploads;
- read-only permissions;
- successful virtual-environment setup;
- successful `./scripts/check.sh` execution;
- total runtime within the existing 20-minute timeout;
- retained run URL in the implementation handoff.

If CI fails, correct the actual Tier 1 setup or test defect. Do not add warning suppression, `continue-on-error`, a larger matrix, or another aggregation job.

### Acceptance criteria

- current `main` has a retained successful one-job CI run;
- the run contains no package or extended checks;
- only routine dependencies are installed;
- no release or qualification action is available;
- CI topology remains within the policy complexity budget.

## Phase 9: Verify repository settings outside code

Using GitHub repository settings, inspect:

### Branch protection and rulesets

Remove requirements for deleted checks, including any historical names such as:

- `Required CI Gate`;
- Rust matrix jobs;
- Python matrix jobs;
- wheel-smoke jobs;
- security jobs;
- FFI jobs;
- qualification jobs.

If a required check remains desired, require only the current `CI / ci` context.

### Actions secrets and variables

Remove publication credentials no longer used by workflows, including any retained:

- crates.io token;
- PyPI token;
- TestPyPI token;
- npm token;
- release-specific signing or attestation credential.

Do not remove unrelated deployment credentials without identifying their owner and use.

### Environments

Remove obsolete release/TestPyPI/PyPI environments if they exist solely for deleted workflows.

### Actions permissions

Confirm default workflow permissions are read-only where repository policy permits.

### Handling unavailable administrative access

If the implementing agent cannot inspect or modify these settings:

- do not claim completion;
- provide a short owner-action checklist;
- name each unverified setting;
- distinguish code-complete from operationally unverified status.

### Acceptance criteria

- deleted checks cannot block merging;
- only `CI / ci` is required, if any check is required;
- no unused publication credential remains available to Actions;
- obsolete release environments are removed;
- unavailable settings are explicitly handed off rather than guessed.

## Phase 10: Closure search and final reconciliation

Run active-tree searches excluding historical plans:

```sh
git grep -n "validate_httpx_compat_profile.py" -- ':!plans/**'
git grep -n "Required CI Gate" -- ':!plans/**'
git grep -n "qualification.yml" -- ':!plans/**'
git grep -n "release.yml" -- ':!plans/**'
git grep -n "candidate identity" -- ':!plans/**'
git grep -n "candidate_sha" -- ':!plans/**'
git grep -n "CARGO_REGISTRY_TOKEN" -- . ':!plans/**'
git grep -n "PYPI_TOKEN" -- . ':!plans/**'
git grep -n "TESTPYPI_TOKEN" -- . ':!plans/**'
git grep -n "NPM_TOKEN" -- . ':!plans/**'
git grep -n "|| true" -- scripts .github ':!plans/**'
git grep -n "|| warn" -- scripts .github ':!plans/**'
git grep -n "2>/dev/null" -- scripts/check.sh
git grep -n "pip install" -- scripts/check.sh
git grep -n -- "--resolved" -- ':!plans/**'
```

Interpretation:

- historical plan references are allowed;
- manual release documentation may contain `cargo publish` commands;
- active workflow secret references must be absent;
- validation failure suppression in `scripts/check.sh` must be absent;
- an intentional preflight grep or shell conditional is acceptable only when it does not mask an executed command failure;
- `--resolved` must be absent if the preferred comparator correction is used.

### Acceptance criteria

- stale active references are removed;
- no validation failure suppression remains;
- no implicit install remains;
- no ignored CLI option remains;
- manual release policy remains intact;
- no workflow expansion occurred;
- all three canonical script modes have documented successful runs in the prepared environment, subject only to explicitly recorded optional MSRV skip.

## 7. Explicit file-level instructions

### `scripts/check.sh`

Required changes:

- select one Python interpreter;
- verify Python version;
- require active virtual environment;
- require tools and modules;
- remove implicit `pip install`;
- remove hidden dependency installation;
- remove broad `|| warn` and `|| true` paths;
- remove stderr suppression from required checks;
- permit only explicit preflight MSRV skip unless another optional prerequisite is justified in the implementation summary;
- use temporary directories for API manifests and package artifacts;
- fail on package dry-run, wheel smoke, and content-validation errors;
- print mode-specific truthful summaries.

### `.github/workflows/ci.yml`

Allowed changes:

- use `python -m pip` inside the active virtual environment;
- install exact Tier 1 prerequisites;
- correct shell activation if needed.

Forbidden changes:

- new jobs;
- matrices;
- extended/package invocation;
- artifact actions;
- write permissions;
- publication credentials;
- failure suppression;
- timeout inflation beyond the current policy budget without measured necessity.

### `AGENTS.md`

Required changes:

- remove deleted-script commands;
- reduce duplicated validation graph;
- point to the three canonical modes;
- retain focused commands only when correct;
- remove unsupported timeout flags or document the exact prerequisite;
- keep manual release language.

### `scripts/compare_httpx_api_manifest.py`

Required change:

- remove the ignored `--resolved` option and stale workflow-oriented parser text, unless the direct minimal alternative is implemented and tested.

### `docs/releases/process.md`

Required changes:

- align with fail-closed package mode;
- document staged dry-run behavior if Cargo requires registry-visible internal dependencies;
- correct optional PyPI build output path;
- preserve local manual publication and immutability guidance.

### Other docs and skills

Required changes:

- remove stale references;
- align setup instructions;
- use `python -m pip`;
- avoid duplicating the command graph;
- preserve the simplified policy.

## 8. Suggested implementation commit sequence

Keep the pass reviewable and small.

### Commit 1: strict validation semantics

Scope:

- `scripts/check.sh` environment helpers;
- Tier 1 strict setup;
- extended failure propagation;
- package failure propagation;
- temporary artifact directories.

Suggested message:

```text
build: make local validation fail closed
```

### Commit 2: stale interface and documentation cleanup

Scope:

- remove ignored comparator option;
- clean `AGENTS.md`;
- reconcile contributor, skill, compatibility, CI, and release docs;
- correct PyPI output command if needed.

Suggested message:

```text
docs: reconcile validation and manual release commands
```

### Commit 3: closure evidence only if needed

Scope:

- narrow fixes discovered by actual CI execution or stale-reference searches;
- no feature work;
- no workflow expansion.

Suggested message:

```text
chore: close validation truthfulness references
```

## 9. Global acceptance criteria

The corrective pass is complete only when all criteria below are satisfied.

### Structural preservation

1. `.github/workflows/ci.yml` is the only push/PR workflow.
2. CI has exactly one job.
3. CI runs on Ubuntu only.
4. CI has no matrix.
5. CI has read-only permissions.
6. CI has no artifact upload/download.
7. CI invokes only routine mode.
8. No release or qualification workflow returns.
9. No publication secret is referenced.
10. Release cadence remains manual.

### Environment correctness

11. `scripts/check.sh` selects one Python interpreter.
12. Python 3.10+ is verified.
13. An active virtual environment is required for Python build/test work.
14. Missing prerequisites fail with exact setup guidance.
15. The script never invokes bare `pip`.
16. The script never installs Python packages.
17. CI installs prerequisites before invoking the script.
18. CI uses `python -m pip`.
19. Tool checks cover commands actually invoked.
20. Missing repository paths fail rather than skip.

### Routine mode

21. Routine mode has no skip path.
22. Routine mode has no warning-based failure conversion.
23. Format failure exits nonzero.
24. Lint policy failure exits nonzero.
25. Clippy failure exits nonzero.
26. Rust test failure exits nonzero.
27. Maturin build failure exits nonzero.
28. Python behavior test failure exits nonzero.
29. Compatibility smoke failure exits nonzero.
30. Routine success summary appears only after all checks pass.

### Extended mode

31. Full compatibility setup is preflighted.
32. Wrong HTTPX version fails with guidance.
33. No hidden dependency installation occurs.
34. Full compatibility test failure exits nonzero.
35. API manifest generation failure exits nonzero.
36. API comparison failure exits nonzero.
37. Feature check/test failure exits nonzero.
38. Documentation check failure exits nonzero.
39. FFI failure exits nonzero.
40. Resource-monitor failure either exits nonzero or the check is removed from aggregate extended mode.
41. Lifecycle failure exits nonzero.
42. Soak failure exits nonzero.
43. Downstream fixture failure exits nonzero.
44. Merge test failure exits nonzero.
45. Benchmark failure either exits nonzero or benchmark execution is moved to a clearly manual command.
46. MSRV skip occurs only before execution when the toolchain is absent.
47. Installed-toolchain MSRV compiler failure exits nonzero.
48. Skips are named in the final summary.
49. Executed failures are never called skipped.
50. Stderr is not hidden.

### Package mode

51. Package mode uses a fresh temporary artifact directory.
52. Stale `dist/` wheels cannot satisfy validation.
53. Zero current-run wheels fails.
54. Ambiguous multiple candidate wheels fail.
55. Crate package/dry-run failures are not converted to warnings.
56. Actual Cargo dependency-order limitations are documented truthfully.
57. Wheel build failure exits nonzero.
58. Wheel smoke failure exits nonzero.
59. Package-content failure exits nonzero.
60. Package mode never publishes.
61. Temporary artifacts are cleaned.
62. Package success summary appears only after every required selected check passes.

### Interface and documentation

63. No active file references `validate_httpx_compat_profile.py`.
64. No accepted comparator option is ignored.
65. `--resolved` is removed from active docs if removed from the parser.
66. `AGENTS.md` points primarily to canonical script modes.
67. Unsupported timeout flags are removed or their dependency is explicit.
68. Contributor and skill setup commands use `python -m pip`.
69. Release docs use correct build/output paths.
70. Release docs remain manual and local.
71. No active doc requires qualification evidence.
72. No active doc treats CI as release authorization.

### Operational closure

73. Current `main` has one retained successful `CI / ci` run.
74. No other push/PR workflow runs.
75. Branch protection does not require deleted checks.
76. Only `CI / ci` is required, if a required check is configured.
77. Unused publication secrets are removed from Actions.
78. Obsolete release environments are removed.
79. Any unavailable administrative verification is explicitly handed off.
80. Final stale-reference searches are clean outside historical plans.

## 10. Rejection conditions

Reject the implementation if any of the following occurs:

- `|| true` remains around a selected validation dependency or test command;
- `|| warn` remains around an executed required check;
- stderr is suppressed for a required check;
- package mode exits zero after a failed dry run;
- package mode exits zero after failed content validation;
- extended mode calls failed tests skipped;
- the script installs `maturin` or compatibility requirements automatically;
- the script mutates a global Python environment;
- a new CI job or matrix is added;
- extended or package mode is added to push/PR CI;
- artifact uploads return;
- release publication returns to GitHub Actions;
- a new workflow validator or evidence schema is introduced;
- stale wheels can satisfy package validation;
- an ignored CLI option remains;
- documentation references deleted scripts;
- branch protection remains tied to deleted checks without explicit owner handoff;
- the implementation claims repository-setting cleanup without verifying it;
- a product test is deleted solely to avoid making extended mode fail;
- timeouts are simply increased to hide nondeterminism;
- warning text replaces real failure semantics;
- the corrective pass expands into unrelated compatibility or networking work.

## 11. Handoff requirements

The implementing agent's final handoff must include:

- baseline SHA;
- final SHA;
- changed files;
- exact final routine command sequence;
- exact final extended command sequence;
- exact final package command sequence;
- Python environment prerequisite and setup command;
- list of all removed `|| true`, `|| warn`, stderr-suppression, and implicit-install paths;
- explanation of the final MSRV skip behavior;
- observed Cargo dry-run behavior for coordinated internal dependencies;
- package artifact-isolation behavior;
- routine local run result;
- extended local run result and any explicit optional skip;
- package local run result;
- controlled failure-injection results;
- one-job CI run URL and conclusion;
- workflow/job count confirmation;
- branch-protection verification;
- Actions-secret/environment verification;
- stale-reference search results;
- any unavailable administrative action as a clear owner checklist;
- explicit confirmation that release remains manual and no workflow can publish.

## 12. Final decision rule

This corrective pass is closed when the simplified architecture remains intact and every retained validation mode tells the truth.

A successful routine run must mean every routine command executed and passed. A successful extended run must mean every executed extended command passed, with any optional preflight skip named explicitly. A successful package run must mean every required package check passed against artifacts built by that invocation. No successful result may be manufactured by warning conversion, hidden dependency installation, stale artifacts, ignored options, or suppressed diagnostics.

The final repository should remain simple: one CI job, one local script, three explicit modes, and manual publication.