# HTTPX 0.28.1 Final Metadata, Native Cancellation, and CI Hygiene Corrective Pass

Status: ready for implementation handoff

Date: 2026-08-05

Audited baseline: `52f540483322a47db11ebff5e17079d21370473f`

Executable adapter baseline: `1aa5cb986bbdb03b92588eb1c7b7ad7070d9ffe7`

Pinned reference: `httpx==0.28.1`

Compatibility designation: `Stage C candidate`

Related planning and status:

- `plans/httpx-native-compressed-raw-adapter-closure.md`
- `plans/httpx-parity-raw-stream-final-corrective-closure.md`
- `plans/httpx-parity-correction-status.md`
- planning PR `#20`

## Objective

Close the remaining verification and repository-hygiene defects after the native compressed raw-stream adapter implementation without reopening the completed adapter architecture or expanding project scope.

The adapter itself is accepted provisionally:

- compressed streaming responses retain one encoded source until first consumption;
- raw selection returns the encoded source;
- decoded selection uses the existing core decompressor;
- raw and decoded selection are mutually exclusive;
- the source is not cloned, replayed, teed, or eagerly buffered;
- read-timeout and pool-lease ownership remain attached to the selected stream;
- the Python binding selects raw or decoded mode explicitly.

This corrective pass is limited to four unresolved closure obligations:

1. compressed-response header metadata must be matched or explicitly governed against HTTPX 0.28.1;
2. native async cancellation must prove release of the selected compressed stream and its pool lease;
3. the detailed adapter plan stranded in PR #20 must be preserved on `main` and the obsolete PR closed cleanly;
4. the final closure record must be bound to an actual existing CI run on the exact final executable tree or a documented-only descendant.

This is a closure pass, not another compatibility roadmap.

## Confirmed current defects

### A. Compressed body-byte parity is tested, but header parity is not

Current native gzip differential tests compare:

- encoded raw bytes;
- decoded bytes;
- raw byte counts;
- raw chunk adaptation;
- one-shot selection.

They do not compare the streamed response values for:

- `Content-Encoding`;
- `Content-Length`.

EggFetch core currently removes both headers during automatic decompression setup before the Python response copies its visible headers. HTTPX 0.28.1 retains response headers and uses `Content-Encoding` when constructing decoded iteration.

The current status calls this a deliberately bounded policy, but no direct compatibility test or stable governance record defines the exact difference.

### B. Cancellation coverage does not reach the native compressed adapter

The direct cancellation oracle currently uses constructed compatibility `Response` instances backed by custom async streams. That is useful wrapper coverage, but it does not prove that cancellation through the built-in Rust `AsyncClient`:

- drops the selected encoded stream;
- releases the core pool lease;
- leaves the response terminally consumed/closed;
- permits a subsequent request when connection concurrency is constrained.

### C. The detailed implementation plan is not preserved on `main`

PR #20 contains the detailed adapter implementation plan but remains open and unmerged. `main` independently contains a short closure record at the same path.

Consequences:

- the detailed acceptance, rejection, stop, and verification criteria are absent from `main`;
- PR #20 now conflicts semantically and by path with the closure record;
- leaving the PR open makes the repository appear to have unfinished planning work after implementation.

### D. Closure is not CI-bound

The status records substantial local evidence for executable SHA `1aa5cb986bbdb03b92588eb1c7b7ad7070d9ffe7`, but it does not record:

- the CI checked-out SHA;
- workflow run ID;
- job ID;
- conclusion;
- whether the run used only the existing single Tier 1 job;
- the relationship between the executable SHA and documentation-only head `52f540483322a47db11ebff5e17079d21370473f`.

The adapter plan explicitly required those identifiers before marking closure complete.

## Scope firewall

### In scope

