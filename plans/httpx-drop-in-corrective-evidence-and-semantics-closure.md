# HTTPX Drop-In Corrective Evidence and Semantics Closure Pass

Status: ready for implementation handoff

## Purpose

Correct the implementation and qualification defects found after the HTTPX drop-in roadmap was implemented. The repository now contains a substantial HTTPX 0.28.1 compatibility facade, but the current `Stage C released` claim is not supported by the committed evidence and several concrete compatibility defects remain in timeout handling, auth flows, streaming lifecycle, data preservation, and release validation.

This pass is complete only when the compatibility implementation, compatibility oracle, downstream substitution harness, status documents, CI policy, and release evidence all agree on one exact candidate commit.

## Baseline

Implementation begins from:

- repository: `eggstack/eggfetch`
- branch: `main`
- audited baseline SHA: `e8c7a03b3972ce2feaf19477afeeac7bb32d5763`
- reference implementation: `httpx==0.28.1`
- current compatibility import: `eggfetch.compat.httpx`
- optional controlled-environment shim: `compat/httpx-shim/`

Record the actual starting SHA in the implementation status file. If `main` has advanced, review every intervening commit and update the baseline section before changing code.

## Problem statement

The corrective pass must address all of the following findings:

1. The CI API manifest compares the root native `eggfetch` package rather than `eggfetch.compat.httpx`.
2. The API manifest comparison is informational and cannot fail CI.
3. The isolated downstream runner installs upstream `httpx==0.28.1`, so it does not prove eggfetch substitution.
4. Missing packages, missing test suites, and skipped downstream runs can still contribute to an overall passing evidence document.
5. The committed evidence reports `overall_pass=true` even though most downstream packages were not available in the evidence environment.
6. The compatibility timeout layer adds a five-second whole-request total deadline that HTTPX does not impose by default.
7. Multi-step auth flows yield follow-up requests that are not dispatched by `Client` or `AsyncClient`.
8. Authentication is bypassed for mounted and custom transports.
9. Sync and async streaming context managers do not close or discard their yielded response in `finally`.
10. Client-level extensions are lost when request-level extensions are omitted.
11. Repeated query parameters and duplicate headers are at risk of collapse when compatibility objects are converted to dictionaries for the native layer.
12. The compatibility policy contains stale and factually incorrect allowed-difference records, including the redirect default.
13. Phase 1 remains marked partially complete while Phase 6 declares all phases complete and released.
14. The stage-decision document contains unresolved `[N]` placeholders.
15. The top-level shim distribution name does not satisfy downstream package metadata requiring `httpx`, and the repository overstates what pip resolution proves.
16. The current release evidence is not bound to one exact green required-CI and immutable dry-run candidate.
17. Documentation and repository governance disagree on whether the required CI gate is mandatory.

## Required outcome

At the end of this pass, eggfetch may claim no more than the highest stage actually supported by retained evidence:

- **Stage B** if ordinary HTTPX-style network client code works after changing imports, but unmodified downstream substitution remains incomplete.
- **Stage C candidate** if the implementation and clean-environment substitution suites pass but immutable release qualification is not complete.
- **Stage C released** only after every acceptance criterion in this plan is satisfied against one exact candidate SHA.

The implementation agent must downgrade the current release claim before beginning corrective work. The claim may be restored only in the final qualification commit after all gates pass.

## Non-goals

This pass must not:

- add Trio or AnyIO backend support;
- add SOCKS proxy support;
- claim compatibility with HTTPX versions other than 0.28.1;
- implement undocumented HTTPX private modules merely to satisfy packages that depend on private internals;
- replace `eggfetch-core` networking with Python networking or httpcore;
- publish a package named `httpx` to public PyPI;
- weaken tests, turn required failures into warnings, or add blanket skips;
- treat import success as equivalent to downstream behavioral compatibility;
- treat generated JSON or status markdown as evidence when the underlying commands did not run successfully.

# Track A — Immediate claim containment and evidence reset

