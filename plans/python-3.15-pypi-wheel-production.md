# Python 3.15 PyPI Wheel Production

Planning baseline: `c249ddc1a11c93d0ab6463dd6bc3b0550e419e4f` (`main`, 2026-09-18)
Status: implementation complete, pending build-only rehearsal
Scope: extend the existing manually dispatched PyPI wheel production matrix from CPython 3.10–3.14 to CPython 3.10–3.15 on the already-supported release platforms; preserve the current version-specific CPython ABI model, release security model, MSRV, and package behavior.

## Objective

Make the next ordinary eggfetch PyPI release produce and validate standard GIL-enabled CPython 3.15 wheels everywhere the project already produces native wheels:

- Linux manylinux2014 x86_64;
- macOS arm64;
- Windows x86_64.

The change must remain a packaging/release-matrix update. It must not introduce abi3/abi3t wheels, free-threaded `3.15t` wheels, new platforms, a second publishing path, a new dependency stack, or a Python-runtime behavior change.

The intended release set becomes:

```text
3 platforms × 6 CPython versions (3.10–3.15) = 18 wheels
18 wheels + 1 sdist = 19 distributions
```

## Current state

At the planning baseline:

- `.github/workflows/pypi.yml` builds CPython 3.10, 3.11, 3.12, 3.13, and 3.14 on all three supported platform/architecture pairs;
- the workflow therefore expects 15 wheels and 16 total distributions including the sdist;
- `scripts/validate_wheel_coverage.py` hard-codes `cp310` through `cp314` and a 15-wheel expectation;
- `crates/eggfetch-python/pyproject.toml` declares `requires-python = ">=3.10"` and classifiers through Python 3.14;
- `README.md`, `docs/releases/process.md`, and `docs/verification-policy.md` describe Python 3.10–3.14 and the 15-wheel/16-distribution release set;
- `eggfetch-python` already uses PyO3 0.29.x, which supports Python 3.15;
- the pinned maturin 1.14.1 release supports the PyO3 0.29 / Python 3.15 build path;
- the currently pinned `actions/setup-python` v5.6.0 action supports prerelease fallback through its `allow-prereleases` input.

As of 2026-09-18, Python 3.15.0 is still prerelease: CPython 3.15.0rc2 was released on 2026-09-01 and final 3.15.0 is scheduled for 2026-10-01. The workflow must therefore be able to build against the current release candidate now without requiring a later workflow rewrite when 3.15 becomes generally available.

## Design constraints

Preserve these existing release properties:

- ordinary wheels remain version-specific CPython wheels, not abi3 or abi3t wheels;
- free-threaded CPython remains outside this wheel matrix;
- Linux remains manylinux2014 x86_64;
- macOS remains arm64 only;
- Windows remains x86_64 only;
- the sdist build may continue using Python 3.12 because the sdist itself is not a per-interpreter binary artifact;
- Trusted Publishing/OIDC, protected `pypi` environment approval, exact tag/version validation, live security preflight, and the manual `workflow_dispatch` publication model remain unchanged;
- Rust MSRV remains 1.89;
- no PyO3, maturin, Tokio, or other dependency bump is permitted merely to add 3.15 if the existing stack builds and tests successfully;
- the package's minimum supported Python remains 3.10.

If the existing dependency/tooling stack unexpectedly fails on CPython 3.15 for a real compatibility reason, stop this plan at that finding and open a narrowly scoped corrective dependency/tooling plan. Do not silently broaden this work into a dependency modernization campaign.

## Part A — Extend the wheel matrix

Update `.github/workflows/pypi.yml`.

Add one Python 3.15 matrix row for each existing platform/architecture pair, preserving the same target and manylinux policy used by the corresponding 3.14 row:

```text
linux   / x86_64 / CPython 3.15 / x86_64-unknown-linux-gnu / manylinux2014
macos   / arm64  / CPython 3.15 / aarch64-apple-darwin
windows / x86_64 / CPython 3.15 / x86_64-pc-windows-msvc
```

Do not introduce a new architecture or platform in this pass.

