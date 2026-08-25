# Compatibility Policy

## Rust MSRV Policy

The minimum supported Rust version (MSRV) is declared in `workspace.package.rust-version` (currently **1.80**) and checked in extended validation via a `cargo check` with the 1.80 toolchain; `rust-toolchain.toml` pins the stable channel for day-to-day development.

- The MSRV may be raised in minor releases. A MSRV bump is announced at least one minor release in advance.
- The MSRV is never raised in a patch release.
- The MSRV is chosen to balance access to language features with distribution packager compatibility.
- CI runs the full test suite against the declared MSRV to catch regressions.

## Python Supported Versions

The supported Python versions are **3.10 through 3.13**. The PyPI release workflow builds interpreter-specific wheels for all four versions on Linux, macOS, and Windows.

- New Python versions are supported as soon as their stable release is compatible with PyO3.
- Dropped Python versions receive a deprecation notice at least one minor release before removal.
- Each supported Python version receives a distinct native wheel (ABI3 is not currently configured).
- Python 3.9 and earlier are not supported and will not receive compatibility fixes.

## OS and Architecture Support Tiers

| Tier | Platforms | CI Coverage | PyPI Wheels |
|------|-----------|-------------|-------------|
| Tier 1 | Linux x86_64, macOS arm64, Windows x86_64 | Wheel build + smoke test in the PyPI release workflow | Native wheels, smoke-tested |
| Tier 2 | macOS x86_64, Linux aarch64 | Not built or tested in CI | None (build from source) |
| Tier 3 | FreeBSD, other Unix variants | Community-supported, not tested | No wheels |

Routine CI runs on a single Ubuntu runner; platform breadth comes from the PyPI wheel matrix, not from routine CI.

## CLI Machine-Output Schema Versioning

The CLI supports `--json-output`, `--ndjson`, and `--base64` modes for machine consumption. Schema changes follow these rules:

- New fields may be added to JSON output without a version bump. Consumers must tolerate unknown fields.
- Existing fields are never renamed or removed in a minor or patch release.
- If a field's type changes (e.g., a string becomes a structured object), the old field is preserved and the new field is added with a distinct name.
- A `schema_version` field is included in structured output. It increments when the schema has breaking changes.

## Error-Kind Stability

Error kinds (Rust `Error` enum variants, Python exception classes, CLI exit codes) are part of the public API:

- New error kinds may be added in minor releases.
- Existing error kinds are never removed or reclassified in minor or patch releases.
- The mapping from error kind to CLI exit code is stable. A condition that produces exit code 4 today will always produce exit code 4.
- Error messages (the human-readable text) may change freely. Do not parse error messages.

## Feature-Flag Compatibility

Cargo feature flags control optional functionality:

- New feature flags may be added in minor releases. They are always opt-in (disabled by default unless documented otherwise).
- Existing feature flags are never removed in minor releases. Deprecated flags emit a compile-time deprecation warning for one minor release before removal.
- Feature flags that change default behavior require a major version bump.
- The `http3` feature is explicitly marked as experimental. Its API surface may change in minor releases until it stabilizes.

## Deprecation Policy

Deprecated items are removed gradually, not immediately:

- **Rust:** Deprecated items use `#[deprecated]` with a `since` version and a `note` explaining the replacement. Deprecated items remain compilable for at least one minor release after deprecation. Removal occurs in the next major version (or the next minor version before 1.0).
- **Python:** Deprecated functions and parameters emit `DeprecationWarning` with a message identifying the replacement. Warnings are issued for at least one minor release before removal.
- **CLI:** Deprecated flags are aliased to their replacement. The alias works identically to the original flag. Aliases are removed in the next major version. A deprecation warning is printed to stderr when a deprecated alias is used.
- **Documentation:** Deprecated items are annotated in the API reference with a deprecation banner and a link to the replacement.

Before removing a deprecated item:

1. Verify that the deprecation warning has been present for at least one minor release.
2. Search the issue tracker for user reports related to the deprecated item.
3. Add a migration note to the changelog.
4. Update all documentation references.
