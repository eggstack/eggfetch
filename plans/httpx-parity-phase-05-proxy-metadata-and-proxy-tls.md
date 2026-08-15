# HTTPX Parity Phase 05 — Proxy Metadata and Proxy TLS Fidelity

Status: implementation handoff plan.
Depends on: Phase 01 safe TLS-context translation.
Recommended order: after Phase 04 so CONNECT ownership and transport metadata are no longer moving underneath this work.

## Objective

Close the remaining HTTPX 0.28.1 proxy-object differences that affect actual network behavior:

- `Proxy(headers=...)` must be carried on the proxy leg rather than rejected;
- proxy URL authentication and explicit proxy headers must have HTTPX-compatible precedence and serialization;
- `Proxy(ssl_context=...)` must control TLS to an HTTPS proxy endpoint independently from TLS to the origin;
- representable proxy SSL contexts should use Phase 01’s safe translation boundary;
- unrepresentable arbitrary Python proxy contexts must fail closed rather than silently use origin/default TLS settings.

The central security property is separation: **proxy credentials, proxy-only headers, and proxy-specific trust policy must never contaminate the origin request or origin TLS session.**

## Reference behavior

HTTPX 0.28.1 normalizes `Proxy` as:

```python
Proxy(
    url,
    *,
    ssl_context=None,
    auth=None,
    headers=None,
)
```

Reference behavior from `httpx/_config.py` and `_transports/default.py`:

- URL credentials are removed from the proxy URL and stored as proxy authentication;
- `headers` are normalized through HTTPX `Headers`, preserving the header representation expected by httpcore;
- for `http://` or `https://` proxy endpoints, HTTPX passes:
  - `proxy_auth` separately;
  - `proxy_headers=proxy.headers.raw`;
  - `proxy_ssl_context=proxy.ssl_context`;
  - origin `ssl_context` separately;
- the proxy SSL context is used only when the connection to the proxy itself is TLS (`https://proxy...`);
- after CONNECT, the origin TLS handshake uses the origin SSL context, not the proxy SSL context;
- SOCKS transports receive proxy auth and origin SSL context, but HTTPX 0.28.1 does not pass `proxy_headers` or `proxy_ssl_context` into the SOCKS transport constructor.

The pinned reference therefore has two distinct TLS trust domains for HTTPS-via-HTTPS-proxy:

1. client → proxy TLS, governed by `proxy.ssl_context`;
2. tunnel → origin TLS, governed by client/transport `verify`/origin SSL context.

## Current EggFetch state

The compatibility `Proxy` object stores URL, headers, auth, and `ssl_context`, but `_convert_proxy()` rejects non-empty `Proxy(headers=...)` before native dispatch.

The native proxy path already has strong separation points:

- `ProxyConfig` identifies proxy endpoint/auth/routing;
- `transport/proxy.rs::connect_to_proxy()` owns TCP and optional TLS to the proxy endpoint;
- `transport/proxy.rs::write_proxy_request()` writes forward-proxy requests and explicitly prevents destination `Proxy-Authorization` from being copied;
- `transport/connect.rs` constructs the CONNECT request independently, then performs a separate origin TLS handshake and sends the origin-form HTTP request with `proxy_auth=None`.

However, `ProxyRequestContext` currently contains only one `tls_config`, and `connect_to_proxy()` uses that same context for TLS to an HTTPS proxy endpoint. This must be split before proxy-specific SSL context semantics can be correct.

## Required implementation tracks

### Track 1 — Normalize compatibility `Proxy.headers` exactly

Refactor `eggfetch.compat.httpx._proxy.Proxy` so `headers=` accepts the same public header input forms HTTPX 0.28.1 accepts through its `Headers` type, rather than only a plain `dict`.

Requirements:

- mapping inputs;
- sequence-of-two-tuples inputs;
- bytes/string header names and values according to the existing compatibility `Headers` rules;
- duplicate proxy headers preserved where HTTPX preserves them;
- repr masking behavior remains reference-compatible for credentials;
- no headers are applied merely because they exist on the object; dispatch decides where they belong.

Add constructor/API-oracle differential tests before wiring transport behavior.

### Track 2 — Add typed proxy headers to the native proxy configuration

Extend `ProxyConfig` with a dedicated proxy-header collection.

Do not reuse ordinary request headers as storage. The type should make proxy-only intent explicit.

Properties:

- duplicate-preserving;
- header names/values validated using the same safe HTTP header primitives as the rest of core;
- `Debug`/`Display` redact sensitive values;
- `Proxy-Authorization`, `Cookie`, bearer/API-key-like values, and other configured sensitive names must not leak through diagnostics;
- cloning a proxy config preserves headers without broadening visibility.

Consider whether the proxy route/cache identity needs a stable hash/fingerprint of headers. If a pooled proxy connection is established under one configuration and then reused for a second logically distinct `Proxy` with different connection-establishment metadata, the cache must not accidentally cross those configurations.

