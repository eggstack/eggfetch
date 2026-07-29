# Verification Policy

This document is the normative statement of CI, verification, and release policy for eggfetch.

## Principles

1. **CI is a fast regression safety net.** It catches regressions introduced by a change. It is not a release authority.
2. **CI does not determine release cadence.** Releases are maintainer decisions performed from a trusted local environment.
3. **Routine CI does not publish packages or create releases.** GitHub Actions holds no publication credentials and performs no publication steps in the push/PR workflow.
4. **Local validation is canonical.** The checked-in `./scripts/check.sh` is the single source of validation. CI repeats the same command on Ubuntu.
5. **Extended checks are opt-in.** Slower or less frequently useful checks run via `./scripts/check.sh extended` and are not triggered by every push or pull request.
6. **Packaging checks are local validation.** `./scripts/check.sh package` validates packaging without publication. `eggfetch-core` receives a full `cargo publish --dry-run`. Dependent crates receive package-structure validation via `cargo package --list` and manifest version verification; full `cargo publish --dry-run` runs at publication time.
7. **crates.io publication is manual.** A maintainer publishes from a trusted local environment. GitHub Actions does not publish.
8. **PyPI publication is manually dispatched.** The PyPI wheel workflow (`.github/workflows/pypi.yml`) is triggered only by `workflow_dispatch`. It is not a merge gate and does not run on push or pull request.
9. **Historical qualification plans are non-normative.** Completed plans are records of past work, not active CI or release requirements.
10. **Verification infrastructure must remain materially simpler than the behavior it verifies.** If verification costs more than the behavior it catches, the verification is wrong.

## Complexity Budget

| Item | Limit |
|------|-------|
| Automatic workflows | 1 |
| Manual-dispatch workflows | 1 (PyPI wheels, release-only) |
| Required runner jobs per push/PR | 1 |
| Routine CI matrices | 0 |
| Routine CI artifact exchange | 0 |
| Evidence schemas | 0 |
| Workflow meta-validation | 0 |
| Warm-cache target | < 10 minutes |
| Cold-cache target | < 20 minutes |

Any addition exceeding this budget requires a concrete regression history and explicit maintainer approval.

## Validation Tiers

### Tier 1: Routine Validation

```sh
./scripts/check.sh
```

Runs on every push and pull request via CI. Also run locally before committing. Contents:

1. Rust formatting check
2. Lint suppression policy check
3. Rust clippy
4. Rust workspace tests (excluding PyO3 crate)
5. Python extension build
6. Ordinary Python behavior tests
7. Compact HTTPX compatibility smoke kernel

### Tier 2: Extended Validation

```sh
./scripts/check.sh extended
```

Runs Tier 1 first, then additional checks. May include an explicit skip when an optional prerequisite (e.g., Rust 1.80 toolchain for MSRV) is unavailable. All executed checks are fail-closed. Includes full HTTPX compatibility, feature combinations, docs, MSRV, resource monitoring, FFI, soak tests, downstream compatibility, lossless merge tests, and benchmarks.

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
| Linux manylinux2014 | x86_64 | 3.10, 3.11, 3.12, 3.13 | maturin-action native |
| Linux manylinux2014 | aarch64 | 3.10, 3.11, 3.12, 3.13 | maturin-action cross-build |
| macOS | x86_64 | 3.10, 3.11, 3.12, 3.13 | maturin-action native |
| macOS | arm64 | 3.10, 3.11, 3.12, 3.13 | maturin-action native |
| Windows | x86_64 | 3.10, 3.11, 3.12, 3.13 | maturin-action native |

20 wheels + 1 sdist = 21 distributions per release.

### Workflow Modes

- **Build-only** (`publish=false`): builds all wheels and sdist, assembles the release set, skips publication.
- **Publish** (`publish=true`): additionally publishes to PyPI via Trusted Publishing (OIDC). Requires a `v<SEMVER>` tag, the `pypi` GitHub environment, and environment approval.

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
