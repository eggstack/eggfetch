# Core Engine Deep Dive

The core engine (`eggfetch-core`) is the sole owner of all HTTP behavior. This document covers the client, request/response types, pipeline lifecycle, and error taxonomy.

See also: [overview.md](overview.md) for the high-level map.

## Module Map

Focused subset for the engine lifecycle (client → request → pipeline → response). The full per-domain module reference lives in [overview.md](overview.md) with details in the sibling deep-dives.

| Module | Public? | Purpose |
|--------|---------|---------|
| `client` | Yes | `Client`, `ClientBuilder` — entry point |
| `request` | Yes | `Request`, `RequestBuilder` — fluent request construction |
| `response` | Yes | `Response`, `HistoryEntry` — response + redirect history |
| `body` | Yes | `RequestBody`, `ResponseBody`, `NativeResponseBody`, `BoxBytesStream` |
| `headers` | Yes | `Headers` — case-insensitive header map wrapper |
| `network_stream` | Yes | `NetworkStream`, `UpgradedStream`, `ConnectionMetadata` — upgrade IO + connection metadata |
| `trace` | Yes | `TraceObserver`, `TraceEvent` — synchronous lifecycle event callbacks |
| `error` | Yes | `Error` enum, `RequestFailure` opt-in detail wrapper, `Result<T>` alias |
| `pipeline/` | Crate-internal | Request lifecycle orchestration split by responsibility: `retry`, `redirect`, `prepare`, `route`, `hyper_dispatch`, `proxy_dispatch`, `h3_dispatch`, `finalize`, plus short `mod` entry points |
| `transport` | Yes | Direct, caller-owned raw-stream dialer, direct-with-socket-options, UDS, proxy, HTTP/3 transport dispatch |
| `stream` | Crate-internal | Unified response-body timeout (`BodyTimeoutStream`: read inactivity + absolute total) plus per-chunk write timeout |

## Client

`Client` owns the connection pool, TLS configuration, and default settings. Created via `Client::new()` (defaults) or `ClientBuilder` (configuration).

```rust
let client = ClientBuilder::new()
    .timeout(Timeout::from_secs(30))
    .default_header("user-agent", "my-app")
    .build()?;
```

Builder-configurable options: headers, timeout, pool, redirects, auth, cookies, proxy, TLS, retry, Hyper canceled-request retry, native dialer, physical connection admission, established-I/O inactivity, decompression, HTTP version policy, max body size, max decompression ratio.

`Client` is `Clone` (cheap — internals are `Arc`-wrapped).

### Native request-failure detail

`Client::send_detailed()` and `RequestBuilder::send_detailed()` are additive
native Rust entry points. They preserve the existing `Error` value and
`Error::kind()` result while attaching at most one terminal
`NetworkFailureKind`. The request carries an optional crate-private atomic
context through retry and redirect reconstruction; ordinary requests carry
`None` and allocate no diagnostic state. Each retry clears the context, and
H3-to-H1/H2 fallback does not retain a failed pre-commit route.

The classifier runs before the transport boundary collapses direct connector
errors into `Error::Connect(String)`. It uses typed I/O refusal evidence,
private direct-connector categories, and a crate-private wrapper around
Hyper-util's standard resolver; it never parses `Display`/`Debug` text.
Standard HTTP/HTTPS resolver failures therefore report DNS while Hyper-util
continues to own address selection, Happy Eyeballs, and TCP establishment.
Direct address fallback reports DNS only for resolution failure, refusal only
when every attempted address was refused, and generic connect for mixed or
otherwise unproven failures. Proxy, UDS, HTTP/3, and caller-owned dialer
routes remain generic/unknown where their current boundaries do not expose
sufficient evidence.

### Native frame execution

`Client::execute_http_body()` is a sibling transport surface for callers that
already own HTTP-level policy. It derives scheme/host/effective-port directly
from the caller-owned `http::Uri` via `http_origin::HttpOrigin` (no
`url::Url` reparse; callers own IDNA/punycode), acquires the same logical
pool permit through the component-based `OriginKey::from_origin`, and
dispatches through the existing standard/direct/resolved/SNI/custom-dialer/UDS
Hyper clients. The request `http_body::Body` is erased to the same internal
Hyper body type used by the high-level path; the raw Hyper response is then
wrapped in `NativeResponseBody` at the response-header boundary.

