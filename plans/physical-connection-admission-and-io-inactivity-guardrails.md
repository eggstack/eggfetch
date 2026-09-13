# Physical Connection Admission and Transport I/O Inactivity Guardrails

Planning baseline: `475bd50f6f9b9f66b95eea814f06b4adb22ede93` (`main`, 2026-09-13; eggfetch 0.1.4)
Parent program: `plans/extensible-embedded-transport-consumer-program.md`
Depends on: `plans/custom-dialer-transport-extension.md` so the lifecycle wrapper is applied to the final Hyper route set
Status: implemented; expanded deterministic qualification follow-up remains

## Objective

Add two opt-in connector-lifecycle controls for native embedded consumers without changing the meaning of existing logical pooling or request/body timeout APIs:

1. a hard bound on live physical Hyper connections, including idle pooled connections;
2. inactivity timeouts around actual established-connection reads and writes.

Both controls should be implemented at the common connector boundary so direct, custom-dialer and other Hyper-based routes receive equivalent semantics.

## Why a new boundary is required

`PoolConfig` deliberately tracks logical in-flight requests. Its `max_connections` field is a compatibility alias for logical concurrency, and HTTP/2 can multiplex multiple requests on one connection. Reinterpreting that field as a socket count would violate current documentation and existing callers.

Likewise, `Timeout.write` currently protects streamed request-body production, while `Timeout.read` protects response progress. Those request-level guards do not necessarily bound the raw socket write performed by Hyper for an already-buffered request. Broadening their behavior silently would make existing timeout configuration mean something different.

The new controls therefore need explicit names and opt-in defaults.

## 1. Introduce a physical connection policy

Add a small native configuration type, for example:

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct PhysicalConnectionPolicy {
    pub max_live: Option<usize>,
    pub admission_timeout: Option<Duration>,
}
```

Exact naming may differ, but avoid `max_connections` because that term already has documented logical semantics in `PoolConfig`.

Expose it through `ClientBuilder`, for example:

```rust
Client::builder()
    .physical_connection_policy(
        PhysicalConnectionPolicy::builder()
            .max_live(32)
            .admission_timeout(Duration::from_secs(30))
            .build(),
    )
```

Defaults must disable the physical cap and therefore preserve existing behavior.

Validation rules:

- zero live connections is invalid rather than a permanently blocked client;
- a timeout with no physical limit may be rejected or stored as inert only if clearly documented; prefer rejecting nonsensical combinations during builder/config validation where possible;
- no global static semaphore; each built `Client` owns its own policy/state;
- client clones share the same admission state.

## 2. Build one connector-lifecycle wrapper

Add a generic crate-internal connector wrapper around a fully configured underlying connector. It should compose after DNS/TCP/custom-dial and destination TLS establishment from the perspective of returned stream ownership, while its admission acquisition occurs before starting a new underlying connection.

Conceptually:

```text
Hyper asks connector for new connection
    |
    -> acquire physical permit (optional)
    -> call inner connector under existing connect timeout
       DNS / TCP / custom dial / destination TLS
    -> wrap returned established stream
         - hold permit for stream lifetime
         - optional read inactivity timer
         - optional write inactivity timer
    -> give wrapped connection to Hyper pool
```

The wrapper must be generic over the existing connector response rather than duplicating standard/direct/custom connector implementations.

The existing `ConnectTimeout<C>` can remain a separate wrapper or be composed with this lifecycle wrapper. Choose an order that preserves the documented connect timeout over DNS/TCP/TLS and does not let physical-admission wait consume an unrelated timeout accidentally.

## 3. Define physical admission timeout semantics

Physical admission wait is distinct from logical pool acquisition. It should have a distinct public setting and a truthful error path.

Required ordering:

```text
logical request permit acquisition
    -> physical connection needed?
       -> physical admission wait
       -> connect establishment
