# Auth, Redirect & Retry Deep Dive

This document covers the authentication subsystem, redirect following, and retry with backoff.

See also: [overview.md](overview.md), [core-engine.md](core-engine.md), [feature-flags.md](feature-flags.md).

## Capability Features

Logical retry (`logical-retry`), redirect following (`redirects`), and
Basic auth (`basic-auth`) are coarse Cargo capability features. The
`http1`/`http2`/default compatibility aliases enable all three, preserving
every existing API and behavior. The lean high-level profile selects
`standard-http1` + `tls-rustls` without them (transport + standard route +
URL API, without `advanced-routing` or the policy bundle): Bearer-only
auth, single-attempt dispatch under the outer total deadline, 3xx returned
without following and with empty history, and no core `base64` edge from `basic-auth` (the `proxy` feature also pulls `base64` when enabled; core's direct `getrandom` edge is jointly owned with `multipart`).
The policy-only `native-http1` + `high-level-url` + `tls-rustls` recipe
without the three policy features remains a valid manual profile where
advanced routing (Dialer, pinned/SNI, UDS) is still needed.
`Client::send`/`send_detailed` dispatch to `pipeline::retry::send_with_retry`
when `logical-retry` is present, to
`pipeline::redirect::send_with_redirects` when only `redirects` is present,
and to `pipeline::lean::send_lean` when both are absent. Hyper's distinct
canceled-idle-request retry (`retry_canceled_requests`) is transport policy
and stays available in every profile. Typed DNS/refused/timeout failures,
body caps, pooling, TLS, and high-level response semantics are unchanged.
Lean coverage lives in `crates/eggfetch-core/tests/lean_policy_tests.rs`
(passes under both lean and full feature sets).

## Authentication

### Supported Schemes

| Scheme | Header | Construction | Feature |
|--------|--------|--------------|---------|
| Basic | `Authorization: Basic <base64(user:pass)>` | `BasicAuth::new(username, password)` | `basic-auth` (in `http1`/`http2`) |
| Bearer | `Authorization: Bearer <token>` | `BearerAuth::new(token)` | always (lean-safe) |

### Security Properties

- **Secret redaction**: `BasicAuth`/`BearerAuth` implement redacting `Debug`/`Display`; `AuthScheme` implements redacting `Debug` only. `Cookie` redacts its value, `CookieJar` reports entry counts only, `Request` renders a redacted URL with length-only body summary, and `ClientConfig` uses a manual redacting `Debug`. Auth secrets are never printed in logs or auth-type formatting output; general downstream-library error strings are not yet systematically audited (see `security-findings.md` F-004).
- **Input validation**: Usernames must not contain `:`. CR/LF is rejected. Violations return `Error::InvalidAuthHeader`.
- **URL credentials rejected**: `https://user:pass@host/` returns an error. Use `BasicAuth` explicitly.

### Configuration Levels

1. **Client-level**: `ClientBuilder::auth(auth)` — sets default auth for all requests.
2. **Request-level**: `RequestBuilder::auth(auth)` — overrides client-level for one request.
3. **Request-level disable**: `RequestBuilder::without_auth()` — prevents client-level auth from being applied.

### Precedence Resolution

`resolve_request_auth()` applies in order:
0. If an explicit `Authorization` header coexists with any configured auth, fail with `Error::ConflictingAuth`; a raw header alone with no configured auth passes through untouched.
1. If request-level explicit auth is set, use it.
2. If request-level auth is disabled (`without_auth()`), no auth.
3. If client-level auth is set, use it.
4. Otherwise, no auth.

### Cross-Origin Redirect Stripping

The redirect engine always strips `Authorization` and `Proxy-Authorization`
from the cloned header set, and additionally strips `Cookie` and `Host` on
cross-origin redirects. Client-level auth is NOT reapplied on cross-origin
redirect hops. Same-origin redirects reapply client-level auth after the
strip, so effective credentials survive while server-mutated header values
do not. A manually-set `Authorization` header (no configured auth — that
combination fails with `ConflictingAuth` on the first hop) is re-attached
by the pipeline on same-origin hops and dropped on cross-origin hops.

## Redirect Following

### Policy

`RedirectPolicy { follow: bool, max_redirects: usize, downgrade: RedirectDowngradePolicy }`
configures redirect behavior (default: `follow = false`, matching HTTPX;
`max_redirects = 20`; `downgrade = Allow` for compatibility).
`ClientBuilder::follow_redirects()` / `max_redirects()` /
`redirect_policy()` / `redirect_downgrade_policy()` set the client default;
`RequestBuilder::redirect_policy()` overrides per request.
`RedirectPolicy::strict(max)` follows redirects while rejecting HTTPS ->
HTTP downgrades.

### Downgrade Policy

`RedirectDowngradePolicy::{Allow, Deny}` is a transport-security boundary:
under `Deny`, a redirect whose resolved target downgrades `https` to `http`
fails with `Error::InvalidRedirectLocation` before the next hop dispatches,
so no request bytes reach the downgraded destination. Relative and
scheme-relative `Location` values resolve against the origin first, so a
relative redirect from `https` stays `https` and remains allowed. The public
`build_redirect_request()` entry point stays compatibility-preserving
(`Allow`); `build_redirect_request_with_redirect_policy()` and the pipeline
enforce the configured policy. Credential stripping, body replayability,
method rewrites, loop bounds, and scheme allow-listing are unchanged.

### Method Rewriting

| Status | Original Method | Rewritten Method |
|--------|----------------|------------------|
| 301, 302 | POST | GET |
| 303 | Any except HEAD | GET (HEAD preserved) |
| 307, 308 | (unchanged) | (unchanged) |

### Header Handling

On redirect (single `pipeline::redirect::advance_redirect_hop()` transformation + shared
`HopBuildParams` hop builder, owned by `pipeline::redirect`):
- **Same-origin**: `Authorization`/`Proxy-Authorization` are stripped from the cloned set, then configured client-level auth is re-applied (or a manually-set `Authorization` header is re-attached when no auth is configured); `Cookie`/`Host` survive.
- **Cross-origin**: `Authorization`, `Cookie`, and `Proxy-Authorization` are stripped, plus `Host` is removed (the transport derives it from the new URL); client-level auth is not reapplied.
- `Host` header is removed on cross-origin hops; the transport derives it from the new destination.
- `Content-Length`, `Content-Type`, and `Transfer-Encoding` are stripped when the body is dropped.
- Destination-specific wire hints (`target`, `sni_hostname`, `trace`) attach only on the first hop and are cleared thereafter. Native physical-route snapshots are separate: direct `resolved_target` and private `proxied_target` are retained only for same-origin redirects; cross-origin redirects return `Error::ResolvedTargetRedirect` before dispatch. A proxied target snapshot is valid only with a compatible effective proxy route; unsupported proxy combinations fail before I/O. Per-request decompression and proxy overrides persist across all hops. The redirects-disabled fast path uses the same first-hop builder, so it cannot diverge except for loop behavior.

### Body Replay

- 301/302: body is dropped only for POST→GET rewrites (method changes to GET); other methods preserve method and body. The stale replay payload is cleared so a later method-preserving hop cannot resurrect it.
- 307/308: body is replayed if replayable (`Bytes` body). Stream bodies return `Error::BodyNotReplayableForRedirect` before any unsafe replay.
- 303 drops the payload for all methods except HEAD (RFC 9110 §15.4.4).

### History

Redirect hops are recorded in `Response::history()` as `HistoryEntry` records containing status, version, URL, headers, and reason phrase. Entries store headers/URL verbatim; only `Debug` redacts sensitive values, for all entries. History entries do not carry body data.

### Total Timeout

The total timeout applies across the entire redirect chain, not per-hop. Each hop receives only the remaining wall-clock budget (the retry loop uses `RequestParts::shrink_total_deadline`; the redirect loop inlines the same remaining-budget computation). A chain of 5 redirects sharing a 10-second total timeout must complete within 10 seconds.

## Retry

### Policy

`RetryPolicy` (opt-in) configures retry behavior:
- `RetryPolicyBuilder` — fluent builder for retry configuration.
- `BackoffPolicy` — exponential backoff with jitter.
- `MethodPolicy` — per-method retry rules.
- `StatusPolicy` — per-status-code retry rules.

### Retryable Conditions

| Condition | Retryable? |
|-----------|-----------|
| Network errors (connect, I/O, hyper) | Yes (for replayable requests) |
| Custom dialer connection/timeout failures | Yes (for replayable requests) |
| Custom dialer authentication/rejection/other failures | No |
| TLS handshake failures (`Error::Tls`) | No — never retried |
| 429 Too Many Requests | Yes (with Retry-After) |
| 408, 502, 503, 504 | Yes (for replayable requests) |
| 500 Internal Server Error | No (not in the default retryable set) |
| `REFUSED_STREAM` (HTTP/2) | Yes |
| `CANCEL`, `GOAWAY` (HTTP/2) | No |
| `H3Connect` (HTTP/3) | Yes (for replayable requests) |
| `H3ConnectionClosed`, `H3Stream`, `H3Protocol` (HTTP/3) | No |
| Body not replayable | No |

H3 performs no transport-level retries and never replays one-shot bodies:
stale QUIC origins are evicted and the original error returned, so a
reconnect happens only through this retry machinery on a later attempt
where the policy above allows it.

### Hyper canceled-request retry

Hyper-util's legacy HTTP/1/2 client has a separate lower-level retry for a
request assigned to a reused idle connection that is found unusable before
transmission. `ClientBuilder::retry_canceled_requests(true)` leaves that
behavior enabled (the default); setting it to `false` surfaces the existing
Hyper/transport error to eggfetch without adding a strict-mode error variant.
The control is applied through the common Hyper builder policy to standard,
direct, resolved-target, SNI, UDS, SOCKS, and custom-dialer clients. It does
not apply to HTTP/3/QUIC, which has its own lifecycle and retry rules.

These controls remain independent:

| Layer | Control | Effect |
|-------|---------|--------|
| Hyper transport | `retry_canceled_requests(false)` | Prevents a hidden retry of one request on a stale reused idle connection |
| eggfetch pipeline | `RetryPolicy` | Starts another logical attempt when method, status/error, body replayability, backoff, and deadline rules allow it |

Disabling the Hyper behavior does not change explicit retry eligibility or
make streaming request bodies replayable. `PoolMetrics` and
`TransportMetrics` also do not infer Hyper socket reuse or its internal retry
count; local server instrumentation is required when exact physical attempts
matter.

### Replay Check

Retries restart the complete logical request (including redirects) under
the original total deadline via the typed `RequestParts::retry_request()`
transformation (driven by `pipeline::retry::send_with_retry`), which preserves every request-local override (transport
hints, proxy/decompression overrides, auth-disable state, redirect and
retry policy) and applies the shrunk total budget. Before retrying,
replayability is verified through one explicit operation:
- `Bytes`/`Empty` bodies are replayed by cloning (`try_clone_for_retry`).
- `Stream` bodies are non-replayable, so the request is single-attempt (no retry); calling `retry_request` on a stream directly returns `Error::BodyNotReplayableForRetry` (never panics, never silently drops fields).

### Backoff

Exponential backoff with jitter:
- `initial_delay × factor^(attempt − 2)`, capped at `max_delay` (factor configurable via `backoff_factor()`, default `2.0`; no delay before attempt 1).
- Full jitter: the capped delay is randomized uniformly over `[0, capped)`, with a 1 ms floor when the cap is positive, to avoid thundering herd.
- 429 without `Retry-After` uses `min(1.5 × jittered delay, max_delay)`.
- `Retry-After` header (both integer seconds and HTTP-date) is respected when enabled via `respect_retry_after(true)` (off by default).

### Context

`RetryContext` provides:
- `attempt` — current attempt number (1-indexed).
- `method` — the HTTP method.
- `body_replayable` — whether the request body can be replayed.
- `cause` — `RetryCause` enum (network error, status code, etc.).
- `remaining_total` — remaining total budget (`Option<Duration>`).
