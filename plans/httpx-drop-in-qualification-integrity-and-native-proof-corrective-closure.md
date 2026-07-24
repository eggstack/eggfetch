# HTTPX Drop-In Qualification Integrity and Native Proof Corrective Closure

Status: READY FOR IMPLEMENTATION

Baseline SHA: `c1b55c8dd9ead6fcd67741f46e81f084ee0b19ae`

Predecessor plans:

- `plans/httpx-drop-in-verification-substitution-and-lifecycle-corrective-closure.md`
- `plans/httpx-drop-in-final-native-qualification-and-evidence-closure.md`

Current compatibility classification: **Stage C candidate**

This plan is a narrowly scoped corrective closure for the remaining verification, evidence, downstream-substitution, and native-proof defects identified after the final native qualification pass. It does not reopen the broader HTTPX compatibility roadmap, reimplement already-corrected facade semantics, or expand Stage C scope.

The implementation may restore a **Stage C released** claim only after every mandatory gate in this plan passes for one immutable candidate SHA and one retained artifact set. Until then, the repository must continue to state **Stage C candidate**.

---

## 1. Objective

Close the remaining integrity defects that prevent the current HTTPX compatibility implementation from being release-qualified.

The corrective pass must deliver all of the following:

1. An executable qualification workflow whose artifact paths, result formats, and dependency graph are internally consistent.
2. A downstream portfolio that runs pinned, hash-verified, package-specific behavioral suites rather than imports or eggfetch-only stand-ins.
3. Exact API-oracle waiver matching by symbol, difference type, member, and expected values.
4. Mandatory candidate-identity propagation through every release-blocking result artifact.
5. Native proxy, TLS, timeout, shutdown, resource, concurrency, and retained-soak evidence using real local transport paths.
6. Stronger tests that fail on partial success, swallowed exceptions, skipped required suites, malformed diagnostics, or missing structured results.
7. Status and documentation generated from the actual current candidate and retained evidence rather than implementation intent.

This plan is complete only when exact-SHA qualification succeeds end to end and retained artifacts independently validate the release claim.

---

## 2. Non-goals

Do not expand this pass into unrelated compatibility work.

The following are explicit non-goals:

- Implementing HTTP/2 support solely to improve Stage C claims.
- Adding Trio or Curio backends.
- Replacing the Rust HTTP engine.
- Reworking the complete HTTPX object model.
- Adding new proxy schemes beyond what eggfetch already claims.
- Publishing the controlled `httpx` replacement distribution to public PyPI.
- Adding broad new SDK compatibility targets beyond the required representative Stage C categories.
- Refactoring unrelated Rust crates or Python APIs.
- Restoring Stage C released before evidence exists.
- Treating local source-tree tests as substitutes for built-wheel qualification.
- Weakening acceptance thresholds to accommodate unstable CI.

---

## 3. Audited remaining defects

### 3.1 Qualification artifact-path mismatch

The evidence job downloads eggfetch and controlled replacement wheels into separate artifact directories but invokes the downstream runner with only the eggfetch directory.

The downstream runner correctly requires both wheels in the same input directory. The workflow must normalize downloaded artifacts into a single immutable candidate artifact directory before any qualification consumer runs.

### 3.2 Evidence artifact lookup mismatch

Artifact hashes are computed from nested download paths, while the evidence generator searches unrelated top-level paths.

The evidence generator must consume an explicit artifact manifest containing exact paths and hashes. It must not search the repository heuristically.

### 3.3 Pytest result-schema mismatch

The evidence generator expects normalized top-level test counts, while the workflow passes the raw `pytest-json-report` schema.

A normalization step or explicit schema-aware parser is required. The producer and consumer must share a versioned result contract.

### 3.4 Downstream matrix does not cover all required entries

Required manifest packages and workflow matrix entries differ. The current parser may warn and succeed if it cannot parse the matrix.

The matrix must be generated from or strictly validated against the manifest. Parse failure must be fatal.

### 3.5 Required downstream entries are not pinned reproducibly

Manifest versions are listed, but the isolated runner installs only the package name. Source hashes are empty, and source locator, hash, install command, and working directory are not enforced.

Required packages must be installed from immutable, hash-verified artifacts or immutable commit archives.

### 3.6 Required downstream commands do not exercise the named package

Several required entries import a package without using it, or do not import the named package at all.

Each required package must execute package-specific behavior that materially traverses its integration with `httpx`.

### 3.7 Required skips remain accepted

The aggregate runner treats `skipped` and `skipped-no-tests` as non-failures for required entries.

Every required result must be exactly `passed`. Zero collection, skip, xfail, crash-only behavior, missing output, or malformed output must fail.

