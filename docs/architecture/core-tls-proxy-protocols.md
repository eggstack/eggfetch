# TLS, Proxy & Protocols Deep Dive

This document covers TLS configuration, HTTP proxy support, HTTP/2, and HTTP/3.

See also: [overview.md](overview.md), [core-engine.md](core-engine.md).

## TLS Configuration

### TlsConfig

Builder-pattern configuration for TLS behavior:

```rust
let tls = TlsConfig::builder()
    .trust_store(TrustStore::from_pem_file("ca-bundle.pem")?)
    .client_identity(ClientIdentity::new("cert.pem", "key.pem")?)
    .min_version(TlsVersion::Tls12)
    .danger_accept_invalid_certs(false)
    .build()?;
```

### Trust Store Hierarchy

Resolution order:
1. Custom `TrustStore` (if provided via `TlsConfig`)
2. Operating system native roots (if available)
3. Packaged Mozilla/WebPKI roots (fallback for minimal containers)

A custom CA bundle **replaces** all default roots. If both system and private CAs are needed, concatenate them into a single PEM file.

### Client Identity (mTLS)

`ClientIdentity` holds a certificate chain and private key for mutual TLS. Supports PEM-encoded chains. Unencrypted PEM private keys are supported; encrypted keys produce an error at construction time.

### Verification Toggle

`TlsConfigBuilder::danger_accept_invalid_certs(true)` disables certificate verification. This is a deliberate escape hatch with documentation warnings.

### Version Policy

`TlsVersion` configures minimum and maximum supported TLS versions (e.g., TLS 1.2, TLS 1.3). QUIC mandates TLS 1.3.

## HTTP Proxy

### Configuration

`Proxy` supports HTTP forwarding and HTTPS CONNECT tunneling:

```rust
let proxy = Proxy::http("http://proxy.example.com:8080")?;
let client = ClientBuilder::new().proxy(proxy).build()?;
```

Per-request override via `RequestBuilder::proxy(ProxyOverride)`:
- `Inherit` — use client proxy (default).
- `Direct` — bypass proxy.
- `Override(proxy)` — use different proxy.

### Proxy-Only Headers

`Proxy::proxy_headers(headers)` attaches headers that are sent to the
proxy endpoint on forward-proxy and CONNECT requests but are never
forwarded into the tunnel or to the origin server:

```rust
let mut headers = Headers::new();
headers.insert("X-Custom-Proxy", "value")?;
let proxy = Proxy::http("http://proxy:8080")?
    .proxy_headers(headers);
```

Proxy-only headers are applied as follows:
- **HTTP forward proxy**: headers appear on the absolute-form request
  to the proxy.  Duplicate names from the origin request are not
  repeated.
- **CONNECT tunnel**: headers appear on the `CONNECT` request to the
  proxy.  After tunnel establishment, the origin request uses only the
  origin header set.
- **SOCKS proxy**: proxy headers are not forwarded (matching HTTPX 0.28.1
  reference behavior).

### Proxy Authentication

`ProxyAuth` supports Basic and Bearer authentication for proxy connections.

### NO_PROXY

`NoProxy` bypass rules support:
- Wildcard (`*` — bypass all)
- host/domain and host:port entries
- IPv4/IPv6 literals and CIDR networks

The Rust core does not read proxy environment variables. The HTTPX Python
compatibility facade may translate `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`,
and `NO_PROXY` into explicit scheme-aware native proxy configuration when
`trust_env=True`; native Rust callers must configure proxies explicitly. The
facade follows HTTPX's `urllib.request` environment precedence and matching:
lowercase names win, scheme-less proxy values are treated as HTTP URLs, and
`localhost` is an exact hostname rule rather than an implicit loopback alias.
Scheme-qualified exclusions constrain scheme and optional port; HTTPX-compatible
bare unbracketed IPv6 is recognized without treating its final colon as a port
separator. Bracketed IPv6 and IPv6 prefix-looking environment values are
rejected before native routing because HTTPX 0.28.1 rejects the corresponding
URL-pattern forms. CIDR-looking IPv4 entries retain HTTPX's exact URL-pattern
host behavior rather than native Rust subnet matching. Native
`NoProxy::parse()` retains bracketed IPv6 and true CIDR support. For ordinary bare domains, the compatibility parser matches
the bare host and subdomains only at a label boundary; a leading-dot entry
matches subdomains but not the bare host. Explicit host ports require an
explicit normalized target port, so an entry such as `example.test:80` does
not bypass an HTTP URL whose default port is omitted. These compatibility rules
are separate from native `NoProxy::parse()`, which retains true CIDR and native
default-port semantics.

