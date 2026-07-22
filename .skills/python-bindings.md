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
- Stubs for auth flows, transports, and stream classes (Phase 3+)

**Testing the compat layer:**

```sh
cd crates/eggfetch-python && maturin develop
EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
```

Validate profiles and manifests:

```sh
python scripts/validate_httpx_compat_profile.py compat/httpx/0.28.1
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
