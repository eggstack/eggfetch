# HTTP/3 Independent Interoperability and Impairment Qualification

Planning baseline: `be7bf441661a923637b92ba8e449342868c56aa5`
Parent: `plans/http3-production-qualification-program.md`

## Objective

Replace the remaining self-interop-heavy HTTP/3 evidence with reproducible independent-stack and adverse-network qualification. The existing `tests/h3_interop_qualification.rs` Quinn/h3 fixtures remain mandatory controls; this plan adds evidence that EggFetch behaves correctly when the peer and network are not its own stack.

## Implementation status

The qualification contract is versioned in `qualification/http3/`:
`corpus.json` defines the implementation-neutral cases and
`impairment-matrix.json` defines the adverse-network scenarios. The opt-in
dependency-free coordinators are `scripts/h3_qualification.py` and
`scripts/h3_impairment.py`; they emit explicit pass/fail/unsupported JSON
and never run from Tier 1. The current evidence records are
`plans/http3-independent-interop-and-impairment-qualification-evidence.json`
and `plans/http3-public-origin-spot-check-ledger.json`.

At the current tree the 20 deterministic Quinn/h3 controls pass. Independent
server, public-origin, and full impairment evidence remain open blockers; the
HTTP/3 experimental label is therefore retained.

## Closure status

This child plan is **not fully satisfied**: the deterministic corpus and
fail-closed coordinators are complete, but no independent server adapters,
public-origin checks, independent drain/reset server, or namespace/netem
runner were available during the qualification window. Those unchecked
acceptance items remain named blockers for any future promotion attempt.

## Closure audit (2026-09-11)

The deterministic Quinn/h3 control corpus passed on frozen executable SHA
`639bf186a71c054e11278d1b160ffe7a6f172c02`. No independent server adapters,
public-origin checks, independent drain/reset server, or namespace/netem
runner were available, so those acceptance items remain explicit blockers;
they are not waived by the local pass. See
`http3-production-qualification-evidence.md` and the JSON evidence ledger.

## Open items (2026-10-22)

The acceptance criteria listed below are **not met** and remain open. They are
recorded here so this file's closure record and its unticked boxes tell the
same story. The original boxes stay unticked and point at the open-item IDs
below. Nothing here was waived, and this file's closure status and closure
audit are unchanged.

