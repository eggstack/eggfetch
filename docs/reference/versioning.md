# Documentation versioning

eggfetch documentation is versioned alongside the code. Every release tag
(`v0.1.0`, `v0.2.0`, etc.) includes the documentation that matches that
release.

## Development docs

The `main` branch contains **development documentation**. It reflects the
current state of the codebase and may describe features that are unreleased
or subject to change. Published development docs should carry a banner:

> This documents the **main** development branch. Released versions may
> differ. See the version selector or release tags for stable documentation.

## Release docs

When a new version is tagged:

1. Documentation is frozen to match the release.
2. The release tag includes the `docs/` directory contents.
3. If a docs site is published, the release version is added to the version
   selector.

## Versioned API references

- **Rust**: `cargo doc` output is tied to the crate version in `Cargo.toml`.
  Published to [docs.rs](https://docs.rs/eggfetch-core) on release.
- **Python**: Docstrings are in the source and rendered by the chosen doc
  tool (pdoc3, mkdocstrings, etc.) at build time.
- **CLI**: `--help` output is generated from clap metadata and reflects the
  binary version.

## Drift prevention

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
