# HTTPX Drop-In Verification, Substitution, and Lifecycle Corrective Closure

Status: ready for implementation handoff

## Purpose

Close the remaining defects left after the first HTTPX corrective pass without expanding the compatibility roadmap. The prior pass fixed several product semantics, but it did not establish trustworthy fail-closed API verification, real downstream substitution, complete auth and streaming ownership, lossless compatibility-data conversion, exact-SHA qualification, or coherent release evidence.

This pass is complete only when the repository can prove one of the following outcomes against one exact candidate SHA:

- **Stage C candidate, accurately bounded:** implementation tests pass, but one or more release-qualification criteria remain explicitly blocked; or
- **Stage C released:** every criterion in this plan passes against built candidate artifacts and retained exact-SHA evidence.

Partial implementation must remain labeled `Stage C candidate`. This plan does not authorize restoring `Stage C released` merely because test count increased or a workflow file exists.

## Audited baseline

Implementation begins from:

- repository: `eggstack/eggfetch`
- branch: `main`
- audited baseline SHA: `7c0032a1cd8e140461467012bf050a622d47cf93`
- reference distribution: `httpx==0.28.1`
- facade import: `eggfetch.compat.httpx`
- existing controlled shim source: `compat/httpx-shim/`
- existing corrective status: `plans/httpx-drop-in-corrective-evidence-and-semantics-closure-status.md`

Before changing code, record the actual starting SHA. If `main` has advanced, review every intervening commit and update the implementation status with whether each finding below still applies.

## Findings this pass must close

The implementation agent must treat the following as release-blocking defects:

1. Required CI still runs the facade manifest comparison with `continue-on-error: true`.
2. The manifest comparator reports 82 unexplained structural/API differences and the allowed-difference matching does not reliably classify them.
3. The isolated downstream runner does not install the top-level shim before downstream installation.
4. The runner ignores shim-identity failures and does not re-check identity after downstream dependencies are installed.
5. Downstream packages are installed without the pinned manifest version and may pull upstream HTTPX.
6. Manifest `test-command` and `min-tests` fields are not executed or enforced.
7. Missing packages, no-tests-collected, skipped suites, and shim failures can still yield an overall passing downstream result.
8. Sync and async auth remain disabled for mounted and custom transports.
9. Async auth does not implement a true async-flow driver.
10. Intermediate auth responses and one-shot-body replay semantics are not proven.
11. Async streaming context cleanup calls synchronous `close()` instead of awaiting `aclose()`.
12. Compatibility headers and query parameters are still converted through plain dictionaries, collapsing duplicates and ordering.
13. Explicit per-request `timeout=None` is not distinguishable from “use the client default.”
14. The qualification workflow does not actually verify a green exact-SHA `Required CI Gate`.
15. Several qualification jobs reference the verification output without directly depending on the verification job.
16. Qualification evidence is generated from an editable/source build rather than downloaded candidate wheels.
17. The evidence generator infers downstream status from imports instead of consuming actual downstream job results.
18. The qualification workflow does not retain a real exact-SHA soak/resource artifact.
19. Current stage-decision and allowed-difference documents still contain stale release claims, placeholders, and an incorrect redirect-default statement.
20. The corrective status file claims fail-closed verification and broader completion than current code provides.
21. Phase 1 remains partially complete for proxy timeout classification, lifecycle/resource assertions, interpreter shutdown, retained soak evidence, and exact CI linkage.
22. The Python 3.10 workaround introduces a collection skip even though the required compatibility test policy rejects collection skips.

## Scope constraints

This pass must remain tightly scoped.

It may:

- repair compatibility and qualification tooling;
- correct the listed client, auth, stream, timeout, parameter, and header semantics;
- add deterministic tests required to prove those corrections;
- add a local/private compatibility wheel whose distribution metadata is `httpx` solely for controlled qualification;
- add exact-SHA lifecycle, shutdown, and soak verification required by the existing roadmap;
- correct status and compatibility documentation.

It must not:

- add Trio support;
- add general AnyIO backend selection;
- add SOCKS support;
- add new protocol features;
- expand compatibility to another HTTPX version;
- implement undocumented private HTTPX modules except where an explicitly retained downstream test requires and approves a narrow surface;
- publish a package named `httpx` to public PyPI;
- replace Rust networking with httpcore or Python network I/O;
- weaken required tests, convert failures to warnings, or authorize blanket skips;
- call import-only smoke tests downstream behavioral suites;
- restore a release claim before exact-SHA qualification is complete.

