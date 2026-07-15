# Threat Model

This document defines the threat model for eggfetch, covering assets, trust boundaries, attacker capabilities, non-goals, and the security properties the system is designed to uphold.

## Assets

The following data and resources are within scope of the threat model:

### Credentials

- **User credentials** -- Basic auth usernames and passwords, Bearer tokens, and any `Authorization` header values provided by the caller.
- **Session cookies and cookie jar state** -- Cookies stored in `CookieJar`, including session identifiers, CSRF tokens, and any persistent cookie state accumulated across requests.
- **Proxy authentication credentials** -- `Proxy-Authorization` header values (Basic, Bearer, or other schemes) provided for proxy authentication.
- **TLS private keys** -- Client certificate private keys used for mutual TLS authentication. Held in memory only; never logged or serialized.

### Request/Response Data

- **Request and response body content** -- Any data transmitted in HTTP request or response bodies, including JSON payloads, file uploads, streaming chunks, and form data.
- **URL parameters and query strings** -- Query parameters may contain API keys, tokens, session identifiers, or other secrets. URL userinfo (e.g., `https://user:pass@host/`) is rejected, but callers may embed secrets in query strings.
- **Request and response headers** -- Headers may contain authentication tokens, session state, API keys, proxy credentials, and other sensitive metadata.

### Infrastructure State

- **Connection pool state** -- Pool permits and per-origin concurrency limits. Pool exhaustion can cause denial of service.
- **DNS resolution results** -- Resolved IP addresses may reveal network topology. DNS cache poisoning could redirect traffic.
- **TLS session state** -- Session tickets and resumed sessions. Compromise could enable traffic decryption.

## Trust Boundaries

eggfetch operates across several trust boundaries. Each boundary requires distinct security properties.

### User <-> eggfetch

The user provides URLs, headers, credentials, TLS configuration, and body content. eggfetch must:

- Not leak credentials in debug output, error messages, logs, or repr diagnostics.
- Not send credentials to unintended destinations (cross-origin redirect stripping).
- Not silently disable TLS verification without explicit opt-in.
- Reject malformed input (URLs, headers, multipart boundaries) that could cause injection.

### eggfetch <-> Network

The network is untrusted. All data received from the network is unvalidated until parsed and verified. eggfetch must:

- Validate all HTTP response headers, status lines, and body framing.
- Enforce decompression limits to prevent zip-bomb attacks.
- Enforce body size limits to prevent memory exhaustion.
- Not trust `Content-Length` or `Transfer-Encoding` headers beyond what the protocol requires.
- Handle malformed chunked encoding, invalid headers, and truncated responses gracefully.

### eggfetch <-> Proxy

The proxy is semi-trusted. The user configures the proxy, so the proxy is not an external attacker. However:

- Proxy credentials (`Proxy-Authorization`) must not be forwarded to the destination server.
- A malicious or compromised proxy may inject headers, modify responses, or return crafted redirect targets.
- CONNECT tunnel data is a transparent byte stream; eggfetch does not inspect or modify tunnel content beyond the initial handshake.
- Proxy response parsing (status line, headers) must be bounded to prevent resource exhaustion from oversized responses.

### eggfetch <-> TLS Layer

The TLS layer (rustls via hyper-rustls) provides confidentiality and integrity for transport. eggfetch must:

- Not bypass certificate verification except through explicit `danger_accept_invalid_certs()` API.
- Not silently downgrade to weaker TLS versions (SSLv3, early TLS) when stricter versions are available.
- Handle certificate-chain and hostname verification failures as hard errors, not fallback triggers.
- Store private key material in memory only; never serialize, log, or include in diagnostics.

### eggfetch <-> FFI Boundary

FFI callers may pass null pointers, invalid lengths, freed handles, or concurrent mutations across the boundary. The FFI layer must:

- Treat null pointer inputs as no-ops for free functions.
- Validate handle types before dereferencing.
- Prevent use-after-free by consuming handles after send/inspect.
- Not expose internal Rust types or memory layouts to callers.

### eggfetch <-> Python Runtime

Python bindings bridge Rust async to Python sync/async via PyO3. The boundary must:

- Release the Python GIL during network I/O and body reads to avoid blocking the interpreter.
- Handle GIL lifecycle correctly in both sync and async contexts.
- Prevent borrow-checker violations across the FFI by using owned types.
- Redact sensitive values in Python `__repr__`, `__str__`, and exception messages.