## A1. Downgrade the compatibility claim

Before functional changes, update all user-facing and machine-readable claims from `released` to `candidate`, `beta`, or the highest lower stage justified by current evidence.

At minimum review:

- `compat/httpx/0.28.1/profile.toml`
- `README.md`
- `AGENTS.md`
- `docs/reference/compatibility-stage-decision.md`
- `docs/reference/compatibility.md`
- migration documentation
- release checklist and process documentation
- `plans/httpx-drop-in-phase-6-status.md`
- runtime diagnostics in `_diagnostics.py`

Do not delete historical status files. Mark superseded claims explicitly and point to the new corrective status file.

## A2. Invalidate stale generated evidence

Move, regenerate, or mark stale the following so they cannot be interpreted as current release evidence:

- `compatibility-evidence.json`
- `compatibility-report.md`
- `performance-budget-results.json` if it is not tied to the corrective candidate
- any compatibility manifest generated from pre-corrective commits

Generated files must contain an explicit status such as `invalidated`, `candidate`, or `qualified`. A file generated from an older SHA must never remain labeled as current.

### Track A acceptance criteria

- [ ] No current document or runtime diagnostic says `Stage C released` before final qualification.
- [ ] The compatibility profile status is not `released` during implementation.
- [ ] Existing evidence files identify their exact source SHA and are visibly marked stale or superseded when applicable.
- [ ] The corrective implementation status file records the downgrade commit and rationale.
- [ ] Compatibility claim linting fails if `released` appears outside approved historical records before final qualification.

# Track B — Repair the executable compatibility oracle

## B1. Compare the correct candidate module

Change API manifest generation so the candidate is:

```text
package = eggfetch.compat.httpx
```

not the root native `eggfetch` package.

Generate separate manifests for:

1. upstream `httpx==0.28.1`;
2. `eggfetch.compat.httpx`;
3. the installed top-level shim, when shim mode is being qualified.

The root native eggfetch API may have its own API-surface test, but it must not be used as the HTTPX compatibility candidate.

## B2. Restore fail-closed manifest comparison

Remove `continue-on-error: true` from the required compatibility comparison. The pure-Python compatibility facade has inspectable explicit signatures, so PyO3 signature limitations are not a valid reason to make this comparison informational.

The comparator must fail on:

- missing public symbols;
- incompatible symbol kinds;
- incompatible constructor or function parameters;
- incompatible parameter defaults;
- incompatible class inheritance or exception MRO;
- missing required properties or methods;
- unexplained extra public symbols where the profile treats extras as incompatible;
- malformed or expired allowed-difference records.

## B3. Improve manifest normalization without hiding differences

Fix the manifest generator and comparator where normalization itself creates false positives. Any normalization rule must be documented and tested. It may normalize representation noise, but it must not discard:

- positional-only versus keyword-only distinctions;
- required versus optional parameters;
- default-value differences;
- method presence;
- inheritance order;
- public attributes needed by downstream code.

## B4. Enforce allowed-difference integrity

Each allowed difference must include:

- stable ID;
- exact public symbol or behavior;
- current reference behavior;
- current eggfetch behavior;
- category;
- linked test IDs;
- owner;
- review milestone or expiry;
- stage impact.

A `resolved` record must describe the implemented behavior, not stale pre-resolution behavior. A record whose two behaviors are already equal must be removed or reclassified as informational documentation rather than an allowed incompatibility.

## B5. Correct known profile errors

At minimum:

- correct the HTTPX redirect default to `follow_redirects=False`;
- remove redirect default from intentional differences if eggfetch matches it;
- correct event-hook, transport, and mount records marked resolved but still describing unsupported behavior;
- correct timeout documentation to distinguish inactivity phase deadlines from eggfetch-native total deadlines;
- ensure exception records distinguish the native API from the compatibility facade.

### Track B acceptance criteria

