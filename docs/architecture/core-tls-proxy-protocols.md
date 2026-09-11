# TLS, Proxy & Protocols Deep Dive

This document covers TLS configuration, HTTP proxy support, HTTP/2, and HTTP/3.

See also: [overview.md](overview.md), [core-engine.md](core-engine.md).

## TLS Configuration

### TlsConfig

Builder-pattern configuration for TLS behavior:

```rust
let tls = TlsConfig::builder()
    .trust_store(TrustStore::from_pem_file("ca-bundle.pem")?)
    .client_identity(ClientIdentity::new("cert.pem", "key.pem")?)
    .min_version(TlsVersion::Tls12)
    .danger_accept_invalid_certs(false)
    .build()?;
```

### Trust Store Hierarchy

Resolution order:
1. Custom `TrustStore` (if provided via `TlsConfig`)
2. Operating system native roots (if available)
3. Packaged Mozilla/WebPKI roots (fallback for minimal containers)

A custom CA bundle **replaces** all default roots. If both system and private CAs are needed, concatenate them into a single PEM file.

httpx2 2.12.0 changed its default verification to OS truststore behavior
(`truststore.SSLContext`); native Rust already follows the same
system-roots-then-WebPKI order, so no native default was weakened. The
0.28.1 facade keeps its qualified `certifi`-default translation; the
httpx2 facade exposes a separate `create_ssl_context(verify=True)` that
prefers `truststore` and falls back to certifi only when unavailable.
Explicit `verify`/`SSLContext`/`cert` conflicts and unrepresentable
`SSLContext` state continue to fail closed; proxy-endpoint TLS stays
isolated from origin trust.

### Client Identity (mTLS)

`ClientIdentity` holds a certificate chain and private key for mutual TLS. Supports PEM-encoded chains. Unencrypted PEM private keys are supported; encrypted keys produce an error at construction time.

### Verification Toggle

`TlsConfigBuilder::danger_accept_invalid_certs(true)` disables certificate verification. This is a deliberate escape hatch with documentation warnings.

### Version Policy

`TlsVersion` configures minimum and maximum supported TLS versions (e.g., TLS 1.2, TLS 1.3). QUIC mandates TLS 1.3.

## HTTP Proxy

### Configuration

`Proxy` supports HTTP forwarding and HTTPS CONNECT tunneling:

```rust
let proxy = Proxy::http("http://proxy.example.com:8080")?;
let client = ClientBuilder::new().proxy(proxy).build()?;
```

Per-request override via `RequestBuilder::proxy(ProxyOverride)`:
- `Inherit` — use client proxy (default).
- `Direct` — bypass proxy.
- `Override(proxy)` — use different proxy.

### Proxy-Only Headers

`Proxy::proxy_headers(headers)` attaches headers that are sent to the
proxy endpoint on forward-proxy and CONNECT requests but are never
forwarded into the tunnel or to the origin server:

```rust
let mut headers = Headers::new();
headers.insert("X-Custom-Proxy", "value")?;
let proxy = Proxy::http("http://proxy:8080")?
    .proxy_headers(headers);
```

Proxy-only headers are applied as follows:
- **HTTP forward proxy**: headers appear on the absolute-form request
  to the proxy.  Duplicate names from the origin request are not
  repeated.
- **CONNECT tunnel**: headers appear on the `CONNECT` request to the
  proxy.  After tunnel establishment, the origin request uses only the
  origin header set.
- **SOCKS proxy**: there is no HTTP proxy-header leg, so proxy headers are not
  forwarded (matching HTTPX 0.28.1 reference behavior). HTTP and HTTPS proxy
  legs do support `Proxy(headers=...)`.

### Proxy Authentication

`ProxyAuth` supports Basic and Bearer authentication for proxy connections.

### NO_PROXY

`NoProxy` bypass rules support:
- Wildcard (`*` — bypass all)
- host/domain and host:port entries
- IPv4/IPv6 literals and CIDR networks