## Implementation invariants

The following rules apply across all tracks:

- All final qualification must use built wheel or sdist artifacts, not `maturin develop` or editable installs.
- Reference and substitution environments must be separate.
- Upstream `httpx` may exist only in reference environments.
- Required checks fail closed on missing, malformed, skipped, unavailable, stale, or zero-test evidence.
- A generated report is not evidence unless it consumes result files emitted by commands that actually ran.
- Every release-relevant artifact must record the same full 40-character candidate SHA.
- A status document may describe blocked work, but it may not mark the pass complete while blockers remain.

# Track 1 — Restore a genuinely fail-closed API oracle

## 1.1 Classify the current 82 manifest differences

Run the existing generator and comparator against:

1. `httpx==0.28.1`;
2. `eggfetch.compat.httpx`;
3. the installed controlled top-level shim.

Produce a machine-readable classification for every current delta:

- generator normalization defect;
- real missing symbol;
- real incompatible signature/default;
- incompatible inheritance/MRO;
- incompatible symbol kind;
- approved stage-bounded difference;
- benign extra symbol permitted by profile policy.

Do not retain an aggregate “structural differences” waiver. Every difference must have a stable identity and an explicit disposition.

## 1.2 Correct generator and comparator normalization

Fix only representational noise. Normalization may canonicalize:

- module qualification in repr strings;
- equivalent annotation spellings;
- object sentinel repr values that are semantically equal;
- deterministic ordering of mappings and symbol lists.

Normalization must not erase:

- positional-only, positional-or-keyword, and keyword-only distinctions;
- required versus optional parameters;
- default-value differences;
- variadic versus explicit parameters;
- properties versus methods;
- sync versus async callables;
- exception inheritance order;
- public attributes required by consumers.

Add focused unit tests for every normalization rule.

## 1.3 Make allowed-difference matching exact and validated

Allowed differences must match by stable symbol path and difference type, not broad substring matching.

Each retained difference must include:

- stable ID;
- symbol path;
- difference type;
- reference value;
- candidate value;
- stage impact;
- owner;
- linked tests;
- review milestone or expiry;
- rationale.

Reject:

- unmatched allowed-difference records;
- duplicate IDs;
- wildcards that match more than the intended symbol;
- `resolved` records that still describe unsupported behavior;
- compatibility-equal behavior recorded as an intentional difference;
- expired records.

## 1.4 Remove informational behavior from required CI

The required compatibility job must execute the comparator without `continue-on-error`.

The aggregate `Required CI Gate` must fail when the comparator fails on any Python version used for the compatibility matrix.

Do not reduce the manifest to a subset solely to make the check green. If a public surface is intentionally outside Stage C, record the bounded exclusion in the profile and ensure user-facing claims match it.

## 1.5 Add negative oracle tests

Add tests proving the comparator fails when fixtures:

- remove `Client`;
- remove `AsyncClient`;
- change a keyword-only parameter into positional-or-keyword;
- change `follow_redirects` default;
- remove a required exception base class;
- replace a property with a method;
- alter the shim export module;
- add an expired allowed difference;
- mark an unresolved behavior `resolved`.

### Track 1 acceptance criteria

- [ ] Required CI compares `httpx==0.28.1` with `eggfetch.compat.httpx`.
- [ ] Qualification also compares the installed controlled top-level shim.
- [ ] No required comparator step uses `continue-on-error`.
- [ ] Every current manifest delta has a machine-readable disposition.
- [ ] Zero unexplained deltas remain for the claimed Stage C surface.
- [ ] Comparator normalization has focused unit tests and does not erase semantic differences.
- [ ] Allowed-difference validation rejects stale, broad, duplicate, expired, and falsely resolved records.
- [ ] Each negative oracle fixture makes the comparator exit nonzero.
- [ ] A comparator failure makes `Required CI Gate` fail.

# Track 2 — Build a real controlled substitution artifact

## 2.1 Separate facade distribution and controlled replacement artifact

Retain the normal eggfetch distribution and `eggfetch.compat.httpx` facade.

For controlled Stage C qualification, build a local/private wheel that:

- has distribution metadata name `httpx`;
- has a version compatible with the pinned 0.28.1 dependency range;
- exports the top-level `httpx` package;
- re-exports from `eggfetch.compat.httpx`;
- declares an explicit marker such as `__eggfetch_shim__ = True`;
- depends on the exact candidate eggfetch version or candidate wheel policy;
- is never published to public PyPI by repository workflows.

