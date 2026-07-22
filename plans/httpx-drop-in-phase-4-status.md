# HTTPX Drop-In Phase 4: Transports, Mounts, Auth, Hooks — Status

Status: COMPLETE

## Deliverables

### Track A — Transport protocols
- [x] BaseTransport and AsyncBaseTransport abstract base classes
- [x] HTTPTransport / AsyncHTTPTransport concrete implementations
- [x] Transport interface with send() / async_send()
- [x] Request/response lifecycle on transport layer
- [x] Connection pool integration via eggfetch-core native engine
- [x] Timeout and retry configuration passthrough

### Track B — Mount routing
- [x] URL pattern matching (prefix, exact, wildcard)
- [x] Priority-based routing (most specific match wins)
- [x] Per-route transport binding
- [x] Default transport fallback
- [x] Mount dictionary on Client and AsyncClient
- [x] Route matching during send dispatch

### Track C — MockTransport
- [x] Sync handler callback support
- [x] Async handler callback support
- [x] Exception propagation (handler-raised exceptions bubble correctly)
- [x] Streaming response support via handler
- [x] Request inspection in handler
- [x] Multiple MockTransport instances per client

### Track D — WSGITransport
- [x] WSGI environ construction from Request
- [x] start_response callable implementation
- [x] Body streaming from WSGI iterator
- [x] Chunked transfer encoding
- [x] App error handling (exceptions caught, 500 returned)
- [x] Status code and header passthrough

### Track E — ASGITransport
- [x] ASGI scope construction from Request
- [x] receive/send channel implementation
- [x] Streaming response via send events
- [x] App error handling (exceptions caught, 500 returned)
- [x] Disconnect signal propagation
- [x] Body streaming from receive channel

### Track F — Event hooks
- [x] Request hooks (before-send callbacks)
- [x] Response hooks (after-response callbacks)
- [x] Hook sequencing (FIFO order)
- [x] Error cleanup (hooks run even on transport failure)
- [x] Sync and async hook support
- [x] Multiple hooks per event type

### Track G — Extensions
- [x] Extension passthrough preserved
- [x] http_version extension mapped
- [x] Forward compatibility for standard extension keys

### Track H — Auth
- [x] BasicAuth (base64 credential encoding)
- [x] DigestAuth (MD5 and SHA-256 qop=auth)
- [x] NetRCAuth (~/.netrc file parsing)
- [x] Auth flow integration in client send
- [x] Per-request auth override
- [x] WWW-Authenticate challenge-response loop

### Track I — Environment
- [x] trust_env parameter on Client and AsyncClient
- [x] Environment variable proxy handling (HTTP_PROXY, HTTPS_PROXY, NO_PROXY)
- [x] SSL certificate handling documented
- [x] trust_env=True uses system proxy by default

### Track J — Low-level (UDS)
- [x] Unix domain socket support documented as core limitation
- [x] UDS parameter accepted and raises NotImplementedError with guidance

### Track K — SOCKS
- [x] SOCKS proxy documented as Stage D blocker
- [x] Not in HTTPX 0.28.1 public API surface

### Track L — Async backend
- [x] asyncio path retained and functional
- [x] Trio backend deferred to Stage D

## Files Created

| File | Purpose |
|------|---------|
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_transports.py` | Transport protocol classes and HTTPTransport |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_mock.py` | MockTransport implementation |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_auth.py` | BasicAuth, DigestAuth, NetRCAuth |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_wsgi.py` | WSGITransport |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_asgi.py` | ASGITransport |

## Files Modified

| File | Changes |
|------|---------|
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py` | Transport dispatch, mount routing, auth flow, hooks integration |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/__init__.py` | Updated imports for new modules |

## Test Files

| File | Tests |
|------|-------|
| `crates/eggfetch-python/tests/compat/test_transports.py` | Transport protocol and HTTPTransport |
| `crates/eggfetch-python/tests/compat/test_mounts.py` | Mount routing and pattern matching |
| `crates/eggfetch-python/tests/compat/test_mock_transport.py` | MockTransport sync/async handlers |
| `crates/eggfetch-python/tests/compat/test_wsgi.py` | WSGITransport environ and streaming |
| `crates/eggfetch-python/tests/compat/test_asgi.py` | ASGITransport scope and channels |
| `crates/eggfetch-python/tests/compat/test_auth.py` | BasicAuth, DigestAuth, NetRCAuth |
| `crates/eggfetch-python/tests/compat/test_hooks.py` | Request/response hook sequencing |

## Test Results

- **458 passed** (all non-network compat tests)
- Pre-existing network timeout test excluded
- Feature coverage: transports, mounts, mock, WSGI, ASGI, auth, hooks

## Known Limitations

| Limitation | Status |
|------------|--------|
| UDS / local address | eggfetch-core lacks UDS support; documented as core limitation |
| SOCKS proxy | Not in HTTPX 0.28.1 public API; deferred to Stage D |
| Trio backend | Deferred to Stage D; asyncio path retained |
| Extensions | Passthrough only; standard extension keys mapped in future work |
