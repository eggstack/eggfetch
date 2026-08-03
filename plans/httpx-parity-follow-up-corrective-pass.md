# HTTPX 0.28.1 Parity — Follow-up Corrective Pass

Status: ready for implementation handoff

Date: 2026-08-03

## Purpose

This plan closes the deterministic defects that remain after the narrow corrective implementation at `d4192677b32bc6bda0d7b895277990a1e1b7a8ad` and the subsequent CI-status documentation updates through `4d46edc0b1609430d7b053e6376121b746ba0cd1`.

The prior corrective pass materially improved the HTTPX compatibility facade. It corrected several Request and Response state transitions, moved compatibility cookies toward a facade-owned path, added protocol validation and permanent built-in transport closure, switched built-in transports to the native streaming entrypoints, and added a compact Tier 1 corrective kernel.

Static review of the resulting code identified a smaller set of second-order defects. These defects are narrow, reproducible, and within the scope already claimed by the corrective closure. They do not justify a new roadmap, broader compatibility expansion, or additional CI architecture.

This document is therefore a **follow-up corrective pass**. It must implement only the concrete findings and closure work described below.

## Audited baseline

Implementation planning is anchored to:

- repository: `eggstack/eggfetch`;
- branch: `main`;
- audited baseline SHA: `4d46edc0b1609430d7b053e6376121b746ba0cd1`;
- corrective implementation SHA: `d4192677b32bc6bda0d7b895277990a1e1b7a8ad`;
- reference package: `httpx==0.28.1`;
- candidate package: `eggfetch.compat.httpx`;
- current compatibility designation: `Stage C candidate`;
- prior corrective plan: `plans/httpx-parity-narrow-corrective-closure.md`;
- current status file: `plans/httpx-parity-correction-status.md`.

Before implementation begins:

1. record the actual starting SHA;
2. compare it with the audited baseline;
3. review every intervening compatibility-layer commit;
4. preserve or update each finding below based on the current implementation;
5. do not silently mark an item obsolete without recording the reason in the implementation handoff or status update.

## Confirmed corrective findings

The implementation agent must treat the following as required work unless an intervening commit has already corrected it and added adequate regression coverage.

### 1. Buffered retained-body redirects construct conflicting body sources

For a body-preserving redirect, `_redirect_stream()` creates a new `ByteStream` for buffered content. `_build_redirect_request()` also passes the same bytes through `content=`. The Request constructor rejects simultaneous `content=` and `stream=` body sources.

Affected behavior includes 307 and 308 redirects for POST, PUT, PATCH, or other methods carrying buffered bodies. Empty-body redirect tests do not exercise this failure.

### 2. Explicit `timeout=None` is represented correctly but reconstructed incorrectly

The request extension correctly uses an HTTPX-style mapping with all four phases set to `None`. The native dispatch conversion reconstructs that mapping using the compatibility `Timeout` constructor without providing an explicit total default. Because that constructor defaults to five seconds, each `None` phase can be replaced with `5.0` rather than remaining disabled.

The candidate therefore distinguishes the extension representation correctly while still applying the wrong native timeout behavior.

### 3. Built-in transport timeout forwarding crosses the wrong type boundary

`HTTPTransport` and `AsyncHTTPTransport` reconstruct `eggfetch.compat.httpx.Timeout` values and pass them to native `eggfetch.Client.stream()` or `eggfetch.AsyncClient.stream()` calls.

The native PyO3 binding accepts numeric timeouts or the native binding Timeout type. It does not accept the pure-Python compatibility Timeout class. A request carrying a timeout extension through a built-in compatibility transport can therefore fail with a type error before dispatch.

### 4. Request-local cookies and explicit Cookie headers are discarded

The compatibility client removes the request’s existing `Cookie` header before each hop and then regenerates a header only from the client-level cookie jar.

This loses:

- `client.get(url, cookies={...})` request-local cookies;
- cookies supplied through a built Request;
- an explicit user `Cookie` header where HTTPX preserves or merges it according to the pinned reference behavior.

The corrective test currently verifies only client-constructor cookies, which does not cover request-local state.

### 5. Query parameters can be applied twice at the native boundary

The compatibility Request constructor incorporates `params` into the URL. Native dispatch then forwards both the already-parameterized URL and the same `params` object. The native binding appends those parameters, producing duplicates.

The same risk exists in the built-in sync and async transport paths.

### 6. Compatibility Response streaming remains incomplete

The live-stream implementation still has material differences from HTTPX 0.28.1:

- iterator-backed `iter_bytes()` and `aiter_bytes()` ignore `chunk_size`;
- text decoding is performed independently per source chunk rather than incrementally;
- a multibyte character split across source chunks can fail or decode incorrectly;
- `iter_raw()` and `iter_bytes()` are still effectively the same path for non-native streams;
- `num_bytes_downloaded` is not updated as live chunks are consumed;
- partial iteration closes the response but may leave `is_stream_consumed` inconsistent with the pinned reference;
- stream-state behavior is not sufficiently tested for early break, generator close, and repeated iteration.

These are not new Stage D features. They are closure defects in behavior that the previous corrective plan already claimed to address.

### 7. Redirect elapsed timing is chain-wide rather than hop-local

Redirect Requests reuse the prior request extension dictionary. The start timestamp is inserted with `setdefault`, so later hops may inherit the first hop’s timestamp.

The elapsed duration for the final response can therefore include previous redirect hops rather than measuring the final request-response cycle. Intermediate response timing may also be contaminated by shared mutable extensions.

### 8. The corrective kernel is candidate-only and misses the failing boundaries

The Tier 1 corrective kernel is useful and small, but its disputed semantic cases currently execute only against `eggfetch.compat.httpx`. It does not derive expected results from pinned HTTPX 0.28.1 and does not cover the exact failing native-boundary cases described above.

### 9. The lint-suppression check fails open when `rg` is unavailable

`scripts/check_lint_suppressions.sh` invokes `rg` inside conditional expressions without verifying that ripgrep exists. In an environment where `rg` is absent, each search command fails as though no forbidden match was found, and the script prints success.

The existing CI log demonstrated this false-green behavior.

### 10. Closure evidence and repository handoff state need reconciliation

The status document contains mixed statements about pending and completed remote verification, and local extended-test results are not equivalent to the Tier 1 CI evidence.

The previous planning PR also remains open even though its plan content is already present on `main`. Repository handoff state should be reconciled only after this follow-up pass is complete.

## Scope

This pass includes only:

- correct buffered and one-shot body handling for retained-body redirects;
- exact timeout mapping and native timeout conversion;
- correct timeout type conversion in built-in transports;
- request-local cookie and explicit Cookie-header preservation;
- one-time query parameter serialization at the Python/native boundary;
- live Response byte, raw, text, line, state, and accounting corrections;
- hop-local elapsed timing;
- direct pinned-HTTPX differential tests for disputed semantics;
- compact additions to the existing Tier 1 corrective kernel;
- fail-closed lint-suppression tooling;
- exact-SHA status and stale-PR reconciliation after successful implementation.

## Non-goals

This pass must not add or implement:

- HTTPX 1.0 compatibility;
- compatibility with any HTTPX version other than 0.28.1;
- Trio or general AnyIO backend support;
- SOCKS support;
- Unix-domain sockets;
- local-address binding;
- arbitrary socket options;
- private HTTPX module emulation;
- Python 3.8 or 3.9 support;
- another Python networking stack;
- new HTTP protocol features;
- new redirect policy features beyond pinned-reference parity;
- broad cookie-jar replacement outside the compatibility facade;
- new GitHub Actions workflows;
- a new CI matrix;
- a new evidence schema;
- a new qualification artifact format;
- release automation;
- benchmark, soak, fuzz, or portfolio-wide downstream expansion.

The only permitted routine-validation change is to strengthen the existing Tier 1 corrective kernel and make the existing lint-suppression check fail closed.

## Architectural invariants

1. All real network I/O remains in `eggfetch-core` and the existing native bindings.
2. The Python compatibility layer owns HTTPX-specific object semantics and orchestration, not sockets or HTTP parsing.
3. A request body has exactly one authoritative representation at construction and redirect replay.
4. A timeout mapping must preserve `None` as disabled at every boundary.
5. Compatibility-layer types must be converted into native binding types before crossing the PyO3 boundary.
6. Query parameters must be serialized exactly once.
7. Request-local cookies must participate in the same scoped selection model as client cookies without activating a second native cookie jar.
8. Raw and decoded response iteration must remain distinct concepts.
9. Stream state, close state, consumed state, and byte accounting must update deterministically.
10. Elapsed timing is response-hop-local unless the pinned reference explicitly specifies otherwise.
11. Unsupported behavior must fail before network activity and must not silently degrade.
12. Tier 1 must remain compact; the full compatibility suite and API oracle remain manual extended validation.
13. Documentation may not claim closure until the final exact-SHA validation and visible CI evidence are recorded.

# Track 0 — Reopen closure and establish failing reference cases

This track should be the first implementation commit or part of the first implementation commit.

## 0.1 Record the follow-up baseline

Update `plans/httpx-parity-correction-status.md` to state:

- starting implementation SHA;
- audited follow-up baseline `4d46edc0b1609430d7b053e6376121b746ba0cd1`;
- follow-up plan path;
- current result: `follow-up corrective pass in progress`;
- prior test counts remain historical evidence;
- Stage C candidate designation remains unchanged;
- no complete parity claim is implied.

Do not delete prior history or rewrite earlier implementation records.

## 0.2 Add direct pinned-reference cases before candidate fixes

Add focused differential tests that run equivalent scenarios against:

- `httpx==0.28.1`;
- `eggfetch.compat.httpx`.

Required cases:

1. buffered POST body through 307;
2. buffered POST body through 308;
3. one-shot streaming body through 307 and 308;
4. explicit `timeout=None` observed at a custom transport and at native-dispatch conversion;
5. scalar and structured timeout observed at built-in transport conversion;
6. request-local cookies with and without client cookies;
7. explicit Cookie header behavior;
8. URL plus `params=` serialization count;
9. direct Request plus native dispatch serialization count;
10. sync and async iterator-backed response with `chunk_size`;
11. split multibyte UTF-8 sequence across source chunks;
12. raw versus decoded iteration;
13. byte accounting during incremental iteration;
14. early break from stream iteration;
15. elapsed timing across a two-hop redirect chain;
16. fail-closed lint script behavior when `rg` is unavailable.

The tests must compare the relevant combination of:

- dispatched method;
- body bytes;
- URL and query pairs;
- Cookie header;
- timeout mapping or native timeout representation;
- exception class;
- yielded chunks;
- decoded text;
- state flags;
- close count;
- byte count;
- elapsed ordering and per-hop isolation.

## 0.3 Do not encode candidate behavior as the oracle

A differential test may use a normalized result structure where exact internal types intentionally differ, but it must derive expected public behavior from HTTPX 0.28.1.

Permitted normalization examples:

- convert URLs to strings;
- convert header objects to ordered multi-items;
- convert timeout objects to four-phase dictionaries;
- record exception class names rather than implementation-specific messages where the message is not API-significant.

Not permitted:

- asserting only the current candidate output;
- replacing a reference mismatch with an allowed difference without justification;
- broad snapshots that obscure which field differs;
- skipping native-boundary cases because MockTransport passes.

### Track 0 acceptance criteria

- The status file explicitly reopens follow-up closure.
- Every confirmed defect has at least one failing or previously failing reference-derived test.
- Both sync and async variants exist where the affected public API has both forms.
- The test output identifies the precise mismatch rather than only reporting a generic failure.
- No production behavior is changed before the disputed reference behavior is captured.

# Track 1 — Correct retained-body redirect replay

## 1.1 Use exactly one body source in redirect construction

Refactor `_redirect_stream()` and `_build_redirect_request()` so a redirect Request receives one of:

- `content=<bytes>` for buffered replayable bodies;
- `stream=<fresh replayable stream>` for explicitly replayable stream wrappers;
- no body for method rewrites to GET;
- an early replay error for one-shot bodies.

It must never pass both `content=` and `stream=`.

## 1.2 Classify replayability before dispatching the next hop

Required behavior:

- buffered bytes are replayable;
- an empty body is replayable;
- a known reusable `ByteStream` may be reconstructed as a fresh stream or replayed as bytes;
- arbitrary iterators and generators are one-shot unless an explicit reusable protocol already exists;
- multipart or file-backed bodies follow their actual replayability rather than being guessed replayable;
- an unreplayable retained body fails before the second transport dispatch.

Use the pinned HTTPX exception class and timing as closely as the supported facade permits. Do not consume the body a second time merely to discover that replay is impossible.

## 1.3 Preserve redirect metadata and cleanup

A replay correction must not regress:

- method rewriting for 301, 302, and 303;
- body retention for 307 and 308;
- body-header stripping when switching to GET;
- auth stripping across origins;
- Cookie regeneration;
- redirect history;
- `next_request` behavior when redirects are not followed;
- intermediate response cleanup.

## 1.4 Remove unreachable redirect code

Remove any unreachable return statements or contradictory replay branches left by the prior patch. Keep the helper small and explicit.

### Track 1 acceptance criteria

- Buffered POST, PUT, and PATCH bodies replay correctly through 307 and 308 in sync and async clients.
- Redirect construction never raises a body-source conflict for a replayable buffered body.
- One-shot body redirects fail before the second dispatch.
- The one-shot source is not silently reused, truncated, or converted to an empty body.
- Existing redirect method, header, history, and cleanup tests remain green.
- The corrective kernel includes at least one buffered replay and one one-shot rejection case.

