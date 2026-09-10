# HTTP/3 Alt-Svc Discovery, Fallback, and Draining

Planning baseline: `4456680361dbfdc2d15b0d43a18cdcb20704f667` (`main`, 2026-09-10)
Parent program: `plans/http3-and-next-httpx-compatibility-program.md`
Depends on: completed post-audit H3 lifecycle hardening
Followed by: `plans/http3-interoperability-and-production-graduation.md`

## Objective

Convert HTTP/3 from an explicitly selected QUIC route into a transport that can be selected safely on the real Internet. Add authenticated Alt-Svc discovery, bounded alternative-service state, failure suppression/fallback, and graceful H3 draining without introducing hidden retries or weakening the existing request replay rules.

The current `H3Connector` already owns per-origin QUIC connection state. Alt-Svc state must remain logically separate: an origin may advertise an alternative even when no QUIC connection exists, and a failed H3 connection must not erase the origin's advertised HTTPS semantics.

## 1. Pin current behavior before changing routing

Add focused tests that capture today's semantics:

- `Http3Only` directly uses H3 and does not fall back to H2/H1;
- `Auto { allow_http3: false }` never attempts H3;
- `Auto { allow_http3: true }` currently chooses the explicit H3 path;
- proxy/SOCKS/UDS/specialized direct precedence is unchanged;
- one-shot request bodies are never replayed by the H3 transport;
- only existing retry policy may cause a new logical attempt.

Acceptance:
- [ ] tests fail if an implementation accidentally changes explicit `Http3Only` semantics;
- [ ] route-precedence and replay invariants are pinned before discovery is added.

## 2. Introduce a bounded Alt-Svc model

Create an internal alternative-service cache owned by the client/configured transport layer, separate from `H3Connector.sender_cache`.

Required model:

- origin key is normalized scheme/host/port of the authenticated HTTPS origin;
- record only supported HTTP/3 protocol identifiers; ignore unsupported alternatives;
- retain alternative authority/port and expiration time;
- support `ma` expiry and `clear` semantics;
- cap cache size and evict deterministically enough for tests without introducing a heavy LRU dependency unless justified;
- expired entries are lazily or periodically removed without background-task leakage;
- no credentials, auth headers, cookies, or URL userinfo are stored in the cache;
- never treat an Alt-Svc advertisement as authorization to change the logical origin for cookies/auth/Host/security policy.

Parsing should be isolated in a small testable module. Malformed fields, quoted values, duplicate alternatives, unknown protocol IDs, zero max-age, overflow, and hostile header sizes must fail closed or be ignored according to the chosen RFC semantics.

Acceptance:
- [ ] malformed/untrusted Alt-Svc values cannot panic or allocate without a configured bound;
- [ ] origin identity and alternative transport authority remain separate types/concepts;
- [ ] cache expiry/clear/bounds have deterministic tests.

## 3. Accept advertisements only from trustworthy response contexts

An Alt-Svc record may be learned only from a response that represents an authenticated HTTPS origin under the existing TLS policy. Do not learn alternatives from plaintext HTTP, failed verification, proxy metadata, synthetic/mock responses unless explicitly test-injected, or a response whose logical origin differs because of redirect state.

For alternative authorities, preserve origin authentication: QUIC TLS must validate the certificate for the original origin/SNI rules required by Alt-Svc, not silently trust the alternative hostname as a new security origin.

Acceptance:
- [ ] HTTP responses cannot install H3 alternatives;
- [ ] cross-origin redirect responses populate only their own logical origin entry;
- [ ] alternative host/port routing cannot leak origin credentials to a different logical origin;
- [ ] TLS/SNI tests demonstrate origin authentication is preserved.

## 4. Define discovery semantics for `Auto`

Do not silently redefine `Http3Only`.

For Auto mode, separate "H3 is permitted" from "an H3 route is known". A conservative first implementation should:

1. send HTTPS over the normal H2/H1 path when no fresh H3 alternative is cached;
2. observe a valid Alt-Svc advertisement;
3. use the cached H3 alternative on a later eligible request;
4. keep explicit `Http3Only` available for callers that require QUIC directly.

If API changes are needed, prefer a small explicit policy enum/config over ambiguous booleans, while preserving current public behavior through compatible defaults/aliases until documented migration is appropriate.

Acceptance:
- [ ] no fresh Alt-Svc entry means Auto does not invent an H3 endpoint;
- [ ] a fresh supported entry can select H3 on a subsequent request;
- [ ] existing default (`allow_http3: false`) remains non-H3 unless an intentional documented API change is made later in the graduation plan.

## 5. Add broken-route suppression

A repeatedly failing advertised H3 alternative must not impose a QUIC timeout on every request.

