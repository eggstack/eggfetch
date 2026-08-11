# Migrating from HTTPX

eggfetch has a compatible API surface with HTTPX but follows different
defaults and conventions. Both libraries expose `Client`/`AsyncClient` with
context managers, streaming responses, and typed auth. The differences
are mostly in defaults, some naming, and the underlying engine.

## Installation

```bash
pip install eggfetch
```

eggfetch compiles a native Rust extension. There is no pure-Python
fallback. Python 3.10 through 3.13 are supported.

## Client and AsyncClient

The API shape is nearly identical.

```python
# HTTPX
import httpx
with httpx.Client() as client:
    r = client.get("https://example.com")

# eggfetch
import eggfetch
with eggfetch.Client() as client:
    r = client.get("https://example.com")
```

```python
# HTTPX
async with httpx.AsyncClient() as client:
    r = await client.get("https://example.com")

# eggfetch
async with eggfetch.AsyncClient() as client:
    r = await client.get("https://example.com")
```

## Request methods

Same names: `get`, `post`, `put`, `patch`, `delete`, `head`, `options`,
`request`.

```python
# Same in both
r = client.get("https://example.com", params={"q": "test"})
r = client.post("https://example.com", json={"key": "value"})
r = client.request("DELETE", "https://example.com/resource")
```

## Timeouts

HTTPX uses `httpx.Timeout()` with named phases. eggfetch uses
`eggfetch.Timeout()` with the same phase names.

```python
# HTTPX
t = httpx.Timeout(connect=5.0, read=30.0, write=5.0, pool=2.0)
r = httpx.get("https://example.com", timeout=t)

# eggfetch
t = eggfetch.Timeout(connect=5, read=30, write=5, pool=2)
r = eggfetch.get("https://example.com", timeout=t)
```

Both accept a plain float as a shorthand for all four operational phases.
HTTPX 0.28.1 does not define a request-wide total timeout; eggfetch also
supports `total` as a native-only wall-clock cap across the request lifecycle.

```python
# eggfetch only: total timeout
t = eggfetch.Timeout(total=60)
r = client.get(url, timeout=t)
```

The `connect` phase is enforced for direct and proxy connection setup. Through
HTTP or HTTPS proxies, `connect` also bounds proxy TCP/TLS setup and origin TLS
after CONNECT. `total`, when explicitly configured through the native API,
remains an outer cap.

## Streaming

Both libraries use a `stream()` context manager. The iterator API differs
slightly.

```python
# HTTPX
with httpx.stream("GET", "https://example.com/large") as r:
    for chunk in r.iter_bytes():
        process(chunk)

# eggfetch
with client.stream("GET", "https://example.com/large") as r:
    for chunk in r.iter_bytes():
        process(chunk)
```

eggfetch streaming responses provide `iter_bytes()`, `iter_text()`, and
`iter_lines()`. HTTPX's `iter_raw()` is not available; use
`iter_bytes()` instead.

For async streaming:

```python
# HTTPX
async with httpx.AsyncClient() as client:
    async with client.stream("GET", url) as r:
        async for chunk in r.aiter_bytes():
            process(chunk)

# eggfetch
async with eggfetch.AsyncClient() as client:
    async with client.stream("GET", url) as r:
        async for chunk in r.aiter_bytes():
            process(chunk)
```

## Authentication

HTTPX uses `auth=httpx.BasicAuth(...)`. eggfetch uses
`auth=eggfetch.BasicAuth(...)`.

```python
# HTTPX
r = httpx.get("https://example.com", auth=httpx.BasicAuth("user", "pass"))

# eggfetch
r = eggfetch.get("https://example.com", auth=eggfetch.BasicAuth("user", "pass"))
```

Bearer auth:

```python
# eggfetch
r = client.get(url, auth=eggfetch.BearerAuth("my-token"))
```

To disable per-request auth:

```python
r = client.get("https://public.example.com", auth=eggfetch.NOAUTH)
```

## Cookies

Both use a `cookies` dict on the client or per-request.

