# HTTPX 0.28.1 Parity — Final Closure Roadmap

Status: ready for implementation handoff

Date: 2026-08-04

Audited baseline: `6ae10308b9db1e215eca19027d4ca9b7575900f6`

Pinned reference: `httpx==0.28.1`

Compatibility designation: `Stage C candidate`

## Purpose

Close the final deterministic defects remaining after the earlier HTTPX parity corrective passes without expanding the compatibility surface, CI architecture, or release process.

This roadmap is intentionally narrow. It addresses only:

1. redirect Cookie regeneration and cross-origin containment;
2. retained-body redirect handling for multipart and other non-byte bodies;
3. raw response iterator lifecycle, accounting, and sync/async symmetry;
4. missing pinned-reference coverage, exact-SHA evidence, and stale planning-PR cleanup.

The work is split into three implementation plans:

- `plans/httpx-parity-final-closure-01-redirect-security-replay.md`
- `plans/httpx-parity-final-closure-02-raw-stream-lifecycle.md`
- `plans/httpx-parity-final-closure-03-verification-status-hygiene.md`

## Current confirmed defects

### 1. Redirect Cookie headers are copied between hops

Current redirect construction copies the previous Request headers and no longer removes `Cookie`. The next-hop preparation path then interprets the copied header as an explicit user header and preserves it.

This differs from HTTPX 0.28.1, which removes the previous outgoing Cookie header and regenerates destination-appropriate cookies from the client jar on each redirect.

Risk:

- cookies can cross origin boundaries;
- path- or domain-scoped cookies can be sent to an ineligible destination;
- request-local or explicit initial Cookie state can survive more hops than the pinned reference;
- a generated header is indistinguishable from a user-supplied explicit header after the first dispatch.

### 2. Multipart and file-backed retained bodies can disappear on 307/308

Current redirect replay logic recognizes buffered `_content` and known `ByteStream` state. Multipart body state held in `_files` and `_multipart_data` is not forwarded and is not rejected.

A retained-method redirect may therefore create a second request with body-related headers but no actual body.

Risk:

- silent request truncation;
- misleading Content-Length or Content-Type state;
- semantic corruption on POST, PUT, or PATCH redirects;
- replay behavior depending on internal body representation rather than public request semantics.

### 3. Native raw iteration bypasses wrapper lifecycle semantics

Current sync `iter_raw()` delegates directly to the native stream when available, bypassing wrapper accounting and state finalization. Current async `aiter_raw()` delegates through decoded byte iteration instead of a distinct raw path.

Risk:

- sync and async divergence;
- `num_bytes_downloaded` not reflecting raw bytes;
- `is_stream_consumed` and `is_closed` remaining incorrect;
- incomplete close/finalization on exhaustion or partial consumption;
- raw and decoded iteration remaining conflated.

### 4. Closure evidence is not sufficient for the remaining edge cases

The compact corrective kernel contains candidate-only assertions for several fixes but does not cover the remaining cross-origin Cookie, multipart redirect, native raw iteration, async, and partial-consumption cases.

The status document also carries historical full-suite counts that were not refreshed after the latest corrective tests were added.

### 5. Superseded planning PRs remain open

PRs `#16` and `#17` remain open even though their plan content is already represented on `main`. This is repository hygiene, not a code blocker, but it must be resolved during final closure.

## Scope

### In scope

- compatibility facade code under `crates/eggfetch-python/python/eggfetch/compat/httpx/`;
- narrow Request metadata needed to distinguish explicit and generated Cookie state;
- redirect helpers and per-hop Cookie preparation;
- body replay classification for existing supported request-body representations;
- Response raw iterator state, chunking, accounting, and close behavior;
- direct differential tests against HTTPX 0.28.1;
- compact Tier 1 regressions using the existing test command;
- exact-SHA status and documentation reconciliation;
- closure of stale planning PRs after implementation is represented on `main`.

### Out of scope

