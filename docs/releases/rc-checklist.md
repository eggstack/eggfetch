# Release Checklist (Manual)

Optional maintainer checklist for release preparation. This is not a required gate — it is a convenience reference. The normative procedure is `process.md`.

## Pre-Publication

- [ ] Version numbers updated in all publishable `Cargo.toml` and `pyproject.toml`
- [ ] CHANGELOG.md updated with release section
- [ ] Worktree is clean
- [ ] `./scripts/check.sh` passes
- [ ] `./scripts/check.sh extended` passes (before release)
- [ ] `./scripts/check.sh package` passes
- [ ] Package contents reviewed

## crates.io Publication

- [ ] `cargo publish -p eggfetch-http-connect` succeeds
- [ ] Verify propagation: `cargo search eggfetch-http-connect`
- [ ] `cargo publish -p eggfetch-core` succeeds
- [ ] Verify propagation: `cargo search eggfetch-core`
- [ ] `cargo publish -p eggfetch-cli` succeeds
- [ ] `cargo publish -p eggfetch-ffi` succeeds
- [ ] `cargo publish -p eggfetch-python` succeeds
- [ ] `cargo publish -p eggfetch-node` succeeds

## Post-Publication

- [ ] Signed version tag created and pushed
- [ ] Required coordinated GitHub Release created from the existing signed tag after PyPI verification
- [ ] PyPI publication (optional, manual)
