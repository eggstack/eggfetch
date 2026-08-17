# Agent Guide

## Quick Commands

```sh
# Canonical validation (run before committing)
./scripts/check.sh              # Tier 1: routine validation (CI runs this)
./scripts/check.sh extended     # Tier 2: extended validation
./scripts/check.sh package      # Tier 3: package validation

# Focused commands
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
python -m pytest crates/eggfetch-python/tests/ -q --ignore=crates/eggfetch-python/tests/compat
python -m pip install -r compat/httpx/0.28.1/requirements.txt
```

Tier 1 serializes the Rust test harness because the resource-stabilization
tests measure process RSS; concurrent workspace tests can otherwise make that
measurement scheduling-dependent.

## Python Environment

`./scripts/check.sh` requires an active virtual environment with Python 3.10+, maturin, pytest, and pytest-asyncio. Setup:

```sh
python3 -m venv .venv
source .venv/bin/activate
python -m pip install maturin pytest pytest-asyncio
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop -m crates/eggfetch-python/Cargo.toml
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
- `missing_docs = "warn"` in workspace `Cargo.toml`.
- Never use `#![allow(warnings)]`, `#![allow(clippy::all)]`, or `#![allow(clippy::pedantic)]`. CI rejects these via `scripts/check_lint_suppressions.sh`.
- Use specific lint names. Justify suppressions with a comment.

## Feature Flags

`eggfetch-core` default: `http1 + tls-rustls`. All other features are opt-in.

Key flags: `http2`, `http3`, `json`, `compression-{gzip,brotli,zstd,deflate}`, `cookies`, `proxy`, `multipart`, `tracing`, `test-util`.

The CLI enables: cookies, multipart, proxy. The Python binding enables all features including http3. `test-util` enables `tokio/test-util` for deterministic time testing.

## HTTPX Compatibility Layer

The `eggfetch.compat.httpx` module provides an HTTPX 0.28.1-compatible asyncio facade (**Stage C qualified**). Import it as:

```python
from eggfetch.compat.httpx import Client, AsyncClient, Request, Response, URL, Headers, Cookies
```

Run compat tests:

```sh
cd crates/eggfetch-python && maturin develop
EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
```

API oracle with typed differences:

```sh
python scripts/compare_httpx_api_manifest.py \
  --reference compat/httpx/0.28.1/reference-api.json \
  --candidate /tmp/eggfetch-api.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml \
  --json --output /tmp/api-result.json
```

The compatibility profile is in `compat/httpx/0.28.1/`. Executable changes require fresh exact-SHA qualification (see `compat/httpx/0.28.1/profile.toml`). Key bounded differences: `Proxy(headers=...)` rejected, `Timeout` maps only `connect`/`read`/`write`/`pool` (no native `total` synthesis), bracketed IPv6 and CIDR `NO_PROXY` forms rejected.

## Tests

Colocated `#[cfg(test)] mod tests` blocks. ~685 Rust, ~558 Python (non-compat), ~1659 Python (compat), 30 FFI tests.

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

Release timing and crates.io publication are manual maintainer actions. GitHub Actions does not publish to crates.io.

PyPI publication is performed via the manually dispatched `.github/workflows/pypi.yml` workflow. It builds 20 wheels across 5 platforms and 4 Python versions, plus a source distribution. PyPI upload uses Trusted Publishing (OIDC) with the `pypi` GitHub environment.

Coordinated versioning across all publishable crates (core, CLI, Python, FFI, Node). Bench and fuzz crates are not published.

Publishing order: eggfetch-core → eggfetch-cli → eggfetch-ffi → eggfetch-python → eggfetch-node. Then tag and dispatch PyPI workflow.

See `docs/releases/process.md` and `docs/releases/compatibility-policy.md`.

## Working Style

- Make the workspace build green before adding new functionality.
- Run `./scripts/check.sh` before committing.
- Keep commits scoped to a single logical change.
- Do not commit without an explicit user request.
- Public items need doc comments. For skeletal types, state which milestone fills in the real implementation.

## Architecture Index

Detailed architecture docs live in `docs/architecture/`. Use this index to find the right deep-dive:

