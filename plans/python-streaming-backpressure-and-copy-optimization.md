# Python Streaming Backpressure and Copy Optimization

Planning baseline: 283d52cbf438abf5051527b1c764237d701d6fb3 (main, 2026-09-20; eggfetch 0.1.9)
Parent program: plans/performance-optimization-no-api-regression-program.md
Prerequisite: plans/performance-benchmark-baseline-and-guardrails.md

## Objective

Correct the Python streaming bridge so slow synchronous consumers cannot block Tokio runtime workers, and remove avoidable Rust-side copying/memmove during byte/text/line chunking, without changing the Python streaming API or its behavioral contracts.

This is the highest-priority optimization in the campaign because the current sync producer pattern can reduce global runtime progress, not merely add a few allocations.

## Current behavior that must be preserved

The public Python surfaces, including HTTPX compatibility facades, depend on:

- synchronous and asynchronous streaming response types;
- iter_bytes / aiter_bytes chunk_size semantics;
- iter_raw / aiter_raw encoded-byte semantics;
- iter_text / aiter_text incremental decoding;
- iter_lines / aiter_lines newline handling;
- one-shot/single-consumption behavior;
- response close/aclose cancellation;
- error translation;
- pool/runtime lease lifetime;
- bounded buffering/backpressure;
- no GIL held across network waits;
- correct UTF-8 and configured charset handling across transport chunk boundaries.

Do not “optimize” by returning different bytes, changing where errors surface, using an unbounded queue, or reading the complete response into memory.

## Part A — remove blocking send from Tokio tasks

The sync byte/text/line/raw producers currently run on a Tokio runtime and write into std::sync::mpsc::sync_channel with blocking Sender::send.

Replace this with a bridge whose producer-side backpressure is async/non-blocking to the Tokio worker.

Preferred design:

- Tokio bounded mpsc on the producer side;
- producer awaits send capacity;
- synchronous Python __next__ waits outside the Tokio worker and outside the GIL;
- cancellation/drop wakes or terminates both sides promptly.

The exact sync receive bridge may use Tokio Receiver::blocking_recv only if it is proven safe in every supported calling context. If calling a sync iterator from a host thread that is itself inside a Tokio runtime can panic or deadlock, use a small runtime-neutral bridge design instead.

Do not solve the problem with:

- unbounded channels;
- try_send plus dropping/reordering data;
- busy loops;
- blocking_in_place on ordinary Tokio worker tasks;
- one dedicated OS thread per stream unless measurement and lifecycle analysis prove it is the least-bad option.

### Required concurrency tests

1. one slow sync byte consumer fills the bounded queue while unrelated requests on the same Client continue making progress;
2. multiple slow sync consumers do not starve a small runtime worker pool;
3. dropping the iterator while producer is waiting on capacity cancels promptly;
4. producer error while queue is full is eventually surfaced and does not wedge runtime shutdown;
5. Client.close with active streaming iterator retains existing runtime-lease semantics;
6. no GIL is held while waiting for network data/channel receive.

Use deterministic barriers or small explicit runtime worker counts rather than flaky timing-only tests.

## Part B — retain Bytes through byte queues

The async byte aliases currently carry Vec<u8>, and raw producers convert incoming Bytes with to_vec.

Change private channel payloads to Bytes wherever Python has not yet required ownership as a Python bytes object.

For both sync and async byte/raw iterators:

- keep incoming transport chunks as Bytes;
- represent pending remainder as Bytes or Option<Bytes>;
- split using split_to, split_off, or slice rather than copying prefix+remainder;
- perform the unavoidable copy only when constructing the Python bytes result.

Do not expose bytes::Bytes to Python or change Python object types.

### Chunk-size contract

For explicit chunk_size:

- every yielded chunk except the final one must remain at most/exactly the current documented size behavior;
- data spanning upstream frames must follow current semantics;
- an oversized upstream frame may be represented by O(1) Rust slices internally, but yielded Python bytes remain independent Python-owned objects.

Raw mode with chunk_size=None must preserve upstream raw chunk behavior currently exposed by eggfetch/HTTPX compatibility.

