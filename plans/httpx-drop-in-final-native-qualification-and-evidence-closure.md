# HTTPX Drop-In Final Native Qualification and Evidence Closure

Status: ready for implementation handoff

## Purpose

Close the remaining release-blocking defects after the verification, substitution, and lifecycle corrective pass without reopening the broader HTTPX compatibility roadmap.

The repository now contains a credible Stage C candidate implementation, but it does not yet contain trustworthy release qualification. The remaining work is concentrated in six areas:

1. API-oracle precision and allowed-difference governance;
2. real, pinned downstream behavioral suites;
3. executable exact-SHA qualification and evidence production;
4. complete async authentication and intermediate-response ownership;
5. lossless client/request merge semantics;
6. native transport lifecycle, timeout, shutdown, and soak proof.

This pass is complete only when one exact candidate SHA can be qualified from built artifacts with retained, internally consistent evidence. Increased test count, a green ordinary CI run, or the presence of a qualification workflow is not sufficient by itself.

## Audited baseline

Implementation begins from:

- repository: `eggstack/eggfetch`
- branch: `main`
- audited baseline SHA: `48622d47830bba68e0cc62d3ed70a308114b573c`
- reference distribution: `httpx==0.28.1`
- facade import: `eggfetch.compat.httpx`
- controlled replacement source: `compat/httpx-controlled-replacement/`
- previous implementation plan: `plans/httpx-drop-in-verification-substitution-and-lifecycle-corrective-closure.md`
- previous implementation status: `plans/httpx-drop-in-verification-substitution-and-lifecycle-corrective-closure-status.md`

Before editing code, record the actual implementation-start SHA. If `main` has advanced, review every intervening commit and update the implementation status with whether each finding in this plan still applies.

## Current bounded claim

The only justified current claim is:

> Stage C candidate: core and facade behavior are substantially implemented and ordinary CI is green, but downstream substitution, native lifecycle proof, and release qualification are not yet complete.

Do not restore `Stage C released` until every final gate in this plan passes against one exact candidate SHA.

## Findings this pass must close

The implementation agent must treat the following as release-blocking:

1. The API comparator matches allowed differences primarily by symbol and glob pattern rather than exact symbol plus difference type.
2. Comparator normalization can erase meaningful variadic and parameter-kind differences.
3. Allowed-difference records are not validated strongly enough for duplicate IDs, broad patterns, expiry, exact difference type, and false `resolved` classifications.
4. Required downstream entries are mostly import or construction smoke checks rather than real behavioral suites.
5. Downstream packages are not installed from the exact manifest version/source and are not verified by artifact hash or commit SHA.
6. Required downstream entries can use `min-tests = 0` or otherwise pass without meaningful behavioral execution.
7. Unknown packages, empty selections, skipped required suites, and other no-execution paths can still produce successful results.
8. The qualification matrix omits several packages marked required and includes informational entries in their place.
9. Qualification invokes pytest JSON reporting without installing its plugin.
10. Qualification invokes runner arguments that the runner does not implement.
11. Evidence generation points at a wheel directory that does not contain both candidate artifacts.
12. Qualification suppresses command failures with `|| true` and substitutes fallback JSON instead of retaining the real failed command result.
13. Artifact-hash verification searches paths that do not match downloaded artifact layout.
14. Evidence generation may emit `overall_pass=false` and still exit successfully.
15. The qualification gate checks job success but does not independently validate the evidence decision and schema.
16. The qualification summary upload path does not correspond to a file that is actually generated.
17. `AsyncClient` drives only synchronous `auth_flow()` generators and does not implement the measured async auth contract.
18. Intermediate auth responses are not drained or closed before follow-up dispatch.
19. One-shot sync and async request bodies are not deterministically rejected when auth replay is required.
20. Request/client query merging collapses repeated values.
21. Header update/merge behavior collapses duplicate incoming headers.
22. Proxy CONNECT and TLS timeout tests primarily inject exceptions through mock transports rather than exercising native local sockets.
23. Resource and shutdown tests primarily use mock transports and explicit close paths.
24. The scheduled “soak” is a one-shot test bundle rather than retained native churn evidence.
25. The current corrective status file names an obsolete candidate SHA and overstates implementation closure.

## Scope constraints

This pass must remain narrowly scoped.

It may:

- refine manifest generation, comparison, and allowed-difference validation;
- revise the downstream manifest schema and isolated runner;
- acquire pinned downstream wheels, sdists, or repository commits for controlled tests;
- add exact behavioral tests required to cover the existing Stage C claim;
- repair the qualification workflow and evidence schema;
- implement measured sync/async auth-driving behavior and response ownership;
- correct header/query merge semantics;
- add local native TCP, proxy, TLS, lifecycle, shutdown, resource, and soak fixtures;
- correct documentation and status artifacts.

It must not:

- add Trio support;
- add general AnyIO backend selection;
- add SOCKS support;
- add another HTTPX reference version;
- add unrelated protocol features;
- replace the Rust networking engine with httpcore or Python networking;
- broaden public claims beyond measured Stage C behavior;
- publish the controlled `httpx` replacement wheel to public package indexes;
- convert required failures into warnings, skips, xfails, or informational results;
- count import-only checks as downstream behavioral coverage;
- treat mock-injected exceptions as proof of native timeout classification;
- restore a release claim before exact-SHA qualification completes.

## Cross-cutting invariants

The following rules apply to every track:

- All final qualification uses built wheel or sdist artifacts, never `maturin develop`, editable installs, or source-tree imports.
- Reference HTTPX and replacement HTTPX environments remain separate.
- Upstream `httpx` may exist only in the reference environment.
- Every required result records the same full 40-character candidate SHA.
- Every required result records the SHA-256 of the exact eggfetch and controlled replacement wheels it used.
- Missing, malformed, skipped, unavailable, stale, zero-test, hash-mismatched, or inconsistent evidence fails closed.
- A command failure must be represented as a failed result; workflows must not replace it with fabricated success-shaped fallback data.
- A generated report is not evidence unless it consumes retained result files from commands that actually ran.
- Test fixtures must distinguish facade behavior, controlled top-level replacement behavior, and upstream reference behavior.
- Status documents must describe blockers accurately and may not call this pass complete while a required criterion is pending.

# Track 0 — Reset status and define one candidate lineage

## 0.1 Correct the current status before implementation

Update `plans/httpx-drop-in-verification-substitution-and-lifecycle-corrective-closure-status.md` so it:

- names `48622d47830bba68e0cc62d3ed70a308114b573c` as the audited baseline for this follow-up;
- records that the prior pass materially improved the implementation but did not close qualification;
- removes claims that all tracks are complete;
- lists the remaining six work areas from this plan;
- retains `Stage C candidate`.

Do not rewrite history. Preserve prior test counts as historical observations, clearly labeled with the SHA that produced them.

## 0.2 Establish candidate identity fields

Define one reusable candidate identity schema for all generated JSON artifacts:

```json
{
  "schema_version": "3",
  "candidate_sha": "<40-char SHA>",
  "eggfetch_version": "<version>",
  "eggfetch_wheel": {
    "filename": "...",
    "sha256": "..."
  },
  "httpx_replacement_wheel": {
    "filename": "...",
    "sha256": "..."
  },
  "reference_httpx_version": "0.28.1",
  "producer": "<job/script name>",
  "started_at": "<UTC ISO-8601>",
  "finished_at": "<UTC ISO-8601>"
}
```

Create a small shared Python module if needed so scripts do not implement inconsistent validation independently.

## Track 0 acceptance criteria

- [ ] Previous status is corrected without erasing historical evidence.
- [ ] Current claim remains Stage C candidate.
- [ ] All new result schemas use one exact candidate identity format.
- [ ] Candidate SHA and both wheel hashes are mandatory for required result artifacts.
- [ ] Schema validation rejects missing, malformed, placeholder, or inconsistent identity fields.

# Track 1 — Make the API oracle exact, typed, and fail-closed

## 1.1 Introduce typed difference records

Refactor `scripts/compare_httpx_api_manifest.py` so every discovered difference has a stable typed record, for example:

```json
{
  "symbol": "Client",
  "difference_type": "parameter-kind",
  "member": "__init__.follow_redirects",
  "reference": "KEYWORD_ONLY",
  "candidate": "POSITIONAL_OR_KEYWORD"
}
```

Required difference types must include at least:

- `missing-symbol`;
- `extra-symbol`;
- `symbol-kind`;
- `parameter-name`;
- `parameter-kind`;
- `parameter-requiredness`;
- `parameter-default`;
- `variadic-shape`;
- `return-annotation`;
- `base-class`;
- `missing-property`;
- `extra-property`;
- `missing-method`;
- `extra-method`;
- `sync-async-kind`.

Do not represent multiple unrelated mismatches as one untyped string.

## 1.2 Remove semantic-erasing normalization

Remove the normalization that drops a leading candidate `*args` merely because the reference lists explicit parameters.

Compare all parameter dimensions:

- order;
- name;
- positional-only, positional-or-keyword, keyword-only, var-positional, and var-keyword kind;
- required versus optional;
- default value;
- annotation where part of the public contract.

Normalization may still canonicalize representationally equivalent values such as:

- `None`, `"None"`, and quoted `None` annotations;
- stable module qualification differences in repr strings;
- deterministic map/list ordering where order is not semantically meaningful;
- opaque sentinel reprs mapped by an explicit known-sentinel table.

Every normalization rule must have a positive and negative unit test.

## 1.3 Replace broad symbol-glob waivers

Revise `compat/httpx/0.28.1/allowed-differences.toml` and its loader so each record matches:

- exact stable ID;
- exact symbol;
- exact member, when applicable;
- exact `difference-type`;
- exact reference value or an explicit canonical value;
- exact candidate value or an explicit canonical value;
- stage impact;
- owner;
- tests;
- review milestone;
- expiry or explicit no-expiry rationale.

Do not permit `*Error`, `Client*`, or other broad patterns in required Stage C validation.

If a small group of symbols genuinely shares one policy decision, duplicate the records with unique IDs rather than using a wildcard.

## 1.4 Add strict allowed-difference schema validation

Add a dedicated validator or integrate validation into the comparator. It must reject:

- duplicate IDs;
- duplicate records for the same exact difference;
- missing required fields;
- unknown categories;
- unknown difference types;
- wildcard symbols or members;
- expired entries;
- an allowed entry that matches zero differences;
- an allowed entry that matches more than one difference;
- a `resolved` entry that still matches a current difference;
- a compatibility-equal behavior recorded as intentional;
- tests that reference nonexistent files or test nodes, where practical.

`resolved` entries should normally be removed from the active allowed-difference file and retained in status/history instead. If retained, the validator must prove they match no current difference.