### A1. Make prerelease setup explicit and self-expiring

Because 3.15 is still prerelease at implementation time, configure the existing `actions/setup-python` step so the 3.15 matrix entries may resolve the current prerelease when no GA 3.15 exists.

Prefer a matrix-sensitive expression equivalent to:

```yaml
allow-prereleases: ${{ matrix.python == '3.15' }}
```

The exact YAML form may differ if required by GitHub Actions expression handling, but the behavior must be:

- 3.10–3.14 continue selecting stable interpreters only;
- 3.15 may fall back to the current prerelease before 3.15.0 final exists;
- once 3.15.0 final is available, the same `python-version: "3.15"` row resolves a stable 3.15 interpreter without another repository change.

Do not hard-code a specific 3.15 release-candidate patch unless a reproducibility defect in `setup-python` requires it. The project currently tracks the latest patch within each supported minor line.

### A2. Preserve the selected-interpreter identity

The maturin build must use the interpreter selected by `setup-python` for that matrix row.

The current `-i python${{ matrix.python }}` path may be retained if the 3.15 rows prove it resolves correctly on Linux, macOS, and Windows. If the prerelease interpreter exposes a naming inconsistency on any runner, prefer wiring maturin to the `setup-python` step's emitted interpreter path rather than adding platform-specific executable-name workarounds.

Any such adjustment should apply uniformly to the existing matrix rather than creating a 3.15-only invocation path.

## Part B — Update release-set accounting

The workflow contains hard-coded cardinality checks that must move with the matrix.

Update the assembled release validation from:

```text
15 wheels
1 sdist
16 total distributions
```

to:

```text
18 wheels
1 sdist
19 total distributions
```

Specifically:

- the `assemble` job must require exactly 18 wheels;
- the `publish` job must require exactly 19 publishable distributions;
- failure messages must report the new expected cardinalities;
- duplicate-filename detection remains unchanged;
- `twine check` remains required for the complete assembled set.

Do not weaken exact-count validation to make the matrix easier to extend. The release workflow is intentionally fail-closed.

## Part C — Extend wheel-coverage validation

Update `scripts/validate_wheel_coverage.py`.

Required changes:

- extend `EXPECTED_PYTHON_VERSIONS` with `cp315`;
- update the module documentation from five to six Python versions;
- update the expected wheel count from 15 to 18;
- retain all three existing platform normalizations;
- retain explicit rejection of abi3 wheels;
- do not accept `cp315t`, `abi3t`, `py3-none-any`, or any new platform as satisfying the CPython 3.15 matrix;
- preserve duplicate/missing/unexpected coverage failures.

The expected Cartesian product after the change is exactly:

```text
{linux_x86_64, macosx_arm64, win_amd64}
×
{cp310, cp311, cp312, cp313, cp314, cp315}
```

If this validator has no direct unit-test harness, validate it against the real 18-wheel build-only workflow artifact set. A small fixture-based regression test is acceptable if it can be added without introducing another packaging framework, but a new test framework is not required for this plan.

## Part D — Update Python package metadata

Update `crates/eggfetch-python/pyproject.toml`.

Add:

```text
Programming Language :: Python :: 3.15
```

The classifier is already recognized by PyPI and may be published once the 3.15 wheel matrix passes.

Do not change:

```toml
requires-python = ">=3.10"
```

Do not add free-threading classifiers or claim abi3/abi3t compatibility in this pass.

No package version bump is required merely to land the build-matrix change. The new wheel coverage should normally take effect with the next ordinary PyPI release version; do not republish or mutate an already-published release as part of this plan.

## Part E — Keep public release documentation truthful

Update all currently authoritative places that describe Python wheel support.

At minimum:

### `README.md`

Change the native-package support statement from Python 3.10–3.14 to Python 3.10–3.15 after the 3.15 build-only qualification passes.

### `docs/releases/process.md`

Update:

- each supported-wheel-matrix row to include 3.15;
- the release-set statement from 15 wheels + 1 sdist = 16 distributions to 18 wheels + 1 sdist = 19 distributions.