The high-level pipeline continues to own header defaults, content-length
normalization, redirects, retries, cookies, auth, decompression and decoded
limits. Native execution does not duplicate those transformations or create
a second connector/TLS/pool stack. Built-in proxy and experimental H3 routes
are explicit v1 unsupported cases because their custom parsers cannot yet
preserve the native frame contract. 101 upgrades are rejected after response
headers identify the status; callers that may receive upgrades must use the
high-level upgrade API because the native request can already have transferred
its body by that point.

`Client::native_service()` exposes this exact native path through
`NativeHttpService`, which implements
`tower_service::Service<http::Request<B>>` for the same body bounds and
response type. The adapter only clones the client/options and delegates from
`call()`; it does not inspect request extensions or add a second policy layer.
Its `poll_ready()` is intentionally always ready because the request is
needed to identify the origin pool. Logical pool admission, physical
connection admission, and transport backpressure therefore occur in the
returned future. This means readiness-based Tower load shedding does not
represent eggfetch pool saturation. Request extensions are not a guaranteed
passthrough contract, and Tonic/gRPC is supported only as an external
consumer pattern, not by an eggfetch-specific API.

## RequestBuilder

Fluent builder for constructing requests. Supports method, URL, headers, query params, body, auth, proxy override, retry override, timeout override, and transport hints.

```rust
let response = client
    .get("https://api.example.com/users")?
    .header("accept", "application/json")
    .query("page", "1")
    .send()
    .await?;
```

Body sources are mutually exclusive: `body()`, `bytes()`, `stream()`, `json()`, `form()`, `multipart()`.

### Transport Hints

`RequestBuilder::transport_hints()` sets typed wire-level overrides via `TransportHints`:

- `target: Option<Bytes>` — overrides the wire request target (e.g. `OPTIONS *`, absolute-form) without changing the logical URL used for routing, cookies, auth, and proxy selection.
- `sni_hostname: Option<String>` — overrides TLS SNI while preserving TCP destination. Requires `advanced-routing`; lean `standard-http1`/`standard-http2` profiles fail closed with `Unsupported`.
- `resolved_target: Option<ResolvedTarget>` — pins direct TCP to caller-supplied
  addresses without a second DNS lookup while preserving logical URL/Host/SNI
  identity. Same-origin redirects retain it; cross-origin redirects and
  incompatible proxy/UDS/H3 routes fail closed. Requires `advanced-routing`; lean profiles fail closed with `Unsupported`.
- `RequestBuilder::proxy_target_addresses()` — separately pins the physical
  ultimate destination for supported proxied routes. It is not a new public
  `TransportHints` field: retry and same-origin redirect reconstruction carry
  the private request state, while cross-origin redirects fail closed.
- `trace: Option<Arc<dyn TraceObserver>>` — installs a callback observer for [`TraceEvent`](../../crates/eggfetch-core/src/trace.rs) emissions during dispatch.

Transport hints survive retry reconstruction via the typed
`RequestParts::retry_request()` transformation. Ordinary destination-specific
hints (`target`, `sni_hostname`, and `trace`) are cleared on redirect hops;
the native `resolved_target` snapshot is retained only for same-origin hops
and causes a cross-origin redirect to fail closed. The redirects-disabled fast
path and the redirect-enabled first hop share one `HopBuildParams` builder, so
the first-hop behavior cannot diverge between the two entry paths.

### Static resolved-destination routing

`RequestBuilder::resolved_addresses()` (requires `advanced-routing`; absent
from lean `standard-http1`/`standard-http2` profiles) separates logical identity from the
physical endpoint:

```text
logical URL / Host / HTTPS SNI  ── request identity ──┐
                                                       ├─ direct TCP only
caller-supplied SocketAddr set ── physical route ──────┘
```

The caller supplies and validates the address set; the engine uses those
addresses exactly and never performs DNS fallback. Retries and same-origin
redirects preserve the snapshot. Cross-origin redirects and proxy, UDS, or
HTTP/3 routing fail closed before dispatch. Each static request uses an
isolated direct Hyper client, so ordinary pooled connections cannot satisfy a
request pinned to a different address set. This is a transport primitive, not
an SSRF or address-authorization policy.

### Proxied route snapshots

