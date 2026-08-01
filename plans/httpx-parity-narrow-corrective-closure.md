# HTTPX 0.28.1 Parity — Narrow Corrective Closure

Status: ready for implementation handoff

Date: 2026-08-01

## Purpose

This plan closes the remaining material HTTPX 0.28.1 behavioral defects left after the implementation of:

- `plans/httpx-parity-correction-roadmap.md`;
- Phases 1 through 5 of that roadmap;
- `plans/httpx-parity-correction-status.md`.

The prior implementation materially improved the compatibility facade. It established facade-owned redirect/auth orchestration, scoped `CookieJar` support, top-level streaming context management, explicit unsupported-option errors, response metadata, mount routing, and substantial focused tests.

However, the current closure declaration is not yet supported by the implementation. Several tests encode eggfetch behavior as the expected result instead of comparing against HTTPX 0.28.1, and several release-relevant behaviors remain incorrect or silently ineffective.

This is a **narrow corrective pass**, not a new roadmap. It must correct only the concrete findings listed below and then reconcile tests, status, and documentation.

## Audited baseline

Implementation planning is anchored to:

- repository: `eggstack/eggfetch`;
- branch: `main`;
- audited baseline SHA: `7de195716ef64787535d089020a99891bae4aa8e`;
- reference package: `httpx==0.28.1`;
- candidate import: `eggfetch.compat.httpx`;
- current declared status: `Stage C candidate`;
- current closure status file: `plans/httpx-parity-correction-status.md`.

Before implementation begins, record the actual starting SHA. If `main` has advanced, review every intervening commit and update the implementation status with whether each finding remains applicable.

## Corrective findings

The implementation agent must treat the following as required corrective work.

1. **Per-request timeout overrides are accepted but not applied.** `Client.send()` and `AsyncClient.send()` accept a `timeout` argument, but dispatch continues to use `self._timeout`. Explicit `timeout=None` and explicit `Timeout(...)` therefore do not reliably affect request extensions or native dispatch.
2. **Request object state differs from HTTPX.** Empty POST/PUT/PATCH requests do not receive `Content-Length: 0`; unread streaming `Request.content` returns `None` instead of raising `RequestNotRead`; stream replacement and replay state are incomplete.
3. **Response object state differs from HTTPX.** Buffered responses do not begin in HTTPX-equivalent consumed/closed state; iterator-backed responses cannot be consumed through `iter_bytes()`/`aiter_bytes()` as HTTPX permits; stream completion and close flags are inconsistent; `request` does not raise when unattached; `has_redirect_location` is too broad; elapsed time remains zero rather than measured.
4. **Raw and decoded response iteration remain conflated for compatibility streams.** Non-native streams do not consistently implement `iter_raw`/`iter_bytes` and async equivalents with HTTPX state transitions.
5. **The compatibility cookie jar is not yet authoritative.** The facade selects scoped cookies into request headers, but cookie values are also flattened and passed to the native client/request path, leaving two active cookie mechanisms.
6. **Body-preserving redirects do not enforce replayability.** The same one-shot request stream may be reused for 307/308 or other retained-body redirects without rewind, replacement, or an early replay error.
7. **Rust-backed HTTPTransport paths are not genuinely transport-streaming.** They call native buffered request APIs and wrap the result as a stream. Their lifecycle may also recreate a native client after close.
8. **Transport protocol validation is inconsistent.** `HTTPTransport` and `AsyncHTTPTransport` accept and store protocol combinations without applying the same `http1`/`http2` validation as the compatibility clients.
9. **Some focused tests are false-green.** Current tests explicitly assert behavior that differs from HTTPX, including streaming `iter_bytes()` raising `ResponseNotRead`, buffered responses starting open, and empty POST/PUT/PATCH requests lacking `Content-Length: 0`.
10. **Default CI does not exercise the corrective parity kernel.** Tier 1 runs only import/client/exception compatibility smoke files. The full suite and API oracle remain extended-only.
11. **Status and README claims are too strong.** The status file declares no blockers, and README states that remaining differences are non-behavioral, while the findings above remain.

