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

## Transport Hints

`Request` carries a typed `TransportHints` struct for wire-level overrides that do not affect logical URL semantics:

- `target: Option<Bytes>` — overrides the wire request target (e.g. `OPTIONS *`, absolute-form) while preserving the logical URL for routing, Host header, cookies, auth, and proxy selection.
- `sni_hostname: Option<String>` — overrides TLS SNI while preserving the TCP destination.

Transport hints survive through retry reconstruction. They are cleared on redirect hops because the destination changes. The Python compat facade extracts `target` and `sni_hostname` from the request extensions dict and passes them through the native `stream()` method.

## Network Stream and Upgrade Support

`Response` carries an optional `NetworkStream` for connection metadata and upgraded-connection IO:

- **101 Switching Protocols**: captures Hyper's upgrade future and attaches an `UpgradedStream` providing async read/write/close/TLS-start. The upgrade is exposed to Python via `response.extensions["network_stream"]` (sync and async).
- **Ordinary responses**: `NetworkStream::Metadata` holds read-only `ConnectionMetadata` (addresses, transport kind, TLS info). The buffered Python response sets `extensions["network_stream"] = None` — the connection has been returned to the pool.
- **HTTP/2**: shared connection metadata; no per-response raw socket exposure.
- **Internal HTTPS CONNECT tunnels**: never surfaced as a writable `NetworkStream`. The canonical access path for the tunnel is the body iterator.

`UpgradedStream` carries an `UpgradedStreamVariant` (`Tcp`/`Tls`/`Adapter`) so callers can detect whether `start_tls` is safe to invoke. Only inner `Tcp` variants are eligible; `Adapter` (Hyper's opaque wrapping of 101 upgrades) and `Tls` (already-encrypted) are rejected before any IO is consumed.

Key types in `eggfetch_core::network_stream`:

| Type | Purpose |
|------|---------|
| `ConnectionMetadata` | Read-only socket addresses, transport kind, TLS info |
| `UpgradedStream` | Owned post-HTTP IO for 101/CONNECT handoff |
| `UpgradedStreamVariant` | Classifies inner IO as `Tcp`/`Tls`/`Adapter` for `start_tls` safety |
| `NetworkStream` | Enum: `Upgraded(UpgradedStream)` or `Metadata(Arc<ConnectionMetadata>)` |
| `TlsInfo` | Negotiated ALPN, version, cipher suite, SNI |
| `ExtraInfo` | `get_extra_info()` compatibility subset |

Python bindings expose `PyNetworkStream` (sync, GIL-released) and `PyAsyncNetworkStream`
(async, awaits on the asyncio loop). Both expose
`read`, `write`, `close`, `is_upgraded`, `get_extra_info`, and
`start_tls(ssl_context, server_hostname, timeout)`. Wrapper selection follows
the caller's API mode: sync `Client.stream()` 101 responses expose the sync
wrapper, and async `AsyncClient.request()` buffered 101 responses expose the
async wrapper. The wrapper is stored behind an `EitherNetworkStream` enum in
the native response. Cloning a `PyNetworkStream` shares the same underlying
`Arc<Mutex<>>` so the IO is shared, not duplicated. The sync wrapper carries
an explicit `tokio::runtime::Handle` plus optional `RuntimeLease` so it can
drive IO without relying on an ambient runtime; the async wrapper does not
block from inside a running Tokio task.

Leading data after 101 headers is preserved inside Hyper's internal rewind buffer and yielded on the first reads from the upgraded stream.

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

The compatibility profile is in `compat/httpx/0.28.1/`. Executable changes require fresh exact-SHA qualification (see `compat/httpx/0.28.1/profile.toml`). Key bounded differences: `Timeout` maps only `connect`/`read`/`write`/`pool` (no native `total` synthesis), bracketed IPv6 and CIDR `NO_PROXY` forms rejected.

## Tests

Colocated `#[cfg(test)] mod tests` blocks. ~959 Rust, ~558 Python (non-compat), ~1794 Python (compat), 30 FFI tests.

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
the qualification handoff). Corrective 07 closed the remaining-parity line
on 2026-08-24 with `qualification-sha = 5c7899fefb6df087dfa1b3578fbef9ba64f87742`.
The earlier `9ffa6cd85848fd16a424b65f73254351911777c4` (the original
Corrective 07 freeze) and `c44d4f25ffebc1a792335163ae4bc106076b3963`
(Corrective 05) qualifications are retained as historical evidence only;
the former was rebaselined to absorb a single-line H3 test fixture
`#[allow]` extension so the same code passes clippy on both the local
qualifier toolchain and stable Rust 1.98+, and the latter was invalidated
by the Corrective 06 changes it predated. Any future executable change
invalidates the current qualification and requires restarting Corrective 07
from the freeze step. Earlier Pass 05/06 records are historical evidence only;
do not silently revive their counts or current-language claims.

