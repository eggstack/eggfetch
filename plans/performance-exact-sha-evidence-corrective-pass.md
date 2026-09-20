# Performance Exact-SHA Evidence Record Corrective Pass

Planning baseline: `280d36e6976c3d842908be412c177bcedbc85030` (`main`, 2026-09-20)
Performance executable freeze: `18a1a4328110df014843a94e155fe678a08b1cb4`
Performance documentation closure: `5c09b468208b1bc5f63908e800e50a226ab10272`
Current documentation-only closure head at planning time: `280d36e6976c3d842908be412c177bcedbc85030`
Parent program: `plans/performance-optimization-no-api-regression-program.md`
Parent closure: `plans/post-performance-requalification-and-closure.md`
Prior qualified compatibility freeze: `37ab02b3873a4f0ce7018bd716e326bcf0595230`
Reference contracts: `httpx==0.28.1`, `httpx2==2.12.0`
Normative verification policy: `docs/verification-policy.md`

## Objective

Correct the evidence records for the completed performance optimization campaign without reopening executable work.

The performance implementation and local qualification are already complete on executable freeze `18a1a4328110df014843a94e155fe678a08b1cb4`. The final performance closure records Tier 1, extended, package, security, exact Rust 1.89.0 MSRV, native Python API/typing, both API oracles, full compatibility, FFI, lifecycle/soak, benchmark, and public-surface checks as passing on that freeze.

The remaining defect is documentary/evidentiary: the live compatibility profiles and parity ledger still identify the prior issue #24 freeze `37ab02b3873a4f0ce7018bd716e326bcf0595230` as the active exact-SHA Stage C binding, while the performance closure claims those records were renewed to `18a1a432`. The claim and the live records therefore disagree.

This corrective must make the repository truthful again by renewing the live evidence records to the already-qualified performance freeze, preserving the issue #24 evidence as historical, and correcting stale closure/index wording. It is not a new performance, compatibility, architecture, or release campaign.

## Confirmed planning-time state

At planning time:

- current `main` is `280d36e6976c3d842908be412c177bcedbc85030`;
- `18a1a4328110df014843a94e155fe678a08b1cb4` is the last executable/test change in the performance campaign;
- the two descendants `5c09b468...` and `280d36e6976c3d842908be412c177bcedbc85030` contain documentation/status material only;
- the diff `18a1a432..280d36e6976c3d842908be412c177bcedbc85030` contains only `AGENTS.md`, `README.md`, architecture documentation, plan files, and the plan index;
- no Rust/Python production source, tests, Cargo manifests/lockfile, build scripts, CI workflow, compatibility runner, or validation script changed after the executable freeze;
- current-head push CI is green: workflow `CI`, run 634, completed successfully on `280d36e6976c3d842908be412c177bcedbc85030`;
- `compat/httpx/0.28.1/profile.toml` still binds `qualification-sha` to `37ab02b3873a4f0ce7018bd716e326bcf0595230`;
- `compat/httpx2/2.12.0/profile.toml` still binds `qualification-sha` to the same issue #24 freeze;
- `plans/httpx-parity-correction-status.md` still presents the issue #24 qualification as the active recorded Stage C state;
- `plans/README.md` still says remote CI is pending even though current-head CI has succeeded;
- `plans/post-performance-requalification-and-closure.md` currently marks “Exact-SHA qualification records point to the final executable freeze” complete even though the live profiles/ledger do not yet do so.

The evidence-record mismatch is therefore real and narrow.

## Scope constraints

This pass is evidence/metadata correction only.

Do not:

- modify Rust or Python executable source;
- modify tests;
- modify Cargo manifests or `Cargo.lock`;
- modify build scripts;
- modify CI workflows;
- modify compatibility runner/oracle code;
- modify validation scripts;
- change the benchmark harness;
- change the HTTPX/HTTPX2 reference versions;
- add a compatibility exception or allowed difference;
- change public API, feature defaults, timeout semantics, streaming behavior, cookie behavior, decompression behavior, or C ABI;
- reopen connector/TLS optimization;
- promote HTTP/3 or Node;
- publish crates or wheels;
- create a new release or tag.

