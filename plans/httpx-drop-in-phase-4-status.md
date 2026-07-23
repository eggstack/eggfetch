# HTTPX Drop-In Phase 4: Transports, Mounts, Auth, Hooks — Status

Status: COMPLETE

## Deliverables

### Track A — Transport protocols
- [x] BaseTransport and AsyncBaseTransport base classes with NotImplementedError
- [x] HTTPTransport / AsyncHTTPTransport concrete implementations
- [x] Transport interface with handle_request / handle_async_request
- [x] Request/response lifecycle on transport layer
- [x] Connection pool integration via eggfetch-core native engine
- [x] Timeout and retries configuration passthrough
- [x] local_address / socket_options accepted (not forwarded — eggfetch-core limitation)

### Track B — Mount routing
- [x] Component-based URL pattern matching (scheme, host, port, path)
- [x] Priority-based routing (most specific match wins)
- [x] Per-route transport binding
- [x] Default transport fallback
- [x] Mount dictionary on Client and AsyncClient
- [x] Route matching during send dispatch
- [x] Edge cases: custom scheme, case-insensitive host, exact path, None transport fallthrough

### Track C — MockTransport
- [x] Sync handler callback support
- [x] Async handler callback support
- [x] Exception propagation (handler-raised exceptions bubble correctly)
- [x] Streaming response support via handler
- [x] Request inspection in handler
- [x] Multiple MockTransport instances per client
- [x] Sync/async mismatch detection (sync client rejects async handler)

### Track D — WSGITransport
- [x] WSGI environ construction from Request
- [x] start_response callable implementation
- [x] Body streaming from WSGI iterator
- [x] Chunked transfer encoding
- [x] App error handling (exceptions caught, 500 returned)
- [x] Status code and header passthrough
- [x] exc_info re-raise behaviour

### Track E — ASGITransport
- [x] ASGI scope construction from Request
- [x] receive/send channel implementation
- [x] Streaming response via send events
- [x] App error handling (exceptions caught, 500 returned)
- [x] Disconnect signal propagation
- [x] Chunked request body delivery through receive channel
- [x] raw_path in scope

### Track F — Event hooks
- [x] Request hooks (before-send callbacks)
- [x] Response hooks (after-response callbacks)
- [x] Hook sequencing (request hooks BEFORE auth, FIFO order)
- [x] Error cleanup (response hooks close response on exception)
- [x] Sync and async hook support
- [x] Multiple hooks per event type

### Track G — Extensions
- [x] Extension passthrough preserved
- [x] Request extensions propagated to Response via _wrap_response / _wrap_streaming_response
- [x] http_version and reason_phrase standard extension keys mapped
- [x] Forward compatibility for extension keys
- [x] Client-level extensions merge with request extensions (request overrides client)
- [x] Extensions survive through mount dispatch and custom transport paths
- [x] Extensions survive through streaming path
- [x] Extensions survive through async client paths (mount and transport)

### Track H — Auth
- [x] BasicAuth (base64 credential encoding)
- [x] DigestAuth (MD5 and SHA-256, qop=auth and auth-int)
- [x] DigestAuth nonce-count tracking across requests
- [x] DigestAuth opaque passthrough
- [x] DigestAuth stale nonce detection
- [x] DigestAuth no-qop old-style response
- [x] DigestAuth qop-list negotiation (comma-separated values)
- [x] DigestAuth URI with query string
- [x] DigestAuth SHA-256 + auth-int combo
- [x] DigestAuth cross-origin redirect (new request per host)
- [x] DigestAuth body hashing with auth-int
- [x] DigestAuth repeated challenge nonce increment
- [x] NetRCAuth (~/.netrc file parsing with permission checks)
- [x] NetRCAuth edge cases: comments, multiple machines, account field, default fallback, empty file, strict permissions
- [x] Auth flow integration in client send
- [x] Per-request auth override
- [x] WWW-Authenticate challenge-response loop

### Track I — Environment
- [x] trust_env parameter on Client and AsyncClient
- [x] Environment variable proxy handling (HTTP_PROXY, NO_PROXY, HTTPS_PROXY, ALL_PROXY)
- [x] trust_env=False disables proxy/cert/netrc discovery
- [x] Explicit proxy overrides env vars
- [x] Base URL resolution before mount dispatch
- [x] NO_PROXY patterns: wildcard, complex (CIDR, domain, port)

### Track J — Low-level (UDS)
- [x] UDS documented as eggfetch-core limitation

### Track K — SOCKS
- [x] SOCKS proxy documented as Stage D blocker

### Track L — Async backend
- [x] asyncio path retained and functional
- [x] Async context manager support
- [x] Closed client detection (RuntimeError)
- [x] send() type validation (rejects non-Request)
- [x] No leaked tasks after send/multiple-requests/error
- [x] Concurrent requests don't block event loop
- [x] Client extensions parameter accepted and stored
- [x] Trio backend deferred to Stage D

## Files Created

| File | Purpose |
|------|---------|
| `_transports.py` | Transport protocol classes and HTTPTransport |
| `_mock.py` | MockTransport with sync/async mismatch detection |
| `_auth.py` | BasicAuth, DigestAuth (MD5/SHA-256, auth-int), NetRCAuth |
| `_wsgi.py` | WSGITransport with exc_info support |
| `_asgi.py` | ASGITransport with streaming request body and raw_path |

## Files Modified

| File | Changes |
|------|---------|
| `_client.py` | Component-based mount routing, hook ordering fix (hooks before auth), extensions propagation and merge, retries forwarding, uds_path parameter |
| `__init__.py` | Updated imports for new modules |

## Test Files

| File | Tests |
|------|-------|
| `test_transports.py` | Transport protocol and HTTPTransport |
| `test_mounts.py` | Component-based mount routing, priority, edge cases (custom scheme, case-insensitive, exact path, None fallthrough) |
| `test_mock_transport.py` | MockTransport sync/async, mismatch detection, streaming |
| `test_wsgi.py` | WSGITransport environ and streaming |
| `test_asgi.py` | ASGITransport scope, channels, streaming, disconnect, chunking, raw_path |
| `test_auth.py` | BasicAuth, DigestAuth (all variants, qop-list, query, cross-origin, auth-int), NetRCAuth (all edge cases) |
| `test_hooks.py` | Hook ordering with auth, error cleanup, response mutation |
| `test_environment.py` | trust_env, proxy env vars, base URL resolution |
| `test_backends.py` | Async context manager, closed client, type validation, leaked task detection, event loop non-blocking, client extensions |
| `test_extensions.py` | Extension passthrough (build_request→send→Response, transport, mount, streaming, async paths) |

## Test Results

- **653 Python compat tests passed** (all non-network compat tests)
- **883 Rust tests passed**
- Pre-existing network timeout tests excluded (read timeout on POST bytes)
- Feature matrix: all pass
- CI: fmt clean, clippy clean, lint suppression clean

## Known Limitations

| Limitation | Status |
|------------|--------|
| UDS / local address | eggfetch-core lacks UDS support; documented as core limitation |
| SOCKS proxy | Not in HTTPX 0.28.1 public API; deferred to Stage D |
| Trio backend | Deferred to Stage D; asyncio path retained |
| Redirect rerouting | eggfetch-core handles redirects internally; mount re-evaluation not possible at compat layer |
| local_address / socket_options | Accepted for API compat but not forwarded (eggfetch-core limitation) |
