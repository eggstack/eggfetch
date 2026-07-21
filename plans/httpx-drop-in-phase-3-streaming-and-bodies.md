# HTTPX Drop-In Phase 3: Streaming and Body Architecture

Status: ready for implementation handoff

## Purpose

Make Python request and response bodies genuinely streaming, bounded, backpressured, cancellation-safe, and behaviorally compatible with HTTPX 0.28.1.

The Rust core already supports streamed bodies. The current Python adapter still buffers ordinary Python request bodies and multipart payloads into byte vectors, while synchronous response iterators may create a dedicated thread and Tokio runtime per stream. This phase replaces those adapter limitations with one coherent stream model shared by requests, redirects, retries, responses, transports, and future compatibility backends.

## Dependencies

Phase 0 must define the stream compatibility corpus. Phase 1 must provide stable lifecycle, cancellation, timeout, and shared-runtime behavior. Phase 2 must provide compatibility `Request`, `Response`, exceptions, and stream base classes.

## Non-goals

- Implementing mount routing, event hooks, or in-process application transports.
- Rewriting the Rust core's functioning stream implementation without evidence of a gap.
- Automatically replaying arbitrary one-shot streams across redirects or retries.
- Buffering unbounded bodies merely to imitate replayability.
- Requiring HTTP/3 parity with HTTPX.
- Adding application-level SSE parsing beyond ordinary line and text streaming.

## Deliverables

1. A unified request-body abstraction across Rust and Python compatibility objects.
2. Sync iterable, async iterable, file-like, byte, string, form, JSON, and multipart body support.
3. Lazy multipart encoding from Python without whole-payload buffering.
4. Raw and decoded response streaming methods.
5. Exact read/consume/close state transitions.
6. Shared-runtime stream bridges without one runtime and thread per stream.
7. Backpressure, cancellation, timeout, and early-close correctness.
8. Replayability integration for redirects and retries.
9. Differential and resource-stability evidence.
10. A phase status file.

## Track A — Define the unified body model

### A1. Body categories

Represent at least:

- empty body;
- known immutable bytes;
- known text encoded to bytes;
- sync iterable of bytes;
- async iterable of bytes;
- file-like reader;
- native Rust byte stream;
- multipart state-machine stream;
- form and JSON bodies that may remain buffered when naturally bounded;
- custom `SyncByteStream` and `AsyncByteStream` implementations.

Every body must carry explicit metadata where known:

- content length;
- replayability;
- sync/async consumption mode;
- consumed state;
- close/finalizer behavior;
- optional content type;
- source description safe for diagnostics.

### A2. Replayability classes

Define a closed internal classification such as:

- replayable immutable;
- replayable seekable with reset operation;
- factory-replayable;
- one-shot sync stream;
- one-shot async stream;
- consumed;
- closed.

Redirect and retry code must consult this classification rather than infer replayability from body size or Python object type ad hoc.

### A3. State machine

Document and test legal transitions:

- fresh -> streaming -> consumed;
- fresh -> buffered -> consumed/readable;
- fresh -> closed;
- streaming -> closed/cancelled;
- consumed -> repeated-read behavior according to body type;
- buffered -> iterator behavior;
- error -> closed/released.

State errors must map to HTTPX-compatible exceptions with request/response context where applicable.

## Track B — Python request streaming

### B1. Sync iterable content

Support `content=` supplied as a sync iterable yielding bytes.

Requirements:

- lazy iteration only after send begins;
- no pre-collection into a list or byte vector;
- validation that yielded items are bytes-like according to the target behavior;
- bounded bridge queue;
- write timeout applied while waiting for the next producer item and while sending;
- producer exceptions mapped predictably;
- iterator close/finalization on cancellation, error, or client close;
- chunked transfer when length is unknown;
- content length when explicitly known through supported stream metadata.

### B2. Async iterable content

Support async iterables for `AsyncClient` and async request construction.

Requirements:

- iteration on the originating Python event loop where required;
- bounded transfer into the Rust core;
- cancellation propagation in both directions;
- no cross-loop use;
- deterministic finalization;
- no blocking Python event-loop thread;
- correct exception mapping.

A sync client receiving an async-only stream and an async client receiving an invalid sync-only stream must match reference validation behavior.

### B3. File-like content

Support binary file objects and compatible readers without reading the whole file first.

Requirements:

- incremental reads;
- content-length detection only when safe and position-aware;
- preserve current file position semantics according to the reference;
- do not close user-owned files unless the target contract does;
- cancellation releases internal references;
- seekable replay may reset only when the original offset is recorded and restoration succeeds;
- non-seekable files remain one-shot.

### B4. Custom stream classes