If implementation discovers that any executable/test/build/validation change is required, stop this corrective and create a separately attributable executable corrective with a new freeze.

## 1. Reconfirm the freeze-to-head descendant classification

Immediately before editing evidence records, compare:

```text
18a1a4328110df014843a94e155fe678a08b1cb4
..
<current main>
```

Classify every changed path.

Expected allowed categories:

1. documentation;
2. compatibility profile metadata;
3. parity/status ledger;
4. plan/index closure records.

Disallowed categories for this evidence-only pass:

- executable Rust/Python/JS source;
- tests;
- manifests/lockfiles;
- build scripts;
- workflows;
- compatibility/validation tooling.

If current `main` has gained a disallowed change after this plan was written, do not blindly bind the profiles to `18a1a432`. Determine whether that later change invalidates the performance freeze under the repository's current exact-SHA policy and, if necessary, select/requalify a new candidate in a separate corrective.

Acceptance:

- [ ] Final closure records the exact current-head SHA.
- [ ] `18a1a432..current-head` is confirmed documentation/evidence-only.
- [ ] No executable/test/build/validation change is silently excluded.

## 2. Reconcile the already-recorded qualification evidence

The normative `docs/verification-policy.md` requires Tier 1, extended, package, and the explicit security command for release-oriented qualification, but it does not require historical plans' three-run repetition rules. Completed plans are explicitly non-normative.

Use the evidence already recorded in `plans/post-performance-requalification-and-closure.md` when it is attributable to `18a1a432`:

- Tier 1: PASS;
- extended: PASS;
- package: PASS;
- security: PASS;
- exact Rust 1.89.0 MSRV: PASS;
- native Python manifest: 66 exports unchanged;
- HTTPX 0.28.1 API oracle: 71 allowed matches, zero unexplained/stale/resolved-active drift;
- HTTPX2 2.12.0 API oracle: 79 allowed matches, zero unexplained/stale/resolved-active drift;
- full pinned compatibility suites: PASS with no new exception;
- FFI: PASS;
- Node Rust tests: PASS; JS surface truthfully skipped because native artifact was absent;
- downstream: truthfully skipped because the qualification artifact manifest was absent;
- benchmark/resource/lifecycle/soak/lossless-merge controls: PASS as recorded.

Do not manufacture evidence that is not present. If an implementation reviewer cannot establish that a claimed result was run against `18a1a432`, rerun only that missing canonical gate on the unchanged freeze or explicitly record the evidence as unavailable. Do not rerun the entire campaign merely because the live profile files were not updated.

Acceptance:

- [ ] Every renewed profile/ledger claim is traceable to the performance closure evidence or a newly recorded exact-freeze rerun.
- [ ] No historical three-run requirement is treated as normative unless the current verification policy has changed to require it.
- [ ] Any newly rerun command is recorded with exact SHA and result.
- [ ] Optional Node/downstream skips remain skips, not passes.

## 3. Renew the HTTPX 0.28.1 compatibility profile

Update:

```text
compat/httpx/0.28.1/profile.toml
```

Required metadata result:

- `stage = "stage-c-qualified"` remains unchanged;
- `status = "qualified"` remains unchanged;
- `qualification-sha = "18a1a4328110df014843a94e155fe678a08b1cb4"`;
- `qualification-date = "2026-09-20"`;
- `previous-qualification-sha = "37ab02b3873a4f0ce7018bd716e326bcf0595230"`.

Add concise profile commentary explaining:

- `37ab02b...` remains valid historical issue #24 evidence;
- the performance campaign changed executable/test inputs after that freeze;
- the new freeze was qualified without public/API compatibility drift;
- the optimization work changed private ownership/backpressure/memory behavior only;
- HTTP/3 and Node remain experimental and are not promoted by this renewal.

Do not alter reference version, compatibility categories, allowed differences, feature extras, or Python-version declarations as part of this corrective.

Acceptance:

- [ ] HTTPX profile binds exactly to `18a1a432...`.
- [ ] Prior issue #24 SHA is retained as historical predecessor.
- [ ] No compatibility rule/exception changes.

## 4. Renew the HTTPX2 2.12.0 compatibility profile

Update:

```text
compat/httpx2/2.12.0/profile.toml
```