## Scope

This pass includes only:

- request-level timeout application;
- Request body/header/read state corrections;
- Response body/stream/request/redirect/timing state corrections;
- one authoritative compatibility cookie path;
- redirect replayability classification and enforcement;
- genuine Rust-backed transport streaming or an explicit bounded fallback;
- transport close and protocol validation;
- correction of false-green tests into direct HTTPX differential tests;
- a small required Tier 1 parity kernel;
- allowed-difference, status, diagnostics, and README reconciliation.

## Non-goals

This pass must not add or implement:

- Trio or general AnyIO backend support;
- SOCKS support;
- Unix-domain sockets;
- local-address binding;
- arbitrary socket options;
- Python 3.8 or 3.9 support;
- private HTTPX module compatibility;
- HTTPX 1.0 or moving-branch compatibility;
- new HTTP protocol features;
- a second networking stack in Python;
- new GitHub Actions workflows;
- a new CI matrix;
- a new qualification artifact format;
- a new evidence schema;
- additional downstream packages;
- broad soak, fuzz, benchmark, or release automation work.

The only CI change permitted is adding a small focused parity kernel to the existing Tier 1 `./scripts/check.sh` path.

## Architectural invariants

1. All real network I/O remains in `eggfetch-core` and the existing native bindings.
2. Python owns HTTPX-specific object behavior and state-machine orchestration, not sockets or HTTP parsing.
3. A request has exactly one effective timeout configuration at dispatch.
4. A compatibility client has exactly one authoritative cookie jar.
5. A Response has one body representation at a time: buffered or live stream, with explicit state transitions.
6. Body-preserving redirect replay must be proven safe before the next hop is dispatched.
7. Unsupported behavior must fail before network activity; it may not silently degrade.
8. Tests must derive expected behavior from pinned HTTPX 0.28.1 where practical.
9. Status and documentation may not claim closure before the final exact-SHA validation pass.

# Track 0 — Correct the status and establish failing reference cases

This track should land first or in the first corrective commit. It prevents further work from being performed under a false “complete” state.

## 0.1 Mark the current closure status as reopened

Update `plans/httpx-parity-correction-status.md` to state:

- corrective review baseline SHA;
- current result: `reopened — narrow corrective pass required`;
- the exact findings in this plan;
- prior test counts are retained as historical information, not current closure proof;
- `Stage C candidate` remains unchanged.

Do not delete the prior implementation history.

## 0.2 Add direct HTTPX reference fixtures for disputed semantics

Before changing candidate behavior, add focused tests that execute the same scenario against `httpx==0.28.1` and `eggfetch.compat.httpx`.

Required reference cases:

- `Request("POST", url)` auto headers and content state;
- `Request(stream=...)` unread `.content`;
- `Response(content=b"...")` initial `is_closed` and `is_stream_consumed`;
- `Response(stream=SyncByteStream(...)).iter_bytes()`;
- async stream equivalent;
- unattached `Response.request` and `.url`;
- `has_redirect_location` across 300, 301, 302, 303, 304, 305, 306, 307, 308, and 399;
- elapsed access before and after read/close;
- request timeout extension for omitted, explicit `None`, scalar, and `Timeout` values.

Tests must compare:

- return values;
- header values;
- exception classes;
- state flags;
- stream close counts;
- transport-observed request extensions.

## 0.3 Remove or rewrite false-green assertions

The following assertions must not remain unless direct HTTPX 0.28.1 observation proves them:

- streaming `Response.iter_bytes()` raises `ResponseNotRead` before `.read()`;
- buffered `Response(content=...)` starts open and unconsumed;
- empty POST/PUT/PATCH direct Request construction omits `Content-Length: 0`;
- elapsed time may be represented as a permanent zero placeholder;
- a streaming Request exposes `content is None` rather than `RequestNotRead`.