`Proxy::resolved_addresses()` controls the physical TCP peer of the logical
proxy URI. `RequestBuilder::proxy_target_addresses()` controls the physical
ultimate destination inside an HTTPS CONNECT or local-resolution SOCKS5
route. The two snapshots are independent and must not be conflated:

```text
logical proxy URI      -> proxy TLS identity / matching / auth
proxy peer snapshot    -> first TCP destination
logical origin URL     -> Host / destination TLS identity / redirect policy
target snapshot        -> CONNECT or SOCKS5 destination address
```

Pinned routes perform no DNS fallback. Successful ordinary forward-proxy and
single-target CONNECT routes use bounded Hyper clients whose keys retain the
proxy and destination policy; multi-address CONNECT fallback remains on the
handshake-specific path. SOCKS client-cache keys include both snapshots.
The reusable forward/CONNECT factories and connectors retain only
connection-scoped policy: logical `Timeout.total` and the request's
`remaining_total` are neither cache identity nor connector state. The pipeline
wraps each proxy dispatch in the current request's outer total timeout, so a
stale pooled connection that needs a new physical connection cannot inherit a
predecessor's budget.
Retries and same-origin redirects retain snapshots, while cross-origin
redirects reject them. HTTPS CONNECT target fallback is deliberately limited
to typed 502/504 proxy rejection. Local-resolution SOCKS5 target fallback is
limited to destination-specific replies 0x03 (network unreachable), 0x04 (host
unreachable), and 0x05 (connection refused); authentication, policy, protocol,
and malformed-response failures stop immediately.
SOCKS5H target pinning and plaintext HTTP forward-proxy target pinning fail
before I/O because their protocol semantics cannot enforce a local ultimate
destination. These controls accept caller-validated addresses only and do not
implement authorization, CIDR, or SSRF policy.

### Proxy Override

`RequestBuilder::proxy()` accepts `ProxyOverride`:
- `Inherit` — use client-level proxy (default)
- `Direct` — bypass proxy for this request
- `Override(Proxy)` — use a different proxy for this request

## Response

`Response` wraps status, version, headers, URL, body, redirect history, wire metadata (original content-encoding/length, reason phrase), trailers, and the optional 101 `network_stream`.

Key methods:
- `status()` → `StatusCode`
- `version()` → `http::Version`
- `headers()` → `&HeaderMap`
- `url()` → `&Url`
- `wire_content_encoding()` → `Option<&str>` — original wire Content-Encoding (before decompression)
- `wire_content_length()` → `Option<&str>` — original wire Content-Length
- `content_length()` → `Option<u64>` — parsed declared wire Content-Length; never decoded body size
- `wire_reason_phrase()` → `Option<&str>` — original wire HTTP/1.x reason phrase
- `bytes()` → buffered body as `Bytes`
- `text()` → buffered body as `String`
- `json()` → optional `DeserializeOwned` decoding through the same single-consume `bytes()` path
- `bytes_stream()` → streaming `BoxBytesStream`
- `raw_bytes_stream()` → streaming body without decompression
- `network_stream()` / `network_stream_mut()` / `take_network_stream()` / `into_network_stream()` → 101 upgrade IO accessors (`None` for ordinary/CONNECT responses)
- `text_lines()` → line-by-line text iterator
- `trailers()` → `Option<HeaderMap>` after body EOF (H1 chunked, H2 trailing HEADERS, H3 trailing headers; `None` until arrival, on no-trailers, or on pre-trailer errors; H1 duplicates collapse upstream)
- `history()` → `&[HistoryEntry]` (redirect chain)

`RequestBuilder::max_decoded_body_size()` and
`RequestBuilder::max_decompression_ratio()` override the corresponding
client limits for one logical request. Request values win over client values
and survive retries and redirects; both buffered and streaming response paths
use the same prepared limits.

### HistoryEntry

Metadata-only redirect record: status code, URL, headers (redacted for cross-origin). Does not carry body data.

## Pipeline Lifecycle

The `pipeline/` directory orchestrates the full request lifecycle. Entry point: `send_with_retry()` (`pipeline::retry`).

```
send_with_retry()           ← retry loop (pipeline::retry)
  send_with_redirects()     ← redirect loop (pipeline::redirect)
    send_single_request()   ← one HTTP round-trip (pipeline::mod orchestration)
```

