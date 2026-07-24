# HTTPX Drop-In Release Qualification Wiring and Exact-SHA Closure Plan

Status: implementation handoff

Baseline commit: `f24b62b552486ac8b35eb8d60210304a5e67cee9`

Target classification after this plan: **Stage C released**, but only after the exact candidate SHA has a retained green Required CI Gate, a retained green qualification run, a validated evidence bundle, and no subsequent release-relevant commits.

Current classification: **Stage C candidate — release qualification is nonfunctional**.

## 1. Purpose

This plan closes the remaining gap between the increasingly credible HTTPX compatibility implementation and an executable, trustworthy release-qualification system.

The product-side facade, typed API oracle, auth ownership, repeated-value merging, and several native tests are materially improved. The remaining defects are concentrated in integration and evidence integrity:

1. `.github/workflows/qualification.yml` still calls removed `--wheel-dir` runner interfaces.
2. The workflow does not generate or propagate the artifact manifest or candidate identity introduced by the latest implementation.
3. Raw pytest JSON is passed to evidence generation without normalization.
4. Artifact paths used by the workflow do not match paths searched by evidence verification.
5. Native timeout, proxy/TLS, shutdown, resource, and soak result sections are mandatory to independent validation but are not produced or supplied.
6. Downstream source hashes are verified against downloaded wheels that are not necessarily the wheels installed.
7. Installed downstream versions are not compared with manifest versions.
8. The release-blocking downstream portfolio does not mechanically cover all eight declared Stage C consumer categories.
9. Several downstream commands are invalid, synthetic, or fail to materially exercise the named package.
10. Proxy, TLS, shutdown, and soak tests are not all native, strict, or invoked by qualification.
11. `scripts/validate_qualification_workflow.py` claims substantially more validation than it performs.
12. The status artifact names a stale candidate SHA and overstates completed tracks.
13. No retained exact-SHA CI or qualification evidence exists for current `main`.

This is a closure plan, not a new compatibility expansion. Do not reopen already-corrected facade semantics unless a release-blocking suite exposes a concrete regression.

## 2. Non-goals

This pass must not:

- broaden the HTTPX compatibility surface beyond the current Stage C profile;
- add unrelated features to the Rust engine, Python API, CLI, or documentation;
- publish crates, wheels, or a PyPI release;
- weaken existing tests to obtain a green workflow;
- replace package-specific downstream behavior with import-only smoke tests;
- accept partial request success in concurrency, shutdown, resource, or soak qualification;
- infer release readiness from local runs, commit messages, checked-in JSON, or status prose;
- merge generated evidence into the repository;
- promote to Stage C released before exact-SHA retained evidence exists.

## 3. Governing invariants

Implementation must preserve the following invariants throughout the pass.

### 3.1 One candidate

Every release-blocking job must operate on one exact 40-character candidate SHA supplied by the qualification workflow.

No job may independently choose `main`, `HEAD`, a branch name, a pull-request merge ref, or a locally resolved commit.

### 3.2 Build once

The eggfetch wheel and the controlled replacement `httpx` wheel are built exactly once for the candidate SHA.

All compatibility, downstream, shim, native, resource, soak, evidence, and final-gate jobs must consume those exact retained wheel artifacts.

### 3.3 One normalized artifact directory

After download, every consuming job must normalize both wheels into one deterministic directory such as:

```text
target/qualification/artifacts/
  eggfetch-<version>-<tags>.whl
  httpx-0.28.1-py3-none-any.whl
  artifact-manifest.json
  candidate-identity.json
```

No release-blocking script may search arbitrary fallback directories.

### 3.4 One canonical identity

A single candidate identity record and canonical identity digest must bind:

- candidate SHA;
- workflow run ID and attempt;
- workflow URL;
- eggfetch version;
- eggfetch wheel filename and SHA-256;
- replacement `httpx` wheel filename and SHA-256;
- reference HTTPX version;
- artifact-manifest SHA-256;
- producer schema version.

Every result must embed or reference the same identity digest.

### 3.5 Fail closed

Missing, malformed, skipped, xfailed, stale, mismatched, partial, unknown, unverified, or empty release-blocking evidence is failure.

A process crash is not a valid structured success condition. A required suite must emit a valid result contract explaining its outcome.

### 3.6 Evidence is derived

`overall_pass` must be mechanically derived from validated result contracts. It must never be authored manually or inferred from job success alone.

### 3.7 Any release-relevant commit invalidates qualification

If code, tests, manifests, workflow files, qualification scripts, compatibility profiles, allowed differences, controlled replacement packaging, or release documentation changes after qualification, the new SHA must be requalified.

## 4. Required final architecture

The qualification workflow must implement this dependency graph:

```text
verify-candidate
        |
        v
build-candidate-artifacts
        |
        v
normalize-artifacts-and-create-identity
        |
        +----------------------+-----------------------+
        |                      |                       |
        v                      v                       v
compat-and-oracle       downstream-matrix       native-proof
        |                      |                       |
        +----------------------+-----------------------+
                               |
                               v
                    aggregate-retained-results
                               |
                               v
                     generate-evidence-bundle
                               |
                               v
                 independent-evidence-validation
                               |
                               v
                     qualification-summary-gate
```

The workflow must not preserve parallel legacy paths that use old arguments, old result formats, or direct wheel globs.

## 5. Phase 0 — Freeze and establish an honest baseline

### 5.1 Freeze release-relevant scope

Before implementation:

1. Record baseline `f24b62b552486ac8b35eb8d60210304a5e67cee9` in the new closure status file.
2. Mark the current state as `Stage C candidate — qualification nonfunctional`.
3. Enumerate every release-relevant path covered by this pass:
   - `.github/workflows/qualification.yml`
   - `.github/workflows/ci.yml`
   - `compat/downstream/**`
   - `compat/httpx/0.28.1/**`
   - `compat/httpx-controlled-replacement/**`
   - `scripts/candidate_identity.py`
   - `scripts/generate_artifact_manifest.py`
   - `scripts/normalize_pytest_result.py`
   - `scripts/run_downstream_compat.py`
   - `scripts/run_isolated_downstream.py`
   - `scripts/generate_compatibility_evidence.py`
   - `scripts/validate_compatibility_evidence.py`
   - `scripts/validate_qualification_workflow.py`
   - native qualification tests under `crates/eggfetch-python/tests/compat/`
   - release and compatibility documentation.
