# HTTP/3 Post-Freeze Diagnostics Requalification Corrective Closure

Planning baseline: `6a0cfd87551b7c634593e7b39cd2fd35d128f727` (`main`, 2026-09-11)
Parent context: `plans/http3-production-qualification-program.md`
Prior frozen executable SHA: `639bf186a71c054e11278d1b160ffe7a6f172c02`
Triggering post-freeze executable commit: `6a0cfd87551b7c634593e7b39cd2fd35d128f727` (`feat: add bounded HTTP3 transport diagnostics`)

## Objective

Restore exact-SHA qualification truth after HTTP/3 transport diagnostics landed after the prior production-qualification freeze.

This is a corrective closure and requalification pass, not a new HTTP/3 feature program and not a new graduation attempt. The H3 production decision remains **experimental retained** unless a separate future program supplies the missing independent interoperability, impairment, public-origin, drain/reconnect, and upstream-risk evidence.

The corrective pass must:

1. treat the prior `639bf18...` HTTPX 0.28.1 and HTTPX2 2.12.0 Stage C bindings as historical for the current executable tree;
2. audit the exact post-freeze diagnostics delta and its public/native API implications;
3. close any defects found in that narrow delta before freezing;
4. freeze one clean executable SHA;
5. recollect all qualification evidence required by repository policy on that exact SHA;
6. update live profiles, ledgers, plan index, and documentation only after the gates pass;
7. prove that every descendant after the new freeze is documentation/profile/ledger-only.

## Why this corrective pass is required

The prior qualification record states that executable files did not change after `639bf18...`. That statement became stale when `6a0cfd8...` changed:

- `crates/eggfetch-core/src/lib.rs`;
- `crates/eggfetch-core/src/transport/http3.rs`;
- `crates/eggfetch-core/src/transport/metrics.rs`;
- H3 tests and documentation associated with the new diagnostics surface.

The new code exposes bounded copied Quinn-derived HTTP/3 diagnostics including route kind, Alt-Svc generation, remote/local address information, RTT, packet/byte counters, loss counters, and sanitized close classification. Because this is executable and public native surface, the prior exact-SHA compatibility qualification cannot be carried forward automatically even if the change is behaviorally orthogonal to the Python compatibility facades.

## Scope boundaries

Included:

- the `6a0cfd8...` HTTP/3 diagnostics delta and any narrowly necessary correction discovered while auditing it;
- focused H3 diagnostics/lifecycle tests;
- full existing repository qualification tiers;
- HTTPX 0.28.1 and HTTPX2 2.12.0 exact-SHA requalification;
- qualification/profile/ledger/index truth repair;
- documentation reconciliation required by the corrected binding.

Excluded:

- changing the retained-experimental H3 graduation outcome;
- adding new external H3 implementations or public-origin evidence;
- adding 0-RTT, WebTransport, datagrams, MASQUE/CONNECT-UDP, migration, or H3-by-default behavior;
- redesigning `TransportMetrics` outside what is required to make the diagnostics surface safe and truthful;
- creating new CI matrices, workflows, evidence formats, or release automation;
- unrelated HTTPX/HTTPX2 feature work.

## 1. Mark the prior binding stale before collecting new evidence

Before treating any current-tree result as qualification evidence:

- update working notes/status so `639bf18...` is clearly historical for the current executable tree;
- do not claim current `main` is Stage C-bound until the new freeze passes;
- preserve the historical evidence verbatim rather than deleting it;
- keep H3's retained-experimental decision separate from HTTPX compatibility stage.

Acceptance:

- [x] no live document implies `639bf18...` is the executable SHA of current `main`;
- [x] historical evidence remains auditable;
- [x] H3 experimental status is not conflated with HTTPX Stage C status.

## 2. Audit the post-freeze diagnostics delta

Review `639bf18..6a0cfd8` with emphasis on executable/native API behavior.

Verify:

### Ownership and boundedness

- diagnostics store copied scalar/value data only;
- `TransportMetrics` does not retain `quinn::Connection`, H3 sender/driver objects, response bodies, request state, origin strings, headers, credentials, or peer-provided reason strings;
- diagnostic storage has a fixed bound with deterministic eviction/replacement semantics;
- repeated connection churn cannot cause monotonic memory growth through diagnostics;
- `stable_id` is diagnostic-only and is not treated as globally durable identity.