Track bounded, per-origin/alternative suppression state with:

- failure timestamp/reason class;
- exponential or capped backoff using deterministic clock/test hooks;
- successful H3 use clears or decays suppression;
- a newly changed Alt-Svc advertisement/generation can become eligible without waiting on stale failure state;
- protocol errors and connectivity/UDP reachability failures are distinguishable where useful, but do not create a fragile taxonomy based on error strings.

This is routing suppression, not request retry policy.

Acceptance:
- [ ] repeated unreachable UDP/H3 endpoints are skipped during suppression;
- [ ] suppression is bounded and expires;
- [ ] recovery is tested;
- [ ] metrics expose attempts/suppression/fallback without secrets.

## 6. Implement safe H3-to-H2/H1 fallback

Fallback must obey request-body replayability and phase boundaries.

Rules:

- if H3 fails before any request body/application data is committed and the request can safely be replayed, Auto may fall back according to explicit policy;
- if body bytes may have been delivered, do not silently replay non-idempotent or one-shot requests;
- `Http3Only` returns the H3 failure rather than falling back;
- retry policy remains the only mechanism for later logical attempts after ambiguous delivery;
- total/connect/write/read deadlines remain monotonic and are not restarted by fallback;
- fallback cannot bypass proxy configuration or alter TLS verification.

Introduce an explicit internal dispatch outcome/state if needed rather than inferring replay safety from error strings.

Acceptance:
- [ ] GET/replayable cases fall back only at documented safe boundaries;
- [ ] streamed/non-replayable POST bodies are never duplicated;
- [ ] deadlines cover the complete discovery/attempt/fallback operation;
- [ ] `Http3Only` remains strict.

## 7. Add GOAWAY/draining-aware H3 connection lifecycle

Extend the H3 connection wrapper so graceful peer shutdown is not treated the same as an arbitrary transport failure.

Required behavior:

- observe available h3 connection/driver close or GOAWAY state from the pinned h3 API;
- mark a connection draining and stop assigning new requests to it when the peer indicates no new requests should be accepted;
- allow eligible in-flight streams to complete;
- evict the drained generation after use and reconnect on a future request;
- preserve HTTP/3 application close codes in internal diagnostics where upstream exposes them;
- work around upstream h3 bugs only with narrow, documented version-gated behavior and regression tests—do not normalize all H3 close errors into success.

Acceptance:
- [ ] graceful close/drain does not create spurious hidden replay;
- [ ] no new streams are assigned to a known draining generation;
- [ ] in-flight streams survive cache replacement where protocol permits;
- [ ] tests cover concurrent requests during drain/reconnect.

## 8. Extend transport observability

Extend `TransportMetrics` rather than overloading `PoolMetrics`.

Candidate exact counters where observable:

- Alt-Svc learned/expired/cleared/rejected;
- H3 route attempted;
- H3 route suppressed;
- safe fallback selected;
- H3 graceful drain/connection close;
- H3 reconnect generation.

For richer per-connection information, use actual Quinn data only where a response/connection can truthfully expose it: RTT, peer address, close reason, and selected statistics. Do not synthesize socket-reuse or congestion values when unavailable.

Acceptance:
- [ ] deterministic fixtures assert exact event counts;
- [ ] metrics are transport-level and do not change HTTPX facade semantics accidentally.

## 9. Security and adversarial tests

Add focused cases for:

- oversized/malformed Alt-Svc headers;
- alternative authority injection;
- redirect/origin confusion;
- TLS certificate mismatch on alternative host;
- stale/expired alternatives;
- UDP black-hole timeout;
- body replay boundaries;
- cancellation while H3 and fallback selection are in progress;
- cache churn and failure-suppression churn.

## Validation

At minimum:

```sh
cargo test -p eggfetch-core --all-features
./scripts/check.sh
```

Add focused feature-gated tests to the existing validation conventions. Do not add a new CI workflow/matrix.

## Non-goals

- 0-RTT;
- WebTransport;
- H3 datagrams;
- MASQUE/CONNECT-UDP;
- connection migration;
- speculative Happy Eyeballs across TCP and QUIC unless later evidence demonstrates it is required;
- removing the experimental label in this implementation plan.

## Exit criteria

- [ ] Alt-Svc discovery is bounded, authenticated, and origin-safe;
- [ ] Auto has explicit discovery/fallback behavior;
- [ ] broken H3 alternatives are suppressed without becoming retry machinery;
- [ ] body replay and deadline invariants survive fallback;
- [ ] H3 draining/reconnection behavior is explicit and tested;
- [ ] Tier 1 is green;
- [ ] HTTP/3 remains experimental pending the separate interoperability/graduation plan.