```python
# HTTPX
with httpx.Client() as client:
    client.cookies.set("session", "abc123")
    r = client.get("https://example.com")

# eggfetch
with eggfetch.Client() as client:
    client.cookies.set("session", "abc123")
    r = client.get("https://example.com")
```

## Proxies

HTTPX uses `proxy=` (single URL string) or mounts. eggfetch uses the
same `proxy=` parameter.

```python
# HTTPX
r = httpx.get("https://example.com", proxy="http://proxy:8080")

# eggfetch
r = eggfetch.get("https://example.com", proxy="http://proxy:8080")
```

eggfetch does **not** read `HTTP_PROXY` or `HTTPS_PROXY` environment
variables. Proxy configuration is explicit only.

```python
# NO_PROXY bypass
client = eggfetch.Client(
    proxy="http://proxy:8080",
    no_proxy="localhost,127.0.0.1,.internal.com",
)
```

## SSL/TLS

Both use `verify` and `cert` parameters.

```python
# HTTPX
r = httpx.get(url, verify=False)
r = httpx.get(url, cert=("/path/cert.pem", "/path/key.pem"))

# eggfetch
r = eggfetch.get(url, verify=False)
r = eggfetch.get(url, cert=("/path/cert.pem", "/path/key.pem"))
```

A custom CA bundle replaces system roots entirely. Concatenate system and
custom CAs into one file if you need both.

## Redirects

HTTPX 0.28.1 does **not** follow redirects by default (`follow_redirects=False`),
and neither does eggfetch. This is **not** a divergence between the two libraries.

```python
# Both HTTPX 0.28.1 and eggfetch: must opt in to follow redirects
r = httpx.get("https://example.com/redirect", follow_redirects=True)
r = eggfetch.get("https://example.com/redirect", follow_redirects=True)
```

Both support `max_redirects`:

```python
r = client.get(url, follow_redirects=True, max_redirects=5)
```

## HTTP/2

Both libraries support HTTP/2 as an opt-in feature.

```python
# HTTPX
r = httpx.get("https://example.com", http2=True)

# eggfetch
r = eggfetch.get("https://example.com", http2=True)
```

eggfetch also supports HTTP/3 (experimental, QUIC transport):

```python
r = eggfetch.get("https://example.com", http3=True)
```

## Errors

The exception names differ.

| HTTPX                        | eggfetch                    |
| ---------------------------- | --------------------------- |
| `httpx.ConnectError`         | `NetworkError`              |
| `httpx.TimeoutException`     | `TimeoutException`          |
| `httpx.HTTPStatusError`      | `HTTPStatusError`           |
| `httpx.TooManyRedirects`     | `TooManyRedirects`          |
| `httpx.DecodingError`        | `BodyError`                 |
| `httpx.InvalidURL`           | `InvalidUrl`                |
| `httpx.HTTPError`            | `EggfetchError` (base)      |

Both raise on 4xx/5xx when you call `raise_for_status()`.

## Retry

HTTPX does not have built-in retry. eggfetch has a `Retry` policy:

```python
retry = eggfetch.Retry(max_attempts=3, backoff_factor=0.2, statuses={429, 503})
r = client.get(url, retries=retry)

# Disable retries per-request
r = client.get(url, retries=False)
```

## Key differences summary

| Feature | HTTPX | eggfetch |
| --- | --- | --- |
| Redirects | Follow by default | **Do not follow** by default |
| Proxy env vars | Respects `HTTP_PROXY` | **Does not** read env vars |
| Retry | Not built-in | Built-in `Retry` policy |
| HTTP/3 | Not available | Experimental (QUIC) |
| Decompression | Manual or auto | Automatic by default |
| Backend | httpcore + anyio | Rust (tokio + hyper) |
| Trio/AnyIO | Supported | Not supported (asyncio only) |
| WSGI/ASGI transports | Supported | Not available |

## What has changed since the original migration guide

This migration guide has been audited against HTTPX 0.28.1. The following
corrections were made:

- Pool timeout support is now correctly documented as supported in both libraries
- Exception hierarchy is documented with the correct eggfetch names
- No unqualified compatibility claims are made

For the machine-readable compatibility profile, see `compat/httpx/0.28.1/`.
