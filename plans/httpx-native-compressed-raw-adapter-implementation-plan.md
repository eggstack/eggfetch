# HTTPX 0.28.1 Native Compressed-Raw Adapter — Corrective Closure

Status: ready for implementation handoff

Date: 2026-08-05

Audited baseline: `63d839a0405ca4b89f6f1f43a1c57e537db3f0be`

Pinned reference: `httpx==0.28.1`

Compatibility designation: `Stage C candidate`

Depends on:

- `plans/httpx-parity-raw-stream-final-corrective-closure.md`
- `plans/httpx-parity-correction-status.md`

## Objective

Close the remaining native compressed-response raw-stream gap without adding a second decompression stack, duplicating response bodies, or redesigning EggFetch transport and pooling.

The previous corrective pass fixed the compatibility wrapper’s deterministic raw-stream state, accounting, chunk adaptation, buffered-response, modality, and finalization behavior. It correctly stopped when the built-in Rust transport exposed only the post-decompression stream to Python.

This pass introduces the smallest core-owned selection boundary needed for a native streaming response to be consumed exactly once in one of two modes:

1. **raw mode** — yield the encoded wire-body bytes;
2. **decoded mode** — yield the existing transparently decoded bytes.

The mode is selected by the first body-consuming operation. It is not legal to consume both representations from one response.

This is a narrow adapter closure, not a new response architecture program.

## Accepted state that must remain closed

Do not reopen the following work unless a new executable regression directly demonstrates a failure:

- redirect Cookie regeneration and cross-origin containment;
- retained buffered and reconstructable multipart body replay;
- pre-dispatch rejection of unreplayable retained bodies;
- four-phase timeout conversion;
- query serialization and request-local Cookie handling;
- hop-local elapsed timing;
- pure-Python raw consumption-state timing;
- raw source-byte accounting before split/coalesce adaptation;
- buffered raw rejection with repeatable decoded reads;
- raw `chunk_size=None` default;
- sync/async wrong-modality behavior;
- normal-exhaustion versus partial-finalization behavior;
- compact Tier 1 validation structure;
- one routine Ubuntu CI job;
- manual crates.io release cadence;
- existing manually dispatched PyPI wheel workflow;
- `Stage C candidate` designation.

## Confirmed blocker

The current response pipeline performs these operations in order:

1. transport produces a streaming encoded body;
2. `response_decode::apply_decompression()` replaces that stream with a decoder-wrapped stream;
3. `apply_read_timeout_and_lease()` attaches timeout and pool-lease handling;
4. the Python binding calls `Response::bytes_stream()` and stores only the resulting stream;
5. both native `iter_raw()` and native decoded iteration therefore see the decoded stream for compressed responses.

The compatibility wrapper cannot reconstruct encoded bytes from decoded bytes. A Python decompressor would solve the opposite problem and would duplicate compression ownership. Disabling decompression for the request would make raw iteration possible but would break decoded iteration on the same response and would require choosing behavior before the caller selects an iterator.

The missing capability is therefore not a Python algorithm. It is a **deferred one-shot raw-or-decoded stream selection boundary owned by core**.

## Preferred implementation shape

The preferred design is a small deferred-decode response-body state, for example:

```text
ResponseBody::EncodedStreaming {
    stream,
    lease,
    content_encoding,
    decompression_limit,
}
```

The exact name may differ, but the semantics must be equivalent:

- the original encoded stream remains the single authoritative source;
- `bytes_stream()` selects decoded mode and wraps the source in the existing decoder chain;
- a new narrow `raw_bytes_stream()` selects raw mode and returns the encoded source unchanged;
- both methods consume the body state atomically/uniquely;
- a second selection returns the existing consumed-body error;
- the pool lease follows whichever stream is selected;
- read timeout applies to source reads in either mode;
- no stream is cloned, replayed, teed, or buffered merely to offer both choices.

A small private helper or internal enum is acceptable. A generalized multi-view response-body framework is not.

## Scope

### In scope

Core:

- one deferred-decode streaming-body representation or equivalent one-shot selector;
- one narrow raw streaming accessor on `eggfetch_core::Response` or its body;
- reuse of the existing `compression::decompress_stream()` implementation for decoded selection;
- correct pool-lease and read-timeout ownership in both selections;
- small unit tests for mode selection and body lifecycle.

