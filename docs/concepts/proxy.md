# Proxy Support

eggfetch supports HTTP forward proxying and HTTPS CONNECT tunneling with proxy authentication and NO_PROXY bypass rules.

CONNECT wire bytes (authority formatting, request serialization, bounded response-head parsing) live in the small `eggfetch-http-connect` crate, owned by the `proxy` feature and absent from non-proxy profiles; `eggfetch-core` owns dialing, TLS, timeouts, status policy, rejection bodies, and pooling around it. See `docs/architecture/core-tls-proxy-protocols.md` § "CONNECT Tunnel".

## HTTP Proxying vs HTTPS Tunneling

- **HTTP targets**: The request is forwarded through the proxy. The proxy sees the full URL, headers, and body.
- **HTTPS targets**: A CONNECT tunnel is established through the proxy. The tunnel is a transparent byte stream; the proxy cannot inspect the encrypted traffic.

Both modes use the same `Proxy` configuration. The transport layer selects the appropriate mode based on the target URL's scheme.

## Pooling and total deadlines

Compatible HTTP forward-proxy requests and single-target HTTPS CONNECT
requests reuse bounded Hyper clients and their keep-alive connections. Cache
identity contains only connection-affecting proxy, origin, TLS, protocol, and
pinning policy; a request's logical `Timeout.total` is never part of the key or
stored in the reusable connector.

The pipeline computes the remaining total budget for each request, including
redirects and retries, and applies it around that request's proxy dispatch.
Consequently, a fresh connection or tunnel created after a stale pooled
connection is detected uses the current request's budget. Multi-target CONNECT
fallback remains request-scoped because candidate retry and body replay need
the current deadline.

## Proxy Configuration

### Client-Level

```rust
use eggfetch_core::Proxy;

let proxy = Proxy::all("http://proxy.example:8080")?;
let client = Client::builder()
    .proxy(proxy)
    .build();
```

### Physical route pinning (native Rust)

Native callers may pin the physical proxy peer without changing the logical
proxy URI. The addresses are attempted in order, never trigger proxy-host
DNS, and are retained across retries. The proxy URI remains authoritative for
proxy matching, credentials, and HTTPS-proxy TLS SNI/certificate verification.

```rust
let proxy = Proxy::all("https://proxy.example:8443")?
    .resolved_addresses(["198.51.100.20:8443".parse()?])?;
```

For a request through a proxy, `proxy_target_addresses()` separately pins the
physical ultimate destination:

```rust
let response = client
    .get("https://service.example/data")?
    .proxy_target_addresses(["203.0.113.10:443".parse()?])
    .send()
    .await?;
```

The logical origin URL remains authoritative for HTTP `Host`, TLS SNI and
certificate verification. Pinned targets are supported for HTTPS CONNECT and
local-resolution `socks5://`; SOCKS5H remote DNS and plaintext HTTP
forward-proxy requests reject this option before opening the proxy. The
caller is responsible for validating supplied addresses. Direct
`resolved_addresses()` remains a separate direct-only API, and a complete
physical route requires pinning both the proxy peer and proxied target.

For multiple pinned local-SOCKS5 targets, only replies 0x03 (network
unreachable), 0x04 (host unreachable), and 0x05 (connection refused) advance
to the next candidate; proxy-wide authentication, policy, protocol, and
malformed-response failures stop.

```python
client = eggfetch.Client(proxy="http://proxy.example:8080")
```

### Per-Request Override

```rust
let proxy = Proxy::http("http://other-proxy:3128")?;
let response = client
    .get("https://example.com")
    .proxy(proxy)
    .send()
    .await?;
```

```python
response = client.get(url, proxy="http://other-proxy:3128")
```

### Disabling Proxy Per-Request

```python
response = client.get(url, proxy=None)
```

## Routing Rules

The `Proxy` type supports three routing rules:

- `Proxy::all(url)` -- route all requests through the proxy
- `Proxy::http(url)` -- route only HTTP requests through the proxy
- `Proxy::https(url)` -- route only HTTPS requests through the proxy

## Proxy Authentication

Proxy auth uses HTTP Basic authentication. Credentials are sent to the proxy only, never forwarded to the destination.

```rust
use eggfetch_core::{Proxy, ProxyAuth};

let auth = ProxyAuth::basic("proxyuser", "proxypass")?;
let proxy = Proxy::all("http://proxy:8080")?.auth(auth);
```

```python
client = eggfetch.Client(
    proxy="http://proxyuser:proxypass@proxy:8080",
)
```

The native Python `proxy=` string is parsed with HTTPX-compatible userinfo
handling (credentials become proxy auth). Strict Rust `Proxy::all()` rejects
URL userinfo; use `.auth(ProxyAuth::basic(...))` or `Proxy::all_compat()`
there instead.

Proxy passwords are redacted in `Debug`, `Display`, logs, and error messages. Credentials in proxy URLs are rejected.

## NO_PROXY Bypass

`NoProxy` defines bypass rules for the proxy. When a URL matches any rule, the request is sent directly without going through the proxy.

```rust
use eggfetch_core::{NoProxy, Proxy};

let no_proxy = NoProxy::parse("localhost, .example.com, 10.0.0.1:8080")?;
let proxy = Proxy::all("http://proxy:8080")?.no_proxy(no_proxy);
```

Supported entry formats:

| Entry | Behavior |
|-------|----------|
| `*` | Wildcard, matches everything |
| `localhost` | Matches `localhost`, `127.0.0.1`, `[::1]` |
| `.example.com` | Domain suffix match (matches `example.com` and subdomains) |
| `example.com` | Exact host match |
| `example.com:8080` | Host + port match |
| `[::1]` | IPv6 literal |

Bypass rules are case-insensitive for host matching. Port matching uses the scheme's default port when the URL has no explicit port.

## Environment Policy

The Rust core does not read proxy environment variables. The HTTPX
compatibility facade may translate scheme-specific proxy variables,
`ALL_PROXY`, lowercase forms, and `NO_PROXY` into explicit native
configuration when `trust_env=True`.

## Limitations

- Pinned proxy peers and proxied targets are immutable route snapshots. They
  survive retries and same-origin redirects, never fall back to DNS, and are
  rejected on cross-origin redirects. CONNECT target fallback is conservative
  and only cycles on typed 502/504 target rejection; SOCKS5 target fallback
  cycles only on destination-specific network/host-unreachable or connection-
  refused replies, never on proxy authentication or policy failures.
- The compatibility facade accepts URL credentials for HTTP/HTTPS and SOCKS5 endpoints and redacts them from display/error output; native Rust callers use `.auth()` for explicit credentials.

## CLI

```bash
# Set proxy
eggfetch --proxy http://proxy:8080 https://example.com

# Proxy auth
eggfetch --proxy http://proxy:8080 --proxy-auth user:pass https://example.com

# Bypass proxy for specific hosts
eggfetch --no-proxy "localhost, .example.com" --proxy http://proxy:8080 https://example.com
```
