# eggfetch Plan Index

This directory contains active implementation plans, live qualification/status records, and historical implementation records. Completed plans are non-normative unless another current document explicitly says otherwise. Verification and release policy remain governed by `docs/verification-policy.md` and `docs/releases/process.md`.

## Active handoff — native request failure introspection (2026-09-14)

Handoff plan: `native-request-failure-introspection.md`

Objective: add an opt-in, transport-generic native Rust request-failure surface that can preserve structured DNS/refusal/connect provenance before the existing public `Error::Connect(String)` collapse, without changing the existing `Error` enum, `Error::kind()` tokens, ordinary `send()` APIs, Python/CLI/HTTPX behavior, or transport policy. The same plan clarifies that `max_decoded_body_size` already bounds unencoded/identity responses as well as decoded compressed bodies; it does not add another body-limit implementation.

The motivating Gregg review is requirements evidence only. No Gregg, EggPool, monitoring, endpoint-status, provider, or other downstream-specific type or adapter belongs in eggfetch. This is a maintenance/ergonomics improvement for unrelated native embedders, not a binary-footprint claim.

## Completed program — native HTTP body and TLS extensibility (2026-09-14)

Handoff program: `native-http-body-and-tls-extensibility-program.md`

Objective: extend `eggfetch-core` for transport-oriented native Rust consumers without changing existing high-level client semantics or adding downstream-specific adapters. Implementation completed in `fdfe060cd7035ddf936d6e0817cb2459f9d3fc0c`; final qualification-only provider/mTLS test hardening completed in `1ea63ba1a4ea81ab548c2a7c9af5563d975793cd`. The program adds a frame-preserving `http_body::Body` interoperability surface, explicit per-`TlsConfig` Rustls `CryptoProvider` selection with provider-neutral mTLS key loading, and explicit additional trust anchors layered on top of the existing base trust policy. Existing `RequestBody`, `ResponseBody`, `TrustStore`, Python/CLI, HTTPX/HTTPX2 and replacement custom-CA semantics remain compatible.

Execution order:

1. `native-http-body-interoperability.md` — add an additive native frame-preserving request/response body surface that reuses the existing transport/TLS/pool engine without exposing Hyper internals or changing public body enums.
2. `tls-crypto-provider-extensibility.md` — allow caller-supplied Rustls `CryptoProvider` policy per `TlsConfig`, remove hard-coded ring key loading from mTLS, and retain current ring/default behavior for callers that do not opt in.
3. `tls-additional-trust-anchors.md` — add a distinct native API for augmenting native/WebPKI/custom base trust with private roots while preserving all existing replacement-style CA APIs.
4. `post-native-http-body-and-tls-extensibility-qualification-and-closure.md` — completed the integration/API-boundedness audit, external-style qualification, dependency/feature checks, exact-SHA HTTPX/HTTPX2 requalification, and documentation/plan closure. Tier 1, extended, package, and three consecutive 1,870-test compatibility runs passed on final qualification tree `1ea63ba1a4ea81ab548c2a7c9af5563d975793cd`. Known optional skips remain explicitly recorded in the program closure record.

The motivating Synvoid evaluation is requirements evidence only. No Synvoid type, feature flag, routing/WAF/site/backend policy, provider-brand product mode, or downstream migration code belongs in eggfetch. The public additions must remain useful to unrelated gateways, service meshes, private-PKI clients, custom-network clients and other native Rust embedders.

## Completed program — extensible embedded transport consumers (2026-09-14)

Handoff program: `extensible-embedded-transport-consumer-program.md`

Objective: let native Rust consumers reuse eggfetch as the single HTTP/TLS engine when they already own the underlying network route, without adding downstream-specific adapters or changing existing retry, pooling, timeout, Python/CLI, HTTPX, or HTTP/3 defaults. The implementation landed in `af03f006f377979550ee6cb96a30b28193c8708d`; qualification and documentation closure completed on executable freeze `43c68bd1bcff45301fc8b6b163b6b6e06d98a786`. This is an ownership/control program, not a binary-size claim; the latest embedded record remains **not a footprint win** against aligned reqwest profiles.

