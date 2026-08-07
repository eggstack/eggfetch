# HTTPX 0.28.1 Parity Completion — Phase 4: Advanced Direct Transport Options

Status: ready for implementation handoff

Date: 2026-08-07

Roadmap: `plans/httpx-parity-completion-roadmap.md`

Prerequisites:

- Phase 1 contract rebaseline complete;
- Phase 2 object contracts complete or blockers recorded;
- Phase 3 signature/type surface complete so the public low-level transport constructor is stable.

Pinned reference: `httpx==0.28.1`

Compatibility designation: `Stage C candidate`

## Objective

Make HTTPX 0.28.1 low-level direct-transport options that the compatibility facade already accepts — `uds`, `local_address`, and `socket_options` — actually functional through `eggfetch-core` rather than raising `NotImplementedError`.

The implementation must remain narrow: extend the existing native connector/transport path, preserve the normal TCP/TLS fast path, preserve pooling/timeout/cancellation semantics, and avoid a generalized connector framework.

## Current confirmed behavior

At the audited baseline, `HTTPTransport` and `AsyncHTTPTransport` expose:

- `local_address`;
- `socket_options`;
- `uds`.

The compatibility facade calls `_validate_transport_options`, which rejects every non-`None` value before network activity. The transport docstrings explicitly say these values are accepted only for API compatibility and are not forwarded to `eggfetch-core`.

This phase closes that functional gap.

## Likely implementation areas

Python facade/binding:

- `crates/eggfetch-python/python/eggfetch/compat/httpx/_transports.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py` validation/plumbing
- native Python client/binding constructor plumbing only as needed to reach core

Rust core:

- direct connector/transport implementation under `crates/eggfetch-core/src/transport/` or current equivalent;
- pool-key/connector ownership types;
- narrowly scoped configuration structures;
- error variants only if current error taxonomy cannot express the new failures;
- `Cargo.toml` only if a safe focused socket primitive dependency is required.

Tests:

- compatibility differential tests;
- core connector tests;
- local end-to-end fixtures using loopback and Unix sockets.

## Architecture constraints

### 1. One networking engine

All real connections continue through `eggfetch-core` and Tokio/Hyper. Do not implement these options using Python `socket`, urllib, httpcore, or a sidecar client.

### 2. Default path remains simple

When all advanced options are unset, existing connector creation and connection behavior should remain unchanged or differ only by a trivial branch/config lookup.

Do not require every ordinary connection to pass through a heavyweight generalized connector abstraction.

### 3. Connector configuration must participate in connection ownership

Connections created with incompatible low-level transport settings must not be reused interchangeably.

At minimum, reason explicitly about pool isolation for:

- ordinary TCP with default bind/options;
- TCP with an explicit local bind;
- TCP with socket options;
- UDS path A;
- UDS path B;
- proxy paths, which must remain distinct from direct paths.

If each low-level transport instance already owns an isolated native client/pool, document and test why that is sufficient. Do not add redundant global key complexity if instance-level isolation already prevents collision.

### 4. Safe Rust only

The workspace forbids unsafe code. Do not add direct libc/FFI socket manipulation.

If Tokio/std APIs cannot express the required pre-connect socket configuration, a small safe dependency such as `socket2` may be considered only after documenting:

- exact missing primitive;
- dependency features used;
- transitive dependency impact;
- why a smaller existing dependency/API cannot do the job.

No general network framework should be added.

## Track 0 — Reference and connector-design proof

### 0.1 Differentially pin public behavior before implementation

Using `httpx==0.28.1`, determine exact observable behavior for:

- `HTTPTransport(uds=...)`;
- `AsyncHTTPTransport(uds=...)`;
- `local_address` with IPv4 and, where supported, IPv6;
- socket option tuple/list shapes;
- invalid option types/values;
- unsupported platform behavior;
- interactions with `http1`/`http2` where observable.