- HTTPX versions other than 0.28.1;
- Trio or AnyIO support;
- SOCKS, UDS, `local_address`, or socket-option support;
- private HTTPX module emulation;
- a second Python networking stack;
- a new multipart replay framework;
- generalized seek/rewind support for arbitrary file objects;
- new decompression code in Python;
- new CI workflows, jobs, matrices, scheduled runs, or evidence artifacts;
- moving the full compatibility suite into routine CI;
- release or publication automation;
- Stage D or unrestricted drop-in compatibility claims.

## Execution order

### Phase 1 — Redirect Cookie and body replay correctness

Implement `httpx-parity-final-closure-01-redirect-security-replay.md` first.

Required result:

- each redirect hop regenerates Cookie state for the destination;
- prior generated Cookie headers never cross hops;
- initial explicit Cookie behavior matches HTTPX 0.28.1;
- request-local cookie behavior matches HTTPX 0.28.1;
- intermediate `Set-Cookie` values are available to the next hop when scoped correctly;
- every retained body is either replayed through one authoritative source or rejected before the next transport dispatch;
- multipart/file-backed bodies are never silently converted to empty bodies.

### Phase 2 — Raw streaming lifecycle parity ✓

Implemented `httpx-parity-final-closure-02-raw-stream-lifecycle.md`.

Result:

- sync and async raw iterators use distinct raw paths;
- raw iteration owns wrapper state and accounting;
- native decoded iteration remains native-authoritative without adding Python decompression;
- early break, exhaustion, explicit close, and repeated consumption match HTTPX 0.28.1 for the supported surface;
- underlying streams close exactly once.

### Phase 3 — Verification and repository closure

Implement `httpx-parity-final-closure-03-verification-status-hygiene.md` last.

Required result:

- all remaining disputed behavior has direct pinned-reference tests;
- Tier 1 remains compact and dependency-light;
- the full compatibility suite and API oracle are rerun on the implementation SHA;
- the status file distinguishes routine CI from manual extended evidence;
- stale test counts are removed or refreshed;
- PRs #16 and #17 are closed with concise supersession comments;
- the designation remains `Stage C candidate`.

## Required implementation sequence

1. Update the status document to mark final closure as in progress and record the starting SHA.
2. Add reference-derived failing tests for redirect Cookies, multipart replay, and raw iterator behavior.
3. Correct redirect Cookie regeneration.
4. Correct retained-body classification and replay-or-reject behavior.
5. Correct sync raw iteration.
6. Correct async raw iteration.
7. Add compact Tier 1 regressions for the highest-risk cases.
8. Run routine and extended validation.
9. Update exact-SHA evidence and compatibility claims.
10. Reconcile stale planning PRs.

Do not update the final status claim before all required validation has passed.

## Global acceptance criteria

The line of work is complete only when every item below is true.

### Redirect Cookie behavior

- The Cookie header sent on a redirect is never copied blindly from the prior outgoing request.
- Destination domain, host-only, path, secure, and expiry rules are applied on every hop.
- Cross-origin redirects cannot receive cookies scoped only to the original origin.
- Same-origin redirects regenerate Cookie state rather than preserving an opaque previous header.
- Intermediate redirect `Set-Cookie` headers update the persistent facade jar before the next request is built.
- Explicit initial Cookie-header behavior matches HTTPX 0.28.1.
- Request-local `cookies=` behavior matches HTTPX 0.28.1.
- Native cookie kwargs remain disabled for compatibility dispatch.

### Retained-body redirects

- Buffered bytes replay through exactly one body source.
- Empty bodies remain replayable.
- Known reusable byte streams are recreated safely.
- Multipart and file-backed bodies are either reconstructed from demonstrably replayable inputs or rejected before the next dispatch.
- Arbitrary iterators, generators, and uncertain file cursors are never reused implicitly.
- No retained-method redirect sends body headers with an empty or missing body unless the original body was empty.
- 301/302/303 method rewriting and body-header stripping remain correct.
- 307/308 preserve the method only when the body can be replayed safely.

### Raw streaming

