# Python PEP 561 Typing Surface

Status: **ready after `python-pyo3-python-version-modernization.md`**  
Parent: `python-interop-api-hygiene-program.md`  
Date: 2026-09-15

## Scope

Ship a first-class typed Python package contract for eggfetch without making private implementation modules part of the supported API.

This plan owns:

- `py.typed` packaging;
- reviewed `.pyi` stubs for the native extension/public package;
- typing coverage for intentionally public compatibility entry points;
- runtime↔stub drift validation; and
- representative downstream type-check fixtures.

It does not own broader runtime behavior, HTTPX behavioral parity, or free-threaded/abi3 packaging.

## Baseline

The Python package currently contains no `py.typed` marker and no `.pyi` files.

The public native API is implemented primarily by the PyO3 extension and re-exported from `eggfetch/__init__.py`. Runtime signatures alone do not give static type checkers a complete contract for extension-defined classes, overloaded request-body inputs, async return types, context managers, exception types, or streaming iterators.

The package also contains large `eggfetch.compat.httpx` and `eggfetch.compat.httpx2` trees. Most implementation modules are underscore-prefixed/private, so PEP 561 does not require those private internals to become a compatibility promise merely because `py.typed` is added.

## Design Decisions

### 1. Use checked in-package stubs

Add materially equivalent files:

```text
crates/eggfetch-python/python/eggfetch/
    py.typed
    __init__.pyi
    _native.pyi
```

Add stubs or typed public modules for compatibility-package entry points only where necessary to make intentionally public exports type-check correctly.

Do not create stubs for every underscore-private implementation module unless a type checker requires a specific import path to resolve a public symbol.

### 2. Manual/reviewed stubs are authoritative initially

Do not make PyO3 automatic stub generation the sole source of truth in this program. The binding uses function-style `#[pymodule]` registration and has substantial dynamic exception/module registration; reviewed stubs are lower risk and easier to align with the explicit public API oracle.

Automatic generation/introspection may be used as a drift aid if supported by the selected PyO3 release, but it must not silently replace the reviewed API contract.

### 3. Share API source-of-truth with runtime oracle

The typing surface should correspond to the public API decisions from `python-public-api-package-correctness.md`.

Where practical, avoid maintaining three unrelated lists for:

- `eggfetch.__all__`;
- native API manifest/oracle;
- stub exports.

A small generation/check script may compare them mechanically without generating the entire stub file.

## Native Types to Model

At minimum cover the supported public native surface, including:

- `Client`;
- `AsyncClient`;
- `Response`;
- `StreamingResponse`;
- sync and async streaming iterator classes;
- `Headers`;
- `Cookies` / `Cookie`;
- `Timeout`;
- `Limits`;
- `Retry`;
- `BasicAuth`, `BearerAuth`, `NoAuth`, `NOAUTH`;
- `File`;
- top-level request helpers;
- public exception hierarchy;
- `NetworkStream` / `AsyncNetworkStream` if the preceding API plan makes them public.

Use protocols/type aliases for reusable argument contracts where that improves readability and avoids enormous repeated unions.

## Input Type Contracts

Model real accepted inputs rather than replacing them wholesale with `Any`.

Recommended aliases/protocols include concepts equivalent to:

```python
HeaderTypes = Mapping[str, str] | Sequence[tuple[str, str]]
QueryParamTypes = Mapping[str, str] | Sequence[tuple[str, str]]
SyncByteStream = Iterable[bytes | bytearray | memoryview | str]
AsyncByteStream = AsyncIterable[bytes | bytearray | memoryview | str]
```

Refine exact aliases to match implementation behavior discovered during the preceding interop pass.

Type contracts should distinguish sync and async bodies:

- sync `Client` / top-level helpers: buffered content or synchronous iterable stream;
- `AsyncClient`: buffered content, synchronous iterable stream, or asynchronous iterable stream.

Do not type an unsupported input merely because HTTPX accepts it.

### Timeout/auth/proxy/cert

Represent the actual native accepted forms, including explicit `None` where meaningful. Avoid implying arbitrary `ssl.SSLContext`/file-like support beyond what the neutral SSL interop layer truly accepts.

### JSON

`json=` input may remain broadly typed (`Any`) because it is serialized through Python JSON semantics. `Response.json()` may return `Any`.

## Return Types and Context Managers

Ensure stubs correctly express:

- sync methods return `Response` or `StreamingResponse`;
- async methods return awaitables/coroutines yielding those types;
- sync client/context manager `__enter__` returns `Client`;
- async `__aenter__` returns/awaits `AsyncClient` according to actual runtime behavior;
- streaming context managers expose the correct stream object;
- iterator methods return the correct sync or async iterator element type;
- `raise_for_status()` return contract;
- `close`/`aclose` semantics;
- network stream `read`/`write`/`start_tls` types.

