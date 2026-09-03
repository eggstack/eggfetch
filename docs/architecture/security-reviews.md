# Security Reviews

This document records security review findings for each major subsystem of eggfetch. Each section summarizes the review scope, key findings, and current posture.

## TLS Review

**Scope**: TLS configuration, certificate verification, trust store resolution, client certificates, SNI/ALPN behavior, and version policy.

### Trust Store Resolution

- **Native roots first**: eggfetch attempts the operating system's native root store before falling back to the packaged Mozilla/WebPKI roots. This preserves the system administrator's trust policy.
- **Packaged roots as construction-only fallback**: The Mozilla roots are loaded only when the native store is unavailable (e.g., minimal containers). They are not tried after a certificate-chain or hostname verification failure. This prevents silent trust escalation.
- **Custom CA replaces all defaults**: When a custom `TrustStore` is provided, it replaces both native and packaged roots. This prevents a misconfigured system trust store from silently undermining the custom policy. If both system and private CAs are needed, the caller concatenates them into a single PEM file.

### Certificate Verification

- **Verification enabled by default**: Certificate chain validation and hostname verification are enforced unless explicitly disabled.
- **Failure is a hard error**: Verification failures are not retried, and the client does not fall back to a permissive mode.
- **Hostname verification**: Standard rustls hostname verification is applied. IP address SANs are handled by rustls.

### Verification Bypass

- **Explicit opt-in API**: `danger_accept_invalid_certs(true)` is the only path to disable verification. The API name deliberately signals danger.
- **No configuration-file bypass**: There is no config file, environment variable, or build flag that silently disables verification.
- **Python repr warning**: `Client.__repr__` prints `[UNSAFE: TLS verification disabled]` when `verify=False` is set, making the insecure state visible in interactive use.

### Client Certificates (mTLS)

- **Key and cert storage**: `ClientIdentity` holds the certificate chain and private key in memory only.
- **No key logging**: Private key material is never included in debug output, error messages, or repr diagnostics. Only the key-label identifier is referenced.
- **Encrypted key rejection**: Encrypted PEM private keys produce an error at construction time. Only unencrypted PEM keys are supported.
- **Rustls negotiation**: The actual mTLS handshake is handled by rustls, which has a well-audited implementation.

### SNI and ALPN

- **SNI**: Configured via hyper-rustls, standard behavior. The server name is derived from the request URI.
- **ALPN**: Set based on `HttpVersionPolicy`. For `Auto`, both `h2` and `http/1.1` are advertised. For `Http2Only`, only `h2`. For `Http1Only`, only `http/1.1`.
- **No ALPN stripping**: The ALPN list is set on the rustls configuration and not modified after construction.

### TLS Version Policy

- **TLS 1.2 minimum**: The default allows TLS 1.2 and 1.3. SSLv3, TLS 1.0, and TLS 1.1 are not supported.
- **Configurable**: `TlsVersion` enum allows pinning to specific versions (e.g., TLS 1.3 only).
- **No downgrade**: The client does not silently downgrade from a higher version to a lower version.

### Findings

No critical or high-severity findings. The trust store fallback is well-scoped and does not introduce silent trust escalation. Verification bypass requires explicit API opt-in. Private key material is correctly excluded from diagnostic output.

---

## Redirect / Auth / Cookie Review

**Scope**: Cross-origin redirect handling, credential stripping, header preservation, redirect history, and cookie/auth interaction.

### Cross-Origin Redirect Stripping

- **Headers stripped**: `Authorization` and `Proxy-Authorization` are stripped on every redirect hop; `Cookie` is additionally stripped (and `Host` reset) on cross-origin hops. (`Set-Cookie` is a response header and never appears on redirect requests.) This prevents credential leakage to third-party origins.
- **Origin determination**: Origin is computed from scheme, host, and port. Port changes are treated as cross-origin (e.g., `https://example.com:443` to `https://example.com:8443`).
- **Chained redirects**: Each hop in a redirect chain is evaluated independently. Credentials are stripped on each cross-origin hop, not just the first.

### Same-Origin Redirects

