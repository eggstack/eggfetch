# Request Lifecycle

Every request through eggfetch follows the same pipeline. Understanding this order explains where headers appear, when cookies are attached, and how redirects interact with auth.

## Pipeline Overview

The lifecycle has 14 steps. Steps 3 through 12 repeat on each redirect hop. The total timeout spans the entire lifecycle including all redirects and retries.

## Pipeline Order

### 1. URL Parsing and Validation

The URL is parsed and validated. Invalid schemes, missing hosts, or malformed URLs produce errors before any network I/O occurs. Only `http` and `https` schemes are supported.

### 2. Request Building

Headers, body, auth, cookies, proxy, timeout, and redirect policy are accumulated on the `RequestBuilder`. Body kwargs (`content`, `data`, `json`) are mutually exclusive. `files` may combine with `data` to form multipart. Query parameters are appended to the URL. Headers are validated (no empty names, no bare CR/LF).

### 3. Cookie Selection

Matching client-jar cookies are computed for the current destination. Cookies are selected by domain (including subdomains for domain cookies) and path prefix. Secure cookies are only sent over HTTPS. Expired cookies are removed before matching.

### 4. Auth Resolution

The precedence chain resolves which authentication to apply:

1. Request-level explicit auth (via `.auth()`)
2. Request-level auth disabled (via `.without_auth()`)
3. Client-level auth (via `ClientBuilder::auth()`)
4. No auth

If a raw `Authorization` header is set and auth is also configured, an error is raised to prevent ambiguity. This ensures there is always exactly one source of authentication truth.

### 5. Validation and Send

Body length and headers are validated. The request is serialized and sent over the transport. For known-length bodies, `Content-Length` is set. For stream bodies with unknown length, the transport selects a safe transfer mode (chunked for HTTP/1.1).

### 6. Connection Acquisition

A pool slot is acquired for the request's origin. If no slot is available and a concurrency limit is configured, the request waits (subject to the pool timeout). Under HTTP/2, multiple streams share a single connection, but pool permits still control logical concurrency. The pool timeout fires if no slot becomes available within the configured duration.

### 7. TLS Handshake (HTTPS)

For HTTPS connections, a TLS handshake occurs after TCP connection. This includes DNS resolution, TCP connection establishment, SNI, certificate chain verification, and hostname verification. The connect timeout covers the entire sequence from DNS through TLS completion.

### 8. Proxy CONNECT (if proxied)

When a proxy is configured for HTTPS targets, a CONNECT tunnel is established through the proxy. The tunnel is a transparent byte stream; the proxy cannot inspect the encrypted traffic. For HTTP targets through a proxy, the request is forwarded directly with the full URL in the request line.

### 9. Request Serialization and Sending

The request line, headers, and body are written to the transport. For streaming bodies, chunks are sent incrementally as they are produced, with no eager buffering. A slow body producer backpressures the transport naturally. The write timeout applies per-chunk for stream bodies.

### 10. Response Reading

Response status, headers, and body are read. The body starts as a live stream. Calling `bytes()` or `text()` buffers the entire body; calling `bytes_stream()` returns an iterator over chunks. The read timeout applies per-chunk, resetting on every arrival.

### 11. Decompression (if enabled)

If the response has a `Content-Encoding` header and automatic decompression is enabled, the body stream is wrapped in a decoder chain. Gzip, deflate, brotli, and zstd are supported. The `Content-Encoding` and `Content-Length` headers are stripped from decoded responses. Decompression-bomb limits are enforced during streaming.

### 12. Redirect Handling (if enabled)

If the response is a redirect (301, 302, 303, 307, 308) and redirect following is enabled, the client builds a follow-up request. Method rewriting, sensitive header stripping, and body replayability checks happen here. The pipeline restarts from step 3 for the new destination. Redirects consume one of the `max_redirects` budget.

### 13. Retry (if configured)

If the response matches the retry policy (retryable status code, transport error, or timeout), the entire logical request restarts from step 2 under the original total deadline. Each retry checks method safety, body replayability, and the retry budget. Backoff delays are applied between attempts.

### 14. Body Consumption

The response body is returned to the caller. The pool permit is released when the body is fully consumed or dropped. For buffered responses, the permit is released immediately. For streaming responses, the permit is held until the stream is consumed or dropped.

## Redirect Loop

On redirect, steps 3 through 12 repeat for the new destination URL. The total timeout applies across all hops. Client-level auth is not reapplied on cross-origin hops. Cookies are recomputed for the new destination. Each redirect hop records metadata in the response's `history` field.

## Error Phases

Errors carry phase identity so callers know which part of the lifecycle failed:

- **URL validation** -- invalid URL before any network I/O
- **Pool** -- waiting for a concurrency slot
- **Connect** -- DNS, TCP, or TLS failure
- **Proxy connect** -- proxy connection or tunnel failure
- **Write** -- request body send timeout
- **Read** -- response body timeout
- **Timeout (total)** -- wall-clock deadline exceeded
- **Protocol** -- HTTP parsing, decompression, or body limit errors

## Example: Full Lifecycle for a GET Request

Here is what happens when you call `client.get("https://example.com/api")`:

1. The URL is parsed into scheme, host, port, and path
2. Client-level headers (User-Agent, etc.) are merged with request-level headers
3. Client-jar cookies for `example.com` are computed
4. Client-level auth (if configured) is resolved
5. A pool slot for `https://example.com:443` is acquired
6. TCP connection to `example.com:443` is established
7. TLS handshake completes (SNI, certificate verification)
8. Request line `GET /api HTTP/1.1` and headers are sent
9. Response status and headers are received
10. Response body chunks are read (or buffered entirely)
11. If compressed, the body is decompressed transparently
12. The response is returned with status, headers, and body

The total time for this sequence is bounded by the configured timeouts at each phase.
