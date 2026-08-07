# HTTPX 0.28.1 Parity Completion — Phase 6: Differential Qualification and Closure

Status: ready for implementation handoff

Date: 2026-08-07

Roadmap: `plans/httpx-parity-completion-roadmap.md`

Prerequisites:

- Phase 1 rebaseline complete;
- Phase 2 object/configuration contract work complete or bounded blockers recorded;
- Phase 3 signature/type work complete or bounded blockers recorded;
- Phase 4 advanced direct transport work complete or bounded blockers recorded;
- Phase 5 SOCKS work complete or bounded blockers recorded.

Pinned reference: `httpx==0.28.1`

Compatibility designation entering this phase: `Stage C candidate`

## Objective

Perform the final exact-SHA-bound qualification of the HTTPX parity-completion program, reconcile the active/resolved difference ledgers and current inventory, validate representative downstream substitution, and publish a truthful final compatibility designation without expanding CI or release machinery.

This phase is primarily verification and evidence closure. Runtime fixes are permitted only when a final differential test exposes a narrow defect directly caused by Phases 2–5. Broad new implementation work must be split into a corrective plan rather than absorbed here.

## Closure principles

### 1. Test the executable SHA actually being claimed

Final runtime evidence must be bound to the exact executable tree containing all Phase 2–5 implementation changes.

A later documentation-only descendant may record that evidence, but must never be described as the SHA actually executed if it was not.

### 2. The allowlist is not the success criterion by itself

A clean API oracle proves every difference is either matched or explicitly allowed. Final closure also requires direct behavioral tests for the high-risk/public changes implemented in this roadmap.

### 3. Remaining differences must be truly intentional or deferred

Do not preserve a difference merely to reach closure. If a cheap public mismatch remains and no stop condition applies, closure is not complete.

### 4. Keep verification proportional

Routine CI stays the existing lightweight job invoking `./scripts/check.sh`. Final qualification may run the full pinned suite, API oracle, downstream runner, and focused local transport fixtures manually/locally. Do not add permanent CI infrastructure for this closure.

## Track 0 — Freeze the final executable candidate

### 0.1 Identify the candidate SHA

Record:

- starting SHA for Phase 6;
- final executable candidate SHA after any narrow final fixes;
- commits from the roadmap baseline through the candidate grouped by phase.

Do not start documentation evidence binding until the executable candidate is frozen.

### 0.2 Inspect the executable diff scope

Confirm the cumulative implementation remained within roadmap boundaries.

Expected categories:

- HTTPX compatibility Python facade/tests;
- narrow native binding plumbing;
- narrow core connector/proxy changes for Phase 4/5;
- compatibility manifests/ledgers/docs;
- focused dependency/feature changes if justified by Phase 4/5.

Flag unexpected broad refactors, unrelated protocol changes, CI/release modifications, or dependency growth before qualification.

### 0.3 Record retained blockers

If a Phase 2–5 stop condition was invoked, list:

- exact symbol/capability;
- reference behavior;
- EggFetch retained behavior;
- differential test demonstrating the boundary;
- why the blocker is acceptable within the final designation.

A blocker must not disappear into generic "intentional difference" wording.

### Track 0 acceptance criteria

- One exact executable candidate SHA is identified.
- Cumulative diff scope is reviewed.
- Any retained blockers are explicit before testing begins.

## Track 1 — Focused regression kernel for the new work

Run focused tests that directly cover every implementation group introduced by this roadmap.

At minimum the final kernel must cover:

### Phase 2 contracts

- Headers Mapping/MutableMapping behavior and inherited methods;
- QueryParams Mapping behavior;
- stream exception hierarchy/constructors;
- `NetRCAuth(file=...)`;
- `URL.raw`;
- `codes` enum/type behavior;
- corrected Timeout/Limits/Proxy/default-encoding semantics.

### Phase 3 signatures/types

- top-level helper argument validation;
- Client/AsyncClient constructor/method argument validation;
- low-level transport signatures;
- transport base-class relationships;
- Sync/Async/ByteStream public relationships;
- custom request stream behavior;
- existing raw/decoded response lifecycle regression cases.

### Phase 4 transports

- local-address end-to-end bind proof;
- socket-option end-to-end proof;
- UDS end-to-end proof on supported Unix targets;
- pool isolation;
- timeout/cancellation/resource release;
- ordinary TCP/TLS regression.