### send_single_request phases:

Preparation (`pipeline::prepare` → `PreparedRequest`) centralizes
request policy so transports own only connection/protocol work:

1. Header merge (client defaults + request overrides) and timeout merging
   (per-field) happen in `pipeline::redirect::send_with_redirects()`; the hop itself is built
   by the shared `HopBuildParams` builder (cookies, auth, hints).
2. Preparation normalizes accept-encoding, Content-Length, user-agent, and
   H2-forbidden headers, validates request size, resolves the wire URI
   (`target` override), wraps stream bodies with the write timeout,
   resolves the effective proxy/origin, acquires the pool guard, and
   computes the remaining-total/deadline. One-shot bodies are moved, never
   cloned.
3. Dispatch selects one `TransportRoute` via `pipeline::route::select_route()` (UDS → custom
   dialer → static/specialized direct → proxy/SOCKS → SNI-direct → H3 →
   standard Hyper). A configured custom dialer is never silently bypassed;
   unsupported combinations with proxy, UDS, resolved targets, local socket
   controls, or H3 fail during preparation before network I/O.
   H3 is `Http3Only` direct or `Auto`-discovered (fresh Alt-Svc + not
   suppressed; `pipeline::h3_dispatch`); safe `Auto` fallback to standard is pre-commit replayable
   only. Hyper request scaffolding is built once via `pipeline::hyper_dispatch::build_hyper_request()`.
4. One common post-transport policy (`pipeline::finalize::finalize_response()`) applies to every route: Alt-Svc learning
   (learnable routes only), decompression wrapping, decoded-size limiting,
   then read/total-timeout + pool-lease attachment. The absolute total
   deadline is installed behind the private `PoolGuard` response lifecycle
   before the guard is placed behind the lease `Arc`, and enforced at the
   final stream boundary for every consumption mode; no Tokio timer is
   created at finalization so bodies can cross runtimes before first poll.
   Public `ResponseBody` variant shapes are unchanged.

### Connector lifecycle ordering

Every Hyper client is assembled as:

```text
physical admission → ConnectTimeout → underlying route connector
  (DNS/TCP/custom dial/TLS) → established I/O guard → Hyper pool
```

In the concrete type, `ConnectTimeout` is inside the lifecycle connector:
the lifecycle wrapper acquires a permit first, then starts the connect-phase
future, and retains the permit in the returned connection until Hyper drops
it. An idle pooled connection still counts, HTTP/2 streams do not consume
additional permits, and failed or cancelled connects release the permit.
Admission wait has its own identifiable error query. The wrapper is shared by
standard, direct/resolved, SNI, custom-dialer, UDS, and SOCKS Hyper routes;
HTTP forward-proxy and compatible HTTPS CONNECT routes now also use Hyper
connectors and its response/body lifecycle. Forward-proxy clients are
bounded and keyed by destination origin plus proxy transport policy. CONNECT
clients are additionally isolated by origin TLS/SNI, protocol policy,
credentials, and any single pinned target. Multi-address CONNECT fallback
retains the handshake-specific path so typed 502/504 retry behavior is not
weakened. Independent H3/QUIC transport remains outside this lifecycle.

### Hyper client construction ownership (`transport::hyper_client`)

All persistent H1/H2 Hyper clients are built through one crate-private
module rather than repeating builder/lifecycle/cache plumbing per route:

- `HyperClientPolicy` owns the shared builder knobs (canceled-request
  retry, H2-only selection, idle timeout, per-host idle cap). Persistent
  singletons (standard, direct, UDS, custom dialer) use `persistent()`;
  cached/isolated routes use `cached_route()`; the forward-proxy cache uses
  `forward_route()`, which never sets `http2_only` because the proxy leg is
  H1 absolute-form framing. Every persistent family receives the same
  resolved idle timeout and effective per-host cap from `Pool`; when a
  timeout is configured the policy also installs the Hyper pool timer the
  timeout requires for background eviction (see
  [core-timeout-pool.md](core-timeout-pool.md)).
- `build_hyper_client()` wraps each route connector as `route connector ->
  ConnectTimeout -> LifecycleConnector` and builds the legacy client with a
  Tokio executor. Concrete monomorphized connector types are kept; no boxed
  dynamic connectors were introduced.
