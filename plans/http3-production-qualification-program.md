# HTTP/3 Production Qualification Program

Planning baseline: `be7bf441661a923637b92ba8e449342868c56aa5` (`main`, 2026-09-11)
Prior evidence: `plans/http3-interoperability-and-production-graduation.md`
Current status: HTTP/3 **experimental**.
Execution status: completed on the frozen executable tree with the
experimental label retained; missing external evidence is recorded as
blockers, not waived.

## Objective

Close the specific blockers left by the 2026-09-11 HTTP/3 graduation decision and make a new evidence-based decision on whether ordinary EggFetch HTTP/3 request/response operation can be described as supported.

This is primarily a qualification and hardening program, not a feature-expansion program. Existing architecture is the baseline: Quinn/h3 transport, bounded H3 connection state, authenticated Alt-Svc discovery, broken-route suppression, safe pre-commit fallback, strict `Http3Only`, timeout integration, trailers, draining, and H3 transport metrics. Do not replace those mechanisms unless qualification exposes a concrete defect.

The prior program closed successfully with the experimental label retained because independent-server interop, public-origin evidence, realistic impairment coverage, and upstream-stack risk closure were incomplete. This program must close those blockers rather than repeating Quinn/h3 self-interop.

## Final-plan outcome (2026-09-11)

The final graduation/requalification plan evaluated the literal gate on frozen
executable SHA `639bf186a71c054e11278d1b160ffe7a6f172c02`. Deterministic H3
controls and repository/compatibility validation are green, and both HTTPX
profiles were renewed on that SHA. HTTP/3 remains experimental because the
independent-server, independent-drain, public-origin, full impairment, and
upstream-risk evidence classes are not closed. The complete record is in
`http3-production-qualification-evidence.md`.

## Program order

Execute in this order:

1. `http3-independent-interop-and-impairment-qualification.md`
2. `http3-upstream-risk-resource-and-observability-hardening.md`
3. `http3-production-graduation-and-compatibility-requalification.md`

Plans 1 and 2 may be developed in parallel where they do not touch the same executable paths, but both must be audited before the final graduation/freeze plan. Unresolved acceptance items remain blockers rather than being waived by the freeze.

## Invariants

- `eggfetch-core` remains the sole networking implementation.
- Do not introduce a second QUIC/H3 client stack for production traffic merely to obtain interop evidence.
- `Http3Only` remains strict: never silently fall back to H2/H1.
- `Auto { allow_http3: true }` may fall back only under the existing safe pre-commit/replayability rules.
- One-shot/non-replayable request bodies are never implicitly duplicated.
- Alt-Svc alternatives may change transport routing only; origin identity, TLS SNI, Host, cookies, auth and redirect policy remain origin-bound.
- External/public interop is qualification evidence, not a Tier-1 network dependency.
- No result may count an unavailable external implementation as a pass.
- 0-RTT, WebTransport, H3 datagrams, MASQUE/CONNECT-UDP and connection migration remain out of scope.

## Qualification-sensitive boundary

At program start, the prior HTTPX 0.28.1 and HTTPX2 2.12.0 executable binding was
`65beb675a5380d3ff4291da6833b91ebf12c769a`. Any Rust/Python source, tests,
manifests, lockfile, validation scripts, package configuration, dependency
changes, or qualification tooling introduced by this program invalidates that
binding for the new tree. The profiles were subsequently renewed on the final
frozen executable SHA `639bf186a71c054e11278d1b160ffe7a6f172c02`; that renewal
is separate from the HTTP/3 graduation decision.

Therefore:

- land all executable H3 qualification/hardening work before the final freeze;
- do not update compatibility profiles to an intermediate SHA;
- after executable work closes, freeze one exact executable SHA;
- rerun the required HTTPX 0.28.1 and HTTPX2 2.12.0 qualification gates on that exact SHA;
- record H3 graduation separately from HTTPX compatibility status;
- only documentation/profile/ledger-only descendants may follow without another executable requalification.