```

A request that reuses an existing pooled connection must not acquire a new physical permit or wait on the physical-admission semaphore.

A permit is acquired only when Hyper invokes the connector to create a connection and remains owned by that returned connection until it is dropped. This naturally means:

- active HTTP/1 connection counts as one;
- idle pooled HTTP/1 connection still counts as one;
- one HTTP/2 TCP connection carrying many logical streams counts as one;
- dropping/evicting/closing the pooled connection releases the permit;
- cancellation during connection establishment releases the permit;
- failed DNS/TCP/TLS/custom dialing releases the permit;
- client teardown drops pooled connections and eventually releases all permits.

Do not release the permit when one request finishes if Hyper retains the connection in its idle pool.

## 4. Error representation for physical admission

Do not reuse `TimeoutPhase::Pool` if that would falsely describe logical request pool waiting. Prefer a distinct additive error classification or a source-preserving helper that callers can identify without changing existing enum payloads.

Because the public `Error` enum is exhaustive, first audit source compatibility before adding a new variant or enum case. A safe implementation may use the existing `Error::Pool` with a stable message plus an additive `Error` query method such as `is_physical_connection_admission_timeout()` if that preserves compatibility better.

The important contract is that an embedding application can distinguish:

- logical request-pool wait timeout;
- physical connection-admission wait timeout;
- connect timeout;
- transport read/write inactivity.

Do not collapse all four into one opaque Hyper error string.

## 5. Add transport I/O inactivity configuration

Introduce a separate opt-in policy, for example:

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct TransportIoTimeout {
    pub read: Option<Duration>,
    pub write: Option<Duration>,
}
```

Expose it through `ClientBuilder` without changing `Timeout` defaults or semantics.

The terminology must make clear this is lower-level established-connection I/O, not request-body producer timing or response-body consumer timing.

Do not add a `total` field here. Overall request deadline already belongs to `Timeout.total`.

## 6. Place I/O timers after connection establishment

The read/write inactivity wrapper should operate on the connector's returned stream that Hyper actually reads/writes after the connection is established. This placement is important:

- DNS/TCP/custom dialing remains connect-phase work;
- destination TLS handshake remains connect-phase work under the existing connect timeout;
- established HTTP transport I/O uses the new read/write guardrails;
- the guardrails also cover Hyper writing a finite buffered request body, which is the gap this plan needs to close.

Do not wrap the raw custom dialer stream *before* destination TLS in a way that reclassifies a slow TLS handshake as transport read/write inactivity.

## 7. Timer semantics

The established connection wrapper should reset inactivity deadlines on actual progress.

### Read

- start/reset timer when Hyper is waiting for read progress;
- any successful read of >0 bytes resets it;
- normal EOF is not a timeout;
- `Poll::Pending` until deadline then returns a deterministic timeout error;
- repeated wakeups without byte progress do not reset the deadline.

### Write

- start/reset timer when Hyper has bytes to write and the underlying stream cannot make progress;
- any successful write of >0 bytes resets it;
- `poll_flush`/`poll_shutdown` need deliberate semantics: either share the write inactivity guard or document why they are outside it;
- zero-length writes must not create/reset a spurious timer;
- vectored writes receive equivalent behavior.

Use `tokio::time::Sleep` or existing timer primitives; do not spawn watchdog tasks per connection.

## 8. Interaction with existing request-level timeouts

Existing timeout behavior stays in place. When both layers are configured, the earliest applicable deadline may fail the operation.

Examples:

- request body producer stalls before yielding bytes -> existing `Timeout.write`;
- body is buffered, Hyper has bytes, peer stops receiving -> new transport write inactivity timeout;
- response server sends no headers -> existing read/connect/header path remains authoritative according to current pipeline;
- established response body stream stops delivering because socket has no bytes -> either existing response read guard or transport read guard may fire first, depending on configured durations;
- total deadline remains outermost wall-clock bound.

Document overlapping timers rather than trying to infer one universal timeout source.

## 9. Apply lifecycle policy to every relevant Hyper route

Audit and cover:

- standard HTTP/HTTPS connector;
- advanced direct connector (local bind/socket options);
- SNI-keyed connector;
- request-scoped resolved-target connector;
- UDS if the policy is defined as "physical Hyper connection" rather than specifically TCP;
- SOCKS/built-in proxy connectors;
- custom-dialer connector;
- any isolated Hyper clients created for special routing.

Choose and document whether UDS counts toward `max_live`. A consistent "live Hyper transport connection" definition is preferable to pretending the cap is strictly TCP if it is applied to UDS too.

HTTP/3/QUIC is outside this plan unless explicitly added. Do not claim one `max_live` policy spans Quinn and Hyper without implementation evidence.

## 10. Preserve logical pool semantics

No field in `PoolConfig` changes meaning. Update docs to make the distinction even clearer:

```text
PoolConfig max_in_flight_*     logical request concurrency
PoolConfig max_idle_*          Hyper idle-pool retention policy
PhysicalConnectionPolicy       live established Hyper connection count
TransportIoTimeout             established stream progress guardrail
```

Do not make the logical pool acquire a physical permit preemptively; that would over-count HTTP/2 streams and requests that reuse existing connections.