- `BoundedClientCache` centralizes the bounded get-or-build-and-evict
  protocol. Capacities are explicit and unchanged: 256 for SNI/custom-SNI,
  64 each for SOCKS/forward/CONNECT. Eviction is arbitrary-entry (no LRU
  dependency). Construction under the cache lock is CPU/local configuration
  only — no network I/O — and failures return before insert so they never
  poison the cache. Route-specific connector construction stays in the route
  modules and `ClientInner` methods.
- Upstream `hyper-util::client::pool` (`cache`/`map`/`singleton`) was
  qualified and not adopted: those primitives cache single-destination or
  unbounded unkeyed services with unnameable types, while eggfetch needs
  bounded maps of configured legacy clients keyed by secret-safe route
  identity coexisting with legacy physical pooling and the lifecycle
  wrappers. Adoption would add an abstraction layer without removing code.

Route/client inventory (authoritative keys live in code; this table is the
map):

| Family | Lifetime | Key/identity | Physical reuse owner |
|---|---|---|---|
| standard Hyper client | client lifetime | immutable client config | Hyper |
| direct advanced client | client lifetime | immutable client config | Hyper |
| UDS client | client lifetime | client UDS/TLS config | Hyper |
| custom-dialer client | client lifetime | dialer/client config | Hyper |
| SNI client | bounded route cache (256) | SNI hostname + immutable client policy | Hyper |
| custom-SNI client | bounded route cache (256) | SNI hostname + dialer/client policy | Hyper |
| resolved-target client | bounded route cache (64) | `ResolvedRouteKey` (origin + ordered addresses + SNI) | Hyper |
| SOCKS client | bounded route cache (64) | `SocksRouteKey` | Hyper |
| HTTP forward proxy | bounded route cache (64) | `ForwardRouteKey` | Hyper |
| HTTPS CONNECT | bounded route cache (64) | `ConnectRouteKey` | Hyper |
| H3 | separate QUIC cache (64) | H3 origin/Alt-Svc policy | H3 connector |

SNI caches use plain hostname strings because every other
connection-affecting dimension is immutable at `ClientInner` scope.
Resolved-route entries are keyed by logical origin plus the full ordered
physical address snapshot plus the exact SNI override; ordinary DNS/direct
clients never share those entries, and every other connection-affecting
dimension stays immutable at `ClientInner` scope exactly as for the SNI
caches.

Reusable route caches own only connection-scoped policy. Every
connection-affecting distinction (proxy endpoint/auth/headers, proxy TLS
token, pinned peer/target, origin TLS token, SNI, socket/dialer identity,
protocol policy, connect-phase timeouts) participates in the route key;
request-scoped state (total/read/write/pool deadlines, retry/redirect
state, bodies, cookies/auth headers, decompression limits, trace observers,
failure contexts) is never captured by reusable connectors nor added to
keys. The cached CONNECT connector retains only the keyed SNI hint for
tunnel establishment; wire target overrides and trace observers belong to
the current dispatch. The contributor checklist and full connection-vs-
request matrix live in `transport::hyper_client` module docs, with
equality/isolation table tests on `ResolvedRouteKey`, `SocksRouteKey`,
`ForwardRouteKey`, and `ConnectRouteKey`.

When `TransportIoTimeout` is enabled, established reads and writes are
guarded at this same boundary. The timers reset on actual byte progress,
cover vectored writes and pending flush/shutdown, and surface as
`Error::TransportIoTimeout` with a read/write direction. They do not change
the request-body or response-body timeout fields.

The standard and specialized-direct Hyper paths share a single
response-lifecycle implementation (`finish_hyper_response()` plus shared
trace helpers, `wrap_incoming` with `SharedTrailers`, and
`await_upgrade` with connector metadata); UDS reuses the same helpers
including 101 upgrade handling (UDS kind without IPs).

### Caller-owned raw streams

`ClientBuilder::dialer()` (requires `advanced-routing`; absent from lean
standard-route profiles) accepts the small public `Dialer` contract rather
than Hyper's `Service`/`Connection` traits. Eggfetch adapts the returned Tokio
stream into Hyper, then performs destination TLS itself for HTTPS, preserving
the logical URL host for `Host`, SNI, and certificate verification. Dialer
errors retain the caller source through `Error::custom_transport_error()` and
never trigger fallback to the ordinary direct connector.

