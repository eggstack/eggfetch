# Release Process Skill

Use this skill when preparing or executing a release of eggfetch.

## Workflow

1. Read `docs/releases/process.md` for the manual release procedure.
2. Read `docs/releases/compatibility-policy.md` for versioning rules.
3. Read `docs/architecture/release-security-checklist.md` and complete all items.

## Publishing Order

### crates.io (manual, local)

1. `eggfetch-core`
2. `eggfetch-cli`
3. `eggfetch-ffi`
4. `eggfetch-python`
5. `eggfetch-node`

crates.io index propagation requires verification between publishes. Do not encode fixed sleeps.

### PyPI (GitHub Actions, manual dispatch)

After crates.io publication and tag creation, dispatch `.github/workflows/pypi.yml`:

1. Select the `v<VERSION>` tag
2. Run with `publish=false` first (optional rehearsal)
3. Inspect assembled artifacts
4. Run with `publish=true` to publish
5. Approve the `pypi` environment deployment

PyPI uses Trusted Publishing (OIDC). No API token is needed.

## Pre-release Validation

```sh
./scripts/check.sh
./scripts/check.sh package
```

Both require a clean worktree.

## Publication

Publish locally from a trusted environment:

```sh
cargo publish -p eggfetch-core
cargo publish -p eggfetch-cli
cargo publish -p eggfetch-ffi
cargo publish -p eggfetch-python
cargo publish -p eggfetch-node
```

crates.io versions are immutable. If publication is partial, bump and republish.

## CI

- Routine CI (`.github/workflows/ci.yml`): runs `./scripts/check.sh` on pushes and pull requests. One Ubuntu job, no matrix.
- PyPI CI (`.github/workflows/pypi.yml`): manually dispatched, builds 12 wheels + 1 sdist (linux-x86_64, macos-arm64, windows-x86_64 × Python 3.10–3.13).

## Architecture References

- Release process: `docs/releases/process.md`
- Compatibility policy: `docs/releases/compatibility-policy.md`
- Build & CI: `docs/architecture/build-ci.md`
- Release checklist: `docs/architecture/release-security-checklist.md`
- Verification policy: `docs/verification-policy.md`