### Track 0 acceptance criteria

- [ ] Status is explicitly reopened before implementation closure work begins.
- [ ] Each disputed behavior has a direct pinned-reference fixture.
- [ ] Candidate tests fail before the associated implementation fix.
- [ ] No corrected test derives expected state solely from candidate implementation comments.
- [ ] Required reference fixtures fail closed when HTTPX 0.28.1 is missing.
- [ ] No new test runner, report format, or CI workflow is introduced.

# Track 1 — Apply per-request timeout semantics correctly

## 1.1 Resolve timeout once per request

Introduce one internal helper shared by sync and async clients:

```python
def _resolve_timeout(client_timeout, override):
    ...
```

Required states:

- omitted override → client timeout;
- explicit `None` → `Timeout(None)` / disabled timeouts;
- scalar → `Timeout(scalar)`;
- compatibility `Timeout` → use that value;
- invalid input → fail before dispatch.

Do not use `None` as both “omitted” and “disable.”

## 1.2 Attach the effective timeout to the Request

Before the first hook/dispatch:

- set `request.extensions["timeout"]` using HTTPX-compatible timeout dictionary or the existing exact representation expected by custom transports;
- preserve an explicit caller-supplied timeout extension where HTTPX does;
- ensure redirects and auth-produced follow-up requests retain or recompute the effective timeout consistently;
- ensure a request built with an explicit timeout already contains the correct extension.

Prefer matching HTTPX’s four-value extension mapping:

- `connect`;
- `read`;
- `write`;
- `pool`.

Do not expose a compatibility `Timeout` object to custom transports if HTTPX exposes a plain mapping.

## 1.3 Pass the effective timeout to native dispatch

`_build_native_kwargs()` and every native sync/async dispatch path must use the effective request timeout, not `self._timeout` unconditionally.

This applies to:

- ordinary request;
- streaming request;
- mounted Rust-backed transport;
- redirect hops;
- auth challenge follow-ups.

## 1.4 Prove override behavior, not only successful requests

Add deterministic tests using:

- a custom transport that records `request.extensions["timeout"]`;
- a local delayed endpoint where a short request override times out but the longer client default does not;
- explicit `timeout=None` against the same delayed endpoint;
- sync and async paths;
- `Client.request`, `Client.stream`, `AsyncClient.request`, and `AsyncClient.stream`.

Avoid assertions that merely prove an immediate request returned 200.

### Track 1 acceptance criteria

- [ ] Omitted timeout uses the client default.
- [ ] Explicit `timeout=None` disables request timeouts.
- [ ] Scalar and `Timeout` overrides reach custom transports in HTTPX-compatible form.
- [ ] Native dispatch uses the request’s effective timeout.
- [ ] Redirect and auth follow-up requests preserve the effective timeout.
- [ ] Sync and async ordinary and streaming calls behave equivalently.
- [ ] Invalid timeout values fail before network activity.
- [ ] Tests demonstrate a behavioral timeout difference, not only extension shape.

# Track 2 — Correct Request and Response body/state semantics

## 2.1 Correct Request auto-header preparation

Match HTTPX direct Request construction:

- add Host only in encoded-content preparation paths, not explicit low-level `stream=` paths;
- add `Content-Length: 0` for empty POST, PUT, and PATCH;
- preserve caller-provided `Content-Length` or `Transfer-Encoding`;
- avoid adding content headers to explicit `stream=` requests;
- preserve IPv6 brackets and non-default ports.

Use one preparation helper rather than scattered header conditions.

## 2.2 Make Request content state explicit

Required behavior:

- unread streaming `.content` raises `RequestNotRead`;
- `.read()` consumes a sync iterable and caches bytes;
- `.aread()` consumes an async iterable and caches bytes;
- after complete read, replace a one-shot stream with a replayable byte stream where HTTPX does;
- repeated read returns cached bytes;
- sync/async stream misuse fails with the HTTPX-equivalent runtime error;
- `is_stream_consumed` changes when the stream is actually consumed.

