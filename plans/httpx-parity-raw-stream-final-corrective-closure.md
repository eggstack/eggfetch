# HTTPX 0.28.1 Raw Stream Parity — Final Corrective Closure

Status: ready for implementation handoff

Date: 2026-08-05

Audited baseline: `eb397395f8a2a0bf0621fbcd9deece98647a85cb`

Pinned reference: `httpx==0.28.1`

Current compatibility designation: `Stage C candidate`

Related implementation:

- `aec56c09f35c631491a54d01e87c75fc46a51cbb` — redirect Cookie security and retained-body replay
- `11eb77a7e121d4b83e75a1bd87ebf7ac240e9046` — prior raw-stream lifecycle implementation

Related plans:

- `plans/httpx-parity-final-closure-roadmap.md`
- `plans/httpx-parity-final-closure-02-raw-stream-lifecycle.md`
- `plans/httpx-parity-final-closure-03-verification-status-hygiene.md`

## Purpose

Close the remaining deterministic raw-response streaming differences between `eggfetch.compat.httpx` and the pinned HTTPX 0.28.1 asyncio-supported surface.

The prior pass materially improved raw iteration, but it did not satisfy its own pinned-reference acceptance criteria. Several candidate-only tests now encode EggFetch behavior that differs from HTTPX. This pass must correct the implementation and the tests together, then replace the current premature closure claim with exact-SHA evidence.

This is one coherent corrective pass. Do not create another roadmap, phase system, evidence framework, or compatibility program around it.

## Current repository state

The following portions of the previous line are accepted as closed and must not be reopened without a new demonstrated regression:

- redirect Cookie regeneration and cross-origin containment;
- retained buffered and reconstructable multipart body handling on 307/308;
- rejection of unreplayable retained bodies before a second dispatch;
- timeout conversion, query serialization, request-local Cookie preservation, and hop-local elapsed corrections from earlier passes;
- stale PR cleanup;
- one lightweight routine Ubuntu CI job;
- manual crates.io release policy and existing manually dispatched PyPI wheel workflow.

The remaining work is limited to raw `Response` iterator semantics and the truthfulness of the associated tests and status record.

# Confirmed residual defects

## 1. Live raw streams are marked consumed too late

Current `iter_raw()` and `aiter_raw()` set `_stream_consumed = True` only after normal completion or in `finally`.

Observable result:

- constructing the iterator leaves the response unconsumed, which is correct;
- advancing the iterator and receiving the first bytes still leaves `is_stream_consumed == False`, which is incorrect;
- the response becomes consumed only after exhaustion or iterator finalization.

HTTPX 0.28.1 marks a live stream consumed when iteration starts, before reading the first source chunk. After the first yielded bytes, the public state must already report consumed.

## 2. Raw byte accounting follows emitted wrapper chunks instead of consumed source chunks

Current accounting increments by each chunk yielded after split/coalesce adaptation.

HTTPX increments `num_bytes_downloaded` by each raw source chunk before chunk-size adaptation. This difference is observable during partial iteration.

Examples that must be captured directly against HTTPX:

- source emits `b"abcd"`, requested `chunk_size=1`: after the first one-byte output, HTTPX has already consumed and counted the four-byte source chunk;
- source emits `b"a"`, `b"b"`, `b"c"`, requested `chunk_size=3`: before the first three-byte output is yielded, all three source chunks have been counted.

Final totals may match while incremental public state is still wrong. End-of-stream-only assertions are insufficient.

## 3. Buffered responses incorrectly permit raw iteration

Current raw guards allow `iter_raw()` and `aiter_raw()` when `_content` exists even if the response stream was already consumed during buffering.

HTTPX buffered responses are consumed and closed by construction. Raw iteration on that already-consumed response raises the pinned stream exception. Buffered decoded reads remain repeatable.

Do not solve this by making buffered `.read()`, `.content`, or decoded iteration single-use.

## 4. Partial raw iterator finalization closes at the wrong point

Current raw methods use unconditional `finally` closure. Closing or finalizing the iterator after partial consumption therefore closes the entire response immediately.

HTTPX performs automatic response close after normal raw exhaustion. Partial iterator termination, explicit iterator-object finalization, source failure, and explicit response close have distinct observable behavior.

Direct runtime fixtures, not assumptions, must establish:

- whether iterator-object `close()`/`aclose()` closes the response;
- whether an early loop break closes it;
- whether source failure closes it;
- when explicit `response.close()`/`response.aclose()` is still required;
- whether elapsed is available at each point.

The implementation must match the reference rather than preserving the current unconditional-finally behavior.

## 5. Raw method defaults and stream modality remain inconsistent

Current raw signatures default `chunk_size` to `8192`; HTTPX 0.28.1 defaults raw iteration to `None`.

Current async raw iteration also accepts a compatibility sync iterator, and sync raw iteration can fail incidentally on an async-only source. HTTPX explicitly rejects sync iteration over async streams and async iteration over sync streams with the pinned runtime error behavior.

The direct differential matrix must also determine and enforce invalid `chunk_size` behavior for zero, negative, and non-integer values.

## 6. Existing tests are not a valid parity oracle

`test_raw_stream_lifecycle.py` is candidate-only and currently asserts at least two non-reference behaviors:

- `is_stream_consumed` remains false after the first yielded raw chunk;
- `num_bytes_downloaded` increments by adapted output length rather than consumed source length.

The existing pinned-HTTPX-required suite imports HTTPX but does not exercise the disputed raw lifecycle/accounting cases.

A large candidate-only test count does not establish compatibility. The incorrect expectations must be removed, replaced, or rewritten from direct HTTPX observations.

## 7. The closure status is presently overstated

`plans/httpx-parity-correction-status.md` currently states that final deterministic closure is complete. That statement is not supportable while the residual differences above remain.

The implementation pass must reopen the status before correction and mark it complete only after the final exact-SHA verification requirements in this plan are satisfied.

# Scope firewall

## In scope

- sync `Response.iter_raw()` behavior;
- async `Response.aiter_raw()` behavior;
- a very small private raw-source/chunk adaptation helper if it reduces duplication;
- public raw consumption state;
- public raw byte accounting;
- raw normal-exhaustion versus partial-finalization close behavior;
- buffered-response raw behavior;
- raw `chunk_size` default and validation;
- sync/async source-modality errors;
- direct HTTPX 0.28.1 differential fixtures;
- deletion or consolidation of incorrect and duplicative raw candidate-only tests;
- a compact Tier 1 regression kernel;
- exact-SHA status correction and existing single-job CI verification.

## Out of scope

- redirect, Cookie, timeout, query, request body, authentication, mount, or hook redesign;
- Rust core transport, pool, retry, TLS, HTTP/2, or HTTP/3 changes unless a narrowly demonstrated native raw-counter blocker requires a separately reviewed adapter change;
- new decompression logic in Python;
- a second networking stack;
- HTTPX version rebasing;
- Trio or AnyIO support;
- SOCKS, UDS, local-address, socket-option, or alternate backend work;
- new CI workflows, jobs, matrices, scheduled runs, or platform expansion;
- moving the full compatibility suite into routine CI;
- new evidence schemas, registries, qualification tooling, dashboards, or generated reports;
- release automation or release-cadence changes;
- compatibility promotion beyond `Stage C candidate`;
- broad refactoring of `Response`, transports, or native bindings.

# Required invariants

The implementation must preserve all of the following:

1. Raw and decoded byte paths remain structurally distinct.
2. Python does not implement content decompression already owned by the native layer.
3. A live response stream is one-shot.
4. Buffered decoded response content remains repeatable.
5. Consumption state changes at the same public observation point as HTTPX.
6. Raw byte accounting counts source/raw bytes exactly once.
7. Chunk adaptation does not eagerly buffer the full response.
8. Normal exhaustion, partial termination, source failure, and explicit response close remain distinguishable.
9. Sync and async behavior match each other where HTTPX matches and differ only where stream modality requires it.
10. Routine validation remains one bounded Tier 1 command.

# Track 0 — Reopen status and establish the executable oracle

## 0.1 Mark the line reopened before claiming a fix

In the first implementation commit that changes executable behavior or tests, update `plans/httpx-parity-correction-status.md` to record:

- audited corrective baseline `eb397395f8a2a0bf0621fbcd9deece98647a85cb`;
- this plan path;
- current status `raw-stream corrective closure in progress`;
- the prior redirect closure remains accepted;
- the prior raw-stream completion statement is superseded.

Do not delete historical SHAs. Label them as prior evidence rather than current closure proof.

## 0.2 Add a direct pinned-reference test module

Preferred file:

- `crates/eggfetch-python/tests/compat/test_raw_stream_httpx_differential.py`