Python binding:

- defer extraction of the core body until the first body-consuming operation;
- select raw mode for native `iter_raw()`/`aiter_raw()`;
- select decoded mode for native `iter_bytes()`/`aiter_bytes()`, `read()`/`aread()`, text, and line operations;
- preserve current wrapper-level accounting and state semantics;
- map second-consumption and closed-stream errors through the existing compatibility exception hierarchy.

Compatibility tests:

- actual loopback gzip raw and decoded comparisons against HTTPX 0.28.1;
- sync and async native streaming paths;
- incremental raw accounting and chunk adaptation;
- partial consumption and explicit close;
- immediate source failure and async cancellation fixtures;
- correction of the incomplete async second-iterator differential check;
- full pinned-suite, API-oracle, routine validation, and final CI evidence.

Documentation:

- exact-SHA status closure;
- bounded compatibility wording;
- narrowly documented core raw-stream accessor if it is public.

### Out of scope

- preserving two independently consumable body streams;
- replaying or cloning response bodies;
- teeing the transport stream into memory, disk, or channels;
- eager full-body buffering;
- Python gzip, deflate, Brotli, or Zstandard implementation;
- replacing `async-compression`;
- a second HTTP client or transport path;
- transport, TLS, HTTP/2, HTTP/3, proxy, retry, redirect, or pool redesign;
- changing request decompression configuration semantics;
- broad public API additions unrelated to raw selection;
- support for reading raw after decoded consumption or decoded after raw consumption;
- HTTPX version rebasing;
- Trio or AnyIO support;
- new CI workflows, jobs, matrices, platforms, or scheduled runs;
- moving the complete compatibility suite into routine CI;
- new evidence schemas, registries, dashboards, or qualification programs;
- release automation;
- compatibility promotion beyond `Stage C candidate`.

# Required invariants

1. The encoded transport body remains single-owner and single-consumption.
2. Raw and decoded selection are mutually exclusive.
3. Decoded behavior continues to use the existing core decompressor.
4. Python does not decode or re-encode content.
5. Raw mode yields the exact encoded bytes received from the transport after protocol framing removal.
6. Decoded mode yields the same decoded bytes as before this change.
7. Pool permits remain held until the selected stream terminates or is dropped/closed.
8. Read timeout remains effective in raw and decoded mode.
9. Cancellation and explicit close release the source and lease once.
10. No full-body buffering is introduced for streaming responses.
11. Existing uncompressed behavior remains unchanged except for the new internal selection route.
12. Existing buffered response behavior remains unchanged.
13. Compatibility wrapper state changes remain at the pinned HTTPX observation points.
14. Routine CI remains the existing single Tier 1 command.
15. The status remains open until full closure evidence exists.

# Track 0 — Establish failing native reference cases

## 0.1 Preserve the current executable mismatch

Retain the existing gzip loopback case as a failing-oracle fixture while implementing the adapter, but invert its final expectation only after raw parity is implemented.

Before production changes, capture for HTTPX and EggFetch:

- response headers relevant to content encoding and length;
- bytes returned by raw iteration;
- bytes returned by decoded iteration on a separate response;
- `num_bytes_downloaded` after the first raw output and after exhaustion;
- `is_stream_consumed` and `is_closed` at the existing lifecycle checkpoints.

Use separate requests for raw and decoded observations. One response remains one-shot.

## 0.2 Pin the runtime explicitly

The direct differential module must continue to fail immediately unless:

```python
httpx.__version__ == "0.28.1"
```

Expected results must be produced by executing HTTPX, not copied into candidate-only assertions.

## 0.3 Add sync and async compressed-native cases

At minimum, add actual built-in transport cases for:

- gzip sync raw;
- gzip async raw;
- gzip sync decoded;
- gzip async decoded;
- raw `chunk_size=None`;
- raw split adaptation with a small positive `chunk_size`;
- partial raw consumption followed by explicit response close;
- normal raw exhaustion;
- decoded read after selecting decoded mode;
- second body selection after raw mode;
- second body selection after decoded mode.

The loopback server must send a deterministic compressed payload and explicit `Content-Encoding` and `Content-Length` headers.

## 0.4 Resolve header behavior deliberately

