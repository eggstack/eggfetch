# Timeout & Pool Deep Dive

This document covers the phase-aware timeout system and the semaphore-based connection pool.

See also: [overview.md](overview.md), [core-engine.md](core-engine.md).

## Timeout System

eggfetch implements phase-aware timeouts that map to specific segments of the request lifecycle.

### Phases

| Phase | What It Covers |
|-------|----------------|
| `Pool` | Waiting for a logical request slot from the concurrency pool |
| `Connect` | TCP connection establishment + TLS handshake (including DNS); for proxy routes, also proxy TCP/TLS setup and origin TLS after CONNECT |
| Physical admission | Optional wait for a live Hyper-connection permit; distinct from logical pool acquisition |
| `ProxyConnect` | Internal classification for proxy TCP setup |
| `ProxyTls` | Internal classification for TLS to an HTTPS proxy endpoint |
| `Write` | Sending request headers and body |
| `Read` | Waiting between response body chunks; proxy protocol reads also cover response headers |
| `Total` | Wall-clock cap across the entire request lifecycle |

### Configuration

Timeouts are configured at two levels:

- **Client-level**: `ClientBuilder::timeout(Timeout::from_secs(10))` — sets defaults for all requests.
- **Request-level**: `RequestBuilder::timeout(Timeout::from_secs(2))` — overrides client defaults per-field.

Request-level overrides are per-field: only fields present in the request-level `Timeout` replace the corresponding client-level fields.

### Implementation

| Phase | Enforcement |
|-------|-------------|
| Pool | `tokio::time::timeout` around pool acquisition |
| Total | `tokio::time::timeout` around the full send |
| Read | Per-chunk wrapper stream (`ReadTimeoutStream`) — deadline resets on each body chunk; direct Hyper/UDS/H3 header acquisition remains owned by the transport future |
| Write | Per-chunk wrapper stream (`WriteTimeoutStream`) — deadline resets on each chunk delivery; H3 propagates `Write` without masking or evicting |
| Connect | Enforced by the direct connector, by proxy TCP/TLS/origin-TLS setup, and by the H3 connector (DNS + QUIC + h3 init as one budget shared across address fallback with fair per-address shares) |

The native `execute_http_body()` response wrapper uses the same read phase
contract: its timer starts when the caller first polls the body and resets
after each returned frame. A delay between response headers and the first body
poll therefore does not consume `Timeout.read`; `TransportIoTimeout` remains
the separate established-connection inactivity control.

Native embedded consumers may additionally set `PhysicalConnectionPolicy` and
`TransportIoTimeout`. The physical policy is applied by the common Hyper
connector wrapper: a permit is acquired only for a newly established
connection and is retained while that connection is active or idle in Hyper's
pool. HTTP/2 multiplexing therefore consumes one physical permit while
logical request permits remain independent. An admission timeout is reported
by `Error::is_physical_connection_admission_timeout()`.

The four resource controls are intentionally separate:

| Control | Resource covered |
|---|---|
| `PoolConfig::max_in_flight_requests*` (and legacy `max_connections*`) | Logical requests, one permit per in-flight request |
| `PoolConfig::max_idle_connections*` / `idle_timeout` | Hyper's retained idle connections |
| `PhysicalConnectionPolicy` | Live established Hyper connections, including idle pooled connections; one permit per HTTP/1 or HTTP/2 connection |
| `TransportIoTimeout` | Read/write inactivity after establishment, including buffered Hyper I/O |

`ClientBuilder::build()` remains infallible. `max_live = Some(0)` and an
`admission_timeout` without `max_live` are invalid configurations; requests
through the Hyper routes fail before connector I/O with a `Pool` error. The
physical admission metrics expose waits, timeouts, current live admitted
connections, and a high-water mark. The live gauge is meaningful when a
physical cap is enabled.

`TransportIoTimeout` wraps established Hyper I/O. Read and write inactivity
deadlines reset only on actual progress; they cover buffered Hyper writes and
connection reads without changing the meaning of request-body `Timeout.write`
or response-stream `Timeout.read`. DNS, TCP, custom dialing, and destination
TLS remain under the connect phase.

