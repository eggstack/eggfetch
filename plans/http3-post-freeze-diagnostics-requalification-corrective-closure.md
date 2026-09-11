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

- [ ] no live document implies `639bf18...` is the executable SHA of current `main`;
- [ ] historical evidence remains auditable;
- [ ] H3 experimental status is not conflated with HTTPX Stage C status.

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

- [ ] delta audit produces no unresolved correctness/security/ownership issue;
- [ ] any correction lands before the new freeze;
- [ ] no unrelated cleanup is bundled into the corrective pass.

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

- [ ] focused diagnostics tests pass repeatedly;
- [ ] existing H3 deterministic suites remain green;
- [ ] no test relies on public Internet or an optional independent server for this corrective closure.

## 4. Freeze one new executable SHA

After Sections 1-3 are complete:

- commit every source, test, script, manifest, lockfile, packaging, and qualification-sensitive change;
- ensure a clean worktree;
- record that commit as `FROZEN_EXECUTABLE_SHA`;
- do not edit executable/test/script/manifest/package files after the freeze;
- if any such file changes, discard all collected qualification evidence and freeze again.

The freeze must include the final diagnostics implementation and all focused tests.

Acceptance:

- [ ] exactly one clean new executable SHA is named;
- [ ] no qualification evidence is collected from a dirty or mixed tree;
- [ ] all later record commits are executable-identical descendants.

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

- [ ] all focused gates are green on the exact freeze SHA;
- [ ] API oracles show zero new unexplained/stale differences;
- [ ] H3 experimental blockers remain recorded, not waived.

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

- [ ] Tier 1 green;
- [ ] extended green;
- [ ] package validation green;
- [ ] required downstream portfolio green;
- [ ] no new waiver was introduced to force closure.

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

- [ ] three consecutive full runs pass without source/dependency changes between runs;
- [ ] API oracle clean;
- [ ] downstream required set green;
- [ ] profile may be rebound to the new SHA only after all gates pass.

## 8. Requalify HTTPX2 2.12.0 independently

On the same frozen SHA:

- run the full pinned HTTPX2 2.12.0 compatibility corpus as part of the three full compatibility runs;
- run its independent API oracle;
- confirm SSE lifecycle and optional WebSocket surface remain green;
- confirm HTTPX2-specific symbols do not leak into the 0.28.1 facade and native H3 diagnostics do not leak into either facade unintentionally;
- run relevant required downstreams.

Acceptance:

- [ ] HTTPX2 evidence independently supports Stage C on the same frozen SHA;
- [ ] zero unexplained/stale oracle differences;
- [ ] no cross-profile leakage;
- [ ] profile update occurs only after evidence is complete.

## 9. Remote CI evidence

Use the repository's existing routine CI only.

Preferred evidence:

1. a successful CI run whose head is the frozen executable SHA itself; or
2. if the record/profile update commit triggers the first available successful run, prove via descendant audit that it is executable-identical to the freeze.

Do not create a special qualification workflow merely for this corrective pass.

Acceptance:

- [ ] at least one successful existing-CI result is recorded;
- [ ] its relationship to `FROZEN_EXECUTABLE_SHA` is explicit;
- [ ] no failed current-tree CI is omitted from the closure record.

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

- [ ] profiles and live ledger agree on exact SHA/date/stage;
- [ ] plan index accurately distinguishes qualification closure from H3 graduation;
- [ ] no current document says the diagnostics commit is a docs-only descendant of `639bf18...`.

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

- [ ] descendant audit lists every changed file and classifies it;
- [ ] no qualification-sensitive file changed after freeze;
- [ ] final `main` is executable-identical to the qualified SHA.

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

- [ ] diagnostics delta audit clean;
- [ ] focused diagnostics/H3 gates green;
- [ ] Tier 1 green;
- [ ] extended green;
- [ ] package validation green;
- [ ] three consecutive full pinned compatibility runs green;
- [ ] HTTPX 0.28.1 oracle clean;
- [ ] HTTPX2 2.12.0 oracle clean;
- [ ] required downstream portfolio green;
- [ ] existing remote CI success recorded;
- [ ] both profiles rebound to the freeze SHA;
- [ ] live ledger/index repaired;
- [ ] final descendant audit proves no post-freeze executable change;
- [ ] H3 remains experimental with unresolved external evidence named explicitly.

## Exit criteria

This corrective pass is closed when the repository can truthfully state all three of the following simultaneously:

1. current `main` is executable-identical to one newly qualified frozen SHA;
2. HTTPX 0.28.1 and HTTPX2 2.12.0 are Stage C qualified on that exact SHA with fresh evidence;
3. HTTP/3 diagnostics are part of that qualified executable tree while HTTP/3 itself remains experimental pending the independent production-graduation evidence.
