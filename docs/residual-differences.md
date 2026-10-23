# Residual Differences from HTTPX 0.28.1

The HTTPX 0.28.1 and httpx2 2.12.0 facades were Stage C qualified on frozen
executable SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c` (2026-10-22, requalification
after the eight-milestone implementation freeze — core-transport M008, M009 and
M010, TLS/protocol M005, python-httpx-compat M004 and M005, and
release-verification M006 and M007 — together with a patch-level dependency
update that cleared 11 live RustSec advisories; no Rust source changed in that
update and the Rust public-API oracle is byte-identical to the frozen snapshots
in all eight profiles).
**That binding was invalidated on 2026-10-23** by the lint-justification
cleanup pass, which changed executable inputs — Rust sources in six crates.
`26fa6192…` remains the last completed qualification; the current tree is
unqualified, and a fresh exact-SHA qualification is required before any
binding can be cited. The invalidation is recorded in
`plans/httpx-parity-correction-status.md`.
The prior `42a9c96db5b25354843bd83bd458c71757e95651` binding is historical.
The `015a56d7ec3edf186eec8ebccff01cbf5584274e` binding (the coordinated
0.2.2 candidate) is historical.
The M007 binding `3fc58fbd99ecb496749b833ee7b27436fe3b412d`,
`bb6e07320f61736702e37f335fd1d48c7c3dfbaf`,
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`, and
`d4979f1dac53f30f07900f54b01a88de06956c1c` bindings are historical; earlier
maintenance freezes remain historical implementation evidence. The
private-architecture campaign changed implementation ownership while
preserving public compatibility and behavior.
HTTP/3 remains separately experimental. These are the retained, tested
bounded differences; the active ledger
(`plans/httpx-parity-correction-status.md`) and parity registry are
authoritative.

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

For an HTTPS origin reached through an HTTP proxy, the CONNECT handshake is
still HTTP/1.1. Ordinary single-target requests then use the Hyper-managed
origin connection when the route is compatible; multi-address target
snapshots and other unsupported framing combinations retain the legacy
HTTP/1.1 fallback so typed proxy rejection and pinning semantics remain
unchanged. The compatibility fixture continues to assert the bounded
H2-only behavior of its local proxy/server combination. Direct TLS, cleartext
prior knowledge, direct/local-address and socket-option routes, SNI override,
SOCKS, and UDS H2 are separately enforced and tested.

Evidence: `TestH2ProxyConnectResidual::test_candidate_proxy_connect_remains_http1`
and parity case `H2-009`.

## Cookie `Domain` attribute policy

The engine jar and the HTTPX 0.28.1 facade jar consume the same `Set-Cookie`
stream, so ADR-0009 rule 3 requires them to reach the same accept/reject
verdict and to contribute the same `Cookie` header. The facade jar is
`http.cookiejar` — the jar HTTPX 0.28.1 itself uses — so its verdict is the
HTTPX-defined one and is unchanged. The engine was aligned to it.

Before the alignment the engine rejected every `Set-Cookie` whose explicit
`Domain` attribute named an IP address, so `Domain=127.0.0.1` served by
`127.0.0.1` was dropped by the engine while the facade stored it and sent
`a=1`. The engine now accepts an IP-literal `Domain` only when it names
exactly the response host. Its `domain_matches` already restricts IP
literals to exact equality (RFC 6265 §5.2.3), so the relaxation cannot widen
cross-host matching: `Domain=127.0.0.1` is still rejected for any other host
and the cookie is only ever returned to the exact IP it names. The engine
stores a normalized domain value (no leading dot) and a non-host-only flag,
so the cookie is returned to the named IP only.

One difference remains, and it is confined to an explicit IPv6-literal `Domain`
attribute:

- `Set-Cookie: a=1; Domain=[::1]` served by `[::1]` is *stored* by both jars,
  but the facade does not return it while the engine does.
  `http.cookiejar` rewrites a dotless request host to `<host>.local`
  (`eff_request_host`), so the request host becomes `[::1].local` and the
  library's leading-dot domain convention (`. [::1]`) no longer
  suffix-matches it. The engine keeps the RFC 6265 §5.2.3/§5.3 behavior and
  returns a cookie whose `Domain` names the host it was served from. A
  host-only cookie for the same IPv6 host is returned by both jars, so the
  difference is specific to the explicit IPv6-literal `Domain` attribute.