Implement HTTPX-compatible `SyncByteStream` and `AsyncByteStream` public base protocols. User subclasses must work without private eggfetch hooks.

Validate:

- iteration method names;
- close/aclose behavior;
- type errors for wrong stream kind;
- response construction with custom streams;
- transport interaction in Phase 4.

## Track C — Multipart and form bodies

### C1. Remove Python multipart buffering

Refactor Python `files=` handling so file paths, file objects, byte streams, and custom file tuples are represented as lazy multipart parts and passed to the Rust multipart encoder.

Do not convert the completed multipart body into `Vec<u8>` before send.

### C2. Match tuple and value forms

Differentially implement supported file forms, including:

- raw file object;
- filename and file object;
- filename, content, and content type;
- filename, content, content type, and per-part headers;
- bytes and string field behavior;
- repeated field names;
- `None` filename behavior where supported;
- mapping and sequence inputs;
- `data=` combined with `files=`.

### C3. Multipart length and chunking

Compute total content length only when every part length and encoder overhead are known without overflow. Otherwise use streaming transfer semantics.

Add tests for:

- empty files;
- large files;
- non-seekable file objects;
- Unicode filenames and field names;
- quoted header parameters;
- custom part headers;
- boundary collision policy;
- cancellation midway through a part;
- redirect and retry replayability.

### C4. Resource ownership

Clarify and test which objects eggfetch opens and therefore closes versus which objects are user-owned. Path inputs opened by eggfetch must close on success, failure, cancellation, and abandoned requests.

## Track D — Response streaming parity

### D1. Raw versus decoded streams

Implement the target methods and semantics for:

- `iter_raw()`;
- `aiter_raw()`;
- `iter_bytes()`;
- `aiter_bytes()`;
- `iter_text()`;
- `aiter_text()`;
- `iter_lines()`;
- `aiter_lines()`;
- `read()`;
- `aread()`.

Raw iterators must expose transport bytes before content decoding. Decoded byte iterators must apply content encodings. Text iterators must incrementally decode character sets without corrupting multibyte characters across chunks.

### D2. Chunk-size behavior

Match the reference for:

- default chunk sizes;
- explicit chunk size;
- zero or invalid values;
- transport chunk coalescing and splitting;
- final partial chunks;
- empty chunks;
- compressed-body chunk boundaries;
- line boundary handling across chunks.

### D3. Line semantics

Differentially verify:

- `\n` and `\r\n`;
- trailing final line without newline;
- empty lines;
- Unicode line boundaries only where the reference recognizes them;
- decoded text with split multibyte characters;
- raw bytes not incorrectly decoded.

### D4. Buffered response iterators

Do not build a complete Python list simply to return an iterator. Buffered response iteration may slice the already buffered body lazily, but should not duplicate it into a second unbounded list.

## Track E — Runtime and bridge architecture

### E1. Shared sync runtime

Use the shared runtime architecture established in Phase 1. A synchronous streaming iterator must not spawn one OS thread and one Tokio runtime per response.

Preferred characteristics:

- one shared runtime service;
- bounded channels per active stream;
- worker tasks rather than worker runtimes;
- prompt task cancellation when the iterator is dropped;
- no daemon thread accumulation;
- deterministic module shutdown.

### E2. Python GIL policy

Release the GIL while blocking for stream chunks, but reacquire it only for Python producer/consumer interaction. Ensure a slow Python iterator does not hold the GIL while Rust waits on network I/O.

### E3. Async loop affinity

Record the event loop associated with async Python producers and consumers when required. Reject cross-loop use with the reference-compatible exception rather than deadlocking.

### E4. Channel bounds and backpressure

Every bridge queue must be explicitly bounded. Add instrumentation and tests proving:

- producer cannot outrun the network indefinitely;
- network reader cannot buffer an unbounded response when Python is slow;
- cancellation unblocks a full queue;
- close unblocks both sides;
- queue capacity does not alter visible chunk order.

## Track F — Timeout, cancellation, and close semantics

### F1. Request producer failures

Test and classify:

- sync iterator raises;
- async iterator raises;
- file read fails;
- multipart part fails;
- producer yields invalid type;
- producer blocks beyond write timeout;
- client closes during production;
- request task is cancelled.

The core connection must be reusable only when protocol state permits. Otherwise it must be discarded safely.

### F2. Response consumer failures

Test:

- consumer stops early;
- iterator object is dropped;
- context exits without reading;
- response closes during blocked read;
- client closes during active response;
- cancellation during decompression;
- malformed compressed stream;
- read timeout after partial data.

Pool permits and transport resources must release deterministically.

### F3. Context-manager semantics