Directly record whether HTTPX preserves wire `Content-Encoding` and `Content-Length` headers on streamed responses.

Do not silently alter EggFetch core’s existing decoded-header policy during this pass.

Use this decision rule:

- if compatibility header parity is already an explicit stage-bounded difference, keep it documented and limit this pass to body bytes;
- if it is not documented and the compatibility surface claims parity, preserve the original wire header metadata narrowly for the Python compatibility response without changing default Rust-core header behavior;
- do not broaden this into general response-header redesign.

### Track 0 acceptance criteria

- The current gzip mismatch is executable against HTTPX 0.28.1.
- Sync and async native paths are both represented.
- Raw and decoded observations use separate one-shot responses.
- Incremental state and accounting are observed, not only final body equality.
- Header behavior is either matched or explicitly bounded under existing governance.
- No candidate-only expectation is treated as the oracle.

# Track 1 — Add a deferred one-shot selection boundary in core

## 1.1 Preserve the encoded source through response processing

Change compressed streaming response handling so `apply_decompression()` no longer irreversibly replaces the only encoded source before a consumer selects a mode.

Preferred behavior:

- uncompressed streaming bodies remain ordinary streaming bodies;
- compressed streaming bodies become a deferred-decode body carrying the encoded stream and existing decoder parameters;
- compressed buffered bodies may continue to decode eagerly because raw iteration on buffered compatibility responses is rejected;
- unsupported encodings and decompression-limit configuration remain validated using existing logic.

Do not duplicate the encoded stream.

## 1.2 Provide decoded selection through the existing API

`Response::bytes_stream()` must preserve its existing decoded semantics.

For a deferred compressed body it should:

1. atomically take the encoded source;
2. construct the existing decoder chain using `compression::decompress_stream()`;
3. attach or preserve the pool lease;
4. return the decoded stream;
5. mark the body consumed so later raw or decoded selection fails.

Existing `bytes()` and `text()` paths should continue to consume decoded bytes through the same authoritative route.

## 1.3 Add one narrow raw accessor

Add the smallest public accessor required by the Python crate, preferably:

```rust
pub fn raw_bytes_stream(&mut self) -> Result<BoxBytesStream>
```

Equivalent naming is acceptable if consistent with the repository.

For a deferred compressed body it must:

1. atomically take the encoded source;
2. bypass decompression;
3. preserve read timeout and lease ownership;
4. return the raw stream;
5. mark the body consumed.

For an ordinary uncompressed streaming body, raw and decoded bytes are identical and the accessor may return the same source path.

For consumed bodies, return the existing body-consumed error. Do not add a new broad error taxonomy.

## 1.4 Keep timeout ordering correct

The read timeout must govern raw transport-body progress in both modes.

Acceptable implementations include:

- applying the read-timeout wrapper to the encoded source before storing deferred decode state; or
- storing timeout configuration with the deferred state and applying the same wrapper at mode selection.

The implementation must not apply two independent read-timeout wrappers to decoded mode.

Add a focused test that proves a stalled raw stream still respects read timeout if a suitable deterministic core fixture already exists. Do not create a long wall-clock test framework solely for this pass.

## 1.5 Preserve lease ownership

The pool lease must travel with the selected stream and release when:

- raw stream reaches EOF;
- decoded stream reaches EOF;
- selected stream returns an error;
- selected stream is dropped after partial consumption;
- the response is closed before selection;
- the binding cancels or drops its iterator.

Use the existing `LeasedResponseStream` pattern. Do not invent a second pool guard.

## 1.6 Core unit tests

Add small tests for:

- compressed deferred body selects raw exactly once;
- compressed deferred body selects decoded exactly once;
- raw then decoded fails;
- decoded then raw fails;
- uncompressed raw selection remains pass-through;
- buffered decoded behavior remains unchanged;
- lease/drop behavior remains single-release where current test utilities permit observation;
- decompression errors remain mapped through existing error variants.

### Track 1 acceptance criteria

- The original encoded stream survives until first body selection.
- `bytes_stream()` remains decoded-authoritative.
- `raw_bytes_stream()` returns encoded bytes.
- Selection is one-shot and mutually exclusive.
- No stream clone, tee, replay, or full-body buffer exists.
- Existing decompression code is reused.
- Read timeout and pool lease remain effective.
- Uncompressed and buffered regressions are absent.