This difference is deliberate, not a defect: the facade keeps HTTPX 0.28.1
behavior by construction, and the engine's behavior is the RFC 6265-conformant
one. Matching the facade here would mean reproducing a standard-library
host-mangling artifact in the native engine.

The two jars also spell the stored domain differently: the facade keeps the
standard library's leading-dot convention (`.example.test`) while the engine
normalizes the dot away (`example.test`). The accept/reject verdict and the
contributed `Cookie` header are identical; only the stored domain string
differs, so a caller comparing a stored domain string across the two
surfaces sees `.example.test` and `example.test` for the same cookie.

Evidence: `crates/eggfetch-core/tests/cookie_policy_parity_tests.rs`,
`crates/eggfetch-python/tests/compat/test_cookie_policy_parity.py`.

## Redirect credential scope: engine vs HTTPX 0.28.1

`eggfetch-core`'s redirect pipeline and `eggfetch.compat.httpx` implement two
different redirect policies. HTTPX 0.28.1 is the facade's specification, so
the facade keeps HTTPX behaviour and the engine keeps its stricter rules.
The difference is deliberate and is pinned by
`crates/eggfetch-python/tests/compat/test_redirect_policy_parity.py`.

| case | engine | facade / HTTPX 0.28.1 |
| --- | --- | --- |
| `Authorization`, same-origin hop | kept | kept |
| `Authorization`, cross-origin hop | dropped | dropped |
| `Authorization`, `https` -> `http` downgrade | dropped | dropped |
| `Authorization`, canonical `http:80` -> `https:443` same-host upgrade | dropped | kept |
| `Proxy-Authorization`, any hop | dropped | kept |
| `Cookie`, same-origin hop | kept | dropped |
| `Cookie`, cross-origin hop | dropped | dropped |
| `Referer` / `Origin`, cross-origin hop | dropped | kept |
| `Host`, cross-origin hop | rewritten | rewritten |
| method, `301`/`303`/`307`/`308` | RFC 9110 table | HTTPX table (agree) |
| method, `302` with a method other than POST/HEAD | preserved (body preserved) | rewritten to `GET` (body dropped) |

The engine's rule is origin-based: credentials survive only while
`origin(previous) == origin(next)`, so any scheme or port change is
cross-origin. HTTPX instead keeps `Authorization` for one special case, the
canonical `http:80` -> `https:443` same-host upgrade, which it treats as an
upgrade of the same trust domain; an `https` -> `http` downgrade is
cross-origin for both implementations and is therefore **stripped by both**.

Why the engine stays conservative:

- `Proxy-Authorization` is a proxy credential. Forwarding it to a different
  origin discloses the proxy secret to that origin, so the engine drops it
  on every hop.
- `Referer` and `Origin` disclose the source URL to the destination origin;
  the engine drops them whenever the origin changes.
- `Authorization` is replayed only within one origin. HTTPX replays a bearer
  token across the canonical `http` -> `https` upgrade; the engine does not,
  because a scheme change is a new origin.
- An explicit `Cookie` header survives only same-origin hops in the engine;
  HTTPX drops it on every redirect. The engine is not weaker on the leak
  path, because it also drops the cookie on every cross-origin hop.

The `302` method difference is a browser-emulation rule in HTTPX (every
non-HEAD method becomes `GET`) versus the engine's RFC 9110 table, which
rewrites only `POST`. The engine preserves the method and the request body
for other methods, so the next origin receives what the client actually
sent.

Consequence for users: `eggfetch.compat.httpx` behaves exactly like HTTPX
0.28.1. The native `eggfetch` client and the engine-based surfaces are
stricter; callers that need HTTPX's redirect credential scope must use the
compatibility facade.

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
- Direct-connector 101 upgrades expose real local/remote addrs and TLS
  version/cipher/ALPN; UDS upgrades expose `Unix`/`TlsUnix` without IPs;
  standard opaque upgrades remain explicitly unavailable. H3 per-response
  metadata stays `None` (`TransportKind::Quic` reserved). No secrets or fake
  zeros are reported.
- Internal proxy CONNECT tunnels are never exposed as writable network
  streams. Their body iterator is the canonical access path.
- Cloned synchronous network-stream wrappers share a single mutable stream;
  their read and write calls are serialized while each call drives the
  originating Tokio runtime. This preserves ownership safety but does not
  provide concurrent full-duplex operations from multiple sync handles.