### 3.8 Typed API differences are still waived by symbol only

The comparator emits typed difference records but matches allowed entries only on symbol name.

Allowed entries must match the exact difference tuple, not merely the class or function name.

### 3.9 Candidate identity schema is not integrated

A candidate identity helper exists, but qualification does not produce and propagate one identity artifact across jobs.

Every release-blocking result must include the same schema version, SHA, artifact names, hashes, producer, run ID, attempt, and timestamps.

### 3.10 Native proof is incomplete

Real local HTTP socket fixtures exist, but qualification lacks deterministic proxy CONNECT stalls, TLS handshake/certificate paths, active-unclosed interpreter shutdown, strict concurrent-read success, and policy-driven retained soak.

### 3.11 Tests were weakened to tolerate failures

Some tests swallow arbitrary exceptions, require only partial success, or accept process crashes as structured fail-closed behavior.

Qualification tests must distinguish an expected fail-closed diagnostic from an unhandled crash.

### 3.12 Status files are stale

The status file names an obsolete candidate SHA and claims implementation completion without exact-SHA qualification.

Status must be regenerated only after qualification, and the current main SHA must be explicit.

---

## 4. Global implementation rules

These rules apply to every track.

### 4.1 Exact candidate identity

One qualification invocation must use one immutable 40-character candidate SHA.

All build, test, downstream, lifecycle, evidence, and summary artifacts must reference that exact SHA.

No result may omit candidate identity.

### 4.2 Built artifacts only

Release qualification must install and test downloaded wheels produced by the qualification build jobs.

The following are prohibited in release evidence jobs:

- `maturin develop`
- editable installs
- direct source-tree import through implicit `PYTHONPATH`
- rebuilding wheels independently in consumer jobs
- installing upstream `httpx`

### 4.3 Fail closed

Every missing, malformed, skipped, stale, unverified, unpinned, or mismatched input must produce a nonzero exit.

Warnings are not acceptable for release-blocking validation failures.

### 4.4 Structured results

Every release-blocking producer must emit a JSON result matching a versioned schema.

Human-readable logs are supplementary only.

### 4.5 No inferred evidence

Evidence generation may consume only retained result artifacts and artifact manifests.

It may not infer success from:

- importability;
- source-code test counts;
- expected workflow layout;
- commit messages;
- status files;
- existence of test files;
- successful upstream jobs without downloaded result artifacts.

### 4.6 No partial-success qualification

All required operations must succeed.

Do not permit thresholds such as “at least one of five threads,” “150 of 200 requests,” or “up to five errors” in release-blocking proof.

Stress tests may define bounded retries only for explicitly classified transient server setup failures, not client request failures.

### 4.7 Stage claim last

Documentation and status updates restoring Stage C released must be the final step after exact-SHA evidence validates.

---

## 5. Track A — Candidate artifact normalization

### Goal

Provide one deterministic artifact directory and one artifact manifest consumed by all qualification jobs.

### Required changes

1. Add a workflow step after downloading the eggfetch and controlled replacement artifacts that creates:

   ```text
   candidate-artifacts/
     eggfetch-<version>-<tags>.whl
     httpx-<version>-py3-none-any.whl
     artifact-manifest.json
     candidate-identity.json
   ```

2. Reject zero or multiple candidate wheels for the current platform/Python compatibility target unless the consumer explicitly selects one by tag.

3. Generate `artifact-manifest.json` with:

   - schema version;
   - candidate SHA;
   - GitHub run ID;
   - run attempt;
   - workflow name;
   - producer job;
   - artifact logical name;
   - filename;
   - size;
   - SHA-256;
   - normalized relative path;
   - wheel distribution name;
   - wheel version;
   - wheel tags.

4. Generate `candidate-identity.json` through `scripts/candidate_identity.py` or a replacement shared module.

5. Update every consumer job to download or reconstruct the same normalized directory.

6. Remove heuristic artifact searches from evidence generation.

### Required tests

- No eggfetch wheel fails.
- No controlled replacement wheel fails.
- Multiple uncontrolled matches fail.
- Wrong distribution metadata fails.
- Hash mismatch fails.
- Candidate SHA mismatch fails.
- Manifest path traversal fails.
- Unlisted extra wheel fails unless explicitly allowed as a platform matrix artifact.
- Artifact filename rename without manifest regeneration fails.

### Acceptance criteria

- Every downstream, shim, compatibility, lifecycle, soak, and evidence job consumes `candidate-artifacts/`.
- No job passes `dist/eggfetch-wheel` or `dist/httpx-replacement-wheel` directly to qualification consumers.
- Evidence uses explicit paths from `artifact-manifest.json`.
- All candidate artifacts have verified SHA-256 hashes before installation.

