# Feature compatibility matrix

This page tracks compatibility with requests and HTTPX across features. HTTPX
claims refer specifically to the pinned 0.28.1 asyncio-supported facade
(`eggfetch.compat.httpx`), not all HTTPX transports or concurrency backends.
HTTPX2 claims refer specifically to the sibling 2.12.0 facade
(`eggfetch.compat.httpx2`, stage in `compat/httpx2/2.12.0/profile.toml`);
the two contracts are independent and never collapsed into one "HTTPX parity"
claim. HTTPX 1.0 pre-releases are preview-only (`compat/httpx/1.0-preview/`)
with no parity claim.
 eggfetch Node.js bindings are experimental and not included in this matrix.

## Supported and tested

| Feature | requests | HTTPX | eggfetch Python | eggfetch CLI | eggfetch Rust |
| --- | --- | --- | --- | --- | --- |
| GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS | Yes | Yes | Yes | Yes | Yes |
| Arbitrary methods | Yes | Yes | Yes | Yes | Yes |
| Custom headers | Yes | Yes | Yes | Yes | Yes |
| Case-insensitive headers | Yes | Yes | Yes | N/A | Yes |
| Query parameters | Yes | Yes | Yes | Yes | Yes |
| JSON request body | Yes | Yes | Yes | Yes | Yes |
| Form-encoded body | Yes | Yes | Yes | Yes | Yes |
| Raw bytes body | Yes | Yes | Yes | Yes | Yes |
| Response status code | Yes | Yes | Yes | Yes | Yes |
| Response headers | Yes | Yes | Yes | Yes | Yes |
| Response text | Yes | Yes | Yes | Yes | Yes |
| Response bytes | Yes | Yes | Yes | Yes | Yes |
| Response JSON | Yes | Yes | Yes | Yes | Yes |
| Raise on status | Yes | Yes | Yes | N/A | N/A |
| Connection pooling | Yes | Yes | Yes | Yes | Yes |
| Context manager client | Yes | Yes | Yes | N/A | N/A |
| Default headers | Yes | Yes | Yes | N/A | Yes |
| Streaming download | Yes | Yes | Yes | Yes | Yes |
| Streaming upload | Yes | Yes | Yes | Yes | Yes |
| Cookies | Yes | Yes | Yes | Yes | Yes |
| Basic auth | Yes | Yes | Yes | Yes | Yes |
| Bearer auth | Yes | Yes | Yes | Yes | N/A |
| Redirect following | Yes | Yes | Yes | Yes | Yes |
| Max redirects | Yes | Yes | Yes | Yes | Yes |
| Custom CA bundle | Yes | Yes | Yes | Yes | Yes |
| Client certificates (mTLS) | Yes | Yes | Yes | Yes | Yes |
| Disable TLS verification | Yes | Yes | Yes | Yes | Yes |
| HTTP proxy endpoint (`http://`) | Yes | Yes | Yes | Yes | Yes |
| HTTPS proxy endpoint (`https://`) | Yes | Yes | Yes | Yes | Yes |
| HTTPS CONNECT tunnel | Yes | Yes | Yes | Yes | Yes |
| SOCKS5 proxy | No | Yes (httpx[socks]) | Yes | Yes | Yes |
| Proxy authentication | Yes | Yes | Yes | Yes | N/A |
| NO_PROXY bypass | Yes | Yes | Yes | Yes | N/A |
| Unix domain sockets | No | Yes | Yes | N/A | Yes |
| Local address binding | No | Yes | Yes | N/A | Yes |
| Socket options | No | Yes | Yes | N/A | Yes |
| Response decompression (gzip) | Yes | Yes | Yes | Yes | Yes |
| Response decompression (brotli) | Yes | Yes | Yes | Yes | Yes |
| Response decompression (zstd) | Yes | Yes | Yes | Yes | Yes |
| Response decompression (deflate) | Yes | Yes | Yes | Yes | Yes |
| Multipart file upload | Yes | Yes | Yes | Yes | Yes |
| Timeouts (connect, read, write) | Partial | Yes | Yes | Yes | Yes |
| Timeout (total wall-clock) | No | No | Yes (native `total`; facade maps 4 phases only) | Yes | Yes |
| Timeout (pool wait) | No | Yes | Yes | Yes | Yes |
| Retry policy | No | No | Yes | Yes | Yes |
| Retry-After header | No | No | Yes | N/A | Yes |
| HTTP/2 | No | Yes | Yes | Yes | Yes |
| HTTP/3 (experimental) | No | No | Yes | Yes | Yes |
| Cross-origin credential stripping | Automatic (host change) | Automatic (cross-origin) | Automatic | Automatic | Automatic |
| Proxy env vars (HTTP_PROXY) | Yes | Yes | Facade Yes (`trust_env=True`); native Python Yes by default (`trust_env=True`), Rust/CLI explicit-only | CLI explicit (`--proxy`/`EGGFETCH_PROXY` only) | Explicit-only |
| Custom transports (sync/async) | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| HTTP transport (HTTPTransport/AsyncHTTPTransport) | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| URL-pattern mount routing (priority matching) | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| MockTransport (no-network testing) | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| WSGITransport (WSGI app testing) | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| ASGITransport (ASGI app testing) | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| Event hooks (request/response sequencing) | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| DigestAuth (MD5/SHA-256) | No | Yes | Facade Yes (native: Basic/Bearer only) | N/A | N/A |
| NetRCAuth | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| FunctionAuth (httpx2 only) | No | No (0.28.1) / Yes (httpx2 2.12.0) | httpx2 facade Yes | N/A | N/A |
| Origin / URL.origin (httpx2 only) | No | No (0.28.1) / Yes (httpx2 2.12.0) | httpx2 facade Yes | N/A | N/A |
| QUERY method (httpx2 only) | No | No (0.28.1) / Yes (httpx2 2.12.0) | httpx2 facade Yes | N/A | N/A |
| Headers merge operators (httpx2 only) | No | No (0.28.1) / Yes (httpx2 2.12.0) | httpx2 facade Yes | N/A | N/A |
| SSE EventSource (httpx2 only) | No | No (0.28.1) / Yes (httpx2 2.12.0) | httpx2 facade Yes (framing over streamed responses) | N/A | N/A |
| WebSocket optional surface (httpx2 only) | No | No (0.28.1) / Yes (httpx2[ws]) | httpx2 facade Yes (wsproto over 101 network_stream) | N/A | N/A |
| Auth flow generator pattern | No | Yes | Facade Yes (native: N/A) | N/A | N/A |
| Async API | No | Yes | Yes | N/A | Yes (native) |

