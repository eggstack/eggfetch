# Release Process

## Versioning Strategy

All publishable crates in the workspace share a single coordinated version number. When a release is tagged, `eggfetch-core`, `eggfetch-cli`, `eggfetch-python`, `eggfetch-ffi`, and `eggfetch-node` all receive the same version. Internal crates that are not published (benchmarks, fuzz targets) are excluded.

Version numbers follow [Semantic Versioning](https://semver.org/):

- **MAJOR** -- incompatible public API changes (Rust trait bounds, Python exception hierarchy, CLI flag removal)
- **MINOR** -- new functionality that is backward-compatible (new feature flags, new optional methods, new CLI flags)
- **PATCH** -- backward-compatible bug fixes

## Pre-1.0 Stability Expectations

Until the 1.0 release, the project operates under pre-1.0 conventions:

- Minor versions may contain breaking changes. Breaking changes are announced in the changelog with migration guidance.
- Patch versions never contain intentional breaking changes.
- The API surface may grow rapidly. Deprecation warnings are issued before removal, but the notice period is shorter than post-1.0 policy.

## What Constitutes a Breaking Change

A change is breaking if any of the following are true:

- A public Rust type, trait, method, or associated function has a changed signature or removed item.
- A Python exception class is renamed, removed, or has its inheritance hierarchy changed.
- A CLI flag is removed or its positional semantics change.
- An error kind is removed or reclassified (e.g., a condition that previously returned `Error::Network` now returns `Error::Timeout`).
- A feature flag is renamed or its default behavior changes.
- The wire format of a machine-output mode (JSON, NDJSON) gains or loses required fields.
- The FFI function signature changes or an exported type layout changes.

Changes that expand the API surface (adding new optional parameters, new enum variants, new CLI flags) are not breaking.

## Release Candidate Process

For substantial releases (new major feature, pre-1.0 breaking changes), a release candidate cycle is recommended:

1. Tag a release candidate (e.g., `v0.2.0-rc.1`).
2. Publish to crates.io and PyPI as pre-release artifacts.
3. Run the full CI matrix, including wheel smoke tests on all supported platforms.
4. Announce the RC to early adopters and collect feedback.
5. Fix issues and cut subsequent RCs as needed (rc.2, rc.3, ...).
6. When stable, promote the final RC to the release version.

Release candidates are optional for patch releases.

## Release Rehearsal

Before the first production release, perform at least one end-to-end release rehearsal on a pre-release tag. This validates the entire pipeline without affecting real users.

Rehearsal steps:

1. Create a release candidate tag (e.g., `v0.1.0-rc.1`) on a clean commit.
2. Push the tag to trigger the release workflow.
3. Verify every job succeeds: CI, wheel builds, CLI builds, smoke tests, crates.io publish, TestPyPI publish, PyPI publish, GitHub Release creation.
4. Install from TestPyPI in a clean environment and run a smoke test.
5. Install from crates.io in a clean environment and run `eggfetch --version`.
6. Verify the GitHub Release contains all expected artifacts (CLI archives, checksums, SBOM).
7. Verify provenance attestations are present on the release artifacts.
8. If any step fails, fix the workflow, tag a new RC, and repeat.

Record the rehearsal result (pass/fail, issues found) before proceeding to the stable release.

## Step-by-Step Release Checklist

1. **Update version numbers.** Set the version in all publishable `Cargo.toml` files and `pyproject.toml`. Ensure they are consistent.

2. **Update CHANGELOG.md.** Move items from `[Unreleased]` into the new version section. Set the release date.

3. **Run the full validation pass.**

   ```sh
   cargo fmt --all --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace --all-features
   cargo check -p eggfetch-core --no-default-features
   cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
   cargo check -p eggfetch-core --all-features
   cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip
   cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-brotli
   cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-zstd
   cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
   cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
   cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
   ```

4. **Run Python validation.**

   ```sh
   cd crates/eggfetch-python
   maturin develop
   python -m pytest -p pytest_asyncio
   maturin build
   ```

5. **Commit and tag.** Create a signed tag `v<VERSION>` on the commit.

6. **Dry-run workflow_dispatch (optional).** Before pushing the tag, you can trigger a dry-run via GitHub Actions workflow_dispatch with `dry_run: true`. This runs the full CI matrix, builds all artifacts, and runs the `--dry-run` cargo publish check without actually publishing to any registry. It is a safe way to validate the entire release pipeline end-to-end.

7. **Push and wait for CI.** The CI pipeline builds and tests on Ubuntu, macOS, and Windows across Python 3.10-3.13.

7. **Build release artifacts.** Produce platform-specific wheels and the CLI binary.

8. **Smoke test artifacts.** Install the wheel into a clean virtual environment. Run a local GET and a streaming response test.

9. **Publish to registries.** Publish crates to crates.io and wheels to PyPI.

10. **Post-publish validation.** Install from the published artifacts (not local). Run a quick smoke test to confirm the published version works.

11. **Create a GitHub Release.** Attach artifacts and link the changelog entry.

## Rollback and Yanking Policy

If a published version has a critical defect:

- **Yank from crates.io.** Run `cargo yank --version <VERSION>` on the affected crates. Yanked versions remain installable by exact version but are excluded from normal resolution.
- **Yank from PyPI.** Use the PyPI admin interface to mark the release as deprecated. Upload a corrected version as a post-release (e.g., `0.1.1`).
- **Announce the yank.** Create a GitHub Release note or issue explaining the defect and the fix.

## Recovery if One Registry Succeeds and Another Fails

If publishing succeeds on crates.io but fails on PyPI (or vice versa):

1. Do not delete the successful publication. Crates.io and PyPI do not support true deletion of published versions.
2. Fix the failing registry's issues.
3. Publish the corrected version to the failing registry with a patch bump or post-release suffix.
4. If the crates.io publication is the one that failed, yank the partially published version and re-release with a patch bump.
5. Document the incident in the changelog under a `[Fixed]` entry for the corrected version.
