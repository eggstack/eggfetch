# Milestone S Plan: Proxy Subsystem

## Objective

Implement a first-class proxy subsystem in `eggfetch-core` supporting HTTP proxying and HTTPS CONNECT tunneling, with secure authentication handling and familiar Python configuration.

Proxy behavior belongs in the Rust connector/transport layer. Python should only normalize configuration and expose errors. The design should leave room for SOCKS5 and proxy routing rules without coupling proxy logic to request/response compatibility code.

## Prerequisites

Milestones Q and R should be complete or sufficiently isolated. The validation/polish pass must have established cross-platform TLS and CI reliability.

Required stable foundations:

- origin-aware pooling
- TLS connector construction
- auth and sensitive-header redaction
- timeout phase/deadline behavior
- redirect security
- sync/async Python API parity

## Scope

Milestone S includes:

- HTTP forward proxy for HTTP targets
- HTTPS CONNECT tunneling
- proxy URL parsing
- proxy Basic authentication
- client-level proxy configuration
- per-request proxy override/disable
- bypass/no-proxy matching if deliberately implemented
- pool keying through proxies
- proxy-specific errors
- Python proxy configuration
- integration tests with local proxy servers

Milestone S does not initially include:

- PAC files
- system proxy auto-discovery
- NTLM/Negotiate proxy auth
- proxy chaining
- transparent interception
- SOCKS5 unless added as a clearly separate optional subtrack

## Core architecture

Introduce a proxy configuration model separate from ordinary request auth.

Suggested types:

```rust
pub struct Proxy {
    uri: Url,
    auth: Option<ProxyAuth>,
    rule: ProxyRule,
}

pub enum ProxyRule {
    All,
    Http,
    Https,
    Custom(...),
}

pub enum ProxyDecision {
    Direct,
    Proxy(ProxyConfig),
}
```

The client connector should determine proxy routing before opening a connection.

Avoid modifying the logical request URL merely to represent the connection route.

## Proxy URL parsing

Support proxy URLs such as:

```text
http://proxy.example:8080
http://user:pass@proxy.example:8080
```

Initial supported proxy transport may be HTTP only, including CONNECT for HTTPS targets.

Requirements:

- validate scheme
- require host
- derive default port where appropriate
- reject fragments/query if unsupported
- redact userinfo in Debug/Display/errors
- reject CR/LF and invalid credentials

Do not expose proxy passwords in logs or exception text.

## HTTP target through HTTP proxy

For an HTTP destination through an HTTP proxy:

- connect TCP to proxy
- send absolute-form request target, e.g. `GET http://example.com/path HTTP/1.1`
- set Host for destination
- apply Proxy-Authorization only to proxy request
- preserve ordinary request headers according to HTTP rules

Ensure origin Authorization and cookies remain destination-scoped and are not confused with proxy credentials.

## HTTPS target through CONNECT

For HTTPS destination:

1. connect TCP to proxy
2. issue `CONNECT host:port HTTP/1.1`
3. include proxy auth if configured
4. validate successful 2xx response
5. establish TLS to destination through tunnel
6. perform normal HTTP request over TLS

The TLS SNI and certificate verification target must be the destination host, never the proxy host.

CONNECT response bodies/extra bytes must be handled safely.

## Connector implementation strategy

Evaluate whether `hyper-util` connector traits can wrap the current connector cleanly.

Likely shape:

- direct connector for ordinary routes
- proxy-aware connector deciding direct versus proxy
- tunneled IO type implementing AsyncRead/AsyncWrite
- TLS wrapping after CONNECT for HTTPS

Keep proxy behavior below the HTTP request execution pipeline so cookies, auth, redirects, compression, and bodies work identically for direct/proxied requests.

## Pool keying

Connection pool keys must include proxy route as well as destination/protocol context.

For HTTP forward proxying, one proxy connection may potentially serve different destinations depending on protocol behavior, but initial conservative pooling can key by:

```text
proxy origin + destination origin + tunnel mode
```

For CONNECT tunnels, key by proxy plus destination origin.

Do not accidentally reuse a tunneled connection for a different destination.

Document the conservative policy; optimize later.

## Timeouts

Define phase behavior:

- pool timeout: acquiring route-specific permit
- connect timeout: TCP connection to proxy plus CONNECT/TLS establishment, unless finer phases are later exposed
- read timeout: proxy CONNECT response and destination response reads
- write timeout: upload/body producer semantics as currently implemented
- total timeout: entire logical request including proxy negotiation and redirects

Add proxy negotiation errors that preserve timeout phase classification where possible.

## Proxy authentication

Initial support: Basic proxy auth.

Generate:

```text
Proxy-Authorization: Basic ...
```

Rules:

- never forward Proxy-Authorization to destination after CONNECT
- strip on redirects because proxy route is recomputed independently
- redact credentials everywhere
- explicit user Proxy-Authorization header should conflict with configured proxy auth or override under a documented policy

Prefer rejecting conflicting sources.

## Client and request API

Rust target:

```rust
let proxy = Proxy::all("http://proxy:8080")?;
let client = Client::builder().proxy(proxy).build()?;

client.get(url).without_proxy().send().await?;
```

Potential methods:

- `ClientBuilder::proxy()`
- `ClientBuilder::proxies()` for routing rules later
- `RequestBuilder::proxy()`
- `RequestBuilder::without_proxy()`

