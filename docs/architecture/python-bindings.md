# Python Bindings Deep Dive

The Python crate (`eggfetch-python`) uses PyO3/maturin to expose `eggfetch-core` to Python. It provides both sync and async APIs over the async Rust engine.

See also: [overview.md](overview.md).

## Native package contract

The supported native Python import surface is the pure-Python `eggfetch`
package. Its explicit `eggfetch.__all__` list is the single public export
contract and is checked against
`crates/eggfetch-python/tests/native_api_manifest.json`. The `_native` PyO3
extension is private implementation machinery: it intentionally has no
`__all__`, so registering an implementation symbol cannot silently create a
second supported API.

The top-level surface exports the complete native exception hierarchy,
including `DecompressionError`, `UnsupportedContentEncoding`, `H3Error`,
`H3ConnectError`, and `H3ProtocolError`. `NetworkStream` and
`AsyncNetworkStream` are public concrete wrappers for writable 101 upgrade
streams; ordinary pooled responses and internal CONNECT tunnels do not expose
one. Their methods and the important client/helper constructors are covered by
the native API oracle. Release validation also checks that the native runtime
version, coordinated Cargo/`pyproject.toml` version, and installed wheel
metadata agree.

### PEP 561 typing contract

`eggfetch/py.typed` marks the package as typed. The reviewed native stubs are
`eggfetch/__init__.pyi` (the explicit package re-export surface) and
`eggfetch/_native.pyi` (the extension declarations). The native API manifest
also carries a reviewed member inventory and semantic contract table.
`check_python_typing_surface.py` verifies every tracked property and method,
sync/async declaration shape, reliable parameter structure, and selected
return/property annotations; the native runtime oracle checks the same
inventory against live PyO3 members and signatures. This keeps runtime
signatures as evidence without attempting to synthesize Python unions from
Rust types. `mypy` consumer fixtures cover native and compatibility entry
points, including negative sync-client/async-body, unsupported verify-sequence,
and stale `start_tls() -> None` cases. Built-wheel validation repeats both the
surface and consumer checks from the installed artifact. Private
underscore-prefixed implementation modules are not part of the typing promise.

The checker also enforces relational contracts: `Client` and `AsyncClient`
constructors and request/verb methods keep their reviewed parameter inventory,
top-level helpers retain the deliberate `limits` versus client-only
`extensions` distinction, and body-capable versus body-less convenience
helpers cannot drift independently. The concrete PyO3 declarations remain
explicit and readable; the guardrails are checker metadata, not generated
public code.

The native stream contract types sync `NetworkStream.start_tls()` as returning
a new `NetworkStream`, and async `AsyncNetworkStream.start_tls()` as an
awaitable resolving to a new `AsyncNetworkStream`. Both expose
`is_upgraded: bool`. `Client` and `AsyncClient` expose typed `is_closed` and
`cookies`; `AsyncClient.close()` is synchronous while `aclose()` is awaitable.
The reviewed `Verify` alias accepts only `bool`, a CA path `str`, an
`ssl.SSLContext`, or a concrete `list[bytes]` of DER certificates.

The native aliases distinguish `SyncBody` from `AsyncBody`: synchronous
clients and helpers accept buffered bytes-like/text values or synchronous
iterables, while `AsyncClient` additionally accepts lazy async iterables.
Compatibility-facade types remain versioned and should not be substituted for
native `eggfetch` types.

## Module Map

| Module | Purpose |
|--------|---------|
| `lib.rs` | Module registration + top-level functions (`get`, `post`, etc.) |
| `client.rs` | `Client` — sync adapter with persistent runtime |
| `async_client.rs` | `AsyncClient` — async adapter targeting asyncio |
| `response.rs` | `PyResponse` — buffered response surface and private lazy buffered iterators |
| `headers.rs` | `PyHeaders` — header wrapper |
| `errors.rs` | Exception hierarchy |
| `auth.rs` | `BasicAuth`, `BearerAuth`, `NoAuth` |
| `cookies.rs` | Cookie handling |
| `proxy.rs` | Internal proxy-override parsing (`ProxyOverride`, `env_proxy_urls()`); the public `Proxy` class lives in the compat facade, not here |
| `retry.rs` | `Retry` configuration class |
| `timeout.rs` | Timeout configuration |
| `tls.rs` | TLS configuration (`verify`, `cert` kwargs) |
| `multipart.rs` | `File` wrapper for multipart uploads |
| `streaming.rs` | `StreamingResponse` + sync/async bytes/text/lines/raw-bytes iterators |
| `conversion.rs` | Python↔Rust type conversion (shared by sync/async) |
| `request_preparation.rs` | Shared client configuration and method/URL/header/body/auth/proxy/retry normalization |
| `limits.rs` | `PyLimits` — pool concurrency limits |
| `extensions.rs` | Request extension extraction (`target`, `sni_hostname`, `trace` only; core `resolved_target` has no Python `extensions=` path and unknown keys are ignored) |
| `network_stream.rs` | `PyNetworkStream` / `PyAsyncNetworkStream` upgrade wrappers |
| `trace_bridge.rs` | `PyTraceObserver` — sync trace-callback bridge |

