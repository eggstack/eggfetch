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
| `errors.rs` | Exception hierarchy |
| `auth.rs` | `BasicAuth`, `BearerAuth` |
| `cookies.rs` | Cookie handling |
| `proxy.rs` | Proxy configuration |
| `retry.rs` | Retry configuration |
| `timeout.rs` | Timeout configuration |
| `tls.rs` | TLS configuration (`verify`, `cert` kwargs) |
| `multipart.rs` | `File` wrapper for multipart uploads |
| `streaming.rs` | `StreamingResponse` — sync/async iterators |
| `conversion.rs` | Python↔Rust type conversion (shared by sync/async) |
| `limits.rs` | `PyLimits` — pool concurrency limits |

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
Streaming responses retain the Tokio handle that created them; synchronous
iterators and buffered reads run on that same client runtime so transport body
state and connection-pool leases are never moved to an unrelated runtime.
The response also retains a runtime lease, so an open stream remains readable
after its `Client` is closed; the runtime shuts down once the last lease is
released.

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
│   └── H3Error
│       ├── H3ConnectError
│       └── H3ProtocolError
├── ProxyError
│   ├── ProxyConnectError
│   └── ProxyAuthError
├── TooManyRedirects
├── BodyError
├── DecompressionError
├── UnsupportedContentEncoding
├── StreamConsumed
├── StreamClosed
├── ResponseNotRead
├── BodyNotReplayableForRetry
├── RetryBudgetExhausted
├── RetryNotConfigured
├── UnsupportedKwarg
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

## HTTPX Compatibility Facade

The `eggfetch.compat.httpx` module provides an HTTPX 0.28.1-compatible facade over the native eggfetch bindings. This enables existing HTTPX code to run against the eggfetch Rust engine with minimal changes.

### Architecture Overview

The facade sits between the user and the native PyO3 bindings:

```
User code
  ↓
eggfetch.compat.httpx (pure Python facade)
  ↓
eggfetch (native PyO3 bindings)
  ↓
eggfetch-core (Rust async engine)
```

The facade owns all HTTPX-shaped API surfaces (URL, Headers, QueryParams, exceptions, client constructors, merge rules) as pure-Python value objects. Network execution still flows through the native `eggfetch` module into `eggfetch-core`. The facade never duplicates networking logic.

### Module Structure

| Module | Purpose |
|--------|---------|
| `eggfetch/compat/httpx/__init__.py` | Public facade — exports all HTTPX symbols |
| `eggfetch/compat/httpx/_urls.py` | `URL` and `QueryParams` value objects |
| `eggfetch/compat/httpx/_headers.py` | `Headers` — case-insensitive, duplicate-preserving |
| `eggfetch/compat/httpx/_cookies.py` | `Cookies` — mapping, domain/path, conflict handling |
| `eggfetch/compat/httpx/_timeout.py` | `Timeout` — per-phase configuration |
| `eggfetch/compat/httpx/_limits.py` | `Limits` — pool concurrency control |
| `eggfetch/compat/httpx/_proxy.py` | `Proxy` — proxy configuration |
| `eggfetch/compat/httpx/_status_codes.py` | `codes` — status code constants |
| `eggfetch/compat/httpx/_exceptions.py` | Exception hierarchy matching HTTPX MRO |
| `eggfetch/compat/httpx/_request.py` | `Request` — construction and auto-headers |
| `eggfetch/compat/httpx/_response.py` | `Response` — metadata, status helpers, raise_for_status |
| `eggfetch/compat/httpx/_client.py` | `Client` and `AsyncClient` — constructors, merge, build_request, send |

### Bridge Pattern (Native ↔ Compat Conversion)

The facade converts between HTTPX-compatible objects and native types at the boundary:

- **URL → core URL**: Pure-Python `URL` normalizes to a string; passed to native `Client.request()`.
- **Headers → native headers**: `Headers` exposes duplicate-preserving iteration; flattened to a list of `(name, value)` pairs for native calls.
- **QueryParams → URL**: Request construction materializes merged query pairs into the URL; native dispatch does not forward them a second time.
- **Timeout → native timeout**: `Timeout` uses an HTTPX-compatible private
  `UNSET` sentinel so omitted and explicit `None` phase values remain distinct;
  its four operational fields map to native phase-aware timeout config and
  never synthesize native `total`.
- **Limits → native limits**: `Limits` fields map to `PoolConfig`.
- **Proxy → native proxy**: `Proxy.url` and supported URL credentials map to
  the native proxy string/authentication path. Non-empty `Proxy(headers=...)`
  is rejected before dispatch because the bounded native API has no proxy-leg
  header channel; it is an explicit Stage C difference.
- **Request → native body**: Body kwargs dispatched by mutual-exclusion rules; auto-headers computed by the facade.
- **Response ← native response**: Status, headers, URL, body, and version extracted from native `PyResponse`.
- **Errors ← native errors**: Exception mapping preserves the most specific HTTPX class with redacted context.

### What Phase 2 Implements

