# HTTPX 0.28.1 Final Closure 03 — Verification, Status, and Repository Hygiene

Status: ready for implementation handoff

Date: 2026-08-04

Depends on:

- `plans/httpx-parity-final-closure-roadmap.md`
- `plans/httpx-parity-final-closure-01-redirect-security-replay.md`
- `plans/httpx-parity-final-closure-02-raw-stream-lifecycle.md`

Audited baseline: `6ae10308b9db1e215eca19027d4ca9b7575900f6`

Pinned reference: `httpx==0.28.1`

Compatibility designation: `Stage C candidate`

## Objective

Close the final line of work with trustworthy, exact-SHA evidence and clean repository state after the redirect and raw-stream fixes land.

This plan does not add new product scope. It verifies that the implementation meets the pinned HTTPX 0.28.1 behavior already defined by the prior plans, preserves the repository’s lightweight validation model, corrects stale status claims, and closes superseded planning PRs.

## Files expected to change

Primary:

- direct differential test modules under `crates/eggfetch-python/tests/compat/`
- `crates/eggfetch-python/tests/compat/test_corrective_kernel.py`
- `plans/httpx-parity-correction-status.md`
- `README.md`, only if current claims require correction
- `docs/architecture/python-bindings.md`, only if current claims require correction
- `docs/reference/compatibility.md`, if present and affected

Repository operations:

- close PR `#16` with a supersession comment;
- close PR `#17` with a supersession comment;
- keep the final implementation PR authoritative until merged.

Do not add a plan registry or evidence system if the repository does not already require one.

# Track 0 — Establish the implementation boundary

## 0.1 Record the exact starting and implementation SHAs

Before final verification, update the status document to include:

- final-closure starting SHA `6ae10308b9db1e215eca19027d4ca9b7575900f6`;
- redirect implementation SHA;
- raw-stream implementation SHA;
- final combined implementation SHA;
- plan file paths;
- current status `verification in progress`.

Do not label the line complete at this point.

## 0.2 Separate code commits from documentation-only commits

The status file must distinguish:

- the SHA containing the final production and test changes;
- later documentation-only or PR-hygiene commits;
- the SHA checked out by CI.

A CI run on the implementation SHA may support a later documentation-only commit only when the relationship is stated explicitly and the documentation commit changes no executable behavior.

## 0.3 Avoid stale aggregate counts

Prefer exact command, SHA, and result over copied aggregate counts.

If test counts are recorded:

- derive them from the referenced run;
- refresh them after every test addition;
- distinguish passed, skipped, and xfailed counts;
- do not reuse counts from a prior implementation SHA.

### Track 0 acceptance criteria

- Every evidence claim is bound to an exact SHA.
- Code and documentation boundaries are explicit.
- Status remains open until validation completes.
- No stale test count is presented as current evidence.

# Track 1 — Complete direct pinned-reference coverage

## 1.1 Required redirect differential matrix

The full compatibility suite must directly compare HTTPX 0.28.1 and EggFetch for:

### Cookie regeneration

- same-origin redirect with client cookie;
- cross-origin redirect with client cookie;
- host-only cookie to sibling host;
- eligible domain cookie to subdomain;
- path-scoped cookie inside and outside path;
- secure cookie across scheme changes;
- initial explicit Cookie header, same-origin and cross-origin;
- request-local `cookies=`, same-origin and cross-origin;
- intermediate Set-Cookie eligible for next hop;
- intermediate Set-Cookie ineligible for next hop;
- duplicate cookie names with different paths where the existing jar supports them.

### Retained-body replay

- buffered POST through 307 and 308;
- buffered PUT and PATCH retained redirects;
- empty retained body;
- known reusable ByteStream;
- iterator and generator rejection before second dispatch;
- multipart immutable values;
- multipart file-backed value;
- mixed data/files multipart;
- 303 method rewrite and body-header removal;
- actual transport dispatch count.

## 1.2 Required raw-stream differential matrix

The full compatibility suite must directly compare:

### Sync

- `iter_raw()` with `chunk_size=None` and small positive sizes;
- source chunk splitting and coalescing;
- empty chunks and zero-length body;
- normal exhaustion;
- early break and explicit close;
- repeated raw iteration;
- read after partial raw consumption;
- incremental raw byte accounting;
- underlying close count;
- elapsed availability.

### Async

Mirror every sync case with `aiter_raw()` and `aclose()`.

### Raw versus decoded

- compressed native response;
- raw bytes differ from decoded bytes;
- raw count reflects transport bytes;
- decoded text and line behavior remains correct;
- no Python decompression path is exercised.

## 1.3 Normalize results narrowly

Create a small normalized result structure containing only public behavior relevant to the case, such as:

