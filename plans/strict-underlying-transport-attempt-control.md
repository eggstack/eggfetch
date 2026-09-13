# Strict Underlying Transport Attempt Control

Planning baseline: `475bd50f6f9b9f66b95eea814f06b4adb22ede93` (`main`, 2026-09-13; eggfetch 0.1.4)
Parent program: `plans/extensible-embedded-transport-consumer-program.md`
Status: implemented; stale-idle qualification follow-up remains

## Objective

Expose explicit native control over Hyper-util's implicit canceled-request retry behavior so an embedding application can require that one call into eggfetch produces at most one underlying HTTP transport attempt unless eggfetch's own explicit `RetryPolicy` performs another logical attempt.

Preserve current default behavior for existing callers. This plan does not redesign eggfetch's retry subsystem.

## Why this is separate from `RetryPolicy`

`eggfetch_core::RetryPolicy` governs explicit logical retries in the request pipeline. It is opt-in and defaults to one attempt. Hyper-util's legacy client separately retries some requests when a reused idle connection is found dead/cancelled before request transmission. That lower-level retry can occur even when eggfetch's `RetryPolicy` is disabled.

For ordinary clients, Hyper's behavior is useful and should remain the default. For orchestrators, accounting proxies, durable job systems and other applications that persist one attempt before dispatch, the hidden retry can violate attempt accounting and failure attribution.

The public API therefore needs two independent controls:

```text
Eggfetch logical RetryPolicy
    controls application-visible logical request retry

Hyper canceled-request retry policy
    controls transparent transport retry inside one logical request
```

Do not merge them.

## 1. Add an additive client configuration field

Add a boolean to the immutable native client configuration representing whether Hyper may retry canceled requests internally. Exact naming should match Hyper closely enough to avoid semantic ambiguity, for example:

```rust
Client::builder().retry_canceled_requests(false)
```

Alternative naming such as `implicit_transport_retries(false)` is acceptable only if rustdoc explicitly states that it maps to Hyper-util's stale/canceled pooled-connection retry behavior and is **not** eggfetch `RetryPolicy`.

Required default:

```text
true
```

This preserves the current effective Hyper-util behavior for callers that do not opt in to strict mode.

The setting is client-wide and immutable after `build()`.

## 2. Centralize Hyper builder policy

Current `ClientInner` constructs multiple `hyper_util::client::legacy::Client` instances for different routes, including standard HTTPS, direct/socket-option routes, SNI variants, request-scoped resolved targets, UDS and SOCKS routes. The custom-dialer plan adds another Hyper-based route.

Do not copy a new `.retry_canceled_requests(...)` call manually into each constructor and rely on future maintainers to remember it.

Introduce a small crate-internal helper or builder-policy object that applies all common Hyper client settings that are intended to be invariant across routes. At minimum audit:

- canceled-request retry setting;
- HTTP version policy / `http2_only` where relevant;
- pool idle timeout;
- pool max idle per host if currently applied on the route;
- Tokio executor/timer behavior.

Do **not** over-generalize every route-specific builder into a new framework. The goal is one obvious location for settings whose omission would change semantics across connector variants.

Acceptance requirement: a source search should show that every Hyper legacy client construction either uses the common policy helper or carries an explicit documented reason why the generic policy does not apply.

## 3. Cover every Hyper-based route

The setting must affect all HTTP/1/2 clients owned by one `Client`:

- standard connector;
- direct/local-address/socket-option connector;
- SNI-keyed clients;
- request-scoped resolved-target clients;
- UDS client;
- SOCKS client;
- custom-dialer client after the parent Plan 1 lands;
- any HTTP CONNECT/forward proxy Hyper client path that uses the same legacy builder.

HTTP/3/Quinn is outside this setting because it is not implemented by Hyper's legacy client. Document that scope explicitly.

If a route does not use the Hyper legacy client, it must not pretend to honor this flag.

## 4. Preserve explicit retry semantics

With `retry_canceled_requests(false)` and an explicit eggfetch `RetryPolicy` configured for multiple attempts:

- one logical eggfetch attempt makes at most one underlying Hyper request attempt;
- an eligible failure may then cause the eggfetch retry engine to construct a new logical attempt according to existing method/body/idempotency rules;
- backoff, `Retry-After`, body replayability and total deadline behavior remain unchanged;
- no extra lower-level retry occurs between those visible attempts.

