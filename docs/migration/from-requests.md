# Migrating from requests

eggfetch is **not** a drop-in replacement for requests. The API surface is
similar, but there are real differences in naming, defaults, and behavior.
This guide covers what changes and what stays the same.

## Installation

```bash
pip install eggfetch
```

eggfetch compiles a native Rust binary extension. There is no pure-Python
fallback. Python 3.10 through 3.13 are supported.

## Sessions and clients

requests uses `Session`. eggfetch uses `Client`.

```python
# requests
import requests
s = requests.Session()
r = s.get("https://example.com")

# eggfetch
import eggfetch
with eggfetch.Client() as client:
    r = client.get("https://example.com")
```

eggfetch `Client` is a context manager. Prefer `with` blocks so the
connection pool is released cleanly. You can call methods directly on the
module without a client for one-off requests:

```python
import eggfetch
r = eggfetch.get("https://example.com")
```

## Request methods

The method names are identical: `get`, `post`, `put`, `patch`, `delete`,
`head`, `options`. The `request` method accepts a method string.

```python
# Same in both
r = client.get("https://example.com")
r = client.post("https://example.com", json={"key": "value"})
r = client.request("DELETE", "https://example.com/resource")
```

## Timeouts

requests uses a single `timeout` as a float (seconds) or a
`(connect, read)` tuple. eggfetch uses a `Timeout` object or a plain float
that applies to all phases.

```python
# requests: (connect_timeout, read_timeout)
r = requests.get("https://example.com", timeout=(3.0, 10.0))

# eggfetch: single float applies to all phases
r = eggfetch.get("https://example.com", timeout=10.0)

# eggfetch: explicit Timeout for per-phase control
t = eggfetch.Timeout(pool=2, connect=5, read=30, total=60)
r = eggfetch.get("https://example.com", timeout=t)
```

eggfetch has phases requests does not: `pool` (time waiting for a
connection slot) and `total` (wall-clock cap across the entire request).

The `connect` phase is accepted but not independently enforced. Use
`total` as a backstop.

## Streaming

requests buffers the full response by default. Use `iter_content` for
streaming. eggfetch has a dedicated streaming API that reads from the
network without buffering the full body.

```python
# requests
r = requests.get("https://example.com/large", stream=True)
for chunk in r.iter_content(chunk_size=8192):
    process(chunk)

# eggfetch: true network streaming via client.stream()
with eggfetch.Client() as client:
    with client.stream("GET", "https://example.com/large") as r:
        for chunk in r.iter_bytes(chunk_size=8192):
            process(chunk)
```

eggfetch also provides `iter_text()` and `iter_lines()` on the streaming
response. Buffered responses (without `stream()`) support
`response.text`, `response.content`, and `response.json()`.

## Authentication

requests uses `auth=("user", "pass")` for Basic auth. eggfetch uses
typed auth objects.

```python
# requests
r = requests.get("https://example.com", auth=("user", "pass"))

# eggfetch: BasicAuth
r = eggfetch.get("https://example.com", auth=eggfetch.BasicAuth("user", "pass"))

# eggfetch: BearerAuth
r = eggfetch.get("https://example.com", auth=eggfetch.BearerAuth("my-token"))
```

Auth can also be set on the client:

```python
with eggfetch.Client(auth=eggfetch.BasicAuth("user", "pass")) as client:
    r = client.get("https://example.com")
```

eggfetch strips `Authorization` headers on cross-origin redirects by
default. requests forwards them unless you use a redirect hook.

To disable auth on a per-request basis when the client has auth configured:

```python
r = client.get("https://public.example.com", auth=eggfetch.NOAUTH)
```

## Cookies

requests stores cookies on `Session.cookies` (a `CookieJar`). eggfetch
exposes cookies on the `Client` object.

```python
# requests
s = requests.Session()
s.cookies.set("session", "abc123")
r = s.get("https://example.com")
print(s.cookies)

# eggfetch
with eggfetch.Client() as client:
    client.cookies.set("session", "abc123")
    r = client.get("https://example.com")
```

Request-level cookies pass as a dict:

```python
r = client.get("https://example.com", cookies={"session": "value"})
```

## Proxies

requests uses a `proxies` dict keyed by scheme. eggfetch takes a single
proxy URL string.

```python
# requests
proxies = {"https": "http://proxy.example.com:8080"}
r = requests.get("https://example.com", proxies=proxies)

# eggfetch
r = eggfetch.get("https://example.com", proxy="http://proxy.example.com:8080")
```

