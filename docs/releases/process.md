# Release Process

Release timing and publication are maintainer decisions performed from a trusted local environment. GitHub Actions does not publish packages, create releases, or authorize releases.

## Versioning Strategy

All publishable crates share a single coordinated version number:

- `eggfetch-core`
- `eggfetch-cli`
- `eggfetch-ffi`
- `eggfetch-python`
- `eggfetch-node`

Version numbers follow [Semantic Versioning](https://semver.org/). Until 1.0, minor versions may contain breaking changes (announced in changelog).

## Pre-Publication Steps

1. **Select the version deliberately.**
2. **Update every coordinated publishable crate version** in `Cargo.toml` and `pyproject.toml`.
3. **Update CHANGELOG.md.** Move items from `[Unreleased]` into the new version section.
4. **Ensure the worktree is clean.**
5. **Run routine validation:**

   ```sh
   ./scripts/check.sh
   ```

6. **Run package validation:**

   ```sh
   ./scripts/check.sh package
   ```

7. **Review package contents** for changed packaging surfaces.
8. **Confirm credentials exist only in your local Cargo configuration or temporary environment.**

## crates.io Publication

Publish manually in dependency order. Before publishing a dependent crate, verify the preceding crate/version is visible to crates.io resolution. Do not encode fixed sleeps as policy — inspect actual registry availability.

```sh
cargo publish -p eggfetch-core
# Wait for crates.io index to propagate, verify with:
# cargo search eggfetch-core

cargo publish -p eggfetch-cli
cargo publish -p eggfetch-ffi
cargo publish -p eggfetch-python
cargo publish -p eggfetch-node
```

**Important:** crates.io versions are immutable. If a published version is incorrect or publication is partial, correct the defect, bump the version, and publish a new version. Do not attempt to overwrite an existing version.

## Tagging and GitHub Releases

Tagging is manual and separate from publication. After successful crates.io publication:

```sh
git tag -s v<VERSION> -m "Release v<VERSION>"
git push origin v<VERSION>
```

A GitHub Release is optional and manual. It must not be described as an automated or required output.

## PyPI (Optional, Manual)

If PyPI publication is desired, build and publish separately from crates.io:

```sh
cd crates/eggfetch-python
maturin build --release
twine upload dist/*
```

A successful publication to one registry must not be deleted because another channel failed. Correct and issue a new version according to each registry's immutability rules.

## Rollback

If a published version has a critical defect:

- **crates.io:** `cargo yank --version <VERSION>` on affected crates. Yanked versions remain installable by exact version but are excluded from normal resolution. Publish a corrected version with a patch bump.
- **PyPI:** Use the PyPI admin interface to mark the release as deprecated. Upload a corrected version.

## What This Process Does NOT Require

- A GitHub Actions dry run
- A candidate SHA input
- An immutable validation tag
- A green qualification workflow
- An evidence manifest
- Candidate identity
- A release manifest
- A CI matrix summary
- An SBOM as a publication gate
- Provenance attestations
- Automated post-publication sleeps
- Release environment approval in GitHub

These may be performed manually when useful, but they are not part of the required release contract.