- Pure-Python value objects with exact HTTPX semantics for URL, QueryParams, Headers, Cookies, Timeout, Limits, Proxy.
- Request and Response objects with HTTPX-compatible construction, properties, and state machines.
- **Request URL params**: `params` argument merges into `request.url` matching HTTPX replacement semantics.
- **Body source exclusion**: `content`, `json`, `data`, `files`, `stream` follow HTTPX mutual-exclusion rules; `data` + `files` is valid (multipart).
- **JSON serialization**: Compact format with `separators=(",", ":")`, UTF-8 encoding, correct headers.
- **Multipart**: `data` + `files` combined in one body; 4-tuple file spec `(filename, fileobj, content_type, headers)` supported.
- **Auto-headers**: No auto `Transfer-Encoding: chunked` for explicit `stream=`; Content-Length for encoded bodies.
- **Response extensions**: `http_version` and `reason_phrase` read from response extensions (bytes or strings).
- **Response.request**: Attaches request; URL derives from attached request.
- **Response.elapsed**: Raises before read/close for streaming; available for buffered responses.
- **Status predicates**: `is_informational`, `is_success`, `is_redirect`, `is_client_error`, `is_server_error`, `is_error`, `has_redirect_location` match HTTPX.
- **raise_for_status()**: Raises for all non-success statuses (informational, redirect, client-error, server-error) with request attachment.
- **next_request**: Exposed and defaults to `None` for Phase 4 redirect support.
- **History**: Copied at construction; setter replaces the list.
- **Stream exceptions**: `ResponseNotRead`, `StreamClosed`, `StreamConsumed` raised at correct state boundaries.
- **Stream state**: `is_closed`, `is_stream_consumed`, and `num_bytes_downloaded` update during sync/async chunked iteration, read, close, and failure paths.
- **Corrective boundaries**: Redirect body replay uses one source, timeout mappings cross as native timeout objects, cookies are merged per hop, and query pairs are serialized once.
- **Encoding**: Callable `default_encoding`; `encoding` setter raises after `text` access.
- **Repr**: Response includes status and reason phrase; URL redacts passwords.
- **Exports**: `main()` and `create_ssl_context()` stubs added for API compatibility.
- Full exception hierarchy (HTTPError → RequestError/TransportError → specific subclasses) with MRO matching the pinned manifest.
- Client and AsyncClient constructors with all HTTPX parameters (auth, params, headers, cookies, verify, cert, trust_env, proxy, timeout, limits, follow_redirects, max_redirects, base_url, default_encoding, event_hooks).
- Configuration merge semantics: client-level defaults merge with per-request overrides for params, headers, cookies, auth, timeout, extensions, and redirect policy.
- `build_request()` producing a fully merged Request without sending.
- `send()` preserving object identity, body, and extensions.
- Top-level helpers (`get`, `post`, etc.) using short-lived facade clients.

### What Phase 3 Implements

**Streaming and body:**
- Stream base classes (`SyncByteStream`, `AsyncByteStream`, `ByteStream`) for custom body producers.
- Response streaming delegation: compat `Response` iterates over native `StreamingResponse` chunks.
- `iter_raw()`/`aiter_raw()` for undecoded transport-level bytes.
- Chunk size parameter on all streaming iterators; compatibility `iter_raw()`/`aiter_raw()` default to `None`, while decoded iterator defaults remain 8192.
- `IncrementalDecoder` for multibyte character safety in text iterators.
- Request streaming: iterable content, file-like objects, and custom `ByteStream` subclasses passed through to the native streaming engine.
- Multipart passthrough: compat `Request` delegates multipart encoding to the native encoder.
- `StreamingRawBytesIterator` and `AsyncStreamingRawBytesIterator` types exposed to Python.

**Transport, mount, hook, and dispatch (Phase 3 correction):**

- **One-hop dispatch contract**: `_dispatch_one_hop()` sends exactly one prepared Request through exactly one selected transport. Native dispatch always uses `follow_redirects=False`. The higher-level client owns auth/redirect orchestration.
- **Timeout extension**: Effective timeout is placed into `request.extensions["timeout"]` unless the caller provided one, matching HTTPX's transport contract.
- **Per-hop event hooks** (Track 4): Request and response hooks run around every dispatch hop. For auth challenges, hooks see each intermediate request/response, not just the final pair. Ordering: auth yields Request → request hook → transport → response hook → auth/redirect decision.
- **Faithful mount matching** (Track 3): `_parse_mount_pattern()` returns a 5-tuple `(scheme, host, port, path, is_wildcard)`. Wildcard domain patterns (`all://*.example.com`) are supported. Priority follows HTTPX 0.28.1: exact host+port+path > wildcard > host+port > host+path > host > scheme > catch-all. Explicit `None` mounts bypass to default transport. Malformed patterns are rejected at construction.
- **Extension preservation** (Track 5): Client and request extensions merge losslessly. Request extensions live on `response.request.extensions`, never on `response.extensions`. Response extensions from the transport handler are preserved without overwriting.
- **Transport ownership and close** (Track 6): Duplicate mounted transport instances close exactly once. Close errors propagate (last error raised). Client close is idempotent.
- **Transport body preservation** (Track 2): Buffered custom transport responses retain their body under `stream=True`. Streaming responses remain lazy. `HTTPTransport`/`AsyncHTTPTransport` always return stream-backed responses.

