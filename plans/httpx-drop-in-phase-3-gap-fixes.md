# Phase 3 Gap Fixes

Status: ready for implementation

## Purpose

Address all remaining gaps from the Phase 3 audit. The original implementation delivered core streaming functionality but left several architectural and correctness items incomplete.

## Priority order

### 1. E1: Shared sync runtime (CRITICAL)

Every sync iterator currently spawns `std::thread::spawn` + `tokio::runtime::Runtime::new()`. Replace with a single shared static runtime.

**Approach**: Create a `SharedRuntime` singleton (lazy_static or OnceLock) that all sync iterators use. Each iterator spawns a tokio task (not a thread) on the shared runtime, which sends chunks via bounded `sync_channel(16)`. The blocking `rx.recv()` in `__next__` stays the same but the producer runs on the shared runtime.

**Files**: `streaming.rs`

### 2. B1/B2: Lazy request streaming

`Request.read()` eagerly collects iterables. Instead, the `Client.send()` method should detect stream-type content and pass it through to the native client as a stream body.

**Approach**: In `_request.py`, when `stream=` is provided, store the iterable without consuming. In `_client.py`, when `stream=True` is set on `send()`, check if the request has an unconsumed stream and pass it through as a Python generator to the native client (which already handles generators via the Rust adapter).

**Files**: `_request.py`, `_client.py`

### 3. B3: File-like body support

Add `hasattr(obj, 'read')` detection in `Request.__init__` and `_client.py` to handle file-like objects as streaming bodies.

**Approach**: When `content=` is a file-like object, treat it as a stream body. Create a sync iterator wrapper that calls `.read(chunk_size)` incrementally.

**Files**: `_request.py`, `_client.py`

### 4. F3: Context-manager discard semantics

HTTPX's `__exit__` discards unread body (does NOT drain). Currently `drain_and_close()` drains.

**Approach**: Change `__exit__` / `__aexit__` on `PyStreamingResponse` to just close (drop the stream), not drain. This matches HTTPX behavior where exiting a `with client.stream(...)` context without reading discards the body.

**Files**: `streaming.rs`

### 5. A2: Replayability classification

Add a `ReplayClass` enum to `body.rs` to replace the boolean `is_replayable()`.

**Approach**: Define `ReplayClass { Immutable, Seekable, OneShot, Consumed, Closed }` and expose it alongside the existing boolean for backward compat.

**Files**: `body.rs`

### 6. E3: Async loop affinity

Record the event loop when async iterators are created and reject cross-loop use.

**Approach**: In async iterator constructors, capture `asyncio.get_event_loop()`. In `__anext__`, verify we're on the same loop. If not, raise `RuntimeError`.

**Files**: `streaming.rs` (async iterators only)

### 7. C1: Lazy multipart from Python file objects

`add_path_file_part` eagerly reads via `std::fs::read`. For `File` wrappers, use the streaming `PartBody::Stream` instead.

**Approach**: Replace `std::fs::read` with a streaming read that uses `tokio::fs::File` and yields chunks. For the Python binding, read synchronously but via a bounded channel bridge (leveraging the shared runtime from E1).

**Files**: `multipart.rs`

### 8. C4: Resource ownership tests

Add tests verifying eggfetch-opened files close on success, failure, cancellation.

**Files**: `test_request_streaming.py` or new `test_resource_ownership.py`

### 9. F1/F2: Producer/consumer failure tests

Add tests for iterator raises, client close during production, consumer stops early, iterator dropped.

**Files**: `test_response_streaming.py`

### 10. H1-H3: Stress/memory/thread tests

Add reference stream server endpoints and thread-count assertions.

**Files**: new `test_stream_resources.py`

### 11. #27: Status file evidence links

Update `plans/httpx-drop-in-phase-3-status.md` with links to test output and CI evidence.

**Files**: `plans/httpx-drop-in-phase-3-status.md`

## Acceptance criteria mapping

| Criterion | Fix |
|---|---|
| 1. Sync iterable lazy with bounded buffering | B1 |
| 2. Async iterable lazy on correct loop | B2 + E3 |
| 3. File-like bodies stream without buffering | B3 |
| 4. User-owned file lifecycle matches reference | C4 |
| 5. Eggfetch-opened files close on all paths | C4 |
| 6. Public stream base classes work | Already met |
| 7. Multipart uses streaming encoder | C1 |
| 8. Multipart tuple forms match corpus | Already partially met |
| 9. Multipart content-length safe | Already met |
| 10-14. Response streaming parity | Already met |
| 15. Sync streams don't create per-stream runtime | E1 |
| 16. Bounded bridge queues | Already met |
| 17-18. Timeout coverage | Already partially met |
| 19. Early close releases resources | Already met |
| 20. Dropped iterators don't retain tasks | E1 (tasks on shared runtime are cleaned up) |
| 21. Redirect/retry decisions | Already met |
| 22. Exception hierarchy | Already met |
| 23-25. Stress/memory/thread tests | H1-H3 |
| 26. Cross-platform wheel tests | Deferred to CI |
| 27. Status file evidence links | #27 |