With the flag left at its default `true`, preserve current Hyper behavior plus the existing eggfetch retry behavior.

Do not automatically set the flag to false merely because `RetryPolicy` is configured. Some callers may deliberately want both layers, and changing this implicitly would be a behavior regression.

## 5. Deterministic stale-idle regression fixture

Add a local test server capable of creating the exact condition Hyper's canceled-request retry is designed for:

1. first request succeeds over a keepalive connection;
2. server closes or invalidates that idle connection without the client immediately observing it;
3. second request attempts to reuse the stale pooled connection;
4. instrument accepted connections and received requests/headers so retry behavior is observable.

Required assertions:

### Default mode

Prove the default remains compatible with current Hyper behavior. If Hyper retries the stale request transparently, the request succeeds and the fixture shows the additional physical attempt/connection.

### Strict mode

With `retry_canceled_requests(false)`, prove there is no transparent second Hyper attempt. The operation must surface the transport failure to eggfetch/application code.

### Strict + explicit eggfetch retry

Configure an eligible explicit `RetryPolicy` and prove the second attempt is performed by eggfetch's retry loop, not Hyper's hidden path. Instrument enough state to distinguish these cases rather than only asserting eventual success.

Tests must be deterministic and local; no external origin is justified.

## 6. Interaction with streamed/non-replayable bodies

Hyper's canceled-request retry behavior has safety rules of its own, but eggfetch must not rely on undocumented assumptions for strict mode.

Add at least one regression proving that disabling the implicit retry works for a request body form relevant to the native API. The explicit eggfetch retry engine must continue to reject non-replayable retry when required.

Do not broaden method/body retry eligibility in this plan.

## 7. Error and metrics behavior

Strict mode should expose the underlying Hyper/transport failure through the existing error taxonomy and source chain. Do not invent a "strict mode" error category.

Review transport metrics so they remain truthful. If current metrics cannot observe Hyper's internal retry separately, document that limitation rather than inferring counts. The deterministic test fixture can count server-side connections independently.

No persistent client-visible metric is required solely for this feature.

## 8. Python/HTTPX/CLI behavior

Do not change compatibility-facing defaults. The new control is native Rust first.

Unless a separate compatibility requirement exists:

- Python `Client` and `AsyncClient` keep current behavior;
- HTTPX/HTTPX2 facades keep current behavior;
- CLI keeps current behavior;
- no new Python kwarg is required.

If the common builder policy refactor touches those client construction paths indirectly, existing parity tests are mandatory regression coverage.

## 9. Documentation

Update:

- native Rust guide/client builder reference;
- retry architecture documentation to distinguish explicit logical retry from Hyper canceled-request retry;
- core engine docs if the common Hyper builder policy becomes an architectural boundary.

Use precise terminology. Do not describe the flag as disabling all retries; it disables only Hyper's implicit canceled-request transport retry.

## Required validation

Run focused stale-pool/retry tests plus all retry, streaming-body and route-specific client tests. Then:

```sh
cargo test -p eggfetch-core retry
cargo test -p eggfetch-core --all-features
./scripts/check.sh
```

The parent closure plan owns extended/package/exact-SHA requalification.

## Non-goals

- no change to retryable status codes;
- no change to idempotency policy;
- no change to backoff/jitter;
- no attempt journal or durable retry state;
- no retry callbacks;
- no HTTP/3 retry redesign;
- no default change to Hyper's current behavior;
- no automatic coupling between this flag and `RetryPolicy`.

## Exit criteria

- [x] native callers can explicitly disable Hyper's canceled-request retry;
- [x] default behavior remains unchanged for callers that do not opt in;
- [x] every Hyper-based client route receives the configured policy;
- [x] common builder policy is centralized enough to prevent route drift without creating a new abstraction framework;
- [ ] deterministic stale-idle tests distinguish default, strict, and strict-plus-explicit-retry behavior;
- [x] explicit eggfetch retry semantics remain unchanged;
- [x] no Python/CLI/HTTPX default changes occur;
- [x] documentation clearly separates the two retry layers;
- [x] routine validation passes.

Implementation note: the requested deterministic stale-idle server regression is intentionally retained as a follow-up rather than represented by a timing-sensitive approximation.
