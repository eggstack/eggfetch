# Agent Guide

## Quick Commands

```sh
# Format, lint, test — run before committing
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

# Feature matrix validation (used in CI)
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --all-features

# Python tests (must build wheel first)
cd crates/eggfetch-python && maturin develop
python -m pytest -p pytest_asyncio

# HTTPX compatibility tests (requires httpx==0.28.1)
pip install -r compat/httpx/0.28.1/requirements.txt
EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/ -v --strict-markers

# Validate compatibility profile
python scripts/validate_httpx_compat_profile.py compat/httpx/0.28.1

# Generate and compare API manifests
python scripts/generate_httpx_api_manifest.py --package httpx --output /tmp/httpx.json
python scripts/generate_httpx_api_manifest.py --package eggfetch --output /tmp/eggfetch.json
python scripts/compare_httpx_api_manifest.py \
  --reference /tmp/httpx.json \
  --candidate /tmp/eggfetch.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml

# Resource regression check
cargo build --release -p eggfetch-bench --bin resource_monitor
./target/release/resource_monitor

```

CI enforces `RUSTFLAGS=-D warnings`. MSRV is Rust 1.80.

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
- CI runs format, clippy, and test checks on pushes and pull requests. It is informational, not a merge gate.

## Feature Flags

`eggfetch-core` default: `http1 + tls-rustls`. All other features are opt-in.

Key flags: `http2`, `http3`, `json`, `compression-{gzip,brotli,zstd,deflate}`, `cookies`, `proxy`, `multipart`, `tracing`, `test-util`.

The CLI enables: cookies, multipart, proxy. The Python binding enables all features including http3. `test-util` enables `tokio/test-util` for deterministic time testing.

## HTTPX Compatibility Layer

The `eggfetch.compat.httpx` module provides an HTTPX 0.28.1 drop-in facade. Import it as:

```python
from eggfetch.compat.httpx import Client, AsyncClient, Request, Response, URL, Headers, Cookies
```

Phase 2 is complete — value objects, request/response, exception hierarchy, client constructors, and merge semantics are implemented. See `plans/httpx-drop-in-phase-2-status.md` for status.

Phase 3 is complete — streaming and bodies: `SyncByteStream`, `AsyncByteStream`, `ByteStream` base classes; `iter_raw()`/`aiter_raw()` for raw transport bytes; chunk size support on all iterators; request streaming via iterables/file-like/custom streams; multipart passthrough. See `plans/httpx-drop-in-phase-3-status.md` for status.

Phase 4 is complete — transports, mounts, auth, hooks, WSGI/ASGI. See `plans/httpx-drop-in-phase-4-status.md` for status.

Phase 4 test files: `test_transports.py`, `test_mounts.py`, `test_mock_transport.py`, `test_wsgi.py`, `test_asgi.py`, `test_hooks.py`, `test_auth.py`.

Run compat tests:

```sh
cd crates/eggfetch-python && maturin develop
EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
```

## Tests

Colocated `#[cfg(test)] mod tests` blocks. ~750+ Rust, ~463+ Python, ~40+ FFI tests.

The full validation pass (pre-release) runs feature-gated subsets:

```sh
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-brotli
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-zstd
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-deflate
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
```

The HTTPX compatibility test suite lives in `crates/eggfetch-python/tests/compat/` and requires `httpx==0.28.1`. Run with `EGGFETCH_COMPAT_REQUIRED=1` for fail-closed behavior. The compatibility profile is in `compat/httpx/0.28.1/`.

CI matrix: Python 3.10-3.13 on Ubuntu, macOS, Windows. CI must install `pytest-asyncio` explicitly.

## Security

- `deny.toml` configures cargo-deny (advisories, licenses, bans, sources).
- `.github/workflows/security.yml` runs cargo-deny and cargo-audit on every push/PR.
- All Debug/Display/error output must redact secrets via `eggfetch_core::redact`.
- See `SECURITY.md` and `docs/architecture/threat-model.md`.

## Release

Coordinated versioning across all publishable crates (core, CLI, Python, FFI, Node). Bench and fuzz crates are not published.

Publishing order: eggfetch-core → eggfetch-cli → eggfetch-ffi → eggfetch-python → eggfetch-node (crates.io index propagation requires waits).

See `docs/releases/process.md` and `docs/releases/compatibility-policy.md`.

## Working Style

- Make the workspace build green before adding new functionality.
- CI runs on pushes and pull requests. It is informational — verify locally before committing.
- Keep commits scoped to a single logical change.
- Do not commit without an explicit user request.
- Public items need doc comments. For skeletal types, state which milestone fills in the real implementation.

## Safety

Do not add `unsafe`. Workspace uses `unsafe_code = "forbid"`. If you think you need `unsafe`, stop and ask.
