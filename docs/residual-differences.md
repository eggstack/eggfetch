# Residual Differences from HTTPX 0.28.1

The HTTPX compatibility profile is Stage C qualified for its documented
Python 3.10+ asyncio surface. These are the retained, tested bounded
differences; the active ledger and parity registry are authoritative.
The current qualification is bound to executable SHA
`5c7899fefb6df087dfa1b3578fbef9ba64f87742`, recorded in
`compat/httpx/0.28.1/profile.toml`.

## SSLContext representability

EggFetch translates helper-created and external `ssl.SSLContext` objects from
their live public state. Default verification, custom CA stores,
`verify=False`, and provenance-bearing helper mTLS contexts are supported when
rustls can represent them. Contexts with unrepresentable cipher suites, ALPN,
TLS-version policy, client-certificate provenance, or non-`ssl.SSLContext`
subclasses fail closed with `TypeError` before dispatch. This is intentionally
narrower than HTTPX's arbitrary OpenSSL context acceptance and is a safety
boundary, not a claim of unrestricted parity.

Corrective 06 makes the translation genuinely fail-closed:

- `CERT_REQUIRED + check_hostname=False` is preserved as certificate
  verification enabled but hostname verification disabled.
- TLS min/max version bounds from the snapshot are forwarded into the native
  `TlsConfigBuilder` via `min_tls_version` / `max_tls_version`; arbitrary
  Python versions outside 1.2–1.3 are rejected.
- External (non-`ssl.SSLContext`) subclass contexts are rejected because
  client-cert, ALPN, and trust state cannot be inspected through public APIs.
- Helper-created contexts are tracked via a weak-keyed registry with a
  public-state fingerprint; live mutation after construction drops the
  metadata and re-classifies from the snapshot.

The `_detect_client_cert()` accessor returns `False` unconditionally because
the public Python `ssl.SSLContext` API does not expose whether `load_cert_chain`
was called; relying on it would silently discard real mTLS identity. Helper
contexts carry `cert_path`/`key_path` provenance through the registry.

Evidence: `test_corrective_01_tls_and_proxy_trust_safety.py`,
`test_corrective_01_tls_network_proof.py`,
`test_ssl_context_network_proof.py`,
`test_ssl_context_translation.py`.

## Coroutine trace callbacks

`inspect.iscoroutinefunction` correctly identifies `async def` callbacks,
callable objects with async `__call__`, and `functools.partial` wrappers
where Python's introspection applies. The HTTPX 0.28.1 compatibility
facade rejects coroutine trace callbacks on both sync `Client` and
`AsyncClient` with `TypeError` before dispatch, because the core
`TraceObserver` is synchronous and core cannot await a Python coroutine
without unbounded reentrancy risk. Sync callbacks work on both APIs.

This is a bounded difference, not a hidden feature: no coroutine is
discarded without the user seeing the rejection. Removing it would
require either a binding-only async bridge (kept feasible in design but
deliberately deferred) or an async-aware core observer (out of scope for
this closure).

Evidence: `test_trace_detection.py`,
`test_corrective_02_extensions_and_wire_metadata.py`.

## HTTP/2 response `stream_id`

HTTPX exposes the h2 stream identifier in `Response.extensions`; EggFetch does
not. Hyper-util returns an `Incoming` body without exposing the underlying h2
`RecvStream` identifier, so synthesizing a value would be misleading. This is
metadata-only and does not affect request, response, retry, or streaming
semantics.

Evidence: `TestStreamIdAbsence::test_stream_id_absent_in_response_extensions`
and parity case `H2-008`.

## HTTP/2 through HTTP CONNECT

For an HTTPS origin reached through an HTTP proxy, the CONNECT handshake and
post-CONNECT origin path remain the hand-rolled HTTP/1.1 transport. Thus an
H2-only request through this proxy route does not use HTTP/2 framing at the
origin. Direct TLS, cleartext prior knowledge, direct/local-address and
socket-option routes, SNI override, SOCKS, and UDS H2 are separately enforced
and tested.

Evidence: `TestH2ProxyConnectResidual::test_candidate_proxy_connect_remains_http1`
and parity case `H2-009`.

## Socket-option safety boundary

HTTPX accepts a valid four-element `(level, option, None, optlen)` socket
option form that carries null-pointer semantics. EggFetch supports the safe
three-element form but rejects arbitrary pointer operations at the Rust FFI
boundary. This is a narrow safety difference, tracked by
`UNSUPPORTED-004`/`TRANSPORT-PARAMS-001`.

## Network-stream classification

These are deliberate ownership classifications rather than additional
residuals:

- A `101 Switching Protocols` response owns an upgraded stream with sync and
  async read/write/close operations. Leading bytes are preserved. `start_tls`
  is allowed only for an inner TCP stream; Hyper adapter and already-TLS
  variants are rejected before I/O is consumed.
- The wrapper type is selected by the calling API mode (sync `Client` returns
  a sync wrapper; async `AsyncClient` returns an async wrapper) for both
  buffered and streaming 101 responses.
- Ordinary HTTP/1.1 and HTTP/2 responses return their connections to the pool,
  so `extensions["network_stream"]` is `None`; shared connection metadata is
  read-only and no raw socket is exposed.
- Internal proxy CONNECT tunnels are never exposed as writable network
  streams. Their body iterator is the canonical access path.

## Historical closures and exclusions

Corrective 04 closed the prior H2-only TLS, cleartext prior-knowledge, and
direct-specialized-route gaps (parity cases `H2-002`, `H2-003`, and `H2-007`).
Corrective 06 closed the SNI override and SOCKS H2-only routes; they are
recorded as `parity` in `compat/httpx/0.28.1/parity-cases.toml` rather than
as bounded differences. There is no active residual for direct `verify=False`
routing, UDS custom ALPN, or server push: those claims were either
implemented, outside the public contract, or not a client-surface
requirement and were pruned from the active residual list.
