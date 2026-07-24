# Feature compatibility matrix

This page tracks compatibility with requests and HTTPX across features.
 eggfetch Node.js bindings are experimental and not included in this matrix.

## Supported and tested

| Feature | requests | HTTPX | eggfetch Python | eggfetch CLI | eggfetch Rust |
| --- | --- | --- | --- | --- | --- |
| GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS | Yes | Yes | Yes | Yes | Yes |
| Arbitrary methods | Yes | Yes | Yes | Yes | Yes |
| Custom headers | Yes | Yes | Yes | Yes | Yes |
| Case-insensitive headers | Yes | Yes | Yes | N/A | Yes |
| Query parameters | Yes | Yes | Yes | Yes | Yes |
| JSON request body | Yes | Yes | Yes | Yes | Yes |
| Form-encoded body | Yes | Yes | Yes | Yes | Yes |
| Raw bytes body | Yes | Yes | Yes | Yes | Yes |
| Response status code | Yes | Yes | Yes | Yes | Yes |
| Response headers | Yes | Yes | Yes | Yes | Yes |
| Response text | Yes | Yes | Yes | Yes | Yes |
| Response bytes | Yes | Yes | Yes | Yes | Yes |
| Response JSON | Yes | Yes | Yes | Yes | Yes |
| Raise on status | Yes | Yes | Yes | N/A | N/A |
| Connection pooling | Yes | Yes | Yes | Yes | Yes |
| Context manager client | Yes | Yes | Yes | N/A | N/A |
| Default headers | Yes | Yes | Yes | N/A | Yes |
| Streaming download | Yes | Yes | Yes | Yes | Yes |
| Streaming upload | Yes | Yes | Yes | Yes | Yes |
| Cookies | Yes | Yes | Yes | Yes | Yes |
| Basic auth | Yes | Yes | Yes | Yes | Yes |
| Bearer auth | Yes | Yes | Yes | Yes | N/A |
| Redirect following | Yes | Yes | Yes | Yes | Yes |
| Max redirects | Yes | Yes | Yes | Yes | Yes |
| Custom CA bundle | Yes | Yes | Yes | Yes | Yes |
| Client certificates (mTLS) | Yes | Yes | Yes | Yes | Yes |
| Disable TLS verification | Yes | Yes | Yes | Yes | Yes |
| HTTP proxy | Yes | Yes | Yes | Yes | Yes |
| HTTPS CONNECT tunnel | Yes | Yes | Yes | Yes | Yes |
| Proxy authentication | Yes | Yes | Yes | Yes | N/A |
| NO_PROXY bypass | Yes | Yes | Yes | Yes | N/A |
| Response decompression (gzip) | Yes | Yes | Yes | Yes | Yes |
| Response decompression (brotli) | Yes | Yes | Yes | Yes | Yes |
| Response decompression (zstd) | Yes | Yes | Yes | Yes | Yes |
| Response decompression (deflate) | Yes | Yes | Yes | Yes | Yes |
| Multipart file upload | Yes | Yes | Yes | Yes | Yes |
| Timeouts (connect, read, write) | Partial | Yes | Yes | Yes | Yes |
| Timeout (total wall-clock) | No | Yes | Yes | Yes | Yes |
| Timeout (pool wait) | No | Yes | Yes | Yes | Yes |
| Retry policy | No | No | Yes | Yes | Yes |
| Retry-After header | No | No | Yes | N/A | Yes |
| HTTP/2 | No | Yes | Yes | Yes | Yes |
| HTTP/3 (experimental) | No | No | Yes | Yes | Yes |
| Cross-origin header stripping | Manual | Manual | Automatic | Automatic | Automatic |
| Proxy env vars (HTTP_PROXY) | Yes | Yes | **No** | **No** | **No** |
| Custom transports (sync/async) | No | Yes | Yes | N/A | N/A |
| HTTP transport (HTTPTransport/AsyncHTTPTransport) | No | Yes | Yes | N/A | N/A |
| URL-pattern mount routing (priority matching) | No | Yes | Yes | N/A | N/A |
| MockTransport (no-network testing) | No | Yes | Yes | N/A | N/A |
| WSGITransport (WSGI app testing) | No | Yes | Yes | N/A | N/A |
| ASGITransport (ASGI app testing) | No | Yes | Yes | N/A | N/A |
| Event hooks (request/response sequencing) | No | Yes | Yes | N/A | N/A |
| DigestAuth (MD5/SHA-256) | No | Yes | Yes | N/A | N/A |
| NetRCAuth | No | Yes | Yes | N/A | N/A |
| Auth flow generator pattern | No | Yes | Yes | N/A | N/A |
| Async API | No | Yes | Yes | N/A | Yes (native) |

## Partially supported with differences

| Feature | Difference |
| --- | --- |
| Auth tuple shorthand | requests accepts `auth=("user","pass")`. eggfetch Python supports this. eggfetch Rust requires `BasicAuth::new("user", "pass")`. |
| Proxy configuration | requests uses a dict by scheme. eggfetch uses a single `proxy=` string. |
| Proxy env vars | eggfetch reads `HTTP_PROXY`/`HTTPS_PROXY`/`NO_PROXY` env vars when `trust_env=True` (default). |
| Timeout tuple | requests accepts `(connect, read)` tuples. eggfetch uses `Timeout` objects. |

## Intentionally unsupported

| Feature | Reason |
| --- | --- |
| UDS (Unix domain sockets) | Not available in eggfetch-core |
| SOCKS proxy | Not in HTTPX 0.28.1 public API, deferred |
| Trio async backend | Deferred to Stage D |
| requests Session hooks | Not implemented |
| requests PreparedRequest | Not part of the public API |

## Sync/async parity

eggfetch provides both `Client` (sync) and `AsyncClient` (async). Both
expose the same request methods, streaming API, cookie handling, and auth
configuration. The sync API blocks on the async Rust engine and releases
the GIL.

## HTTPX compatibility status

eggfetch targets HTTPX 0.28.1 compatibility in phases. The current status:

- **Phase 0**: Compatibility profile defined, manifest generators created, differential tests mandatory
- **Phase 1**: Timeout, pool, and lifecycle behavior alignment
- **Phase 2**: Value objects, request/response, exception hierarchy, client constructors, merge semantics
- **Phase 3**: Streaming and bodies, byte streams, chunk size support, request streaming, multipart passthrough
- **Phase 4**: Transports, mounts, auth, hooks, WSGI/ASGI
- **Phase 5**: Downstream validation, 12-package consumer portfolio, expanded behavior corpus, upstream test inventory, evidence reporting, performance budgets, compatibility-stage decision (Stage C)

See `compat/httpx/0.28.1/` for the machine-readable profile and allowed differences.

### Corrected claims

The following statements from earlier documentation have been corrected:

1. **Pool timeout**: HTTPX 0.28.1 supports pool timeout via `Timeout(pool=...)`. eggfetch also supports this. The compatibility matrix has been updated to reflect this.
2. **Redirect default**: HTTPX 0.28.1 defaults to `follow_redirects=False`, same as eggfetch. The earlier claim that "HTTPX follows redirects by default" was incorrect for version 0.28.1.
3. **Proxy env vars**: eggfetch reads `HTTP_PROXY`/`HTTPS_PROXY`/`NO_PROXY` env vars when `trust_env=True` (default). Implemented in Phase 1.
