# Milestone F Plan: Python Sync API

## Objective

Expose a Python synchronous API over the async Rust core using PyO3 and maturin. The sync API should feel familiar to users of `requests` and HTTPX while preserving the project invariant: there is exactly one networking implementation, and it lives in `eggfetch-core`.

The sync layer is an adapter. It should own or borrow a Tokio runtime, release the GIL while blocking, call the async Rust client, and return Python wrapper objects. It must not implement HTTP behavior itself.

## Required prerequisite

Do not start this milestone until the implementation hardening pass for streaming, timeouts, and pool lifetimes is either complete or its remaining limitations are explicitly accepted and documented.

Python APIs will freeze semantics around blocking behavior, body consumption, timeout errors, and streaming. Ambiguity in the Rust core should be resolved first.

## Scope

Milestone F includes:

- PyO3/maturin packaging foundation
- importable Python module
- synchronous top-level request helpers
- synchronous `Client`
- runtime ownership model
- GIL release around blocking operations
- Python `Response` wrapper with minimal buffered access
- Python exception mapping foundation
- basic Python tests

Milestone F does not include:

- Python async API
- full HTTPX response API
- cookies
- redirects unless already implemented in core
- auth flows
- multipart/files
- proxy support
- transport injection
- ASGI/WSGI transports
- full requests/httpx compatibility

## Public Python API target

The first sync API should support:

```python
import eggfetch

r = eggfetch.get("https://example.com")
print(r.status_code)
print(r.headers)
print(r.content)
print(r.text)

r = eggfetch.post("https://example.com/api", content=b"hello")

with eggfetch.Client(headers={"user-agent": "eggfetch"}, timeout=5.0) as client:
    r = client.get("https://example.com", params={"q": "test"})
```

Top-level helpers should include:

- `request(method, url, **kwargs)`
- `get(url, **kwargs)`
- `post(url, **kwargs)`
- `put(url, **kwargs)`
- `patch(url, **kwargs)`
- `delete(url, **kwargs)`
- `head(url, **kwargs)`
- `options(url, **kwargs)`

`Client` methods should mirror the same set.

## Packaging

Use `maturin` for Python packaging.

Expected files:

```text
crates/eggfetch-python/
  Cargo.toml
  pyproject.toml
  README.md or package docs if needed
  src/lib.rs
  python/eggfetch/__init__.py if using mixed Rust/Python package layout
  tests/
```

The package should build locally with:

```sh
cd crates/eggfetch-python
maturin develop
python -c "import eggfetch; print(eggfetch.__version__)"
```

If maturin is not installed in CI yet, document local setup first and add CI in a later packaging pass.

## Rust/PyO3 structure

Suggested modules inside `eggfetch-python/src`:

```text
lib.rs
client.rs
response.rs
request.rs
headers.rs
timeout.rs
errors.rs
runtime.rs
conversion.rs
```

Keep these small and adapter-oriented.

`eggfetch-python` may depend on:

- `eggfetch-core`
- `pyo3`
- `tokio`

Avoid direct `hyper` dependencies in the Python crate.

## Runtime ownership model

The sync `Client` should own a runtime and an `eggfetch_core::Client`.

Recommended design:

```rust
#[pyclass]
struct PyClient {
    runtime: tokio::runtime::Runtime,
    client: eggfetch_core::Client,
    closed: AtomicBool or bool guarded by PyO3 access rules,
}
```

Use a current-thread runtime first unless the core requires a multi-thread runtime for DNS/TLS/IO behavior. If multi-thread is needed, document why.

Do not create a new runtime per request for persistent clients.

Top-level helper functions may either:

1. create a short-lived client and runtime per call, or
2. use an internal global runtime and short-lived core client.

Preferred for correctness and simplicity: top-level helpers create a short-lived sync client path with clear lifecycle. Optimize later if needed.

## GIL behavior

All blocking network execution must release the GIL.

Use `Python::allow_threads` around runtime blocking:

```rust
py.allow_threads(|| runtime.block_on(future))
```

Do not hold Python references inside the future being awaited unless they are converted before releasing the GIL.

Convert Python arguments into owned Rust types before `allow_threads`.

Convert Rust results into Python objects after the future completes.

## Client lifecycle

Support context manager behavior:

```python
with eggfetch.Client() as client:
    r = client.get("https://example.com")
```

Required methods:

- `__enter__`
- `__exit__`
- `close()`
- `is_closed` property if easy

