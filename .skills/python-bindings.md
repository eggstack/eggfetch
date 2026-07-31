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
- Downstream validation, expanded behavior corpus (29 cases), upstream HTTPX test inventory (36 derived cases), evidence report generation, compatibility-stage decision (Stage C justified)

**Phase 6 / Corrective Closure implements:**
- Typed difference records in API oracle (`scripts/compare_httpx_api_manifest.py --validate`)
- Lossless merge semantics (`crates/eggfetch-python/tests/compat/test_merge_lossless.py`)
- Separate sync/async auth drivers
- Behavioral downstream fixtures (`compat/downstream/behavioral_fixtures/`)
- Native lifecycle proof fixtures (`test_native_timeout_classification.py`, `test_soak.py`, proxy and TLS tests)

**Phase 1 / HTTPX Parity Correction implements:**
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

**Testing the compat layer:**

```sh
cd crates/eggfetch-python && maturin develop
EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
```

Phase 4 test files:

```sh
EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/test_transports.py crates/eggfetch-python/tests/compat/test_mock_transport.py crates/eggfetch-python/tests/compat/test_mounts.py crates/eggfetch-python/tests/compat/test_auth.py crates/eggfetch-python/tests/compat/test_hooks.py crates/eggfetch-python/tests/compat/test_wsgi.py crates/eggfetch-python/tests/compat/test_asgi.py -v --strict-markers
```

Phase 1 corrective closure test files:

```sh
EGGFETCH_COMPAT_REQUIRED=1 pytest \
  crates/eggfetch-python/tests/compat/test_top_level_helpers_parity.py \
  crates/eggfetch-python/tests/compat/test_client_stream_overrides.py \
  crates/eggfetch-python/tests/compat/test_auth_input_normalization.py \
  crates/eggfetch-python/tests/compat/test_client_mutability_and_state.py \
  crates/eggfetch-python/tests/compat/test_protocol_and_unsupported_options.py \
  -v --strict-markers
```

Validate profiles and manifests:

```sh
python scripts/generate_httpx_api_manifest.py --package httpx --output /tmp/httpx.json
python scripts/generate_httpx_api_manifest.py --package eggfetch --output /tmp/eggfetch.json
python scripts/compare_httpx_api_manifest.py \
  --reference /tmp/httpx.json \
  --candidate /tmp/eggfetch.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml
```

## Architecture Reference

- Python bindings: `docs/architecture/python-bindings.md`
- Python API guide: `docs/python/guide.md`
