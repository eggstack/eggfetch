# HTTPX 1.0 preview redesign notes (`1.0.dev6`, 2026-08-31)

Preview only. No parity claim. No implementation from dev releases.

Source: pinned `httpx==1.0.dev6` wheel (`37 KiB`; stable `0.28.1` is `72 KiB`),
`reference-api.json` (36 symbols, generated with the shared manifest tooling),
and `preview-delta-0.28.1-vs-1.0.dev6.json` (192 differences vs `0.28.1`,
empty allowed set, ungated). Upstream tagline: "An HTTP toolkit ... Supports
both client and server functionality ... tightly constrained API ... minimal
footprint" (only `truststore` dependency).

Impact values: `reuse` (EggFetch already has an equivalent native
capability), `adapter` (compatibility-only facade work if a future stable
requires it), `engine` (real core work if a future stable requires it),
`not-applicable` (outside EggFetch's client scope), `unknown` (upstream still
churning; cannot classify yet).

## Scope headline

The preview is a full redesign, not an incremental `0.28.1` evolution:
`0.28.1` exposes 69 top-level symbols; `1.0.dev6` exposes 36, of which 26 are
new and 59 `0.28.1` symbols are gone. `Client.__init__` collapses from 20
parameters (`auth`, `params`, `cookies`, `verify`, `cert`, `trust_env`,
`http1`, `http2`, `proxy`, `mounts`, `timeout`, `follow_redirects`, `limits`,
`max_redirects`, `event_hooks`, `base_url`, `transport`, `default_encoding`)
to 3 (`url`, `headers`, `transport`). `Request` collapses to
`(method, url, headers, content)`; `Response` to
`(status_code, *, headers, content)`.

## Bucket classification

