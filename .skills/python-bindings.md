# Python Bindings Skill

Use this skill when working on the eggfetch-python crate (PyO3/maturin bindings).

## Workflow

1. Read `docs/architecture/python-bindings.md` for the module map and API surface.
2. Read `docs/python/guide.md` for the user-facing API documentation.
3. Read existing Python source in `crates/eggfetch-python/src/` for code conventions.

## Building and Testing

```sh
cd crates/eggfetch-python
maturin develop
python -m pytest -p pytest_asyncio
```

CI must install `pytest-asyncio` explicitly. The `PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1` env var is required.

## Key Constraints

- All HTTP logic lives in eggfetch-core. The Python crate is a thin adapter.
- Sync API blocks on async engine and releases the GIL during network I/O.
- Async API targets asyncio via pyo3-async-runtimes.
- Response surface must be requests/httpx-compatible.
- Body kwargs (`content`, `data`, `json`) are mutually exclusive. `files` may combine with `data` but conflicts with `content` and `json`.
- Secret redaction applies to all Debug/Display/output paths.

## Exception Hierarchy

```
EggfetchError
├── RequestError
├── InvalidUrl
├── TimeoutException
│   ├── ConnectTimeout
│   ├── ReadTimeout
│   ├── WriteTimeout
│   └── PoolTimeout
├── NetworkError
├── ProtocolError
│   ├── Http2Error
│   │   ├── Http2GoAway
│   │   ├── Http2StreamReset
│   │   └── Http2FlowControlError
│   └── Http3Error
├── BodyError
└── HTTPStatusError
```

## HTTPX Compatibility Layer

The `eggfetch.compat.httpx` module provides an HTTPX 0.28.1 drop-in facade over the eggfetch Rust engine. Import path:

```python
from eggfetch.compat.httpx import Client, AsyncClient, Request, Response
```

**Phase 2 implements:**
- Value objects: `URL`, `QueryParams`, `Headers`, `Cookies`, `Timeout`, `Limits`, `Proxy`
- Status code helpers (`codes`)
- Request and Response objects with full HTTPX-compatible metadata
- `Client` and `AsyncClient` with constructor signatures, merge semantics, `build_request()`, `send()`
- Top-level helpers: `get`, `post`, `put`, `patch`, `delete`, `head`, `options`, `request`, `stream`
- Complete exception hierarchy matching HTTPX MRO

**Phase 3 implements:**
- Stream base classes: `SyncByteStream`, `AsyncByteStream`, `ByteStream`
- Response streaming delegation to native engine
- `iter_raw()`/`aiter_raw()` for undecoded transport bytes
- Chunk size parameter on all iterators (default 8192)
- Request streaming bodies (iterables, file-like, custom streams)
- Multipart passthrough to native encoder
- `StreamingRawBytesIterator` and `AsyncStreamingRawBytesIterator` types

**Phase 4 implements:**
- Transport layer: `BaseTransport`, `AsyncBaseTransport`, `Transport`, `AsyncTransport`, `MockTransport`
- Mounts: named transport routing with `Client.mount()` / `Client.unmount()`
- Auth: `Auth` base class, `BasicAuth`, digest auth, netrc integration
- Hooks: request/event hooks on `Client` and `Request`
- WSGI/ASGI transport: local app dispatch via `WSGITransport`, `ASGITransport`

**Phase 5 implements:**
- Downstream validation, expanded behavior corpus (30 cases), evidence report generation, compatibility-stage decision (Stage C justified)

**Phase 6 / Corrective Closure implements:**
- Typed difference records in API oracle (`scripts/compare_httpx_api_manifest.py --validate`)
- Lossless merge semantics (`crates/eggfetch-python/tests/compat/test_merge_lossless.py`)
- Separate sync/async auth drivers
- Behavioral downstream fixtures (`compat/downstream/behavioral_fixtures/`)
- Native lifecycle proof fixtures (`test_native_timeout_classification.py`, `test_soak.py`, proxy and TLS tests)