The existing differently named shim may remain for development, but it must not be presented as satisfying downstream `Requires-Dist: httpx` metadata.

## 2.2 Prove artifact identity by metadata and symbols

Do not infer shim identity from path substrings. A normal installed replacement wheel may live at `site-packages/httpx/__init__.py`.

The identity verifier must assert:

- `importlib.metadata.distribution("httpx")` resolves to the controlled replacement distribution;
- the distribution version is the expected pinned compatibility version;
- `httpx.__eggfetch_shim__ is True`;
- `httpx.Client.__module__` and `httpx.AsyncClient.__module__` resolve to eggfetch compatibility code;
- runtime diagnostics report the candidate eggfetch version and Stage C candidate status;
- the installed distribution inventory contains no upstream HTTPX wheel metadata;
- `pip check` passes in the controlled environment;
- the direct URL or artifact hash matches the locally built replacement wheel.

## 2.3 Add a hard public-publish guard

Add a repository check that fails if any public release workflow attempts to upload the controlled replacement distribution.

The replacement wheel may be uploaded only as a private CI artifact for qualification.

### Track 2 acceptance criteria

- [ ] A local/private replacement wheel has distribution metadata name `httpx`.
- [ ] Installing that wheel satisfies downstream `Requires-Dist: httpx` metadata in the controlled environment.
- [ ] `pip check` passes without upstream HTTPX installed.
- [ ] Identity verification uses distribution metadata and explicit shim markers, not path-name heuristics.
- [ ] The replacement wheel imports `Client` and `AsyncClient` from eggfetch compatibility code.
- [ ] A negative fixture using upstream HTTPX fails identity verification.
- [ ] A negative fixture using the differently named development shim where `Requires-Dist: httpx` remains unsatisfied fails qualification.
- [ ] Public publish workflows cannot upload the controlled `httpx` replacement artifact.

# Track 3 — Rebuild downstream execution as real pinned suites

## 3.1 Upgrade the downstream manifest schema

Every required downstream entry must include:

- package name;
- exact version;
- source type: wheel, sdist, or repository commit;
- source URL or package index identity;
- artifact hash or commit SHA;
- required versus informational classification;
- public versus private HTTPX API dependence;
- install command or installation policy;
- exact test command;
- expected minimum collected test count;
- expected maximum skips;
- network policy;
- timeout;
- supported Python versions;
- covered compatibility category IDs;
- known incompatibilities and whether they are release-blocking.

Do not use `usage=public` as an overloaded replacement for requiredness.

## 3.2 Install in a controlled order

For each required substitution environment:

1. create a clean venv;
2. install the exact candidate eggfetch wheel;
3. install the exact controlled replacement `httpx` wheel;
4. verify shim identity;
5. install the pinned downstream source and its dependencies without contacting an uncontrolled HTTPX source;
6. run `pip check`;
7. re-run shim identity and upstream-distribution checks after all dependency installation;
8. execute the manifest’s exact test command;
9. collect structured test counts and output;
10. retain environment and artifact metadata.

The runner must never silently fall back to `pytest --pyargs` or import smoke when a manifest command exists.

## 3.3 Pin and verify sources

Use exact versions from the manifest. Installing `pkg["name"]` without its pinned version is prohibited.

Where a published wheel omits tests, fetch the pinned sdist or repository commit with the expected hash. If no meaningful executable suite is available, classify that package as informational and do not count it toward required category coverage.

## 3.4 Enforce real test execution

Parse the declared runner’s structured output where available. For pytest, emit JSON or JUnit results and record:

- collected count;
- passed count;
- failed count;
- error count;
- skipped count;
- xfailed/xpassed count;
- duration;
- timeout status.

Required entries fail on:

- source unavailable;
- hash mismatch;
- installation failure;
- `pip check` failure;
- shim identity failure;
- upstream HTTPX presence;
- missing command;
- zero tests when `min-tests > 0`;
- collected count below `min-tests`;
- unexpected skip, xfail, error, failure, or timeout.

## 3.5 Define the minimum required Stage C portfolio

Retain at least one passing real consumer suite for each category:

- sync SDK/client usage;
- asyncio SDK/client usage;
- ASGI test-client behavior;
- mock transport behavior;
- streaming/SSE behavior;
- custom auth flow behavior;
- event-hook or instrumentation behavior;
- custom or mounted transport behavior.