Do not infer behavior from httpcore source if a public end-to-end test can determine it.

### 0.2 Locate the smallest native connector seam

Document the existing flow from:

compat transport → native Python binding → EggFetch client config → pool/direct transport → TCP connect → optional TLS.

Identify the narrowest point at which these settings can be applied before the connection is handed to Hyper.

### 0.3 Define a bounded connector configuration

Prefer a small internal representation equivalent to:

```text
Direct endpoint:
- TCP: optional local bind + socket options
- Unix: socket path (Unix platforms only)
```

The exact Rust type is implementation-defined. Do not create a public generalized connector trait hierarchy unless the current architecture already has one and extending it is clearly smaller.

### Track 0 acceptance criteria

- Reference behavior is pinned by tests.
- The exact native connector seam is documented in implementation notes/PR.
- Pool/ownership implications are understood before code changes.
- Any dependency addition is justified before being added.

## Track 1 — Implement `local_address`

### 1.1 Bind before connect

When `local_address` is provided, create/bind the outbound socket to the requested local address before connecting to the remote origin.

Do not simulate support by checking the address after the OS has already selected a source address.

### 1.2 Preserve address-family correctness

Differentially determine HTTPX behavior for:

- IPv4 local bind + IPv4 destination;
- IPv6 local bind + IPv6 destination where CI/host supports IPv6;
- family mismatch;
- unavailable local address;
- malformed address input.

Map failures into the existing connection/transport error taxonomy as closely as the HTTPX facade requires.

### 1.3 Default behavior unchanged

`local_address=None` must use the current direct connect path with no explicit bind.

### 1.4 End-to-end proof

Use a local loopback HTTP server that records the peer address. The test must prove the observed source address corresponds to the requested bind in a configuration where the host can distinguish it.

Do not count a constructor/no-error test as functional proof.

### Track 1 acceptance criteria

- Outbound source binding occurs before connect.
- A local fixture observes the expected bound source address.
- invalid/unavailable address failures are reference-compatible at the facade level;
- default-path connect/pooling tests remain passing.

## Track 2 — Implement `socket_options`

### 2.1 Pin the accepted representation

Use HTTPX 0.28.1 to determine the supported public tuple/list representation, expected to be socket-option triples but not to be assumed without a reference test.

Test:

- one option;
- multiple options;
- invalid tuple lengths;
- invalid level/name/value types;
- OS-level option application failure.

### 2.2 Apply options before connect

Socket options that HTTPX applies to newly created connection sockets must be applied before the connect operation at the corresponding native point.

Do not apply options to an already-established pooled stream unless the reference specifically behaves that way.

### 2.3 Choose a minimal safe implementation

Preferred order:

1. existing Tokio/std safe APIs;
2. an already-present crate capability;
3. a narrowly configured safe socket crate.

Do not add unsafe blocks or direct libc calls.

### 2.4 Define unsupported-option behavior explicitly

If a platform cannot support an option:

- propagate a deterministic connection/configuration error that matches HTTPX's public behavior as closely as practical;
- do not silently ignore the option;
- do not mark the parity gap resolved merely because common Linux options work.

Platform-specific expected failures may remain documented when HTTPX itself delegates to OS support.

### 2.5 End-to-end proof

Use a socket option that can be inspected reliably on the created socket or a controlled connector fixture.

A test that only verifies the Python constructor accepts the value is insufficient.

### Track 2 acceptance criteria

- Valid supported options reach the created native socket before connect.
- Invalid representations fail deterministically.
- Unsupported OS options are not silently ignored.
- No unsafe code is introduced.
- Default path is unaffected when the option list is empty/unset.

## Track 3 — Implement Unix domain socket transport

### 3.1 Unix-only native endpoint

On supported Unix targets, connect using `tokio::net::UnixStream` or the smallest existing safe equivalent, then adapt the stream into the existing Hyper client connection machinery.

Do not emulate UDS through a TCP proxy.

