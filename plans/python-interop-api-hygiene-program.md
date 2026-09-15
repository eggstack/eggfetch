# Python Interop and API Hygiene Program

Status: **implementation complete; final qualification pending**
Date: 2026-09-15  
Target repository: `eggstack/eggfetch`

## Objective

Tighten eggfetch's native Python binding boundary without reshaping the Rust HTTP engine around Python semantics.

The program addresses five related concerns that have accumulated as the Python surface matured:

1. package/version/public-export correctness;
2. native↔compat layering and duplicated sync/async normalization;
3. request-body interoperability for Python asynchronous iterables;
4. PyO3 / supported-Python modernization; and
5. first-class PEP 561 typing and mechanical API drift checks.

The expected result is a Python package whose supported public API is explicit, typed, package-correct, and structurally difficult to drift across sync/async adapters or compatibility facades.

## Baseline

Current `main` is the documentation/profile descendant of qualified executable SHA `97e87e42c8f4d5659739e7b23ff9005a4ec1ae53`.

The current native Python implementation has the following confirmed issues:

- `_native.__version__` is hard-coded to `0.1.0`, while the Python package and Rust Python crate are version `0.1.4`.
- wheel smoke validation checks only that `__version__` is a non-empty string, so release-version drift is not detected.
- `_native.__all__`, top-level `eggfetch.__all__`, and the documented exception/network-stream surface are not identical and there is no authoritative native public-API manifest.
- `build_request_body()` / `is_python_iterable()` classify `__aiter__` as streamable, while `python_iterable_to_request_body()` requires synchronous `try_iter()`.
- `Client` and `AsyncClient` duplicate most client-construction and request-normalization semantics.
- the native TLS binding imports private code from `eggfetch.compat.httpx._ssl_context`, reversing the documented `compat -> native -> core` dependency direction.
- no `.pyi` native stubs or `py.typed` marker are packaged.
- the binding stack remains on PyO3 / `pyo3-async-runtimes` 0.23 and the release matrix stops at Python 3.13.
- `PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1` is set globally in release validation despite the package not intentionally publishing an abi3 compatibility contract.

## Architectural Invariants

This program must preserve the following boundaries:

```text
Python application / compatibility facade
              |
              v
      eggfetch-python binding
              |
              v
        eggfetch-core
```

`eggfetch-core` remains language-neutral. Python-specific argument unions, asyncio integration, PEP 561 typing, `ssl.SSLContext` introspection, and compatibility-facade semantics belong in `eggfetch-python` or the pure-Python package.

No plan may introduce HTTPX objects, PyO3 types, Python exceptions, or Python runtime assumptions into `eggfetch-core` merely to reduce binding code.

## Program Decisions

### Public API

- `eggfetch.__all__` becomes the authoritative supported native Python package surface.
- `eggfetch._native` remains an implementation module and does not receive a separate compatibility promise.
- exceptions documented as part of the native hierarchy must be intentionally exported or explicitly documented as private; accidental mismatch is not acceptable.
- objects users can receive from public APIs, especially network-stream wrappers, must have an explicit export/typing decision.

### Version ownership

The Python package version must have one mechanically consistent release value. `_native.__version__` should derive from build/package metadata (prefer `env!("CARGO_PKG_VERSION")` given the crate/package release lockstep) and wheel validation must compare runtime version to installed distribution metadata.

### SSL interop ownership

Generic `ssl.SSLContext` snapshotting, representability classification, mutation detection, and registry/provenance handling must move out of the versioned HTTPX compatibility implementation and into a neutral eggfetch Python interop module.

HTTPX-specific helper defaults may remain in the compatibility facade, but native `eggfetch.Client` / `AsyncClient` may not import `eggfetch.compat.httpx.*` to function.

### Request bodies

- sync `Client` and top-level synchronous helpers support lazy synchronous iterable content;
- async `AsyncClient` supports both lazy synchronous iterable content and lazy asynchronous iterable content;
- sync APIs reject async-only iterables early with a clear `TypeError`;
- async iteration remains demand-driven by Rust body polling and must not be eagerly buffered.

### Sync/async code sharing

Share Python-argument normalization and Rust builder preparation, not execution/runtime ownership.

Sync code retains its explicit Tokio runtime, GIL-release behavior, stream/runtime leases, and blocking dispatch. Async code retains `pyo3-async-runtimes` dispatch and asyncio cancellation behavior.

### Typing

Ship PEP 561-compatible in-package typing for supported public Python surfaces. Private underscore implementation modules do not need a compatibility promise merely because `py.typed` is present.

