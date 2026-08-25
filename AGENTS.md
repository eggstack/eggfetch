# Agent Guide

eggfetch is a Rust-native async HTTP client engine (tokio + hyper) with thin adapters: a CLI,
Python bindings (sync + asyncio), C FFI, and Node bindings. There is exactly one networking
implementation, living entirely in `eggfetch-core`.

## Quick Commands

```sh
# Canonical validation (run before committing)
./scripts/check.sh              # Tier 1: routine validation (CI runs this)
./scripts/check.sh extended     # Tier 2: extended validation (includes Tier 1)
./scripts/check.sh package      # Tier 3: package validation (includes Tier 1)

cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
python -m pytest crates/eggfetch-python/tests/ -q --ignore=crates/eggfetch-python/tests/compat
python -m pip install -r compat/httpx/0.28.1/requirements.txt   # extended-tier deps
```

Tier 1 runs Rust tests with `--test-threads=1` (`--workspace --exclude eggfetch-python`) because
resource-stabilization tests measure process RSS; concurrent workspace tests make that
measurement scheduling-dependent. Do not parallelize them locally.

## Validation Tiers

| Tier | Command | When |
|------|---------|------|
| Routine | `./scripts/check.sh` | Every commit. CI repeats it on ubuntu-latest for pushes/PRs to `main` |
| Extended | `./scripts/check.sh extended` | Before release (manual): full compat suite, API oracle, feature matrix, MSRV, docs, FFI, soak |
| Package | `./scripts/check.sh package` | Before publish (manual): crate packaging + wheel build/smoke |

Details: `docs/verification-policy.md`.

## Python Environment

`./scripts/check.sh` requires an active virtual environment (it refuses to run outside one)
with Python 3.10+, maturin, pytest, and pytest-asyncio. Setup:

```sh
python3 -m venv .venv
source .venv/bin/activate
python -m pip install maturin pytest pytest-asyncio
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop -m crates/eggfetch-python/Cargo.toml
```

Rebuild the extension with `maturin develop` after changing `crates/eggfetch-python` Rust code;
stale extension modules cause confusing Python test failures.

## Skills

Specialized skills live in `.skills/`: [rust-development.md](.skills/rust-development.md),
[python-bindings.md](.skills/python-bindings.md), [cli-development.md](.skills/cli-development.md),
[documentation.md](.skills/documentation.md), [security-review.md](.skills/security-review.md),
[release-process.md](.skills/release-process.md), [fuzz-testing.md](.skills/fuzz-testing.md),
[ffi-development.md](.skills/ffi-development.md).

## Crate Boundaries

eggfetch-core owns all HTTP behavior. CLI and Python are thin adapters.

- eggfetch-core: no PyO3, no clap, no CLI arg parsing
- eggfetch-cli, eggfetch-python: no direct hyper/tokio TCP/networking — all I/O through eggfetch-core
- eggfetch-ffi, eggfetch-node: `unsafe_code = "allow"` (sole exceptions)

**Hard rule**: no parallel synchronous networking path. Python sync blocks on the async Rust
engine while releasing the GIL. If you write HTTP logic outside eggfetch-core, stop and refactor.

## Lint Policy

- Pedantic clippy workspace-wide; `unsafe_code = "forbid"` (except FFI/Node);
  `missing_docs = "warn"` (see workspace `Cargo.toml`).
- Never use `#![allow(warnings)]`, `#![allow(clippy::all)]`, or `#![allow(clippy::pedantic)]`;
  `scripts/check_lint_suppressions.sh` (part of Tier 1) rejects them.
- Use specific lint names and justify suppressions with a comment.

## Feature Flags

`eggfetch-core` default: `http1 + tls-rustls`. Everything else is opt-in:
`http2`, `http3`, `json`, `compression-{gzip,brotli,zstd,deflate}`, `cookies`, `proxy`,
`multipart`, `tracing`, `test-util`.

- CLI enables: cookies, multipart, proxy
- Python binding enables: http2, http3, cookies, multipart, proxy, all four compressions
- `test-util` enables `tokio/test-util` for deterministic time testing