Remove duplicate or divergent stream-consumed fields if possible.

## 2.3 Correct buffered Response initial state

For `Response(content=...)`, `text=`, `html=`, or `json=`:

- construct the appropriate in-memory byte stream;
- read it through the same public state transition used by HTTPX;
- finish with content cached;
- set `is_stream_consumed=True`;
- set `is_closed=True`;
- expose `num_bytes_downloaded` according to HTTPX behavior;
- add generated content headers where HTTPX does.

Do not special-case buffered content into a permanently open pseudo-response.

## 2.4 Correct live Response iteration

For sync streams:

- `iter_raw()` consumes the live raw stream once;
- `iter_bytes()` decodes from raw iteration;
- `iter_text()` incrementally decodes byte chunks;
- `iter_lines()` incrementally handles CRLF and final unterminated lines;
- exhaustion closes the stream and sets state flags;
- second raw iteration raises `StreamConsumed`;
- iteration after close raises `StreamClosed`.

Implement the same state machine for async methods.

Do not make `iter_bytes()` require a prior `.read()` on a valid live response stream.

## 2.5 Correct Response request and redirect properties

Required behavior:

- `.request` raises when unattached;
- `.url` reaches the same missing-request failure through `.request`;
- assigning `.request` updates the effective URL;
- `has_redirect_location` is true only for the redirect statuses HTTPX follows and only when Location is present;
- `next_request` remains available for manual redirect responses.

## 2.6 Measure elapsed time

Use a monotonic start timestamp at one-hop transport dispatch.

Bind completion to response lifecycle:

- buffered response: elapsed set after body read completes;
- streaming response: elapsed unavailable until full consumption or close;
- explicit close before full read still sets elapsed;
- custom, Mock, WSGI, ASGI, mounted, and native transport paths use the same boundary;
- elapsed is not hardcoded to zero.

A small bound such as `elapsed >= 0` is insufficient. Use a deterministic delayed custom/local transport to prove nonzero measurement.

### Track 2 acceptance criteria

- [ ] Empty POST/PUT/PATCH Requests include `Content-Length: 0` like HTTPX.
- [ ] Explicit low-level `stream=` does not receive auto Host/content headers.
- [ ] Unread streaming Request content raises `RequestNotRead`.
- [ ] Read request streams become replayable where HTTPX does.
- [ ] Buffered Responses begin consumed and closed like HTTPX.
- [ ] Live sync and async `iter_bytes()` consume without requiring a prior read.
- [ ] Raw, decoded, text, and line iteration have distinct behavior.
- [ ] Stream exhaustion and explicit close update state once.
- [ ] Reuse after consumption/close raises the correct exception.
- [ ] Unattached Response request and URL access raise.
- [ ] `has_redirect_location` matches HTTPX across representative 3xx codes.
- [ ] Measured elapsed time reflects actual transport duration.

# Track 3 — Make the compatibility cookie jar authoritative

## 3.1 Disable native automatic cookie handling for compatibility clients

When the compatibility facade creates native clients or native request kwargs:

- do not pass flattened client cookies into native client construction;
- do not pass `request.cookies` as native per-request cookies;
- do not permit the native layer to independently persist Set-Cookie state for the compatibility path;
- pass only the final facade-generated `Cookie` header.

If the native binding has automatic cookie handling enabled by default, add the smallest explicit flag needed to disable it for compatibility clients. Do not redesign the native cookie jar.

## 3.2 Define request-cookie merge behavior

At request construction or before first dispatch:

- merge per-request cookies with the client jar according to HTTPX 0.28.1;
- generate one Cookie header for the concrete URL;
- ensure per-request cookies do not permanently pollute the client jar unless HTTPX does;
- regenerate the header on every redirect/auth hop;
- remove stale carried Cookie headers before regeneration.

