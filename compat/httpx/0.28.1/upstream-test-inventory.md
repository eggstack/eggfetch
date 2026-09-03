# HTTPX 0.28.1 Upstream Test Inventory

**Phase 6 — Final Qualification Inventory**

| Field | Value |
|-------|-------|
| Reference | httpx==0.28.1 |
| Source | https://github.com/encode/httpx |
| Commit | 0.28.1 (tag) |
| License | BSD-3-Clause |
| Generated | 2026-07-23 (original) |
| Rebaselined | 2026-08-07 |
| Rebaseline SHA | f9eb1a455907d43210886b7b047d18bde8716652 |
| Qualified | 2026-08-10 (historical; superseded) |
| Qualification SHA | 40beeec09f3e88db8901f39388da665c47ab84f6 (historical; current SHA in `profile.toml`) |

---

## 1. Test Classification

HTTPX 0.28.1's test suite lives in `tests/` within the source distribution. Tests are
organized by module and cover internal implementation details alongside public contracts.

### 1.1 Public Contract Tests

These exercise documented public API behavior. They can run unchanged or with minimal
fixture adaptation against eggfetch's compatibility layer.

| Category | HTTPX Test Files | Applicability |
|----------|------------------|---------------|
| Client construction | `tests/test_client.py` | High — constructor signatures, context managers |
| Request building | `tests/test_content.py`, `tests/test_multipart.py` | High — body types, encoding |
| Response handling | `tests/test_response.py` | High — status, headers, streaming |
| Transports | `tests/test_transport.py` | High — MockTransport, custom transports |
| Auth | `tests/auth/` | High — BasicAuth, DigestAuth flows |
| Redirects | `tests/test_redirects.py` | High — status-specific method handling |
| Cookies | `tests/test_cookies.py` | High — jar behavior, domain matching |
| Timeouts | `tests/test_config.py` | High — Timeout, Limits objects |
| Errors | `tests/test_exceptions.py` | High — exception hierarchy |
| URL handling | `tests/test_url.py` | Medium — URL construction, query params |
| Headers | `tests/test_headers.py` | Medium — case-insensitive, multi-value |

### 1.2 httpcore-Internal Tests

Tests coupled to httpcore internals. **Not applicable** to eggfetch (which uses
a Rust backend via PyO3).

| File | Reason Excluded |
|------|-----------------|
| `tests/test_main.py` | httpcore connection pool details |
| `tests/sync/test_core.py` | httpcore sync transport internals |
| `tests/async/test_core.py` | httpcore async transport internals |
| `tests/decoders/` | httpcore content decoders |
| `tests/test_content.py` (partial) | httpcore stream body encoding |

### 1.3 Packaging/Internal Tests

Implementation-specific tests. **Not applicable**.

| File | Reason Excluded |
|------|-----------------|
| `tests/test_utils.py` | Internal utility functions |
| `tests/test_content.py` (partial) | Internal `_content.py` module |
| `tests/models/` | Internal model details |

### 1.4 Private Behavior Tests

Intentionally excluded from the compatibility contract.

| Behavior | Reason Excluded |
|----------|-----------------|
| Trio/AnyIO backend tests | eggfetch uses asyncio only |
| HTTP/2 transport internals | Protocol-level, not API contract |
| Connection pooling details | httpcore responsibility |
| SSL/TLS verification internals | Transport implementation detail |

---

## 2. Key Public Contract Areas

### 2.1 Client Construction and Configuration

**HTTPX API Surface:**
- `Client(...)` / `AsyncClient(...)` constructors
- Parameters: `auth`, `params`, `headers`, `cookies`, `proxy`, `proxies`, `verify`, `cert`, `timeout`, `limits`, `http2`, `event_hooks`, `default_encoding`, `follow_redirects`, `base_url`, `transport`, `async_transport`, `mounts`, `trust_env`

**HTTPX Tests:**
- `tests/test_client.py::test_client construction with parameters`
- `tests/test_client.py::test_client defaults`
- `tests/test_client.py::test_context manager`
- `tests/test_client.py::test_async_client construction`

**eggfetch Coverage:**
- `test_client.py::TestClientConstructor` — constructor defaults, custom headers/cookies/params/timeout/auth/base_url
- `test_client.py::TestClientContextManager` — context manager, close
- `test_client.py::TestAsyncClient` — async context manager, close
- `test_httpx_required.py::TestBasicClientShape` — method existence, context managers

### 2.2 Request Building