## 11. Metrics and observability

Extend existing transport metrics only where the values can be observed exactly at the lifecycle wrapper.

Useful bounded counters/gauges include:

- physical admission waits;
- physical admission timeouts;
- current live admitted connections;
- high-water live admitted connections;
- transport read inactivity timeouts;
- transport write inactivity timeouts.

Do not infer socket reuse counts from logical request permits. Do not add per-origin unbounded metric maps solely for this feature.

If current `TransportMetrics` snapshot structure is public and exhaustive, audit source compatibility before adding public fields. An additive separate sub-snapshot or methods may be safer.

## 12. Deterministic tests — physical admission

Add local tests proving:

1. `max_live = 1` allows one physical connection;
2. a second simultaneous connection waits even when logical in-flight limit is higher;
3. releasing/dropping the first connection admits the next;
4. an idle pooled connection retains its permit;
5. reusing an existing idle connection does not acquire another permit;
6. failed connect releases its permit;
7. cancelled connect releases its permit;
8. admission timeout is distinguishable from connect timeout;
9. HTTP/2 multiple logical streams do not each consume physical permits;
10. custom-dialer and standard routes follow the same lifecycle policy within their respective clients;
11. client teardown does not leak permits/tasks.

Avoid brittle timing assertions; use paused Tokio time or synchronization barriers where feasible.

## 13. Deterministic tests — transport inactivity

Add controlled local streams/servers proving:

1. buffered request to a peer that stops reading triggers transport write inactivity;
2. a stream that makes periodic write progress resets the deadline;
3. read stall after establishment triggers transport read inactivity;
4. periodic read progress resets the deadline;
5. EOF is not a timeout;
6. connect/TLS stall remains classified by connect timeout rather than transport I/O timeout;
7. existing streamed-body producer timeout remains unchanged;
8. disabling transport I/O timeouts preserves current behavior;
9. vectored write path cannot bypass the timer;
10. cancellation drops timers/permits without background work.

## 14. Documentation

Update:

- `docs/architecture/core-timeout-pool.md` with the four-way logical/idle/physical/I/O distinction;
- `docs/architecture/core-engine.md` with connector wrapper ordering;
- `docs/rust/guide.md` with embedded-resource-control examples;
- rustdoc for new policy types and builder methods;
- metrics docs if the observable surface changes.

Do not market the physical cap as an HTTP/2 stream limit; it is specifically a connection-resource control.

## Dependency and performance policy

No new runtime dependency should be necessary. Use Tokio semaphores/timers already in the graph.

The disabled/default path should have negligible overhead. If no physical cap and no transport I/O timeout are configured, prefer construction that either omits the wrapper or leaves only a simple delegation layer. Measure before introducing micro-optimizations.

Do not add a benchmark framework. Existing resource/benchmark tools are sufficient if measurement is warranted.

## Required validation

Run focused pool/timeout/connector tests, all affected HTTP/2/proxy/custom-dialer tests, then:

```sh
cargo test -p eggfetch-core
cargo test -p eggfetch-core --all-features
./scripts/check.sh
```

The parent closure plan owns extended/package/exact-SHA requalification.

## Non-goals

- no reinterpretation of `PoolConfig.max_connections`;
- no change to existing `Timeout` field meanings;
- no HTTP/2 stream scheduler;
- no HTTP/3/QUIC connection admission in this plan;
- no per-provider/account resource policy;
- no global process-wide connection semaphore;
- no background watchdog tasks;
- no new CI workflow or benchmark dependency.

## Exit criteria

- [x] native callers can opt into a hard live physical Hyper-connection cap;
- [x] idle pooled connections retain physical permits until actually dropped;
- [x] HTTP/2 logical streams do not consume one permit each;
- [x] physical-admission wait/timeout is distinct from logical pool and connect waits;
- [x] native callers can opt into established transport read/write inactivity guards;
- [x] buffered socket writes are covered by the write guardrail;
- [x] TLS/connect establishment remains under connect timeout semantics;
- [x] existing `PoolConfig` and `Timeout` behavior is unchanged when new policies are disabled;
- [x] all Hyper-based routes are audited for consistent wrapper application;
- [x] metrics are truthful and bounded;
- [ ] deterministic tests cover admission, reuse, idle retention, failure, cancellation and I/O progress reset;
- [x] routine validation passes.

Implementation note: the focused unit coverage proves permit retention and invalid-policy handling; the broader admission/reuse/progress-reset matrix remains a follow-up recorded by the parent closure plan.