The current corrective boundary derives one monotonic request deadline for
multi-phase HTTP/HTTPS proxy setup. `Proxy(headers=...)` is resolved: proxy
headers are carried on the proxy leg and never forwarded into the tunnel or to
the origin. HTTPX `Timeout` conversion maps only its `connect`, `read`, `write`, and `pool`
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

### SSLContext translation and proxy trust isolation

The Python `ssl.SSLContext` translation layer must never classify
custom CA stores using CA count, similarity, names, or ordering
heuristics.  Two stores with identical cardinalities but different
contents must produce different `verify` kwargs.  Helper-created
contexts are guarded by a construction fingerprint (SHA-256 over
extractable public state); post-construction mutation
(`load_verify_locations`, `set_minimum_version`, `set_ciphers`,
`set_alpn_protocols`) drops the stored metadata and reclassifies
from the live snapshot.  Passthrough contexts (caller-supplied via
`verify=<SSLContext>`) carry no `cert_path` and no special `verify`
kwarg; this prevents a caller-supplied mTLS context from being
silently downgraded to no client auth, and prevents a caller-supplied
`verify=False` from inheriting the helper's default trust.

The translator preserves representable TLS settings exactly:
`CERT_REQUIRED + check_hostname=False` is translated as certificate
verification enabled and hostname verification disabled; explicit
TLS 1.2/1.3 min/max bounds from the snapshot are forwarded into the
native `TlsConfigBuilder` via `min_tls_version` / `max_tls_version`.
Arbitrary caller-created `ssl.SSLContext` subclasses (anything other
than the standard `ssl.SSLContext` class itself) are rejected with
`TypeError` before dispatch because Python's public API does not
expose client-cert loading, ALPN state, or trust-store mutations
through documented interfaces.

Proxy endpoint TLS is sourced exclusively from the proxy
configuration.  The origin `TlsConfig` is never used as a fallback
for the proxy handshake.  An origin `verify=False`, a custom origin
CA bundle, an origin mTLS client identity, an origin SNI override,
or the origin TLS version policy must not influence the proxy
endpoint.  Callers that need a specific trust anchor for the proxy
must configure it explicitly via `Proxy(ssl_context=...)`; otherwise
the proxy endpoint is verified using rustls' default trust anchors.

`Proxy.__repr__` and `Headers.__repr__` redact the values of
`authorization`, `proxy-authorization`, `cookie`, and `set-cookie`
to `<redacted>` so credentials do not appear in diagnostic dumps.
The raw values remain available to protocol code through
`Proxy.headers` and to engine code through the native API.

### H2-only semantics and residual classification

`HttpVersionPolicy::Http2Only` is enforced at three layers: the rustls
ALPN list is restricted to `h2` only; the hyper-util legacy client is
built with `http2_only(true)` on the standard, direct, SNI, SOCKS, and
UDS paths; and `DirectStream` / `UdsStream` / `SocksStream` signal
ALPN `h2` back to hyper-util via `Connected::negotiated_h2()` so the
connection is correctly typed as H2. With all five in place, H2-only
over TLS correctly rejects H1 ALPN, and H2-only over cleartext sends
the H2 client preface directly over TCP (h2c prior knowledge).
H1-only and Auto policies must not regress; they do not set
`http2_only`. Do not introduce a parallel "HTTPX-only" client engine
or fork hyper to add H2 capability.

The `stream_id` metadata field on HTTP/2 responses remains a residual
difference. `h2 0.4.15` exposes `StreamId` on `ResponseFuture::stream_id()`
and `RecvStream::stream_id()`, but `hyper_util::client::legacy::ResponseFuture`
returns `Response<hyper::body::Incoming>` and `Incoming` wraps
`h2::RecvStream` privately, so the stream identifier is not reachable
through the current hyper abstraction. Document this as metadata-only;
do not synthesize a stream ID from request ordering, sequence numbers,
or any other source. See `docs/residual-differences.md` for the
classification rules separating protocol enforcement, h2c prior
knowledge, specialized transport, and metadata differences.

Current retained bounded differences are: `stream_id` metadata is unavailable;
HTTP/2 origin framing through an HTTP CONNECT proxy remains HTTP/1.1; and
HTTPX's four-element null-pointer `socket_options` form is rejected at the
safe Rust boundary. SSLContext state that rustls cannot represent likewise
fails closed before dispatch. Ordinary pooled responses expose no writable
network stream, internal CONNECT tunnels are not surfaced, and only 101
responses own an upgraded stream. Coroutine trace callbacks on AsyncClient
(and on sync `Client`) are rejected with a `TypeError` before dispatch
because the core `TraceObserver` is synchronous and core cannot await a
Python coroutine without unbounded reentrancy risk; sync callbacks work
on both sync `Client` and `AsyncClient`. The SNI override and SOCKS HTTPS
H2-only routes are now closed; they are recorded as `parity` rather than
residual in `compat/httpx/0.28.1/parity-cases.toml`.
