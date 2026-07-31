# HTTPX Parity Correction Phase 3 — Transport, Mount, Hook, and One-Hop Dispatch Boundaries

Status: ready for implementation handoff

Depends on:

- `plans/httpx-parity-correction-roadmap.md`
- `plans/httpx-parity-correction-phase-1-entrypoints-client-lifecycle.md`
- `plans/httpx-parity-correction-phase-2-request-response-semantics.md`

## Objective

Establish a precise single-hop dispatch boundary for the compatibility facade and correct transport, mount, event-hook, extension, body-preservation, and ownership behavior around that boundary.

The central design requirement is:

> One compatibility dispatch call sends exactly one prepared Request through exactly one selected transport and returns exactly one Response without internally performing compatibility-visible redirects, auth challenge loops, or cookie transitions.

This permits Phase 4 to implement HTTPX’s redirect/auth/cookie state machine in the Python compatibility layer while all real network I/O remains in Rust.

## Audited files

Review at minimum:

- `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_transports.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_mock.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_wsgi.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_asgi.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_request.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_response.py`
- native Python `Client.request`, `Client.stream`, `AsyncClient.request`, and `AsyncClient.stream`
- core redirect configuration and one-hop request behavior
- current mount and event-hook tests

## Scope constraints

This phase may:

- force native compatibility dispatch to `follow_redirects=False` per hop;
- add a small internal one-hop dispatch abstraction;
- correct custom transport adapters;
- replace mount matching with a faithful public-pattern matcher;
- correct hook placement around each dispatch;
- expose missing response/body metadata needed for wrapping;
- add focused sync and async tests.

This phase must not:

- move socket or HTTP protocol work into Python;
- implement redirects, auth challenge loops, or cookies beyond the one-hop interface required by Phase 4;
- add SOCKS, UDS, local-address, or socket-option functionality;
- add a new CI workflow or generalized plugin framework;
- retain silent no-op transport options.

# Track 1 — Define the one-hop dispatch contract

## 1.1 Introduce an internal dispatch result contract

Create one internal sync path and one internal async path that accept:

- a fully prepared compatibility `Request`;
- selected transport;
- stream/buffer mode;
- effective timeout through request extensions or explicit internal argument.

They return a compatibility `Response` representing one transport response.

The dispatch contract must not:

- apply auth;
- follow redirects;
- run user event hooks;
- mutate cookie state;
- append history.

Those operations belong to the higher-level client state machine.

## 1.2 Force native transport to one-hop mode

Every native compatibility dispatch must explicitly disable native redirect following, regardless of the client-level compatibility `follow_redirects` setting.

The compatibility client will decide whether and how to construct and send a redirect request in Phase 4.

Confirm that setting `follow_redirects=False` on native requests:

- returns redirect response bodies and headers intact;
- does not mutate away the Location header;
- does not pre-apply redirected cookies or auth;
- preserves a replayable request body for compatibility orchestration;
- does not create native-only history that conflicts with compatibility history.

If native client-level redirect defaults cannot be overridden per request, add the smallest binding/core option needed to guarantee single-hop behavior.

## 1.3 Put effective timeout data into request extensions

Match HTTPX’s public transport contract by ensuring built requests include an effective timeout extension unless the caller supplied one.

Preserve explicit caller extensions. The compatibility transport adapter may translate this extension into native timeout configuration, but must not delete unrelated extension keys.

## 1.4 Set response request, stream binding, and timing once

The one-hop wrapper must:

- attach the exact dispatched compatibility Request;
- preserve response extensions;
- preserve buffered versus streaming body type;
- bind elapsed timing to the response lifecycle;
- ensure transport close/error paths close the response body once.

Do not repeatedly wrap an already compatible Response and lose `_content`, `_stream`, or state flags.

### Track 1 acceptance criteria

- [ ] Native compatibility dispatch never follows redirects internally.
- [ ] One-hop dispatch does not apply auth, hooks, cookies, or history.
- [ ] Effective timeout data is available to custom transports through request extensions.
- [ ] The returned Response is attached to the exact Request sent.
- [ ] Buffered versus streaming body state survives dispatch wrapping.
- [ ] One-hop errors do not leak response or transport resources.

# Track 2 — Correct custom and built-in transport adaptation

## 2.1 Enforce transport request/response types

Sync clients require a `BaseTransport`-compatible object returning a compatibility Response with a sync byte stream.

Async clients require an `AsyncBaseTransport`-compatible object returning a compatibility Response with an async byte stream.

Do not automatically execute a sync transport inline on the event loop unless HTTPX 0.28.1 permits that exact configuration. If the existing nonstandard `async_transport` argument is retained, classify it as an eggfetch extension rather than part of HTTPX parity.

Raise clear runtime errors for sync/async stream mismatches before body consumption.

## 2.2 Preserve buffered custom transport responses

Fix the path where a custom or mounted transport returns:

```python
Response(200, content=b"body")
```

and the caller requested `stream=True`.

The response must still yield `b"body"` through streaming iteration. Acceptable implementation approaches include:

- retaining the buffered content and treating it as an already-read response;
- wrapping content in the correct sync/async byte stream while preserving content state.

It is not acceptable to extract a missing `_stream` and discard `_content`.

## 2.3 Preserve custom streaming response objects

For custom transport responses with explicit streams:

- do not eagerly read when `stream=True`;
- read and close when `stream=False` according to HTTPX client behavior;
- bind response state and timing;
- preserve iterator type;
- propagate stream exceptions without remapping ordinary application errors into network errors.

## 2.4 Correct HTTPTransport/AsyncHTTPTransport buffering mode

The Rust-backed transport classes must dispatch through native buffered or streaming APIs according to caller/client mode.

Do not always call buffered `client.request()` and then pretend the response is streaming.

Where the public transport protocol does not carry a separate stream flag, preserve HTTPX’s expectation that transport responses expose a stream and allow the higher client layer to decide whether to read it. A native transport may therefore need always to return a compatibility stream-backed Response.

## 2.5 Preserve exception taxonomy

Only map native eggfetch transport exceptions into compatibility transport exceptions.

Do not catch arbitrary exceptions from custom, Mock, WSGI, or ASGI transports and reinterpret them as an eggfetch network error unless HTTPX would map them. Application exceptions and assertion failures should remain visible.

### Track 2 acceptance criteria

- [ ] Sync and async transport stream-type mismatches fail clearly.
- [ ] Buffered custom responses retain their body under `stream=True`.
- [ ] Streaming custom responses remain lazy under `stream=True`.
- [ ] Non-stream sends fully read and close transport responses as HTTPX does.
- [ ] Rust-backed transports use a real streaming path where required.
- [ ] Arbitrary custom transport exceptions are not incorrectly remapped.
- [ ] Mock, WSGI, and ASGI transports retain their intended exception behavior.

# Track 3 — Implement faithful mount matching

## 3.1 Model mount patterns explicitly

Replace ad hoc scoring over raw strings with an internal pattern object equivalent to HTTPX’s public URL-pattern behavior.

Support at minimum patterns present in HTTPX 0.28.1 public mounts:

- `all://`;
- `http://`;
- `https://`;
- exact host;
- host plus port;
- wildcard domains such as `all://*.example.com`;
- exact-domain patterns where wildcard semantics distinguish apex and subdomains;
- path prefixes if the existing facade intentionally supports them as an extension.

Do not let a non-HTTPX path extension alter the priority of standard HTTPX patterns unless documented.

## 3.2 Match HTTPX priority ordering

Priority must be deterministic and reference-derived. More specific patterns should win according to HTTPX’s scheme, host, and port ordering, not arbitrary insertion order except where the reference uses stable insertion order as a tie-breaker.

Add differential cases for overlapping patterns.

## 3.3 Preserve explicit `None` mounts

A matching mount with value `None` is meaningful, especially for bypassing proxy routes.

The matcher must distinguish:

- no pattern matched;
- a pattern matched a transport object;
- a pattern matched `None`.

Use a dedicated sentinel or return structure. Do not use plain `None` for both states.

When `None` matches, dispatch through the client’s default direct transport rather than falling through into a broader proxy mount.

## 3.4 Reject malformed patterns early

Invalid schemes, wildcard syntax, ports, or ambiguous patterns must raise during client construction, not on the first unrelated request.

### Track 3 acceptance criteria

- [ ] Scheme-only, host, host-port, wildcard, and catch-all patterns match the pinned reference.
- [ ] Priority among overlapping mounts is deterministic and differential-tested.
- [ ] Explicit `None` mounts bypass broader routes correctly.
- [ ] No-match and matched-None states are distinct.
- [ ] Malformed mount patterns fail at construction.
- [ ] Existing nonstandard path matching is either preserved as a documented extension or removed from the parity claim.

# Track 4 — Place event hooks at per-hop boundaries

## 4.1 Remove hooks from the outer one-time send wrapper

Request hooks must not run only once before auth modifies the request. Response hooks must not run only on the final response.

Move hook invocation to the higher-level per-hop send function that Phase 4 will use for each auth or redirect request.

This phase should expose the correct function boundary even before the complete state machine lands.

## 4.2 Define exact ordering

For each actual hop:

1. auth/state-machine code yields the concrete Request;
2. request hook runs on that Request;
3. one-hop transport dispatch occurs;
4. the Response is attached and prepared;
5. response hook runs on that Response;
6. redirect/auth/cookie state logic decides the next action.

Hooks must see the same Request that reaches the transport.

## 4.3 Handle hook exceptions safely

If a request hook raises:

- no transport dispatch occurs;
- no response exists to close;
- client remains usable unless closed independently.

If a response hook raises:

- close the response;
- release pool/stream resources;
- propagate the original hook exception;
- do not continue auth or redirects.

## 4.4 Match sync and async hook rules

Sync Client accepts and calls sync hooks.

AsyncClient must await async hooks. Determine pinned HTTPX behavior for a sync callable supplied to AsyncClient and match it explicitly; do not rely only on `asyncio.iscoroutinefunction`, which misses callable objects returning awaitables.

