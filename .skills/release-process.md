# Release Process Skill

Use this skill when preparing or executing a release of eggfetch.

## Workflow

1. Read `docs/releases/process.md` for the manual release procedure.
2. Read `docs/releases/compatibility-policy.md` for versioning rules.
3. Read `docs/architecture/release-security-checklist.md` and complete all items.

## Publishing Order

1. `eggfetch-core`
2. `eggfetch-cli`
3. `eggfetch-ffi`
4. `eggfetch-python`
5. `eggfetch-node`

crates.io index propagation requires verification between publishes. Do not encode fixed sleeps.

## Pre-release Validation

```sh
./scripts/check.sh
./scripts/check.sh package
```

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

CI runs `./scripts/check.sh` on pushes and pull requests. It is a regression safety net, not a release authority.

## Architecture References

- Release process: `docs/releases/process.md`
- Compatibility policy: `docs/releases/compatibility-policy.md`
- Build & CI: `docs/architecture/build-ci.md`
- Release checklist: `docs/architecture/release-security-checklist.md`