A package that only imports successfully does not cover a category.

A package that relies on private HTTPX internals must either:

- be informational; or
- have an explicit, narrowly approved private-surface compatibility decision and tests.

## 3.6 Make aggregation fail closed

The aggregate downstream result must set `overall_pass=true` only when:

- every required entry ran;
- every required entry passed;
- every required category is covered by a real executed suite;
- no required entry is skipped, unavailable, unexecuted, below minimum count, or stale;
- every result references the same candidate and artifact hashes.

Unknown package names must exit nonzero.

### Track 3 acceptance criteria

- [ ] Every required downstream entry is installed from the pinned version/source and verified hash.
- [ ] The exact manifest `test-command` is executed.
- [ ] `min-tests` and skip budgets are enforced.
- [ ] The controlled replacement wheel is installed before downstream dependencies.
- [ ] Shim identity and upstream absence are checked both before and after downstream installation.
- [ ] `pip check` passes after all packages are installed.
- [ ] Missing package, zero tests, skip, timeout, hash mismatch, upstream HTTPX, and identity mismatch all fail required entries.
- [ ] Import-only entries are informational and cannot satisfy category coverage.
- [ ] All required Stage C categories have passing executed consumer suites.
- [ ] `overall_pass` is false whenever any required entry did not execute and pass.
- [ ] Runner meta-tests cover every false-green path listed above.

# Track 4 — Complete auth state-machine behavior across transports

## 4.1 Remove transport-based auth suppression

Delete logic that disables auth when a mounted, custom, mock, ASGI, WSGI, sync, or async transport is selected.

Auth transforms requests before dispatch regardless of transport destination. Transport selection decides where a yielded request is sent; it does not own client auth policy.

## 4.2 Implement separate sync and async auth drivers

Implement a generic sync driver that supports the pinned HTTPX auth contract, including custom multi-yield flows.

Implement a generic async driver that:

- uses an async auth flow when provided;
- awaits each yielded request/response transition as required;
- falls back only according to measured HTTPX behavior;
- does not run asynchronous auth logic through a synchronous generator accidentally.

Do not special-case DigestAuth in the client.

## 4.3 Handle intermediate response ownership

For each auth round trip:

- dispatch the yielded request;
- feed the response back into the flow;
- close or drain intermediate responses according to HTTPX-observed behavior;
- retain only the final response for the caller;
- preserve request context on auth and transport exceptions.

Add socket/pool assertions that repeated challenge flows do not leak permits or connections.

## 4.4 Enforce replayability

When auth requires replaying a request body:

- replay buffered/replayable bodies safely;
- reject one-shot sync iterators when replay is required;
- reject one-shot async iterators when replay is required;
- raise the matching compatibility stream/replay exception with request context;
- never silently send an empty or partially consumed body.

## 4.5 Prove hook ordering

Record pinned HTTPX observations for:

- request hook;
- auth mutation;
- transport dispatch;
- intermediate auth response;
- final response hook.

Implement and test the same order for sync and async clients.

### Track 4 acceptance criteria

- [ ] BasicAuth modifies requests sent through native, custom, mounted, and mock transports.
- [ ] DigestAuth completes a real two-request challenge through native and mock transports.
- [ ] A custom sync auth flow with more than one yield returns the final response.
- [ ] A custom async auth flow with more than one yield returns the final response.
- [ ] Mounted and custom transports observe auth-modified requests.
- [ ] Intermediate auth responses release their stream and pool resources.
- [ ] One-shot sync and async bodies fail deterministically when replay is required.
- [ ] Per-request auth disabling works through every transport path.
- [ ] Hook/auth/transport ordering matches committed HTTPX observations.
- [ ] Auth failures retain compatibility request context.

# Track 5 — Correct async stream ownership and lifecycle proof

## 5.1 Use asynchronous cleanup in async contexts

Change `AsyncClient.stream()` cleanup to await `response.aclose()`.

Do not call synchronous `close()` on an async native stream unless the response type is known to be purely synchronous. Provide one internal cleanup helper if mixed custom transports require both paths.

## 5.2 Make close operations idempotent

Verify repeated calls to:

- `Response.close()`;
- `Response.aclose()`;
- client close/aclose after response close;
- context-manager cleanup after user-initiated close;

are safe and do not double-release permits, raise spurious errors, or block.

## 5.3 Add observable lifecycle tests

