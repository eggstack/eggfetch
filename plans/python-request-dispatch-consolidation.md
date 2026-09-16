# Python Request Dispatch Consolidation

Planning baseline: `025b1a5a6a94b017b3f3b183c3d562e7ee9bcca7` (`main`, 2026-09-15; eggfetch 0.1.4)
Parent program: `plans/post-audit-maintenance-security-and-proxy-modernization-program.md`
Reference compatibility contracts: `httpx==0.28.1`, `httpx2==2.12.0`

## Objective

Remove the remaining runtime-independent request-construction duplication between the Python sync client, async client, and top-level request helpers while preserving every existing Python/HTTPX semantic distinction.

The target is one authoritative transformation from normalized Python request state to an `eggfetch_core::RequestBuilder`. Runtime ownership, GIL release, coroutine conversion, callback-error propagation, and Python response wrapping remain adapter-specific.

This plan is a maintenance refactor, not a Python API redesign.

## Current structure

`crates/eggfetch-python/src/request_preparation.rs` already does the hard first half correctly: it converts Python arguments into owned Rust state through `PreparedClientConfig` and `PreparedRequest`. Sync and async callers therefore share parsing of:

- methods/URLs/query parameters;
- headers;
- buffered/sync/async bodies;
- form/JSON/multipart;
- cookies;
- timeout;
- auth;
- proxy settings and proxy extras;
- retries;
- HTTPX extension hints;
- redirect overrides.

After this normalization, however, multiple callers independently repeat the second half:

- call `client.request(method, url)`;
- apply headers;
- apply body;
- apply timeout;
- apply decompression override;
- resolve `AuthOverride` into inherit/disable/override builder calls;
- resolve `ProxyOverride` and construct `eggfetch_core::Proxy` with proxy headers/TLS config;
- apply per-request redirect policy overrides;
- apply retry policy;
- apply transport hints/extensions;
- send;
- inspect trace callback errors;
- consume/wrap the response.

The sync and async clients are especially close. The top-level `request()`/verb helpers have a short-lived-client path with some legitimate differences, but they still reproduce part of the same builder application logic.

This duplication is risky because future request fields can land in one path and be silently omitted in another even though argument preparation itself is shared.

## Design requirements

### Keep two boundaries distinct

The refactor should establish two explicit boundaries rather than one giant shared helper.

1. **Python normalization boundary** — existing `prepare_request()` / `prepare_client_config()` code, which may hold the GIL and understands Python values.
2. **Core dispatch preparation boundary** — new runtime-independent Rust code that consumes owned `PreparedRequest` state and configures `eggfetch_core::RequestBuilder` consistently.

The actual network-driving boundary remains different for sync and async:

- sync obtains its owned runtime guard/handle and runs with the GIL detached;
- async returns a Python awaitable through `pyo3_async_runtimes`;
- top-level sync helper may construct a short-lived runtime/client;
- response conversion may need a runtime lease/handle depending on surface.

Do not hide these runtime semantics inside a generic abstraction.

## Required implementation

# 1. Inventory semantic differences before refactoring

Build a compact source-backed matrix for these dispatch paths:

- `PyClient::request`;
- `PyAsyncClient::request`;
- top-level `eggfetch.request` and verb helpers;
- streaming equivalents where they apply;
- HTTPX/httpx2 facade call paths that eventually enter these methods.

For each field in `PreparedRequest`, record whether each path:

- applies it directly;
- transforms it;
- intentionally ignores it;
- rejects it earlier;
- handles it at client construction rather than request construction.

At minimum cover:

- method;
- URL;
- headers;
- body;
- timeout;
- auth inherit/disable/override;
- proxy inherit/disable/override;
- proxy-only headers;
- proxy TLS config;
- retry;
- redirect follow/max overrides;
- decompression;
- transport hints/extensions;
- trace callback error slot.

Do not assume two visually similar branches are semantically identical. In particular, top-level helper behavior can differ because its client is short-lived and constructed from request-level `verify`, `cert`, `limits`, etc.

