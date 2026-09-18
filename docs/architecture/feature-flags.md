# Feature Flags

eggfetch-core uses feature flags to make protocol, TLS, and optional
request/response capabilities explicit. The default remains the ordinary
secure HTTP/1.1 client with Rustls, native roots preferred, and packaged
WebPKI roots available as a construction fallback. `--no-default-features`
is a supported API/build profile with no HTTP protocol implementation
selected; requests return a clear unsupported-feature error until `http1` or
`http2` is enabled.

## Current Features

The following features are declared in `crates/eggfetch-core/Cargo.toml`:

```toml
[features]
default = ["http1", "tls-rustls", "tls-native-roots"]
transport-http1 = ["hyper/http1", "hyper-util/http1", "hyper-rustls?/http1"]
transport-http2 = ["dep:h2", "hyper/http2", "hyper-util/http2", "hyper-rustls?/http2"]
standard-route = []
advanced-routing = []
native-http1 = ["transport-http1", "standard-route", "advanced-routing"]
native-http2 = ["transport-http2", "standard-route", "advanced-routing"]
standard-http1 = ["transport-http1", "standard-route", "high-level-url"]
standard-http2 = ["transport-http2", "standard-route", "high-level-url"]
http1 = ["native-http1", "high-level-url", "logical-retry", "redirects", "basic-auth"]
http2 = ["native-http2", "high-level-url", "logical-retry", "redirects", "basic-auth"]
high-level-url = ["dep:url", "dep:percent-encoding"]
logical-retry = ["dep:getrandom", "dep:httpdate"]
redirects = []
basic-auth = ["dep:base64"]
tls-rustls = ["dep:hyper-rustls", "dep:pem-rfc7468", "dep:rustls", "dep:tokio-rustls", "dep:webpki-roots", "hyper-rustls/ring", "hyper-rustls/logging", "hyper-rustls/tls12"]
tls-native-roots = ["tls-rustls", "dep:rustls-native-certs"]
http3 = ["http1", "tls-rustls", "dep:quinn", "dep:h3", "dep:h3-quinn", "high-level-url"]
json = ["dep:serde", "dep:serde_json"]
compression-gzip = ["dep:async-compression", "async-compression/gzip", "dep:tokio-util", "tokio/io-util", "dep:flate2"]
compression-brotli = ["dep:async-compression", "async-compression/brotli", "dep:tokio-util", "tokio/io-util", "dep:brotli"]
compression-zstd = ["dep:async-compression", "async-compression/zstd", "dep:tokio-util", "tokio/io-util", "dep:zstd"]
compression-deflate = ["dep:async-compression", "async-compression/deflate", "dep:tokio-util", "tokio/io-util", "dep:flate2"]
cookies = ["dep:cookie", "high-level-url"]
multipart = ["dep:getrandom"]
proxy = ["http1", "tls-rustls", "tokio/io-util", "high-level-url", "dep:eggfetch-http-connect", "dep:base64"]
tracing = ["dep:tracing"]
test-util = ["tokio/test-util"]
```

`http1`/`http2` are compatibility aliases: they preserve the existing
high-level string/URL API plus logical retry, redirect following, and Basic
auth. `transport-http1`/`transport-http2` are primitive Hyper protocol
slices; `standard-route` is the ordinary DNS -> TCP/TLS path;
`advanced-routing` owns custom Dialer, DirectConnector / local-address /
socket options, resolved-target/pinned-address cache, SNI-override cache,
and UDS. `native-http1`/`native-http2` preserve their historical behavior
(transport + both route capabilities) for low-level `http::Request`/
`http::Uri` embedding; selecting them without `high-level-url` omits the
`url`/`idna`/ICU closure. `standard-http1`/`standard-http2` are lean
high-level recipes (transport + standard route + URL API without advanced
routing and without the policy bundle). `logical-retry`, `redirects`, and
`basic-auth` are coarse capability features: the lean profile selects
`standard-http1` without them for a single-attempt Bearer-only standard-route
client. Native callers own any IDNA/punycode conversion before constructing
`http::Uri`.

## Default Features

```toml
default = ["http1", "tls-rustls", "tls-native-roots"]
```

The defaults advertise HTTP/1.1, Rustls TLS, and native-root loading. Cookies
are deliberately not enabled by the core default feature set; the Python
binding enables them for its public cookie API. `http1` can be selected alone
for a cleartext-only build. `tls-rustls` without `tls-native-roots` uses the
packaged WebPKI roots deterministically; `tls-native-roots` adds system-store
loading and is included in the default profile.

## Supported Core Profiles

