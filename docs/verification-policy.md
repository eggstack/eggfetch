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

1. H3 qualification runner self-test (`test_h3_qualification.py`): a pure
   unit self-test of the runner's manifest and corpus validation. It opens no
   socket, resolves no name, needs no QUIC peer, and requires neither the
   `http3` feature nor any qualification artifact. It asserts the runner's
   logic only and makes **no H3 conformance claim**; H3 stays experimental.
2. Rust formatting check
3. Lint suppression guard self-test (`check_lint_suppressions.sh --self-test`):
   asserts the guard's verdict for every supported suppression form, so a
   broken guard fails the tier instead of passing silently
4. Lint suppression policy check (`check_lint_suppressions.sh`)
5. Adapter feature ownership check (`check_adapter_features.py`)
6. Release version/ref validation (`test_validate_release_versions.py`)
7. Rust clippy
8. Rust workspace tests (excluding PyO3 crate)
9. Stable Rust public-contract fixtures for the supported profile set
10. Python extension build
11. Native Python API manifest and relational guardrails (`check_native_python_api.py`)
12. Python typing surface + fixture checks (`check_python_typing_surface.py`, `check_python_typing.py`)
13. Ordinary Python behavior tests
14. Compact HTTPX compatibility smoke kernel
15. Node binding prototype check: `cargo test -p eggfetch-node` always;
   the JS surface (`node test.js`) runs only when `node` and a built
   `eggfetch.node` artifact are present, otherwise an explicit skip is
   recorded (the prototype has no npm publication pipeline, so CI
   environments without a manually built artifact skip truthfully).

### Lint-Suppression Policy Enforcement

`scripts/check_lint_suppressions.sh` scans every `*.rs` file under `crates/`
and `fuzz/`. A blanket lint is rejected when it appears **anywhere in the
lint list** of an `allow`, `deny` or `expect` attribute, whatever else the
attribute contains — including a `reason = "…"` field and additional lints.
The rejected lints are `warnings`, `clippy::all`, `clippy::pedantic`,
`clippy::nursery` and `clippy::restriction`. `expect` has the same effect as
`allow` and is treated identically; the blanket `deny` forms stay forbidden
because they escalate whole lint groups and evade the allow-based policy.

`crates/eggfetch-ffi/` and `crates/eggfetch-node/` remain exempt from the
pedantic rule (they override `pedantic` clippy by workspace policy); the
exemption covers `allow` and `expect` of `clippy::pedantic` only.

The guard's `--self-test` mode asserts the verdict for every supported form
in a temporary directory. It runs in Tier 1, modifies no working-tree file,
and takes well under a second.

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
feature combinations plus isolated feature-gated tests, docs, MSRV, resource
monitoring, FFI, lifecycle (timeout/proxy-TLS/shutdown), soak tests,
downstream compatibility (release-blocking; fails closed when the artifact
manifest is absent),
lossless merge tests,
and benchmarks.

#### The downstream compatibility gate fails closed

The `compat/downstream/` portfolio is release-blocking. Tier 2 does not
report success while skipping it: when
`target/downstream-qualification/artifact-manifest.json` is absent, the step
**fails** with an actionable message and `./scripts/check.sh extended` cannot
complete on a checkout that lacks the artifact. That is the intended
behavior for a release-blocking gate.

The operator obligation is to build the eggfetch wheel and the controlled
httpx replacement wheel, write their paths and SHA-256 hashes into that
manifest, and re-run the tier. The procedure is documented in
`compat/downstream/README.md` ("Update Process"); the consumer is
`scripts/run_downstream_compat.py`. **Nothing in this repository produces the
manifest automatically and no CI job does either** — the qualification
workflow that used to build it was removed, so the message names only the
artifact and the scripts that exist.

**The portfolio needs network access.** The isolated runner creates a virtual
environment, downloads the pinned downstream packages and their wheels from
PyPI, and verifies each package's source hash against
`compat/downstream/manifest.toml`. The package test suites themselves run
with outbound network disabled. The gate states this before it runs, rather
than leaving it to be discovered as a failure.

#### `-D warnings` scope in Tier 2

Warnings are errors in the all-features workspace checks, in clippy, and in
the all-features rows of the feature matrix and the MSRV gate. The
profile-isolated rows (`--no-default-features` and its feature combinations)
run with a bare `RUSTFLAGS=`, which overrides the CI-wide `-D warnings`
flag: the lean and native profiles retain private helpers for other
supported profiles, and the global flag turns that profile-specific dead
code into errors. The exemption is limited to those rows, so Tier 2's
outcome depends on the code rather than on the caller's environment.

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
- A coordinated public release includes a manually created GitHub Release tied to the already-pushed signed tag, after PyPI publication and initial registry verification. This is a maintainer release step, not an automatic workflow or CI gate.
- Packaging dry runs are local and distinct from publication.