4. Prohibit status promotion until the exact-SHA gate in Phase 10 passes.

### 5.2 Baseline negative assertions

Add tests or workflow-validator fixtures proving the current known defects are detected:

- workflow invokes removed `--wheel-dir` argument;
- workflow omits artifact-manifest generation;
- workflow omits candidate-identity generation;
- workflow passes raw pytest JSON to evidence generation;
- workflow omits native result arguments;
- downstream matrix differs from required manifest set;
- evidence artifact lookup cannot resolve nested download paths;
- required downstream installation is unpinned;
- required category coverage is incomplete;
- stale status SHA is rejected.

Acceptance criteria:

- Each defect has a deterministic negative fixture or test.
- Tests fail before implementation and pass only when the corresponding defect is corrected.
- No fixture merely searches for a vague word such as `soak` or `resource`.

## 6. Phase 1 — Normalize artifacts and create the candidate identity once

### 6.1 Canonical artifact directory

Update `generate_artifact_manifest.py` or add a small orchestration wrapper so that every job uses one directory supplied explicitly by argument.

Required behavior:

1. Accept the exact candidate SHA and source download directories.
2. Require exactly one eggfetch wheel.
3. Require exactly one controlled replacement `httpx` wheel.
4. Copy, not symlink, both wheels into the canonical directory.
5. Reject duplicate or ambiguous wheel candidates.
6. Compute SHA-256 by streaming file contents.
7. Record absolute or workspace-relative canonical paths.
8. Verify distribution names and versions from wheel metadata:
   - eggfetch distribution name must be `eggfetch`;
   - replacement distribution name must be `httpx`;
   - replacement version must equal the reference profile version `0.28.1`.
9. Reject an upstream HTTPX wheel or a replacement wheel missing the eggfetch shim marker package.
10. Emit `artifact-manifest.json` atomically.

Required artifact-manifest fields:

```json
{
  "schema_version": "1",
  "candidate_sha": "<40-char SHA>",
  "producer": "generate_artifact_manifest.py",
  "artifacts": [
    {
      "artifact_type": "eggfetch",
      "distribution": "eggfetch",
      "version": "<version>",
      "filename": "<filename>",
      "path": "<canonical path>",
      "sha256": "<64-char digest>",
      "size_bytes": 123
    },
    {
      "artifact_type": "httpx-controlled-replacement",
      "distribution": "httpx",
      "version": "0.28.1",
      "filename": "<filename>",
      "path": "<canonical path>",
      "sha256": "<64-char digest>",
      "size_bytes": 123
    }
  ]
}
```

### 6.2 Canonical identity and digest

Revise `candidate_identity.py` so there is one schema and one creation path.

Required behavior:

1. Load and validate the artifact manifest.
2. Derive wheel records from the manifest rather than duplicate caller-provided values.
3. Include run ID, run attempt, workflow URL, candidate SHA, and exact timestamps.
4. Canonicalize JSON using sorted keys and stable separators.
5. Compute `identity_digest = sha256(canonical_identity_without_digest)`.
6. Store the digest in the final identity object.
7. Validate the digest when reading the identity.
8. Require `started_at < finished_at` only after finalization; permit a distinct in-progress creation state or generate both timestamps at finalization.
9. Remove unused top-level duplicate wheel fields unless every consumer requires them.
10. Reject empty run IDs, attempts, URLs, hashes, filenames, or producer values.

Required identity fields:

```json
{
  "schema_version": "4",
  "candidate_sha": "<40-char SHA>",
  "eggfetch_version": "<version>",
  "reference_httpx_version": "0.28.1",
  "artifact_manifest_sha256": "<64-char digest>",
  "eggfetch_wheel": {
    "filename": "...",
    "sha256": "..."
  },
  "httpx_replacement_wheel": {
    "filename": "...",
    "sha256": "..."
  },
  "run_id": "...",
  "run_attempt": "...",
  "workflow_run_url": "...",
  "producer": "candidate_identity.py",
  "started_at": "...",
  "finished_at": "...",
  "identity_digest": "<64-char digest>"
}
```

### 6.3 Retained identity artifact

The normalization job must upload one artifact containing:

```text
candidate-artifacts/
  *.whl
  artifact-manifest.json
  candidate-identity.json
```

Every downstream job must download this artifact rather than separately downloading wheel artifacts and reconstructing paths.

Acceptance criteria:

- Tampering with either wheel causes manifest validation failure.
- Tampering with the manifest causes identity validation failure.
- Tampering with the identity causes digest validation failure.
- Every consumer can locate both wheels using only `artifact-manifest.json`.
- No consumer performs fallback path scanning.

## 7. Phase 2 — Introduce one result contract for every release-blocking producer

### 7.1 Result schema

Define one versioned result contract used by:

- compatibility suite;
- facade API oracle;
- top-level shim API oracle;
- each downstream package;
- downstream aggregate;
- native timeout suite;
- proxy/TLS suite;
- shutdown suite;
- resource suite;
- soak suite;
- workflow validation.

Required common fields:

```json
{
  "schema_version": "1",
  "producer": "<script or suite>",
  "suite": "<stable suite identifier>",
  "candidate_sha": "<40-char SHA>",
  "identity_digest": "<64-char digest>",
  "started_at": "<RFC3339>",
  "finished_at": "<RFC3339>",
  "status": "passed|failed|error",
  "overall_pass": true,
  "metrics": {},
  "diagnostics": []
}
```

Rules:

- `overall_pass=true` is legal only with `status=passed`.
- `failed`, `error`, `skipped`, `xfailed`, `unavailable`, `partial`, and `unknown` must never be treated as release success.
- A producer must write its result atomically even when the underlying test process fails.
- Missing result output is a separate aggregation failure.
- Result timestamps and identity must be validated before aggregation.

