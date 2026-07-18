# Release Process Skill

Use this skill when preparing or executing a release of eggfetch.

## Workflow

1. Read `docs/releases/process.md` for the release workflow.
2. Read `docs/releases/compatibility-policy.md` for versioning rules.
3. Read `docs/architecture/release-security-checklist.md` and complete all items.
4. Read `docs/architecture/build-ci.md` for the CI pipeline.

## Publishing Order

1. `eggfetch-core`
2. `eggfetch-cli`
3. `eggfetch-ffi`
4. `eggfetch-python`
5. `eggfetch-node`

Crate.io index propagation requires waits between publishes. Bench and fuzz crates are not published.

## Pre-release Validation

```sh
# Full test suite
cargo test --workspace --all-features

# Feature matrix
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --all-features

# Feature-gated tests
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-brotli
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-zstd
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,http3

# Python tests
cd crates/eggfetch-python && maturin develop
python -m pytest -p pytest_asyncio

# Lint
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Architecture References

- Release process: `docs/releases/process.md`
- Compatibility policy: `docs/releases/compatibility-policy.md`
- Build & CI: `docs/architecture/build-ci.md`
- Release checklist: `docs/architecture/release-security-checklist.md`