## Sync Adapter

Each `PyClient` owns a tokio runtime and an `eggfetch_core::Client`.

### Request Flow

1. Normalize Python arguments through `request_preparation.rs` (including lazy
   sync-body handling and sync rejection of async-only bodies).
2. Release the GIL via `py.detach`.
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

Sync streaming producers use a bounded runtime-neutral bridge: producer-side
backpressure awaits capacity without blocking a Tokio worker, while the sync
iterator waits with the GIL detached. Rust-side byte payloads remain `Bytes`
through queueing and chunk splitting; conversion to Python-owned bytes happens
only at the iterator boundary. Buffered `PyResponse` objects retain raw
content and initialize their private decoded-text cache only when `.text`,
`.json()`, or text iteration first needs it.

Core response construction extracts status, wire metadata, charset, and
`Set-Cookie` values before moving the ordinary core `HeaderMap` into
`PyHeaders`; it does not clone the complete map. Buffered `iter_bytes()`,
`iter_text()`, and `iter_lines()` keep a reference to the response plus a
private cursor and create one Python value per `__next__`. Text chunks count
Unicode scalars and lines retain `str.lines()` CRLF/trailing-line semantics,
including preserving a final standalone carriage return rather than treating
it as a CRLF delimiter.
Async `StreamingResponse.aread()` keeps the collected Rust `Bytes` for the
response cache and creates the final Python `bytes` in the GIL bridge without
an intermediate full-body `Vec`.

### Top-Level Helpers

`eggfetch.get(...)`, `eggfetch.post(...)`, etc. create a short-lived runtime and client per call. The `Client` class owns a persistent runtime for connection reuse.

## Async Adapter

`AsyncClient` targets asyncio via `pyo3-async-runtimes`. It accepts both lazy
synchronous iterables and lazy `AsyncIterable[bytes | str]` request bodies;
the latter are advanced only when the Rust body is polled, so transport
backpressure is preserved and iterator errors/cancellation propagate through
the request future.

### Request Flow

Each request method uses `future_into_py` to convert the Rust future into a Python awaitable. The async block buffers the response body before returning.

### Shared preparation boundary

`request_preparation.rs` is the single GIL-held normalization boundary for
both adapters. `prepare_client_config()` owns constructor policy conversion
(TLS, protocol flags, limits, defaults, timeout, redirects, cookies, auth,
proxies and `NO_PROXY`, retries, and transport options); the adapter then
chooses its runtime ownership and calls `apply_client_config()`. The sync
adapter creates and owns a Tokio runtime, while `AsyncClient` uses the
`pyo3-async-runtimes` asyncio bridge. Runtime ownership is not hidden in the
shared type.

`prepare_request()` performs the corresponding request-local normalization
once for sync and async dispatch. It classifies `content=` as buffered,
synchronous iterable, or asynchronous iterable and stores the iterator it
obtained, avoiding a second probe of one-shot objects. Async-only bodies are
rejected before dispatch by sync APIs and are supported lazily by
`AsyncClient`; each `__anext__()` is awaited only when core polls the request
body. Producer exceptions become `BodyError`, `StopAsyncIteration` is EOF,
and cancelling the request drops the in-flight producer future.

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

Source of truth: `crates/eggfetch-python/src/errors.rs`.

