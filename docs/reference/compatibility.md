# Feature compatibility matrix

This page compares requests, HTTPX 0.28.1, the sibling HTTPX2 2.12.0 facade,
and native eggfetch APIs. HTTPX claims cover only the documented
asyncio-supported public surface, not every transport or backend. Each facade
has a separate compatibility profile. HTTPX 1.0 preview remains unqualified,
and experimental Node.js bindings are not included in this matrix.

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
| HTTP proxy endpoint (`http://`) | Yes | Yes | Yes | Yes | Yes |
| HTTPS proxy endpoint (`https://`) | Yes | Yes | Yes | Yes | Yes |
| HTTPS CONNECT tunnel | Yes | Yes | Yes | Yes | Yes |
| SOCKS5 proxy | No | Yes (httpx[socks]) | Yes | Yes | Yes |
| Proxy authentication | Yes | Yes | Yes | Yes | N/A |
| NO_PROXY bypass | Yes | Yes | Yes | Yes | N/A |
| Unix domain sockets | No | Yes | Yes | N/A | Yes |
| Local address binding | No | Yes | Yes | N/A | Yes |
| Socket options | No | Yes | Yes | N/A | Yes |
| Response decompression (gzip) | Yes | Yes | Yes | No (no decoders; `--no-compress` passes through) | Yes |
| Response decompression (brotli) | Yes | Yes | Yes | No (no decoders; `--no-compress` passes through) | Yes |
| Response decompression (zstd) | Yes | Yes | Yes | No (no decoders; `--no-compress` passes through) | Yes |
| Response decompression (deflate) | Yes | Yes | Yes | No (no decoders; `--no-compress` passes through) | Yes |
| Multipart file upload | Yes | Yes | Yes | Yes | Yes |
| Timeouts (connect, read, write) | Partial | Yes | Yes | Yes | Yes |
| Timeout (total wall-clock) | No | No | Yes (native `total`; facade maps 4 phases only) | Yes | Yes |
| Timeout (pool wait) | No | Yes | Yes | Yes | Yes |
| Retry policy | No | No | Yes | Yes | Yes |
| Retry-After header | No | No | Yes | N/A | Yes |
| HTTP/2 | No | Yes | Yes | No (CLI builds without `http2`/`http3`) | Yes |
| HTTP/3 (experimental) | No | No | Yes | No (CLI builds without `http2`/`http3`) | Yes |
| Cross-origin credential stripping | Automatic (host change) | Automatic (cross-origin) | Automatic | Automatic | Automatic |
| Proxy env vars (HTTP_PROXY) | Yes | Yes | Facade Yes (`trust_env=True`); native Python Yes by default (`trust_env=True`), Rust/CLI explicit-only | CLI explicit (`--proxy`/`EGGFETCH_PROXY` only) | Explicit-only |
| Custom transports (sync/async) | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| HTTP transport (HTTPTransport/AsyncHTTPTransport) | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| URL-pattern mount routing (priority matching) | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| MockTransport (no-network testing) | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| WSGITransport (WSGI app testing) | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| ASGITransport (ASGI app testing) | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| Event hooks (request/response sequencing) | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| DigestAuth (MD5/SHA-256) | No | Yes | Facade Yes (native: Basic/Bearer only) | N/A | N/A |
| NetRCAuth | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| FunctionAuth (httpx2 only) | No | No (0.28.1) / Yes (httpx2 2.12.0) | httpx2 facade Yes | N/A | N/A |
| Origin / URL.origin (httpx2 only) | No | No (0.28.1) / Yes (httpx2 2.12.0) | httpx2 facade Yes | N/A | N/A |
| QUERY method (httpx2 only) | No | No (0.28.1) / Yes (httpx2 2.12.0) | httpx2 facade Yes | N/A | N/A |
| Headers merge operators (httpx2 only) | No | No (0.28.1) / Yes (httpx2 2.12.0) | httpx2 facade Yes | N/A | N/A |
| SSE EventSource (httpx2 only) | No | No (0.28.1) / Yes (httpx2 2.12.0) | httpx2 facade Yes (framing over streamed responses) | N/A | N/A |
| WebSocket optional surface (httpx2 only) | No | No (0.28.1) / Yes (httpx2[ws]) | httpx2 facade Yes (wsproto over 101 network_stream) | N/A | N/A |
| Auth flow generator pattern | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| Async API | No | Yes | Yes | N/A | Yes (native) |

## Partially supported with differences

| Feature | Difference |
| --- | --- |
| Auth tuple shorthand | requests accepts `auth=("user","pass")`. eggfetch Python supports this. eggfetch Rust requires `BasicAuth::new("user", "pass")`. |
| Proxy configuration | requests uses a dict by scheme. eggfetch uses a single `proxy=` string. |
| Proxy env vars | The HTTPX facade selects `HTTP_PROXY`/`HTTPS_PROXY` by request scheme, uses `ALL_PROXY` as fallback, and honors lowercase forms plus `NO_PROXY` when `trust_env=True`; native Python also reads env by default (`trust_env=True`); native Rust configuration and the CLI (`--proxy`/`EGGFETCH_PROXY`) remain explicit. |
| Timeout tuple | requests accepts `(connect, read)` tuples. eggfetch uses `Timeout` objects. |

## Intentionally unsupported

| Feature | Reason |
| --- | --- |
| Trio async backend | Deferred to Stage D |
| Python 3.8/3.9 | Requires Python 3.10+ (tokio runtime requirement) |
| Private HTTPX modules | `_transports`, `_content`, `_models`, `_decoders`, `_exceptions`, `_multipart`, `_urlparse`, `_config` excluded from contract |
| HTTPX four-element null-pointer `socket_options` | Outside the safe Rust boundary; safe three-element form is supported |
| Unrepresentable `ssl.SSLContext` state | Fails closed with `TypeError` (safety boundary; representable state translates exactly) |

## Sync/async parity

eggfetch provides both `Client` (sync) and `AsyncClient` (async). Both
expose the same request methods, streaming API, cookie handling, and auth
configuration. The sync API blocks on the async Rust engine and releases
the GIL.

## Qualification status

The HTTPX 0.28.1 and HTTPX2 2.12.0 facades are Stage C qualified on frozen
executable SHA `8af32d7ca4f1140e266875798522f5da09c17850` (2026-11-16). The
full compatibility, extended, package, and live security gates passed on that
freeze. Earlier bindings are historical. Current evidence is in
`plans/httpx-parity-correction-status.md`; exact facade scopes are recorded in
`compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml`.

Stage C covers only the documented public asyncio surfaces. It does not claim
unrestricted HTTPX replacement. Trio/AnyIO, Python 3.8/3.9, private HTTPX
modules, and unrepresentable `ssl.SSLContext` state remain out of scope.

Facade-specific API coverage is summarized in the matrix above. For exact
coverage and remaining differences, see `compat/httpx/0.28.1/profile.toml`,
`compat/httpx2/2.12.0/profile.toml`, and `docs/residual-differences.md`.

Machine-readable API difference records are maintained under `compat/`. They
support compatibility maintenance and do not expand the claims above.