## Attacker Capabilities

The following attacker models are in scope:

### Network-Positioned Attacker (MITM)

An attacker on the same network segment can intercept, modify, or replay traffic for non-TLS connections. For TLS connections, the attacker can attempt downgrade attacks, present forged certificates, or exploit TLS implementation bugs. eggfetch mitigates this through:

- TLS verification by default (certificate chain + hostname verification).
- TLS 1.2 minimum (configurable); no SSLv3 or early TLS support.
- No silent fallback from verification failure to permissive mode.

### Malicious Server

A server controlled by the attacker can send crafted HTTP responses designed to exploit parsing bugs, trigger excessive resource consumption, or redirect to internal resources. Capabilities include:

- Crafting redirect chains that loop infinitely or target internal/private networks (SSRF).
- Sending excessively large or deeply nested compressed bodies (zip bombs).
- Injecting response headers that exploit downstream consumers.
- Sending malformed chunked encoding, invalid content-length values, or truncated responses.

### Decompression Bomb

A server sends a compressed response that expands to an enormous size (e.g., a small gzip file containing petabytes of zeros). Without limits, this exhausts memory. eggfetch mitigates this with:

- `max_decoded_body_size`: hard limit on total decoded bytes.
- `max_decompression_ratio`: ratio limit comparing decoded to compressed bytes.
- Both limits enforced during streaming, not just on buffered reads.

### Server-Side Request Forgery (SSRF)

A malicious server responds with a redirect to an internal/private IP address (e.g., `http://169.254.169.254/` for cloud metadata). eggfetch's redirect engine:

- Follows redirects based on the user's redirect policy.
- Does not inherently block redirects to private networks (this is a user-configurable concern).
- Users should set `max_redirects` and consider blocking private-range destinations in their application layer.

### Malicious Proxy

A compromised or malicious proxy can:

- Inject headers into proxied responses.
- Modify response bodies or status codes.
- Return crafted CONNECT responses to exploit proxy-response parsing.
- Forward proxy credentials to the destination.

eggfetch mitigates proxy attacks by:

- Parsing proxy responses with bounded header/line limits.
- Never forwarding `Proxy-Authorization` to the destination.
- Treating CONNECT tunnel data as opaque after the handshake.
- Allowing per-request proxy configuration to override client defaults.

### Crafted Multipart Boundaries

A malicious caller or compromised input source can craft multipart boundaries containing CR/LF characters that enable header injection. eggfetch mitigates this by:

- Generating random boundaries via `getrandom` (cryptographically secure).
- Validating custom boundaries via `Boundary::try_new()` -- rejects CR, LF, and whitespace.
- Ensuring boundaries cannot appear in body content through length-aware framing.

## Non-Goals

The following attack classes are out of scope:

- **Physical access attacks** -- physical device compromise, cold-boot attacks, or hardware keyloggers.
- **Compromised root CA store** -- if the operating system's trust store is compromised, eggfetch cannot provide stronger guarantees than the store. Custom CA bundles are a mitigation, not a defense against a compromised OS.
- **Attacks on the Rust compiler or standard library** -- correctness of the Rust language, standard library, or compiler is assumed.
- **Network-level denial of service** -- TCP SYN floods, amplification attacks, or BGP hijacking are outside the scope of an HTTP client library.
- **Application-layer logic bugs** -- eggfetch does not validate the semantic correctness of request or response bodies beyond framing and encoding.
- **Multi-tenant isolation** -- eggfetch does not isolate concurrent requests from different callers within the same process beyond pool concurrency limits.

## Security Properties

The following properties are enforced by the current implementation:

### 1. No Credential Leakage in Diagnostic Output

All credential-carrying types (`BasicAuth`, `BearerAuth`, `ProxyAuth`) implement custom `Debug` and `Display` traits that redact sensitive values. `Response` debug output replaces `Authorization`, `Proxy-Authorization`, `Cookie`, and `Set-Cookie` header values with `<redacted>`. URL debug output strips userinfo, query strings, and fragments. This applies to Rust debug output, Python repr, CLI verbose output, and error messages.

### 2. Cross-Origin Redirect Strips Sensitive Headers

On cross-origin redirects (scheme, host, or port mismatch), the redirect engine strips `Authorization`, `Proxy-Authorization`, `Cookie`, and `Set-Cookie` headers from the redirected request. Client-level auth is not reapplied on cross-origin hops. Same-origin redirects preserve headers and reapply client-level auth. Port changes are treated as cross-origin.

