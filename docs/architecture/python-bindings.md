# Python Bindings Deep Dive

The Python crate (`eggfetch-python`) uses PyO3/maturin to expose `eggfetch-core` to Python. It provides both sync and async APIs over the async Rust engine.

See also: [overview.md](overview.md).

## Module Map

| Module | Purpose |
|--------|---------|
| `lib.rs` | Module registration + top-level functions (`get`, `post`, etc.) |
| `client.rs` | `Client` — sync adapter with persistent runtime |
| `async_client.rs` | `AsyncClient` — async adapter targeting asyncio |
| `response.rs` | `PyResponse` — buffered response surface |
| `headers.rs` | `PyHeaders` — header wrapper |
| `errors.py` | Exception hierarchy |
| `auth.rs` | `BasicAuth`, `BearerAuth` |
| `cookies.rs` | Cookie handling |
| `proxy.rs` | Proxy configuration |
| `retry.rs` | Retry configuration |
| `timeout.rs` | Timeout configuration |
| `tls.rs` | TLS configuration (`verify`, `cert` kwargs) |
| `multipart.rs` | `File` wrapper for multipart uploads |
| `streaming.rs` | `StreamingResponse` — sync/async iterators |
| `conversion.rs` | Python↔Rust type conversion (shared by sync/async) |

## Sync Adapter

Each `PyClient` owns a tokio runtime and an `eggfetch_core::Client`.

### Request Flow

1. Convert Python arguments to owned Rust types (headers, URL, body bytes, timeout).
2. Release the GIL via `py.allow_threads`.
3. Block on the async Rust engine via `runtime.block_on(future)`.
4. Buffer the response body via `response.bytes().await`.
5. Re-acquire the GIL and return a `PyResponse` with buffered data.

### Streaming Sync Flow

`client.stream("GET", url)` returns a `StreamingResponse` context manager. Iterating advances the stream one chunk at a time, releasing the GIL during each read.

### Top-Level Helpers

`eggfetch.get(...)`, `eggfetch.post(...)`, etc. create a short-lived runtime and client per call. The `Client` class owns a persistent runtime for connection reuse.

## Async Adapter

`AsyncClient` targets asyncio via `pyo3-async-runtimes`.

### Request Flow

Each request method uses `future_into_py` to convert the Rust future into a Python awaitable. The async block buffers the response body before returning.

### Design Decisions

- **Unified response construction**: both sync and async use `PyResponse::from_core_response_with_body()`.
- **Pre-resolved futures**: `__aenter__`/`__aexit__` return pre-resolved `asyncio.Future` objects.
- **Cancellation safety**: cancelling an in-flight request drops the Rust future cleanly; pool permits are released via RAII.

## Response Surface

`PyResponse` presents a requests/httpx-compatible surface:

| Property/Method | Description |
|-----------------|-------------|
| `status_code` | HTTP status code |
| `reason_phrase` | Status reason phrase |
| `headers` | `PyHeaders` wrapper |
| `url` | Final URL (after redirects) |
| `content` | Raw bytes |
| `text` | Decoded text |
| `encoding` | Detected or explicit encoding |
| `http_version` | `"HTTP/1.1"`, `"HTTP/2"`, etc. |
| `history` | Redirect history |
| `json(**kwargs)` | JSON deserialization |
| `iter_bytes()` | Byte chunk iterator |
| `iter_text()` | Text chunk iterator |
| `iter_lines()` | Line iterator |
| `raise_for_status()` | Raise `HTTPStatusError` for 4xx/5xx |
| `cookies` | Response cookies |

### Text Decoding

Priority: explicit `encoding` kwarg > Content-Type charset > UTF-8 fallback. Uses `encoding_rs` for non-UTF-8 charsets.

## Exception Hierarchy

```
EggfetchError
├── RequestError
├── InvalidUrl
├── TimeoutException
│   ├── ConnectTimeout
│   ├── ReadTimeout
│   ├── WriteTimeout
│   └── PoolTimeout
├── NetworkError
├── ProtocolError
│   ├── Http2Error
│   │   ├── Http2GoAway
│   │   ├── Http2StreamReset
│   │   └── Http2FlowControlError
│   └── Http3Error
├── BodyError
└── HTTPStatusError
```

## Kwargs Reference

| Kwargs | Type | Description |
|--------|------|-------------|
| `headers` | dict/sequence | Request headers |
| `params` | dict/sequence | Query parameters |
| `content` | bytes/str/bytearray | Raw body |
| `data` | dict/sequence | Form data (urlencoded) |
| `json` | any | JSON-serializable object |
| `files` | dict | Multipart file uploads |
| `timeout` | float/Timeout | Request timeout (seconds) |
| `cookies` | dict | Request-local cookies |
| `auth` | Auth/NOAUTH | Authentication override |
| `follow_redirects` | bool | Redirect following |
| `max_redirects` | int | Maximum redirects |
| `verify` | bool/str | TLS verification |
| `cert` | str/tuple | Client certificate |
| `http2` | bool | HTTP/2 negotiation |
| `http3` | bool | HTTP/3 (experimental) |
| `decompress` | bool | Automatic decompression |

Body kwargs (`content`, `data`, `json`) are mutually exclusive. `files` may combine with `data` but conflicts with `content` and `json`.
