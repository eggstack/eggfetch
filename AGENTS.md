# Agent Guide

eggfetch is a Rust-native async HTTP client (tokio + hyper). All networking lives in `eggfetch-core`; CLI, Python, FFI, and Node are thin adapters.
Skills in `.skills/` carry task workflows (rust, python-bindings, cli, ffi, security-review, release-process, fuzz-testing, documentation). Architecture entry point: `docs/architecture/overview.md`.

## Commands

```sh
./scripts/check.sh          # Tier 1: required before every commit (CI repeats this on ubuntu-latest)
./scripts/check.sh extended # Tier 2: before release (full compat, API oracle, feature matrix, MSRV, docs, FFI, soak)
./scripts/check.sh package  # Tier 3: before publish (crate packaging + wheel build/smoke)

# Focused equivalents (same flags check.sh uses):
cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1
cargo test -p eggfetch-core --all-features <filter> -- --test-threads=1
python -m pytest crates/eggfetch-python/tests/ -q --ignore=crates/eggfetch-python/tests/compat
python -m pytest crates/eggfetch-python/tests/compat/<file> -v
EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

- `check.sh` refuses to run outside an active venv with Python 3.10+, maturin, pytest, pytest-asyncio. Setup: `python3 -m venv .venv && source .venv/bin/activate && python -m pip install maturin pytest pytest-asyncio`.
- After changing `crates/eggfetch-python` Rust code, rebuild before testing: `PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop -m crates/eggfetch-python/Cargo.toml`. Stale `.so` causes confusing failures.
- Never parallelize Rust workspace tests (`--test-threads=1`): resource-stabilization tests measure process RSS; concurrency makes them flaky.

## Boundaries and lint

- `eggfetch-core`: no PyO3, no clap, no CLI parsing. `eggfetch-cli`/`eggfetch-python`: no direct hyper/tokio TCP — all I/O through core. Hard rule: no parallel sync networking path; Python sync blocks on the async engine with GIL released. If you write HTTP logic outside core, refactor.
- `unsafe_code = "forbid"` workspace-wide; only `eggfetch-ffi` and `eggfetch-node` override to `"allow"`. Never add new `unsafe` without explicit discussion.
- Pedantic clippy (`-D warnings`); `missing_docs = "warn"`. `scripts/check_lint_suppressions.sh` (Tier 1) rejects `allow(warnings)`, `clippy::all`, `clippy::pedantic` (except inside FFI/Node), `clippy::nursery`, `clippy::restriction`. Use specific lint names with a justifying comment.
- Core default features: `http1 + tls-rustls` only. Everything else opt-in (`http2`, `http3`, `json`, `compression-{gzip,brotli,zstd,deflate}`, `cookies`, `proxy`, `multipart`, `tracing`, `test-util`). CLI enables cookies/multipart/proxy; Python enables http2/http3/cookies/multipart/proxy + all compressions. `test-util` = `tokio/test-util` for deterministic time tests.
- Core request reconstruction goes through typed helpers (`RequestParts::retry_request`, `into_request`, `advance_redirect_hop`) with exhaustive construction — a new field must fail to compile, never be silently dropped. `prepare_single_request()` + `select_route()` (UDS → direct → proxy/SOCKS → SNI → H3 → standard) share one post-transport policy for all routes.
- `Request` `TransportHints` (`target`, `sni_hostname`) override wire behavior without changing logical URL/routing; they survive retry, are cleared on redirect, and arrive from the compat facade via the request extensions dict through native `stream()`.
- HTTP/3 stays experimental (pinned h3 0.0.8, re-audit on bump). Graduation gate/blockers: `docs/architecture/core-tls-proxy-protocols.md` § "Production Graduation Decision"; workflow details live in the rust skill. Never make H3 qualification a Tier 1/CI gate.

## HTTPX compat (easy to break)

Facades `eggfetch.compat.httpx` (0.28.1) and `eggfetch.compat.httpx2` (2.12.0) coexist without cross-mutation; `compat/httpx/1.0-preview/` is reconnaissance only. Both are Stage C qualified on frozen SHA `78a77ea15...` (`compat/*/profile.toml`); any executable change invalidates qualification. Never hand-edit generated API manifests — regenerate via `scripts/generate_httpx_api_manifest.py` + `scripts/compare_httpx_api_manifest.py`.

- Timeouts: map only `connect`/`read`/`write`/`pool`; never synthesize native `total`. Preserve omitted-vs-`None`; reject bare `Timeout()`. Proxy setup uses one monotonic deadline (min of explicit `total` + phase budgets).
- `NO_PROXY`: compat env parser accepts bare unbracketed IPv6 but rejects bracketed/CIDR-looking forms; native `NoProxy::parse()` is richer. Do not unify.
- `ssl.SSLContext` → rustls: never heuristic-match CA stores by count/names; helper contexts carry a construction fingerprint (mutation drops it and reclassifies from live snapshot); passthrough contexts carry no `cert_path`/inherited trust; subclasses and unrepresentable state fail closed with `TypeError`. Proxy TLS comes only from `Proxy(ssl_context=...)`, never origin `verify`/CA/mTLS/SNI.
- Redact `authorization`, `proxy-authorization`, `cookie`, `set-cookie` in all Debug/Display/`__repr__`/errors via `eggfetch_core::redact`; raw values stay available to engine code.
- `Http2Only` enforced at three layers (ALPN `h2`-only, `http2_only(true)` on direct/SNI/SOCKS/UDS/standard, `Connected::negotiated_h2()`); h2c uses prior knowledge. Never fork a second engine for HTTPX.
- Do not paper over residuals (see `docs/residual-differences.md`): no synthesized `stream_id`, CONNECT-proxy origin framing stays H1.1, 4-element null-pointer `socket_options` rejected, H1 duplicate trailers collapse upstream while H2 duplicates preserved, sync trace callbacks ok but coroutines rejected with `TypeError`. `Proxy(headers=...)` rides the proxy leg only.
- SSE is Python framing over streamed responses; WebSocket uses wsproto over the core 101 `network_stream`. Never open raw sockets from Python or add a second Rust reader.

## Upgrades

Only 101 responses own a writable stream (`response.extensions["network_stream"]`); pooled responses are `None`, CONNECT tunnels are never surfaced (use body iterator). `start_tls` allows only inner-`Tcp` variants (reject `Adapter`/`Tls` before I/O). Sync `Client.stream()` yields `PyNetworkStream`, async yields `PyAsyncNetworkStream`; clones share one stream (Arc/Mutex). Honor leading rewind-buffer bytes on first reads.

## Working style

- Make the workspace green before adding functionality; run Tier 1 before committing. Keep commits to one logical change; never commit without an explicit user request.
- Never add CI jobs, matrices, evidence schemas, or publish automation without an explicit request. CI is one job (`ci.yml`) repeating `check.sh`; releases are manual (order: core → cli → ffi → python → node, then tag; PyPI via manually dispatched `pypi.yml`, Trusted Publishing). Verification tiers and complexity budget: `docs/verification-policy.md`.