Acceptance:

- [x] Every `PreparedRequest` field has an explicit owner in each dispatch path.
- [x] Intentional differences are identified before code is moved.

# 2. Introduce one core-builder application primitive

Create a crate-private helper/type in the Python binding layer whose only job is to transform owned normalized request state into a configured core `RequestBuilder`.

A reasonable shape is conceptually:

```rust
struct PreparedDispatch {
    builder: eggfetch_core::RequestBuilder,
    trace_error_slot: Option<...>,
}

fn prepare_core_dispatch(
    client: &eggfetch_core::Client,
    request: PreparedRequest,
    inherited: RequestDispatchDefaults,
) -> PyResult<PreparedDispatch>;
```

Exact names/types are implementation choices. The important properties are:

- the helper is runtime-neutral;
- it takes owned values so no Python borrow crosses an async/GIL boundary;
- it applies all semantically common `RequestBuilder` settings once;
- it does not perform network I/O;
- it does not call `block_on`;
- it does not create Python awaitables;
- it does not wrap `Response` into Python classes;
- it preserves the trace callback slot for post-send inspection rather than swallowing it.

If top-level requests require an intentionally different default (for example auth-disable behavior on a client with no inherited auth), represent that as a typed input/default policy rather than forking another manual builder sequence.

Avoid an over-generic abstraction. This helper exists for Python adapter state application, not as a new core public API.

# 3. Make request state exhaustiveness visible

The refactor should make future additions difficult to omit silently.

Prefer consuming/destructuring `PreparedRequest` in one authoritative location. If a new field is later added to `PreparedRequest`, compilation should force a decision about how it maps to core dispatch.

Do not replace explicit ownership with `..` destructuring that would recreate the same drift risk.

Where a field is intentionally runtime-specific, have the shared transformation return it explicitly rather than letting it vanish.

Acceptance:

- [x] Adding a new non-defaulted `PreparedRequest` field requires touching the shared dispatch transformation or an explicitly documented runtime-specific boundary.
- [x] Sync and async request methods no longer contain parallel manual application of the same request settings.

# 4. Preserve auth and proxy three-state semantics

The consolidation must preserve the distinction between:

- inherit;
- explicitly disable;
- override.

For auth, `without_auth()` must remain distinct from simply doing nothing when a client can carry default auth.

For proxies, `without_proxy()` must remain distinct from inherit, including environment proxy behavior. An override must continue to use the compatibility proxy URL normalization and attach proxy-only headers/TLS configuration without leaking those values into ordinary origin headers.

Do not collapse these states into `Option<T>`.

Add focused tests that would fail if inherit/disable are conflated after consolidation.

# 5. Preserve redirect override semantics

Current request-level `follow_redirects` and `max_redirects` behavior updates only the specified parts of a default policy.

The shared helper must preserve this partial-override behavior. It must not replace unspecified request settings with hard-coded defaults if the client carries a different configured value.

Test independently:

- only `follow_redirects` overridden;
- only `max_redirects` overridden;
- both overridden;
- neither overridden;
- client defaults retained.

# 6. Preserve trace callback error precedence

The current sync/async paths record Python trace callback failures in a slot and surface them after transport dispatch, including cases where transport also fails.

Do not move Python exception objects into core or make trace callback handling a generic transport concern.

The shared preparation primitive may carry the slot forward, but sync/async runtime-specific code remains responsible for checking it at the same semantic point as today.

Regression coverage must prove callback exceptions are not swallowed and their precedence relative to transport errors remains unchanged.

# 7. Keep streaming and body ownership correct

`PreparedRequest` can contain buffered, sync-iterable, or async-iterable bodies. The async-body bridge is already a qualified ownership boundary.

The consolidation must not:

- clone one-shot streams;
- convert streaming bodies to eager buffers;
- hold the GIL during Rust network I/O;
- cause Python iterators/generators to outlive their required runtime ownership incorrectly;
- change replayability classification used by redirects/retries.

