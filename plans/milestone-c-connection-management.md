# Milestone C Plan: Connection Management

## Objective

Turn connection handling into a deliberate, testable subsystem instead of incidental behavior hidden inside the low-level HTTP client. eggfetch needs predictable connection reuse, bounded resource consumption, explicit pool acquisition behavior, and a foundation that can later support proxies, HTTP/2 multiplexing, HTTP/3, alternate DNS, and Python sync/async wrappers.

This milestone should preserve the single async Rust engine. Python sync behavior later must block on this same pool rather than creating separate sync connections.

## Scope

Milestone C includes:

- connection pool configuration
- idle connection reuse
- max idle connection policy
- max total connection policy if practical
- per-host limit policy if practical
- pool acquisition behavior
- graceful client shutdown semantics
- connection close behavior
- instrumentation hooks or internal counters for tests
- concurrency tests

Milestone C does not include:

- final timeout taxonomy, except pool acquisition must be designed for Milestone D
- proxy tunnels
- SOCKS
- redirect behavior
- HTTP/2 multiplexing policy beyond not blocking future support
- Python bindings
- CLI behavior

## Design goals

Connection management should be explicit and observable enough to test. The public API does not need to expose every internal metric, but tests should be able to verify whether reuse and limits work.

The pool should be owned by `Client`. Cloning a `Client` should share the same underlying pool and configuration. Dropping all client handles should eventually release idle connections.

Top-level one-shot requests in future Python bindings may create short-lived clients, but persistent `Client` objects must reuse connections.

## Configuration surface

Add connection-related options to `ClientBuilder`.

Suggested options:

```rust
Client::builder()
    .max_idle_connections(usize)
    .max_idle_connections_per_host(usize)
    .max_connections(usize)
    .max_connections_per_host(usize)
    .idle_timeout(Duration)
    .build()?;
```

If some limits are difficult to enforce cleanly with the selected hyper integration, document the limitation and implement the subset that is reliable. Do not expose configuration that does not work.

Required minimum:

- idle timeout
- max idle connections per host or equivalent
- clear reuse behavior

Preferred for MVP:

- max total in-flight connections
- max per-host in-flight connections
- acquisition waits when the pool is saturated

## Pool acquisition model

A request should acquire a connection or connection slot before network execution. If the pool is saturated, the request should wait until a slot is available or until the future is cancelled.

Milestone D will add a pool acquisition timeout. Milestone C should therefore structure acquisition so it can be wrapped cleanly by a timeout later.

Do not busy-wait. Use async synchronization primitives.

## Interaction with Hyper

Hyper and hyper-util provide connection/client machinery, but eggfetch should still own the user-facing pooling policy. The implementation should inspect how much policy can be delegated safely and where eggfetch needs an outer semaphore or pool manager.

A practical implementation may use:

- hyper/hyper-util for protocol-level connection reuse
- eggfetch-owned semaphores for max in-flight constraints
- eggfetch-owned config and lifecycle state

Avoid depending on undocumented behavior.

## HTTP/1.1 behavior

For HTTP/1.1, verify:

- keep-alive reuse happens for compatible responses
- `Connection: close` prevents reuse
- server-side close is handled gracefully
- dropped response bodies do not poison future requests silently
- unread bodies are drained or the connection is discarded according to safe behavior

If a response body is not fully consumed, the connection generally should not be reused unless the body is drained safely. Make this behavior explicit.

## Future HTTP/2 considerations

HTTP/2 allows multiplexing many streams over one TCP/TLS connection. The pool abstraction should not assume one request equals one connection forever.

Use terminology such as connection slot and request permit carefully. If necessary, split:

- request concurrency limits
- connection count limits
- per-origin limits

For Milestone C, it is acceptable to implement HTTP/1.1-oriented behavior while keeping the type names and boundaries general.

## Graceful shutdown

Add a client close/shutdown concept if needed.

Rust API candidates:

```rust
client.close().await;
```

or rely on drop for now while preparing explicit close for Python context managers later.

Python bindings will need:

```python
with Client() as client:
    ...

async with AsyncClient() as client:
    ...
```

Therefore the core should eventually support deterministic cleanup. Milestone C should define and document how cleanup works, even if explicit close is minimal initially.

## Internal observability

Add test-only or crate-private metrics sufficient to verify behavior.

Potential metrics:

- connections opened
- connections reused
- idle connections closed
- acquisition waits
- acquisition cancellations

Avoid stabilizing these as public API yet unless they are clearly useful.

## Tests

Build concurrency and lifecycle tests against a local server.

Required tests:

- repeated requests to the same origin reuse a connection
- different origins do not share a connection incorrectly
- `Connection: close` response is not reused
- dropped client releases idle resources eventually
- max concurrency limit blocks additional requests
- cancelled waiting acquisition releases its waiter cleanly
- unread response body does not cause corrupted reuse
- idle timeout closes stale idle connection

Suggested local test server behavior:

- count accepted TCP connections
- expose request IDs per connection
- optionally delay responses to test saturation
- optionally close connections after response

## Error behavior

Connection acquisition failure should map to a specific eggfetch error. In Milestone C this may be generic. Milestone D should refine timeout-specific acquisition errors.

Connection reset, EOF, broken pipe, and protocol-level errors should preserve sources and map to stable public variants where possible.

## Documentation updates

Update architecture docs to explain:

- Client owns shared connection state
- cloned clients share pools
- one-shot helpers create short-lived clients later
- response body consumption affects reuse
- sync Python wrappers will not create a separate networking path

## Acceptance criteria

Milestone C is complete when:

- `Client` has documented pool-related behavior
- repeated requests reuse connections in tests
- idle timeout behavior is tested
- connection close behavior is tested
- saturated pool acquisition behavior is tested if limits are implemented
- cancellation of acquisition waiters is safe
- body consumption versus reuse behavior is documented
- `cargo test -p eggfetch-core` passes reliably

## Risks

The largest risk is fighting hyper's internal pooling rather than layering policy cleanly around it. Prefer a conservative first version with a clear documented subset over a brittle over-customized pool.

Another risk is using terms that become wrong under HTTP/2. Keep abstractions origin- and transport-aware enough that multiplexed protocols can be added later.

## Handoff notes

Do not start Python bindings until the client lifecycle is stable. Python context managers and top-level helper functions will force lifecycle semantics into the public API, so the core behavior should be correct first.
