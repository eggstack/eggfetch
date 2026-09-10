# HTTP/3 Interoperability and Production Graduation

Planning baseline: `4456680361dbfdc2d15b0d43a18cdcb20704f667` (`main`, 2026-09-10)
Parent program: `plans/http3-and-next-httpx-compatibility-program.md`
Depends on: `plans/http3-alt-svc-discovery-fallback-and-draining.md`

## Objective

Decide whether EggFetch's ordinary HTTP/3 client path can graduate from experimental to supported based on reproducible interoperability, failure-mode, resource, and platform evidence. This is a qualification plan, not a mandate to remove the label.

Upstream Hyper still describes HTTP/3 integration as unfinished and lists h3 hardening/interoperability as remaining work. EggFetch therefore needs its own evidence boundary around the currently pinned Quinn/h3 stack.

## 1. Dependency and upstream-risk review

Before qualification, inventory the exact versions of:

- `quinn` / `quinn-proto` / rustls integration;
- `h3`;
- `h3-quinn`;
- transitive QUIC/TLS dependencies.

Compare the pinned versions with current upstream stable releases and open correctness issues. In particular, inspect issues that can alter client-visible semantics around clean close, buffered data, stream reset/cancel, GOAWAY, QPACK/frame parsing, and connection-driver lifecycle.

Upgrade dependencies only when there is a concrete correctness/security/interoperability benefit and the migration remains contained. Any upgrade is qualification-sensitive and must land before the final program freeze.

Acceptance:
- [ ] exact dependency versions and relevant upstream blockers are recorded;
- [ ] known upstream defects are either fixed by upgrade, covered by a narrow workaround/test, or named as graduation blockers;
- [ ] no unsupported fork of h3/Quinn is introduced merely to force graduation.

## 2. Build a deterministic local interoperability harness

Add an extended/manual test harness capable of running EggFetch against at least two independent HTTP/3 implementations in addition to EggFetch's existing h3/Quinn fixtures when those implementations are available locally.

Preferred external implementations include ngtcp2/nghttp3, quiche, or another maintained independent stack. The harness must not make Tier 1 depend on network downloads or public Internet availability.

Exercise:

- GET/HEAD and request bodies;
- large streaming upload/download;
- response trailers;
- concurrent multiplexed requests;
- cancellation/reset;
- graceful close/drain;
- malformed/early-close behavior where controllable;
- IPv4 and IPv6 when available;
- Alt-Svc alternative port/authority cases.

Use capability detection with explicit skip reporting in extended/manual validation rather than silently passing when external servers are absent.

Acceptance:
- [ ] local deterministic h3 fixtures remain mandatory;
- [ ] independent-server tests can run reproducibly without changing EggFetch source;
- [ ] optional absence is visible and cannot be mistaken for graduation evidence.

## 3. Real-network spot checks

Create a documented manual qualification procedure for a small set of major public HTTPS origins known to advertise H3 at qualification time. These checks are evidence supplements, not routine CI.

Verify:

- first request discovers Alt-Svc over authenticated HTTPS;
- later request selects H3 when eligible;
- protocol version and response content are correct;
- fallback remains usable when UDP is blocked;
- no credentials are exposed in diagnostics;
- DNS/IPv4/IPv6 variations do not produce permanent poisoned state.

Do not bake volatile public hostnames into Tier 1 tests.

Acceptance:
- [ ] qualification ledger records date, origins, negotiated protocol, and result;
- [ ] transient Internet failure is distinguished from deterministic test failure.

## 4. Network impairment and fallback qualification

Use local controllable impairment where practical to test:

- UDP black-hole/no response;
- packet loss and reordering;
- high latency/jitter;
- path becoming unreachable during an active response;
- reset/close during upload;
- server restart/drain;
- DNS returning one bad and one good address.

The objective is not to benchmark congestion control. It is to prove bounded deadlines, cancellation, resource cleanup, safe fallback, and lack of duplicate non-replayable requests.

Acceptance:
- [ ] every impairment terminates within the configured timeout/cancellation contract;
- [ ] no leaked pool permit, H3 cache generation, driver task, or response body remains after bounded cleanup;
- [ ] non-replayable requests are never duplicated;
- [ ] suppression/recovery behavior is deterministic.

## 5. Resource and soak evidence