## 1.5 Compare both facade and installed replacement

Required CI and qualification must perform two separate comparisons against `httpx==0.28.1`:

1. reference versus `eggfetch.compat.httpx`;
2. reference versus the installed controlled top-level `httpx` replacement.

The top-level replacement comparison must run inside a clean environment containing the candidate eggfetch wheel and controlled replacement wheel, with no upstream HTTPX distribution installed.

Both comparisons must emit structured JSON and fail nonzero on:

- unexplained differences;
- stale allowed differences;
- invalid allowed-difference schema;
- candidate identity mismatch;
- wrong imported distribution identity.

## 1.6 Complete negative-oracle coverage

Add negative fixtures proving nonzero exit for:

- removed `Client`;
- removed `AsyncClient`;
- keyword-only parameter changed to positional-or-keyword;
- positional-only parameter changed to positional-or-keyword;
- required parameter made optional;
- optional parameter made required;
- explicit parameter replaced by `*args` or `**kwargs`;
- changed `follow_redirects` default;
- removed exception base class;
- property replaced by method;
- sync callable replaced by async or vice versa;
- top-level replacement exporting a non-eggfetch `Client`;
- duplicate allowed-difference ID;
- wildcard allowed-difference symbol;
- allowed entry matching multiple differences;
- expired entry;
- falsely resolved entry;
- stale entry matching no current difference.

## Track 1 acceptance criteria

- [ ] Comparator emits typed, stable difference records.
- [ ] Parameter kinds, requiredness, order, defaults, and variadic shape are compared.
- [ ] No normalization removes a meaningful compatibility distinction.
- [ ] Active allowed differences match one exact typed difference each.
- [ ] Wildcard matching is prohibited in required validation.
- [ ] Duplicate, broad, stale, expired, multi-match, and falsely resolved records fail validation.
- [ ] Required CI compares both facade and installed controlled replacement.
- [ ] Both oracle result files record the exact candidate SHA and wheel hashes.
- [ ] Every negative fixture exits nonzero.
- [ ] A comparator failure makes the aggregate required CI gate fail.

# Track 2 — Replace downstream smoke checks with pinned behavioral suites

## 2.1 Upgrade the downstream manifest to schema version 2

Revise `compat/downstream/manifest.toml` so every entry includes:

- package name;
- exact package version;
- `usage = "required" | "informational"`;
- source type: `wheel`, `sdist`, or `git`;
- exact source locator;
- source SHA-256 or git commit SHA;
- Python versions supported by this portfolio entry;
- public versus private HTTPX API dependence;
- exact install command or machine-readable install policy;
- exact test working directory;
- exact test command as an argument array where possible;
- test result format: pytest JSON, JUnit XML, unittest, or custom JSON;
- minimum collected count;
- maximum skipped count;
- maximum xfailed count;
- timeout;
- network policy;
- category IDs covered;
- known incompatibilities;
- release-blocking classification;
- update owner and review cadence.

Do not use shell strings unless required by the upstream suite. Prefer argument arrays to avoid quoting ambiguity.

## 2.2 Reclassify import-only entries

Any entry whose command only imports a package, constructs an object, or prints a version must be informational.

In particular, do not count the existing import checks for SDKs, auth extensions, websocket extensions, or testing libraries as behavioral coverage.

A required entry must execute behavior that invokes the eggfetch-backed HTTPX surface and assert an outcome.

## 2.3 Build a minimum real Stage C portfolio

Retain at least one required, passing behavioral suite for each category:

1. sync SDK/client behavior;
2. asyncio SDK/client behavior;
3. ASGI test-client behavior;
4. mock transport and request matching;
5. streaming or SSE consumption;
6. custom auth flow;
7. event hooks or instrumentation;
8. custom or mounted transport behavior.

Preferred approach:

- use a pinned upstream sdist or repository commit when the published wheel omits tests;
- execute a stable, narrowly selected upstream test subset that uses public HTTPX APIs;
- record the upstream test node IDs or selection expression in the manifest;
- patch only test configuration needed for offline execution, and record the patch hash;
- do not patch assertions to accommodate eggfetch behavior.

Where no stable upstream suite is available, add a repository-owned downstream contract fixture that imports the real downstream package and executes its actual client/auth/stream/transport integration. Such a fixture must still be tied to an exact downstream version and must not reduce to an import test.

## 2.4 Pin source acquisition

Modify `scripts/run_isolated_downstream.py` to acquire exactly the declared source.

For package-index sources:

- install `name==version`;
- use `--no-deps` for the source artifact when dependency installation is handled separately;
- download first;
- verify SHA-256 before installation;
- reject a filename/version mismatch.

For git sources:

- clone or fetch only the declared commit;
- verify `HEAD` equals the manifest SHA;
- disable submodule or network expansion unless explicitly declared;
- retain the repository commit in the result artifact.

Do not install `pkg["name"]` without the exact version.

## 2.5 Control dependency installation

The isolated sequence must be:

1. create a clean venv;
2. install the exact candidate eggfetch wheel with `--no-deps` if necessary;
3. install the exact controlled replacement wheel;
4. verify replacement distribution identity and both wheel hashes;
5. install declared non-HTTPX runtime/test dependencies;
6. install the exact downstream source without allowing upstream HTTPX replacement;
7. run `pip check`;
8. re-verify controlled replacement identity and distribution inventory;
9. execute the exact test command;
10. parse structured results;
11. write one result JSON file even on failure;
12. remove the environment unless retention was explicitly requested.