## Transport Hints

`Request` carries typed `TransportHints` for wire-level overrides that do not affect logical URL
semantics: `target: Option<Bytes>` overrides the wire request target (e.g. `OPTIONS *`,
absolute-form) while preserving routing, Host, cookies, auth, and proxy selection;
`sni_hostname: Option<String>` overrides TLS SNI while preserving the TCP destination.
Hints survive retry reconstruction and are cleared on redirect hops (destination changed).
The Python compat facade passes `target`/`sni_hostname` from the request extensions dict
through the native `stream()` method.

## HTTPX Compatibility Layer

`eggfetch.compat.httpx` targets HTTPX 0.28.1 (asyncio only; Stage C qualified):

```python
from eggfetch.compat.httpx import Client, AsyncClient, Request, Response, URL, Headers, Cookies
```

```sh
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop -m crates/eggfetch-python/Cargo.toml
EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
```

The compatibility profile lives in `compat/httpx/0.28.1/` (`profile.toml`, API manifests,
`allowed-differences.toml`, `parity-cases.toml`). The qualification is bound to an exact
executable SHA recorded there; **any executable change invalidates it** and requires restarting
Corrective 07 from the freeze step (see `plans/httpx-parity-correction-status.md`). Extended-tier
gates regenerate and compare the API manifest via `scripts/generate_httpx_api_manifest.py` +
`scripts/compare_httpx_api_manifest.py`; do not hand-edit generated manifests.

## Hard Parity Constraints

Rules that are easy to violate accidentally; each was an explicit corrective pass.

**Timeouts**
- HTTPX `Timeout` maps only `connect`/`read`/`write`/`pool`. Never synthesize a native
  `Timeout.total` from phases; native callers may set `total` explicitly as an outer deadline.
- Preserve HTTPX's omitted-vs-explicit-`None` phase distinction; reject `Timeout()` unless a
  scalar or all four phases are supplied.
- Multi-phase HTTP(S) proxy setup uses one monotonic request deadline (min of explicit `total`
  and each phase budget). Direct Hyper/UDS/H3 transport futures are governed only by an explicit
  native total; read timeouts attach after transport setup.

**NO_PROXY**: the compat environment parser accepts bare unbracketed IPv6 but rejects bracketed
IPv6 and IPv6-prefix/CIDR-looking forms before dispatch; native `NoProxy::parse()` keeps the
richer bracketed/CIDR behavior. Do not unify them silently.

**SSLContext translation** (Python `ssl.SSLContext` → rustls):
- Never classify custom CA stores by count, names, similarity, or ordering heuristics.
- Helper-created contexts carry a construction fingerprint; post-construction mutation drops the
  stored metadata and reclassifies from the live snapshot.