These recipes make the intended capability boundary explicit. Unless noted,
the profiles exclude `http2`, `http3`, `json`, all compression features,
`cookies`, `multipart`, `proxy`, `tracing`, and `test-util`.

| Profile | Cargo recipe | Includes |
| --- | --- | --- |
| Ordinary default | `eggfetch-core = { version = "0.1" }` | H1, Rustls, native roots preferred with WebPKI construction fallback |
| H1 cleartext | `default-features = false, features = ["http1"]` | HTTP/1.1 only; HTTPS is unavailable |
| H1 deterministic HTTPS | `default-features = false, features = ["http1", "tls-rustls"]` | H1, Rustls, packaged WebPKI roots; no system-store loading |
| H1 native-root HTTPS | `default-features = false, features = ["http1", "tls-rustls", "tls-native-roots"]` | H1, Rustls, native roots with WebPKI construction fallback; explicit form of the default trust profile |
| H1 + native Rust JSON | `default-features = false, features = ["http1", "tls-rustls", "json"]` | Deterministic H1 HTTPS plus Serde request/response helpers; JSON is not in the default graph |
| H1 updater transport | `default-features = false, features = ["http1", "tls-rustls", "tls-native-roots", "proxy"]` | H1, Rustls, native roots with WebPKI fallback, explicit proxy routing (incl. opt-in `ProxyEnvironment`); excludes http2/http3/compression/cookies/multipart/json/tracing |
| H1 lean Bearer client | `default-features = false, features = ["standard-http1", "tls-rustls"]` | H1 standard-route, Rustls, packaged WebPKI roots, high-level URL API with Bearer auth, timeouts, body limits, pooling, TLS, and typed failures; omits advanced routing (`advanced-routing`: custom Dialer, resolved-target pinning, SNI override, local-address/socket options, UDS) and policy (`logical-retry`, `redirects`, `basic-auth`). 3xx returns without a second hop; each request dispatches once under the outer total deadline. |

Add `http2` to an H1/TLS profile for HTTP/2 ALPN and multiplexing. The
`http3` feature implies `http1` and `tls-rustls` but not `tls-native-roots`;
HTTP/3 remains experimental. The `proxy` feature likewise implies H1 and
Rustls because HTTPS proxy endpoints and tunnels need them. Add
`tls-native-roots` explicitly when those profiles should prefer the platform
trust store.

Native embedding slices (manual, not Tier 2 gates): replace `http1` with
`native-http1` (and `http2` with `native-http2`) and omit `high-level-url`
to build the `Client::execute_http_body`/`NativeHttpService` transport
without `url`, `idna`, ICU, or `percent-encoding`. Example:
`default-features = false, features = ["native-http1", "tls-rustls"]`.
Built-in proxy, cookies, HTTP/3, and the string-URL `RequestBuilder` API
require `high-level-url` and are unavailable in the minimal slice.
For the leanest standard-route native transport without advanced routing,
select `transport-http1` + `standard-route` (e.g.
`default-features = false, features = ["transport-http1", "standard-route", "tls-rustls"]`);
`native-http1` retains advanced routing for compatibility.

## Feature Reference

### http1

**Status:** implemented.
High-level HTTP/1.1 surface: `native-http1` transport plus `high-level-url`
string/URL API plus `logical-retry`, `redirects`, and `basic-auth` policy
capabilities. For the transport-only slice without `url`, select
`native-http1` directly. For a Bearer-only single-attempt standard-route
client without advanced routing or retry/redirect/Basic machinery, select
`standard-http1` + `tls-rustls`. `http1` alone (without TLS) is cleartext-only.

### http2

**Status:** implemented.
High-level HTTP/2 surface: `native-http2` transport plus `high-level-url`
plus `logical-retry`, `redirects`, and `basic-auth` (same policy bundle as
`http1`).
When enabled, the client can negotiate HTTP/2 via ALPN for HTTPS connections. The `HttpVersionPolicy` enum controls which protocol versions are advertised. `Auto` (default) advertises both `h2` and `http/1.1`; `Http2Only` advertises only `h2`; `Http1Only` advertises only `http/1.1`. Without this feature, `Http2Only` and `Auto` silently downgrade to `Http1Only`. The Python crate exposes `Client(http2=True)` and `AsyncClient(http2=True)` for enabling HTTP/2 negotiation, and `Client(http1=False, http2=True)` / `AsyncClient(http1=False, http2=True)` for HTTP/2-only prior-knowledge mode.

### native-http1 / native-http2

