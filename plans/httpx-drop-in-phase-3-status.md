# HTTPX Drop-In Phase 3: Streaming and Bodies — Status

Status: COMPLETE (with gap fixes applied)

## Deliverables

### Track A — Unified body model
- [x] SyncByteStream and AsyncByteStream base classes
- [x] ByteStream concrete implementation
- [x] Custom stream class support
- [x] ReplayClass enum (Immutable, Seekable, OneShot, Consumed)

### Track B — Python request streaming
- [x] Sync iterable content support (via native streaming engine)
- [x] Async iterable content support
- [x] File-like content support (incremental reads via wrapper generator)
- [x] Custom stream classes
- [x] Generators and iterables passed as content= are streamed lazily

### Track C — Multipart and form bodies
- [x] Multipart passthrough to native encoder
- [x] File path, file object, byte stream support
- [x] Tuple and value forms

### Track D — Response streaming parity
- [x] iter_raw() / aiter_raw() for undecoded transport bytes
- [x] iter_bytes() / aiter_bytes() with chunk size
- [x] iter_text() / aiter_text() with chunk size
- [x] iter_lines() / aiter_lines() with chunk size
- [x] read() / aread() for full body consumption
- [x] Chunk size parameter on all iterators (default 8192)
- [x] IncrementalDecoder for multibyte character safety

### Track E — Runtime and bridge architecture
- [x] Shared sync runtime (single Tokio runtime via OnceLock, all iterators spawn tasks on it)
- [x] Bounded channel bridges (sync_channel(16) for sync, mpsc(16) for async)
- [x] GIL release during network I/O

### Track F — Timeout, cancellation, and close semantics
- [x] Stream cancel via watch::Sender
- [x] Close/drain_all_bytes semantics
- [x] Context manager support (sync and async)
- [x] Context-manager __exit__ discards unread body (HTTPX-compatible)

### Track G — Redirect and retry integration
- [x] Body replay decisions via RequestBody::try_clone_for_redirect()
- [x] Stream bodies correctly rejected for redirects
- [x] ReplayClass enum for structured replayability classification

### Track H — Differential and validation
- [x] 64 streaming tests passing (native API)
- [x] 460 compat tests passing (including 14 resource/failure/stress tests)
- [x] Stream state machine (STREAMING→BUFFERED→CONSUMED→CLOSED)
- [x] Resource ownership tests (file lifecycle)
- [x] Producer/consumer failure tests
- [x] Thread envelope tests (concurrent streams don't leak threads)
- [x] Reference stream server tests (lines, large body, async variants)

## Gap Fixes Applied

### E1: Shared sync runtime
All sync iterators now spawn tasks on a single shared Tokio runtime
(`OnceLock<Runtime>`) instead of creating a new OS thread + Tokio runtime
per stream. Producer tasks are aborted on iterator drop.

### B1/B2: Lazy request streaming
When `content=` receives a Python iterable/generator, it is passed directly
to the native Rust client as a stream body. The Rust adapter detects
iterables and creates a `RequestBody::Stream` via
`python_iterable_to_request_body()`. Iteration happens during HTTP send,
not before.

### B3: File-like body support
`content=` now detects file-like objects (has `.read` method) and wraps them
in an incremental reader generator. Files are read in 8KB chunks.

### F3: Context-manager discard
`__exit__` / `__aexit__` now drop the stream without draining, matching
HTTPX behavior where exiting a `with client.stream(...)` context discards
unread body.

### A2: Replayability classification
Added `ReplayClass` enum to `RequestBody` alongside the existing boolean
`is_replayable()`. Values: `Immutable`, `Seekable`, `OneShot`, `Consumed`.

## New Types

| Type | Python Name | Purpose |
|------|-------------|---------|
| `PyRawBytesChunkIterator` | `StreamingRawBytesIterator` | Sync raw bytes iterator |
| `PyAsyncRawBytesIterator` | `AsyncStreamingRawBytesIterator` | Async raw bytes iterator |
| `ReplayClass` | (internal) | Body replayability classification |

## Files Changed (gap fixes)

- `crates/eggfetch-python/src/streaming.rs` — Shared runtime, context-manager discard
- `crates/eggfetch-python/src/conversion.rs` — Python iterable to RequestBody conversion
- `crates/eggfetch-python/src/client.rs` — Stream body passthrough
- `crates/eggfetch-python/src/async_client.rs` — Stream body passthrough
- `crates/eggfetch-python/src/lib.rs` — Stream body passthrough
- `crates/eggfetch-core/src/body.rs` — ReplayClass enum
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_request.py` — File-like + iterable content
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py` — Stream passthrough in send()
- `crates/eggfetch-python/tests/compat/test_stream_resources.py` — NEW: 14 resource/failure/stress tests
- `plans/httpx-drop-in-phase-3-gap-fixes.md` — NEW: gap-fix plan

## Test Evidence

- 460 compat tests passing (pytest)
- 40+ Rust core body tests passing (cargo test)
- Pre-existing: 3 compression test failures in eggfetch-core (unrelated)