**Corrective Closure Phases 1-4 implements:**
- Explicit top-level function signatures matching HTTPX 0.28.1 (`request`, `get`, `post`, `put`, `patch`, `delete`, `head`, `options`, `stream`)
- Client-only options (`cookies`, `proxy`, `verify`, `timeout`, `trust_env`) separated from request args in top-level helpers
- `stream()` implemented as real `@contextmanager` yielding open response
- Auth normalization: tuples become `BasicAuth`, callables become `_FunctionAuth`, `None` disables auth
- Client state enum (unopened/opened/closed) preventing reopen after close
- Property setters for `auth`, `base_url`, `cookies`, `event_hooks`, `headers`, `params`, `timeout`
- HTTPX default headers (`Accept`, `Accept-Encoding`, `Connection`, `User-Agent`)
- Protocol validation: `http1=False, http2=False` raises `ValueError`, `http1=False, http2=True` raises `NotImplementedError`
- Transport unsupported option rejection: `uds`, `local_address`, `socket_options` raise `NotImplementedError`
- `Response.is_closed` public property for stream context manager compatibility
- Request construction: params-in-URL with duplicates, `data`+`files` multipart, compact JSON, stream auto-headers
- Response metadata: HTTP version, reason phrase, elapsed, `raise_for_status()` return, `next_request`
- Stream state: raw/decoded/text/line boundaries, exception handling, encoding
- One-hop native/custom transport boundary, mount matching, hook per-hop ordering
- Redirect state machine: method/body rewriting, cross-origin auth stripping, Cookie header stripping on all redirects, max_redirects
- Auth lifecycle: Basic, Digest, NetRC, callable, custom sync/async, auth through all transport types
- Scoped cookie jar: domain/path/secure/expiry selection, CookieConflict, multiple Set-Cookie
- Event hooks: request/response per-hop ordering, hook exception cleanup
- Cleanup: intermediate response cleanup, stream consumed state, close-once behavior

**Corrective Closure Phase 5 (historical differential closure) implemented:**
- Compact parity case registry (`compat/httpx/0.28.1/parity-cases.toml`)
- API oracle reconciliation: 0 unexplained, 0 stale, 0 resolved-in-active
- Resolved difference ledger: Cookies base-class fix, `main` export restored
- Runtime diagnostics: unsupported surfaces listed
- Closure status file (`plans/httpx-parity-correction-status.md`)

**Narrow corrective closure (current):**

- Per-request timeout overrides use HTTPX's four-value extension mapping.
- Request and Response state follows HTTPX for empty bodies, unread streams, buffered responses, live iteration, and redirect-location detection.
- Compatibility cookies are emitted by the facade jar only; native cookie kwargs are not used.
- Retained-body redirects replay buffered bytes through exactly one body source and reject one-shot streams before a second dispatch.
- Native dispatch converts compatibility timeout mappings explicitly and serializes URL parameters only once.
- Request-local and explicit Cookie state is merged with the scoped facade jar per hop; explicit Cookie headers are stripped on every redirect and regenerated from the jar.
- Live response iteration coalesces chunk sizes, decodes split text incrementally, and updates stream accounting and state.
- The compact `test_corrective_kernel.py` suite runs in Tier 1; the full compatibility suite and API oracle remain Tier 2 gates.

**Testing the compat layer:**

```sh
cd crates/eggfetch-python && maturin develop
EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
```

Focused corrective closure tests:

```sh
EGGFETCH_COMPAT_REQUIRED=1 pytest \
  crates/eggfetch-python/tests/compat/test_top_level_helpers_parity.py \
  crates/eggfetch-python/tests/compat/test_client_stream_overrides.py \
  crates/eggfetch-python/tests/compat/test_auth_input_normalization.py \
  crates/eggfetch-python/tests/compat/test_client_mutability_and_state.py \
  crates/eggfetch-python/tests/compat/test_protocol_and_unsupported_options.py \
  crates/eggfetch-python/tests/compat/test_request_construction_parity.py \
  crates/eggfetch-python/tests/compat/test_response_stream_state_parity.py \
  crates/eggfetch-python/tests/compat/test_redirect_state_machine_parity.py \
  crates/eggfetch-python/tests/compat/test_hook_cookie_auth_ordering.py \
  crates/eggfetch-python/tests/compat/test_cookie_scope_parity.py \
  -v --strict-markers
```

Validate profiles and manifests:

```sh
python scripts/generate_httpx_api_manifest.py --package eggfetch.compat.httpx --output /tmp/eggfetch-api.json
python scripts/compare_httpx_api_manifest.py \
  --reference compat/httpx/0.28.1/reference-api.json \
  --candidate /tmp/eggfetch-api.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml
```

## Architecture Reference

- Python bindings: `docs/architecture/python-bindings.md`
- Python API guide: `docs/python/guide.md`
