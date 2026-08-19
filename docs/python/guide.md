# eggfetch Python API Guide

eggfetch is a fast, modern HTTP client for Python powered by a Rust core. It supports sync and async APIs, streaming responses, HTTP/2, HTTP/3, cookies, proxies, retries, and multipart uploads.

## Installation

```sh
pip install eggfetch
```

Requires Python 3.10 through 3.13.

## Top-Level Functions

The quickest way to make requests. Each function creates a short-lived client internally.

```python
import eggfetch

# GET
response = eggfetch.get("https://example.com")

# POST with JSON
response = eggfetch.post(
    "https://api.example.com/data",
    json={"key": "value"},
)

# PUT, PATCH, DELETE, HEAD, OPTIONS
response = eggfetch.put("https://api.example.com/resource", content=b"data")
response = eggfetch.patch("https://api.example.com/resource", json={"update": True})
response = eggfetch.delete("https://api.example.com/resource/1")
response = eggfetch.head("https://example.com")
response = eggfetch.options("https://example.com")

# Arbitrary method
response = eggfetch.request("PURGE", "https://example.com")
```

All top-level functions accept keyword arguments for headers, params, timeout, auth, cookies, proxy, verify, cert, retries, follow_redirects, max_redirects, and decompress.

## Client Class (Sync)

A reusable client with connection pooling and shared configuration.

```python
import eggfetch

client = eggfetch.Client(
    headers={"User-Agent": "my-app/1.0"},
    timeout=30,
    follow_redirects=True,
    max_redirects=10,
    auth=eggfetch.BearerAuth("my-token"),
    limits=eggfetch.Limits(max_connections=100, max_keepalive_connections=20),
)

response = client.get("https://api.example.com/data")
response = client.post("https://api.example.com/data", json={"key": "value"})

client.close()
```

The sync client releases the GIL during network I/O so other Python threads can run while a request is in progress.

## AsyncClient Class (asyncio)

An async client for use with `asyncio`. Returns coroutines instead of blocking.

```python
import asyncio
import eggfetch

async def main():
    async with eggfetch.AsyncClient(
        timeout=30,
        follow_redirects=True,
    ) as client:
        response = await client.get("https://example.com")
        print(response.status_code)

        # Concurrent requests
        responses = await asyncio.gather(
            client.get("https://api.example.com/a"),
            client.get("https://api.example.com/b"),
        )

asyncio.run(main())
```

## Response Objects (Buffered)

Top-level functions and the `Client` return buffered `Response` objects with all data loaded in memory.

```python
response = eggfetch.get("https://example.com")

# Status and metadata
print(response.status_code)    # 200
print(response.reason_phrase)  # "OK"
print(response.http_version)   # "HTTP/2"
print(response.url)            # "https://example.com/"
print(response.encoding)       # "utf-8" (from Content-Type charset)

# Headers (case-insensitive dict-like)
print(response.headers["content-type"])

# Body
print(response.text)           # decoded string
print(response.content)        # raw bytes

# Cookies set by the server
for cookie in response.cookies:
    print(f"{cookie.name}={cookie.value}")

# Redirect history
for entry in response.history:
    print(f"  {entry.status_code} -> {entry.url}")
```

## StreamingResponse Objects

Use `client.stream()` for streaming responses without buffering the full body.

```python
async with eggfetch.AsyncClient() as client:
    async with client.stream("GET", "https://example.com/large") as response:
        # Iterate over byte chunks
        async for chunk in response.aiter_bytes():
            process(chunk)

        # Or iterate over lines
        async for line in response.aiter_text():
            print(line)
```

Streaming responses also support a buffered fallback:

```python
async with eggfetch.AsyncClient() as client:
    async with client.stream("GET", "https://example.com/data") as response:
        # Buffer the entire body into text
        text = await response.text()
```

The sync client supports streaming too:

```python
with eggfetch.Client() as client:
    with client.stream("GET", "https://example.com/large") as response:
        for chunk in response.iter_bytes():
            process(chunk)
```

## Headers

Headers are case-insensitive and returned as `Headers` objects on responses.

```python
response = eggfetch.get("https://example.com")

# Access by name
ct = response.headers["content-type"]

# Check presence
if "x-request-id" in response.headers:
    print(response.headers["x-request-id"])

# Iterate
for name, value in response.headers:
    print(f"{name}: {value}")
```

## Timeouts

Accept a float (seconds) or a `Timeout` object for fine-grained control.

