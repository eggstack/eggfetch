# Dependency Policy

eggfetch follows a conservative dependency policy. Every dependency must have an explicit reason. The project trades breadth of features for correctness, auditability, and a small transitive tree.

## Current Posture

eggfetch-core has the following direct dependencies. The `optional` marker
means the dependency is absent from the core dependency graph unless its
owning feature is selected.

- **eggfetch-http-connect** -- shared CONNECT wire primitive (authority
  formatting, request serialization, bounded response-head parsing). The
  only internal HTTP-logic exception to core ownership; it carries no
  sockets, TLS, retry, or policy, is published before `eggfetch-core`, and
  is owned by the `proxy` feature (absent from non-proxy profiles).
- **bytes** -- efficient byte buffer types for request and response bodies.
- **futures-core** -- `Stream` trait definition.
- **futures-util** -- `StreamExt` for stream combinators.
- **http** -- standard HTTP types (`Method`, `StatusCode`, `HeaderMap`, `Uri`).
- **http-body** -- body trait abstraction.
- **http-body-util** -- body combinators for http-body (`Full`, `Empty`).
- **hyper** -- HTTP/1.1 and HTTP/2 protocol implementation; protocol features
  are enabled by the matching `http1`/`http2` core features.
- **h2** -- typed HTTP/2 error reasons for transport error classification (optional, behind `http2`).
- **hyper-util** -- high-level client utilities built on hyper; H1/H2 protocol
  support follows the matching core feature.
- **hyper-rustls** -- optional TLS integration via Rustls (`tls-rustls`). The
  standard connector receives a policy-built `TlsConfig`; it does not construct
  a second native/WebPKI fallback.
- **pin-project-lite** -- lightweight pin projections for stream wrappers (read/write timeout streams).
- **rustls** -- optional memory-safe TLS implementation (`tls-rustls`).
- **tokio** -- async runtime.
- **tokio-rustls** -- optional async TLS streams for tokio + Rustls
  (`tls-rustls`).
- **url** -- high-level string-URL parsing, query serialization, redirects,
  cookies, proxy/no-proxy matching (optional, behind `high-level-url`;
  absent from minimal native transport slices).
- **percent-encoding** -- proxy auth decoding for high-level proxy URLs
  (optional, behind `high-level-url`; absent from minimal native slices).
- **thiserror** -- ergonomic error definitions.
- **cookie** -- RFC 6265 cookie parsing and representation (optional, behind `cookies` feature).
- **percent-encoding** -- percent-encoding for URL query strings and cookie values.
- **tower-service** -- `Service` trait for transport connector abstractions (UDS, SOCKS, connect-timeout wrappers) and the public `NativeHttpService` interoperability boundary. The full `tower` framework, `tower-layer`, and Tonic remain outside the core dependency graph; the manual Tonic 0.14.6 qualification fixture owns its `codegen`-only dependency separately.
- **base64** -- Basic auth credential encoding (optional, behind
  `basic-auth`; proxy auth carries its own `dep:base64` edge behind `proxy`).
  Bearer-only lean profiles omit core's direct `base64` edge.
- **flate2** -- buffered gzip/deflate decompression for non-streaming response reads (optional, behind `compression-gzip`/`compression-deflate`).
- **getrandom** -- cryptographically secure random bytes for retry jitter
  (optional, behind `logical-retry`) and multipart boundary generation
  (optional, behind `multipart`). Cargo unification keeps it when either
  capability is selected; builds with neither omit core's direct edge
  (transitive `getrandom` via ring/Rustls for TLS crypto remains).
- **httpdate** -- HTTP-date parsing for Retry-After header support
  (optional, behind `logical-retry`; absent from lean profiles).
- **pem-rfc7468** -- optional PEM parsing for custom CA bundles and client
  certificates (`tls-rustls`).
- **webpki-roots** -- optional packaged Mozilla/WebPKI root certificates
  (`tls-rustls`).