---

## 6. Track B — Versioned result contracts

### Goal

Eliminate producer/consumer schema mismatches.

### Required changes

Create shared result schemas or validators for:

1. `compat-tests-result.json`
2. `api-oracle-result.json`
3. `downstream-result.json`
4. `native-timeout-result.json`
5. `native-proxy-tls-result.json`
6. `shutdown-result.json`
7. `resource-result.json`
8. `soak-result.json`
9. `qualification-summary.json`

Every result must include:

- `schema_version`
- `candidate_identity`
- `producer`
- `run_id`
- `run_attempt`
- `job_name`
- `started_at`
- `finished_at`
- `status`
- `errors`
- `metrics`

For pytest-based suites, add a normalizer that converts raw pytest JSON into:

```json
{
  "schema_version": "1",
  "candidate_identity": { ... },
  "status": "passed",
  "collected": 123,
  "passed": 123,
  "failed": 0,
  "errors": 0,
  "skipped": 0,
  "xfailed": 0,
  "xpassed": 0,
  "duration_seconds": 12.34
}
```

### Required behavior

- `collected` must equal the sum of terminal test outcomes expected by the schema.
- Required suites must have `collected > 0`.
- Required suites must have zero skipped, xfailed, xpassed, failed, and errors.
- Missing fields fail.
- Unknown schema versions fail.
- Result SHA or artifact hash mismatch fails.

### Acceptance criteria

- The evidence generator never parses raw pytest JSON directly.
- Every producer validates its own result before upload.
- The final gate revalidates every downloaded result independently.

---

## 7. Track C — Exact typed API-oracle waiver governance

### Goal

Ensure one allowed difference cannot suppress unrelated incompatibilities on the same symbol.

### Required allowed-difference key

Each non-resolved allowed entry must identify an exact expected difference tuple:

```toml
[[difference]]
id = "CLIENT-TIMEOUT-DEFAULT-001"
category = "stage-bounded"
symbol = "Client"
difference-type = "parameter-default"
member = "timeout"
reference = "Timeout(timeout=5.0)"
candidate = "Timeout(timeout=5.0)"
...
```

Where exact values are unstable object representations, use a documented canonical value generated by the manifest normalizer.

### Matching requirements

An allowed entry matches only when all applicable fields match:

- symbol;
- difference type;
- member;
- canonical reference value;
- canonical candidate value.

No wildcard, regex, glob, prefix, suffix, or symbol-only matching is allowed in release mode.

### Schema requirements

Require:

- unique ID;
- category;
- symbol;
- difference type;
- member field, including explicit empty string for symbol-level differences;
- canonical reference;
- canonical candidate;
- rationale;
- compatibility stage impact;
- owner;
- review milestone;
- test references;
- expiry or explicit non-expiring justification.

`resolved` entries must not waive current differences. They may remain only as historical records in a separate resolved ledger or must fail as stale if present in the active allowed file.

### Required negative tests

- Same symbol, wrong difference type does not match.
- Same symbol and type, wrong member does not match.
- Same tuple, changed reference value does not match.
- Same tuple, changed candidate value does not match.
- Duplicate tuple fails.
- Duplicate ID fails.
- Wildcard fails.
- Missing member fails.
- Missing expected values fails.
- Expired entry fails.
- Resolved entry matching a current delta fails.
- One entry cannot satisfy two differences.

### Acceptance criteria

- Every observed difference maps one-to-one to one active allowed entry.
- Every active allowed entry maps to exactly one observed difference.
- Zero unexplained and zero stale entries are required for both facade and top-level shim oracles.

---

## 8. Track D — Reproducible downstream source acquisition

### Goal

Run immutable downstream package versions with verified provenance.

### Manifest schema changes

For every required package, require one of:

#### PyPI artifact mode

- exact project name;
- exact version;
- exact wheel or sdist filename;
- SHA-256;
- Python/platform compatibility;
- download source URL recorded for traceability;
- install mode.

#### Git archive mode

- repository URL;
- immutable full commit SHA;
- archive SHA-256;
- subdirectory if applicable;
- install command;
- test command;
- test working directory.

#### Vendored fixture mode

Allowed only for an eggfetch-authored behavioral fixture, not as a substitute for every external downstream package.

Require:

- fixture ID;
- fixture directory hash;
- categories covered;
- named external integration assumptions;
- reason an unmodified downstream source cannot be used.

### Runner changes