At minimum, the effective route identity must distinguish differences that affect CONNECT/proxy authentication/TLS establishment.

### Track 3 — Define proxy-auth vs explicit-header precedence from the reference

Create pinned-reference tests before choosing precedence.

Cases to resolve explicitly:

- credentials embedded in proxy URL only;
- `Proxy(auth=(...))` only;
- both URL auth and explicit `auth`;
- explicit `Proxy-Authorization` header only;
- configured auth plus explicit `Proxy-Authorization` header;
- duplicate proxy authorization headers;
- non-Basic arbitrary proxy auth header values.

Then implement the exact HTTPX/httpcore 1.0.9 behavior.

Do not automatically append a second `Proxy-Authorization` header if reference behavior replaces/merges differently.

### Track 4 — Apply proxy headers to HTTP forward-proxy requests

Refactor `write_proxy_request()` or introduce a proxy-request-head builder with two header channels:

1. origin/request headers;
2. proxy-only headers.

For an HTTP target sent through an HTTP/HTTPS forward proxy:

- serialize the absolute-form request target as today;
- include normal origin headers according to current behavior;
- add proxy headers exactly as HTTPX supplies them to the proxy connection/request;
- apply proxy authentication according to Track 3;
- prevent any accidental duplicate generation from destination headers.

Important semantic distinction: a forward HTTP proxy receives the entire request and may choose to forward arbitrary headers itself. EggFetch’s security obligation is that proxy metadata is not accidentally merged into a direct-origin path or into the post-CONNECT origin request. It cannot control how an external forward proxy handles headers intentionally sent to it.

### Track 5 — Apply proxy headers to CONNECT only, then strip at tunnel boundary

In `transport/connect.rs`, extend CONNECT request construction to include proxy headers.

Requirements:

- headers are on the CONNECT request to the proxy;
- proxy authentication precedence matches Track 3;
- after status 200 and tunnel establishment, the origin request must use only the origin header set;
- no proxy-only header may appear in the HTTP request sent inside the tunnel;
- no proxy-only header may become a default client header for later requests;
- redirect/retry paths reconstruct the correct proxy request independently per hop.

Add an origin fixture that records every header after CONNECT and a proxy fixture that records every CONNECT header. The test must prove separation, not merely successful status codes.

### Track 6 — Split proxy TLS configuration from origin TLS configuration

Refactor `ProxyRequestContext` so the two TLS roles are explicit, for example:

```text
origin_tls_config: Option<&TlsConfig>
proxy_tls_config: Option<&TlsConfig>
```

Names are illustrative.

Then:

- `connect_to_proxy()` uses only proxy TLS config for an `https://` proxy endpoint;
- CONNECT origin handshake uses only origin TLS config;
- HTTP target through HTTPS proxy uses proxy TLS config for client→proxy TLS and has no second TLS layer;
- HTTP proxy endpoint ignores proxy TLS config because there is no TLS-to-proxy layer, matching the reference’s effective behavior;
- SOCKS does not suddenly gain proxy SSL-context behavior that HTTPX 0.28.1 does not have.

This split must be reflected in pool/cache keys so connections established under different proxy trust contexts are never reused interchangeably.

### Track 7 — Translate `Proxy.ssl_context` through Phase 01 only

At the Python compatibility boundary:

- `None` means reference/default proxy TLS behavior;
- an exactly representable context becomes a native proxy `TlsConfig`;
- helper-created contexts with reconstructable metadata use the same safe registry/snapshot mechanism as Phase 01;
- an arbitrary context with unrepresentable security state fails before dispatch.

Do not use the origin context as a substitute for an unsupported proxy context. That changes trust policy and may create a false-success security bug.

### Track 8 — Verify default HTTPS-proxy trust behavior

Current `connect_to_proxy()` falls back to packaged WebPKI roots when no TLS config is supplied. Compare this with HTTPX 0.28.1’s actual default proxy TLS behavior in the pinned environment.

If HTTPX uses the transport-created default SSL context for proxy TLS when no `proxy.ssl_context` is given, EggFetch must determine whether its existing default trust policy is equivalent enough for the Stage C contract. Pay special attention to:

- system trust vs packaged roots;
- `SSL_CERT_FILE` / `SSL_CERT_DIR` and `trust_env` interaction;
- custom origin `verify` context not implicitly becoming proxy trust unless the reference does so;
- hostname verification against the proxy hostname.

Do not broaden trust roots simply to make fixtures pass.

### Track 9 — SOCKS behavior must remain reference-bounded

Create explicit differential tests for `Proxy(headers=...)` and `Proxy(ssl_context=...)` when the URL scheme is `socks5`/`socks5h`.

