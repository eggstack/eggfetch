# HTTP/3 Upstream Risk, Resource, and Observability Hardening

Planning baseline: `be7bf441661a923637b92ba8e449342868c56aa5`
Parent: `plans/http3-production-qualification-program.md`

## Objective

Close the non-interop blockers to a supportable HTTP/3 designation: upstream h3/Quinn correctness risk, long-running lifecycle/resource evidence, and sufficient QUIC diagnostics. Make only narrowly justified production changes discovered by qualification.

## Closure audit (2026-09-11)

The frozen tree records the resolved dependency graph, deterministic lifecycle
and diagnostic coverage, cache-bound tests, and EggFetch-owned Alt-Svc fuzz
target. No dependency upgrade or upstream workaround was justified before
freeze. The open h3 buffered-data-on-close issue remains a promotion blocker;
long external soak and broader platform evidence were not available. See
`http3-production-qualification-evidence.md` for the evidence and explicit
dispositions.

## 1. Freeze and audit the dependency graph

Record the exact resolved versions and relevant feature flags for:

- `quinn`, `quinn-proto`, `quinn-udp`;
- `h3`;
- `h3-quinn`;
- rustls and QUIC crypto integration;
- runtime/network dependencies that materially affect H3 behavior.

The current code intentionally uses h3 0.0.8's `i-implement-a-third-party-backend-and-opt-into-breaking-changes` feature to inspect connection closing state. Treat this as a version-sensitive integration contract.

For each current upstream correctness/security issue affecting ordinary client semantics, create an audit row with:

- upstream issue/reference;
- affected versions;
- EggFetch reachability;
- ordinary-operation impact;
- reproduction/regression test;
- disposition: upgrade / local safe workaround / proven not reachable / graduation blocker.

At minimum re-evaluate the current buffered-data-on-connection-close class and request cancellation/STOP_SENDING/reset behavior identified during planning, plus any newer issues in h3, h3-quinn or Quinn at execution time.

Acceptance:
- [ ] exact resolved dependency/feature set is recorded;
- [ ] every known ordinary-client correctness issue has an explicit disposition;
- [ ] no issue capable of silently corrupting a successful response is waived without evidence;
- [ ] the unstable h3 feature dependency has a focused regression test and documented re-audit trigger;
- [ ] no private long-lived fork is introduced solely to claim graduation.

## 2. Dependency upgrade decision

Upgrade Quinn/h3/h3-quinn only if current upstream versions materially improve correctness, security, interoperability or remove the unstable API dependency at acceptable migration cost.

If upgrading:

- update the exact risk ledger;
- rerun all H3 hardening/Alt-Svc/interop tests;
- verify GOAWAY/draining behavior against the new API rather than mechanically porting old assumptions;
- verify timeout, stream cancellation, trailers and error classification;
- run the independent interop plan after the upgrade;
- treat lockfile/manifest changes as qualification-sensitive.

If not upgrading, document why the pinned versions are acceptable under the declared scope and which upstream defects still block graduation.

Acceptance:
- [ ] upgrade/no-upgrade is an explicit evidence-based decision;
- [ ] dependency changes do not broaden feature scope;
- [ ] all version-sensitive behavior is covered by tests.

## 3. Targeted upstream-defect regression cases

Add focused tests for any upstream defect that EggFetch depends on being fixed or mitigated. Prefer minimal protocol fixtures over broad integration tests so failures identify the upstream contract that changed.

Important classes:

- response bytes buffered before/with connection close are not silently lost;
- cancel/drop sends or triggers appropriate stream cancellation behavior within the capabilities of h3-quinn;
- reset can be observed without requiring unrelated body progress where the API permits;
- GOAWAY/closing state cannot admit new work to a draining generation;
- driver termination wakes/cleans dependent request state;
- QPACK/frame errors surface as errors rather than partial successful responses.

Do not duplicate upstream parser fuzzing unless EggFetch owns an additional parser/state layer.

Acceptance:
- [ ] every locally mitigated upstream issue has a regression test;
- [ ] tests distinguish complete response EOF from truncated/error termination;
- [ ] no workaround silently converts protocol failure into success.

## 4. Resource and soak qualification

Extend the existing resource/soak conventions with H3-specific scenarios. Keep routine tests bounded; longer runs may be qualification/manual tier.

Required scenarios:

- thousands of sequential requests over reused H3 connections;
- high concurrent multiplexing against one origin;
- origin churn several times beyond the H3 and Alt-Svc cache bounds;
- repeated connect failure -> suppression -> expiry/new advertisement -> recovery;
- repeated GOAWAY/drain/reconnect cycles;
- cancellation storms before headers, during upload and during download;
- response bodies dropped partially consumed and unconsumed;
- long-lived streaming responses followed by cleanup;
- repeated Client construct/drop loops;
- server disappearance/reappearance cycles.