Keep the manual rehearsal/publication sequence and Trusted Publishing instructions unchanged.

### `docs/verification-policy.md`

Update:

- each PyPI wheel-matrix row to include 3.15;
- the expected release-set count to 18 wheels + 1 sdist = 19 distributions.

Do not change the distinction between routine CI and the manually dispatched release-only wheel workflow.

### `plans/README.md`

Register this plan as the active packaging/release-matrix plan. On completion, change the entry to completed and record the implementation/qualification evidence rather than leaving an unqualified support claim.

## Part F — Qualification before publication

This plan is not complete merely because the matrix is syntactically updated.

### F1. Local repository gates

Run the canonical checks affected by packaging/release metadata:

```sh
./scripts/check.sh
./scripts/check.sh package
```

Also run any existing workflow/YAML validation already included by those commands.

No new routine CI workflow should be added.

### F2. Build-only PyPI workflow rehearsal

Dispatch `.github/workflows/pypi.yml` with:

```text
publish=false
```

on the implementation ref before using the change for an actual publication.

The run must produce exactly 18 wheel artifacts plus one sdist.

Every 3.15 matrix job must:

- install the intended CPython 3.15 interpreter;
- build exactly one wheel;
- produce a version-specific `cp315` wheel, not abi3/abi3t;
- install that wheel into the isolated smoke-test environment;
- pass `scripts/wheel_smoke.py`;
- pass wheel metadata validation;
- upload the expected wheel artifact.

Record the exact interpreter version selected for each 3.15 job. Before 2026-10-01 this is expected to be a 3.15 prerelease; after GA it should be stable 3.15.x.

### F3. Release-set proof

The assemble job must prove:

- exactly 18 wheels are present;
- exactly one sdist is present;
- no duplicate filenames exist;
- `scripts/validate_wheel_coverage.py` reports the full 18-element matrix;
- `twine check` passes for all 19 distributions.

Inspect the CPython 3.15 wheel filenames and require the equivalent tags for:

```text
cp315-cp315-manylinux...x86_64
cp315-cp315-macosx...arm64
cp315-cp315-win_amd64
```

The exact manylinux/macOS deployment tag prefix may remain whatever maturin currently emits for the corresponding target; the Python and ABI tags must be `cp315-cp315`.

### F4. Publication readiness

A successful `publish=false` rehearsal is the gate for claiming 3.15 wheel-production support in release documentation.

Actual PyPI publication remains a maintainer release decision and continues to require:

- an exact matching `v<SEMVER>` tag;
- exact checked-tag/HEAD identity;
- release-version validation;
- live security preflight;
- protected `pypi` environment approval;
- Trusted Publishing/OIDC.

This plan must not weaken or bypass any of those controls.

## Failure handling

If any 3.15 row fails, classify the failure before changing dependencies.

Expected categories include:

1. `setup-python` cannot resolve the current prerelease on one runner;
2. the interpreter executable selected by the action is not the executable maturin receives;
3. PyO3 0.29.x fails against the selected 3.15 build;
4. maturin 1.14.1 cannot construct or tag the 3.15 wheel correctly;
5. eggfetch native code has an actual CPython 3.15 compile/runtime incompatibility;
6. smoke/package validation exposes a 3.15-only behavioral defect.

For categories 1–2, correct the workflow plumbing within this plan if the fix is narrow and does not change the supported platform matrix.

For categories 3–6, record the exact failure and stop before opportunistically upgrading broad dependency sets. Open a focused corrective plan if executable/package code or dependency versions must change.

## Non-goals

This plan does not include:

- CPython free-threaded 3.15t wheels;
- PEP 803 abi3t adoption;
- conventional abi3 adoption;
- Linux aarch64 wheels;
- macOS x86_64/universal2 wheels;
- Windows arm64 wheels;
- PyPy or GraalPy wheels;
- dropping Python 3.10;
- changing `requires-python`;
- changing eggfetch Python API behavior;
- upgrading PyO3 or maturin without a demonstrated 3.15 blocker;
- changing Rust MSRV;
- automatic PyPI publication on tags;
- a second release workflow;
- changes to crates.io release policy;
- republishing an existing PyPI version solely to backfill a 3.15 wheel.

