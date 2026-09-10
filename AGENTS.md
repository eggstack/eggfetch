# Agent Guide

eggfetch is a Rust-native async HTTP client engine (tokio + hyper) with thin adapters: a CLI,
Python bindings (sync + asyncio), C FFI, and experimental Node.js prototype bindings. There is exactly one networking
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
Hints survive retry reconstruction (typed `RequestParts::retry_request`) and are cleared
on redirect hops (destination changed). The redirects-disabled fast path and the
redirect-enabled first hop share one `HopBuildParams` builder, so the two cannot diverge.
The Python compat facade passes `target`/`sni_hostname` from the request extensions dict
through the native `stream()` method.

## Core Pipeline Internals (for maintainers)

- `RequestParts` is the complete logical-request state; all retry/redirect reconstruction
  goes through typed helpers (`retry_request`, `into_request`, `advance_redirect_hop`)
  with exhaustive construction — a new field fails to compile rather than being dropped.
- `prepare_single_request()` normalizes a hop into an internal `PreparedRequest`
  (headers/body/version/URI/proxy/pool/deadlines); `select_route()` then picks one
  declarative `TransportRoute` (UDS → direct → proxy/SOCKS → SNI → H3 → standard).
  One common post-transport policy applies to every route.
- `transport/direct.rs` owns the shared Hyper response lifecycle
  (`finish_hyper_response` + trace helpers + `wrap_incoming` with
  `SharedTrailers` + `map_send_error` + `await_upgrade` with connector
  metadata); `transport/uds.rs` reuses the same helpers including 101
  upgrade handling (UDS reports `Unix`/`TlsUnix` without IPs).
- `transport/http3.rs` owns the QUIC lifecycle: `Vacant -> Connecting -> Ready ->
  Failed/Closed -> Evicted -> Reconnectable` via per-origin `OnceCell` (caches
  success only; failures stay reconnectable). Bounded 64-entry origin cache
  with generation-scoped eviction (in-flight streams survive, evictions
  counted in `TransportMetrics`); multi-address fallback under one shared
  connect budget with fair per-address shares; `Timeout.connect` bounds
  DNS + QUIC + h3 init, `total` stays the outer pipeline deadline,
  read/write apply at the body boundaries; QUIC idle derives from
  `PoolConfig::idle_timeout` (default 30 s, never `Timeout.pool`); bidi
  streams derive from the effective per-origin in-flight limit (default
  100); no transport-level retries (draining only evicts for the *next*
  request), one-shot bodies never replayed. Only `H3Connect` is retryable.
  Alt-Svc state lives separately in `transport/alt_svc.rs` (bounded 64-entry
  `AltSvcCache` + `BrokenRouteSuppressor`, `h3`-only, `ma`/`clear`, SNI stays
  origin); `Auto { allow_http3: true }` discovers (no fresh entry = H1/H2),
  `Http3Only` is strict direct with no fallback/suppression; safe `Auto`
  fallback is pre-commit + replayable only via explicit `H3DispatchError`;
  GOAWAY draining uses pinned h3 0.0.8 `is_closing()`/`is_h3_no_error()`
  (feature `i-implement-...`, re-audit on bump), marks generations draining,
  preserves close codes, reconnects next request. Test hooks are
  `test-util`-gated (`cache_len`, `contains_origin`, `is_draining_key`,
  `close_reason_key`); behavior tests live in `tests/h3_hardening.rs`,
  `tests/h3_alt_svc_discovery.rs`, and `tests/h3_interop_qualification.rs`
  (loopback-only unless `EGGFETCH_H3_INTEROP_URLS` is set; absence is an
  explicit skip, not evidence). HTTP/3 remains experimental; the graduation
  gate, exact pinned versions, manual spot-check procedure, and named
  blockers live in `docs/architecture/core-tls-proxy-protocols.md`
  (§ "Production Graduation Decision").
- Trailers: `SharedTrailers` is populated by `wrap_incoming` (H1/H2) and the
  H3 body unfold (via `recv_trailers`) without buffering; `Response::trailers()`
  is `None` until EOF, on no-trailers, or on pre-trailer body errors. H1
  duplicate same-name trailers collapse upstream in hyper (`insert`); H2
  duplicates are preserved. Python/FFI/Node defer trailer exposure; the
  HTTPX facade is unchanged (0.28.1 has no `trailers`).
- Metadata: direct-connector 101 upgrades downcast to `DirectStream` for real
  local/remote addrs + TLS version/cipher/ALPN; UDS upgrades report UDS kind
  without IPs; standard opaque upgrades stay explicitly unavailable
  (`None`, default kind). No secrets, no fake zeros. H3 per-response metadata
  stays `None`; `TransportKind::Quic` is reserved.