1. Stop installing `pkg["name"]`.
2. Acquire the exact artifact declared in the manifest.
3. Verify SHA-256 before installation.
4. Install with resolver controls that prevent replacement of the controlled `httpx` distribution.
5. Record installed distribution name, version, and source artifact hash.
6. Re-run shim identity and distribution metadata checks after all dependency installation.
7. Run `pip check`.
8. Reject any installed upstream HTTPX provenance.
9. Execute the exact manifest test command in the declared working directory.
10. Record the exact command and environment policy in the result.

### Network policy

- Network may be used during source acquisition only where the workflow explicitly allows it.
- Test execution for required suites must be isolated from external network access.
- Local loopback servers are allowed.

### Acceptance criteria

- Every required result records an immutable source hash.
- Installed version equals manifest version.
- No required package is installed by unversioned name.
- The controlled replacement remains the active `httpx` distribution before and after downstream installation and testing.

---

## 9. Track E — Package-specific downstream behavior

### Goal

Demonstrate that each required package actually works against the controlled replacement.

### Required Stage C categories

The required portfolio must collectively cover:

1. sync SDK client;
2. asyncio SDK client;
3. ASGI test client;
4. mock transport and request matching;
5. streaming/SSE consumption;
6. custom auth flow;
7. event hooks/instrumentation;
8. custom or mounted transport.

### Required package behavior

Each required external package entry must:

- import the named package;
- construct the named package’s integration object;
- send or process at least one request through the controlled replacement;
- assert package-specific observable behavior;
- execute against loopback or in-process transport only;
- produce a nonzero collected/passed count;
- report zero skip/xfail.

Examples of acceptable proof:

- **respx**: construct a `respx` router, register a route, send through an HTTPX client, assert route call count and response.
- **pytest-httpx**: execute an actual pytest fixture test using `HTTPXMock`, register a response, assert request matching and consumption.
- **Starlette**: construct a Starlette app and `TestClient`, send a request, assert response body and lifespan behavior.
- **httpx-sse**: parse a loopback SSE stream and assert event fields and termination.
- **httpx-auth**: instantiate an auth class from the package, dispatch against a local handler, and assert emitted authorization behavior.
- **httpx-ws or equivalent event-hook package**: exercise the package-specific hook or transport integration, not import-only behavior.
- **sync SDK**: instantiate a pinned SDK with injected controlled client/transport, execute a mocked API request, assert request shape and parsed response.
- **async SDK**: perform the equivalent async request and stream or response parsing.

### Internal behavioral fixtures

The existing `compat/downstream/behavioral_fixtures/` may remain as contract fixtures, but they must not be represented as unmodified downstream project proof.

Classify results separately:

- `external_downstream`
- `eggfetch_behavioral_fixture`

Both are useful, but only the external downstream category satisfies package substitution claims.

### Required runner enforcement

- Import-only commands are prohibited for required entries.
- Commands that never import the named package fail validation.
- Commands that import but never reference the package beyond import fail validation unless the declared suite is a real pytest file from that package.
- `min-collected` and `min-passed` must be enforced.
- Required results must have `status == "passed"` exactly.
- Skipped, xfailed, xpassed, zero-test, crash, timeout, malformed output, and missing result all fail.

### Acceptance criteria

- All eight Stage C categories have release-blocking package-specific proof.
- Every required package test uses the named package materially.
- No required entry uses an import-only command.

---

## 10. Track F — Manifest-driven qualification matrix

### Goal

Eliminate drift between the manifest and workflow matrix.

### Preferred implementation

Generate the GitHub Actions matrix from a manifest-validation job:

1. `prepare-downstream-matrix` checks out the exact candidate SHA.
2. It validates the manifest.
3. It emits JSON containing all required package IDs.
4. `downstream-substitution` consumes the JSON via `fromJSON`.

### Alternative implementation

A static matrix is acceptable only if a strict parser proves exact set equality. Parse failure must exit nonzero.

### Required validation

- Required manifest entries equal matrix entries exactly.
- No informational package appears in the required matrix unless deliberately included in a separate informational matrix.
- Every category is represented.
- Duplicate package IDs fail.
- Empty required matrix fails.
- Matrix generation result includes manifest SHA-256.

### Acceptance criteria

- Adding or removing a required package changes the matrix automatically or fails CI.
- No warning-only parse path exists.

---

## 11. Track G — Candidate identity propagation

### Goal

Make every release-blocking artifact provably belong to the same candidate build.

### Required identity fields

Use one shared identity object with:

- schema version;
- candidate SHA;
- eggfetch version;
- eggfetch wheel filename and SHA-256;
- controlled replacement filename and SHA-256;
- reference HTTPX version;
- workflow run ID;
- workflow run attempt;
- producer job;
- started and finished timestamps.

### Required workflow behavior