### 7.2 Normalize pytest JSON correctly

Update `normalize_pytest_result.py` and its workflow integration.

It must:

1. Parse the actual `pytest-json-report` schema, including the `summary` object.
2. Record collected, passed, failed, errors, skipped, xfailed, xpassed, and duration.
3. Require collected > 0 for release-blocking suites.
4. Require failed = errors = skipped = xfailed = xpassed = 0 unless a suite-specific policy explicitly permits otherwise. No such exceptions are permitted in this closure pass.
5. Embed candidate SHA and identity digest.
6. Accept an expected suite identifier.
7. Reject malformed reports or absent summary data.
8. Produce the common result contract.

Add parser fixtures for:

- all passing;
- one failed;
- one error;
- skipped-only;
- xfailed;
- zero collected;
- malformed JSON;
- report missing summary;
- candidate identity mismatch.

### 7.3 Structured command results

For non-pytest commands, create a normalizer that records:

- command;
- exit code;
- bounded stdout/stderr excerpts;
- exact assertions performed;
- metrics reported by the command;
- common identity fields.

A zero exit code alone must not be enough when the producer is expected to report specific metrics.

Acceptance criteria:

- Every release-blocking job uploads a valid result contract.
- Aggregation rejects raw pytest JSON, ad hoc job-status JSON, or files without identity digests.
- A process crash results in a structured `error`, not an accepted absence or arbitrary nonzero code.

## 8. Phase 3 — Rebuild downstream acquisition and installation around the exact verified artifact

### 8.1 Exact source acquisition

Refactor `run_isolated_downstream.py` so source verification and installation operate on the same file.

For PyPI packages:

1. Resolve the exact `name==version` from `source-locator`.
2. Download one exact wheel or sdist into a package-specific source directory.
3. Match the downloaded filename and SHA-256 against the manifest.
4. Reject multiple candidates.
5. Store the exact downloaded path in the result.
6. Install that exact local path, not `pkg["name"]`.
7. Use a constraints file containing:
   - `httpx==0.28.1`;
   - the exact downstream package version;
   - any explicitly pinned auxiliary dependencies required by the fixture.
8. Confirm the installed distribution version equals the manifest version.
9. Confirm the installed distribution location belongs to the test venv.
10. Re-run controlled replacement identity verification after installation.
11. Run `pip check` and fail on any conflict.

For git packages, if introduced later:

- require an immutable full commit SHA;
- archive the commit;
- verify an archive hash;
- install the archived source;
- record the commit and hash.

### 8.2 Do not verify one artifact and install another

Delete the current pattern that:

- downloads a pinned wheel for hash verification;
- then installs an unconstrained package name.

Add a negative test where:

- manifest pins version A;
- resolver would otherwise install newer version B;
- the runner must install A and report A.

Add a negative test where the verified wheel is swapped before installation; installation must fail due hash revalidation.

### 8.3 Dependency handling

The controlled replacement `httpx` wheel must remain installed and must satisfy downstream dependency resolution.

Required checks before and after downstream installation:

```python
import httpx
assert httpx.__eggfetch_shim__ is True
assert httpx.__version__ == "0.28.1"
assert "eggfetch" in httpx.Client.__module__
assert "eggfetch" in httpx.AsyncClient.__module__
```

Also require:

- `pip show httpx` names the controlled replacement distribution;
- no second upstream HTTPX package directory exists;
- downstream install does not overwrite the shim;
- `pip check` succeeds.

### 8.4 Platform-safe venv paths

Replace hard-coded `venv/bin/python` and `venv/bin/pip` paths with `sysconfig` or platform-aware helpers so the runner works on Windows as well as Unix.

Acceptance criteria:

- The exact hash-verified downstream artifact is the installed artifact.
- Installed version equals manifest version.
- A newer available release cannot silently replace the pinned version.
- Controlled replacement identity survives dependency installation.
- Runner works on Linux, macOS, and Windows path conventions.

## 9. Phase 4 — Replace synthetic downstream commands with committed behavioral fixtures

### 9.1 Fixture layout

Create committed package-specific fixtures:

```text
compat/downstream/behavioral_fixtures/
  httpx_contract/
  respx/
  pytest_httpx/
  starlette/
  anthropic/
  httpx_sse/
  httpx_auth/
  opentelemetry_httpx/
```

Each fixture must be executable as a real Python module or pytest file. Do not embed complex compound statements in `python -c` strings.

Each fixture must:

- import the named downstream package;
- exercise the downstream package’s public integration with HTTPX;
- route all HTTP through a local app, local server, or controlled transport;
- assert a concrete request/response, auth, stream, hook, or interception outcome;
- assert controlled replacement identity during execution;
- emit or be normalized into the common result contract.

### 9.2 Required eight-category portfolio

The release-blocking manifest must contain exactly one or more required representatives for every declared Stage C category:

| Stage C category | Required representative | Required behavior |
|---|---|---|
| `contract-tests` | HTTPX 0.28.1 public contract subset | Run a pinned selected upstream public test subset against the replacement distribution without installing upstream HTTPX |
| `mock-transport-request-matching` | `respx` | Register a route, send through `httpx.Client`, assert route match, call count, status, and body |
| `framework-test-client` | `pytest-httpx` | Execute a real pytest test using the `httpx_mock` fixture and assert request matching and response injection |
| `asgi-test-client` | `starlette` | Construct `TestClient`, call a Starlette route, assert status/body and lifecycle |
| `sdk-async-client` | `anthropic` or another pinned async SDK | Construct the SDK with a controlled async HTTP client/transport and exercise one local or mocked request path without credentials or external network |
| `streaming-sse-consumption` | `httpx-sse` | Parse multiple SSE events from a streamed response and assert event fields and stream closure |
| `custom-auth-flow` | `httpx-auth` | Apply a real package auth object, send a request, and assert the generated auth header or challenge flow |
| `event-hooks-instrumentation` | `opentelemetry-instrumentation-httpx` or another actual instrumentation package | Instrument a client, execute a request, assert hook/span callback execution, then uninstrument cleanly |