- [ ] Required CI generates the candidate manifest from `eggfetch.compat.httpx`.
- [ ] The manifest comparison is a required, fail-closed step.
- [ ] The shim manifest is also compared when shim qualification is enabled.
- [ ] Zero unexplained public API deltas remain for the claimed stage.
- [ ] Every allowed difference passes schema and semantic validation.
- [ ] A negative test proves deleting a required facade symbol fails CI.
- [ ] A negative test proves changing a required default or keyword-only parameter fails CI.
- [ ] No required API-oracle step uses `continue-on-error`.
- [ ] The compatibility report records exact reference and candidate manifest hashes.

# Track C — Rebuild downstream substitution validation

## C1. Separate reference and substitution environments

Use two distinct environment classes:

- **Reference environment:** installs upstream `httpx==0.28.1` and runs reference observations.
- **Substitution environment:** must not contain the upstream HTTPX distribution or files and must resolve `import httpx` to the eggfetch-backed shim.

Never install upstream HTTPX into a substitution environment.

## C2. Make shim identity explicit

Every substitution run must assert before testing:

- `httpx.__file__` points to the controlled eggfetch shim;
- `httpx.Client.__module__` and `httpx.AsyncClient.__module__` resolve to the eggfetch compatibility implementation;
- the native extension version and emulated HTTPX version are recorded;
- upstream HTTPX files are absent from the environment;
- the installed distribution inventory is retained as evidence.

## C3. Define honest package-resolution modes

The current `httpx-eggfetch-shim` distribution does not satisfy metadata such as `Requires-Dist: httpx>=0.27,<0.29`. Document and test two distinct modes:

1. **Explicit compatibility module mode** — downstream source changes its import to `eggfetch.compat.httpx`; this supports Stage B but is not no-source-change substitution.
2. **Controlled replacement mode** — a local/private wheel provides the top-level `httpx` module for an isolated environment. Install downstream dependencies in a controlled order without upstream HTTPX, and record that this is not a public-PyPI dependency replacement.

Do not claim normal pip dependency resolution is solved unless the installed distribution metadata genuinely satisfies `Requires-Dist: httpx` and `pip check` passes without upstream HTTPX.

If a technically and legally acceptable distribution strategy cannot satisfy dependency metadata, Stage C documentation must state that consumers need an explicit constraints/bootstrap procedure.

## C4. Pin real downstream sources and commands

Extend `compat/downstream/manifest.toml` so every required entry includes:

- exact version and source artifact or repository commit;
- source URL or package index identifier;
- expected package hash where practical;
- test command;
- expected minimum collected test count;
- required versus informational classification;
- public versus private HTTPX API usage;
- network policy;
- timeout;
- supported platform(s);
- expected result.

For packages whose wheel omits tests, obtain the pinned source distribution or repository checkout. Import-only smoke tests may be retained as informational, but they cannot count as passing downstream suites.

## C5. Run downstream code against eggfetch

The runner must:

1. create a clean environment;
2. install eggfetch from the candidate wheel, not the source tree;
3. install the controlled shim or use an explicitly documented import rewrite fixture;
4. install the pinned downstream source without pulling upstream HTTPX;
5. assert shim identity;
6. run the declared test command;
7. retain stdout, stderr, collection count, pass/fail/skip counts, package inventory, and environment metadata;
8. fail when the required suite is missing, collects fewer tests than expected, times out, or is skipped.

Packages relying only on private HTTPX internals must be marked out of the public Stage C claim unless a specific compatibility decision approves those internals.

## C6. Define the required portfolio

At minimum, the required Stage C portfolio must include one successfully exercised consumer in each category:

- synchronous SDK client;
- asyncio SDK client;
- ASGI framework/test client;
- mock transport consumer;
- streaming/SSE consumer;
- custom auth flow consumer;
- event-hook/instrumentation consumer;
- custom transport or mount consumer.

A single package may cover multiple categories only when the evidence identifies the exact tests for each category.

### Track C acceptance criteria