- a direct HTTPX 0.28.1 oracle for compressed streamed response headers;
- the smallest compatibility-only metadata preservation required by that oracle;
- one native async compressed cancellation and lease-release integration test;
- preservation of the detailed PR #20 plan under a non-conflicting path on `main`;
- closure of PR #20 after preservation is verified;
- exact final-tree validation using existing commands;
- exact GitHub Actions run/job evidence from the existing CI workflow;
- correction of current status wording and SHA bindings.

### Out of scope

- redesigning `Response`, transport, pool, timeout, compression, TLS, proxy, retry, redirect, HTTP/2, or HTTP/3 architecture;
- offering two independently readable response bodies;
- stream cloning, replay, teeing, duplicate requests, or eager body buffering;
- Python decompression or re-encoding;
- replacing `async-compression`;
- changing compression format support;
- HTTPX version rebasing;
- Trio or AnyIO support;
- changing the Stage C candidate designation;
- new workflows, jobs, matrices, scheduled runs, runners, evidence systems, or dashboards;
- adding the full pinned suite to routine CI;
- release automation;
- reopening redirect, cookie, timeout, query, or request-body replay work;
- broad cleanup unrelated to this closure.

## Required invariants

1. The encoded response stream remains single-owner and one-shot.
2. Raw and decoded selection remain mutually exclusive.
3. Existing core decompression remains authoritative for decoded operations.
4. No stream clone, tee, replay, duplicate request, or eager full-body buffer is added.
5. Core default decoded-header behavior must not change accidentally.
6. Any compatibility-only header preservation must be narrow and read-only.
7. Read timeout remains applied once to the selected source.
8. Pool lease releases once on EOF, error, explicit close, iterator drop, or cancellation.
9. Existing uncompressed and buffered behavior remains unchanged.
10. Routine CI remains one Ubuntu `ci` job running `./scripts/check.sh`.
11. crates.io release remains manual.
12. The existing manually dispatched PyPI wheel process remains unchanged.
13. Status remains reopened until CI evidence is recorded.
14. The final designation remains `Stage C candidate`.

# Track 0 — Reopen status and preserve the evidence boundary

## 0.1 Reopen the current completion wording first

Before executable changes, update the active section of `plans/httpx-parity-correction-status.md` to state:

`Native compressed raw body selection is implemented; final metadata, native cancellation, CI evidence, and planning-hygiene closure remain open.`

Do not delete the existing local evidence. Mark it provisional and preserve its exact executable SHA.

## 0.2 Record the corrective baseline

The status must record:

- corrective baseline: `52f540483322a47db11ebff5e17079d21370473f`;
- adapter executable baseline: `1aa5cb986bbdb03b92588eb1c7b7ad7070d9ffe7`;
- this corrective plan path;
- PR #20 as an open hygiene item.

## 0.3 Keep executable and documentation boundaries distinct

Every subsequent status update must distinguish:

- executable commits;
- test-only commits if separate;
- documentation-only commits;
- the exact tree checked out by CI.

### Track 0 acceptance criteria

- The active status is not marked complete while any required item remains open.
- Existing successful local evidence is retained and correctly labeled.
- Baseline and executable SHAs are exact, not abbreviated.
- PR #20 is listed as an unresolved hygiene item until its contents are preserved.

# Track 1 — Resolve compressed response header metadata

## 1.1 Add a direct pinned header oracle

Extend the existing native gzip differential fixture. For separate sync and async streamed requests, record from HTTPX 0.28.1 and EggFetch:

- `response.headers.get("content-encoding")`;
- `response.headers.get("content-length")`;
- encoded raw body length;
- decoded body length;
- whether header values change before and after raw or decoded consumption.

Use separate one-shot responses for raw and decoded observations.

The oracle must execute both implementations and compare normalized public values. Do not copy expected strings from documentation into a candidate-only test.

## 1.2 Preferred resolution: preserve wire metadata only for the compatibility facade

If the oracle confirms HTTPX retains the wire values, implement the smallest metadata snapshot that allows `eggfetch.compat.httpx` to present those values without changing default Rust-core response-header behavior.