| Redesign area | What changed in `1.0.dev6` | Impact | Note |
|---|---|---|---|
| client vs toolkit/server scope | New `Server(app, host, port)` + `handle_stream`/`serve`; `NetworkBackend.listen`/`serve`; `open_connection`; content helpers (`HTML`, `JSON`, `Form`, `File`, `Files`, `MultiPart`); `HTTPParser`; `quote`/`unquote`/`urlencode`/`urldecode` | `not-applicable` | EggFetch is a client engine. Server/toolkit scope is an explicit non-goal unless a future stable contract makes it strategically desirable. No work. |
| sync/async API organization | `AsyncClient`, `AsyncByteStream`, `AsyncBaseTransport`, `AsyncHTTPTransport` gone; single sync `Client` with `get`/`put`/`post`/`patch`/`delete`/`request`/`stream`/`build_request`/`close` (no `head`/`options`/`send` methods on the class in dev6); `timeout()` is now a sync context-manager function | `unknown` | Single-client organization may be a dev-stage simplification. `__all__` also lists `run`/`serve_http`/`StatusCode`, which do not exist as attributes (broken dev exports). Cannot classify until an RC freezes the model. No work. |
| transport customization | New `Transport` / `Connection` / `ConnectionPool(backend)` / `NetworkBackend(ssl_ctx)` / `NetworkStream` / `DuplexStream` / `Stream` model; `BaseTransport`, `HTTPTransport`, `MockTransport`, `WSGITransport`, `ASGITransport`, mounts gone | `adapter` | If a stable 1.0 keeps this model, mapping belongs in the version-specific facade over the single Rust engine (the same bridge pattern as `MockTransport` today). No second networking stack. No work now. |
| request/response/URL/header models | `Request`/`Response` collapsed (above); `URL` gains `copy_with`/`copy_set_param`/`copy_append_param`/`copy_remove_param`/`copy_merge_params`/`join` + `target`/`query`/`params` properties; `Headers` becomes immutable (`Mapping`, `copy_set`/`copy_update`/`copy_remove`, no `encoding`/`raw`, no mutating `clear`/`pop`/`get_list`/`multi_items`); `QueryParams` gains `copy_*` + `multi_dict`; new `Method` type | `adapter` | Value-object reshaping is facade work. Core URL/headers already cover the semantics. No work now. |
| streaming and upgrade/network-stream semantics | New `Stream`/`DuplexStream`/`ByteStream(data)`/`FileStream`/`MultiPartStream` + `Content` hierarchy (`Content`, `Binary`, `Text`, `HTML`, `JSON`, `Form`, `File`, `Files`, `MultiPart`, `Empty`) with `open`/`parse`/`headers`; `SyncByteStream`/`AsyncByteStream` gone; `ByteStream.__init__` is `(data)` not `(stream)` | `reuse` | EggFetch core already streams without eager buffering and owns the 101 `network_stream` lifecycle. Any stable streaming shape maps onto that engine. No work now. |
| TLS/proxy configuration | `verify`, `cert`, `trust_env`, `proxy`, `create_ssl_context`, `truststore`-via-facade gone; `NetworkBackend(ssl_ctx)` takes a raw `ssl.SSLContext`; no proxy surface in dev6 | `reuse` | Core `TlsConfig`/proxy/CONNECT/SOCKS already exceed the preview surface. A stable TLS/proxy model would reuse them. No work now. |
| timeout/limits semantics | `Timeout` class and `Limits` gone; `timeout(duration)` is a context manager; `PoolTimeout`/`ConnectTimeout`/`ReadTimeout`/`WriteTimeout` gone | `reuse` | Core phase-aware timeouts + logical pool limits already cover this space. No work now. |
| auth/cookies/redirects | `Auth`, `BasicAuth`, `DigestAuth`, `NetRCAuth`, `Cookies`, `CookieConflict`, `TooManyRedirects`, top-level `codes`, `USE_CLIENT_DEFAULT` gone; no auth/cookie/redirect params on `Client` | `reuse` | Core auth/cookie-jar/redirect engine already implements the removed semantics. No work now. |
| optional protocol helpers | `quote`/`unquote`/`urlencode`/`urldecode`, `HTTPParser`, `HTML`/`JSON`/`Text`/`Binary` content types are new top-level surface | `not-applicable` | URL-codec helpers duplicate the standard library; content helpers are facade-level constructors. No engine work now or at stable unless the contract requires them. |
| exception taxonomy | 20 exception symbols gone (`HTTPError`, `RequestError`, `TransportError`, `TimeoutException`, `NetworkError`, `ProtocolError` stays, `ProxyError`, `DecodingError`, stream errors, etc.); only preview `ProtocolError` (parsers) remains | `adapter` | If a stable 1.0 ships a new taxonomy, the facade remaps native errors to it (same pattern as today). No work now. |
| public/private module boundary | 33.5 KiB footprint, 2 dependencies (`truststore` only); `__all__` lists 36 names but 3 (`StatusCode`, `run`, `serve_http`) raise `AttributeError` on access in dev6 | `unknown` | Packaging/minimalism is upstream's concern. Broken dev exports confirm the API is still churning; packaging deltas are ignored unless they change the public contract. No work. |

## Removed / reintroduced concepts

- Removed (59): all async variants, all auth/cookie/proxy/timeout/limit config,
  all transports/mounts, full exception taxonomy, top-level helpers
  (`get`/`post`/`put`/`patch`/`delete`/`head`/`options`/`request`/`stream`),
  `main`, `codes`, `create_ssl_context`, `USE_CLIENT_DEFAULT`.
- Added (26): `Server`, `Connection`, `ConnectionPool`, `Transport`,
  `NetworkBackend`, `NetworkStream`, `DuplexStream`, `Stream`, `FileStream`,
  content hierarchy (`Content`, `Binary`, `Text`, `HTML`, `JSON`, `Form`,
  `File`, `Files`, `MultiPart`), `Method`, `HTTPParser`, `open_connection`,
  `timeout` (function), `quote`/`unquote`/`urlencode`/`urldecode`.
- Redesigned in place (7 signature groups + 52 member diffs): `Client`,
  `Request`, `Response`, `URL`, `Headers`, `QueryParams`, `ByteStream`.
- Nothing removed in dev6 has been reintroduced yet (no RC). Reintroduction
  tracking starts at the RC/stable trigger.

## Why no implementation

- Dev exports are broken (`StatusCode`, `run`, `serve_http` in `__all__` but
  missing), signatures are still collapsing, and whole subsystems (auth,
  cookies, proxy, timeouts, async) are absent rather than stabilized.
- Per the plan, no implementation work starts solely because a dev release
  temporarily exposes an API. The trigger is an RC with a frozen public API
  or a stable 1.0 plus release notes confirming no further reset.