The Rust core does not read proxy environment variables. The HTTPX Python
compatibility facade may translate `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`,
and `NO_PROXY` into explicit scheme-aware native proxy configuration when
`trust_env=True`; native Rust callers must configure proxies explicitly. The
facade follows HTTPX's `urllib.request` environment precedence and matching:
lowercase names win, scheme-less proxy values are treated as HTTP URLs, and
`localhost` is an exact hostname rule rather than an implicit loopback alias.
Scheme-qualified exclusions constrain scheme and optional port; HTTPX-compatible
bare unbracketed IPv6 is recognized without treating its final colon as a port
separator. Bracketed IPv6 and IPv6 prefix-looking environment values are
rejected before native routing because HTTPX 0.28.1 rejects the corresponding
URL-pattern forms. CIDR-looking IPv4 entries retain HTTPX's exact URL-pattern
host behavior rather than native Rust subnet matching. Native
`NoProxy::parse()` retains bracketed IPv6 and true CIDR support. For ordinary bare domains, the compatibility parser matches
the bare host and subdomains only at a label boundary; a leading-dot entry
matches subdomains but not the bare host. Explicit host ports require an
explicit normalized target port, so an entry such as `example.test:80` does
not bypass an HTTP URL whose default port is omitted. These compatibility rules
are separate from native `NoProxy::parse()`, which retains true CIDR and native
default-port semantics.

httpx2 2.12.0 fixed IPv6 CIDR `NO_PROXY` (`all://[addr]/subnet` mount form;
0.28.1 emits the malformed `all://[addr/prefix]` form that never matches).
The 0.28.1 facade preserves its qualified oddities unchanged; the httpx2
facade uses a versioned parser policy with the fixed form behind an explicit
profile boundary. Malformed CIDR never broadens bypass — it fails closed
before dispatch.

HTTP proxy endpoints may use `http://` or `https://`. An HTTPS endpoint first
verifies the proxy hostname over TLS, then uses the existing absolute-form
forwarding or CONNECT path. For HTTPS origins, origin TLS is layered after
CONNECT and uses the origin hostname independently.

**Proxy endpoint TLS is independent from origin TLS**.  The
`proxy_tls_config` is sourced exclusively from the proxy configuration;
the origin `TlsConfig` is never reused as a fallback for the proxy
handshake.  This means an origin `verify=False`, a custom origin CA
bundle, an origin mTLS client identity, an origin SNI override, and
the origin TLS version policy are not propagated to the proxy
endpoint.  Callers that need a specific trust anchor for the proxy
must configure it explicitly via `Proxy(ssl_context=...)`; otherwise
the proxy endpoint is verified using rustls' default trust anchors
(system roots).

Proxy setup maps HTTPX's phase timeouts directly: proxy TCP/TLS and origin TLS
use `connect`, CONNECT writes and tunneled request writes use `write`, and
CONNECT/response headers use `read`. A native `total` timeout remains an
optional monotonic outer deadline; the compatibility facade does not synthesize
one from HTTPX's scalar timeout.

The Python compatibility facade accepts and stores HTTPX proxy metadata,
forwards `Proxy(headers=...)` through the native proxy-leg header
channel, and redacts the values of `authorization`,
`proxy-authorization`, `cookie`, and `set-cookie` in `Proxy.__repr__` and
`Headers.__repr__` so credentials never appear in diagnostic dumps.  The
raw values remain available to protocol code through `Proxy.headers` and
to engine code through the native API.  Ordinary three-element socket
options accept integer, `bytes`, and `bytearray` values; the arbitrary
four-element null-pointer form remains intentionally bounded out.

### CONNECT Tunnel

For HTTPS through a proxy, the transport establishes a CONNECT tunnel:
1. Send `CONNECT host:port HTTP/1.1` to the proxy.
2. Read the 200 response.
3. Upgrade the connection to TLS.
4. Send the actual HTTP request over the TLS tunnel.

The tunneled body stream knows the declared `Content-Length` (when the
response carries an explicit non-chunked length) and uses it only to tell
a truncated body apart from a complete one: EOF short of the declared
length is an error, while EOF after every declared byte — or EOF on a
close-delimited body — ends the stream cleanly. An abrupt close without
TLS `close_notify` is normal for real servers once all bytes arrived.

## SOCKS5 Proxy

### Supported Schemes

The native Rust API retains the useful `socks5://` (local resolution) versus
`socks5h://` (proxy-side resolution) distinction. The HTTPX 0.28.1
compatibility facade normalizes both schemes to HTTPX/httpcore's observed wire
behavior: hostnames are sent as `ATYP_DOMAIN`, while IPv4 and IPv6 literals
are sent as their corresponding literal address types.