## Part C — eliminate front-drain algorithms

### Byte pending buffers

Remove Vec::drain(..take).collect patterns that shift/copy remainders.

### UTF-8 pending state

IncrementalDecoder's UTF-8 fallback currently stores a Vec and drains valid prefix bytes. Replace repeated front drains with a cursor, BytesMut split, or another bounded representation that retains incomplete trailing code units without moving an arbitrarily large remainder.

Preserve replacement/error behavior exactly as current decode_bytes/IncrementalDecoder semantics require.

### Line buffering

complete_lines repeatedly finds newline then drains from the front of String.

Rework to scan each appended region efficiently and retain only the incomplete tail. Suitable designs include:

- one scan collecting line boundary ranges, then one tail move;
- split_off at the final incomplete boundary;
- a cursor plus periodic compaction.

Preserve CRLF stripping, final unterminated line behavior, empty-line behavior, and charset decoding.

Do not change iter_lines from decoded text semantics to a byte-line parser unless equivalence is exhaustively proven.

## Part D — avoid duplicate sync/async logic drift

The current sync/async/raw/decoded iterators contain repeated chunk-splitting algorithms.

After behavior is covered by tests, extract crate-private helpers for:

- splitting pending Bytes by requested chunk size;
- line-tail extraction;
- incremental decode state where sensible.

Do not introduce a large abstraction framework. The goal is one canonical chunking algorithm so future fixes do not land in only one iterator family.

## Performance qualification

Compare to the baseline cases.

Required evidence:

- sync iter_bytes large-frame/small-chunk case;
- async equivalent;
- raw sync/async equivalents;
- newline-dense line iteration;
- multi-stream slow-consumer scenario;
- process RSS under bounded backpressure;
- runtime progress of an unrelated request/task while queues are full.

Success is primarily:

- no Tokio worker blocked on bounded channel send;
- O(1) Rust-side remainder splitting for Bytes;
- materially lower copy/memmove work for small chunk_size;
- bounded memory stays bounded;
- no representative throughput regression for large chunk_size/native-frame-size workloads.

## API/compatibility tests

Run the native Python tests and full compatibility suite cases covering streaming.

Add regressions for:

- exact byte concatenation;
- chunk sizes 1, small, default, larger than frame/body;
- empty chunks/body;
- multibyte UTF-8 split at every byte boundary;
- non-UTF-8 configured encodings;
- CRLF/LF/final unterminated lines;
- decompressed versus raw compressed body;
- read and total timeouts while consumer is slow;
- stream error ordering;
- close/aclose/drop cancellation;
- sync iterator called from unusual embedding contexts supported today.

The HTTPX/HTTPX2 API manifests must not change.

## Exit criteria

- [ ] No synchronous bounded channel send executes on a Tokio async worker.
- [ ] Backpressure remains bounded and lossless.
- [ ] Byte/raw queues retain Bytes until Python object creation.
- [ ] Oversized frames are split without copying the remainder.
- [ ] Pending byte and line algorithms do not repeatedly front-drain large buffers.
- [ ] Sync/async/raw/decoded variants share canonical private chunking logic where practical.
- [ ] Slow consumers cannot starve unrelated runtime work.
- [ ] Cancellation and runtime/client shutdown remain prompt and correct.
- [ ] Python native and compatibility streaming tests are green.
- [ ] Benchmark evidence is recorded.
- [ ] No Python-visible API/behavior regression.

## Implementation record

The sync iterator bridge now uses a bounded runtime-neutral queue: Tokio
producers await async capacity while synchronous consumers wait on a
condition variable with the GIL released. Queue closure wakes both sides and
iterator drop propagates cancellation. Byte and raw channels retain Bytes,
split remainders with Bytes ownership, and only copy when creating Python
bytes objects. UTF-8 fallback state uses a split-capable BytesMut buffer and
line extraction retains only the incomplete tail after one boundary scan.

The native streaming test slice passed (68 streaming tests plus the response
compatibility tests); full runtime progress and slow-consumer measurements
remain final-closure evidence.