1. Build job creates the canonical identity.
2. Consumer jobs download it rather than recreating it.
3. Consumer jobs validate it before installation.
4. Every result embeds the exact identity or an identity digest plus the full identity artifact reference.
5. Final evidence verifies every embedded identity digest is identical.
6. A post-candidate commit invalidates the release claim and requires a new candidate identity.

### Negative tests

- Missing identity fails.
- Wrong candidate SHA fails.
- Different wheel hash fails.
- Different run attempt fails unless explicitly linked as a rerun of the same immutable artifacts.
- Consumer-generated identity fails.
- Result from another run fails.
- Empty producer fails.
- Invalid timestamp order fails.

### Acceptance criteria

- No result SHA check is optional.
- All release-blocking results resolve to one identity digest.

---

## 12. Track H — Evidence generation redesign

### Goal

Generate release evidence exclusively from validated retained artifacts.

### Required inputs

The evidence job must download:

- canonical candidate identity;
- artifact manifest;
- compatibility test results for all required Python versions;
- facade API-oracle result;
- top-level shim API-oracle result;
- downstream package result for every required package;
- native timeout result;
- proxy/TLS result;
- shutdown result for required platforms;
- resource result;
- retained soak result;
- qualification workflow validation result.

### Evidence requirements

Evidence must include:

- exact identity object and digest;
- artifact hashes;
- manifest hash;
- required package inventory;
- required category inventory;
- exact source hashes;
- exact commands;
- collected/passed/skipped/failed counts;
- API difference and allowance counts;
- native proof metrics;
- platform/Python matrix;
- workflow run URL and attempt;
- overall decision;
- blockers when false.

### Fail conditions

- Missing required artifact.
- Duplicate package result.
- Result for unexpected package.
- Wrong identity digest.
- Any skip/xfail/failure/error.
- Empty required inventory.
- Missing category.
- Missing source hash.
- Artifact hash mismatch.
- Oracle stale or unexplained difference.
- Native proof below threshold.
- Soak not retained or not exact candidate.

### Acceptance criteria

- `overall_pass=true` is possible only when every required artifact validates.
- Evidence generation exits nonzero when false.
- Independent validation uses no shared mutable state from the generator.

---

## 13. Track I — Deterministic native proxy proof

### Goal

Prove proxy behavior and timeout classification through a real local proxy path.

### Required fixtures

Implement a deterministic loopback proxy fixture supporting:

- ordinary HTTP forwarding;
- CONNECT success;
- CONNECT response stall;
- CONNECT establishment followed by upstream stall;
- proxy authentication challenge where supported by current claims;
- malformed CONNECT response;
- proxy close before response.

Use synchronization events or socket barriers rather than timing-only sleeps where possible.

### Required tests

- HTTP request traverses proxy and records target authority.
- HTTPS CONNECT path reaches the proxy.
- CONNECT-response stall maps to the documented timeout class.
- Upstream-after-CONNECT stall maps correctly.
- Proxy refusal differs from connect timeout.
- Explicit `timeout=None` disables timeout behavior for a bounded fixture that later releases.
- Per-request timeout overrides client timeout.
- Exceptions retain the request.
- Proxy sockets close after client shutdown.

### Acceptance criteria

- Tests use real TCP sockets and the native engine path.
- Broad `Exception` acceptance is prohibited in release-blocking assertions.
- Expected exception classes and elapsed bounds are explicit.

---

## 14. Track J — Deterministic TLS proof

### Goal

Exercise real TLS handshake and certificate behavior locally.

### Required fixtures

Add generated test certificates or deterministic repository test certificates for:

- trusted local CA and server certificate;
- untrusted certificate;
- hostname mismatch;
- TLS listener that accepts TCP but stalls before ServerHello;
- TLS listener that completes handshake then stalls response body.

Do not expose private production material. Test keys must be repository-only fixtures clearly marked non-production.

### Required tests

- Trusted CA succeeds.
- Default verification rejects untrusted certificate.
- Hostname mismatch rejects.
- `verify=False` behavior matches documented policy.
- TLS handshake stall maps to the expected timeout class.
- Post-handshake read stall maps to read/total timeout as claimed.
- Certificate errors retain request context.
- TLS sockets and tasks are released after exceptions.

### Acceptance criteria

- TLS tests use the native engine and real sockets.
- No mock transport substitutes for TLS proof.

---

## 15. Track K — Shutdown and resource ownership

### Goal

Prove bounded interpreter shutdown and zero material resource leakage.

### Required subprocess scenarios

Run separate subprocesses for:

1. unused unclosed sync client;
2. used unclosed sync client;
3. unclosed sync streaming response;
4. unused unclosed async client;
5. used unclosed async client;
6. cancelled async request with client not explicitly closed;
7. proxy request with client not explicitly closed;
8. TLS request with client not explicitly closed.