### Configuration

```rust
let proxy = Proxy::all("socks5://proxy.example.com:1080")?;
let client = ClientBuilder::new().proxy(proxy).build()?;
```

With authentication:
```rust
let proxy = Proxy::all("socks5://user:pass@proxy.example.com:1080")?;
```

### Protocol Flow

1. TCP connect to SOCKS5 proxy
2. Method negotiation (exactly no-auth without credentials, or exactly
   username/password with credentials)
3. Optional username/password subnegotiation (RFC 1929)
4. CONNECT command with destination address (IPv4, IPv6, or domain name)
5. Parse reply — tunnel established
6. For HTTP: speak origin-form HTTP over the tunnel
7. For HTTPS: perform origin TLS handshake over the tunnel, then HTTP

### DNS Resolution Semantics

The compatibility path is reference-driven rather than inferred from the
scheme name. HTTPX 0.28.1 sends hostname destinations as `ATYP_DOMAIN` for
both accepted SOCKS schemes, never substitutes loopback for an unresolved
name, and sends IP literals unchanged.

### Authentication

Supports username/password authentication via URL userinfo (`socks5://user:pass@host:port`) or explicit `.auth()` on the Proxy builder. Credentials are never exposed in Debug/Display output.

### Pool Isolation

The Rust client owns persistent Hyper SOCKS clients in a per-client cache
keyed by proxy endpoint, scheme, and authentication identity. This keeps the
SOCKS handshake and HTTP connection pool alive across compatible requests,
while different endpoints or credentials remain isolated. Credentials are
held only in the opaque internal key and are not included in debug/display
output.

## HTTP/2

### Feature Gating

Behind the `http2` Cargo feature. When not enabled, `Http2Only` and `Auto` silently downgrade to `Http1Only`.

### Version Policy

`HttpVersionPolicy` enum:
- `Http1Only` — only HTTP/1.1.
- `Http2Only` — only HTTP/2 (fails if server does not negotiate). Enforced at both the ALPN layer (only `h2` is advertised) and at the hyper-util legacy client layer (`http2_only(true)`).
- `Auto` (default) — both `h2` and `http/1.1` advertised.

### ALPN

ALPN protocols are set on the rustls configuration based on the version policy. The connector builder handles `enable_http1()` / `enable_http2()`:

- `enable_http2()` alone → `alpn_protocols = vec![b"h2"]` (h2-only).
- `enable_http1().enable_http2()` → `alpn_protocols = vec![b"h2", b"http/1.1"]` (auto).
- `enable_http1()` alone → ALPN stays empty (h1-only).

The standard hyper-rustls path passes an empty ALPN list to `hyper_rustls::HttpsConnectorBuilder` so the builder can populate it from the `enable_http1`/`enable_http2` calls. Direct and UDS connectors perform their own TLS handshake; the ALPN they advertise is determined by the `TlsConfig` and is shared across the three paths.

### Shared Hyper response lifecycle

The standard Hyper, specialized-direct (socket options / local address),
SNI-override, and UDS paths share one response-lifecycle implementation in
`transport/direct.rs`: shared trace helpers (`emit_send_start` /
`emit_receive_complete` / `emit_send_failed`), shared dispatch error
mapping (`map_send_error`, including write-timeout unwrapping and H2
classification), a single `finish_hyper_response()` converter that
captures the 101 upgrade future before consuming the body, builds the
streaming response with `SharedTrailers`, and attaches the
`UpgradedStream` via connector-aware downcasting (direct → real addrs/TLS,
UDS → `Unix`/`TlsUnix` without IPs, opaque → explicitly unavailable).

Transport selection itself is declarative: `prepare_single_request()`
builds a `PreparedRequest`, `select_route()` picks one `TransportRoute`
(UDS → specialized-direct → proxy/SOCKS → SNI-direct → H3 → standard),
and one common post-transport policy (decompression, decoded-size limit,
read-timeout + pool lease) applies to every route.

### `http2_only` enforcement

For `HttpVersionPolicy::Http2Only`, the legacy hyper-util client is built with `http2_only(true)`. This is what enforces the protocol contract:

- For **TLS** connections, `http2_only(true)` causes hyper-util to attempt an HTTP/2 handshake on every accepted socket. When ALPN does not negotiate `h2` (or the server only advertises `http/1.1`), the H2 handshake fails and the request is rejected with a `RequestError` / `ConnectError`. There is no silent downgrade to HTTP/1.1.
- For **cleartext** connections, `http2_only(true)` causes hyper-util to send the H2 client preface directly over the TCP socket. This is HTTP/2 prior knowledge (h2c) and matches HTTPX's behavior on cleartext H2 servers.

Both the standard hyper-rustls client and the direct / UDS clients are configured with `http2_only(true)` when H2-only is selected. The `Http1Only` and `Auto` policies do not set `http2_only`.

Corrective 06 extends this to the SNI override and SOCKS routes: `ClientInner::sni_client()` and `ClientInner::socks_client()` now read `self.config.http_version_policy` and call `builder.http2_only(true)` when `Http2Only` is active. The SNI client further restricts the connector's TLS ALPN list to `h2` (via `DirectConnector::with_alpn_h2_only()`) so no silent H1 fallback is possible. The SOCKS client restricts the rustls ALPN list to `h2` for HTTPS transports. `SocksStream::connected()` now inspects ALPN on the inner TLS stream and calls `negotiated_h2()` when `h2` is negotiated, mirroring the direct-stream behavior. The SNI client cache key remains the hostname string alone; `HttpVersionPolicy` is currently client-level immutable, so a single cache key per hostname is still sound.

### Direct connector ALPN signaling

`DirectConnector` and `UdsConnector` wrap their own TLS handshake (via `tokio_rustls::TlsConnector`) and signal the negotiated ALPN protocol back to hyper-util so the legacy client knows whether the connection is H2:

```rust
impl Connection for DirectStream {
    fn connected(&self) -> hyper_util::client::legacy::connect::Connected {
        let mut connected = Connected::new();
        if let Self::Tls(tls) = self {
            if let Some(b"h2") = tls.get_ref().1.alpn_protocol() {
                connected = connected.negotiated_h2();
            }
        }
        connected
    }
}
```

Without this signal, an H2-only legacy client would not see the ALPN result and could incorrectly attempt H1 framing on a connection that just negotiated h2.

### Forbidden Header Stripping

Per RFC 9113 §8.2.2, the pipeline strips before sending: `Connection`, `Keep-Alive`, `Proxy-Connection`, `Transfer-Encoding`, `Upgrade`, and `TE` (except `trailers`).

### Error Taxonomy

| Error | Meaning |
|-------|---------|
| `Http2GoAway` | Server sent GOAWAY frame |
| `Http2StreamReset` | Stream reset via RST_STREAM |
| `Http2FlowControl` | Flow-control error |
| `Http2Protocol` | Generic HTTP/2 protocol error |

The H2 classification function in `crates/eggfetch-core/src/transport/direct.rs` maps hyper error strings to these variants. `last_stream_id` is currently hardcoded to `0` because hyper does not expose the inner h2 error's stream identifier (see the `stream_id` residual in `docs/residual-differences.md`).

### Retry Classification

`REFUSED_STREAM` is retryable for replayable requests (RFC 9113 §7.2.4). `CANCEL`, `GOAWAY`, flow-control, and protocol errors are not retried.

### Pool Interaction

HTTP/2 multiplexes streams on a single connection, but eggfetch's pool permits still control logical request concurrency. hyper handles connection-level multiplexing; the server's `SETTINGS_MAX_CONCURRENT_STREAMS` is respected internally.

## HTTP/3

### Feature Gating

Behind the `http3` Cargo feature. Experimental — the label is retained
while the QUIC/h3 ecosystem matures (no 0-RTT, WebTransport, datagrams,
connection migration, MASQUE, or Happy Eyeballs in this milestone).
Authenticated Alt-Svc discovery, broken-route suppression, safe fallback,
and draining are implemented; see below.

### Transport

Uses `quinn` for QUIC transport and `h3` for the HTTP/3 protocol layer. QUIC mandates TLS 1.3; 0-RTT is disabled.

### Version Policy

`HttpVersionPolicy::Http3Only` routes over QUIC directly (strict, no
fallback, no discovery). `Auto { allow_http3: true }` uses authenticated
Alt-Svc discovery: H2/H1 unless a fresh `h3` alternative is cached and not
suppressed; no fresh entry means never invent an H3 endpoint.
`Auto { allow_http3: false }` (default) never attempts H3. H3 never bypasses
proxy rules: proxy/UDS/SNI routes are selected first in `select_route()`.