Match HTTPX for sync and async stream contexts, including the point at which the request is sent, the type returned by `stream()`, and close behavior on exceptions inside the context.

## Track G — Redirect and retry integration

### G1. Body replay decisions

For every redirect status and retry decision, test:

- immutable body replay;
- seekable file replay;
- one-shot stream rejection;
- method rewriting that removes the body;
- `Content-Length` and transfer-encoding header changes;
- user-provided body headers;
- cancellation between attempts.

### G2. Error compatibility

Map non-replayable behavior to the reference contract where one exists. Eggfetch-specific retry features may raise eggfetch-specific native exceptions outside the compatibility module, but the HTTPX facade must not expose unrelated classes for ordinary HTTPX behavior.

## Track H — Differential and stress validation

### H1. Reference stream server

Use deterministic endpoints for:

- one-byte chunks;
- random but seeded chunk boundaries;
- delayed chunks;
- infinite stream with explicit test cancellation;
- compressed streams;
- truncated streams;
- upload echo with read pacing;
- server stops reading request body;
- bidirectional backpressure.

### H2. Memory envelopes

Prove bounded resident memory for:

- multi-gigabyte synthetic download consumed slowly;
- multi-gigabyte synthetic upload generated lazily;
- multipart upload of large files;
- many concurrent small streams;
- many abandoned streams;
- compressed expansion up to configured safety limits.

Tests may use generated data and bounded run sizes in PR CI, with larger scheduled profiles.

### H3. Thread/task envelopes

Assert that active threads and runtime tasks scale according to the shared architecture, not one unbounded thread/runtime per stream.

## Expected files

Likely changes include:

- `crates/eggfetch-core/src/body.rs`
- `crates/eggfetch-core/src/multipart/`
- `crates/eggfetch-core/src/client.rs`
- `crates/eggfetch-python/src/streaming.rs`
- `crates/eggfetch-python/src/conversion.rs`
- `crates/eggfetch-python/src/multipart.rs`
- Python compatibility stream modules;
- `crates/eggfetch-python/tests/compat/test_request_streaming.py`
- `crates/eggfetch-python/tests/compat/test_response_streaming.py`
- `crates/eggfetch-python/tests/production/test_stream_resources.py`
- benchmark and resource-monitor workloads;
- streaming and multipart documentation;
- `plans/httpx-drop-in-phase-3-status.md`.

## Acceptance criteria

This phase is complete only when:

- [ ] Sync iterable request content is consumed lazily with bounded buffering.
- [ ] Async iterable request content is consumed lazily on the correct event loop.
- [ ] File-like request bodies stream without whole-file buffering.
- [ ] User-owned file lifecycle matches the reference contract.
- [ ] Eggfetch-opened files close on all success and failure paths.
- [ ] Public sync and async byte-stream base classes work for user subclasses.
- [ ] Python multipart uploads use the core streaming encoder rather than a completed byte vector.
- [ ] Multipart tuple forms, repeated fields, headers, and filenames match the differential corpus.
- [ ] Multipart content length is emitted only when safely known.
- [ ] Raw response iterators expose undecoded transport bytes.
- [ ] Decoded byte iterators apply content encoding correctly.
- [ ] Text iterators preserve multibyte characters across arbitrary chunk boundaries.
- [ ] Line iterators match reference handling of CRLF, empty lines, and final partial lines.
- [ ] Buffered iterators do not allocate a second whole-body Python list.
- [ ] Synchronous streams do not create one Tokio runtime or OS thread per stream.
- [ ] Every Python/Rust bridge queue is bounded and cancellation-safe.
- [ ] Write timeout covers waiting on Python producers and network writes according to the profile.
- [ ] Read timeout covers headers and body inactivity according to the profile.
- [ ] Early close releases pool permits and transport resources.
- [ ] Dropped iterators do not retain background tasks or threads.
- [ ] Redirect and retry replayability decisions are deterministic and tested.
- [ ] Request and response stream exceptions match the compatibility hierarchy.
- [ ] Large synthetic upload and download tests remain within committed memory thresholds.
- [ ] Concurrent stream stress tests remain within committed thread/task thresholds.
- [ ] Sync and asyncio differential stream suites have no unexplained differences.
- [ ] Built-wheel streaming tests pass on Linux, macOS, and Windows.
- [ ] `plans/httpx-drop-in-phase-3-status.md` links exact test, resource, and CI evidence.

## Handoff notes

The implementation should avoid solving Python streaming by adding larger buffers. The core design test is whether a producer or consumer can be arbitrarily slow without unbounded memory growth, leaked background work, or incorrect timeout behavior. Preserve backpressure end to end.
