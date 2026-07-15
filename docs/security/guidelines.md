# Security Guidelines

This document covers the security model of eggfetch across the Rust core, Python bindings, and CLI.

## TLS Verification

eggfetch verifies server certificates and hostnames by default using the system trust store. Never disable certificate verification in production.

### Disabling verification

The escape hatches exist for local development and testing only:

- **CLI**: `--no-verify` flag
- **Python**: `Client(verify=False)` or `AsyncClient(verify=False)`
- **Rust**: `TlsConfig::builder().danger_accept_invalid_certs(true)`

When verification is disabled, the client skips certificate chain validation, hostname verification, and SNI checks. Any active network attacker can intercept traffic. The Python `Client.__repr__` prints a `[UNSAFE: TLS verification disabled]` warning when `verify=False` is set.

### Custom CA bundles

For corporate proxies or private certificate authorities, provide a custom CA bundle instead of disabling verification:

- **Python**: `Client(verify="/path/to/ca-bundle.crt")`
- **CLI**: `--ca-bundle /path/to/ca-bundle.crt`
- **Rust**: `TlsConfig::builder().ca_bundle_pem(path)`

### TLS version pinning

The minimum and maximum TLS versions can be restricted:

```rust
TlsConfig::builder()
    .min_tls_version(TlsVersion::TLS_1_3)
    .max_tls_version(TlsVersion::TLS_1_3)
```

The default allows TLS 1.2 and 1.3. Restricting to 1.3 only may break compatibility with older servers.

## Credential Handling

### Avoid credentials in CLI arguments

Command-line arguments are visible to other users on the same machine via `ps` or `/proc`. Pass credentials through environment variables or flags that read from files rather than embedding them in the command line.

### Secret redaction

eggfetch redacts sensitive values in all diagnostic output:

- `Debug` and `Display` for `BasicAuth` and `BearerAuth` show `<redacted>` instead of passwords or tokens.
- `Debug` and `Display` for `ProxyAuth` redact the password.
- `Response` debug output replaces `Authorization`, `Proxy-Authorization`, `Cookie`, and `Set-Cookie` header values with `<redacted>`.
- `Debug` for response URLs strips the username, password, query string, and fragment.

This redaction is applied at the type level, so secrets never appear in logs, error messages, or formatted output regardless of how they are printed.

### Redirect credential stripping

On cross-origin redirects, eggfetch strips the `Authorization`, `Cookie`, and `Proxy-Authorization` headers from the redirected request. This prevents credential leakage to third-party origins. The redirect engine determines origin by comparing the scheme, host, and port of the original and redirect URLs.

## Decompression Bomb Protection

Compressed responses can expand to enormous sizes, allowing denial-of-service attacks through decompression bombs. eggfetch enforces two limits during streaming decompression:

- **max_decoded_body_size**: Maximum uncompressed bytes the client will accept. Configurable via `ClientBuilder::max_decoded_body_size()` or CLI `--max-body-size`.
- **max_decompression_ratio**: Ratio of compressed to uncompressed output. Configurable via `ClientBuilder::max_decompression_ratio()` or CLI `--max-decompression-ratio`.

When either limit is exceeded, the response stream is terminated with `Error::DecodedBodyTooLarge` or `Error::DecompressionRatioExceeded`. Both limits are enforced during streaming, so memory usage stays bounded even for large responses.

## Proxy Environment Policy

eggfetch does **not** read `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, or `NO_PROXY` environment variables. Proxy configuration is explicit only, set via `ClientBuilder::proxy()` or the `--proxy` CLI flag. This avoids surprising behavior when multiple proxy-aware libraries coexist in the same process.

The `NO_PROXY` matching rules are available through `NoProxy::from_env()` or `NoProxy::parse()` for explicit configuration. `NoProxy` supports wildcard matching, exact host match, and domain suffix matching (e.g., `.example.com`).

`Proxy-Authorization` headers are never forwarded to the destination server. Credentials are only sent to the proxy itself during the CONNECT handshake.

## Cookie Security

The cookie subsystem enforces standard security attributes:

- **Secure**: Cookies with the `Secure` attribute are only sent over HTTPS connections. Plain HTTP requests never carry secure cookies.
- **HttpOnly**: The `http_only` flag is parsed and stored; the flag is available for inspection but eggfetch does not expose cookies to JavaScript (there is no DOM).
- **SameSite**: `Strict`, `Lax`, and `None` values are parsed and available for matching logic.
- **Domain and path matching**: Follows RFC 6265 domain matching and path prefix rules.
- **Host-only cookies**: Set when no `Domain` attribute is present; these only match the exact host.
- **Expiration**: Persistent cookies with an `Expires` or `Max-Age` attribute are stored and expired automatically. Stale cookies can be purged with `CookieJar::expire_stale()`.

## Client Certificates

Client certificates (mTLS) are supported via `TlsConfig::builder().client_certificate_pem(cert_path, key_path)`. The private key is parsed and held in memory only; it is not logged or included in error messages beyond the key-label identifier. The CLI accepts client certificates via `--client-cert` and `--client-key` flags.

## Dependency Audit

eggfetch tracks Rust advisory databases and runs `cargo audit` as part of CI. Python dependencies in the test harness are pinned and audited. The workspace uses `forbid(unsafe_code)` so no `unsafe` blocks are introduced without explicit review.

## Supported Versions

eggfetch supports the current and previous minor release. Security fixes are backported to the supported window. Older versions receive no security patches.

## Reporting Vulnerabilities

Report security vulnerabilities privately to the maintainers through the project's GitHub Security Advisories page. Do not open public issues for security bugs. Include a reproduction case and note which version is affected. Acknowledgments are given to reporters who follow responsible disclosure practices.
