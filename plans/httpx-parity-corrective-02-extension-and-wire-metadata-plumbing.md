# HTTPX Parity Corrective 02 — Extension and Wire Metadata Plumbing

Baseline reviewed: `4571cb55bc2ff49822608d750dfef185cff40ebc`
Depends on: Corrective 01 for the final safe TLS/SNI boundary where relevant.

## Objective

Make HTTPX request extensions behave consistently across normal buffered and streaming dispatch, bridge the Python `trace` extension into the core trace observer without storing Python objects in `eggfetch-core`, and propagate truthful response metadata through the native Python bindings and HTTPX compatibility facade.

The core typed `TransportHints` approach from Phase 03 should be retained. The defect is primarily incomplete plumbing and facade exposure, not the core abstraction.

## Current defects

### 1. Buffered request path drops transport extensions

The native `Client.request()`/verb methods do not accept an `extensions` argument. The compat facade only injects `request.extensions` when calling the native `stream()` path. Therefore `target` and `sni_hostname` work asymmetrically: they can reach the Rust engine for streaming calls but are lost for ordinary `Client.get()`, `Client.request()`, and corresponding async buffered calls.

Required correction: all native request entry points used by the compatibility facade must share the same extension extraction path.

### 2. Trace observer exists in core but HTTPX trace callback is not bridged

Core has typed trace events and pinned vocabulary, but HTTPX/httpcore request extension behavior expects a callback supplied via `request.extensions["trace"]`.

Required correction: add a Python-side adapter that receives Rust trace events and invokes the supplied Python callback with the pinned httpcore 1.0.9 event name/info shape. Python callables must not be stored in `eggfetch-core`.

### 3. Wire reason phrase is retained in core but lost in Python buffered response conversion

Core `Response` now stores `wire_reason_phrase`, and manual proxy parsing populates it. Native `PyResponse` currently derives `canonical_reason()` when building the Python response, which loses a noncanonical HTTP/1.x wire phrase.

Required correction: prefer `response.wire_reason_phrase()` when present, then fall back to canonical reason only when no wire phrase exists.

### 4. Response extension assembly is incomplete/inconsistent

HTTPX uses response `extensions` for transport metadata. The compat wrapper currently overlays fields opportunistically from native attributes, but the native response types do not provide a single explicit metadata export contract.

Required correction: define a small, truthful native response-extension export that can carry available values such as:

- `http_version`
- `reason_phrase`
- `network_stream` when actually owned/exposed by Corrective 03
- `stream_id` only if genuinely available; otherwise absent

Do not merge request extensions into response extensions.

## Required design

### A. One typed extension extraction helper in Python bindings

Create one binding-layer helper used by sync/async buffered and streaming request methods. It should:

1. accept `extensions: Optional[dict]`;
2. extract only supported native transport hints into `eggfetch_core::TransportHints`;
3. validate types deterministically;
4. extract/prepare a trace callback adapter separately from `TransportHints`;
5. reject malformed supported values with the closest HTTPX-compatible error behavior;
6. ignore/pass through unknown extension keys at the facade/request-object level as HTTPX does, without attempting to serialize arbitrary Python values into core.

Do not duplicate target/SNI parsing across four request paths.

### B. Add extensions to native buffered request dispatch

The native sync and async `request()` methods need an optional `extensions` argument, or an equivalent private internal method must be introduced that all public verb methods call.

Requirements:

- normal `.request()` receives the same `TransportHints` as `.stream()`;
- generated verb helpers (`get`, `post`, `put`, `patch`, `delete`, `head`, `options`) can remain API-stable if the compat facade dispatches through the common request method;
- no change may cause arbitrary Python extension objects to cross the GIL-release boundary;
- extract values before releasing the GIL;
- retries preserve intended hints according to existing core policy;
- redirects clear per-hop transport hints according to the previously documented policy unless HTTPX differential evidence says otherwise.

### C. Python trace callback bridge

Pin behavior to the existing `compat/httpx/0.28.1/trace-vocabulary.md` and HTTPX/httpcore 1.0.9 behavior.

Suggested architecture:

- core emits `TraceEvent` through the existing observer trait;
- Python binding installs an observer implementation that sends serializable event records to a binding-owned channel/callback bridge;
- callback execution occurs while the GIL is held;
- callback is invoked in request order;
- callback exceptions abort the operation and surface through the compatibility facade;
- async callback behavior must match the reference contract: if HTTPX expects an async callable for async transport, await it; do not silently call an async function without awaiting;
- callback info dictionaries contain only the keys the pinned event vocabulary defines and values with matching broad types.

If invoking Python synchronously from inside Rust transport execution creates reentrancy/GIL hazards, use a deterministic bridge that queues the trace event and allows the binding/facade layer to execute the callback at the correct lifecycle boundary. Do not move the Python callable into core.

### D. Target extension semantics

