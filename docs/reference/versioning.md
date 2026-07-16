# Versioning Policy

eggfetch follows a coordinated versioning strategy across all publishable crates.
This document describes how versions are assigned, what constitutes a breaking
change, and what stability expectations users can rely on.

## Coordinated versioning

All publishable crates in the workspace share a single version number. When
eggfetch-core is at `0.3.0`, the CLI, Python bindings, FFI bindings, and Node
bindings are also at `0.3.0`. This eliminates version-matrix confusion for
consumers who depend on multiple crates from the project.

The bench and fuzz crates are internal and not published. Their versions are
not coordinated with the public crates.

## Semantic versioning

eggfetch adheres to [Semantic Versioning 2.0.0](https://semver.org/). Given a
version number `MAJOR.MINOR.PATCH`:

- **MAJOR** — incompatible API changes
- **MINOR** — new functionality in a backward-compatible manner
- **PATCH** — backward-compatible bug fixes

### Pre-1.0 stability expectations

While the project is below version 1.0.0, the following relaxed rules apply:

- **Minor versions** may contain breaking changes. A minor bump (e.g.,
  `0.1.0` → `0.2.0`) signals that the API has changed in ways that may
  require consumer updates.
- **Patch versions** are always backward-compatible. A patch bump (e.g.,
  `0.1.0` → `0.1.1`) contains only bug fixes and internal improvements.

Once the project reaches 1.0.0, full semantic versioning guarantees apply:
MAJOR bumps for breaking changes, MINOR for backward-compatible additions,
PATCH for fixes.

## Breaking changes by surface

The following describes what constitutes a breaking change for each public
surface. A breaking change requires a MINOR version bump (pre-1.0) or MAJOR
version bump (post-1.0).

### Rust API

- Public function or method signature changes (parameter types, return types,
  generic constraints)
- New required trait implementations for public traits
- Error variant removal or reordering
- Module or type reorganization that breaks import paths
- Feature-flag removal that changes the default API surface

### Python API

- Method signature changes (parameter names, types, defaults)
- Exception hierarchy changes (new required catch clauses, removed exceptions)
- Return type changes (e.g., `bytes` → `str`)
- Removed or renamed public attributes
- Changes to `__init__` constructor parameters

### CLI

- Flag removal or rename
- Output format changes (JSON schema, field names, nesting)
- Exit code reassignment
- Subcommand removal or rename
- Positional argument changes

### Error types

- New error variants are additive and non-breaking
- Removal of existing error variants is breaking
- Error variant field additions are non-breaking unless they change exhaustiveness assumptions

### Machine-readable output

- JSON or NDJSON schema changes (new required fields, type changes, field
  removal) require a minor version bump
- Additive schema changes (new optional fields) are non-breaking

## Version format

Versions follow the format `MAJOR.MINOR.PATCH` as specified by Semantic
Versioning. Pre-release identifiers (e.g., `0.1.0-alpha.1`) may be used for
preview releases but are not published to crates.io or PyPI.

## Tag format

Git tags use the format `vMAJOR.MINOR.PATCH` (e.g., `v0.1.0`, `v0.2.1`).
Release artifacts and CI workflows are triggered by tags matching this pattern.

## MSRV policy

The minimum supported Rust version (MSRV) is documented in
`workspace.package.rust-version` and enforced by `rust-toolchain.toml`. MSRV
changes are treated as breaking changes and require a minor version bump.

## Feature-flag stability

Adding a new feature flag is non-breaking (consumers are unaffected unless they
opt in). Removing or renaming a feature flag is breaking. Changing the set of
default features is breaking if it alters the API surface for consumers who do
not explicitly set `default-features = false`.

## Documentation versioning

eggfetch documentation is versioned alongside the code. Every release tag
(`v0.1.0`, `v0.2.0`, etc.) includes the documentation that matches that
release.

### Development docs

The `main` branch contains **development documentation**. It reflects the
current state of the codebase and may describe features that are unreleased
or subject to change. Published development docs should carry a banner:

> This documents the **main** development branch. Released versions may
> differ. See the version selector or release tags for stable documentation.

### Release docs

When a new version is tagged:

1. Documentation is frozen to match the release.
2. The release tag includes the `docs/` directory contents.
3. If a docs site is published, the release version is added to the version
   selector.

### Versioned API references

- **Rust**: `cargo doc` output is tied to the crate version in `Cargo.toml`.
  Published to [docs.rs](https://docs.rs/eggfetch-core) on release.
- **Python**: Docstrings are in the source and rendered by the chosen doc
  tool (pdoc3, mkdocstrings, etc.) at build time.
- **CLI**: `--help` output is generated from clap metadata and reflects the
  binary version.

### Drift prevention

CI runs these checks to prevent documentation drift:

| Check | What it validates |
|-------|-------------------|
| `cargo doc --no-deps` | Rust doc links and examples compile |
| `cargo test --doc` | Rust doc tests pass |
| `check_doc_examples.py` | Python code blocks in docs are syntactically valid |
| `check_doc_links.py` | Internal markdown links resolve |
| `check_api_surface.py` | Public Python exports match documented API |

If a public API item is added or removed, update the corresponding docs and
the CI checks will catch stale references.
