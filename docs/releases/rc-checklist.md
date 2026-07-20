# Release Candidate Gate Checklist

## Purpose

This checklist defines the mandatory evidence required before tagging `0.1.0-rc1`. Every item must be verified against one immutable commit SHA.

## Candidate SHA

**SHA:** ________________
**Date:** ________________
**Branch:** ________________

## Code Quality

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes (excluding eggfetch-python)
- [ ] `bash scripts/check_lint_suppressions.sh` passes (no forbidden broad suppressions)
- [ ] `cargo test --workspace --exclude eggfetch-python --all-features` passes
- [ ] `cargo test -p eggfetch-cli --test integration -- --test-threads=1` passes (all 56 tests)
- [ ] `cargo test -p eggfetch-ffi --all-features` passes (30 tests)
- [ ] `cargo test -p eggfetch-core --all-features` passes (753+ tests)
- [ ] Python tests pass across 3.10–3.13 on Ubuntu, macOS, Windows
- [ ] `cargo check -p eggfetch-core --no-default-features` passes
- [ ] `cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls` passes
- [ ] `cargo check -p eggfetch-core --all-features` passes
- [ ] Feature-gate tests pass for compression, proxy, HTTP/3

## Packages and Artifacts

- [ ] `cargo publish -p eggfetch-core --dry-run` succeeds
- [ ] `cargo package --list` contains only intended release files for all publishable crates
- [ ] No fuzz corpora, local plans, benchmark output, or secrets in packages
- [ ] All path dependencies have version fields for publication
- [ ] Python wheels build for all declared platforms (linux-x86_64, linux-aarch64, macos-x86_64, macos-aarch64, win-amd64)
- [ ] `twine check` passes for all Python artifacts
- [ ] Sdist installs and imports successfully in a clean environment
- [ ] CLI binaries build for all declared targets
- [ ] CLI binary runs `eggfetch --version` from extracted archive
- [ ] CLI binary runs `eggfetch --help` from extracted archive
- [ ] CLI local-server smoke tests pass (GET, JSON, exit codes)
- [ ] Release manifest (`release-manifest.json`) generated with per-artifact metadata

## Supply Chain

- [ ] SHA-256 checksums generated for every CLI archive
- [ ] SBOM generated via cargo-cyclonedx
- [ ] Release manifest references exact source commit SHA
- [ ] Provenance attestations generated for CLI binaries and SBOM
- [ ] `cargo-deny` passes (advisory, license, ban, source checks)

## Semantics and Compatibility

- [ ] 51 differential tests pass against pinned requests/HTTPX
- [ ] All tests use local deterministic servers (no public internet)
- [ ] Known divergences documented in `docs/reference/compatibility.md`
- [ ] Streaming, retries, redirects, TLS, compression, multipart, proxy, cookies, auth have passing coverage
- [ ] HTTP/3 remains explicitly experimental and non-default

## Resource Regression

- [ ] Resource monitor runs successfully and reports `"passed": true`
- [ ] Thresholds: max delta RSS 50 MB, max peak RSS 100 MB
- [ ] Buffered download RSS delta is bounded
- [ ] Streaming download RSS delta is bounded
- [ ] Connection reuse shows no monotonic RSS growth
- [ ] Cancelled requests show no unbounded RSS growth
- [ ] Concurrent streaming RSS is bounded

## Documentation

- [ ] `cargo test --doc -p eggfetch-core --all-features` passes (doctests)
- [ ] Python doc examples execute against installed package
- [ ] Internal link validation passes
- [ ] CLI help matches checked-in reference
- [ ] `docs/reference/compatibility.md` matches tested behavior
- [ ] `docs/releases/process.md` reflects current workflow

## CI Matrix

- [ ] All CI jobs pass on the candidate SHA
- [ ] No required job is skipped unexpectedly
- [ ] CI matrix summary artifact (`ci-matrix-summary.json`) uploaded
- [ ] Release summary artifact uploaded

## Release Workflow

- [ ] Dry-run workflow_dispatch completes successfully
- [ ] All build jobs succeed (wheels, CLI binaries, SBOM, package validation)
- [ ] Smoke tests pass (wheel install, CLI binary execution, twine check, sdist install)
- [ ] No registry credentials required for dry-run (publish jobs skipped)
- [ ] Release summary job reports all job statuses
- [ ] Release manifest generated with all artifacts and checksums

## CI Enforcement

- [ ] `Required CI Gate` job exists in CI workflow with `if: always()`
- [ ] Gate uses `scripts/evaluate_ci_gate.py` for deterministic evaluation
- [ ] Gate fails when any required job fails, is cancelled, is missing, or is unexpectedly skipped
- [ ] Gate is the single required status check for branch protection on `main`
- [ ] Evaluator tests pass: `python -m pytest scripts/test_evaluate_ci_gate.py -v`

## Immutable Candidate

- [ ] `candidate_sha` input provided with full 40-character SHA
- [ ] Workflow validates SHA format, commit existence, and HEAD match
- [ ] All build jobs check out the same candidate SHA
- [ ] Immutable validation tag created (e.g., `rc-dry-run-<short-sha>`)
- [ ] Evidence manifest references the exact candidate SHA

## Dry-Run Safety

- [ ] `dry_run=true` is the safe default for manual RC validation
- [ ] Registry publication, GitHub Release, tag creation, and production environment deployment are technically gated off
- [ ] `verify-no-side-effects` job confirms no publishing or repository mutations occurred
- [ ] Post-run verification confirms no crates.io/PyPI/npm publications exist

## Accepted Limitations

- [ ] HTTP/3 is experimental and non-default
- [ ] eggfetch-node is experimental (N-API bindings)
- [ ] CLI integration tests may be flaky under parallel execution (pass single-threaded)
- [ ] eggfetch-python excluded from Rust workspace test runs (requires pyo3-ffi build deps)

## Sign-off

| Role | Name | Date | SHA Verified |
|------|------|------|--------------|
| Release engineer | | | |
| Security reviewer | | | |

**Decision:** PROCEED / BLOCK

**Notes:**