**HTTPX API Surface:**
- `Request(method, url, *, content, data, json, files, stream, params, headers, cookies, extensions, timeout, follow_redirects)`
- Auto-headers: Host, Content-Length, Content-Type, Transfer-Encoding
- Body mutual exclusion (content vs json vs data vs stream)

**HTTPX Tests:**
- `tests/test_content.py::test_request body types`
- `tests/test_multipart.py::test_multipart encoding`
- `tests/test_request.py::test_request construction`

**eggfetch Coverage:**
- `test_request.py::TestRequestConstruction` — method, URL, empty body
- `test_request.py::TestBodyMutualExclusion` — content/json/data exclusivity
- `test_request.py::TestAutoHeaders` — Host, Content-Length, Content-Type, Transfer-Encoding
- `test_request.py::TestParamsMerging` — QueryParams object, dict
- `test_request.py::TestRead` — content reading, stream consumption
- `test_request_streaming.py` — stream construction, transfer-encoding

### 2.3 Response Handling

**HTTPX API Surface:**
- `Response(status_code, *, headers, stream, content, text, json, html, request, history, elapsed, extensions, encoding)`
- Properties: `status_code`, `headers`, `content`, `text`, `json()`, `links`, `history`, `elapsed`, `encoding`, `charset_encoding`, `is_stream_consumed`, `stream`, `http_version`, `extensions`, `request`
- Iterators: `iter_bytes()`, `iter_text()`, `iter_lines()`, `iter_raw()`
- Async iterators: `aiter_bytes()`, `aiter_text()`, `aiter_lines()`, `aiter_raw()`
- `raise_for_status()` — returns self on success, raises `HTTPStatusError` on error
- Status predicates: `is_success`, `is_redirect`, `is_client_error`, `is_server_error`, `is_error`, `is_informational`

**HTTPX Tests:**
- `tests/test_response.py::test_response construction`
- `tests/test_response.py::test_response streaming`
- `tests/test_response.py::test_raise_for_status`
- `tests/test_response.py::test_response links`

**eggfetch Coverage:**
- `test_response.py::TestResponseConstruction` — status, content, text, json, html, conflicting sources
- `test_response.py::TestStatusPredicates` — is_success, is_redirect, is_client_error, is_server_error, is_error, is_informational
- `test_response.py::TestRaiseForStatus` — success returns self, 4xx/5xx raise, exception has response
- `test_response.py::TestHistory` — default empty, with history
- `test_response.py::TestLinks` — Link header parsing
- `test_response.py::TestIterators` — iter_bytes, iter_text, iter_lines
- `test_response.py::TestEncoding` — encoding setter, charset_encoding
- `test_response.py::TestElapsed` — default zero
- `test_response_streaming.py` — sync/async streaming, iter_raw, aiter_raw, read, close

### 2.4 Transport and Mounts

**HTTPX API Surface:**
- `BaseTransport` / `AsyncBaseTransport` — abstract base classes
- `HTTPTransport(...)` / `AsyncHTTPTransport(...)` — concrete implementations
- `MockTransport(handler)` — deterministic testing
- `Client(mounts={...})` — per-host transport routing

**HTTPX Tests:**
- `tests/test_transport.py::test_transport base class`
- `tests/test_transport.py::test_mock_transport`
- `tests/test_client.py::test_transport mounts`

**eggfetch Coverage:**
- `test_transports.py::TestBaseTransport` — handle_request not implemented, close, context manager
- `test_transports.py::TestAsyncBaseTransport` — async variants
- `test_transports.py::TestHTTPTransport` — constructor defaults, custom params
- `test_transports.py::TestAsyncHTTPTransport` — async constructor, context manager
- `test_transports.py::TestTransportDispatch` — custom transport overrides, receives full request
- `test_mock_transport.py::TestMockTransportSync` — basic handler, request/response, exceptions, status codes, streaming
- `test_mock_transport.py::TestMockTransportAsync` — async handler, sync-in-async
- `test_mounts.py` — comprehensive mount routing tests (scheme, host, port, path, priority, edge cases)

### 2.5 Authentication

**HTTPX API Surface:**
- `Auth` — base class with `auth_flow(request)` generator
- `BasicAuth(username, password, encoding)` — Basic authentication
- `DigestAuth(username, password)` — Digest authentication
- `NetRCAuth(filename)` — netrc-based authentication
- Auth tuple shorthand: `auth=("user", "pass")`
- Per-request auth disabling: `auth=None`

**HTTPX Tests:**
- `tests/auth/test_basic.py`
- `tests/auth/test_digest.py`
- `tests/auth/test_netrc.py`
- `tests/auth/test_auth.py`

