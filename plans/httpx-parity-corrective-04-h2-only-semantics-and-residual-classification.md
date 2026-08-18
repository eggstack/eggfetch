# HTTPX Parity Corrective 04 — H2-Only Semantics and Residual Classification

Baseline reviewed: `4571cb55bc2ff49822608d750dfef185cff40ebc`
Reference: `httpx==0.28.1` / `httpcore==1.0.9`
Depends on: Corrective 02 for final extension/metadata behavior.

## Objective

Resolve the mismatch between the current “H2-only enabled” claim and the actual HTTPX semantics of `http1=False, http2=True`. Implement the missing behavior where it can be done cleanly within the existing Hyper/h2 architecture; otherwise retain each unsupported behavior as a precise bounded difference and ensure the compatibility profile does not describe it as full prior-knowledge parity.

This phase also performs the final `stream_id` feasibility check. It must not trigger a Hyper replacement or a parallel transport stack solely to expose metadata.

## Reference semantics that matter

For the pinned HTTPX/httpcore behavior, `http1=False, http2=True` means HTTP/2 is required rather than preferred.

Important observable cases:

1. **TLS destination advertising H2 via ALPN:** request succeeds as HTTP/2.
2. **TLS destination that only supports HTTP/1.1:** HTTPX does not silently downgrade to HTTP/1.1 when HTTP/1 is disabled.
3. **Cleartext HTTP destination:** httpcore can use HTTP/2 prior knowledge when HTTP/1 is disabled and HTTP/2 enabled.
4. **Reported response version:** successful H2-only requests report HTTP/2.

The current EggFetch tests explicitly record deviations in cases 2 and 3 and specialized direct-connector routes.

## Current gaps

### 1. H2-only TLS can silently fall back to HTTP/1.1

The facade accepts the flag combination, but the connector behavior does not strictly enforce the negotiated protocol. A TLS server advertising only `http/1.1` can receive a successful HTTP/1 request from an H2-only EggFetch client.

That is not a cosmetic difference; it changes the transport protocol contract.

### 2. Cleartext H2 prior knowledge is not supported

HTTPX/httpcore sends the HTTP/2 client preface directly for cleartext HTTP when configured H2-only. EggFetch's standard Hyper-rustls path currently sends/falls back to HTTP/1.1 for cleartext connections.

### 3. Specialized direct connector routes do not provide H2

Requests using options such as `local_address` or `socket_options` switch to a custom direct connector path whose H2 capability differs from the ordinary pooled Hyper path.

### 4. Current tests normalize candidate differences instead of closing them

The H2 differential suite contains separate candidate assertions documenting fallback/absence. That is useful evidence, but those rows must remain active allowed differences if they are not implemented. A passing candidate-only test is not evidence of parity.

### 5. `stream_id` remains unavailable through Hyper

The existing residual analysis is sound unless the exact currently pinned Hyper/hyper-util versions expose a supported response extension or API seam carrying the real h2 stream ID.

## Required feasibility work

Before coding, perform a focused source/API inspection of the exact dependency versions in `Cargo.lock` and record the result in the implementation commit/closure record.

### A. TLS H2-only enforcement

Determine whether the existing connector stack can:

- advertise only `h2` ALPN when policy is H2-only;
- reject a TLS connection when negotiated ALPN is absent or `http/1.1`;
- prevent Hyper from constructing/sending an HTTP/1.1 request on an H2-only policy;
- do so without breaking Auto/H1-only policy or duplicating the networking stack.

Potential implementation seams may include connector-level ALPN configuration and post-handshake protocol validation before handing the stream to Hyper.

If cleanly possible, implement it.

If the abstraction prevents enforcement without a large connector rewrite, retain a bounded difference. But the resulting docs must say “H2-only flag accepted; TLS protocol enforcement differs” rather than “H2-only parity complete.”

### B. Cleartext h2 prior knowledge

Inspect whether the existing Hyper client builder or lower-level `hyper::client::conn::http2` path can be integrated into the existing core transport dispatch for the H2-only cleartext case without creating a second general networking implementation.

An acceptable implementation must:

- remain in `eggfetch-core`;
- reuse existing DNS/TCP timeout/local-address/socket-option infrastructure as much as possible;
- send the H2 prior-knowledge preface directly;
- support normal request/response body streaming and cancellation;
- use existing pool/limit semantics or a clearly equivalent H2 connection path;
- not create a Python-specific transport path.

A narrowly selected H2 connection handshake within the canonical core transport is acceptable. A parallel “HTTPX-only” client engine is not.

If implementing h2c would require disproportionate duplication or destabilize pool behavior, retain it as a bounded difference.

### C. Specialized direct connector H2

If H2 support can be added to the direct connector using the same policy/handshake approach as standard connections, do so and test it.

If not, classify separately:

- `local_address` + H2-only
- socket-options + H2-only
- any UDS/H2 behavior if part of the current public compatibility surface

Do not collapse several distinct transport constraints into one vague “H2 difference.”

### D. Stream ID feasibility

Inspect exact pinned Hyper/hyper-util/h2 source/API for a supported real-ID path.

Allowed outcomes:

1. **Real ID is exposed through a stable public API/response extension:** thread it through core/Python response extensions and add differential tests.
2. **No supported path:** retain `stream_id` residual exactly as absent.

Forbidden approaches:

- parse debug strings;
- infer IDs from request ordering;
- assign sequential local numbers;
- return `0` or another sentinel as if it were the stream ID;
- fork/replace Hyper solely for this metadata field.

## Implementation requirements if TLS enforcement is feasible

When `HttpVersionPolicy::Http2Only` is active for TLS:

- configure ALPN for H2-only, not `[h2, http/1.1]`;
- after handshake, verify negotiated protocol is `h2` before HTTP dispatch;
- failure maps to a stable protocol/connect error consistent with the compatibility layer's differential expectation;
- no HTTP/1 bytes are sent after a non-H2 negotiation;
- H1-only and Auto behavior remain unchanged;
- proxy CONNECT destination TLS uses the same H2-only enforcement when the eventual origin request is configured H2-only, if the proxy transport supports H2 at that layer.

If the proxy/manual transport currently supports only HTTP/1.1 inside CONNECT, document that separately rather than falsely reporting H2.

## Implementation requirements if cleartext H2 is feasible

- H2-only cleartext sends the standard HTTP/2 connection preface as the first application bytes.
- No HTTP/1.1 upgrade (`Upgrade: h2c`) is required unless that is what the pinned reference does for the tested path; match prior-knowledge behavior.
- A server that only understands HTTP/1 fails rather than receiving an HTTP/1 fallback.
- multiplexing/reuse works for multiple requests on one H2 connection.
- response `http_version` is HTTP/2.
- cancellation/streaming semantics reuse existing core body abstraction.

## Required differential tests

All tests use local fixtures.

### Protocol matrix

For both sync and async compatibility clients:

| Client policy | Server | Reference | Candidate required outcome |
| --- | --- | --- | --- |
| H1 only | H1 TLS | success H1 | success H1 |
| H1 only | H2 TLS that accepts H1 too | H1 behavior | matching behavior |
| H2 only | H2 TLS | success H2 | success H2 |
| H2 only | H1-only TLS | failure | failure if implemented; otherwise active bounded-difference row |
| Auto H1+H2 | H2 TLS | negotiated behavior | matching negotiated behavior |
| H2 only | cleartext H2 prior-knowledge server | success H2 | success if implemented; otherwise active bounded-difference row |
| H2 only | cleartext H1 server | failure | no silent H1 fallback |

### Wire proof

For H2-only cleartext, capture first bytes accepted by fixture and assert H2 preface when claiming support.

For H2-only TLS vs H1-only peer, fixture must prove candidate does not send a valid HTTP/1.1 request after negotiation when claiming enforcement.

### Specialized paths

Repeat representative H2-only tests with:

- `local_address`;
- one supported socket option;
- proxy route if current docs claim H2 capability through it;
- UDS only if HTTPX differential semantics and EggFetch transport make it relevant.

Each unsupported route gets its own explicit parity/residual case.

### Streaming/reuse

For supported H2-only routes:

- multiple concurrent or sequential requests reuse/multiplex appropriately;
- streaming body response works;
- async cancellation does not poison the entire connection beyond reference behavior;
- reported HTTP version is HTTP/2.

### Stream ID

If implemented, assert the candidate ID is the actual numeric stream identifier and differential behavior matches broad reference shape.

If not implemented, test that it is absent and keep the active residual.

## Ledger/documentation rules

If a behavior remains unsupported, add/retain a stable active difference with:

- exact symbol/option;
- exact reference behavior;
- exact EggFetch behavior;
- reason rooted in transport abstraction;
- concrete differential test names;
- migration impact.

Do not mark a behavior `resolved` merely because the constructor accepts the option.

Update `docs/residual-differences.md` so `stream_id` and H2 limitations are separated. A user should be able to tell whether they are facing:

- metadata-only absence (`stream_id`);
- protocol enforcement difference;
- h2c prior-knowledge absence;
- specialized transport limitation.

## Decision rule

Prefer correctness and architectural coherence over nominal parity percentage.

Implement a missing H2 behavior only when it can be expressed as part of the canonical `eggfetch-core` transport architecture with bounded complexity. If closure would require a second HTTP stack or destabilizing replacement of Hyper, retain a precise bounded difference and make qualification claims narrower.

## Acceptance criteria

Corrective 04 is complete only when:

- [ ] Exact dependency-level feasibility is recorded for TLS H2-only enforcement, h2c prior knowledge, specialized direct connector H2, and `stream_id`.
- [ ] H2-only constructor acceptance is no longer conflated with protocol parity.
- [ ] TLS H2-only either rejects H1 negotiation correctly or remains an explicit active bounded difference with differential evidence.
- [ ] Cleartext prior knowledge either sends real H2 preface or remains an explicit active bounded difference.
- [ ] Specialized direct-connector H2 behavior is implemented or separately classified.
- [ ] No H2-only path silently reports H2 while transmitting HTTP/1.1.
- [ ] Auto and H1-only policies do not regress.
- [ ] `stream_id` is actual or absent; never synthesized.
- [ ] Sync and async differential protocol tests cover all supported/retained cases.
- [ ] Documentation and ledgers distinguish protocol differences from metadata differences.
- [ ] No second compatibility-only networking backend is introduced.

## Out of scope

Do not replace Hyper/hyper-util solely for h2c or `stream_id`. Do not add HTTP/2 server support, HTTP/3 changes, or a broad transport refactor unrelated to the exact H2-only compatibility behaviors above.