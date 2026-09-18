# Rust Development Skill

Use this skill when writing, modifying, or reviewing Rust code in the eggfetch workspace.

## Workflow

For multi-target proxy pins, preserve typed fallback boundaries: HTTPS CONNECT
advances only on typed 502/504 proxy rejection, and local SOCKS5 advances only
on destination-specific 0x03/0x04/0x05 replies; proxy-wide authentication,
policy, protocol, and malformed-response failures stop.

1. Read `AGENTS.md` for crate boundaries, lint policy, and quick commands.
2. Read `docs/architecture/dependency-policy.md` before adding any dependency.
3. Read `CONTRIBUTING.md` for coding conventions.

## Pre-commit Checklist

```sh
./scripts/check.sh              # Tier 1: routine validation (CI runs this)
```

`check.sh` already runs `cargo fmt --check`, lint-suppression policy,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`,
workspace tests single-threaded, the Python build/tests, the compat smoke
kernel, and the Node prototype check. Focused equivalents (same flags):

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1
```

## Key Constraints

- `unsafe_code = "forbid"` workspace-wide, except `eggfetch-ffi` and `eggfetch-node` (sole exceptions for their FFI/N-API boundaries). Never add `unsafe` elsewhere. If you think you need it, stop and ask.
- All HTTP logic belongs in `eggfetch-core`, except the shared CONNECT wire bytes owned by `eggfetch-http-connect` (target formatting, request serialization, bounded response-head parsing; no sockets, TLS, retry, or policy; enabled by the `proxy` feature, absent from non-proxy profiles). CLI, Python, FFI, and Node are adapters.
- No parallel synchronous networking path. Python sync blocks on async Rust engine.
- The opt-in `json` feature owns the direct `serde`/`serde_json` dependencies and provides replayable `RequestBuilder::json()` plus single-consume `Response::json()` helpers. JSON parsing is explicit and does not validate media type; body setters are last-call-wins. Per-request decoded-body limits override client limits across retries/redirects. Keep the default feature graph unchanged.
- `RequestBuilder::resolved_addresses()` is a native direct-routing escape hatch: it uses exactly the supplied socket addresses, preserves logical Host/TLS identity, and fails closed for proxy, UDS, H3, or cross-origin redirect combinations. It is not an SSRF policy or a Python compatibility extension.
- Native proxy route controls remain separate from direct static routing: `Proxy::resolved_addresses()` pins the physical proxy peer without changing logical proxy URI/TLS identity, while `RequestBuilder::proxy_target_addresses()` pins the ultimate HTTPS CONNECT or local-resolution SOCKS5 destination. Preserve both immutable snapshots through retries/same-origin redirects, reject cross-origin reuse and unsupported SOCKS5H/plain-forward combinations before I/O, include snapshots in reusable SOCKS route keys, and never add an Egress dependency or facade exposure for symmetry. See `docs/architecture/core-tls-proxy-protocols.md`.
- Cached forward-proxy and compatible single-target CONNECT clients/connectors contain only connection-scoped policy. Do not pass or retain a logical request's total/deadline state or add it to route keys; the current shrinking request budget is enforced by the outer proxy dispatch timeout. The cached CONNECT connector retains only the keyed SNI hint — never wire target overrides, trace observers, read/write budgets, retry/redirect state, bodies, or failure contexts. Preserve this ownership when changing pooling or stale-connection recovery.
- `TlsConfig` route identity is an opaque per-build token, never the root-store pointer. `Clone` shares it, `TlsConfigBuilder::build()` mints fresh, and `danger_accept_invalid_certs` mints new; proxy and CONNECT route keys carry the token so strict/weak clones never alias. Never render it in diagnostics or replace it with a structural digest. See `docs/architecture/core-tls-proxy-protocols.md`.
- Any new reusable route/client cache must answer the checklist in `transport::hyper_client` docs (connection identity, excluded request state, bound, secret handling, pool ownership, reconnect deadlines, isolation regression) and add key equality/isolation tests; false misses are acceptable, false hits are defects.
- `ClientBuilder::dialer()` is a native-only client-scoped raw-stream seam: eggfetch owns HTTP, destination TLS, logical Host/SNI identity, pooling, redirects, and retries. A request `target` override changes only the wire path/query. Custom dialing fails closed with proxy, UDS, resolved-target, local/socket routing, and H3; `DialError` source chains are preserved without delegating source `Debug` output into eggfetch diagnostics.
- `Client::execute_http_body()` is the additive native frame-preserving seam: it accepts a caller-owned `http_body::Body<Data = Bytes>`, returns an `http::Response<NativeResponseBody>`, and shares the existing Hyper routes/pool/TLS. It derives scheme/host/effective-port directly from `http::Uri` via `http_origin::HttpOrigin` and never reparses through `url::Url`; callers own IDNA/punycode conversion before constructing the URI. It intentionally omits high-level redirects/retries/cookies/auth/decompression and rejects the custom proxy/H3 routes before dispatch; a 101 upgrade is rejected after its status is received because upgrades cannot be identified before I/O. Native response read timeouts start on the first body poll and reset after each frame, matching the high-level streaming contract.
- `Client::native_service()` is the additive `tower_service::Service<http::Request<B>>` wrapper around `execute_http_body()`. Keep it cloneable, always-ready, and free of transport/policy duplication: origin-aware pool and transport backpressure occur in `call()`'s future, and arbitrary request extensions are not a guaranteed Hyper passthrough. Do not add the full Tower framework or Tonic to core; the external-style `qualification/native-tower-service/` fixture is manual and outside routine CI, pinned to Tonic 0.14.6 with `codegen` only (no Tonic `transport`/`Channel`, server, or TLS features).
- `Client::send_detailed()` and `RequestBuilder::send_detailed()` are additive native-only error entry points. Preserve the exact underlying `Error` and `Error::kind()` values; `RequestFailure` may add only evidence-backed `NetworkFailureKind` detail. Classify standard HTTP/HTTPS DNS through the crate-private resolver marker, refusal from typed `io::ErrorKind`, direct DNS/refusal from private connector provenance, and routes without structured evidence as generic/`None`. Never search display/debug text, change `Error`, or let transient retry/H3-fallback failures survive into the terminal result. The lean profile (no `logical-retry`/`redirects`) keeps this contract via `pipeline::lean::send_lean`; do not make detailed failure reporting depend on the policy features.
- Logical retry (`logical-retry`), redirect following (`redirects`), and Basic auth (`basic-auth`) are coarse capability features owned by the `http1`/`http2` compatibility aliases. `logical-retry` owns `dep:getrandom` + `dep:httpdate`; `basic-auth` owns `dep:base64` (proxy carries its own `dep:base64` edge); `multipart` shares `dep:getrandom` with retry. Request/policy rebuilds stay exhaustive (`RequestParts::retry_request` gated on `logical-retry`, `advance_redirect_hop` gated on `redirects`); `#[cfg]` on assignment statements is rejected by rustc — use the `set_*` setters for cfg'd fields. Never add a subtractive `minimal` feature; capability reduction is opt-in absence only.
- `TlsConfigBuilder::crypto_provider()` selects an `Arc<CryptoProvider>` locally without installing a process default; mTLS keys must load through that provider. `additional_ca_certificate_path/pem/der` augments the selected base trust store through the one authoritative TLS builder, while all existing `ca_certificate_*` and `add_ca_certificate_path` methods remain replacement-style. Keep origin and HTTPS-proxy trust configurations separate; see `docs/architecture/core-tls-proxy-protocols.md`.
- The external-style `qualification/native-http-body-tls/` fixture qualifies a separately supplied AWS-LC provider, mTLS key loading, private-root augmentation, custom dialing, frame bodies, and process-default isolation. It is outside routine workspace CI.
- Run that fixture manually after executable TLS/body changes; its `target/` directory is generated build output and must not be committed. Keep its alternate provider dependency isolated from the ordinary workspace graph.
- Public items need doc comments. For skeletal types, state which milestone fills in the real implementation.
- Never use `#![allow(warnings)]`, `#![allow(clippy::all)]`, or `#![allow(clippy::pedantic)]`.
- Use specific lint names. Justify suppressions with a comment.

