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
- Sync streaming responses keep using the originating client Tokio runtime for
  body reads and iterator producers; do not introduce a shared replacement
  runtime for transport-owned response streams. A live stream also retains a
  runtime lease so it remains readable after `Client.close()`.
- Async API targets asyncio via pyo3-async-runtimes.
- Response surface must be requests/httpx-compatible.
- Body kwargs (`content`, `data`, `json`) are mutually exclusive. `files` may combine with `data` but conflicts with `content` and `json`.
- Secret redaction applies to all Debug/Display/output paths.

## Exception Hierarchy

Source of truth: `crates/eggfetch-python/src/errors.rs`.

```
EggfetchError
├── RequestError
│   ├── InvalidUrl
│   ├── TimeoutException
│   │   ├── ConnectTimeout
│   │   ├── ReadTimeout
│   │   ├── WriteTimeout
│   │   └── PoolTimeout
│   ├── NetworkError
│   ├── ProtocolError
│   ├── BodyError
│   ├── TooManyRedirects
│   ├── DecompressionError
│   ├── UnsupportedContentEncoding
│   ├── ProxyError
│   │   ├── ProxyConnectError
│   │   └── ProxyAuthError
│   ├── BodyNotReplayableForRetry
│   ├── RetryBudgetExhausted
│   ├── RetryNotConfigured
│   ├── Http2Error
│   │   ├── Http2GoAway
│   │   ├── Http2StreamReset
│   │   └── Http2FlowControlError
│   └── H3Error
│       ├── H3ConnectError
│       └── H3ProtocolError
├── HTTPStatusError
├── UnsupportedKwarg
├── StreamConsumed
├── StreamClosed
└── ResponseNotRead
```

## HTTPX Compatibility Layer

The `eggfetch.compat.httpx` module provides an HTTPX 0.28.1 compatibility facade over the eggfetch Rust engine (Stage C qualified on frozen executable SHA `78a77ea153aae239ce7b722aeb9909a87df3bbb5`). The sibling `eggfetch.compat.httpx2` module targets httpx2 2.12.0 and is independently Stage C qualified on the same SHA; the prior `639bf186...` binding is historical after the post-freeze HTTP/3 diagnostics audit. The two facades coexist and importing one never mutates the other. Import paths:

```python
from eggfetch.compat.httpx import Client, AsyncClient, Request, Response
from eggfetch.compat.httpx2 import Client as H2Client  # sibling 2.12.0 surface
```

**httpx2 delta surface** (in `eggfetch.compat.httpx2` only; never backported
silently into 0.28.1): `FunctionAuth`, `Origin` + `URL.origin`, `QUERY`
(`query` top-level + client helpers), `Headers` `|`/`|=` operators, SSE
(`EventSource` framing over streamed responses), optional WebSocket
(wsproto framing over the core 101 `network_stream`; handshake via the
normal pipeline), `alias_httpx()` explicit opt-in, truststore default,
RFC 9110 status renames with reference `DeprecationWarning`
(`URL.raw` alone uses `HTTPXDeprecationWarning`).