## 3.3 Preserve scoped and duplicate cookies end to end

Add differential tests for:

- same name, different path;
- same name, different domain;
- host-only versus domain cookie;
- secure cookie over HTTP and HTTPS;
- expired and deletion cookies;
- multiple Set-Cookie headers;
- cookie set on redirect response used on next hop;
- cookie set on auth challenge used on auth follow-up;
- ambiguous `.get(name)` raising `CookieConflict`.

Add a transport fixture that records the exact outgoing Cookie header and proves no duplicate/native-added header appears.

### Track 3 acceptance criteria

- [ ] Exactly one cookie jar mutates compatibility client cookie state.
- [ ] No compatibility cookie collection is flattened into native client/request cookie arguments.
- [ ] Each hop carries at most one facade-generated Cookie header.
- [ ] Domain, path, secure, expiry, and host-only selection match HTTPX.
- [ ] Per-request cookie merge behavior matches HTTPX.
- [ ] Redirect and auth follow-ups receive newly set cookies in reference order.
- [ ] Duplicate-name scoped cookies remain representable.
- [ ] Tests prove that native cookie injection is disabled, not merely hidden.

# Track 4 — Enforce redirect replayability

## 4.1 Classify request body replayability

Introduce a small internal classification used by redirect construction:

- no body;
- buffered bytes / compatibility byte stream — replayable;
- already-read stream replaced with byte stream — replayable;
- seekable file-like input with recorded initial position — replayable after seek;
- one-shot sync iterable — not replayable after first dispatch;
- one-shot async iterable — not replayable after first dispatch;
- multipart — replayability derived from every part.

Avoid a general-purpose body framework. This classifier exists only to decide whether a retained-body redirect may proceed.

## 4.2 Rewind or rebuild before retained-body redirects

For 307/308 and other method/body-retaining cases:

- reuse cached bytes safely;
- rewind eligible file inputs to the recorded position;
- rebuild multipart stream only if all parts are replayable;
- reject one-shot consumed bodies before dispatching the redirect request.

Use HTTPX’s exception class and message category, typically `StreamConsumed`, for unsafe replay.

## 4.3 Do not over-restrict method-changing redirects

When 301/302/303 converts the request to GET:

- drop the body;
- remove body-specific headers;
- no replayability error should be raised for the discarded one-shot body.

## 4.4 Prove the second request body

Tests must record the bytes observed by both first and second transport hops.

Required cases:

- 307 with bytes body — identical bytes on both hops;
- 308 with seekable file — identical bytes on both hops;
- 307 with one-shot sync iterator — reference-equivalent failure before invalid second dispatch;
- async one-shot equivalent;
- 302 POST with one-shot body — redirects to bodyless GET without replay error;
- manual `next_request` state for replayable and non-replayable cases according to HTTPX.

### Track 4 acceptance criteria

- [ ] Retained-body redirects never silently send an empty or partial body.
- [ ] Buffered bodies replay byte-for-byte.
- [ ] Seekable bodies rewind to the correct original position.
- [ ] One-shot sync and async bodies fail before an invalid second dispatch.
- [ ] Method-changing redirects discard bodies without unnecessary replay failure.
- [ ] Multipart replayability is derived from its parts.
- [ ] Manual redirect behavior matches HTTPX for body ownership and `next_request`.

# Track 5 — Correct Rust-backed transport streaming and lifecycle

## 5.1 Use native streaming APIs in HTTPTransport

`HTTPTransport.handle_request()` and `AsyncHTTPTransport.handle_async_request()` must return a Response backed by an actual unconsumed native stream where the binding supports it.

Required properties:

- no eager full-body read in the transport;
- client-level `stream=False` reads the returned transport stream;
- client-level `stream=True` leaves it live;
- native pool/connection ownership remains attached until read or close;
- response extensions and request attachment remain intact.

