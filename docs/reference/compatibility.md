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
| Async API | No | Yes | Yes | N/A | Yes (native) |

## Partially supported with differences

| Feature | Difference |
| --- | --- |
| Redirect default | requests/HTTPX follow redirects by default. eggfetch does **not**. Set `follow_redirects=True`. |
| Auth tuple shorthand | requests accepts `auth=("user","pass")`. eggfetch Python supports this. eggfetch Rust requires `BasicAuth::new("user", "pass")`. |
| Proxy configuration | requests uses a dict by scheme. eggfetch uses a single `proxy=` string. |
| Proxy env vars | eggfetch does not read `HTTP_PROXY`/`HTTPS_PROXY`/`NO_PROXY` env vars. |
| Timeout tuple | requests accepts `(connect, read)` tuples. eggfetch uses `Timeout` objects. |
| HTTPX transports | HTTPX supports WSGI/ASGI in-process transports. eggfetch does not. Tracked as required-later. |
| HTTPX mounts | HTTPX supports per-host transport mounts. eggfetch does not. Tracked as required-later. |

## Intentionally unsupported

| Feature | Reason |
| --- | --- |
| WSGI/ASGI transports | eggfetch is a network client, not a server testing tool |
| Trio/AnyIO | asyncio only for now |
| SOCKS proxy | Not implemented |
| requests Session hooks | Not implemented |
| requests PreparedRequest | Not part of the public API |
| HTTPX event hooks | Not implemented |
| HTTPX mock transports | Use a real test server instead |

## Sync/async parity

eggfetch provides both `Client` (sync) and `AsyncClient` (async). Both
expose the same request methods, streaming API, cookie handling, and auth
configuration. The sync API blocks on the async Rust engine and releases
the GIL.

## HTTPX compatibility status

eggfetch targets HTTPX 0.28.1 compatibility in phases. The current status:

- **Phase 0 (current)**: Compatibility profile defined, manifest generators created, differential tests mandatory
- **Phase 1**: Timeout, pool, and lifecycle behavior alignment
- **Phase 2**: Transport, mount, and hook surfaces

See `compat/httpx/0.28.1/` for the machine-readable profile and allowed differences.

### Corrected claims

The following statements from earlier documentation have been corrected:

1. **Pool timeout**: HTTPX 0.28.1 supports pool timeout via `Timeout(pool=...)`. eggfetch also supports this. The compatibility matrix has been updated to reflect this.
2. **Redirect default**: Both HTTPX and requests follow redirects by default. eggfetch does **not**. This is an intentional security-first difference.
3. **Proxy env vars**: HTTPX reads `HTTP_PROXY`/`HTTPS_PROXY`/`NO_PROXY` by default. eggfetch does not. This is tracked as a required-later item.
