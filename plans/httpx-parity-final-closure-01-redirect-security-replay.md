# HTTPX 0.28.1 Final Closure 01 — Redirect Cookie Security and Body Replay

Status: ready for implementation handoff

Date: 2026-08-04

Depends on: `plans/httpx-parity-final-closure-roadmap.md`

Audited baseline: `6ae10308b9db1e215eca19027d4ca9b7575900f6`

Pinned reference: `httpx==0.28.1`

## Objective

Correct the two remaining redirect defects without redesigning the cookie or request-body subsystems:

1. regenerate Cookie state per destination hop instead of preserving the previous outgoing Cookie header;
2. ensure every retained-method redirect either replays one authoritative body source or fails before the next transport dispatch.

This plan is security-sensitive because the current Cookie behavior can cross origin boundaries. It must be implemented before raw-stream or documentation closure work.

## Files expected to change

Primary:

- `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_request.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_cookies.py`, only if the existing jar API lacks a narrow helper
- `crates/eggfetch-python/tests/compat/`
- `crates/eggfetch-python/tests/compat/test_corrective_kernel.py`

Possible documentation/status touch at the start:

- `plans/httpx-parity-correction-status.md`

Do not change Rust core behavior unless a compatibility-facade blocker is demonstrated and reviewed.

# Track 0 — Reopen closure and capture the reference

## 0.1 Mark final closure as in progress

Before production changes, update the status file with:

- starting SHA `6ae10308b9db1e215eca19027d4ca9b7575900f6`;
- this plan path;
- status `final redirect and raw-stream closure in progress`;
- statement that the previous CI run remains historical evidence;
- Stage C candidate designation unchanged.

Do not remove prior history.

## 0.2 Add direct pinned-reference redirect cases

Create focused tests that execute equivalent scenarios against:

- `httpx==0.28.1`;
- `eggfetch.compat.httpx`.

Required Cookie cases:

1. client cookie on same-origin 302 redirect;
2. client cookie on cross-origin 302 redirect;
3. host-only cookie redirected to a sibling host;
4. domain cookie redirected to an eligible subdomain;
5. path-scoped cookie redirected inside and outside its path;
6. secure cookie across HTTPS to HTTP downgrade attempt;
7. explicit `Cookie` header on initial request followed by same-origin redirect;
8. explicit `Cookie` header on initial request followed by cross-origin redirect;
9. request-local `cookies=` on initial request followed by same-origin redirect;
10. request-local `cookies=` on initial request followed by cross-origin redirect;
11. intermediate redirect response sets a cookie eligible for the next hop;
12. intermediate redirect response sets a cookie ineligible for the next hop.

Required body cases:

1. buffered POST through 307;
2. buffered PUT through 307;
3. buffered PATCH through 308;
4. empty retained body through 307;
5. known reusable `ByteStream` through 307;
6. arbitrary iterator through 307;
7. generator through 308;
8. multipart bytes-only form through 307;
9. multipart file-like object through 307;
10. mixed data/files multipart through 308;
11. retained body with explicit Content-Length;
12. method rewrite through 303 with multipart input.

For each case record at minimum:

- number of transport dispatches;
- destination URL;
- method;
- outgoing Cookie header;
- outgoing Content-Type, Content-Length, and Transfer-Encoding;
- body bytes observed by the transport;
- exception class and whether it occurred before the second dispatch;
- redirect history length.

## 0.3 Use the pinned reference as the oracle

Tests may normalize:

- URLs to strings;
- headers to ordered multi-items;
- exception class names;
- body chunks to a byte sequence.

Tests must not:

- assert only EggFetch behavior;
- preserve the current copied-Cookie behavior as an allowed difference;
- infer multipart replayability without observing the reference and the current body representation;
- accept a second dispatch with an empty body when the original request had a body.

### Track 0 acceptance criteria

- Every confirmed Cookie and body defect has a direct reference-derived test.
- Both sync and async clients are covered for all public paths that have both forms.
- Cross-origin behavior is tested with distinct hostnames, not only different paths.
- Multipart tests verify actual bytes received by the second transport hop.
- Production code is unchanged until the disputed behavior is captured.

# Track 1 — Distinguish explicit Cookie input from generated outgoing state

## 1.1 Define the minimum internal state model

The redirect path must distinguish:

- persistent client-jar cookies;
- request-local `cookies=` additions;
- a user-supplied explicit `Cookie` header;
- a generated outgoing Cookie header produced for a specific URL;
- cookies extracted from response `Set-Cookie` headers.

Use the smallest private representation that preserves this distinction. Acceptable options include:

- a private Request field recording the original explicit Cookie header;
- a private extension key scoped to the compatibility facade;
- a private structured cookie-input record attached during Request construction.