If the native transport protocol cannot expose a live stream through this public transport path without a broad core rewrite, stop and document a bounded blocker rather than wrapping buffered content as a false live stream.

## 5.2 Correct custom transport response handling

For custom transports:

- buffered `Response(content=...)` remains buffered and closed;
- live stream Response remains live;
- non-stream Client sends read and close live responses before returning;
- stream Client sends leave them open;
- stream type mismatches fail clearly;
- ordinary application exceptions are not remapped into network exceptions.

## 5.3 Make transport closure permanent

After `HTTPTransport.close()` or `AsyncHTTPTransport.aclose()`:

- later `handle_request`/`handle_async_request` must fail;
- the native client must not be recreated;
- repeated close is idempotent;
- close exceptions follow the existing project policy and are not silently swallowed if HTTPX exposes them.

## 5.4 Apply protocol validation to transports

Reuse the compatibility client protocol validator:

- both false → reject;
- H2-only → either genuinely enforce or explicitly reject;
- supported combinations configure the native client;
- validation occurs before native client creation.

Do not store ignored protocol flags.

### Track 5 acceptance criteria

- [ ] Rust-backed transports use genuine live native streams or report an explicit blocker.
- [ ] Transport responses are not falsely reclassified from buffered to live.
- [ ] Client-level stream mode controls reading, not transport-level eager buffering.
- [ ] Pool/resource ownership is released on read completion and explicit close.
- [ ] Custom buffered and streaming responses retain correct body/state behavior.
- [ ] Closed transports cannot recreate native clients.
- [ ] Protocol combinations are validated consistently for clients and transports.
- [ ] Sync and async behavior are equivalent.

# Track 6 — Replace false-green coverage and add a small Tier 1 parity kernel

## 6.1 Add focused differential test files

Prefer updating existing files rather than adding many new modules. The corrective set should cover:

- timeout override propagation;
- Request auto-header/content state;
- Response initial/iteration/request/redirect/timing state;
- cookie single-authority behavior;
- redirect replayability;
- built-in/custom transport streaming and close permanence.

Every subtle case must execute against HTTPX 0.28.1 and candidate code, or use an expected value captured by a shared reference fixture.

## 6.2 Add a compact required Tier 1 parity kernel

Extend the existing `tier1_compat_smoke()` in `scripts/check.sh` with a small set of deterministic tests covering the corrected release-blocking behaviors.

The Tier 1 kernel should include approximately 10–25 fast cases, not the entire compatibility suite.

Required categories:

- top-level stream lifetime;
- request timeout extension override;
- empty POST Request preparation;
- unread Request content;
- buffered Response state;
- live Response iteration state;
- missing Response request;
- one cookie-authority assertion;
- one retained-body redirect replay assertion;
- one transport-close permanence assertion.

It must not:

- install extra services;
- use external network access;
- add a matrix;
- run long soak or downstream suites;
- duplicate the full extended suite.

## 6.3 Keep extended validation authoritative for full parity

`./scripts/check.sh extended` remains responsible for:

- full compatibility suite;
- API manifest comparison;
- downstream behavioral fixtures;
- existing lifecycle/resource checks.

Do not move all extended work into Tier 1.

## 6.4 Update parity registry and allowed differences

For every corrected behavior:

- update `parity-cases.toml` with the actual differential test;
- remove any active allowed difference that is now resolved;
- do not create an allowed difference for a required behavior merely to make the oracle green;
- ensure no test file listed in the registry is absent or only self-referential.

### Track 6 acceptance criteria

- [ ] Each corrective finding has a direct differential regression test.
- [ ] Former false-green tests now match HTTPX reference behavior.
- [ ] Tier 1 includes a small deterministic parity kernel.
- [ ] Tier 1 remains materially lighter than extended validation.
- [ ] Full compatibility suite remains in extended mode.
- [ ] Required corrective tests cannot pass by skip or xfail.
- [ ] Parity registry links to executable tests.
- [ ] No new workflow, matrix, report schema, or evidence artifact is added.