- Passthrough caller-supplied contexts carry no `cert_path`/special `verify` kwarg (never
  downgrade someone's mTLS context or inherit helper trust into a `verify=False` context).
- Non-standard `ssl.SSLContext` subclasses and rustls-unrepresentable state fail closed with
  `TypeError` before dispatch. Representable settings translate exactly (including
  `CERT_REQUIRED` + `check_hostname=False` and min/max TLS version bounds).

**Proxy trust isolation**: proxy-endpoint TLS comes exclusively from the proxy configuration
(`Proxy(ssl_context=...)`). Origin `verify`, CA bundle, mTLS identity, SNI override, or version
policy must never influence the proxy handshake; without an explicit proxy context, rustls
default trust anchors apply.

**Redaction**: `Proxy.__repr__` and `Headers.__repr__` redact `authorization`,
`proxy-authorization`, `cookie`, `set-cookie` to `<redacted>`. Raw values stay available to
protocol/engine code. All Debug/Display/error output must redact secrets via
`eggfetch_core::redact`.

**H2-only semantics**: `HttpVersionPolicy::Http2Only` is enforced at three layers (rustls ALPN
list restricted to `h2`; `http2_only(true)` on standard/direct/SNI/SOCKS/UDS paths;
`Connected::negotiated_h2()` from the stream types). Cleartext H2-only uses the client preface
(h2c prior knowledge). H1-only/Auto must not set `http2_only`. Do not introduce a parallel
"HTTPX-only" client engine or fork hyper to add capability.

**Residual differences** (do not paper over): `stream_id` metadata is unreachable through
hyper's legacy client (`Incoming` wraps `h2::RecvStream` privately) — never synthesize one;
HTTP/2 origin framing through an HTTP CONNECT proxy remains HTTP/1.1; HTTPX's four-element
null-pointer `socket_options` form is rejected at the safe boundary. Classification rules:
`docs/residual-differences.md`. Coroutine trace callbacks are rejected with `TypeError` before
dispatch (core `TraceObserver` is synchronous); sync callbacks work on `Client` and
`AsyncClient`.

**Proxy headers**: `Proxy(headers=...)` rides the proxy leg only — never forwarded through a
CONNECT tunnel or to the origin.

## Network Stream and Upgrades

- Only 101 Switching Protocols responses own a writable `UpgradedStream`, exposed to Python via
  `response.extensions["network_stream"]`. Ordinary pooled responses set it to `None` (connection
  returned to the pool); internal HTTPS CONNECT tunnels are never surfaced as a writable stream
  (use the body iterator).
- `UpgradedStreamVariant` (`Tcp`/`Tls`/`Adapter`) gates `start_tls`: only inner `Tcp` variants are
  eligible; `Adapter` (Hyper's opaque 101 wrapper) and `Tls` (already encrypted) are rejected
  before any IO.
- Wrapper selection follows the caller's API mode: sync `Client.stream()` gets `PyNetworkStream`
  (GIL-released, carries an explicit runtime `Handle` + optional `RuntimeLease`); async
  `AsyncClient.request()` gets `PyAsyncNetworkStream`. Both expose `read`, `write`, `close`,
  `is_upgraded`, `get_extra_info`, `start_tls(...)` behind `EitherNetworkStream`. Cloning shares
  the underlying IO (Arc<Mutex<>>), not duplicated.
- Leading data written right after 101 headers is preserved by Hyper's rewind buffer and yielded
  on the first upgraded-stream reads.

Key types: `eggfetch_core::network_stream` (`ConnectionMetadata`, `UpgradedStream`,
`UpgradedStreamVariant`, `NetworkStream`, `TlsInfo`, `ExtraInfo`).

## Release

- Release timing and crates.io publication are manual maintainer actions; GitHub Actions never
  publishes to crates.io.
- PyPI publication is the manually dispatched `.github/workflows/pypi.yml`: 12 wheels
  (linux-x86_64, macos-arm64, windows-x86_64 × Python 3.10–3.13) plus one sdist, uploaded via
  Trusted Publishing (OIDC, `pypi` environment).
- Coordinated versions across publishable crates; bench/fuzz are not published.
- Publishing order: eggfetch-core → eggfetch-cli → eggfetch-ffi → eggfetch-python →
  eggfetch-node, then tag and dispatch PyPI workflow. See `docs/releases/process.md` and
  `docs/releases/compatibility-policy.md`.

## Working Style

- Make the workspace build green before adding new functionality; run `./scripts/check.sh`
  before committing.
- Keep commits scoped to a single logical change; do not commit without an explicit user request.
- Public items need doc comments; for skeletal types, state which milestone fills in the real
  implementation.
- Do not add CI jobs, matrices, evidence formats, release workflows, or publication automation
  without an explicit user request. Prefer direct tests in the existing local check path.

## Architecture Docs

Deep dives live in `docs/architecture/` (filenames match topics). Start points:
`overview.md` (crate graph, module map), `core-engine.md` (client/pipeline),
`core-timeout-pool.md`, `core-tls-proxy-protocols.md`, `build-ci.md` (CI/lint/MSRV/release),
`feature-flags.md` (validation matrix), `threat-model.md`. Residual-difference policy:
`docs/residual-differences.md`.
