# Agent Guide

eggfetch is a Rust-native async HTTP client (tokio + hyper). All networking lives in `eggfetch-core` plus the small `eggfetch-http-connect` CONNECT wire primitive it owns; CLI, Python, FFI, and Node are thin adapters (Python sync blocks on the async engine with GIL released).
Start at `docs/architecture/overview.md` (§ Deep-Dive Index). Normative CI/release rules: `docs/verification-policy.md`. Conventions: `CONTRIBUTING.md`. Active work: `plans/README.md`. Task workflows: `.skills/`.

## Commands

```sh
./scripts/check.sh          # Tier 1: required before every commit (CI repeats it on ubuntu-latest)
./scripts/check.sh extended # Tier 2: before release (full compat, API oracle, feature matrix, MSRV, docs, FFI, soak, bench)
./scripts/check.sh package  # Tier 3: before publish (crate packaging + wheel build/smoke)
./scripts/check_security.sh # Live RustSec/license/source preflight before publication
```

- `check.sh` refuses to run outside an active venv with Python 3.10+ and pinned tooling in `scripts/ci-requirements.txt`. Setup: `python3 -m venv .venv && source .venv/bin/activate && python -m pip install -r scripts/ci-requirements.txt`.
- After changing `crates/eggfetch-python` Rust code, rebuild before testing: `maturin develop -m crates/eggfetch-python/Cargo.toml`. Stale `.so` causes confusing failures.
- Focused equivalents (same flags `check.sh` uses):
  - `cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1`
  - `cargo test -p eggfetch-core --all-features <filter> -- --test-threads=1`
  - `python -m pytest crates/eggfetch-python/tests/ -q --ignore=crates/eggfetch-python/tests/compat`
  - `EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -v --strict-markers`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo fmt --all -- --check`
- Never parallelize Rust workspace tests (`--test-threads=1`): RSS-stabilization tests go flaky under concurrency. Workspace tests exclude `eggfetch-python` (PyO3 builds separately via `maturin develop`).
- Tier 2 requires the exact Rust 1.89.0 toolchain (`rustup toolchain install 1.89.0 --profile minimal`); it fails, never skips. Full compat also needs pinned deps (`pip install -r compat/httpx/0.28.1/requirements.txt` + `compat/httpx2/2.12.0/requirements.txt`). The extended Rust public-surface oracle requires `cargo-public-api 0.52.0`, `cargo-semver-checks 0.49.0`, and the pinned nightly in `compat/rust-public-api/README.md` (six profiles vs planning baseline `03ecba973010e2858bf16a2b5f84d51ce70adae4`).
- `qualification/` fixtures and H3 (experimental) are manual, never Tier 1 gates. Tier 1 Node JS surface (`node test.js`) is an explicit SKIP when `node` or `crates/eggfetch-node/eggfetch.node` is absent, not a failure. `scripts/performance_benchmark.py` is non-CI timing evidence only.

## Boundaries and lint

- `eggfetch-core`: no PyO3, no clap, no CLI parsing. `eggfetch-cli`/`eggfetch-python`: no direct hyper/tokio TCP — all I/O through core. If you write HTTP logic outside core, refactor.
- `eggfetch-http-connect` is the only exception: generic CONNECT wire bytes only (target formatting, serialization, bounded head parsing). No sockets, TLS, retry, or policy; owned by the `proxy` feature.
- `unsafe_code = "forbid"` workspace-wide; only `eggfetch-ffi` and `eggfetch-node` override to `"allow"`. Never add new `unsafe` without explicit discussion.
- Pedantic clippy (`-D warnings`); `missing_docs = "warn"`. `scripts/check_lint_suppressions.sh` rejects blanket `allow`/`deny(warnings, clippy::all/pedantic/nursery/restriction)` (except FFI/Node `pedantic`). Use specific lint names with a justifying comment.
- Core default is `http1 + tls-rustls + tls-native-roots` (`http1` alone is cleartext-only). Canonical recipes: `docs/architecture/feature-flags.md#supported-core-profiles`.
  - `http1`/`http2` are aliases: `native-http1`/`native-http2` + `high-level-url` + `logical-retry` + `redirects` + `basic-auth`.
  - Without `high-level-url` there is no `url`/`idna`/ICU/`percent-encoding`; native callers own IDNA/punycode before constructing `http::Uri`.
  - Lean `standard-http1`/`standard-http2` (+ `tls-rustls`): Bearer-only single-attempt standard-route client, 3xx returned without following. `transport-http1` + `standard-route` without `advanced-routing` is the leanest native transport.
  - CLI enables cookies/multipart/proxy (no compression, no http2/http3); Python enables http2/http3/cookies/multipart/proxy + all compressions.