# Track 2 — Normalize timeout semantics and native conversion

## 2.1 Introduce one canonical four-phase mapping helper

Use one internal representation for compatibility timeouts:

```python
{
    "connect": float | None,
    "read": float | None,
    "write": float | None,
    "pool": float | None,
}
```

Required inputs:

- omitted timeout uses client or transport default;
- explicit `None` produces all four phases as `None`;
- scalar produces the scalar for all four phases;
- compatibility Timeout preserves each configured phase;
- an existing request extension remains authoritative unless a public API override is explicitly supplied according to HTTPX behavior.

## 2.2 Preserve disabled phases during reconstruction

Do not reconstruct all-`None` mappings by calling a constructor whose default total repopulates the phases.

Use one of the following bounded approaches:

- construct the compatibility Timeout with an explicit `timeout=None` plus explicit phases;
- bypass compatibility object reconstruction and convert the mapping directly;
- add a narrowly scoped constructor/helper that cannot introduce a default.

The selected approach must also handle partially specified mappings without filling disabled phases accidentally.

## 2.3 Add an explicit compatibility-to-native conversion boundary

Create or reuse a helper that converts the four-phase compatibility mapping into a native binding-supported timeout value.

The built-in transport and direct native-client paths must not pass the pure-Python compatibility Timeout object into PyO3.

The conversion must preserve:

- all-disabled phases;
- scalar-equivalent phases;
- distinct connect/read/write/pool values;
- zero values;
- omitted versus explicitly disabled behavior.

If the native binding cannot represent an HTTPX phase exactly, the implementation must:

1. identify the exact limitation;
2. use the closest existing supported representation only when behavior remains correct;
3. otherwise fail explicitly before network dispatch;
4. document the bounded difference in the existing allowed-difference mechanism.

Do not silently collapse structured timeouts to a total scalar.

## 2.4 Apply the same conversion in all dispatch paths

Audit and unify:

- sync direct native dispatch;
- async direct native dispatch;
- `HTTPTransport.handle_request()`;
- `AsyncHTTPTransport.handle_async_request()`;
- mounted built-in transports;
- custom transports, which should continue receiving the HTTPX-style extension mapping.

### Track 2 acceptance criteria

- `timeout=None` remains all-disabled through native dispatch.
- Scalar timeout values reach native dispatch with all phases equal.
- Structured timeout values preserve distinct phases.
- Built-in transports accept request timeout extensions without PyO3 type errors.
- Custom transports continue observing an HTTPX-style dictionary.
- Sync and async behavior match.
- No duplicate timeout conversion logic remains where one helper can be used safely.
- Tests cover omitted, `None`, zero, scalar, and structured phase values.

# Track 3 — Correct cookies and query serialization at the dispatch boundary

## 3.1 Define cookie precedence from the pinned reference

Capture and implement HTTPX 0.28.1 behavior for:

- client cookies only;
- request-local cookies only;
- client plus request-local cookies with different names;
- same-name collisions;
- scoped domain/path cookies;
- explicit Cookie header;
- redirects to same origin;
- redirects across origins;
- cookies set by intermediate redirect responses.

The compatibility facade must continue to own one authoritative persistent jar. Request-local cookies may be merged into the outgoing header for that request without being flattened into the native persistent jar unless HTTPX persists them.

## 3.2 Stop deleting valid request-local cookie state

Refactor per-hop cookie preparation so it does not blindly discard:

- `request.cookies`;
- a valid explicit Cookie header;
- request-local cookies merged during `build_request()`.

Use a deterministic merge step that produces one outgoing Cookie header.

The implementation must distinguish:

- persistent client-jar state;
- request-local cookie additions;
- explicit user header behavior;
- response cookie extraction into the persistent jar.

## 3.3 Keep the native cookie mechanism disabled for compatibility dispatch

Do not reintroduce:

- native-client constructor cookies;
- native per-request `cookies=` kwargs;
- dictionary flattening of scoped cookie state.

The final wire header must be produced once by the compatibility facade.

## 3.4 Serialize query parameters exactly once

Select one authoritative boundary:

- either Request construction applies `params` to the URL and dispatch forwards only the URL;
- or Request retains parameters separately and dispatch performs the serialization once.

For the current architecture, prefer the smallest correction that preserves public Request semantics and avoids duplicate native application.

Audit all relevant paths:

- top-level request helpers;
- `Client.build_request()`;
- `Client.send()`;
- async equivalents;
- direct native dispatch;
- built-in transports;
- mounted transports;
- redirects preserving query strings.

