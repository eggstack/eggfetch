# Feature matrix by platform

This page tracks which features are available in each eggfetch surface:
Rust (eggfetch-core), Python (eggfetch-python), CLI (eggfetch-cli), and
Node.js (eggfetch-node, experimental).

## Feature flags (Rust / eggfetch-core)

| Feature flag | Default | Python default | CLI default |
| --- | --- | --- | --- |
| `http1` | Yes | Yes | Yes |
| `http2` | No | Yes | Yes |
| `http3` | No | Yes | No |
| `tls-rustls` | Yes | Yes | Yes |
| `cookies` | No | Yes | Yes |
| `proxy` | No | Yes | Yes |
| `multipart` | No | Yes | Yes |
| `compression-gzip` | No | Yes | Yes |
| `compression-brotli` | No | Yes | Yes |
| `compression-zstd` | No | Yes | Yes |
| `compression-deflate` | No | Yes | Yes |
| `json` | No | N/A | N/A |

## API surface by language

### Rust (eggfetch-core)

| API | Available |
| --- | --- |
| `Client` / `ClientBuilder` | Yes |
| `RequestBuilder` with method chaining | Yes |
| `Response` (buffered + streaming) | Yes |
| `Timeout` / `TimeoutBuilder` | Yes |
| `TlsConfig` / `TlsConfigBuilder` | Yes |
| `RetryPolicy` / `RetryPolicyBuilder` | Yes |
| `Proxy` / `NoProxy` | Yes (feature-gated) |
| `Multipart` / `Part` / `Boundary` | Yes (feature-gated) |
| `CookieJar` | Yes (feature-gated) |
| `BasicAuth` / `BearerAuth` | Yes |
| `HttpVersionPolicy` | Yes |
| `RedirectPolicy` | Yes |
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
| `no_proxy=` kwarg | Yes |
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

### Node.js (eggfetch-node, experimental)

Node.js bindings via N-API (napi-rs). **Experimental** — API surfaces may
change before 1.0.

## Limitations per platform

### Python

- No Trio/AnyIO support (asyncio only).
- No WSGI/ASGI in-process transports.
- No SOCKS proxy support.
- `connect` timeout is accepted but not independently enforced.
- Encrypted private keys for mTLS produce a clear error at construction.
- HTTP/3 is experimental; API surfaces may change.

### CLI

- No streaming upload from stdin pipe (use `--body-file -` for buffered).
- No response hooks or scripting beyond shell piping.
- No session state between invocations.
- `http2` and `http3` flags are compile-time; check the binary's feature
  set. The default CLI build enables `cookies`, `multipart`, and `proxy`
  but **not** `http2` or `http3`.

### Node.js (experimental)

- Experimental; API surfaces may change before 1.0.
- Requires Node.js 16+.
- HTTP/3 support depends on the Rust feature flags at build time.