Use a constraints file or local wheelhouse so the resolver cannot replace controlled `httpx` with an upstream artifact.

After installation, reject any environment containing:

- more than one distribution named `httpx`;
- an `httpx` distribution whose direct URL/hash is not the controlled wheel;
- an imported `httpx` without `__eggfetch_shim__ is True`;
- `Client` or `AsyncClient` not originating from eggfetch compatibility code.

## 2.6 Make the runner fail closed

Required runner behavior:

- unknown package name: exit 2;
- selected package list resolves to zero entries: exit 2;
- required entry with no command: exit 2;
- source missing or hash mismatch: exit 1;
- dependency resolution replacing controlled HTTPX: exit 1;
- `pip check` failure: exit 1;
- zero collected tests for a required entry: exit 1;
- collected count below minimum: exit 1;
- skips above budget: exit 1;
- unexpected xfail/xpass: exit 1;
- timeout: exit 1;
- malformed result file: exit 1;
- import-only required entry: manifest validation failure;
- a required entry marked skipped for any reason: aggregate failure.

Remove successful `skipped-no-tests` behavior for required entries.

## 2.7 Add `--output` and deterministic result files

Implement an explicit `--output <path>` argument in both downstream scripts.

The isolated runner must write a result file containing:

- candidate identity;
- downstream source identity and hash;
- exact commands;
- environment inventory;
- pre/post shim verification;
- `pip check` output;
- collected/passed/failed/error/skipped/xfail/xpass counts;
- duration;
- final status;
- failure reason;
- stdout/stderr paths or bounded embedded excerpts.

The aggregate runner must consume result files, not scrape interleaved stdout.

## 2.8 Add false-green meta-tests

Add tests proving aggregate failure for:

- unknown package;
- empty package selection;
- missing source;
- wrong source hash;
- unpinned source;
- missing command;
- import-only required command;
- zero tests;
- below-minimum count;
- skipped required suite;
- xfail above budget;
- timeout;
- malformed JSON;
- missing result file;
- upstream HTTPX installed after dependencies;
- wrong controlled wheel hash;
- candidate SHA mismatch;
- required category not covered by any behavioral suite.

## Track 2 acceptance criteria

- [ ] Manifest schema version 2 contains exact source, hash, command, budget, and category data.
- [ ] Every required entry executes meaningful HTTPX-dependent behavior.
- [ ] Import-only entries are informational.
- [ ] Every required source is pinned by version plus hash or exact git SHA.
- [ ] Runner installs the exact declared source.
- [ ] Controlled HTTPX identity is verified before and after dependency installation.
- [ ] `pip check` passes without upstream HTTPX installed.
- [ ] Unknown, empty, skipped, zero-test, malformed, stale, and hash-mismatch paths fail closed.
- [ ] All eight required Stage C categories have real behavioral coverage.
- [ ] Aggregate `overall_pass` is true only when every required entry and category passes.
- [ ] All false-green meta-tests pass.

# Track 3 — Make qualification executable, artifact-driven, and decisive

## 3.1 Use one exact candidate artifact set

The qualification workflow must:

- verify the exact candidate SHA has a successful `Required CI Gate`;
- check out that exact SHA in every source-reading job;
- build eggfetch wheels and the controlled replacement wheel once per needed platform/Python ABI;
- compute SHA-256 immediately after build;
- upload an artifact manifest containing filenames, hashes, candidate SHA, and build job identity;
- make all test jobs download artifacts from that manifest;
- prohibit rebuilding candidate code inside consumer/evidence jobs.

## 3.2 Repair workflow dependencies and tools

Install every CLI/plugin actually used, including at least:

- `pytest`;
- `pytest-asyncio`;
- `pytest-timeout`;
- `pytest-json-report`, when JSON reporting remains the selected format;
- build tooling required for the controlled wheel;
- any TOML compatibility dependency for Python 3.10.

Pin workflow test tooling to bounded versions in a qualification requirements file rather than ad hoc `pip install` commands.

Add a workflow-lint test that validates:

- every referenced runner argument exists;
- every pytest option has a declared plugin dependency;
- every artifact name downloaded is produced by exactly one upstream job;
- every job using `needs.verify.outputs` declares `verify` in `needs`;
- every required matrix entry appears in the downstream manifest as required;
- informational entries are not substituted for missing required categories.

## 3.3 Stop suppressing failures

Remove `|| true` from required test, downstream, comparator, and evidence commands.

When a command must always produce a result artifact:

1. run it through a small wrapper;
2. capture its real exit code;
3. require the command itself to write structured output, or write a failure envelope containing the real exit code and stderr;
4. upload the result under `if: always()`;
5. exit with the original nonzero code after artifact creation.

Do not fabricate zero-failure fallback objects.

## 3.4 Consume job artifacts rather than rerun suites in evidence

The evidence job should download:

- all compatibility matrix results;
- both facade and top-level oracle results;
- all required downstream result files;
- shim identity result;
- wheel/sdist smoke results;
- native timeout result;
- shutdown result;
- resource result;
- soak result;
- artifact manifest;
- exact CI gate identity.

It must not rerun compatibility or downstream suites merely to create evidence.

This ensures the evidence describes the jobs that actually gated qualification.

## 3.5 Fix artifact hash verification

Artifact verification must use explicit paths from the downloaded artifact manifest.