`httpx-ws` must not be labeled as event-hook instrumentation unless its fixture actually exercises event hooks or instrumentation. It may remain informational under an accurately named category, or be removed from the Stage C release-blocking set.

### 9.3 HTTPX contract subset

Acquire the pinned HTTPX 0.28.1 source artifact and verify its hash.

Run a curated public-behavior subset that does not depend on HTTPX private internals unavailable by design. The subset must cover at least:

- request construction;
- response construction;
- headers and repeated values;
- query parameters and repeated values;
- URL behavior;
- cookies;
- timeout configuration;
- sync client request flow;
- async client request flow;
- auth flow;
- event hooks;
- mock transport;
- exception request context.

Record the exact selected test paths and reasons for exclusions in the manifest or a companion file. Exclusions must not be silently expanded.

### 9.4 Required fixture thresholds

Every required package must define:

- `release-blocking = true`;
- `min-collected >= 1`;
- `min-passed >= 1`;
- `max-skipped = 0`;
- `max-xfailed = 0`;
- deterministic timeout;
- exact source hash;
- exact package version;
- exact category IDs.

Acceptance criteria:

- Every required fixture materially uses the named package.
- No required fixture is import-only.
- No invalid one-line Python syntax remains.
- All eight categories have at least one passing release-blocking result.
- Category names are defined once and reused by manifest validation, runners, evidence validation, and documentation.

## 10. Phase 5 — Generate the downstream matrix from the manifest

### 10.1 Single source of truth

Add a script such as:

```text
scripts/generate_downstream_matrix.py
```

It must:

1. Validate the manifest.
2. Select `release-blocking=true` packages.
3. Require complete eight-category coverage.
4. Emit stable sorted GitHub Actions matrix JSON.
5. Emit the exact expected package set and category-to-package mapping.
6. Fail on duplicate names, unknown categories, missing categories, informational-only categories, or empty selection.

### 10.2 Dynamic matrix job

Add a `prepare-downstream-matrix` job that:

- checks out the exact candidate SHA;
- runs the generator;
- publishes matrix JSON as a job output;
- uploads the generated matrix as a retained artifact;
- produces a common result contract.

The downstream matrix job must consume this output:

```yaml
strategy:
  fail-fast: false
  matrix: ${{ fromJSON(needs.prepare-downstream-matrix.outputs.matrix) }}
```

Do not keep a hand-written package list in the workflow.

### 10.3 Aggregate exact result set

The downstream aggregate job must:

1. Download every package result artifact.
2. Require one and only one result for each expected release-blocking package.
3. Reject extra, duplicate, or missing results.
4. Require every result to have the same candidate SHA and identity digest.
5. Require every status to be `passed`.
6. Require all eight categories to be covered by passed results.
7. Emit a common aggregate result contract.

Acceptance criteria:

- Adding or removing a required package in the manifest automatically changes the matrix.
- A stale hand-authored matrix cannot exist.
- Omitting `pytest-httpx`, the instrumentation consumer, or any other required package fails aggregation.
- Informational packages do not accidentally become release blockers.

## 11. Phase 6 — Complete native proxy and TLS proof

### 11.1 Actual HTTP proxy routing

Correct the current test that sends directly to the proxy address as though it were an origin server.

Required successful proxy test:

```python
with local_http_server() as origin:
    with local_proxy_server() as proxy:
        with Client(proxy=f"http://{proxy_host}:{proxy_port}") as client:
            response = client.get(f"http://{origin_host}:{origin_port}/health")
```

The proxy fixture must record:

- absolute-form HTTP request target;
- target host and port;
- request method;
- whether forwarding completed;
- connection counts.

The test must assert those observations.

### 11.2 CONNECT tunnel proof

Add a successful HTTPS-through-proxy test:

1. Start the local TLS origin.
2. Start the proxy.
3. Configure `Client(proxy=..., verify=<local CA>)`.
4. Request the HTTPS origin.
5. Assert proxy observed CONNECT to the correct authority.
6. Assert TLS request succeeds through the tunnel.

### 11.3 CONNECT stall timeout

Add a deterministic proxy fixture mode that:

- accepts TCP;
- reads the CONNECT request;
- signals a synchronization event;
- never returns `200 Connection Established` until released or stopped.

The client test must:

- configure the proxy through the real `proxy=` path;
- use a bounded connect timeout;
- assert the exact reference-compatible exception class;
- assert `.request` points to the original request;
- assert elapsed time falls within policy bounds;
- assert the fixture observed CONNECT before timeout.

Do not simulate this with `MockTransport`.

### 11.4 TLS handshake stall

Add a deterministic TLS-stall fixture that:

- accepts TCP;
- signals that the connection was accepted;
- reads or ignores ClientHello;
- never sends ServerHello.

Compare reference HTTPX 0.28.1 behavior and lock the expected compatibility exception class and message category.

Required assertions:

- exact exception class;
- `.request` context;
- bounded elapsed time;
- accepted connection synchronization;
- no leaked fixture thread or socket.

### 11.5 TLS certificate validation

Add native tests for:

- valid local CA succeeds;
- untrusted self-signed certificate fails;
- hostname mismatch fails;
- `verify=False` succeeds where intended;
- custom CA path succeeds;
- async equivalents for at least success and verification failure.

Acceptance criteria:

- Proxy behavior uses the real native proxy path.
- CONNECT success and CONNECT stall are both proven.
- TLS success, handshake stall, trust failure, and hostname failure are proven.
- Mock-only timeout classification tests may remain as unit tests but cannot satisfy native qualification.

## 12. Phase 7 — Complete native shutdown, concurrency, resource, and retained soak proof

### 12.1 Native shutdown subprocesses

Replace or supplement MockTransport shutdown scenarios with subprocess fixtures that create real local servers and real native clients.

Required subprocess scenarios:

1. unused sync client without explicit close;
2. used sync client without explicit close;
3. unread native response at interpreter exit;
4. partially consumed native streamed response at interpreter exit;
5. active keep-alive connection at interpreter exit;
6. successful proxied request then exit;
7. successful TLS request then exit;
8. async client with completed request then loop shutdown;
9. cancelled async native request then loop shutdown;
10. auth challenge sequence using native origin responses;
11. close/request race with deterministic barriers.

