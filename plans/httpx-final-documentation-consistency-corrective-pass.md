# HTTPX 0.28.1 Final Documentation Consistency Corrective Pass

Status: ready for implementation handoff

Date: 2026-08-05

Audited baseline: `0ad2275c0fae50140d87c5b2f9b6da07d08dde3c`

Final executable SHA: `cf4680ac056bf241ca4f4e8fa0e076459bccc9e3`

Pinned reference: `httpx==0.28.1`

Compatibility designation: `Stage C candidate`

Authoritative CI evidence:

- workflow: `CI`
- run ID: `31034568903`
- job: `ci`
- job ID: `92403300331`
- checked-out SHA: `cf4680ac056bf241ca4f4e8fa0e076459bccc9e3`
- conclusion: `success`
- routine command: `./scripts/check.sh`

Related records:

- `plans/httpx-final-metadata-ci-hygiene-corrective-pass.md`
- `plans/httpx-native-compressed-raw-adapter-implementation-plan.md`
- `plans/httpx-native-compressed-raw-adapter-closure.md`
- `plans/httpx-parity-correction-status.md`
- planning PR #20, closed without merge after preservation
- planning PR #21, merged

## Objective

Correct the remaining documentation inconsistencies after the HTTPX 0.28.1 compressed raw-stream, wire-metadata, native-cancellation, pool-lease, planning-preservation, and CI closure landed successfully.

The executable work is accepted and must not be reopened by this pass.

This pass exists only because the current documentation contains four conflicting or imprecise statements:

1. the short adapter closure record still says that final corrective closure remains open;
2. the active status does not record the documentation-only evidence commit `0ad2275c0fae50140d87c5b2f9b6da07d08dde3c`;
3. the active routine-validation summary says that 532 Python behavior tests passed, while the authoritative CI log reports 532 collected, 492 passed, and 40 skipped;
4. the active status does not state the final bounded Stage C candidate designation explicitly, while the file ends with a historical superseded designation that says closure remained open.

The correction must make all active records agree without altering executable code, tests, CI, release policy, compatibility scope, historical evidence, or the accepted architecture.

## Accepted technical state — do not reopen

The following state is already accepted and is outside implementation scope for this pass:

- compressed streaming responses retain one encoded source until first body selection;
- raw selection exposes the encoded source;
- decoded selection reuses the existing core decompressor;
- raw and decoded selection are one-shot and mutually exclusive;
- no stream clone, replay, tee, duplicate request, or eager full-body buffer was introduced;
- read-timeout and pool-lease ownership remain attached to the selected source;
- core retains only the original wire `Content-Encoding` and `Content-Length` metadata needed by the compatibility facade;
- automatic core decompression continues to remove those two headers from core-visible decoded response headers;
- the HTTPX compatibility facade overlays only those two original wire values;
- sync and async raw and decoded gzip behavior matches HTTPX 0.28.1 in the covered native path;
- native async cancellation is exercised through the built-in Rust client path;
- a constrained follow-up request proves pool-lease release after cancellation and explicit close;
- the complete 820-line adapter implementation plan is preserved on `main` under a unique path;
- PR #20 is closed without merging its conflicting path;
- the existing single Ubuntu CI job passed on the final executable SHA;
- compatibility remains bounded to the documented HTTPX 0.28.1 asyncio-supported surface;
- crates.io release remains manual;
- routine CI remains one lightweight job invoking `./scripts/check.sh`.

No new technical investigation is required unless a documentation edit unexpectedly reveals that one of these statements is false. In that event, stop under the stop conditions rather than expanding this pass.

## Confirmed documentation defects

### A. Short closure record contradicts the active status

Current file:

`plans/httpx-native-compressed-raw-adapter-closure.md`

Current status wording says, in substance:

- adapter implementation complete;
- final corrective closure remains open.

That was accurate before the metadata/cancellation/CI corrective pass completed, but it is now stale. The active status records the final executable SHA, direct metadata parity, native cancellation and lease proof, planning preservation, and exact CI identifiers.

The short closure record must therefore state that the adapter and its final corrective closure are complete for the documented bounded surface.

### B. The closure record still describes metadata as out of scope