Do not remove legitimate repeated query keys supplied intentionally by the user. The goal is to eliminate duplicate application, not deduplicate the query data model.

### Track 3 acceptance criteria

- Request-local cookies are emitted on the intended request.
- Client and request-local cookies follow pinned-reference precedence.
- Explicit Cookie headers follow pinned-reference behavior.
- Domain/path scoping remains intact.
- Native cookie kwargs remain absent from compatibility dispatch.
- Response Set-Cookie extraction continues to update the persistent facade jar.
- A simple `params={"a": "1"}` request is dispatched with exactly one `a=1` pair.
- Intentionally repeated pairs remain repeated exactly as supplied.
- Sync, async, built-in transport, and mounted transport paths agree.

# Track 4 — Complete Response streaming semantics

## 4.1 Separate raw source iteration from decoded byte iteration

Define explicit internal layers:

1. raw source chunks from the transport or compatibility stream;
2. decoded/decompressed byte chunks where applicable;
3. text decoding;
4. line splitting.

For non-native compatibility streams, raw and byte iteration may yield the same bytes only when there is no content decoding to perform, but the methods must retain separate state and delegation points.

Do not implement a second decompression stack in Python. Native decoding remains authoritative where native streams expose decoded and raw methods.

## 4.2 Honor `chunk_size`

For iterator-backed and async-iterator-backed responses:

- coalesce or split source chunks so `iter_bytes(chunk_size=N)` and `aiter_bytes(chunk_size=N)` match pinned-reference behavior;
- preserve `chunk_size=None` behavior;
- handle empty chunks consistently;
- do not eagerly buffer the entire body solely to enforce chunk size.

A small internal chunk buffer is acceptable.

## 4.3 Use incremental text decoding

Replace independent `chunk.decode(encoding)` calls with an incremental decoder so split multibyte characters are handled correctly.

Required cases:

- UTF-8 code point split across two source chunks;
- split newline sequence;
- final decoder flush;
- invalid byte behavior matching HTTPX’s configured error handling;
- custom/default encoding behavior already supported by the facade.

Do not eagerly call `.read()` to implement live text iteration.

## 4.4 Correct line iteration across arbitrary chunk boundaries

Line iteration must correctly handle:

- `\n`;
- `\r\n` split across chunks;
- a final line without a trailing newline;
- multiple lines in one chunk;
- empty lines;
- text decoded from split multibyte sequences.

Use the pinned HTTPX output as the oracle.

## 4.5 Update byte accounting incrementally

`num_bytes_downloaded` must update as raw wire/transport bytes are consumed according to HTTPX semantics.

Clarify through reference tests whether the count reflects raw bytes or decoded bytes for the supported facade path, then implement that behavior consistently for:

- native streaming responses;
- compatibility iterator streams;
- full `read()` and `aread()`;
- partial iteration;
- zero-length bodies.

## 4.6 Correct partial-consumption state

Capture HTTPX behavior for:

- exhausting the iterator;
- breaking early;
- explicitly closing after a partial read;
- generator finalization;
- calling a second iterator after partial or complete consumption;
- calling `.read()` after partial iteration;
- closing an unread response.

Set `is_closed` and `is_stream_consumed` to match the reference. Do not assign `is_stream_consumed=True` only after normal loop completion if HTTPX marks it earlier.

Ensure close operations remain idempotent and close the underlying native or compatibility stream exactly once.

## 4.7 Preserve buffered-response behavior

Do not regress:

- buffered Response starts closed and consumed;
- `.content` is immediately available;
- repeated `read()` returns buffered bytes;
- `num_bytes_downloaded` initial behavior matches the pinned reference;
- buffered iteration can split according to `chunk_size` without altering closed state.

### Track 4 acceptance criteria

- Sync and async byte iterators honor `chunk_size`.
- A split UTF-8 code point decodes correctly.
- Raw and decoded method paths are no longer hard aliases where the native stream distinguishes them.
- Line iteration matches HTTPX for CRLF and arbitrary chunk boundaries.
- Byte accounting matches the reference during and after iteration.
- Early break, explicit close, unread close, repeated iteration, and read-after-iteration match HTTPX state and exceptions.
- Underlying streams close exactly once.
- No eager full-body buffering is introduced for normal streaming iteration.

# Track 5 — Make elapsed timing hop-local

## 5.1 Stop sharing mutable timing extensions across redirects

Redirect Request construction must not reuse the same mutable extensions dictionary when that causes per-hop internal metadata to leak.

Use one of:

- a shallow copy with internal timing keys removed;
- a new request extensions dictionary preserving only public transport extensions;
- a dedicated non-public timing field outside public request extensions.

