# Troubleshooting

Common errors and how to resolve them.

## Certificate Errors

### Expired or self-signed certificate

**Error**: `certificate verification failed` or `hostname verification failed`

The server's certificate is not trusted by the system trust store.

- Use `verify="/path/to/ca-bundle.crt"` (Python) or `--ca-bundle /path/to/ca-bundle.crt` (CLI) to provide a custom CA bundle.
- For self-signed certificates in development, use `verify=False` (Python) or `--no-verify` (CLI). Never disable verification in production.
- Ensure the system clock is correct; an incorrect clock causes valid certificates to appear expired.

### Custom CA or corporate proxy

**Error**: `TLS error: invalid peer certificate` or `certificate verification failed`

Corporate proxies that perform TLS interception present their own CA certificate. Provide the proxy's CA certificate:

```python
client = eggfetch.Client(verify="/path/to/proxy-ca.crt")
```

```sh
eggfetch --ca-bundle /path/to/proxy-ca.crt https://example.com
```

## Connection Timeouts

### connect-timeout vs total-timeout

**Error**: `connect timeout after ...` or `total timeout after ...`

- `connect-timeout` limits only the TCP+TLS handshake phase. Default is no limit.
- `total-timeout` limits the entire request lifecycle including connect, write, and read. Default is no limit.

A common mistake is setting only `connect-timeout` for a slow server. Use `total-timeout` to cap the overall duration:

```python
client = eggfetch.Client(timeout=eggfetch.Timeout(total=30.0))
```

### Connection refused

**Error**: `connect error: connection refused`

The target host is not listening on the expected port. Verify the URL, port, and that the server is running.

## Read Timeouts

**Error**: `read timeout after ...`

The server accepted the connection but is slow to send the response body. Increase the timeout or use streaming to read incrementally:

```python
async with eggfetch.AsyncClient() as client:
    async with client.stream("GET", "https://example.com/large") as resp:
        async for chunk in resp.aiter_bytes():
            ...
```

## Too Many Redirects

**Error**: `too many redirects (20 followed, max 20)`

The redirect chain exceeds `max_redirects` (default: 20). This usually means a redirect loop.

- Check if the server is redirecting in a cycle (e.g., HTTP to HTTPS and back).
- Increase the limit if the chain is legitimately long: `Client(max_redirects=50)`.
- Disable redirects entirely: `Client(follow_redirects=False)`.

## Proxy Connection Failures

### Proxy URL errors

**Error**: `invalid proxy URL: ...`

The proxy URL scheme is not `http://` or `http+CONNECT://`. Ensure the proxy URL uses a supported scheme.

### Proxy authentication required

**Error**: `proxy authentication required`

The proxy requires credentials. Provide them in the proxy URL or via `ProxyAuth`:

```rust
Proxy::http("http://proxy:8080")?.auth("user", "password")
```

### NO_PROXY bypass

Requests to hosts matching `NO_PROXY` bypass the proxy. Verify your bypass rules match the expected hosts. `NO_PROXY` rules support exact host match, domain suffix match (`.example.com`), and wildcard (`*`).

## TLS Version Mismatch

**Error**: `TLS error: handshake failure`

The server and client cannot agree on a TLS version. Check that the server supports TLS 1.2 or higher. You can restrict the range with `TlsConfig::builder().min_tls_version(TlsVersion::TLS_1_3)`.

## HTTP/2 Negotiation Failures

**Error**: `HTTP/2 protocol error: ...` or `HTTP/2 GOAWAY: ...`

- The server does not support HTTP/2 or the ALPN negotiation failed. Try HTTP/1.1 explicitly with `HttpVersionPolicy::Http1Only`.
- A GOAWAY frame means the server is closing the connection. The request may be retried.
- `REFUSED_STREAM` resets are automatically retried by the retry subsystem.

## Decompression Errors

### Unsupported content encoding

**Error**: `unsupported content encoding: br`

The server sent a content encoding not enabled in your feature set. Enable the relevant compression feature:

```toml
[dependencies]
eggfetch-core = { features = ["compression-brotli"] }
```

### Decompression bomb protection

**Error**: `decoded body exceeded max decoded body size` or `decompression ratio exceeded max ratio`

The response expanded beyond the configured safety limits. Increase the limits if the response is legitimately large, or investigate whether the server is returning unexpected content.

## Body Too Large Errors

**Error**: `decoded body exceeded max decoded body size`

The decompressed response exceeds `max_decoded_body_size`. Increase the limit or use streaming to process the body in chunks without buffering the entire response.

## Pool Exhaustion

**Error**: `pool error: ...` (pool acquisition timeout or cancellation)

All connection pool slots are occupied. Increase the pool size:

```rust
ClientBuilder::new().max_connections(100).max_connections_per_host(20)
```

```python
eggfetch.Client(max_connections=100, max_connections_per_host=20)
```

If requests are hanging, a timeout or cancellation issue may be holding pool slots. Set `total-timeout` to prevent indefinite requests.

## Cookie Parsing Errors

**Error**: `malformed Set-Cookie header` or unexpected cookie behavior

The server sent a `Set-Cookie` header that does not conform to RFC 6265. eggfetch parses cookies leniently but rejects malformed domain or path attributes. Inspect the raw `Set-Cookie` header with `response.headers["set-cookie"]` for details.

## Multipart Encoding Errors

**Error**: `request build error: ...` during multipart upload

- Ensure file paths in `files=` exist and are readable.
- Filenames containing `@` must be escaped or use the `File` constructor directly.
- Content-type detection is based on file extension; override with `File(path, content_type="...")`.

## Retry Budget Exhausted

**Error**: `retry budget exhausted after N attempts`

The retry policy ran out of attempts. Increase `max_retries` or check whether the error is retryable. Streaming request bodies (`BodyNotReplayableForRetry`) cannot be retried because the body has been consumed.

## Python-Specific Issues

### GIL release

The synchronous `Client` releases the Python GIL while awaiting the Tokio runtime. Other Python threads can execute concurrently during network I/O. This is by design and does not require special handling.

### Async event loop issues

**Error**: `RuntimeError: no running event loop`

`AsyncClient` methods must be called from within an active async context. Use `await` and ensure you are inside an `asyncio.run()` or event loop. The standalone `eggfetch.request()` async function handles runtime setup internally.

## CLI-Specific Issues

### Exit codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 2 | Usage error (bad arguments, invalid URL, TLS config) |
| 3 | Connection error (connect, TLS handshake, proxy) |
| 4 | Timeout (connect or total) |
| 5 | Protocol error (HTTP/2, decompression, redirect loop) |
| 6 | HTTP status error (`--check-status` failed) |
| 7 | I/O error (read/write failure) |
| 130 | Interrupted (Ctrl+C) |

### Binary content detection

The CLI detects binary response bodies and writes to a file in download mode (`-o`) rather than printing to stdout. Use `-o -` to force stdout even for binary content.