The lifecycle wrapper is installed after `ConnectTimeout`, so admission wait
does not consume the connect budget and DNS/TCP/custom dialing/destination
TLS remain connect-phase work. It is shared by standard, direct,
resolved-target, SNI, custom-dialer, UDS, SOCKS, HTTP forward-proxy, and
compatible HTTPS CONNECT Hyper clients. HTTP/3 uses QUIC and remains outside
this Hyper-specific policy. Proxy connector setup still reports proxy-connect
and proxy-TLS phases; a reused proxy connection does not rerun those phases.

### Error Model

Timeout errors carry phase identity:

```rust
Error::Timeout { phase: TimeoutPhase::Read, elapsed: Duration::from_secs(5) }
```

This enables Python bindings to map to specific exception classes (`ConnectTimeout`, `ReadTimeout`, etc.).

### Cancellation Safety

Cancelled timeout-wrapped operations release pool permits cleanly. The pool uses `OwnedSemaphorePermit` with RAII drop semantics.

### Multi-phase proxy deadlines

When a request has a total timeout, the proxy transport creates one monotonic
deadline at request dispatch. Proxy TCP connect, proxy TLS, CONNECT write/read,
origin TLS, and proxy request/response-header setup each use the smaller of
their configured phase budget and the remaining total budget. A phase never
receives a fresh copy of the original total duration. HTTPX compatibility
requests configure only connect/read/write/pool; native callers may set
`total` explicitly as the outer cap. For cached Hyper forward-proxy and
single-target CONNECT routes, the reusable connector has no request-total
state: `ConnectTimeout` owns configured connection establishment, while the
current request's outer `send_with_total_timeout` cancels a fresh connector
invocation when its remaining total budget expires. Multi-target fallback
continues to receive the request-local deadline for candidate sequencing.

## Connection Pool

The pool controls **logical in-flight request concurrency** (`max_in_flight_requests*`, aliases `max_connections*` for pre-1.0; new names win), not physical TCP connections. Hyper manages actual TCP connections internally; idle caps (`max_idle_connections*`, `idle_timeout`) are physical idle-pool policy.

This distinction is intentional: use `PoolConfig` for logical work and
`PhysicalConnectionPolicy` for an independent live-connection cap. Existing
`PoolConfig.max_connections` semantics are unchanged.

### Hyper Idle-Pool Policy

The physical idle policy is resolved once from `PoolConfig` and applied
uniformly to every persistent H1/H2 Hyper client family (standard, direct,
UDS, custom dialer, resolved-target, SNI, custom-SNI, SOCKS, HTTP
forward-proxy, and HTTPS CONNECT):

- `idle_timeout` (`Limits::keepalive_expiry`) — how long an idle connection
  is retained. When set, the shared Hyper builder policy also installs a
  `hyper_util::rt::TokioTimer`, which hyper-util requires for idle eviction
  to run in the background (the builder defaults to no timer). Without the
  timer an expired connection would linger until the next checkout happened
  to discard it; with it, idle sockets are closed proactively. `Timeout.pool`
  and `Timeout.total` remain acquisition/outer budgets and are never reused
  as idle policy.
- Effective per-host idle cap —
  `max_idle_connections_per_host.or(max_idle_connections)` (see
  `PoolConfig::effective_max_idle_per_host` / `Pool::max_idle_per_host`) —
  passed as Hyper's per-host idle limit. There is no global idle-connection
  cap: Hyper only supports per-host capping. A zero cap disables idle
  retention for that client. The cap needs no timer; Hyper enforces it
  synchronously when a connection goes idle.

Isolated one-shot clients (resolved target) receive the same values for
consistency even though they are not retained. The forward-proxy route keeps
its documented H1-only framing exception; H2-only selection and
canceled-request retry policy are unchanged. H3/QUIC idle policy stays with
the H3 connector and never takes the Hyper timer.

### Semaphore-Based Concurrency

The pool uses tokio semaphores to limit concurrent in-flight requests:
- **Global limit**: maximum concurrent logical requests across all origins.
- **Per-origin limit**: maximum concurrent logical requests to a single origin.

Under H1 one request typically owns its slot; under H2/H3 many permits multiplex over one connection/QUIC session. One permit never equals one TCP connection.

### Origin Keying