When a request has a wire `target` override, the Hyper request still carries
the logical absolute scheme and authority needed for connector selection; only
the path-and-query portion is replaced. This keeps custom dialing directed at
the logical destination while allowing the caller to control the origin-form
request target.

`retry_canceled_requests(false)` disables only Hyper's implicit retry when a
reused idle connection is unusable before the request begins. All Hyper
legacy-client construction goes through `HyperClientPolicy`
(`transport::hyper_client`), which also applies shared idle-pool settings;
route-specific `http2_only` configuration remains at each policy
constructor, with the forward-proxy route as the documented H1-only
exception. The setting is shared
by standard, direct, resolved-target, SNI, UDS, SOCKS, and custom-dialer
clients, but not the independent HTTP/3 transport. It is separate from the
opt-in eggfetch `RetryPolicy`, whose behavior is unchanged, and no socket-reuse
metric is inferred from it.

## Network Stream and Upgrade Support

`Response` carries an optional `network_stream` field of type `Option<NetworkStream>`:

- For **101 Switching Protocols** responses, Hyper's `OnUpgrade` is captured before consuming the body. The upgrade future is awaited and converted via connector-aware downcasting: direct-connector upgrades recover real local/remote addrs plus TLS version/cipher/ALPN, UDS upgrades report `Unix`/`TlsUnix` without IPs, and standard opaque upgrades remain explicitly unavailable (`None` addrs, default kind). The response body is set to an empty buffered body.
- For **ordinary** responses, `network_stream` is `None` — the connection is managed by the pool and raw IO access would corrupt pool state.
- For **internal HTTPS CONNECT tunnels**, `network_stream` is `None` — the tunnel is owned by the proxy implementation; the canonical access path is the body iterator.
- For **H3**, per-response metadata stays `None`; `TransportKind::Quic` is reserved for future connector-derived endpoint info.
- `UpgradedStream` carries an `UpgradedStreamVariant` (`Tcp`/`Tls`/`Adapter`) classification so callers can detect whether `start_tls` is safe to invoke. Only inner `Tcp` variants support `start_tls`; `Adapter` (Hyper-opaque wrapping of 101 upgrades, including UDS adapter wrapping) and `Tls` (already-encrypted) are rejected before any IO is consumed.
- The `UpgradedStream` provides async `read()`, `write_all()`, `close()`, `flush()`, and `start_tls()` operations, plus `metadata()` for connection info.
- Leading data: downcast branches carry Hyper's `read_buf` explicitly as `leading_data`; the opaque branch preserves Hyper's internal rewind buffer and yields it on the first reads.

**Ownership rules**: once an upgrade handoff succeeds, the connection is removed from the HTTP pool and must never be reused. Closing the response and closing the upgraded stream are independent operations.

## Trace Observer