- **Headers handled**: Same-origin redirects strip `Authorization`/`Proxy-Authorization` from the cloned set, then re-apply configured client-level auth; `Cookie`/`Host` survive.
- **Client auth reapplied**: Client-level auth is reapplied on same-origin redirects, ensuring the credentials are present even if the original header was modified by the server.

### User-Provided Headers

- **Request-level overrides**: User-provided headers on `RequestBuilder` take precedence over client defaults. On redirect, the redirect engine uses the user-provided headers for the initial request and strips sensitive headers on cross-origin hops.
- **No silent re-injection**: Client-level auth is not re-injected on cross-origin redirects, even if the client has auth configured.

### Redirect History

- **Metadata only**: `HistoryEntry` records status code, URL, and headers for each hop but does not store body content. This prevents unbounded memory growth on long redirect chains.
- **Total timeout**: The total timeout is a single deadline across the entire redirect chain, preventing infinite loops from extending the effective timeout.

### Cookie/Auth Interaction

- **Independent systems**: Disabling auth (via `without_auth()`) does not affect cookie handling. Cookies are sent independently of the auth state.
- **Request-local cookies**: Request-local cookies (via `cookies=` kwarg) are not added to the persistent jar. They are stripped on cross-origin redirects.
- **Secure flag**: Cookies with `secure=true` are only sent over HTTPS.

### Findings

No critical or high-severity findings. Cross-origin credential stripping is comprehensive and covers all sensitive header types. Port changes are correctly treated as cross-origin. Redirect history is bounded.

---

## Proxy Review

**Scope**: HTTP proxying, HTTPS CONNECT tunneling, proxy authentication, NO_PROXY matching, and proxy response parsing.

### CONNECT Tunnel

- **Transparent byte stream**: After the CONNECT handshake, the tunnel is a transparent byte stream. eggfetch does not inspect, modify, or intercept tunnel content beyond the initial handshake.
- **No TLS interception**: eggfetch performs TLS through the tunnel but does not terminate TLS at the proxy. The proxy sees encrypted bytes.

### Proxy Authentication

- **Proxy-Authorization not forwarded**: The `Proxy-Authorization` header is consumed during the proxy handshake and never included in the request sent to the destination. This is enforced for both HTTP forward proxying and HTTPS CONNECT tunneling.
- **Proxy credentials from URL rejected**: URLs containing proxy credentials (e.g., `http://user:pass@proxy:8080/`) are rejected with a redacted error. Callers must use explicit `ProxyAuth` configuration.

### NO_PROXY Matching

- **Exact host match**: `localhost` matches only `localhost`.
- **Domain suffix match**: `.example.com` matches `foo.example.com` and `bar.example.com`.
- **Wildcard**: `*` matches all hosts.
- **Port-specific**: `localhost:8080` matches only port 8080.
- **Case-insensitive**: Host matching is case-insensitive per RFC.

### HTTP Forward Proxy

- **Absolute-form URI**: Requests through an HTTP proxy use absolute-form URI (e.g., `GET http://example.com/path HTTP/1.1`), per RFC 7230.
- **No credential embedding**: Proxy credentials are sent via `Proxy-Authorization`, not via URL userinfo.

### Proxy Response Parsing

- **Bounded parsing**: Proxy response status lines and headers are parsed with `httparse`, which enforces limits on line length and header count.
- **Header size limits**: The proxy response parser rejects responses with headers exceeding reasonable size limits.
- **Line limits**: Status lines are bounded to prevent resource exhaustion from oversized responses.

### Environment Variable Policy

- **Explicit core proxy**: the Rust core does not read proxy environment variables. The HTTPX compatibility facade explicitly opts into scheme-aware environment translation only when `trust_env=True`.
- **NO_PROXY available explicitly**: `NoProxy::from_env()` and `NoProxy::parse()` are available for callers who want to read environment variables explicitly.

### Findings

No critical or high-severity findings. Proxy credentials are correctly isolated from destination requests. CONNECT tunnels are properly opaque. Response parsing is bounded.

---

## Body Review

**Scope**: Request and response body handling, Content-Length processing, multipart encoding, path traversal, and decompression limits.

### Content-Length Handling