Preferred shape:

1. before core strips decoded headers, retain only the original `Content-Encoding` and `Content-Length` values as optional response metadata;
2. expose them through the narrowest read-only core or Python-binding surface needed by the compatibility facade;
3. let the compatibility facade overlay those two values in its visible response headers;
4. continue using the retained encoding parameter already owned by the deferred decoder for decoded body selection;
5. do not use the visible compatibility header as the internal decompression source of truth.

Acceptable implementation forms include:

- two narrowly named optional metadata fields on core `Response` with read-only accessors;
- one small internal wire-content metadata struct;
- hidden Python-native getters consumed only by `eggfetch.compat.httpx`.

Choose the form with the fewest new public types and no generalized extension map.

## 1.3 Preserve default core behavior

The ordinary Rust-core response must continue to expose its documented decoded-header policy unless a direct existing core test proves that policy is already wrong.

Do not globally stop stripping headers merely to satisfy the Python compatibility facade.

Do not change direct non-compat Python response headers without first identifying and updating their explicit contract and tests. Prefer overlaying metadata in the compatibility wrapper only.

## 1.4 Fallback stop condition

If preserving the two wire values requires a broad response metadata redesign or changes default Rust-core behavior, stop before implementation and document:

- the exact ownership boundary preventing a narrow snapshot;
- why a two-field snapshot is insufficient;
- the public surface that would have to change;
- the direct HTTPX fixture demonstrating the mismatch.

Do not silently call the mismatch closed through prose alone.

A stage-bounded behavioral difference may be retained only if the repository already has a compatible behavior-governance mechanism that does not create a stale API-oracle entry. Do not add a behavior-only row to the API allowlist if the API comparator cannot match it.

## 1.5 Header tests

Required tests:

- sync native gzip streamed raw headers match HTTPX;
- async native gzip streamed raw headers match HTTPX;
- sync native gzip decoded-read headers match HTTPX;
- async native gzip decoded-read headers match HTTPX;
- wire `Content-Length` equals encoded body length where HTTPX reports it;
- decoded body length remains distinct from wire `Content-Length`;
- uncompressed response headers are unchanged;
- direct core decoded-header policy remains unchanged;
- compatibility header overlay does not affect internal decoder selection.

### Track 1 acceptance criteria

- Header behavior is determined by an executable HTTPX 0.28.1 oracle.
- Compatibility-visible `Content-Encoding` matches HTTPX for streamed gzip responses.
- Compatibility-visible `Content-Length` matches HTTPX for streamed gzip responses.
- Raw and decoded requests each retain the expected header metadata.
- Core default decoded-header behavior remains unchanged.
- Only the two required wire metadata values are retained.
- No generalized metadata framework is introduced.
- No header value is inferred from decoded body length.
- No API allowlist entry is added that the oracle treats as stale.

# Track 2 — Prove native async cancellation and lease release

## 2.1 Build one deterministic native compressed fixture

Add one local HTTP/1.1 fixture that:

1. sends a deterministic gzip response using chunked transfer or another existing fixture mechanism that exposes incremental body progress;
2. sends at least one encoded body segment;
3. signals the test that the first segment was sent;
4. blocks before sending the remainder using a synchronization primitive;
5. can be released by the test without arbitrary sleeps.

Use `threading.Event`, an async event bridged through the fixture, or an existing deterministic synchronization utility.

Do not use external network access.

## 2.2 Exercise the built-in async client

Use EggFetch `AsyncClient`, not a constructed compatibility `Response`.

Recommended sequence:

1. create a client constrained to one available connection or one per-origin permit;
2. open the compressed streamed response;
3. create `aiter_raw()` and read the first encoded output;
4. start the next `anext()` while the server is blocked;
5. cancel that pending task;
6. assert `CancelledError` propagates;
7. call `iterator.aclose()` and `response.aclose()`;
8. release the server fixture;
9. issue a second request through the same constrained client;
10. require the second request to complete within a short deterministic test timeout.