- **rustls-native-certs** -- optional platform-native certificate store loading
  (`tls-native-roots`, which implies `tls-rustls`). Linux/macOS portable PEM
  paths and other platform loaders are selected inside the same authoritative
  `TlsConfig` root builder.

The native frame API and `NativeHttpService` adapter add no dependency: they expose the existing direct
`http-body` 1.x contract and erases bodies at the existing Hyper boundary.
Likewise, `TlsConfigBuilder::crypto_provider` accepts a caller-owned Rustls
`CryptoProvider`; eggfetch does not add AWS-LC, FIPS, or another provider to
its ordinary feature graph merely to support injection. Alternate providers
belong to the embedding application's dependency/qualification profile. The
external-style `qualification/native-http-body-tls/` fixture carries AWS-LC
only in that isolated qualification crate, where it also proves mTLS key
loading; it is not a runtime dependency of `eggfetch-core`.

## Core dependency ownership inventory

This source-backed inventory records why each direct dependency exists and
which profile owns it. “Cleartext” means ordinary HTTP operation without
Rustls; a dependency can still be shared by other optional routes.

| Dependency | Core use sites | Cleartext H1 | Rustls TLS | Optional-only owner | Gating decision |
| --- | --- | ---: | ---: | --- | --- |
| `eggfetch-http-connect` | CONNECT wire bytes (`transport/connect`, itself `#[cfg(feature = "proxy")]`) | no | no | `proxy` | optional; owned by `proxy` to avoid a second CONNECT impl |
| `bytes` | bodies, headers, request/response types | yes | shared | no | foundational |
| `futures-core`, `futures-util` | streams, bodies, pipeline, retry | yes | shared | no | foundational |
| `http`, `http-body`, `http-body-util` | HTTP types and Hyper body adaptation | yes | shared | no | foundational |
| `hyper` | client protocol engine | H1 via `http1`; H2 via `http2` | shared when TLS | no | protocol features own H1/H2 flags |
| `hyper-util` | legacy client and Tokio integration | H1 via `http1`; H2 via `http2` | shared when TLS | no | protocol features own H1/H2 flags |
| `hyper-rustls` | standard HTTPS connector | no | yes | `tls-rustls` | optional; root policy is supplied by `TlsConfig` |
| `h2` | typed H2 error classification | no | H2 only | `http2` | optional |
| `rustls`, `tokio-rustls` | TLS config and async handshakes | no | yes | `tls-rustls` | optional |
| `webpki-roots` | deterministic bundled roots | no | yes | `tls-rustls` | optional but always available with Rustls |
| `rustls-native-certs` | system trust-store loader | no | yes | `tls-native-roots` | optional; default enables it |
| `pem-rfc7468` | custom CA and mTLS PEM parsing | no | yes | `tls-rustls` | optional with TLS API |
| `quinn`, `h3`, `h3-quinn` | QUIC/HTTP3 transport | no | H3 TLS | `http3` | optional; `http3` implies H1 + Rustls |
| `async-compression`, `tokio-util` | streaming decompression | no | no | compression features | optional per compression feature |
| `flate2` | buffered gzip/deflate decoding | no | no | gzip/deflate | optional |
| `brotli`, `zstd` | buffered codec support | no | no | brotli/zstd | optional |
| `cookie` | cookie jar | no | no | `cookies` | optional |
| `tracing` | opt-in diagnostics | no | no | `tracing` | optional |
| `getrandom` | retry jitter (`retry.rs`) and multipart boundaries (`multipart.rs`) | no | no | `logical-retry` + `multipart` (joint owners) | optional; unification keeps it when either is selected |
| `httpdate` | Retry-After parsing (`retry.rs`) | no | no | `logical-retry` | optional; absent from lean profiles |
| `base64` | Basic auth (`auth.rs`); proxy auth (`proxy.rs`, own edge) | no | no | `basic-auth` + `proxy` | optional; Bearer-only lean profiles omit the direct edge |
| `tower-service` | custom connector `Service` implementations | yes | shared | no | foundational |
| `pin-project-lite` | timeout/body stream projections | yes | shared | no | foundational |
| `percent-encoding` | high-level proxy auth decoding | no | no | `high-level-url` | optional; absent from native slices |
| `url` | high-level URL parsing, routing, pool keys, redirects/cookies/proxy | no | no | `high-level-url` | optional; native transport uses `http::Uri` via `http_origin::HttpOrigin` |
| `thiserror` | public error taxonomy | yes | shared | no | foundational |

