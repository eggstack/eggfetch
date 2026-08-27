# Compression and Decompression

eggfetch transparently decompresses response bodies based on `Content-Encoding` headers. Supported formats are gzip, deflate, brotli, and zstd, each behind a feature flag.

## Supported Formats

| Encoding | Wire name | Feature flag |
|----------|-----------|-------------|
| gzip | `gzip`, `x-gzip` | `compression-gzip` |
| deflate | `deflate` | `compression-deflate` |
| Brotli | `br` | `compression-brotli` |
| Zstandard | `zstd` | `compression-zstd` |

The Python crate enables all compression features by default. When building eggfetch-core directly, only the features you enable are available.

## Automatic Decompression

When automatic decompression is enabled (the default when compression features are compiled in), the client:

1. Advertises supported encodings in `Accept-Encoding`
2. Decodes compressed response bodies transparently
3. Strips `Content-Encoding` and `Content-Length` from decoded headers

If the server responds with an unsupported encoding and decompression is enabled, an `UnsupportedContentEncoding` error is returned. If decompression is disabled, raw bytes pass through unchanged.

## Accept-Encoding Negotiation

The `Accept-Encoding` header is automatically generated based on compiled-in features. The encoding order matches mainstream client defaults (most common first). For example, with gzip and brotli enabled:

```
Accept-Encoding: gzip, deflate, br
```

Without any compression features compiled, no `Accept-Encoding` header is sent.

## Content-Encoding Parsing

`Content-Encoding` headers with multiple encodings (e.g., `gzip, br`) are supported. Decoders are applied in reverse order: the innermost encoding is decoded first. For example, `Content-Encoding: gzip, br` means decode brotli first, then gzip.

Nesting depth is limited to 4 layers to prevent stack overflow from adversarial multi-layer compression. Encodings beyond the limit produce a `Decompression` error.

## Streaming Decompression

The primary decompression path wraps the response body stream in a decoder chain. Each decoder reads compressed chunks from the inner stream and produces decompressed output. This approach:

- Does not buffer the entire response body in memory
- Preserves backpressure from the network to the decoder
- Supports arbitrarily large responses

The decoder chain is built lazily: the first poll triggers decoding of the first chunk.

## Buffered Decompression

For response bodies that have already been collected into memory, synchronous decompression is used via `flate2` for gzip and deflate. Brotli and zstd are only supported through the streaming path.

Buffered decompression is used when `response.bytes()` or `response.text()` is called on a response that has already been read.

## Decompression-Bomb Limits

Protect against decompression bombs with resource limits:

```rust
let client = Client::builder()
    .max_decoded_body_size(10 * 1024 * 1024)  // 10 MB
    .max_decompression_ratio(20.0)             // 20x expansion
    .build();
```

| Limit | Description |
|-------|-------------|
| `max_decoded_body_size` | Hard limit on total decoded bytes, including unencoded responses |
| `max_decompression_ratio` | Ratio of decoded to compressed bytes after which decompression is rejected |

Both limits are checked during streaming response consumption. If either is exceeded, a `DecodedBodyTooLarge` or `DecompressionRatioExceeded` error is returned. The ratio limit prevents zip bombs where a small compressed payload expands to enormous size.

## Disabling Decompression

To receive raw compressed bytes without decoding:

```rust
let client = Client::builder()
    .automatic_decompression(false)
    .build();
```

```python
response = client.get(url, decompress=False)
```

When decompression is disabled, the `Content-Encoding` header is preserved and the body is returned as-is.

## Python API

| Kwarg | Description |
|-------|-------------|
| `decompress=True` | Enable automatic decompression (default) |
| `decompress=False` | Disable decompression, return raw bytes |

```python
import eggfetch

# Default: automatic decompression
response = client.get("https://example.com")

# Disable for this request
response = client.get("https://example.com", decompress=False)
```

## CLI

```bash
# Disable compression
eggfetch --no-compress https://example.com

# Set body size limit
eggfetch --max-body-size 10485760 https://example.com

# Set decompression ratio limit
eggfetch --max-decompression-ratio 20.0 https://example.com
```