The `trace` module defines a typed event vocabulary (derived from httpcore 1.0.9's `Trace` context manager) and the callback trait transports use to emit lifecycle events. Observers are installed per request via `TransportHints::trace` — they survive retry reconstruction and are cleared on redirect hops because they are destination-specific. A `resolved_target` is the only transport hint with a same-origin redirect exception.

### Events

`TraceEvent` has ten variants, each carrying a `TracePhase` (`Started`/`Complete`/`Failed`) plus structured metadata where applicable (`Close`, `SendRequestBody`, `ReceiveResponseBody`, and `ResponseClosed` carry no extra fields). Request/response header events are emitted consistently; DNS/connect/TLS phases are observed via `TransportMetrics` to avoid duplicate taxonomy (see `trace.rs` reconciliation docs):

| Event | Extra fields | httpcore dotted name |
|-------|--------------|----------------------|
| `ConnectTcp` | `host`, `port` | `connect_tcp.<phase>` |
| `ConnectUnixSocket` | `path` | `connect_unix_socket.<phase>` |
| `StartTls` | `server_hostname` | `start_tls.<phase>` |
| `Retry` | `delay_ms` | `retry.<phase>` |
| `Close` | — | `close.<phase>` |
| `SendRequestHeaders` | `method`, `target` | `send_request_headers.<phase>` |
| `SendRequestBody` | — | `send_request_body.<phase>` |
| `ReceiveResponseHeaders` | `status` | `receive_response_headers.<phase>` |
| `ReceiveResponseBody` | — | `receive_response_body.<phase>` |
| `ResponseClosed` | — | `response_closed.<phase>` |

Connection-level events map to the `"connection"` logger prefix; HTTP message events map to `"http11"`. `event_to_httpcore_name()` produces these names and `event_to_info()` produces the flat info dictionary (values are opaque `EventValue::{String, U16, U64}`) that the Python compatibility layer forwards to user callbacks.

### Observer Contract

`TraceObserver` is a synchronous, `Send + Sync` callback trait: `on_event(&self, &TraceEvent) -> OnEventAction`. It is invoked inside the transport's async context and **must not block or perform I/O**; for the Python binding, the GIL is acquired only at delivery points, never across network waits.

- `OnEventAction::Continue` lets the transport proceed.
- `OnEventAction::Abort` makes the transport short-circuit and return an error. The observer has already recorded the failure in its binding-owned error slot; callers surface that error after unwinding. This is how a raising Python callback stops dispatch without producing discarded work.

Built-in implementations: `NoopTraceObserver` (discard everything) and `CollectingTraceObserver` (thread-safe event vector with `drain()`/`len()`, used by tests). A callback failure surfaces as `Error::TraceCallbackAborted`.

### Python Bridge

Python callables never enter `eggfetch-core`: `crates/eggfetch-python/src/trace_bridge.rs` wraps them in `PyTraceObserver`. Only sync callables are accepted — coroutine functions are detected via `inspect.iscoroutinefunction()` and rejected eagerly with `TypeError` before dispatch (the core trait is synchronous; an un-awaited coroutine would be silently dropped). Details in [python-bindings.md](python-bindings.md).

## Error Taxonomy

`Error` is a single `thiserror`-derived enum. Each variant has a `kind()`
method returning a static string for programmatic matching.

| Category | Variants |
|----------|----------|
| **Input validation** | `InvalidUrl`, `InvalidMethod`, `InvalidHeaderName`, `InvalidHeaderValue`, `RequestBuild`, `InvalidResolvedTarget` |
| **Connection** | `Connect`, `Tls`, `Protocol`, `Hyper`, `HyperClient`, `Io` |
| **Timeout** | `Timeout { phase, elapsed }` — phase is `Pool`, `Connect`, `ProxyConnect`, `ProxyTls`, `Write`, `Read`, or `Total`; established transport stalls use `TransportIoTimeout { direction, elapsed }` |
| **Pool** | `Pool`; physical admission timeouts remain distinguishable with `Error::is_physical_connection_admission_timeout()` |
| **Redirect** | `InvalidRedirectLocation`, `TooManyRedirects { followed, max }`, `BodyNotReplayableForRedirect`, `ResolvedTargetRedirect` |
| **Auth** | `InvalidAuthHeader`, `ConflictingAuth` |
| **Body** | `Body`, `Decompression`, `UnsupportedContentEncoding`, `DecodedBodyTooLarge`, `DecompressionRatioExceeded`, `Unsupported` |
| **JSON** | `JsonSerialize`, `JsonDeserialize` |
| **Proxy** | `InvalidProxyUrl`, `ProxyConnect`, `ProxyAuthRequired`, `ProxyConnectRejected`, `MalformedProxyResponse` |
| **TLS** | `TlsConfig`, `CaBundle`, `ClientCert`, `PrivateKey`, `CertificateVerification`, `HostnameVerification` |
| **Retry** | `BodyNotReplayableForRetry`, `RetryBudgetExhausted { attempts }`, `RetryNotConfigured` |
| **HTTP/2** | `Http2GoAway`, `Http2StreamReset`, `Http2FlowControl`, `Http2Protocol` |
| **HTTP/3** | `H3Connect`, `H3ConnectionClosed`, `H3Stream`, `H3Protocol` |
| **Trace** | `TraceCallbackAborted` |

`Error` is `Clone` — hyper/IO errors are wrapped in `Arc` to enable cloning.

## Security Invariants

- `unsafe_code = "forbid"` in this crate.
- All `Debug`/`Display` implementations redact secrets via `redact` module.
- CR/LF injection prevention in headers and auth values.
- URL credentials (`user:pass@host`) are rejected.