- [ ] No substitution environment installs upstream HTTPX.
- [ ] Every substitution run asserts that `import httpx` resolves to eggfetch-backed code.
- [ ] Every required package uses a pinned source and explicit test command.
- [ ] Every required package collects at least its declared minimum number of tests.
- [ ] Missing packages, no-tests-collected, skips, and timeouts fail required entries.
- [ ] Import-only checks are reported separately and cannot satisfy Stage C.
- [ ] All required portfolio categories have at least one passing real consumer suite.
- [ ] The evidence distinguishes public-API compatibility from private-internal compatibility.
- [ ] `overall_pass` is false when any required downstream entry is unavailable or unexecuted.
- [ ] A negative test proves the runner fails when upstream HTTPX is accidentally installed.
- [ ] A negative test proves the runner fails when `httpx.__file__` does not point to the shim.
- [ ] The package-resolution documentation makes no unsupported claim that `httpx-eggfetch-shim` satisfies downstream `Requires-Dist: httpx` metadata.

# Track D — Correct timeout compatibility semantics

## D1. Separate HTTPX phase timeouts from eggfetch total timeout

The HTTPX compatibility facade must model the HTTPX 0.28.1 public timeout object: connect, read, write, and pool inactivity deadlines. It must not add an implicit whole-request total deadline.

The eggfetch-native API may retain an explicit `total` timeout as an eggfetch extension, but that extension must not be silently enabled by `eggfetch.compat.httpx.Timeout(5.0)` or the default compatibility client constructor.

## D2. Fix conversion into native timeout configuration

When converting the compatibility timeout object to the native layer:

- preserve connect/read/write/pool values independently;
- set native total timeout to `None` unless an explicitly documented eggfetch-only API requested it;
- preserve `timeout=None` as disabling all compatibility phase deadlines;
- preserve per-request timeout overrides without filling unspecified phases incorrectly.

## D3. Add behavioral timeout differentials

Add local deterministic tests that compare HTTPX and eggfetch for:

- a response lasting longer than five seconds while delivering chunks more frequently than the read deadline;
- a stalled response-header phase;
- a stalled body chunk;
- a stalled request-body producer;
- pool acquisition timeout;
- DNS/TCP/TLS connect timeout;
- scalar timeout;
- phase-specific timeout object;
- `timeout=None`;
- redirects and retries without an implicit compatibility total deadline.

### Track D acceptance criteria

- [ ] `eggfetch.compat.httpx.Timeout(5.0)` does not configure a five-second native total deadline.
- [ ] A healthy response lasting more than five seconds completes when each chunk arrives within the read deadline.
- [ ] A stalled body chunk raises compatibility `ReadTimeout` with the original request attached.
- [ ] A stalled request-body producer raises `WriteTimeout`.
- [ ] Pool and connect failures map to `PoolTimeout` and `ConnectTimeout` respectively.
- [ ] `timeout=None` disables all four compatibility phase deadlines.
- [ ] Compatibility timeout repr, equality, attributes, signatures, and defaults match the pinned reference manifest.
- [ ] Native eggfetch total-timeout functionality remains available only through native APIs or an explicitly named extension.

# Track E — Implement complete authentication flow dispatch

## E1. Implement generic flow driving

Refactor sync and async send paths so auth is a request/response state machine:

1. obtain the appropriate sync or async auth flow;
2. dispatch every request yielded by the flow;
3. feed each response back into the flow;
4. continue until the flow terminates;
5. return the final response;
6. close or retain intermediate responses according to HTTPX-observed behavior;
7. preserve request and response context on failures.

Do not special-case DigestAuth in the client. The generic mechanism must support user-defined auth flows.

## E2. Apply auth before all transport dispatch

Auth must operate consistently whether the final request is handled by:

- the native transport;
- a custom sync transport;
- a custom async transport;
- a mounted transport;
- `MockTransport`;
- ASGI or WSGI transport where applicable.

Use pinned HTTPX observations to determine exact ordering among request hooks, auth transformation, transport dispatch, and response hooks. Commit the observed order as differential tests.