**Status:** implemented.
Low-level transport slices for `http::Request`/`http_body::Body` embedding
(`Client::execute_http_body`, `NativeHttpService`). They preserve the
historical behavior (primitive transport + standard route + advanced
routing) without the high-level URL layer. Select without `high-level-url`
for minimal embedding builds without `url`/`idna`/ICU. Existing
`http1`/`http2` names retain their high-level behavior; use the `native-*`
names only when explicitly omitting the URL layer but retaining advanced
routing. For standard-route-only native transport without advanced routing,
select `transport-http1`/`transport-http2` + `standard-route` directly.

### transport-http1 / transport-http2

**Status:** implemented.
Primitive Hyper protocol slices owning only the `hyper`/`hyper-util`/
`hyper-rustls` protocol flags. Every H1/H2 route builds on these. Combined
with `standard-route` (and optionally `high-level-url` via `standard-http1`/
`standard-http2`) for lean standard-route clients, or with `native-http1`/
`native-http2` for full compatibility profiles.

### standard-route / advanced-routing

**Status:** implemented.
Routing capability markers. `standard-route` is the ordinary DNS -> TCP/TLS
Hyper path (typed DNS/refused/connect provenance, connect/total/read/write
timeouts, pooling, TLS verification, body caps, cancellation).
`advanced-routing` owns custom `Dialer`, DirectConnector / local-address /
socket-option route, resolved-target/pinned-address cache, SNI-override
cache, and UDS. `native-http1`/`native-http2` enable both; the lean profile
enables only `standard-route`. Advanced builder methods (`dialer`,
`local_address`, `socket_options`, `uds_path`, `resolved_addresses`) and the
`Dialer`/`SocketOption` re-exports are absent without `advanced-routing`;
pinned/SNI hints supplied via `TransportHints` fail closed with
`Unsupported` in lean profiles.

### standard-http1 / standard-http2

**Status:** implemented.
Lean high-level recipes: `transport-http1`/`transport-http2` + `standard-route`
+ `high-level-url`, without `advanced-routing` and without the
`logical-retry`/`redirects`/`basic-auth` policy bundle. Select with
`tls-rustls` for HTTPS. Each request dispatches once (`pipeline::lean::send_lean`)
under the outer total deadline; 3xx returns without following and with empty
history. Keeps Bearer auth, timeouts, body limits, pooling, TLS, and typed
failures.

### high-level-url

**Status:** implemented.
High-level string/URL request semantics backed by `url` (plus
`percent-encoding` for proxy auth decoding). Required by `Request`,
`RequestBuilder`, `Response`, redirects, cookies, and built-in proxy/HTTP/3
routing. Native callers provide a valid `http::Uri` and own any
IDNA/punycode conversion; the native path performs none.

### logical-retry

**Status:** implemented.
Logical retry orchestration: `RetryPolicy`/`RetryPolicyBuilder`,
`BackoffPolicy`/`MethodPolicy`/`StatusPolicy`, backoff/jitter
(`getrandom`), `Retry-After` HTTP-date parsing (`httpdate`), replay checks,
and the `ClientBuilder::retry` / `RequestBuilder::retry` /
`without_retry` APIs plus the `pipeline::retry` loop. Enabled by the
`http1`/`http2` compatibility aliases; omitted by the lean profile, which
dispatches once under the outer total deadline. Hyper's distinct
canceled-idle-request retry (`retry_canceled_requests`) is transport policy
and remains available in all profiles.

### redirects

**Status:** implemented.
Redirect-following loop, hop reconstruction (`advance_redirect_hop`),
history (`Response::history`), and the `RedirectPolicy` /
`ClientBuilder::follow_redirects` / `max_redirects` / `redirect_policy` /
`RequestBuilder::redirect_policy` APIs. Enabled by the `http1`/`http2`
compatibility aliases; omitted by the lean profile, which returns 3xx
responses as ordinary responses with no second hop and no history.

### basic-auth

**Status:** implemented.
Basic auth (`BasicAuth`, `AuthScheme::basic`, `AuthScheme::Basic`) and the
core `base64` dependency. Enabled by the `http1`/`http2` compatibility
aliases; omitted by the lean Bearer-only profile. Bearer auth
(`BearerAuth`, `AuthScheme::bearer`) needs no Base64 and remains available
without this feature. Proxy auth (`ProxyAuth`, CONNECT helpers) is owned
by the `proxy` feature's own `base64` edge plus the separate
`eggfetch-http-connect` crate.

### http3

