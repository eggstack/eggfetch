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

eggfetch streaming responses provide `iter_bytes()`, `iter_text()`,
`iter_lines()`, and their async `aiter_*` counterparts. Raw encoded
iteration is available in both the native API and the HTTPX compatibility
facade, but the two differ: the native streaming path yields decoded bytes
by default (automatic decompression), while the facade preserves HTTPX raw
semantics — a streaming response selects either exact encoded raw bytes or
the decoded path on first body consumption (see
`docs/reference/compatibility.md` and `docs/residual-differences.md`). Use
`iter_bytes()` for decoded chunks; consult the compatibility reference when
you need the original wire encoding.

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

The Rust core configures proxies explicitly and never reads
`HTTP_PROXY`/`HTTPS_PROXY` from the environment. The native Python
`Client`/`AsyncClient` read `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, and
`NO_PROXY` by default (`trust_env=True`; pass `trust_env=False` to disable).
The HTTPX compatibility facade honors HTTPX-compatible environment discovery
(scheme-specific variables with `ALL_PROXY` fallback, lowercase forms,
`NO_PROXY` URL-pattern rules) when `trust_env=True` (the default), matching
HTTPX 0.28.1 precedence including the bare-unbracketed-IPv6 acceptance and
bracketed/CIDR-form rejection boundary.

```python
# NO_PROXY bypass comes from the environment (trust_env=True, the default)
import os
os.environ["NO_PROXY"] = "localhost,127.0.0.1,.internal.com"
client = eggfetch.Client(proxy="http://proxy:8080")
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
custom CAs into one file if you need both. For `ssl.SSLContext` handling,
representable contexts (default, custom CA, `verify=False`,
provenance-bearing helper mTLS) translate exactly to rustls; contexts with
rustls-unrepresentable state (ciphers, ALPN, TLS-version policy,
client-certificate provenance, non-`ssl.SSLContext` subclasses) fail closed
with `TypeError` before dispatch. There is no arbitrary OpenSSL context
passthrough.

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

| Feature | HTTPX 0.28.1 | eggfetch native Python | eggfetch `compat.httpx` facade |
| --- | --- | --- | --- |
| Redirects | Do **not** follow by default (`follow_redirects=False`) | Do **not** follow by default | Do **not** follow by default |
| Proxy env vars | Respects `HTTP_PROXY` etc. (`trust_env=True`) | Reads env by default (`trust_env=True`); Rust core/CLI are explicit-only | Honors HTTPX-compatible discovery when `trust_env=True` |
| Retry | Not built-in | Built-in `Retry` policy | Built-in `Retry` policy (native engine) |
| HTTP/3 | Not available | Experimental (QUIC) | Experimental (QUIC) |
| Decompression | Decoded `iter_bytes()` / raw `iter_raw()` both available | Automatic decoded iteration; raw encoded path selectable | Decoded/raw selectable per first-consumption boundary |
| Backend | httpcore + anyio | Rust (tokio + hyper) | Rust (tokio + hyper) |
| Trio/AnyIO | Supported | Not supported (asyncio only) | Not supported (asyncio only) |
| WSGI/ASGI transports | Supported | Not in native API | Supported (`WSGITransport`, `ASGITransport`) |
| Mock/custom transports, mounts | Supported | Not in native API | Supported (`MockTransport`, mounts) |

Only 101 Switching Protocols responses own a writable `network_stream`
(`response.extensions["network_stream"]`); ordinary pooled responses expose
`None`. Sync callbacks work for tracing on both clients; coroutine trace
callbacks are rejected with `TypeError`.

## What has changed since the original migration guide

This migration guide has been audited against HTTPX 0.28.1. The following
corrections were made:

- Pool timeout support is now correctly documented as supported in both libraries
- Exception hierarchy is documented with the correct eggfetch names
- No unqualified compatibility claims are made

For the machine-readable compatibility profile, see `compat/httpx/0.28.1/`.
