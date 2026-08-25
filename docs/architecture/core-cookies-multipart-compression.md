# Cookies, Multipart & Compression Deep Dive

This document covers the cookie subsystem, multipart form data, and response decompression.

See also: [overview.md](overview.md), [core-engine.md](core-engine.md).

## Cookies

Feature-gated behind `cookies`.

### RFC 6265 Implementation

- **Parsing**: `Set-Cookie` response headers are parsed into `Cookie` structs.
- **Storage**: `CookieJar` is a thread-safe store (uses `DashMap` internally).
- **Matching**: domain/path matching per RFC 6265 §5.1.4.
- **Secure flag**: cookies with `secure=true` are only sent over HTTPS.
- **Expiry**: `Max-Age=0` or past `Expires` removes the cookie. Negative `Max-Age` is treated as zero.

### Cookie Attributes

| Attribute | Description |
|-----------|-------------|
| `name` | Cookie name |
| `value` | Cookie value |
| `domain` | Domain scope (with or without leading dot) |
| `path` | Path prefix |
| `secure` | HTTPS-only |
| `httpOnly` | Not accessible via JavaScript |
| `sameSite` | `Strict`, `Lax`, or `None` |
| `expires` | Absolute expiry time |
| `maxAge` | Relative expiry from creation |

### Host-Only vs Domain

Cookies set without a `Domain` attribute are host-only — not sent to subdomains. Cookies with an explicit `Domain` are sent to the specified domain and all subdomains.

### Client Integration

Each `Client` owns a `CookieJar`. On each request, matching cookies are computed and added to the `Cookie` header. `Set-Cookie` responses are ingested before redirect hops.

### Python API

```python
# Client-level cookies
client = eggfetch.Client(cookies={"session": "abc123"})

# Request-local cookies (not persisted in jar)
r = client.get(url, cookies={"csrf": "token"})

# Response cookies
print(r.cookies)
```

## Multipart

Feature-gated behind `multipart`.

### Core Types

| Type | Description |
|------|-------------|
| `Multipart` | Owns a `Boundary` and a list of `Part`s |
| `Part` | Single part: name, filename, content_type, headers, body |
| `PartBody` | `Bytes(Bytes)` or `Stream { stream, length }` |
| `Boundary` | Validated multipart boundary string |
| `MultipartEncoder` | Streaming `Stream<Item = Result<Bytes>>` state machine |

### Encoder State Machine

```
PartHeader → PartBody → TrailingCrlf → PartHeader → ... → FinalBoundary → Done
```

The encoder preserves backpressure: a slow part body stream naturally backpressures the encoder.

### Known-Length Calculation

`Multipart::content_length()` uses checked arithmetic to sum all parts. Returns `Some(u64)` only when every part has a known length; returns `None` for streaming parts.

### Replayability

A multipart body is replayable only when all parts are `PartBody::Bytes`. Stream parts make it non-replayable.

### Python API

```python
# Simple file upload
eggfetch.post(url, files={"file": b"data"})

# With metadata
eggfetch.post(url, files={"file": ("report.txt", b"contents", "text/plain")})

# Mixed form + files
eggfetch.post(url, data={"description": "sample"}, files={"file": open("data.bin", "rb")})

# Path-backed
eggfetch.post(url, files={"file": eggfetch.File("/path/to/file.pdf")})
```

## Compression

Feature-gated behind `compression-gzip`, `compression-brotli`, `compression-zstd`, `compression-deflate`.

### Supported Codings

| Coding | Feature | Implementation |
|--------|---------|----------------|
| gzip | `compression-gzip` | `async-compression` (streaming), `flate2` (buffered) |
| deflate | `compression-deflate` | `async-compression` (streaming), `flate2` (buffered) |
| brotli | `compression-brotli` | `async-compression` |
| zstd | `compression-zstd` | `async-compression` |

### Accept-Encoding Negotiation

`accept_encoding_value()` generates the `Accept-Encoding` header based on enabled features. `ContentCoding` enum identifies the response encoding.

### Resource Limits

| Limit | Default | Error |
|-------|---------|-------|
| `max_decoded_body_size` | Configurable | `DecodedBodyTooLarge` |
| `max_decompression_ratio` | Configurable | `DecompressionRatioExceeded` |
| Nesting depth | 4 | Prevents zip-bomb chains |

### Decompression Modes

- **Streaming**: `decompress_stream()` wraps the response body stream with an async decoder.
- **Buffered**: `decompress_buffered()` decodes a collected `Bytes` buffer synchronously via `flate2`.

Automatic decompression removes `Content-Encoding` and `Content-Length` from
the visible core response headers. The response retains only the original
wire values of those two headers as narrow read-only metadata for adapters
that need HTTPX-compatible visibility; the values are never inferred from
decoded body length. The compatibility facade overlays that metadata while
core remains authoritative for decoder selection.

### Response Decode Pipeline (`response_decode`)

The crate-internal `response_decode` module is the single dispatch point where decompression is applied to a completed response. `pipeline::send_single_request` calls it after transport dispatch (step 10 of the lifecycle in [core-engine.md](core-engine.md)).

Entry point: `apply_decompression(response, content_encoding, limit)`:

1. **Validate first**: unsupported `Content-Encoding` values fail with `UnsupportedContentEncoding` before any body transformation.
2. **Transform the body** according to its variant:
   - `Streaming` + encoding → `ResponseBody::EncodedStreaming`. When the streaming body carries a pool lease, it is attached directly to the encoded wrapper so lease ownership never routes through a destructure/rebuild round-trip.
   - `Buffered` + non-empty bytes → decoded synchronously via `decompress_buffered()`.
   - Empty buffered bodies and already-encoded bodies pass through unchanged.
3. **Strip visible headers**: `Content-Encoding` and `Content-Length` are removed from the header map; their original wire values remain available via `Response::wire_content_encoding()` / `wire_content_length()` (see above).

The empty-body special case matters: a zero-length body with a `Content-Encoding` header is left untouched rather than fed through a decoder, matching how encoders emit empty payloads.

### Python API

```python
# Automatic decompression (default)
r = client.get(url)

# Disable decompression
r = client.get(url, decompress=False)
```