## Feature Matrix Validation

Tier 2 (`tier2_feature_matrix` + `tier2_feature_tests` in `scripts/check.sh`)
is the authority. Before release it runs:

```sh
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,tls-native-roots
cargo check -p eggfetch-core --all-features
# plus tier2_feature_tests: gzip/brotli/zstd/deflate/proxy subsets
```

Tier 2 also runs the exact Rust 1.89.0 MSRV matrix: core with no features,
`http1`, and `http1,tls-rustls`, core with all features, and the complete
workspace with all targets/features. The MSRV toolchain is required for this
gate; install it with `rustup toolchain install 1.89.0 --profile minimal`.
Stable remains the normal development and CI toolchain, and the workspace
continues to use Edition 2021.

See `docs/architecture/feature-flags.md` for the exact validation matrix and
the supported core profile recipes. The
`http3` and `multipart,proxy` combos there are manual checks, not Tier 2
gates — do not add new CI combinations without explicit maintainer
approval (see `docs/verification-policy.md`). The `native-http1`/
`native-http2` slices without `high-level-url` are likewise manual
compile checks (`cargo check -p eggfetch-core --no-default-features
--features native-http1,tls-rustls`), not Tier 2 gates. The lean
high-level profile (`native-http1,high-level-url,tls-rustls` without
`logical-retry`/`redirects`/`basic-auth`) is also manual: check it plus
`cargo test -p eggfetch-core --no-default-features --features
native-http1,high-level-url,tls-rustls --test lean_policy_tests`; do not
promote it to a Tier 1/2 gate without approval.