The closure record currently says that core header removal remains bounded and that the closure is about body-byte selection only.

That statement is obsolete after `cf4680ac056bf241ca4f4e8fa0e076459bccc9e3`, which added a narrow two-field wire-metadata snapshot and a compatibility-only overlay.

The corrected record must distinguish:

- core decoded-header policy remains unchanged;
- compatibility-visible wire metadata now matches HTTPX for `Content-Encoding` and `Content-Length`;
- no generalized metadata framework was added.

Do not rewrite the adapter architecture section beyond what is needed to make this distinction accurate.

### C. The active status omits the documentation-only evidence SHA

Current file:

`plans/httpx-parity-correction-status.md`

The active section correctly records:

- corrective baseline SHA;
- adapter executable baseline SHA;
- metadata/cancellation final executable SHA;
- preserved implementation-plan SHA;
- CI run and job identifiers.

It does not record the documentation-only evidence commit:

`0ad2275c0fae50140d87c5b2f9b6da07d08dde3c`

Add this SHA as the prior final evidence-binding commit and explicitly state that it contains documentation/status changes only and is a descendant of the CI-tested executable SHA.

Do not imply that CI run `31034568903` checked out `0ad2275c...`; it checked out `cf4680ac...`.

### D. The current routine test count is imprecise

The authoritative CI log for run `31034568903`, job `92403300331`, reports:

```text
492 passed, 40 skipped
```

That is 532 collected behavior tests, not 532 passed tests.

The active current validation section must say exactly:

- Python behavior tests: 532 collected;
- 492 passed;
- 40 skipped.

The skips must not be described as failures, unexpected omissions, or a regression unless the existing skip audit says so. The routine CI still passed.

Do not rewrite historical sections whose counts refer to earlier exact SHAs unless their wording is also demonstrably false from their own recorded evidence.

### E. The active final designation is missing

The active section declares the work complete but does not provide the explicit bounded final designation required by the corrective plan.

The file later contains a clearly labeled historical superseded designation stating that closure remained open. Historical text may remain, but the active section must state the current designation before historical records begin.

Required current wording:

**Stage C candidate — deterministic compressed raw-stream body and metadata closure complete for the documented HTTPX 0.28.1 asyncio-supported surface.**

Do not promote the project beyond Stage C candidate.

## Scope firewall

### In scope

Only the following documentation corrections are in scope:

1. update the status line and metadata description in `plans/httpx-native-compressed-raw-adapter-closure.md`;
2. add the prior documentation-only evidence SHA to the active section of `plans/httpx-parity-correction-status.md`;
3. correct the active routine Python behavior count to `532 collected, 492 passed, 40 skipped`;
4. add the exact active Stage C candidate designation;
5. ensure historical superseded records remain clearly labeled as historical;
6. bind the documentation correction to an exact correction commit and existing CI run without self-referential SHA requirements;
7. allow the existing CI workflow to run naturally on the documentation-only correction branch or merged tree;
8. record any new CI result only if the repository workflow actually runs it.

### Out of scope

Do not modify:

- Rust source;
- Python source;
- tests or fixtures;
- compatibility manifests or allowlists;
- `Cargo.toml`, `Cargo.lock`, `pyproject.toml`, or dependency files;
- CI workflows or scripts;
- release workflows or release policy;
- README or architecture documentation unless a direct contradiction is discovered during implementation;
- the preserved 820-line implementation plan;
- the prior corrective plan’s acceptance criteria;
- HTTPX version pinning;
- transport, pool, timeout, decompression, streaming, or response ownership code;
- compatibility designation beyond inserting the required existing Stage C wording.

Do not create:

- another roadmap;
- another evidence format;
- a registry entry;
- a dashboard;
- a release checklist;
- a new CI job;
- a new validation script;
- generated artifacts.

## Track 0 — Freeze the executable state

### 0.1 Confirm the baseline

Before editing, confirm that `main` is at or is an understood documentation-only descendant of:

`0ad2275c0fae50140d87c5b2f9b6da07d08dde3c`

If executable files changed after that SHA, stop and audit those changes separately. Do not absorb new executable work into this documentation-only pass.

### 0.2 Capture the allowed file set