Each subprocess must:

- exit zero within the platform deadline;
- emit no forbidden warnings;
- leave no child process;
- leave fixture threads joined;
- report a structured result.

### 12.2 Strict concurrency

Restore a meaningful concurrency assertion.

Required sync and async tests:

- schedule N operations;
- record every operation result by index;
- require exactly N successes;
- require zero swallowed exceptions;
- require every thread/task to terminate;
- require response body correctness;
- use synchronization barriers rather than relaxing success thresholds.

Do not catch and discard broad exceptions merely to reduce CI flakiness.

If the native engine genuinely cannot support the chosen concurrency pattern, treat that as a product defect and fix the engine or reduce the documented supported pattern. Do not convert it into partial success.

### 12.3 Resource policy execution

Make `resource-thresholds.toml` executable policy rather than documentation.

The resource suite must read the current platform section and measure:

- file descriptor or handle delta;
- thread delta;
- RSS growth;
- completed operations;
- hung operations;
- timeout overshoot;
- fixture cleanup.

Required rules:

- unsupported metric collection requires a documented platform-specific equivalent, not a release-blocking skip;
- all thresholds must be reported in the result contract;
- observed values and pass/fail decisions must be retained;
- thresholds may not be increased in the same commit merely to make a failing candidate pass without justification and review.

### 12.4 Retained soak

The qualification workflow must execute the configured soak policy, not just short churn tests.

Minimum current policy:

- duration: at least 300 seconds;
- completed requests: at least 500;
- zero request failures;
- zero hangs;
- zero leaked responses;
- bounded FD/handle delta;
- bounded thread delta;
- bounded RSS growth.

The soak workload must include:

- sync GET and POST;
- async GET and POST;
- response body reads;
- repeated client creation and closure;
- keep-alive reuse;
- redirects;
- auth challenge/replay;
- cancellation;
- proxied HTTP;
- TLS;
- timeout recovery.

Use deterministic local fixtures. No external network is permitted.

The short `test_soak.py` churn tests may remain in ordinary CI. Qualification must additionally run the retained-duration workload and upload its metrics.

Acceptance criteria:

- All scheduled concurrency operations succeed.
- Native shutdown scenarios pass without warnings or hangs.
- Resource policy is read and enforced.
- Retained soak meets both duration and operation-count minima.
- All native result contracts share the candidate identity digest.

## 13. Phase 8 — Rewrite the qualification workflow around the new contracts

### 13.1 Remove obsolete invocations

Delete every use of:

```text
--wheel-dir
```

from qualification jobs unless a separate script explicitly still defines that interface. The intended closure architecture uses `--artifact-manifest` and `--candidate-identity`.

### 13.2 Required workflow jobs

The workflow must contain, at minimum:

1. `verify-candidate`
2. `build-candidate-artifacts`
3. `normalize-candidate-artifacts`
4. `validate-workflow`
5. `compat-suite`
6. `facade-api-oracle`
7. `shim-api-oracle`
8. `prepare-downstream-matrix`
9. `downstream-substitution`
10. `aggregate-downstream-results`
11. `shim-substitution`
12. `native-timeout`
13. `proxy-tls`
14. `shutdown`
15. `resource`
16. `soak`
17. `aggregate-qualification-results`
18. `generate-evidence`
19. `independent-evidence-validation`
20. `qualification-summary-gate`

Jobs may be combined only when retained result boundaries and failure diagnosis remain explicit.

### 13.3 Exact checkout

Every job that reads repository content must use:

```yaml
with:
  ref: ${{ needs.verify-candidate.outputs.candidate_sha }}
```

Jobs that do not depend directly on `verify-candidate` must receive the candidate SHA through a valid upstream dependency and still check out that exact SHA.

### 13.4 Artifact consumption

All consuming jobs must download the canonical `candidate-artifacts` bundle and validate:

```text
artifact-manifest.json
candidate-identity.json
eggfetch wheel
replacement httpx wheel
```

before installing anything.

### 13.5 Compatibility and oracle results

The compatibility job must:

- install only the candidate artifacts and test requirements;
- run the compatibility suite;
- save raw pytest JSON;
- normalize it into the common result contract;
- upload raw and normalized reports.

The API jobs must:

- generate manifests for the facade and top-level shim separately;
- run the exact typed comparator;
- embed candidate SHA and identity digest in normalized results;
- reject unexplained, stale, duplicate, wildcard, resolved-active, or malformed waivers.

### 13.6 Native results

Each native suite job must:

- install candidate wheels from the canonical bundle;
- run its exact suite;
- normalize results;
- upload raw logs and structured results.

The soak job must use an appropriate job timeout exceeding the configured soak duration while retaining a strict workload-level deadline.

### 13.7 No ad hoc job-status substitutes

Delete `job-result-*.json` files that contain only job name, status, and SHA if they are used as evidence substitutes.

GitHub job status may be included as metadata, but release evidence must come from suite result contracts.

Acceptance criteria:

- The workflow uses only current CLI interfaces.
- Every required result artifact is produced before aggregation.
- No release-blocking step uses `continue-on-error`, `|| true`, or `if: always()` to convert failure into success.
- `if: always()` is permitted only for uploading diagnostics after failure; uploaded failure diagnostics must not satisfy the final gate.

## 14. Phase 9 — Make workflow validation enforce its documented contract

### 14.1 Replace vague checks

Rewrite `validate_qualification_workflow.py` so every documented check is implemented.

Required checks:

1. YAML parses successfully.
2. Required jobs exist.
3. All `needs` references name existing jobs.
4. Every exact-SHA checkout uses the verified candidate expression.
5. No release job checks out `main`, `HEAD`, or a PR merge ref.
6. Every downloaded artifact is produced by exactly one reachable upstream job.
7. Candidate artifact bundle is generated before all consumers.
8. Artifact manifest and identity are passed to every relevant script.
9. Runner arguments match current parser contracts.
10. No `--wheel-dir` invocation remains for runners that require `--artifact-manifest`.
11. Raw pytest JSON is normalized before evidence consumption.
12. Required pytest plugins are installed in the job that invokes them.
13. Downstream matrix is generated from the manifest rather than hard-coded.
14. Matrix output is consumed by the downstream job.
15. Downstream aggregate depends on every matrix result artifact.
16. Evidence generation receives all mandatory result sections.
17. Candidate identity is supplied to evidence generation.
18. Independent validation consumes the generated evidence artifact.
19. Final gate depends on independent validation and all required jobs.
20. Soak suite path and configured duration/count are invoked.
21. Resource policy file is explicitly supplied to the resource runner.
22. No release step contains `|| true`, permissive `continue-on-error`, or ignored exit status.
23. Diagnostic upload steps cannot mask required job failure.
24. Evidence and status are not committed to the repository.

### 14.2 Parser-contract validation

Do not rely only on substring matching.

Preferred implementation:

- expose each script’s argument parser through a testable builder function;
- parse workflow command tokens with `shlex` where possible;
- compare supplied flags with parser-supported flags;
- verify required flags are present.

At minimum, add explicit validation for all qualification scripts.

### 14.3 Negative workflow fixtures

Create fixture workflows that fail for each major defect:

- obsolete argument;
- missing artifact manifest;
- missing candidate identity;
- raw pytest report passed directly;
- hard-coded incomplete matrix;
- missing native result;
- missing soak invocation;
- missing resource policy;
- stale branch checkout;
- `|| true`;
- missing plugin;
- evidence gate not in final dependencies.

Acceptance criteria:

- The validator fails the current pre-fix workflow.
- The validator passes the corrected workflow.
- Every negative fixture fails for the intended reason and diagnostic code.
- Header documentation lists only checks actually implemented.

## 15. Phase 10 — Make evidence generation and independent validation complete

### 15.1 Required release mode

Add an explicit release mode to `generate_compatibility_evidence.py`, or make all sections mandatory by default.

Release evidence must require:

- candidate identity;
- artifact manifest;
- compatibility result;
- facade API result;
- shim API result;
- downstream aggregate result;
- shim substitution result;
- native timeout result;
- proxy/TLS result;
- shutdown result;
- resource result;
- soak result;
- workflow-validation result.

Optional release-relevant sections are prohibited.

### 15.2 Identity consistency

For every input:

1. Require `candidate_sha`.
2. Require `identity_digest`.
3. Require both to match the canonical candidate identity.
4. Reject absent identity fields.
5. Reject result timestamps outside the qualification run interval.
6. Reject duplicate suite identifiers.
7. Reject stale results from a previous run or attempt.

### 15.3 Artifact verification by manifest path

Stop searching fallback locations by filename.

Evidence generation must:

- load each exact path from `artifact-manifest.json`;
- verify path exists within the normalized artifact directory;
- recompute SHA-256;
- compare with both manifest and candidate identity;
- reject path traversal or paths outside the bundle.

### 15.4 Compute overall pass across every section

`overall_pass=true` must require:

- every mandatory result is present;
- every mandatory result validates;
- every mandatory result has `overall_pass=true`;
- all identities match;
- all hashes match;
- all eight downstream categories are represented by passed release-blocking results;
- API oracle has no unexplained or stale differences;
- native metrics satisfy policy;
- soak metrics satisfy duration and count;
- workflow validator passes.

### 15.5 Independent validator

`validate_compatibility_evidence.py` must independently recompute the decision rather than trust top-level `overall_pass`.

It must validate:

- schema versions;
- identity digest;
- artifact hashes;
- exact mandatory suite set;
- exact downstream package set;
- exact downstream category coverage;
- zero required skips/xfails/failures/errors;
- API oracle tuple integrity;
- native timeout classifications;
- proxy/TLS scenario coverage;
- shutdown scenario set;
- resource thresholds;
- soak duration and request count;
- workflow validation result;
- candidate SHA and run metadata.

The validator must fail if `overall_pass=true` disagrees with recomputation.

### 15.6 Evidence bundle

Upload a retained bundle containing:

```text
qualification-evidence/
  compatibility-evidence.json
  compatibility-report.md
  artifact-manifest.json
  candidate-identity.json
  results/
    compat.json
    facade-api.json
    shim-api.json
    downstream-aggregate.json
    downstream/*.json
    shim-substitution.json
    native-timeout.json
    proxy-tls.json
    shutdown.json
    resource.json
    soak.json
    workflow-validation.json
  raw/
    pytest reports
    logs
    resource metrics
    soak metrics
```

Do not check these generated files into git.

Acceptance criteria:

- Removing any mandatory result makes generation fail.
- Altering any identity digest makes generation and validation fail.
- Altering any wheel makes validation fail.
- Top-level `overall_pass` is recomputed and cannot override failed sections.
- Independent validation passes the retained bundle without repository-local fallback files.

## 16. Phase 11 — Ordinary CI and qualification sequencing

### 16.1 Required CI Gate

Before qualification, the exact candidate SHA must have a green ordinary Required CI Gate covering at least:

- Rust formatting;
- clippy with warnings denied;
- Rust tests;
- Python compatibility tests;
- Python 3.10–3.13 matrix;
- controlled replacement build and identity smoke;
- API oracle and negative oracle fixtures;
- downstream runner negative fixtures;
- workflow validator and negative workflow fixtures;
- result-contract tests.

### 16.2 Qualification input verification

The qualification `verify-candidate` job must confirm:

- input is a full SHA;
- SHA exists in the repository;
- SHA is reachable from `main`;
- Required CI Gate for that exact SHA completed successfully;
- no ambiguous prefix resolution is used.

Use GitHub API data or a trustworthy workflow linkage. Do not infer ordinary CI success from the qualification workflow itself.

### 16.3 Exact-SHA run

Run qualification against the final implementation SHA.

If any release-relevant correction is committed after the run begins or after it completes:

- invalidate the previous run;
- run ordinary CI again;
- run qualification again;
- generate a new evidence bundle.

### 16.4 Final gate