Use the same qualification SHA/date and previous qualification SHA as the HTTPX profile.

Preserve:

- `reference = "httpx2==2.12.0"`;
- existing Python-version contract;
- all feature-extra mappings;
- compatibility categories;
- experimental status boundaries.

Acceptance:

- [ ] HTTPX2 profile binds exactly to `18a1a432...`.
- [ ] Both profile files point to the same executable freeze and date.
- [ ] No HTTPX2 contract expansion or exception change.

## 5. Add a new active parity-ledger entry

Update:

```text
plans/httpx-parity-correction-status.md
```

Add a new top-level recorded state dated 2026-09-20 stating that Stage C evidence is renewed after the API-safe performance optimization campaign and is bound to `18a1a4328110df014843a94e155fe678a08b1cb4`.

The entry must:

- identify `37ab02b...` as the immediately preceding historical binding;
- explain why renewal was required: executable/test inputs changed during the performance campaign even though public API/compatibility behavior did not;
- summarize the exact qualification evidence from the performance closure;
- record the 71/79 API-oracle results with zero unexplained/stale/resolved-active drift;
- state no new allowed difference was introduced;
- state no Cargo/dependency/feature-default change occurred;
- state the native Python manifest remained at 66 exports;
- record FFI status and truthful Node/downstream skips;
- keep HTTP/3 and Node experimental;
- link the parent performance closure and this corrective plan.

Do not delete the issue #24 entry. It becomes the next historical record in the ledger.

Acceptance:

- [ ] First/current ledger entry is the performance freeze.
- [ ] Issue #24 entry remains intact below it as historical evidence.
- [ ] Ledger, both profiles, and performance closure all identify the same executable freeze.

## 6. Correct the performance closure record

Update:

```text
plans/post-performance-requalification-and-closure.md
```

Do not rewrite benchmark or validation history.

Add a short evidence-corrective addendum stating:

- the executable freeze remains `18a1a432...`;
- the original closure commit incorrectly claimed profile/ledger renewal before those files were updated;
- this corrective renews the live records without changing executable evidence;
- current-head CI run 634 is green on the documentation-only descendant;
- the exact-SHA completion checkbox is considered satisfied only after the profile/ledger changes from this corrective land.

The addendum should make the historical inconsistency visible rather than silently editing prose as if it never occurred.

Acceptance:

- [ ] Closure truthfully documents the evidence-record defect and correction.
- [ ] Benchmark/results data remain unchanged.
- [ ] No claim implies current documentation SHA is the executable qualification freeze.

## 7. Correct the plan index/current CI status

Update:

```text
plans/README.md
```

For the performance campaign entry:

- change status from “implementation and local requalification complete” to completed/closed after this corrective lands;
- replace “remote CI remains pending push” with the actual current-head push CI result;
- record executable freeze `18a1a432...`;
- record this evidence-corrective plan as the final closure correction;
- distinguish executable freeze from documentation/evidence descendant.

Add or retain a short active-corrective entry while implementation is in progress, then mark it complete in the same final evidence commit/descendant once the profiles and ledger are synchronized.

Acceptance:

- [ ] No stale “remote CI pending” wording remains.
- [ ] Index points readers to this corrective.
- [ ] Executable versus documentation/evidence SHAs are not conflated.

## 8. Validate the evidence-only patch

Because this corrective must not change executable behavior, validation should focus first on proving that fact.

Required checks:

```sh
git diff --name-only 18a1a4328110df014843a94e155fe678a08b1cb4..HEAD
git diff -- Cargo.toml Cargo.lock crates/ scripts/ .github/
```

Expected: no executable/test/build/validation changes after the freeze beyond source-documentation files already classified as non-executable. If a Rust source file appears, inspect whether it is rustdoc/comment-only rather than assuming.

Run the repository's cheapest canonical consistency validation appropriate to documentation/profile edits:

```sh
./scripts/check.sh
```

Also run the API-oracle/profile validation commands invoked by Tier 1/extended if profile metadata has dedicated validation not covered by Tier 1. Do not automatically run expensive benchmarks or package builds solely because TOML metadata changed.