### 3.2 Separate transport endpoint from HTTP authority

For UDS requests:

- the Unix path selects the transport endpoint;
- the request URL still supplies HTTP scheme/authority/path/Host semantics in the same manner as HTTPX.

Do not replace the HTTP Host header with the filesystem path.

### 3.3 Pin HTTP vs HTTPS semantics

Use the reference to determine what HTTPX 0.28.1 does for:

- `http://` URL over a UDS transport;
- `https://` URL over a UDS transport, if accepted;
- HTTP/2 requests over UDS if supported by the reference.

Do not invent TLS-over-UDS behavior. If HTTPS/HTTP2 requires a significantly broader architecture and the reference behavior is not important to intended downstream use, stop that subcase explicitly rather than quietly claiming full UDS parity.

### 3.4 Non-Unix behavior

On Windows/non-Unix targets, match HTTPX's constructor/use-time public failure behavior as closely as practical.

Do not make the entire package fail to import because UDS support is unavailable on the platform.

### 3.5 End-to-end local fixture

Create a temporary Unix socket HTTP server and prove:

- request reaches the socket server;
- Host/request target are correct;
- response body/headers return through the normal facade;
- close/cancellation releases the stream and socket path can be cleaned up;
- two different UDS paths cannot reuse one another's connections.

### Track 3 acceptance criteria

- Real HTTP requests traverse UnixStream on supported platforms.
- Authority/Host semantics match the reference.
- platform gating is clean and deterministic.
- pool/transport ownership cannot cross UDS paths.
- no Python-side network implementation is introduced.

## Track 4 — Native binding and compatibility plumbing

### 4.1 Remove reject-only validation

Replace `_validate_transport_options` rejection behavior only after the corresponding native capability exists.

Do not remove the guard first and silently drop the values.

### 4.2 Forward options through one native configuration path

`HTTPTransport` and `AsyncHTTPTransport` should construct their native clients/connectors with the same underlying advanced option representation.

Avoid independent sync and async implementations.

### 4.3 Keep options low-level

HTTPX exposes these knobs on its low-level transport. Do not automatically add them to EggFetch's high-level non-compat public `Client` API unless the native binding requires an internal constructor field and adding it is explicitly reviewed.

If native Python objects need hidden/internal parameters, keep them private to the binding bridge where possible.

### 4.4 Preserve transport lifecycle

Closing the compat transport must close its native client/pool exactly once. Advanced connector config must not create orphan pools or background tasks.

### Track 4 acceptance criteria

- No compatibility value is ignored.
- Sync and async transports share the native implementation.
- Non-compat public API expansion is minimized.
- close/cancellation/pool semantics remain correct.

## Track 5 — Pool, timeout, cancellation, and TLS regression proof

### 5.1 Pool isolation

Prove incompatible transport configurations cannot share connections.

At minimum test separate transport instances/configurations for:

- default TCP vs explicitly bound TCP;
- two local bind configurations when host allows;
- UDS A vs UDS B;
- relevant different socket-option sets if connector instances could otherwise share a pool.

### 5.2 Timeout ownership

Connection timeout must include the actual advanced endpoint connection establishment.

Do not let UDS or pre-connect socket configuration bypass existing connect-timeout enforcement.

### 5.3 Cancellation/close

Reuse the established cancellation/lease testing pattern:

- cancel an in-flight advanced connection/request;
- explicitly close where required;
- prove a subsequent constrained request can acquire a lease;
- prove descriptors/Unix streams are released.

### 5.4 TLS regression

Ordinary HTTPS over TCP must continue using the existing TLS configuration and verification path.

If any advanced TCP socket construction changes how the stream is passed to TLS, rerun mTLS/custom CA/verification tests that exercise that seam.

### Track 5 acceptance criteria

- pool isolation is proven;
- timeouts include advanced connect setup;
- cancellation releases resources/leases;
- ordinary HTTP/HTTPS transport behavior remains unchanged.