Use a tri-state override model analogous to auth:

- inherit client proxy
- explicit proxy override
- direct/no proxy

## Python API

Target familiar configuration:

```python
client = eggfetch.Client(proxy="http://localhost:8080")
response = client.get(url)

response = eggfetch.get(url, proxy="http://localhost:8080")
response = client.get(url, proxy=None)  # explicit direct route, if omission/inherit is distinguishable
```

HTTPX increasingly uses `proxy=` rather than older `proxies=` forms. Prefer a simple singular `proxy=` first.

Later routing map support may accept:

```python
proxies={"http://": ..., "https://": ...}
```

Do not implement both prematurely unless compatibility needs justify it.

Expose a `Proxy` class only if it adds meaningful auth/routing configuration.

## Environment variables and NO_PROXY

Decide whether to support:

- `HTTP_PROXY`
- `HTTPS_PROXY`
- `ALL_PROXY`
- `NO_PROXY`

Recommended initial policy:

- explicit configuration only by default
- optional `trust_env=True` support later or within this milestone only if thoroughly specified

Environment-driven behavior can be surprising and security-sensitive. Do not silently enable it without an explicit policy.

If `NO_PROXY` is implemented, support and test:

- exact hosts
- domain suffixes
- ports
- IPv4/IPv6 literals
- localhost
- wildcard `*`
- CIDR only if deliberately supported

## Redirect behavior

Proxy route must be recomputed for every redirect destination.

Destination auth/cookies follow existing redirect security rules.

Proxy credentials remain tied to the selected proxy, not the destination.

Cross-origin redirects through the same proxy must still strip destination Authorization/cookies as already required.

## Compression and streaming interaction

Proxies should be transparent to:

- streamed uploads
- streamed responses
- multipart bodies
- response decompression
- read/write timeouts

Add integration tests combining proxy with large streaming upload/download and compressed response.

## TLS interception behavior

A normal CONNECT proxy does not intercept TLS. If a corporate proxy performs interception, certificate verification will only succeed if its CA is trusted through the configured/native trust store.

Document this rather than adding insecure automatic bypasses.

## Error taxonomy

Add structured errors:

- invalid proxy URL
- proxy connection failure
- proxy authentication required/failed where distinguishable
- CONNECT rejected with status
- malformed CONNECT response
- proxy tunnel closed
- unsupported proxy scheme

Python hierarchy may include:

```python
ProxyError(RequestError)
ProxyConnectError(ProxyError)
ProxyAuthenticationError(ProxyError)
```

Do not leak proxy credentials in messages.

## Test infrastructure

Build local test proxies.

### HTTP forward proxy server

Capabilities:

- record absolute-form requests
- forward to local destination
- require Basic auth optionally
- return controlled failures

### CONNECT proxy server

Capabilities:

- parse CONNECT target
- establish tunnel to local TLS server
- relay bidirectionally
- require auth optionally
- reject selected targets/statuses

Keep test proxy implementation deterministic and test-only.

## Tests

### Parsing/config tests

- valid proxy URL
- default port
- invalid scheme
- missing host
- redacted Debug/Display
- credentials validation
- request override/disable precedence

### HTTP proxy tests

- absolute-form target
- destination Host correct
- request body streaming
- response streaming
- proxy auth sent only to proxy
- destination auth remains distinct

### CONNECT tests

- successful HTTPS request through tunnel
- SNI/certificate validation uses destination
- rejected CONNECT maps to ProxyError
- proxy auth challenge/failure
- tunnel close/error
- pool reuse only for compatible destination route

### Security tests

- Proxy-Authorization never reaches destination
- proxy password absent from errors/reprs
- redirect destination credentials stripped
- URL userinfo redacted/rejected as policy dictates

### Python tests

- sync proxy request
- async proxy request
- proxy auth
- explicit direct override
- timeout/error mapping
- streamed response through proxy
- multipart upload through proxy
- compressed response through proxy

## SOCKS5 extension

Do not mix SOCKS into the initial HTTP proxy implementation unless the abstraction is stable.

A later feature-gated subtrack can add:

- SOCKS5 CONNECT
- username/password auth
- remote versus local DNS selection

Use separate errors and dependencies.

## Feature gating

Use a `proxy` feature for HTTP proxy support.

If SOCKS is later added, use `socks` separately.

Python wheels should enable HTTP proxy support by default once stable.

## Documentation

Document:

- supported proxy schemes
- HTTP versus CONNECT behavior
- proxy auth
- trust environment policy
- pooling policy
- timeout semantics
- TLS interception implications
- differences from requests/httpx

## Acceptance criteria

Milestone S is complete when:

- HTTP destinations work through forward proxying
- HTTPS destinations work through CONNECT tunnels
- proxy credentials never reach destinations or logs
- route-specific pooling is safe
- redirects recompute proxy routes
- streaming, multipart, cookies, auth, compression, and timeouts work through proxies
- Python sync/async proxy behavior matches
- deterministic local proxy integration tests pass

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps

cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy

cd crates/eggfetch-python
maturin develop
python -m pytest
```

## Handoff note

Milestone T adds user-facing TLS configuration. Proxy CONNECT must already preserve destination SNI and verification so custom trust/client-certificate support can be layered cleanly afterward.