## E3. Support required body semantics

Honor the auth contract for flows that require request or response bodies. Ensure one-shot request streams are not replayed silently. Emit the correct stream or replayability exception when an auth flow requires an unavailable body.

## E4. Add end-to-end auth tests

Tests must include:

- BasicAuth through native and MockTransport paths;
- DigestAuth 401 challenge followed by an actually dispatched authenticated request;
- multiple auth round trips;
- custom sync auth flow yielding more than one request;
- custom async auth flow;
- auth disabled per request;
- mounted transport receiving the auth-modified request;
- response hooks observing the final response;
- intermediate-response cleanup;
- streamed request body replay rejection.

### Track E acceptance criteria

- [ ] DigestAuth completes a real two-request challenge-response exchange against a local server.
- [ ] The authenticated follow-up request is actually dispatched and its response returned.
- [ ] Custom auth generators with multiple yields work for sync and async clients.
- [ ] Mounted and custom transports observe auth-modified requests.
- [ ] Hook/auth/transport order matches committed HTTPX reference observations.
- [ ] Intermediate responses do not leak sockets or body streams.
- [ ] One-shot bodies fail deterministically when an auth flow requires replay.
- [ ] Auth exceptions preserve the compatibility request context.

# Track F — Fix streaming context ownership and cleanup

## F1. Close yielded responses in context-manager cleanup

Replace empty `finally` blocks in sync and async `stream()` context managers with deterministic response cleanup.

Required behavior:

- exiting normally closes/discards unread body data;
- exiting after partial consumption closes the response;
- exceptions in the user block still close the response;
- cancellation of an async context closes or aborts the stream task;
- no implicit full drain is performed unless reference behavior requires it;
- the underlying connection is reused or discarded according to protocol safety.

## F2. Make cleanup idempotent

Repeated `close()` or `aclose()` calls must not panic, block indefinitely, double-release permits, or resurrect closed streams.

## F3. Verify resource release

Add integration tests with observable socket and task state. Do not rely only on Python object flags.

Measure:

- server-observed connection closure;
- pool permit release;
- thread count stabilization;
- task count stabilization;
- file descriptor stabilization on supported Unix runners;
- clean interpreter shutdown after unconsumed streams.

### Track F acceptance criteria

- [ ] Sync `with client.stream(...)` closes the response on every exit path.
- [ ] Async `async with client.stream(...)` closes the response on every exit path.
- [ ] The response reports closed state after context exit.
- [ ] An unread response does not retain a pool permit after context exit.
- [ ] Context exit during exceptions and cancellation releases resources.
- [ ] Repeated close/aclose calls are safe and idempotent.
- [ ] Socket-level tests prove unsafe unread connections are not returned to the reusable pool.
- [ ] No thread, task, FD, or RSS growth exceeds committed thresholds after repeated early exits.

# Track G — Preserve compatibility data and merge semantics

## G1. Fix extension merging

Client-level extensions must be preserved when request-level extensions are omitted. When both are present, request values override matching client keys while unrelated client keys remain.

Verify this across:

- `build_request()`;
- `send()`;
- native dispatch;
- streaming dispatch;
- custom transport;
- mounted transport;
- sync and async clients;
- response extension propagation.

## G2. Preserve repeated query parameters

Do not convert `QueryParams` to a dictionary when doing so collapses repeated keys. Carry an ordered multi-value representation or encode the complete query into the URL before native dispatch.

Test:

```text
?a=1&a=2&b=3
```

including ordering, blank values, bytes/Unicode encoding, client/request merging, and redirects.

## G3. Preserve duplicate headers where HTTP semantics permit them

Avoid dictionary conversion for compatibility `Headers` where duplicate raw header fields are significant. Preserve case-insensitive lookup while retaining raw ordered pairs for transport serialization.

Include tests for:

- duplicate `Set-Cookie` response fields;
- duplicate request headers allowed by HTTP;
- comma-joinable versus non-joinable fields;
- custom transports observing raw duplicates;
- native transport wire behavior.

