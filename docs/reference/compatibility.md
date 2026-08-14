# Feature compatibility matrix

This page tracks compatibility with requests and HTTPX across features. HTTPX
claims refer specifically to the pinned 0.28.1 asyncio-supported facade, not
all HTTPX transports or concurrency backends.
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
| Timeout (total wall-clock) | No | Yes | Yes | Yes | Yes |
| Timeout (pool wait) | No | Yes | Yes | Yes | Yes |
| Retry policy | No | No | Yes | Yes | Yes |
| Retry-After header | No | No | Yes | N/A | Yes |
| HTTP/2 | No | Yes | Yes | Yes | Yes |
| HTTP/3 (experimental) | No | No | Yes | Yes | Yes |
| Cross-origin header stripping | Manual | Manual | Automatic | Automatic | Automatic |
| Proxy env vars (HTTP_PROXY) | Yes | Yes | Yes | Yes | Yes |
| Custom transports (sync/async) | No | Yes | Yes | N/A | N/A |
| HTTP transport (HTTPTransport/AsyncHTTPTransport) | No | Yes | Yes | N/A | N/A |
| URL-pattern mount routing (priority matching) | No | Yes | Yes | N/A | N/A |
| MockTransport (no-network testing) | No | Yes | Yes | N/A | N/A |
| WSGITransport (WSGI app testing) | No | Yes | Yes | N/A | N/A |
| ASGITransport (ASGI app testing) | No | Yes | Yes | N/A | N/A |
| Event hooks (request/response sequencing) | No | Yes | Yes | N/A | N/A |
| DigestAuth (MD5/SHA-256) | No | Yes | Yes | N/A | N/A |
| NetRCAuth | No | Yes | Yes | N/A | N/A |
| Auth flow generator pattern | No | Yes | Yes | N/A | N/A |
| Async API | No | Yes | Yes | N/A | Yes (native) |

## Partially supported with differences

| Feature | Difference |
| --- | --- |
| Auth tuple shorthand | requests accepts `auth=("user","pass")`. eggfetch Python supports this. eggfetch Rust requires `BasicAuth::new("user", "pass")`. |
| Proxy configuration | requests uses a dict by scheme. eggfetch uses a single `proxy=` string. |
| Proxy env vars | The HTTPX facade selects `HTTP_PROXY`/`HTTPS_PROXY` by request scheme, uses `ALL_PROXY` as fallback, and honors lowercase forms plus `NO_PROXY` when `trust_env=True`; native Rust configuration remains explicit. |
| Timeout tuple | requests accepts `(connect, read)` tuples. eggfetch uses `Timeout` objects. |

## Intentionally unsupported

| Feature | Reason |
| --- | --- |
| Trio async backend | Deferred to Stage D |
| Python 3.8/3.9 | Requires Python 3.10+ (tokio runtime requirement) |
| Private HTTPX modules | `_transports`, `_content`, `_models`, `_decoders`, `_exceptions`, `_multipart`, `_urlparse`, `_config` excluded from contract |
| SSL context on Proxy | TLS handled by Rust engine (security boundary) |
| ALL_PROXY env var | Supported by the HTTPX facade as the scheme-specific fallback; native Rust callers configure proxies explicitly |

## Sync/async parity

eggfetch provides both `Client` (sync) and `AsyncClient` (async). Both
expose the same request methods, streaming API, cookie handling, and auth
configuration. The sync API blocks on the async Rust engine and releases
the GIL.

## HTTPX compatibility status

eggfetch targets HTTPX 0.28.1 compatibility in phases. The current status:

- **Phase 0**: Compatibility profile defined, manifest generators created, differential tests mandatory
- **Phase 1**: Timeout, pool, and lifecycle behavior alignment; contract rebaseline (150 active differences classified: 89 must-close, 61 intentional)
- **Phase 2**: Object contracts — Headers MutableMapping, QueryParams Mapping, exception hierarchy, NetRCAuth(file=...), URL.raw, codes IntEnum, Timeout/Limits/Proxy/default-encoding semantics (34 must-close resolved)
- **Phase 3**: Signature alignment — top-level helper args, Client/AsyncClient constructors, transport signatures, base-class relationships, stream types (55 must-close resolved)
- **Phase 4**: Direct transport — local_address, socket_options, UDS end-to-end, pool isolation, timeout/cancellation/resource release
- **Phase 5**: SOCKS5 proxy — HTTP/HTTPS through SOCKS5, auth, DNS/address-type behavior, NO_PROXY bypass, credential redaction
- **Phase 6 / Differential Closure**: Final qualification — API oracle clean (76 active differences, all intentional/deferred), full pinned compat suite passing, downstream behavioral fixtures validated

**Current status: Stage C qualified.** Pass 06 closed the pinned HTTPX IPv6
environment-form parity and route/pre-dispatch evidence gaps. Qualification is
bound to the exact executable SHA in `compat/httpx/0.28.1/profile.toml`; any
executable change requires fresh qualification. The compatibility facade does
not claim unrestricted HTTPX replacement. Trio/AnyIO, Python 3.8/3.9, and
private HTTPX modules remain outside scope.

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
7. **Proxy metadata**: HTTPX proxy URL credentials are translated into the core proxy authentication path. Arbitrary `Proxy(headers=...)` metadata and Python `ssl_context` objects are retained by the facade but are not yet forwarded into the Rust proxy engine; default verified proxy TLS is supported. These remain bounded Stage C differences and are covered by the corrective handoff rather than silently treated as equivalent.
8. **Timeout semantics**: HTTPX's scalar/default timeout maps to its four operational `connect`, `read`, `write`, and `pool` values, preserving omitted versus explicitly supplied `None` phase values. The facade does not create an EggFetch-native `total` deadline; native callers may configure that outer cap explicitly. Direct Hyper/UDS/H3 response-header acquisition remains a bounded transport limitation; body reads and proxy protocol reads are phase-aware.
9. **Stream exception hierarchy**: eggfetch's stream exceptions (StreamClosed, StreamConsumed, RequestNotRead, ResponseNotRead) now match HTTPX 0.28.1 exactly — inheriting from RuntimeError, accepting no arguments. Resolved in Phase 2.

### Phase 1 rebaseline (2026-08-07)

The active allowlist was rebaselined against the current `main` SHA `f9eb1a4...`. All 150 active differences are classified as `must-close` (89), `intentional` (61), or `deferred` (0). The `must-close` differences are assigned to implementation Phases 2 (34 entries) and 3 (55 entries). All must-close entries have been resolved. See `allowed-differences.toml` for the full classification with phase assignments.

### Historical Phase 6 differential closure (2026-08-10)

Historical qualification SHA: `40beeec09f3e88db8901f39388da665c47ab84f6`. Current exact-SHA evidence is recorded in the profile and corrective closure status.