Use call-and-inspect semantics where needed:

- call hook;
- await the result if it is awaitable in async mode;
- enforce reference behavior in sync mode.

### Track 4 acceptance criteria

- [ ] Request hooks run on every actual hop and see the transported Request.
- [ ] Response hooks run on every actual hop before redirect/auth continuation.
- [ ] Request-hook failure prevents dispatch.
- [ ] Response-hook failure closes the Response and halts continuation.
- [ ] Async callable objects returning awaitables are handled correctly.
- [ ] Hook ordering is proven for direct, mounted, custom, Mock, WSGI, ASGI, and native transports.

# Track 5 — Preserve extensions end to end

## 5.1 Merge client and request extensions losslessly

Client extensions provide defaults. Request extensions override matching keys. Repeated or nested extension values must not be coerced or serialized unnecessarily.

Timeout insertion must not overwrite an explicit `extensions["timeout"]`.

## 5.2 Pass extensions through transports

Custom transports receive the Request with extensions intact.

Native transport adapters consume only recognized internal keys and preserve the full extension mapping on the Response where HTTPX does.

Standard response extensions such as:

- `http_version`;
- `reason_phrase`;
- `network_stream` where applicable;

must remain available.

## 5.3 Avoid request-extension leakage into response extensions

Do not copy all request extensions into response extensions merely for convenience. Preserve only behavior the pinned reference exposes, while ensuring request extensions remain on `response.request.extensions`.

Audit the current wrapper, which may merge request extensions into response extensions.

### Track 5 acceptance criteria

- [ ] Client and request extension merge is lossless.
- [ ] Explicit timeout extension is preserved.
- [ ] Custom transports see all request extensions.
- [ ] Standard response extensions survive native wrapping.
- [ ] Request-only extensions are not falsely reported as response metadata.
- [ ] Extension behavior is covered for buffered and streaming sends.

# Track 6 — Correct transport ownership and close semantics

## 6.1 Define ownership once

A Client owns:

- its default transport if it constructed or was given it under HTTPX semantics;
- each mounted transport instance, but duplicate object instances must be closed once;
- its native client/transport resources.

Do not close the same transport multiple times merely because it is mounted under several patterns.

## 6.2 Propagate close errors according to HTTPX

Audit current broad `except Exception: pass` close behavior.

Match HTTPX’s expected close behavior. Do not silently swallow all custom transport close errors if the reference propagates them.

Native cleanup may remain idempotent, but unexpected application transport cleanup failures should not disappear without an explicit policy.

## 6.3 Close partially opened resources

If client construction, transport lazy initialization, hook execution, or first dispatch fails after some resources are opened, close those resources deterministically.

### Track 6 acceptance criteria

- [ ] Duplicate mounted transport instances close once.
- [ ] Default and mounted transport ownership is explicit.
- [ ] Close error behavior matches HTTPX or has an exact documented difference.
- [ ] Partial initialization does not leak native or in-process transport resources.
- [ ] Sync and async close paths remain type-correct.

# Testing plan

Suggested focused files:

- `test_one_hop_dispatch.py`
- `test_transport_body_preservation.py`
- `test_mount_pattern_parity.py`
- `test_event_hook_per_hop.py`
- `test_extension_propagation.py`
- `test_transport_ownership.py`

Required test transports:

- buffered sync custom transport;
- streaming sync custom transport;
- buffered async custom transport;
- streaming async custom transport;
- mounted transport returning a redirect;
- explicit `None` mount under a broader proxy/catch-all mount;
- hook that raises before dispatch;
- hook that raises after response;
- duplicate transport instance under two mount patterns.

Run through the existing suite:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers

./scripts/check.sh
```

Do not add a new CI job, matrix, or generic transport plugin system.

# Phase completion criteria

Phase 3 is complete only when:

- every Track 1–6 acceptance item is satisfied;
- native dispatch represents one hop with redirects disabled;
- custom and mounted transport responses never lose buffered or streaming content;
- mount matching handles explicit `None` and wildcard patterns correctly;
- event hooks run at the per-hop boundary in reference order;
- request and response extensions are not conflated;
- transport close ownership is deterministic;
- required focused tests pass with zero skips and zero xfails;
- ordinary compatibility and routine validation remain green;
- no new CI architecture was introduced.

## Stop conditions

Stop and record a blocker if:

- the native binding cannot guarantee one-hop redirect-disabled behavior;
- native responses cannot expose an unconsumed stream to HTTPTransport without a broad engine rewrite;
- the existing pool lease cannot remain attached to a compatibility streaming Response;
- HTTPX mount wildcard semantics conflict with a retained eggfetch extension and cannot coexist cleanly;
- correct hook ordering requires a callback inside the Rust redirect loop rather than facade-owned one-hop orchestration.

The preferred resolution is to keep redirect orchestration in the compatibility layer. Do not introduce a complex cross-language callback framework unless one-hop dispatch is demonstrably impossible.