# Track 7 — Reconcile status, README, diagnostics, and closure evidence

## 7.1 Keep claims conservative during implementation

Until final validation passes:

- `Stage C candidate` remains the maximum claim;
- status remains reopened/in progress;
- README must not state that all remaining differences are non-behavioral;
- diagnostics must list any unresolved behavior blocker.

## 7.2 Run final validation on the final implementation SHA

After the last code/test commit, with a clean working tree, run:

```sh
./scripts/check.sh

./scripts/check.sh extended
```

At minimum, retain in the status file:

- exact full SHA;
- Tier 1 result;
- full compatibility result count;
- required skips/xfails count;
- API oracle unexplained/stale counts;
- relevant downstream fixture results;
- any remaining blocker.

Do not claim a GitHub Actions result that is not visible for the exact SHA.

## 7.3 Reconcile the closure status

Only after all global acceptance criteria pass, update `plans/httpx-parity-correction-status.md` with:

- this corrective plan listed as complete;
- exact implementation SHA;
- corrected test results;
- timeout, state, cookie, replay, and transport findings explicitly closed;
- remaining intentional unsupported surfaces;
- truthful final claim decision.

If any blocker remains, status must say `partially implemented` or `blocked`, not `complete`.

## 7.4 Correct README language

The README may say the facade is a Stage C candidate with scoped unsupported surfaces only if:

- no remaining required behavior difference from this plan exists;
- allowed differences do not hide ordinary behavioral incompatibility;
- focused and full compatibility checks pass.

Remove or qualify “drop-in” language if any of the corrective blockers remains.

### Track 7 acceptance criteria

- [ ] Status remains reopened until final exact-SHA validation.
- [ ] README does not overstate compatibility during implementation.
- [ ] Final status records actual local and visible CI evidence separately.
- [ ] No invisible or missing workflow run is reported as green.
- [ ] Final claim follows the result, not the implementation commit message.
- [ ] Remaining unsupported surfaces are exact and user-visible.

# Required validation cases

The following cases are mandatory and may not be replaced by broad test-count claims.

## Timeout

- [ ] Sync custom transport sees client-default timeout mapping.
- [ ] Sync custom transport sees explicit disabled timeout mapping.
- [ ] Sync custom transport sees explicit shorter timeout mapping.
- [ ] Async equivalents pass.
- [ ] Delayed local endpoint proves override behavior.
- [ ] Redirect and auth follow-up preserve the same effective timeout.

## Request state

- [ ] Empty POST, PUT, and PATCH match HTTPX auto headers.
- [ ] Explicit stream receives no auto headers.
- [ ] Unread sync and async streaming content raises `RequestNotRead`.
- [ ] Completed reads cache bytes and establish replayable state.
- [ ] Sync/async stream misuse fails correctly.

## Response state

- [ ] Buffered content response initial state matches HTTPX.
- [ ] Live sync `iter_bytes()` consumes and closes correctly.
- [ ] Live async `aiter_bytes()` consumes and closes correctly.
- [ ] Raw second consumption raises `StreamConsumed`.
- [ ] Iteration after close raises `StreamClosed`.
- [ ] Missing request and URL access raise.
- [ ] Redirect-location status set matches HTTPX.
- [ ] Delayed transport produces nonzero elapsed.

## Cookies

- [ ] Native client receives no compatibility cookie dict.
- [ ] Native request receives no compatibility cookie dict.
- [ ] Outgoing Cookie header is generated once by facade jar.
- [ ] Duplicate-name scoped cookies remain distinct.
- [ ] Redirect/auth cookie propagation matches HTTPX.

## Redirect replay

- [ ] Bytes replay on 307/308.
- [ ] Seekable file replay on 307/308.
- [ ] One-shot sync stream fails before second dispatch.
- [ ] One-shot async stream fails before second dispatch.
- [ ] Body-dropping redirect does not require replay.

