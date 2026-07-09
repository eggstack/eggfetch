# Milestone G Plan: Python Async API

## Objective

Expose a Python `asyncio` API over the same async Rust core used by the sync Python API and Rust API. The goal is an HTTPX-shaped `AsyncClient` that preserves eggfetch's single-engine invariant while integrating naturally with Python async code.

This milestone should not introduce a second runtime model or duplicate HTTP logic. It should adapt Rust futures into Python awaitables and Python async context managers.

## Prerequisites

Milestone F should be complete and stable enough that Python exception classes, response wrappers, argument conversion, timeout conversion, and package layout already exist.

The implementation hardening pass should be complete or explicitly accepted. Async response streaming and cancellation semantics will expose core lifecycle behavior more directly than the sync API.

## Scope

Milestone G includes:

- `AsyncClient`
- async top-level helper functions if feasible
- `async with AsyncClient()` support
- awaitable request methods
- `asyncio` integration
- cancellation behavior review
- async response object reuse of Milestone F wrappers where possible
- basic async Python tests

Milestone G does not include:

- Trio or AnyIO backend support
- ASGI transport support
- full streaming API unless already stable enough
- cookies/auth/redirects/files/proxies
- full HTTPX parity

## Public API target

```python
import eggfetch

async with eggfetch.AsyncClient(timeout=5.0) as client:
    response = await client.get("https://example.com", params={"q": "test"})
    print(response.status_code)
    print(response.text)

response = await eggfetch.async_get("https://example.com")
```

Required methods:

- `AsyncClient.request(method, url, **kwargs)`
- `AsyncClient.get(url, **kwargs)`
- `AsyncClient.post(url, **kwargs)`
- `AsyncClient.put(url, **kwargs)`
- `AsyncClient.patch(url, **kwargs)`
- `AsyncClient.delete(url, **kwargs)`
- `AsyncClient.head(url, **kwargs)`
- `AsyncClient.options(url, **kwargs)`
- `AsyncClient.aclose()`
- `AsyncClient.__aenter__()`
- `AsyncClient.__aexit__()`

Top-level async helper naming should be deliberate. Avoid colliding confusingly with sync `get()` unless Python dispatch can be made clear. Options:

1. `await eggfetch.async_get(url)` for initial milestone.
2. No top-level async helpers initially; require `AsyncClient`.

Preferred: implement `AsyncClient` first and defer top-level async helpers if naming is awkward.

## Runtime integration

Do not create a separate independent networking implementation.

The async Python layer should call into `eggfetch_core::Client` futures.

Options for PyO3 async integration:

- `pyo3-async-runtimes` if it fits the current PyO3 version and dependency policy.
- A small custom bridge if dependencies are undesirable.

The bridge must integrate with `asyncio` and preserve cancellation behavior as much as practical.

If adding `pyo3-async-runtimes`, document why the dependency is justified and keep features minimal.

## Client ownership

`AsyncClient` should own an `eggfetch_core::Client`.

It should not own a blocking runtime like the sync `Client` does. The Rust core futures should execute under an async runtime compatible with the PyO3 bridge.

Clarify whether a Tokio runtime is globally initialized by the Python module or supplied by the bridge. Document shutdown behavior.

Avoid creating a new runtime per request.

## Cancellation semantics

Python async cancellation is important.

When a Python task awaiting `client.get()` is cancelled:

- the Rust future should be dropped if possible
- pool permits should be released
- upload streams should be dropped
- response body leases should not leak

Add tests that cancel in-flight requests and verify later requests still work.

If cancellation cannot be fully propagated through the selected bridge, document the limitation and do not overclaim.

## Request kwargs

Mirror Milestone F's supported kwargs initially:

- `headers`
- `params`
- `content`
- `data` if implemented in F
- `timeout`

Unsupported kwargs should fail loudly.

Do not expand request feature surface in this milestone unless it is trivial and already supported by core.

## Response behavior

For Milestone G, the async request methods may return the same buffered Python `Response` object used by the sync API.

This means:

```python
response = await client.get(url)
response.content
response.text
```

If async streaming is implemented in this milestone, use distinct methods:

```python
async with client.stream("GET", url) as response:
    async for chunk in response.aiter_bytes():
        ...
```

However, async streaming can be deferred to Milestone H unless core response lifetime is fully ready.

## Context manager behavior

Support:

```python
async with eggfetch.AsyncClient() as client:
    ...
```

`aclose()` should be idempotent.

Requests after close should raise a deterministic exception.

If the underlying core has async shutdown, call it in `aclose()`.

## Error mapping

Reuse the Python exception hierarchy from Milestone F.

Ensure exceptions raised inside awaited Rust futures become Python exceptions of the correct type.

Cancellation should normally surface as Python `asyncio.CancelledError`, not as an eggfetch request error, if the Python task is cancelled externally.

## Tests

Required async Python tests:

- `AsyncClient` can be imported
- `async with AsyncClient()` works
- `await client.get()` works against local server
- `await client.post(content=b"...")` works
- headers and params work
- invalid URL maps to Python exception
- timeout maps to Python exception
- closed client raises deterministic error
- unsupported kwarg raises clear error
- many concurrent requests complete successfully
- cancelling an in-flight request does not poison the client
- after cancellation, a later request succeeds

Use `pytest-asyncio` or the least heavy equivalent. If dependency minimization is preferred, use `asyncio.run()` inside ordinary pytest tests.

## Documentation

Document:

- `asyncio` is the only supported async backend initially
- Trio/AnyIO are not supported yet
- sync and async APIs share the same Rust engine
- cancellation semantics and known limitations
- streaming status, if any

## Acceptance criteria

Milestone G is complete when:

- `AsyncClient` works under `asyncio`
- requests are awaitable
- async context manager works
- no duplicate HTTP logic exists in Python crate
- cancellation is tested at least for one in-flight request path
- concurrent requests work
- Python exception mapping works for async requests
- docs clearly state backend support and limitations

Required validation:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cd crates/eggfetch-python && maturin develop && python -m pytest
```

## Handoff notes

Do not chase AnyIO compatibility here. A high-quality `asyncio` implementation is more valuable than a broad but vague async backend claim. Trio/AnyIO can be revisited once the core and Python API are stable.