### Alt-Svc Discovery (`transport/alt_svc.rs`)

Separate from the QUIC `sender_cache`: an origin may advertise an
alternative with no session, and evicting a failed session never erases the
advertised route.

- Origin key is normalized `(scheme, host, port)`; only `https` origins
  learn, and only `h3` protocol IDs are recorded (`h2`, drafts ignored).
- Retains alternative authority/port, `ma` expiry (default 24 h), and
  `clear`; `ma=0` never caches. Cache bounded at 64 entries, arbitrary-entry
  eviction, lazy expiry, no background tasks. No credentials/cookies/userinfo
  stored; alternative affects only UDP routing — `Host`, cookies, auth, and
  security policy always use the logical origin.
- Parsing is isolated and bounded (8 KiB header cap, 16 alternatives max,
  512 B authority cap). Malformed quoting, unknown protocols, bad ports,
  `ma` overflow, userinfo injection, and oversized values fail closed.
- Trust: learning requires authenticated HTTPS (verified TLS per policy,
  not `danger_accept_invalid_certs`), no proxy route, and hop-local origin.
  Plaintext `http`, proxy metadata, and cross-origin confusion are rejected
  before parsing (counted as `altsvc_rejected`). QUIC TLS validates the
  original origin (SNI = origin host), never the alternative hostname.

### Broken-Route Suppression

Per-origin suppression with exponential backoff (5 s base × 2ⁿ, 300 s cap,
deterministic `Instant` injection for tests), bounded at 64 entries.
Successful H3 use clears it; a changed alternative authority/port creates a
new generation that re-enables without waiting, while same-endpoint
re-advertisement preserves suppression. Only route failures
(`Connectivity`/`Protocol`/`Closed` from `Error` variants, never strings)
suppress; request-scoped `Other` (write/read/total timeouts) and graceful
drains never suppress. This is routing suppression, not retry policy.

### Safe H3-to-H2/H1 Fallback

`Auto` only, pre-commit only, replayable bodies only (`Empty`/`Bytes`;
one-shot streams never duplicate). `Http3Only` returns the H3 failure
strictly. Retry policy remains the only later-attempt mechanism after
ambiguous delivery. `total`/`connect`/`write`/`read` deadlines are monotonic
(reused, never restarted); fallback cannot bypass proxy or alter TLS
verification. Implemented via explicit `H3DispatchError { error,
body_committed, draining, failure_class }` rather than string inference.

### Draining (GOAWAY)

Observed through the pinned h3 0.0.8 API (requires the
`i-implement-a-third-party-backend-and-opt-into-breaking-changes` feature,
which only removes `#[non_exhaustive]` and exposes `ConnectionState`; no
runtime change, re-audit on version bump): `is_closing()` signals GOAWAY
(`RemoteClosing` on new streams) and driver `poll_close` carries the
application close code (`is_h3_no_error()` for graceful). Draining marks the
cached generation so no new streams are assigned; in-flight streams keep
their clones and the detached driver until complete. The drained generation
is evicted (generation-scoped via `Arc::ptr_eq`) and the next request
reconnects (counted as `h3_reconnected`). Close codes are preserved in
diagnostics via `close_reason`; close errors are never normalized into
success.

### Lifecycle

Per-origin (`host:port`) state machine:

`Vacant -> Connecting -> Ready -> Failed/Closed -> Evicted -> Reconnectable`

- One connection attempt per origin at a time. The per-origin
  `tokio::sync::OnceCell` serializes DNS + QUIC + h3 init; concurrent
  waiters share the same attempt.
- Failures are not cached (`OnceCell` keeps only success), so a failed
  origin stays reconnectable.
- Terminal transport failures evict the stale entry (scoped to the failed
  generation via `Arc::ptr_eq`, so a concurrent fresh connection is never
  dropped). Eviction removes only cache ownership: in-flight streams hold
  their own sender clones and the detached driver task keeps driving until
  they complete.
- Client drop releases cache ownership and the endpoint; driver tasks end
  when their connections close. In-flight bodies remain valid until
  consumed or dropped.

### Origin Cache