**eggfetch Coverage:**
- `test_auth.py::TestAuth` — base class not implemented
- `test_auth.py::TestBasicAuth` — credentials, encoding, auth_flow, single yield, repr, empty credentials
- `test_auth.py::TestDigestAuth` — credentials, challenge handling, stale nonce, SHA-256, qop variants, nonce count, opaque, URI with query, qop negotiation, cross-origin redirect, body hashing
- `test_auth.py::TestNetRCAuth` — parsing, default entry, missing file, auth flow, permissions, comments, multiple machines, empty file

### 2.6 Redirects

**HTTPX API Surface:**
- Status-specific method changes: 301/302/303 → GET, 307/308 → preserve method
- Cross-origin header stripping (Authorization, Cookie)
- `follow_redirects=True/False` (default: False in both HTTPX 0.28.1 and eggfetch)
- `max_redirects` limit
- Redirect history on response

**HTTPX Tests:**
- `tests/test_redirects.py::test redirect status codes`
- `tests/test_redirects.py::test redirect method changes`
- `tests/test_redirects.py::test cross-origin header stripping`
- `tests/test_redirects.py::test max redirects`

**eggfetch Coverage:**
- `test_httpx_required.py::TestRedirectBehavior` — no follow by default, follow with flag
- `test_behavior_cases.py::REDIRECT-001/002` — 302 status match between httpx and eggfetch
- `test_httpx_required.py` — redirect endpoints in server fixtures

### 2.7 Cookies

**HTTPX API Surface:**
- `Cookies()` — jar with set/get/delete/clear/update
- Domain/path matching
- Secure flag handling
- `extract_cookies(request)` / `set_cookie_header(response)`
- Request-level cookies merge with client cookies

**HTTPX Tests:**
- `tests/test_cookies.py::test_cookie jar behavior`
- `tests/test_cookies.py::test_domain matching`
- `tests/test_cookies.py::test_cookie merging`

**eggfetch Coverage:**
- `test_cookies.py::TestCookiesConstruction` — empty, from dict, from list, from Cookies, invalid type
- `test_cookies.py::TestCookiesMutation` — set, set with domain/path, get, delete, clear, update
- `test_cookies.py::TestCookiesDunder` — setitem, getitem, delitem, contains, len, iter, bool, items, keys, values, repr
- `test_cookies.py::TestCookiesSetDefault` — setdefault existing/missing/none
- `test_cookies.py::TestCookiesFromRequest` — extract_cookies, set_cookie_header
- `test_merge_matrix.py::TestCookiesMerging` — client+request cookie merge semantics

### 2.8 Timeouts

**HTTPX API Surface:**
- `Timeout(timeout, *, connect, read, write, pool)` — per-phase timeouts
- `Limits(max_connections, max_keepalive_connections, keepalive_expiry)`
- Scalar timeout shorthand: `timeout=5.0`

**HTTPX Tests:**
- `tests/test_config.py::test_timeout construction`
- `tests/test_config.py::test_limits construction`

**eggfetch Coverage:**
- `test_config_objects.py::TestTimeoutConstruction` — scalar, per-phase, default, integer
- `test_config_objects.py::TestTimeoutProperties` — as_dict
- `test_config_objects.py::TestTimeoutValidation` — negative, non-number
- `test_config_objects.py::TestTimeoutEq` — equality
- `test_config_objects.py::TestTimeoutRepr` — repr
- `test_config_objects.py::TestTimeoutCopy` — copy, deepcopy
- `test_config_objects.py::TestLimitsConstruction` — defaults, custom
- `test_config_objects.py::TestLimitsEq` — equality
- `test_config_objects.py::TestLimitsRepr` — repr
- `test_merge_matrix.py::TestTimeoutOverride` — client timeout used

### 2.9 Error Handling

**HTTPX API Surface:**
- Exception hierarchy:
  - `HTTPError` (base)
    - `HTTPStatusError` (raise_for_status)
    - `RequestError`
      - `TransportError`
        - `TimeoutException` → `ConnectTimeout`, `ReadTimeout`, `WriteTimeout`, `PoolTimeout`
        - `NetworkError` → `ConnectError`, `ReadError`, `WriteError`, `CloseError`
        - `ProtocolError` → `LocalProtocolError`, `RemoteProtocolError`
        - `ProxyError`
        - `UnsupportedProtocol`
      - `DecodingError`
      - `TooManyRedirects`
  - `StreamError` (separate branch) → `RequestNotRead`, `ResponseNotRead`, `StreamClosed`, `StreamConsumed`
  - `InvalidURL` (standalone)
  - `CookieConflict` (standalone)