eggfetch does **not** read `HTTP_PROXY` or `HTTPS_PROXY` environment
variables. Proxy configuration is explicit only. To bypass the proxy for
certain hosts:

```python
client = eggfetch.Client(
    proxy="http://proxy.example.com:8080",
    no_proxy="localhost,127.0.0.1,.internal.example.com",
)
```

## SSL/TLS

requests uses `verify` (bool or CA bundle path) and `cert` (str or tuple).
eggfetch uses the same parameter names with the same semantics.

```python
# requests
r = requests.get("https://example.com", verify=False)
r = requests.get("https://example.com", verify="/path/to/ca-bundle.pem")
r = requests.get("https://example.com", cert=("/path/cert.pem", "/path/key.pem"))

# eggfetch: identical
r = eggfetch.get("https://example.com", verify=False)
r = eggfetch.get("https://example.com", verify="/path/to/ca-bundle.pem")
r = eggfetch.get("https://example.com", cert=("/path/cert.pem", "/path/key.pem"))
```

A custom CA bundle **replaces** the default system roots entirely. If you
need both, concatenate them into a single PEM file.

## Redirects

requests follows redirects by default. eggfetch does **not** follow
redirects by default. You must opt in.

```python
# requests: follows by default
r = requests.get("https://example.com/redirect")

# eggfetch: must set follow_redirects=True
r = eggfetch.get("https://example.com/redirect", follow_redirects=True)

# Or set it on the client
with eggfetch.Client(follow_redirects=True) as client:
    r = client.get("https://example.com/redirect")
```

Max redirects:

```python
r = client.get(url, follow_redirects=True, max_redirects=5)
```

## Error handling

requests raises `requests.exceptions.*`. eggfetch raises its own hierarchy
rooted at `EggfetchError`.

| requests                       | eggfetch                |
| ------------------------------ | ----------------------- |
| `ConnectionError`              | `NetworkError`          |
| `Timeout`                      | `TimeoutException`      |
| `HTTPError`                    | `HTTPStatusError`       |
| `TooManyRedirects`            | `TooManyRedirects`      |
| `SSLError`                     | (part of `NetworkError`)|
| `InvalidURL`                   | `InvalidUrl`            |
| `ContentDecodingError`         | `BodyError`             |

```python
# requests
try:
    r = requests.get("https://example.com", timeout=5)
    r.raise_for_status()
except requests.exceptions.Timeout:
    print("timed out")
except requests.exceptions.HTTPError as e:
    print(f"HTTP error: {e}")

# eggfetch
try:
    r = eggfetch.get("https://example.com", timeout=5)
    r.raise_for_status()
except eggfetch.TimeoutException:
    print("timed out")
except eggfetch.HTTPStatusError as e:
    print(f"HTTP error: {e}")
```

`raise_for_status()` is available on the response object and works the
same way.

## JSON

Both libraries support `response.json()` and `json=` on requests.

```python
# Same pattern
r = client.post("https://example.com/api", json={"key": "value"})
data = r.json()
```

## Multipart file uploads

requests uses `files=` with dicts or tuples. eggfetch supports the same
tuple formats plus a `File` wrapper.

```python
# requests
files = {"file": open("data.bin", "rb")}
r = requests.post("https://example.com/upload", files=files)

# eggfetch
with open("data.bin", "rb") as f:
    r = client.post("https://example.com/upload", files={"file": f})

# eggfetch: tuple formats
files = {"file": ("data.bin", b"content", "application/octet-stream")}
r = client.post("https://example.com/upload", files=files)

# eggfetch: File wrapper for path-based uploads
r = client.post("https://example.com/upload", files={"file": eggfetch.File("/path/to/file")})
```

## Key differences summary

| Feature | requests | eggfetch |
| --- | --- | --- |
| Redirects | Follow by default | **Do not follow** by default |
| Streaming | `stream=True` + `iter_content` | `client.stream()` context manager |
| Auth types | Tuple `(user, pass)` | `BasicAuth` / `BearerAuth` objects |
| Timeout phases | `timeout` float or tuple | `Timeout` object with per-phase control |
| Proxy env vars | Reads `HTTP_PROXY` etc. | **Does not** read proxy env vars |
| Async | No built-in async | Built-in `AsyncClient` |
| HTTP/2 | No | Yes (opt-in) |
| HTTP/3 | No | Experimental (opt-in) |
| Retry | Manual or via `urllib3` | Built-in `Retry` policy |
| Compression | Manual via `Accept-Encoding` | Automatic decompression |
