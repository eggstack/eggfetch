# Python Interop Boundary and Async Body Pass

Status: **implemented; final qualification pending**
Parent: `python-interop-api-hygiene-program.md`  
Date: 2026-09-15

## Scope

Correct the Python binding's internal dependency direction, add a real asynchronous request-body bridge, and consolidate duplicated sync/async argument normalization without merging their runtime/lifecycle models.

This plan owns:

- neutral `ssl.SSLContext` interop placement;
- removal of native runtime dependency on versioned HTTPX compatibility internals;
- lazy `AsyncIterable[bytes | str]` request bodies for `AsyncClient`;
- explicit sync rejection of async-only body iterables; and
- shared client/request preparation used by both `Client` and `AsyncClient`.

## Baseline Findings

### Native TLS depends on compatibility internals

`crates/eggfetch-python/src/tls.rs` imports:

```text
eggfetch.compat.httpx._ssl_context
```

to snapshot/classify `ssl.SSLContext`, inspect the registry, and translate representable state into `eggfetch_core::TlsConfig`.

That reverses the documented dependency direction. Generic Python↔rustls representability is a binding concern, not an HTTPX-0.28.1-specific concern.

### Async iterable misclassification

`build_request_body()` and `is_python_iterable()` accept an object with either `__iter__` or `__aiter__`, while `python_iterable_to_request_body()` calls `try_iter()` and therefore supports only synchronous iteration.

Both sync and async clients call this same synchronous converter. Async-only generators are therefore classified as streamable and then fail after entering the streaming path.

### Sync/async semantic duplication

`client.rs` and `async_client.rs` independently implement most of the same policy:

- TLS construction;
- HTTP version flags;
- limits;
- default headers;
- timeout;
- redirects;
- cookie initialization;
- auth;
- explicit proxy and proxy extras;
- environment proxy / NO_PROXY;
- retry;
- local address;
- socket options;
- UDS;
- request URL/params/header/body/cookie parsing;
- request-local auth/proxy/retry/redirect/decompression/extensions application.

The actual network engine remains shared in core, but the Python argument-to-core mapping has become a drift surface.

## Architectural Target

### Neutral interop layer

Use a layout materially equivalent to:

```text
eggfetch/
    _ssl_context.py        # or _interop/ssl_context.py
    compat/
        httpx/
        httpx2/
```

The neutral module owns only generic binding semantics:

- snapshot of public `ssl.SSLContext` state;
- bounded CA extraction;
- representability classification for eggfetch/rustls;
- fingerprint/mutation detection;
- provenance registry for helper-created contexts when needed by translation;
- safe translation metadata used by native PyO3.

HTTPX-specific helper defaults and compatibility APIs remain inside the facade.

Native Rust may import the neutral `eggfetch._ssl_context` module, but must not require `eggfetch.compat.httpx` or `eggfetch.compat.httpx2` for ordinary native API operation.

### Shared binding preparation

Create internal Rust structures/functions materially equivalent to:

```rust
struct PreparedPyClientConfig { ... }
struct PreparedPyRequest { ... }
```

or a smaller set of dedicated helpers if that keeps ownership/lifetimes clearer.

The preparation layer should consume Python arguments while the GIL is held and produce owned Rust/core-ready values.

Sync/async execution then consumes the same prepared semantics through separate dispatchers.

Do **not** create an abstraction that hides runtime ownership or forces sync and async stream lifetimes into the same executor path.

## Part A — SSLContext Boundary Migration

### Move generic machinery

Move/copy generic `_ssl_context.py` mechanisms to the neutral eggfetch package module.

Preserve current safety properties:

- no OpenSSL pointer extraction;
- no private-key extraction;
- bounded CA count/bytes;
- fail-closed handling for unrepresentable external contexts;
- helper-created context provenance;
- post-construction mutation detection;
- TLS version bounds restricted to values core can represent;
- custom ciphers/ALPN/client-cert state that cannot be represented must not be silently discarded.

### Compatibility shim

During migration, retain `eggfetch.compat.httpx._ssl_context` as a thin re-export/import shim if compatibility tests or internal imports require the historical private location.

The shim must not own divergent logic. There should be one implementation of snapshot/classification/registry behavior.

Audit HTTPX2 imports as well.

### Native TLS import

Update Rust binding `tls.rs` to import the neutral module.

Also correct the accepted-input error message so it does not claim support for a generic "file-like object" unless such handling is actually implemented and tested.

### Isolation test

Add an import/runtime test proving native `eggfetch.Client(verify=<SSLContext>)` works when compatibility modules are not imported and without any dependency on version-specific facade state.

A test that monkeypatches/removes `eggfetch.compat.httpx._ssl_context` while exercising neutral SSL translation is appropriate if it can be made deterministic without corrupting package imports.

## Part B — Request Body Classification

Replace ambiguous `is_python_iterable()` behavior with explicit classification, for example:

```rust
enum PythonBodyKind {
    Buffered(...),
    SyncIterable(...),
    AsyncIterable(...),
}
```

or separate predicates/helpers.

Required semantics:

- `bytes`, `bytearray`, `memoryview`, `str` -> buffered content;
- mapping-like `content=` values remain rejected;
- object with valid sync iteration -> sync streaming body;
- object with only async iteration -> async streaming body for `AsyncClient`;
- async-only body passed to sync `Client`/top-level sync helper -> immediate clear `TypeError` before network dispatch;
- invalid object -> immediate `TypeError`.

Avoid double-probing Python iterators in ways that consume one-shot state before dispatch.

## Part C — Lazy AsyncIterable Bridge

Implement an async Python body adapter that is demand-driven by Rust body polling.

Required behavior:

1. obtain/store the Python async iterator once;
2. when Rust body polling requests another item, enter the appropriate Python event-loop/task context;
3. call `__anext__()`;
4. convert the returned awaitable to a Rust future via the supported `pyo3-async-runtimes` bridge;
5. await exactly one produced item;
6. translate `StopAsyncIteration` to EOF;
7. accept bytes-like values or `str` according to existing sync-body behavior;
8. reject other chunk types with a stable Python-facing body error;
9. propagate Python producer exceptions through the request future;
10. avoid prefetching the next chunk before transport demand;
11. preserve cancellation: cancelling the request must stop further pulls;
12. close/finalize the generator on early drop where safely possible.

Do not buffer the async iterable into a `Vec` or Python list.

### Runtime-context rule

Use the async runtime bridge's supported task-local/event-loop propagation APIs rather than inventing a second Tokio runtime or calling Python awaitables from `spawn_blocking`.

The body must remain bound to the same Python async execution context as the request.

## Part D — Shared Client Configuration Preparation

Extract duplicated constructor semantics from `PyClient::new` and `PyAsyncClient::new`.

At minimum share:

- `verify_disabled` derivation;
- TLS config construction;
- HTTP1/HTTP2/HTTP3 policy resolution;
- limits conversion;
- default headers;
- timeout;
- redirect policy;
- cookie jar initialization;
- auth override;
- explicit proxy + proxy headers/TLS extras;
- `trust_env` proxy/NO_PROXY handling;
- retry policy;
- local address;
- socket options;
- UDS;
- resulting core `ClientBuilder` preparation.

Preferred interface:

```rust
fn prepare_client_config(py: Python<'_>, args: ...) -> PyResult<PreparedPyClientConfig>
fn apply_client_config(config: PreparedPyClientConfig) -> PyResult<eggfetch_core::Client>
```

Exact type shape is implementation-dependent; prioritize ownership clarity over minimizing lines.

Sync-only runtime creation stays in `client.rs`.

## Part E — Shared Request Preparation

Extract request-local duplicated semantics before dispatch:

- method validation;
- URL parsing;
- query params;
- body-kwarg validation;
- headers;
- multipart/body classification;
- automatic content type;
- request cookies;
- timeout;
- auth override;
- proxy override/extras;
- retry override;
- redirect override;
- decompression;
- extensions/transport hints/trace bridge;
- client-level-only `verify`/`cert` rejection.

The shared prepared request should contain owned values suitable for either execution adapter.

Body representation may require an enum so sync and async streams remain distinct until execution.

## Drift Tests

Add tests that exercise the same semantic inputs against both client types and assert equivalent results/errors where the APIs intentionally agree.

Useful table-driven cases:

- protocol flag combinations;
- invalid local address;
- socket-option validation;
- proxy normalization;
- env NO_PROXY parse errors;
- auth disable/override;
- timeout types;
- conflicting body kwargs;
- mapping passed as content;
- redirect overrides;
- decompression override;
- request extensions.

The point is to make future sync/async divergence visible.

## Async Body Test Matrix

At minimum cover:

- async generator yielding multiple bytes chunks;
- async generator yielding `str` chunks;
- mixed bytes/str if current sync semantics allow both;
- empty async generator;
- delayed generator proving no eager full buffering;
- generator exception after one or more chunks;
- invalid chunk type;
- cancellation while producer is awaiting;
- cancellation after at least one chunk;
- server stops reading / request future is dropped;
- sync generator through `AsyncClient` remains supported;
- async-only generator through sync `Client` rejected before dispatch;
- top-level synchronous helper rejects async-only generator;
- retry/replay semantics remain correct: non-replayable streaming bodies may not be silently replayed.

Where exact byte production timing matters, use a local deterministic server and synchronization events rather than sleeps alone.

## HTTPX Compatibility Constraints

The HTTPX 0.28.1 and HTTPX2 facades already own compatibility rules for request streams. The native refactor must not broaden/narrow facade behavior accidentally.

After moving SSL helpers, facade imports should resolve through the neutral implementation or the compatibility shim without behavioral change.

Do not delete the shim until all versioned facade tests/import paths have been audited and the final closure plan explicitly permits it.

## Validation

Run at minimum:

```text
./scripts/check.sh
./scripts/check.sh extended
```

plus focused native Python async-body, SSLContext translation, TLS network-proof, proxy/TLS, close/cancellation, and HTTPX/HTTPX2 compatibility tests touched by the migration.

Exact-SHA compatibility profile rebinding remains deferred to final closure.

## Acceptance Criteria

- [x] native `tls.rs` no longer imports `eggfetch.compat.httpx.*` for generic SSLContext translation.
- [x] generic SSL snapshot/classification/registry logic has one implementation in a neutral eggfetch interop module.
- [x] compatibility facades reuse that implementation or a thin non-divergent shim.
- [x] current fail-closed SSL representability/mutation/provenance behavior is preserved.
- [x] native TLS input error messages match actual accepted types.
- [x] body classification distinguishes buffered, sync iterable, and async iterable content without consuming one-shot streams early.
- [x] sync APIs reject async-only iterables with deterministic `TypeError` before network dispatch.
- [x] `AsyncClient` lazily consumes async iterables under transport backpressure.
- [x] async producer exceptions propagate correctly.
- [x] request cancellation prevents further generator pulls and does not poison the client.
- [x] non-replayable body retry/redirect safety remains intact.
- [x] sync/async client construction shares one semantic preparation path.
- [x] sync/async request normalization shares one semantic preparation path.
- [x] runtime/lifecycle execution remains separately implemented for sync and async adapters.
- [x] no Python-specific type or runtime assumption is introduced into `eggfetch-core`.
- [x] Tier 1 and Tier 2 pass after implementation.

## Handoff

Once stable, continue with `python-pyo3-python-version-modernization.md`. Do not renew frozen compatibility profiles yet.
