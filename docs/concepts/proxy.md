# Proxy Support

eggfetch supports HTTP forward proxying and HTTPS CONNECT tunneling with proxy authentication and NO_PROXY bypass rules.

## HTTP Proxying vs HTTPS Tunneling

- **HTTP targets**: The request is forwarded through the proxy. The proxy sees the full URL, headers, and body.
- **HTTPS targets**: A CONNECT tunnel is established through the proxy. The tunnel is a transparent byte stream; the proxy cannot inspect the encrypted traffic.

Both modes use the same `Proxy` configuration. The transport layer selects the appropriate mode based on the target URL's scheme.

## Proxy Configuration

### Client-Level

```rust
use eggfetch_core::Proxy;

let proxy = Proxy::all("http://proxy.example:8080")?;
let client = Client::builder()
    .proxy(proxy)
    .build();
```

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
response = client.get(url, proxy=eggfetch.NO_PROXY)
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
    proxy="http://proxy:8080",
    proxy_auth=("proxyuser", "proxypass"),
)
```

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

- Limited connection reuse through proxies (each request opens a fresh connection)
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
