# HTTPX Parity Phase 02 — HTTP/2-Only and Prior-Knowledge Mode

Status: implementation handoff plan.
Depends on: none of the other new parity phases.
Recommended landing: independently, before extension/network-stream work.

## Objective

Close the current explicit mismatch where `eggfetch.compat.httpx` rejects `http1=False, http2=True` even though the Rust core already models `HttpVersionPolicy::Http2Only` and the connector builder already has an HTTP/2-only branch.

The target is HTTPX 0.28.1/httpcore 1.0.9 behavior for both:

- HTTPS HTTP/2-only connections; and
- cleartext HTTP/2 prior knowledge (`h2c` without HTTP/1.1 Upgrade).

## Reference behavior

HTTPX passes `http1` and `http2` directly to httpcore’s connection pool. In httpcore 1.0.9, when `http2=True` and `http1=False`, a connection is treated as HTTP/2 even without ALPN. This is the reference mechanism for cleartext HTTP/2 prior knowledge.

The valid constructor matrix is:

- `http1=True, http2=False`: HTTP/1.1 only;
- `http1=True, http2=True`: negotiate HTTP/2 over TLS where available, otherwise HTTP/1.1; cleartext ordinarily remains HTTP/1.1;
- `http1=False, http2=True`: HTTP/2 only / prior knowledge;
- `http1=False, http2=False`: invalid because no protocol is enabled.

## Current EggFetch state

The core already contains:

- `HttpVersionPolicy::Http1Only`;
- `HttpVersionPolicy::Http2Only`;
- `HttpVersionPolicy::Auto`;
- connector construction with `(enable_http1, enable_http2) == (false, true)` selecting the HTTP/2-only Hyper/rustls connector branch.

The compatibility facade currently stops `http1=False, http2=True` in `_validate_protocol_options()` with `NotImplementedError`, so existing core capability is not exposed or differentially proven.

The key implementation risk is not the enum. It is proving the generated Hyper client actually uses HTTP/2 prior knowledge correctly for plaintext HTTP, HTTPS failure behavior, proxy routes, UDS, and custom transport boundaries.

## Required implementation tracks

### Track 1 — Replace facade rejection with exact validation

Change compatibility protocol validation so:

- both false -> the same class/timing of failure as HTTPX where practical;
- H2-only -> accepted;
- ordinary combinations remain unchanged.

Do not change the native EggFetch `http2=True` meaning unless needed to expose an explicit `http1` control in the native Python API. The compatibility facade may translate the pair directly to a private native protocol policy if that avoids broadening the native public surface.

### Track 2 — Add a native binding for protocol policy if necessary

If `eggfetch.Client`/`AsyncClient` cannot currently express HTTP/2-only separately from `http2=True`, add the smallest binding-level mechanism required.

Preferred options, in order:

1. a private/internal constructor kwarg used only by the compatibility facade;
2. a narrow public `http1=` kwarg only if there is a clear native API use case and documentation is updated;
3. do **not** infer H2-only from unrelated request state.

The binding must map directly to `HttpVersionPolicy::Http2Only`.

### Track 3 — Verify Hyper client construction for cleartext H2

Inspect and test each connector class used by the compatibility layer:

- default hyper-rustls connector;
- advanced direct connector used for `local_address`/`socket_options`;
- UDS connector;
- SOCKS connector where HTTPX can plausibly use H2 over the tunneled stream;
- HTTP proxy CONNECT origin-TLS path.

For direct cleartext TCP, prove that the Hyper client sends the HTTP/2 client preface immediately rather than attempting an HTTP/1.1 request or Upgrade dance.

If one specialized connector cannot support prior knowledge with the existing Hyper builder, keep that route explicitly bounded rather than silently falling back to HTTP/1.1.

### Track 4 — HTTPS H2-only semantics

For TLS targets:

- ALPN should advertise/select only the HTTP/2 protocol for H2-only clients;
- if the server does not negotiate HTTP/2, fail instead of falling back to HTTP/1.1;
- preserve certificate/SNI behavior;
- preserve connect/read/write/pool timeout semantics;
- do not accidentally enable HTTP/3 because EggFetch has an independent HTTP/3 feature.

### Track 5 — Pool and reuse semantics

Verify H2-only clients retain the same logical pooling behavior as the existing HTTP/2 path:

- multiplex requests on one usable H2 connection subject to configured limits;
- do not create an HTTP/1.1 fallback connection after an H2 protocol failure;
- cancellation releases logical permits;
- closing the client terminates H2 connections cleanly;
- redirect hops remain on the normal protocol-selection policy for their new origin.

### Track 6 — Compatibility bookkeeping

Add the constructor/protocol behavior to the pinned parity registry.

If there is an active or historical allowed-difference record for H2-only mode, move it to the resolved ledger only after network differentials pass. If no record exists, add the missing case to the parity registry so the gap cannot regress silently again.

## Required differential fixtures

Use deterministic local servers rather than internet ALPN tests.

### Plaintext prior-knowledge server

Implement a minimal local H2 server fixture that:

- expects the connection preface as the first bytes;
- answers one GET;
- answers multiple multiplexed requests;
- can intentionally reject/close to exercise exception parity.

Run the same requests through HTTPX 0.28.1 and EggFetch.

Assertions:

- both report `HTTP/2`;
- no HTTP/1 request line is sent;
- request path/headers/body remain correct;
- streaming response works;
- repeated requests reuse as expected.

### TLS ALPN fixture

Test servers advertising:

- H2 only;
- HTTP/1.1 only;
- H2 + HTTP/1.1.

For `http1=False, http2=True`, EggFetch must match HTTPX’s success/failure outcome and must never downgrade to HTTP/1.1.

### Negative constructor matrix

Differentially test all four boolean combinations for:

- `Client`;
- `AsyncClient`;
- `HTTPTransport`;
- `AsyncHTTPTransport`.

### Specialized route smoke tests

At minimum:

- H2-only + `local_address`;
- H2-only + safe three-tuple `socket_options`;
- H2-only + UDS where semantically applicable;
- H2-only through HTTP CONNECT to an H2 origin;
- H2-only through SOCKS to an H2 origin if the existing proxy fixture supports it.

If HTTPX itself does not support one combination, document it as outside the reference rather than inventing EggFetch behavior.

## Security and correctness constraints

- never silently fall back to HTTP/1.1 in H2-only mode;
- never disable certificate verification or hostname verification to make H2 negotiation succeed;
- preserve H2 forbidden-header filtering;
- preserve GOAWAY/reset/flow-control error handling;
- cleartext prior knowledge must only be used when the caller explicitly selected H2-only;
- no automatic h2c protocol probing.

## Non-goals

- HTTP/1.1 Upgrade to h2c;
- server push;
- extended CONNECT beyond what the current client supports;
- changing HTTP/3 policy;
- redesigning the H2 error taxonomy.

## Acceptance criteria

This phase is complete when:

1. `Client(http1=False, http2=True)` and `AsyncClient(...)` construct successfully in the compatibility facade.
2. `HTTPTransport` and `AsyncHTTPTransport` accept the same combination.
3. Plain HTTP requests in H2-only mode send the HTTP/2 connection preface and succeed against a prior-knowledge H2 server.
4. TLS H2-only succeeds when H2 is negotiated and fails rather than downgrading when only HTTP/1.1 is available.
5. Response `http_version` matches the reference.
6. Sync/async streaming and cancellation behave correctly.
7. Existing ordinary `http2=True` negotiation behavior is unchanged.
8. Focused HTTPX differential tests pass with no capability skips on the primary Linux development environment.
9. The parity/allowed-difference ledger no longer treats H2-only as unsupported.
10. `./scripts/check.sh` passes.
