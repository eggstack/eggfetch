# Error Reference

Complete reference for eggfetch error types across Rust, Python, and the CLI.

## Rust Error Variants

The `eggfetch_core::Error` enum is the single error type for the library. Use `error.kind()` to get a machine-readable category string.

| Variant | `kind()` | Description |
|---------|----------|-------------|
| `InvalidUrl` | `invalid_url` | URL could not be parsed |
| `InvalidMethod` | `invalid_method` | HTTP method is invalid |
| `InvalidHeaderName` | `invalid_header_name` | Header name contains invalid bytes |
| `InvalidHeaderValue` | `invalid_header_value` | Header value contains invalid bytes |
| `RequestBuild` | `request_build` | Request construction failed |
| `Connect` | `connect` | TCP connection failed |
| `Tls` | `tls` | TLS handshake or protocol error |
| `Protocol` | `protocol` | HTTP protocol error |
| `Body` | `body` | Body processing error |
| `Hyper` | `hyper` | Underlying hyper engine error |
| `HyperClient` | `hyper_client` | Hyper-util client error |
| `Io` | `io` | I/O error |
| `Unsupported` | `unsupported` | Feature not yet supported |
| `Pool` | `pool` | Connection pool error |
| `InvalidRedirectLocation` | `invalid_redirect_location` | Redirect Location header missing or invalid |
| `InvalidAuthHeader` | `invalid_auth_header` | Auth header value contains invalid bytes |
| `ConflictingAuth` | `conflicting_auth` | Multiple auth sources provided |
| `BodyNotReplayableForRedirect` | `body_not_replayable_for_redirect` | Streaming body cannot be resent on redirect |
| `TooManyRedirects` | `too_many_redirects` | Redirect chain exceeds max |
| `Decompression` | `decompression` | Decompression stream error |
| `UnsupportedContentEncoding` | `unsupported_content_encoding` | Content-Encoding not supported |
| `Timeout { phase }` | `timeout_{phase}` | Timeout elapsed (see phases below) |
| `InvalidProxyUrl` | `invalid_proxy_url` | Proxy URL is malformed |
| `ProxyConnect` | `proxy_connect` | Proxy connection failed |
| `ProxyAuthRequired` | `proxy_auth_required` | Proxy requires authentication |
| `ProxyConnectRejected` | `proxy_connect_rejected` | Proxy rejected CONNECT tunnel |
| `MalformedProxyResponse` | `malformed_proxy_response` | Proxy response could not be parsed |
| `DecodedBodyTooLarge` | `decoded_body_too_large` | Decompressed body exceeds size limit |
| `DecompressionRatioExceeded` | `decompression_ratio_exceeded` | Compression ratio exceeds limit |
| `TlsConfig` | `tls_config` | TLS configuration error |
| `CaBundle` | `ca_bundle` | CA certificate bundle could not be parsed |
| `ClientCert` | `client_cert` | Client certificate or key could not be loaded |
| `PrivateKey` | `private_key` | Private key could not be parsed or decrypted |
| `CertificateVerification` | `certificate_verification` | Server certificate verification failed |
| `HostnameVerification` | `hostname_verification` | Hostname did not match certificate |
| `BodyNotReplayableForRetry` | `body_not_replayable_for_retry` | Streaming body cannot be retried |
| `RetryBudgetExhausted` | `retry_budget_exhausted` | Retry attempts exceeded budget |
| `RetryNotConfigured` | `retry_not_configured` | Retry not enabled |
| `Http2GoAway` | `http2_go_away` | Server sent GOAWAY frame |
| `Http2StreamReset` | `http2_stream_reset` | Stream reset by server |
| `Http2FlowControl` | `http2_flow_control` | HTTP/2 flow-control violation |
| `Http2Protocol` | `http2_protocol` | HTTP/2 protocol error |
| `H3Connect` | `h3_connect` | HTTP/3 connection error |
| `H3ConnectionClosed` | `h3_connection_closed` | HTTP/3 connection closed by peer |
| `H3Stream` | `h3_stream` | HTTP/3 stream error |
| `H3Protocol` | `h3_protocol` | HTTP/3 protocol error |

### Timeout phases

The `Timeout` variant includes a phase discriminant. The `kind()` string includes the phase:

| Phase | `kind()` |
|-------|----------|
| Pool | `timeout_pool` |
| Connect | `timeout_connect` |
| Proxy Connect | `timeout_proxy_connect` |
| Proxy TLS | `timeout_proxy_tls` |
| Write | `timeout_write` |
| Read | `timeout_read` |
| Total | `timeout_total` |

## Python Exception Hierarchy

All exceptions are in the `eggfetch` module. The hierarchy is:

```
Exception
  EggfetchError
    RequestError
      InvalidUrl
      TimeoutException
        PoolTimeout
        ConnectTimeout
        ReadTimeout
        WriteTimeout
      NetworkError
      ProtocolError
      BodyError
      TooManyRedirects
      DecompressionError
      UnsupportedContentEncoding
      ProxyError
        ProxyConnectError
        ProxyAuthError
      BodyNotReplayableForRetry
      RetryBudgetExhausted
      RetryNotConfigured
      Http2Error
        Http2GoAway
        Http2StreamReset
        Http2FlowControlError
      H3Error
        H3ConnectError
        H3ProtocolError
    HTTPStatusError
    UnsupportedKwarg
    StreamConsumed
    StreamClosed
    ResponseNotRead
```

### Mapping from Rust errors

| Rust variant | Python exception |
|-------------|-----------------|
| `InvalidUrl` | `InvalidUrl` |
| `InvalidMethod`, `InvalidHeaderName`, `InvalidHeaderValue`, `RequestBuild`, `Unsupported`, `Pool`, `InvalidRedirectLocation`, `InvalidAuthHeader`, `ConflictingAuth`, `TlsConfig`, `CaBundle`, `ClientCert`, `PrivateKey` | `RequestError` |
| `Connect`, `Tls`, `CertificateVerification`, `HostnameVerification`, `Hyper`, `HyperClient`, `Io` | `NetworkError` |
| `Protocol` | `ProtocolError` |
| `Body` | `BodyError` |
| `Timeout` (Pool) | `PoolTimeout` |
| `Timeout` (Connect, ProxyConnect, ProxyTls) | `ConnectTimeout` |
| `Timeout` (Read) | `ReadTimeout` |
| `Timeout` (Write) | `WriteTimeout` |
| `Timeout` (Total) | `TimeoutException` |
| `Decompression` | `DecompressionError` |
| `UnsupportedContentEncoding` | `UnsupportedContentEncoding` |
| `InvalidProxyUrl` | `ProxyError` |
| `ProxyConnect` | `ProxyConnectError` |
| `ProxyAuthRequired` | `ProxyAuthError` |
| `ProxyConnectRejected` | `ProxyConnectError` |
| `MalformedProxyResponse` | `ProtocolError` |
| `DecodedBodyTooLarge`, `DecompressionRatioExceeded` | `RequestError` |
| `BodyNotReplayableForRetry` | `BodyNotReplayableForRetry` |
| `RetryBudgetExhausted` | `RetryBudgetExhausted` |
| `RetryNotConfigured` | `RetryNotConfigured` |
| `Http2GoAway` | `Http2GoAway` |
| `Http2StreamReset` | `Http2StreamReset` |
| `Http2FlowControl` | `Http2FlowControlError` |
| `Http2Protocol` | `Http2Error` |
| `H3Connect` | `H3ConnectError` |
| `H3ConnectionClosed`, `H3Protocol`, `H3Stream` | `H3ProtocolError` or `H3Error` |

## CLI Exit Codes

| Code | Constant | Meaning |
|------|----------|---------|
| 0 | `EXIT_SUCCESS` | Request completed successfully |
| 2 | `EXIT_USAGE` | Usage error: bad arguments, invalid URL, TLS config, cert errors |
| 3 | `EXIT_CONNECT` | Connection failed: TCP, TLS handshake, proxy connect |
| 4 | `EXIT_TIMEOUT` | A timeout was exceeded |
| 5 | `EXIT_PROTOCOL` | Protocol error: HTTP/2, decompression, redirect loop, body error |
| 6 | `EXIT_STATUS` | HTTP status error (triggered by `--check-status`) |
| 7 | `EXIT_IO` | I/O error reading or writing |
| 130 | -- | Interrupted by signal (Ctrl+C) |

## Common Error Scenarios

| Scenario | Error kind | Solution |
|----------|-----------|----------|
| Server certificate expired | `certificate_verification` | Provide correct CA bundle or fix server cert |
| Self-signed cert in dev | `certificate_verification` | Use `verify=False` / `--no-verify` for testing only |
| Connection refused | `connect` | Verify server is running on expected host:port |
| DNS resolution failure | `connect` | Check hostname spelling and network connectivity |
| Read timeout on large response | `timeout_read` | Increase read timeout or use streaming |
| Redirect loop | `too_many_redirects` | Increase `max_redirects` or disable redirects |
| Proxy auth failed | `proxy_auth_required` | Provide proxy credentials |
| Decompression bomb blocked | `decoded_body_too_large` | Increase `max_decoded_body_size` if legitimate |
| Streaming body cannot retry | `body_not_replayable_for_retry` | Use a buffered body for retryable requests |
| HTTP/2 stream refused | `http2_stream_reset` | Automatically retried if error is retryable |
| Pool slot unavailable | `pool` | Increase `max_connections` or reduce concurrency |