## Track 6 — Differential and ledger closure

Add direct HTTPX comparisons for the supported advanced transport cases. For UDS/local binding/socket options, use equivalent local fixtures against HTTPX and EggFetch rather than comparing internal connector objects.

After passing:

- regenerate candidate API manifest;
- remove resolved functional-gap documentation/allowlist records owned by this phase;
- move historical difference records to `resolved-differences.toml` where applicable;
- update compatibility docs to mark only the actually proven platforms/semantics as supported.

Do not claim functionality for a stopped subcase.

## Required validation

Mandatory:

- focused core connector tests;
- focused Python HTTPX differential tests;
- existing transport/mount/proxy/TLS/pool/timeout/cancellation tests;
- `./scripts/check.sh`;
- API oracle after ledger changes.

Run the full pinned compatibility suite at the phase boundary if practical; Phase 6 will rerun it authoritatively.

Do not add a new CI matrix. Platform-specific UDS tests should use existing conditional skip/gating conventions.

## Phase acceptance criteria

Phase 4 is complete only when:

- `local_address` performs a real pre-connect bind;
- `socket_options` are applied to the real outbound socket and never silently ignored;
- UDS performs real end-to-end Unix-socket HTTP on supported Unix targets;
- public invalid/platform-specific behavior is differentially pinned;
- connector/pool ownership prevents incompatible configuration reuse;
- timeout/cancellation/close semantics are preserved;
- ordinary direct HTTP/HTTPS performance/behavior path is not architecturally replaced;
- `./scripts/check.sh` passes;
- API oracle/ledger is clean after resolving Phase 4 records;
- no SOCKS, Trio/AnyIO, CI/release expansion, unsafe code, or generalized connector framework is introduced.

## Rejection criteria

Reject the implementation if:

- advanced parameters are accepted but ignored;
- Python sockets/httpcore are used to bypass the Rust engine;
- UDS is implemented by proxying through TCP;
- local bind occurs after connect;
- socket options are applied only after a pooled connection exists;
- incompatible endpoints/configurations can share one connection;
- unsafe Rust/direct libc is added;
- a large networking framework is added for three focused primitives;
- advanced connect paths bypass existing timeout/cancellation/TLS/error mapping;
- docs claim support beyond the tested platform/reference behavior.

## Stop conditions

Stop the affected sub-track rather than broadening scope if:

- safe arbitrary socket-option application is impossible with existing APIs and no acceptable focused safe dependency exists;
- UDS requires replacing the Hyper transport/pool architecture rather than adding a contained stream connector;
- HTTPX's HTTPS-over-UDS behavior requires a substantial new TLS abstraction not justified by intended use;
- platform behavior cannot be tested deterministically in the available environment.

A stop report must contain:

- missing primitive;
- reference reproducer;
- current EggFetch reproducer;
- exact acceptance criteria affected;
- smallest future implementation proposal.

Independent Phase 4 tracks should continue where safe.

## Suggested commit decomposition

1. `test: pin HTTPX advanced direct transport behavior`
2. `feat: add explicit local-address connector binding`
3. `feat: apply HTTP transport socket options`
4. `feat: add Unix-domain HTTP transport`
5. `test: prove advanced transport pool and lifecycle parity`
6. `docs: resolve advanced transport compatibility gaps`

## Handoff checklist

Report:

- starting SHA;
- final Phase 4 executable SHA;
- any dependency added and justification;
- files changed in core/bindings/facade;
- local-address fixture/result;
- socket-option fixture/result;
- UDS fixture/result and platforms covered;
- pool-isolation tests;
- timeout/cancellation/TLS regressions;
- `./scripts/check.sh` result;
- full pinned suite result if run;
- API oracle before/after count;
- exact remaining advanced-transport limitations;
- confirmation of no unsafe code, SOCKS scope, new CI, or release automation.