Bounded at 64 entries (`H3_CACHE_MAX_ENTRIES`), matching the spirit of the
SNI (256) / SOCKS (64) caches without a new LRU dependency. Eviction is
arbitrary-entry and never removes the origin being inserted. Boundedness is
pinned by unit tests in `transport/http3.rs` via `test-util` cache hooks
(`cache_len`, `contains_origin`); in-flight survival across unrelated
eviction is covered by `tests/h3_hardening.rs`.

### Address Fallback

DNS resolves the complete address set (not just the first record). Each
candidate is attempted in order under one shared connect deadline with a
fair share per remaining address (`remaining / addrs_left`), so a stalling
candidate cannot starve later ones. Cancellation drops the loop promptly.
All-address failure preserves the `Connect` (DNS) / `H3Connect`
(handshake) taxonomy without leaking sensitive detail.

### Timeouts

- `Timeout.connect` bounds DNS + QUIC handshake + h3 init as one budget
  shared across fallback attempts (phase `Connect` on expiry).
- `Timeout.total` stays the outer pipeline deadline and is never restarted
  per address or reconnect (phase `Total` wins when tighter).
- `Timeout.write` applies to streamed request-body progress via the
  pre-transport wrapper; stalls surface as `Write`, never as H3 protocol
  errors, and never evict the shared connection.
- `Timeout.read` applies to response-body progress at the documented
  post-transport boundary (phase `Read`).

### Idle Lifetime

QUIC idle derives from `PoolConfig::idle_timeout`
(`Limits::keepalive_expiry`), defaulting to 30 s when unset for
compatibility. `Timeout.pool` is an acquisition budget and never controls
idle lifetime. Quinn idles on packet activity while Hyper idles on pool
checkout, so the mapping is documented as policy-equivalent, not identical.
Idle connection counts (`max_idle_connections`) do not apply to H3's
one-connection-per-origin model.

### Stream Limits

Logical concurrency (pool permits, one per request) is separate from the
physical QUIC stream cap. `max_concurrent_bidi_streams` derives from the
effective per-origin in-flight limit (`max_in_flight_requests_per_origin`
or alias `max_connections_per_host`) when set (so the transport never
contradicts configured concurrency), otherwise 100. Unidirectional streams
stay at 100 (control traffic). Every H3 request still holds a pool permit,
so the logical per-origin limit remains the upper bound on concurrent
streams.

### Failure and Retry

The transport never retries internally (except draining eviction, which
creates a fresh generation for the *next* request, never resending the
current body) and never replays one-shot bodies. Stale entries are evicted
on transport failure and the original error is returned; replayable
idempotent requests reconnect only through the existing retry machinery on
a later attempt, or via safe `Auto` fallback pre-commit. Only `H3Connect`
is retryable; `H3ConnectionClosed` / `H3Stream` / `H3Protocol` are not.

### Error Taxonomy

| Error | Meaning |
|-------|---------|
| `H3Connect` | QUIC connection failed (retryable for replayable requests) |
| `H3ConnectionClosed` | Peer closed the connection (not retried) |
| `H3Stream` | Stream error (not retried) |
| `H3Protocol` | HTTP/3 protocol error (not retried) |

### Stress Evidence

`tests/h3_hardening.rs` (loopback fixtures only): connect/total
precedence, stalled-body read timeout, shared concurrent init, per-host
pool gating, failure non-poisoning, distinct-origin stabilization,
fail/reconnect cycles, partial-body drop reuse, client-drop release, and
prompt cancellation with continued usability.

`tests/h3_alt_svc_discovery.rs` (loopback only): explicit strictness,
Auto discovery (learn-then-H3), suppression with fast skip and new-generation
recovery, safe fallback with monotonic deadlines, draining reconnect,
exact observability, and adversarial cases (plaintext/injection/oversized/
stale/cancellation/clear, SNI preservation, one-shot non-replay).

`tests/h3_interop_qualification.rs` (20 deterministic loopback controls;
external cases are selected by the qualification runner): mandatory Quinn self-interop,
GET/HEAD, buffered POST upload, 1 MiB streaming download, 20-way
multiplexed concurrency, H3 trailers after EOF, UDP-blackhole
connect-budget termination, bad-then-good non-poisoning,
server-restart reconnect, early-close / mid-response-drop termination
with recovery, prompt cancellation, 100-request reuse soak,
fail/reconnect stabilization, 70-origin boundedness beyond the
64-entry cache, cancellation storms, construction/drop loops, exact
metrics with truthful `None` H3 per-response metadata, IPv6
capability detection, and a doc-pinned platform-scope check. The external
adapter path can select GET/HEAD, buffered and streaming upload, large
streaming download, trailers, multiplexing and reuse; server-controlled
lifecycle cases remain unsupported unless an adapter can induce them.