The second request is the public integration proof that the selected stream and pool lease were released.

## 2.3 Compare public state with HTTPX

Run the same public sequence against HTTPX 0.28.1 where practical and compare:

- first raw output was encoded;
- pending read cancellation result;
- `is_stream_consumed`;
- `is_closed` after explicit close;
- second iterator rejection;
- subsequent request success.

Do not require private pool internals or exact transport object identities to match.

## 2.4 Avoid brittle timing

The test may use a bounded timeout solely as a deadlock guard. It must not use fixed sleeps as its primary synchronization or success mechanism.

Keep wall-clock runtime small enough for the existing test tiers.

## 2.5 Core-level fallback

If native HTTP framing makes a deterministic pending-read fixture impossible in the current harness, add:

- one core test proving a dropped/cancelled selected `EncodedStreaming` stream releases its `PoolGuardArc` exactly once;
- one native async test proving cancellation reaches terminal public state;
- a documented reason the two observations are split.

Do not create a new server framework.

### Track 2 acceptance criteria

- The test uses the built-in EggFetch async client and compressed native response path.
- Cancellation occurs while a raw source read is genuinely pending.
- `CancelledError` is preserved.
- The response becomes terminal after explicit close.
- A second body selection fails.
- A subsequent constrained-client request succeeds, proving lease release.
- No arbitrary sleep is required for correctness.
- The test completes within existing routine time limits or remains in the existing extended suite as directed by current tier policy.
- No pool or timeout architecture changes are introduced solely for testing.

# Track 3 — Preserve the detailed adapter plan and close PR #20

## 3.1 Preserve the original plan under a unique path

Copy the complete detailed plan from PR #20 head `4a802ee7cd9eb8531ca127b538018b567742794d` into `main` under a non-conflicting archival implementation-plan path, preferably:

`plans/httpx-native-compressed-raw-adapter-implementation-plan.md`

The current short closure record may remain at:

`plans/httpx-native-compressed-raw-adapter-closure.md`

Do not overwrite the closure record with the old planning status.

## 3.2 Preserve substantive content

The archived implementation plan must retain:

- objective and scope firewall;
- confirmed blocker;
- preferred implementation shape;
- Tracks 0 through 5;
- global acceptance criteria;
- rejection criteria;
- stop conditions;
- commit decomposition;
- handoff checklist;
- closure statement.

A short summary is not an adequate replacement.

## 3.3 Link the closure record to the archived plan

Update the short closure record to identify:

- the archived implementation plan path;
- original planning PR #20;
- adapter executable SHA;
- final corrective status path.

## 3.4 Close PR #20 only after preservation lands

After the archived plan exists on `main`:

1. verify its substantive content against PR #20;
2. add a concise PR comment identifying the preservation commit and new path;
3. close PR #20 without merging its conflicting file;
4. do not delete the branch until normal repository policy permits it.

### Track 3 acceptance criteria

- The complete detailed PR #20 plan is available on `main` under a unique path.
- The current closure record remains intact.
- The closure record links to the detailed implementation plan.
- No material acceptance, rejection, or stop criteria are lost.
- PR #20 contains a closing comment pointing to the preservation commit.
- PR #20 is closed and no longer appears as active unfinished work.
- No force merge or conflict-prone overwrite is used.

# Track 4 — Execute bounded verification

## 4.1 Focused tests

Run from a clean environment with the extension rebuilt:

```sh
python -m pip install -r compat/httpx/0.28.1/requirements.txt
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 \
  maturin develop -m crates/eggfetch-python/Cargo.toml
python -m pytest \
  crates/eggfetch-python/tests/compat/test_raw_stream_httpx_differential.py \
  crates/eggfetch-python/tests/compat/test_raw_stream_lifecycle.py \
  crates/eggfetch-python/tests/compat/test_response.py \
  crates/eggfetch-python/tests/compat/test_response_metadata_parity.py \
  -q --strict-markers
```

