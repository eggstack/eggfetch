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
│   └── H3Error
│       ├── H3ConnectError
│       └── H3ProtocolError
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
- Chunk size parameter on all iterators; compatibility raw iterators default to `None` and native decoded iterators retain their existing defaults
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
- SOCKS5 proxy support: HTTP/HTTPS through SOCKS5, reference-pinned method/auth/address behavior, persistent route pools, and NO_PROXY bypass

**Phase 6 / Differential Closure implements:**
- Typed difference records in API oracle (`scripts/compare_httpx_api_manifest.py --validate`)
- Lossless merge semantics (`crates/eggfetch-python/tests/compat/test_merge_lossless.py`)
- Separate sync/async auth drivers
- Behavioral downstream fixtures (`compat/downstream/behavioral_fixtures/`)
- Native lifecycle proof fixtures (`test_native_timeout_classification.py`, `test_soak.py`, proxy and TLS tests)
- Final qualification is recorded only after the current corrective transport plan's exact-SHA gates pass; current evidence is bound to the SHA in `compat/httpx/0.28.1/profile.toml`, while historical Phase 6 counts remain non-current audit records.
- The HTTPX environment facade follows `urllib.request` precedence and URL-pattern `NO_PROXY` semantics: bare domains match the bare host and subdomains at label boundaries, leading-dot domains match subdomains only, localhost/IP literals are exact, explicit host ports require an explicit normalized target port, and CIDR-looking values do not become native subnet rules. Bare unbracketed IPv6 literals follow the pinned HTTPX environment form; bracketed IPv6 and IPv6 prefix-looking values are rejected before dispatch. Native `NoProxy::parse()` retains its richer bracketed-IPv6 and CIDR behavior.
- Corrective 05 closed the exact-SHA ledger and requalification gates; Stage C
  is qualified on `c44d4f25ffebc1a792335163ae4bc106076b3963` (also recorded in
  `compat/httpx/0.28.1/profile.toml`), and executable changes require a new
  exact-SHA qualification.
- The compatibility `Timeout` constructor uses a private `UNSET` sentinel so
  omitted phase values inherit the scalar while explicit `None` disables only
  that phase; `Timeout()` follows HTTPX validation and requires a scalar or all
  four phases. Conversion still forwards only `connect`, `read`, `write`, and
  `pool` to native Rust and never synthesizes native `total`.

All corrective closure phases (1-6) are complete, plus the remaining-parity program (Phases 01-05). The facade is Stage C qualified for Python 3.10+ asyncio. Key boundaries:

- Timeout conversion forwards only HTTPX's `connect`, `read`, `write`, `pool`; native `total` is EggFetch-only.
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
- H2-only is enforced for direct TLS, cleartext prior knowledge, and direct/UDS
  specialized routes; H2 origin framing through HTTP CONNECT remains HTTP/1.1,
  and `stream_id` remains unavailable metadata.
- The HTTPX four-element null-pointer `socket_options` form is rejected at the
  safe Rust boundary; the safe three-element form is supported.
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