Expected modified files:

- `plans/httpx-native-compressed-raw-adapter-closure.md`
- `plans/httpx-parity-correction-status.md`

A third Markdown file may be changed only to correct a direct contradiction discovered while editing. Document the reason in the commit message or PR description.

No non-Markdown file may change.

### 0.3 Preserve existing evidence

Treat the following as authoritative and immutable for this pass:

- executable SHA `cf4680ac056bf241ca4f4e8fa0e076459bccc9e3`;
- CI run `31034568903`;
- CI job `92403300331`;
- checked-out SHA `cf4680ac056bf241ca4f4e8fa0e076459bccc9e3`;
- CI conclusion `success`;
- routine Python behavior result `492 passed, 40 skipped`;
- compatibility smoke result `117 passed`;
- full pinned result `1384 passed, 0 failed, 0 skipped, 0 xfailed`;
- API oracle result: 121 allowed matches and zero stale, unexplained, resolved-in-active, or requires-resolution entries.

Do not invent a new result or change a count to make sections look uniform.

### Track 0 acceptance criteria

- Baseline ancestry is confirmed.
- No executable change is included.
- The allowed file set is documented.
- Existing evidence remains exact-SHA-bound.
- No new technical claim is introduced without existing evidence.

## Track 1 — Correct the short adapter closure record

### 1.1 Replace the stale open status

Change the opening status from an open-final-corrective statement to wording equivalent to:

```text
Status: adapter implementation and final corrective closure complete for the documented HTTPX 0.28.1 asyncio-supported surface.
```

Keep the wording bounded. Do not claim unrestricted replacement or release readiness.

### 1.2 Describe the final metadata state accurately

Replace the obsolete “body-byte selection only” statement with a concise description of the final state:

- core still removes visible `Content-Encoding` and `Content-Length` during automatic decoded-response processing;
- core retains a narrow read-only snapshot of the original wire values;
- the HTTPX compatibility facade restores only those two values;
- raw and decoded body selection remains one-shot and unchanged;
- no Python decompression, generalized metadata framework, or transport redesign exists.

### 1.3 Link to authoritative evidence

Keep links to:

- the preserved detailed implementation plan;
- the final corrective plan;
- the authoritative status file.

The short closure record should not duplicate every test count or CI identifier. The status file remains the evidence authority.

### 1.4 Preserve historical structure

Do not replace the preserved implementation plan or collapse the adapter architecture explanation. This pass corrects stale status and metadata wording only.

### Track 1 acceptance criteria

- The closure record no longer says final corrective closure remains open.
- It states the bounded surface explicitly.
- It describes the compatibility-only two-header overlay accurately.
- It does not claim that core-visible decoded headers retain the wire headers.
- It does not claim generalized HTTPX parity.
- It links to the authoritative status and preserved plan.

## Track 2 — Correct the active status evidence

### 2.1 Add the documentation-only evidence SHA

In the active current corrective section, add:

```text
Prior documentation-only evidence-binding SHA:
`0ad2275c0fae50140d87c5b2f9b6da07d08dde3c`
```

State that it is a documentation-only descendant of the final executable SHA.

Do not call it the CI checked-out SHA.

### 2.2 Correct routine Python behavior wording

Replace wording equivalent to:

```text
532 Python behavior tests passed
```

with exact wording equivalent to:

```text
Python behavior tests: 532 collected; 492 passed and 40 skipped.
```

Preserve the successful routine conclusion.

Do not alter the 117 compatibility smoke count.

### 2.3 Add the current designation

Add a dedicated active subsection before historical records:

```markdown
## Current designation

**Stage C candidate — deterministic compressed raw-stream body and metadata closure complete for the documented HTTPX 0.28.1 asyncio-supported surface.**
```

Equivalent placement is acceptable if it remains unambiguously part of the active current status.

### 2.4 Keep historical designations historical

The existing historical superseded designation may remain verbatim because it records an earlier state. It must remain under a clearly historical heading and must not be the only designation visible in the file.

Do not rewrite history to pretend the earlier open state never existed.

### 2.5 Clarify executable versus documentation evidence

The active status must distinguish:

- final executable SHA: `cf4680ac...`;
- CI checked-out SHA: `cf4680ac...`;
- prior documentation-only evidence SHA: `0ad2275c...`;
- documentation consistency correction SHA created by this pass, recorded in a later evidence-only commit if needed.

Avoid self-referential requirements. A commit cannot reliably contain its own final SHA.

Recommended sequence:

1. commit the substantive documentation corrections;
2. allow existing CI to run on that correction commit if the workflow triggers;
3. create one evidence-only follow-up commit that records the correction commit SHA and its CI identifiers;
4. treat the evidence-only commit as a documentation-only descendant; do not require it to contain its own SHA.

If the workflow does not run for the correction commit because of repository configuration, retain the existing executable CI evidence and state that the new changes are documentation-only. Do not create a new workflow or weaken policy.

### Track 2 acceptance criteria

- `0ad2275c...` is recorded as documentation-only evidence, not executable or CI checkout evidence.
- Current routine count says 532 collected, 492 passed, 40 skipped.
- The 117 smoke result remains unchanged.
- The exact current Stage C candidate wording is present.
- Historical open wording remains clearly historical.
- Executable, CI, and documentation SHAs are not conflated.

## Track 3 — Validate documentation consistency

### 3.1 Review the final diff

The final substantive correction diff must contain only Markdown changes.

Expected paths:

```text
plans/httpx-native-compressed-raw-adapter-closure.md
plans/httpx-parity-correction-status.md
```

Reject any unexpected executable, test, workflow, dependency, or generated-file change.

### 3.2 Perform targeted text checks

Verify that active records satisfy all of the following:

- no active section says closure remains open;
- the short closure record says complete;
- current designation appears exactly once in the active status;
- `0ad2275c...` is present and labeled documentation-only;
- `cf4680ac...` remains the executable and CI-tested SHA;
- run `31034568903` and job `92403300331` remain unchanged;
- current routine result contains `492 passed` and `40 skipped`;
- current routine result does not say `532 passed`;
- compatibility smoke remains `117 passed`;
- full pinned result remains `1384 passed`;
- historical superseded sections remain explicitly historical.

Use ordinary review or existing shell utilities. Do not add a validation script to the repository.

### 3.3 Do not rerun unnecessary extended validation

Because this pass changes documentation only:

- do not rerun the full pinned HTTPX compatibility suite solely for this pass;
- do not rerun the API oracle solely for this pass;
- do not rebuild the extension solely for this pass;
- do not add documentation-only checks to routine CI.

The existing exact-SHA executable evidence remains authoritative.

If any executable file changes unexpectedly, this exemption no longer applies. Stop and separate the work.

### 3.4 Existing CI behavior

The existing workflow may run automatically on the documentation correction commit or merge. Allow it to run unchanged.

Required properties if it runs:

- one existing Ubuntu `ci` job;
- unchanged `./scripts/check.sh` command;
- no new matrix or job;
- successful conclusion before the evidence-only follow-up records it.

A documentation-only CI run is useful repository evidence but does not replace or alter the authoritative executable run `31034568903`.

### Track 3 acceptance criteria

- Final substantive diff is Markdown-only.
- Active records contain no contradiction.
- Current counts match the authoritative CI log.
- Current and historical designations are clearly separated.
- No unnecessary full-suite or oracle rerun is required.
- Existing CI remains unchanged.

## Track 4 — Bind the correction without self-reference

### 4.1 Substantive correction commit

Recommended commit:

```text
docs: correct final HTTPX closure records
```

This commit should contain the two documentation corrections and no product changes.

### 4.2 Evidence-only follow-up

After the correction commit is pushed and any existing CI run completes, create one evidence-only commit, for example:

```text
docs: bind HTTPX documentation consistency evidence
```

Update the active status with:

- substantive documentation correction SHA;
- CI run ID and job ID for that correction SHA if a run occurred;
- explicit statement that this evidence commit is documentation-only and does not change executable evidence.

Do not require the evidence-only commit to name its own SHA.

### 4.3 Preserve authoritative executable evidence

Do not replace:

- executable SHA `cf4680ac...`;
- executable CI run `31034568903`;
- executable CI job `92403300331`.