```python
import eggfetch

# Scalar timeout: applies to pool, connect, write, and read phases
response = eggfetch.get("https://example.com", timeout=5.0)

# Timeout object (scalar)
timeout = eggfetch.Timeout(10.0)
response = eggfetch.get("https://example.com", timeout=timeout)

# Per-phase timeout (HTTPX-compatible)
timeout = eggfetch.Timeout(pool=2.0, connect=5.0, read=30.0, total=60.0)
response = eggfetch.get("https://example.com", timeout=timeout)
```

## Auth

### BasicAuth

```python
auth = eggfetch.BasicAuth("username", "password")
response = eggfetch.get("https://api.example.com", auth=auth)
```

### BearerAuth

```python
auth = eggfetch.BearerAuth("my-token")
response = eggfetch.get("https://api.example.com", auth=auth)
```

### Tuple shorthand

Pass a `(username, password)` tuple directly:

```python
response = eggfetch.get("https://api.example.com", auth=("user", "pass"))
```

### Disable auth for one request

```python
client = eggfetch.Client(auth=eggfetch.BearerAuth("token"))

# This request skips the client-level auth
response = client.get("https://public.example.com", auth=eggfetch.NOAUTH)
```

## Cookies

### Client-level cookies

```python
client = eggfetch.Client(cookies={"session": "abc123"})
response = client.get("https://example.com")
```

### Per-request cookies

```python
response = eggfetch.get("https://example.com", cookies={"lang": "en"})
```

### Inspecting response cookies

```python
response = eggfetch.get("https://example.com")
for cookie in response.cookies:
    print(f"{cookie.name}={cookie.value} (domain={cookie.domain})")
    print(f"  secure={cookie.is_secure}, http_only={cookie.is_http_only}")
    print(f"  same_site={cookie.same_site}")
    print(f"  expires={cookie.expires}")
```

### Cookies class

The `Cookies` class provides dict-like access:

```python
cookies = response.cookies
print(cookies["session"])        # by name
print("session" in cookies)      # membership test
```

## Proxy Configuration

```python
# Set proxy per request
response = eggfetch.get(
    "https://example.com",
    proxy="http://proxy:8080",
)

# Set proxy on client
client = eggfetch.Client(proxy="http://proxy:8080")
response = client.get("https://example.com")

# Disable proxy for one request
response = client.get("https://internal.example.com", proxy=False)
```

eggfetch reads `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, and `NO_PROXY` environment variables when `trust_env=True` (the default for the compatibility layer). Set `trust_env=False` to disable environment variable discovery.

## TLS/SSL

### Disable certificate verification

```python
response = eggfetch.get("https://self-signed.example.com", verify=False)
```

### Custom CA bundle

```python
response = eggfetch.get("https://example.com", verify="/path/to/ca-bundle.pem")
```

### Client certificate (mTLS)

```python
# Combined PEM file
response = eggfetch.get("https://example.com", cert="/path/to/combined.pem")

# Separate cert and key
response = eggfetch.get("https://example.com", cert=("/path/to/cert.pem", "/path/to/key.pem"))
```

## Retry

```python
import eggfetch

# Simple retry (3 attempts, default backoff)
response = eggfetch.get("https://api.example.com", retries=True)

# Custom retry policy
retry = eggfetch.Retry(
    max_attempts=5,
    backoff_factor=0.3,
    max_delay=60.0,
    initial_delay=1.0,
    statuses={429, 502, 503, 504},
    respect_retry_after=True,
    max_elapsed=300.0,
    allow_post=True,
)
response = eggfetch.get("https://api.example.com", retries=retry)

# Disable retries for one request
response = eggfetch.get("https://api.example.com", retries=False)
```

## Multipart Uploads

Use the `files=` keyword with file paths or the `File` class.

### File paths

```python
response = eggfetch.post(
    "https://upload.example.com",
    files={"document": "/path/to/report.pdf"},
)
```

### File with metadata

```python
f = eggfetch.File("/path/to/photo.jpg", filename="avatar.jpg", content_type="image/jpeg")
response = eggfetch.post("https://upload.example.com", files={"photo": f})
```

### Multiple files with form data

```python
response = eggfetch.post(
    "https://upload.example.com",
    data={"description": "My photo"},
    files={
        "photo1": "/path/to/photo1.jpg",
        "photo2": eggfetch.File("/path/to/photo2.jpg", filename="second.jpg"),
    },
)
```

## Resource Limits

Control connection pool concurrency and keep-alive behavior:

```python
# HTTPX-compatible defaults
limits = eggfetch.Limits(
    max_connections=100,
    max_keepalive_connections=20,
    keepalive_expiry=5.0,
)

client = eggfetch.Client(limits=limits)