After close, client requests should raise a deterministic Python exception.

Dropping the Python object should clean up runtime/client resources. If explicit async shutdown exists in core, call it in `close()`.

## Request kwargs for Milestone F

Support a deliberately small but useful subset:

- `headers`: mapping or sequence of pairs
- `params`: mapping or sequence of pairs
- `content`: bytes or str
- `data`: optional alias for content only if simple
- `timeout`: float, `None`, or `Timeout` object

Defer:

- `json`
- `files`
- `cookies`
- `auth`
- `follow_redirects`
- `stream`
- `proxies`
- `verify`
- `cert`

If unsupported kwargs are passed, raise a clear `NotImplementedError` or `TypeError` with the unsupported argument name. Do not silently ignore unsupported arguments.

## Timeout mapping

Expose a Python `Timeout` type only if the Rust semantics are ready enough.

Minimum support:

```python
eggfetch.get(url, timeout=5.0)
eggfetch.get(url, timeout=None)
```

If per-phase `Timeout(connect=..., read=..., write=..., pool=...)` is exposed, each exposed field must correspond to honest Rust behavior or be documented as accepted-but-not-independently-enforced.

Preferred for Milestone F: expose scalar timeout and `None` first. Add per-phase Python constructor once read/write/connect semantics are stronger.

## Response wrapper

Implement minimal sync `Response` properties/methods:

- `status_code`
- `headers`
- `url`
- `content`
- `text`
- `is_success`
- `raise_for_status()`

For Milestone F, prefer buffered responses by default. The Python sync API can call Rust `response.bytes().await` during request execution and store the bytes inside the Python response.

Streaming response iteration can wait until Milestone H unless the implementation hardening pass has already stabilized streaming response leases.

Python `Response.content`, `Response.text`, and `Response.json()` semantics later should be cache-friendly. Establish `content` caching now.

## Header wrapper

For Milestone F, a simple immutable Python mapping-like view is sufficient.

Requirements:

- case-insensitive lookup
- preserves string display
- supports `response.headers["content-type"]`
- supports `.get()`
- supports iteration over header names/items if simple

Do not collapse `set-cookie` behavior incorrectly. If multi-value access is not implemented yet, document it and avoid pretending full mapping parity.

## Error mapping

Create a Python exception hierarchy.

Initial hierarchy:

```python
EggfetchError(Exception)
RequestError(EggfetchError)
InvalidUrl(RequestError)
TimeoutException(RequestError)
PoolTimeout(TimeoutException)
ConnectTimeout(TimeoutException)
ReadTimeout(TimeoutException)
WriteTimeout(TimeoutException)
NetworkError(RequestError)
ProtocolError(RequestError)
BodyError(RequestError)
HTTPStatusError(EggfetchError)
```

Only map phase-specific timeout exceptions if Rust returns phase-specific errors honestly.

`raise_for_status()` should raise `HTTPStatusError` for 4xx/5xx and include response/status context.

## Tests

Add Python tests under `crates/eggfetch-python/tests` or a top-level integration path.

Required tests:

- package imports
- `eggfetch.__version__` exists
- top-level `get()` against local test server
- top-level `post(content=b"...")` against local test server
- `Client` context manager works
- closed client raises deterministic error
- headers argument reaches server
- params argument serializes correctly
- scalar timeout maps to Rust timeout
- invalid URL maps to Python exception
- `raise_for_status()` raises on 4xx/5xx
- unsupported kwarg raises clear error

Use a Python local HTTP server for Python-level tests or expose a small Rust test binary only if needed. Keep tests deterministic.

## Documentation

Add Python usage examples to README, clearly marked as early API.

Document current compatibility level:

- requests/httpx-inspired, not drop-in
- sync only in Milestone F
- no redirects/cookies/auth/files yet
- streaming response API deferred unless implemented

## Acceptance criteria

Milestone F is complete when:

- `maturin develop` builds an importable `eggfetch` module
- top-level sync helpers work
- `Client` works and reuses the underlying Rust client
- network execution releases the GIL
- persistent client does not create a runtime per request
- basic response properties work
- basic Python exception mapping works
- unsupported kwargs fail loudly
- Python tests pass locally
- Rust workspace validation still passes

Required validation:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cd crates/eggfetch-python && maturin develop && python -m pytest
```

## Handoff notes

Do not implement the async Python API in this milestone. Keep the sync path correct and minimal first. The async layer will have different runtime/cancellation concerns and should be isolated in Milestone G.