Execution order:

1. `custom-dialer-transport-extension.md` — add a general caller-supplied raw-stream dialer below eggfetch-owned HTTP/TLS, with fail-closed route-combination semantics and no protocol-specific coupling.
2. `strict-underlying-transport-attempt-control.md` — expose Hyper's canceled-request retry policy separately from eggfetch `RetryPolicy` and apply it consistently to every Hyper client route.
3. `physical-connection-admission-and-io-inactivity-guardrails.md` — add opt-in physical live-connection admission and established-transport read/write inactivity controls without changing logical `PoolConfig` or existing `Timeout` meanings.
4. `post-extensible-transport-qualification-and-closure.md` — completed the synthetic external-style consumer, dependency/footprint impact, repository gates and exact-SHA compatibility, then performed documentation/plan closure.

The public API must remain transport-generic. No Eggpool, Eggress, provider/account, SSH, pproxy, Trojan, Shadowsocks or other downstream protocol model becomes part of eggfetch. Downstream migration remains outside this repository.

## Completed program — embedded Rust client footprint and routing (2026-09-12)

Handoff program: `embedded-rust-client-footprint-and-routing-program.md`

Objective: make `eggfetch-core` a cleaner embedded Rust HTTP engine with truthful feature/dependency ownership, first-class caller-supplied resolved-destination routing, native JSON ergonomics, and measured footprint evidence, without compromising default eggfetch capabilities, Python/CLI behavior, HTTPX compatibility, or existing transport/security goals.

Execution order and outcomes:

1. `core-feature-dependency-and-tls-boundary-hardening.md` — done; feature
   ownership and TLS trust construction are explicit while secure defaults
   remain unchanged.
2. `static-resolution-and-pinned-destination-routing.md` — done; typed
   request-scoped static routing preserves logical identity and fails closed
   for incompatible redirects/routes.
3. `native-rust-json-and-response-ergonomics.md` — done; native request and
   response JSON helpers are owned by the opt-in `json` feature.
4. `embedded-consumer-footprint-qualification.md` — done; bounded evidence
   classifies the result as **not a footprint win** against aligned reqwest
   Rustls profiles.
5. `post-embedded-engine-compatibility-requalification-and-closure.md` —
   done on frozen executable/test/fixture SHA
   `22a6f5c0dc0207c1356b6143c0eda3d4075063b0`; Tier 1, extended, package,
   API oracles, and three consecutive full compatibility runs passed.
6. `post-embedded-engine-documentation-and-plan-hygiene.md` — done as the
   documentation/profile/plan-index-only descendant of that SHA. The final
   documentation commit `a7df01df31fd624e3b6fff1db48a0120fd8b9779` passed
   routine remote CI in run `34741393283`.

Plans 1–4 were executable/test/qualification work and invalidated the prior
exact-SHA compatibility evidence. Plan 5 owned the single post-program freeze;
Plan 6 remained documentation/profile/ledger-only. The final exact-SHA
compatibility records are the two versioned profiles under `compat/`.

This program does **not** migrate CodeGG or add a CodeGG-specific facade. The
engine is ready for downstream migration evaluation on ownership/control/API
grounds, but any downstream migration remains outside eggfetch scope.

## Completed corrective closure — H3 post-freeze diagnostics requalification (2026-09-11)

Handoff plan: `http3-post-freeze-diagnostics-requalification-corrective-closure.md`

Trigger: executable/native HTTP/3 diagnostics landed in
`6a0cfd87551b7c634593e7b39cd2fd35d128f727` after the prior qualification
freeze `639bf186a71c054e11278d1b160ffe7a6f172c02`. The prior binding is now
historical. The corrected executable tree was frozen at
`78a77ea153aae239ce7b722aeb9909a87df3bbb5`.