The implementation-neutral case contract is
`qualification/http3/corpus.json`. Adapter manifests must capture an exact
source revision or immutable image digest and are run by
`scripts/h3_qualification.py`. Results retain pass/fail/unsupported status
per implementation and per case in JSON; missing external servers never pass
the gate. The qualification-only impairment contract is
`qualification/http3/impairment-matrix.json`, coordinated by
`scripts/h3_impairment.py` with a platform-specific namespace/netem runner.

### Dependency and Upstream-Risk Review (2026-09-11 frozen-tree audit)

Pinned for this milestone (exact versions from `Cargo.lock` at review time):

- `quinn 0.11.11` (`rustls` + `ring` + `runtime-tokio`)
- `quinn-proto 0.11.16`, `quinn-udp 0.5.15` (transitive QUIC transport)
- `h3 0.0.8` with `i-implement-a-third-party-backend-and-opt-into-breaking-changes`
  (exposes `ConnectionState::is_closing()` / `is_h3_no_error()`; re-audit on bump)
- `h3-quinn 0.0.10` bridge
- QUIC/TLS companions: `rustls 0.23.41`, `ring 0.17.14`,
  `tokio-rustls 0.26.4`, `tokio 1.52.3`, `http 1.4.2`
  (`hyper 1.10.1` remains the H1/H2 engine; H3 bypasses hyper)