### Truthfulness

- all exposed fields come directly from Quinn or EggFetch route state;
- unavailable values remain `None` rather than inferred;
- packet/byte/loss counters have documented connection/path scope and are not mislabeled as request metrics;
- close classification preserves protocol/application codes without leaking peer reason text;
- graceful H3 close/drain classification remains based on typed/public upstream state where available, not string parsing;
- diagnostics captured before and after close cannot be mistaken for the same temporal snapshot.

### Privacy/security

- diagnostics contain no Host/origin name, cookies, authorization headers, proxy credentials, request/response bodies, certificate contents, or arbitrary peer strings;
- address exposure is intentional native observability and documented as such;
- Debug/Display implementations do not accidentally reintroduce sensitive state;
- Alt-Svc generation data does not expose advertisement contents.

### Lifecycle

- diagnostic capture does not alter connection lifetime;
- driver shutdown, drain, reconnect, cache eviction, and Client drop still release QUIC resources;
- recording a close snapshot cannot deadlock or panic during teardown;
- metrics locking cannot block H3 driver progress indefinitely;
- diagnostics remain valid when a connection disappears between ordinary request operations.

### Feature gating/API stability

- all H3-only exported types are correctly behind `feature = "http3"`;
- builds without `http3` remain clean;
- public names/types are deliberate and documented as native EggFetch API rather than HTTPX compatibility surface;
- adding diagnostics does not mutate `eggfetch.compat.httpx` or `eggfetch.compat.httpx2` public APIs.

Acceptance:

- [x] delta audit produces no unresolved correctness/security/ownership issue;
- [x] any correction lands before the new freeze;
- [x] no unrelated cleanup is bundled into the corrective pass.

## 3. Focused diagnostics regression gate

Add or confirm focused tests covering the new diagnostics behavior. Prefer deterministic loopback H3 fixtures and direct unit tests over external infrastructure.

Required coverage:

- a successful explicit `Http3Only` connection records an `Explicit` route snapshot;
- an Alt-Svc-selected H3 connection records `AltSvc` plus the correct generation;
- RTT/address/path counters are copied and structurally plausible without exact-value assertions that would be flaky;
- diagnostics storage never exceeds its documented bound under connection churn;
- graceful and non-graceful closes produce sanitized close classifications;
- no peer reason string or origin/credential data appears in diagnostics/debug output;
- `open_streams` or equivalent unavailable fields remain explicitly unavailable;
- dropping the client/connection does not keep live QUIC handles solely because diagnostics were recorded;
- no-http3 feature builds compile without H3 diagnostic exports;
- H3 hardening, Alt-Svc, interop-loopback, drain/reconnect, and timeout tests remain green.

If the diagnostic ring/buffer uses replacement semantics, test wraparound explicitly.

Acceptance:

- [x] focused diagnostics tests pass repeatedly;
- [x] existing H3 deterministic suites remain green;
- [x] no test relies on public Internet or an optional independent server for this corrective closure.

## 4. Freeze one new executable SHA

After Sections 1-3 are complete:

- commit every source, test, script, manifest, lockfile, packaging, and qualification-sensitive change;
- ensure a clean worktree;
- record that commit as `FROZEN_EXECUTABLE_SHA`;
- do not edit executable/test/script/manifest/package files after the freeze;
- if any such file changes, discard all collected qualification evidence and freeze again.

The freeze must include the final diagnostics implementation and all focused tests.

Acceptance:

- [x] exactly one clean new executable SHA is named;
- [x] no qualification evidence is collected from a dirty or mixed tree;
- [x] all later record commits are executable-identical descendants.

## 5. Focused pre-qualification gate on the frozen SHA

Before the expensive full qualification, run the high-risk clusters directly.

Required minimum:

- H3 diagnostics-focused tests;
- `h3_hardening`;
- `h3_alt_svc_discovery`;
- deterministic/local portion of `h3_interop_qualification`;
- transport metrics exact-count/bound tests;
- feature-matrix checks with and without `http3`;
- Python import/isolation checks proving no compatibility-facade API leakage;
- HTTPX 0.28.1 and HTTPX2 API oracle checks.

The independent/public H3 evidence gates remain intentionally outside this corrective pass; their absence continues to justify `experimental` status and must not make this requalification fail.

Acceptance:

- [x] all focused gates are green on the exact freeze SHA;
- [x] API oracles show zero new unexplained/stale differences;
- [x] H3 experimental blockers remain recorded, not waived.

## 6. Full repository verification on the frozen SHA

Run the repository's existing qualification tiers without inventing a new workflow.

### Tier 1

Run `./scripts/check.sh` and require the normal formatting, lint, Rust workspace tests, Python behavior tests, compatibility smoke, and existing validation hooks to pass.

### Tier 2 / extended

Run the existing extended verification including:

- full compatibility suite;
- dual compatibility API oracles;
- feature matrix / feature tests;
- documentation/examples where currently required;
- lifecycle/soak/resource checks;
- FFI/native validation already part of repository policy;
- required downstream compatibility portfolio.

Explicit existing skips remain acceptable only where repository policy already permits them. Do not convert a new failure into a skip for this corrective closure.

### Tier 3 / package

Run existing package validation, including crate dry-run/package content and Python wheel/package smoke according to current policy.

Acceptance:

- [x] Tier 1 green;
- [x] extended green;
- [x] package validation green;
- [x] required downstream portfolio green;
- [x] no new waiver was introduced to force closure.

## 7. Requalify HTTPX 0.28.1 on the new freeze

The diagnostics change is native/H3-focused, but exact-SHA policy requires renewed evidence for the compatibility facade.

On the frozen SHA:

- run the full pinned HTTPX 0.28.1 compatibility suite three consecutive times with required mode enabled;
- run the API oracle against `httpx==0.28.1`;
- verify zero unexplained and zero stale differences;
- run the required downstream portfolio;
- confirm the facade still does not expose the new native H3 diagnostic types unless already intentionally part of its contract;
- preserve existing allowed differences unless evidence requires a deliberate change.

Acceptance:

- [x] three consecutive full runs pass without source/dependency changes between runs;
- [x] API oracle clean;
- [x] downstream required set green;
- [x] profile may be rebound to the new SHA only after all gates pass.

## 8. Requalify HTTPX2 2.12.0 independently

On the same frozen SHA:

- run the full pinned HTTPX2 2.12.0 compatibility corpus as part of the three full compatibility runs;
- run its independent API oracle;
- confirm SSE lifecycle and optional WebSocket surface remain green;
- confirm HTTPX2-specific symbols do not leak into the 0.28.1 facade and native H3 diagnostics do not leak into either facade unintentionally;
- run relevant required downstreams.

Acceptance:

- [x] HTTPX2 evidence independently supports Stage C on the same frozen SHA;
- [x] zero unexplained/stale oracle differences;
- [x] no cross-profile leakage;
- [x] profile update occurs only after evidence is complete.

## 9. Remote CI evidence

Use the repository's existing routine CI only.

Preferred evidence:

1. a successful CI run whose head is the frozen executable SHA itself; or
2. if the record/profile update commit triggers the first available successful run, prove via descendant audit that it is executable-identical to the freeze.

Do not create a special qualification workflow merely for this corrective pass.

Acceptance:

- [x] at least one successful existing-CI result is recorded;
- [x] its relationship to `FROZEN_EXECUTABLE_SHA` is explicit;
- [x] no failed current-tree CI is omitted from the closure record.

## 10. Record/profile truth repair

Only after Sections 5-9 pass, update:

- `compat/httpx/0.28.1/profile.toml`;
- `compat/httpx2/2.12.0/profile.toml`;
- their compatibility READMEs where they display current binding;
- `plans/httpx-parity-correction-status.md`;
- `plans/README.md`;
- `plans/ROADMAP.md` only as needed to reflect the corrected current state;
- H3 architecture/reference docs only where the diagnostics API or qualification binding needs reconciliation.

Required record semantics:

- prior `639bf18...` qualification becomes historical;
- new `qualification-sha` equals `FROZEN_EXECUTABLE_SHA` exactly in both profiles;
- evidence counts/commands are copied from actual runs, not inferred from the previous pass;
- HTTP/3 remains **experimental** with the same external blockers unless separately satisfied with new evidence;
- diagnostics are described as supported native observability on the qualified tree, not as evidence that H3 itself graduated.

Acceptance:

- [x] profiles and live ledger agree on exact SHA/date/stage;
- [x] plan index accurately distinguishes qualification closure from H3 graduation;
- [x] no current document says the diagnostics commit is a docs-only descendant of `639bf18...`.