This was a narrow corrective closure, not another H3 feature or graduation
program. It completed with the following sequence:

1. audited the diagnostics delta for boundedness, truthfulness, privacy,
   lifecycle safety, and feature gating;
2. closed the directly-found counter-semantics defect and ran focused H3/
   diagnostics regression gates;
3. froze one clean executable SHA;
4. ran Tier 1, extended, package validation, three consecutive full pinned
   compatibility runs, both API oracles, required downstreams, and existing
   remote CI;
5. rebound both compatibility profiles and the live ledger to that exact SHA;
6. performed a final descendant audit proving all post-freeze changes are
  documentation/profile/ledger-only.

The audit corrected the native diagnostics field to report received UDP
datagrams rather than a path-packet counter, documented the bounded copied
snapshot contract, and added explicit-route and sanitized-close regression
coverage. HTTP/3 remains **experimental**; diagnostics do not satisfy the
missing independent non-Quinn interoperability, independent GOAWAY/drain,
public-origin, realistic impairment, or upstream-risk evidence required for
production graduation. The exact evidence is in
`plans/http3-post-freeze-diagnostics-requalification-corrective-closure.md`
and the live HTTPX ledger.

HTTP/3 remains **experimental** throughout this corrective pass. The new
transport diagnostics improve observability but do not satisfy the missing
independent non-Quinn interoperability, independent GOAWAY/drain, public-origin,
realistic impairment, or upstream-risk evidence required for production
graduation.

## Completed qualification program — HTTP/3 production qualification (2026-09-11)

Handoff program: `http3-production-qualification-program.md`

Objective: close the blockers left by the prior HTTP/3 graduation attempt and make a new evidence-based decision on whether ordinary H3 operation can move from **experimental** to **supported**. This was a qualification/hardening program, not a feature-expansion program.

Execution is complete with a truthful retained-**experimental** outcome. The
child plans remain evidence records, but their unchecked external acceptance
items are not silently treated as satisfied. A future promotion attempt must
supply the missing evidence and perform a new executable freeze and
compatibility requalification.

Execution order:

1. `http3-independent-interop-and-impairment-qualification.md` — build/reuse one implementation-neutral H3 corpus; qualify against at least two independent non-Quinn servers; record public-origin evidence; run realistic loss/latency/reordering/UDP-block/MTU/address-family impairment; prove replay-safe fallback and independent drain/reconnect behavior.
2. `http3-upstream-risk-resource-and-observability-hardening.md` — audit the exact Quinn/h3/h3-quinn/rustls stack and current ordinary-client correctness issues; upgrade/work around only where justified; add targeted regressions; run lifecycle/resource soak; characterize cache pressure; improve real QUIC diagnostics without fabricated response metadata.
3. `http3-production-graduation-and-compatibility-requalification.md` — audit child-plan closure, freeze one exact executable SHA, run full repository/compatibility qualification, make the literal H3 graduation decision, renew HTTPX 0.28.1 and HTTPX2 2.12.0 Stage C on the new executable tree, and perform the documentation/descendant truth pass.

Plans 1 and 2 may overlap where implementation paths do not conflict, but both must close before plan 3 freezes the tree.

Final decision: HTTP/3 remains **experimental**. Deterministic controls and
repository gates are green, but independent non-Quinn interop, independent
GOAWAY/drain, public-origin, realistic impairment, and upstream-risk closure
evidence remain blockers. Missing evidence remains a blocker; it is never
converted into a pass.

The prior HTTPX 0.28.1 and HTTPX2 2.12.0 Stage C profiles were bound to
executable SHA `65beb675a5380d3ff4291da6833b91ebf12c769a`; that binding is
now historical because this program changed tests and qualification tooling.
The profiles were renewed on frozen executable SHA
`639bf186a71c054e11278d1b160ffe7a6f172c02` after three consecutive full
compatibility passes and clean API oracles. That binding is now historical for
current `main` because the later diagnostics commit changed executable code;
the active corrective closure above owns requalification.

