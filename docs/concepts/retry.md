# Retry

eggfetch provides policy-driven retries with exponential backoff, method safety checks, body replayability validation, and Retry-After header support.

## RetryPolicy

Retries are opt-in and disabled by default. Build a policy with the builder:

```rust
use eggfetch_core::RetryPolicy;

let policy = RetryPolicy::builder()
    .max_attempts(3)
    .backoff_factor(0.5)
    .max_delay(Duration::from_secs(30))
    .retry_status(429)
    .retry_status(503)
    .build();
```

```python
retry = eggfetch.Retry(max_attempts=3, backoff_factor=0.5)
client = eggfetch.Client(retry=retry)
```

## Method Safety

By default, only safe methods are retried:

| Method | Retried by default |
|--------|-------------------|
| GET | Yes |
| HEAD | Yes |
| OPTIONS | Yes |
| POST | No |
| PUT | No |
| DELETE | No |
| PATCH | No |

Use `.allow_post_retry()`, `.allow_put_retry()`, etc. to opt in:

```rust
let policy = RetryPolicy::builder()
    .max_attempts(3)
    .allow_post_retry()
    .build();
```

## Status Codes

By default, these status codes trigger a retry: 408, 429, 502, 503, 504.

Use `.retry_status(code)` to add custom codes. The first call replaces the defaults; subsequent calls add to the set. Use `.retry_statuses(iter)` to replace the entire set.

## Body Replayability

A request is only retried if the body is replayable. `Empty` and `Bytes` bodies are replayable. `Stream` bodies are not, unless a replay factory is provided (not yet implemented). If the body is not replayable, the retry is skipped.

## Exponential Backoff with Jitter

Backoff uses bounded exponential delays:

- Attempt 1: no delay
- Attempt 2: `initial_delay` with jitter
- Attempt 3: `initial_delay * factor` with jitter
- Subsequent: doubles, capped at `max_delay`

Default values: `initial_delay=500ms`, `factor=0.5`, `max_delay=30s`.

## Retry-After Support

Enable `Retry-After` header support to respect server-requested delays:

```rust
let policy = RetryPolicy::builder()
    .max_attempts(3)
    .respect_retry_after(true)
    .build();
```

`Retry-After` values are parsed as either seconds or HTTP-date format. Delays exceeding `max_delay` are capped.

## Total Timeout Integration

The retry engine wraps the entire logical request attempt (including redirects) under a single total deadline. Each retry restarts the complete logical request under the original total deadline. If the total timeout expires, no further retries are attempted.

## Transport Errors

The following errors are retryable:

- Connection refused or reset
- TCP I/O errors
- Hyper protocol errors
- Pool or connect timeouts
- HTTP/2 `REFUSED_STREAM` stream resets
- HTTP/3 connection errors (QUIC handshake failure, connection closed)

Invalid URLs, decompression errors, and body limit errors are not retried.

## Python API

```python
import eggfetch

# Simple retry with defaults
retry = eggfetch.Retry(max_attempts=3)

# Full configuration
retry = eggfetch.Retry(
    max_attempts=5,
    backoff_factor=0.3,
    max_delay=60.0,
    retry_after=True,
)

client = eggfetch.Client(retry=retry)
response = client.get(url)

# Per-request override
response = client.get(url, retry=eggfetch.Retry(max_attempts=1))
```

## CLI

```bash
# Enable retries
eggfetch --retry 3 https://example.com

# With delay configuration
eggfetch --retry 3 --retry-delay 1.0 https://example.com
```