Hyper's HTTP/3 integration remains unfinished upstream and the h3
ecosystem still carries active correctness/interoperability work around
clean close, buffered data, stream reset/cancel, GOAWAY, QPACK/frame
parsing, and connection-driver lifecycle. In particular, [h3 issue #338](https://github.com/hyperium/h3/issues/338)
reports that buffered frame data can be discarded when a QUIC connection
error arrives in the same receive batch; the affected h3 0.0.8 frame-layer
behavior can turn an otherwise clean close into a truncated/error response.
EggFetch has no proven workaround for that ordinary-response path, so this
is a graduation blocker. Related open reset/STOP_SENDING work remains
tracked in the H3 upstream-risk ledger. No unsupported fork of h3/Quinn is
introduced to force graduation, and no dependency upgrade is taken in this
milestone: the required independent and impairment evidence is also absent,
so changing versions now would invalidate the completed frozen-tree gates.
Any future upgrade must land before a new freeze and requalification.

### Interoperability Evidence (this milestone)

- Deterministic loopback Quinn/h3 fixtures are mandatory and green
  (`h3_hardening`, `h3_alt_svc_discovery`, `h3_interop_qualification`).
  The interop corpus covers GET/HEAD, request bodies (buffered upload),
  large streaming download, concurrent multiplexed requests, response
  trailers, cancellation/reset, server-restart reconnect, early-close /
  mid-response drop termination with client recovery, UDP-blackhole
  budgets, bad-then-good non-poisoning, 100-request reuse soak,
  fail/reconnect stabilization, cancellation storms,
  construction/drop loops, many-distinct-origin boundedness, exact
  metrics with truthful `None` H3 per-response metadata, and IPv6
  capability detection. Alt-Svc alternative authority/port, suppression,
  safe fallback, and draining are covered in
  `h3_alt_svc_discovery.rs`.
- External independent servers (ngtcp2/nghttp3, quiche, quic-go, …) are
  supported through the pinned adapter manifest and capability-selected
  cases. Generic endpoint cases run through the same EggFetch client path;
  server-controlled cancellation/reset/GOAWAY/restart cases remain
  unsupported until the adapter supplies those controls. No external-server
  pass is claimed in this milestone.
- Real-network public-origin spot checks are manual qualification
  supplements, not Tier 1; none are claimed in this milestone. The
  manual procedure below is the qualification instrument when spot
  checks are run.
- Deterministic local impairment coverage includes UDP blackhole,
  bad-then-good origin behavior, server restart, early-close / mid-response
  drop, cancellation during connect, and cancellation storms. The complete
  required loss/reordering/jitter/duplication/MTU/address-family matrix is
  represented in `qualification/http3/impairment-matrix.json`; execution
  remains qualification-only and unsupported scenarios stay explicit.

### Manual Public-Origin Spot-Check Procedure (Tier 2/manual, not CI)

Do not bake volatile public hostnames into Tier 1 tests. When a
qualifier runs spot checks, follow this procedure and record the
ledger fields below:

1. Pick a small set of major public HTTPS origins known to advertise
   H3 at qualification time (record the exact hostnames and date;
   they are volatile and are not part of the repo).
2. For each origin, over authenticated HTTPS with default trust:
   confirm the first request discovers Alt-Svc, then confirm a later
   eligible request selects H3 with the correct protocol version and
   response content.
3. With UDP blocked (e.g. firewall drop, not just unplugged), confirm
   the request still terminates within the configured
   timeout/cancellation contract and falls back to a usable H1/H2
   path where the policy permits it.
4. Confirm diagnostics expose no credentials (redaction policy holds).
5. Vary DNS/IPv4/IPv6 where available and confirm no permanent
   poisoned state (a later request to a healthy origin succeeds).
6. Record per origin: date, origin, negotiated protocol, result, and
   whether any failure was transient Internet failure versus a
   deterministic EggFetch failure. Transient Internet failure is not
   graduation evidence and never fails Tier 1.

Ledger for this milestone: no public-origin spot-check pass is
claimed; the fields above are empty by design until a qualifier runs
the procedure on a frozen SHA.

### Resource and Platform Scope

- Soak coverage is bounded and deterministic: 100 sequential reused-H3
  requests, 10 fail/reconnect cycles, 20-iteration cancellation storms,
  20-iteration construction/drop loops, 70-distinct-origin boundedness,
  20-way multiplexed concurrency, and 1 MiB streaming download.
  Long-duration RSS/descriptor soak with allocator-threshold policy
  remains extended-tier future work (existing
  `resource_monitor`/resource-stabilization conventions apply; no
  fragile exact-RSS assertions in H3 tests).
- Fuzz coverage for EggFetch-owned Alt-Svc parsing/state lives in
  `fuzz/fuzz_targets/fuzz_alt_svc.rs` (header parsing, cache
  learn/expiry/clear bounds, trust gating, plus suppressor
  observe/suppress/recover transitions) plus deterministic adversarial
  unit/integration tests; h3/QPACK internals are not re-fuzzed.
- Platform scope reuses existing CI/package mechanisms (no new matrix):
  UDP socket binding, IPv6 availability (capability-detected, never
  assumed), certificate stores, timer precision, and address metadata
  follow the same policy as the rest of the engine. Lack of a
  platform-specific H3 test is not described as support evidence.

### Production Graduation Decision (2026-09-11): experimental retained

HTTP/3 remains **experimental** for ordinary request/response operation.
The objective gate in
`plans/http3-interoperability-and-production-graduation.md` does not
pass yet; concrete blockers:

1. No two-independent-non-Quinn-server interoperability pass recorded
   on frozen executable SHA
   `639bf186a71c054e11278d1b160ffe7a6f172c02`; the current evidence ledger is
   `plans/http3-independent-interop-and-impairment-qualification-evidence.json`.
2. No public-origin Alt-Svc spot-check ledger recorded.
3. Impairment harness covers blackhole/restart/early-close/cancellation
   locally; packet-loss/reordering/jitter/duplication/MTU/address-family
   matrix execution remains unavailable without a namespace/netem runner.
4. Upstream h3 issue #338 (buffered data lost on connection close) remains
   open and has no EggFetch workaround or reachability proof; related
   cancellation/reset issues remain under review. Graduation must be
   evidence-driven, not label-driven.

Removing the experimental label requires the full gate (deterministic
suites + Alt-Svc/fallback/draining + two independent interop passes +
spot checks + impairment/resource + no known corrupting upstream issue
+ documented platform scope + green Tier 1/extended). Advanced QUIC
features (0-RTT, WebTransport, datagrams, MASQUE, migration) stay
separately experimental/unimplemented regardless.

### Python API

```python
client = eggfetch.Client(http3=True)
r = client.get("https://example.com")  # Uses QUIC
```