Test normal exit, unread exit, partial consumption, exception exit, cancellation, and early task termination.

Observe at least:

- response closed state;
- server-side connection closure or abort;
- pool permit release;
- task count stabilization;
- thread count stabilization where relevant;
- file descriptor stabilization on Unix;
- clean interpreter shutdown.

### Track 5 acceptance criteria

- [ ] `async with client.stream(...)` awaits `response.aclose()` on every exit path.
- [ ] Unread and partially read async responses do not retain pool permits.
- [ ] Cancellation during the context closes or aborts the underlying stream.
- [ ] Sync and async response close operations are idempotent.
- [ ] Unsafe unread connections are not returned to the reusable pool.
- [ ] Repeated early exits remain within committed task, thread, FD, and RSS thresholds.
- [ ] Clean interpreter shutdown passes after unconsumed sync and async responses.

# Track 6 — Preserve repeated values and explicit timeout intent

## 6.1 Preserve query parameter multiplicity and order

Replace plain dictionary conversion of `QueryParams` with an ordered multi-value representation or a fully encoded URL query that preserves:

- repeated keys;
- original ordering;
- blank values;
- Unicode and percent encoding;
- client/request merge behavior;
- redirect behavior.

Test wire-observed values for:

```text
?a=1&a=2&b=&a=3
```

## 6.2 Preserve duplicate request and response headers

Replace plain dictionary conversion of compatibility headers where duplicates matter.

Carry ordered raw header pairs through:

- native transport serialization;
- custom transports;
- mounted transports;
- mock transports;
- response wrapping.

At minimum test:

- duplicate `Set-Cookie` response fields;
- duplicate non-comma-joinable request fields where legal;
- comma-joinable fields;
- case-insensitive lookup over retained raw pairs;
- raw ordering visible to custom transports.

## 6.3 Distinguish default timeout from explicit `None`

Introduce a private sentinel for “use client default.”

Required semantics:

- omitted request timeout uses the client timeout;
- explicit `timeout=None` disables connect/read/write/pool compatibility deadlines;
- scalar request timeout sets four phase deadlines without setting a total deadline;
- a phase-specific timeout preserves unspecified/reference behavior exactly;
- native eggfetch total timeout remains available only through native APIs or an explicit extension.

## 6.4 Audit adjacent lossy conversions

Review only directly related conversion boundaries:

- cookies with same name but different domain/path;
- URL userinfo and percent encoding;
- multipart per-part headers;
- response history request attachment;
- extension values.

Add tests only where current conversion is lossy or incorrect.

### Track 6 acceptance criteria

- [ ] Repeated query keys reach native and custom transports in exact order and multiplicity.
- [ ] Query merging matches pinned HTTPX differential cases.
- [ ] Duplicate raw headers survive all supported transport paths.
- [ ] Multiple `Set-Cookie` headers remain independently observable.
- [ ] No relevant compatibility conversion uses a plain dictionary when duplicates or ordering are semantically significant.
- [ ] Omitted timeout and explicit `timeout=None` produce different native configurations.
- [ ] Explicit `timeout=None` disables all compatibility phase deadlines.
- [ ] Scalar and phase-specific timeout behavior passes differential tests.
- [ ] Adjacent lossy boundaries identified by the audit have regression tests.

# Track 7 — Repair the required Python matrix without skips

## 7.1 Remove the Python 3.10 collection skip

Use the existing supported TOML fallback dependency or add `tomli` for Python 3.10 test environments.

The required compatibility suite must collect the downstream portfolio meta-tests on Python 3.10. Do not solve import compatibility by adding a module-level skip.

## 7.2 Keep required skip auditing strict

Required compatibility jobs must retain zero unexplained:

- skips;
- xfails;
- collection errors;
- zero-test modules.

Approved platform-specific exclusions must be encoded as separate non-required jobs or explicit profile policy, not hidden inside the required suite.

### Track 7 acceptance criteria

- [ ] Python 3.10 collects and runs downstream portfolio meta-tests.
- [ ] No module-level TOML-related skip remains.
- [ ] Required compatibility jobs on Python 3.10–3.13 report zero unexplained skips, xfails, and collection errors.
- [ ] A fixture proving collection skip detection still fails the required job.

# Track 8 — Make qualification exact-SHA and artifact-driven

## 8.1 Verify the actual required check

The qualification `verify` job must query GitHub check-run or workflow-run data for the supplied full candidate SHA and confirm:

- exact `head_sha` match;
- check name `Required CI Gate`;
- completed status;
- success conclusion;
- run attempt and URL recorded.

Checking out the SHA is not CI verification.

Grant only the read permissions required for actions/check metadata.

## 8.2 Make every job depend on verification

Every job using `needs.verify.outputs.sha` must directly include `verify` in its `needs` list.

Add a workflow-structure test that parses the workflow and fails when a job references another job’s outputs without declaring that dependency.

## 8.3 Use candidate artifacts everywhere

Build candidate wheels, sdist, and controlled replacement wheel once.

All downstream, smoke, compatibility, shutdown, soak, and evidence jobs must download and install those artifacts. Remove `maturin develop` from final qualification and evidence generation.

Development CI may continue to use editable builds, but it does not count as release qualification.

## 8.4 Emit structured job result artifacts

Each qualification job must emit JSON containing:

- schema version;
- candidate SHA;
- artifact hashes;
- job/run ID and attempt;
- platform and Python version;
- command executed;
- start/end timestamps;
- counts and result;
- failure reason when applicable.

Downstream matrix jobs must upload one result artifact per package.

## 8.5 Add a real retained soak/resource job

Run the existing or corrected soak harness against the built candidate wheel and exact SHA.

Cover:

- keep-alive churn;
- connection failure churn;
- streaming early close;
- concurrent sync clients;
- concurrent async clients;
- retries and redirects;
- proxy paths;
- auth challenge loops;
- interpreter shutdown.

Emit machine-readable thresholds for task, thread, FD, RSS, pool, request success, and timeout counts. Retain the artifact for the release-review period.

## 8.6 Fix controlled shim qualification

Install the controlled replacement artifact and verify identity by metadata and marker. Remove path assertions that reject normal `site-packages/httpx/__init__.py` installation.

Run both:

- facade compatibility tests through `eggfetch.compat.httpx`;
- top-level replacement tests through `import httpx`.

### Track 8 acceptance criteria

- [ ] Qualification fails unless the exact candidate SHA has a successful completed `Required CI Gate`.
- [ ] The CI run URL and attempt are retained.
- [ ] Every job referencing verification outputs directly depends on `verify`.
- [ ] A workflow-structure test enforces output/dependency correctness.
- [ ] Final qualification uses built wheels and sdist, never editable/source installs.
- [ ] The controlled replacement wheel is included in the candidate artifact set.
- [ ] Every matrix job emits a structured result artifact bound to the candidate SHA and artifact hashes.
- [ ] A retained exact-SHA soak/resource artifact exists and passes thresholds.
- [ ] Shim qualification accepts normal package paths and verifies identity through metadata and markers.
- [ ] The aggregate qualification gate fails on any missing, skipped, cancelled, stale, or unsuccessful required job.

# Track 9 — Replace inferred evidence with consumed evidence

## 9.1 Redesign the evidence generator inputs

The final generator must take explicit paths to result artifacts for:

- required CI verification;
- facade manifest comparison;
- top-level shim manifest comparison;
- compatibility behavior tests;
- downstream suites;
- wheel/sdist smoke tests;
- controlled replacement tests;
- shutdown/lifecycle tests;
- soak/resource results;
- package inventories;
- artifact hashes.

It must not discover success by importing packages from its own environment or counting test function definitions in source files.

## 9.2 Validate evidence consistency

Before writing `overall_pass=true`, verify:

- all required result categories are present;
- all results reference the same full candidate SHA;
- artifact hashes match the built candidate artifacts;
- all required entries executed and passed;
- all minimum counts and skip budgets passed;
- the exact CI gate is green;
- the soak result is present and passing;
- no placeholder, unknown, pending, unavailable, or stale value exists;
- the profile stage matches the mechanically derived stage.

## 9.3 Add negative evidence fixtures

Evidence generation must fail for fixtures containing:

- mismatched SHA;
- missing downstream package result;
- no tests collected;
- skipped required suite;
- upstream HTTPX identity;
- artifact hash mismatch;
- missing soak result;
- absent CI result;
- failed manifest comparison;
- placeholder `[N]`;
- stale or invalidated source evidence;
- `overall_pass=true` with a required failure.

## 9.4 Keep invalidated historical evidence unambiguous

Historical evidence may remain, but either:

- remove `overall_pass`; or
- set it to `false` and add a separate `historical_result` field.

