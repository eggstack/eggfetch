# Python Bindings Skill

Use this skill when working on the eggfetch-python crate (PyO3/maturin bindings).

The supported native import surface is `eggfetch.__all__` in
`crates/eggfetch-python/python/eggfetch/__init__.py`. The `_native` extension is private and must not
define a second `__all__` contract. Keep the public exception hierarchy,
`NetworkStream`/`AsyncNetworkStream` upgrade wrappers, signatures, and runtime
version synchronized with `crates/eggfetch-python/tests/native_api_manifest.json`; run
`scripts/check_native_python_api.py` after binding changes.

## Workflow

1. Read `docs/architecture/python-bindings.md` for the module map and API surface.
   Private streaming state/bridge/decoding/iterator ownership lives under
   `crates/eggfetch-python/src/streaming/`; keep `streaming.rs` as the PyO3
   class/wiring boundary.
2. Read `docs/python/guide.md` for the user-facing API documentation.
3. Read existing Python source in `crates/eggfetch-python/src/` for code conventions.

## Building and Testing

```sh
maturin develop -m crates/eggfetch-python/Cargo.toml
python -m pytest crates/eggfetch-python/tests/ -q --ignore=crates/eggfetch-python/tests/compat
```

Requires an active venv with the pinned tooling in
`scripts/ci-requirements.txt` (Python 3.10+, maturin, pytest, pytest-asyncio;
`check.sh` refuses to run without it). Rebuild after every Rust change in
`crates/eggfetch-python` — a stale `.so` causes confusing failures.
Tier 2 compat runs `EGGFETCH_COMPAT_REQUIRED=1 pytest .../compat/ -v --strict-markers`.

CI must install `pytest-asyncio` and `mypy` explicitly. The binding uses
interpreter-specific PyO3 wheels; do not add an ABI3 forward-compatibility
environment override.

PEP 561 typing is a reviewed contract for the public native package. Run
`python scripts/check_python_typing_surface.py` for stub/manifest/exception
drift plus reviewed class-member, signature-shape, sync/async, and semantic
return checks, then `python scripts/check_python_typing.py` for native and
compatibility consumer fixtures. Package validation runs
`python scripts/check_wheel_typing.py --wheel <wheel>` so the installed wheel
is checked for the same surface contract and positive/negative consumers, not
just the source tree. The native `_native` module remains private; the
compatibility facades have concise package stubs only for intentionally public
entry points and do not promise typed private implementation modules.

## Key Constraints

- All HTTP logic lives in eggfetch-core (CONNECT wire in `eggfetch-http-connect`). The Python crate is a thin adapter.
- Sync API blocks on async engine and releases the GIL during network I/O.
- Sync streaming responses keep using the originating client Tokio runtime for
  body reads and iterator producers; do not introduce a shared replacement
  runtime for transport-owned response streams. A live stream also retains a
  runtime lease so it remains readable after `Client.close()`.
- Async API targets asyncio via pyo3-async-runtimes.
- Response surface must be requests/httpx-compatible.
- Body kwargs (`content`, `data`, `json`) are mutually exclusive. `files` may combine with `data` but conflicts with `content` and `json`.
- `Client` and top-level sync helpers accept lazy synchronous iterable bodies;
  `AsyncClient` additionally accepts lazy async iterables of `bytes | str`.
  Sync APIs reject async-only iterables before dispatch.
- Shared client and request argument normalization lives in
  `crates/eggfetch-python/src/request_preparation.rs`; keep runtime ownership and dispatch-specific
  lifecycle behavior in the sync and async adapters.
- `prepare_client_config()` and `apply_client_config()` are the shared
  constructor path; do not reintroduce sync/async copies of TLS, proxy,
  limits, cookie, or transport-option setup.
- Request body classification obtains a synchronous or asynchronous iterator
  once and passes that owned iterator to the lazy adapter. Do not probe a
  one-shot iterable again during dispatch or buffer an async iterable.
- Generic `ssl.SSLContext` interop lives in `eggfetch._ssl_context`. The
  historical `eggfetch.compat.httpx._ssl_context` path is only a thin shim.
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

The `eggfetch.compat.httpx` module provides an HTTPX 0.28.1 compatibility facade over the eggfetch Rust engine. The sibling `eggfetch.compat.httpx2` module targets httpx2 2.12.0 and is independently Stage C qualified; the exact active executable SHA is recorded in the live status ledger and both profiles. The two facades coexist and importing one never mutates the other. Import paths:

```python
from eggfetch.compat.httpx import Client, AsyncClient, Request, Response
from eggfetch.compat.httpx2 import Client as H2Client  # sibling 2.12.0 surface
```

**httpx2 delta surface** (in `eggfetch.compat.httpx2` only; never backported
silently into 0.28.1): `FunctionAuth`, `Origin` + `URL.origin`, `QUERY`,
`Headers` `|`/`|=` operators, SSE (`EventSource` framing over streamed
responses), optional WebSocket (wsproto framing over the core 101
`network_stream`; handshake via the normal pipeline), `alias_httpx()`
explicit opt-in, truststore default, RFC 9110 status renames with reference
`DeprecationWarning` (`URL.raw` alone uses `HTTPXDeprecationWarning`).
Full facade/phase history lives in `docs/architecture/python-bindings.md`;
parity cases live in `compat/httpx2/2.12.0/parity-cases.toml` (see also
`compat/httpx/0.28.1/parity-cases.toml`).

**Implemented surface** (historical phase detail lives in `docs/architecture/python-bindings.md`):
value/request/response objects, `Client`/`AsyncClient` with merge semantics,
top-level helpers, HTTPX-matching exception MRO, bidirectional streaming,
transport/mount/auth/hook/WSGI/ASGI/SOCKS layers, typed oracle difference
records, lossless merge tests, and behavioral/downstream fixtures.

**Qualification state (current):** both facades are Stage C qualified on the
exact executable SHA recorded in the live ledger
`plans/httpx-parity-correction-status.md` and both
`compat/*/profile.toml` files. Executable or qualification-input changes
require a new exact-SHA qualification; docs-only commits do not. Historical
corrective-pass and phase plans remain in `plans/` as records only — do not
treat their step lists as current gates. HTTP/3 remains separately
experimental; that transport decision does not change the HTTPX parity claim.
Key boundaries:

- Timeout conversion forwards only HTTPX's `connect`, `read`, `write`, `pool`; native `total` is EggFetch-only. The compat `Timeout` constructor uses a private `UNSET` sentinel so omitted phase values inherit the scalar while explicit `None` disables only that phase; `Timeout()` follows HTTPX validation and requires a scalar or all four phases.
- `Proxy(headers=...)` is forwarded on the proxy leg only.
- `Proxy(ssl_context=...)` is translated to native TlsConfig for the proxy endpoint TLS handshake.
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

**Testing the compat layer** (full suite is a Tier 2 gate; the Tier 1 smoke
kernel is `test_imports.py`, `test_client.py`, `test_exceptions.py`,
`test_corrective_kernel.py` — see the verification skill):

```sh
maturin develop -m crates/eggfetch-python/Cargo.toml
EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
```

Run focused parity files by name under `crates/eggfetch-python/tests/compat/`
(e.g. `test_httpx2_api_parity.py`, `test_raw_stream_lifecycle.py`) rather than
relying on a frozen list here; the directory listing is authoritative.

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