Reason (from this file's own closure status below): the deterministic local
Quinn/h3 corpus passed on frozen executable SHA
`639bf186a71c054e11278d1b160ffe7a6f172c02`, but no independent server
adapters, public-origin checks, independent drain/reset server, or
namespace/netem runner were available during the qualification window, so the
items are explicitly named blockers for any future promotion attempt.

- **OI-1** — the deterministic-corpus group: an implementation-independent
  required-semantics corpus, a mandatory green local Quinn/h3, unsupported
  capabilities reported separately from pass/fail, and no conversion of an
  external-server absence into a passing test.
- **OI-2** — the two-implementation group: at least two reproducible
  non-Quinn implementations, captured implementation/version identity, a
  corpus pass on at least two independent implementations, and failures that
  retain diagnosable logs/qlog/close information.
- **OI-3** — the public-origin group: at least three recorded successful
  current public-origin H3 checks, explicit classification of public failures
  that cannot alter deterministic results, and no volatile public endpoint as
  a Tier-1 dependency.
- **OI-4** — the impairment group: an explicit pass/fail/unsupported
  impairment matrix, bounded and safely falling-back UDP-blackhole Auto
  behavior, no duplication of one-shot/non-idempotent/non-replayable
  requests, demonstrated recovery after suppression, and no resource leak
  after bounded cleanup.
- **OI-5** — the independent drain/reset group: an independent
  drain/reconnect demonstration, reset/cancel behavior tested independently of
  normal EOF, and no reliance on string-matching an implementation-specific
  error where typed state is available.
- **OI-6** — the graduation-gate group: two independent non-Quinn H3 servers
  passing the required corpus, one independent GOAWAY/drain/reconnect source,
  a public-origin ledger with three successful current H3 deployments, a
  bounded and replay-safe impairment matrix, explicit IPv4/IPv6 limitations,
  reproducible machine/audit-readable evidence, and a focused regression test
  for any product defect found before close.

## 1. Define the qualification corpus once

Create one implementation-neutral client corpus that can target local Quinn/h3 fixtures and externally supplied independent H3 servers without duplicating assertions per server.

Required cases where the server implementation supports the necessary control:

- authenticated TLS + ALPN `h3` negotiation;
- GET and HEAD;
- buffered POST;
- streaming upload;
- large streaming download (at least 1 MiB, chunked consumption);
- response trailers;
- concurrent multiplexed requests over reusable H3 state;
- cancellation before response headers;
- cancellation during upload;
- cancellation/response drop during download;
- stream reset / abrupt response termination;
- graceful connection close;
- GOAWAY/drain followed by a request that reconnects correctly;
- repeated connection reuse;
- IPv4 and IPv6 where the server supports both;
- alternative port advertised through authenticated Alt-Svc;
- server restart/recovery.

Keep capability-specific cases explicit. A server that cannot induce GOAWAY, for example, may report that case unsupported, but that unsupported case cannot satisfy the graduation requirement for GOAWAY evidence.

Acceptance:
- [ ] one corpus defines required semantics independent of server implementation; — Deferred (OI-1).
- [ ] local Quinn/h3 remains mandatory and green; — Deferred (OI-1).
- [ ] unsupported capabilities are reported separately from pass/fail; — Deferred (OI-1).
- [ ] no external-server absence is converted into a passing test. — Deferred (OI-1).

## 2. Reproducible independent-server adapters

Provide a documented adapter/harness for at least three candidate independent implementations so qualification is not blocked by one project's packaging. At final graduation, at least two must actually pass.

Preferred implementations:

1. ngtcp2/nghttp3;
2. quiche;
3. quic-go;
4. optional MsQuic-based server as additional diversity.

Use pinned container image digests, source revisions, or locally built version identifiers in the evidence ledger. Do not download/build them during Tier 1. Extended/manual qualification may use containers or the QUIC interop runner.

The harness should accept server endpoints/configuration through environment or a qualification manifest and emit machine-readable results containing implementation, version, cases attempted, pass/fail/unsupported and timestamps.

Acceptance:
- [ ] at least two non-Quinn implementations can be launched/referenced reproducibly; — Deferred (OI-2).
- [ ] exact implementation/version identity is captured; — Deferred (OI-2).
- [ ] required corpus passes on at least two independent implementations before graduation; — Deferred (OI-2).
- [ ] failures retain enough logs/qlog/close information for diagnosis. — Deferred (OI-2).

## 3. Public-origin spot-check ledger

Add a manual qualification procedure for several current public origins that advertise HTTP/3 at execution time. Do not hard-code volatile hosts into routine tests.

For each origin record:

- date/time;
- hostname only (never credentials/query secrets);
- authenticated H1/H2 discovery response and observed Alt-Svc value/category;
- whether subsequent eligible traffic used H3;
- response status/body integrity check appropriate to the endpoint;
- remote address family;
- fallback result with H3 made unavailable where the environment permits;
- any transient/network-specific failure.

Use at least three unrelated operators/CDNs when practical so evidence is not one deployment family.

Acceptance:
- [ ] at least three successful current public-origin H3 checks are recorded for graduation; — Deferred (OI-3).
- [ ] public failures do not alter deterministic test results and are classified explicitly; — Deferred (OI-3).
- [ ] no volatile public endpoint becomes a Tier-1 dependency. — Deferred (OI-3).

## 4. Network impairment harness

Use a controllable Linux environment (network namespaces/netem, QUIC interop runner simulator, or equivalent) for the qualification-only impairment suite. Keep platform-specific setup outside the core production transport.

Required scenarios where host capabilities permit:

- UDP completely black-holed;
- UDP explicitly rejected/blocked;
- fixed packet loss at multiple nonzero rates;
- latency and jitter;
- packet reordering;
- packet duplication;
- reduced MTU / PMTU black-hole behavior where practical;
- path loss after response headers;
- path loss during upload;
- server disappearance/restart;
- first resolved address unreachable and later address reachable;
- IPv6 failure with IPv4 success and the reverse where the host supports both.

Do not assert a particular congestion algorithm or throughput. Assert protocol/client invariants.

For each scenario verify as applicable:

- configured connect/read/write/total deadline remains bounded;
- cancellation terminates work promptly;
- pool permit/body/driver resources are released;
- no non-replayable body is duplicated;
- safe Auto fallback occurs only pre-commit and only for replayable requests;
- `Http3Only` never falls back;
- broken-route suppression activates only for intended failure classes;
- suppression expires or is invalidated by a new Alt-Svc generation correctly;
- recovery establishes fresh H3 when the path returns;
- cache state does not remain permanently poisoned.

Acceptance:
- [ ] required impairment matrix has explicit pass/fail/unsupported results; — Deferred (OI-4).
- [ ] UDP-blackhole Auto behavior is bounded and falls back safely; — Deferred (OI-4).
- [ ] one-shot/non-idempotent non-replayable requests are never duplicated; — Deferred (OI-4).
- [ ] recovery after suppression is demonstrated; — Deferred (OI-4).
- [ ] no resource leak is observed after bounded cleanup. — Deferred (OI-4).

## 5. Independent drain/reset qualification

The prior implementation uses h3 0.0.8 connection state for GOAWAY/draining. Prove those semantics against at least one independent server rather than only the local h3 server.

Required evidence:

- a request already committed to a healthy stream completes or reports the actual stream/connection error;
- GOAWAY marks the generation unusable for new work;
- no hidden replay occurs merely because the connection begins draining;
- the next eligible request obtains fresh H3 state;
- application/transport close information is preserved where upstream exposes it.

Acceptance:
- [ ] at least one independent implementation demonstrates drain/reconnect semantics; — Deferred (OI-5).
- [ ] reset/cancel behavior is tested independently from normal EOF; — Deferred (OI-5).
- [ ] no reliance on string-matching an implementation-specific error is introduced when typed state is available. — Deferred (OI-5).

## 6. Evidence artifact

Create a qualification ledger under an appropriate existing docs/plans evidence location. It must distinguish:

- deterministic local evidence;
- independent local/container evidence;
- impairment evidence;
- public Internet spot checks;
- unsupported/skipped cases.

Do not call the program graduated until the ledger itself demonstrates the parent gate.

## Validation

Routine Tier 1 remains network-independent. Add only deterministic local tests to `./scripts/check.sh` if appropriate under existing policy. Independent implementations and netem belong to extended/manual qualification unless they can be made hermetic without expanding CI policy.

Run at minimum after executable changes:

- existing H3 hardening suite;
- existing Alt-Svc discovery suite;
- existing H3 interop qualification suite;
- new implementation-neutral corpus locally;
- independent-server corpus;
- impairment matrix;
- `./scripts/check.sh`.

## Non-goals

- adding another production H3 backend;
- benchmarking Quinn against other QUIC stacks;
- requiring public Internet in CI;
- 0-RTT, WebTransport, datagrams, MASQUE or migration;
- changing H3 default enablement.

## Exit criteria

- [ ] at least two independent non-Quinn H3 servers pass the required corpus; — Deferred (OI-6).
- [ ] at least one independent server supplies GOAWAY/drain/reconnect evidence; — Deferred (OI-6).
- [ ] public-origin ledger contains at least three successful current H3 deployments; — Deferred (OI-6).
- [ ] impairment matrix demonstrates bounded and replay-safe behavior; — Deferred (OI-6).
- [ ] IPv4/IPv6 limitations are explicit; — Deferred (OI-6).
- [ ] evidence is reproducible and machine/audit readable; — Deferred (OI-6).
- [ ] any product defect found is fixed before this plan closes, with a focused regression test. — Deferred (OI-6).