Do not infer explicitness from the mere presence of `request.headers["cookie"]` after dispatch preparation because generated and explicit headers are indistinguishable at that point.

## 1.2 Preserve initial-request behavior

Initial requests must continue to merge Cookie state according to the pinned reference:

- persistent client jar;
- request-local `cookies=`;
- explicit Cookie header precedence.

Do not reintroduce native cookie kwargs.

The final wire request must contain at most one Cookie header produced by the compatibility facade.

## 1.3 Avoid public API expansion

The explicit/generated marker must remain private.

Do not add:

- a new public Request constructor argument;
- a second public cookie object;
- a public redirect-cookie policy;
- a second persistent jar.

### Track 1 acceptance criteria

- The implementation can identify whether the initial Cookie header was user supplied or generated.
- Initial one-hop Cookie behavior remains unchanged where it already matches HTTPX.
- The compatibility facade remains the only Cookie authority.
- No native client or transport receives a `cookies=` kwarg.
- No public API surface is added.

# Track 2 — Regenerate Cookie state on every redirect hop

## 2.1 Remove the prior outgoing Cookie header

`_redirect_headers()` or the equivalent redirect-construction boundary must remove the prior outgoing `Cookie` header for every redirect, including same-origin redirects.

This must occur before the next Request is prepared.

Do not preserve a prior generated Cookie header as an explicit header.

## 2.2 Build next-hop cookies from destination-eligible state

The next redirect Request must derive Cookie state from:

- the persistent client jar after processing the redirect response;
- only any request-local or explicit state that HTTPX 0.28.1 actually carries to that redirect hop.

The pinned reference currently regenerates from the client jar rather than copying the old header. Match that behavior unless the direct differential tests show a narrower exception.

Destination selection must continue to apply:

- host-only rules;
- domain matching;
- path matching;
- secure-only rules;
- expiry and deletion;
- cookie ordering already supported by the facade jar.

## 2.3 Process intermediate Set-Cookie before building the next hop

Verify ordering in both sync and async redirect loops:

1. receive redirect response;
2. extract all `Set-Cookie` headers into the persistent facade jar;
3. resolve destination URL;
4. build the redirect Request;
5. generate destination Cookie header;
6. dispatch the next hop.

Do not dispatch the next hop before jar extraction completes.

## 2.4 Preserve auth and host boundary behavior

Cookie changes must not regress:

- Authorization stripping across origins;
- HTTPS upgrade exception behavior already implemented for Authorization;
- Host header replacement;
- redirect fragment handling;
- redirect history and `next_request`.

Cookie scope and Authorization scope are separate. Do not copy the Authorization HTTPS-upgrade exception into Cookie handling.

### Track 2 acceptance criteria

- No prior-hop Cookie header is copied into a redirect Request.
- Same-origin redirects regenerate Cookie state.
- Cross-origin redirects receive only cookies eligible for the destination.
- A host-only cookie never reaches a sibling host.
- A domain cookie reaches an eligible subdomain only when the reference does.
- Path and secure scoping remain correct.
- Intermediate Set-Cookie values are visible to the next hop when eligible.
- Explicit initial Cookie behavior matches the reference for same-origin and cross-origin redirects.
- Request-local cookies match the reference for same-origin and cross-origin redirects.
- Sync and async results are identical.

# Track 3 — Create one retained-body replay classifier

## 3.1 Classify all current body representations

Introduce one small internal helper that classifies a Request body as one of:

- `empty`;
- `buffered-bytes`;
- `reusable-byte-stream`;
- `multipart-reconstructable`;
- `one-shot-stream`;
- `file-cursor-uncertain`;
- `unsupported-body-state`.

The helper must inspect the actual current Request representation, including:

- `_content`;
- `_stream`;
- `_files`;
- `_multipart_data`;
- any private encoded multipart state already present.

Do not scatter replay decisions across `_redirect_stream()`, `_build_redirect_request()`, and dispatch code.

## 3.2 Use exactly one body source

A redirected Request must receive exactly one of:

- `content=<bytes>`;
- `stream=<fresh reusable stream>`;
- `data=` and `files=` only when reconstruction is demonstrably safe and produces a fresh body;
- no body for a method rewrite to GET;
- an exception before dispatch.

Never pass both `content=` and `stream=`.

Never preserve body headers while omitting a non-empty body.

## 3.3 Prefer bytes over reconstruction when bytes already exist

If the original request has a stable buffered byte representation, replay those bytes directly.

Do not re-encode multipart data on the second hop when the exact encoded bytes already exist, because re-encoding may change boundaries or file cursor behavior.

## 3.4 Keep uncertain file and multipart inputs fail-closed

For file-like objects, generators, and multipart encoders whose input cursor may have advanced:

- do not call `seek(0)` automatically;
- do not assume repeatability because an object has a `read()` method;
- do not consume the source again merely to test replayability;
- raise the pinned-reference-compatible stream exception before the second transport dispatch.

Use the existing exception hierarchy where possible. Prefer `StreamConsumed` if it is the closest supported reference outcome.

## 3.5 Allow narrow reconstruction only when provably safe

A multipart request may be reconstructed only when every part is already represented by immutable reusable values and the current encoder can create a fresh stream without mutating shared cursors.

Examples that may qualify after tests:

- text-only fields;
- bytes-valued file parts;
- immutable in-memory values already copied by Request construction.

Examples that must remain rejected unless an existing explicit replay contract exists:

- open file objects;
- generators;
- custom streaming readers;
- reused multipart encoders;
- unknown objects with `read()`.

Do not add a generalized rewind abstraction in this pass.

### Track 3 acceptance criteria

- Every current body representation has an explicit replay classification.
- Buffered bodies replay through one source.
- Empty bodies remain empty and replayable.
- Multipart bodies never disappear silently.
- File-backed or one-shot multipart redirects fail before the second dispatch unless safely reconstructed.
- Body headers match the actual replayed body.
- No arbitrary file rewind behavior is added.
- The helper is shared by sync and async redirect paths.

# Track 4 — Integrate replay classification into redirect construction

## 4.1 Method rewrites take precedence

For 301, 302, and 303 cases that rewrite the method to GET:

- drop the body;
- remove Content-Length and Transfer-Encoding;
- remove Content-Type when HTTPX does so for the captured case;
- do not run replay classification unnecessarily.

## 4.2 Retained methods require successful replay classification

For 307 and 308, and any other case retaining method/body:

- classify before building the next Request;
- build the next Request from one body source;
- fail before the next transport call when not replayable.

## 4.3 Preserve redirect metadata

Continue to preserve:

- public request extensions;
- hop-local internal timing reset;
- redirect history;
- `next_request` behavior;
- response cleanup;
- custom transport extensions.

Do not preserve private per-hop Cookie or timing markers that must be regenerated.

## 4.4 Validate actual dispatch count

Tests for unreplayable bodies must assert that the transport handler was invoked exactly once.

An exception after the second dispatch is not acceptable.

### Track 4 acceptance criteria

- 307/308 buffered bodies arrive byte-for-byte on the second hop.
- Multipart immutable bodies either arrive byte-for-byte or fail before the second hop according to the reference-derived policy.
- File-backed and generator bodies do not dispatch a second request unless an explicit reusable contract exists.
- 303 body dropping remains correct.
- Redirect history and response cleanup remain green.
- Sync and async dispatch counts agree.

# Track 5 — Add bounded regression coverage

## 5.1 Tier 1 additions

Add only the highest-risk deterministic regressions to `test_corrective_kernel.py`:

- cross-origin redirect does not preserve an original-domain Cookie;
- intermediate Set-Cookie is regenerated for the next eligible hop;
- multipart or file-backed retained body does not silently become empty;
- unreplayable retained body fails before a second dispatch.

Keep the Tier 1 file compact. Parameterization is preferred over duplicated tests.

Do not install HTTPX in Tier 1.

## 5.2 Full compatibility coverage

The full suite must include all Track 0 differential scenarios, including:

- same-origin and cross-origin;
- sync and async;
- 307 and 308;
- POST, PUT, and PATCH where relevant;
- explicit header and request-local cookie variants;
- multipart immutable and file-backed variants.

## 5.3 Negative assertions

Tests must explicitly reject:

- copied Cookie headers;
- sibling-host leaks;
- duplicate Cookie headers;
- duplicate Cookie names from two authorities;
- second dispatch for one-shot bodies;
- empty second-hop body with retained body headers;
- simultaneous `content` and `stream` construction.

### Track 5 acceptance criteria

- Tier 1 catches the security boundary and silent-body-loss regressions.
- Full tests establish exact HTTPX 0.28.1 behavior.
- No test relies solely on candidate output for disputed behavior.
- Runtime remains proportionate.
- No workflow or matrix change is made.

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

During implementation, run the new redirect module directly with verbose node IDs so each differential case is visible.

# Final acceptance checklist

This plan is complete only when:

- direct pinned-reference tests pass;
- prior-hop Cookie headers are removed on all redirects;
- next-hop Cookie state is regenerated for the destination;
- cross-origin and path/domain/security boundaries are correct;
- intermediate Set-Cookie ordering is correct;
- all body representations are classified;
- multipart bodies replay safely or fail before redispatch;
- no retained body disappears;
- no arbitrary file rewind framework is added;
- Tier 1 remains compact;
- routine checks pass;
- the implementation SHA is handed to Plan 03 for exact evidence closure.