The module must:

- import `httpx` directly;
- fail immediately unless `httpx.__version__ == "0.28.1"`;
- execute equivalent public-state cases against HTTPX and EggFetch;
- compare normalized public observations, not private HTTPX implementation fields;
- remain part of extended/manual compatibility validation, not routine CI.

Small local helper classes may provide deterministic sync and async source chunks and close counters. Keep them inside the test module unless an existing fixture module is clearly reusable.

## 0.3 Capture the complete disputed state matrix

For both sync and async raw iteration, capture:

- state immediately after response construction;
- state after iterator object construction without advancing;
- state during the first iteration step;
- first yielded chunk;
- `num_bytes_downloaded` after the first output;
- state after normal exhaustion;
- state after early break;
- state after iterator-object finalization;
- state after explicit response close;
- second raw-iterator behavior;
- decoded read behavior after partial raw consumption;
- `.content` behavior after partial raw consumption;
- source close count;
- response `is_closed`;
- elapsed availability;
- exception class and relevant stable message fragment.

## 0.4 Required source/chunk fixtures

At minimum, run the matrix with:

1. one source chunk larger than requested output size;
2. several source chunks smaller than requested output size;
3. `chunk_size=None`;
4. zero-length source;
5. empty chunks around non-empty chunks;
6. source exception after one chunk;
7. buffered `content=` response;
8. sync source passed to async iteration;
9. async source passed to sync iteration;
10. `chunk_size=0`;
11. negative `chunk_size`;
12. non-integer `chunk_size`.

Use an actual native streamed response for at least one sync and one async case. Mock/compat streams alone do not prove the native boundary.

### Track 0 acceptance criteria

- At least one test fails against the current baseline for each confirmed defect category.
- HTTPX is executed as the expected-behavior oracle.
- The fixture observes state after the first output, not only after exhaustion.
- Sync and async matrices are both present.
- Native streamed responses are exercised.
- No expected value is copied from EggFetch’s current implementation.

# Track 1 — Correct consumption and close-state transitions

## 1.1 Apply guards in pinned-reference order

At the first iteration step, match HTTPX guard ordering for:

- already consumed stream;
- already closed response;
- wrong stream modality.

Do not rely on incidental Python `TypeError` from calling `iter()` or `async for` on the wrong object.

## 1.2 Mark live consumption before reading the source

After guards pass and before the first source chunk is requested:

- mark the live response stream consumed;
- reset or initialize raw byte accounting according to the reference;
- leave iterator construction without advancement unconsumed if the reference does so.

The first successful `next()`/`anext()` must leave `is_stream_consumed == True`.

The same must hold when the source is empty or raises before yielding data, according to the direct reference fixture.

## 1.3 Remove unconditional close behavior where it diverges

Do not use a blanket `finally: close()` merely because it guarantees cleanup.

Implement the reference distinction among:

- normal exhaustion;
- early break;
- iterator-object close/finalization;
- source exception;
- async cancellation;
- explicit response close.

Normal exhaustion must still close exactly once. Explicit response close must remain idempotent.

## 1.4 Preserve primary exceptions

When the source raises:

- preserve the mapped primary exception;
- do not replace it with a close/finalization error unless HTTPX does;
- leave consumed/closed state consistent with the direct fixture;
- do not leak a native stream if the reference closes it at that point.

### Track 1 acceptance criteria

- Consumption flips at the same public point as HTTPX.
- Empty and immediately failing streams have reference-compatible state.
- Partial iterator termination no longer closes too early or too late.
- Normal exhaustion closes once.
- Explicit close remains idempotent.
- Source exceptions and cancellation preserve the primary failure.
- Sync and async results match the reference matrix.

# Track 2 — Make raw-source accounting authoritative

## 2.1 Count source chunks before output adaptation

For compatibility raw sources:

- increment `num_bytes_downloaded` exactly once for each raw source chunk consumed;
- increment before passing that chunk into split/coalesce adaptation;
- never increment again for chunks emitted from the adaptation buffer.

This must make partial observations match HTTPX for both splitting and coalescing cases.

## 2.2 Use an unadapted native source or authoritative native counter

Current native calls pass the requested `chunk_size` into the native raw iterator and then count wrapper outputs. That cannot prove source-byte accounting when the native iterator has already split or coalesced data.

Preferred approaches, in order:

1. request unadapted native raw chunks (`chunk_size=None`) and perform bounded adaptation once in the compatibility wrapper;
2. read an existing authoritative native raw-byte counter after each native output;
3. expose one narrow native adapter accessor if the counter already exists but is unavailable to Python.

Do not introduce a second transport implementation or broad native response redesign.

If no existing native primitive can provide raw source accounting without core redesign, trigger the stop condition below rather than fabricating parity.

## 2.3 Use one bounded chunk adapter

A small private helper may be introduced to share split/coalesce behavior between sync and async raw paths.

The helper must:

- accept raw source chunks incrementally;
- split large chunks;
- coalesce small chunks;
- flush a remainder exactly once;
- preserve `chunk_size=None` behavior;
- handle empty chunks according to HTTPX;
- validate invalid sizes according to HTTPX;
- never buffer the full body.

Do not create a general streaming framework.

## 2.4 Keep compressed/native accounting honest

For at least one gzip-compressed native response:

- raw iteration must yield compressed bytes;
- decoded iteration must yield decoded bytes;
- raw byte accounting must follow the compressed/raw source length;
- no Python decompressor may be added.

### Track 2 acceptance criteria

- Splitting case reports the full consumed source chunk after the first output where HTTPX does.
- Coalescing case reports all consumed source chunks before the first output.
- No raw byte is double-counted.
- Final totals remain correct.
- `chunk_size=None` preserves source chunk boundaries where the reference does.
- Native compressed raw accounting matches HTTPX.
- Memory remains bounded by source/output chunk adaptation, not response size.

# Track 3 — Correct buffered, default, and modality behavior

## 3.1 Match buffered raw behavior

For `Response(content=...)`:

- preserve HTTPX-compatible consumed and closed construction state;
- make raw iteration raise the pinned stream exception;
- preserve repeatable `.content`, `.read()`, and decoded iteration behavior;
- do not increment `num_bytes_downloaded` through an invalid raw re-iteration.

## 3.2 Match raw method defaults

Change public raw method defaults only as required to match HTTPX 0.28.1:

- `iter_raw(chunk_size=None)`;
- `aiter_raw(chunk_size=None)`.

Update API manifests or allowed differences only if the existing governance files actually track these signatures. A resolved mismatch should be removed from the active allowlist and recorded through the existing resolved-difference mechanism; do not invent a new ledger.

## 3.3 Match invalid `chunk_size` behavior

Use the direct fixture to match HTTPX for:

- zero;
- negative integer;
- float;
- string;
- boolean if behavior is not already implied by integer handling.

Do not silently translate invalid values to `8192` through truthiness logic.

## 3.4 Enforce stream modality

Match HTTPX when:

- sync raw iteration is attempted on an async-only stream;
- async raw iteration is attempted on a sync-only stream.

The response must not be partially consumed before the modality error unless the reference does so.

### Track 3 acceptance criteria

- Buffered raw iteration matches HTTPX while buffered decoded reads remain repeatable.
- Raw defaults match the pinned signatures.
- Invalid sizes raise the same exception class and stable message behavior.
- Wrong-modality calls raise deliberate reference-compatible errors.
- No unrelated public signature changes occur.

# Track 4 — Replace incorrect tests and reduce duplication

## 4.1 Remove assertions that protect non-reference behavior

At minimum, delete or reverse tests that assert:

- the stream remains unconsumed after the first raw output;
- accounting advances only by emitted wrapper output length;
- iterator-object finalization closes the response when HTTPX does not;
- buffered raw iteration succeeds when HTTPX raises.

Do not retain both old and new tests under different names.

## 4.2 Consolidate the oversized candidate-only lifecycle module

`test_raw_stream_lifecycle.py` should be reduced to candidate regression coverage not already expressed by the direct differential module.

Preferred result:

- direct HTTPX differential module owns disputed compatibility truth;
- candidate-only module owns focused internal regressions and native integration cases;
- Tier 1 kernel owns only a few deterministic high-value cases.

Delete redundant permutations rather than increasing the test count again.

## 4.3 Keep Tier 1 compact

Extend `test_corrective_kernel.py` only with the smallest deterministic set needed to prevent recurrence without installing HTTPX in routine CI.

Maximum recommended additions:

1. consumption is true after the first raw output;
2. split-source accounting reflects source bytes;
3. buffered raw iteration raises;
4. one sync/async modality guard, if inexpensive.

The full direct differential matrix remains Tier 2/manual.