Capture, where portable/available:

- RSS trend using existing repository policy rather than fragile exact allocator values;
- process descriptor/socket count;
- task/driver lifecycle counters or test hooks;
- H3 cache cardinality;
- Alt-Svc/suppressor cardinality;
- pool permits/in-flight logical request state;
- connection creation/drain/reconnect counters.

Acceptance:
- [ ] bounded caches remain within configured limits;
- [ ] descriptor/socket count returns to a bounded steady state;
- [ ] driver/task state does not grow monotonically;
- [ ] client drop eventually releases owned QUIC resources after outstanding references finish;
- [ ] no monotonic memory trend attributable to retained per-origin/session state is observed;
- [ ] failures/cancellation do not leak logical pool permits.

## 5. Cache eviction quality review

The current H3 connection cache is bounded, which is a correctness requirement. Review whether its eviction selection is effectively arbitrary under `DashMap` iteration and whether multi-origin qualification demonstrates avoidable hot-origin churn.

Only change policy if evidence shows meaningful churn or handshake amplification. If changed, prefer a simple bounded idle/LRU-like policy that does not add a large dependency or hold connection objects longer than necessary.

Acceptance:
- [ ] current eviction behavior is characterized under cache pressure;
- [ ] any policy change has deterministic tests for bound, active-reference safety and expected victim selection;
- [ ] no eviction change is made solely for aesthetic reasons.

## 6. QUIC/H3 diagnostic surface

Use Quinn/h3 data already available to improve diagnosis where it can be associated reliably with a connection/session.

Evaluate exposing at client/transport diagnostic level:

- selected remote socket address;
- current/smoothed RTT;
- connection close reason and transport/application code;
- Quinn connection/path statistics, including loss where stable;
- bytes/packet aggregates where stable;
- active/open stream counts where stable;
- handshake/connect duration already available through metrics;
- whether the route was explicit `Http3Only` or Alt-Svc-discovered Auto;
- Alt-Svc generation and suppression/recovery counters without leaking host secrets.

Do not attach connection-level values to an individual response if reuse/multiplexing makes that association unreliable. Prefer snapshots or client-level diagnostics.

Consider optional qlog support for qualification/debug builds if Quinn exposes a contained integration. It must be opt-in, must not log credentials/body data by default, and must not become a normal dependency if it materially increases footprint.

Acceptance:
- [ ] close/fallback failures can be correlated to real transport state;
- [ ] no fabricated zero/default metadata is presented as observed fact;
- [ ] diagnostic state is bounded and does not retain live connections;
- [ ] sensitive headers, cookies, auth and request bodies are not emitted;
- [ ] qlog, if added, is opt-in and documented as diagnostic output.

## 7. Fuzz/property hardening of EggFetch-owned state

Retain and extend fuzz/property coverage for EggFetch-owned H3 policy boundaries:

- Alt-Svc parser/cache;
- suppressor generation transitions;
- fallback eligibility/replay state;
- drain/reconnect state;
- timeout/deadline transitions.

Add corpus seeds for malformed/oversized Alt-Svc, repeated `clear`, generation replacement, suppression expiry and state churn discovered by the qualification work.

Acceptance:
- [ ] EggFetch-owned untrusted H3 policy state has deterministic fuzz/property coverage;
- [ ] no duplicate h3 QPACK/frame parser implementation is introduced.

## 8. Platform evidence

Re-run H3 build and deterministic runtime coverage on the project's supported packaging platforms to the extent the existing infrastructure allows. Record actual evidence separately for Linux, macOS, Windows and relevant architectures.

Do not create a new permanent CI matrix without explicit policy approval. Manual/extended platform evidence is acceptable, but unsupported/unexecuted combinations must remain explicit.

Acceptance:
- [ ] platform table records build/runtime/IPv6 evidence separately;
- [ ] support documentation does not infer H3 runtime support from a generic package build alone.

## Validation

After any executable change run focused H3 suites first, then `./scripts/check.sh`. Before final closure also run extended and package validation required by the parent/final plan.

## Non-goals

- new H3 application features;
- 0-RTT;
- WebTransport/datagrams/MASQUE;
- replacing Quinn;
- duplicating h3 parser internals;
- promising upstream h3 stability beyond EggFetch's qualified scope.

## Exit criteria

- [ ] upstream dependency risk ledger is current and complete for ordinary H3 operation;
- [ ] all graduation-relevant upstream issues are fixed, mitigated/proven irrelevant, or explicitly block promotion;
- [ ] H3 soak/resource evidence is bounded;
- [ ] cache pressure behavior is characterized and corrected only if justified;
- [ ] diagnostic coverage is sufficient for qualification failures;
- [ ] platform scope is evidence-based;
- [ ] focused and routine validation are green.