```python
{
    "chunks": [...],
    "cookie": "...",
    "dispatches": 1,
    "consumed": True,
    "closed": True,
    "downloaded": 12,
    "exception": "StreamConsumed",
}
```

Do not use broad snapshots that obscure the mismatched field.

## 1.4 Do not convert defects into allowed differences casually

Any newly discovered mismatch must be handled by one of:

- fix the candidate;
- document a narrow intentional difference with explicit user-visible consequence and review;
- record a bounded native blocker under the stop conditions.

Do not add an allowed-difference entry merely to make the oracle green.

### Track 1 acceptance criteria

- Every mandatory redirect and raw-stream case has a direct pinned-reference comparison.
- Sync and async public paths are both covered.
- Built-in native transport paths are exercised, not only MockTransport.
- Candidate-only assertions are not the sole proof for disputed behavior.
- Every remaining difference is either fixed, intentionally documented, or blocked with a precise reason.

# Track 2 — Keep Tier 1 compact and high-value

## 2.1 Extend the existing corrective kernel only

Add a small deterministic subset covering the highest-risk regressions:

- cross-origin Cookie containment;
- intermediate Set-Cookie regeneration;
- multipart/file-backed retained body cannot disappear;
- unreplayable retained body fails before second dispatch;
- raw iteration marks consumed and closes on exhaustion;
- partial raw iteration followed by explicit close;
- async raw path is distinct;
- raw byte accounting updates.

Use local adapters and fixtures. Do not install HTTPX in Tier 1.

## 2.2 Preserve one routine command

The existing routine path remains:

```sh
./scripts/check.sh
```

Do not add:

- another workflow;
- another CI job;
- a platform matrix;
- a scheduled run;
- an artifact upload;
- an evidence bundle;
- an automatic extended-suite run;
- release automation.

## 2.3 Keep runtime proportionate

Use parameterization and deterministic local fixtures.

Avoid:

- public network calls;
- wall-clock sleeps longer than necessary;
- randomized fuzzing;
- broad downstream portfolio execution;
- redundant copies of full-suite cases.

### Track 2 acceptance criteria

- Tier 1 catches all four remaining defect classes.
- Tier 1 remains one existing command and one existing CI job.
- No new external dependency is introduced solely for routine CI.
- Runtime remains proportionate to the current repository.
- Full differential breadth remains outside Tier 1.

# Track 3 — Run the complete verification sequence

Run in this order from the final implementation SHA.

## 3.1 Formatting and routine validation

```sh
./scripts/check.sh
```

Record:

- exact SHA;
- command;
- conclusion;
- test summary;
- skipped tests;
- environment versions relevant to reproduction.

## 3.2 Full pinned compatibility suite

Use the repository’s existing pinned-reference setup and run:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ \
  -q --strict-markers
```

Record:

- exact SHA;
- HTTPX version observed by the test environment;
- passed, failed, skipped, and xfailed counts;
- deprecation warnings separately from failures;
- exact failing node IDs if not clean.

Do not mark closure complete if mandatory cases are skipped.

## 3.3 API oracle

Run the existing manifest generation and comparison commands:

```sh
python scripts/generate_httpx_api_manifest.py \
  --package eggfetch.compat.httpx \
  --output /tmp/eggfetch-api.json

python scripts/compare_httpx_api_manifest.py \
  --reference compat/httpx/0.28.1/reference-api.json \
  --candidate /tmp/eggfetch-api.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml \
  --json \
  --output /tmp/api-result.json