Explicitly out of scope for this program: 0-RTT, WebTransport, H3 datagrams, MASQUE/CONNECT-UDP, connection migration, and automatic H3-by-default policy changes.

The current machine-readable evidence ledger is
`http3-independent-interop-and-impairment-qualification-evidence.json`.
The corpus and impairment contracts are under `../qualification/http3/`;
they are qualification inputs, not routine CI gates. The ledger retains the
experimental label until external evidence satisfies the parent gate.

## Completed program — HTTP/3 graduation and next HTTPX compatibility (2026-09-11)

Handoff program: `http3-and-next-httpx-compatibility-program.md`
(completed 2026-09-11 on frozen executable SHA
`65beb675a5380d3ff4291da6833b91ebf12c769a`).

HTTPX 0.28.1 and HTTPX2 2.12.0 had renewed Stage C results on that SHA
(evidence: `httpx-parity-correction-status.md`), but those results are now
historical because the active qualification program changed executable tests
and validation tooling. They were superseded by the qualification on
`639bf186a71c054e11278d1b160ffe7a6f172c02`, which is itself now historical
for current `main` pending the active corrective closure. HTTP/3 retained
experimental with blockers; HTTPX 1.0 remains preview-only. Child plans below
are historical records:

1. `http3-alt-svc-discovery-fallback-and-draining.md` — done.
2. `http3-interoperability-and-production-graduation.md` — done
   (experimental retained with blockers; valid outcome).
3. `httpx2-2.12-profile-and-delta-baseline.md` — done.
4. `httpx2-2.12-core-facade-parity.md` — done.
5. `httpx2-2.12-sse-and-websocket-parity.md` — done.
6. `httpx-1.0-preview-tracking.md` — done (preview-only).
7. `post-next-scope-compatibility-requalification-and-closure.md` — done.
8. `post-next-scope-documentation-and-plan-hygiene.md` — done.

## Normative live compatibility records

`httpx-parity-correction-status.md` remains the live exact-SHA status for
both facades until the active corrective closure updates it. Profiles:
`compat/httpx/0.28.1/profile.toml`, `compat/httpx2/2.12.0/profile.toml`.
Preview: `compat/httpx/1.0-preview/` (unqualified by design; status in
`preview-status.toml`, delta in `preview-delta-0.28.1-vs-1.0.dev6.json`,
notes in `redesign-notes.md`).

### HTTPX 1.0 migration trigger

A future HTTPX 1.0 implementation/qualification program opens only when all
three hold (recorded here and in `plans/ROADMAP.md`; never by renaming the
preview plan into a Stage C plan):

1. upstream publishes an RC with an explicitly frozen public API, or a
   stable 1.0 release;
2. release notes indicate no further major compatibility reset before stable;
3. a fresh delta inventory shows the target is stable enough to justify
   implementation, pinned to the exact RC/stable release.

## Recently completed program — post-audit maturation (2026-09-09)

`post-audit-architecture-and-surface-maturation-program.md` executed and closed:

1. `core-request-and-transport-consolidation.md` — done (`0477d35`)
2. `http3-lifecycle-and-policy-hardening.md` — done (`a505b6c`)
3. `node-binding-maturation.md` — done, experimental outcome (`8139e10`)
4. `native-protocol-observability-and-api-cleanup.md` — done (`363f2e3`)
5. `post-maturation-httpx-requalification-and-closure.md` — done, Stage C renewed on `d034a1005857a7f403222dda4bda5f2f204a44fe`
6. `post-maturation-documentation-and-plan-hygiene.md` — done, documentation-only

The prior Corrective 08 and broad truth-refresh plans remain historical evidence for earlier executable states.

## Historical plans

Other plan files in this directory record prior milestones, corrective passes, validation work and release preparation. Consult them for design history and prior acceptance criteria, but they are not automatically current requirements.

Plan files stay in place; this index is the authoritative navigation layer.
