# Timeouts

eggfetch implements phase-aware timeouts that map to specific segments of the request lifecycle. Each phase has its own optional duration, and timeout errors identify which phase failed.

## Timeout Phases

| Phase | What it measures |
|-------|-----------------|
| `Pool` | Time waiting for a connection slot from the pool |
| `Connect` | Time to establish TCP connection and TLS handshake (including DNS) |
| `ProxyConnect` | TCP connection to the proxy server (proxy requests only) |
| `ProxyTls` | TLS handshake over a CONNECT tunnel (HTTPS through HTTP proxy only) |
| `Write` | Per-chunk time for the request body producer to yield the next chunk |
| `Read` | Per-chunk time to wait for response headers or a body chunk |
| `Total` | Wall-clock cap across the entire request lifecycle |

## Default Behavior

All timeout phases are disabled by default. A `Timeout` with no fields set allows requests to run indefinitely (subject to OS-level TCP timeouts).

## Scalar vs Per-Phase Configuration

A scalar timeout sets pool, connect, write, and read phases to the same duration. The total timeout is not set by scalar constructors.

```rust
use eggfetch_core::Timeout;

// All phases except total get 5 seconds
let t = Timeout::from_secs(5);
```

Use the builder for per-phase control:

```rust
let t = Timeout::builder()
    .pool(Duration::from_secs(2))
    .connect(Duration::from_secs(5))
    .read(Duration::from_secs(30))
    .total(Duration::from_secs(60))
    .build();
```

## Client-Level vs Request-Level

Timeouts are configured at two levels:

- **Client-level**: `ClientBuilder::timeout(timeout)` sets defaults for all requests
- **Request-level**: `RequestBuilder::timeout(timeout)` overrides client defaults per-field

Request-level overrides are per-field: only fields present in the request-level `Timeout` replace the corresponding client-level fields. Fields set to `None` at the request level preserve the client-level value.

```rust
// Client: 10 seconds for everything
let client = Client::builder()
    .timeout(Timeout::from_secs(10))
    .build();

// Request: override only read timeout to 30 seconds
let response = client
    .get("https://example.com")
    .timeout(Timeout {
        read: Some(Duration::from_secs(30)),
        ..Timeout::default()
    })
    .send()
    .await?;
```

## Enforcement Details

- **Pool** and **Total** are enforced with `tokio::time::timeout`
- **Read** is enforced per chunk by a wrapper stream that fires an error if no chunk arrives within the duration. The deadline resets on every chunk arrival
- **Write** is enforced per chunk by a wrapper stream that fires an error if the body producer does not yield a chunk within the duration. The deadline resets on every chunk delivery. Only applies to streamed request bodies; buffered bodies complete synchronously
- **Connect** is accepted and merged but not independently enforced because hyper-util does not expose a per-connect deadline. Use `total` as a backstop

## Timeout Errors

Timeout errors carry phase identity:

```rust
Error::Timeout {
    phase: TimeoutPhase::Read,
    elapsed: Duration::from_secs(5),
}
```

In Python, this maps to specific exception classes: `PoolTimeout`, `ConnectTimeout`, `ReadTimeout`, `WriteTimeout`.

## Python API

```python
import eggfetch

# Scalar timeout (all phases)
client = eggfetch.Client(timeout=10.0)

# Per-phase timeout
t = eggfetch.Timeout(
    pool=2.0,
    connect=5.0,
    read=30.0,
    total=60.0,
)
client = eggfetch.Client(timeout=t)
```