## 4.4 Require actual native stream coverage

A test named `native` must reach the built-in/native transport stream path. A `MockTransport` returning a compatibility `Response(content=...)` is not sufficient proof of native raw streaming.

Use the repository’s existing local HTTP server/native stream fixtures. Do not add external network dependencies.

### Track 4 acceptance criteria

- No known incorrect expectation remains.
- Direct differential tests execute HTTPX 0.28.1.
- Candidate-only duplication is reduced, not expanded.
- Tier 1 remains fast and bounded.
- Native raw coverage reaches the actual built-in/native path.
- Test names accurately describe the exercised boundary.

# Track 5 — Verification and truthful closure status

## 5.1 Focused correction commands

Run from a clean environment with the extension rebuilt:

```sh
python -m pip install -r compat/httpx/0.28.1/requirements.txt
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 \
  maturin develop -m crates/eggfetch-python/Cargo.toml
python -m pytest \
  crates/eggfetch-python/tests/compat/test_raw_stream_httpx_differential.py \
  crates/eggfetch-python/tests/compat/test_raw_stream_lifecycle.py \
  crates/eggfetch-python/tests/compat/test_corrective_kernel.py \
  -q --strict-markers
```

If the implementation uses a differently named direct differential file, record the exact path and command in status.

## 5.2 Routine validation

Run the existing command only:

```sh
./scripts/check.sh
```

Do not modify CI to run the complete compatibility suite.

## 5.3 Extended pinned compatibility

Run:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Record passed, failed, skipped, and xfailed counts from this exact run. Do not copy historical totals.

## 5.4 Existing API oracle

Run the repository’s existing manifest generation and comparison commands. Resolve only differences caused by this pass.

Expected closure result:

- zero unexplained differences;
- zero stale allowed entries;
- zero active entries for a signature mismatch that this pass resolves;
- no new broad or permanent exception added to hide raw behavior.

## 5.5 Exact-SHA status update

After all executable changes are committed, update `plans/httpx-parity-correction-status.md` with:

- corrective baseline SHA;
- direct differential test commit SHA;
- final executable implementation SHA;
- exact focused command and result;
- exact routine command and result;
- exact full pinned-suite command and result;
- exact API-oracle result;
- final pushed tree SHA checked out by CI;
- CI run and job identifiers;
- any documentation-only commits after the executable boundary.

The status must state that redirect closure remained complete and raw-stream closure was reopened and then corrected.

## 5.6 Preserve designation boundaries

The final designation may be:

`Stage C candidate — deterministic raw-stream corrective closure complete for the documented HTTPX 0.28.1 asyncio-supported surface.`

Do not use:

- unrestricted drop-in replacement;
- full HTTPX replacement;
- complete support for every transport/backend;
- release-ready solely because this pass closes.

### Track 5 acceptance criteria

- Focused direct differential tests pass.
- Existing Tier 1 passes.
- Full pinned compatibility suite passes with exact current counts.
- API oracle is clean under existing governance.
- CI passes on the exact final pushed tree or an explicitly related documentation-only descendant.
- Status no longer relies on the superseded `11eb77a` evidence as final proof.
- Compatibility remains Stage C candidate.

# Global acceptance criteria

This line is complete only when all of the following are true:

1. `is_stream_consumed` matches HTTPX after first sync and async raw iteration steps.
2. Iterator construction without advancement matches HTTPX.
3. Empty and immediately failing sources match HTTPX state transitions.
4. Incremental byte accounting counts consumed raw source chunks, not adapted outputs.
5. Split and coalesce cases match HTTPX after the first output.
6. Buffered raw iteration matches HTTPX without breaking repeatable decoded reads.
7. Normal exhaustion, partial termination, iterator finalization, source failure, cancellation, and explicit close match the reference.
8. Raw method defaults and invalid-size behavior match HTTPX 0.28.1.
9. Sync/async wrong-modality behavior matches HTTPX.
10. At least one actual native sync and async streamed response is covered.
11. Raw and decoded bytes remain distinct for a compressed native response.
12. Incorrect candidate-only assertions are removed.
13. The candidate-only raw lifecycle suite is consolidated rather than enlarged.
14. Tier 1 remains compact and uses the existing single command.
15. Full pinned compatibility and the existing API oracle pass.
16. Final status is exact-SHA-bound and CI-bound.
17. No out-of-scope subsystem or release process changes.

# Rejection criteria

Reject the implementation if any of the following occurs:

- tests still expect `is_stream_consumed == False` after raw bytes are yielded;
- accounting is asserted only after exhaustion;
- accounting increments from adapted output chunks;
- buffered raw iteration is accepted without a direct HTTPX fixture proving it;
- unconditional `finally` closure remains without matching partial-finalization evidence;
- a MockTransport compatibility response is labeled native coverage;
- wrong-modality behavior is accepted through incidental `TypeError`;
- invalid `chunk_size` values are silently normalized;
- a new Python decompressor is introduced;
- the full body is buffered to simplify chunking;
- a second raw byte counter is invented when an authoritative native counter already exists;
- candidate-only tests are used as the parity oracle;
- test volume grows while incorrect/duplicative tests remain;
- a new CI job, matrix, workflow, evidence format, or release workflow is added;
- the compatibility designation is promoted beyond Stage C candidate;
- status is marked complete before exact final-tree CI evidence exists.

# Stop conditions

Stop and document the exact blocker rather than expanding architecture when:

1. the native stream cannot expose unadapted raw chunks or authoritative incremental raw-byte accounting without a substantive Rust core redesign;
2. direct HTTPX runtime behavior conflicts with the assumed source reading in this plan;
3. matching partial-finalization behavior would require relying on nondeterministic garbage collection;
4. the correction would require implementing decompression in Python;
5. a public behavior is ambiguous between HTTPX sync and async paths.

For an ambiguous case, add the smallest standalone executable HTTPX fixture and record its output first. The pinned runtime result overrides prose assumptions in this plan.

If a narrow native adapter accessor is required, stop after documenting:

- the unavailable value;
- the exact existing native owner of that value;
- the smallest proposed adapter surface;
- why wrapper-side inference is incorrect;
- tests that would consume the accessor.

Submit that adapter change separately for review. Do not redesign the transport or response model inside this corrective pass.

# Suggested commit decomposition

1. `test: add pinned HTTPX raw stream differential cases`
   - reopen status;
   - add direct failing reference fixtures;
   - remove immediately contradictory expectations.

2. `fix: align raw stream state and close transitions`
   - guard ordering;
   - consumed-state timing;
   - normal versus partial close behavior;
   - modality errors.

3. `fix: count raw source bytes before chunk adaptation`
   - source-authoritative accounting;
   - shared bounded chunk helper if needed;
   - default and invalid-size behavior;
   - buffered raw guard.

4. `test: consolidate raw stream regressions`
   - reduce candidate-only duplication;
   - add actual native sync/async coverage;
   - keep compact Tier 1 cases.

5. `docs: bind raw stream closure to final evidence`
   - exact SHAs and commands;
   - API-oracle result;
   - final CI run;
   - bounded Stage C candidate wording.

Combining commits 2 and 3 is acceptable if the implementation is inseparable. Do not combine the initial failing oracle with the production fix if that obscures whether the tests caught the baseline defects.

# Handoff checklist

Before implementation:

- [ ] Confirm `main` is still at or descended only by understood changes from `eb397395f8a2a0bf0621fbcd9deece98647a85cb`.
- [ ] Read `AGENTS.md` and preserve the existing validation tiers.
- [ ] Reproduce the four principal mismatches against the pinned HTTPX runtime.
- [ ] Mark the status line reopened.

During implementation:

- [ ] Mark live consumption at the pinned first-iteration point.
- [ ] Count source chunks before adaptation.
- [ ] Match buffered raw behavior.
- [ ] Match normal versus partial close behavior.
- [ ] Match raw defaults, invalid sizes, and modality errors.
- [ ] Preserve decoded/native decompression ownership.
- [ ] Remove incorrect and redundant tests.

Before closure:

- [ ] Focused direct differential command passes.
- [ ] `./scripts/check.sh` passes.
- [ ] Full pinned compatibility suite passes.
- [ ] Existing API oracle passes.
- [ ] Actual native sync and async paths are proven.
- [ ] Status records the final executable SHA and final CI-tested tree.
- [ ] CI passes without workflow expansion.
- [ ] Final wording remains bounded to Stage C candidate.

# Final closure statement

This corrective line is closed only when the direct HTTPX 0.28.1 runtime matrix and EggFetch produce equivalent public raw-stream state, accounting, chunking, modality, buffered-response, and close behavior for the documented asyncio-supported surface, and that result is bound to the exact final implementation and CI-tested tree.
