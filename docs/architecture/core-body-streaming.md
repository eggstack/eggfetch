# Body & Streaming Deep Dive

This document covers request/response body types, streaming adapters, and the pool permit lifecycle.

See also: [overview.md](overview.md), [core-engine.md](core-engine.md).

## Request Body

`RequestBody` has three variants:

| Variant | Description |
|---------|-------------|
| `Empty` | No body (GET, HEAD, etc.) |
| `Bytes(Bytes)` | Fixed buffer — fully known length |
| `Stream { stream, length }` | Chunked upload — `length` is `Option<usize>` |

When `length` is `Some(n)`, the body is sent with `Content-Length`. When `None`, hyper selects a safe transfer mode (e.g., chunked transfer encoding for HTTP/1.1).

With the opt-in `json` feature, `RequestBuilder::json()` serializes once into
the replayable `Bytes` variant. It sets `Content-Type: application/json` only
when no content type was supplied. Body setters follow last-call-wins
semantics; replacing JSON later does not remove its already-set content type.

### Replayability

`RequestBody::try_clone_for_redirect()` is used during redirect following. `Bytes` bodies are replayed by cloning. `Stream` bodies are non-replayable — 307/308 redirects with stream bodies return `Error::BodyNotReplayableForRedirect`.

### Streaming Request Behavior

Stream bodies are wrapped in a hyper `StreamBody` and piped to the transport incrementally. The producer stream is polled lazily: each chunk is sent as soon as it is produced, so a slow producer backpressures the transport.

### Native frame-preserving interoperability

Transport-oriented Rust embedders can use
`Client::execute_http_body(http::Request<B>, NativeRequestOptions)` with any
`B: http_body::Body<Data = bytes::Bytes>`. The request body is erased only at
the existing Hyper boundary, so DATA and trailer frames remain frames and
backpressure/cancellation are preserved. No body is collected to infer a
length, and the body is one-shot.

The result is an ordinary `http::Response<NativeResponseBody>`. Its opaque
eggfetch-owned body implements `http_body::Body<Data = Bytes, Error = Error>`
and forwards DATA/trailer frames while holding the logical pool lease until
EOF, error, or drop. Hyper's `Incoming` is not part of the public contract.
Read timeouts start when the caller first polls the body and reset after each
frame; the absolute native total deadline starts with the logical request,
never resets on DATA/trailer frames, and can already be expired on first
poll (Total wins when already expired or tied). Delaying body consumption
after receiving headers therefore does not consume the read budget but can
exhaust the total budget. Established transport I/O inactivity remains the
lower-level lifecycle control.

This native surface deliberately does not apply high-level redirects,
logical retries, cookies, auth, decompression, or decoded-body limits. The
current high-level proxy and experimental H3 routes reject native frame
execution before body transfer because they do not expose the same frame
contract. A 101 upgrade is rejected after response headers identify the
status; it cannot be rejected earlier without knowing the response status.
Existing `RequestBody`, `ResponseBody`, `bytes_stream()`, and
`Response::trailers()` semantics are unchanged.

`Client::native_service()` is an ergonomic wrapper over this same native
execution boundary. It implements the standard
`tower_service::Service<http::Request<B>>` trait without buffering request
bodies or flattening response frames. `poll_ready()` always returns ready;
the request-scoped origin pool permit and transport backpressure are acquired
inside `call()` and released by the same response-body lifecycle. The service
does not forward arbitrary request extensions as a Hyper contract and does
not apply high-level request policy. The full Tower framework is not a core
dependency.

## Response Body

`ResponseBody` has four variants:

| Variant | Description |
|---------|-------------|
| `Buffered { bytes }` | Collected body — fully in memory |
| `Streaming { stream, lease }` | Live chunk stream (`BoxBytesStream`) with optional pool permit (`Option<PoolGuardArc>`). Public shape is frozen: no timeout fields. |
| `EncodedStreaming { stream, lease, content_encoding, limit }` | Encoded source for streaming compressed responses; first body-consuming operation selects decoded vs raw mode one-shot. Public shape is frozen. |
| `Consumed` | Body already consumed — second access returns error |

The published `ResponseBody` field shapes are a compatibility contract
(external crates may match them exhaustively without `..`; see
`tests/response_body_public_shape.rs`). Read/total timeout state never
appears as public variant fields; it travels behind the private
`PoolGuard` response lifecycle (see below).

### Single-Consumption Semantics

Body types are single-consume:
- `bytes_stream()` on a streaming body replaces it with `Consumed`.
- `bytes()` on a consumed body returns an error.
- Calling `bytes()` or `text()` on a streaming body consumes it (transitions to `Consumed`).

This prevents accidental double-reads and enforces ownership transfer.

With the opt-in `json` feature, `Response::json()` consumes the response through
the same `bytes()` path and then deserializes with `serde_json`. Decompression,
decoded-size limits, timeouts, pool-lease release, and single-consumption
behavior therefore remain unchanged; parse errors do not expose the payload.

### LeasedResponseStream

Streaming responses carry an internal `Arc<PoolGuard>` (the `PoolGuardArc`). This holds the pool permits acquired for the request plus the private
response read/total lifecycle policy installed at finalization. Permits are released when:
- The response body is fully consumed.
- The body stream reaches a terminal timeout/error/EOF, without waiting for drop.
- The response body is dropped.

Buffered and already-consumed responses do not carry a lease. Manually
constructed lease-free bodies carry no implicit client timeout.

This ensures per-origin logical-request limits remain meaningful while response bodies are in flight. Dropping early releases the permit without waiting for trailers.