Retain the logical URL for:

- routing/connect destination
- cookies
- auth
- redirects
- proxy selection
- security/origin checks

Use `target` only for the request-target written on the wire.

Differential fixtures must include:

- `OPTIONS *`
- custom origin-form target
- absolute-form target through forward proxy
- unusual escaped path/query bytes supported by HTTPX
- invalid CR/LF/NUL rejection
- buffered sync
- streaming sync
- buffered async
- streaming async

### E. SNI extension semantics

`sni_hostname` changes only TLS server-name/certificate verification identity. It must not alter:

- TCP destination
- Host header
- proxy routing
- logical request URL

Pooling must not reuse an SNI-specific TLS connection for a request with a different SNI identity. Use the existing per-SNI client/cache mechanism only if its key fully separates transport policy.

Add buffered-path tests because that is the currently missing facade route.

### F. Response metadata export

Create a small binding helper, e.g. conceptually `response_extensions_from_core()`, used by buffered and streaming Python response conversion.

Rules:

- `http_version`: encode in the same broad form expected by the compat wrapper/reference; normalize only at the HTTPX facade boundary if necessary;
- `reason_phrase`: use actual wire phrase when available, otherwise canonical/default behavior;
- `network_stream`: include only if Corrective 03 provides a live/metadata wrapper appropriate for that response;
- `stream_id`: omit if unavailable; never set `0`, infer from ordering, or parse debug strings;
- unknown/native-only metadata should not pollute the HTTPX compatibility response unless documented as additive.

### G. Streaming response metadata

Ensure `PyStreamingResponse` exposes enough immutable response metadata before body consumption to build the same compat response fields as buffered responses:

- status
- URL
- headers
- HTTP version
- reason phrase
- available extension dict

Streaming H2 tests must assert `resp.http_version == "HTTP/2"`, not only body delivery.

## Required tests

### Request extension matrix

For each of sync/async and buffered/streaming:

1. `target=b"*"` produces `OPTIONS *` on the server;
2. normal logical URL remains unchanged in request/response objects;
3. invalid target bytes fail before wire dispatch;
4. `sni_hostname` permits a cert whose identity matches the override while TCP still connects to the URL host;
5. wrong SNI override fails certificate validation;
6. SNI-specific connection is not reused for a different SNI identity.

### Trace differential tests

Use a local server and compare event sequences with HTTPX/httpcore 1.0.9 for at least:

- successful HTTP/1.1 request;
- request-body send;
- response-body receive if the core observer supports those events;
- connection failure;
- TLS request;
- sync callback;
- async callback;
- callback raises -> request aborts and exception propagates.

The candidate may omit events the core cannot truthfully observe only if the difference is explicitly classified. Do not claim trace parity based solely on core unit tests.

### Reason phrase tests

Local raw HTTP/1.1 server sends a noncanonical status line such as `HTTP/1.1 299 Custom Phrase Here` or another parseable code/phrase fixture.

Assert:

- core retains the phrase where the transport parser can observe it;
- native buffered response exposes it;
- compat buffered response exposes it;
- compat streaming response exposes it where available;
- redirect history retains the actual phrase;
- H2 does not fabricate a wire reason phrase.

### Metadata tests

Assert response extension shape separately for:

- ordinary HTTP/1.1 buffered;
- ordinary HTTP/1.1 streaming;
- HTTP/2 buffered;
- HTTP/2 streaming;
- 101 upgrade once Corrective 03 lands;
- custom/mock transport extensions remain preserved exactly and are not overwritten by absent native metadata.

## Acceptance criteria

Corrective 02 is complete only when:

- [ ] One binding-layer parser handles native supported request extensions for sync/async buffered/streaming paths.
- [ ] Buffered `.request()`/`.get()` can exercise `target` and `sni_hostname` successfully.
- [ ] Streaming behavior remains unchanged or improves; no regression from unifying plumbing.
- [ ] Unknown extension keys remain on compatibility Request objects and are not incorrectly serialized into core.
- [ ] HTTPX trace callback is either fully bridged for the supported event subset with differential tests or explicitly remains an active difference; core observer existence alone is not called parity.
- [ ] Trace callback exceptions propagate and abort the request as tested.
- [ ] Actual wire reason phrase is preferred where available.
- [ ] Buffered and streaming compatibility responses expose the same available HTTP version/reason metadata.
- [ ] H2 streaming response metadata is tested explicitly.
- [ ] `stream_id` remains absent unless the real ID is available.
- [ ] No Python callable/object is stored in `eggfetch-core`.
- [ ] No duplicate networking path is introduced.
- [ ] Focused differential extension/metadata tests pass.

## Out of scope

Do not add a generic Python-object extension map to core. Do not fabricate metadata Hyper does not expose. Do not broaden this phase into arbitrary HTTPX private-extension compatibility beyond the pinned public/httpcore extension behaviors already identified.