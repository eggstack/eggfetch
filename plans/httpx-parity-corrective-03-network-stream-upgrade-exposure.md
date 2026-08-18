# HTTPX Parity Corrective 03 — Network Stream and Upgrade Exposure

Baseline reviewed: `4571cb55bc2ff49822608d750dfef185cff40ebc`
Depends on: Corrective 01 safe TLS translation boundary and Corrective 02 response-extension export.

## Objective

Finish the Phase 04 work at the compatibility boundary without compromising Hyper connection-pool safety. EggFetch should expose a usable `network_stream` only when it actually owns a stream or can safely expose immutable metadata. Ordinary pooled Hyper HTTP/1.1 and HTTP/2 connections must remain non-writable and may remain an explicit bounded difference from httpcore.

The primary targets are:

- correct 101 upgrade exposure through `Response.extensions["network_stream"]`;
- a truthful disposition for successful CONNECT;
- runtime-safe sync/async stream wrappers;
- `start_tls` behavior that uses the same safe TLS policy as Corrective 01 or is explicitly bounded when the concrete upgraded I/O cannot support it.

## Current defects / ambiguities

### 1. Core 101 ownership exists but compat response does not consistently expose it

`eggfetch-core` captures Hyper `OnUpgrade`, wraps `hyper::upgrade::Upgraded`, and attaches `NetworkStream::Upgraded` to core responses. Native `PyResponse` stores `_network_stream`, but the compatibility response wrapper does not consistently place it in the HTTPX `extensions` dict.

Required correction: connect the native response metadata export from Corrective 02 to the compatibility facade so that 101 responses expose a live `network_stream` object when ownership was actually transferred.

### 2. CONNECT semantics are overclaimed

Core response comments/documentation describe successful CONNECT as potentially carrying an upgraded stream, but the existing proxy CONNECT code consumes the tunnel internally to perform origin TLS and send the destination request. That is not equivalent to returning the raw CONNECT tunnel to the caller.

Required correction: distinguish two cases:

1. **HTTP proxy CONNECT used internally to service an HTTPS origin request**: this tunnel remains internal and must not be exposed as a user-owned raw stream.
2. **A user request whose response itself represents a transport upgrade/tunnel the public API can safely hand off**: expose only if there is a real supported public route and ownership transfer.

Do not set `network_stream` for an internal CONNECT tunnel merely because a CONNECT occurred inside the proxy implementation.

### 3. Sync wrapper assumes an ambient Tokio runtime

`PyNetworkStream` methods use `tokio::runtime::Handle::current().block_on(...)`. A stream object can outlive the client or be called in Python code where no current Tokio runtime is entered on that thread. The wrapper therefore needs an explicit runtime/lifecycle design rather than relying on ambient runtime state.

### 4. Async wrapper contract is unclear

The async wrapper currently delegates through runtime/blocking behavior because the underlying trait object has Send/lifetime constraints. Ensure exposed Python methods are genuinely awaitable where the HTTPX/httpcore async contract expects them, do not block the Python event loop unnecessarily, and make cancellation/close behavior deterministic.

### 5. `start_tls` is only partial

The sync wrapper builds default webpki roots internally and does not use the safe SSLContext translation boundary. Hyper adapter-backed upgraded streams cannot necessarily be converted into the concrete type expected by the existing TLS upgrade implementation.

Required correction: either implement `start_tls(ssl_context=..., server_hostname=..., timeout=...)` for the subset of owned streams where it can safely work, using Corrective 01 translation, or explicitly reject unsupported adapter-backed streams with a documented bounded difference. Never consume/lose the underlying stream before determining that the operation is supported.

## Required design

### A. Define ownership classes explicitly

Retain a core enum or equivalent that makes these states unambiguous:

- `OwnedUpgraded`: EggFetch owns bidirectional I/O after a protocol upgrade;
- `MetadataOnly`: immutable connection metadata only, no read/write;
- `Unavailable`: no safe stream/metadata handle can be exposed.

If ordinary pooled responses are always unavailable under Hyper, do not create metadata-only placeholders simply to make the extension present. HTTPX parity documentation should state the precise difference.

### B. Preserve 101 upgrade leading bytes

The existing Hyper rewind behavior and end-to-end leading-data test should remain authoritative.

Add/retain tests proving:

- bytes written immediately after the 101 headers are returned first by `network_stream.read()`;
- no bytes are lost or duplicated;
- body iteration is not attempted on the 101 Hyper `Incoming` body;
- once upgraded, the connection is not returned to the normal HTTP pool.

### C. Expose the stream through response extensions

With Corrective 02's response metadata helper:

- native buffered/upgrade response exposes the stream object;
- compatibility `Response.extensions["network_stream"]` references the wrapper;
- it is absent for ordinary pooled responses;
- custom transport-provided `network_stream` extensions are not overwritten by a missing native value;
- sync response gets sync wrapper; async response gets async wrapper where the native path distinguishes them.

If the compatibility facade currently buffers 101 responses through a code path that would destroy the upgrade object, route upgrade responses through an ownership-preserving conversion path.

### D. Runtime ownership for sync streams

A sync network-stream wrapper must carry or reference a runtime handle/lease that remains valid for the lifetime of the owned stream.

Suggested requirements:

- the wrapper receives a `tokio::runtime::Handle` plus whatever runtime lease is required when constructed from a sync client;
- closing the parent Client does not invalidate an already-transferred upgraded stream unless HTTPX reference behavior does so;
- dropping/closing the stream releases its runtime lease;
- wrapper methods do not use `Handle::current()` as their only runtime source;
- no nested-runtime panic when invoked from a thread that is already inside Tokio; use the repository's established sync-facade bridging pattern.

### E. Async wrapper semantics

For async streams:

- `read`, `write`, `aclose`/`close` equivalents exposed by the reference should be awaitable in the same broad way as httpcore async stream methods;
- operations must not hold the GIL during network I/O;
- cancellation drops or closes the owned I/O safely;
- a client close followed by valid owned-stream operations matches reference behavior or is explicitly classified;
- double close is idempotent;
- use-after-close returns a stable mapped error rather than panicking.

### F. `get_extra_info`

Compare against the pinned httpcore contract and expose only truthful values.

At minimum test/define:

- local/client address;
- peer/server address;
- TLS metadata where known;
- unknown keys -> `None`/reference behavior;
- metadata unavailable through Hyper upgrade -> absent/None rather than fabricated defaults presented as facts.

If current `ConnectionMetadata::default()` yields empty addresses for Hyper 101, document that those keys are unavailable. Prefer capturing addresses at the connector boundary if it can be done without invasive pool changes.

### G. Safe `start_tls`

First make support detection non-destructive.

Required sequence:

1. determine whether the underlying owned I/O supports TLS wrapping;
2. translate the supplied SSLContext/config through Corrective 01 before taking ownership irreversibly;
3. build the TLS connector from the exact representable config;
4. use `server_hostname` for SNI/verification exactly as supported;
5. apply timeout;
6. on successful handshake return a new stream wrapper with TLS metadata;
7. on pre-handshake unsupported/unrepresentable input, leave the original stream usable if the reference semantics permit it;
8. on handshake failure, define whether the underlying stream is consumed/closed by comparison with httpcore and test it.

If Hyper's opaque adapter cannot support the required conversion, explicitly reject `start_tls` for that stream class. Do not pretend default-root TLS is equivalent to caller context semantics.

### H. CONNECT classification

Add tests and docs differentiating internal proxy CONNECT from public upgraded-stream behavior.

Acceptance must not require exposing the internal HTTPS-proxy tunnel. The goal is truthful compatibility, not leaking an internal transport object that would break the request pipeline.

## Required tests

### 101 sync

- local upgrade server returns 101;
- compat response has `extensions["network_stream"]`;
- first read returns leading bytes;
- write/read echo succeeds;
- close is idempotent;
- upgraded stream remains usable to the degree HTTPX allows after Client context exits;
- no connection reuse as ordinary HTTP afterward.

### 101 async

Equivalent async cases with awaited operations and cancellation.

### Ordinary response negative cases

For HTTP/1.1 and HTTP/2, sync/async buffered/streaming:

- `network_stream` is absent when no safe handle is available;
- docs/ledger explicitly classify this difference from httpcore;
- no writable proxy/placeholder object is exposed.

### Internal CONNECT

Use a local proxy + TLS origin:

- normal HTTPS-through-proxy request succeeds;
- response does not expose the internal proxy tunnel as `network_stream` unless there is a specific reference reason it should;
- closing response/client cleans tunnel resources;
- proxy connection cannot be commandeered through response extensions.

### Runtime lifecycle

- stream created on sync client can perform I/O without an ambient Tokio runtime on the Python calling thread;
- client closes while transferred upgrade stream remains owned -> behavior matches declared contract;
- dropping stream releases runtime lease;
- repeated creation/close does not leak worker threads/runtimes materially.

### `start_tls`

Where supported:

- local plaintext upgraded stream -> TLS server transition succeeds with representable context;
- custom CA works;
- wrong CA fails;
- SNI override works;
- timeout works;
- unrepresentable SSLContext fails before destructive ownership transition where feasible.

Where unsupported:

- explicit deterministic error;
- test proves no false success/default-root substitution;
- ledger documents the exact stream class/operation difference.

## Acceptance criteria

Corrective 03 is complete only when:

- [ ] 101-owned streams are visible through HTTPX-compatible response extensions.
- [ ] Leading upgrade bytes are preserved end-to-end.
- [ ] An upgraded connection is never returned to the ordinary HTTP pool.
- [ ] Internal proxy CONNECT tunnels are not incorrectly exposed as user-owned streams.
- [ ] Sync stream methods do not rely solely on ambient `Handle::current()`.
- [ ] Stream/runtime lifetime after Client close is explicitly tested.
- [ ] Async operations are awaitable/cancellation-safe to the extent claimed.
- [ ] `get_extra_info` never fabricates unavailable address/TLS metadata.
- [ ] `start_tls` uses Corrective 01 TLS translation where supported.
- [ ] Unsupported `start_tls` stream classes fail explicitly rather than silently using different TLS policy.
- [ ] Ordinary pooled HTTP/1.1/H2 raw stream absence remains documented if Hyper cannot safely expose it.
- [ ] No pool corruption, socket aliasing, or writable shared H2 connection is introduced.

## Out of scope

Do not replace Hyper solely to expose ordinary pooled sockets. Do not expose writable H2 shared connection I/O. Do not implement WebSocket protocol semantics; this phase only supplies the underlying upgraded byte stream expected by compatibility consumers.