## Partially supported with differences

| Feature | Difference |
| --- | --- |
| Auth tuple shorthand | requests accepts `auth=("user","pass")`. eggfetch Python supports this. eggfetch Rust requires `BasicAuth::new("user", "pass")`. |
| Proxy configuration | requests uses a dict by scheme. eggfetch uses a single `proxy=` string. |
| Proxy env vars | The HTTPX facade selects `HTTP_PROXY`/`HTTPS_PROXY` by request scheme, uses `ALL_PROXY` as fallback, and honors lowercase forms plus `NO_PROXY` when `trust_env=True`; native Python also reads env by default (`trust_env=True`); native Rust configuration and the CLI (`--proxy`/`EGGFETCH_PROXY`) remain explicit. |
| Timeout tuple | requests accepts `(connect, read)` tuples. eggfetch uses `Timeout` objects. |

## Intentionally unsupported

| Feature | Reason |
| --- | --- |
| Trio async backend | Deferred to Stage D |
| Python 3.8/3.9 | Requires Python 3.10+ (tokio runtime requirement) |
| Private HTTPX modules | `_transports`, `_content`, `_models`, `_decoders`, `_exceptions`, `_multipart`, `_urlparse`, `_config` excluded from contract |
| HTTPX four-element null-pointer `socket_options` | Outside the safe Rust boundary; safe three-element form is supported |
| Unrepresentable `ssl.SSLContext` state | Fails closed with `TypeError` (safety boundary; representable state translates exactly) |

## Sync/async parity

eggfetch provides both `Client` (sync) and `AsyncClient` (async). Both
expose the same request methods, streaming API, cookie handling, and auth
configuration. The sync API blocks on the async Rust engine and releases
the GIL.

## HTTPX compatibility status

eggfetch targets HTTPX 0.28.1 compatibility in phases. The current status:

- **Phase 0–6 / Differential Closure**: Profile, timeout/pool/lifecycle,
  object contracts, signatures, direct transport, SOCKS5, and final oracle
  clean — all must-close resolved. Full history in
  `plans/httpx-parity-correction-status.md`.