The final gate must fail unless:

- candidate verification passed;
- build and normalization passed;
- all suite jobs passed;
- all expected structured results are present;
- aggregation passed;
- evidence generation passed;
- independent evidence validation passed;
- evidence says `overall_pass=true`;
- evidence candidate SHA equals the workflow input SHA;
- current release candidate SHA still equals the qualified SHA.

Acceptance criteria:

- A green final gate cannot occur with a missing, skipped, cancelled, neutral, or failed dependency.
- GitHub matrix cancellation or partial completion cannot be interpreted as success.
- Exact candidate SHA is visible in the final summary.

## 17. Phase 12 — Status and documentation reconciliation

### 17.1 Status source of truth

Update:

`plans/httpx-drop-in-qualification-integrity-and-native-proof-corrective-closure-status.md`

or replace it with a final closure status that names:

- final candidate SHA;
- ordinary CI run ID and URL;
- qualification run ID and URL;
- workflow run attempt;
- evidence artifact name;
- candidate identity digest;
- both wheel filenames and hashes;
- downstream package set;
- downstream category coverage;
- native suite summaries;
- soak duration and completed operation count;
- independent validator result;
- final mechanically derived stage.

### 17.2 No stale SHA

The status file must not continue naming `9ac95122...` or baseline `f24b62...` as the current candidate after release-relevant implementation commits.

Add a status validation script or test that rejects:

- candidate SHA not equal to the qualified evidence SHA;
- run URL missing or placeholder;
- evidence artifact missing or placeholder;
- release claim without `overall_pass=true`;
- release claim when current candidate differs from qualified candidate.

### 17.3 Documentation wording

Until qualification passes, documentation must use only:

> Stage C candidate

After qualification passes, documentation may use:

> Stage C released for the explicitly qualified HTTPX 0.28.1 profile and retained evidence bundle.

Do not use unqualified claims such as “full drop-in replacement” if the active allowed-difference ledger still contains Stage C exceptions.

Acceptance criteria:

- Status is derived from evidence rather than implementation checkboxes.
- README, compatibility docs, migration docs, skills, and release instructions use consistent terminology.
- Generated evidence remains out of git.

## 18. Required implementation order

Execute in this order to avoid building new logic around stale interfaces:

1. Phase 0 baseline and negative fixtures.
2. Phase 1 canonical artifact manifest and identity.
3. Phase 2 common result contracts and pytest normalization.
4. Phase 3 exact downstream artifact acquisition and installation.
5. Phase 4 committed package-specific behavioral fixtures.
6. Phase 5 manifest-generated downstream matrix and aggregate.
7. Phase 6 native proxy/TLS proof.
8. Phase 7 native shutdown/concurrency/resource/soak proof.
9. Phase 8 workflow rewrite.
10. Phase 9 workflow validator completion.
11. Phase 10 evidence generator and independent validator completion.
12. Run ordinary local validation.
13. Commit implementation.
14. Run exact-SHA Required CI Gate.
15. Correct only genuine failures; any correction creates a new candidate SHA.
16. Re-run exact-SHA Required CI Gate as needed.
17. Run exact-SHA qualification.
18. Validate retained evidence independently.
19. Update status and documentation with the exact successful run.
20. Re-run ordinary CI and qualification if the status/documentation change is release-relevant under repository policy.

## 19. Suggested commit decomposition

Use small commits with independently reviewable boundaries.

1. `test: add qualification integration negative fixtures`
2. `fix: normalize candidate artifacts and identity`
3. `fix: standardize qualification result contracts`
4. `fix: install exact verified downstream artifacts`
5. `test: add package-specific downstream behavioral fixtures`
6. `fix: generate downstream matrix from manifest`
7. `test: add native proxy and TLS qualification proof`
8. `test: add strict shutdown resource concurrency and soak proof`
9. `fix: rewire qualification workflow to current contracts`
10. `fix: enforce qualification workflow integrity`
11. `fix: make compatibility evidence complete and fail closed`
12. `docs: reconcile HTTPX qualification status`

Do not combine test weakening with product corrections. Do not hide failing behavior behind a threshold change in the same commit.

## 20. Required local validation commands

Exact commands may be adapted to repository tooling, but the handoff must provide equivalent coverage.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

```bash
python -m pytest crates/eggfetch-python/tests/compat/test_result_contracts.py -v
python -m pytest crates/eggfetch-python/tests/compat/test_oracle_negative.py -v
python -m pytest crates/eggfetch-python/tests/compat/test_downstream_runner_negative.py -v
python -m pytest crates/eggfetch-python/tests/compat/test_downstream_portfolio.py -v
python -m pytest crates/eggfetch-python/tests/compat/test_native_timeout_classification.py -v
python -m pytest crates/eggfetch-python/tests/compat/test_timeout_proxysis.py -v
python -m pytest crates/eggfetch-python/tests/compat/test_shutdown.py -v
python -m pytest crates/eggfetch-python/tests/compat/test_resource_assertions.py -v
python -m pytest crates/eggfetch-python/tests/compat/test_soak.py -v
```

```bash
python scripts/validate_qualification_workflow.py .github/workflows/qualification.yml
python scripts/validate_httpx_compat_profile.py compat/httpx/0.28.1/profile.toml
```

Build the candidate artifacts once, then validate the complete local pipeline using those built wheels:

```bash
python scripts/generate_artifact_manifest.py \
  --candidate-sha "$(git rev-parse HEAD)" \
  --eggfetch-wheel-dir <built-eggfetch-dir> \
  --httpx-wheel-dir <built-replacement-dir> \
  --output-dir target/qualification/artifacts
```

```bash
python scripts/candidate_identity.py create \
  --artifact-manifest target/qualification/artifacts/artifact-manifest.json \
  --candidate-sha "$(git rev-parse HEAD)" \
  --run-id local \
  --run-attempt 1 \
  --workflow-run-url local://qualification \
  --output target/qualification/artifacts/candidate-identity.json
```