**httpx2 core facade parity** (`plans/httpx2-2.12-core-facade-parity.md`, done):
shared helpers reused where semantics identical (`_asgi/_cookies/_mock/
_request/_response/_stream/_transports/_wsgi` re-exported); profile-specific
_auth/_urls/_client/_api/_headers/_config/_exceptions/_status_codes/
_sse/websockets` stay behind explicit boundaries. Behavior deltas:
IPv6 CIDR `NO_PROXY` fix (0.28.1 oddities preserved), decoder cap native 4
vs reference 5 (intentionally stricter), multipart `try_header` validation
before bytes, WSGI framing preservation. Streaming parity
(`plans/httpx2-2.12-sse-and-websocket-parity.md`, done): SSE framing over
streamed responses, optional WebSocket wsproto framing over the existing 101
`network_stream`. Parity cases `H2X-API-001..005,
H2X-AUTH-001, H2X-TLS-001, H2X-PROXY-001, H2X-COMP-001, H2X-MP-001,
H2X-WSGI-001, H2X-META-001` (core) plus `H2X-SSE-001/002`, `H2X-WS-001..003`
(streaming) in `compat/httpx2/2.12.0/parity-cases.toml`;
tests `test_httpx2_api_parity.py` + `test_httpx2_behavior.py` (core),
`test_httpx2_sse.py` + `test_httpx2_websocket.py` (streaming).

**Implemented surface** (historical phase detail lives in `docs/architecture/python-bindings.md`):

- Value objects: `URL`, `QueryParams`, `Headers`, `Cookies`, `Timeout`, `Limits`, `Proxy`; status helpers (`codes`)
- Request/Response with full HTTPX-compatible metadata; `Client`/`AsyncClient` constructors, merge semantics, `build_request()`, `send()`; top-level helpers (`get`, `post`, `put`, `patch`, `delete`, `head`, `options`, `request`, `stream`)
- Complete exception hierarchy matching the HTTPX MRO
- Streaming both directions: `SyncByteStream`/`AsyncByteStream` base classes, request streaming bodies (iterables, file-like, custom streams), multipart passthrough to the native encoder, raw iterators with chunk-size control
- Transport layer: `BaseTransport`, `AsyncBaseTransport`, `Transport`, `AsyncTransport`, `MockTransport`; named mounts via `Client.mount()`/`Client.unmount()`
- Auth: `Auth`, `BasicAuth`, digest auth, netrc integration; request/event hooks on `Client` and `Request`
- WSGI/ASGI local transports
- SOCKS5 proxy support with persistent per-route pools and NO_PROXY bypass
- Typed difference records in the API oracle, lossless merge tests, behavioral downstream fixtures, native lifecycle proof fixtures

**Differential closure / corrective passes (current state):**

- Corrective passes 01–08, the post-audit maturation program, the next-scope
  program, and the active H3 requalification are complete; Stage C is
  qualified on `78a77ea153aae239ce7b722aeb9909a87df3bbb5` (also recorded in
  both compatibility profiles and `plans/httpx-parity-correction-status.md`).
  Executable changes require a new exact-SHA qualification.
- Closure evidence: typed difference records gated by `allowed-differences.toml`, lossless merge semantics (`crates/eggfetch-python/tests/compat/test_merge_lossless.py`), separate sync/async auth drivers, behavioral downstream fixtures (`compat/downstream/behavioral_fixtures/`), and native lifecycle proof fixtures (`test_native_timeout_classification.py`, `test_soak.py`, proxy and TLS tests).

The facade is Stage C qualified for the documented Python 3.10+ asyncio
surface on the frozen SHA above. HTTP/3 remains separately experimental;
that transport decision does not change the HTTPX parity claim.
Key boundaries:

- Timeout conversion forwards only HTTPX's `connect`, `read`, `write`, `pool`; native `total` is EggFetch-only. The compat `Timeout` constructor uses a private `UNSET` sentinel so omitted phase values inherit the scalar while explicit `None` disables only that phase; `Timeout()` follows HTTPX validation and requires a scalar or all four phases.
- `Proxy(headers=...)` is forwarded on the proxy leg (resolved in Phase 05).
- `Proxy(ssl_context=...)` is translated to native TlsConfig for the proxy endpoint TLS handshake (resolved in Phase 05).
- Arbitrary Python ssl_context objects unrepresentable by rustls are rejected
  at construction time; helper-created and passthrough contexts are accepted
  only when their live state and mTLS provenance are representable.
- SSLContext classification uses a construction fingerprint
  (SHA-256 over extractable public state) for helper-created contexts;
  post-construction mutation drops the stored metadata and reclassifies
  from the live snapshot.  Passthrough contexts are not assigned a
  cert path or `verify` kwarg.  Two CA stores with identical
  cardinalities but different contents produce different `verify`
  kwargs (no CA-count heuristic).
- Proxy endpoint TLS is sourced exclusively from the proxy
  configuration; the origin `TlsConfig` is never used as a fallback
  for the proxy handshake.
- `Proxy.__repr__` and `Headers.__repr__` redact sensitive header
  values (`authorization`, `proxy-authorization`, `cookie`,
  `set-cookie`) to `<redacted>` so credentials do not appear in
  diagnostic dumps.
- Environment proxy follows HTTPX's `NO_PROXY` URL-pattern rules; bare unbracketed IPv6 accepted, bracketed/CIDR forms rejected.
- H2-only is enforced for direct TLS, cleartext prior knowledge, SNI override,
  SOCKS HTTPS, and direct/UDS specialized routes; H2 origin framing through
  HTTP CONNECT remains HTTP/1.1, and `stream_id` remains unavailable metadata.
- The HTTPX four-element null-pointer `socket_options` form is rejected at the
  safe Rust boundary; the safe three-element form is supported.
- One native extension parser serves sync/async buffered/streaming requests;
  `target`, `sni_hostname`, and `trace` share `extract_native_extensions()`.
- Sync trace callbacks work on both sync `Client` and `AsyncClient`;
  coroutine trace callbacks are rejected with `TypeError` before dispatch
  because the core `TraceObserver` is synchronous. Callback exceptions
  abort the request at the declared boundary and propagate as the
  original exception.
- 101 `network_stream` wrappers are chosen by caller API mode (sync wrapper
  for sync `Client` buffered/streaming, async wrapper for async `AsyncClient`
  buffered/streaming). Ordinary pooled responses and internal CONNECT
  tunnels expose no writable network stream. Direct upgrades carry real
  addrs/TLS, UDS reports `Unix` without IPs, opaque stays unavailable.
- Trailers are core-only in this milestone: `Response::trailers()` exists
  in Rust; Python native, HTTPX facade, FFI, and Node defer exposure
  (facade unchanged — HTTPX 0.28.1 has no `trailers`). Do not add facade
  surface without pinned reference evidence.
- `Limits(max_connections=...)` facade naming is unchanged (native
  `max_in_flight_requests*` are Rust-only aliases).
- Raw iteration marks streams consumed before first source read, counts source bytes before chunk adaptation, closes on normal exhaustion only.
- `test_corrective_kernel.py` runs in Tier 1; full compat suite, API oracle, and downstream runner are Tier 2/manual gates. Executable changes require fresh exact-SHA qualification (see `compat/httpx/0.28.1/profile.toml`).

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

Pinned raw-stream differential and native-boundary checks:

```sh
EGGFETCH_COMPAT_REQUIRED=1 pytest \
  crates/eggfetch-python/tests/compat/test_raw_stream_httpx_differential.py \
  crates/eggfetch-python/tests/compat/test_raw_stream_lifecycle.py \
  -q --strict-markers
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

### Corrective transport notes

The Rust core keeps proxy configuration explicit; the HTTPX compatibility
facade delegates environment discovery to Python's `urllib.request` policy.
`local_address` uses HTTPX's host-only form and binds with an OS-selected
source port. Socket options are classified from the running Python `socket`
module rather than copied Linux constants. UDS traffic uses the normal Hyper
HTTP/TLS path, and SOCKS tunnels use origin-form requests after the handshake;
the client retains a persistent SOCKS Hyper pool per route. HTTPX 0.28.1's
valid four-element `(level, option, None, optlen)` socket-option form is
accepted by its constructor and forwarded to the platform API; the facade
rejects arbitrary null-pointer operations at its safe Rust boundary and
supports the safe three-element form.
