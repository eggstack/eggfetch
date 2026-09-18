# Feature matrix by platform

This page tracks which features are available in each eggfetch surface:
Rust (eggfetch-core), Python (eggfetch-python), CLI (eggfetch-cli), and
Node.js (eggfetch-node, experimental).

## Feature flags (Rust / eggfetch-core)

| Feature flag | Default | Python default | CLI default |
| --- | --- | --- | --- |
| `http1` | Yes | Yes | Yes |
| `http2` | No | Yes | No |
| `http3` | No | Yes | No |
| `tls-rustls` | Yes | Yes | Yes |
| `tls-native-roots` | Yes | Yes | Yes |
| `native-http1` / `native-http2` | Via `http1`/`http2` | Via `http1`/`http2` | Via `http1` |
| `transport-http1` / `transport-http2` | Via `http1`/`http2` | Via `http1`/`http2` | Via `http1` |
| `standard-route` | Via `http1` | Yes | Yes |
| `advanced-routing` | Via `http1` | Yes | Yes |
| `standard-http1` / `standard-http2` | No (lean opt-in) | N/A | N/A |
| `high-level-url` | Yes (via `http1`) | Yes | Yes |
| `logical-retry` | Yes (via `http1`) | Yes | Yes |
| `redirects` | Yes (via `http1`) | Yes | Yes |
| `basic-auth` | Yes (via `http1`) | Yes | Yes |
| `cookies` | No | Yes | Yes |
| `proxy` | No | Yes | Yes |
| `multipart` | No | Yes | Yes |
| `compression-gzip` | No | Yes | No (CLI never sends `Accept-Encoding`; encoded bodies fail unless `--no-compress`) |
| `compression-brotli` | No | Yes | No (same as above) |
| `compression-zstd` | No | Yes | No (same as above) |
| `compression-deflate` | No | Yes | No (same as above) |
| `json` | No | N/A | N/A |
| `tracing` | No | N/A | N/A |
| `test-util` | No | N/A | N/A |

## API surface by language

### Rust (eggfetch-core)

| API | Available |
| --- | --- |
| `Client` / `ClientBuilder` | Yes |
| `RequestBuilder` with method chaining | Yes |
| `Response` (buffered + streaming) | Yes |
| `Timeout` / `TimeoutBuilder` | Yes |
| `TlsConfig` / `TlsConfigBuilder` | Yes |
| `RetryPolicy` / `RetryPolicyBuilder` | Yes (requires `logical-retry`; absent from lean `standard-http1`/`standard-http2` profiles) |
| `Proxy` / `NoProxy` | Yes (feature-gated) |
| `Multipart` / `Part` / `Boundary` | Yes (feature-gated) |
| `CookieJar` | Yes (feature-gated) |
| `BasicAuth` | Yes (requires `basic-auth`; lean profiles are Bearer-only) |
| `BearerAuth` | Yes (available in all profiles, including lean) |
| `HttpVersionPolicy` | Yes |
| `RedirectPolicy` | Yes (requires `redirects`; lean profiles return 3xx without following) |
| `Dialer` / `DialTarget` / `SocketOption` | Yes (require `advanced-routing`; absent from lean profiles) |
| `ClientBuilder::dialer` / `local_address` / `socket_options` / `uds_path` | Yes (require `advanced-routing`; absent from lean profiles) |
| `RequestBuilder::resolved_addresses` / `TransportHints::{sni_hostname, resolved_target}` | Yes (require `advanced-routing`; fail closed with `Unsupported` in lean profiles) |
| `PoolConfig` / `PoolMetrics` | Yes |
| `ContentCoding` (Accept-Encoding) | Yes |
| Streaming body (`BoxBytesStream`) | Yes |
| Error taxonomy (`Error` enum) | Yes |

### Python (eggfetch-python)

| API | Available |
| --- | --- |
| `Client` / `AsyncClient` | Yes |
| `client.get/post/put/patch/delete/head/options` | Yes |
| `client.request(method, url)` | Yes |
| `client.stream()` / `async_client.stream()` | Yes |
| `Response` (status_code, headers, text, json, content) | Yes |
| `Response.raise_for_status()` | Yes |
| `Response.iter_bytes/iter_text/iter_lines` | Yes |
| `StreamingResponse` context manager | Yes |
| `StreamingResponse.aiter_bytes/aiter_text/aiter_lines` | Yes |
| `Timeout` | Yes |
| `BasicAuth` / `BearerAuth` / `NoAuth` / `NOAUTH` | Yes |
| `Retry` | Yes |
| `File` (path-based upload) | Yes |
| `Headers` (case-insensitive) | Yes |
| `Cookies` | Yes |
| `proxy=` kwarg | Yes |
| `verify=` / `cert=` kwarg | Yes |
| `http2=` / `http3=` kwarg | Yes |
| `decompress=` kwarg | Yes |
| `follow_redirects=` / `max_redirects=` | Yes |
| `retries=` kwarg | Yes |
| Top-level `get/post/...` functions | Yes |
| Context manager (`with`) | Yes |
| Async context manager (`async with`) | Yes |
| `__version__` | Yes |

