# HTTPX 0.28.1 Final Closure 02 — Raw Stream Lifecycle and Accounting

Status: ready for implementation handoff

Date: 2026-08-04

Depends on:

- `plans/httpx-parity-final-closure-roadmap.md`
- `plans/httpx-parity-final-closure-01-redirect-security-replay.md`

Audited baseline: `6ae10308b9db1e215eca19027d4ca9b7575900f6`

Pinned reference: `httpx==0.28.1`

## Objective

Finish Response streaming parity for the supported surface by making raw iteration a first-class path with correct state transitions, byte accounting, chunking, close behavior, and sync/async symmetry.

The current implementation improved decoded byte and text iteration but still has two defects:

- sync `iter_raw()` delegates directly to the native raw iterator and bypasses wrapper lifecycle/accounting;
- async `aiter_raw()` delegates through decoded bytes instead of a distinct native raw path.

This plan must not introduce a second decompression stack in Python.

## Files expected to change

Primary:

- `crates/eggfetch-python/python/eggfetch/compat/httpx/_response.py`
- native Python stream adapter files only if a narrow raw primitive is missing
- `crates/eggfetch-python/tests/compat/`
- `crates/eggfetch-python/tests/compat/test_corrective_kernel.py`

Possible supporting files:

- `crates/eggfetch-python/python/eggfetch/compat/httpx/_transports.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_exceptions.py`, only if existing mapping is incomplete

Do not alter unrelated Rust transport, pool, compression, or protocol behavior.

# Track 0 — Capture exact HTTPX streaming state

## 0.1 Add direct pinned-reference cases before changing production code

Execute equivalent cases against:

- `httpx==0.28.1`;
- `eggfetch.compat.httpx`.

Required sync cases:

1. uncompressed native response, `iter_raw(chunk_size=None)`;
2. uncompressed native response, `iter_raw(chunk_size=1)`;
3. source chunks larger than requested chunk size;
4. source chunks smaller than requested chunk size;
5. empty source chunks;
6. zero-length body;
7. raw iterator exhausted normally;
8. raw iterator broken after first chunk, then explicit `close()`;
9. raw iterator object explicitly closed without exhaustion;
10. second `iter_raw()` call after partial consumption;
11. second `iter_raw()` call after full consumption;
12. `.read()` after partial raw iteration;
13. `.content` after partial raw iteration;
14. unread response followed by `close()`;
15. underlying stream close-count observation;
16. `num_bytes_downloaded` before iteration, after one chunk, and after exhaustion.

Required async cases mirror the sync cases using:

- `aiter_raw()`;
- `aclose()`;
- async iterator finalization.

Required raw-versus-decoded cases:

1. gzip-compressed native response;
2. another already-supported content encoding if fixtures exist;
3. raw bytes differ from decoded bytes;
4. raw byte count differs from decoded output length;
5. decoded `iter_bytes()` remains correct after raw-path refactoring;
6. raw iteration does not invoke Python decompression.

## 0.2 Capture public state only

Record:

- yielded chunks;
- `is_stream_consumed`;
- `is_closed`;
- `num_bytes_downloaded`;
- exception class;
- underlying close count;
- whether elapsed becomes available;
- whether a second iterator is allowed.

Do not use private HTTPX internals as the expected result when public state is observable.

## 0.3 Normalize only implementation-specific representation

Permitted normalization:

- chunk sequences to lists of bytes;
- exception class names;
- elapsed to available/unavailable plus broad ordering;
- close counters to integers.

Not permitted:

- treating raw and decoded bytes as equivalent for compressed bodies;
- ignoring state flags because yielded data matches;
- using EggFetch’s current close timing as the oracle;
- skipping async cases because sync passes.

### Track 0 acceptance criteria

- Every current raw-iteration defect has a failing or previously failing direct reference case.
- Sync and async cases cover normal exhaustion and partial consumption.
- At least one compressed native response proves raw and decoded paths are distinct.
- Byte accounting is observed incrementally, not only after completion.
- Underlying close count is observable.

# Track 1 — Define one internal raw-source abstraction

## 1.1 Separate the layers explicitly

The Response implementation must retain these logical layers:

1. raw transport/source bytes;
2. decoded/decompressed bytes;
3. text decoding;
4. line splitting.

Introduce a small private raw-source helper or pair of helpers for sync and async behavior. The helper must own:

- raw source selection;
- raw chunk-size adaptation;
- wrapper byte accounting;
- wrapper consumption state;
- close/finalization.

Do not duplicate the full iterator algorithm independently in `iter_raw()` and `aiter_raw()` if a small shared state helper can be used safely.

## 1.2 Select the correct source

Source precedence should be explicit:

- native stream raw primitive when available;
- compatibility sync stream for `iter_raw()`;
- compatibility async stream for `aiter_raw()`;
- buffered content only where HTTPX permits raw iteration of buffered responses.