HTTP proxy endpoints may use `http://` or `https://`. An HTTPS endpoint first
verifies the proxy hostname over TLS, then uses the existing absolute-form
forwarding or CONNECT path. For HTTPS origins, origin TLS is layered after
CONNECT and uses the origin hostname independently.

**Proxy endpoint TLS is independent from origin TLS**.  The
`proxy_tls_config` is sourced exclusively from the proxy configuration;
the origin `TlsConfig` is never reused as a fallback for the proxy
handshake.  This means an origin `verify=False`, a custom origin CA
bundle, an origin mTLS client identity, an origin SNI override, and
the origin TLS version policy are not propagated to the proxy
endpoint.  Callers that need a specific trust anchor for the proxy
must configure it explicitly via `Proxy(ssl_context=...)`; otherwise
the proxy endpoint is verified using rustls' default trust anchors
(system roots).

Proxy setup maps HTTPX's phase timeouts directly: proxy TCP/TLS and origin TLS
use `connect`, CONNECT writes and tunneled request writes use `write`, and
CONNECT/response headers use `read`. A native `total` timeout remains an
optional monotonic outer deadline; the compatibility facade does not synthesize
one from HTTPX's scalar timeout.

The Python compatibility facade accepts and stores HTTPX proxy metadata,
forwards `Proxy(headers=...)` through the native proxy-leg header
channel, and redacts the values of `authorization`,
`proxy-authorization`, `cookie`, and `set-cookie` in `Proxy.__repr__` and
`Headers.__repr__` so credentials never appear in diagnostic dumps.  The
raw values remain available to protocol code through `Proxy.headers` and
to engine code through the native API.  Ordinary three-element socket
options accept integer, `bytes`, and `bytearray` values; the arbitrary
four-element null-pointer form remains intentionally bounded out.

### CONNECT Tunnel

For HTTPS through a proxy, the transport establishes a CONNECT tunnel:
1. Send `CONNECT host:port HTTP/1.1` to the proxy.
2. Read the 200 response.
3. Upgrade the connection to TLS.
4. Send the actual HTTP request over the TLS tunnel.

## SOCKS5 Proxy

### Supported Schemes

The native Rust API retains the useful `socks5://` (local resolution) versus
`socks5h://` (proxy-side resolution) distinction. The HTTPX 0.28.1
compatibility facade normalizes both schemes to HTTPX/httpcore's observed wire
behavior: hostnames are sent as `ATYP_DOMAIN`, while IPv4 and IPv6 literals
are sent as their corresponding literal address types.

### Configuration

```rust
let proxy = Proxy::all("socks5://proxy.example.com:1080")?;
let client = ClientBuilder::new().proxy(proxy).build()?;
```

With authentication:
```rust
let proxy = Proxy::all("socks5://user:pass@proxy.example.com:1080")?;
```

### Protocol Flow

1. TCP connect to SOCKS5 proxy
2. Method negotiation (exactly no-auth without credentials, or exactly
   username/password with credentials)
3. Optional username/password subnegotiation (RFC 1929)
4. CONNECT command with destination address (IPv4, IPv6, or domain name)
5. Parse reply — tunnel established
6. For HTTP: speak origin-form HTTP over the tunnel
7. For HTTPS: perform origin TLS handshake over the tunnel, then HTTP

### DNS Resolution Semantics

The compatibility path is reference-driven rather than inferred from the
scheme name. HTTPX 0.28.1 sends hostname destinations as `ATYP_DOMAIN` for
both accepted SOCKS schemes, never substitutes loopback for an unresolved
name, and sends IP literals unchanged.

### Authentication

Supports username/password authentication via URL userinfo (`socks5://user:pass@host:port`) or explicit `.auth()` on the Proxy builder. Credentials are never exposed in Debug/Display output.

### Pool Isolation

The Rust client owns persistent Hyper SOCKS clients in a per-client cache
keyed by proxy endpoint, scheme, and authentication identity. This keeps the
SOCKS handshake and HTTP connection pool alive across compatible requests,
while different endpoints or credentials remain isolated. Credentials are
held only in the opaque internal key and are not included in debug/display
output.

## HTTP/2

### Feature Gating

Behind the `http2` Cargo feature. When not enabled, `Http2Only` and `Auto` silently downgrade to `Http1Only`.

### Version Policy

`HttpVersionPolicy` enum:
- `Http1Only` — only HTTP/1.1.
- `Http2Only` — only HTTP/2 (fails if server does not negotiate). Enforced at both the ALPN layer (only `h2` is advertised) and at the hyper-util legacy client layer (`http2_only(true)`).
- `Auto` (default) — both `h2` and `http/1.1` advertised.

