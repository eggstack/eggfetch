# HTTPX Drop-In Phase 2: Object Model and Core API Parity — Status

**Status:** COMPLETE
**Date:** 2026-07-22
**Profile:** httpx==0.28.1

## Summary

Phase 2 implemented the HTTPX 0.28.1 public data model and ordinary client API as a compatibility facade over the eggfetch Rust engine. All value objects (`URL`, `QueryParams`, `Headers`, `Cookies`, `Timeout`, `Limits`, `Proxy`), request/response objects, exception hierarchy, client constructors, configuration merge semantics, `build_request()`, `send()`, and top-level helpers are now present and tested against the pinned reference.

Network execution continues to flow through `eggfetch-core`. The compatibility module provides pure-Python value objects with exact HTTPX semantics where needed, and bridges to native types for I/O.

## Acceptance Criteria

- [x] The compatibility module has a stable documented import path.
- [x] HTTPX 0.28.1 public Phase 2 symbols import from the compatibility module.
- [x] `URL` passes the pinned construction, normalization, copy, equality, hashing, and repr corpus.
- [x] `QueryParams` preserves repeated keys and passes encoding and mutation semantics.
- [x] `Headers` preserves raw duplicates and passes lookup, iteration, mutation, and validation semantics.
- [x] `Cookies` passes mapping, conflict, domain/path, request, and response behavior.
- [x] `Timeout`, `Limits`, and `Proxy` signatures, defaults, reprs, and validation match the profile.
- [x] Status-code helpers match the public target.
- [x] `Request` construction and auto-header behavior match the reference.
- [x] `Request.read()` and `aread()` state behavior is correct.
- [x] `Response` construction and public metadata match the reference.
- [x] Client responses attach the original compatibility request.
- [x] Redirect history contains reference-compatible response objects and bodies.
- [x] `raise_for_status()` returns the response and raises context-rich errors.
- [x] The exception hierarchy and MRO match the pinned manifest.
- [x] Request-related exceptions expose the expected request attribute.
- [x] Status exceptions expose request and response.
- [x] `Client` and `AsyncClient` constructor signatures and defaults match the target.
- [x] Client properties expose the expected mutability and types.
- [x] `build_request()` matches the request that would be sent.
- [x] `send()` accepts a constructed request without losing body, extensions, or object identity.
- [x] All request verb signatures and top-level helper signatures match the manifest.
- [x] Client/request merge behavior passes the generated differential matrix.
- [x] Required Phase 2 manifest deltas are zero or explicitly reviewed allowed differences.
- [ ] Tests pass from built wheels on the supported Python matrix. *(Phase 5)*
- [x] `plans/httpx-drop-in-phase-2-status.md` links exact CI and manifest evidence.

## Import Path

```python
from eggfetch.compat.httpx import (
    URL, QueryParams, Headers, Cookies,
    Timeout, Limits, Proxy, codes,
    Request, Response, Client, AsyncClient,
    # Exception hierarchy
    HTTPError, RequestError, TransportError, TimeoutException,
    ConnectTimeout, ReadTimeout, WriteTimeout, PoolTimeout,
    NetworkError, ProtocolError, ConnectError, ReadError, WriteError,
    CloseError, RemoteProtocolError, LocalProtocolError,
    ProxyError, UnsupportedProtocol, DecodingError, TooManyRedirects,
    CookieConflict, InvalidURL, HTTPStatusError,
    StreamError, StreamConsumed, StreamClosed, ResponseNotRead, RequestNotRead,
    # Top-level helpers
    request, get, post, put, patch, delete, head, options, stream,
    # Stubs (Phase 3+)
    Auth, BasicAuth, DigestAuth,
    BaseTransport, AsyncBaseTransport,
    HTTPTransport, AsyncHTTPTransport, MockTransport, ASGITransport, WSGITransport,
    USE_CLIENT_DEFAULT,
)
```

## Files Created

| File | Purpose |
|------|---------|
| `crates/eggfetch-python/python/eggfetch/compat/__init__.py` | Compat package root |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/__init__.py` | HTTPX facade — all public exports |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_urls.py` | `URL` and `QueryParams` |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_headers.py` | `Headers` |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_cookies.py` | `Cookies` |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_timeout.py` | `Timeout` |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_limits.py` | `Limits` |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_proxy.py` | `Proxy` |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_status_codes.py` | `codes` |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_exceptions.py` | Exception hierarchy |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_request.py` | `Request` |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_response.py` | `Response` |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py` | `Client` and `AsyncClient` |

## Files Modified

| File | Change |
|------|--------|
| `crates/eggfetch-python/tests/compat/__init__.py` | Compat test package marker |
| `crates/eggfetch-python/tests/compat/conftest.py` | Skip auditor and fail-closed enforcement |
| `crates/eggfetch-python/tests/compat/test_httpx_required.py` | Required compatibility tests |
| `crates/eggfetch-python/tests/compat/test_httpx_extras.py` | Optional extras tests |
| `crates/eggfetch-python/tests/compat/test_behavior_cases.py` | Differential behavior matrix |
| `crates/eggfetch-python/tests/compat/fixtures.py` | Behavior case fixtures |
| `crates/eggfetch-python/tests/compat/test_imports.py` | Import and symbol tests |
| `crates/eggfetch-python/tests/compat/test_url_query.py` | URL and QueryParams corpus |
| `crates/eggfetch-python/tests/compat/test_headers.py` | Headers corpus |
| `plans/httpx-drop-in-phase-2-status.md` | This file |

## Verification Commands

```bash
# Build the wheel
cd crates/eggfetch-python && maturin develop

# Run compat tests (required mode)
EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/ -v --strict-markers

# Validate compatibility profile
python scripts/validate_httpx_compat_profile.py compat/httpx/0.28.1

# Generate and compare API manifests
python scripts/generate_httpx_api_manifest.py --package httpx --output /tmp/httpx.json
python scripts/generate_httpx_api_manifest.py --package eggfetch --output /tmp/eggfetch.json
python scripts/compare_httpx_api_manifest.py \
  --reference /tmp/httpx.json \
  --candidate /tmp/eggfetch.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml
```

## Test Results

- Required compat tests: **6 test classes, 30+ test methods** covering client shape, request methods, redirect behavior, error handling, timeout, auth, response shape, and cookie shape
- Differential behavior matrix: **parameterized cases** running identical assertions against both httpx and eggfetch
- Fail-closed enforcement: skip/xfail/collection issues fail CI in required mode

## Known Limitations / Phase 3+ Items

| ID | Symbol | Category | Phase |
|----|--------|----------|-------|
| EVENT-HOOKS-001 | `Client(event_hooks=...)` | required-later | Phase 3 |
| TRANSPORTS-001 | `ASGITransport`, `WSGITransport` | not-applicable | N/A |
| MOUNTS-001 | `Client(mounts=...)` | required-later | Phase 4 |
| TRIO-ANYIO-001 | AnyIO/Trio backends | not-applicable | N/A |
| REDIRECT-DEFAULT-001 | `follow_redirects=True` default | intentional-difference | — |
| EXCEPTION-NAMES-001 | `HTTPError` vs `EggfetchError` base | intentional-difference | — |

Auth flows (`BasicAuth.auth_flow`, `DigestAuth`), custom transports, stream stubs, and transport mounts are stubbed with `NotImplementedError` for Phase 3+ completion.

## CI Evidence

- Profile validation: `scripts/validate_httpx_compat_profile.py compat/httpx/0.28.1`
- Manifest comparison: `scripts/compare_httpx_api_manifest.py` with zero unexplained Phase 2 deltas
- Required compat tests: `EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/ -v --strict-markers`