Add/extend tests for:

- many sequential origins beyond the H3/Alt-Svc cache bound;
- repeated connect/fail/reconnect cycles;
- hundreds/thousands of requests on reused H3 connections;
- cancellation storms;
- partial body drops;
- long-lived streaming responses;
- client construction/drop loops.

Measure process/task/descriptor behavior using existing repository soak/resource conventions. Avoid fragile exact RSS assertions where allocator behavior is nondeterministic; use bounded trend or existing threshold policy.

Acceptance:
- [ ] cache/task/file-descriptor growth stabilizes under repeated cycles;
- [ ] dropping Client eventually closes owned QUIC resources after in-flight references are gone;
- [ ] no monotonically growing per-origin state remains.

## 6. QUIC/H3 observability qualification

Validate any metrics introduced by the discovery plan and expose only data that Quinn/h3 actually provides.

Useful candidates include:

- RTT;
- remote address;
- close reason/application code;
- path packet/loss statistics;
- H3 route/fallback/suppression counters.

If per-response metadata cannot be associated reliably with a specific reused H3 connection, keep it at client/transport diagnostics rather than attaching guessed values to responses.

Acceptance:
- [ ] no fabricated metadata;
- [ ] diagnostic data is bounded and redacted;
- [ ] metrics do not retain connection objects indefinitely.

## 7. Fuzz/adversarial coverage

Extend existing fuzz/property infrastructure where it adds value for EggFetch-owned parsing/state:

- Alt-Svc parser/cache operations;
- fallback/draining state machine;
- timeout/replay transitions.

Do not duplicate h3's own QPACK/frame parser fuzzing unless EggFetch adds its own parser layer.

Acceptance:
- [ ] EggFetch-owned untrusted parser/state boundaries have deterministic fuzz/property targets or equivalent adversarial tests.

## 8. Platform qualification

Prove the `http3` feature builds and its deterministic tests pass on the platforms already supported by EggFetch packaging where the underlying UDP/QUIC stack supports them. Reuse existing CI/package mechanisms; do not create a new matrix solely for H3.

At minimum ensure no platform-specific assumptions about:

- UDP socket binding;
- IPv6 availability;
- certificate stores;
- timer precision;
- local/remote address metadata.

Acceptance:
- [ ] supported-platform limitations are explicit;
- [ ] lack of a platform-specific H3 test is not described as evidence of support.

## 9. Graduation decision gate

HTTP/3 may lose the experimental label only when all required conditions below are true:

- deterministic H3 functional/hardening suites pass;
- Alt-Svc discovery/fallback/draining acceptance criteria pass;
- at least two independent non-Quinn server implementations pass the required interoperability corpus;
- realistic public-origin spot checks pass;
- UDP-blocked and impairment fallback tests pass;
- resource/soak checks pass;
- no known upstream issue remains that predictably corrupts ordinary successful GET/POST/streaming behavior without an adequate pinned workaround;
- supported-platform scope is documented;
- Tier 1 and extended validation are green.

If any condition is not met, close this plan with HTTP/3 still experimental and record concrete blockers. That is a valid outcome.

## 10. Stable-surface decision if graduation succeeds

Only after the gate passes:

- remove experimental wording for ordinary HTTP/3 request/response operation;
- document exact stable scope: H3 request/response, streaming, trailers, discovery/fallback, supported platforms;
- keep advanced QUIC features separately experimental/unimplemented;
- decide deliberately whether any default Auto behavior changes. A stable H3 label does not itself require enabling H3 discovery by default.

## Validation

Use existing repository tiers. The interoperability harness belongs in Tier 2/manual qualification if external tools are optional. Normal CI remains `./scripts/check.sh`.

## Non-goals

- 0-RTT as a graduation requirement;
- WebTransport;
- datagrams/MASQUE;
- connection migration;
- benchmarking H3 against every client/server;
- adding public-network tests to routine CI;
- claiming upstream h3 maturity that does not exist.

## Exit criteria

- [ ] an auditable H3 interoperability/resource/failure evidence record exists;
- [ ] the graduation decision follows the objective gate;
- [ ] any dependency changes are frozen before final compatibility requalification;
- [ ] HTTP/3 documentation is either truthfully promoted to supported or truthfully retained as experimental with named blockers.