If streaming methods use a separate response-consumption path but the same request-builder application, share only the builder phase.

# 8. Consolidate top-level helpers where semantically valid

The top-level `request()` helper builds a short-lived client because it accepts request-level TLS/limit-style configuration that persistent client request methods reject.

Retain that construction model. After the short-lived client exists and Python inputs have been normalized, route the request through the same builder-application primitive where semantics match.

If any top-level behavior must remain different, encode the difference explicitly and add a test. Do not preserve duplicate code solely because it already exists.

# 9. Keep the compatibility facades thin

Do not move HTTPX-specific objects or policy into the new helper. HTTPX/httpx2 compatibility should continue to normalize its documented public surface into the native Python request contract, which then enters the shared Rust adapter path.

This plan must not create a second compatibility dispatch implementation.

# 10. Focused regression matrix

Add/retain focused tests for sync and async parity across:

- raw headers and duplicate headers;
- params/query encoding;
- buffered request body;
- sync iterable request body;
- async iterable request body;
- JSON/form/multipart;
- timeout override;
- decompression override;
- cookies;
- auth inherit/disable/override;
- proxy inherit/disable/override;
- proxy headers/TLS extras;
- redirect partial overrides;
- retry override;
- transport extensions (`target`, SNI/static routing, trace where supported);
- ordinary buffered response;
- streaming response;
- cancellation/close;
- trace callback failure.

Prefer table-driven or shared tests where practical so the test suite itself does not recreate large sync/async duplication.

## Code-quality acceptance criteria

- [x] Request normalization remains separate from network dispatch.
- [x] One crate-private transformation applies common `PreparedRequest` state to `eggfetch_core::RequestBuilder`.
- [x] Sync and async methods contain only runtime-specific dispatch/response mechanics plus calls to the shared preparation code.
- [x] Top-level helpers share the common mapping wherever semantically valid.
- [x] No new public Rust or Python type is introduced solely for the refactor.
- [x] No core HTTP policy is duplicated in the Python crate.
- [x] Existing explicit `#[allow(clippy::too_many_arguments)]` boundaries are reduced where the new owned configuration naturally permits it, but no abstraction is added solely to silence Clippy.

## Required validation

Run throughout implementation:

```sh
./scripts/check.sh
```

Before closure, also run the directly affected Python tests and compatibility kernels, including sync/async streaming and trace tests.

Because this is executable compatibility-sensitive work, the parent program's final exact-SHA requalification will renew the full HTTPX/HTTPX2 evidence. Do not independently claim Stage C renewal from this plan alone.

If the refactor changes exported symbols, signatures, stubs, or package contents unexpectedly, treat that as a defect unless explicitly justified and update the native API/typing oracle tests before proceeding.

## Non-goals

- no Python public API expansion;
- no new HTTPX version;
- no Trio/AnyIO runtime support;
- no rewrite of PyO3 async bridging;
- no core `RequestBuilder` redesign unless a concrete core defect blocks consolidation;
- no behavior normalization based only on what looks aesthetically cleaner;
- no response-class redesign;
- no Node/FFI changes except those already owned by the separate adapter feature plan.

## Exit criteria

This plan is complete when normalized Python request state has one authoritative runtime-independent mapping into the core request builder, all intentional surface differences remain explicit and tested, Tier 1 remains green, and the change is ready for the parent program's final exact-SHA compatibility qualification.

## Closure record — complete (2026-09-16)

`prepare_core_dispatch()` is now the single crate-private mapping from owned
`PreparedRequest` state to the core builder, shared by sync, async, streaming,
and top-level paths. Runtime ownership, response adaptation, and trace callback
handling remain at their respective boundaries. The compatibility-only proxy URL
conversion correction preserves HTTPX credential semantics while native raw
proxy parsing remains fail closed. Native Python tests passed 560/560 and the
final exact-SHA compatibility qualification passed 1,870/1,870 in each run.
