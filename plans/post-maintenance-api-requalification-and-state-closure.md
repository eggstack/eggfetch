# Post-Maintenance API Requalification and State Closure

Planning baseline: 03ecba973010e2858bf16a2b5f84d51ce70adae4
Parent program: plans/api-preserving-maintenance-and-interop-hardening-program.md
Predecessor child plans:
- plans/rust-public-api-regression-oracle.md
- plans/python-native-surface-relational-guardrails.md
- plans/python-ssl-context-private-contract-hardening.md
- plans/core-private-module-decomposition.md
- plans/connect-wire-overlap-conformance.md
Normative verification policy: docs/verification-policy.md

## Objective

Freeze one final executable candidate for the API-preserving maintenance campaign, prove zero supported-surface/capability regression relative to the planning baseline, renew exact-SHA compatibility evidence only where current policy requires it, and reconcile repository planning/issue state truthfully.

This plan is the sole final qualification owner for the campaign. Child plans may run focused/Tier 1/extended checks while developing, but they must not create competing final freeze claims.

## Part A — establish the final executable freeze

Identify the last commit in this campaign that changes any of:

- Rust/Python executable source;
- tests whose behavior affects qualification;
- validation scripts/oracles;
- Cargo/Python packaging metadata;
- feature wiring;
- build configuration.

Record:

- planning baseline SHA 03ecba973010e2858bf16a2b5f84d51ce70adae4;
- final executable/test/tooling SHA;
- rustc/cargo stable versions used for the main qualification;
- exact Rust 1.89.0 MSRV result;
- Python version used for primary native/compat qualification;
- pinned Rust public-API oracle tool/nightly versions;
- pinned semver-check tool version if retained;
- OS/architecture;
- any truthfully skipped optional artifacts.

After this freeze, only documentation/evidence/index/issue bookkeeping may land without reopening executable qualification.

## Part B — exact Rust API proof

Run the Rust API oracle established by rust-public-api-regression-oracle.md against the planning baseline.

Required result:

- zero added public items;
- zero removed public items;
- zero moved canonical public paths;
- zero signature/generic-bound/trait-bound drift;
- zero public field/variant-shape drift;
- zero feature-profile exposure drift for every reviewed profile;
- historical compatibility re-export paths still compile.

Run the complementary semver checker and fast compile-contract fixtures.

Do not regenerate snapshots to match the candidate unless a separately approved public API change superseded this campaign; if that occurred, this campaign is no longer API-preserving and must be reopened/re-scoped.

## Part C — native Python public-surface proof

Run:

- scripts/check_native_python_api.py;
- scripts/check_python_typing_surface.py;
- scripts/check_python_typing.py;
- the new relational guardrail mutation/self-tests;
- ordinary native Python tests from a rebuilt extension.

Require:

- unchanged eggfetch.__all__;
- unchanged constructor/method/runtime signatures;
- unchanged exception bases/MRO;
- unchanged reviewed properties/method inventory;
- unchanged semantic typing returns;
- expected Client/AsyncClient/top-level mirror relationships;
- no new public stub-only or runtime-only symbols.

The private _ssl_context contract is not a new supported export and must not appear in eggfetch.__all__ or public stubs.

## Part D — SSL/TLS security and compatibility proof

Run all SSLContext-focused tests added or affected by the campaign, including:

- private schema/version/type validation;
- helper registry mutation invalidation;
- custom CA translation;
- verify=False;
- certificate verification with hostname verification disabled;
- TLS 1.2/1.3 bounds;
- unrepresentable context rejection;
- helper mTLS provenance;
- destination TLS network proof;
- proxy TLS network proof;
- bounded CA extraction;
- secret-redaction controls.

No new residual difference or compatibility waiver may be added solely to make this refactor pass.

## Part E — core/private decomposition proof

Run focused core tests proving no behavior changed through file/module moves:

- client configuration/defaults;
- URL validation/redaction;
- request/content-length behavior;
- route selection;
- standard/direct/custom/SNI/UDS/resolved-target behavior;
- route/client cache key isolation and bounds;
- HTTP-version policy;
- proxy URL/NO_PROXY/environment behavior;
- native versus HTTPX NO_PROXY distinctions;
- proxy auth/redaction;
- TLS/proxy connection identity;
- transport metrics relevant to moved code.

Inspect visibility after the final tree:

- no new public module;
- no accidental pub item;
- no optional feature became unconditional;
- no new production dependency.

## Part F — CONNECT conformance proof

Run the final differential/conformance suite for:

- ConnectTarget formatting;
- request serialization;
- response-head parsing;
- bounds;
- malformed/truncated input;
- read-ahead preservation;
- ProxyAuth boundary inputs;
- production error mapping/redaction.

Confirm source ownership:

- transport/connect.rs still delegates wire request/response mechanics to eggfetch-http-connect;
- status/retry/TLS/deadline/routing policy stays in core;
- no new production parser was introduced.

Record whether auth/parser overlap was safely reduced or intentionally retained. Both are acceptable if the acceptance criteria of the child plan are satisfied.

## Part G — HTTPX/HTTPX2 and adapter regression proof

Because Python binding/TLS/core private plumbing can affect compatibility even when the public surface is unchanged, run:

- full HTTPX 0.28.1 compatibility suite;
- full HTTPX2 2.12.0 compatibility suite;
- both API-oracle comparisons;
- C FFI tests;
- Node Rust tests;
- Node JS surface only when the artifact exists under current policy;
- CLI tests;
- lifecycle/shutdown/soak/resource controls;
- frame/trailer/native transport tests;
- downstream compatibility when its artifact manifest is available.

Do not convert optional artifact absence into PASS. Record the current policy-defined skip reason.

