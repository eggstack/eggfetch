# Milestone D Plan: Timeout System

## Objective

Implement a precise, phase-aware timeout system for eggfetch. Timeout behavior is a core compatibility feature for a requests/httpx-like Python API and a correctness requirement for high-concurrency Rust usage.

Timeouts must be implemented in the async Rust engine only. Python sync and async APIs should later expose the same configuration and receive the same error classification.

## Scope

Milestone D includes:

- timeout configuration model
- connect timeout
- pool acquisition timeout
- write timeout
- read timeout
- optional total request timeout
- timeout error taxonomy
- cancellation safety review
- tests for each timeout phase
- documentation of timeout semantics

Milestone D does not include:

- retries
- redirects
- Python exception mapping, except designing stable variants for it
- CLI flags, except documenting future mapping
- advanced adaptive timeout behavior

## Compatibility target

HTTPX exposes strict timeout behavior with multiple phases. eggfetch should follow this model conceptually because it is more precise than a single request deadline.

The project should model at least:

- connect timeout: time allowed to establish TCP/TLS connection
- pool timeout: time allowed to acquire a connection or request permit
- write timeout: time allowed while sending request body data
- read timeout: time allowed while waiting for response headers or response body chunks
- total timeout: optional wall-clock cap across the whole request

If total timeout conflicts with phase timeouts, the first elapsed constraint should determine the error when it can be identified reliably.

## Public Rust API

Add a `Timeout` configuration type.

Suggested shape:

```rust
#[derive(Clone, Debug)]
pub struct Timeout {
    pub connect: Option<Duration>,
    pub pool: Option<Duration>,
    pub write: Option<Duration>,
    pub read: Option<Duration>,
    pub total: Option<Duration>,
}
```

Also support convenience constructors:

```rust
Timeout::disabled()
Timeout::from_secs(5)
Timeout::default()
Timeout::builder()
```

`Timeout::from_secs(5)` should probably mean each phase gets five seconds, mirroring user expectations from requests-like APIs, while total timeout should be explicit unless a compatibility layer chooses otherwise.

Add configuration at both client and request level:

```rust
Client::builder().timeout(Timeout::from_secs(10)).build()?;
client.get(url).timeout(Timeout::from_secs(2)).send().await?;
```

Request-level timeout overrides client-level timeout.

## Python-facing future shape

Do not implement Python in this milestone, but design the model so later bindings can expose:

```python
eggfetch.get(url, timeout=5.0)
eggfetch.Timeout(5.0)
eggfetch.Timeout(connect=2.0, read=10.0, write=10.0, pool=1.0)
```

The Rust model should be expressive enough to represent that cleanly.

## Error taxonomy

Add a stable timeout error variant that identifies the phase.

Suggested shape:

```rust
pub enum TimeoutPhase {
    Connect,
    Pool,
    Write,
    Read,
    Total,
}

pub enum Error {
    Timeout { phase: TimeoutPhase },
    // ...
}
```

Preserve source errors where relevant, but a true elapsed timeout should be distinguishable from network EOF, reset, TLS failure, or protocol error.

Python will later map these to exception classes similar to:

- `TimeoutException`
- `ConnectTimeout`
- `PoolTimeout`
- `WriteTimeout`
- `ReadTimeout`

Therefore the Rust classification must be stable.

## Implementation strategy

Use `tokio::time::timeout` or equivalent around each phase, but avoid wrapping too broad a future in a way that loses phase identity.

Recommended phase boundaries:

1. Pool acquisition.
2. TCP connection establishment if eggfetch controls it directly.
3. TLS handshake if not included in connect phase already.
4. Request body write operations.
5. Response header wait.
6. Response body chunk reads.
7. Total request wrapper.

The exact placement depends on hyper integration. Where hyper hides some details, document limitations and preserve the best possible classification.

## Pool timeout

Pool timeout depends on Milestone C acquisition structure.

Expected behavior:

- if a connection/request permit is immediately available, no wait occurs
- if saturated, the future waits
- if the pool timeout elapses, return `TimeoutPhase::Pool`
- if the waiter is cancelled, it must not leak a permit or waiter entry

Tests must verify cancellation safety.

## Connect timeout

Connect timeout should include TCP connection establishment and probably TLS handshake, unless the implementation can separate them reliably.

Document whether DNS resolution is included. Initially, if using the platform/hyper path where DNS is part of connect, include DNS in connect timeout.

Future resolver work may split DNS timeout, but do not add that complexity now.

## Write timeout

Write timeout applies when transmitting request headers/body to the peer.

For simple byte bodies this may be difficult to trigger because writes may complete quickly. Design the body abstraction and test harness so slow request-body consumption can be simulated.

For streamed request bodies later, write timeout should apply per write/chunk rather than only to the entire body unless total timeout is used.

## Read timeout

Read timeout should apply to:

- waiting for response headers
- waiting for each response body chunk

This is important for streaming. A response that sends one byte every interval shorter than read timeout should continue unless total timeout is reached. A stalled body should trigger read timeout.

## Total timeout

Total timeout is optional but useful. If implemented now, it should wrap the entire request lifecycle from before pool acquisition until response body buffering completes for buffered responses.

For streaming responses, total timeout semantics are trickier. The total timeout may either apply until response headers are received or across the full body stream. Decide and document the behavior. A conservative initial approach is to apply total timeout to buffered request completion and leave streaming total behavior for Milestone E.

## Tests

Create local tests for each phase.

Required tests:

- pool timeout with saturated pool
- connect timeout against unroutable or controlled delayed connect target if reliable
- read timeout waiting for response headers
- read timeout waiting for delayed response body chunk
- no read timeout when chunks arrive within the allowed interval
- request-level timeout overrides client default
- disabled timeout does not prematurely fail local delayed responses
- timeout error reports the expected phase
- cancelled timeout-wrapped operation does not leak pool permits

Avoid flaky tests. Prefer controlled local servers over external network targets.

## Documentation

Document:

- default timeout values
- how scalar timeouts map to phases
- how request-level overrides work
- what each phase means
- whether DNS is part of connect timeout
- read timeout per-chunk semantics
- total timeout semantics
- cancellation behavior

## Acceptance criteria

Milestone D is complete when:

- `Timeout` configuration is public and documented
- client-level and request-level timeout configuration works
- pool, connect, write, read, and total phases are represented in the type system, even if one phase has documented implementation limits
- timeout errors include phase identity
- local tests cover pool and read timeouts robustly
- connect/write timeout behavior is tested where practical or explicitly deferred with implementation notes
- no timeout path leaks connection permits or corrupts client state

## Risks

The main risk is imprecise phase classification due to hyper abstraction boundaries. Do not fake precision. If a phase cannot yet be isolated, document it and return the most honest error.

The second risk is flaky timeout tests. Keep durations generous and use deterministic local synchronization where possible.

## Handoff notes

Once this milestone lands, the Python sync adapter can expose requests-like scalar timeouts without inventing behavior. Avoid starting Python exception mapping before the Rust error taxonomy is stable.