Each process must:

- exit within a bounded deadline;
- return zero where clean implicit teardown is expected;
- produce no fatal interpreter error;
- produce no unhandled task warning;
- produce no thread-pool panic;
- avoid hanging daemon or non-daemon threads.

Where explicit close is contractually required, the test must document and assert the exact expected warning or behavior rather than silently substituting explicit close.

### Resource metrics

Collect where supported:

- file descriptor delta;
- socket count delta;
- thread count delta;
- task count delta;
- RSS growth;
- operation completion count;
- timeout overshoot;
- hung operations.

Use the committed threshold policy as executable input, not documentation only.

### Acceptance criteria

- Threshold file is parsed by the qualification job.
- Missing platform policy fails.
- Unsupported metrics have an explicit substitute metric and documented rationale.
- Resource result includes before/after samples and thresholds.

---

## 16. Track L — Strict concurrency proof

### Goal

Replace weakened partial-success concurrency tests with deterministic correctness tests.

### Required changes

1. Remove blanket `except Exception: pass` from release-blocking concurrency tests.
2. Collect and report every thread or task exception.
3. Require all scheduled operations to complete successfully.
4. Use a server capable of handling the expected concurrency.
5. Use barriers to start workers simultaneously where testing contention.
6. Fail if any worker remains alive after the join deadline.
7. Test shared-client concurrency only where the client contract claims it.
8. Separately test one-client-per-thread behavior.

### Required scenarios

- Five concurrent sync reads, all successful.
- Repeated concurrent batches.
- Concurrent response body reads where supported.
- Async gather with all operations successful.
- Cancellation of one async operation does not corrupt others.
- Close during in-flight operation follows documented semantics.

### Acceptance criteria

- No partial-success threshold.
- No swallowed exception.
- Every failure includes structured worker diagnostics.

---

## 17. Track M — Retained soak implementation

### Goal

Run the committed soak policy against exact candidate artifacts and retain metrics.

### Required policy integration

Read `compat/httpx/0.28.1/resource-thresholds.toml` or replace it with a versioned executable policy.

The qualification soak must honor at minimum:

- configured duration;
- configured request count;
- zero hung operations;
- maximum error count of zero for deterministic loopback workloads;
- resource thresholds;
- timeout overshoot thresholds.

### Required workload mix

Include:

- sync GET;
- sync POST;
- async GET;
- async POST;
- response-body reads;
- streaming response open/read/close;
- repeated client creation/destruction;
- connection reuse;
- cancellation;
- redirects;
- auth multi-step flow;
- proxy path;
- TLS path.

### Required output

The soak result must record:

- exact candidate identity;
- policy hash;
- start/end timestamps;
- duration;
- operation counts by category;
- failures by category;
- latency summary;
- timeout overshoot;
- resource before/after/peak values;
- pass/fail decision.

### Acceptance criteria

- The workflow actually invokes the soak suite.
- The soak duration and request count meet policy.
- Zero deterministic client failures.
- Retained soak artifact is required by the final qualification gate.

---

## 18. Track N — Fail-closed tooling tests

### Goal

Prove validation tools fail for the right reason and emit structured diagnostics.

### Required changes

Do not accept an arbitrary crash as sufficient success in negative tests.

A fail-closed negative test passes only when:

- process exit is nonzero;
- structured result exists when the command contract requires one;
- result status is an expected failure status;
- result contains a specific diagnostic code;
- no unrelated traceback is the sole diagnostic.

### Required diagnostic codes

Define stable codes for at least:

- unknown package;
- empty selection;
- missing test command;
- import-only required command;
- source hash missing;
- source hash mismatch;
- upstream HTTPX detected;
- shim identity mismatch;
- pip check failure;
- zero tests;
- skipped required suite;
- xfailed required suite;
- below minimum count;
- malformed result;
- identity mismatch;
- artifact mismatch.

### Acceptance criteria

- Negative tests assert diagnostic code and result schema.
- Unhandled crashes fail the meta-test.

---

## 19. Track O — Qualification workflow integrity validator

### Goal

Make workflow linting catch the current cross-job defects.

### Required validator capabilities

The validator must parse workflow YAML with a real YAML parser or a sufficiently strict repository-owned parser.

It must validate:

- every `needs.verify.outputs` reference has direct or documented transitive dependency;
- every downloaded artifact is produced;
- every required consumer gets both candidate wheels and identity;
- artifact normalization occurs before downstream/evidence consumers;
- every pytest plugin is installed;
- no `|| true` or warning-success fallback in required steps;
- no parse-failure-success behavior;
- downstream matrix equals required manifest set;
- evidence inputs are all produced;
- final gate requires every release-blocking job;
- final summary path exists;
- soak suite is invoked;
- resource policy is read;
- evidence job uses normalized result contracts;
- checkout uses exact candidate SHA in every candidate job.

