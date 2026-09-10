# HTTPX2 2.12 SSE and WebSocket Parity

Planning baseline: `4456680361dbfdc2d15b0d43a18cdcb20704f667` (`main`, 2026-09-10)
Parent program: `plans/http3-and-next-httpx-compatibility-program.md`
Depends on: `plans/httpx2-2.12-core-facade-parity.md`

## Objective

Implement HTTPX2 2.12's higher-level streaming protocol surface—native server-sent events and the optional WebSocket API—without introducing any independent socket/HTTP stack outside `eggfetch-core`.

EggFetch already has true streamed responses and a 101 `network_stream` abstraction. SSE should therefore be a framing/parser layer over normal response streaming. WebSockets should reuse the existing HTTP upgrade/network-stream ownership path and add protocol framing above it.

## 1. Pin reference API and dependency behavior

Before implementation, derive exact API/signature/error manifests for HTTPX2 2.12 SSE and `ws` surfaces, including behavior when optional WebSocket dependencies are absent.

Capture:

- sync/async entry points;
- context-manager behavior;
- event/message objects and fields;
- exception hierarchy;
- close/cancellation semantics;
- optional-extra import failures;
- timeout/proxy/auth/header interactions.

Acceptance:
- [ ] all public SSE/WS symbols and signatures are enumerated;
- [ ] optional dependency behavior has direct reference probes;
- [ ] no private upstream object is promoted accidentally.

## 2. SSE parser and lifecycle

Implement SSE at the Python compatibility layer unless profiling demonstrates a compelling core reason otherwise. It is application framing, not network transport.

Required semantics:

- parse UTF-8 event-stream fields and comments;
- `data`, `event`, `id`, retry handling as reference exposes them;
- CR/LF/CRLF chunk-boundary correctness;
- multi-line data concatenation;
- BOM/empty field behavior as reference;
- streaming incrementally without whole-response buffering;
- sync and async iteration;
- response/context close propagation;
- cancellation releases the native body/pool lease;
- `max_event_size` bounds event buffering exactly or more safely with an explicit documented difference.

Do not add a second Rust network reader for SSE.

Acceptance:
- [ ] byte-by-byte/chunk-split differential tests match reference events;
- [ ] oversized never-terminated events are bounded;
- [ ] dropping/closing an SSE stream releases connection resources;
- [ ] sync/async behavior is equivalent where reference says it is.

## 3. SSE reconnect boundaries

If HTTPX2's SSE API includes reconnect behavior, implement only the observed contract. Reconnection must flow through the normal EggFetch client/request pipeline so redirects, cookies, auth, proxies, timeouts and retries retain their existing semantics.

Never replay arbitrary request bodies as part of SSE reconnection.

Acceptance:
- [ ] reconnect behavior is directly reference-tested;
- [ ] retry/id fields cannot override security or native retry policy unexpectedly.

## 4. WebSocket handshake over existing HTTP machinery

Implement the HTTP WebSocket handshake using the existing request pipeline where possible:

- correct `Upgrade`, `Connection`, `Sec-WebSocket-*` headers;
- key/accept validation;
- subprotocol negotiation;
- cookies/auth/redirect policy according to HTTPX2 reference behavior;
- proxy/SOCKS/TLS routing through existing core connectors;
- acquire the writable stream only from the existing 101 `network_stream` extension;
- reject non-101/invalid handshake responses with reference-compatible errors.

Do not open raw TCP/TLS sockets from Python.

Acceptance:
- [ ] handshake works over direct HTTP/HTTPS and reference-required proxy/SOCKS routes supported by EggFetch;
- [ ] TLS verification/SNI is identical to normal requests;
- [ ] non-upgrade responses cannot expose writable pooled sockets.

## 5. WebSocket protocol framing

Add a bounded WebSocket framing/state layer on top of `NetworkStream`/`UpgradedStream`.

Required behavior according to HTTPX2 2.12's vendored WS contract:

- client masking;
- text/binary messages;
- fragmentation/reassembly;
- ping/pong;
- close handshake and close codes/reasons;
- maximum message-size enforcement across fragmented messages;
- protocol errors;
- unsolicited/duplicate pong behavior matching the pinned reference;
- partial reads/writes and frame boundaries;
- backpressure rather than unbounded buffering.

Select a small maintained protocol dependency only if it materially reduces correctness risk and fits dependency policy; otherwise isolate the framing implementation with strong tests/fuzzing. Do not vendor a large HTTP client merely for WebSocket framing.

Acceptance:
- [ ] RFC-level framing corpus plus HTTPX2 differential tests pass;
- [ ] max-message applies to total fragmented message, not each fragment;
- [ ] invalid UTF-8/control frames/masking/length encodings fail safely;
- [ ] buffers have explicit maxima.

## 6. Sync/async ownership and cancellation

Reuse the current sync/async `network_stream` wrappers and runtime leases.

Guarantees:

- only one logical WebSocket owner controls the upgraded stream;
- sync operations release the GIL around blocking I/O;
- async operations are cancellation-safe;
- close is idempotent;
- dropping a WebSocket cannot return an upgraded connection to the ordinary HTTP pool;
- concurrent send/receive rules match reference behavior or are explicitly bounded.

Acceptance:
- [ ] cancellation/drop tests show no leaked runtime lease or open upgraded socket;
- [ ] leading bytes after 101 remain visible to the WS decoder;
- [ ] close during partial frame read/write terminates deterministically.

## 7. Proxy/SOCKS/WebSocket security regression

HTTPX2 has had post-fork WebSocket/SOCKS/TLS correctness fixes. Add explicit network tests proving:

- `wss://` through SOCKS still performs origin TLS;
- HTTP CONNECT/HTTPS proxy trust remains isolated from origin trust;
- proxy headers never reach the WebSocket origin;
- credentials are redacted from handshake/protocol errors;
- redirects cannot leak Authorization/Cookie across origins;
- oversized WS frames/messages cannot exhaust memory before rejection.

Acceptance:
- [ ] direct differential/reference tests cover each supported topology;
- [ ] any intentional topology limitation is recorded in the HTTPX2 profile rather than silently skipped.

## 8. Optional-extra packaging semantics

Match the declared HTTPX2 compatibility contract without forcing every EggFetch install to carry a large WS dependency.

Prefer an optional Python/package feature or dependency that produces clear errors when the WS API is requested without support. Ensure package validation tests both base install and enabled optional surface if packaging machinery supports it without a new CI architecture.

Acceptance:
- [ ] base HTTP client/SSE do not require WebSocket-only dependencies;
- [ ] enabled WS surface is package-smoke tested;
- [ ] missing optional dependency behavior is documented and reference-compatible where practical.

## 9. Fuzz/property testing

Add fuzz/property targets where EggFetch owns untrusted framing:

- SSE line/event parser;
- WebSocket frame parser/state transitions;
- fragmented size accounting;
- close/control-frame validation.

If a mature framing crate owns parsing, focus EggFetch tests on adapter/lifecycle state rather than duplicating its fuzz suite.

## Validation

Run focused HTTPX2 SSE/WS differential tests, existing `network_stream`/upgrade tests, HTTPX 0.28.1 regressions, and Tier 1. Keep external interoperability fixtures in Tier 2/manual validation if needed.

## Non-goals

- a generic browser WebSocket implementation unrelated to HTTPX2;
- WebTransport;
- HTTP/2 extended CONNECT WebSockets unless required by the pinned reference surface;
- a second socket/TLS implementation in Python;
- automatic SSE retries beyond reference semantics;
- Stage C declaration before final freeze.

## Exit criteria

- [ ] HTTPX2 SSE API is incremental, bounded, and lifecycle-safe;
- [ ] HTTPX2 optional WebSocket API works through EggFetch's existing upgrade transport;
- [ ] proxy/SOCKS/TLS security cases pass;
- [ ] sync/async cancellation and close behavior pass;
- [ ] API/differential corpus has no unexplained SSE/WS gaps in the declared target;
- [ ] HTTPX 0.28.1 regressions and Tier 1 remain green.