# Unlimited (native default)
client = eggfetch.Client()  # no limits configured
```

| Parameter | Description | HTTPX default |
|-----------|-------------|---------------|
| `max_connections` | Maximum concurrent connections | 100 |
| `max_keepalive_connections` | Maximum idle keep-alive connections | 20 |
| `keepalive_expiry` | Seconds before idle connections close | 5.0 |
| `max_connections_per_host` | Maximum connections per host | None |

## Environment Trust

By default, eggfetch reads proxy environment variables (`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY`). Disable this with `trust_env=False`:

```python
# Trust environment variables (default)
client = eggfetch.Client(trust_env=True)

# Ignore all environment proxy settings
client = eggfetch.Client(trust_env=False)
```

## HTTP/2 and HTTP/3

Enable HTTP/2 or HTTP/3 on the client:

```python
# HTTP/2 (auto-negotiate; falls back to HTTP/1.1 if the server does not
# support h2)
client = eggfetch.Client(http2=True)
response = client.get("https://example.com")

# HTTP/2 only — refuses to fall back to HTTP/1.1. Fails with a
# ConnectError / RequestError if the server does not negotiate h2.
# Over cleartext TCP, sends the H2 client preface directly (h2c prior
# knowledge) when the server accepts it.
client = eggfetch.Client(http1=False, http2=True)
response = client.get("https://example.com")

# HTTP/3 (QUIC)
client = eggfetch.Client(http3=True)
response = client.get("https://example.com")
```

The `stream_id` metadata field exposed by HTTPX is intentionally
absent for H2 responses. See `docs/residual-differences.md` for the
classification of HTTPX gaps and the differential tests that pin
each behavior.

## Error Handling

eggfetch provides a rich exception hierarchy:

```python
import eggfetch

try:
    response = eggfetch.get("https://example.com", timeout=5)
except eggfetch.TimeoutException:
    print("Request timed out")
except eggfetch.NetworkError:
    print("Connection failed")
except eggfetch.TooManyRedirects:
    print("Too many redirects")
except eggfetch.EggfetchError as e:
    print(f"Request error: {e}")
```

### Exception hierarchy

```
EggfetchError
  +-- RequestError
  |     +-- InvalidUrl
  |     +-- TimeoutException
  |     |     +-- PoolTimeout
  |     |     +-- ConnectTimeout
  |     |     +-- ReadTimeout
  |     |     +-- WriteTimeout
  |     +-- NetworkError
  |     +-- ProtocolError
  |     +-- BodyError
  |     +-- TooManyRedirects
  |     +-- DecompressionError
  |     +-- UnsupportedContentEncoding
  |     +-- ProxyError
  |     |     +-- ProxyConnectError
  |     |     +-- ProxyAuthError
  |     +-- BodyNotReplayableForRetry
  |     +-- RetryBudgetExhausted
  |     +-- RetryNotConfigured
  |     +-- Http2Error
  |     |     +-- Http2GoAway
  |     |     +-- Http2StreamReset
  |     |     +-- Http2FlowControlError
  |     +-- H3Error
  |           +-- H3ConnectError
  |           +-- H3ProtocolError
  +-- HTTPStatusError
  +-- UnsupportedKwarg
  +-- StreamConsumed
  +-- StreamClosed
  +-- ResponseNotRead
```

## Context Managers

Both `Client` and `AsyncClient` support context managers for automatic cleanup.

```python
# Sync
with eggfetch.Client() as client:
    response = client.get("https://example.com")

# Async
async with eggfetch.AsyncClient() as client:
    response = await client.get("https://example.com")
```

Streaming responses also support context managers:

```python
async with eggfetch.AsyncClient() as client:
    async with client.stream("GET", "https://example.com") as response:
        async for chunk in response.bytes():
            process(chunk)
```

## Full Example

```python
import eggfetch

# Simple GET
response = eggfetch.get("https://httpbin.org/get", timeout=10)
print(response.status_code, response.text)

# POST with JSON and auth
response = eggfetch.post(
    "https://httpbin.org/post",
    json={"hello": "world"},
    auth=eggfetch.BearerAuth("my-token"),
    headers={"X-Custom": "value"},
)
print(response.json())

# Reusable client with connection pooling
with eggfetch.Client(
    headers={"User-Agent": "my-app/1.0"},
    timeout=30,
    follow_redirects=True,
    auth=eggfetch.BearerAuth("api-key"),
    retries=eggfetch.Retry(max_attempts=3),
) as client:
    # Multiple requests reuse the connection pool
    r1 = client.get("https://api.example.com/users")
    r2 = client.get("https://api.example.com/posts")
    print(r1.json(), r2.json())

# Streaming download
with eggfetch.Client() as client:
    with client.stream("GET", "https://example.com/large-file") as resp:
        with open("download.bin", "wb") as f:
            for chunk in resp.bytes():
                f.write(chunk)
```