- **Correctives 01–08 + post-maturation** (superseded SHAs `c44d4f25`,
  `5c7899f`, `d24101b`, `d034a10`): fail-closed SSLContext translation,
  single extension parser, caller-mode `network_stream`, H2-only
  propagation, redaction hardening, request/transport consolidation, H3
  lifecycle hardening, observability cleanup. Historical evidence only.
- **Next-scope requalification (current)**: Renewed HTTPX 0.28.1 and earned HTTPX2 2.12.0 Stage C on the frozen executable SHA `65beb675a5380d3ff4291da6833b91ebf12c769a`, qualified 2026-09-11. Executable scope: H3 Alt-Svc discovery/fallback/draining, H3 interop evidence (20-test corpus, experimental retained), httpx2 sibling facade (+ SSE/optional WS), 1.0-preview tracking, oracle generalization. Full evidence in `plans/httpx-parity-correction-status.md`.

**httpx2 core facade** (`eggfetch.compat.httpx2`, `H2X-API/META/AUTH/TLS/
PROXY/COMP/MP/WSGI`): `FunctionAuth`, `Origin`/`URL.origin`, `QUERY`,
`Headers` `|`/`|=`, truststore OS-trust default, IPv6 CIDR `NO_PROXY` fix
(0.28.1 oddities preserved), chained-decoder cap native 4 vs reference 5
(intentionally stricter), bounded decode with close-on-failure, multipart
`try_header` validation, WSGI framing, status aliases with reference
`DeprecationWarning` (`URL.raw` alone uses `HTTPXDeprecationWarning`).

**httpx2 streaming protocols** (`H2X-SSE-001/002`, `H2X-WS-001..003`,
qualified on the same SHA): SSE `EventSource`/`ServerSentEvent` framing
over streamed responses (incremental, bounded by `max_event_size`,
close/cancel releases the body/pool lease); optional WebSocket
(`websocket` + `httpx2.websockets.*`, wsproto framing over the existing
101 `network_stream`, handshake via the normal pipeline, max-message
enforced across fragments, proxy trust isolated).

**Current status: Stage C qualified for each facade independently.** Next-scope requalification renewed the
0.28.1 claim (`eggfetch.compat.httpx`) and earned the independent httpx2 2.12.0 claim (`eggfetch.compat.httpx2`), each bound to
its profile's exact executable SHA. Proxy headers are forwarded on the proxy leg;
proxy ssl_context is translated to native TlsConfig; create_ssl_context
returns a real ssl.SSLContext; SSLContext translation is fail-closed for
unrepresentable state; H2-only mode is enforced on standard TLS, SNI
override, direct-specialized, UDS, and SOCKS HTTPS routes; transport
hints (target, sni_hostname, trace) are supported by one shared parser;
sync trace callbacks work on both sync `Client` and `AsyncClient`;
network stream metadata and upgrade lifecycle are owned by the
response. Qualification is bound to the exact executable SHA in
`compat/httpx/0.28.1/profile.toml`; any executable change requires fresh
qualification. The compatibility facade does not claim unrestricted HTTPX
replacement. Trio/AnyIO, Python 3.8/3.9, and private HTTPX modules remain
outside scope. The retained bounded differences are unrepresentable
SSLContext state (rejected before dispatch), HTTP/2 `stream_id` metadata,
HTTP/2 origin framing through HTTP CONNECT proxies, HTTPX's unsafe
four-element null-pointer socket-option form, ordinary pooled
`network_stream` absence, internal CONNECT tunnel non-exposure, and
async coroutine trace callback rejection.

See `compat/httpx/0.28.1/` for the machine-readable profile and allowed differences.

### Allowed Differences

The API oracle produces structured difference records with typed tuples. Each difference is categorized as `required-now`, `required-later`, `intentional-difference`, `not-public`, or `not-applicable`. The active allowlist lives in `allowed-differences.toml` and gates CI enforcement.

### Resolved Differences

`resolved-differences.toml` is a separate historical ledger of previously-allowed differences that have been resolved (implemented and verified). It serves as an audit trail and must NOT appear in the active `allowed-differences.toml`. Entries are generated from the corrective closure pass and are tracked independently of the active allowlist.

### Corrected claims

The following statements from earlier documentation have been corrected:

1. **Pool timeout**: HTTPX 0.28.1 supports pool timeout via `Timeout(pool=...)`. eggfetch also supports this. The compatibility matrix has been updated to reflect this.
2. **Redirect default**: HTTPX 0.28.1 defaults to `follow_redirects=False`, same as eggfetch. The earlier claim that "HTTPX follows redirects by default" was incorrect for version 0.28.1.
3. **Proxy env vars**: the HTTPX facade selects `HTTP_PROXY`/`HTTPS_PROXY` by request scheme, uses `ALL_PROXY` fallback, and honors lowercase forms plus `NO_PROXY` when `trust_env=True`; native Rust configuration remains explicit. Bare unbracketed IPv6 environment literals match HTTPX 0.28.1; bracketed IPv6 and IPv6 prefix-looking forms fail before dispatch with `InvalidURL`, while native Rust parsing retains its richer syntax.
4. **SOCKS proxy**: HTTPX 0.28.1 exposes SOCKS proxy support as an optional public feature via `httpx[socks]`. eggfetch supports SOCKS5 (`socks5://` and `socks5h://`) with the pinned username/password method matrix, domain/IP address types, route-local pooling, origin TLS, and `NO_PROXY` bypass. Both schemes use the reference's domain ATYP for hostnames; native Rust configuration retains its explicit DNS distinction.
5. **UDS, local_address, socket_options**: HTTPX 0.28.1 exposes `UDS`, `local_address`, and `socket_options` transport parameters. eggfetch implements these through the native Rust engine with HTTPS, fixed/chunked streaming, reuse, and host-only local-address evidence. The safe three-element socket-option form is supported; the valid `(level, option, None, optlen)` form remains a bounded safe-Rust difference because arbitrary pointer semantics are not exposed.
6. **Proxy endpoint TLS**: `https://` proxy URLs establish and verify TLS to the proxy hostname before HTTP forwarding or CONNECT. Origin TLS after CONNECT remains independently verified against the origin hostname.
7. **Proxy metadata**: HTTPX proxy URL credentials are translated into the core proxy authentication path. `Proxy(headers=...)` metadata is forwarded on the proxy leg and never forwarded into the tunnel or to the origin. `Proxy(ssl_context=...)` is translated to a native `TlsConfig` for the proxy endpoint TLS handshake, separate from origin TLS config. Arbitrary Python ssl_context objects that cannot be represented by rustls are rejected at construction time with a clear TypeError. Default verified proxy TLS is supported. These are now resolved and tracked in `resolved-differences.toml`.
8. **Timeout semantics**: HTTPX's scalar/default timeout maps to its four operational `connect`, `read`, `write`, and `pool` values, preserving omitted versus explicitly supplied `None` phase values. The facade does not create an EggFetch-native `total` deadline; native callers may configure that outer cap explicitly. Direct Hyper/UDS/H3 response-header acquisition remains a bounded transport limitation; body reads and proxy protocol reads are phase-aware.
9. **Stream exception hierarchy**: eggfetch's stream exceptions (StreamClosed, StreamConsumed, RequestNotRead, ResponseNotRead) now match HTTPX 0.28.1 exactly — inheriting from RuntimeError, accepting no arguments. Resolved in Phase 2.
10. **SSL context translation**: `eggfetch.compat.httpx.create_ssl_context()` now returns a genuine Python `ssl.SSLContext` matching HTTPX 0.28.1 construction behavior. Passing an `ssl.SSLContext` to `Client(verify=...)` or `AsyncClient(verify=...)` triggers snapshot-based translation. Exactly representable contexts (default, custom CA, disabled verification, and provenance-bearing mTLS helpers) are reconstructed as native `TlsConfig` state. Passthrough contexts are classified from live state; unrepresentable ciphers, ALPN, TLS versions, or client-certificate provenance are rejected deterministically before dispatch because rustls cannot reproduce them safely.

### Phase 1 rebaseline (2026-08-07)

The active allowlist was rebaselined against the current `main` SHA `f9eb1a4...`. All 150 active differences are classified as `must-close` (89), `intentional` (61), or `deferred` (0). The `must-close` differences are assigned to implementation Phases 2 (34 entries) and 3 (55 entries). All must-close entries have been resolved. See `allowed-differences.toml` for the full classification with phase assignments.

### Historical Phase 6 differential closure (2026-08-10)

Historical qualification SHA: `40beeec09f3e88db8901f39388da665c47ab84f6`. Current exact-SHA evidence is recorded in the profile and corrective closure status.
