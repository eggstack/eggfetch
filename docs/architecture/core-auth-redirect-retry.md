# Auth, Redirect & Retry Deep Dive

This document covers the authentication subsystem, redirect following, and retry with backoff.

See also: [overview.md](overview.md), [core-engine.md](core-engine.md).

## Authentication

### Supported Schemes

| Scheme | Header | Construction |
|--------|--------|--------------|
| Basic | `Authorization: Basic <base64(user:pass)>` | `BasicAuth::new(username, password)` |
| Bearer | `Authorization: Bearer <token>` | `BearerAuth::new(token)` |

### Security Properties

- **Secret redaction**: `AuthScheme`, `BasicAuth`, `BearerAuth` implement custom `Debug`/`Display` that redact sensitive values. `Cookie` redacts its value, `CookieJar` reports entry counts only, `Request` renders a redacted URL with length-only body summary, and `ClientConfig` uses a manual redacting `Debug`. Credentials are never printed in logs or error messages.
- **Input validation**: Usernames must not contain `:`. CR/LF is rejected. Violations return `Error::InvalidAuthHeader`.
- **URL credentials rejected**: `https://user:pass@host/` returns an error. Use `BasicAuth` explicitly.

### Configuration Levels

1. **Client-level**: `ClientBuilder::auth(auth)` — sets default auth for all requests.
2. **Request-level**: `RequestBuilder::auth(auth)` — overrides client-level for one request.
3. **Request-level disable**: `RequestBuilder::without_auth()` — prevents client-level auth from being applied.

### Precedence Resolution

`resolve_request_auth()` applies in order:
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
do not.

## Redirect Following

### Policy

`RedirectPolicy { follow: bool, max_redirects: usize }` configures redirect
behavior (default: `follow = false`, matching HTTPX; `max_redirects = 20`).
`ClientBuilder::follow_redirects()` / `max_redirects()` /
`redirect_policy()` set the client default; `RequestBuilder::redirect_policy()`
overrides per request.

### Method Rewriting

| Status | Original Method | Rewritten Method |
|--------|----------------|------------------|
| 301, 302 | POST | GET |
| 303 | Any | GET |
| 307, 308 | (unchanged) | (unchanged) |

### Header Handling

On redirect (single `advance_redirect_hop()` transformation + shared
`HopBuildParams` hop builder):
- **Same-origin**: `Authorization`/`Proxy-Authorization` are stripped from the cloned set, then configured client-level auth is re-applied; `Cookie`/`Host` survive.
- **Cross-origin**: `Authorization`, `Cookie`, and `Proxy-Authorization` are stripped, plus `Host` is reset to the new destination; client-level auth is not reapplied.
- `Host` header is updated to the new destination.
- `Content-Length`, `Content-Type`, and `Transfer-Encoding` are stripped when the body is dropped.
- Destination-specific transport hints (`target`, `sni_hostname`, `trace`) attach only on the first hop and are cleared thereafter. Per-request decompression and proxy overrides persist across all hops. The redirects-disabled fast path uses the same first-hop builder, so it cannot diverge except for loop behavior.

### Body Replay

- 301/302/303: body is dropped (method changes to GET); the stale replay payload is cleared so a later method-preserving hop cannot resurrect it.
- 307/308: body is replayed if replayable (`Bytes` body). Stream bodies return `Error::BodyNotReplayableForRedirect` before any unsafe replay.
- 303 drops the payload for all methods except HEAD (RFC 9110 §15.4.4).

### History

Redirect hops are recorded in `Response::history()` as `HistoryEntry` records containing status code, URL, and headers (redacted for cross-origin). History entries do not carry body data.

### Total Timeout

The total timeout applies across the entire redirect chain, not per-hop. Each hop receives only the remaining wall-clock budget (`RequestParts::shrink_total_deadline`). A chain of 5 redirects sharing a 10-second total timeout must complete within 10 seconds.

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
| Network errors (connect, TLS, I/O) | Yes (for replayable requests) |
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

### Replay Check

Retries restart the complete logical request (including redirects) under
the original total deadline via the typed `RequestParts::retry_request()`
transformation, which preserves every request-local override (transport
hints, proxy/decompression overrides, auth-disable state, redirect and
retry policy) and applies the shrunk total budget. Before retrying,
replayability is verified through one explicit operation:
- `Bytes`/`Empty` bodies are replayed by cloning (`try_clone_for_retry`).
- `Stream` bodies are non-replayable → `Error::BodyNotReplayableForRetry` (never panics, never silently drops fields).

### Backoff

Exponential backoff with jitter:
- Base delay × 2^attempt, capped at a maximum.
- Jitter randomizes the delay to avoid thundering herd.
- `Retry-After` header (both integer seconds and HTTP-date) is respected.

### Context

`RetryContext` provides:
- `attempt` — current attempt number (0-based).
- `cause` — `RetryCause` enum (network error, status code, etc.).
- `last_response` — the response that triggered the retry (if any).
