# Native Protocol Observability and API Cleanup

Planning baseline: `48f4457069d77528bd21fb342ff6ee4f27e3ae4f` (`main`, 2026-09-04)
Parent program: `plans/post-audit-architecture-and-surface-maturation-program.md`
Depends on: `plans/core-request-and-transport-consolidation.md`
Recommended after: `plans/http3-lifecycle-and-policy-hardening.md`

## Objective

Close narrow protocol and diagnostics gaps that now matter more than broad feature acquisition: add HTTP trailer support at the core response boundary, expose truthful connection/protocol metadata and transport metrics where the connector architecture can actually observe them, and clarify native concurrency-limit terminology before API stabilization.

This plan must not synthesize metadata that Hyper/Quinn cannot expose and must not break the HTTPX facade's public naming contract.

## 1. Add HTTP trailer support to the core response model

The current Hyper incoming-body adapter discards trailer frames. Replace that limitation with an explicit trailer-capable response lifecycle.

Required semantics:

- preserve data-frame streaming exactly as today;
- capture trailing headers when the body protocol yields them;
- make trailers available only after they have arrived / the body has advanced far enough;
- support HTTP/1.1 chunked trailers and HTTP/2 trailing HEADERS where Hyper exposes them;
- define H3 trailer behavior consistently where the h3 crate exposes trailing headers;
- do not eagerly buffer the entire body to obtain trailers;
- preserve cancellation and pool-lease lifetime.

The exact API may be `Response::trailers()` / async retrieval or an internal trailer store populated during streaming, but it must make lifecycle constraints explicit.

Acceptance:

- [ ] H1 chunked trailer fixture preserves duplicate/multi-value trailing headers correctly.
- [ ] H2 trailing HEADERS are surfaced.
- [ ] ordinary responses without trailers behave unchanged.
- [ ] partial consumption does not falsely report trailers that have not arrived.
- [ ] body errors before trailers remain body errors rather than fabricated EOF/trailer state.

## 2. Preserve trailer support through Python/FFI only where justified

The core capability is mandatory; adapter exposure should be narrow and truthful.

Python native API may expose trailers directly if there is a clear existing response-extension location. For the HTTPX facade, compare pinned HTTPX behavior before adding or changing any public surface; do not invent compatibility behavior.

The FFI/Node layers may defer trailer exposure if doing so would substantially broaden those APIs, but the core must no longer discard the protocol information.

Acceptance:

- [ ] adapter behavior is documented explicitly.
- [ ] no facade API is changed without pinned reference evidence.

## 3. Improve connection metadata capture at connector boundaries

Current upgraded Hyper streams use default `ConnectionMetadata` because local/remote socket information is not recoverable from the opaque `Upgraded` value alone. Move metadata capture earlier where custom connectors can observe it.

Target metadata where available:

- local socket address;
- remote socket address;
- transport kind (TCP/UDS/QUIC/proxy tunnel as representable);
- TLS version/cipher/ALPN where rustls exposes it safely;
- negotiated HTTP version already known at response level;
- proxy endpoint identity without credentials;
- H3/QUIC endpoint information where Quinn exposes it.

Requirements:

- no secret-bearing metadata;
- no fake zero/default values represented as real observations;
- use `Option`/unknown states for unavailable information;
- preserve Hyper opaque-stream limitation where unavoidable.

Acceptance:

- [ ] custom direct connector captures real local/remote addresses in local fixtures.
- [ ] UDS reports UDS transport without inventing IP addresses.
- [ ] TLS metadata is verified against controlled local TLS fixtures.
- [ ] unavailable Hyper-opaque metadata remains explicitly unavailable.

## 4. Expand transport observability without confusing logical pool metrics

`PoolMetrics` currently reports logical permit waits/cancellations. Keep that semantic stable and add a separate transport-observation surface rather than overloading pool counters with physical connection claims.

Candidate counters/gauges where accurately observable:

- connector attempts;
- successful connects;
- connect failures;
- DNS resolution attempts/failures if centralized and observable;
- TLS handshakes/failures;
- H3 connection creations/reconnects/cache evictions;
- proxy CONNECT attempts/failures;
- upgraded connections created;
- response-body cancellations/early drops where meaningful.

Do not claim exact Hyper internal connection reuse counts unless the integration can observe them reliably.

Acceptance:

- [ ] metric names state whether they count logical requests, connector events or protocol connections.
- [ ] counters are atomic/concurrency-safe and low-overhead.
- [ ] tests assert exact counts in deterministic local scenarios.
- [ ] unsupported physical-connection metrics remain absent rather than estimated.

## 5. Trace integration

Where existing `TraceObserver` events already cover phases, reconcile metrics/metadata with trace rather than creating a duplicate event taxonomy.

Consider narrowly adding trace events only for genuinely missing high-value phases such as DNS/connect/TLS if the connector architecture can emit them consistently across direct, proxy, SOCKS, UDS and H3 paths.

Do not make trace callback semantics asynchronous; the HTTPX facade intentionally rejects coroutine callbacks because the core observer is synchronous.

Acceptance:

- [ ] existing trace event names/semantics remain compatible.
- [ ] no transport emits contradictory duplicate lifecycle events.

## 6. Clarify native concurrency-limit naming

The native pool controls logical in-flight request permits, not physical connection counts under H2/H3. Before API stabilization, provide terminology that accurately reflects this.

Preferred direction:

- introduce native names such as `max_in_flight_requests` and `max_in_flight_requests_per_origin` (exact naming may vary);
- retain existing `max_connections` / `max_connections_per_host` as compatibility aliases for the current pre-1.0 release line if removing them immediately would create needless breakage;
- document that idle-connection limits passed to Hyper are physical idle-pool policy while semaphore limits are logical request concurrency;
- keep the HTTPX facade's `Limits(max_connections=...)` naming unchanged because that is part of the pinned external contract.

Do not rename public APIs mechanically without migration tests/docs.

Acceptance:

- [ ] native documentation no longer implies one permit equals one TCP connection.
- [ ] H1/H2/H3 examples distinguish logical request limits from protocol stream/socket limits.
- [ ] HTTPX facade signatures remain unchanged.
- [ ] aliases/deprecations, if used, have tests and migration text.

## 7. Validate timeout/lease behavior with trailers and metadata

Because trailer capture extends response-body lifecycle, test:

- read timeout while waiting for trailers;
- total deadline expiry before trailers;
- body completion followed by trailer availability;
- partial body drop releases pool permit without waiting forever for trailers;
- metadata remains valid after body completion where documented;
- no strong-reference cycle keeps clients/connections alive solely because metadata/trailers are retained.

## 8. Documentation hooks for later truth refresh

During implementation, update only code-level docs and narrowly required reference docs. Broad repository documentation reconciliation belongs to `post-maturation-documentation-and-plan-hygiene.md` after final qualification.

Record any intentionally unavailable metric/metadata field so the final docs can state it accurately.

## Validation

Run focused Rust protocol fixtures plus:

```sh
./scripts/check.sh
```

Run directly affected Python/HTTPX response/network-stream/trace tests. If the environment supports extended validation, run it at plan closure.

Do not renew exact-SHA HTTPX qualification here.

## Non-goals

- no OpenTelemetry dependency or tracing backend integration;
- no packet capture subsystem;
- no fabricated Hyper socket-reuse statistics;
- no HTTPX facade signature changes;
- no broad response API redesign unrelated to trailers;
- no public browser-style networking timing API in this pass.

## Exit criteria

- [ ] core no longer discards supported HTTP trailers.
- [ ] trailer lifecycle is streaming-safe and timeout-safe.
- [ ] connection metadata is truthful and connector-derived where available.
- [ ] transport metrics are distinct from logical pool metrics.
- [ ] native limit terminology reflects actual semantics.
- [ ] HTTPX facade names/contract remain intact.
- [ ] Tier 1 and focused compatibility tests pass.