### Phase 5 SOCKS

- HTTP through SOCKS;
- HTTPS through SOCKS;
- authentication if reference-supported;
- DNS/address-type behavior;
- proxy errors/malformed replies;
- timeout/cancellation;
- pool route isolation/reuse;
- `NO_PROXY`, `trust_env`, and environment-variable behavior assigned by Phase 1;
- credential redaction.

### Track 1 acceptance criteria

- Every roadmap implementation group has direct final-SHA coverage.
- High-risk transport functionality is proven end to end, not by constructor/API tests alone.
- Existing redirect/cookie/raw-stream/cancellation closure tests remain passing.

## Track 2 — Run routine repository validation

Run the unchanged routine command:

```sh
./scripts/check.sh
```

Record the exact result on the executable candidate SHA.

Do not change `scripts/check.sh` merely to make the new extended transport cases part of routine CI unless those tests naturally belong to existing fast/local suites and do not materially increase routine burden.

If the routine test suite has legitimately changed its test counts, report the exact new collected/passed/skipped counts rather than copying historical values.

### Track 2 acceptance criteria

- `./scripts/check.sh` passes on the executable candidate SHA.
- No routine validation failure is waived without a corrective pass.
- Routine CI architecture remains unchanged.

## Track 3 — Full pinned HTTPX compatibility suite

Run exactly against `httpx==0.28.1`:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ \
  -q --strict-markers
```

Record:

- Python version;
- installed HTTPX version;
- exact executable candidate SHA;
- pass/fail/skip/xfail counts;
- warnings that materially affect interpretation.

Expected closure criterion is zero failures. Skips are allowed only when they correspond to explicit platform/environment gates and are audited rather than silently accepted.

For UDS/IPv6/platform-specific tests, report why each skip occurred and whether the corresponding capability is claimed on the current platform.

### Track 3 acceptance criteria

- Full pinned suite has zero failures.
- Skips/xfails are explicitly understood.
- No test is disabled merely to reach closure.

## Track 4 — API oracle and difference-ledger reconciliation

### 4.1 Generate final candidate manifest

```sh
python scripts/generate_httpx_api_manifest.py \
  --package eggfetch.compat.httpx \
  --output /tmp/eggfetch-api.json
```

### 4.2 Run final comparison

```sh
python scripts/compare_httpx_api_manifest.py \
  --reference compat/httpx/0.28.1/reference-api.json \
  --candidate /tmp/eggfetch-api.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml \
  --json \
  --output /tmp/api-result.json