```bash
python scripts/run_downstream_compat.py \
  --artifact-manifest target/qualification/artifacts/artifact-manifest.json \
  --candidate-identity target/qualification/artifacts/candidate-identity.json \
  --required-only \
  --output target/qualification/results/downstream-aggregate.json
```

Finally generate and independently validate a complete local evidence bundle. The generator must refuse to run if any mandatory input is missing.

## 21. Mandatory negative acceptance tests

The implementation is incomplete unless each condition below fails deterministically:

1. eggfetch wheel missing;
2. replacement wheel missing;
3. duplicate eggfetch wheels;
4. upstream HTTPX wheel substituted for replacement;
5. wheel hash tampered;
6. artifact manifest tampered;
7. candidate identity digest tampered;
8. result candidate SHA mismatched;
9. result identity digest mismatched;
10. raw pytest JSON passed without normalization;
11. zero tests collected;
12. one skipped required test;
13. one xfailed required test;
14. unknown downstream package;
15. empty downstream selection;
16. missing required package fixture;
17. import-only required fixture;
18. invalid required fixture command;
19. source hash missing;
20. source hash mismatch;
21. pinned version differs from installed version;
22. upstream HTTPX replaces shim after dependency installation;
23. `pip check` failure;
24. one Stage C category lacks a release-blocking result;
25. static matrix differs from manifest-generated set;
26. one matrix result artifact is missing;
27. proxy test does not configure `proxy=`;
28. CONNECT stall fixture is not observed;
29. TLS handshake stall returns wrong exception class;
30. shutdown subprocess exceeds deadline;
31. concurrency completes fewer operations than scheduled;
32. resource threshold exceeded;
33. soak duration below policy;
34. soak operation count below policy;
35. soak contains one request failure;
36. workflow contains obsolete `--wheel-dir`;
37. workflow omits candidate identity;
38. workflow omits a mandatory evidence input;
39. workflow contains `|| true` in a release step;
40. final gate omits independent validation;
41. evidence top-level `overall_pass=true` while a section fails;
42. status SHA differs from evidence SHA;
43. post-qualification release-relevant commit exists.

## 22. Final global acceptance criteria

This plan is complete only when all criteria below are true.

1. Current candidate SHA is explicitly frozen and recorded.
2. Required CI Gate is green for that exact SHA.
3. Candidate wheels are built exactly once.
4. Both wheels are normalized into one canonical artifact bundle.
5. Artifact manifest validates exact distribution identities, versions, paths, sizes, and SHA-256 hashes.
6. Candidate identity validates exact SHA, run metadata, manifest hash, wheel hashes, and canonical digest.
7. Every release-blocking result contains the exact candidate SHA and identity digest.
8. Raw pytest output is normalized through a tested parser.
9. Every required result uses one versioned common contract.
10. API oracle uses exact typed one-to-one waivers with no unexplained or stale differences.
11. Downstream source verification and installation use the same exact artifact.
12. Installed downstream version equals the manifest version.
13. Controlled replacement identity survives every downstream dependency installation.
14. Every required downstream fixture materially uses the named package.
15. All eight Stage C consumer categories have passed release-blocking proof.
16. Downstream matrix is generated from the manifest.
17. Downstream aggregation rejects missing, duplicate, extra, skipped, xfailed, failed, or identity-mismatched results.
18. HTTP proxy routing is tested through the real `proxy=` configuration.
19. HTTPS CONNECT success is tested through the real proxy path.
20. CONNECT stall timeout is deterministic and correctly classified.
21. TLS success, handshake stall, trust failure, and hostname mismatch are correctly classified.
22. Sync and async native timeout exceptions retain request context.
23. Every scheduled concurrency operation succeeds.
24. Native shutdown subprocess scenarios exit within policy without forbidden warnings.
25. Resource metrics satisfy the executable platform policy.
26. Retained soak satisfies configured duration and operation-count minima with zero failures and hangs.
27. Qualification workflow calls only current script interfaces.
28. Workflow validator detects argument, artifact, identity, matrix, plugin, native-input, and final-gate defects.
29. Evidence generation requires every release-blocking result.
30. Evidence verifies exact artifact paths and hashes from the manifest.
31. Evidence `overall_pass` is recomputed across all sections.
32. Independent validation reproduces the pass decision.
33. Qualification final gate is green for the exact candidate SHA.
34. Evidence bundle is retained as a workflow artifact.
35. Generated evidence is not committed to git.
36. Status names the exact candidate SHA, run URLs, artifact, identity digest, and wheel hashes.
37. Documentation does not overstate compatibility beyond the qualified Stage C profile.
38. No release-relevant commit follows qualification without a new qualification run.

## 23. Explicit stop conditions

Stop and keep the repository at Stage C candidate if any of the following remains true:

- workflow still invokes `--wheel-dir` for the rewritten runners;
- candidate identity is absent from any release-blocking result;
- a raw pytest report is consumed as normalized evidence;
- artifact verification relies on fallback filename searches;
- downstream installation uses an unconstrained package name;
- a required package is represented only by an import or synthetic package-independent check;
- any Stage C category has no passing required representative;
- proxy or TLS qualification is satisfied only by MockTransport;
- shutdown qualification is satisfied only by explicitly closed mock clients;
- concurrency allows partial success;
- resource or soak policy is not executed;
- native sections remain optional to `overall_pass`;
- workflow validator documentation exceeds its implementation;
- exact-SHA Required CI or qualification evidence is absent;
- status names a stale SHA;
- any implementation commit lands after the retained qualification run.

## 24. Handoff decision rule

The implementing agent must report one of two outcomes.

### Outcome A — closed

Report closure only with:

- exact candidate SHA;
- Required CI run URL;
- qualification run URL;
- evidence artifact name;
- candidate identity digest;
- wheel filenames and hashes;
- downstream package/category summary;
- native suite metrics;
- soak duration and completed operation count;
- independent validator success;
- confirmation that no release-relevant commit followed qualification.

### Outcome B — still candidate

If any required evidence is absent or failed, report:

> Stage C candidate — release qualification incomplete.

List the exact failed criterion and retain all diagnostic artifacts. Do not substitute implementation completion, local success, or prose assertions for retained qualification evidence.