Embedded footprint qualification (`qualification/embedded/` +
`scripts/qualify-embedded-footprint.sh` →
`docs/architecture/embedded-footprint.md`) is manual/bounded, never a CI
gate. The current record is not a footprint win; never claim slimming. The
native JSON helpers are opt-in and must remain absent from minimal profiles.

## HTTP/3 Constraints

- Lifecycle lives in `crates/eggfetch-core/src/transport/http3.rs`:
  per-origin `OnceCell` (success cached, failures reconnectable), bounded
  64-entry cache, generation-scoped eviction (counted in
  `TransportMetrics`), shared connect budget with fair address shares, no
  transport-level retries (draining only evicts for the *next* request).
- Discovery lives in `transport/alt_svc.rs` (separate `AltSvcCache` +
  `BrokenRouteSuppressor`, `h3`-only, `ma`/`clear`, SNI stays origin):
  `Auto { allow_http3: true }` discovers (no fresh entry = H1/H2),
  `Http3Only` is strict direct with no fallback/suppression; safe `Auto`
  fallback is pre-commit + replayable only via explicit `H3DispatchError`;
  GOAWAY draining uses pinned h3 0.0.8 `is_closing()`/`is_h3_no_error()`
  (feature `i-implement-...`, re-audit on bump).
- `Timeout.connect` bounds H3 DNS + QUIC + h3 init; `total` is the outer
  pipeline deadline; read/write apply at the body boundaries. QUIC idle
  derives from `PoolConfig::idle_timeout` (never `Timeout.pool`); bidi
  streams derive from the effective per-origin in-flight limit. Only
  `H3Connect` is retryable.
- Behavior tests: `cargo test -p eggfetch-core --all-features --test h3_hardening -- --test-threads=1`
  plus ` --test h3_alt_svc_discovery`, ` --test h3_interop_qualification`
  (loopback controls; external endpoints are supplied only by the
  qualification runner and absence is an explicit unsupported result) and
  unit tests in `transport/http3.rs`
  + `transport/alt_svc.rs`. Fuzz: `fuzz/fuzz_targets/fuzz_alt_svc.rs`
  (parser/cache + suppressor transitions). The deterministic H3 suites
  re-passed on the frozen executable recorded in the live ledger
  `plans/httpx-parity-correction-status.md`; earlier freeze SHAs in plan
  history are not the current binding. Do not hardcode a SHA here — read
  the ledger.
  The graduation gate and named blockers live in
  `docs/architecture/core-tls-proxy-protocols.md`
  (§ "Production Graduation Decision"). The implementation-neutral corpus
  and opt-in machine-readable runner are in `qualification/http3/` and
  `scripts/h3_qualification.py`; `scripts/h3_impairment.py` coordinates
  platform-specific netem runners. The evidence record is
  `plans/http3-production-qualification-evidence.md`; neither runner is part
  of Tier 1.