Do not search guessed directories such as `target/wheels`, `dist`, or repository root.

For each artifact:

- assert the file exists;
- assert filename matches the manifest;
- recompute SHA-256;
- assert it matches the build manifest;
- assert candidate SHA matches all result files;
- assert the artifact was downloaded from the current workflow run, not inherited from an older run.

## 3.6 Make evidence generation fail on a negative decision

`scripts/generate_compatibility_evidence.py` must exit nonzero when:

- any required category fails;
- `overall_pass` would be false;
- a required result is absent;
- identity fields disagree;
- a result is skipped or unavailable;
- a hash does not match;
- an oracle contains unexplained or stale differences;
- a downstream category lacks coverage;
- evidence contains placeholders;
- candidate Git HEAD differs from the declared candidate, where source is checked out.

Writing a JSON file with `overall_pass=false` is useful diagnostics, but the command must still fail.

## 3.7 Validate evidence independently in the final gate

Add a separate `validate_compatibility_evidence.py` command used by the final qualification gate.

The final gate must download the generated evidence and independently assert:

- expected schema version;
- exact candidate SHA;
- exact artifact hashes;
- all required categories present;
- `overall_pass is true`;
- no placeholders;
- no failed, skipped, unavailable, or malformed result;
- all run IDs and URLs point to the current qualification run and exact CI run;
- qualification occurred after the required CI gate completed;
- no release-relevant commit exists after the candidate SHA in the qualified lineage.

The final job must not rely solely on `${{ needs.<job>.result }}`.

## 3.8 Produce a real qualification summary artifact

Write an explicit summary JSON and Markdown file containing:

- candidate identity;
- required CI run URL and attempt;
- qualification run URL and attempt;
- artifact names and hashes;
- matrix coverage;
- downstream package/source inventory;
- test counts and budgets;
- native lifecycle/soak metrics;
- final decision;
- remaining allowed differences by typed category.

Upload paths must correspond to files created in the workflow.

## Track 3 acceptance criteria

- [ ] Qualification verifies the exact candidate’s successful Required CI Gate.
- [ ] All jobs test downloaded candidate artifacts, not editable/source builds.
- [ ] Every command-line option used by workflows exists and is covered by a workflow-lint test.
- [ ] All pytest plugins used are installed from pinned qualification requirements.
- [ ] No required command uses `|| true` or success-shaped fallback JSON.
- [ ] Evidence consumes retained job results instead of rerunning suites.
- [ ] Artifact hashes are verified using explicit downloaded paths.
- [ ] Evidence generation exits nonzero whenever `overall_pass` is false.
- [ ] Final qualification independently validates evidence content.
- [ ] Qualification summary files are actually generated and retained.
- [ ] A deliberately failed downstream/oracle/lifecycle fixture makes the final qualification gate fail.

# Track 4 — Complete async auth and response ownership

## 4.1 Measure the HTTPX 0.28.1 auth contract

Before implementation, add focused differential observations for:

- `Auth.auth_flow`;
- `Auth.sync_auth_flow`;
- `Auth.async_auth_flow`;
- a subclass implementing only `auth_flow`;
- a subclass overriding `sync_auth_flow`;
- a subclass overriding `async_auth_flow`;
- multi-yield sync and async flows;
- request-body buffering before auth replay;
- intermediate response close/drain state;
- request and response hook ordering.

Commit observation fixtures and expected outputs. Do not infer async behavior from the sync implementation.

## 4.2 Implement separate sync and async drivers

Implement a sync auth driver used by `Client.send()` and an async auth driver used by `AsyncClient.send()`.

The async driver must:

- call the measured `async_auth_flow` contract when present;
- support an async generator or async iterator as required by the measured reference;
- await transitions correctly;
- use measured fallback behavior for subclasses that implement only `auth_flow`;
- never accidentally run an asynchronous auth flow through `next()` or synchronous `.send()`;
- preserve request context on exceptions.

Keep transport selection below auth transformation. Native, mounted, custom, mock, ASGI, and WSGI dispatch must all receive the auth-modified yielded request.

## 4.3 Own intermediate responses explicitly

For every nonfinal auth response:

- determine from measured HTTPX behavior whether it must be fully read, drained, or closed;
- perform the correct sync or async cleanup before dispatching the next request;
- do not call sync `close()` on a response that requires `await aclose()`;
- do not expose intermediate responses through final response hooks;
- retain final response history only if the reference does so;
- ensure pool permits and sockets are released.

Add a helper with explicit sync/async variants rather than broad `hasattr` cleanup where ownership differs.

## 4.4 Enforce replayability

When auth yields a follow-up request that reuses a body:

- replay immutable bytes and buffered content correctly;
- replay seekable files only according to measured behavior;
- reject consumed one-shot sync iterators;
- reject consumed one-shot async iterators;
- raise the matching compatibility exception with the original request attached;
- never silently send empty, truncated, or duplicate body data.

Tests must assert wire-observed body bytes on both requests.

## 4.5 Correct hook ordering

Match committed reference observations for:

- initial request hook;
- auth mutation;
- first dispatch;
- intermediate response handling;
- second auth mutation;
- second dispatch;
- final response hook.

Verify sync and async behavior separately.

## 4.6 Test all dispatch paths

Required tests:

- BasicAuth through native sync and async clients;
- BasicAuth through mounted and custom transports;
- DigestAuth real two-request challenge through a local native server;
- custom sync auth with three yields;
- custom async auth with three yields;
- ASGI transport auth;
- WSGI transport auth;
- per-request auth disable through every path;
- intermediate response cleanup under repeated challenges;
- sync one-shot body replay rejection;
- async one-shot body replay rejection;
- hook failure cleanup;
- auth failure request context.

## Track 4 acceptance criteria

- [ ] Reference auth behavior is committed as deterministic observations.
- [ ] Sync and async clients use separate auth drivers.
- [ ] Custom async multi-yield auth executes correctly.
- [ ] Auth remains active through native, mounted, custom, mock, ASGI, and WSGI paths.
- [ ] Intermediate responses are drained/closed using the correct sync or async operation.
- [ ] Repeated auth challenges do not leak pool permits, sockets, tasks, or file descriptors.
- [ ] One-shot sync and async bodies fail deterministically when replay is required.
- [ ] Replayable bodies are byte-identical across attempts.
- [ ] Hook ordering matches HTTPX observations.
- [ ] Auth exceptions retain request context.

# Track 5 — Make header and query merging lossless

## 5.1 Measure merge semantics

Add differential observations for HTTPX 0.28.1 covering:

- client query params plus request query params with overlapping keys;
- repeated client values overridden by repeated request values;
- ordering of nonoverlapping and overlapping keys;
- blank values;
- Unicode values;
- already percent-encoded input;
- client duplicate headers plus request duplicate headers;
- case-insensitive header replacement;
- list-of-tuples input versus `Headers` input;
- redirects preserving the final encoded query and headers as applicable.

## 5.2 Correct query merge behavior

`Client._merge_params()` and `AsyncClient._merge_params()` must not iterate unique keys and fetch only the final value.

Implement measured semantics that generally:

- preserve all client entries whose keys are not overridden;
- remove all client entries for keys present in request params;
- append all request entries for each overridden/new key in their original order;
- preserve duplicates, blank values, and Unicode;
- avoid double encoding.

Use `multi_items()` or an equivalent ordered multi-value representation throughout.

## 5.3 Correct header update behavior

`Headers.update()` and client header merging must preserve duplicate incoming values according to measured HTTPX behavior.

For each incoming header name:

- remove prior values according to case-insensitive replacement semantics;
- append every incoming duplicate value in incoming order;
- do not remove a just-appended duplicate while processing the next duplicate;
- retain unrelated existing headers and their relative order where the reference does.

Implement batch-by-key replacement rather than one tuple at a time.

## 5.4 Add wire-level native tests

Use a local native HTTP server that records raw request target and raw repeated header lines.

Required cases include:

```text
?a=1&a=2&b=&a=3
```

and duplicate headers such as:

```text
X-Tag: one
X-Tag: two
X-Other: keep
X-Tag: three
```

Test:

- direct request construction;
- client defaults merged with request values;
- sync native dispatch;
- async native dispatch;
- redirect follow-up where relevant;
- controlled top-level `httpx` replacement import.

## Track 5 acceptance criteria

- [ ] Merge behavior is based on committed HTTPX observations.
- [ ] Request query values preserve multiplicity and order after client-default merging.
- [ ] Duplicate incoming headers survive `Headers.update()` and client merging.
- [ ] Case-insensitive replacement matches HTTPX behavior.
- [ ] Blank, Unicode, and percent-encoded values are not corrupted or double encoded.
- [ ] Wire-level sync and async native tests observe exact expected query/header sequences.
- [ ] Facade and controlled top-level replacement pass the same merge tests.

# Track 6 — Prove native timeout, lifecycle, shutdown, and soak behavior

## 6.1 Build deterministic local network fixtures

Add reusable local fixtures that do not require external internet access:

- HTTP origin that delays response headers;
- HTTP origin that delays body chunks;
- origin that stops reading an upload body;
- connection pool fixture with a held connection;
- HTTP proxy that accepts TCP but stalls before CONNECT response;
- HTTP proxy that returns CONNECT success then stalls upstream TLS;
- bare TCP server that accepts but does not complete TLS handshake;
- server that closes during partial response;
- redirect chain fixture;
- streaming response fixture with observable connection closure.

Fixtures must expose synchronization barriers rather than relying only on sleeps. Every test must have a hard outer timeout.

## 6.2 Test native timeout classification

Using the real Rust-backed native transport, assert deterministic mapping for:

- TCP connect timeout;
- proxy CONNECT response timeout;
- TLS handshake timeout;
- response-header/read timeout;
- body-chunk read timeout;
- upload/write timeout;
- pool acquisition timeout.

Each test must assert:

- compatibility exception class;
- attached request;
- stable phase classification;
- elapsed-time bounds with platform-appropriate tolerance;
- connection/socket cleanup after failure.

MockTransport exception injection may remain as unit coverage but cannot satisfy this gate.

## 6.3 Add native response lifecycle tests

Exercise real local sockets for:

- fully consumed response;
- unread response closed by context exit;
- partially consumed response;
- exception inside stream context;
- async cancellation during body read;
- client close while response exists;
- response close followed by client close;
- repeated idempotent close/aclose;
- redirect history cleanup;
- intermediate auth response cleanup.

Observe:

- response closed state;
- server-side EOF or connection reset;
- pool permit release;
- ability to reuse safe connections;
- refusal to reuse unsafe unread connections;
- task/thread stabilization;
- Unix FD stabilization;
- Windows handle/thread policy where FD inspection is unavailable.

