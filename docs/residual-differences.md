# Residual Differences from HTTPX 0.28.1

The HTTPX compatibility profile is Stage C qualified for its documented
Python 3.10+ asyncio surface. These are the retained, tested bounded
differences; the active ledger and parity registry are authoritative.

## SSLContext representability

EggFetch translates helper-created and external `ssl.SSLContext` objects from
their live public state. Default verification, custom CA stores,
`verify=False`, and provenance-bearing helper mTLS contexts are supported when
rustls can represent them. Contexts with unrepresentable cipher suites, ALPN,
TLS-version policy, or client-certificate provenance fail closed with
`TypeError` before dispatch. This is intentionally narrower than HTTPX's
arbitrary OpenSSL context acceptance and is a safety boundary, not a claim of
unrestricted parity.

Evidence: `test_corrective_01_tls_and_proxy_trust_safety.py`,
`test_corrective_01_tls_network_proof.py`, and
`test_ssl_context_network_proof.py`.

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
socket-option routes, and UDS H2 are separately enforced and tested.

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
- Ordinary HTTP/1.1 and HTTP/2 responses return their connections to the pool,
  so `extensions["network_stream"]` is `None`; shared connection metadata is
  read-only and no raw socket is exposed.
- Internal proxy CONNECT tunnels are never exposed as writable network
  streams. Their body iterator is the canonical access path.

## Historical closures and exclusions

Corrective 04 closed the prior H2-only TLS, cleartext prior-knowledge, and
direct-specialized-route gaps (parity cases `H2-002`, `H2-003`, and `H2-007`).
There is no active residual for direct `verify=False` routing, UDS custom ALPN,
or server push: those claims were either implemented, outside the public
contract, or not a client-surface requirement and were pruned from the active
residual list.