An invalidated file must not simultaneously present an unqualified current-looking `overall_pass=true`.

### Track 9 acceptance criteria

- [ ] The generator consumes actual job result artifacts and takes their paths explicitly.
- [ ] It does not infer downstream success from imports.
- [ ] It does not infer executed test count from source code.
- [ ] All evidence is bound to one candidate SHA and one artifact hash set.
- [ ] Every negative fixture exits nonzero.
- [ ] `overall_pass=true` is impossible with a missing, stale, skipped, unavailable, or failed required result.
- [ ] Invalidated historical evidence cannot be mistaken for a current pass.

# Track 10 — Close the remaining production-lifecycle blockers

This track is limited to the Phase 1 items already identified as blockers.

## 10.1 Proxy and TLS timeout classification

Add deterministic local integration coverage for:

- a proxy that accepts TCP but stalls its CONNECT response;
- a real connector path that stalls TLS handshake;
- correct timeout exception class and request context;
- no reliance on an implicit compatibility total timeout.

## 10.2 Cross-platform interpreter shutdown

Run clean shutdown subprocess tests on Linux, macOS, and Windows for:

- unused sync client;
- used sync client;
- unused async client;
- used async client;
- unread streaming response;
- partially read streaming response;
- auth challenge sequence;
- close/request race.

## 10.3 Resource assertions

Add platform-appropriate measurements:

- Linux: FD, task/thread, RSS, socket state;
- macOS: FD, thread, RSS through supported APIs or stable subprocess measurement;
- Windows: handle/thread/process memory measurements where stable.

Do not accept partial request success as a substitute for semantic correctness. Separate scheduling tolerance from required operation success.

### Track 10 acceptance criteria

- [ ] Slow proxy CONNECT response has a deterministic timeout classification test.
- [ ] Real local TLS handshake stall maps to the expected compatibility timeout.
- [ ] Interpreter shutdown tests pass on Linux, macOS, and Windows.
- [ ] Close/request and auth/race tests include leak assertions appropriate to each platform.
- [ ] The final candidate soak includes lifecycle paths and passes committed thresholds.
- [ ] Phase 1 status contains no unclassified release-blocking lifecycle item.
- [ ] Any platform measurement limitation is explicitly bounded and does not produce a false pass.

# Track 11 — Reconcile claims, profile, and implementation status

## 11.1 Correct current stale documents immediately

Before final qualification, update current documents so they consistently say `Stage C candidate`.

Correct at minimum:

- `docs/reference/compatibility-stage-decision.md`;
- `compat/httpx/0.28.1/allowed-differences.toml`;
- `README.md`;
- `AGENTS.md`;
- migration and compatibility docs;
- runtime diagnostics;
- prior corrective status.

Remove literal `[N]` placeholders from any document presented as current evidence.

Correct the HTTPX default to `follow_redirects=False`. Remove the redirect-default intentional difference if eggfetch matches it.

## 11.2 Create a new implementation status file

Create:

```text
plans/httpx-drop-in-verification-substitution-and-lifecycle-corrective-closure-status.md
```

It must contain:

- starting SHA;
- implementation commits;
- final candidate SHA;
- changed files by track;
- exact commands;
- test counts;
- required CI run URL and attempt;
- qualification run URL and attempt;
- artifact names and hashes;
- downstream result table with collected counts;
- soak/resource artifact and thresholds;
- criterion-by-criterion mapping;
- remaining allowed differences;
- blockers;
- mechanically derived final stage.

## 11.3 Reconcile prior status files without rewriting history

Amend earlier status documents with correction notices that identify:

- the earlier overclaim;
- the corrective commit that supersedes it;
- the replacement evidence;
- whether the older phase is now actually complete.

### Track 11 acceptance criteria

- [ ] No current document says `Stage C released` before exact-SHA qualification passes.
- [ ] No current evidence document contains `[N]`, pending, unknown, or not-run placeholders while claiming completion.
- [ ] The redirect-default policy correctly states HTTPX 0.28.1 behavior.
- [ ] The prior corrective status no longer claims fail-closed checks that are informational.
- [ ] The new status maps every acceptance criterion to retained evidence or an explicit blocker.
- [ ] Earlier status files contain correction notices rather than silently rewritten claims.
- [ ] The final stage is mechanically derived from evidence, not manually asserted.

# Required test matrix