## G4. Audit other lossy conversion boundaries

Review conversions for:

- cookies with domain/path distinctions;
- URL userinfo and percent encoding;
- multipart tuple forms and per-part headers;
- request extensions;
- response history request attachment;
- reason phrase and HTTP version extension types.

### Track G acceptance criteria

- [ ] Client extensions survive when request extensions are omitted.
- [ ] Request extensions override only matching client keys.
- [ ] Repeated query keys reach the server in the correct order and multiplicity.
- [ ] Query merging matches HTTPX 0.28.1 differential cases.
- [ ] Duplicate raw headers survive supported transport paths.
- [ ] Multiple `Set-Cookie` fields remain independently observable.
- [ ] No compatibility conversion uses a plain dictionary when the source abstraction permits duplicates and ordering that matter.
- [ ] Differential tests cover every corrected conversion boundary.

# Track H — Close remaining production-semantics gaps

## H1. Complete Phase 1 acceptance criteria

Resolve or explicitly reclassify every incomplete item in `plans/httpx-drop-in-phase-1-status.md`, including:

- slow proxy CONNECT response classification;
- real TLS-handshake timeout integration coverage;
- socket-level assertions after client and stream context exit;
- close/request race leak assertions;
- interpreter shutdown on Linux, macOS, and Windows;
- retained scheduled soak evidence;
- non-Linux resource measurement policy;
- exact green required-CI run linkage.

## H2. Add retained soak workflow

Add a scheduled or manually dispatched soak workflow that:

- runs against an exact commit SHA;
- covers keep-alive churn, failure churn, streaming early-close, concurrent clients, retries, redirects, proxy paths, and interpreter shutdown;
- emits machine-readable metrics;
- fails committed thresholds;
- retains artifacts for the release-review period;
- does not run as part of every pull request unless runtime is acceptable.

## H3. Harden flaky-test policy

Do not relax correctness assertions merely to accommodate slow CI. If scheduling tolerance is needed, separate timing tolerance from semantic success. A concurrency test may allow variable completion order, but it must not accept fewer successful operations than the contract requires.

### Track H acceptance criteria

- [ ] Phase 1 status contains no incomplete release-blocking criteria.
- [ ] Slow proxy CONNECT response has a deterministic timeout classification test.
- [ ] Real TLS handshake timeout behavior is tested through a real local connector path.
- [ ] Interpreter-shutdown tests pass on Linux, macOS, and Windows.
- [ ] Close/race tests include FD, task, socket, or RSS leak assertions as applicable.
- [ ] A retained soak artifact exists for the final candidate SHA.
- [ ] Soak results are machine-readable and threshold-enforced.
- [ ] No required test is weakened to permit partial success.

# Track I — Restore CI and governance consistency

## I1. Reaffirm the required merge gate

Choose and document one policy. For this compatibility release, the required policy is:

- pull requests and `main` commits must pass the stable `Required CI Gate`;
- the compatibility job is part of that gate;
- required jobs cannot be informational;
- administrators are not silently exempt unless an explicit repository policy says otherwise;
- release qualification requires the exact candidate's green gate.

Update contradictory documentation introduced by commits that describe CI as informational.

## I2. Add corrective jobs to the gate

The required compatibility job must execute:

- profile validation;
- facade API manifest comparison;
- mandatory behavior differential corpus;
- streaming lifecycle tests;
- auth-flow integration tests;
- substitution-harness meta-tests;
- required clean-environment downstream suites that fit normal CI duration;
- evidence-generator consistency tests.

Long downstream suites and soak jobs may run in a separate qualification workflow, but Stage C release must depend on their successful exact-SHA result.

## I3. Add negative tests for evidence tools

Evidence tooling must fail on:

- wrong candidate package;
- upstream HTTPX present in substitution environment;
- missing downstream suite;
- zero tests collected;
- unexpected skip;
- stale SHA;
- mismatched package versions;
- missing artifact;
- placeholder values such as `[N]`;
- an allowed difference marked resolved while describing unsupported behavior;
- `overall_pass=true` with any required failed/unavailable result.