# Track 2 — Adapt the Python native streaming response

## 2.1 Stop eagerly extracting only `bytes_stream()`

`PyStreamingResponse::from_core_response()` currently consumes `Response::bytes_stream()` immediately and stores only that stream.

Replace this with one narrow mode-capable holder until first consumption. Acceptable shapes include:

- `Mutex<Option<eggfetch_core::Response>>`; or
- a small core body-selection handle returned by `Response`.

Prefer the shape with the fewest new public core types.

Metadata may still be copied into Python fields during construction. The body owner itself must remain available for later raw-or-decoded selection.

## 2.2 Centralize mode selection

Replace the current generic `take_stream()` with an explicit internal selection method, conceptually:

```text
take_stream(Decoded)
take_stream(Raw)
```

Decoded selection is used by:

- `iter_bytes()`;
- `aiter_bytes()`;
- `read()`;
- `aread()`;
- text iteration and reads;
- line iteration.

Raw selection is used only by:

- `iter_raw()`;
- `aiter_raw()`.

Do not allow an iterator constructor to select both paths or fall back from raw to decoded silently.

## 2.3 Preserve native state semantics

The binding must continue to enforce:

- iterator construction alone does not consume until the pinned first-advance point where applicable;
- first advance selects the body and marks it consumed;
- a second body-consuming operation raises the mapped consumed exception;
- closed response raises the mapped closed exception in reference order;
- normal exhaustion closes;
- partial iterator finalization leaves explicit response close behavior aligned with the pinned reference;
- cancellation does not resurrect the body or permit another selection.

Do not let the core body state and Python `body_state` disagree.

## 2.4 Raw accounting remains wrapper-authoritative

Once `raw_bytes_stream()` returns true encoded source chunks:

- the compatibility wrapper continues counting source chunks before adaptation;
- the native iterator must expose unadapted raw chunks when called with `chunk_size=None`;
- split/coalesce adaptation remains owned once by `_RawByteChunker`;
- compressed raw `num_bytes_downloaded` must match HTTPX after first output and exhaustion.

Do not count decoded lengths or `Content-Length` as a substitute for consumed raw bytes.

## 2.5 Fix incomplete async differential execution

The current async partial-finalization differential constructs the second async iterator without advancing it. Because async-generator code executes on `anext()`, this does not prove the consumed guard.

Replace it with an awaited observation that advances the second iterator and captures the actual exception and state.

## 2.6 Add immediate failure and cancellation cases

Add direct reference cases for:

- sync source raises before yielding any bytes;
- async source raises before yielding any bytes;
- async raw iteration is cancelled while waiting for the next source item;
- explicit `aclose()` after cancellation;
- source/response close counts and public state after cancellation.

Use deterministic synchronization primitives. Do not use arbitrary sleeps as the primary assertion mechanism.

### Track 2 acceptance criteria

- Python defers raw-or-decoded selection until first body consumption.
- Raw native iterators select the core raw accessor.
- Decoded operations select the existing decoded accessor.
- Core and Python consumed/closed states remain consistent.
- Compressed raw accounting is source-authoritative.
- The async second-iterator test actually executes the guard.
- Immediate-failure and cancellation behavior match HTTPX.
- No Python decompression or re-encoding is added.

# Track 3 — Close compressed native differential coverage

## 3.1 Replace the unresolved mismatch assertion

Delete or rewrite the current test that asserts:

```python
expected_raw != actual_raw
```

Final behavior must assert equality with HTTPX for encoded raw bytes and accounting.

Do not keep both an “unresolved” test and a new parity test.

## 3.2 Required native compressed matrix

For both sync and async built-in clients, verify:

### Raw mode

- joined raw bytes equal HTTPX raw bytes;
- raw bytes differ from the known decoded payload;
- gzip signature/payload is not reconstructed in Python;
- `num_bytes_downloaded` matches after first output;
- final raw count matches HTTPX;
- positive chunk splitting preserves source accounting;
- `chunk_size=None` preserves unadapted source semantics;
- normal exhaustion closes;
- partial finalization and explicit close match HTTPX.

### Decoded mode

- decoded bytes equal HTTPX decoded bytes;
- decoded bytes equal the known original payload;
- existing text and line decoding still work on compressed responses;
- decoded operations cannot be followed by raw selection;
- raw selection cannot be followed by decoded operations.