Record exact passed, failed, skipped, and xfailed counts.

## 4.2 Core-focused tests

Run existing focused package/test filters for:

- response body selection;
- response decompression;
- pipeline timeout/lease handling;
- pool release behavior;
- Python streaming bindings.

Do not add a new test runner.

## 4.3 Routine validation

Run exactly:

```sh
./scripts/check.sh
```

The command must complete successfully. Do not split or weaken it to obtain a passing result.

## 4.4 Full pinned compatibility

Run exactly:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Record current counts. Do not reuse `1379 passed` unless produced by the final executable SHA.

## 4.5 API oracle

Run the existing manifest generation and comparison commands.

Required result:

- zero unexplained differences;
- zero stale allowed entries;
- zero unresolved differences;
- no new unmatched behavior-only allowlist entry.

## 4.6 Feature validation

Re-run only feature configurations touched by the metadata implementation. At minimum:

- gzip;
- default feature set.

Do not repeat unrelated broad feature matrices unless ordinary repository commands already require them.

### Track 4 acceptance criteria

- Focused metadata and native cancellation tests pass.
- Core-focused tests pass.
- `./scripts/check.sh` passes without skips introduced by this pass.
- Full pinned compatibility completes with exact current counts.
- API oracle is clean.
- No validation architecture expansion occurs.
- Test runtime remains appropriate for the existing project scale.

# Track 5 — Bind the final tree to existing CI

## 5.1 Preserve current CI shape

Do not modify `.github/workflows/ci.yml` unless a deterministic defect in the existing workflow itself prevents execution.

Expected shape remains:

- trigger on `main` push and pull request;
- one Ubuntu `ci` job;
- one `Run validation` step invoking `./scripts/check.sh`.

## 5.2 Push the final executable tree

After executable and test changes are complete, push them before the final status-only commit.

Record:

- final executable SHA;
- plan-preservation SHA if separate;
- any documentation-only descendant.

## 5.3 Capture CI identifiers

For the successful existing CI run, record:

- checked-out commit SHA;
- workflow name;
- workflow run ID;
- job name;
- job ID;
- attempt number if retried;
- conclusion;
- start and completion timestamps;
- confirmation that `./scripts/check.sh` ran.

If CI runs on a documentation-only descendant of the executable tree, state that relationship explicitly.

## 5.4 Failure handling

If the run fails:

- inspect the failing step;
- correct product/test code only if the failure is real;
- retry through the existing workflow;
- do not weaken CI;
- do not mark closure complete from local evidence alone.

### Track 5 acceptance criteria

- The existing single CI job passes.
- The CI checked-out SHA contains the final executable changes or is an explicitly documented-only descendant.
- Workflow run and job identifiers are recorded.
- CI invokes the unchanged Tier 1 command.
- No new job, matrix, runner, scheduled trigger, or release action is added.

# Track 6 — Final truthful status and documentation

## 6.1 Update the status after CI only

The final active section of `plans/httpx-parity-correction-status.md` must record:

- corrective baseline SHA;
- adapter executable baseline SHA;
- metadata/cancellation final executable SHA;
- archived implementation-plan preservation SHA;
- documentation-only final SHA if applicable;
- focused test command and exact result;
- routine command and exact result;
- full pinned command and exact result;
- API-oracle result;
- exact CI checked-out SHA;
- workflow run ID;
- job ID;
- CI conclusion;
- PR #20 closure state.

## 6.2 Final wording

Allowed wording:

**Stage C candidate — deterministic compressed raw-stream body and metadata closure complete for the documented HTTPX 0.28.1 asyncio-supported surface.**

Do not claim:

- unrestricted HTTPX replacement;
- complete transport/backend parity;
- Trio or AnyIO parity;
- compatibility with newer HTTPX versions;
- release readiness solely from this closure.