- Metrics: `TransportMetrics` (connector/DNS/TLS attempts, UDS/proxy, H3
  creations/evictions, Alt-Svc learned/expired/cleared/rejected, H3
  attempted/suppressed/fallback/drain/close/reconnect, 101 upgrades) is
  separate from `PoolMetrics`
  (logical waits/cancellations). Hyper reuse counts absent, never estimated.
  `Client::transport_metrics()` is the accessor; tests assert exact counts.
- Limits: native `max_in_flight_requests*` preferred (logical permits, not
  TCP counts); `max_connections*` are pre-1.0 aliases (new wins). Idle caps
  are physical Hyper policy. Facade `Limits(max_connections=...)` unchanged.
- Trace reconciled with metrics: request/response header events emitted
  consistently; DNS/connect/TLS observed via metrics to avoid duplicate
  taxonomy. Existing event names/phases compatible; observer stays sync.

## HTTPX Compatibility Layer

`eggfetch.compat.httpx` targets HTTPX 0.28.1 (asyncio only; Stage C qualified).
`eggfetch.compat.httpx2` targets httpx2 2.12.0 (sibling profile; stage in
`compat/httpx2/2.12.0/profile.toml`). The two facades coexist; importing one
never mutates the other. `compat/httpx/1.0-preview/` is reconnaissance only.

Core facade parity (`plans/httpx2-2.12-core-facade-parity.md`, done):
`FunctionAuth` (`_auth.py`), `Origin` + `URL.origin` (`_urls.py`), `QUERY`
(`_api.py` + `_client.py`), `Headers` `|`/`|=` (`_headers.py`), truststore
OS-trust default (`__init__.create_ssl_context`), IPv6 CIDR `NO_PROXY` fix
(versioned parser; 0.28.1 oddities preserved), decoder cap (native 4 vs
reference 5, intentionally stricter), multipart header validation
(core `try_header` before bytes), WSGI framing preservation, status
aliases with reference `DeprecationWarning` (`URL.raw` alone uses
`HTTPXDeprecationWarning`). Shared helpers are reused where semantics are
identical (`_asgi/_cookies/_mock/_request/_response/_stream/_transports/
_wsgi` re-exported from `httpx`); profile-specific behavior stays behind
explicit boundaries. SSE/WS belong to the next plan
(`httpx2-2.12-sse-and-websocket-parity.md`).

```python
from eggfetch.compat.httpx import Client, AsyncClient, Request, Response, URL, Headers, Cookies
```

```sh
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop -m crates/eggfetch-python/Cargo.toml
EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
```

The compatibility profile lives in `compat/httpx/0.28.1/` (`profile.toml`, API manifests,
`allowed-differences.toml`, `parity-cases.toml`). The qualification is bound to an exact
executable SHA recorded there; **any executable change invalidates it** and requires a fresh
exact-SHA requalification from a new freeze, following the current closure plan and status
procedure (see `plans/httpx-parity-correction-status.md`). Extended-tier
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
null-pointer `socket_options` form is rejected at the safe boundary. H1 duplicate
same-name trailers collapse upstream in hyper (`HeaderMap::insert`); H2 duplicates are
preserved — never synthesize collapsed values. Classification rules:
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

Deep dives live in `docs/architecture/` (filenames match topics); `overview.md` is the entry
point and links every deep dive. Index:

**Engine**
- `overview.md` — crate graph, module maps, request lifecycle summary, deep-dive index
- `core-engine.md` — Client/builder, pipeline lifecycle, error taxonomy
- `core-body-streaming.md` — body model, streaming adapters, pool-permit lifetime
- `core-timeout-pool.md` — phase-aware timeouts, semaphore pool, origin keying
- `core-auth-redirect-retry.md` — auth schemes, redirect policy, retry/backoff
- `core-tls-proxy-protocols.md` — TLS config, HTTP proxy/CONNECT/SOCKS, HTTP/2, HTTP/3
- `core-cookies-multipart-compression.md` — cookie jar, multipart encoder, decompression

**Adapters & tooling**
- `cli.md` — argument model, output modes, exit codes
- `python-bindings.md` — sync/async adapters, compat facade internals, exception hierarchy
- `ffi-and-node.md` — C ABI handles, runtime bridge, N-API prototype (Node explicitly experimental; contract lives in that doc)
- `testing-fuzzing.md`, `benchmarks.md` — test strategy, fuzz targets, Criterion suites

**Cross-cutting**
- `feature-flags.md` (validation matrix), `dependency-policy.md` (adding deps),
  `build-ci.md` (CI/lint/MSRV/release), `threat-model.md`,
  `security-findings.md`, `security-reviews.md`, `incident-runbook.md`,
  `release-security-checklist.md`

Residual-difference policy: `docs/residual-differences.md`. Verification tiers:
`docs/verification-policy.md`. Historical plans in `plans/` are non-normative records;
the live exact-SHA qualification ledger is `plans/httpx-parity-correction-status.md`.