## 6.4 Strengthen interpreter-shutdown proof

Run subprocess tests for sync and async clients with:

- unused client, no explicit close;
- used client, no explicit close;
- unread response, no explicit close;
- partially read streaming response, no explicit close;
- active auth challenge sequence;
- pending/just-cancelled async read;
- explicit close race with request completion;
- context-managed normal paths.

Require:

- subprocess exits within a committed timeout;
- no fatal Python errors;
- no panic/backtrace;
- no “event loop is closed” noise;
- no lingering child process;
- platform-specific resource thresholds.

Run at least Linux, macOS, and Windows variants in qualification. Record unsupported measurements explicitly, but do not skip the entire shutdown scenario.

## 6.5 Define short qualification churn and retained weekly soak

Use two levels:

### Qualification churn

A bounded 5–10 minute exact-SHA job that runs:

- repeated sync and async native requests;
- keep-alive reuse;
- redirects;
- streaming early exits;
- cancellations;
- auth challenges;
- pool contention;
- proxy and TLS timeout failures;
- client open/close cycles.

### Weekly retained soak

A 30–60 minute scheduled run against current `main` that records:

- request count;
- success/failure count by expected category;
- latency percentiles;
- FD/handle baseline, peak, and final values;
- RSS baseline, peak, and final values;
- thread/task baseline, peak, and final values;
- pool state where observable;
- server-side accepted/closed connection counts;
- unexpected exceptions or hangs.

The weekly run is not sufficient for a release candidate unless the exact candidate also passes qualification churn. A recent weekly soak may supplement, not replace, exact-SHA proof.

## 6.6 Create explicit resource thresholds

Store thresholds in a versioned configuration file, separated by platform where needed.

Thresholds must define:

- allowable final FD/handle delta;
- allowable thread/task delta;
- allowable retained RSS growth after warm-up;
- maximum timeout overshoot;
- maximum hung-operation count;
- minimum completed operation count;
- expected server-side connection closure count.

Do not use only `ru_maxrss` as a final-memory measure because it cannot demonstrate post-workload recovery. Use a current-RSS mechanism appropriate to the platform, with a documented fallback.

## 6.7 Retain structured native evidence

Native lifecycle and soak jobs must emit JSON containing candidate identity, platform, workload parameters, thresholds, measurements, and pass/fail decision.

Upload:

- raw JSON;
- concise Markdown summary;
- relevant server logs;
- failure diagnostics;
- process/thread dumps on timeout where practical.

## Track 6 acceptance criteria

- [ ] Proxy CONNECT and TLS handshake stalls use real local sockets and native transport.
- [ ] Connect, proxy, TLS, read, write, and pool timeouts map to the expected compatibility classes.
- [ ] Every timeout exception retains request context.
- [ ] Native unread, partial, cancelled, redirect, and auth-response paths release resources.
- [ ] Unsafe unread connections are not returned to the reusable pool.
- [ ] Interpreter shutdown passes without explicit close for required scenarios on Linux, macOS, and Windows.
- [ ] Qualification churn runs for the exact candidate SHA and stays within versioned thresholds.
- [ ] Weekly soak emits retained structured metrics for native workloads.
- [ ] Resource measurements include current/final values, not only lifetime maxima.
- [ ] A deliberately leaked fixture makes lifecycle/soak validation fail.

# Track 7 — Reconcile status and mechanically derive the final stage

## 7.1 Create a new implementation status file

Create:

`plans/httpx-drop-in-final-native-qualification-and-evidence-closure-status.md`

It must record:

- implementation-start SHA;
- every implementation commit;
- exact final candidate SHA;
- exact Required CI Gate URL and attempt;
- exact qualification run URL and attempt;
- eggfetch and controlled replacement wheel filenames and hashes;
- oracle result inventory;
- downstream package/source/test inventory;
- native lifecycle and soak result inventory;
- evidence artifact names and retention period;
- criterion-by-criterion final decision.

Do not copy test counts from local runs into the final qualification section unless the same counts appear in retained exact-SHA artifacts.

## 7.2 Correct prior status claims

Update prior status documents with a brief supersession notice pointing to the new status file.

Correct obsolete candidate SHAs and remove statements that the previous pass fully completed all tracks.

## 7.3 Derive rather than declare the stage

The final status script or document must mechanically derive:

- `Stage C candidate` if implementation tests pass but any qualification criterion is absent or failed;
- `Stage C released` only if every required final gate passes and evidence validation returns true.

No manual prose override may contradict the machine-readable evidence decision.

## Track 7 acceptance criteria

- [ ] New status file contains exact SHA, run, artifact, and hash evidence.
- [ ] Prior status files are accurately superseded.
- [ ] No stale candidate SHA or unqualified release claim remains in current documentation.
- [ ] Final stage agrees with validated evidence.
- [ ] `Stage C released` appears only after all final gates pass.

# Required implementation sequence

Execute in this order to avoid building evidence on unstable foundations:

1. Track 0: status correction and candidate identity schema.
2. Track 1: exact typed API oracle.
3. Track 5: lossless merge semantics.
4. Track 4: async auth and intermediate response ownership.
5. Track 2: pinned downstream behavioral portfolio and runner.
6. Track 6: native timeout/lifecycle/shutdown fixtures and metrics.
7. Track 3: qualification and evidence workflow repair.
8. Track 7: final status and stage derivation.