Because HTTPX 0.28.1’s SOCKS transport constructor does not receive proxy headers or proxy SSL context, match the actual reference outcome rather than applying the HTTP-proxy implementation universally.

If the reference accepts the object attributes but they have no SOCKS network effect, document and test that exact behavior.

### Track 10 — Failure, retry, and resource semantics

Proxy metadata must remain correct across:

- connect retries;
- request retries;
- redirects to a new origin;
- `NO_PROXY` bypass;
- per-request proxy disable/override where native API supports it;
- client close and async cancellation.

A bypassed request must not carry proxy-only headers to the origin.

### Track 11 — Close compatibility ledger entries

After behavioral differentials pass:

- move `PROXY-HEADERS-001` to `resolved-differences.toml` if fully resolved for its stated HTTP/HTTPS proxy scope;
- update or split any `Proxy.ssl_context` entry into resolved and residual forms;
- correct `docs/reference/compatibility.md` and diagnostics;
- add upstream-derived parity cases for proxy header forwarding and dual-TLS separation.

Do not delete a residual arbitrary-context difference merely because the representable subset works.

## Differential fixture matrix

Build local fixtures that expose both legs independently.

### HTTP forward proxy

Cases:

- one custom proxy header;
- duplicate custom proxy headers;
- custom proxy authorization header;
- configured proxy auth;
- auth/header precedence;
- origin headers with same names as proxy headers;
- request body streaming;
- redirect through same proxy;
- `NO_PROXY` bypass proving zero proxy-header leakage.

Compare the exact request head observed by the proxy against HTTPX 0.28.1.

### CONNECT proxy

Proxy captures CONNECT; origin captures tunneled request.

Assert:

- proxy headers present on CONNECT as reference requires;
- proxy auth present with correct precedence;
- proxy headers absent inside tunnel;
- origin Authorization/Cookie semantics unchanged;
- rejected CONNECT response does not expose secrets in exception text.

### HTTPS proxy endpoint

Use separate local CAs for proxy and origin.

Create a matrix where:

- proxy CA trusted / origin CA trusted → success;
- proxy CA untrusted / origin trusted → fail at proxy TLS;
- proxy trusted / origin untrusted → fail at origin TLS;
- proxy context trust differs from origin context trust → proves separation;
- proxy hostname mismatch → fail at proxy TLS;
- representable custom proxy context succeeds;
- unrepresentable context fails pre-dispatch.

### Sync/async and lifecycle

Run the same essential matrix for `HTTPTransport`/`AsyncHTTPTransport` and `Client`/`AsyncClient` routes that use native proxy dispatch.

Add cancellation during:

- TCP connect to proxy;
- TLS-to-proxy handshake;
- CONNECT wait;
- origin TLS handshake.

Verify permits/runtime leases/streams are released.

## Security requirements

- proxy-only headers never appear on direct or post-CONNECT origin requests due to EggFetch merging;
- `Proxy-Authorization` remains redacted in every error/debug path;
- custom proxy headers with secret-looking names/values follow the project’s redaction policy;
- proxy TLS trust and origin TLS trust are stored and keyed separately;
- no fallback from an unsupported proxy SSL context to an unrelated/default context;
- hostname verification remains enabled according to the chosen proxy context;
- no credentials are embedded into cache keys or logs in plaintext;
- header count/size limits remain bounded by existing request/proxy parsing limits;
- no `unsafe` additions.

## Non-goals

- adding arbitrary headers to SOCKS negotiation where HTTPX does not;
- implementing a generic Layer-7 proxy framework;
- changing `NO_PROXY` behavior closed by prior passes;
- replacing the proxy parser/transport wholesale;
- making unrepresentable arbitrary Python SSL contexts work through unsafe OpenSSL introspection.

## Acceptance criteria

This phase is complete when:

1. `Proxy(headers=...)` accepts HTTPX-compatible header inputs and no longer fails solely because headers are non-empty for supported HTTP/HTTPS proxy routes.
2. Forward-proxy wire captures match HTTPX 0.28.1 for proxy header/auth precedence cases.
3. CONNECT wire captures contain the expected proxy metadata, and tunneled origin captures contain none of that proxy-only metadata unless the application separately supplied the same header as an origin header.
4. `Proxy.ssl_context` for an HTTPS proxy is distinct from origin TLS configuration and is proven with separate proxy/origin CA fixtures.
5. Representable proxy SSL contexts work through the native rustls engine; unrepresentable contexts fail closed before dispatch.
6. HTTP and SOCKS proxy cases do not gain behavior the pinned reference does not have.
7. Proxy route/pool reuse does not cross materially different auth/header/TLS configurations.
8. Error paths redact proxy credentials and sensitive metadata.
9. Sync/async differential and cancellation tests pass.
10. `PROXY-HEADERS-001` and related active ledger entries are resolved or narrowed truthfully.
11. `./scripts/check.sh` passes.