```
EggfetchError
├── RequestError
│   ├── InvalidUrl
│   ├── TimeoutException
│   │   ├── ConnectTimeout
│   │   ├── ReadTimeout
│   │   ├── WriteTimeout
│   │   └── PoolTimeout
│   ├── NetworkError
│   ├── ProtocolError
│   ├── BodyError
│   ├── TooManyRedirects
│   ├── DecompressionError
│   ├── UnsupportedContentEncoding
│   ├── ProxyError
│   │   ├── ProxyConnectError
│   │   └── ProxyAuthError
│   ├── BodyNotReplayableForRetry
│   ├── RetryBudgetExhausted
│   ├── RetryNotConfigured
│   ├── Http2Error
│   │   ├── Http2GoAway
│   │   ├── Http2StreamReset
│   │   └── Http2FlowControlError
│   └── H3Error
│       ├── H3ConnectError
│       └── H3ProtocolError
├── HTTPStatusError
├── UnsupportedKwarg
├── StreamConsumed
├── StreamClosed
└── ResponseNotRead
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
| `verify` | bool/str/`SSLContext`/list[bytes] | TLS verification (bool, CA path, `ssl.SSLContext`, or DER list) |
| `cert` | str/tuple | Client certificate |
| `http2` | bool | HTTP/2 negotiation |
| `http3` | bool | HTTP/3 (experimental) |
| `decompress` | bool | Automatic decompression |

Body kwargs (`content`, `data`, `json`) are mutually exclusive. `files` may combine with `data` but conflicts with `content` and `json`.

## HTTPX Compatibility Facades

Two versioned, independent facades share the single Rust engine:

- `eggfetch.compat.httpx` — HTTPX 0.28.1, Stage C qualified on the exact
  executable SHA recorded in `plans/httpx-parity-correction-status.md`; prior
  bindings are historical after qualification-sensitive changes.
- `eggfetch.compat.httpx2` — httpx2 2.12.0 sibling (independently Stage C
  qualified on the same frozen SHA; the prior `639bf186...` binding is
  historical after the post-freeze HTTP/3 diagnostics audit;
  profile in `compat/httpx2/2.12.0/profile.toml`). Core facade adds `FunctionAuth`,
  `Origin` + `URL.origin`, `QUERY`, `Headers` merge operators, truststore
  OS-trust default, IPv6 CIDR `NO_PROXY` fix, decoder/multipart/WSGI
  hardening, and status aliases; SSE (`EventSource` over streamed
  responses) and optional WebSocket (wsproto framing over the core 101
  `network_stream`; handshake via the normal pipeline) are implemented in
  the same facade. Shared helpers are reused where semantics are
  identical; profile-specific behavior stays behind explicit boundaries.
  Importing one facade never mutates the other.
- `compat/httpx/1.0-preview/` — original HTTPX 1.0 reconnaissance only;
  no implementation promise until an RC/stable trigger.

The 0.28.1 facade is Stage C qualified for the documented Python 3.10+
asyncio-supported surface on the current SHA above. HTTP/3 remains separately
experimental; its retained label and blockers do not weaken or extend either
compatibility profile.

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
| `eggfetch/_ssl_context.py` | Neutral SSLContext snapshot, classification, construction fingerprint |
| `eggfetch/compat/httpx/_ssl_context.py` | Backward-compatible shim to the neutral SSL interop module |
| `eggfetch/compat/httpx/_diagnostics.py` | `diagnostics_summary` — redacted client diagnostics |

### httpx2 Facade Module Structure

`eggfetch/compat/httpx2/` subclasses/re-exports the 0.28.1 facade — no
~97 KB client fork. Shared semantics are re-exported unchanged
(`_asgi/_cookies/_mock/_request/_response/_stream/_transports/_wsgi` from
`httpx`); profile-specific modules subclass or wrap:

| Module | Purpose |
|--------|---------|
| `httpx2/__init__.py` | Public 2.12.0 surface + truststore `create_ssl_context` |
| `httpx2/_auth.py` | Re-exports shared auth + `FunctionAuth` callable adapter |
| `httpx2/_urls.py` | `URL.origin` + frozen `Origin` (scheme/IDNA/ports/IPv6) |
| `httpx2/_client.py` | Subclasses base clients + `query()`/`sse()`/`websocket()` |
| `httpx2/_api.py` | Top-level helpers incl. `query` + `websocket` |
| `httpx2/_headers.py` | Subclassed `Headers` with `\|`/`\|=` merge operators |
| `httpx2/_config.py` | `Timeout` (message names httpx2) + SSE/WS defaults; re-exports `Limits` from the 0.28.1 facade |
| `httpx2/_exceptions.py` | Re-exported hierarchy + `HTTPXDeprecationWarning` |
| `httpx2/_status_codes.py` | RFC 9110 canonical `codes` + `DeprecationWarning` aliases |
| `httpx2/_alias.py` | Explicit opt-in `alias_httpx()` only |
| `httpx2/_sse.py` | `EventSource`/`ServerSentEvent` framing over streamed responses |
| `httpx2/websockets/` | Optional WebSocket (wsproto framing over 101 `network_stream`) |

Behavior hardening owned by core/facade boundaries: IPv6 CIDR `NO_PROXY`
(versioned parser), chained-decoder cap (native 4 vs reference 5,
intentionally stricter), multipart `try_header` validation, WSGI framing.
Parity cases `H2X-API/META/AUTH/TLS/PROXY/COMP/MP/WSGI` (core) plus
`H2X-SSE-001/002`, `H2X-WS-001..003` (streaming) in
`compat/httpx2/2.12.0/parity-cases.toml`; differential tests
`test_httpx2_api_parity.py` + `test_httpx2_behavior.py` (core),
`test_httpx2_sse.py` + `test_httpx2_websocket.py` (streaming).
Packaging: SSE needs no extra dependency; WS needs `wsproto` (+ `anyio`
for async sessions) only when used — `Client.websocket`/`connect_ws`
raise a clear `ImportError` otherwise, so base installs stay lean
(see `docs/architecture/dependency-policy.md`).

### SSLContext Translation

The Python `ssl.SSLContext` API exposes a large surface (custom ciphers,
ALPN, session tickets, custom verify flags, etc.) that rustls cannot
represent directly.  The compat layer classifies every context before
dispatch through three deterministic gates:

1. **Construction fingerprint**: a SHA-256 over the extractable public
   state (`verify_mode`, `check_hostname`, `min_version`, `max_version`,
   per-cert DER hashes, cipher list, options flags, and a class-name
   sentinel).  A helper-created context whose live state matches the
   stored fingerprint reuses the helper-recorded metadata.
2. **Mutation detection**: a context whose live state diverges from
   the stored fingerprint (e.g. after `load_verify_locations`,
   `set_minimum_version`, `set_ciphers`, `set_alpn_protocols`) loses
   its stored metadata and is reclassified from the live snapshot.
   This means the user's post-construction edits always win, never
   the helper's original intent.
3. **Representability gate**: external (caller-created) contexts are
   classified into `EXACTLY_REPRESENTABLE`,
   `REPRESENTABLE_WITH_DEFAULTS`, or `UNREPRESENTABLE`.  Custom cipher
   lists, custom ALPN, client certificates without path provenance,
   and explicit version bounds outside TLS 1.2/1.3 cause
   `UNREPRESENTABLE` → `TypeError` before dispatch.  Two CA stores
   with identical cardinalities but different contents produce
   different `verify` kwargs (no CA-count heuristic).

The registry distinguishes **passthrough** contexts (caller-supplied
via `verify=<SSLContext>`) from **helper-created** contexts
(`create_ssl_context()`).  Passthrough contexts are not assigned a
cert path or `verify` kwarg; helper contexts are.  This prevents a
caller-supplied mTLS context from being silently downgraded to no
client auth, and prevents a caller-supplied `verify=False` from
inheriting the helper's default trust.

Rust consumes this state through exactly one private helper,
`eggfetch._ssl_context._export_ssl_context_state`. Its bounded mapping carries
schema version 1, classification, observable verification/version/CA state,
and validated helper provenance. Unknown versions, missing or wrongly typed
fields, and unrepresentable classifications fail before I/O. The mapping is
internal architecture only and is not included in `eggfetch.__all__` or the
typing surface.

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
  the native proxy string/authentication path. `Proxy(headers=...)` is
  forwarded through the native proxy-leg header channel. `Proxy(auth=...)`
  is sent as `Proxy-Authorization` on the proxy leg.
- **Proxy header redaction**: `Proxy.__repr__` redacts the values of
  `authorization`, `proxy-authorization`, `cookie`, and `set-cookie` to
  `<redacted>` so credentials do not appear in diagnostic dumps.  The raw
  values remain available to protocol code through `Proxy.headers` and
  to engine code through the native API.  The `Headers` type's
  `Debug`/`__str__` redacts the same values; the raw values remain available
  to protocol code.
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
- **Typed extensions plumbing** (corrective 02, refined 06): `request.extensions={"target": ..., "sni_hostname": ..., "trace": <callable>}` is extracted by `extract_native_extensions` into `TransportHints` before dispatch.  Python callables are wrapped by `PyTraceObserver` (a `TraceObserver` implementation in `crates/eggfetch-python/src/trace_bridge.rs`) so callables never enter `eggfetch-core`.  Both sync `Client` and `AsyncClient` detect coroutine callbacks via `inspect.iscoroutinefunction(callback)`; the observer records a `NotAwaited` error eagerly in its `CallbackErrorSlot` and short-circuits `on_event` with `Abort` so the transport stops issuing events and   never produces a discarded coroutine.  The slot is checked after dispatch on every path (success or failure) and the original callback error propagates as a `TypeError` for not-awaited coroutines or as the original exception for raised ones.  `resolved_target` is core-only and cannot be set from Python `extensions=` (same-origin redirects preserve it, cross-origin hops fail closed; proxy/UDS/H3 combinations are rejected); unknown extension keys are ignored at this layer. Once installed, hints ride the core retry/redirect pipeline.  Corrective 06 unifies the extension parser across all four sync/async buffered/streaming paths so `AsyncClient.stream()` no longer uses a hand-rolled `target`/`sni_hostname` parser and `AsyncClient.request()` (buffered) forwards the full extension dict including `trace`.
- **Wire metadata on response** (corrective 02): Native `Response.extensions` and `StreamingResponse.extensions` expose `{http_version, reason_phrase, network_stream}` snapshots that mirror the core `Response` wire state.  `Response.reason_phrase` prefers `wire_reason_phrase()` over the canonical lookup table so unusual status codes do not silently collapse to empty strings.
- **101 Switching Protocols and `network_stream`** (corrective 03, refined 06): When the core captures a Hyper upgrade future, `Response.extensions["network_stream"]` exposes a live `PyNetworkStream` (sync, GIL-released) or `PyAsyncNetworkStream` (async, awaits on the asyncio loop) wrapper. Wrapper selection in Corrective 06 follows the **caller's API mode** (sync vs async), not the buffered-vs-streaming response path: sync `Client.stream()` 101 responses expose the sync wrapper, and async `AsyncClient.request()` buffered 101 responses expose the async wrapper. The two are stored behind an `EitherNetworkStream` enum so the Python-facing attribute delivers the correct wrapper type. Ordinary buffered responses set `extensions["network_stream"] = None` because the connection has been returned to the pool; internal HTTPS CONNECT tunnels are also classified as `None` because the canonical access path is the body iterator. `start_tls(ssl_context=..., server_hostname=..., timeout=...)` is rejected for Hyper-opaque `Adapter` variants and for streams that are already TLS-wrapped; only `Tcp`-variant streams use the Corrective 01 safe TLS translation (and Corrective 06 makes that translation genuinely fail-closed for unrepresentable SSLContext state). The sync wrapper carries an explicit `tokio::runtime::Handle` plus optional `RuntimeLease` so it can drive IO without relying on an ambient runtime; cloning a `PyNetworkStream` shares the same underlying `Arc<Mutex<>>`. Sync read/write hold the lock across the blocking IO, so concurrent clones serialize and a slow peer stalls all clones; for independent concurrent IO, do not clone — open separate streams. Leading data after 101 headers is preserved inside Hyper's rewind buffer and returned by the first reads from the upgraded stream.
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
scheme-less HTTP proxy normalization, and `NO_PROXY` URL-pattern behavior.
Ordinary bare domains match the bare host and subdomains only at a domain-label
boundary; leading-dot domains exclude the bare host; localhost and IP literals
are exact; and an explicit host port matches only an explicit normalized target
port. Scheme-qualified entries retain their scheme and optional URL-pattern
port, while CIDR-looking values remain exact host text rather than native
subnet matching. Bare unbracketed IPv6 literals follow the pinned HTTPX
environment form; bracketed IPv6 and IPv6 prefix-looking values are rejected
before dispatch. Native Rust parsing retains its richer bracketed-IPv6 and
CIDR behavior. UDS uses the
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
while origin TLS after CONNECT uses the origin hostname.  Proxy and origin
TLS configurations are kept independent: the proxy endpoint is verified
using `Proxy(ssl_context=...)` (or rustls' default trust when not supplied),
never reusing the origin's `verify=` setting.  `Proxy(headers=...)` is
forwarded through the native proxy-leg header channel; sensitive header
values are redacted in diagnostic surfaces (`__repr__`, `__str__`) but
remain observable to protocol code through `Proxy.headers`.