**HTTPX Tests:**
- `tests/test_exceptions.py::test exception hierarchy`
- `tests/test_exceptions.py::test exception constructors`

**eggfetch Coverage:**
- `test_exceptions.py::TestExceptionHierarchy` — full MRO verification for all exception types
- `test_exceptions.py::TestStreamErrorHierarchy` — StreamError branch
- `test_exceptions.py::TestStandaloneExceptions` — InvalidURL, CookieConflict
- `test_exceptions.py::TestExceptionConstructors` — constructor signatures
- `test_exceptions.py::TestRaiseForStatus` — success, informational, 4xx, 5xx, redirect
- `test_httpx_required.py::TestErrorHandling` — 4xx/5xx raises, base exception is EggfetchError

### 2.10 Event Hooks

**HTTPX API Surface:**
- `Client(event_hooks={"request": [...], "response": [...]})`
- Request hooks run before auth
- Response hooks run after dispatch
- Hook errors propagate and close streams

**HTTPX Tests:**
- `tests/test_client.py::test event hooks`
- `tests/test_client.py::test hook ordering`

**eggfetch Coverage:**
- `test_hooks.py::TestSyncHooks` — request hook, response hook, ordering, error handling, modification, auth ordering
- `test_hooks.py::TestAsyncHooks` — async request/response hooks, sync-in-async, mixed hooks

### 2.11 URL and Query Parameters

**HTTPX API Surface:**
- `URL(url)` — constructor from string, URL, or components
- `QueryParams(params)` — from dict, list of tuples, string
- `Client(base_url=...)` — base URL joining

**HTTPX Tests:**
- `tests/test_url.py::test URL construction`
- `tests/test_url.py::test query parameters`

**eggfetch Coverage:**
- `test_url_query.py` — URL construction, query parameter handling
- `test_merge_matrix.py::TestBaseUrlMerging` — relative URL joining, absolute URL wins, empty base_url
- `test_merge_matrix.py::TestParamsMerging` — client params, request params, override, add

### 2.12 Status Codes

**HTTPX API Surface:**
- `httpx.codes` — named constants (OK=200, NOT_FOUND=404, etc.)

**HTTPX Tests:**
- `tests/test_status_codes.py`

**eggfetch Coverage:**
- `test_config_objects.py::TestStatusCodes` — OK, NOT_FOUND, INTERNAL_SERVER_ERROR, BAD_REQUEST, UNAUTHORIZED, FORBIDDEN, MOVED_PERMANENTLY, FOUND, SERVICE_UNAVAILABLE, TOO_MANY_REQUESTS, BAD_GATEWAY, GATEWAY_TIMEOUT, CREATED, NO_CONTENT, CONTINUE

### 2.13 WSGI/ASGI Transports

**HTTPX API Surface:**
- `ASGITransport(app)` — in-process ASGI testing
- `WSGITransport(app)` — in-process WSGI testing

**HTTPX Tests:**
- `tests/test_asgi.py`
- `tests/test_wsgi.py`

**eggfetch Coverage:**
- `test_asgi.py` — ASGI transport tests
- `test_wsgi.py` — WSGI transport tests

**Note:** These are marked as `required-later` in the allowed-differences profile. eggfetch
implements them as thin adapters for test compatibility, not as primary functionality.

---

## 3. Coverage Gap Summary

### 3.1 Fully Covered Areas

| Area | eggfetch Tests | Notes |
|------|----------------|-------|
| Client construction | test_client.py, test_httpx_required.py | All constructor parameters (Phase 3 signatures) |
| Request building | test_request.py, test_request_streaming.py | All body types, auto-headers |
| Response handling | test_response.py, test_response_streaming.py | All properties, iterators, status |
| Transport/Mounts | test_transports.py, test_mounts.py, test_mock_transport.py | Comprehensive |
| Authentication | test_auth.py | Basic, Digest, NetRC, tuple shorthand |
| Cookies | test_cookies.py, test_merge_matrix.py | Full cookie jar API |
| Timeouts | test_config_objects.py | Timeout, Limits objects |
| Error handling | test_exceptions.py | Full exception hierarchy (Phase 2 stream exceptions) |
| Event hooks | test_hooks.py | Request/response hooks, ordering |
| Status codes | test_config_objects.py | Named constants |
| Object contracts | Phase 2 | Headers MutableMapping, QueryParams Mapping, URL.raw, codes IntEnum |
| Signature alignment | Phase 3 | Top-level helpers, Client/AsyncClient constructors, transport signatures |
| Stream types | Phase 3 | SyncByteStream, AsyncByteStream, custom streams, decoded/raw response lifecycle |
| Direct transport | Phase 4 | local_address, socket_options, UDS (Unix domain sockets) |
| Pool isolation | Phase 4 | Different local addresses, socket options, UDS vs TCP |
| SOCKS proxy | Phase 5 | HTTP/HTTPS through SOCKS5, auth, DNS, NO_PROXY bypass |
| Timeout/cancellation | Phase 4 | End-to-end timeout, cancellation, resource release |