Prefer the smallest change that preserves custom transport extension behavior.

## 5.2 Start the timer exactly once per dispatch hop

For each actual transport hop:

- record a fresh start time before dispatch;
- do not overwrite it mid-hop;
- do not inherit the previous hop’s start time;
- set final elapsed when the response is fully read or closed, matching HTTPX streaming behavior;
- use native elapsed where it is already authoritative and correct.

## 5.3 Preserve timing behavior for buffered and live responses

Required behavior:

- buffered response elapsed is available after the response is returned;
- streaming response elapsed raises until read or close;
- manual close finalizes elapsed;
- each redirect history response has its own elapsed value;
- final response elapsed does not include prior redirect hops.

Tests should use ordering and bounded sleeps rather than brittle exact timing thresholds.

### Track 5 acceptance criteria

- Each redirect hop has a distinct start timestamp.
- Final response elapsed excludes earlier redirect delays.
- History responses retain their own elapsed measurements.
- Streaming elapsed remains unavailable until read or close.
- Sync and async behavior match.
- No private timing key leaks into a redirected request by shared dictionary identity.

# Track 6 — Strengthen compact validation without expanding CI

## 6.1 Add only high-value cases to the existing Tier 1 kernel

Extend `test_corrective_kernel.py` with a compact set of deterministic regressions covering at least:

- buffered 307 replay;
- one-shot 307 rejection before second dispatch;
- explicit `timeout=None` native conversion helper behavior;
- built-in transport timeout conversion type;
- request-local cookies;
- query parameters applied once;
- split UTF-8 live text iteration;
- partial stream state;
- lint tool fail-closed behavior, if inexpensive enough for Tier 1.

Keep the kernel small. More exhaustive variants belong in the full compatibility suite.

A reasonable target is no more than approximately 20–25 focused corrective tests total in the file unless parameterization keeps collection and runtime similarly small.

## 6.2 Keep pinned HTTPX differential tests in Tier 2 where dependencies require it

Tier 1 must not begin installing HTTPX or requests solely for the corrective kernel.

Direct HTTPX differential cases should remain in the full compatibility suite, which already requires the pinned reference environment.

Tier 1 may encode the already-established reference result in narrow regression assertions after the Tier 2 differential test establishes it.

## 6.3 Make lint suppression checking fail closed

Update `scripts/check_lint_suppressions.sh` so one of the following is true:

- `rg` is explicitly required and absence causes a clear failure; or
- the script uses a portable fallback such as `grep` with equivalent matching behavior.

Prefer the simpler solution consistent with `scripts/check.sh` tool requirements.

The script must never print success when no search tool ran.

## 6.4 Do not add CI architecture

Prohibited changes:

- additional workflow files;
- additional CI jobs;
- additional platform matrices;
- artifact uploads;
- evidence bundles;
- nightly schedules;
- automatic extended-suite execution;
- automatic release or publication steps.

The existing workflow should continue to invoke `./scripts/check.sh` once.

### Track 6 acceptance criteria

- Existing Tier 1 remains one routine command and one CI job.
- The corrective kernel catches each high-risk regression listed above.
- Full differential coverage remains in Tier 2.
- Missing `rg` can no longer produce a false success.
- Tier 1 runtime remains proportionate to the current project and does not add external network test dependencies.
- No new workflow, matrix, evidence, or release mechanism is introduced.

# Track 7 — Reconcile documentation, status, and stale handoff state

This track must land only after production fixes and validation pass.

## 7.1 Record the exact implementation SHA

Update `plans/httpx-parity-correction-status.md` with:

- exact implementation SHA;
- Tier 1 result from that SHA;
- full compatibility result from that SHA;
- API oracle result from that SHA;
- visible CI run ID and conclusion for the pushed implementation SHA;
- any bounded intentional differences discovered during this pass.

Do not cite CI for an earlier commit as proof for a later documentation commit unless the later commit changes documentation only and that relationship is stated explicitly.

## 7.2 Distinguish local extended evidence from CI evidence

The status file must clearly separate:

- routine CI Tier 1 results;
- local/manual Tier 2 full compatibility results;
- API oracle results;
- package or release validation, if not run.

Do not state that CI ran the full compatibility suite when it did not.

## 7.3 Reconcile README and architecture claims

Review compatibility claims in:

- `README.md`;
- `AGENTS.md`;
- `docs/architecture/python-bindings.md`;
- `docs/reference/compatibility.md`, if affected;
- `plans/httpx-parity-correction-status.md`.

Keep the designation at Stage C candidate unless a separate, explicitly authorized qualification effort changes it.

Do not claim full drop-in parity outside the documented supported surface.

