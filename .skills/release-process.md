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

# Release manifest
python scripts/generate_release_manifest.py --output compatibility-manifest.json

# Package content validation
python scripts/validate_package_content.py path/to/wheel.whl
```

## CI

CI runs format, clippy, and test checks on pushes and pull requests. The Required CI Gate is a mandatory merge prerequisite. Verify locally before committing.

## Dry-Run Release Validation

Trigger a dry-run release via GitHub Actions:

1. Create an immutable validation tag: `git tag rc-dry-run-$(git rev-parse --short=12 HEAD)`
2. Push the tag: `git push origin rc-dry-run-$(git rev-parse --short=12 HEAD)`
3. Dispatch the workflow:
   ```sh
   gh workflow run release.yml \
     --ref rc-dry-run-$(git rev-parse --short=12 HEAD) \
     -f candidate_sha=$(git rev-parse HEAD) \
     -f version=0.1.0-rc1 \
     -f dry_run=true
   ```
4. Monitor the run: `gh run watch`
5. Download evidence: `gh run download <run-id> --dir release-dry-run-evidence`

## Architecture References

- Release process: `docs/releases/process.md`
- Compatibility policy: `docs/releases/compatibility-policy.md`
- Build & CI: `docs/architecture/build-ci.md`
- Release checklist: `docs/architecture/release-security-checklist.md`