The matrix intentionally does not gate tiny ubiquitous dependencies merely to
reduce crate count. `getrandom` is jointly owned by `logical-retry` and
`multipart` (unification keeps it when either is selected); `httpdate` is
owned by `logical-retry`; `base64` is owned by `basic-auth` plus the
`proxy` feature's own edge. No dependency upgrade was needed, so
the workspace MSRV is Rust 1.89.

These are small, well-audited crates with minimal transitive trees. Together
with the default feature set, they provide the dependencies required to build
a working HTTPS client.

Per-origin pool semaphores, Alt-Svc state, and the H3 sender cache use
standard-library `RwLock<HashMap<...>>` with short-lived locks (never held
across `.await` or I/O); no concurrent-map crate is in the dependency graph.

`eggfetch-http-connect` itself depends only on `tokio` (`io-util` for
`AsyncRead`/`AsyncWrite`/`BufReader`), `base64` (Basic auth encoding), and
`thiserror` (neutral protocol errors). It has no Hyper, TLS, DNS, socket,
retry, or routing dependencies by design.

Downstream size/dependency evidence for the minimal profiles lives in
[embedded-footprint.md](embedded-footprint.md) (manual qualification in
`qualification/embedded/`). Minimal trees verifiably exclude
cookies/proxy/compression/multipart/H2/H3, JSON, the proxy-owned
`eggfetch-http-connect` wire crate, and (in lean high-level profiles
without the policy features) core's direct `base64`/`httpdate`/`getrandom`
edges; serde/serde_json enter
only when the native json feature is selected. The minimal native transport
slice (`native-http1` + `tls-rustls` without `high-level-url`) additionally
excludes `url`, `idna`, ICU, and `percent-encoding`; the native
`http::Request` path derives scheme/host/effective-port from `http::Uri`
(`http_origin::HttpOrigin`) without reparsing through `url::Url`. The
leanest native slice (`transport-http1` + `standard-route` + `tls-rustls`,
without `advanced-routing`) additionally omits the advanced route
constructors/caches/dispatch arms at the code level; dependency closure is
otherwise the same as the minimal slice (no new dependency is added for
footprint reduction).
The exact supported core recipes and their excluded capabilities are listed in
[feature-flags.md](feature-flags.md#supported-core-profiles).

## Optional Later Dependencies

Features that are not core to HTTP/1.1 client behavior are optional and feature-gated:

- **pyo3**, **pyo3-async-runtimes** -- Python bindings (eggfetch-python crate only).
- **encoding_rs** -- charset decoding for non-UTF-8 responses (eggfetch-python crate only).
- **clap** -- CLI argument parsing (eggfetch-cli crate only).
- **serde**, **serde_json** -- native Rust JSON request/response helpers,
  optional behind the json feature and absent from non-JSON core profiles.
- **async-compression**, **tokio-util**, **brotli**, **zstd** -- streaming and buffered decompression for gzip, brotli, deflate, and zstd (optional, behind the respective compression features).
- **tracing** -- structured logging (optional, behind `tracing`).

These dependencies stay optional. They do not enter `default` features without discussion.

Note: `eggfetch-python` enables the full core profile (`cookies`,
`multipart`, `proxy`, all four compression codecs, `http2`, `http3`), so
everything listed above except `clap`/`tracing` enters the Python wheel's
dependency tree. The CLI enables only `cookies`, `multipart`, `proxy`.

## Python Compatibility Optional Dependencies (httpx2 SSE/WS)

The `httpx2` 2.12.0 streaming surface reuses the single Rust engine and
adds only narrow Python framing dependencies (plan
`httpx2-2.12-sse-and-websocket-parity.md` §§5/8; pinned in
`compat/httpx2/2.12.0/requirements.txt`):

- **wsproto** — WebSocket framing/state over the existing 101
  `network_stream`. Selected because it materially reduces framing
  correctness risk (masking, fragmentation/reassembly, ping/pong, close
  codes) without vendoring a second HTTP client or socket/TLS stack.
  Required only when the WS API is used; base client/SSE never import it
  at module load (`Client.websocket`/`connect_ws` raise a clear
  `ImportError` pointing at `httpx2[ws]` when absent).
- **anyio** — async WS session task-group/queue primitives for
  `AsyncWebSocketSession` (cancellation-safe background receive/keepalive).
- **truststore** — OS-trust default for httpx2 `create_ssl_context`
  (`verify=True`), falling back to certifi only when unavailable.

SSE itself has no extra dependency: `EventSource`/`ServerSentEvent` are
pure-Python framing over normal streamed responses.

## Selection Criteria

When evaluating a new dependency:

1. **Explicit reason required.** Every dependency must solve a real problem. "It might be useful" is not a reason.
2. **Prefer Rustls over native TLS.** Rustls is memory-safe, portable, and has a smaller audit surface than OpenSSL or platform-native TLS.
3. **Minimize transitive trees.** A convenience crate that pulls in 30 transitive dependencies needs a strong justification. Prefer direct, focused crates.
4. **Avoid proc-macro-heavy crates.** Proc macros slow compilation and expand the attack surface. Use them only when they materially improve correctness or maintainability (e.g., thiserror for error definitions).
5. **Keep features non-default.** Optional behavior behind feature flags. Users pay only for what they use.

## Audit Tools

The project uses:

- **cargo-deny** for license compliance, advisory database checks, duplicate dependency detection, and source restrictions. Configuration lives in `deny.toml` at the workspace root.
- **cargo-audit** for known vulnerability scanning against the RustSec advisory database.

These are **explicit live security-review tools**. They are not part of routine
CI or `./scripts/check.sh extended`, because their advisory databases are
time-dependent. The canonical fail-closed release/security gate is:

```sh
./scripts/check_security.sh
```

Findings should be addressed during security review, but these tools are not automatic merge or release gates under the simplified CI policy.

### Dependency-policy graph coverage

`deny.toml` audits the dependency families users can enable in supported
combinations, not just the default build:

- `[graph] all-features = true` is a valid superset for policy purposes:
  `eggfetch-core` features are additive with no mutually exclusive semantics,
  so full-feature scanning covers HTTP/1 + HTTP/2, Rustls/native roots,
  proxy/SOCKS, cookies, compression codecs, tracing, JSON, experimental
  HTTP/3 dependencies, and workspace crates where cargo-deny sees them.
  Runtime product profiles in [feature-flags.md](feature-flags.md) remain
  authoritative for behavior; the deny graph is dependency policy.
- HTTP/3 dependencies are included in scanning while HTTP/3 itself stays
  experimental; inclusion is not a graduation claim.
- `[graph] targets` covers the supported publication families: Linux x86_64,
  macOS x86_64/arm64, and Windows x86_64 (a supported wheel/release target
  per `docs/releases/process.md`). Linux AArch64 is omitted because it is
  not a documented release target. Do not add targets indiscriminately.
- `cargo-audit` lockfile scanning remains an independent RustSec check; deny
  all-features coverage does not replace it.

### Routine validation tooling pins

Routine CI installs exact Python tool versions from
`scripts/ci-requirements.txt` (`python -m pip install -r
scripts/ci-requirements.txt`), so a given eggfetch SHA always selects the
same maturin/pytest/pytest-asyncio/mypy. The maturin pin there must stay
aligned with `scripts/release-requirements.txt`; a maturin bump changes both
files in one reviewed diff. Update rule: tool bumps are reviewed repository
changes — bump one tool set deliberately, run `./scripts/check.sh` plus
affected packaging/type gates, and never auto-update validation tooling
independently of source review. No Dependabot/Renovate automation owns these
files.

Every dependency in the tree must have an explicit reason documented in code or review.

## Pool Key Semantics

The connection pool uses `OriginKey` as its concurrency-control key. The
key determines which requests share a concurrency slot and which get
independent slots. The key is **not** a transport-level connection
identifier — it controls logical concurrency permits, not TCP connection
reuse.

### Key Composition

The pool key is composed of:

- **`(scheme, host, port)`** for direct connections, where port uses the
  scheme's default when not explicit. `http://example.com:80` and
  `http://example.com` share a slot; `http://example.com` and
  `https://example.com` are independent.

When a proxy is involved, the key extends to:

- **`(proxy_origin, destination_origin, tunnel_mode)`**, where
  `tunnel_mode` is `true` for HTTPS CONNECT tunneling and `false` for
  HTTP forward proxying. (Conceptual summary: the struct also carries
  `proxy_scheme`, so plain vs TLS-to-proxy routes get independent slots.)
  This means:
  - Direct and proxied requests to the same destination have independent
    concurrency slots.
  - Different proxies sharing the same destination get independent slots.
  - HTTP forwarding and HTTPS CONNECT tunneling through the same proxy
    are keyed separately.

### Pool Permits (Concurrency Control)

Pool permits are semaphore-based concurrency tokens, not transport
handles. Acquiring a permit means "this request is allowed to proceed";
dropping the permit means "this request is done and another may start."
The pool does **not** track or manage TCP connections or TLS sessions.

Each permit holds:

- An optional global semaphore permit (total concurrent requests).
- An optional per-origin semaphore permit (per-`(scheme, host, port)`
  or per-`(proxy, destination, tunnel)` concurrency).

Permits are acquired before the request is sent and released when the
response body is fully consumed, explicitly closed, or dropped. Streaming
response bodies carry an `Arc<PoolGuard>` that holds the permits until
the body is consumed or dropped.

### Connection Reuse

Standard direct requests use Hyper's physical connection pool and may reuse
compatible TCP/TLS connections across requests. Each request:

1. Acquires a pool permit.
2. Reuses a compatible pooled connection or opens a TCP connection (and TLS
   handshake for HTTPS).
3. Sends the request and reads the response.
4. Releases the logical pool permit.

For proxy connections specifically:

Successful ordinary HTTP forward-proxy and compatible HTTPS CONNECT requests
use bounded Hyper client pools. Their connectors retain proxy DNS/pinning,
proxy TLS, CONNECT rejection parsing, origin TLS/SNI, and phase-aware setup
policy. Multi-address CONNECT fallback remains handshake-specific to preserve
typed 502/504 retry semantics. SOCKS routes use their own persistent Hyper
pools. Static resolved-destination requests use
an isolated direct client so ordinary DNS and incompatible static route
connections cannot cross the caller's constraint.

## Decoded-Body Limits

The client supports configurable limits on decoded response bodies to
prevent excessive memory consumption from large responses and compressed
responses that expand to large sizes.

- `max_decoded_body_size`: Hard limit on total decoded bytes. When
  exceeded during streaming or buffered reads, returns
  `Error::DecodedBodyTooLarge`. Default: unlimited.
- `max_decompression_ratio`: Optional limit comparing decoded bytes to
  compressed bytes. Applied once enough input has been observed to make
  a meaningful comparison. Default: unlimited.

These limits are enforced in both streaming and buffered paths. When a
limit is exceeded, the underlying response stream is discarded and the
pool lease is released.

## TLS root policy

When `tls-native-roots` is enabled, `hyper-rustls` is configured with native
and packaged WebPKI root support. Native roots are attempted first. The
packaged Mozilla roots are a construction fallback only when the native store
is unavailable; they are not tried after a certificate-chain or hostname
verification failure. With only `tls-rustls`, the packaged roots are selected
directly for deterministic operation without platform-store loading.
Enterprise or private CAs therefore require explicit configuration. Native
Rust callers can augment the selected base with
`additional_ca_certificate_path/pem/der`; compatibility facades retain their
replacement-style CA behavior and are not changed by this native API.