A sync method invoked on an async-only stream, or async method on a sync-only stream, must raise the pinned-reference-compatible error.

## 1.3 Keep decoded iteration native-authoritative

For native streams:

- `iter_bytes()` and `aiter_bytes()` should continue to use native decoded behavior when available;
- do not decode compressed bodies in Python;
- do not implement gzip, deflate, Brotli, or Zstandard handling in the facade.

Raw-path refactoring must not force decoded iteration through a Python decoder.

### Track 1 acceptance criteria

- Raw and decoded paths are structurally distinct.
- Native raw bytes are available to raw iterators.
- Native decoded bytes remain available to decoded iterators.
- Compatibility streams use raw==decoded only when no content-decoding layer exists.
- No second decompression stack is introduced.

# Track 2 — Match stream-consumption state transitions

## 2.1 Mark consumption at the reference point

HTTPX marks live raw streams consumed when iteration begins, not only after normal exhaustion.

Match the direct reference result for:

- first `next()` or first async iteration step;
- iterator construction without advancing;
- early break;
- source exception;
- cancellation.

Do not leave `is_stream_consumed=False` after raw bytes have been yielded.

## 2.2 Preserve buffered-response semantics

Buffered responses must remain:

- closed at construction;
- consumed at construction;
- repeatedly readable;
- iterable according to the pinned reference’s buffered chunk behavior.

Do not make buffered content single-use while fixing live streams.

## 2.3 Enforce repeated-consumption errors

After a live raw stream has begun consumption:

- a second raw iterator must raise the correct stream exception;
- decoded iteration or `.read()` after partial raw consumption must match the reference;
- `.content` must remain unavailable unless the body was fully buffered by a supported read path.

Use the existing exception hierarchy. Do not add candidate-specific exception types.

### Track 2 acceptance criteria

- `is_stream_consumed` changes at the same observable point as HTTPX.
- Early break does not leave the response falsely unconsumed.
- Repeated raw iteration raises the reference-compatible exception.
- Read-after-partial-raw behavior matches the reference.
- Buffered responses remain repeatable.
- Sync and async state transitions agree.

# Track 3 — Count raw bytes correctly

## 3.1 Reset accounting when live raw iteration begins

Match HTTPX behavior for initial and reset state. For a live stream, raw iteration must begin with the correct `num_bytes_downloaded` value and update it as each raw source chunk is consumed.

Do not increment accounting from wrapper output chunks after coalescing or splitting if that would diverge from raw source bytes.

## 3.2 Count raw transport bytes, not decoded output

For compressed native responses:

- `num_bytes_downloaded` must follow HTTPX raw-byte semantics;
- decoded byte length must not be substituted;
- `iter_bytes()` must obtain authoritative native accounting where available.

Preferred strategies, in order:

1. read the native stream or response’s authoritative raw-byte counter;
2. have the native adapter expose the existing counter narrowly;
3. count raw chunks in a shared raw layer only when decoded iteration can flow through that layer without adding Python decompression.

Do not fake raw accounting from decoded lengths.

## 3.3 Preserve partial accounting

After one yielded raw chunk, `num_bytes_downloaded` must reflect all raw source bytes consumed to produce that output, including any bytes currently buffered by chunk-size adaptation.

Tests must cover coalescing where multiple source chunks are consumed before one output chunk is yielded.

### Track 3 acceptance criteria

- Raw accounting increments during iteration.
- Compressed responses count raw bytes.
- Coalescing and splitting do not double-count.
- Partial iteration exposes the reference-compatible count.
- Zero-length bodies report the reference-compatible value.
- Sync and async accounting agree.

# Track 4 — Honor raw `chunk_size`

## 4.1 Implement bounded chunk adaptation

For compatibility streams and any native raw primitive that does not already honor the requested size, use a small incremental byte buffer to:

- split large source chunks;
- coalesce small source chunks;
- flush the remainder at end-of-stream;
- handle `chunk_size=None` according to HTTPX;
- ignore or preserve empty chunks according to the pinned reference.

Do not buffer the full response body.

## 4.2 Validate invalid sizes

Capture and match HTTPX behavior for:

- `chunk_size=0`;
- negative values;
- non-integer values if accepted by the public signature path.

Do not silently normalize invalid values unless HTTPX does.

## 4.3 Do not alter decoded chunking accidentally

Raw chunk-size changes must not regress:

- `iter_bytes()`;
- `aiter_bytes()`;
- incremental text decoding;
- line iteration.

### Track 4 acceptance criteria

- Requested positive chunk sizes are honored.
- `chunk_size=None` matches the reference.
- Remainder bytes are emitted exactly once.
- Empty chunks match the reference.
- Invalid sizes raise the reference-compatible error.
- No eager full-body buffering occurs.

# Track 5 — Finalize and close exactly once

## 5.1 Close on normal exhaustion

When raw iteration reaches end-of-stream:

- close/aclose the response;
- finalize elapsed timing;
- close the underlying source exactly once;
- preserve consumed state.

## 5.2 Handle partial consumption safely

For early break:

- the response remains consumed;
- explicit `close()` or `aclose()` releases the underlying stream;
- repeated close is idempotent;
- generator finalization must not double-close.

Do not rely solely on nondeterministic garbage collection for resource release.

## 5.3 Handle exceptions and cancellation

If the source raises or async iteration is cancelled:

- preserve the mapped transport/stream exception;
- close the underlying stream when the reference does;
- leave wrapper state consistent;
- do not mask the original exception with a close error unless HTTPX does.

## 5.4 Keep native and wrapper close ownership explicit

Document in code which layer owns close invocation. Avoid both:

- wrapper closing twice because the native iterator already closed;
- wrapper never closing because it assumed the native iterator would.

A private one-time close guard is acceptable if needed. Do not add a broad resource manager framework.

### Track 5 acceptance criteria

- Normal exhaustion closes exactly once.
- Explicit close after early break closes exactly once.
- Repeated close is harmless.
- Elapsed becomes available at the reference-compatible point.
- Exceptions and cancellation do not leak the stream.
- Close errors do not obscure the primary failure incorrectly.
- Sync and async behavior agree.

# Track 6 — Reconcile decoded iteration with the raw lifecycle

## 6.1 Remove direct lifecycle bypasses

Audit `iter_bytes()`, `aiter_bytes()`, `iter_text()`, `aiter_text()`, `iter_lines()`, and `aiter_lines()` for assumptions made before the raw lifecycle is corrected.

Ensure:

- decoded iteration marks the stream consumed correctly;
- byte accounting remains raw-authoritative;
- close happens once;
- incremental text decoding remains correct;
- line splitting remains correct.

## 6.2 Avoid nested consumption guards

If decoded native iteration invokes a native method that itself consumes the stream, the wrapper must not call its own public `iter_raw()` in a way that trips duplicate consumption checks.

Use private source helpers where necessary. Public iterators should remain the API boundary, not internal recursion points.

## 6.3 Preserve raw-after-decoded and decoded-after-raw behavior

Direct tests must establish and enforce:

- raw after full decoded iteration;
- decoded after full raw iteration;
- raw after partial decoded iteration;
- decoded after partial raw iteration.

### Track 6 acceptance criteria

- Existing split-UTF-8 and chunk-size tests remain green.
- Raw and decoded paths cannot both consume the same live stream.
- Byte accounting remains correct for decoded native responses.
- Text and line iteration do not regress.
- No nested public-iterator recursion causes false StreamConsumed errors.

# Track 7 — Add bounded regression coverage

## 7.1 Tier 1 additions

Add a compact set of candidate regressions covering:

- raw iteration marks consumed and closed on exhaustion;
- raw byte accounting updates;
- partial raw iteration followed by close;
- async raw iteration uses a raw path;
- raw and decoded content differ for a small controlled adapter fixture, if it does not add an external dependency.

Keep total corrective-kernel size proportionate. Prefer no more than several additional parameterized cases.

Tier 1 must not install HTTPX or perform public-network requests.

## 7.2 Full differential coverage

The full compatibility suite must contain every Track 0 case, including compressed native fixtures, partial consumption, close counts, and async symmetry.

## 7.3 Avoid false-green mocks

A custom compatibility stream alone is not sufficient proof for the native raw path.

At least one test must exercise the built-in native transport/stream adapter for both sync and async paths.

### Track 7 acceptance criteria

- Tier 1 catches the primary lifecycle regression without growing into the full qualification suite.
- Full tests compare directly with HTTPX 0.28.1.
- Native raw paths are actually executed.
- Async coverage is not simulated through sync wrappers.
- No new CI job or workflow is added.

# Validation commands

Routine:

```sh
./scripts/check.sh
```

Focused compatibility:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ \
  -q --strict-markers
```

Run the new raw-stream differential module separately during development with verbose node IDs.

# Stop condition

If the native Python extension exposes no raw stream primitive or authoritative raw byte counter, and adding one requires a broad Rust API redesign:

1. stop before adding Python decompression or fake accounting;
2. document the exact missing native primitive;
3. add the smallest reproducible test;
4. preserve truthful bounded behavior;
5. create a separate reviewed native-adapter follow-up.

Do not claim this plan complete under that condition.

# Final acceptance checklist

This plan is complete only when:

- direct pinned-reference raw tests pass;
- sync and async raw paths are distinct and symmetric;
- live raw streams become consumed at the correct point;
- raw byte accounting is authoritative and incremental;
- raw chunk sizing matches HTTPX;
- normal exhaustion and partial close release resources exactly once;
- compressed raw and decoded paths remain distinct;
- decoded text/line behavior remains green;
- Tier 1 remains compact;
- routine checks pass;
- the implementation SHA is handed to Plan 03 for exact evidence closure.
