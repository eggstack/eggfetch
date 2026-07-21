# Connection Pooling

eggfetch uses a semaphore-based concurrency pool to limit how many requests may be in flight simultaneously. The pool does not manage TCP connections directly; hyper handles connection lifecycle and reuse internally.

## Origin Keying

Per-origin limits are keyed by `(scheme, host, port)`, where the port uses the scheme's default when not explicit.

- `http://example.com:80` shares a limit with `http://example.com` (default port 80)
- `http://example.com` and `https://example.com` are distinct origins
- `http://example.com:8080` is distinct from `http://example.com`

When a proxy is involved, the pool key extends to `(proxy_origin, destination_origin, tunnel_mode)`. Direct and proxied requests to the same destination have independent concurrency slots. HTTP forwarding and HTTPS CONNECT tunneling through the same proxy are keyed separately.

### Examples

| URL pair | Same origin? |
|----------|-------------|
| `http://a.com` and `http://a.com:80` | Yes (same default port) |
| `http://a.com` and `https://a.com` | No (different scheme) |
| `http://a.com` and `http://a.com:8080` | No (different port) |
| `http://a.com` via proxy-a and `http://a.com` via proxy-b | No (different proxy) |
| `http://a.com` HTTP forward and `http://a.com` HTTPS tunnel | No (different tunnel mode) |

## Per-Origin vs Global Limits

The pool enforces two levels of concurrency:

- **Per-origin**: limits how many requests can target a single `(scheme, host, port)` simultaneously
- **Global**: limits total concurrent requests across all origins

A request must acquire both a per-origin permit and a global permit before proceeding. Permits are RAII guards: dropping the response body or the `PoolGuard` releases the slot.

Per-origin semaphores are created lazily on first use for each origin. The global semaphore, if configured, is created at pool construction time.

## HTTP/2 Multiplexing

Under HTTP/2, multiple logical requests share a single TCP connection via stream multiplexing. The pool's per-origin limit still applies: it bounds the number of concurrent requests, not the number of connections.

If the server's `SETTINGS_MAX_CONCURRENT_STREAMS` limit is reached, hyper internally queues streams until a slot opens. This may cause pool acquisition to wait even though logical permit slots are available. The server's stream limit is respected by h2 internally; eggfetch does not override it.

The h2 library enforces a default of 100 concurrent streams per connection, or whatever the server advertises via `SETTINGS_MAX_CONCURRENT_STREAMS`. This is transparent to eggfetch.

## HTTP/3 and QUIC

When HTTP/3 is selected, requests bypass the hyper transport and are sent over QUIC via Quinn. These requests still acquire pool permits for concurrency limiting, but the underlying QUIC connection lifecycle is managed independently by the H3 connector.

Quinn enforces a maximum of 100 concurrent bidirectional streams per QUIC connection by default. This is independent of the pool's logical permit limit. The H3 connector maintains a per-origin cache of QUIC connections for reuse.

## Idle Timeout and Eviction

The `idle_timeout` configuration controls how long an idle connection remains in the pool. Expired connections are evicted. The `max_idle_connections` and `max_idle_connections_per_host` settings limit the number of idle connections kept for reuse.

These settings control hyper's internal connection pool, not eggfetch's concurrency semaphores. eggfetch delegates connection lifecycle management to hyper.

## Pool Metrics

`PoolMetrics` exposes observable counters:

- `acquisition_waits` -- total times an acquire call had to wait for a permit
- `acquisition_cancellations` -- total times an acquire call was cancelled while waiting

These track logical permits, not raw TCP sockets. Socket-level counters are not exposed because hyper owns socket lifecycle. Under HTTP/2, a single TCP connection may carry multiple concurrent streams, but the pool still tracks one permit per logical request.

## Configuration

Pool behavior can be configured via `PoolConfig` directly or via the higher-level `Limits` type.

### Using Limits

The `Limits` type provides HTTPX-compatible resource limits:

```rust
use eggfetch_core::Limits;

// HTTPX-compatible defaults: 100 max connections, 20 idle, 5s keepalive
let limits = Limits::compat();

let client = Client::builder()
    .limits(limits)
    .build();
```

| Constructor | max_connections | max_idle | keepalive_expiry |
|-------------|----------------|----------|------------------|
| `Limits::compat()` | 100 | 20 | 5s |
| `Limits::native()` | None (unlimited) | None | None |

### Using PoolConfig

For finer control, use `PoolConfig` directly:

```rust
use eggfetch_core::pool::PoolConfig;

let config = PoolConfig {
    max_idle_connections: Some(100),
    max_idle_connections_per_host: Some(10),
    max_connections: Some(200),
    max_connections_per_host: Some(20),
    idle_timeout: Some(Duration::from_secs(90)),
};
```

| Field | Description |
|-------|-------------|
| `max_idle_connections` | Maximum idle connections to keep in the pool |
| `max_idle_connections_per_host` | Maximum idle connections per origin |
| `max_connections` | Maximum total concurrent requests |
| `max_connections_per_host` | Maximum concurrent requests per origin |
| `idle_timeout` | Duration after which idle connections are closed |

All fields are optional. When `None`, the corresponding limit is not applied. The default `PoolConfig` has all fields set to `None`, meaning unlimited concurrency.

## Cancellation Safety

Cancelled operations release pool permits cleanly. The pool uses `OwnedSemaphorePermit` with RAII drop semantics, so dropping a future mid-acquisition returns the slot to the pool. This ensures that cancelled requests do not permanently consume concurrency slots.

When a request times out or is cancelled during body streaming, the `PoolGuard` is dropped along with the response body, releasing both the per-origin and global permits.

## Default Behavior

When no pool configuration is provided, there are no concurrency limits. Every request acquires and immediately releases a trivial permit. This means unlimited concurrent requests are allowed by default.

For production use, it is recommended to configure explicit limits to prevent overwhelming servers:

```rust
let client = Client::builder()
    .pool_config(PoolConfig {
        max_connections: Some(100),
        max_connections_per_host: Some(10),
        ..Default::default()
    })
    .build();
```

## Relationship to Connection Reuse

The pool controls logical request concurrency, not physical TCP connections. Connection reuse is handled by hyper's internal connection pool. Under HTTP/1.1, hyper may keep a connection alive for reuse. Under HTTP/2, a single connection carries multiple multiplexed streams.

eggfetch does not expose direct control over TCP connection lifecycle. The idle timeout and idle connection limits configure hyper's internal connection pool, which operates independently of eggfetch's concurrency semaphores.