### Track I acceptance criteria

- [ ] Repository documentation and actual branch/ruleset behavior agree that `Required CI Gate` is mandatory.
- [ ] The required gate includes the repaired compatibility job.
- [ ] A failing manifest comparison makes the aggregate gate fail.
- [ ] A failing required downstream suite makes qualification fail.
- [ ] Evidence-generator negative tests cover every listed malformed state.
- [ ] Branch/ruleset evidence records the exact required check name.
- [ ] No release qualification relies on a commit merged while the exact candidate's required gate was red or absent.

# Track J — Regenerate coherent status and release evidence

## J1. Create one corrective implementation status file

Create:

```text
plans/httpx-drop-in-corrective-evidence-and-semantics-closure-status.md
```

It must include:

- starting SHA;
- final candidate SHA;
- implementation commits;
- changed files by track;
- commands executed;
- test counts;
- CI and qualification run links;
- artifact names and hashes;
- acceptance-criterion mapping;
- remaining intentional differences;
- blockers, if any;
- final stage decision.

## J2. Reconcile earlier phase status files

Do not rewrite history as though earlier claims were always correct. Add a correction notice to affected phase status files identifying:

- which criteria were overstated;
- which corrective commit closed them;
- which evidence supersedes the old evidence;
- whether the phase is now complete.

Remove all placeholders and contradictions.

## J3. Generate evidence from actual results

The final evidence generator must consume machine-readable result files produced by actual commands. It must not infer success merely because a test file or script exists.

The final bundle must include:

- candidate SHA;
- compatibility and shim manifest hashes;
- behavior differential results;
- required downstream results with collected test counts;
- substitution environment package inventories;
- performance results;
- resource and soak results;
- package artifact hashes;
- CI gate result;
- immutable dry-run result;
- allowed differences;
- overall decision.

## J4. Enforce evidence consistency

`overall_pass` may be true only when:

- all required fields are present;
- every required test category passed;
- all required downstream entries ran and passed;
- no required result is skipped, unavailable, missing, or stale;
- all evidence references the same candidate SHA;
- package and artifact versions match;
- no unresolved placeholder exists;
- the compatibility profile claim matches the approved stage.

### Track J acceptance criteria

- [ ] The corrective status file maps every criterion in this plan to evidence.
- [ ] Earlier phase status files contain correction notices where required.
- [ ] No current document contains unresolved `[N]`, TODO, pending, or not-run fields while claiming release completion.
- [ ] Final evidence is generated from actual result files rather than repository presence checks.
- [ ] All evidence records reference one exact candidate SHA.
- [ ] `overall_pass` becomes false under every negative consistency fixture.
- [ ] The final stage decision is mechanically derived from passed stage requirements.

# Track K — Immutable release requalification

## K1. Freeze the corrective candidate

After implementation is complete and required CI is green:

1. identify the exact full candidate SHA;
2. confirm the working tree and branch contain no later release-relevant changes;
3. create an immutable non-publishing validation ref according to repository policy;
4. dispatch qualification workflows against that exact SHA;
5. record all run IDs and attempts.

## K2. Build and test installable artifacts

Use built wheels and source distributions, not editable installs, for final compatibility qualification.

Required artifact matrix must match the documented support policy for:

- Python 3.10 through 3.13;
- Linux x86_64 and other claimed Linux targets;
- macOS x86_64 and arm64 where claimed;
- Windows x86_64;
- source distribution;
- controlled shim artifact where applicable.

Each artifact must pass:

- content validation;
- metadata validation;
- isolated installation;
- runtime diagnostics;
- ordinary request smoke tests;
- compatibility import smoke tests;
- sync and asyncio streaming smoke tests;
- exception-context smoke tests;
- clean interpreter shutdown.

## K3. Run final downstream and soak qualification