```

Record:

- unexplained differences;
- stale allowed differences;
- resolved active differences;
- requires-resolution differences;
- any changed allowed-difference entry with justification.

## 3.4 Optional existing extended checks

Run only existing repository commands already used for extended validation. Do not create a new qualification framework.

If an optional toolchain is unavailable, record the skip truthfully. Do not present it as a pass.

### Track 3 acceptance criteria

- Routine validation passes on the final implementation SHA.
- Full compatibility passes with all mandatory cases executed.
- API oracle reports zero unexplained and zero stale differences.
- Any skips are explicitly listed and do not include mandatory closure cases.
- No new validation architecture is introduced.

# Track 4 — Verify visible CI against the pushed tree

## 4.1 Push the implementation before citing CI

The CI run must check out the final implementation tree or a later documentation-only tree whose relationship is explicitly stated.

Record:

- run ID;
- workflow name;
- job name;
- checked-out SHA from logs;
- conclusion;
- routine command executed.

## 4.2 Do not overstate CI coverage

The status file must state clearly:

- CI runs routine Tier 1 through `./scripts/check.sh`;
- full pinned compatibility and API oracle are manual/local extended evidence unless the existing workflow already runs them;
- CI success alone is not proof of the full compatibility matrix.

## 4.3 Handle documentation-only follow-up correctly

If the status file is updated after CI:

- identify the implementation SHA tested by CI;
- identify the later documentation-only SHA;
- state that no executable files changed between them;
- do not imply CI reran when it did not.

### Track 4 acceptance criteria

- A visible CI run passes.
- CI logs show the expected checked-out SHA.
- Status text describes CI scope accurately.
- Documentation-only boundaries are explicit.
- No earlier run is cited for later executable changes.

# Track 5 — Reconcile compatibility documentation

## 5.1 Update the status document

`plans/httpx-parity-correction-status.md` must contain:

- exact final implementation SHA;
- final pushed tree SHA;
- CI run ID and conclusion;
- routine test result;
- full compatibility result;
- API oracle result;
- any remaining intentional differences;
- explicit distinction between CI and manual evidence;
- final designation.

Remove or qualify stale historical counts.

## 5.2 Review README claims

Ensure README language does not claim:

- unrestricted drop-in HTTPX parity;
- compatibility with unpinned HTTPX versions;
- Trio/AnyIO support;
- unsupported transport features;
- completion beyond Stage C candidate.

Keep concise supported-surface language.

## 5.3 Review architecture/reference documents

Update only statements affected by the final fixes:

- redirect cookies are regenerated per destination;
- retained bodies replay or fail before redispatch;
- raw and decoded streaming paths are distinct;
- raw accounting and lifecycle match the pinned reference for the supported surface.

Do not expand documentation into a new evidence dossier.

## 5.4 Preserve historical records

Do not erase earlier roadmap, corrective-pass, or status history.

Mark superseded claims as historical where necessary.

### Track 5 acceptance criteria

- Current claims match the implemented surface.
- Stage C candidate remains the designation.
- Historical evidence is preserved but clearly labeled.
- No stale “pending” and “passed” contradiction remains.
- Documentation contains no unrestricted parity claim.

# Track 6 — Close stale planning PRs

## 6.1 Close PR #16

After confirming its plan content is represented on `main`:

- add a concise comment stating that the plan was incorporated and superseded by the later corrective/final closure work;
- close the PR;
- do not merge the stale branch.

## 6.2 Close PR #17

After confirming its plan content is represented on `main`:

- add a concise comment stating that the follow-up plan was incorporated and superseded by the final closure roadmap;
- close the PR;
- do not merge the stale branch.

## 6.3 Keep the final handoff authoritative

The final closure implementation PR should remain the authoritative execution record until merged.

Do not leave multiple open planning PRs that appear to compete for implementation.

### Track 6 acceptance criteria

- PR #16 is closed with a supersession comment.
- PR #17 is closed with a supersession comment.
- Neither obsolete branch is merged solely for cleanup.
- The final closure PR is the sole active handoff for this line of work.

# Track 7 — Final claim decision

The final claim may be recorded only when all prior tracks pass.

Required wording boundary:

**Stage C candidate — final deterministic closure complete for the documented HTTPX 0.28.1 asyncio-supported surface.**

Do not claim:

- complete HTTPX replacement for every ecosystem use;
- compatibility beyond HTTPX 0.28.1;
- support for excluded transports or concurrency backends;
- release readiness unless separate release checks have been performed.

### Track 7 acceptance criteria

- All mandatory differential cases pass.
- Routine CI passes.
- Full compatibility and API oracle pass.
- Documentation is exact-SHA-bound.
- Stale PRs are closed.
- No scope expansion or CI expansion occurred.
- Final designation remains bounded and truthful.

# Rejection criteria

Reject closure if:

- mandatory cases are skipped or xfailed;
- candidate-only tests are cited as the sole reference proof;
- cross-origin Cookie behavior lacks direct coverage;
- multipart silent-body-loss lacks direct coverage;
- native raw sync or async paths are not exercised;
- raw byte accounting is inferred from decoded length;
- stale test counts remain labeled current;
- CI is cited for a SHA it did not check out;
- the API oracle is not rerun after public-surface changes;
- PR #16 or #17 remains open without an explicit reason;
- new CI jobs, matrices, evidence formats, or release automation are added;
- compatibility is promoted beyond Stage C candidate.

# Final handoff report template

The implementer should provide:

```text
Starting SHA:
Redirect implementation SHA:
Raw-stream implementation SHA:
Final implementation SHA:
Final pushed tree SHA:

Routine validation:
- command:
- result:
- test summary:

Full HTTPX compatibility:
- pinned HTTPX version:
- command:
- result:
- mandatory skips:

API oracle:
- unexplained:
- stale allowed:
- requires resolution:

CI:
- run ID:
- checked-out SHA:
- conclusion:

Repository hygiene:
- PR #16 closed:
- PR #17 closed:

Remaining intentional differences:
Final designation:
```

This plan is complete only when every field is populated accurately or explicitly marked not applicable with a reason.