| Topic | Document |
|-------|----------|
| Workspace layout, crate graph, module map | [docs/architecture/overview.md](docs/architecture/overview.md) |
| Client, RequestBuilder, Response, pipeline | [docs/architecture/core-engine.md](docs/architecture/core-engine.md) |
| RequestBody, ResponseBody, streaming adapters | [docs/architecture/core-body-streaming.md](docs/architecture/core-body-streaming.md) |
| Phase-aware timeouts, connection pool | [docs/architecture/core-timeout-pool.md](docs/architecture/core-timeout-pool.md) |
| Auth, redirect following, retry with backoff | [docs/architecture/core-auth-redirect-retry.md](docs/architecture/core-auth-redirect-retry.md) |
| TLS config, HTTP proxy, HTTP/2, HTTP/3 | [docs/architecture/core-tls-proxy-protocols.md](docs/architecture/core-tls-proxy-protocols.md) |
| Cookies, multipart, compression | [docs/architecture/core-cookies-multipart-compression.md](docs/architecture/core-cookies-multipart-compression.md) |
| CLI argument model, exit codes | [docs/architecture/cli.md](docs/architecture/cli.md) |
| Python sync/async adapter, PyO3 bridge | [docs/architecture/python-bindings.md](docs/architecture/python-bindings.md) |
| C ABI handles, N-API prototype | [docs/architecture/ffi-and-node.md](docs/architecture/ffi-and-node.md) |
| Unit/integration tests, fuzz targets, property tests | [docs/architecture/testing-fuzzing.md](docs/architecture/testing-fuzzing.md) |
| CI pipeline, lint policy, MSRV, release process | [docs/architecture/build-ci.md](docs/architecture/build-ci.md) |
| Feature flag reference and validation matrix | [docs/architecture/feature-flags.md](docs/architecture/feature-flags.md) |
| Dependency selection criteria, pool key semantics | [docs/architecture/dependency-policy.md](docs/architecture/dependency-policy.md) |
| Threat model, trust boundaries | [docs/architecture/threat-model.md](docs/architecture/threat-model.md) |
| Security review records | [docs/architecture/security-reviews.md](docs/architecture/security-reviews.md) |
| Security findings tracker | [docs/architecture/security-findings.md](docs/architecture/security-findings.md) |
| Pre-release security checklist | [docs/architecture/release-security-checklist.md](docs/architecture/release-security-checklist.md) |
| Vulnerability response and CVE process | [docs/architecture/incident-runbook.md](docs/architecture/incident-runbook.md) |

> Do not add CI jobs, matrices, evidence formats, release workflows, or publication automation without an explicit user request. Prefer direct tests in the existing local check path.

### HTTPX corrective closure

The compact `test_corrective_kernel.py` is part of Tier 1; the pinned
transport differential suite, full pinned-reference compat, API oracle, and
downstream isolated runner are extended gates. The pinned reference remains
`httpx==0.28.1` (with its installed `httpcore`/`socksio` versions recorded in
the qualification handoff). The corrective transport profile is bound to the
exact SHA recorded in `compat/httpx/0.28.1/profile.toml`; any executable change
requires a fresh qualification pass. Pass 05 reopened the prior Stage C
qualification before correcting ordinary-domain and default-port `NO_PROXY`
semantics. Pass 06 closed IPv6 environment-form parity and route/pre-dispatch
evidence on the exact SHA recorded in the profile. Pass-04 timeout/runtime/
proxy work and pass-05 generic-domain/default-port fixes remain historical
accepted evidence and were not reopened. Do not silently revive historical
qualification counts.

The current corrective boundary derives one monotonic request deadline for
multi-phase HTTP/HTTPS proxy setup. `Proxy(headers=...)` remains an explicit
Stage C bounded difference: non-empty metadata is rejected at conversion until
it can be carried on the proxy leg without a generalized routing redesign.
HTTPX `Timeout` conversion maps only its `connect`, `read`, `write`, and `pool`
values; it must not synthesize native `Timeout.total`. Native callers may set
`total` explicitly as an outer deadline, and proxy setup uses the smaller of
that deadline and each configured phase budget.
The compatibility constructor preserves HTTPX's omitted-vs-explicit-`None`
phase distinction and rejects `Timeout()` unless a scalar or all four phases
are supplied. Direct Hyper/UDS/H3 transport futures are governed only by an
explicit native total; read timeouts attach to response body chunks after
transport setup, while proxy protocol reads retain header-phase enforcement.
The compatibility environment parser accepts HTTPX's bare unbracketed IPv6
forms but rejects bracketed IPv6 and IPv6 prefix-looking `NO_PROXY` forms
before native routing is constructed; native `NoProxy::parse()` retains its
richer bracketed IPv6 and CIDR behavior.
