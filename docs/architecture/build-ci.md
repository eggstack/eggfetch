# Build & CI Deep Dive

This document covers the build system, CI pipeline, lint policy, MSRV, and release process.

See also: [overview.md](overview.md).

## Build Configuration

### rust-toolchain.toml

Pins to the stable Rust channel.

### rustfmt.toml

- `max_width = 100`
- Standard rustfmt defaults

### .clippy.toml

Pedantic clippy enabled workspace-wide.

### deny.toml

cargo-deny configuration for:
- Advisory database (security vulnerabilities)
- License compliance
- Dependency bans
- Source restrictions

## CI Pipeline

GitHub Actions workflow at `.github/workflows/ci.yml`. Runs on push to `main` and pull requests.

### Jobs

| Job | What It Does |
|-----|-------------|
| `rust-format` | `cargo fmt --all -- --check` |
| `rust-lint` | `cargo clippy --workspace --all-targets --all-features -- -D warnings` + lint suppression check |
| `rust-msrv` | Check with Rust 1.80 toolchain |
| `rust-test` | Build + feature matrix + tests on Ubuntu/macOS/Windows |
| `rust-doc` | Documentation build + doctests |
| `docs-syntax` | Python doc example syntax, internal links, CLI help, API surface |
| `docs-runtime` | Execute Python doc examples |
| `resource-monitor` | Build and run resource regression check |
| `python` | Python tests (12 combos: 3 OS × 4 Python versions) |
| `wheel-smoke` | Build wheel and smoke test in clean environment |
| `compat-httpx` | HTTPX 0.28.1 compatibility tests, manifest comparison, doc claim linting |
| `matrix-summary` | Aggregate results into JSON report |

### Environment

- `CARGO_TERM_COLOR=always`
- `RUSTFLAGS=-D warnings` — warnings are errors
- `PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1`

### Feature Matrix Validation

```sh
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --all-features
cargo test -p eggfetch-core --all-features
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-brotli
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-zstd
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-deflate
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,multipart,proxy
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
```

### Additional Workflows

| File | Purpose |
|------|---------|
| `security.yml` | cargo-deny + cargo-audit on push/PR |
| `benchmarks.yml` | Criterion benchmark tracking |
| `ffi.yml` | FFI build and test |
| `release.yml` | Coordinated release automation |

## Lint Policy

- Pedantic clippy workspace-wide.
- `unsafe_code = "forbid"` (except FFI/Node).
- `missing_docs = "warn"`, `missing-docs-in-crate-items = true`.
- Never use `#![allow(warnings)]`, `#![allow(clippy::all)]`, or `#![allow(clippy::pedantic)]`.
- CI rejects blanket suppressions via `scripts/check_lint_suppressions.sh`.
- Use specific lint names. Justify suppressions with a comment.

## MSRV

**Rust 1.80** — checked in CI via `cargo check` with the 1.80 toolchain.

## Release Process

Coordinated versioning across all publishable crates. See `docs/releases/process.md` and `docs/releases/compatibility-policy.md`.

### Publishing Order

1. `eggfetch-core`
2. `eggfetch-cli`
3. `eggfetch-ffi`
4. `eggfetch-python`
5. `eggfetch-node`

Crate.io index propagation requires waits between publishes. Bench and fuzz crates are not published.

## Quick Commands

```sh
# Format, lint, test — run before committing
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

# Python tests
cd crates/eggfetch-python && maturin develop
python -m pytest -p pytest_asyncio

# Resource regression check
cargo build --release -p eggfetch-bench --bin resource_monitor
./target/release/resource_monitor
```