## 11. Post-record descendant audit

Compare `FROZEN_EXECUTABLE_SHA` to final `main` after record/documentation commits.

Allowed post-freeze changes:

- Markdown/documentation;
- compatibility profile TOML used only as qualification metadata;
- qualification/status ledger files;
- plan index/roadmap text.

Disallowed without another requalification:

- Rust/Python/JS source;
- tests;
- scripts;
- Cargo/Python/Node manifests or lockfiles;
- workflows/validation behavior;
- packaging configuration;
- machine-readable qualification inputs that alter what is executed.

Acceptance:

- [x] descendant audit lists every changed file and classifies it;
- [x] no qualification-sensitive file changed after freeze;
- [x] final `main` is executable-identical to the qualified SHA.

## H3 graduation status during this closure

This plan must not reopen or weaken the production-graduation gate.

HTTP/3 remains experimental because the prior qualification program still lacks required evidence including:

- at least two independent non-Quinn implementation passes over the required corpus;
- independent GOAWAY/drain/reconnect behavior;
- realistic public-origin qualification evidence;
- executed realistic loss/latency/reordering/MTU/address-family impairment matrix;
- closure/disposition of ordinary-client upstream correctness risks to the level required by the production graduation program.

The new diagnostics surface improves the ability to collect that evidence later, but does not itself satisfy it.

## Validation summary

The corrective closure is complete only when all of the following are true on one exact frozen executable SHA:

- [x] diagnostics delta audit clean;
- [x] focused diagnostics/H3 gates green;
- [x] Tier 1 green;
- [x] extended green;
- [x] package validation green;
- [x] three consecutive full pinned compatibility runs green;
- [x] HTTPX 0.28.1 oracle clean;
- [x] HTTPX2 2.12.0 oracle clean;
- [x] required downstream portfolio green;
- [x] existing remote CI success recorded;
- [x] both profiles rebound to the freeze SHA;
- [x] live ledger/index repaired;
- [x] final descendant audit proves no post-freeze executable change;
- [x] H3 remains experimental with unresolved external evidence named explicitly.

## Exit criteria

This corrective pass is closed when the repository can truthfully state all three of the following simultaneously:

1. current `main` is executable-identical to one newly qualified frozen SHA;
2. HTTPX 0.28.1 and HTTPX2 2.12.0 are Stage C qualified on that exact SHA with fresh evidence;
3. HTTP/3 diagnostics are part of that qualified executable tree while HTTP/3 itself remains experimental pending the independent production-graduation evidence.

## Closure record (2026-09-11)

The clean executable freeze is `78a77ea153aae239ce7b722aeb9909a87df3bbb5`,
whose parent is the prior qualification tree. The diagnostics audit found and
corrected the native `received_packets`/UDP-datagram counter conflation; no
other executable defect was identified. Focused evidence is green:

- `h3_hardening`: 12/12;
- `h3_alt_svc_discovery`: 17/17, including explicit-route diagnostics;
- `h3_interop_qualification`: 20/20;
- native close classification unit coverage: 1/1, with typed close codes and
  peer reason text excluded from debug output.

On that unchanged executable tree, Tier 1, extended, and package validation
all passed. The extended tier's downstream portfolio passed 4/4, and its API
oracles were clean with 71 allowed HTTPX 0.28.1 differences and 79 allowed
HTTPX2 2.12.0 differences. The extended-tier compatibility run plus two
additional consecutive pinned runs each passed 1,870 tests with 26
non-failing warnings and no skips, xfails, or failures. The profiles and live
ledger now bind both Stage C claims to the freeze SHA. HTTP/3 remains
experimental with the external blockers named above.

Remote routine CI run `34620344393` passed on documentation/profile/ledger
descendant `3fd26bbc81b236fce2693fec406fa2a61865dad3`:
https://github.com/eggstack/eggfetch/actions/runs/34620344393. The final
post-freeze descendant audit is limited to the 17 files changed by the
documentation/profile/ledger commit: the three repository guidance files in
`.skills/`, `AGENTS.md`, `README.md`, the two compatibility-profile READMEs,
the two profile TOMLs, the two affected architecture documents, the two
reference documents, and the four plan/status documents. No Rust/Python/JS
source, test, script, manifest, lockfile, workflow, packaging, or
qualification-input file changed after `78a77ea`; `main` remains
executable-identical to that freeze.
