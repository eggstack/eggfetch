# HTTPX 0.28.1 Parity Completion — Phase 3: Exact Signatures, Transport Inheritance, and Stream Type Surface

Status: ready for implementation handoff

Date: 2026-08-07

Roadmap: `plans/httpx-parity-completion-roadmap.md`

Prerequisites:

- Phase 1 contract rebaseline complete;
- Phase 2 Python object/configuration contracts complete or bounded blockers recorded.

Pinned reference: `httpx==0.28.1`

Compatibility designation: `Stage C candidate`

## Objective

Align the remaining public Python call signatures, parameter kinds/defaults, transport inheritance relationships, and stream type surface with HTTPX 0.28.1 while preserving the already-accepted networking and stream lifecycle behavior.

This phase is primarily about source compatibility and Python type shape. It must not reopen redirect/cookie/auth/raw-stream/cancellation behavior and must not implement the advanced native transport capabilities assigned to Phases 4–5.

## Why this phase is separate

The existing facade often accepts HTTPX-compatible calls but exposes broader `*args`/`**kwargs` signatures or different class relationships. Those differences matter to:

- Python's own argument validation;
- wrappers that inspect accepted keyword/positional parameters;
- dependency-injection frameworks;
- IDE/type tooling;
- libraries using `inspect.signature`, `issubclass`, or `isinstance`;
- code that relies on transport/stream base classes.

They should be fixed without conflating the work with native connector implementation.

## Inputs from Phase 1

Phase 1 must provide the exact difference IDs assigned here. Expected families include:

- top-level HTTP verb/helper signature records (`HTTP-METHOD-ARGS-*` or equivalent);
- `Client` and `AsyncClient` constructor/method parameter-kind/default records;
- `HTTPTransport` / `AsyncHTTPTransport` parameter signature records (`TRANSPORT-PARAMS-*` or equivalent);
- transport base-class records for Mock/WSGI/ASGI transports;
- `ByteStream`, `SyncByteStream`, and `AsyncByteStream` inheritance/method/constructor records;
- any remaining public class signature records not owned by Phase 2.

Do not use these family names if Phase 1 rebaseline renamed them; use the current ledger IDs.

## Likely implementation files

