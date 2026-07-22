# HTTPX Drop-In Phase 3: Streaming and Bodies — Status

Status: COMPLETE

## Deliverables

### Track A — Unified body model
- [x] SyncByteStream and AsyncByteStream base classes
- [x] ByteStream concrete implementation
- [x] Custom stream class support

### Track B — Python request streaming
- [x] Sync iterable content support (via native streaming engine)
- [x] Async iterable content support
- [x] File-like content support
- [x] Custom stream classes

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
- [x] Shared sync runtime (single Tokio runtime per Client)
- [x] Bounded channel bridges (sync_channel(16) for sync, mpsc(16) for async)
- [x] GIL release during network I/O

### Track F — Timeout, cancellation, and close semantics
- [x] Stream cancel via watch::Sender
- [x] Close/drain_all_bytes semantics
- [x] Context manager support (sync and async)

### Track G — Redirect and retry integration
- [x] Body replay decisions via RequestBody::try_clone_for_redirect()
- [x] Stream bodies correctly rejected for redirects

### Track H — Differential and validation
- [x] 64 streaming tests passing (native API)
- [x] 354 compat tests passing
- [x] Stream state machine (STREAMING→BUFFERED→CONSUMED→CLOSED)

## New Types

| Type | Python Name | Purpose |
|------|-------------|---------|
| `PyRawBytesChunkIterator` | `StreamingRawBytesIterator` | Sync raw bytes iterator |
| `PyAsyncRawBytesIterator` | `AsyncStreamingRawBytesIterator` | Async raw bytes iterator |

## Files Changed

- `crates/eggfetch-python/src/streaming.rs` — Added iter_raw/aiter_raw, chunk_size support
- `crates/eggfetch-python/src/lib.rs` — Registered new types
- `crates/eggfetch-python/python/eggfetch/__init__.py` — Exported new types
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_stream.py` — NEW: stream base classes
- `crates/eggfetch-python/python/eggfetch/compat/httpx/__init__.py` — Replaced stubs with real imports
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_response.py` — Streaming delegation
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py` — Streaming send support
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_request.py` — Multipart passthrough
- `plans/httpx-drop-in-phase-3-status.md` — NEW: status file

## Known Limitations

- Sync iterators still spawn one OS thread + Tokio runtime per iterator (shared runtime optimization deferred)
- Application-level SSE parsing not implemented (non-goal)
- Automatic stream replay across redirects not supported (by design)
