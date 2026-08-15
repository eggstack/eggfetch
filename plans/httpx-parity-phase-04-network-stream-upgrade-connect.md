# HTTPX Parity Phase 04 — `network_stream`, CONNECT, and Upgrade Handoff

Status: implementation handoff plan with mandatory feasibility gate.
Depends on: Phase 03 typed request/response transport metadata.
Risk: high; connection ownership and pool safety.

## Objective

Implement the useful HTTPX 0.28.1/httpcore 1.0.9 `response.extensions["network_stream"]` behavior without corrupting Hyper-managed pooled connections.

Priority order:

1. correct HTTP/1.1 `101 Switching Protocols` ownership transfer;
2. correct successful HTTP CONNECT ownership transfer;
3. `get_extra_info()` metadata for live ordinary responses;
4. raw read/write/start-TLS on ordinary pooled connections only if it can be provided safely;
5. HTTP/2 shared-network-stream metadata without pretending a multiplexed stream is a private socket.

This phase must not expose a raw socket handle that remains concurrently owned by Hyper.

## Reference behavior

httpcore 1.0.9 exposes a `NetworkStream` in every HTTP/1.1 and HTTP/2 response extension.

The interface shape is:

- `read(max_bytes, timeout=None) -> bytes`;
- `write(buffer, timeout=None)`;
- `close()`;
- `start_tls(ssl_context, server_hostname=None, timeout=None) -> NetworkStream`;
- `get_extra_info(info) -> Any`.

For HTTP/1.1:

- ordinary responses expose the connection’s network stream;
- `101` responses and successful CONNECT responses wrap the stream in an upgrade object;
- any bytes already read past the response head are preserved as leading data and returned before new socket reads.

For HTTP/2:

- `network_stream` refers to the shared underlying connection stream;
- `stream_id` separately identifies the logical H2 stream.

HTTPX documents this extension as the low-level basis for CONNECT, protocol upgrades, WebSockets, TLS/socket inspection, and address metadata.

## Current EggFetch state

The direct Hyper path currently:

1. awaits `hyper_client.request(request)`;
2. copies status/version/headers;
3. calls `hyper_response.into_body()`;
4. wraps the body into an EggFetch byte stream.

No underlying connector/stream handle, Hyper upgrade future, socket address metadata, or TLS session metadata is retained on the response.

The existing Python live-stream work already has runtime-lease machinery for a response body that outlives client close. Reuse that ownership discipline where possible rather than creating another unrelated lifetime mechanism.

## Mandatory feasibility gate

Before broad implementation, create a small internal proof that answers these questions using the exact Hyper/hyper-util versions in the workspace:

1. Can an `OnUpgrade`/upgrade future be captured from a client response before consuming the body?
2. Does it resolve correctly for both `101` and CONNECT 2xx responses in the current legacy client path?
3. Can the upgraded IO be converted into a Tokio `AsyncRead + AsyncWrite` object owned solely by EggFetch after handoff?
4. Can local/peer socket addresses be captured from the connector without exposing raw ownership?
5. Can TLS negotiated information be captured or safely queried after handshake?
6. Can ordinary pooled connections expose metadata without making the socket independently writable outside Hyper?
7. What does the HTTP/2 connector expose for underlying connection metadata, and can it be associated with responses without a global/racy lookup?

If 1-3 fail with the current transport abstraction, stop this phase and write a decision note. Do not replace the entire HTTP stack as an incidental compatibility patch.

## Required implementation tracks

### Track 1 — Add typed connection metadata to connector IO

Wrap connector-produced IO in a small core type that can retain read-only metadata while still implementing the IO traits Hyper expects.

Potential metadata:

- local/client socket address;
- peer/server socket address;
- transport kind (TCP, UDS, SOCKS tunnel, proxy tunnel);
- TLS negotiated protocol/version/cipher data that rustls exposes safely;
- SNI/server name used;
- a connection identity token for correlation, not a raw fd.