- `crates/eggfetch-python/python/eggfetch/compat/httpx/__init__.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_request.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_response.py` only if public method signatures require it
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_stream.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_transports.py`
- WSGI/ASGI/Mock transport modules
- compatibility tests and API manifest/ledger files.

## Scope firewall

### In scope

- exact public parameter ordering;
- positional-only / positional-or-keyword / keyword-only behavior;
- public default values and sentinels where semantics are already supported;
- public top-level helper signatures;
- public Client/AsyncClient method signatures;
- transport constructor signature shape;
- public transport inheritance relationships;
- public stream class inheritance and close/iteration method surface;
- runtime argument rejection matching the reference;
- allowlist cleanup for resolved signature/type differences.

### Out of scope

Do not implement:

- UDS behavior;
- local-address binding;
- socket-option application;
- SOCKS;
- Trio/AnyIO;
- Python 3.8/3.9;
- private HTTPX modules;
- a new typing package/stub generator;
- new native networking primitives;
- changes to raw/decoded response lifecycle semantics already accepted in earlier closure.

If a signature includes a parameter whose runtime capability belongs to Phase 4 or 5, preserve the existing explicit unsupported behavior until that owning phase implements it. Signature parity does not justify falsely claiming functional support.

## Track 0 — Generate a precise signature/type inventory

### 0.1 Use the existing reference manifest

Extract the exact reference tuple for each target symbol from `compat/httpx/0.28.1/reference-api.json` and/or direct `inspect.signature` against installed `httpx==0.28.1`.

Do not hand-transcribe signatures from memory.

For each callable record:

- parameter names;
- order;
- parameter kinds;
- defaults, including sentinel/default object representation where relevant;
- presence of `*` keyword-only boundary;
- variadic parameters;
- return annotation only if the current oracle treats it as contract-relevant.

For each type record:

- direct public bases;
- inherited iteration/close/context-manager behavior;
- abstract/protocol expectations observable through normal public use.

### 0.2 Build differential argument-validation tests

For each important callable, test both valid and invalid invocation forms.

A signature is not considered matched merely because `inspect.signature` looks correct. Python runtime argument acceptance must match.

### Track 0 acceptance criteria

- Every Phase 3 target symbol has a pinned reference signature/type record.
- Tests cover runtime argument validation, not metadata alone.
- No private HTTPX type is promoted into the compatibility contract solely to make MRO text look identical.

## Track 1 — Top-level request/helper functions

### 1.1 Replace broad variadic helpers

Where the facade currently exposes helpers through broad `*args, **kwargs`, define normal Python signatures matching HTTPX 0.28.1 for the target functions.

Expected helpers include, as present in the reference manifest:

- `request`;
- `get`;
- `options`;
- `head`;
- `post`;
- `put`;
- `patch`;
- `delete`.

Do not assume all verbs share identical parameter lists. Use the reference manifest.

### 1.2 Preserve one authoritative dispatch implementation

Exact wrappers should normalize arguments and delegate into the existing implementation. Do not duplicate request construction, auth, cookie, timeout, proxy, redirect, or response logic per verb.

Preferred structure:

- one internal implementation function;
- thin explicit-signature public wrappers.

### 1.3 Differentially test positional rejection

Cover at minimum:

- URL position;
- accidentally positional keyword-only arguments;
- unknown keywords;
- duplicate argument specification;
- supported body/query/header/auth/timeout parameters;
- verb-specific body parameters where HTTPX differs.

### Track 1 acceptance criteria

- Target helper `inspect.signature` output matches the reference tuple.
- Runtime invalid positional/keyword calls fail in the same broad way as HTTPX (`TypeError` where appropriate).
- Valid requests still traverse the existing single dispatch path.
- No helper-specific request behavior is duplicated.

## Track 2 — `Client` and `AsyncClient` public call surface

### 2.1 Align constructor signature shape

Using the pinned reference, align:

- parameter order;
- keyword-only boundary;
- defaults;
- accepted documented parameters already supported by the facade.

Do not introduce Phase 4/5 functionality here merely because a low-level transport parameter exists elsewhere.

### 2.2 Align major methods

At minimum audit and align signatures for:

- `build_request`;
- `request`;
- verb helpers;
- `send`;
- `stream`;
- `close` / `aclose` where applicable.

Preserve current lifecycle and state-machine behavior.

### 2.3 Preserve sentinel semantics

HTTPX uses sentinels to distinguish "use client default" from explicit `None` for some parameters. If the facade already models that distinction internally, expose the exact public signature around it.

If Phase 2 corrected config defaults, use those canonical objects rather than creating duplicate sentinel semantics.

### 2.4 Do not fake signatures

Do not solve normal callable mismatches by assigning a custom `__signature__` while leaving a permissive `*args/**kwargs` runtime implementation.

A narrow `__signature__` override is acceptable only if:

- Python prevents expressing the reference signature directly for a descriptor/wrapped extension callable;
- runtime argument validation is independently equivalent;
- the Phase 1 inventory explicitly records why the override is necessary.

### Track 2 acceptance criteria

- Target constructor/method signatures match the reference.
- Runtime argument acceptance/rejection matches the reference for the differential corpus.
- Existing client default merge, auth, cookies, redirects, mounts, hooks, and close behavior remain unchanged.

## Track 3 — Low-level transport constructor signatures

### 3.1 Align `HTTPTransport` and `AsyncHTTPTransport` public signature shape

The current compatibility constructors accept `uds`, `local_address`, and `socket_options` but reject them before network activity. Phase 4 will make them functional.

This phase must align the constructor's public parameter tuple to HTTPX 0.28.1, including:

- parameter names/order;
- keyword-only behavior;
- limits default semantics;
- retry parameter behavior;
- proxy parameter shape;
- any extra EggFetch parameter not present in HTTPX, such as a compatibility-layer-only `timeout`, according to Phase 1 classification.

### 3.2 Handle extra parameters deliberately

If the current transport exposes an EggFetch-only parameter that is absent in HTTPX:

- remove it from the compatibility constructor if doing so does not break an established compatibility consumer;
- or retain it only if Phase 1 explicitly classifies it as additive and it can coexist with exact reference source compatibility.

Do not let an additive parameter force a broad variadic signature.

### 3.3 Do not claim advanced-option functionality yet

Until Phase 4 lands, `uds`, `local_address`, and `socket_options` may continue to raise the existing explicit unsupported exception when non-`None`.

Documentation must still say they are pending Phase 4 during this intermediate state.

### Track 3 acceptance criteria

- Transport constructor signatures match the pinned reference for targeted fields.
- Default no-option construction continues using the current native client path.
- Advanced parameters are not silently ignored.
- No native connector change occurs in this phase.

## Track 4 — Transport base-class relationships

### 4.1 Align public transport inheritance

Use the HTTPX public transport relationships captured in Phase 1/reference manifest.

Expected areas to audit:

- `HTTPTransport` → `BaseTransport`;
- `AsyncHTTPTransport` → `AsyncBaseTransport`;
- `WSGITransport` relationship to sync base;
- `ASGITransport` relationship to async base;
- `MockTransport` relationship(s) used by HTTPX.

Do not infer the exact MockTransport MRO; take it from the pinned reference.

### 4.2 Preserve method semantics

Changing base classes must not alter:

- sync vs async handler selection;
- context-manager close behavior;
- custom/mock transport dispatch;
- response wrapping;
- errors raised when a sync transport is used in an async context or vice versa.

### Track 4 acceptance criteria

- `issubclass`/`isinstance` checks match the reference for target transports.
- Existing WSGI/ASGI/MockTransport behavioral tests remain passing.
- No transport dispatch path is duplicated.

## Track 5 — Stream base classes and public type surface

### 5.1 Rebaseline exact HTTPX stream hierarchy

Audit:

- `ByteStream`;
- `SyncByteStream`;
- `AsyncByteStream`.

Record exact public bases and which classes implement:

- `__iter__`;
- `__aiter__`;
- `close`;
- `aclose`;
- context-manager behavior if present;
- constructor arguments.

### 5.2 Align the facade type shape

Make the public classes satisfy the same intended sync/async contracts without introducing a second response-stream implementation.

Compatibility request streams must still be consumable by the existing client/transport adapters.

### 5.3 Protect the accepted raw/decoded lifecycle

This track must not change the response body's already-accepted one-shot/lifecycle semantics.

Regression tests must keep coverage for:

- repeated stream consumption;
- `StreamConsumed`/`StreamClosed` behavior;
- partial iteration then close;
- sync and async raw iteration;
- decoded iteration;
- compressed native raw streams;
- underlying close exactly once.

These tests are guards, not a request to redesign streaming.

### 5.4 Request-body stream interoperability

Direct differential tests should cover passing supported custom `SyncByteStream`/`AsyncByteStream` implementations through Request/Client paths and ensuring the correct sync/async restrictions are enforced.

### Track 5 acceptance criteria

- Target stream MRO/type checks match HTTPX.
- Iteration and close methods exist on the correct public classes.
- Custom request streams remain functional.
- Existing response raw/decoded lifecycle tests pass unchanged.

## Track 6 — Oracle and ledger closure

### 6.1 Run focused signature/type tests

Include direct HTTPX comparisons for all Phase 3 targets.

### 6.2 Regenerate the candidate manifest

Run the existing API generator and comparator.

Expected outcome:

- Phase 3 target tuples disappear from active differences;
- no new unexplained differences are introduced by changing class bases/signatures.

### 6.3 Move resolved records

Remove resolved Phase 3 records from the active allowlist and preserve them in the resolved ledger per repository convention.

Do not bulk-delete records by family name without confirming every tuple now matches.

## Required validation

Mandatory before Phase 3 completion:

```sh
./scripts/check.sh
```

and the existing API-oracle commands.

Also run focused compatibility modules for:

- client/top-level helpers;
- transports/mounts/mock/WSGI/ASGI;
- stream/request/response lifecycle;
- any new signature-differential tests.

A full pinned compatibility suite is strongly preferred at the phase boundary because class-base changes can have broad effects, but Phase 6 remains the final qualification authority.

## Phase acceptance criteria

Phase 3 is complete only when:

- every Phase 1 difference assigned to Phase 3 is resolved or has a bounded blocker;
- public helper/client/transport signatures targeted by the plan match HTTPX 0.28.1;
- runtime argument validation matches the reference rather than only `inspect.signature` metadata;
- target transport inheritance relationships match;
- target stream public type surface matches;
- current custom/mock/WSGI/ASGI behavior remains functional;
- accepted response stream lifecycle semantics do not regress;
- `./scripts/check.sh` passes;
- API oracle has zero unexplained/stale entries after ledger cleanup;
- no advanced native transport, SOCKS, Trio/AnyIO, CI, or release work is pulled into this phase.

## Rejection criteria

Reject the implementation if:

- normal callables only receive a forged `__signature__` while runtime remains permissive/incompatible;
- exact wrappers duplicate request construction/dispatch logic per HTTP verb;
- advanced transport parameters are silently ignored;
- class-base changes break custom transport dispatch or close semantics;
- stream inheritance changes reopen or regress raw/decoded lifecycle behavior;
- private HTTPX classes are copied solely to achieve textual MRO identity;
- Phase 4/5 networking behavior is partially added without its required end-to-end tests;
- active allowlist records are removed without confirming their exact oracle tuples.

## Stop conditions

Stop the affected target and record a bounded blocker if:

- a reference signature depends on a private sentinel/type that cannot be represented without exposing private HTTPX implementation;
- exact signature shape would break an established non-compat EggFetch API outside `eggfetch.compat.httpx`;
- matching a stream base relationship requires replacing the native streaming bridge;
- changing a transport base would make sync/async dispatch ambiguous in a way the current architecture cannot safely represent.

Continue independent Phase 3 targets where possible.

## Suggested commit decomposition

1. `test: pin remaining HTTPX signature and type differences`
2. `fix: align HTTPX helper and client signatures`
3. `fix: align HTTPX transport signatures and inheritance`
4. `fix: align HTTPX stream type surface`
5. `test: resolve signature and type allowlist entries`

## Handoff checklist

Report:

- starting SHA;
- final Phase 3 executable SHA;
- exact symbols/difference IDs resolved;
- any blocked/retained signature/type differences;
- reference signature inventory source;
- focused differential results;
- stream lifecycle regression results;
- `./scripts/check.sh` result;
- full pinned suite result if run;
- API oracle before/after allowed counts;
- confirmation that advanced transport behavior and SOCKS remain assigned to Phases 4–5;
- confirmation that CI/release architecture was unchanged.