- **Server-trusted Content-Length**: Content-Length conflicts are resolved by trusting the server header, consistent with HTTP/1.1 semantics. This is the standard behavior for HTTP clients.
- **No double-counting**: The body stream is consumed exactly once. Content-Length is used for framing, not for allocating buffers.

### Multipart Encoding

- **Boundary validation**: Boundaries are validated via `Boundary::try_new()` to reject CR, LF, whitespace, and empty strings. This prevents header injection via multipart framing.
- **Random generation**: Random boundaries use 50 alphanumeric characters generated via `getrandom` (CSPRNG). The probability of collision is negligible.
- **Known-length calculation**: `Multipart::content_length()` uses checked arithmetic to sum all part lengths. Returns `None` when any part has unknown length, falling back to chunked transfer encoding.
- **Replayability check**: Multipart bodies are non-replayable when any part is a stream. Redirect behavior for 307/308 correctly rejects non-replayable multipart bodies.

### Path Traversal in Filenames

- **Basename only**: Filenames in multipart `Content-Disposition` headers are derived from the basename of the path, stripping directory components. This prevents path traversal via crafted filenames.
- **No filesystem access from filename**: The filename is a metadata field in the multipart header, not a filesystem path. eggfetch does not read or write files based on the filename in a multipart part.

### Decoded-Body Limits

- **max_decoded_body_size**: Hard limit on total decoded bytes. Enforced during streaming, not just on buffered reads. When exceeded, the response stream is terminated with `Error::DecodedBodyLimit`.
- **max_decompression_ratio**: Ratio limit comparing decoded to compressed bytes. Applied once enough input has been observed to make a meaningful comparison. Prevents zip-bomb attacks.
- **Both limits enforced in streaming path**: Memory usage stays bounded even for large responses because limits are checked per-chunk.

### Nested Compression

- **Single layer only**: eggfetch decompresses a single layer of content encoding (e.g., gzip, deflate, brotli, zstd). Nested compression (e.g., gzip inside gzip) is not supported. This is consistent with HTTP semantics where `Content-Encoding` is a single layer.

### Findings

No critical or high-severity findings. Multipart boundary validation is thorough and prevents injection. Path traversal via filenames is correctly mitigated. Decompression limits are enforced during streaming.

---

## Retry Review

**Scope**: Retry policy, method safety, body replayability, backoff, budget enforcement, and deadline handling.

### Method Safety

- **Idempotent by default**: Only GET, HEAD, OPTIONS, PUT, and DELETE are retried by default. POST and PATCH are not retried unless the caller explicitly opts in.
- **Explicit opt-in**: Callers can configure retry for any method, but the default policy is conservative.

### Body Replayability

- **Streaming body check**: Streaming request bodies (`RequestBody::Stream`) are not replayable. The retry subsystem checks `RequestBody::is_replayable()` before retrying.
- **Multipart replayability**: Multipart bodies are non-replayable when any part is a stream. The retry subsystem correctly handles this case.
- **307/308 redirect rejection**: Non-replayable request bodies are not forwarded on 307/308 redirects, consistent with the redirect engine's safety model.

### Retry Budget

- **Max retries**: Configurable via `RetryPolicy::max_retries()`. Default is a reasonable limit.
- **Backoff**: Exponential backoff with jitter is applied between retries. The backoff computation uses `Retry-After` headers when present.
- **Budget enforcement**: The retry budget is enforced per-request. Once the budget is exhausted, no further retries are attempted.

### Deadline Handling

- **No deadline extension**: The total timeout is not extended across retries. Each retry consumes time from the same deadline. This prevents retry amplification from extending the effective timeout.
- **Retry-After parsing**: `Retry-After` headers are parsed for both delay-second and HTTP-date formats. Malformed values are treated as zero delay.

### Secrets Across Retries

- **No credential leakage**: Credentials are carried in the request headers and are not logged or exposed during retry attempts. The retry subsystem operates on the request object without inspecting or logging credential values.

### Findings

No critical or high-severity findings. The retry policy is conservative and correctly handles body replayability. Deadline enforcement prevents retry amplification. Secret leakage across retries is prevented by the credential redaction system.