Requirements:

- metadata may be shared through `Arc`;
- no method may allow arbitrary raw reads/writes while Hyper still owns the connection;
- no raw file descriptor/socket object is exposed to Python;
- secret material and TLS keys are never retained.

### Track 2 — Preserve Hyper response extensions before `into_body()`

Refactor direct and specialized send paths so they can inspect/take relevant Hyper response extensions before consuming the response into an EggFetch body.

Capture upgrade state only for responses where it is meaningful.

Do not discard the body state needed to finish an ordinary response.

### Track 3 — Implement a core upgraded-stream owner

Create an internal `UpgradedStream`/`HijackedStream` abstraction that owns the post-HTTP IO after Hyper transfers it.

It must provide async operations:

- read up to N bytes with an optional deadline;
- write all supplied bytes with an optional deadline;
- shutdown/close;
- start a new TLS layer where a rustls configuration is safely available;
- get read-only metadata.

Ownership rules:

- once handoff succeeds, the HTTP pool must never reuse that connection;
- closing the response and closing the upgraded stream must be idempotent and not double-close;
- cancellation during the upgrade future must not leak the connection;
- the upgraded stream may outlive the `Client` if its runtime/transport lease remains valid, matching the existing live-stream principle.

### Track 4 — Preserve leading data

httpcore explicitly preserves bytes that h11 read beyond the response headers before a CONNECT/Upgrade handoff.

Determine how Hyper surfaces already-buffered bytes in its upgraded IO. If Hyper’s `Upgraded` object already includes them, test and rely on that behavior. If not, capture and prepend them in an EggFetch wrapper.

Required test: server sends the `101`/CONNECT response headers and the first application-protocol bytes in one write. The first `network_stream.read()` must return those bytes rather than losing them.

### Track 5 — Python sync/async network-stream wrappers

Implement compatibility objects with the HTTPX/httpcore method names.

Sync wrapper:

- uses the same runtime ownership discipline as native sync streaming;
- releases the GIL while waiting on Rust async read/write/TLS operations;
- preserves per-call timeout behavior.

Async wrapper:

- directly awaits Rust operations through the existing Tokio/PyO3 async bridge;
- is cancellation-safe;
- does not create a nested runtime.

Exception mapping should use HTTPX-compatible `ReadTimeout`, `WriteTimeout`, `ConnectTimeout`, `ReadError`, `WriteError`, or protocol errors as appropriate.

### Track 6 — `get_extra_info()` compatibility

Implement the subset of reference keys EggFetch can provide exactly.

Required baseline keys:

- `client_addr`;
- `server_addr`;
- `is_readable` or equivalent only if it can be determined without consuming data;
- TLS information through an EggFetch compatibility object where the reference returns `ssl_object`.

Important: a rustls TLS session is not a Python `ssl.SSLObject`. Do not fabricate one. If the reference key `ssl_object` cannot be represented by the required Python type, expose only safely representable metadata keys and keep `ssl_object` as an explicit residual difference.

A compatibility wrapper may expose read-only negotiated TLS properties, but it must not masquerade as `ssl.SSLObject` unless it actually satisfies that contract.

### Track 7 — `start_tls()`

For an upgraded raw TCP stream, `start_tls()` is useful for CONNECT tunnels and STARTTLS-like protocols.

Integrate with the safe TLS translation work from Phase 01:

- representable `ssl.SSLContext` -> rustls config -> wrap the owned stream;
- optional `server_hostname` overrides the TLS server name;
- timeout applies to the handshake;
- on handshake failure, close or leave stream state exactly as the chosen contract specifies and test against the reference.

Do not support arbitrary unrepresentable Python contexts here more broadly than Phase 01 allows.

### Track 8 — Ordinary HTTP/1.1 response behavior

Separate metadata compatibility from raw-I/O compatibility.

Safe minimum:

- attach a network-stream compatibility object that can return connection metadata while the response is live;
- `close()` may map to response/connection close semantics where exact;
- raw `read`/`write` must not bypass HTTP framing on a normal pooled connection unless ownership can be exclusively transferred.

If the reference permits raw operations that EggFetch cannot safely expose while Hyper owns the connection, retain a narrow tested difference. Do not corrupt the pool to pass an API-shape test.

### Track 9 — HTTP/2 semantics

HTTP/2 uses one connection for multiple logical streams. The response extension in httpcore points to the shared network stream; direct arbitrary reads/writes can interfere with all active H2 streams.

EggFetch must:

- expose read-only connection metadata if available;
- pair it with Phase 03’s real `stream_id` if that is implementable;
- never expose a supposedly private per-response raw socket;
- document any restricted read/write behavior if Hyper does not permit safe shared-stream access.

Do not downgrade an H2 response to a dedicated connection just to emulate the object shape.

## Required local fixtures

### HTTP/1.1 Upgrade fixture

Server behavior:

- accept request with upgrade headers;
- return `101 Switching Protocols`;
- include application bytes immediately after the headers;
- echo subsequent bytes bidirectionally.

Assertions against HTTPX and EggFetch:

- response status/headers;
- extension presence;
- leading bytes preserved;
- read/write round trip;
- partial read sizes;
- timeout;
- explicit close;
- client close while upgraded stream remains owned.

### CONNECT fixture

Origin/proxy fixture should allow a successful CONNECT and then raw echo bytes.

Assertions:

- tunnel is not returned to HTTP pool;
- network stream can exchange bytes;
- timeout/error mapping;
- optional TLS start inside tunnel where Phase 01 can represent the context;
- proxy credentials/headers do not appear in tunneled origin data.

### Metadata fixture

For a normal streamed TLS response, compare the shape and values that can be meaningfully matched:

- local/peer address family and port relationships;
- TLS negotiated version/protocol where available;
- lifecycle before and after response close.

### HTTP/2 fixture

Verify that multiple responses can share connection metadata while retaining distinct stream IDs. Ensure closing one response does not close the underlying H2 connection unless required.

## Security requirements

- no duplicated ownership of raw IO;
- no use-after-close or double-close;
- no raw fd/socket leakage to Python;
- no raw write access to a connection still managed by Hyper;
- no TLS key/session secret exposure;
- upgraded connections are removed from reusable pool state;
- timeouts and cancellation close/release resources deterministically;
- CONNECT/Upgrade must not bypass proxy/header redaction rules;
- no `unsafe` additions.

## Non-goals

- replacing Hyper/hyper-util;
- arbitrary socket monkey-patching;
- WebSocket framing itself;
- HTTP/2 extended CONNECT implementation unless already supported by the underlying stack;
- pretending rustls objects are CPython `ssl.SSLObject` instances.

## Acceptance criteria

This phase is complete when:

1. The feasibility gate documents how upgrade ownership works in the pinned Hyper stack.
2. HTTP/1.1 `101` and successful CONNECT can hand the connection to a core-owned upgraded-stream object without pool reuse or double ownership.
3. Leading data following response headers is preserved exactly.
4. Sync and async compatibility wrappers support read/write/close with reference-like timeout/error behavior for handed-off streams.
5. `start_tls()` works for Phase-01-representable TLS configurations or is precisely bounded where not representable.
6. `get_extra_info()` exposes real metadata only; no fabricated `ssl_object` or address values.
7. Ordinary HTTP/1.1 and HTTP/2 connections remain pool-safe. Any raw-I/O limitation on non-upgraded responses is explicitly tested and documented.
8. Client close/cancellation/resource-release tests show no leaked connection/runtime lease.
9. Pinned HTTPX differential Upgrade/CONNECT tests pass for all implemented operations.
10. `./scripts/check.sh` passes.

If exclusive handoff cannot be obtained from the current Hyper path, closure is a written architectural decision and retained difference, not an unsafe workaround.