## Required evidence classes

The final decision requires all of the following evidence classes, not substitutes for one another:

### Independent protocol interoperability

At least two maintained HTTP/3 server implementations that do not use Quinn/h3 must pass the required EggFetch corpus. Preferred candidates are ngtcp2/nghttp3, quiche, quic-go, or an MsQuic-based server. Quinn/h3 remains a control, not an independent implementation.

### Realistic network impairment

Exercise loss, latency/jitter, reordering, duplication where supported, UDP black-hole/block, MTU/PMTU failure where practical, active-path loss, and address-family failure. Assertions are semantic and resource-bounded, not congestion-control benchmarks.

### Upstream dependency risk closure

Audit the exact pinned Quinn/h3/h3-quinn/rustls dependency set against current correctness/security issues. Any issue capable of corrupting ordinary successful GET/POST/streaming/cancellation behavior must be fixed by an upstream upgrade, covered by a narrow safe workaround plus regression test, proven irrelevant to EggFetch with evidence, or remain a graduation blocker.

### Lifecycle/resource stability

Repeated reuse, multiplexing, origin churn, failure/suppression/recovery, drain/reconnect, cancellation, partial-body drop, and client construction/destruction must demonstrate bounded caches, tasks, descriptors/sockets and memory trend under the repository's existing resource conventions.

### Diagnostic sufficiency

Failures must be diagnosable using real Quinn/h3 data where available. Never synthesize per-response QUIC facts that cannot be reliably associated with the response.

### Platform scope

The supported H3 platform claim must be no broader than demonstrated build/runtime evidence. Missing platform evidence is a scope limitation, not a pass.

## Graduation gate

Ordinary HTTP/3 may lose the experimental label only if every condition below is true on the final executable candidate:

- deterministic H3 hardening, Alt-Svc, fallback and lifecycle suites are green;
- the required corpus passes against at least two independent non-Quinn server implementations;
- public-origin spot checks are recorded against multiple current H3 deployments;
- impairment tests prove bounded deadlines, cancellation, cleanup, suppression/recovery and replay safety;
- resource/soak evidence is bounded;
- no unresolved upstream defect is known to corrupt ordinary successful H3 operation under the declared supported scope;
- GOAWAY/draining/reconnect behavior is demonstrated against at least one independent server where controllable;
- IPv4/IPv6 and multi-address behavior are exercised to the extent claimed as supported;
- diagnostics expose enough connection/close/path state to investigate failures without retaining connection objects indefinitely;
- Tier 1, extended verification, package validation, compatibility gates and remote CI are green on the frozen executable tree.

If any required condition is not met, retain `experimental` and record the exact blockers. That is a valid program outcome. Do not weaken the gate to force promotion.

## Stable scope if graduation succeeds

The supported claim is limited to ordinary HTTP/3 client operation:

- authenticated Alt-Svc discovery;
- explicit `Http3Only`;
- GET/HEAD and ordinary request bodies;
- buffered and streaming request/response bodies;
- trailers;
- multiplexing/reuse;
- safe Auto fallback/suppression/recovery;
- graceful drain/reconnect;
- documented supported platforms.

Graduation does not imply 0-RTT, WebTransport, datagrams, MASQUE, migration, or H3-by-default. Any default-policy change requires a separate explicit decision.

## Exit criteria

- [x] both implementation/qualification child plans were audited, with unsatisfied acceptance items and blockers explicitly recorded;
- [x] one exact executable SHA was frozen after all qualification-sensitive changes;
- [x] a reproducible evidence ledger identifies executed controls, unavailable independent servers, impairment scenarios, upstream dependency review, resource results and empty public-spot-check records;
- [x] HTTPX 0.28.1 and HTTPX2 2.12.0 were requalified on the frozen executable SHA after this program's qualification-sensitive changes;
- [x] HTTP/3 was retained as experimental with named unresolved blockers;
- [x] documentation and plan index match the actual decision.