### 3.2 Intentionally Excluded Areas

| Area | Reason |
|------|--------|
| Trio/AnyIO backends | Deferred to Stage D; eggfetch uses asyncio only |
| Python 3.8/3.9 support | Deferred; eggfetch requires Python 3.10+ |
| Private HTTPX modules | Excluded from contract (_transports, _content, _models, etc.) |
| SSL context on Proxy | Intentional security boundary (TLS handled by Rust engine) |
| ALL_PROXY/lowercase env vars | Supported by the facade with scheme-aware fallback and runtime precedence; native Rust configuration remains explicit |

### 3.3 Not Applicable Areas

| Area | Reason |
|------|--------|
| HTTP/2 transport internals | Protocol detail, not API contract |
| Connection pooling details | httpcore responsibility |
| SSL/TLS verification internals | Transport implementation detail |

---

## 4. Test Source References

All HTTPX tests referenced are from the httpx 0.28.1 source distribution:
- Repository: https://github.com/encode/httpx
- Tag: 0.28.1
- License: BSD-3-Clause

The eggfetch compatibility tests are located at:
- `crates/eggfetch-python/tests/compat/`

---

## 5. Phase 1 Rebaseline Notes (2026-08-07)

This inventory was refreshed during the Phase 1 contract rebaseline. Key corrections:

1. **Redirect default**: HTTPX 0.28.1 defaults to `follow_redirects=False`, same as eggfetch. The earlier claim that "HTTPX follows redirects by default" was incorrect for version 0.28.1.
2. **Proxy environment**: the HTTPX facade selects `HTTP_PROXY`/`HTTPS_PROXY` by request scheme, uses `ALL_PROXY` fallback, and honors lowercase forms plus `NO_PROXY` when `trust_env=True`.
3. **SOCKS proxy**: HTTPX 0.28.1 exposes SOCKS proxy support as an optional public feature (`httpx[socks]`). eggfetch supports SOCKS5 (`socks5://` and `socks5h://`) with the pinned authentication method and address-type behavior, route pooling, origin TLS, and NO_PROXY bypass. Native Rust configuration retains its explicit local-versus-remote DNS distinction.
4. **SSL context**: HTTPX `Proxy(..., ssl_context=...)` is a constructor parameter. eggfetch `Proxy(ssl_context=...)` is translated to a native `TlsConfig` for the proxy endpoint TLS handshake, separate from origin TLS config (resolved in Phase 05; see `resolved-differences.toml`). Arbitrary Python ssl_context objects unrepresentable by rustls are rejected at construction with `TypeError`.
5. **StreamError base class**: HTTPX `StreamError` inherits from `RuntimeError`; eggfetch inherits from `Exception`. Resolved in Phase 2.

The full classification of all active differences is in `allowed-differences.toml`.

## 6. Phase 6 Qualification Evidence (2026-08-10)

**Historical qualification SHA**: `40beeec09f3e88db8901f39388da665c47ab84f6` (superseded by corrective transport work)

### Routine validation
- `cargo fmt --all -- --check`: clean
- `check_lint_suppressions.sh`: OK
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: clean
- `cargo test --workspace`: all pass (493 core, 69 adverse, 12 cross-feature, 13 direct-transport, 9 h3, 21 http2, 20 http3, 94 integration, 17 pool, 35 proxy, 5 resource-stabilization, 4 retry, 12 timeout, 10 tls, 30 ffi)

### Full pinned compat suite
- Command: `EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers`
- Result: 1450 passed, 2 flaky (read timeout on lightweight test servers, pass on re-run)
- Python 3.12.3, httpx 0.28.1

### API oracle
- 76 total differences, all matched by allowlist
- 0 unexplained, 0 stale, 0 resolved-in-active, 0 requires-resolution
- Remaining 76 active allowed entries are intentional/deferred (stage-bounded)

### Downstream behavioral fixtures
- 54 passed, 8 failed
- 5 `__eggfetch_shim__` detection tests fail (expected: running with real httpx, not through shim)
- 3 SSE tests fail (httpx-sse EventSource API incompatibility)
- All actual behavioral tests pass (anthropic, starlette, mock transport, auth, hooks, sync/async client)