Any new documentation-only run must be listed separately.

### 4.4 PR and branch hygiene

The planning PR for this corrective pass should merge normally.

The implementation may land through the repository’s usual process. Do not reopen PR #20 or PR #21.

Do not delete historical planning files.

### Track 4 acceptance criteria

- Substantive correction commit is documentation-only.
- Evidence binding does not use a self-referential SHA requirement.
- Existing executable evidence remains authoritative.
- New documentation-only CI evidence is separated clearly if present.
- No old PR is reopened.

## File-by-file implementation guidance

### `plans/httpx-native-compressed-raw-adapter-closure.md`

Required edits:

- change opening status from open to complete;
- retain the adapter executable SHA;
- retain links to preserved implementation plan and final corrective plan;
- update metadata paragraph to describe the narrow compatibility overlay;
- remove or replace wording that says the closure concerns body bytes only;
- keep out-of-scope limitations and Stage C boundary.

Do not add full test tables or CI logs to this short record.

### `plans/httpx-parity-correction-status.md`

Required edits in the active section:

- add prior documentation-only evidence SHA `0ad2275c...`;
- correct behavior test count to `532 collected; 492 passed; 40 skipped`;
- add explicit current designation;
- preserve final executable SHA and CI identifiers;
- preserve full pinned and API-oracle results;
- preserve historical sections;
- later record the substantive documentation correction SHA and any documentation-only CI run in an evidence follow-up.

Do not rewrite historical exact-SHA evidence unless necessary to keep headings unambiguous.

## Global acceptance criteria

This documentation corrective line is complete only when all of the following are true:

1. `main` is an understood descendant of `0ad2275c0fae50140d87c5b2f9b6da07d08dde3c`.
2. No executable, test, workflow, dependency, manifest, or release file changes.
3. `plans/httpx-native-compressed-raw-adapter-closure.md` no longer says final corrective closure remains open.
4. The short closure record states completion only for the documented HTTPX 0.28.1 asyncio-supported surface.
5. The short closure record accurately describes core header removal and the compatibility-only two-header overlay.
6. The short closure record does not say metadata parity remains out of scope.
7. The preserved 820-line implementation plan remains unchanged and linked.
8. `plans/httpx-parity-correction-status.md` records `0ad2275c...` as a documentation-only evidence-binding SHA.
9. `0ad2275c...` is not mislabeled as the executable or CI checked-out SHA.
10. `cf4680ac...` remains the final executable SHA.
11. CI run `31034568903` and job `92403300331` remain bound to `cf4680ac...`.
12. Current routine Python behavior wording says 532 collected, 492 passed, and 40 skipped.
13. Current routine wording does not claim 532 behavior tests passed.
14. The 117-test compatibility smoke result remains unchanged.
15. The 1384-test full pinned result remains unchanged.
16. API-oracle counts remain unchanged.
17. The active current designation is exactly or materially equivalent to the required Stage C candidate wording.
18. Historical superseded open wording remains clearly historical.
19. The substantive documentation correction commit SHA is recorded in a later evidence-only update.
20. Any documentation-only CI run is recorded separately from executable CI evidence.
21. No self-referential final-SHA requirement is introduced.
22. Existing CI remains one Ubuntu job invoking `./scripts/check.sh`.
23. No new CI workflow, job, matrix, artifact, scheduled trigger, or release action is added.
24. No full pinned suite or API oracle is rerun solely to validate Markdown changes.
25. No compatibility promotion beyond Stage C candidate occurs.
26. No unrestricted HTTPX replacement or release-readiness claim is introduced.

## Rejection criteria

Reject the implementation if any of the following occurs:

- Rust or Python source changes;
- tests or fixtures change;
- workflow or validation scripts change;
- dependency or lockfile changes;
- the preserved implementation plan is edited or shortened;
- the active closure still says open;
- the short record still says body-byte selection only while omitting metadata parity;
- core is described as retaining wire headers in its visible decoded header map;
- `Content-Length` is described as decoded-body length;
- `0ad2275c...` is described as the CI checked-out executable SHA;
- `cf4680ac...` is replaced as the final executable SHA;
- the current status continues to say 532 behavior tests passed;
- skipped tests are hidden or converted into passed tests in prose;
- the active current Stage C designation is absent;
- historical evidence is deleted to avoid contradiction rather than labeled correctly;
- a new roadmap, evidence schema, registry, or dashboard is added;
- full compatibility or API-oracle execution is added to routine CI;
- a new CI job, matrix, runner, schedule, or release workflow is added;
- crates.io release automation is added;
- HTTPX version pinning changes;
- compatibility is promoted beyond Stage C candidate;
- unrestricted HTTPX equivalence or release readiness is claimed;
- an evidence commit attempts to record its own final SHA through unstable amend loops.

## Stop conditions

Stop and document the blocker instead of expanding scope when any of the following is true:

1. `main` contains executable changes after `0ad2275c...` that have not been audited.
2. The authoritative CI logs no longer support the recorded `492 passed, 40 skipped` result.
3. The recorded run or job identifiers resolve to a different checked-out SHA.
4. Correcting the documentation requires changing product behavior.
5. A direct contradiction is found outside the two expected files and cannot be corrected without broad documentation churn.
6. Repository policy prevents the documentation correction from landing without modifying CI.
7. The preserved implementation plan is missing or materially different from PR #20.
8. The current compatibility designation has been changed by a separate approved line of work.

For any stop condition, record:

- exact file and statement;
- current SHA;
- conflicting evidence;
- smallest separate follow-up required;
- which acceptance criteria remain open.

Do not mark this documentation line complete under a stop condition.

## Suggested commit decomposition

1. `docs: correct final HTTPX closure records`
   - update the short closure record;
   - add prior documentation-only SHA;
   - correct current behavior counts;
   - add explicit active designation;
   - preserve historical records.

2. `docs: bind HTTPX documentation consistency evidence`
   - record the first correction commit SHA;
   - record its existing CI run/job if one occurred;
   - state that this commit is an evidence-only descendant;
   - retain executable evidence unchanged.

Do not split this into multiple plans or product commits.

## Handoff checklist

Before editing:

- [ ] Confirm `main` ancestry from `0ad2275c...`.
- [ ] Confirm no executable changes have landed.
- [ ] Read the two target files in full.
- [ ] Confirm CI run `31034568903`, job `92403300331`, and checked-out SHA `cf4680ac...`.
- [ ] Confirm the CI log says `492 passed, 40 skipped`.
- [ ] Confirm the preserved 820-line implementation plan remains present.

During the substantive correction:

- [ ] Change the short closure status to complete.
- [ ] Describe the two-header compatibility overlay accurately.
- [ ] Remove stale body-byte-only scope wording.
- [ ] Add `0ad2275c...` as documentation-only evidence.
- [ ] Correct the routine behavior count.
- [ ] Add the active Stage C candidate designation.
- [ ] Preserve historical superseded sections.
- [ ] Avoid all non-Markdown changes.

Before the evidence follow-up:

- [ ] Review the diff for only the two expected files.
- [ ] Verify no active “closure remains open” statement remains.
- [ ] Verify `532 passed` is absent from the active current section.
- [ ] Verify `492 passed` and `40 skipped` are present.
- [ ] Verify executable and documentation SHAs are distinct.
- [ ] Allow the existing CI job to complete if triggered.

Before final closure:

- [ ] Record the substantive correction SHA.
- [ ] Record any documentation-only CI run/job separately.
- [ ] Keep run `31034568903` as executable evidence.
- [ ] Ensure current designation is explicit.
- [ ] Ensure short and authoritative records agree.
- [ ] Ensure no CI or release expansion occurred.
- [ ] Ensure compatibility remains Stage C candidate.

## Final closure statement

This documentation line is closed only when the short adapter record and authoritative HTTPX correction status agree that deterministic compressed raw-stream body, wire-metadata, native cancellation, lease-release, planning-preservation, and CI closure are complete for the documented HTTPX 0.28.1 asyncio-supported surface; current routine counts accurately report 532 collected, 492 passed, and 40 skipped; executable and documentation evidence SHAs are distinguished; the active Stage C candidate designation is explicit; and no executable, CI, release, or compatibility-scope change is introduced.