Those may be evaluated separately if desired.

## Acceptance criteria

The implementation is complete only when all of the following are true:

- [ ] `.github/workflows/pypi.yml` contains CPython 3.15 rows for Linux x86_64, macOS arm64, and Windows x86_64.
- [ ] Before Python 3.15 GA, the 3.15 rows can resolve a prerelease without allowing prerelease fallback for 3.10–3.14.
- [ ] After GA, the same 3.15 matrix configuration naturally resolves stable 3.15.x without a required follow-up patch.
- [ ] The wheel workflow expects exactly 18 wheels and 19 total distributions.
- [ ] `scripts/validate_wheel_coverage.py` requires `cp315` and exactly the 18 expected platform/interpreter combinations.
- [ ] abi3, abi3t, free-threaded, and unexpected-platform wheels do not satisfy the matrix.
- [ ] `crates/eggfetch-python/pyproject.toml` contains the Python 3.15 classifier while retaining `requires-python >=3.10`.
- [ ] `README.md`, `docs/releases/process.md`, and `docs/verification-policy.md` describe Python 3.10–3.15 and the 18-wheel/19-distribution release set.
- [ ] `./scripts/check.sh` passes.
- [ ] `./scripts/check.sh package` passes.
- [ ] A manual `publish=false` PyPI workflow rehearsal passes on all 18 wheel jobs.
- [ ] All three 3.15 wheels install and pass `scripts/wheel_smoke.py` under the selected 3.15 interpreter.
- [ ] The 3.15 wheels are tagged `cp315-cp315` for the existing supported platforms.
- [ ] The assemble job validates all 18 wheels plus one sdist and `twine check` passes.
- [ ] No dependency, MSRV, public API, Trusted Publishing, security-preflight, or release-authentication change is introduced unless separately justified by a recorded blocker.

## Closure record

Implementation (local gates green; `publish=false` rehearsal pending
maintainer dispatch):

- implementation: matrix + validator + classifier + docs changes on
  `main` (see `plans/README.md` active entry); Tier 1 (`./scripts/check.sh`)
  and Tier 3 (`./scripts/check.sh package`) pass locally after the change.
- `publish=false` workflow run URL/ID: pending — dispatch
  `.github/workflows/pypi.yml` with `publish=false` from the
  implementation commit and record the run here before publishing a
  release that claims 3.15 wheels.
- selected CPython 3.15 patch/prerelease per platform: pending rehearsal
  (expected 3.15.0rc2-line prerelease before 2026-10-01, stable 3.15.x after).
- CPython 3.15 wheel filenames: pending rehearsal (must be
  `cp315-cp315` for manylinux x86_64, macosx arm64, win_amd64).
- 18-wheel coverage-validator output: proven locally against an
  18-filename fixture set (pass on full matrix; fail-closed on a missing
  wheel and on abi3/`py3-none-any` intruders).
- Tier 1/package-check results: green locally (Node JS surface explicit
  SKIP, no built `eggfetch.node` artifact — pre-existing).
- PyO3 0.29.x and maturin 1.14.1 pins: unchanged, as required.
- runner-specific workflow adjustment: none — maturin keeps the uniform
  `-i python${{ matrix.python }}` invocation; only the
  `allow-prereleases: ${{ matrix.python == '3.15' }}` expression was added.
- qualification timing: implementation landed before Python 3.15.0 final
  (2026-10-01); rehearsal patch identity to be recorded at dispatch.
- deliberately unchanged: `compat/*/profile.toml` facade `python-versions`
  and httpx `_diagnostics.py` (exact-SHA Stage C evidence stays
  3.10–3.14-bound until renewed on a 3.15-tested SHA); only the httpx2
  profile comment was refreshed to keep the wheel-vs-facade distinction
  truthful. No dependency, MSRV, public API, Trusted Publishing,
  security-preflight, or release-authentication change.
