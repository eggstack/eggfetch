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

### Proxy Authentication

`ProxyAuth` supports Basic and Bearer authentication for proxy connections.

### NO_PROXY

`NoProxy` bypass rules support:
- Wildcard (`*` — bypass all)
- `localhost` / `127.0.0.1`
- Domain suffix (`.example.com`)
- Host:port pairs
- IPv6 addresses

eggfetch does **not** read environment variables for proxy configuration.

### CONNECT Tunnel

For HTTPS through a proxy, the transport establishes a CONNECT tunnel:
1. Send `CONNECT host:port HTTP/1.1` to the proxy.
2. Read the 200 response.
3. Upgrade the connection to TLS.
4. Send the actual HTTP request over the TLS tunnel.

## SOCKS5 Proxy

### Supported Schemes

- `socks5://` — SOCKS5 with local DNS resolution (client resolves the hostname, sends IP to proxy)
- `socks5h://` — SOCKS5 with remote DNS resolution (sends hostname to proxy for resolution)

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
2. Method negotiation (no-auth or username/password)
3. Optional username/password subnegotiation (RFC 1929)
4. CONNECT command with destination address (IPv4, IPv6, or domain name)
5. Parse reply — tunnel established
6. For HTTP: speak HTTP over the tunnel
7. For HTTPS: perform origin TLS handshake over the tunnel, then HTTP

### DNS Resolution Semantics

- `socks5://`: Resolves the destination hostname locally, sends the IP address to the proxy (ATYP_IPV4 or ATYP_IPV6)
- `socks5h://`: Sends the domain name to the proxy for remote resolution (ATYP_DOMAIN)

### Authentication

Supports username/password authentication via URL userinfo (`socks5://user:pass@host:port`) or explicit `.auth()` on the Proxy builder. Credentials are never exposed in Debug/Display output.

### Pool Isolation

SOCKS proxy connections are keyed separately from direct and HTTP proxy connections. Different SOCKS proxies or different credential sets create independent pool slots.

## HTTP/2

### Feature Gating

Behind the `http2` Cargo feature. When not enabled, `Http2Only` and `Auto` silently downgrade to `Http1Only`.

### Version Policy

`HttpVersionPolicy` enum:
- `Http1Only` — only HTTP/1.1.
- `Http2Only` — only HTTP/2 (fails if server doesn't negotiate).
- `Auto` (default) — both `h2` and `http/1.1` advertised.

### ALPN

ALPN protocols are set on the rustls configuration based on the version policy. The connector builder handles `enable_http1()` / `enable_http2()`.

### Forbidden Header Stripping

Per RFC 9113 §8.2.2, the pipeline strips before sending: `Connection`, `Keep-Alive`, `Proxy-Connection`, `Transfer-Encoding`, `Upgrade`, and `TE` (except `trailers`).

### Error Taxonomy

| Error | Meaning |
|-------|---------|
| `Http2GoAway` | Server sent GOAWAY frame |
| `Http2StreamReset` | Stream reset via RST_STREAM |
| `Http2FlowControl` | Flow-control error |
| `Http2Protocol` | Generic HTTP/2 protocol error |

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
