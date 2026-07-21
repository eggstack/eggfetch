# HTTPX Drop-In Phase 4: Transports, Extensions, Authentication, and Async Backends

Status: ready for implementation handoff

## Purpose

Implement the public extension boundaries that distinguish an HTTPX-like request API from an actual HTTPX-compatible client used by frameworks, test suites, SDKs, instrumentation, and custom networking environments.

This phase adds transport protocols, mount routing, event hooks, request/response extensions, custom authentication flows, in-process test transports, environment integration, and supported async backend behavior. Real network I/O remains in `eggfetch-core`; Python-only mock, WSGI, and ASGI transports may execute entirely in Python because they do not perform network I/O.

## Dependencies

Phase 0 defines the target surface and mandatory oracle. Phase 1 provides production lifecycle and environment controls. Phase 2 provides compatible requests, responses, errors, clients, and value objects. Phase 3 provides stream types and body bridges used by all transports.

## Non-goals

- Supporting private HTTPX transport internals.
- Making HTTP/3 part of HTTPX 0.28.1 parity.
- Reimplementing a general application server.
- Running WSGI or ASGI applications over a real socket unless a separate explicit test asks for it.
- Adding unsupported proxy protocols without an optional feature boundary.
- Claiming Trio support through a blocking shim without cancellation and context semantics evidence.
- Silently falling back to the default network transport when a custom transport rejects a request.

## Deliverables

1. Compatible sync and async transport base interfaces.
2. Rust-backed HTTP transport adapters.
3. URL-pattern mount routing.
4. Mock, WSGI, and ASGI transports.
5. Event hooks with reference-compatible sequencing and error behavior.
6. Request/response extension propagation.
7. Custom authentication flow interfaces and required built-ins.
8. Explicit environment proxy, certificate, and netrc behavior.
9. Low-level transport options such as UDS and local address where included by the target.
10. Optional SOCKS support if required for the target extra.
11. A validated asyncio and Trio/AnyIO backend strategy.
12. A phase status file.

## Track A — Transport protocols

### A1. Public base transports

Implement public interfaces equivalent to:

- `BaseTransport` with `handle_request(request)` and `close()`;
- `AsyncBaseTransport` with `handle_async_request(request)` and `aclose()`.

Match:

- method signatures;
- request and response object types;
- context-manager behavior if public;
- subclassing expectations;
- exception propagation;
- close idempotence;
- stream ownership.

The compatibility facade may define these as Python abstract/protocol-style classes while adapting Rust-backed transports behind them.

### A2. HTTP transports

Implement public `HTTPTransport` and `AsyncHTTPTransport` objects backed by eggfetch-core.

Support target-profile constructor options such as:

- TLS verification and certificates;
- trust environment where applicable;
- HTTP/1 and HTTP/2 flags;
- limits;
- proxy;
- local address;
- Unix domain socket;
- transport-level retries;
- socket options if public in the pinned target.

Transport-level retries must remain distinct from eggfetch's richer native retry policy. The compatibility adapter must reproduce HTTPX behavior rather than expose unrelated defaults.

### A3. Transport error boundary

Custom transports may return responses or raise compatibility transport exceptions. Normalize only errors that belong to the public transport contract. Do not wrap arbitrary user exceptions into generic network errors if HTTPX lets them propagate.

### A4. Transport lifecycle

Clients own transports they construct internally. Ownership of user-supplied transports must match the reference. Test shared transport objects, client closure, nested clients, and repeated close.

## Track B — Mount routing

### B1. URL pattern model

Implement the target's URL-pattern matching and priority rules for mount keys, including combinations of:

- scheme;
- host;
- wildcard host;
- port;
- path if supported;
- catch-all patterns.

Do not implement mounts as unordered dictionary prefix checks. Generate differential routing cases against the reference.

### B2. Per-route transports and proxies

Support:

- separate HTTP and HTTPS transports;
- per-domain transports;
- per-domain proxies;
- explicit `None` routes where meaningful;
- direct bypass routes;
- custom schemes;
- no-proxy patterns implemented through mounts where the reference does so.

### B3. Routing observability

Test which transport receives each request and ensure:

- the original compatibility request object is passed;
- base URL resolution occurs before route selection as in the reference;
- redirects are rerouted for the new URL;
- auth and sensitive-header stripping occur at the correct layer;
- transport close occurs once even if mounted multiple times according to ownership rules.

## Track C — Mock transport

### C1. Handler contract

Implement `MockTransport` with sync and async handler compatibility. The handler must receive a compatibility `Request` and may return a compatibility `Response`.

Match:

- handler signatures;
- response request attachment;
- stream handling;
- handler exception propagation;
- sync/async mismatch errors;
- close behavior.

### C2. Streaming mocks

Allow mock responses with custom sync or async streams. Ensure single-consumption and close semantics are identical to network responses where applicable.

### C3. Test utility independence

Mock transport tests must not require a local socket server. They should be usable by downstream package fixtures in Phase 5.

## Track D — WSGI transport

### D1. Environ construction

Implement the public WSGI transport with correct:

- request method;
- script name and path info;
- query string;
- server name and port;
- URL scheme;
- WSGI version, threading, multiprocess, and run-once fields;
- input stream;
- error stream;
- header translation;
- remote address option.

### D2. Start-response and body lifecycle

Handle:

- status parsing;
- duplicate response headers;
- `exc_info` behavior;
- iterable body streaming;
- iterable `close()`;
- application exceptions;
- body read and early close;
- request body input.

### D3. App error policy

Match the target's `raise_app_exceptions` behavior and response-generation policy for app failures.

## Track E — ASGI transport

### E1. Scope construction

Implement HTTP ASGI scopes matching the reference for:

- ASGI version/spec version;
- HTTP version;
- method;
- scheme;
- path and raw path;
- query string;
- headers as bytes pairs;
- server and client tuples;
- root path.

### E2. Receive channel

Stream request bodies through ASGI `receive` messages without eager buffering. Correctly emit `more_body`, disconnect behavior, and cancellation.

### E3. Send channel

Handle:

- response start;
- multiple response body messages;
- empty chunks;
- completion;
- invalid message order;
- application exceptions;
- early client close;
- async response streaming.

### E4. Backend compatibility

The ASGI transport must operate under each async backend claimed by the compatibility stage. Avoid asyncio-specific futures inside a Trio run.

## Track F — Event hooks

### F1. Hook configuration

Support client-level event hooks for request and response events, preserving list semantics and mutability where public.

### F2. Sequencing

Differentially establish:

- whether request hooks run before auth, redirect, transport dispatch, and body send;
- whether response hooks run before body read in buffered methods;
- hook order across redirect chains;
- hook execution for retries where native retry features are used outside strict HTTPX parity;
- sync versus async hook requirements;
- behavior when hooks mutate request or response objects.

### F3. Exceptions and cleanup

Match propagation when a hook raises. Ensure transport response streams are closed when a response hook fails and no body/resource leak remains.

## Track G — Extensions

### G1. Request extension dictionary

Preserve the public request `extensions` mapping through:

- request construction;
- client merge/build;
- transport dispatch;
- redirects where the reference copies or changes values;
- response attachment.

### G2. Standard extensions

Support or preserve target-profile extensions used for:

- timeout values;
- HTTP version and reason phrase metadata;
- network stream access where public;
- tracing callbacks;
- socket or transport metadata.

Do not invent stable public semantics for undocumented internal extension keys. The profile should distinguish documented and observed-but-internal keys.

### G3. Custom extension passthrough

Unknown user extension keys must survive transport round trips according to the reference without being serialized onto the wire.

## Track H — Authentication interfaces

### H1. Public auth protocol

Implement custom authentication flow contracts for sync and async use. Match the target's generator or flow behavior, including multiple request/response exchanges.

Support:

- request transformation;
- response challenge inspection;
- body access requirements;
- sync and async flow variants;
- auth object reuse across requests;
- thread/task safety expectations;
- redirect credential policy.

### H2. Built-in auth

Implement and validate target-profile built-ins, including at least:

- basic auth;
- digest auth;
- netrc auth where public;
- tuple shorthand;
- explicit auth disable/override behavior.

Bearer auth may remain an eggfetch-native convenience if it is not a public HTTPX built-in, but must not pollute the compatibility manifest.

### H3. Digest correctness

Test digest variants included by the target:

- algorithms;
- qop modes;
- nonce count;
- stale challenges;
- repeated challenges;
- body hashing if required;
- quoted parameter parsing;
- redirect and cross-origin behavior.

Use deterministic local challenge fixtures and compare emitted authorization fields after normalizing nonce-dependent values.

### H4. Netrc security

When environment trust permits netrc:

- match lookup precedence;
- validate file permissions or errors according to the reference/platform;
- avoid logging credentials;
- isolate tests from the developer's real home directory;
- honor `trust_env=False`.

## Track I — Proxy and environment parity

### I1. Environment variables

Complete measured behavior for:

- HTTP, HTTPS, and all-proxy variables;
- lowercase and uppercase precedence;
- CGI safety behavior if applicable;
- `NO_PROXY` host, domain, IP, CIDR, port, and wildcard cases supported by the reference;
- percent-encoded credentials;
- malformed proxy URLs.

### I2. Certificate environment

Implement `SSL_CERT_FILE` and `SSL_CERT_DIR` behavior where supported by the target and Rust TLS stack. If a platform or rustls limitation prevents exact behavior, the allowed difference must block the relevant stage unless an equivalent safe implementation exists.

### I3. Explicit versus environmental configuration

Explicit constructor arguments must override or compose with environment settings exactly as measured. `trust_env=False` must disable proxy, certificate, and netrc environment discovery.

## Track J — Low-level networking options

### J1. Unix domain sockets

Expose UDS through the low-level HTTP transport on supported platforms. Test host header behavior, URL scheme handling, connection reuse, error mapping, and unsupported-platform errors.

### J2. Local address

Support binding outbound sockets to the configured local address where included by the target. Test IPv4/IPv6 and invalid address behavior.

### J3. Socket options