Per-origin pool limits are keyed by a composite `OriginKey`:

| Scenario | Key |
|----------|-----|
| Direct request | `(scheme, host, port)` |
| Proxied request | `(scheme, host, port, proxy_host, proxy_port, proxy_scheme, is_tunnel)` — the destination origin plus proxy endpoint and tunnel mode, so plain vs TLS-to-proxy and tunnel vs forwarding routes get independent slots |

Port uses the scheme's default when not explicit. Examples:
- `http://example.com:80` and `http://example.com` share a limit.
- `http://example.com` and `https://example.com` are independent.
- `http://example.com:8080` is distinct from `http://example.com`.
- Direct and proxied requests to the same destination have independent slots.

### PoolGuard

When a request acquires a pool slot, it receives a `PoolGuard` (wrapped in `Arc` for streaming responses). The guard holds the semaphore permit and releases it on drop. This ensures:
- Streaming responses hold their slot until fully consumed.
- Dropped responses release their slot immediately.
- Buffered responses release their slot after the body is collected.

### Pool Metrics vs Transport Metrics

`PoolMetrics` (logical) exposes:
- `acquisition_waits` — number of times a request waited for a slot.
- `acquisition_cancellations` — number of times a pool acquisition was cancelled.

`TransportMetrics` (`Client::transport_metrics()`, atomic, low-overhead) counts connector/protocol events where observable: direct/DNS/TLS attempts, UDS/proxy attempts, physical admission waits/timeouts/live/high-water values, established read/write inactivity timeouts, H3 creations/evictions, Alt-Svc learned/expired/cleared/rejected, H3 attempted/suppressed/fallback/drain/close/reconnect, and 101 upgrades. Names state whether they count connector events or protocol connections; logical requests stay in `PoolMetrics`.

Socket-level reuse counts (connections opened/reused/closed) and per-connection H2 stream counts remain absent because hyper owns socket lifecycle and eggfetch cannot observe reuse reliably — never estimated. See `transport/metrics.rs` and `tests/transport_metrics_tests.rs` + `tests/h3_alt_svc_discovery.rs` for exact-count evidence.

The same observability boundary applies to Hyper's canceled-request retry:
`ClientBuilder::retry_canceled_requests(false)` can prevent the hidden retry,
but `PoolMetrics` and `TransportMetrics` do not report Hyper's internal retry
or socket-reuse count. Use the native flag plus server-side instrumentation
when strict physical-attempt accounting is required. HTTP/3 is outside this
Hyper-specific control.

### H3 Pool and Idle Mapping

H3 requests acquire pool permits exactly like H1/H2 (one permit per
request, held on the streaming body until consumed or dropped), so the
effective per-origin in-flight limit gates H3 streams as logical
concurrency. The H3 origin cache (`host:port`, 64 entries) is separate
from the pool's per-origin semaphore table; creations/evictions are
counted in `TransportMetrics`.

Idle lifetime for H3 derives from `PoolConfig::idle_timeout`
(`Limits::keepalive_expiry`): 5 s under `Limits::compat()`, unset (idle
connections never proactively closed) under `Limits::native()`; `Timeout.pool` and
`Timeout.total` are acquisition/outer budgets and never close idle QUIC
connections. Physical QUIC stream caps derive from the effective
per-origin in-flight limit (`max_in_flight_requests_per_origin` or alias
`max_connections_per_host`, default 100 bidi) and are documented in
[core-tls-proxy-protocols.md](core-tls-proxy-protocols.md).

### Environment-Variable Proxy Policy

The Rust core does not read proxy environment variables; native proxy
configuration is explicit via `ClientBuilder::proxy()` or
`RequestBuilder::proxy()`. The HTTPX compatibility facade translates
scheme-specific proxy variables and `NO_PROXY` when `trust_env=True`.

For SOCKS requests, the client keeps a persistent Hyper client per effective
SOCKS route rather than constructing one during each request. The route cache
is owned by `ClientInner`, includes the proxy endpoint/scheme/authentication
identity in its key, and is released with the client. The request's total
timeout still wraps the full SOCKS connect, negotiation, origin TLS, and HTTP
exchange; cancellation drops the failed operation without invalidating a
follow-up connection on the route.