### Trailers (`SharedTrailers`, `Response::trailers()`)

Hyper yields trailers as a final `Frame::trailers` (H1 chunked trailers, H2 trailing HEADERS); H3 trailing headers come from `recv_trailers()` after data EOF. `wrap_incoming` stores them in a shared `Arc<Mutex<Option<HeaderMap>>>` without buffering the body; the H3 unfold does the same. `Response::trailers()` clones them after EOF and is `None` until arrival, on no-trailers, or on pre-trailer body errors (errors stay body errors). `SharedTrailers::store()` is first-write-wins; duplicate trailer fields are preserved as received (`HeaderMap::get_all`). Read timeouts apply while waiting for trailers at the body boundary, and the absolute total deadline spans trailer completion (delayed trailers past total report `Total` with trailers `None`). Python/FFI/Node defer exposure; the HTTPX facade is unchanged.

## BoxBytesStream

The universal stream type:

```rust
pub type BoxBytesStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>;
```

Used for both request and response streaming. The `stream` module provides two wrapper adapters:

### BodyTimeoutStream

Wraps a `BoxBytesStream` and is the single authoritative high-level
response timeout owner. The per-chunk read timeout yields
`Error::Timeout { phase: Read }` when no chunk arrives in time and resets
on every chunk arrival; its timer starts on first body poll. The absolute
native total deadline yields `Error::Timeout { phase: Total }`, never
resets, can already be expired on first poll (winning before inner
transport is polled), and wins ties when both deadlines are observably
expired together. A ready chunk at or after the absolute deadline never
extends the request. After a timeout the stream fuses (next poll is EOF)
so the outer lease wrapper releases the pool permit; drop remains ordinary
cancellation. The former focused `ReadTimeoutStream` was removed once its
read-only coverage moved here.

The private read/total policy travels behind the `PoolGuard` lease, never
as public `ResponseBody` fields. `bytes()`, `bytes_stream()`,
`raw_bytes_stream()` (and therefore `text()`/`json()`) read that policy
from the lease and enforce it at the final stream boundary for the
selected mode, so raw encoded and decoded compressed paths share one
mechanism with the timeout outside the decoder.

### WriteTimeoutStream

Wraps a request body producer stream and enforces a per-chunk write timeout. If the producer does not yield the next chunk within the duration, yields `Error::Timeout { phase: Write }`. Only applies to streamed request bodies; buffered bodies complete synchronously. The per-chunk timer starts on first body poll, so connect/TLS/proxy setup is not charged to the first chunk.

## Python Streaming Surface

The Python bindings expose two consumption models:

| API | Behavior |
|-----|----------|
| `response.iter_bytes()` | Iterates over pre-buffered chunks |
| `client.stream().iter_bytes()` | Streams chunks from network (live) |

Streaming uses `StreamingResponse` with a four-state machine: `streaming` → `buffered`/`consumed` → `closed`. The GIL is released during network reads.

### Raw byte iterators

The compatibility facade's `iter_raw(chunk_size=None)` and `aiter_raw(chunk_size=None)` yield undecoded transport-level bytes, with bounded splitting/coalescing performed after each source chunk. Live compatibility streams become consumed before the first source read; normal exhaustion closes them, while partial iterator finalization and source failure remain distinguishable from explicit response close. The native Python `StreamingRawBytesIterator` and `AsyncStreamingRawBytesIterator` accept an optional chunk size and expose native source boundaries when available.

Compressed streaming responses use `ResponseBody::EncodedStreaming` internally. The encoded source remains single-owner until the first body-consuming operation selects one mode: `Response::raw_bytes_stream()` returns encoded bytes unchanged, while `bytes_stream()`, `bytes()`, and `text()` construct the existing decoder chain. The selection is mutually exclusive and one-shot; the private read/total policy behind the pool lease applies to the selected source. Python must not add a second decompressor or buffer/tee the body. Automatic decompression continues to remove `Content-Encoding` and `Content-Length` from core response headers. Core retains only the original values of those two wire headers in narrow read-only response metadata so the HTTPX compatibility facade can overlay them without changing core's decoded-header policy or deriving wire length from decoded bytes. Decoded `bytes_stream()` chunk boundaries are not stable across decoders/transports; tests compare ordered bytes rather than chunk counts. `Response::text_lines()` consumes the decoded path (`bytes_stream()`) and splits UTF-8 lines with a 1 MiB per-line cap (exceeding it reports `Error::Body`).

### Request streaming

Python-side request bodies support iterables, file-like objects, and custom `ByteStream` subclasses. These are bridged to `RequestBody::Stream` on the Rust side via bounded channels, preserving backpressure and GIL release during production.

## Resource Limits

- `max_decoded_body_size` — maximum bytes made available after decoding,
  including ordinary unencoded/identity responses. The same cap applies to
  buffered and streaming bodies. A wire `Content-Length` is useful for an
  early metadata check, but the streaming limit remains authoritative when
  the header is absent, incorrect, or describes encoded bytes.
- `max_decompression_ratio` — maximum ratio of decoded to compressed size.

Exceeding either limit yields `Error::DecodedBodyTooLarge` or
`Error::DecompressionRatioExceeded`; callers do not need a second manual
accumulation loop to enforce the decoded-size cap.
The client values can be overridden per request with
`RequestBuilder::max_decoded_body_size()` and
`RequestBuilder::max_decompression_ratio()`. Request overrides take
precedence and are retained across retries and redirects. `Response::json()`
uses these same limits through the ordinary decoded body path.