### 3. Proxy Auth Never Forwarded to Destination

`Proxy-Authorization` headers are consumed during the proxy handshake (CONNECT for HTTPS, request-line for HTTP forward proxying) and are never included in the request sent to the destination server. This is enforced regardless of whether the proxy is HTTP or HTTPS.

### 4. TLS Verification Cannot Be Silently Disabled

Certificate verification is enabled by default. Disabling it requires explicit opt-in via `danger_accept_invalid_certs(true)` (Rust), `verify=False` (Python), or `--no-verify` (CLI). The Python `Client.__repr__` prints a `[UNSAFE: TLS verification disabled]` warning when verification is off. Verification failure is a hard error; the client does not fall back to a permissive mode.

### 5. Decompression Has Configurable Resource Limits

Both `max_decoded_body_size` and `max_decompression_ratio` are enforced during streaming decompression, not just on buffered reads. When either limit is exceeded, the response stream is terminated and the pool lease is released. Limits default to unlimited but can be configured at client or request level.

### 6. Multipart Boundaries Are Randomly Generated and Validated

Random boundaries use 50 alphanumeric characters generated via `getrandom` (CSPRNG). Custom boundaries pass through `Boundary::try_new()` which rejects CR, LF, whitespace, and empty strings. Boundary validation prevents header injection via multipart framing.

### 7. Redirect Following Has Configurable Limits

`max_redirects` caps the total number of redirect hops in a chain. The total timeout is a single deadline across the entire redirect chain, preventing infinite redirect loops from extending the deadline. Users can disable redirect following entirely via `follow_redirects(false)`.

### 8. Retry Does Not Replay Unsafe Methods

The retry subsystem only retries idempotent methods by default (GET, HEAD, OPTIONS, PUT, DELETE). Non-idempotent methods (POST, PATCH) are not retried unless the caller explicitly opts in. Body replayability is checked: streaming bodies cannot be replayed and are not retried. The retry budget (max retries, backoff) is enforced and the deadline is not extended across retries.

### 9. FFI Handles Are Null-Safe and Opaque

All FFI free functions treat null pointers as no-ops. Handle types are opaque (callers see only opaque pointers). Handles are consumed by send/inspect operations, preventing use-after-free. The FFI layer validates handle types before dereferencing and returns error handles for invalid operations.

### 10. URL Credentials Are Rejected

URL userinfo (e.g., `https://user:pass@host/`) is rejected with an error. Callers must use explicit auth schemes (`BasicAuth`, `BearerAuth`) instead. The error message does not echo the password. This prevents accidental credential embedding in URLs that may appear in logs or history.

### 11. Cookie Jar Integrity

Cookies are matched against requests using RFC 6265 domain/path rules. Secure cookies are only sent over HTTPS. Host-only cookies (set without `Domain` attribute) are not sent to subdomains. Stale cookies are expired automatically. Request-local cookies are not added to the persistent jar.

### 12. No Silent Environment Proxy

eggfetch does not read `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, or `NO_PROXY` environment variables. Proxy configuration is always explicit. This prevents surprising behavior when multiple proxy-aware libraries coexist in the same process.

## Attack Tree Summary

```
Attacker Goal: Compromise confidentiality, integrity, or availability
├── Network Position
│   ├── Non-TLS traffic interception (mitigated: TLS default)
│   ├── TLS downgrade attack (mitigated: TLS 1.2 minimum)
│   └── Certificate forgery (mitigated: verification default)
├── Malicious Server
│   ├── Zip bomb (mitigated: decompression limits)
│   ├── SSRF via redirects (mitigated: configurable limits)
│   ├── Header injection via multipart (mitigated: boundary validation)
│   ├── Redirect loop (mitigated: max_redirects + total timeout)
│   └── Malformed response parsing (mitigated: bounded parsing)
├── Malicious Proxy
│   ├── Credential forwarding (mitigated: Proxy-Auth not forwarded)
│   ├── Proxy response injection (mitigated: bounded parsing)
│   └── Tunnel interception (mitigated: transparent byte stream)
└── FFI Caller
    ├── Null pointer dereference (mitigated: null-safe free)
    ├── Use-after-free (mitigated: handle consumption)
    └── Type confusion (mitigated: opaque handle types)
```
