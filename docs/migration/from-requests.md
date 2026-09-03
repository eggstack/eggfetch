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
connection slot) and `total` (native-only wall-clock cap across the entire
request).

Each configured phase is independently enforced: `connect` bounds direct and
proxy connection setup (through HTTP/HTTPS proxies it also bounds proxy
TCP/TLS setup and origin TLS after CONNECT), while `total`, when explicitly
configured through the native API, remains an outer cap.

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

Both libraries strip `Authorization` on cross-origin redirects by default:
requests drops it when the host changes (`Session.rebuild_auth`), and
eggfetch strips `Authorization`/`Proxy-Authorization` on every redirect hop
(plus `Cookie`/`Host` on cross-origin hops) before re-applying configured
client-level auth.

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

The Rust core configures proxies explicitly and never reads proxy
environment variables. The native Python `Client`/`AsyncClient` read
`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, and `NO_PROXY` by default
(`trust_env=True`; pass `trust_env=False` for explicit-only configuration).
To bypass the proxy for certain hosts with an explicit proxy:

```python
import os
os.environ["NO_PROXY"] = "localhost,127.0.0.1,.internal.example.com"
client = eggfetch.Client(proxy="http://proxy.example.com:8080")
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

## Retry

requests itself ships no built-in retry; retries are configured through the
urllib3 `Retry` adapter mounted on a `Session` (backoff, status allowlist,
method allowlist). eggfetch ships a native `Retry` policy instead:

```python
retry = eggfetch.Retry(max_attempts=3, backoff_factor=0.2, statuses={429, 503})
r = client.get(url, retries=retry)

# Disable retries per-request
r = client.get(url, retries=False)
```

Only safe methods (GET, HEAD, OPTIONS) are retried by default; POST/PUT
require explicit opt-in, and one-shot streaming bodies are not retried.

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
| Auth types | Tuple `(user, pass)` | `BasicAuth` / `BearerAuth` objects (tuple shorthand also accepted) |
| Timeout phases | `timeout` float or `(connect, read)` tuple | `Timeout` object with per-phase control (`pool`, `connect`, `read`, `write`, native `total`) |
| Proxy env vars | Reads `HTTP_PROXY` etc. | Native Python reads env by default (`trust_env=True`); Rust core/CLI are explicit-only |
| Async | No built-in async | Built-in `AsyncClient` |
| HTTP/2 | No | Yes (opt-in) |
| HTTP/3 | No | Experimental (opt-in) |
| Retry | Via urllib3 adapter mounted on `Session` | Built-in `Retry` policy |
| Compression | Decoded `iter_content`, raw via `stream + raw` | Automatic decoded iteration; raw encoded path selectable |