If present in the pinned public target, support validated socket option tuples without allowing unsafe arbitrary memory interaction. Map platform errors clearly.

## Track K — SOCKS optional support

If HTTPX 0.28.1's optional SOCKS extra is included in the final Stage D contract, implement it behind an optional Python and Rust feature.

Required coverage:

- SOCKS5 and SOCKS5h name-resolution behavior;
- authentication;
- TCP connect timeout;
- proxy errors;
- TLS over SOCKS;
- environment and mounts integration;
- wheel extras and dependency policy.

If deferred, record it as a Stage D blocker rather than silently excluding it from a full drop-in claim.

## Track L — Async backend architecture

### L1. Backend contract

HTTPX operates through AnyIO-compatible asyncio and Trio environments. Define a compatibility backend layer that does not expose Tokio or asyncio implementation assumptions to user code.

### L2. Asyncio path

Retain the direct efficient Tokio/asyncio bridge where correct, but remove explicit construction of asyncio-only futures from compatibility objects when a backend-neutral awaitable can be used.

### L3. Trio path

Choose and document an implementation capable of:

- Trio cancellation semantics;
- task-local context behavior;
- no blocking Trio run thread;
- async streaming producers and consumers;
- ASGI transport operation;
- timeout and close semantics;
- no orphan Tokio tasks after Trio cancellation.

Potential designs include a dedicated shared Tokio service accessed through a backend-neutral portal, or a Rust future bridge integrated with Trio guest mode. A simple call to a blocking sync client in a worker thread is not acceptable for Stage D unless it passes the full semantics and performance envelope and is explicitly approved.

### L4. Backend detection and errors

Match the reference when async APIs are called outside an async context or with incompatible streams. Avoid importing optional backend packages unless needed.

### L5. AnyIO test matrix

Run the same async compatibility corpus under:

- asyncio;
- asyncio with alternative loop policy where supported;
- Trio;
- cancellation and task-group fixtures.

## Track M — CI and evidence

Add required jobs for:

- custom sync and async transports;
- mount routing;
- mock transport;
- WSGI transport;
- ASGI transport under asyncio;
- ASGI and network compatibility under Trio when implemented;
- event hooks;
- custom auth and digest;
- environment trust;
- UDS on Unix;
- SOCKS extra if included;
- built-wheel optional-extra smoke tests.

Upload route traces, backend matrix summaries, and manifest deltas on failure.

## Expected files

Likely additions or changes include:

- Python compatibility transport modules;
- Python compatibility auth modules;
- core connector support for UDS/local address/SOCKS;
- compatibility client mount and hook dispatch;
- request/response extension handling;
- `crates/eggfetch-python/tests/compat/test_transports.py`;
- `test_mounts.py`;
- `test_mock_transport.py`;
- `test_wsgi.py`;
- `test_asgi.py`;
- `test_hooks.py`;
- `test_auth.py`;
- `test_environment.py`;
- `test_backends.py`;
- optional dependency metadata;
- compatibility documentation;
- `plans/httpx-drop-in-phase-4-status.md`.

## Acceptance criteria

This phase is complete only when:

- [ ] Public sync and async base transports match signatures and subclass behavior.
- [ ] Rust-backed HTTP transports support the pinned constructor options.
- [ ] User-supplied transport ownership and close behavior match the reference.
- [ ] Mount routing matches the reference priority corpus.
- [ ] Redirects reroute through mounts using the redirected URL.
- [ ] Mock transport supports sync, async, streaming, and exception cases.
- [ ] WSGI environ, response, iterable, close, and app-exception behavior match the target.
- [ ] ASGI scope, request messages, response messages, streaming, and app-exception behavior match the target.
- [ ] Event hooks run in the measured order and clean up resources when they fail.
- [ ] Request and response extensions survive every public compatibility path.
- [ ] Custom sync and async authentication flows work without private eggfetch APIs.
- [ ] Basic, digest, and netrc auth meet the pinned compatibility corpus.
- [ ] Environment proxy and no-proxy behavior matches the target profile.
- [ ] `trust_env=False` disables proxy, certificate, and netrc discovery.
- [ ] UDS and local-address support pass on supported platforms.
- [ ] SOCKS support is implemented and tested or remains an explicit blocker to Stage D.
- [ ] Asyncio compatibility tests pass without leaked tasks or backend-specific public objects.
- [ ] Trio compatibility tests pass for requests, streaming, cancellation, ASGI, auth, and close.
- [ ] The async API does not block the active event-loop or Trio run thread.
- [ ] Optional dependencies are isolated behind documented extras and wheel smoke tests.
- [ ] Phase 4 required manifest deltas are zero or explicitly stage-blocking.
- [ ] `plans/httpx-drop-in-phase-4-status.md` links exact backend, transport, and CI evidence.

## Handoff notes

This is the largest architectural phase. Implement it incrementally behind the compatibility module and keep native eggfetch paths stable. Do not route custom transports through serialized method/URL calls; transports must receive the full compatibility `Request` with its stream and extensions intact. Backend support must be proven with cancellation and streaming, not only a single successful GET.