### ALPN

ALPN protocols are set on the rustls configuration based on the version policy. The connector builder handles `enable_http1()` / `enable_http2()`:

- `enable_http2()` alone → `alpn_protocols = vec![b"h2"]` (h2-only).
- `enable_http1().enable_http2()` → `alpn_protocols = vec![b"h2", b"http/1.1"]` (auto).
- `enable_http1()` alone → ALPN stays empty (h1-only).

The standard hyper-rustls path passes an empty ALPN list to `hyper_rustls::HttpsConnectorBuilder` so the builder can populate it from the `enable_http1`/`enable_http2` calls. Direct and UDS connectors perform their own TLS handshake; the ALPN they advertise is determined by the `TlsConfig` and is shared across the three paths.

### `http2_only` enforcement

For `HttpVersionPolicy::Http2Only`, the legacy hyper-util client is built with `http2_only(true)`. This is what enforces the protocol contract:

- For **TLS** connections, `http2_only(true)` causes hyper-util to attempt an HTTP/2 handshake on every accepted socket. When ALPN does not negotiate `h2` (or the server only advertises `http/1.1`), the H2 handshake fails and the request is rejected with a `RequestError` / `ConnectError`. There is no silent downgrade to HTTP/1.1.
- For **cleartext** connections, `http2_only(true)` causes hyper-util to send the H2 client preface directly over the TCP socket. This is HTTP/2 prior knowledge (h2c) and matches HTTPX's behavior on cleartext H2 servers.

Both the standard hyper-rustls client and the direct / UDS clients are configured with `http2_only(true)` when H2-only is selected. The `Http1Only` and `Auto` policies do not set `http2_only`.

### Direct connector ALPN signaling

`DirectConnector` and `UdsConnector` wrap their own TLS handshake (via `tokio_rustls::TlsConnector`) and signal the negotiated ALPN protocol back to hyper-util so the legacy client knows whether the connection is H2:

```rust
impl Connection for DirectStream {
    fn connected(&self) -> hyper_util::client::legacy::connect::Connected {
        let mut connected = Connected::new();
        if let Self::Tls(tls) = self {
            if let Some(b"h2") = tls.get_ref().1.alpn_protocol() {
                connected = connected.negotiated_h2();
            }
        }
        connected
    }
}
```

Without this signal, an H2-only legacy client would not see the ALPN result and could incorrectly attempt H1 framing on a connection that just negotiated h2.

### Forbidden Header Stripping

Per RFC 9113 §8.2.2, the pipeline strips before sending: `Connection`, `Keep-Alive`, `Proxy-Connection`, `Transfer-Encoding`, `Upgrade`, and `TE` (except `trailers`).

### Error Taxonomy

| Error | Meaning |
|-------|---------|
| `Http2GoAway` | Server sent GOAWAY frame |
| `Http2StreamReset` | Stream reset via RST_STREAM |
| `Http2FlowControl` | Flow-control error |
| `Http2Protocol` | Generic HTTP/2 protocol error |

The H2 classification function in `crates/eggfetch-core/src/transport/direct.rs` maps hyper error strings to these variants. `last_stream_id` is currently hardcoded to `0` because hyper does not expose the inner h2 error's stream identifier (see the `stream_id` residual in `docs/residual-differences.md`).

### Retry Classification

`REFUSED_STREAM` is retryable for replayable requests (RFC 9113 §7.2.4). `CANCEL`, `GOAWAY`, flow-control, and protocol errors are not retried.

### Pool Interaction

HTTP/2 multiplexes streams on a single connection, but eggfetch's pool permits still control logical request concurrency. hyper handles connection-level multiplexing; the server's `SETTINGS_MAX_CONCURRENT_STREAMS` is respected internally.

## HTTP/3

### Feature Gating

Behind the `http3` Cargo feature. Experimental — API surfaces may change.

### Transport

Uses `quinn` for QUIC transport and `h3` for the HTTP/3 protocol layer. QUIC mandates TLS 1.3; 0-RTT is disabled in the initial implementation.

### Version Policy

`Http3Only` variant is available when the `http3` feature is enabled. `Auto` does not automatically negotiate HTTP/3 — callers must explicitly select `Http3Only`.

### Error Taxonomy

| Error | Meaning |
|-------|---------|
| `H3Connect` | QUIC connection failed |
| `H3ConnectionClosed` | Peer closed the connection |
| `H3Stream` | Stream error |
| `H3Protocol` | HTTP/3 protocol error |

### Python API

```python
client = eggfetch.Client(http3=True)
r = client.get("https://example.com")  # Uses QUIC
```