- `iter_raw()` and `aiter_raw()` are distinct raw paths.
- The wrapper marks raw streams consumed at the same point as HTTPX 0.28.1.
- `num_bytes_downloaded` counts raw transport bytes, including during partial iteration.
- `chunk_size=None` and explicit chunk sizes match the reference.
- Exhaustion closes the response and underlying stream.
- Early break followed by explicit close produces correct state.
- Repeated raw iteration raises the correct stream exception.
- Read-after-partial-raw-iteration matches the pinned reference.
- Sync and async semantics agree.
- The underlying stream is closed exactly once.

### Verification and evidence

- Direct differential tests run equivalent cases against `httpx==0.28.1` and `eggfetch.compat.httpx`.
- Candidate-only tests are not used as the sole proof for disputed behavior.
- Routine CI remains one existing job invoking `./scripts/check.sh`.
- No new CI workflow, matrix, scheduled task, artifact, or release mechanism is added.
- The full compatibility suite is rerun after the final implementation commit.
- The API oracle is rerun after the final implementation commit.
- The status file records exact SHAs and separates CI evidence from local/manual evidence.
- Test counts in documentation match the referenced run or are omitted in favor of exact command/result records.
- PRs #16 and #17 are closed without merging obsolete branches.

## Rejection criteria

Reject the implementation if any of the following occurs:

- a prior-hop Cookie header is preserved across a redirect without reference-derived justification;
- cross-origin Cookie containment is asserted only through same-origin tests;
- multipart or file-backed bodies can disappear silently;
- body replay is implemented by rewinding arbitrary file objects or iterators without an explicit supported contract;
- both `content=` and `stream=` are passed to a redirected Request;
- sync raw iteration delegates directly without wrapper state/accounting;
- async raw iteration aliases decoded byte iteration when a native raw path exists;
- `num_bytes_downloaded` counts decoded bytes for compressed native responses;
- the implementation adds a Python decompression stack;
- a required differential mismatch is converted into an allowed difference without review;
- full parity or unrestricted drop-in compatibility is claimed;
- CI is expanded beyond the existing lightweight structure;
- stale evidence from an earlier SHA is presented as proof for the final implementation.

## Stop conditions

Stop and record a bounded blocker rather than expanding scope when:

- the native Python extension exposes no usable raw primitive and fixing it requires a broad Rust public-API redesign;
- correct multipart replay requires a new generalized file-rewind abstraction;
- matching HTTPX requires changing an established public EggFetch API outside the compatibility facade;
- the pinned HTTPX reference exhibits behavior that conflicts with the current security boundary and cannot be safely normalized;
- a required fix would introduce a second cookie jar or second decompression path.

A stop-condition report must include:

- the exact missing primitive;
- the smallest reproducer;
- the affected acceptance criteria;
- the bounded behavior retained instead;
- a separate follow-up proposal, not an unreviewed scope expansion.

## Suggested commit decomposition

A narrow implementation should use commits similar to:

1. `test: reopen final HTTPX parity closure cases`
2. `fix: regenerate redirect cookies per destination`
3. `fix: reject unsafe retained-body redirects`
4. `fix: align raw response iterator lifecycle`
5. `test: complete final HTTPX differential coverage`
6. `docs: record exact HTTPX closure evidence`

The implementation may combine adjacent commits when the diff remains reviewable, but must not collapse unrelated redirect, streaming, and evidence changes into one opaque patch.

## Final handoff checklist

Before declaring this roadmap complete, the implementer must report:

- starting SHA;
- final implementation SHA;
- files changed by phase;
- direct HTTPX differential command and result;
- Tier 1 `./scripts/check.sh` result;
- full compatibility suite command and result;
- API oracle command and result;
- CI run ID and checked-out SHA;
- any retained intentional differences;
- confirmation that no CI or release architecture was added;
- confirmation that PRs #16 and #17 were closed as superseded;
- final compatibility designation.

Final designation after successful completion remains:

**Stage C candidate — final deterministic closure complete for the documented HTTPX 0.28.1 asyncio-supported surface.**