| Area | Sync | Asyncio | Native | Custom/mount/mock | Built wheel | Top-level shim | Differential |
|------|------|---------|--------|-------------------|-------------|----------------|--------------|
| API manifest/signatures | yes | yes | n/a | n/a | yes | yes | yes |
| Downstream substitution | yes | yes | as used | as used | yes | yes | reference env |
| Auth state machine | yes | yes | yes | yes | yes | yes | yes |
| Stream cleanup | yes | yes | yes | yes | yes | yes | yes |
| Repeated params/headers | yes | yes | yes | yes | yes | yes | yes |
| Timeout omitted vs None | yes | yes | yes | where applicable | yes | yes | yes |
| Shutdown/resources | yes | yes | yes | yes | yes | yes | no |
| Soak | yes | yes | yes | selected paths | yes | yes | no |

Every required test must have a stable ID or direct acceptance-criterion mapping in the final status.

# Recommended implementation sequence

Execute in this order:

1. Track 11.1 — correct stale current claims and status immediately.
2. Track 1 — repair and enforce the API oracle.
3. Track 2 — build the controlled replacement artifact.
4. Track 3 — rebuild downstream execution around pinned real suites.
5. Tracks 4–7 — close auth, streaming, conversion, timeout, and Python matrix defects.
6. Track 10 — close the remaining lifecycle blockers.
7. Track 8 — repair exact-SHA artifact qualification.
8. Track 9 — generate evidence only from retained job artifacts.
9. Tracks 11.2–11.3 — reconcile status and derive the final stage.

Tracks 4, 5, 6, and 7 may proceed in parallel after Tracks 1–3 define reliable tests. Tracks 8, 9, and final status reconciliation must remain last.

# Mandatory implementation checkpoints

## Checkpoint A — Verification foundation

Required before product-semantic work is called verified:

- facade manifest comparator fails closed;
- controlled replacement artifact builds and identifies correctly;
- downstream runner fails all false-green fixtures.

## Checkpoint B — Semantic closure

Required before qualification work begins:

- auth works through all transport paths;
- async auth flow works;
- async stream context awaits cleanup;
- duplicate params/headers are preserved;
- explicit timeout `None` works;
- Python 3.10 required suite runs without skips.

## Checkpoint C — Lifecycle closure

Required before freezing a candidate:

- proxy/TLS timeout integration tests pass;
- cross-platform shutdown passes;
- leak/resource assertions pass;
- retained soak workflow is operational.

## Checkpoint D — Candidate qualification

Required before restoring a release claim:

- exact SHA has green `Required CI Gate`;
- built artifacts are frozen and hashed;
- all required downstream suites pass against the controlled replacement;
- lifecycle and soak artifacts pass against the same SHA;
- evidence generator consumes all retained artifacts and reports a coherent pass.

# Completion gate

This corrective pass is complete only when all of the following are true:

- [ ] The facade and top-level shim API oracles fail closed with zero unexplained deltas.
- [ ] The controlled replacement wheel satisfies `Requires-Dist: httpx` in qualification without upstream HTTPX.
- [ ] Downstream suites use pinned sources, exact commands, enforced minimum counts, and fail-closed aggregation.
- [ ] Shim identity is verified before and after downstream dependency installation.
- [ ] Auth is applied through native, custom, mounted, mock, ASGI, and WSGI paths as applicable.
- [ ] Sync and async multi-step auth flows dispatch every yielded request and clean intermediate responses.
- [ ] Async streaming context cleanup awaits `aclose()` and passes lifecycle/resource tests.
- [ ] Repeated query parameters and duplicate headers remain lossless through supported paths.
- [ ] Explicit per-request `timeout=None` disables compatibility phase timeouts.
- [ ] Python 3.10–3.13 required compatibility jobs run with zero unexplained skips or collection issues.
- [ ] The exact candidate SHA has a retained successful `Required CI Gate`.
- [ ] Qualification uses only built candidate artifacts.
- [ ] Exact-SHA downstream, shutdown, resource, and soak artifacts all pass.
- [ ] Evidence is generated solely from actual retained result files and cannot false-pass negative fixtures.
- [ ] Current documentation and status files contain no stale release claim, placeholder, or policy contradiction.
- [ ] One exact candidate SHA and artifact hash set is used across CI, qualification, downstream, lifecycle, soak, and evidence.
- [ ] `Stage C released` is restored only if every criterion above passes.

If any criterion remains blocked, the final status must identify the blocker and leave the profile at `Stage C candidate` or a lower accurately supported stage.