### Required negative workflow fixtures

- Missing replacement wheel.
- Wrong wheel directory.
- Matrix missing required package.
- Parse warning that exits zero.
- Evidence input missing.
- Soak omitted.
- Consumer checks out default branch.
- Required job absent from final gate.
- Heuristic artifact search.

### Acceptance criteria

- The current known workflow-path defects are represented by negative tests.
- Validator failure blocks ordinary CI.

---

## 20. Track P — CI and qualification sequence

### Required ordinary CI jobs

Ordinary CI must include at least:

- qualification workflow validator;
- allowed-difference schema validator;
- API-oracle negative tests;
- downstream manifest validator;
- downstream runner negative tests;
- candidate identity tests;
- result-contract tests;
- native fixture unit tests;
- documentation consistency tests;
- Required CI Gate.

### Required qualification jobs

1. `verify-candidate`
2. `build-candidate-artifacts`
3. `normalize-candidate-artifacts`
4. `validate-package-content`
5. `wheel-smoke-matrix`
6. `compatibility-matrix`
7. `facade-api-oracle`
8. `shim-api-oracle`
9. `prepare-downstream-matrix`
10. `downstream-substitution-matrix`
11. `native-timeout-proof`
12. `native-proxy-tls-proof`
13. `shutdown-matrix`
14. `resource-proof`
15. `retained-soak`
16. `generate-evidence`
17. `independent-evidence-validation`
18. `qualification-gate`

### Candidate verification

`verify-candidate` must:

- validate SHA format;
- prove the commit exists;
- query exact-SHA check runs;
- require the named `Required CI Gate` completed successfully;
- reject missing, queued, in-progress, neutral, skipped, stale, other-SHA, or other-repository checks;
- record run URL and attempt.

### Acceptance criteria

- Qualification cannot start build consumers unless exact-SHA CI is verified.
- Final gate fails if any required job is skipped, cancelled, neutral, or absent.

---

## 21. Track Q — Status and documentation reconciliation

### Goal

Make repository claims reflect retained evidence.

### Required changes

1. Update the final status file with the actual candidate SHA only after qualification.
2. List every implementation and qualification commit relevant to the candidate.
3. Record the qualification run URL and attempt.
4. Record artifact names and hashes or link to the retained evidence artifact.
5. Remove claims that all tracks are complete while qualification is pending.
6. Keep Stage C candidate until evidence validates.
7. Restore Stage C released only in a commit whose parent candidate evidence is still valid or through a documented claim-update process that does not change release-relevant code.
8. Add a mechanical documentation consistency check for:

   - stale candidate SHAs;
   - contradictory Stage C candidate/released claims;
   - references to unexecuted soak;
   - claims that downstream sources are hash-pinned when hashes are empty;
   - claims that all eight categories pass without result inventory.

### Acceptance criteria

- Current status names current candidate and evidence run.
- No stale plan commit is represented as the current candidate.
- No release claim exists without retained exact-SHA evidence.

---

## 22. Required implementation order

Implement in this order to avoid building evidence atop unstable contracts.

### Phase 0 — Freeze and status correction

- Confirm baseline SHA.
- Mark current status as Stage C candidate and qualification incomplete.
- Record all known blockers from this plan.

### Phase 1 — Contracts and identity

- Track A: candidate artifact normalization.
- Track B: versioned result contracts.
- Track G: candidate identity propagation.
- Add negative tests.

Checkpoint 1 requires all identity and artifact-path tests green.

### Phase 2 — Oracle precision

- Track C.
- Migrate active allowed differences to exact typed tuples.
- Run both facade and shim oracles.

Checkpoint 2 requires zero unexplained, zero stale, one-to-one matches.

### Phase 3 — Downstream reproducibility

- Track D: immutable source acquisition.
- Track E: package-specific behavioral suites.
- Track F: manifest-driven matrix.
- Track N: structured fail-closed diagnostics.

Checkpoint 3 requires all eight Stage C categories represented and every required package passing.

### Phase 4 — Native proof

- Track I: proxy.
- Track J: TLS.
- Track K: shutdown/resource.
- Track L: concurrency.
- Track M: retained soak.

Checkpoint 4 requires strict native proof and executable policy thresholds.

### Phase 5 — Workflow and evidence

- Track H: evidence redesign.
- Track O: workflow validator.
- Track P: qualification sequence.

Checkpoint 5 requires a complete dry-run qualification on a test candidate SHA.

### Phase 6 — Exact-SHA qualification and claims