## 7.4 Reconcile stale planning PRs

After this plan or its implementation is represented on `main`:

- close stale PR #16 if it remains open and its content is already incorporated;
- leave a concise closure comment explaining that its planning content was superseded or incorporated;
- ensure this follow-up plan’s PR remains the authoritative handoff until merged;
- do not merge stale implementation branches merely to clean up repository state.

This is repository hygiene, not an implementation dependency.

### Track 7 acceptance criteria

- Status claims are exact-SHA-bound.
- Tier 1 CI and manual Tier 2 evidence are clearly distinguished.
- README and architecture text do not overstate compatibility.
- Stage C candidate remains the designation.
- Stale planning PR state is reconciled without merging obsolete code.
- No status file says both “pending” and “passed” for the same evidence.

# Required implementation sequence

The preferred order is:

1. Track 0 — reopen status and add failing reference tests;
2. Track 1 — redirect replay;
3. Track 2 — timeout semantics and conversion;
4. Track 3 — cookies and query serialization;
5. Track 4 — streaming semantics;
6. Track 5 — elapsed timing;
7. Track 6 — compact Tier 1 and lint fail-closed behavior;
8. Track 7 — exact-SHA closure and repository hygiene.

Tracks 1 through 5 may be split into separate commits. Do not combine unrelated fixes into one large compatibility rewrite.

## Suggested commit decomposition

A bounded implementation may use commits similar to:

1. `test: reopen HTTPX follow-up parity cases`
2. `fix: correct redirect body replay`
3. `fix: preserve timeout semantics across native dispatch`
4. `fix: preserve request cookies and single query serialization`
5. `fix: complete response stream state and decoding`
6. `fix: isolate per-hop elapsed timing`
7. `test: strengthen corrective kernel and lint check`
8. `docs: record HTTPX follow-up closure evidence`

Commit names are illustrative. Keep each commit internally coherent and independently reviewable.

# Mandatory test matrix

The implementation handoff must include the following minimum cases.

## Redirect replay

- sync buffered POST + 307;
- sync buffered POST + 308;
- async buffered POST + 307;
- async buffered POST + 308;
- sync one-shot body rejected before second hop;
- async one-shot body rejected before second hop;
- GET rewrite paths remain body-free;
- retained Content-Length remains correct.

## Timeout conversion

- omitted timeout;
- explicit `None`;
- zero scalar;
- positive scalar;
- four distinct phases;
- direct native dispatch;
- built-in sync transport;
- built-in async transport;
- custom transport observes dictionary mapping;
- no pure-Python compatibility Timeout crosses the PyO3 boundary.

## Cookies

- client cookies only;
- request cookies only;
- client plus request cookies;
- same-name precedence;
- explicit Cookie header;
- same-origin redirect;
- cross-origin redirect;
- redirect response Set-Cookie propagation;
- domain/path-scoped duplicate names.

## Query serialization

- mapping with one key;
- repeated pair list;
- existing URL query plus `params` behavior matching HTTPX;
- direct Request + send;
- top-level helper;
- built-in transport;
- sync and async.

## Streaming

- buffered response iteration;
- live iterator with source chunk larger than requested chunk size;
- live iterator with source chunk smaller than requested chunk size;
- `chunk_size=None`;
- split UTF-8 code point;
- CRLF split across chunks;
- raw and byte iteration paths;
- incremental byte count;
- early break;
- explicit close after early break;
- repeated iteration;
- read after partial iteration;
- close exactly once;
- async equivalents.

## Timing

- buffered direct response;
- streaming response before read;
- streaming response after read;
- close without read;
- two-hop redirect with deliberate first-hop delay;
- history response elapsed values;
- sync and async.

## Tooling and closure

- lint script succeeds when no forbidden suppressions exist and search tool is available;
- lint script fails clearly when required search tooling is unavailable, or fallback behavior is tested;
- Tier 1 command passes;
- full pinned compatibility suite passes;
- API oracle has no unexplained or stale entries;
- visible CI run passes on the pushed implementation SHA.

# Global acceptance criteria

This follow-up pass is complete only when all of the following are true:

1. Buffered retained-body redirects use exactly one body source and replay correctly.
2. One-shot retained-body redirects fail before a second dispatch.
3. Explicit `timeout=None` remains disabled through native dispatch.
4. Structured timeout phases survive all sync and async dispatch paths.
5. Built-in transports do not pass compatibility Timeout objects into PyO3.
6. Request-local cookies and explicit Cookie behavior match HTTPX 0.28.1.
7. The compatibility facade remains the sole cookie authority for compatibility dispatch.
8. Query parameters are applied exactly once without removing intentional duplicate keys.
9. Live byte iteration honors chunk size without full eager buffering.
10. Incremental text decoding handles split multibyte sequences.
11. Raw, byte, text, and line iteration have correct layering and state transitions.
12. Byte accounting, close state, and consumed state match pinned-reference behavior.
13. Elapsed timing is isolated per transport hop.
14. The compact Tier 1 kernel catches the high-risk regressions.
15. The full compatibility suite contains direct HTTPX 0.28.1 differential coverage.
16. The lint-suppression check fails closed when its search tool is unavailable.
17. `./scripts/check.sh` passes without adding CI jobs or matrices.
18. `./scripts/check.sh extended` passes in the prepared reference environment.
19. The API oracle reports zero unexplained, stale, or resolved-in-active entries.
20. Status and documentation claims are exact, bounded, and exact-SHA-bound.
21. The visible CI run for the implementation SHA passes.
22. Stale planning PR state is reconciled after the authoritative plan or implementation lands.

# Rejection criteria

The implementation must be rejected or returned for correction if any of the following occurs:

- redirect replay is “fixed” by eagerly consuming all request streams;
- replayable bodies are sent through both `content=` and `stream=`;
- one-shot bodies are silently replayed as empty bodies;
- explicit `timeout=None` becomes a default nonzero timeout;
- structured timeout phases are silently collapsed to one scalar;
- compatibility Timeout instances are passed directly into native PyO3 APIs;
- request-local cookies are persisted globally without reference support;
- native cookie kwargs are re-enabled for compatibility dispatch;
- query duplicates are removed indiscriminately rather than preventing double application;
- streaming is implemented by buffering the full response before yielding;
- `iter_raw()` remains an unconditional alias for decoded byte iteration where the transport distinguishes them;
- split multibyte text remains decoded independently per source chunk;
- stream state is adjusted only to satisfy candidate-only assertions without HTTPX reference evidence;
- timing remains shared across redirect Requests;
- the lint check continues to report success when no search command ran;
- a new workflow, job, matrix, artifact format, or release process is added;
- the full compatibility suite is moved into routine CI;
- documentation claims complete HTTPX parity or promotes beyond Stage C without separate authorization;
- closure is declared without exact-SHA local and visible CI evidence.

# Stop conditions

Stop and record a bounded blocker rather than expanding scope if:

- the native binding cannot express a required timeout phase without a core API change larger than a focused adapter correction;
- native raw streaming bytes are not exposed separately from decoded bytes and correcting that would require a substantial core streaming redesign;
- exact HTTPX cookie precedence requires a broader cookie API rewrite outside the compatibility facade;
- a reference behavior depends on unsupported AnyIO or Trio internals;
- a correction would require adding a second Python networking implementation;
- the fix would require a new CI platform or release mechanism.

A blocker record must state:

- the exact reference case;
- the current candidate behavior;
- the smallest missing primitive;
- why the primitive cannot be added within this pass;
- the bounded user-visible consequence;
- whether an existing allowed-difference entry is appropriate.

Do not use a blocker as a reason to leave a silent incorrect fallback.

# Handoff checklist

Before handing implementation back for review, the implementation agent must provide:

- starting SHA;
- final implementation SHA;
- list of changed files grouped by track;
- direct HTTPX differential test locations;
- Tier 1 corrective-kernel test locations;
- exact `./scripts/check.sh` result;
- exact full compatibility-suite result;
- exact API-oracle result;
- visible CI run ID and conclusion;
- confirmation that no new CI workflow, job, matrix, evidence format, or release automation was added;
- confirmation that compatibility Timeout objects do not cross into native PyO3 APIs;
- confirmation that native cookie kwargs remain disabled for compatibility dispatch;
- confirmation that query parameters are serialized exactly once;
- confirmation that Stage C candidate remains the designation;
- any bounded blockers or allowed differences, with exact identifiers;
- stale PR reconciliation performed or explicitly left for maintainers after merge.

## Final closure statement template

Use a statement materially equivalent to the following only after all acceptance criteria pass:

> The HTTPX 0.28.1 follow-up corrective pass is complete at `<exact SHA>`. Buffered retained-body redirects replay through a single body source, one-shot bodies fail before redispatch, timeout disablement and structured phases survive native conversion, request-local cookies and query serialization match the pinned reference, live stream decoding/state/accounting are corrected, elapsed timing is hop-local, the compact Tier 1 kernel passes, the full pinned differential suite and API oracle pass, and visible CI passes without adding workflow or release complexity. The facade remains a Stage C candidate.

Do not use this statement while any confirmed finding remains open.