The stub should match runtime, not idealized HTTPX signatures.

## Exception Typing

Mirror the actual public exception inheritance exactly.

Add a test/oracle that compares the public stub exception names against the runtime public exception names so new exceptions cannot be added in Rust without a typing decision.

## Compatibility Entry Points

Because `py.typed` applies recursively, audit public imports from:

- `eggfetch.compat.httpx`;
- `eggfetch.compat.httpx2`;
- intentionally public SSE/WebSocket modules under HTTPX2.

Prefer one of these approaches per package:

1. inline annotations in existing public pure-Python entry modules, relying on private underscore modules as implementation details; or
2. concise package-level `.pyi` files that re-export/declare public objects.

Do not attempt a full annotation campaign across the large private compatibility implementation as a prerequisite for this pass.

The goal is useful/public typing, not a zero-`Any` score inside private compatibility code.

## Packaging

Ensure maturin includes:

- `eggfetch/py.typed`;
- native/public `.pyi` files;
- any compatibility package stubs chosen above.

The current package-content validator already allows `.pyi` entries; extend validation to require the marker and required stub files rather than merely permitting them.

Add isolated wheel assertions such as:

```python
import importlib.resources
assert importlib.resources.files("eggfetch").joinpath("py.typed").is_file()
```

and inspect the wheel archive directly in package validation.

## Type Checker Selection

Use one primary type checker in repository validation, chosen explicitly and pinned/managed in the validation environment.

`mypy` or Pyright are both acceptable. Prefer the checker that fits existing project tooling with the least dependency/runtime burden.

Do not require every major type checker to run in routine CI.

Use a second checker manually during initial qualification if useful to detect stub portability problems.

## Stub Drift Validation

Add at least two levels of checking:

### Runtime/stub structural check

Use `stubtest` or an equivalent purpose-built script to compare extension/runtime symbols and signatures where introspection is reliable.

Known PyO3 introspection limitations may require explicit allowlists. Any allowlist must be narrow and documented; do not suppress entire classes/modules.

### Consumer fixtures

Create small downstream-style typing fixtures that should type-check, for example:

```python
import eggfetch

with eggfetch.Client() as client:
    response: eggfetch.Response = client.get("https://example.com")
    body: bytes = response.content
```

and:

```python
async def body():
    yield b"a"

async def use() -> None:
    async with eggfetch.AsyncClient() as client:
        response = await client.post("https://example.com", content=body())
        reveal_type(response)
```

Also add negative/static cases where practical, especially async-only content passed to sync `Client`.

Do not make typing fixtures perform external network I/O.

## Public API / Stub Consistency Gate

Add a mechanical check that:

- every `eggfetch.__all__` symbol is represented in the supported typing surface;
- no public stub-only symbol is absent from runtime without an intentional typing-only construct;
- exception hierarchy names are synchronized;
- package marker and stub files are present in built wheels.

This check should run in Tier 1 or package validation depending on cost. Static stub syntax/checking belongs in routine validation; installed-wheel typing smoke belongs in package/release validation.

## Documentation

Add a short typing section to Python documentation covering:

- PEP 561 support;
- supported type-checker expectations;
- distinction between native `eggfetch` API and versioned compatibility facade types;
- current Python version support;
- how request streaming types differ between `Client` and `AsyncClient`.

Do not promise completeness for private underscore modules.

## Validation

Run:

```text
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

plus:

- stub syntax/type checker validation;
- runtime/stub structural comparison;
- consumer fixture type checking;
- isolated built-wheel marker/stub inspection.

## Acceptance Criteria

- [ ] `eggfetch/py.typed` is present in source and built wheels.
- [ ] native public package has reviewed `.pyi` coverage.
- [ ] every supported `eggfetch.__all__` symbol has a typing decision.
- [ ] public exception hierarchy is represented correctly in stubs.
- [ ] sync/async request-body types reflect actual post-interop behavior.
- [ ] response/stream/context-manager return types are accurate.
- [ ] compatibility-package public entry points remain usable by type checkers without requiring a full annotation campaign for private underscore modules.
- [ ] runtime/stub structural drift is mechanically checked.
- [ ] representative downstream typing fixtures pass.
- [ ] package validation fails if marker/stubs are missing from the wheel.
- [ ] no private implementation module is accidentally declared supported solely due to PEP 561 packaging.
- [ ] Tier 1, Tier 2, and package validation pass.

## Handoff

Once typing/package checks are stable, freeze the full program through `post-python-interop-api-qualification-and-closure.md`.
