# Agent Guide

eggfetch is a Rust-native async HTTP client (tokio + hyper). All networking lives in `eggfetch-core` plus the small `eggfetch-http-connect` CONNECT wire primitive it owns; CLI, Python, FFI, and Node are thin adapters.
Start at `docs/architecture/overview.md` (§ Deep-Dive Index). Normative CI/release rules: `docs/verification-policy.md`. Task workflows: `.skills/` (`rust-development`, `python-bindings`, `cli-development`, `ffi-development`, `fuzz-testing`, `security-review`, `release-process`, `documentation`).

## Commands

```sh
./scripts/check.sh          # Tier 1: required before every commit (CI repeats it on ubuntu-latest)
./scripts/check.sh extended # Tier 2: before release (full compat, API oracle, feature matrix, MSRV, docs, FFI, soak)
./scripts/check.sh package  # Tier 3: before publish (crate packaging + wheel build/smoke)
./scripts/check_security.sh # Live RustSec/license/source preflight before publication
```

- `check.sh` refuses to run outside an active venv with Python 3.10+ and pinned tooling in `scripts/ci-requirements.txt`. Setup: `python3 -m venv .venv && source .venv/bin/activate && python -m pip install -r scripts/ci-requirements.txt`.
- After changing `crates/eggfetch-python` Rust code, rebuild before testing: `maturin develop -m crates/eggfetch-python/Cargo.toml`. Stale `.so` causes confusing failures.
- Focused equivalents (same flags `check.sh` uses):
  - `cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1`
  - `cargo test -p eggfetch-core --all-features <filter> -- --test-threads=1`
  - `python scripts/check_adapter_features.py` + `python scripts/test_validate_release_versions.py`
  - `python scripts/check_native_python_api.py` + `python scripts/check_python_typing_surface.py` + `python scripts/check_python_typing.py`
  - `python -m pytest crates/eggfetch-python/tests/ -q --ignore=crates/eggfetch-python/tests/compat`
  - `EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -v --strict-markers`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo fmt --all -- --check`
- Never parallelize Rust workspace tests (`--test-threads=1`): RSS-stabilization tests go flaky under concurrency.
- Tier 2 requires the exact Rust 1.89.0 toolchain (`rustup toolchain install 1.89.0 --profile minimal`); it fails, never skips. `qualification/` fixtures and H3 (experimental) are manual, never Tier 1 gates.

## Boundaries and lint

- `eggfetch-core`: no PyO3, no clap, no CLI parsing. `eggfetch-cli`/`eggfetch-python`: no direct hyper/tokio TCP — all I/O through core. No parallel sync networking path; Python sync blocks on the async engine with GIL released. If you write HTTP logic outside core, refactor.
- `eggfetch-http-connect` is the only exception to core-owned HTTP logic: generic CONNECT wire bytes only (target formatting, request serialization, bounded response-head parsing). No sockets, TLS, retry, or client policy; owned by the `proxy` feature, absent from non-proxy profiles.
- `unsafe_code = "forbid"` workspace-wide; only `eggfetch-ffi` and `eggfetch-node` override to `"allow"`. Never add new `unsafe` without explicit discussion.
- Pedantic clippy (`-D warnings`); `missing_docs = "warn"` (public items need doc comments). `scripts/check_lint_suppressions.sh` rejects blanket `allow`/`deny(warnings, clippy::all/pedantic/nursery/restriction)` (except FFI/Node `pedantic`). Use specific lint names with a justifying comment.
- Core default is `http1 + tls-rustls + tls-native-roots` (`http1` alone is cleartext-only). Canonical recipes: `docs/architecture/feature-flags.md#supported-core-profiles`.
  - `http1`/`http2` are aliases: `native-http1`/`native-http2` + `high-level-url` + `logical-retry` + `redirects` + `basic-auth`.
  - `native-http1`/`native-http2` are `transport-http1`/`transport-http2` + `standard-route` + `advanced-routing`. Without `high-level-url` there is no `url`/`idna`/ICU/`percent-encoding`; native callers own IDNA/punycode before constructing `http::Uri`.
  - Lean high-level `standard-http1`/`standard-http2` (+ `tls-rustls`): Bearer-only single-attempt standard-route client, 3xx returned without following. Leanest native: `transport-http1` + `standard-route` without `advanced-routing`.
  - CLI enables cookies/multipart/proxy (no compression, no http2/http3); Python enables http2/http3/cookies/multipart/proxy + all compressions.