## HTTP trailers and transport observability (native-only)

- Core `Response::trailers()` surfaces H1 chunked trailers, H2 trailing
  HEADERS, and H3 trailing headers without buffering. H1 duplicate
  same-name trailers collapse upstream in hyper (`HeaderMap::insert`);
  H2 duplicates are preserved. Python/FFI/Node defer trailer exposure;
  the HTTPX facade is unchanged (0.28.1 has no `trailers`).
- `TransportMetrics` counts connector/DNS/TLS attempts, UDS/proxy outcomes,
  physical admission waits/timeouts/live/high-water, established read/write
  inactivity timeouts, H3 creations/evictions, and 101 upgrades where observable. Hyper
  socket-reuse and per-connection H2 stream counts remain absent (never
  estimated). `PoolMetrics` counts logical waits/cancellations.
- Native `max_in_flight_requests*` (logical permits) preferred;
  `max_connections*` are pre-1.0 aliases. Facade `Limits` naming unchanged.

## Caller-owned HTTP/3 datagram routing (native-only)

- `ClientBuilder::datagram_dialer` installs a `DatagramDialer` so every HTTP/3
  generation (explicit `Http3Only` or Alt-Svc discovered) is established over
  one caller-owned fixed-target `DatagramRoute` instead of a direct QUIC
  endpoint. HTTPX has no equivalent concept, so there is no facade counterpart
  and no parity claim either way.
- Rust-only: the Python, FFI, and Node adapters expose nothing here, so no
  generated API manifest or typing surface changes.
- Configuring a dialer means Eggfetch creates no direct QUIC socket, so a
  failing or refusing route cannot silently downgrade. That is a deliberate
  behavioral difference from a default H3 client and is asserted directly.
- Eggfetch retains the whole QUIC/TLS stack: the seam carries datagrams only,
  and SNI and certificate validation stay bound to the logical origin even when
  the route's physical peer is a relay. No MASQUE/CONNECT-UDP protocol is
  implemented — a future provider may implement this generic contract without
  moving MASQUE policy into eggfetch.
- `H3RouteKind` still describes discovery (`Explicit`/`AltSvc`), not physical
  routing, so no public enum changed. Route identity is observable only through
  `H3ConnectionDiagnostic::remote_address`, which reports the physical peer as
  path metadata.
- Detailed design, invariants, and evidence: `core-tls-proxy-protocols.md` §
  "Caller-owned datagram routing (experimental)", `threat-model.md` §
  "Malicious Datagram Route", and ADR-0007.

## IPv6 pool-key divergence between the high-level and native origin paths (Rust-only)

- Measured during the core duplicate-helper consolidation (M010). The two
  origin normalizations are now one rule, but the *inputs* they are fed
  differ for IPv6 literals:
  - the high-level path builds the pool origin from the URL **host**;
    `url::Url::host_str()` returns the bracketed `[::1]` for
    `http://[::1]:8080/`;
  - the native path builds it from the URI **authority**; `http::Uri::host()`
    returns `[::1]`, which `HttpOrigin::from_uri()` strips to `::1`.
- Consequence: the same IPv6 endpoint produces two different pool keys
  (`[::1]:8080` high-level, `::1:8080` native), so the per-origin
  concurrency limit is not shared between the two entry paths for that
  endpoint. Nothing else changes: the scheme, the port, and the
  lowercase-host normalization are identical.
- Not changed here: aligning the two forms would alter pool keying, which
  the consolidation milestone explicitly excludes. It is recorded rather
  than papered over, and the (previously incorrect) doc comments in
  `http_origin.rs` that claimed the native form matches the high-level
  `url` form were corrected to state the divergence.
- Rust-only: the Python, FFI, and Node adapters reach the same core code
  paths, so no generated API manifest or typing surface changes, and there
  is no HTTPX counterpart to compare against.

## Historical closures and exclusions

Corrective 04 closed the prior H2-only TLS, cleartext prior-knowledge, and
direct-specialized-route gaps (parity cases `H2-002`, `H2-003`, and `H2-007`).
Corrective 06 closed the SNI override and SOCKS H2-only routes; they are
recorded as `parity` in `compat/httpx/0.28.1/parity-cases.toml` rather than
as bounded differences. There is no active residual for direct `verify=False`
routing, UDS custom ALPN, or server push: those claims were either
implemented, outside the public contract, or not a client-surface
requirement and were pruned from the active residual list.