### What Phase 4 Implements

**Redirect, authentication, cookie, and history state machine:**

- **Python-level redirect loop** (Track 1, 2): `_send_handling_redirects()` replaces the native redirect-following. Each hop runs request/response hooks, dispatches through `_send_single_request()`, and builds the next request via `_build_redirect_request()`. `follow_redirects=True` follows automatically; `False` sets `response.next_request`.
- **Method rewriting** (Track 2.1): 303 → GET (non-HEAD), 302 → GET (non-HEAD), 301 POST → GET. 307/308 retain method and body.
- **URL resolution** (Track 2.2): Absolute, relative, scheme-relative, and malformed Location headers resolved. Fragment inheritance per RFC 7231 7.1.2.
- **Header stripping** (Track 2.3): `Authorization` stripped on cross-origin redirects (except HTTP→HTTPS same-host). `Host` updated. `Cookie` header stripped on all redirects (same-origin and cross-origin) and regenerated from the client jar for the destination URL. Content headers stripped when method changes to GET.
- **History management** (Track 1.2): Single authoritative history list. Redirect responses appended when followed. Auth challenge responses appended when auth yields a follow-up. Final response gets `response.history = list(history)`.
- **Manual redirects** (Track 2.5): `response.next_request` populated for unfollowed redirects.
- **max_redirects** (Track 2.6): `TooManyRedirects` raised with request attached.
- **Scoped cookie jar** (Track 4): `Cookies` wraps `http.cookiejar.CookieJar` (Preferred A architecture). Domain/path/secure/expiry scoping. Multiple Set-Cookie headers parsed. `CookieConflict` on ambiguous `.get(name)`. Cookies extracted from each response, set on each request hop.
- **Auth flow integration** (Track 3): Auth generators drive the outer loop. Auth-produced requests go through the full redirect handler. Cross-origin auth stripping matches HTTPX. Intermediate auth challenge responses added to history.
- **Hook ordering** (Track 5): Per-hop order: auth yields → Cookie header set → request hook → transport → response hook → cookie extraction → redirect/auth decision. Each actual transport hop produces exactly one request-hook and one response-hook call.
- **Resource cleanup** (Track 6): Auth generators closed on all exits. Intermediate redirect responses read and closed when followed. `TooManyRedirects` preserves the request reference.

### Corrective parity closure

The facade has one authoritative Python cookie jar, request-relative timeout mapping, explicit buffered/live response state, body replay classification (buffered, reusable-stream, multipart-reconstructable, one-shot), explicit Cookie header stripping on redirects, and raw stream lifecycle management (consumed state, source-byte accounting, bounded chunk adaptation, normal-exhaustion close). For the documented HTTPX 0.28.1 asyncio-supported surface, retained bodies replay or fail before redispatch and redirects regenerate cookies for each destination. Compatibility raw bytes remain distinct from decoded response data: the core response retains compressed encoded bytes until first selection, and the Python binding selects raw or decoded mode exactly once. The existing core decoded-header policy removes `Content-Encoding` and `Content-Length` from automatically decompressed response metadata; the facade overlays only the original wire values for those headers using the native binding's narrow metadata accessors. Native cancellation coverage uses the built-in async client and proves lease release with a constrained follow-up request. `scripts/check.sh` Tier 1 runs `tests/compat/test_corrective_kernel.py`; `tests/compat/test_raw_stream_httpx_differential.py`, the complete pinned HTTPX suite, and the API oracle remain extended validation. Keep `plans/httpx-parity-raw-stream-final-corrective-closure.md` and `plans/httpx-parity-correction-status.md` exact-SHA-bound when changing compatibility claims.

### Corrective transport closure

The compatibility facade discovers proxy environment state through
`urllib.request.getproxies()`, matching HTTPX 0.28.1's lowercase precedence,
scheme-less HTTP proxy normalization, and `NO_PROXY` behavior. UDS uses the
shared Hyper HTTP/TLS path and has executable fixed-length, chunked, TLS, and
keep-alive coverage. `local_address` remains HTTPX's host-only bind form and
is tested against the server-observed source address. SOCKS clients are
persistent per effective route, advertise exactly the reference-selected
authentication method, send HTTPX-compatible destination address types, and
preserve origin-form requests after CONNECT. The facade intentionally bounds
`socket_options` to safe three-element tuples; integer, `bytes`, and
`bytearray` values are converted losslessly. HTTPX's valid four-element
`(level, option, None, optlen)` form is accepted by its constructor and
forwarded to the platform socket API, but EggFetch does not expose arbitrary
null-pointer socket operations in its safe Rust boundary. HTTP and HTTPS proxy
endpoint schemes are both supported; HTTPS proxy TLS uses the proxy hostname,
while origin TLS after CONNECT uses the origin hostname.
Non-empty `Proxy(headers=...)` is rejected before native dispatch because the
bounded native API has no proxy-leg header channel; this remains an explicit
Stage C difference rather than silently dropping metadata.