## 6.3 Documentation scope

Update only directly affected files, likely:

- `plans/httpx-parity-correction-status.md`;
- `plans/httpx-native-compressed-raw-adapter-closure.md`;
- the archived detailed implementation plan;
- `README.md` only if its bounded header statement changes;
- the directly affected Python/core architecture document.

Do not add another roadmap, registry, dashboard, qualification document, or release checklist.

### Track 6 acceptance criteria

- Status is marked complete only after successful final-tree CI.
- Every validation claim is exact-SHA-bound.
- Header behavior is described accurately.
- Native cancellation and lease proof is named explicitly.
- PR #20 is recorded as closed.
- Historical and current evidence remain clearly separated.
- Compatibility remains Stage C candidate.

# Global acceptance criteria

This corrective line is complete only when all of the following are true:

1. `main` is descended from audited baseline `52f540483322a47db11ebff5e17079d21370473f` through understood changes only.
2. Existing one-shot raw-or-decoded body selection remains intact.
3. No stream clone, replay, tee, duplicate request, or eager full-body buffer is introduced.
4. Existing core decompression remains the only decoded-body implementation.
5. HTTPX 0.28.1 compressed streamed header behavior is measured directly.
6. Compatibility-visible `Content-Encoding` matches the pinned reference.
7. Compatibility-visible `Content-Length` matches the pinned reference.
8. Wire content length is not replaced by decoded body length.
9. Core default decoded-header behavior remains unchanged.
10. Sync and async gzip raw body parity remains passing.
11. Sync and async gzip decoded body parity remains passing.
12. Native async cancellation reaches the built-in Rust client path.
13. Cancellation while a compressed raw read is pending propagates correctly.
14. Explicit close after cancellation is terminal.
15. A subsequent constrained-client request succeeds, proving lease release.
16. A second body selection after cancellation/close fails.
17. The complete detailed PR #20 implementation plan is preserved on `main` under a unique path.
18. The short closure record remains intact and links to that plan.
19. PR #20 is closed with a comment pointing to the preservation commit.
20. Focused tests pass with exact current counts.
21. Core-focused tests pass.
22. `./scripts/check.sh` completes successfully.
23. Full pinned compatibility completes successfully.
24. API oracle reports zero unexplained, stale, and unresolved differences.
25. Existing single-job CI passes on the final executable tree or a documented-only descendant.
26. CI run ID, job ID, checked-out SHA, and conclusion are recorded.
27. No CI, release, transport, pool, or compatibility-scope expansion occurs.
28. Final status remains bounded to Stage C candidate.

# Rejection criteria

Reject the pass if any of the following occurs:

- header parity is asserted without executing HTTPX 0.28.1;
- visible `Content-Length` is derived from decoded bytes;
- default Rust-core header behavior is changed only to simplify the compatibility facade;
- a generalized response extension framework is introduced for two header values;
- Python performs decompression or re-encoding;
- the body is cloned, replayed, teed, or buffered to retain metadata;
- native cancellation is tested only with constructed compatibility responses;
- cancellation success relies primarily on sleeps;
- pool lease release is assumed without a subsequent constrained request or equivalent direct proof;
- the old detailed plan is replaced by a summary;
- PR #20 is force-merged over the closure record;
- PR #20 remains open after its contents are preserved;
- local test results are treated as final CI evidence;
- status lacks CI run or job identifiers;
- routine CI is weakened or split;
- a new workflow, job, matrix, runner, or scheduled trigger is added;
- full compatibility is added to routine CI;
- release automation is added;
- compatibility is promoted beyond Stage C candidate.

# Stop conditions

Stop and document the exact blocker before broadening scope when:

1. preserving two wire header values requires a generalized response metadata redesign;
2. compatibility-only overlay cannot be isolated from direct core or non-compat Python behavior;
3. native cancellation cannot be made deterministic with the existing local server fixtures;
4. lease release cannot be observed without exposing private pool internals globally;
5. GitHub Actions does not run on the pushed final tree because of repository configuration outside this pass;
6. the original PR #20 plan cannot be preserved without losing material content;
7. direct HTTPX sync and async header behavior conflicts materially.

For any stop condition, record:

- exact module/type boundary;
- failing executable fixture;
- smallest additional surface required;
- why the broader alternative was rejected;
- which acceptance criteria remain open.

Do not mark closure complete under a stop condition.

# Suggested commit decomposition

1. `docs: reopen final HTTPX closure status`
   - record baseline;
   - mark metadata, cancellation, CI, and PR hygiene open.

2. `test: pin compressed response header and cancellation oracle`
   - sync/async header observations;
   - deterministic native cancellation fixture;
   - failing reference comparisons before product changes.

3. `fix: preserve compressed wire metadata for HTTPX compatibility`
   - narrow two-field snapshot;
   - compatibility-only overlay;
   - core default policy unchanged.

4. `test: prove native compressed cancellation releases lease`
   - constrained-client second request;
   - state and second-selection assertions;
   - consolidate duplicate cases.

5. `docs: preserve native raw adapter implementation plan`
   - copy complete PR #20 plan under unique path;
   - link closure record;
   - close PR #20 after landing.

6. `docs: bind final HTTPX closure to CI evidence`
   - exact commands/counts;
   - final executable SHA;
   - CI run/job IDs;
   - bounded Stage C candidate wording.

Combining commits 2 through 4 is acceptable when required for a buildable intermediate tree. Keep the final status-only evidence commit separate.

# Handoff checklist

Before implementation:

- [ ] Confirm `main` equals or is an understood descendant of `52f540483322a47db11ebff5e17079d21370473f`.
- [ ] Read `AGENTS.md` and relevant Rust/Python skill guidance.
- [ ] Reopen active status wording.
- [ ] Fetch and preserve the complete PR #20 plan content.
- [ ] Reproduce HTTPX compressed header behavior directly.

During metadata work:

- [ ] Retain only original `Content-Encoding` and `Content-Length` metadata.
- [ ] Keep core default stripped-header policy unchanged.
- [ ] Overlay wire values only in the HTTPX compatibility surface.
- [ ] Keep internal decompression independent of visible compatibility headers.
- [ ] Add sync and async raw/decoded header comparisons.

During cancellation work:

- [ ] Use the built-in EggFetch `AsyncClient`.
- [ ] Block a compressed raw source read deterministically.
- [ ] Cancel the pending read.
- [ ] Explicitly close iterator and response.
- [ ] Prove a second constrained-client request succeeds.
- [ ] Avoid arbitrary sleeps.

During repository hygiene:

- [ ] Copy the complete PR #20 plan to a unique path on `main`.
- [ ] Link the short closure record to it.
- [ ] Comment on PR #20 with the preservation commit/path.
- [ ] Close PR #20 without merging its conflicting file.

Before final closure:

- [ ] Focused tests pass.
- [ ] Core-focused tests pass.
- [ ] `./scripts/check.sh` passes.
- [ ] Full pinned compatibility passes.
- [ ] API oracle is clean.
- [ ] Final executable tree is pushed.
- [ ] Existing CI passes.
- [ ] CI checked-out SHA, run ID, job ID, and conclusion are recorded.
- [ ] Final status remains Stage C candidate.
- [ ] No CI or release expansion occurred.

# Final closure statement

This line is closed only when EggFetch’s built-in compressed streaming path matches HTTPX 0.28.1 for encoded body bytes, decoded body bytes, and visible wire content metadata; native async cancellation demonstrably releases the selected stream and pool lease; the detailed implementation plan is preserved and PR #20 is closed; and the exact final executable tree is verified by the existing single routine CI job with recorded run and job identifiers.