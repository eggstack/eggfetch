# Rust Development Skill

Use this skill when writing, modifying, or reviewing Rust code in the eggfetch workspace.

## Workflow

1. Read `AGENTS.md` for crate boundaries, lint policy, and quick commands.
2. Read `docs/architecture/dependency-policy.md` before adding any dependency.
3. Read `CONTRIBUTING.md` for coding conventions.

## Pre-commit Checklist

```sh
./scripts/check.sh              # Tier 1: routine validation (CI runs this)
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1
```

## Key Constraints

- `unsafe_code = "forbid"` workspace-wide, except `eggfetch-ffi` and `eggfetch-node` (sole exceptions for their FFI/N-API boundaries). Never add `unsafe` elsewhere. If you think you need it, stop and ask.
- All HTTP logic belongs in `eggfetch-core`. CLI, Python, FFI, and Node are adapters.
- No parallel synchronous networking path. Python sync blocks on async Rust engine.
- Public items need doc comments. For skeletal types, state which milestone fills in the real implementation.
- Never use `#![allow(warnings)]`, `#![allow(clippy::all)]`, or `#![allow(clippy::pedantic)]`.
- Use specific lint names. Justify suppressions with a comment.

## Feature Matrix Validation

Before committing changes to eggfetch-core, verify compilation across feature combinations:

```sh
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
cargo check -p eggfetch-core --all-features
```

## HTTP/3 Constraints

- Lifecycle lives in `crates/eggfetch-core/src/transport/http3.rs`:
  per-origin `OnceCell` (success cached, failures reconnectable), bounded
  64-entry cache, generation-scoped eviction (counted in
  `TransportMetrics`), shared connect budget with fair address shares, no
  transport-level retries.
- `Timeout.connect` bounds H3 DNS + QUIC + h3 init; `total` is the outer
  pipeline deadline; read/write apply at the body boundaries. QUIC idle
  derives from `PoolConfig::idle_timeout` (never `Timeout.pool`); bidi
  streams derive from the effective per-origin in-flight limit. Only
  `H3Connect` is retryable.
- Behavior tests: `cargo test -p eggfetch-core --all-features --test h3_hardening -- --test-threads=1`
  plus unit tests in `transport/http3.rs`. Keep the experimental label.

## Observability & Limits (native-protocol-observability)

- Trailers: `SharedTrailers` + `Response::trailers()` (no buffering; `None`
  until EOF/no-trailers/pre-trailer error). H1 duplicates collapse upstream
  (`insert`); H2 duplicates preserved. Tests: `trailer_tests.rs`.
- Metadata: direct 101 downcasts to `DirectStream` (real addrs/TLS);
  UDS reports `Unix` without IPs; opaque stays unavailable. No secrets,
  no fake zeros. Tests: `network_stream_tests.rs`.
- Metrics: `TransportMetrics` (connector/DNS/TLS, UDS/proxy, H3, upgrades)
  separate from `PoolMetrics` (logical). No Hyper reuse estimates.
  `Client::transport_metrics()`; exact-count tests in
  `transport_metrics_tests.rs`.
- Limits: `max_in_flight_requests*` preferred (logical); `max_connections*`
  are pre-1.0 aliases (new wins). Facade `Limits` unchanged.
- Trace: request/response headers emitted; DNS/connect/TLS via metrics
  (no duplicate taxonomy). Observer stays sync.

## Architecture References

- Overview: `docs/architecture/overview.md`
- Core engine: `docs/architecture/core-engine.md`
- Body & streaming: `docs/architecture/core-body-streaming.md`
- Timeouts & pool: `docs/architecture/core-timeout-pool.md`
- Auth, redirect & retry: `docs/architecture/core-auth-redirect-retry.md`
- TLS, proxy & protocols: `docs/architecture/core-tls-proxy-protocols.md`
- Cookies, multipart & compression: `docs/architecture/core-cookies-multipart-compression.md`
- Feature flags: `docs/architecture/feature-flags.md`
- Dependency policy: `docs/architecture/dependency-policy.md`

## HTTPX Compatibility

When working on the compatibility layer:

- **Typed API oracle**: `scripts/compare_httpx_api_manifest.py --validate` produces structured difference records gated by `allowed-differences.toml`.

### Compatibility test files

```sh
# Lossless merge semantics
python -m pytest crates/eggfetch-python/tests/compat/test_merge_lossless.py -v

# Native lifecycle and soak
python -m pytest crates/eggfetch-python/tests/compat/test_native_timeout_classification.py crates/eggfetch-python/tests/compat/test_soak.py -v

# Behavioral downstream fixtures
python -m pytest compat/downstream/behavioral_fixtures/ -v
```