## Transports

- [ ] Rust-backed sync transport returns live stream.
- [ ] Rust-backed async transport returns live stream.
- [ ] Non-stream Client reads and closes live transport response.
- [ ] Stream Client leaves response live until caller exit.
- [ ] Closed transport cannot reopen.
- [ ] Invalid protocol combinations fail before native client creation.

# Global acceptance criteria

This corrective pass is complete only when all of the following hold:

- [ ] Every Track 0–7 acceptance criterion is satisfied.
- [ ] Per-request timeout overrides affect both extensions and native behavior.
- [ ] Request and Response state matches pinned HTTPX for all mandatory cases.
- [ ] No false-green assertion remains for the disputed semantics.
- [ ] The compatibility facade has one authoritative cookie jar.
- [ ] No flattened compatibility cookie state enters native automatic cookie handling.
- [ ] Retained-body redirects never silently replay a consumed stream.
- [ ] Built-in transports provide genuine live streaming or the compatibility claim is explicitly reduced.
- [ ] Closed built-in transports cannot recreate resources.
- [ ] Tier 1 runs a compact corrective parity kernel.
- [ ] Extended validation runs the complete compatibility suite and API oracle.
- [ ] Required corrective tests have zero skips and zero xfails.
- [ ] API oracle has zero unexplained required-now differences.
- [ ] Active allowed differences do not waive any finding in this plan.
- [ ] `plans/httpx-parity-correction-status.md` is exact-SHA-bound and truthful.
- [ ] README and diagnostics match the proven compatibility surface.
- [ ] No new CI workflow, matrix, evidence schema, release automation, or networking implementation was introduced.

# Rejection conditions

Do not mark this pass complete if any of the following remains:

- `timeout` is accepted but dispatch still uses only `self._timeout`;
- an immediate 200 response is the only proof of timeout override;
- empty POST/PUT/PATCH differs from HTTPX preparation;
- streaming Request content returns `None` rather than raising unread-state error;
- buffered Response state remains open/unconsumed;
- valid live `iter_bytes()` requires a prior `.read()`;
- elapsed is assigned a constant zero placeholder;
- Response request returns `None` when unattached;
- arbitrary 3xx statuses with Location are treated as followable redirects;
- scoped cookies are still flattened into native cookie arguments;
- a one-shot body can reach a second retained-body redirect dispatch;
- HTTPTransport wraps a buffered native response as a false live stream;
- a closed HTTPTransport recreates its native client;
- false-green tests remain in the required suite;
- Tier 1 still omits every corrected behavior;
- the status file says “none” under blockers without exact-SHA proof;
- README states that all remaining differences are non-behavioral while any finding remains.

# Suggested commit decomposition

Keep the implementation reviewable and avoid one large opaque commit.

1. `test: reopen HTTPX parity closure and add failing reference cases`
2. `fix: apply per-request timeout overrides`
3. `fix: align HTTPX request and response state semantics`
4. `fix: make compatibility cookie jar authoritative`
5. `fix: enforce redirect body replayability`
6. `fix: correct HTTP transports streaming and lifecycle`
7. `test: add narrow Tier 1 parity kernel and reconcile registry`
8. `docs: close HTTPX corrective status on exact SHA`

Tests may accompany each implementation commit rather than being deferred, but the initial reference cases should land before or with the corresponding fix.

# Handoff requirements

The implementation agent must provide:

- starting SHA;
- final SHA;
- commits mapped to Tracks 0–7;
- exact focused test commands and results;
- Tier 1 result;
- extended result;
- API oracle result;
- unresolved blockers, if any;
- final status decision: `complete`, `partially implemented`, or `blocked`.

“Complete” is permitted only when all global acceptance criteria pass. A partial result must retain `Stage C candidate` and must not restore the previous no-blockers declaration.