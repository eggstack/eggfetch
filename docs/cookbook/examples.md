# Cookbook

Practical, runnable examples for common eggfetch patterns.

## JSON REST API call

```python
import eggfetch

r = eggfetch.get("https://api.github.com/zen")
print(r.status_code, r.text)
```

A one-liner for simple GET requests. For anything more complex, use a
`Client` for connection reuse.

## Client with default headers

```python
import eggfetch

with eggfetch.Client(headers={"Accept": "application/json"}) as client:
    r = client.get("https://api.github.com/zen")
    data = r.json()
    print(data)
```

## POST JSON body

```python
import eggfetch

with eggfetch.Client() as client:
    r = client.post(
        "https://httpbin.org/post",
        json={"name": "eggfetch", "version": "0.1.0"},
    )
    print(r.json())
```

## Form-encoded POST

```python
import eggfetch

with eggfetch.Client() as client:
    r = client.post(
        "https://httpbin.org/post",
        data={"username": "alice", "password": "s3cret"},
    )
    print(r.json()["form"])
```

## Large streaming download

```python
import eggfetch

with eggfetch.Client() as client:
    with client.stream("GET", "https://httpbin.org/stream-bytes/1000000") as r:
        total = 0
        for chunk in r.iter_bytes(chunk_size=65536):
            total += len(chunk)
        print(f"Downloaded {total} bytes")
```

The body is never fully buffered in memory.

## Streaming upload

```python
import eggfetch

def generate():
    for i in range(10):
        yield f"line {i}\n".encode()

with eggfetch.Client() as client:
    r = client.post("https://httpbin.org/post", content=generate())
    print(r.status_code)
```

## SSE-like line streaming

```python
import eggfetch

with eggfetch.Client() as client:
    with client.stream("GET", "https://httpbin.org/stream/3") as r:
        for line in r.iter_lines():
            print(line)
```

Each line is decoded from bytes to str automatically.

## Cookie session and login flow

```python
import eggfetch

with eggfetch.Client() as client:
    # Set cookies before the first request
    client.cookies.set("csrf_token", "abc123")

    # Login
    r = client.post(
        "https://example.com/login",
        data={"user": "alice", "pass": "s3cret"},
    )

    # Subsequent requests reuse the session cookies
    r = client.get("https://example.com/dashboard")
    print(r.text)
```

## Basic authentication

```python
import eggfetch

r = eggfetch.get(
    "https://httpbin.org/basic-auth/alice/s3cret",
    auth=eggfetch.BasicAuth("alice", "s3cret"),
)
print(r.status_code)
```

## Bearer token authentication

```python
import eggfetch

token = "ghp_abcdef1234567890"
r = eggfetch.get(
    "https://api.github.com/user",
    auth=eggfetch.BearerAuth(token),
)
print(r.json()["login"])
```

## Custom CA and mTLS

```python
import eggfetch

client = eggfetch.Client(
    verify="/etc/corp/ca-bundle.pem",
    cert=("/path/to/client-cert.pem", "/path/to/client-key.pem"),
)
with client:
    r = client.get("https://internal.corp.example.com/api")
    print(r.status_code)
```

## Proxy with NO_PROXY

```python
import eggfetch

client = eggfetch.Client(
    proxy="http://proxy.corp.example.com:8080",
    proxy_auth="proxyuser:proxypass",
    no_proxy="localhost,127.0.0.1,.internal.corp.example.com",
)
with client:
    # Bypasses proxy
    r = client.get("https://internal.corp.example.com/api")
    # Uses proxy
    r = client.get("https://external.example.com")
```

## Retry policy configuration

```python
import eggfetch

retry = eggfetch.Retry(
    max_attempts=4,
    backoff_factor=0.3,
    statuses={429, 502, 503, 504},
)

with eggfetch.Client(retries=retry) as client:
    r = client.get("https://flaky-api.example.com/data")
    print(r.status_code)

# Disable retries on a per-request basis
r = client.get("https://example.com", retries=False)
```

## gzip/brotli/zstd response handling

eggfetch decompresses responses automatically when compression features
are compiled in (enabled by default in the Python package).

```python
import eggfetch

# Automatic decompression is on by default
r = eggfetch.get("https://httpbin.org/gzip")
print(r.json()["gzipped"])

# Disable decompression per-request
r = client.get(url, decompress=False)
```

## Concurrent async requests

```python
import asyncio
import eggfetch

async def main():
    async with eggfetch.AsyncClient() as client:
        urls = [
            "https://httpbin.org/get",
            "https://httpbin.org/ip",
            "https://httpbin.org/headers",
        ]
        tasks = [client.get(url) for url in urls]
        responses = await asyncio.gather(*tasks)
        for r in responses:
            print(r.status_code, r.url)

asyncio.run(main())
```

## Async streaming

```python
import asyncio
import eggfetch

async def main():
    async with eggfetch.AsyncClient() as client:
        async with client.stream("GET", "https://httpbin.org/stream-bytes/100000") as r:
            async for chunk in r.aiter_bytes(chunk_size=8192):
                print(f"Got {len(chunk)} bytes")

asyncio.run(main())
```

## CLI: JSON output

```bash
curl -s https://api.github.com/zen | eggfetch --json https://httpbin.org/post --json --json
```

## CLI: NDJSON output

```bash
eggfetch --ndjson https://httpbin.org/stream/5
```

## CLI: File upload

```bash
eggfetch -X POST https://httpbin.org/post \
    --file "document=@report.pdf" \
    --file "image=@photo.jpg:image/jpeg"
```

## CLI: Download mode

```bash
eggfetch --download --no-clobber https://example.com/large-file.zip
```

Derives the filename from Content-Disposition or the URL path.

## Error handling patterns

```python
import eggfetch

try:
    r = eggfetch.get("https://example.com/api", timeout=5)
    r.raise_for_status()
except eggfetch.TimeoutException:
    print("Request timed out")
except eggfetch.HTTPStatusError as e:
    print(f"HTTP {e.response.status_code}: {e}")
except eggfetch.NetworkError as e:
    print(f"Connection failed: {e}")
except eggfetch.EggfetchError as e:
    print(f"Request error: {e}")
```

## Timeout configuration

```python
import eggfetch

# Simple: 10 seconds for everything
r = eggfetch.get(url, timeout=10)

# Per-phase control
t = eggfetch.Timeout(pool=2, connect=5, read=30, total=60)
r = eggfetch.get(url, timeout=t)
```
