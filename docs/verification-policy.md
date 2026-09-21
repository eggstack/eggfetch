# Verification Policy

This document is the normative statement of CI, verification, and release policy for eggfetch.

## Principles

1. **CI is a fast regression safety net.** It catches regressions introduced by a change. It is not a release authority.
2. **CI does not determine release cadence.** Releases are maintainer decisions performed from a trusted local environment.
3. **Routine CI does not publish packages or create releases.** GitHub Actions holds no publication credentials and performs no publication steps in the push/PR workflow.
4. **Local validation is canonical.** The checked-in `./scripts/check.sh` is the single source of validation. CI repeats the same command on Ubuntu.
5. **Extended checks are opt-in.** Slower or less frequently useful checks run via `./scripts/check.sh extended` and are not triggered by every push or pull request.
6. **Packaging checks are local validation.** `./scripts/check.sh package` validates packaging without publication. `eggfetch-http-connect` (the dependency leaf) receives a full `cargo publish --dry-run`. Dependent crates receive package-structure validation via `cargo package --list` and manifest version verification; full `cargo publish --dry-run` runs at publication time.
7. **crates.io publication is manual.** A maintainer publishes from a trusted local environment. GitHub Actions does not publish.
8. **PyPI publication is manually dispatched.** The PyPI wheel workflow (`.github/workflows/pypi.yml`) is triggered only by `workflow_dispatch`. It is not a merge gate and does not run on push or pull request.
9. **Historical qualification plans are non-normative.** Completed plans are records of past work, not active CI or release requirements.
10. **Verification infrastructure must remain materially simpler than the behavior it verifies.** If verification costs more than the behavior it catches, the verification is wrong.

Routine validation does not fetch live RustSec data. The explicit,
fail-closed `./scripts/check_security.sh` command is required before package
publication and is also required by the manual PyPI workflow when
`publish=true`; it refreshes advisory data and reports the scan time and tool
versions. This is release-time vulnerability intelligence, not an automatic
merge gate or a claim that vulnerabilities cannot exist.

## Complexity Budget

| Item | Limit |
|------|-------|
| Automatic workflows | 1 |
| Manual-dispatch workflows | 1 (PyPI wheels, release-only) |
| Required runner jobs per push/PR | 1 |
| Routine CI matrices | 0 |
| Routine CI artifact exchange | 0 |
| Automatic evidence schemas | 0 |
| Workflow meta-validation | 0 |
| Warm-cache target | < 10 minutes |
| Cold-cache target | < 20 minutes |

Any addition exceeding this budget requires a concrete regression history and explicit maintainer approval.

Qualification-only evidence is separate from automatic CI: the HTTP/3
program keeps versioned corpus, impairment, and result records under
`qualification/http3/` and `plans/`, but routine CI neither downloads
external servers nor consumes those records as a pass/fail gate. Missing or
unsupported manual evidence remains visible and cannot be converted into a
pass.

## Validation Tiers

### Tier 1: Routine Validation

```sh
./scripts/check.sh
```

Runs on every push and pull request via CI. Also run locally before committing. Contents:

1. Rust formatting check
2. Lint suppression policy check
3. Adapter feature ownership check (`check_adapter_features.py`)
4. Release version/ref validation (`test_validate_release_versions.py`)
5. Rust clippy
6. Rust workspace tests (excluding PyO3 crate)
7. Stable Rust public-contract fixtures for the supported profile set
8. Python extension build
9. Native Python API manifest and relational guardrails (`check_native_python_api.py`)
10. Python typing surface + fixture checks (`check_python_typing_surface.py`, `check_python_typing.py`)
11. Ordinary Python behavior tests
12. Compact HTTPX compatibility smoke kernel
13. Node binding prototype check: `cargo test -p eggfetch-node` always;
   the JS surface (`node test.js`) runs only when `node` and a built
   `eggfetch.node` artifact are present, otherwise an explicit skip is
   recorded (the prototype has no npm publication pipeline, so CI
   environments without a manually built artifact skip truthfully).

### Tier 2: Extended Validation

```sh
./scripts/check.sh extended
```

Runs Tier 1 first, then additional checks. The exact Rust 1.89.0 toolchain is a
required prerequisite for the MSRV gate; if it is unavailable, validation
fails with an installation command rather than recording a skip. All executed
checks are fail-closed. Includes
full HTTPX compatibility, Rust public API oracle and semver cross-check,
API manifest oracle (both facades),
feature combinations, docs, MSRV, resource
monitoring, FFI, lifecycle (timeout/proxy-TLS/shutdown), soak tests,
downstream compatibility (skipped without a prebuilt artifact manifest),
lossless merge tests,
and benchmarks.

### Tier 3: Package Validation

```sh
./scripts/check.sh package
```

Runs Tier 1 first, then fail-closed local packaging checks. Crate dry-runs, wheel build, wheel smoke test, and package-content validation all use fresh temporary artifacts. Must never publish. The worktree must be clean; `--allow-dirty` is not used.

## PyPI Wheel Release Workflow

The PyPI wheel workflow (`.github/workflows/pypi.yml`) is a manually dispatched, release-only pipeline. It is **not** triggered by pushes or pull requests and is **not** a branch-protection requirement.

### Wheel Matrix

| Operating system | Architecture | Python versions | Build method |
|---|---|---|---|
| Linux manylinux2014 | x86_64 | 3.10, 3.11, 3.12, 3.13, 3.14, 3.15 | maturin-action native |
| macOS | arm64 | 3.10, 3.11, 3.12, 3.13, 3.14, 3.15 | maturin-action native |
| Windows | x86_64 | 3.10, 3.11, 3.12, 3.13, 3.14, 3.15 | maturin-action native |

18 wheels + 1 sdist = 19 distributions per release.

### Workflow Modes

- **Build-only** (`publish=false`): builds all wheels and sdist, assembles the release set, skips publication.
- **Publish** (`publish=true`): validates that the checked-out commit is the
  exact matching `v<SEMVER>` tag, runs the live security preflight, and then
  publishes to PyPI via Trusted Publishing (OIDC). It requires the `pypi`
  GitHub environment and environment approval.

### Publication Security

- PyPI uses Trusted Publishing (OIDC), not a long-lived API token.
- Only the publish job receives `id-token: write`.
- The publish job requires the protected `pypi` GitHub environment with required reviewers.
- Publishing from branches or non-version tags is impossible.
- Existing PyPI versions are not silently skipped.

## Rules for New Automatic Checks

A new automatic check is permitted only when all of the following are true:

1. It catches a plausible product regression.
2. The regression cannot be covered by an existing test in the routine job.
3. The check is deterministic.
4. The check adds less than two minutes of expected runtime or replaces an equivalent-cost check.
5. It does not require artifact choreography or external services.
6. It does not duplicate another job.
7. Its ongoing maintenance cost is documented.

## Release Policy

- Release timing is a maintainer decision.
- crates.io publication is performed locally by a maintainer. GitHub Actions does not publish to crates.io.
- PyPI publication is performed via the manually dispatched PyPI wheel workflow with Trusted Publishing (OIDC).
- No automatic workflow publishes, tags, creates releases, or authorizes a candidate SHA.
- Packaging dry runs are local and distinct from publication.