- Core request rebuilds go through exhaustive typed helpers (`RequestParts::retry_request` needs `logical-retry`, `advance_redirect_hop` needs `redirects`) — a new field must fail to compile, never silently drop. Pipeline is `prepare.rs` → `route.rs` (`select_route()`: UDS → dialer → static/direct → proxy/SOCKS → SNI → H3 → standard) → `finalize.rs` (one post-transport policy for all routes), entered via `retry.rs` when `logical-retry` is present, `redirect.rs` when only `redirects` is present, or `lean.rs` single-hop when both are absent.
- Hyper client construction is centralized in `transport/hyper_client.rs`; keep route connectors/keys explicit, forward stays H1-only. UDS/Custom/Direct/SNI arms and their clients/caches, plus `dialer`/`local_address`/`socket_options`/`uds_path`/`resolved_addresses` builder methods and `Dialer`/`SocketOption` re-exports, require `advanced-routing` and are absent from lean profiles (pinned/SNI hints fail closed there).
- All origin TLS routes build through `TlsConfig`/`TrustStore`. `additional_ca_*` augments the base store; `ca_certificate_*` stays replacement-style. Native-root construction failure may fall back to WebPKI roots, but cert/hostname verification failure must never retry with another store. `crypto_provider()` is per-config, never process-global.
- Proxy fallback is typed only: CONNECT advances on 502/504 rejection, local SOCKS5 on destination-specific 0x03/0x04/0x05; auth/policy/protocol/malformed failures stop. Never store a request's shrinking total deadline in cached route connectors; enforce transport via the outer dispatch timeout and retain the absolute deadline behind the private `PoolGuard` response lifecycle through EOF/trailers (read starts on first poll/resets per chunk; total never resets, Total wins ties). `ResponseBody` public variant shapes are frozen — never add public timeout fields/variants or `#[non_exhaustive]`; `BodyTimeoutStream` stays the single high-level timeout owner.

## Python / HTTPX compat (easy to break)

- Shared setup lives in `crates/eggfetch-python/src/request_preparation.rs` (`prepare_client_config`/`apply_client_config`); `content=` takes one iterator, async-only bodies are rejected before sync dispatch. `_native` is private; public surface is `crates/eggfetch-python/python/eggfetch/__init__.py` + `__all__` (keep `crates/eggfetch-python/tests/native_api_manifest.json` in sync).
- `TransportHints` (`target`, `sni_hostname`, `resolved_target`, `trace`) override wire behavior only; they survive retry, redirect hops clear them except same-origin resolved destination. Native resolved-address pinning is Rust-only, never falls back to DNS.
- Facades `eggfetch.compat.httpx` (0.28.1) and `eggfetch.compat.httpx2` (2.12.0) coexist without cross-mutation. Never hand-edit generated API manifests — regenerate via `scripts/generate_httpx_api_manifest.py` + `scripts/compare_httpx_api_manifest.py`.
- Timeouts: map only `connect`/`read`/`write`/`pool`; never synthesize native `total`. Compat `NO_PROXY` parser differs from native `NoProxy::parse()` — do not unify. `ssl.SSLContext` translation is fail-closed (fingerprint mutation reclassifies, subclasses/unrepresentable state → `TypeError`); proxy TLS comes only from `Proxy(ssl_context=...)`.
- Redact `authorization`, `proxy-authorization`, `cookie`, `set-cookie` in all Debug/Display/`__repr__`/errors. `Http2Only` is enforced at three layers (ALPN, `http2_only(true)`, `negotiated_h2()`); never fork a second engine. Do not paper over `docs/residual-differences.md`.
- Only 101 responses own `response.extensions["network_stream"]`; CONNECT tunnels stay body-iterator only. SSE is Python framing over streams; WebSocket uses wsproto over the 101 stream — never raw sockets from Python.

## Working style

- Make the workspace green before adding functionality; run Tier 1 before committing. One logical change per commit; never commit without an explicit user request.
- Never add CI jobs, matrices, evidence schemas, or publish automation without an explicit request. CI is one job (`ci.yml`) repeating `check.sh`; releases are manual (order: http-connect → core → cli → ffi → python → node, then tag; PyPI via manually dispatched `pypi.yml`, Trusted Publishing). See `docs/verification-policy.md`.
