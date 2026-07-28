# Agent Guide

## Quick Commands

```sh
# Format, lint, test — run before committing
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

# Feature matrix validation
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --all-features

# Python tests (must build wheel first)
cd crates/eggfetch-python && maturin develop
python -m pytest -p pytest_asyncio

# HTTPX compatibility tests (requires httpx==0.28.1)
python -m pip install -r compat/httpx/0.28.1/requirements.txt
EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/ -v --strict-markers

# Generate and compare API manifests
python scripts/generate_httpx_api_manifest.py --package httpx --output /tmp/httpx.json
python scripts/generate_httpx_api_manifest.py --package eggfetch --output /tmp/eggfetch.json
python scripts/compare_httpx_api_manifest.py \
  --reference /tmp/httpx.json \
  --candidate /tmp/eggfetch.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml

# Package content validation
python scripts/validate_package_content.py path/to/wheel.whl
```

## Validation Tiers

| Tier | Command | When |
|------|---------|------|
| Routine | `./scripts/check.sh` | Every commit, CI |
| Extended | `./scripts/check.sh extended` | Before release, manual |
| Package | `./scripts/check.sh package` | Before publish, manual |

CI repeats Tier 1 on Ubuntu for every push/PR. See `docs/verification-policy.md`.

## Skills

Specialized skills for common tasks live in `.skills/`:

| Skill | When to Use |
|-------|-------------|
| [rust-development.md](.skills/rust-development.md) | Writing, modifying, or reviewing Rust code |
| [python-bindings.md](.skills/python-bindings.md) | Working on eggfetch-python (PyO3/maturin) |
| [cli-development.md](.skills/cli-development.md) | Working on eggfetch-cli |
| [documentation.md](.skills/documentation.md) | Updating docs, verifying accuracy |
| [security-review.md](.skills/security-review.md) | Security reviews, addressing findings |
| [release-process.md](.skills/release-process.md) | Preparing or executing releases |
| [fuzz-testing.md](.skills/fuzz-testing.md) | Fuzz targets, property tests |
| [ffi-development.md](.skills/ffi-development.md) | FFI and Node.js bindings |

## Crate Boundaries

eggfetch-core owns all HTTP behavior. CLI and Python are thin adapters.

- eggfetch-core: no PyO3, no clap, no CLI arg parsing
- eggfetch-cli, eggfetch-python: no direct hyper/tokio TCP/networking — all I/O through eggfetch-core
- eggfetch-ffi, eggfetch-node: unsafe_code = "allow" (sole exceptions)

**Hard rule**: no parallel synchronous networking path. Python sync blocks on async Rust engine. If you write HTTP logic outside eggfetch-core, stop and refactor.

## Lint Policy

- Pedantic clippy enabled workspace-wide. `unsafe_code = "forbid"` (except FFI/Node).
- `missing_docs = "warn"`, `missing-docs-in-crate-items = true`.
- Never use `#![allow(warnings)]`, `#![allow(clippy::all)]`, or `#![allow(clippy::pedantic)]`. CI rejects these via `scripts/check_lint_suppressions.sh`.
- Use specific lint names. Justify suppressions with a comment.

## Feature Flags

`eggfetch-core` default: `http1 + tls-rustls`. All other features are opt-in.

Key flags: `http2`, `http3`, `json`, `compression-{gzip,brotli,zstd,deflate}`, `cookies`, `proxy`, `multipart`, `tracing`, `test-util`.

The CLI enables: cookies, multipart, proxy. The Python binding enables all features including http3. `test-util` enables `tokio/test-util` for deterministic time testing.

## HTTPX Compatibility Layer

The `eggfetch.compat.httpx` module provides an HTTPX 0.28.1-compatible asyncio facade (Stage C candidate). Import it as:

```python
from eggfetch.compat.httpx import Client, AsyncClient, Request, Response, URL, Headers, Cookies
```

The compatibility stage is **Stage C candidate** (asyncio drop-in). The corrective closure pass applies:

- **Typed difference records** — API oracle produces structured difference records; `allowed-differences.toml` gates enforcement; `resolved-differences.toml` tracks historical/behavioral entries separate from the active allowlist.
- **Lossless merge semantics** — header and query parameter merge preserves order and duplicates across transports.
- **Separate sync/async auth drivers** — `Auth` base class dispatches to sync and async implementations independently.
- **Behavioral downstream fixtures** — `compat/downstream/behavioral_fixtures/` exercises real consumer patterns.
- **Native lifecycle proof fixtures** — timeout classification, soak, proxy, and TLS tests validate engine behavior under load.

Runtime diagnostics:

```python
from eggfetch.compat.httpx import get_compatibility_info, diagnostics_summary
info = get_compatibility_info()
print(info.provider)          # "eggfetch"
print(info.emulated_version)  # "0.28.1"
print(info.compatibility_stage)  # "stage-c-candidate"
```

Run compat tests:

```sh
cd crates/eggfetch-python && maturin develop
EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
```

API oracle with typed differences:

```sh
python scripts/compare_httpx_api_manifest.py \
  --reference compat/httpx/0.28.1/reference-api.json \
  --candidate /tmp/eggfetch-api.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml \
  --json --output /tmp/api-result.json
```

## Tests

Colocated `#[cfg(test)] mod tests` blocks. ~880+ Rust, ~1170+ Python, ~30+ FFI tests.

The full validation pass (pre-release) runs feature-gated subsets:

```sh
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-brotli
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-zstd
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-deflate
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
```

The HTTPX compatibility test suite lives in `crates/eggfetch-python/tests/compat/` and requires `httpx==0.28.1`. Run with `EGGFETCH_COMPAT_REQUIRED=1` for fail-closed behavior. The compatibility profile is in `compat/httpx/0.28.1/`.

## Security

- `deny.toml` configures cargo-deny (advisories, licenses, bans, sources).
- All Debug/Display/error output must redact secrets via `eggfetch_core::redact`.
- See `SECURITY.md` and `docs/architecture/threat-model.md`.

## Release

Release timing and crates.io publication are manual maintainer actions. GitHub Actions does not publish packages.

Coordinated versioning across all publishable crates (core, CLI, Python, FFI, Node). Bench and fuzz crates are not published.

Publishing order: eggfetch-core → eggfetch-cli → eggfetch-ffi → eggfetch-python → eggfetch-node.

See `docs/releases/process.md` and `docs/releases/compatibility-policy.md`.

## Working Style

- Make the workspace build green before adding new functionality.
- Run `./scripts/check.sh` before committing.
- Keep commits scoped to a single logical change.
- Do not commit without an explicit user request.
- Public items need doc comments. For skeletal types, state which milestone fills in the real implementation.

## Safety

Do not add `unsafe`. Workspace uses `unsafe_code = "forbid"`. If you think you need `unsafe`, stop and ask.

> Do not add CI jobs, matrices, evidence formats, release workflows, or publication automation without an explicit user request. Prefer direct tests in the existing local check path.