Do not start final qualification until Tracks 1, 2, 4, 5, and 6 pass locally and in ordinary CI.

# Mandatory checkpoints

## Checkpoint A — Oracle and semantic precision

Required before downstream work is considered trustworthy:

- typed oracle output;
- exact allowed-difference matching;
- full negative-oracle suite;
- lossless query/header merge tests;
- sync and async auth contract tests.

Failure at this checkpoint retains Stage C candidate and blocks later release qualification.

## Checkpoint B — Controlled downstream substitution

Required before qualification workflow closure:

- manifest schema v2 valid;
- pinned and hashed required sources;
- eight real Stage C categories covered;
- no import-only required entries;
- false-green runner meta-tests pass;
- controlled HTTPX remains installed after dependency resolution.

## Checkpoint C — Native lifecycle proof

Required before final evidence generation:

- native proxy/TLS timeout classification;
- native unread/partial/cancel lifecycle tests;
- cross-platform shutdown tests;
- exact-SHA qualification churn;
- structured resource metrics within thresholds.

## Checkpoint D — Exact-SHA qualification

Required for release:

- successful exact-SHA Required CI Gate;
- successful built-artifact qualification;
- all required result artifacts retained;
- evidence generator returns success and `overall_pass=true`;
- independent final evidence validator passes;
- no release-relevant commit exists after qualification candidate without rerunning qualification.

# Required test commands

Adapt exact paths as implementation evolves, but the final repository must provide documented commands equivalent to:

```bash
# Rust core
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

# Python compatibility
python -m pytest crates/eggfetch-python/tests/compat \
  -v --strict-markers --timeout=30

# Oracle unit and negative tests
python -m pytest \
  crates/eggfetch-python/tests/compat/test_oracle_negative.py \
  crates/eggfetch-python/tests/compat/test_api_manifest_comparator.py \
  -v

# Downstream manifest and false-green tests
python -m pytest \
  crates/eggfetch-python/tests/compat/test_downstream_portfolio.py \
  crates/eggfetch-python/tests/compat/test_downstream_runner_negative.py \
  -v

# Auth, merge, and lifecycle tests
python -m pytest \
  crates/eggfetch-python/tests/compat/test_auth.py \
  crates/eggfetch-python/tests/compat/test_auth_replay.py \
  crates/eggfetch-python/tests/compat/test_hook_auth_ordering.py \
  crates/eggfetch-python/tests/compat/test_lossy_conversions.py \
  crates/eggfetch-python/tests/compat/test_lifecycle.py \
  crates/eggfetch-python/tests/compat/test_timeout_proxysis.py \
  crates/eggfetch-python/tests/compat/test_shutdown.py \
  crates/eggfetch-python/tests/compat/test_resource_assertions.py \
  -v --timeout=120

# Qualification workflow/static validation
python scripts/validate_qualification_workflow.py

# Evidence negative tests
python -m pytest \
  crates/eggfetch-python/tests/compat/test_evidence_negative.py \
  -v
```

The final status must list the exact commands and retained CI jobs actually used.

# Final completion gate

This corrective line is complete only when all of the following are true for one exact candidate SHA:

- [ ] Ordinary Required CI Gate is green.
- [ ] Facade and installed top-level replacement API oracles pass with exact typed difference matching.
- [ ] Zero unexplained or stale allowed differences remain.
- [ ] Allowed-difference schema validation passes with no wildcard or multi-match records.
- [ ] Sync and async auth drivers match committed HTTPX observations.
- [ ] Intermediate auth responses release native resources.
- [ ] One-shot body replay failures are deterministic and byte-safe.
- [ ] Client/request query and header merging is lossless on the wire.
- [ ] Every required downstream source is exact, pinned, and hash verified.
- [ ] Every required downstream entry executes a real behavioral suite.
- [ ] All eight Stage C downstream categories pass.
- [ ] No required entry is skipped, unavailable, zero-test, below budget, or import-only.
- [ ] Native proxy CONNECT, TLS, read, write, connect, and pool timeout classification passes.
- [ ] Native unread, partial, cancelled, redirect, and auth lifecycle tests pass.
- [ ] Linux, macOS, and Windows shutdown scenarios pass.
- [ ] Exact-SHA qualification churn stays within versioned resource thresholds.
- [ ] Qualification uses only downloaded candidate artifacts.
- [ ] Every result records the same candidate SHA and artifact hashes.
- [ ] Evidence generation consumes retained results and exits successfully only when `overall_pass=true`.
- [ ] Independent evidence validation passes.
- [ ] Qualification summary and raw evidence artifacts are retained and linked.
- [ ] Current status and documentation contain no stale SHA, placeholder, contradiction, or unqualified release claim.

If any item is absent or failed, the mechanically derived result remains **Stage C candidate**.

# Handoff deliverables

The implementation agent must leave:

1. code and tests for Tracks 1, 4, 5, and 6;
2. downstream manifest schema v2 and pinned source inventory;
3. repaired isolated and aggregate downstream runners;
4. workflow/static validation tooling;
5. repaired qualification workflow;
6. schema-v3 result and evidence tooling;
7. negative fixtures for oracle, runner, lifecycle, and evidence false-green paths;
8. exact-SHA qualification artifacts;
9. final corrective status file with run links and hashes;
10. documentation whose Stage C claim matches the validated evidence.

No later agent should infer completion from commit messages. Completion is determined only by the final gate above and its retained exact-SHA evidence.
