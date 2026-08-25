# Build & CI Deep Dive

This document covers the build system, CI pipeline, lint policy, MSRV, and release process.

See also: [overview.md](overview.md).

## Build Configuration

### rust-toolchain.toml

Pins to the stable Rust channel.

### rustfmt.toml

- `max_width = 100`
- Standard rustfmt defaults

### .clippy.toml

Pedantic clippy enabled workspace-wide.

### deny.toml

cargo-deny configuration for:
- Advisory database (security vulnerabilities)
- License compliance
- Dependency bans
- Source restrictions

## CI Pipeline

Two GitHub Actions workflows:

- **`ci.yml`** — routine push/PR validation. One Ubuntu job, no matrix, no artifact exchange. Calls `./scripts/check.sh`. This is the only automatic workflow.
- **`pypi.yml`** — manual-dispatch PyPI release pipeline. Builds 12 wheels across 3 platforms (linux-x86_64, macos-arm64, windows-x86_64) and 4 Python versions, builds and validates an sdist, assembles the release set, and optionally publishes via Trusted Publishing (OIDC).

See [verification-policy.md](../verification-policy.md) for the normative policy.

### Routine Validation (Tier 1)

| Step | Command |
|------|---------|
| Rust formatting | `cargo fmt --all -- --check` |
| Lint suppression | `bash scripts/check_lint_suppressions.sh` |
| Rust clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| Rust tests | `cargo test --workspace --exclude eggfetch-python --all-features` |
| Python build | `maturin develop -m crates/eggfetch-python/Cargo.toml` |
| Python tests | `pytest crates/eggfetch-python/tests/ -q --ignore=.../compat` |
| HTTPX compat smoke | `pytest .../test_imports.py .../test_client.py .../test_exceptions.py .../test_corrective_kernel.py` |

### Extended Validation (Tier 2)

Run `./scripts/check.sh extended` for: full HTTPX compatibility, API manifest comparison, feature matrix, feature-gated tests, MSRV, docs, FFI, resource monitoring, lifecycle, soak, downstream, merge, and benchmarks. All executed checks are fail-closed. The only permitted skip is MSRV when the Rust 1.80 toolchain is not installed.

### Package Validation (Tier 3)

Run `./scripts/check.sh package` for: core publish dry-run (`cargo publish --dry-run -p eggfetch-core`), dependent-crate package-structure validation (`cargo package --list` plus structured internal dependency version verification via cargo metadata for eggfetch-cli, eggfetch-ffi, eggfetch-python, eggfetch-node), wheel build, exactly-one-wheel resolution, wheel smoke, and package content validation. Uses fresh temporary artifacts; stale repository wheels are never used. The worktree must be clean.

### PyPI Wheel Pipeline

Run manually via `workflow_dispatch` from `.github/workflows/pypi.yml`. The pipeline:

1. **validate-release** — version coherence, internal dependency topology, routine + package validation
2. **build-wheel** — 12 wheel jobs across 3 platforms × 4 Python versions
3. **build-sdist** — source distribution with isolated build test
4. **assemble** — downloads all artifacts, validates coverage matrix (12 wheels + 1 sdist), runs twine check
5. **publish** — optional OIDC upload to PyPI (requires `publish=true`, version tag, `pypi` environment approval)

## Environment

- `CARGO_TERM_COLOR=always`
- `RUSTFLAGS=-D warnings` — warnings are errors
- `PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1`

## Lint Policy

- Pedantic clippy workspace-wide.
- `unsafe_code = "forbid"` (except FFI/Node).
- `missing_docs = "warn"`, `missing-docs-in-crate-items = true`.
- Never use `#![allow(warnings)]`, `#![allow(clippy::all)]`, or `#![allow(clippy::pedantic)]`.
- CI rejects blanket suppressions via `scripts/check_lint_suppressions.sh`.
- Use specific lint names. Justify suppressions with a comment.

## MSRV

**Rust 1.80** — checked in extended validation via `cargo check` with the 1.80 toolchain.

## Release Process

Release timing and publication are maintainer decisions. See `docs/releases/process.md`.

### Publishing Order

1. `eggfetch-core`
2. `eggfetch-cli`
3. `eggfetch-ffi`
4. `eggfetch-python`
5. `eggfetch-node`

crates.io index propagation requires verification between publishes. Bench and fuzz crates are not published.
