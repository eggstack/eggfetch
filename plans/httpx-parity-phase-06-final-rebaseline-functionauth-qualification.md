# HTTPX Parity Phase 06 — Final Rebaseline, FunctionAuth Drift, and Exact-SHA Qualification

Status: final handoff/closure plan.
Depends on: Phases 01-05 landed or terminated at their explicit feasibility gates.
Purpose: restore a truthful pinned compatibility contract after executable changes.

## Objective

Rebaseline the HTTPX 0.28.1 compatibility profile after the remaining-parity implementation phases, eliminate stale or contradictory difference records, classify any irreducible residual gaps narrowly, and re-run the repository’s existing exact-SHA qualification procedure.

This phase is mostly contract/evidence work. It must not become a late implementation grab-bag. If a new behavioral failure appears here, either fix it in a small clearly scoped corrective commit with new focused tests or reopen the phase that owns the behavior.

## Why this phase is mandatory

The current Stage C qualification is bound to executable SHA `516d517542b1ed23dfcb31c053c53cdc363cf05b`. Any executable change made by Phases 01-05 invalidates that exact-SHA evidence until a new qualification is completed.

The current profile already has strong infrastructure:

- frozen `httpx==0.28.1` reference API manifest;
- active `allowed-differences.toml`;
- historical `resolved-differences.toml`;
- parity/upstream-derived case registries;
- full pinned compatibility suite;
- API oracle;
- required isolated downstream compatibility runner;
- exact-SHA status record.

Use those mechanisms. Do not introduce a new evidence schema, a new CI workflow, or a second qualification framework.

## Pinned reference rule

The closure contract remains **HTTPX 0.28.1**.

Do not mutate the 0.28.1 reference manifest to match unreleased HTTPX `master` behavior. A future stable HTTPX version should receive its own compatibility profile/versioned reference rather than rewriting historical 0.28.1 evidence.

## Forward drift: `FunctionAuth`

HTTPX `master` contains commit `ae1b9f66238f75ced3ced5e4485408435de10768` (`Expose FunctionAuth in __all__`, 2025-12-10). EggFetch already has an internal `_FunctionAuth` adapter used for callable auth normalization.

For this 0.28.1 closure:

- do **not** add public `FunctionAuth` merely to make EggFetch resemble unreleased master;
- record the upstream drift in a small future-version note/roadmap section if one does not already exist;
- when the next stable HTTPX profile is created, determine whether the upstream public class contract is identical to EggFetch’s internal adapter before exporting/renaming it;
- if maintainers intentionally want an additive EggFetch export before a new profile, treat that as a separate product/API decision and do not count it as 0.28.1 parity.

## Required implementation tracks

### Track 1 — Freeze the final executable SHA

After Phases 01-05 are complete:

1. ensure the working tree contains no unrelated implementation changes;
2. run focused tests for every changed phase;
3. run `./scripts/check.sh`;
4. commit any final executable corrections;
5. record the exact executable SHA before documentation/evidence-only commits continue.

The qualification status must distinguish:

- implementation/executable SHA;
- later documentation/status-only descendant SHAs;
- CI run SHA.

Do not claim a test result from one executable tree as evidence for a different executable tree.

### Track 2 — Regenerate/re-run the API oracle against the pinned manifest

Use the repository’s existing manifest generation and comparison scripts.

Required result classes:

- zero unexplained differences;
- zero stale active allowed differences;
- zero resolved differences remaining in the active allowlist;
- zero malformed difference records.

Review API shape for all areas touched by this program:

- `create_ssl_context`;
- `Client`/`AsyncClient` protocol flags;
- `HTTPTransport`/`AsyncHTTPTransport` protocol flags;
- `Proxy` constructor/header semantics;
- response extension-visible objects if they are part of the public manifest;
- any compatibility network-stream wrapper classes intentionally exported.

Do not whitelist a newly discovered mismatch merely to make the oracle green. Every new active difference needs an owner, exact behavioral tuple, rationale, tests, and classification.

### Track 3 — Reconcile active and resolved difference ledgers

Audit every current active entry related to:

- `create_ssl_context`;
- arbitrary `ssl.SSLContext`;
- `Proxy(headers=...)`;
- `Proxy.ssl_context`;
- HTTP/2-only mode;
- `target`;
- `sni_hostname`;
- `trace`;
- `network_stream`;
- HTTP/2 `stream_id`;
- socket-option four-tuple boundary;
- concurrency backend/Python-version scope.

