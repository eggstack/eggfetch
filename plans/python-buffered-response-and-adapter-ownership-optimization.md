# Python Buffered Response and Adapter Ownership Optimization

Planning baseline: `b6bcc33aa1ad04dfa22af219b82601d3c1377743`
Parent program: `plans/second-pass-performance-ownership-optimization-program.md`
Prerequisite: `plans/second-pass-performance-benchmark-and-guardrails.md`

## Objective

Reduce Python adapter memory amplification and time-to-first-item by transferring core response ownership cleanly into Python wrappers, making buffered response iterators genuinely lazy, and removing the final avoidable full-body Rust copy from async `aread()`.

Public Python signatures, returned types, HTTPX/HTTPX2 compatibility, exception behavior, and streaming lifecycle semantics are frozen.

## Part A — move core response headers into PyHeaders

### Buffered responses

`PyResponse::from_core_response_with_body()` receives `&mut eggfetch_core::Response`.

Before moving headers, extract every value that depends on them:

- wire content-encoding/content-length snapshots;
- charset;
- Set-Cookie values/cookie jar update;
- reason/status/version;
- any compatibility metadata.

Then use `std::mem::take(response.headers_mut())` (or equivalent private ownership transfer) to construct `PyHeaders` without cloning the whole HeaderMap.

Do not move headers early enough to break cookie parsing, charset detection, or response metadata.

### Streaming responses

`PyStreamingResponse::from_core_response()` owns the core `Response`.

Perform the same metadata extraction before moving the ordinary HeaderMap into `PyHeaders`. Confirm that subsequent body-state operations do not read the core response headers.

Redirect-history conversion is not required to change unless a similarly safe owned extraction exists; do not create new public HistoryEntry APIs merely to optimize it.

## Part B — replace eager buffered iterators with private lazy iterator objects

Current buffered methods prebuild all Python objects and a Python list before returning an iterator.

Introduce private PyO3 iterator classes for:

- bytes chunks;
- text chunks;
- lines.

The public methods remain exactly:

- `Response.iter_bytes(chunk_size=8192)`;
- `Response.iter_text(chunk_size=8192)`;
- `Response.iter_lines()`.

### Bytes iterator

Hold a cheap reference/clone to the response's `Bytes` plus a byte cursor and chunk size.

Each `__next__` creates only the next Python `bytes`.

Preserve:

- `chunk_size == 0` ValueError timing;
- exact chunk boundaries;
- final short chunk;
- fresh independent iterator on every method call.

### Text iterator

Do not materialize `Vec<char>`.

Retain the same semantic contract as the current implementation: `chunk_size` counts Unicode scalar values/chars, not UTF-8 bytes. Keep a byte cursor plus character progress sufficient to locate only the next chunk boundary.

The decoded text remains backed by the existing lazy `OnceLock<String>`; iterator creation may initialize text because `iter_text` is text-dependent, but it must not pre-create every Python string.

### Lines iterator

Yield line strings one at a time with behavior equivalent to the current `str::lines()` implementation, including CRLF handling and trailing-line behavior.

Do not silently change to HTTPX streaming line semantics; this is the already-buffered `PyResponse` contract.

## Part C — remove async aread intermediate Vec

Current `PyStreamingResponse.aread()`:

1. drains into `BytesMut`;
2. freezes to `Bytes`;
3. caches a cheap clone;
4. returns `bytes.to_vec()`.

Change the async bridge so it creates/returns the final Python `bytes` object without an intermediate full-body Vec when supported cleanly by PyO3.

The network drain must continue outside the GIL. Only final Python object construction should attach to Python.

Preserve:

- exact `bytes` result type;
- content cache as `Bytes`;
- STATE_STREAMING -> STATE_BUFFERED transition;
- cancellation/close behavior;
- repeated consumption errors;
- pool-lease release timing;
- exception translation.

If PyO3's future conversion requires an owned Vec and avoiding it would add unsafe code or awkward lifetime coupling, stop and record the candidate as rejected. Unsafe buffer sharing is not allowed.

## Part D — Python compatibility/resource proof

Add focused tests for:

- large buffered iterators can yield first item without pre-consuming/materializing the rest;
- repeated `iter_bytes`/text/lines calls are independent;
- multibyte UTF-8 text chunk boundaries match the current implementation;
- CRLF and no-final-newline behavior;
- empty bodies;
- large async `aread()`;
- `type(await response.aread()) is bytes`;
- content/text/cache behavior after `aread()`;
- close/cancel/consumed-state errors.

Run the native Python API manifest and typing checks. No new export should appear for private iterator pyclasses unless the current module machinery would expose them; keep them private and out of `__all__`.

## Performance acceptance

Compare against Plan 1.

Required evidence:

- core->PyHeaders constructor cost improves or at minimum eliminates the known whole-map clone without regression;
- buffered iterator construction/time-to-first-item becomes independent of total chunk/line count apart from required text decoding;
- large-body peak RSS is materially lower for iterator construction/partial consumption;
- full-consumption throughput is non-regressed materially;
- async `aread()` removes one full-size intermediate Rust allocation or is explicitly rejected if PyO3 cannot do so safely.

## Acceptance criteria

- [ ] Buffered core HeaderMap transfers into PyHeaders after metadata extraction.
- [ ] Streaming core HeaderMap transfers into PyHeaders after metadata extraction.
- [ ] `iter_bytes`, `iter_text`, and `iter_lines` no longer prebuild all Python objects/lists.
- [ ] Iterator values and exceptions match existing tests plus new multibyte/CRLF/empty/large controls.
- [ ] Async `aread()` intermediate Vec is removed safely or explicitly rejected with evidence.
- [ ] No native Python API manifest or typing drift.
- [ ] HTTPX 0.28.1 and HTTPX2 2.12.0 compatibility gain no new exception.
- [ ] No unsafe code or new production dependency.
- [ ] Python performance/RSS evidence is recorded.
- [ ] Tier 1 is green.

## Execution record — 2026-09-20

Buffered and streaming response conversion now transfers the ordinary core
HeaderMap after metadata extraction; history entries retain the intentional
clone needed for independent response objects. Buffered `iter_bytes`,
`iter_text`, and `iter_lines` are private lazy iterators: construction and
first yield do not prebuild the remaining Python values, repeated iterators
remain independent, and UTF-8/CRLF/empty-body behavior is covered by focused
tests. Async `aread()` now passes the collected bytes directly to PyO3 rather
than first copying them into a second full-size Rust `Vec`.

The renewed loopback controls measured buffered byte/text/line construction
plus first-use at approximately 12.0 us, 6.38 ms, and 5.58 ms, with full
consumption at approximately 0.41, 23.51, and 50.40 ms and peak RSS of about
54.1, 55.7, and 69.9 MiB. Full line traversal retains the expected per-item
Python-call cost of a lazy iterator; the previous eager-list path was not
reintroduced because it retained the entire Python object list and violated
the bounded-memory objective. Async 1 KiB/1 MiB/4 MiB `aread()` controls and
the exact Python `bytes` type assertion passed. Native API/typing checks,
HTTPX 0.28.1/2.12.0 compatibility, and Tier 1/extended suites passed with no
new export, exception, or return-type drift. No unsafe code or dependency was
added.