### Lifecycle/error mode

- immediate source failure;
- failure after one raw chunk;
- cancellation while pending;
- explicit close before selection;
- explicit close after partial raw consumption;
- second raw iterator;
- decoded read after partial raw consumption;
- raw iterator after decoded read.

## 3.3 Compression-format scope

Gzip is mandatory because it reproduces the blocker.

Add one deflate case only if the existing loopback fixture and compiled features make it inexpensive. Brotli and Zstandard permutations are not required for closure unless the implementation contains format-specific branching outside the existing decompressor.

The adapter itself must remain format-agnostic.

## 3.4 Consolidate tests

Ownership should remain:

- `test_raw_stream_httpx_differential.py` — pinned compatibility truth;
- `test_raw_stream_lifecycle.py` — compact candidate/native integration regressions not duplicated by the differential matrix;
- `test_corrective_kernel.py` — only the smallest Tier 1 recurrence guards.

Delete redundant cases rather than increasing test volume indiscriminately.

### Track 3 acceptance criteria

- Native gzip raw bytes equal HTTPX in sync and async paths.
- Native decoded bytes remain correct in sync and async paths.
- Raw and decoded selections are mutually exclusive.
- Incremental accounting matches HTTPX.
- Cancellation and immediate failure are directly compared.
- The obsolete unresolved assertion is removed.
- Test duplication remains bounded.

# Track 4 — Verification and closure evidence

## 4.1 Focused native adapter validation

From a clean environment with the extension rebuilt, run at minimum:

```sh
python -m pip install -r compat/httpx/0.28.1/requirements.txt
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 \
  maturin develop -m crates/eggfetch-python/Cargo.toml
python -m pytest \
  crates/eggfetch-python/tests/compat/test_raw_stream_httpx_differential.py \
  crates/eggfetch-python/tests/compat/test_raw_stream_lifecycle.py \
  crates/eggfetch-python/tests/compat/test_corrective_kernel.py \
  crates/eggfetch-python/tests/compat/test_response.py \
  crates/eggfetch-python/tests/compat/test_response_metadata_parity.py \
  -q --strict-markers
```

Record exact passed, failed, skipped, and xfailed counts.

## 4.2 Core-focused validation

Run focused Rust tests for the changed body, decompression, response, pipeline, and binding modules before the aggregate command.

Use existing package/test filters. Do not add a new test runner or evidence script.

## 4.3 Routine validation

Run the existing command:

```sh
./scripts/check.sh
```

A local environment stall is not passing evidence. Diagnose whether it is deterministic. Retry in a clean environment and rely on the existing CI job for routine final-tree confirmation.

Do not weaken, skip, or split routine checks merely to obtain a green result.

## 4.4 Full pinned compatibility

Run:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

This run must complete. Record current exact counts. Historical `1389 passed` or partial-run counts are not closure evidence for the new executable SHA.

## 4.5 Existing API oracle

Run the existing manifest generation and comparison commands.

Required result:

- zero unexplained differences;
- zero stale allowed entries;
- zero unresolved differences;
- no new broad exception hiding raw-stream behavior.

If a new public core method does not affect the Python compatibility API manifest, do not create a new allowlist entry for it.

## 4.6 Final CI

Use the existing single routine CI job only.

The final status must record:

- final executable SHA;
- any later documentation-only SHA;
- exact CI checked-out SHA;
- workflow run ID;
- job ID;
- conclusion;
- confirmation that the workflow still invokes only the existing Tier 1 path.

Do not add the full compatibility suite or API oracle to routine CI.

### Track 4 acceptance criteria

- Focused adapter and differential tests pass.
- Core-focused tests pass.
- `./scripts/check.sh` completes successfully.
- Full pinned compatibility completes successfully with exact current counts.
- API oracle is clean.
- Existing CI passes on the exact final tree or an explicitly identified documentation-only descendant.
- No validation architecture expansion occurs.

# Track 5 — Truthful status and bounded documentation

## 5.1 Update the status only after executable closure

Update `plans/httpx-parity-correction-status.md` with:

- adapter-plan baseline SHA `63d839a0405ca4b89f6f1f43a1c57e537db3f0be`;
- direct differential test SHA if separate;
- core adapter implementation SHA;
- Python binding implementation SHA if separate;
- final executable SHA;
- exact focused-test result;
- exact routine result;
- exact full pinned-suite result;
- exact API-oracle result;
- final CI tree/run/job identifiers;
- documentation-only descendants, if any.

Remove the active “blocked by missing core raw-body boundary” statement only after native compressed raw parity passes.

Keep the old blocker record as historical context if useful, clearly labeled superseded.

## 5.2 Final designation

Allowed final wording:

**Stage C candidate — deterministic raw-stream corrective closure complete for the documented HTTPX 0.28.1 asyncio-supported surface.**

Do not claim:

- unrestricted HTTPX replacement;
- complete backend parity;
- Trio/AnyIO parity;
- release readiness based solely on this work;
- compatibility with newer HTTPX versions.

## 5.3 Documentation scope

Update only directly affected documents, likely:

- `plans/httpx-parity-correction-status.md`;
- `README.md`, if its raw-stream caveat changes;
- `docs/architecture/core-body-streaming.md`;
- `docs/architecture/python-bindings.md`;
- `.skills/python-bindings.md`, only if implementation guidance changed;
- public Rust API docs for the narrow raw accessor.

Do not add another roadmap, registry, evidence document type, or release checklist.

### Track 5 acceptance criteria

- Status no longer claims an active raw-body blocker after parity is proven.
- Every result is exact-SHA-bound.
- Current and historical evidence are clearly separated.
- Compatibility remains Stage C candidate.
- Documentation reflects one-shot raw-or-decoded selection accurately.
- No unrelated documentation churn occurs.

# Global acceptance criteria

This line is complete only when all of the following are true:

1. A compressed native streaming response retains its encoded source until first body selection.
2. Raw selection yields the same encoded bytes as HTTPX 0.28.1.
3. Decoded selection yields the same decoded bytes as HTTPX 0.28.1.
4. Raw and decoded selections are mutually exclusive and one-shot.
5. No response body is cloned, replayed, teed, or eagerly buffered to support selection.
6. Existing core decompression remains the only decompression implementation.
7. Pool lease and read timeout work in both modes.
8. Raw incremental byte accounting matches HTTPX after first output and exhaustion.
9. Raw chunk adaptation remains bounded and occurs once.
10. Sync and async native gzip cases pass.
11. Immediate failure and async cancellation match HTTPX.
12. The async second-iterator differential actually advances the iterator.
13. The obsolete explicit-mismatch assertion is removed.
14. Uncompressed native streaming remains unchanged.
15. Buffered compatibility behavior remains unchanged.
16. Focused tests pass.
17. Routine validation completes successfully.
18. Full pinned compatibility completes successfully.
19. API oracle is clean.
20. Existing single-job CI passes on the final tree.
21. Status is exact-SHA- and CI-bound.
22. Compatibility remains Stage C candidate.
23. No out-of-scope transport, CI, or release expansion occurs.

# Rejection criteria

Reject the implementation if any of the following occurs:

- Python implements decompression or re-encoding;
- the request is dispatched twice to obtain raw and decoded forms;
- the body is cloned, replayed, or teed;
- the full compressed body is buffered to make raw iteration possible;
- raw mode still yields decoded bytes;
- decoded mode is implemented by decoding bytes previously exposed through raw mode;
- raw and decoded reads both succeed on one response;
- `Content-Length` is used as a substitute for incremental raw accounting;
- the existing decompressor is duplicated or bypassed for decoded operations;
- pool leases release before the selected stream is terminally owned or closed;
- read timeout is lost in raw mode;
- the Python and core consumed states can diverge;
- cancellation permits a second body selection;
- the unresolved mismatch test remains as passing evidence;
- candidate-only tests replace the HTTPX oracle;
- a MockTransport response is labeled native proof;
- full pinned compatibility does not complete;
- local stall is recorded as pass;
- a new CI workflow, job, matrix, scheduled run, or evidence system is added;
- routine CI is weakened;
- release automation is added;
- the compatibility designation is promoted.

# Stop conditions

Stop and document the exact blocker before expanding architecture if:

1. preserving the encoded stream requires cloning or replaying a non-replayable transport body;
2. the pool lease cannot follow a deferred selected stream without changing pool semantics globally;
3. read-timeout ownership cannot be preserved without double wrapping or changing timeout meaning;
4. `eggfetch_core::Response` cannot expose one narrow raw stream method without a broad public response rewrite;
5. direct HTTPX behavior conflicts between sync and async paths in a way not covered by the pinned runtime fixture;
6. changing header metadata would alter default Rust-core behavior outside compatibility scope;
7. a fix requires Python decompression, a second request, or a second transport.

When a stop condition is reached, record:

- the exact unavailable ownership boundary;
- the type/module that owns it;
- the smallest proposed additional API;
- why wrapper-side inference is incorrect;
- the failing direct HTTPX fixture;
- the reason broader changes were rejected.

Do not mark closure complete under a stop condition.

# Suggested commit decomposition

1. `test: pin native compressed raw differential`
   - add sync/async gzip raw and decoded observations;
   - add header decision fixture;
   - add immediate-failure/cancellation cases;
   - correct async second-iterator execution;
   - keep status open.

2. `feat(core): defer compressed response stream selection`
   - deferred encoded streaming body;
   - decoded `bytes_stream()` selection;
   - narrow `raw_bytes_stream()` selection;
   - timeout and lease ownership;
   - core unit tests.

3. `fix(python): select native raw or decoded response body`
   - defer core response/body extraction;
   - mode-specific stream selection;
   - state/error mapping;
   - compressed raw accounting.

4. `test: close native compressed raw parity`
   - invert/remove unresolved mismatch assertion;
   - pass full native matrix;
   - consolidate candidate-only tests;
   - keep Tier 1 compact.

5. `docs: bind native raw closure evidence`
   - exact SHAs and commands;
   - full pinned result;
   - API oracle;
   - final CI identifiers;
   - bounded Stage C candidate wording.

Combining commits 2 and 3 is acceptable if the core and binding boundary cannot compile independently. Keep the direct failing oracle distinguishable from the production fix whenever practical.

# Handoff checklist

Before implementation:

- [ ] Confirm `main` is still at or descended only by understood changes from `63d839a0405ca4b89f6f1f43a1c57e537db3f0be`.
- [ ] Read `AGENTS.md` and relevant Rust/Python skills.
- [ ] Reproduce the native gzip raw mismatch against HTTPX 0.28.1.
- [ ] Record raw and decoded header behavior.
- [ ] Keep the status open.

During core work:

- [ ] Preserve one encoded source until first selection.
- [ ] Keep decoded `bytes_stream()` behavior.
- [ ] Add narrow raw stream selection.
- [ ] Enforce one-shot mutual exclusion.
- [ ] Preserve read timeout.
- [ ] Preserve pool lease ownership.
- [ ] Avoid clone, tee, replay, or eager buffering.
- [ ] Reuse existing decompression.

During binding work:

- [ ] Stop eagerly extracting only the decoded stream.
- [ ] Select decoded mode for all decoded consumers.
- [ ] Select raw mode only for raw iterators.
- [ ] Keep core and Python state aligned.
- [ ] Count encoded source chunks before adaptation.
- [ ] Preserve normal/partial close semantics.
- [ ] Fix async second-iterator execution.
- [ ] Add immediate-failure and cancellation parity.

Before closure:

- [ ] Native gzip raw sync equals HTTPX.
- [ ] Native gzip raw async equals HTTPX.
- [ ] Native gzip decoded sync equals HTTPX.
- [ ] Native gzip decoded async equals HTTPX.
- [ ] Raw/decoded mutual exclusion passes.
- [ ] Focused adapter tests pass.
- [ ] Core-focused tests pass.
- [ ] `./scripts/check.sh` completes successfully.
- [ ] Full pinned compatibility completes successfully.
- [ ] API oracle is clean.
- [ ] Existing CI passes on the final pushed tree.
- [ ] Status records exact implementation and CI SHAs.
- [ ] No CI or release expansion occurred.
- [ ] Final designation remains Stage C candidate.

# Closure statement

This line is closed only when the built-in EggFetch transport preserves a compressed response’s encoded source until first one-shot body selection, native raw iteration yields the same encoded bytes and incremental accounting as HTTPX 0.28.1, decoded operations retain current behavior, all focused and full validation completes, and the exact final tree passes the existing single routine CI job.
