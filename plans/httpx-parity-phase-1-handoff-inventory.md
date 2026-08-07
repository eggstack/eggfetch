# HTTPX Parity — Phase 1 Implementation Handoff Inventory

Date: 2026-08-07
Baseline SHA: `f9eb1a455907d43210886b7b047d18bde8716652`
Reference: `httpx==0.28.1`
Active differences: 150 (89 must-close, 61 intentional, 0 deferred)

## Summary

| Phase | must-close entries | Behavior groups |
|-------|-------------------|-----------------|
| Phase 2 | 34 | Object/config contracts |
| Phase 3 | 55 | Signatures, transport inheritance, streams |
| Phase 4 | 0 (Phase 1 scope) | Advanced transport (UDS, local_address, socket_options) |
| Phase 5 | 0 (Phase 1 scope) | SOCKS proxy, ALL_PROXY env vars |

## Phase 2 — Python Object and Configuration Contracts (34 entries)

### Group: Exception Hierarchy (9 entries)

| Symbol | IDs | Reference behavior | EggFetch behavior | Native changes |
|--------|-----|-------------------|-------------------|----------------|
| InvalidURL | EXCEPTION-OPTIONAL-MSG-001, -2 | `InvalidURL(message)` required | Accepts `InvalidURL(message='')` | No |
| CookieConflict | EXCEPTION-OPTIONAL-MSG-002, -2 | `CookieConflict(message)` required | Accepts `CookieConflict(message='')` | No |
| StreamError | STREAM-ERROR-BASE-001, -2, -3 | Inherits `RuntimeError` | Inherits `Exception` | No |
| ResponseNotRead | STREAM-ERROR-SIG-001 | `ResponseNotRead()` no args | Accepts `*args` | No |
| StreamClosed | STREAM-ERROR-SIG-002 | `StreamClosed()` no args | Accepts `*args` | No |
| StreamConsumed | STREAM-ERROR-SIG-003 | `StreamConsumed()` no args | Accepts `*args` | No |
| RequestNotRead | STREAM-ERROR-ARGS-001 | `RequestNotRead()` no args | Accepts `*args` | No |

**Required test**: Confirm exception constructors match HTTPX signatures exactly.

### Group: Headers MutableMapping Contract (4 entries)

| Symbol | IDs | Reference behavior | EggFetch behavior | Native changes |
|--------|-----|-------------------|-------------------|----------------|
| Headers | MUTABLE-MAPPING-003, -2, -3, -4 | Inherits `Generic, MutableMapping`; has `popitem`, `setdefault` | Plain object; missing `popitem`/`setdefault`, has extra `append` | No |

**Required test**: `isinstance(Headers(), MutableMapping)` returns True; `popitem()` and `setdefault()` work.

### Group: QueryParams Mapping Contract (1 entry)

| Symbol | IDs | Reference behavior | EggFetch behavior | Native changes |
|--------|-----|-------------------|-------------------|----------------|
| QueryParams | MAPPING-001 | Inherits `Generic, Mapping` | Plain object | No |

**Required test**: `isinstance(QueryParams(), Mapping)` returns True.

### Group: NetRCAuth Parameter Name (2 entries)

| Symbol | IDs | Reference behavior | EggFetch behavior | Native changes |
|--------|-----|-------------------|-------------------|----------------|
| NetRCAuth | NETRC-PARAM-NAME-001, -2 | `NetRCAuth(file=...)` | `NetRCAuth(auth_file=...)` | No |

**Required test**: `NetRCAuth(file="...")` works as keyword argument.

### Group: URL Raw Property (5 entries)

| Symbol | IDs | Reference behavior | EggFetch behavior | Native changes |
|--------|-----|-------------------|-------------------|----------------|
| URL | URL-RAW-002, -2, -3, -4, -5 | `URL(url)` with `.raw` property | No `.raw`; different constructor params | No |

**Required test**: `URL("https://example.com").raw` returns raw components.

### Group: Status Codes Type (11 entries)

| Symbol | IDs | Reference behavior | EggFetch behavior | Native changes |
|--------|-----|-------------------|-------------------|----------------|
| codes | CODES-KIND-001, -2, -3 through -11 | `IntEnum` class with integer methods | Plain constant namespace | No |

**Required test**: `isinstance(codes.OK, int)` and `codes.OK == 200` both work; `isinstance(codes, IntEnum)` is False (intentional).

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
- Classification: 89 must-close, 61 intentional, 0 deferred
- Phase 2 owns 34 must-close entries (object/config contracts)
- Phase 3 owns 55 must-close entries (signatures, transport inheritance, streams)
- Phase 4 and Phase 5 have no must-close entries from the API oracle (their scope is native/transport features not captured by the Python API manifest)
- No runtime, dependency, CI, or release changes were introduced