### PyO3 / Python support

Upgrade PyO3 and `pyo3-async-runtimes` together to the current mutually compatible release line selected at implementation time (research baseline: 0.29.x). Add Python 3.14 to the supported release matrix after behavior/package qualification. Treat Python 3.15 pre-release qualification as forward evidence only until final release/policy update.

Free-threaded Python and abi3/abi3t publishing are explicitly separate projects.

## Work Breakdown

Execute in this order:

1. `python-public-api-package-correctness.md`
   - fix version ownership and release assertions;
   - define authoritative exports;
   - add a native API manifest/oracle;
   - reconcile exception/network-stream exposure.

2. `python-interop-boundary-and-async-body.md`
   - remove native dependency on `compat.httpx._ssl_context`;
   - establish a neutral SSL interop module;
   - add a real AsyncIterable request-body bridge;
   - reject async-only bodies in sync APIs;
   - consolidate shared sync/async client/request normalization.

3. `python-pyo3-python-version-modernization.md`
   - upgrade PyO3 + async runtime bridge coherently;
   - qualify Python 3.14 and current supported versions;
   - remove obsolete forward-compatibility escape hatches/dead build dependencies where proven safe.

4. `python-pep561-typing-surface.md`
   - add `py.typed` and reviewed stubs;
   - model native public APIs and intended public compatibility entry points;
   - add stub/runtime/package consumer validation.

5. `post-python-interop-api-qualification-and-closure.md`
   - freeze one executable SHA after all implementation work;
   - run native Python, package, type, version-matrix, HTTPX/HTTPX2 API and behavioral qualification;
   - renew compatibility profiles/ledger once rather than after every child plan.

## Explicit Non-Goals

- no Rust-core redesign;
- no HTTP protocol feature campaign;
- no change to HTTPX parity merely to simplify native bindings;
- no Trio/AnyIO backend project;
- no coroutine trace-callback project unless required by a discovered regression;
- no free-threaded Python qualification in this program;
- no abi3 or abi3t distribution migration;
- no Edition/MSRV work beyond preserving the completed Rust 1.89 policy;
- no broad dependency upgrade beyond the PyO3/runtime bridge and narrowly required compatibility corrections.

## Qualification Policy

Any executable binding, PyO3 dependency, packaged Python surface, or compatibility-facade bridge change invalidates the current exact executable qualification binding for the affected Python compatibility claims.

Do not repeatedly rebind profiles after each child plan. Implement the complete bounded program, stabilize it, then freeze one SHA and run the final closure qualification once.

Documentation-only descendants are permitted after that freeze only after confirming they do not alter executable/package inputs.

## Program Acceptance Criteria

- [x] runtime `eggfetch.__version__` equals installed distribution metadata and release metadata.
- [x] version drift is mechanically detected by wheel/package validation.
- [x] `eggfetch.__all__` is the documented authoritative public native Python surface.
- [x] a native API oracle prevents accidental root export/signature/exception-hierarchy drift.
- [x] generic SSLContext interop no longer lives behind or requires `eggfetch.compat.httpx.*`.
- [x] `eggfetch` native operation does not import a versioned compatibility facade as an implementation prerequisite.
- [x] sync APIs reject async-only body iterables before dispatch.
- [x] `AsyncClient` lazily consumes `AsyncIterable[bytes | str]` with cancellation/error propagation and transport backpressure.
- [x] sync and async bindings share one semantic normalization/preparation layer for duplicated client/request policy.
- [x] runtime-specific sync and async execution/lifecycle behavior remains separate.
- [x] supported Python package ships `py.typed` and reviewed stubs.
- [x] wheel validation proves stubs/marker are present.
- [x] runtime/stub drift and representative consumer type checking are gated.
- [x] PyO3 and `pyo3-async-runtimes` are upgraded coherently.
- [x] Python 3.14 is included in package metadata and the release matrix; final published-package qualification remains pending.
- [x] current supported Python versions have local native behavior coverage on the available interpreters.
- [x] obsolete `PYO3_USE_ABI3_FORWARD_COMPATIBILITY` usage is removed from ordinary release validation.
- [x] dead direct `pyo3-build-config` dependency is removed.
- [x] Rust 1.89 MSRV policy remains intact.
- [x] Tier 1, Tier 2, package validation, typing gates, and release-matrix smoke tests pass.
- [ ] HTTPX 0.28.1 and HTTPX2 2.12.0 API oracles and full pinned compatibility suites pass on the final exact SHA.
- [ ] compatibility profiles and live ledger bind to the final qualified SHA.