### CLI (eggfetch-cli)

| Feature | Available |
| --- | --- |
| All HTTP methods (`-X`) | Yes |
| Headers (`-H NAME:VALUE`) | Yes |
| Query params (`-q NAME=VALUE`) | Yes |
| JSON body (`--json`) | Yes |
| Form fields (`--form`) | Yes |
| File upload (`--file NAME=@PATH`) | Yes |
| Raw body (`--body`, `--body-file`) | Yes |
| Auth (`--auth`, `--bearer`) | Yes |
| Bearer from env (`EGGFETCH_BEARER`) | Yes |
| Proxy (`--proxy`) | Yes |
| Proxy auth (`--proxy-auth`) | Yes |
| NO_PROXY (`--no-proxy`) | Yes |
| TLS verify/no-verify (`--verify`/`--no-verify`) | Yes |
| Custom CA (`--cacert`) | Yes |
| Client cert (`--cert`, `--key`) | Yes |
| Timeouts (`--timeout`, `--total-timeout`, `--read-timeout`) | Yes |
| Retries (`--retry`, `--retry-delay`) | Yes |
| Follow/no-follow redirects | Yes |
| Max redirects (`--max-redirects`) | Yes |
| HTTP version (`--http1`, `--http2`, `--http3`) | Yes (compile-time) |
| Output file (`-o`) | Yes |
| Download mode (`--download`) | Yes |
| Include headers (`-i`) | Yes |
| Headers only (`--headers-only`) | Yes |
| No body (`--no-body`) | Yes |
| JSON output (`--json-output`) | Yes |
| NDJSON output (`--ndjson`) | Yes |
| Base64 encoding (`--base64`) | Yes |
| No clobber (`--no-clobber`) | Yes |
| Decompression (`--no-compress` to disable) | Yes |
| Cookie jar (`--cookie-jar`) | Yes |
| Shell completions (`--generate-completion`) | Yes |
| Exit codes (0-7, 130) | Yes |
| JSON error output | Yes |
| Secret redaction in verbose output | Yes |
| TTY-aware output | Yes |

### Node.js (eggfetch-node, experimental prototype)

Node.js bindings via N-API (napi-rs). **Experimental prototype, not a
supported binding** — API surfaces may change without notice. Narrow
guarantees only: common-verb plus arbitrary-method requests, UTF-8
string bodies, buffered responses (`status`, `url`, `headers`,
`getAll`, `text`, `bytes`, `json`, `ok`). See
`docs/architecture/ffi-and-node.md` for the full support contract.

## Limitations per platform

### Python

- No Trio/AnyIO support (asyncio only).
- No WSGI/ASGI in-process transports in the native API (the HTTPX compatibility facades provide `WSGITransport`/`ASGITransport`).
- Native proxy bypass comes from `NO_PROXY` in the environment (`trust_env=True`, the default); proxy credentials travel in the proxy URL. There is no `no_proxy=`/`proxy_auth=` kwarg on the native `Client`.
- Encrypted private keys for mTLS produce a clear error at construction.
- HTTP/3 is experimental (retained this milestone; see "Production Graduation Decision" in `docs/architecture/core-tls-proxy-protocols.md`); API surfaces may change.
- Compatibility facades: `eggfetch.compat.httpx` (HTTPX 0.28.1, Stage C) and `eggfetch.compat.httpx2` (httpx2 2.12.0, Stage C) coexist; HTTPX 1.0 is preview-only with no parity claim.

### CLI

- No streaming upload from stdin pipe (use `--body-file -` for buffered).
- No response hooks or scripting beyond shell piping.
- No session state between invocations.
- `http2` and `http3` flags are compile-time; check the binary's feature
  set. The default CLI build enables `cookies`, `multipart`, and `proxy`
  but **not** `http2` or `http3`.

### Node.js (experimental prototype)

- Experimental prototype, not a supported binding; API surfaces may change without notice.
- String-only request bodies (no lossless `Buffer`/`Uint8Array` path).
- Buffered responses only (no streaming; bodies must fit in memory).
- No cancellation, request configuration (headers/timeout/redirects/TLS/proxy/auth/HTTP-version), structured errors, or generated TypeScript declarations (`index.d.ts` is a stub).
- Requires Node.js 16+.
- HTTP/3 support depends on the Rust feature flags at build time.
- JS test surface (`test.js`) runs only when a built `./eggfetch.node` artifact is present; Tier 1 records an explicit skip otherwise.