HTTP/3 remains experimental. Run only the ordinary repository H3 tests required by current extended validation; this campaign does not reopen the external graduation gate.

## Part H — canonical repository gates

On the final executable SHA run current policy:

- ./scripts/check.sh;
- ./scripts/check.sh extended;
- ./scripts/check.sh package if source/tooling/package changes require package qualification under current policy;
- ./scripts/check_security.sh;
- exact Rust 1.89.0 MSRV checks owned by extended validation;
- cargo doc/workspace doctests through the canonical scripts.

Follow current docs/verification-policy.md rather than copying obsolete historical repetition requirements.

## Part I — exact-SHA compatibility evidence

If this campaign changes executable source/tests/validation in a way that invalidates the currently recorded Stage C executable freeze, renew only the canonical exact-SHA compatibility records identified by AGENTS.md/current policy after all gates pass.

Do not scatter the new SHA through historical plans or unrelated docs.

If the campaign's only final descendants are documentation/index changes, clearly distinguish final executable SHA from documentation head SHA.

## Part J — planning and repository state hygiene

After executable qualification passes:

### New campaign

- mark the parent program complete;
- add execution records to each child plan describing what changed, what was deliberately retained, and the final evidence;
- update plans/README.md from Active to Completed for this campaign.

### Existing stale headings

Review plans/README.md headings whose own Status field says complete but whose heading still says Active.

At minimum inspect:

- second-pass performance closure truth/semantics;
- second-pass performance ownership optimization;
- issue #24 corrective.

Change headings only when their recorded status actually supports completion. Do not rewrite historical evidence.

### Issue #24

Reconcile the GitHub issue with repository truth.

If 0.1.9 has been published and the downstream-facing fix is available, close the issue with a concise reference to the fixed release/qualification evidence.

If publication is still pending, do not falsely close it. Instead ensure plans/README.md and the issue clearly say implementation/qualification are complete but publication is pending.

Issue state is repository/project hygiene and must not be represented as code qualification.

### Python 3.15

Do not close or relabel the separate Python 3.15 wheel-production plan unless its required publish=false 18-wheel rehearsal has actually run and passed.

This maintenance campaign may touch Python tooling but does not substitute for that release-matrix evidence.

## Part K — documentation truth pass

Review only directly affected docs:

- docs/reference/versioning.md;
- docs/verification-policy.md;
- docs/architecture/build-ci.md;
- docs/architecture/python-bindings.md;
- docs/architecture/core-engine.md;
- docs/architecture/core-tls-proxy-protocols.md;
- AGENTS.md.

Required outcome:

- Rust API oracle workflow is discoverable;
- private SSLContext schema is described only as internal architecture;
- new private module locations are accurate;
- CONNECT ownership description matches implementation;
- no docs imply a capability addition;
- H3/Node maturity labels are unchanged.

## Completion criteria

- [ ] One final executable SHA is recorded.
- [ ] Exact Rust public-surface comparison to 03ecba973010e2858bf16a2b5f84d51ce70adae4 reports zero drift in all reviewed profiles.
- [ ] Fast Rust compile-contract and semver checks pass.
- [ ] Native Python manifest, relational guardrails, typing checks, and ordinary tests report zero public drift.
- [ ] SSLContext private contract/security/network tests pass with unchanged accepted/rejected behavior.
- [ ] Core private decomposition tests pass with no visibility/feature/dependency drift.
- [ ] CONNECT conformance tests pass and production wire ownership remains singular.
- [ ] HTTPX 0.28.1 and HTTPX2 2.12.0 compatibility/API-oracle results contain zero new unexplained differences.
- [ ] FFI/CLI/Node-Rust/lifecycle/resource/native-frame checks are green or truthfully skipped per policy.
- [ ] Tier 1, extended, applicable package, security, docs, and exact MSRV gates pass.
- [ ] Canonical exact-SHA compatibility records are renewed when required.
- [ ] plans/README.md accurately distinguishes active, completed, and separately pending work.
- [ ] Issue #24 state matches publication reality.
- [ ] Python 3.15 rehearsal remains open unless independently completed.
- [ ] Parent and child plans contain final execution records.

## Stop conditions

Do not close the campaign if:

- any supported public Rust/Python/C/CLI surface changed;
- any feature/default exposure changed;
- an SSLContext case became newly accepted/rejected;
- a compatibility waiver was added to excuse a refactor;
- CONNECT auth or parser behavior changed outside documented equivalence;
- Node/H3 maturity was broadened;
- a required qualification gate failed;
- an optional skip was reported as a pass;
- repository status text claims publication/qualification that has not occurred.

If a public change becomes necessary, stop this campaign and open a separate versioned API-change program rather than silently absorbing it into maintenance.

## Execution record — local closure (2026-09-21)

The final executable/test/tooling freeze is
`df2549f7c64ebfccde61ed36fef785d39e83b38d`, based on planning baseline
`03ecba973010e2858bf16a2b5f84d51ce70adae4`. Qualification used Rust 1.98.1,
exact MSRV Rust 1.89.0, Python 3.12.3, Linux x86_64, and the pinned Rust API
toolchain/tooling recorded in the child plan.

Local evidence is complete: Tier 1; extended compatibility (including HTTPX
0.28.1 and HTTPX2 2.12.0); exact Rust API/semver; feature matrix; docs; FFI;
lifecycle/soak; package; security; and wheel smoke/typing checks all pass.
Policy-approved skips are the absent Node native artifact and absent downstream
qualification artifact. Issue #24 remains fixed-but-unpublished, and Python
3.15 remains under its separate pending rehearsal plan. Remote CI run
`35606617959` passed for head `9f16730cece5a1dbf6c936079852de165ed21add`.