If `./scripts/check.sh` or the profile/oracle checks expose a real mismatch, investigate before closure.

Acceptance:

- [ ] Evidence-only diff classification is clean.
- [ ] Tier 1 passes on the final evidence descendant.
- [ ] Both profile files parse and their qualification SHAs agree.
- [ ] API-oracle/profile consistency checks remain green.
- [ ] No executable requalification is falsely claimed from a docs-only command.

## 9. Record final pushed CI and descendant integrity

Push the corrective evidence commit(s), then record the routine GitHub CI result for the final closure head.

The final descendant must contain only:

- profile metadata;
- parity/status ledger;
- plan/closure/index documentation;
- other narrowly necessary documentation consistency edits.

If any executable/test/build/validation file changes during corrective implementation, the evidence-only closure rule is broken.

Acceptance:

- [ ] Final pushed routine CI is green.
- [ ] Final `18a1a432..closure-head` classification remains documentation/evidence-only.
- [ ] No executable SHA newer than `18a1a432` is mislabeled as the performance freeze.

## Expected files changed by implementation

Expected:

- `compat/httpx/0.28.1/profile.toml`;
- `compat/httpx2/2.12.0/profile.toml`;
- `plans/httpx-parity-correction-status.md`;
- `plans/post-performance-requalification-and-closure.md`;
- `plans/performance-exact-sha-evidence-corrective-pass.md`;
- `plans/README.md`.

Possibly, only if required for consistency:

- narrow compatibility README/status prose.

Unexpected and scope-expanding:

- `crates/**` executable source;
- tests;
- `Cargo.toml` / `Cargo.lock`;
- `scripts/**`;
- `.github/workflows/**`;
- benchmark harness changes.

## Final acceptance criteria

- [ ] `18a1a4328110df014843a94e155fe678a08b1cb4` remains the performance executable freeze.
- [ ] Freeze-to-final-head delta is evidence/documentation only.
- [ ] HTTPX 0.28.1 profile binds to `18a1a432...`.
- [ ] HTTPX2 2.12.0 profile binds to `18a1a432...`.
- [ ] Both profiles identify `37ab02b...` as the prior historical qualification.
- [ ] Live parity ledger's newest recorded state binds to `18a1a432...`.
- [ ] No new compatibility exception or allowed difference is introduced.
- [ ] Performance closure contains a transparent corrective addendum.
- [ ] Plan index no longer says remote CI is pending.
- [ ] Current remote CI success is recorded accurately.
- [ ] Node/downstream skips remain truthfully represented.
- [ ] HTTP/3 and Node remain experimental.
- [ ] Tier 1/profile consistency validation is green on the final evidence descendant.
- [ ] Final pushed CI is green.
- [ ] No executable, test, dependency, build, or validation change is made to “fix” an evidence-record mismatch.

## Closure rule

This plan is complete only when the live profile files, parity ledger, performance closure, and plan index all agree that `18a1a4328110df014843a94e155fe678a08b1cb4` is the qualified performance executable freeze and the final pushed descendant contains evidence/documentation changes only.

## Corrective closure record

The evidence-only correction updated both Stage C profiles, the live parity
ledger, the three canonical compatibility records, the performance closure,
and this plan index. The `18a1a432..HEAD` audit remains limited to
documentation, profile metadata, parity/status records, and plan/index files;
no executable, test, dependency, build, workflow, compatibility-runner, or
validation-script path changed. Local profile parsing, exact-SHA agreement,
and the repository Tier 1 gate passed on the final corrective working tree.
The implementation-time pre-corrective head was
`20493f10f41277add3cc9bd12d6898a49bf75c6e`; its complete freeze-to-head
path was documentation/evidence-only. Direct API-oracle comparisons on the
unchanged executable tree passed with 71 HTTPX matches and 79 HTTPX2 matches,
with no unexplained, stale, or resolved-active drift.

The final pushed routine-CI result is recorded in the closure/index records
after the corrective commit is pushed. HTTP/3 and Node remain experimental;
the absent Node artifact and downstream qualification manifest remain truthful
skips.

If later executable work lands before this corrective is implemented, do not mechanically reuse this plan's SHA assumptions. Re-evaluate the qualification candidate first.