Run the full required downstream portfolio and retained soak workflow against the exact built candidate artifacts. Source-tree or editable-install success is insufficient.

## K4. Restore the stage claim only after proof

Only after every final criterion passes may one final documentation commit change the profile to `released`. If that documentation commit changes release-relevant files after the qualified candidate, either:

- include the claim change in the candidate before qualification; or
- rerun qualification against the new exact SHA.

There must be no unqualified post-candidate release commit.

### Track K acceptance criteria

- [ ] One immutable candidate SHA is used for CI, artifacts, downstream suites, soak tests, and dry run.
- [ ] The exact candidate has a green `Required CI Gate`.
- [ ] All claimed wheels and the sdist install and pass smoke tests in clean environments.
- [ ] The controlled shim artifact passes identity and substitution tests without upstream HTTPX installed.
- [ ] All required downstream suites pass against built candidate artifacts.
- [ ] The retained soak run passes against the same candidate.
- [ ] The dry-run workflow is non-publishing and records no side effects.
- [ ] Artifact hashes and evidence manifest hashes are retained.
- [ ] No release-relevant commit exists after the qualified SHA without a new qualification run.
- [ ] `Stage C released` is restored only if every Stage C requirement and every criterion in this plan passes.

# Required test matrix

The implementation must add or retain coverage in the following matrix.

| Area | Sync | Asyncio | Native transport | Mock/custom transport | Wheel install | Differential |
|------|------|---------|------------------|-----------------------|---------------|--------------|
| API/signatures | yes | yes | n/a | n/a | yes | yes |
| Timeout phases | yes | yes | yes | where applicable | yes | yes |
| Auth flows | yes | yes | yes | yes | yes | yes |
| Streaming context cleanup | yes | yes | yes | yes | yes | yes |
| Repeated params/headers | yes | yes | yes | yes | yes | yes |
| Extensions | yes | yes | yes | yes | yes | yes |
| Downstream substitution | yes | yes | as used | as used | yes | reference comparison |
| Shutdown/resources | yes | yes | yes | yes | yes | no |

Every required test must have a stable ID or directly map to an acceptance criterion.

# Recommended implementation sequence

Execute in this order:

1. Track A — downgrade claims and invalidate stale evidence.
2. Track B — repair the compatibility oracle.
3. Track C — rebuild substitution validation and packaging truth.
4. Tracks D through G — fix timeout, auth, streaming, and conversion semantics.
5. Track H — close production-semantics and soak gaps.
6. Track I — wire all required validation into CI and restore governance consistency.
7. Track J — regenerate coherent status and evidence.
8. Track K — run immutable release requalification.

Tracks D, E, F, and G may be implemented in parallel after Tracks A through C establish trustworthy tests. Tracks J and K must remain last.

# Completion gate

This corrective pass is complete only when all of the following are true:

- [ ] The compatibility oracle inspects the facade and fails closed.
- [ ] Downstream substitution environments contain no upstream HTTPX.
- [ ] Required downstream suites actually run against eggfetch-backed `import httpx`.
- [ ] Missing, skipped, or zero-test downstream entries cannot produce an overall pass.
- [ ] Compatibility defaults do not impose an artificial total timeout.
- [ ] Multi-step and custom auth flows dispatch every yielded request.
- [ ] Auth works through native, mounted, mock, and custom transports.
- [ ] Streaming context managers deterministically close yielded responses.
- [ ] Extensions, repeated query values, and duplicate headers are preserved.
- [ ] Remaining Phase 1 resource, shutdown, and soak criteria are closed.
- [ ] Required CI and documentation describe the same merge policy.
- [ ] Status files and generated evidence contain no contradictions or placeholders.
- [ ] One exact candidate SHA has green CI, artifact, downstream, soak, and dry-run evidence.
- [ ] The final compatibility stage claim does not exceed the retained proof.

If any criterion remains blocked, the implementation status must state the blocker and leave the profile at `candidate` or the appropriate lower stage. Partial completion must not be relabeled as released.
