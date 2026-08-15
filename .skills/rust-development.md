# Rust Development Skill

Use this skill when writing, modifying, or reviewing Rust code in the eggfetch workspace.

## Workflow

1. Read `AGENTS.md` for crate boundaries, lint policy, and quick commands.
2. Read `docs/architecture/dependency-policy.md` before adding any dependency.
3. Read `CONTRIBUTING.md` for coding conventions.

## Pre-commit Checklist

```sh
./scripts/check.sh              # Tier 1: routine validation (CI runs this)
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

## Key Constraints

- `unsafe_code = "forbid"` workspace-wide. Never add `unsafe`. If you think you need it, stop and ask.
- All HTTP logic belongs in `eggfetch-core`. CLI, Python, FFI, and Node are adapters.
- No parallel synchronous networking path. Python sync blocks on async Rust engine.
- Public items need doc comments. For skeletal types, state which milestone fills in the real implementation.
- Never use `#![allow(warnings)]`, `#![allow(clippy::all)]`, or `#![allow(clippy::pedantic)]`.
- Use specific lint names. Justify suppressions with a comment.

## Feature Matrix Validation

Before committing changes to eggfetch-core, verify compilation across feature combinations:

```sh
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --all-features
```

## Architecture References

- Overview: `docs/architecture/overview.md`
- Core engine: `docs/architecture/core-engine.md`
- Body & streaming: `docs/architecture/core-body-streaming.md`
- Timeouts & pool: `docs/architecture/core-timeout-pool.md`
- Auth, redirect & retry: `docs/architecture/core-auth-redirect-retry.md`
- TLS, proxy & protocols: `docs/architecture/core-tls-proxy-protocols.md`
- Cookies, multipart & compression: `docs/architecture/core-cookies-multipart-compression.md`
- Feature flags: `docs/architecture/feature-flags.md`
- Dependency policy: `docs/architecture/dependency-policy.md`

## HTTPX Compatibility

When working on the compatibility layer:

- **Typed API oracle**: `scripts/compare_httpx_api_manifest.py --validate` produces structured difference records gated by `allowed-differences.toml`.

### Compatibility test files

```sh
# Lossless merge semantics
python -m pytest crates/eggfetch-python/tests/compat/test_merge_lossless.py -v

# Native lifecycle and soak
python -m pytest crates/eggfetch-python/tests/compat/test_native_timeout_classification.py crates/eggfetch-python/tests/compat/test_soak.py -v

# Behavioral downstream fixtures
python -m pytest compat/downstream/behavioral_fixtures/ -v
```