**Status:** implemented (experimental).
Enables HTTP/3 support over QUIC. When enabled, the client can negotiate HTTP/3 using the `quinn` crate for QUIC transport and the `h3` crate for the HTTP/3 protocol layer. `HttpVersionPolicy::Http3Only` routes direct QUIC (strict); `Auto { allow_http3: true }` discovers via authenticated Alt-Svc (no fresh entry = H1/H2). QUIC mandates TLS 1.3, so this feature explicitly implies `http1` and `tls-rustls` for discovery/fallback and TLS. 0-RTT (early data) is disabled. The Python crate exposes `Client(http3=True)` and `AsyncClient(http3=True)`, plus `H3Error`, `H3ConnectError`, and `H3ProtocolError` exception types. This feature is experimental; API surfaces may change as the QUIC/h3 ecosystem matures.

Hardened lifecycle policy (see [core-tls-proxy-protocols.md](core-tls-proxy-protocols.md)): bounded 64-entry per-origin QUIC cache with `OnceCell`-shared init and stale-eviction reconnect, multi-address fallback under one shared connect budget, phase-correct connect/total/read/write timeouts, keepalive-derived QUIC idle (default 30 s), pool-derived stream caps, no transport-level retries, plus separate bounded Alt-Svc cache/suppressor, pre-commit replayable-only safe fallback via `H3DispatchError`, and GOAWAY draining (pinned h3 0.0.8 `is_closing()`/`is_h3_no_error()`). `TransportMetrics::h3_diagnostics()` exposes bounded copied Quinn snapshots for HTTP/3 builds; unavailable stream counts are reported as `None`. Tests: `tests/h3_hardening.rs` + `tests/h3_alt_svc_discovery.rs` + `tests/h3_interop_qualification.rs` (GET/HEAD/upload/streaming/multiplex/trailers/early-close/soak/boundedness) plus unit tests in `transport/http3.rs` + `transport/alt_svc.rs` + `transport/metrics.rs` (loopback only). The implementation-neutral corpus, independent-server runner and qualification-only impairment matrix live in `qualification/http3/` and `scripts/`; external absence is an explicit unsupported result. Fuzz: `fuzz/fuzz_targets/fuzz_alt_svc.rs` (parser/cache + suppressor transitions). Graduation: retained experimental this milestone (see "Production Graduation Decision" in core-tls-proxy-protocols.md).

### tls-rustls

**Status:** implemented.
Enables the Rustls transport configuration. Native roots are preferred at
runtime and packaged WebPKI roots are used only when native roots are
unavailable. Verification failures never trigger the fallback. This feature
also supports custom CA bundles via `TrustStore`,
client certificates via `ClientIdentity`, TLS version policy via `TlsVersion`,
verification toggle via `TlsConfigBuilder::danger_accept_invalid_certs(true)`,
SNI configuration, and additive native roots via
`TlsConfigBuilder::additional_ca_certificate_path/pem/der`. The Python crate exposes `verify=` and `cert=` kwargs
for TLS configuration. The Rustls transport and its configuration dependencies
are enabled only by `tls-rustls`. Without that feature, HTTPS is rejected at
the transport boundary; cleartext H1 remains available when `http1` is
selected.

### tls-native-roots

**Status:** implemented. Implies `tls-rustls` and enables system/native trust
store loading. The default profile includes it. Without this feature,
`NativeOnly` fails clearly at TLS configuration construction and the default
`NativeWithWebPkiFallback` policy resolves to packaged WebPKI roots. The
native store is never tried after certificate-chain or hostname verification
failure.

### json

**Status:** implemented (optional). Enables serde and serde_json and adds
RequestBuilder::json() plus Response::json(). Request JSON is serialized
into a replayable byte body and sets Content-Type: application/json only
when the caller has not supplied a content type. Response JSON consumes the
body once through the normal decoded-body/limit path and does not require a
particular media type. JSON errors have distinct json_serialize and
json_deserialize kinds. The feature remains opt-in and is not in default;
Python continues to expose its own json.dumps() boundary. The native
`Response::json()` path does not cache or re-read the body, and request-level
decoded-body limits are available independently of this feature.

### Resolved destination routing

This is a native API capability rather than a Cargo feature. A request can use
RequestBuilder::resolved_addresses() to provide a non-empty set of validated
SocketAddr values. Direct routing uses exactly that set without DNS and
preserves logical URL/Host/TLS identity. Same-origin redirects retain the
snapshot; cross-origin redirects, proxies, UDS, and HTTP/3 fail closed. The
caller owns address validation and redirect policy; this primitive is not an
SSRF policy engine.

### compression-gzip

**Status:** implemented.
Enables gzip decompression of response bodies. This is behind a feature flag to avoid pulling in compression dependencies for users who do not need them. Uses `async-compression` for streaming decode and `flate2` for buffered decode. Enables `Content-Encoding: gzip` transparent decompression.

### compression-brotli