```

### 4.3 Required oracle state

Final oracle must report:

- zero unexplained differences;
- zero stale active allowlist entries;
- zero resolved-in-active entries;
- zero requires-resolution entries.

The raw active-allowed count need not be zero. Remaining entries are acceptable only if Phase 1 classified them as intentional/deferred and final implementation has not invalidated that rationale.

### 4.4 Audit every remaining active group

Review each remaining group, not only the count.

Expected retained categories may include:

- Trio/AnyIO backend differences;
- Python 3.8/3.9 interpreter support;
- private HTTPX modules excluded from contract;
- reviewed EggFetch-only additive members;
- narrowly documented platform/subcase blockers that satisfied a roadmap stop condition.

Anything else requires explicit review before closure.

### 4.5 Preserve resolved history

Ensure newly resolved records are represented in `resolved-differences.toml` according to existing convention and are absent from the active allowlist.

Do not erase historical rationale/evidence.

### Track 4 acceptance criteria

- Oracle state is clean.
- Every remaining active difference is reviewed and truthful.
- Newly closed differences are recorded as resolved.
- No public mismatch is hidden through an allowlist-only edit.

## Track 5 — Refresh the upstream compatibility inventory

### 5.1 Make the current inventory current

Refresh `compat/httpx/0.28.1/upstream-test-inventory.md` or the current canonical inventory established in Phase 1.

Record:

- final executable candidate SHA;
- pinned reference version;
- update date;
- current supported/partial/deferred areas.

### 5.2 Remove stale gap descriptions

Ensure the inventory no longer describes already-closed work as missing, including earlier historical statements about redirects, multipart, environment handling, mounts/transports, raw streams, or cancellation.

Add the new achieved state for:

- Python object/signature contracts;
- advanced direct transport options;
- SOCKS;
- any explicitly retained limitation.

### 5.3 Preserve historical inventories appropriately

If the project intentionally keeps old snapshots, label them as historical and link to the current exact-SHA inventory/status. Do not silently rewrite evidence that was meant to describe an earlier state.

### Track 5 acceptance criteria

- There is one unambiguous current inventory/status authority.
- It is exact-SHA-bound.
- It agrees with the final active ledger and docs.

## Track 6 — Downstream substitution validation

Use the repository's existing isolated downstream compatibility runner rather than invoking fixture directories directly.

Expected command:

```sh
python scripts/run_downstream_compat.py
```

Use whatever arguments/environment the existing runner documents for the full intended portfolio; do not invent a second downstream harness.

### 6.1 Interpret results behaviorally

For each downstream failure, classify:

- real HTTPX public-contract incompatibility;
- private/internal HTTPX dependency outside the contract;
- environment/dependency problem unrelated to EggFetch;
- unsupported Trio/Python-version/backend requirement;
- newly introduced regression.

Do not automatically fix packages that depend on private HTTPX internals if the project contract excludes them.

### 6.2 Require correction for in-scope regressions

If a downstream package fails because a valid HTTPX 0.28.1 public call in the supported asyncio/Python version surface still differs, closure is blocked until corrected or a roadmap stop condition is explicitly invoked.

### Track 6 acceptance criteria

- Existing downstream runner completes and results are recorded.
- In-scope public failures are fixed or explicitly blocked.
- Private/out-of-scope failures are labeled accurately rather than counted as parity success/failure without context.

## Track 7 — Documentation truthfulness and final compatibility wording

Audit at minimum:

- `README.md`;
- `docs/reference/compatibility.md`;
- `compat/httpx/0.28.1/profile.toml`;
- current parity status record;
- current inventory.

### 7.1 Update achieved feature rows

If Phases 4–5 succeeded, update rows/limitations for:

- UDS;
- `local_address`;
- `socket_options`;
- SOCKS;
- `ALL_PROXY`/environment semantics if implemented.

State platform or feature-build limitations precisely.

### 7.2 Preserve real deferred boundaries

Continue to state clearly that:

- Trio/AnyIO is not supported unless separately implemented;
- Python 3.8/3.9 is not supported unless separately implemented;
- private HTTPX modules are outside the compatibility contract;
- the reference target is HTTPX 0.28.1.

### 7.3 Final designation decision

Do not automatically change `Stage C candidate` to Stage D.

After evidence is complete, make an explicit decision:

**Option A — retain Stage C candidate**

Use when meaningful intentionally deferred public differences remain, especially concurrency-backend/runtime support.

Suggested wording:

> Stage C candidate — high-fidelity HTTPX 0.28.1 compatibility for the documented Python ≥3.10 asyncio-supported surface, including the qualified low-level transport features documented here.

**Option B — promote only through the repository's existing stage policy**

If the project already defines Stage D criteria and all of them are demonstrably satisfied, record the exact evidence and make the promotion as a separate explicit decision. Do not invent new Stage D criteria in this phase.

### 7.4 Avoid “drop-in” without qualification

A short "drop-in" label must not appear in isolation if Trio, interpreter versions, private modules, or other relevant public capabilities remain outside scope.

Use bounded wording adjacent to any replacement claim.

### Track 7 acceptance criteria

- All active docs agree with each other and the final ledger.
- No unsupported feature is described as supported.
- No HTTPX public feature is incorrectly described as nonexistent.
- Final stage/designation is explicit and evidence-backed.

## Track 8 — Exact-SHA evidence binding and repository hygiene

### 8.1 Create/update the authoritative status record

Record:

- roadmap starting baseline;
- Phase 1–5 implementation SHAs or ranges;
- final executable SHA;
- routine validation result;
- focused final kernel result;
- full pinned suite result;
- API oracle result;
- downstream result;
- relevant platform details for UDS/socket tests;
- final active allowed-difference count;
- final designation.

### 8.2 Documentation-only descendant discipline

If the final status commit occurs after the executable candidate:

- label it documentation-only;
- state that executable evidence belongs to the prior exact SHA;
- do not claim a CI run checked out the docs commit if it did not.

If normal CI runs on the docs commit, record that as documentation-descendant CI evidence separately.

### 8.3 Reconcile planning branches/PRs created by implementation

If implementation agents created planning or superseded PRs during Phases 1–5:

- ensure required plan content exists on `main`;
- close obsolete/conflicting planning PRs rather than merging duplicate stale files;
- leave concise supersession comments where useful.

Do not fabricate PR cleanup work if no such PRs exist.

### 8.4 No release automation changes

Crates.io release remains manual under the established project policy. PyPI/wheel release behavior remains whatever the repo already defines outside this roadmap.

Do not add release jobs to mark HTTPX closure.

### Track 8 acceptance criteria

- Status is exact-SHA-bound.
- Executable and documentation evidence are not conflated.
- Superseded planning state is clean if applicable.
- Release and CI architecture remain unchanged.

## Final program acceptance criteria

The HTTPX parity-completion roadmap is complete only when all of the following are true:

1. Phase 1 produced a finite classification/ownership inventory for the active differences.
2. Every `must-close` difference from that inventory is either resolved or represented by an explicit stop-condition blocker.
3. Phase 2 targeted public object/configuration contracts match HTTPX 0.28.1.
4. Phase 3 targeted signatures/type relationships match at both inspection and runtime argument-validation levels.
5. Phase 4 claimed advanced direct-transport options have real end-to-end native proof.
6. Phase 5 claimed SOCKS behavior has real end-to-end proxy proof and correct TLS/DNS/auth semantics.
7. Existing redirect/cookie/auth/streaming/compression/cancellation behavior remains passing.
8. `./scripts/check.sh` passes on the final executable candidate.
9. Full pinned compatibility suite has zero failures.
10. API oracle has zero unexplained/stale/requires-resolution/resolved-in-active entries.
11. Remaining active differences are genuinely intentional/deferred and reviewed individually.
12. Downstream runner has no unresolved in-scope public-contract regressions.
13. Current inventory and user-facing documentation agree with the evidence.
14. Final designation is explicit and bounded.
15. No unnecessary CI/release architecture, second networking stack, or out-of-scope Trio/AnyIO/Python-version work was added.

## Rejection criteria

Reject final closure if:

- final tests are run on a different executable SHA than the one claimed;
- a failure is converted into an allowed difference merely to obtain a green oracle;
- UDS/local-address/socket-options/SOCKS are marked supported from constructor-only tests;
- platform skips are not audited;
- downstream in-scope failures are ignored;
- stale inventory text remains the apparent current status;
- README says unrestricted drop-in while the detailed contract remains bounded;
- docs claim HTTPX lacks a public feature that it actually exposes;
- CI/release infrastructure is expanded solely for closure evidence;
- an unrelated broad refactor is folded into Phase 6.

## Stop/corrective-pass conditions

Create a narrow corrective plan instead of expanding Phase 6 if:

- final qualification reveals a new behavioral subsystem not covered by Phases 2–5;
- a transport defect requires redesign of the pool/connector architecture;
- SOCKS or UDS support is only partially functional in a way that changes the product scope decision;
- downstream validation reveals a substantial class of public HTTPX usage not represented in the active contract;
- new executable changes after the frozen candidate become large enough that previous final tests are no longer representative.

Small localized fixes with immediate focused/full reruns may remain in Phase 6, but document the new executable SHA and rerun every affected evidence gate.

## Suggested commit decomposition

1. `test: qualify final HTTPX parity completion candidate`
2. `fix: close final bounded HTTPX differential defects` only if needed
3. `docs: reconcile final HTTPX difference ledgers and inventory`
4. `docs: record exact HTTPX parity completion evidence`

## Final handoff checklist

The closing report must include:

- roadmap starting baseline SHA;
- Phase 1–5 relevant SHAs;
- final executable SHA;
- documentation/evidence SHA if different;
- focused final regression command/result;
- `./scripts/check.sh` result;
- full pinned suite command/result;
- API oracle command/result;
- active allowed-difference count before roadmap and after closure;
- exact remaining intentional/deferred difference groups;
- UDS/local-address/socket-option support matrix and test platform;
- SOCKS schemes/auth/DNS/environment support matrix;
- downstream runner command/result and any excluded/private failures;
- CI run/check-out SHA if CI ran;
- exact user-facing compatibility designation;
- confirmation that Trio/AnyIO and Python 3.8/3.9 remain deferred unless separately justified;
- confirmation that routine CI and release policy were not expanded.
