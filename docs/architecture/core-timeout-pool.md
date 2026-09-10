# Timeout & Pool Deep Dive

This document covers the phase-aware timeout system and the semaphore-based connection pool.

See also: [overview.md](overview.md), [core-engine.md](core-engine.md).

## Timeout System

eggfetch implements phase-aware timeouts that map to specific segments of the request lifecycle.

### Phases

| Phase | What It Covers |
|-------|----------------|
| `Pool` | Waiting for a connection slot from the concurrency pool |
| `Connect` | TCP connection establishment + TLS handshake (including DNS); for proxy routes, also proxy TCP/TLS setup and origin TLS after CONNECT |
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
`total` explicitly as the outer cap.

## Connection Pool

The pool controls **logical in-flight request concurrency** (`max_in_flight_requests*`, aliases `max_connections*` for pre-1.0; new names win), not physical TCP connections. Hyper manages actual TCP connections internally; idle caps (`max_idle_connections*`, `idle_timeout`) are physical idle-pool policy.

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
| Proxied request | `(proxy_origin, destination_origin, tunnel_mode)` |

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

`TransportMetrics` (`Client::transport_metrics()`, atomic, low-overhead) counts connector/protocol events where observable: direct/DNS/TLS attempts, UDS/proxy attempts, H3 creations/evictions, Alt-Svc learned/expired/cleared/rejected, H3 attempted/suppressed/fallback/drain/close/reconnect, and 101 upgrades. Names state whether they count connector events or protocol connections; logical requests stay in `PoolMetrics`.

Socket-level reuse counts (connections opened/reused/closed) and per-connection H2 stream counts remain absent because hyper owns socket lifecycle and eggfetch cannot observe reuse reliably — never estimated. See `transport/metrics.rs` and `tests/transport_metrics_tests.rs` + `tests/h3_alt_svc_discovery.rs` for exact-count evidence.

### H3 Pool and Idle Mapping

H3 requests acquire pool permits exactly like H1/H2 (one permit per
request, held on the streaming body until consumed or dropped), so the
effective per-origin in-flight limit gates H3 streams as logical
concurrency. The H3 origin cache (`host:port`, 64 entries) is separate
from the pool's per-origin semaphore table; creations/evictions are
counted in `TransportMetrics`.

Idle lifetime for H3 derives from `PoolConfig::idle_timeout`
(`Limits::keepalive_expiry`), defaulting to 30 s; `Timeout.pool` and
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
