# HTTPX Parity — Phase 1 Implementation Handoff Inventory

Date: 2026-08-07
Baseline SHA: `f9eb1a455907d43210886b7b047d18bde8716652`
Reference: `httpx==0.28.1`
Active differences: 150 (89 must-close, 61 intentional, 0 deferred)

## Summary

| Phase | must-close entries | Behavior groups |
|-------|-------------------|-----------------|
| Phase 2 | 0 (resolved) | Object/config contracts — completed 2026-08-07 |
| Phase 3 | 55 | Signatures, transport inheritance, streams |
| Phase 4 | 0 (Phase 1 scope) | Advanced transport (UDS, local_address, socket_options) |
| Phase 5 | 0 (Phase 1 scope) | SOCKS proxy, ALL_PROXY env vars |

## Phase 2 — Python Object and Configuration Contracts (0 remaining)

Phase 2 completed 2026-08-07. All 34 must-close entries have been resolved and moved to `resolved-differences.toml`.

### Resolved Groups

| Group | IDs | Resolution |
|-------|-----|------------|
| Exception Hierarchy | EXCEPTION-OPTIONAL-MSG-001/-2, STREAM-ERROR-SIG-001/-2/-3, STREAM-ERROR-ARGS-001, STREAM-ERROR-BASE-001/-2/-3 | StreamError inherits RuntimeError; constructors match HTTPX no-arg signature; InvalidURL/CookieConflict require message |
| Headers MutableMapping | MUTABLE-MAPPING-003/-2/-3/-4 | Headers inherits MutableMapping; popitem/setdefault implemented |
| QueryParams Mapping | MAPPING-001 | QueryParams inherits Mapping |
| NetRCAuth Parameter | NETRC-PARAM-NAME-001/-2 | file= keyword added; auth_file= retained as alias |
| URL Raw Property | URL-RAW-002/-2/-3/-4/-5 | URL.raw property returns (scheme, host, port, path) tuple |
| Status Codes Type | CODES-KIND-001 through -11 | codes is now IntEnum with integer methods |

## Phase 3 — Exact Signatures, Transport Inheritance, Stream Types (55 entries)

### Group: HTTP Method Parameter Style (36 entries)

| Symbol | IDs | Reference behavior | EggFetch behavior | Native changes |
|--------|-----|-------------------|-------------------|----------------|
| get | HTTP-METHOD-ARGS-001 through -4 | Explicit keyword params `(url, *, params, ...)` | `(*args, **kwargs)` delegated to `request()` | No |
| post | HTTP-METHOD-ARGS-002 through -4 | Same | Same pattern | No |
| put | HTTP-METHOD-ARGS-003 through -4 | Same | Same pattern | No |
| patch | HTTP-METHOD-ARGS-004 through -4 | Same | Same pattern | No |
| delete | HTTP-METHOD-ARGS-005 through -4 | Same | Same pattern | No |
| head | HTTP-METHOD-ARGS-006 through -4 | Same | Same pattern | No |
| options | HTTP-METHOD-ARGS-007 through -4 | Same | Same pattern | No |
| request | HTTP-METHOD-ARGS-008 through -4 | Same | Same pattern | No |
| stream | HTTP-METHOD-ARGS-009 through -4 | Same | Same pattern | No |

**Required test**: `client.get("url", params={"k": "v"})` works with explicit keyword.

### Group: Transport Constructor Parameters (6 entries)

| Symbol | IDs | Reference behavior | EggFetch behavior | Native changes |
|--------|-----|-------------------|-------------------|----------------|
| HTTPTransport | TRANSPORT-PARAMS-001, -2 | `limits` has `Limits(...)` default | `limits` defaults to `None` | No |
| AsyncHTTPTransport | TRANSPORT-PARAMS-001B, -2 | Same | Same | No |
| WSGITransport | TRANSPORT-PARAMS-001F | Inherits `BaseTransport` | Inherits `object` | No |
| MockTransport | TRANSPORT-PARAMS-001G | Inherits `AsyncBaseTransport, BaseTransport` | Inherits `object` | No |

**Required test**: `HTTPTransport(limits=Limits(...))` default matches HTTPX; transport isinstance checks.

### Group: Stream Constructor Signatures (13 entries)

| Symbol | IDs | Reference behavior | EggFetch behavior | Native changes |
|--------|-----|-------------------|-------------------|----------------|
| ByteStream | STREAM-CTOR-001 through -4 | `ByteStream(stream=None)` with ABC inheritance | `ByteStream(content=b'')` | No |
| SyncByteStream | STREAM-CTOR-001B through -5 | Same pattern | Same pattern | No |
| AsyncByteStream | STREAM-CTOR-001C through -4 | Same pattern | Same pattern | No |

**Required test**: `ByteStream(stream=...)` constructor works; ABC inheritance is correct.

## Intentional Differences (61 entries)

These are reviewed and preserved with positive rationale:

| Behavior case | Symbols | Count | Rationale |
|---------------|---------|-------|-----------|
| extra-public-properties | Limits, Proxy, Request, Response | 29 | Additive; do not conflict with HTTPX API |
| timeout-extra-methods | Timeout | 12 | `as_dict()` is additive convenience |
| client-extra-params | Client, AsyncClient | 6 | `extensions` param is additive |
| extra-symbol | COMPATIBILITY_INFO, CompatibilityInfo, _build_response, diagnostics_summary, get_compatibility_info | 5 | EggFetch-only diagnostic/compat symbols |
| basicauth-extra-param | BasicAuth | 4 | `encoding` param is additive |
| auth-internal-methods | DigestAuth | 2 | Extra `username`/`password` properties |
| extra-methods | ASGITransport | 2 | `handle_request` is additive |
| private-internal-symbols | create_ssl_context | 1 | TLS handled by Rust engine |

## Deferred Differences (0 entries)

None. All active differences are classified as `must-close` or `intentional`.

## Acceptance Traceability

- Starting SHA: `f9eb1a455907d43210886b7b047d18bde8716652`
- API oracle: 121 allowed matches, 0 unexplained, 0 stale, 0 resolved-in-active
- Classification: 55 must-close (Phase 3), 61 intentional, 0 deferred
- Phase 2 owns 0 must-close entries (all 34 resolved 2026-08-07)
- Phase 3 owns 55 must-close entries (signatures, transport inheritance, streams)
- Phase 4 and Phase 5 have no must-close entries from the API oracle (their scope is native/transport features not captured by the Python API manifest)
- No runtime, dependency, CI, or release changes were introduced
