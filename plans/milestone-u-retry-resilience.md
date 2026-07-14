# Milestone U Plan: Retry and Resilience Policy

## Objective

Implement an opt-in, policy-driven retry subsystem in `eggfetch-core`. Retries must be idempotency-aware, body-replayability-aware, deadline-aware, and observable. Python should configure the core policy rather than implement retries independently.

## Scope

Implement:

- `RetryPolicy` and builder
- maximum attempts and retry budget
- retryable transport errors
- optional retryable status codes
- idempotency/method rules
- request-body replayability checks
- exponential backoff with bounded jitter
- `Retry-After` parsing where enabled
- total-timeout and cancellation integration
- redirect/auth/cookie/proxy/TLS interaction
- Python client/request retry configuration

Do not enable retries by default. Do not retry arbitrary POST/PATCH requests unless explicitly allowed or protected by an idempotency key/policy.

## Core model

Suggested types:

```rust
pub struct RetryPolicy {
    max_attempts: usize,
    max_elapsed: Option<Duration>,
    backoff: BackoffPolicy,
    retry_methods: MethodPolicy,
    retry_statuses: StatusPolicy,
    retry_errors: ErrorPolicy,
    respect_retry_after: bool,
}

pub struct RetryContext {
    attempt: usize,
    method: Method,
    body_replayable: bool,
    error_or_status: RetryCause,
    remaining_total: Option<Duration>,
}
```

Place execution around a single logical request attempt, outside the transport but integrated with redirects deliberately. Initial recommendation: each retry restarts the complete logical request, including redirects, under one original total deadline.

## Retry safety

Default retryable methods:

- GET
- HEAD
- OPTIONS
- TRACE only if otherwise supported
- PUT and DELETE may be configurable, not assumed universally safe

POST/PATCH require explicit policy or a configured idempotency-key rule.

Retry only replayable bodies. Empty, bytes, form, JSON, and fully buffered multipart are replayable. Live streams and multipart streams are not unless a replay factory exists.

Return a specific error when policy requests a retry but the body is not replayable; alternatively return the original failure with retry metadata. Choose and document one behavior.

## Retryable errors/statuses

Initial transport errors may include connection reset before response, refused connection, selected transient proxy failures, and selected HTTP/2 stream errors later. Never retry certificate verification, invalid URL/header, auth configuration, malformed protocol, decoded-body limits, or deterministic request-build errors.

Optional statuses commonly include 408, 429, 502, 503, and 504. Status retries must drain/close the prior body safely.

## Backoff

Use bounded exponential backoff with jitter. Avoid a new randomness subsystem if existing `getrandom` can seed a small internal generator safely. Make tests deterministic through an injectable clock/random source or deterministic test policy.

Respect `Retry-After` seconds and HTTP-date forms only when enabled and within remaining budget/max delay.

## Deadlines and cancellation

The original logical total deadline covers all attempts and backoff sleeps. Pool/read/write/connect phase limits apply per attempt where appropriate. Cancellation during backoff or I/O must stop immediately and release resources.

## State interactions

- Cookies received from failed/status attempts: define policy. Recommended: process valid response cookies, matching ordinary response behavior.
- Auth: do not leak credentials across retry/redirect boundaries.
- Proxy: retry direct/proxy path selected for that request; do not silently switch routes.
- Multipart/streams: enforce replayability.
- Response history: retries should not appear as redirect history. Add separate attempt metadata only if exposed.

## Python API

Target:

```python
Retry(max_attempts=3, backoff_factor=0.2, statuses={429, 503})
Client(retries=retry)
client.get(url, retries=False)
```

Support `None`/omitted inheritance, `False` disable, and policy override. Avoid accepting a bare integer unless semantics are unambiguous; if supported, define it as total maximum attempts or retries consistently.

## Tests

Required:

- transient connection failure then success
- retryable status then success
- non-retryable error/status not retried
- POST not retried by default
- replayable PUT/body retry
- streamed body not retried
- total deadline includes sleep and all attempts
- cancellation during backoff
- Retry-After seconds/date and cap
- prior response body/permit released
- proxy, redirect, cookie, auth interactions
- deterministic jitter tests
- sync/async Python parity

## Acceptance criteria

- retries are opt-in
- unsafe methods/bodies are not retried accidentally
- one deadline spans all attempts
- retry decisions are testable and documented
- no resource/credential leaks occur
- Python and Rust APIs expose the same policy semantics