Rules:

- fully resolved behavior moves to `resolved-differences.toml` with the resolving SHA/tests;
- partially resolved behavior must be split into precise resolved and residual records;
- no record may describe a raising stub as “functionally equivalent”;
- no record may say a symbol is absent when it exists as a public raising stub or implemented helper;
- security-motivated residuals must state exactly what input fails, at what stage, and why fail-closed behavior is retained.

### Track 4 — Expected residual differences after successful implementation

Do not predeclare all of these as permanent; verify actual outcomes. However, the likely legitimate residual set is narrow:

#### Product-scope exclusions

- Trio/AnyIO backend support;
- Python 3.8/3.9 installation support;
- private HTTPX modules;
- HTTPX CLI emulation.

These remain outside the qualified surface and should not be counted as accidental parity failures.

#### Safe-Rust/native-boundary differences

- four-element socket option `(level, option, None, optlen)` if no safe cross-platform Rust representation is implemented;
- arbitrary Python `ssl.SSLContext` states that cannot be losslessly represented in rustls under Phase 01;
- exact Python `ssl.SSLObject` exposure from `network_stream.get_extra_info("ssl_object")` if the transport is rustls-based;
- `network_stream` raw operations on ordinary Hyper-owned pooled connections if Phase 04 concludes that exclusive ownership cannot be provided safely;
- HTTP/2 `stream_id` only if the pinned Hyper stack exposes no reliable public stream-ID source.

Each residual must have a focused test proving the exact reference/candidate divergence. Broad records such as “extensions unsupported” are no longer acceptable if several extension keys now work.

### Track 5 — Update the compatibility profile scope truthfully

Review `compat/httpx/0.28.1/profile.toml` and surrounding docs.

Preserve:

- reference version 0.28.1;
- Python 3.10+ EggFetch supported range unless separately changed;
- asyncio/Tokio backend declaration;
- module/public-private scope.

Update:

- qualification SHA/date;
- qualification status;
- counts/summary only from the final run;
- any capability notes that changed because of Phases 01-05.

Do not invent a “Stage D” label unless the repository’s compatibility policy already defines one and its criteria are satisfied. The conservative outcome may remain “Stage C qualified” with a materially smaller bounded-difference set. Claim wording matters more than stage inflation.

### Track 6 — Full pinned compatibility execution

Run the existing required command with fail-closed environment behavior:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Requirements:

- no unexpected skips;
- no xfails hiding mandatory behavior;
- capability-dependent skips are acceptable only where the repository’s established qualification policy explicitly allows them and the skipped capability is outside the claim;
- retain environment/package versions in the evidence record.

Run the full suite repeatedly according to the repository’s existing qualification convention so flaky lifecycle/network behavior is not accepted on one lucky pass.

Do not hard-code historical test counts in this plan. Record the counts produced by the final tree.

### Track 7 — Run focused phase-closure differential suites

Before treating the monolithic pass as sufficient, run grouped focused commands for:

- TLS/create_ssl_context/context translation;
- H2-only/plaintext prior knowledge/ALPN;
- target/SNI/trace/response extensions;
- network_stream Upgrade/CONNECT/lifecycle;
- proxy headers/proxy TLS separation.

Each group must have zero failures and zero unexplained skips.

This makes regressions diagnosable and prevents a large full-suite number from obscuring a missing targeted case.

### Track 8 — Required downstream compatibility runner

Use the repository’s existing isolated downstream runner and candidate/replacement wheel process.

Retain the required portfolio currently used by qualification unless repository policy has deliberately changed it. At the present baseline that includes:

- `respx`;
- `httpx-sse`;
- `httpx-auth`;
- `httpx-ws`.

The runner must exercise the built candidate wheel/shim in the controlled environment, not simply run packages against installed upstream HTTPX.

If Phase 04 changes WebSocket/upgrade behavior, pay particular attention to `httpx-ws` plus the new direct Upgrade/CONNECT differentials; a tiny downstream fixture is not sufficient by itself to prove network-stream parity.

Record artifact hashes as the existing qualification procedure requires.

### Track 9 — Documentation truth pass

Update only after executable qualification is stable:

- `README.md` HTTPX compatibility section;
- `docs/reference/compatibility.md`;
- migration documentation if user-visible compatibility changes materially;
- `AGENTS.md` bounded-difference summary;
- `.skills/python-bindings.md` if implementation boundaries changed;
- diagnostics/compatibility info exposed by the package;
- `plans/httpx-parity-correction-status.md` or the repository’s current authoritative HTTPX status record.

Remove stale statements such as:

- “ssl_context unsupported” if a safe subset is now supported;
- “Proxy(headers) rejected” if resolved;
- “H2-only unsupported” if resolved;
- broad extension omissions if narrowed.

Replace them with precise residual limitations where needed.

Historical plan/status files may retain historical wording when clearly labeled historical. Do not rewrite past evidence as if it always reflected the new implementation.

### Track 10 — FunctionAuth future-profile note

Add one concise note to the appropriate planning/compatibility roadmap stating:

- HTTPX master now exports `FunctionAuth`;
- the pinned 0.28.1 reference does not;
- EggFetch already has an internal callable-auth adapter;
- next stable HTTPX rebaseline should evaluate public export/signature/behavior then.

No runtime implementation is required for this track unless maintainers separately request it.

### Track 11 — Remote CI confirmation

After the final documentation/status commit is pushed, verify the existing routine CI run:

- checked out the expected pushed SHA;
- executed the unchanged `./scripts/check.sh` path;
- completed successfully.

Do not add or expand workflows for this purpose.

The authoritative full HTTPX qualification remains the local/manual extended evidence recorded against the executable SHA; routine CI confirms repository health, not every manual compatibility dimension.

## Final claim review

Before closing, answer explicitly in the status record:

1. What exact HTTPX version is targeted?
2. What Python versions/backends are included?
3. Which public HTTPX features remain intentionally outside scope?
4. Which public features remain bounded because of a safe-Rust/rustls/Hyper architectural boundary?
5. Are those residuals behavioral, API-shape-only, or installation/backend differences?
6. What exact executable SHA was qualified?
7. What commands and downstream artifacts constitute evidence?

The public claim should be no broader than those answers support.

## Non-goals

- upgrading the pinned reference from HTTPX 0.28.1;
- implementing unreleased `FunctionAuth` as part of 0.28.1 parity;
- adding Trio/AnyIO;
- restoring Python 3.8/3.9;
- adding new CI infrastructure;
- reopening already-closed unrelated HTTP semantics;
- declaring unrestricted parity if Phase 01/04 feasibility gates retain narrow differences.

## Acceptance criteria

This phase, and the overall remaining-parity program, is complete when:

1. A final executable SHA is frozen and all later evidence/documentation commits clearly reference it.
2. `./scripts/check.sh` passes on that executable tree.
3. Focused differential suites for Phases 01-05 pass against pinned HTTPX/httpcore behavior for every implemented feature.
4. The full required compatibility suite completes cleanly according to the repository’s existing repeated-run qualification convention.
5. API oracle reports zero unexplained differences, zero stale active entries, and zero resolved-in-active entries.
6. The required downstream compatibility portfolio passes using the built candidate wheel/shim, with artifact hashes recorded.
7. Every resolved gap has moved out of `allowed-differences.toml`; every remaining gap is precise, tested, and intentionally classified.
8. `create_ssl_context` ledger/documentation no longer contradicts runtime behavior.
9. H2-only, proxy headers, proxy TLS, and extension documentation reflects actual final behavior.
10. Trio/AnyIO, Python 3.8/3.9, and private modules remain explicit scope exclusions rather than accidental test omissions.
11. HTTPX master’s `FunctionAuth` export is recorded only as future-version drift, not folded into the 0.28.1 reference.
12. Existing remote routine CI passes on the pushed documentation/status descendant without any new workflow complexity.
13. The final compatibility status document states the exact supported surface and does not claim unrestricted HTTPX replacement unless no public behavioral residuals remain.

## Closure rule

If Phase 01 or Phase 04 leaves an irreducible architectural residual, that does not invalidate this program. Closure means the remaining difference is narrow, tested, security-justified, accurately documented, and no longer confused with a missing ordinary implementation task.

Conversely, a green aggregate test count is not closure if the allowlist still contains stale records, an extension is silently ignored, proxy metadata is dropped, or unsupported TLS state is silently approximated.
