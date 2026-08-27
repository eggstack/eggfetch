# Body & Streaming Deep Dive

This document covers request/response body types, streaming adapters, and the pool permit lifecycle.

See also: [overview.md](overview.md), [core-engine.md](core-engine.md).

## Request Body

`RequestBody` has three variants:

| Variant | Description |
|---------|-------------|
| `Empty` | No body (GET, HEAD, etc.) |
| `Bytes(Bytes)` | Fixed buffer — fully known length |
| `Stream { stream, length }` | Chunked upload — `length` is `Option<u64>` |

When `length` is `Some(n)`, the body is sent with `Content-Length`. When `None`, hyper selects a safe transfer mode (e.g., chunked transfer encoding for HTTP/1.1).

### Replayability

`RequestBody::try_clone_for_redirect()` is used during redirect following. `Bytes` bodies are replayed by cloning. `Stream` bodies are non-replayable — 307/308 redirects with stream bodies return `Error::BodyNotReplayableForRedirect`.

### Streaming Request Behavior

Stream bodies are wrapped in a hyper `StreamBody` and piped to the transport incrementally. The producer stream is polled lazily: each chunk is sent as soon as it is produced, so a slow producer backpressures the transport.

## Response Body

`ResponseBody` has three variants:

| Variant | Description |
|---------|-------------|
| `Buffered(Bytes)` | Collected body — fully in memory |
| `Streaming(LeasedResponseStream)` | Live chunk stream with pool permit |
| `Consumed` | Body already consumed — second access returns error |

### Single-Consumption Semantics

Body types are single-consume:
- `bytes_stream()` on a streaming body replaces it with `Consumed`.
- `bytes()` on a consumed body returns an error.
- Calling `bytes()` or `text()` on a streaming body consumes it (transitions to `Consumed`).

This prevents accidental double-reads and enforces ownership transfer.

### LeasedResponseStream

Streaming responses carry an internal `Arc<PoolGuard>` (the `PoolGuardArc`). This holds the pool permits acquired for the request. Permits are released when:
- The response body is fully consumed.
- The response body is dropped.
- The response body is explicitly closed.

Buffered and already-consumed responses do not carry a lease.

This ensures per-origin concurrency limits remain meaningful while response bodies are in flight.

## BoxBytesStream

The universal stream type:

```rust
pub type BoxBytesStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>;
```

Used for both request and response streaming. The `stream` module provides two wrapper adapters:

### ReadTimeoutStream

Wraps a `BoxBytesStream` and enforces a per-chunk read timeout. If no chunk arrives within the configured duration, yields `Error::Timeout { phase: Read }`. The deadline resets on every chunk arrival.

### WriteTimeoutStream

Wraps a request body producer stream and enforces a per-chunk write timeout. If the producer does not yield the next chunk within the duration, yields `Error::Timeout { phase: Write }`. Only applies to streamed request bodies; buffered bodies complete synchronously.

## Python Streaming Surface

The Python bindings expose two consumption models:

| API | Behavior |
|-----|----------|
| `response.iter_bytes()` | Iterates over pre-buffered chunks |
| `client.stream().iter_bytes()` | Streams chunks from network (live) |

Streaming uses `StreamingResponse` with a four-state machine: `streaming` → `buffered`/`consumed` → `closed`. The GIL is released during network reads.

### Raw byte iterators

The compatibility facade's `iter_raw(chunk_size=None)` and `aiter_raw(chunk_size=None)` yield undecoded transport-level bytes, with bounded splitting/coalescing performed after each source chunk. Live compatibility streams become consumed before the first source read; normal exhaustion closes them, while partial iterator finalization and source failure remain distinguishable from explicit response close. The native Python `StreamingRawBytesIterator` and `AsyncStreamingRawBytesIterator` accept an optional chunk size and expose native source boundaries when available.

Compressed streaming responses use `ResponseBody::EncodedStreaming` internally. The encoded source remains single-owner until the first body-consuming operation selects one mode: `Response::raw_bytes_stream()` returns encoded bytes unchanged, while `bytes_stream()`, `bytes()`, and `text()` construct the existing decoder chain. The selection is mutually exclusive and one-shot; the read-timeout wrapper and pool lease remain attached to the selected source. Python must not add a second decompressor or buffer/tee the body. Automatic decompression continues to remove `Content-Encoding` and `Content-Length` from core response headers. Core retains only the original values of those two wire headers in narrow read-only response metadata so the HTTPX compatibility facade can overlay them without changing core's decoded-header policy or deriving wire length from decoded bytes.

### Request streaming

Python-side request bodies support iterables, file-like objects, and custom `ByteStream` subclasses. These are bridged to `RequestBody::Stream` on the Rust side via bounded channels, preserving backpressure and GIL release during production.

## Resource Limits

- `max_decoded_body_size` — maximum decoded body size, including identity
  and other unencoded responses.
- `max_decompression_ratio` — maximum ratio of compressed to decoded size.

Exceeding either limit yields `Error::DecodedBodyTooLarge` or `Error::DecompressionRatioExceeded`.