- Pipeline is `prepare.rs` → `route.rs` (`select_route()`: UDS → dialer → static/direct → proxy/SOCKS → SNI → H3 → standard) → `finalize.rs` (one post-transport policy), entered via `retry.rs` (`logical-retry`), `redirect.rs` (only `redirects`), or `lean.rs` single-hop when both are absent. New request fields must go through the exhaustive typed rebuild helpers (`retry_request` / `advance_redirect_hop`) so omission fails to compile. Hyper client construction is centralized in `transport/hyper_client.rs`; forward stays H1-only. UDS/dialer/pinning/SNI arms require `advanced-routing` and fail closed in lean profiles.
- Do not add public helpers beside existing Alt-Svc, lifecycle, metrics, dialer, or pool surfaces (inventory: `docs/architecture/rust-surface-containment.md`).
- TLS: `additional_ca_*` augments the base store, `ca_certificate_*` replaces it. Native-root construction failure may fall back to WebPKI roots; cert/hostname verification failure must never retry with another store. `crypto_provider()` is per-config, never process-global.
- Proxy fallback is typed only: CONNECT advances on 502/504, local SOCKS5 on destination-specific 0x03/0x04/0x05; auth/policy/protocol/malformed failures stop. Never cache a request's shrinking total deadline in route connectors; total is enforced by the outer dispatch and spans the `PoolGuard` body lifecycle through EOF/trailers (read = first-poll/per-chunk inactivity, total never resets, Total wins ties).
- `ResponseBody` public variant shapes are frozen — no new timeout fields/variants or `#[non_exhaustive]`; `BodyTimeoutStream` stays the single high-level timeout owner. Preserve chunking, cancellation, backpressure, decoding, and public surface when touching perf-sensitive paths.

## Python / HTTPX compat (easy to break)

- Shared setup lives in `crates/eggfetch-python/src/request_preparation.rs`; `content=` takes one iterator, async-only bodies are rejected before sync dispatch. `_native` is private; public surface is `python/eggfetch/__init__.py` + `__all__` (keep `tests/native_api_manifest.json` in sync).
- `TransportHints` override wire behavior only; they survive retry, redirect hops clear them except same-origin resolved destination. Native resolved-address pinning is Rust-only, never falls back to DNS.
- Facades `eggfetch.compat.httpx` (0.28.1) and `eggfetch.compat.httpx2` (2.12.0) coexist without cross-mutation. Never hand-edit generated API manifests — regenerate via `scripts/generate_httpx_api_manifest.py` + `scripts/compare_httpx_api_manifest.py`.
- Live qualification ledger `plans/httpx-parity-correction-status.md` (+ `compat/*/profile.toml`) is authoritative for the current Stage C SHA — never hardcode a SHA outside the canonical qualification records (`docs/residual-differences.md`, `docs/reference/compatibility*.md`). Do not paper over `docs/residual-differences.md`.
- Timeouts: map only `connect`/`read`/`write`/`pool`; never synthesize native `total`. Compat `NO_PROXY` parser differs from native `NoProxy::parse()` — do not unify. `ssl.SSLContext` translation is fail-closed (mutation reclassifies, subclasses/unrepresentable state → `TypeError`); proxy TLS comes only from `Proxy(ssl_context=...)`.
- Redact `authorization`, `proxy-authorization`, `cookie`, `set-cookie` in all Debug/Display/`__repr__`/errors. `Http2Only` is enforced at three layers (ALPN, `http2_only(true)`, `negotiated_h2()`); never fork a second engine.
- Only 101 responses own `response.extensions["network_stream"]`; CONNECT tunnels stay body-iterator only. SSE is Python framing over streams; WebSocket uses wsproto over the 101 stream — never raw sockets from Python.
- Python `aread()` builds the final `bytes` inside the GIL bridge (no Rust `Vec` copy); buffered iterators stay private and lazy. Cookie watermark shortcuts require isolated large-jar evidence showing a material win with unchanged semantics.

## Working style

- Make the workspace green before adding functionality; run Tier 1 before committing. One logical change per commit; never commit without an explicit user request.
- Never add CI jobs, matrices, evidence schemas, or publish automation without an explicit request. CI is one job (`ci.yml`) repeating `check.sh`; releases are manual (order: http-connect → core → cli → ffi → python → node, then tag; `eggfetch-bench` and `fuzz/` are never published; PyPI via manually dispatched `pypi.yml`, Trusted Publishing). See `docs/verification-policy.md`.
