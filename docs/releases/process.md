# Release Process

Release timing and publication are maintainer decisions performed from a trusted local environment. crates.io publication is manual. PyPI publication is dispatched manually via GitHub Actions.

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

   Package validation is fail-closed and requires a clean worktree. `eggfetch-core` receives a full `cargo publish --dry-run` since it has no internal dependencies. Dependent crates (`eggfetch-cli`, `eggfetch-ffi`, `eggfetch-python`, `eggfetch-node`) receive local package-structure validation via `cargo package --list` and manifest version verification, because their internal dependencies are not yet on crates.io. Full `cargo publish --dry-run -p <crate>` runs at publication time, after each crate's dependencies are visible in the registry.

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

## PyPI Publication

PyPI publication is performed via the manually dispatched `.github/workflows/pypi.yml` workflow.

### Supported Wheel Matrix

| Operating system | Architecture | Python versions |
|---|---|---|
| Linux manylinux2014 | x86_64 | 3.10, 3.11, 3.12, 3.13 |
| macOS | arm64 | 3.10, 3.11, 3.12, 3.13 |
| Windows | x86_64 | 3.10, 3.11, 3.12, 3.13 |

12 wheels + 1 source distribution = 13 distributions per release.

### Dispatch Procedure

1. Create and push a signed `v<VERSION>` tag after crates.io publication:

   ```sh
   git tag -s v<VERSION> -m "Release v<VERSION>"
   git push origin v<VERSION>
   ```

2. Go to Actions → PyPI Wheels → Run workflow.
3. Select the `v<VERSION>` tag from the branch/tag dropdown.
4. **First run (rehearsal):** set `publish=false`. This builds all wheels and the sdist without uploading. Inspect the assembled artifacts.
5. **Second run (publication):** select the same `v<VERSION>` tag, set `publish=true`. Approve the `pypi` environment deployment when prompted.
6. Verify the PyPI release at `https://pypi.org/project/eggfetch/<VERSION>/`.

### Trusted Publishing Configuration

- **PyPI project:** `eggfetch`
- **Owner:** `eggstack`
- **Repository:** `eggfetch`
- **Workflow filename:** `pypi.yml`
- **Environment:** `pypi`

No PyPI API token is required. The workflow uses OIDC (OpenID Connect) for authentication via GitHub's `id-token: write` permission, granted only to the publish job.

### Immutability

PyPI versions are immutable. If a version already exists on PyPI, the publish job fails. Correct the defect, bump the version, and publish a new version.

## Tagging and GitHub Releases

Tagging is manual and separate from publication. After successful crates.io publication:

```sh
git tag -s v<VERSION> -m "Release v<VERSION>"
git push origin v<VERSION>
```

A GitHub Release is optional and manual. It must not be described as an automated or required output.

## Full Release Sequence

1. Bump all coordinated versions in Cargo.toml and pyproject.toml.
2. Update CHANGELOG.md.
3. Run `./scripts/check.sh` and `./scripts/check.sh package`.
4. Manually publish crates.io packages in dependency order.
5. Verify crates.io propagation between publishes.
6. Create and push signed `v<VERSION>` tag.
7. Dispatch PyPI Wheels from the tag with `publish=false` (optional rehearsal).
8. Inspect assembled artifacts.
9. Dispatch from the same tag with `publish=true`.
10. Approve the `pypi` environment deployment.
11. Verify PyPI release and installation on representative platforms.

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