- Run ordinary CI against final candidate.
- Run qualification using the exact candidate SHA.
- Retain all artifacts.
- Independently validate evidence.
- Update Track Q status and docs last.

---

## 23. Mandatory acceptance checkpoints

### Checkpoint 1 — Artifact and identity integrity

Must prove:

- one normalized artifact directory;
- both candidate wheels present;
- artifact manifest valid;
- candidate identity valid;
- every result contract valid;
- wrong paths and hashes fail.

### Checkpoint 2 — Oracle integrity

Must prove:

- exact tuple matching;
- no symbol-wide waivers;
- no wildcard entries;
- no stale active entries;
- both facade and shim oracles pass.

### Checkpoint 3 — Downstream substitution integrity

Must prove:

- immutable source hashes;
- exact versions installed;
- controlled replacement active before and after dependency install;
- named package materially exercised;
- all eight categories pass;
- zero required skips/xfails/errors;
- matrix equals manifest.

### Checkpoint 4 — Native production proof

Must prove:

- real proxy CONNECT paths;
- real TLS paths;
- exact timeout classes;
- strict concurrency success;
- bounded implicit shutdown scenarios;
- threshold-driven resource result;
- retained policy-compliant soak.

### Checkpoint 5 — Qualification integrity

Must prove:

- exact-SHA Required CI Gate verified;
- every job uses downloaded artifacts;
- every result has one identity digest;
- evidence consumes every required artifact;
- final gate fails on any missing/failed/skipped job.

### Checkpoint 6 — Claim integrity

Must prove:

- status current;
- no contradictory docs;
- Stage C released only when exact-SHA evidence has `overall_pass=true`.

---

## 24. Final global acceptance criteria

The corrective pass is complete only when all statements below are true for one immutable candidate SHA.

1. Ordinary Required CI Gate is green.
2. Qualification verifies that exact CI result.
3. Candidate artifacts are built once and normalized.
4. Candidate identity includes exact wheel hashes.
5. Every result embeds or references the same identity digest.
6. Facade API oracle passes with exact typed one-to-one waivers.
7. Top-level shim API oracle passes with exact typed one-to-one waivers.
8. No active wildcard, symbol-only, stale, or unexplained waiver exists.
9. Every required downstream source is immutable and hash-verified.
10. Every required downstream version matches the manifest.
11. Every required package materially exercises its own HTTPX integration.
12. All eight Stage C categories have release-blocking proof.
13. Required downstream results have zero skips, xfails, failures, and errors.
14. Controlled replacement identity survives dependency installation.
15. Native proxy CONNECT proof passes.
16. Native TLS verification and handshake timeout proof pass.
17. Native timeout classes and request context assertions pass.
18. Strict concurrency tests have 100% scheduled operation success.
19. Shutdown subprocess scenarios exit within bounds.
20. Resource metrics satisfy executable platform policy.
21. Retained soak meets configured duration and request count with zero deterministic failures.
22. Evidence consumes all required retained results.
23. Independent evidence validation passes.
24. Qualification summary has `overall_pass=true`.
25. Status and documentation name the exact candidate and evidence run.
26. No release-relevant commit exists after the qualified candidate without requalification.

---

## 25. Blocking conditions

Keep the repository at **Stage C candidate** if any of the following is true:

- evidence job uses only one candidate wheel directory;
- evidence searches artifacts heuristically;
- raw pytest JSON is consumed without normalization;
- required matrix and manifest differ;
- matrix parse failure warns and succeeds;
- required source hash is empty;
- required package is installed by unversioned name;
- required command is import-only;
- required command does not materially use the named package;
- required suite skips or xfails;
- required result is missing or malformed;
- API waiver matches only by symbol;
- one waiver covers multiple differences;
- candidate identity is optional;
- result SHA is optional;
- proxy or TLS proof uses mocks only;
- timeout assertion accepts arbitrary `Exception`;
- concurrency test tolerates failed workers;
- negative test accepts arbitrary process crash without structured diagnostic;
- soak policy is not executed;
- soak permits deterministic request failures;
- shutdown test explicitly closes an otherwise untested unused client only;
- status references an obsolete candidate;
- exact-SHA CI or qualification artifacts are unavailable.

---

## 26. Handoff guidance

The implementing agent must not mark this plan complete based on local test counts or commit messages.

For each phase, the agent should update a status file containing:

- exact commit SHA;
- files changed;
- commands executed;
- structured result artifact paths;
- acceptance criteria passed;
- unresolved blockers;
- whether Stage C remains candidate.

The final status update must include the exact GitHub Actions qualification run and retained artifact inventory.

Do not restore Stage C released until every criterion in Section 24 passes for the same immutable candidate SHA.
