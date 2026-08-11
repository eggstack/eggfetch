# Timeout & Pool Deep Dive

This document covers the phase-aware timeout system and the semaphore-based connection pool.

See also: [overview.md](overview.md), [core-engine.md](core-engine.md).

## Timeout System

eggfetch implements phase-aware timeouts that map to specific segments of the request lifecycle.

### Phases

| Phase | What It Covers |
|-------|----------------|
| `Pool` | Waiting for a connection slot from the concurrency pool |
| `Connect` | TCP connection establishment + TLS handshake (including DNS) |
| `ProxyConnect` | Waiting for proxy to establish CONNECT tunnel |
| `ProxyTls` | TLS handshake over proxy tunnel |
| `Write` | Sending request headers and body |
| `Read` | Waiting for response headers or body chunks |
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
| Read | Per-chunk wrapper stream (`ReadTimeoutStream`) — deadline resets on each chunk |
| Write | Per-chunk wrapper stream (`WriteTimeoutStream`) — deadline resets on each chunk delivery |
| Connect | Accepted and merged but not independently enforced by hyper-util. `total` should be used as a backstop |

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
origin TLS, and proxy request/response-header setup each derive their remaining
time from that deadline. A phase never receives a fresh copy of the original
total duration. Phase-specific errors retain the phase that exhausted the
shared budget.

## Connection Pool

The pool controls **logical request concurrency**, not physical TCP connections. hyper manages actual TCP connections internally.

### Semaphore-Based Concurrency

The pool uses tokio semaphores to limit concurrent in-flight requests:
- **Global limit**: maximum concurrent requests across all origins.
- **Per-origin limit**: maximum concurrent requests to a single origin.

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

### Pool Metrics

`PoolMetrics` exposes:
- `acquisition_waits` — number of times a request waited for a slot.
- `acquisition_cancellations` — number of times a pool acquisition was cancelled.

Socket-level counters (connections opened/reused/closed) were removed because hyper owns socket lifecycle and eggfetch cannot observe individual socket events.

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