## Observability & Limits (native-protocol-observability)

- Trailers: `SharedTrailers` + `Response::trailers()` (no buffering; `None`
  until EOF/no-trailers/pre-trailer error). H1 duplicates collapse upstream
  (`insert`); H2 duplicates preserved. Tests: `trailer_tests.rs`.
- Metadata: direct 101 downcasts to `DirectStream` (real addrs/TLS);
  UDS reports `Unix` without IPs; opaque stays unavailable. No secrets,
  no fake zeros. Tests: `network_stream_tests.rs`.
- Metrics: `TransportMetrics` (connector/DNS/TLS, UDS/proxy, H3 +
  Alt-Svc learned/expired/cleared/rejected, H3 attempted/suppressed/fallback/
  drain/close/reconnect, upgrades)
  separate from `PoolMetrics` (logical). No Hyper reuse estimates.
  `Client::transport_metrics()`; HTTP/3 builds additionally expose bounded
  copied Quinn snapshots via `h3_diagnostics()` (remote address, RTT, path
  counters, route/generation, sanitized close code; no live handles or peer
  reason text). Stream counts remain unavailable (`None`) rather than guessed.
  Exact-count and bound tests live in `transport/metrics.rs` and
  `h3_alt_svc_discovery.rs`.
- Physical lifecycle: `PhysicalConnectionPolicy` is a native Hyper-route
  cap, separate from logical pool permits; permits remain held by active or
  idle Hyper connections and are shared by client clones. `TransportIoTimeout`
  guards established Hyper reads/writes, resets only on byte progress, and
  includes vectored writes plus pending flush/shutdown. The lifecycle wrapper
  is used by standard, direct/resolved, SNI, custom-dialer, UDS, and SOCKS
  Hyper clients, including ordinary forward-proxy and compatible CONNECT
  routes; multi-address handshakes and H3/QUIC are intentionally outside this
  policy. Deterministic lifecycle tests are in
  `transport/lifecycle.rs` and metrics snapshots include admission/live/high-
  water and read/write inactivity counters.
- **Underlying attempt control**: `ClientBuilder::retry_canceled_requests`
  defaults to `true` to preserve Hyper-util's transparent retry when a reused
  idle connection is unusable before transmission. Set it to `false` for
  strict native attempt accounting; this is independent of eggfetch's
  explicit `RetryPolicy`, applies to every Hyper HTTP/1/2 client route through
  `HyperClientPolicy` in `transport::hyper_client`, and has no meaning for
  the independent H3 path.
- **Route-client caches**: SNI/custom-SNI (256 entries) and resolved-route /
  SOCKS/forward/CONNECT (64 each) share `BoundedClientCache`
  get-or-build-and-evict mechanics; only the connectors and keys differ per
  route. The resolved key is origin + ordered addresses + exact SNI. Never add an LRU
  crate or adopt `hyper-util::client::pool` for this (qualified and rejected:
  unkeyed/unbounded/unnameable primitives). Keep capacities explicit,
  immutable-client policy out of keys, and construction under the cache lock
  free of network I/O. Every persistent Hyper family shares the resolved
  idle timeout (with the Hyper pool timer the timeout requires) and the
  effective per-host idle cap from `Pool`; forward stays H1-only.
- Limits: `max_in_flight_requests*` preferred (logical); `max_connections*`
  are pre-1.0 aliases (new wins). Facade `Limits` unchanged.
- Trace: request/response headers emitted; DNS/connect/TLS via metrics
  (no duplicate taxonomy). Observer stays sync.
- Native request-failure detail is an opt-in wrapper around the unchanged
  public `Error`; its network subtype is route-dependent evidence, not a
  complete transport taxonomy. Keep this surface separate from tracing and
  metrics, and never recommend matching error display strings.

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
- Benchmarks (Criterion suites, RSS monitor — not published): `docs/architecture/benchmarks.md`

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