**Status:** implemented.
Enables Brotli decompression of response bodies. Uses `async-compression` for streaming decode. Enables `Content-Encoding: br` transparent decompression.

### compression-zstd

**Status:** implemented.
Enables Zstandard decompression of response bodies. Uses `async-compression` for streaming decode. Enables `Content-Encoding: zstd` transparent decompression.

### compression-deflate

**Status:** implemented.
Enables deflate decompression of response bodies. Uses `async-compression` for streaming decode. HTTP deflate is typically zlib-wrapped; this decoder handles the standard format. Enables `Content-Encoding: deflate` transparent decompression.

### cookies

**Status:** implemented.
Enables cookie jar support for persistent cookies across requests. Provides RFC 6265 cookie parsing, domain/path matching, cookie jar with thread-safe storage, and automatic Set-Cookie ingestion on responses. The Python crate exposes `client.cookies`, `response.cookies`, and a `cookies=` kwarg for initial cookies.

Python request-local `cookies=` values are serialized into the request header,
are not persisted in the client jar, and are removed on cross-origin redirects.

### multipart

**Status:** implemented.
Enables streaming multipart/form-data request bodies. Provides `Multipart`, `Part`, `PartBody`, and `Boundary` types with a builder API, a streaming encoder backed by a state machine, known-length calculation when all parts have known sizes, boundary generation and validation, and per-part headers and content types. The Python crate exposes `files=` kwarg support including bytes, tuples, path-backed `File` wrapper, and mixed `data=` + `files=`.

Python `files=` accepts bytes, `(filename, data)` tuples, `(filename, data, content_type)` triples, `(filename, data, content_type, headers)` quads, and `eggfetch.File(path)` objects. Files are read via synchronous std::fs (blocking in GIL context) for path-backed parts. Cancellation safely drops file handles and streams.

Boundary generation uses `getrandom` to seed its internal xorshift PRNG.
The feature owns `dep:getrandom` jointly with `logical-retry` (Cargo
unification keeps randomness when either capability is selected); builds
with neither feature omit core's direct `getrandom` edge (transitive
`getrandom` via ring/Rustls for TLS crypto remains).

### proxy

**Status:** implemented.
Enables HTTP proxy and SOCKS5 proxy support in eggfetch-core. The feature
implies `http1` and `tls-rustls` because HTTPS proxy endpoints, CONNECT
tunnels, and HTTPS-over-SOCKS require the Rustls transport. Provides HTTP
proxying, HTTPS CONNECT tunneling, SOCKS5 tunneling (socks5:// and socks5h://),
proxy authentication, per-request and per-client proxy configuration via
`ClientBuilder::proxy()` and `RequestBuilder::proxy()`, and `NO_PROXY`-style
bypass behavior. The Python crate exposes `Client(proxy=...)`,
`AsyncClient(proxy=...)`, and per-request `proxy=` kwarg. The feature flag is
required for proxy functionality; it pulls in tunnel and proxy-protocol
dependencies, including the shared `eggfetch-http-connect` CONNECT wire
crate (absent from non-proxy profiles) and its own `dep:base64` edge for
`Proxy-Authorization` encoding (independent of `basic-auth`).

### tracing

**Status:** implemented (optional `tracing` dependency gate).
Enables structured logging via the tracing ecosystem. This is opt-in to avoid pulling in logging dependencies for users who do not need them.

### test-util

**Status:** implemented.
Enables `tokio/test-util` for deterministic time testing. This feature is for internal use only and should not be enabled by downstream consumers. It allows tests to control time progression for timeout-related scenarios.

## Rules

- Do not add a feature just to silence a clippy lint.
- Do not enable optional behavior in `default` without discussion.
- Every feature must have a clear purpose and be documented here.
- Features that are not core to HTTP/1.1 client behavior stay optional.

## Validation matrix

The repository validates the following core combinations via Tier 2
(`./scripts/check.sh extended`: `tier2_feature_matrix` + `tier2_feature_tests`)
before a release. This list must match `scripts/check.sh` exactly — do not
add combinations here without updating the script (and vice versa):

```text
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,tls-native-roots
cargo check -p eggfetch-core --all-features
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-gzip
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-brotli
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-zstd
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,compression-deflate
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
```

Manual (not Tier 2 gates) compile checks for other combinations:

```text
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,multipart,proxy
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
cargo check -p eggfetch-core --no-default-features --features transport-http1,standard-route,high-level-url,tls-rustls
cargo check -p eggfetch-core --no-default-features --features standard-http1,tls-rustls
cargo test -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --test lean_route_tests
cargo test -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --test lean_policy_tests
```
