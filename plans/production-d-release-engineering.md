# Production Track D Plan: Release Engineering

## Objective

Establish reproducible, secure, and maintainable release processes for `eggfetch-core`, the Python package, and the CLI. Releases should be automated but require explicit approval, produce verifiable artifacts, and follow documented compatibility and deprecation policies.

## Scope

Implement:

- coordinated versioning strategy
- crates.io publishing policy
- PyPI wheel/sdist publishing
- multi-platform CLI binaries
- changelog and release notes
- MSRV and Python-version policy
- semantic-versioning and deprecation policy
- signed checksums/provenance where practical
- release candidate and rollback process
- post-release smoke tests

## Versioning

Decide whether workspace crates share one version. Recommended initially: coordinated versions for core, Python bindings, and CLI to reduce compatibility ambiguity.

Document pre-1.0 stability expectations and what constitutes breaking changes for Rust, Python, CLI, error kinds, and machine-readable output.

## Artifact matrix

Python wheels for declared platforms/versions, including Linux manylinux/musllinux decisions, macOS architectures, and Windows x86_64. Build sdists only if source builds are supported/documented.

CLI artifacts should include supported OS/architectures, compressed archives, checksums, and install instructions.

Rust crates must package only required files and pass `cargo package` verification.

## Automation

Create release workflows triggered by signed/version tags or manual dispatch with environment approval.

Stages:

1. verify tag/version consistency
2. run full CI/security/test matrix
3. build artifacts in clean runners
4. smoke-test artifacts
5. generate checksums/SBOM/provenance
6. publish to staging/TestPyPI where useful
7. require approval
8. publish crates/PyPI/GitHub release
9. run post-publish install/request smoke tests

Prevent partial releases where possible. Document recovery if one registry succeeds and another fails.

## Reproducibility and provenance

Pin toolchain and build tooling for release jobs. Record compiler, maturin, Python ABI, dependency lock, and source commit. Use GitHub artifact attestations/SLSA-style provenance and sign release tags/checksums where practical.

## Changelog

Adopt a maintained changelog or generated release-note workflow with categories:

- added
- changed
- fixed
- security
- deprecated
- removed

Every public API change should be documented before release.

## Compatibility policy

Document:

- Rust MSRV and update cadence
- Python supported versions and deprecation notice period
- OS/architecture support tiers
- CLI machine-output schema versioning
- error-kind stability
- feature-flag compatibility

## Deprecation

Provide warnings and at least one documented migration path before removals when feasible. Python deprecations should use standard warnings; Rust uses `#[deprecated]`; CLI flags should retain aliases for a defined window.

## Release candidates

Use prereleases for substantial protocol/API changes. Test RC artifacts against example projects and internal eggstack consumers before stable promotion.

## Post-release validation

Install from public registries into clean environments and run local-server tests for:

- Python sync/async
- TLS
- streaming
- multipart/compression/proxy
- CLI

Verify package metadata, docs links, and artifact checksums.

## Rollback and yanking

Document when to yank crates, delete/replace GitHub assets, or publish corrective Python releases. Never silently replace immutable published artifacts.

## Acceptance criteria

- versioning and compatibility policies are documented
- release workflow builds and smoke-tests all artifacts
- publishing requires approval and uses trusted environments/secrets
- provenance/checksums accompany releases
- public-registry post-release tests pass
- changelog, deprecation, rollback, and security-release processes are operational
