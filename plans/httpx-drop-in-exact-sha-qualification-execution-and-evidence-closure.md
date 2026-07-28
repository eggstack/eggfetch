# HTTPX Drop-In Exact-SHA Qualification Execution and Evidence Closure

Status: implementation handoff plan

Baseline commit: `5681139c9408531ece8f6d354d682d18d2e174a5`

Target repository: `eggstack/eggfetch`

Primary workflow: `.github/workflows/qualification.yml`

Primary compatibility profile: `compat/httpx/0.28.1`

Primary downstream portfolio: `compat/downstream/manifest.toml`

## 1. Purpose

This plan closes the remaining gap between the current HTTPX compatibility implementation and an executable, independently verifiable Stage C release qualification.

The repository now contains most of the intended qualification components, but the current pipeline still stops before meaningful qualification because candidate-bundle generation, identity binding, downstream category coverage, result normalization, evidence assembly, and strict native proof are not wired together under one coherent contract.

The objective is not to add more compatibility breadth. The objective is to make one exact candidate SHA progress through the complete qualification graph without manual repair, without rerunning substitute suites in the evidence job, without accepting broad exception classes, and without allowing any missing result to be interpreted as success.

The work is complete only when a retained GitHub Actions run for one exact candidate SHA contains:

1. a green ordinary Required CI Gate for the same SHA;
2. one canonical candidate bundle with exact wheel hashes;
3. one candidate identity bound to the bundle manifest;
4. passing required downstream results covering all eight Stage C categories;
5. passing strict native timeout, proxy, TLS, shutdown, resource, concurrency, and soak results;
6. passing facade and controlled-replacement API-oracle results;
7. one evidence document assembled only from retained result artifacts;
8. an independent evidence validation result;
9. a fail-closed qualification summary with `overall_pass=true`;
10. a mechanically generated status document naming the exact SHA, run, attempt, wheel hashes, and evidence artifacts.

Until all ten conditions exist for the same candidate identity, the compatibility stage remains `Stage C candidate`.

## 2. Current audited state

The current implementation has the right broad topology but retains deterministic blockers.

### 2.1 Candidate-bundle generation blockers

The workflow invokes `generate_artifact_manifest.py` without the parser-required `--output` argument.

The same parser requires generation-only arguments during `--validate-only` invocation.

The controlled replacement distribution is named `httpx`, so its wheel filename does not contain the word `controlled`. The current filename classifier therefore classifies it as ordinary `httpx` instead of `httpx-controlled-replacement`.

The generated candidate identity does not contain the digest of the artifact manifest that is subsequently used to validate it.

These defects prevent the normalization job from producing a valid bundle.

### 2.2 Downstream portfolio blockers

The declared Stage C portfolio has eight categories, but `contract-tests` and `sdk-async-client` do not currently have release-blocking proof.

The matrix generator warns rather than failing when required category coverage is incomplete.

Several manifest test commands remain one-line shell expressions with weak or invalid behavior:

- the respx command uses invalid compound-statement syntax in `python -c`;
- the pytest-httpx command imports the package but tests only the candidate's own `MockTransport`;
- the httpx-ws entry does not prove event-hook or instrumentation behavior;
- informational SDK entries do not provide offline release-blocking SDK integration proof.

The matrix output uses hyphenated property names and embeds test commands directly into GitHub expressions. This is fragile and makes shell quoting part of the qualification contract.

The optional-dependency field names are inconsistent between the manifest, matrix generator, and workflow.

The artifact acquisition script chooses a wheel from PyPI by package and version rather than validating an exact declared filename or immutable URL. A manifest hash can therefore refer to a different distribution file than the one selected by the downloader.

### 2.3 Evidence blockers

The evidence job calls `run_downstream_compat.py` with obsolete `--wheel-dir` syntax.

It reruns compatibility and downstream tests rather than consuming the results already produced by release-blocking jobs.

Raw pytest-json-report files are passed to a parser that does not read their nested `summary` structure.

Artifact verification ignores the bundle-relative paths recorded in the artifact manifest.

Resource results are required by independent validation but are not passed into evidence generation.

Native and generic result sections are optional to the evidence generator even when operating in release qualification mode.

The current `overall_pass` calculation does not include every required native result section.

### 2.4 Native proof blockers

Positive CONNECT and HTTPS-through-proxy tests catch arbitrary exceptions and pass.

TLS-handshake stall proof accepts a broad union of errors instead of the intended compatibility class.

Shutdown qualification does not yet require subprocess interpreter teardown with active or abandoned native resources.

The resource policy is not represented as a normalized result with candidate identity.

The soak test is not bound to a declared duration/request policy in its retained result.

### 2.5 Workflow validation blockers

The workflow validator checks naming and broad topology, but not actual command-line parser contracts, result schemas, exact category coverage, bundle digest binding, or evidence completeness.

The final gate checks job conclusions, but it cannot compensate for jobs whose result artifacts are malformed, generated from rerun substitutes, or not bound to the candidate identity.

### 2.6 Documentation blocker

The current status file still names an obsolete candidate SHA and marks tracks complete without retained exact-SHA evidence.

## 3. Scope

This pass is limited to release-qualification execution and evidence integrity.

Included:

- command-line contract cleanup;
- canonical candidate-bundle creation;
- candidate identity and digest binding;
- deterministic downstream artifact acquisition;
- release-blocking coverage for all eight Stage C categories;
- committed package-specific behavioral suites;
- normalized result artifacts;
- strict native proxy/TLS/timeout/shutdown/resource/soak proof;
- evidence assembly from retained artifacts only;
- independent evidence validation;
- fail-closed workflow validation and final gate;
- exact-SHA qualification execution;
- mechanically generated status reconciliation.

Excluded:

- new HTTPX public API surface;
- transport-engine replacement;
- performance optimization unrelated to qualification correctness;
- new protocol support;
- public publication of the controlled `httpx` replacement;
- compatibility claims beyond the pinned HTTPX 0.28.1 profile;
- unrelated refactors of the Rust core or Python facade.

## 4. Non-negotiable truth constraints

1. The current stage remains `Stage C candidate` until retained evidence validates.
2. A green ordinary CI run is necessary but not sufficient.
3. A qualification result is valid only for one full 40-character candidate SHA.
4. Every release-blocking result must carry the same candidate identity digest.
5. Wheels must be built once per qualification run and reused by all qualification jobs.
6. Evidence generation must not rerun a release-blocking suite.
7. A missing, skipped, cancelled, malformed, or zero-test required result is a failure.
8. No required command may use `|| true`, `|| echo`, `continue-on-error`, or catch-and-ignore behavior.
9. No positive native test may pass after catching an arbitrary exception.
10. Category coverage is counted only from passing release-blocking result records.
11. A source hash is valid only when it matches the exact installed artifact bytes.
12. The controlled replacement must be verified before dependency installation, after dependency installation, and after tests.
13. A matrix warning for missing required coverage is a qualification failure.
14. Dry-run mode may skip publishing, but it may not relax qualification.
15. Status documentation may be updated to released only by consuming validated evidence.

## 5. Target architecture

The qualification data flow must be linear and acyclic:

```text
exact candidate SHA
  -> Required CI Gate lookup
  -> build candidate wheels once
  -> canonical artifact manifest
  -> candidate identity
  -> bundle index
  -> release-blocking jobs
       -> normalized result artifacts
  -> result aggregation
  -> evidence generation
  -> independent evidence validation
  -> qualification summary
  -> generated status
```

The evidence job must consume result artifacts from prior jobs. It must not invoke pytest, rebuild wheels, reinstall a different candidate, or run downstream tests again.

## 6. Canonical candidate bundle

The bundle layout must be exactly:

```text
candidate-bundle/
  artifact-manifest.json
  candidate-identity.json
  bundle-index.json
  wheels/
    eggfetch-<version>-<tags>.whl
    httpx-0.28.1-py3-none-any.whl
```

No other wheel may be present in the canonical bundle.

### 6.1 Artifact manifest schema

Use a new or explicitly versioned manifest schema with these required fields:

```json
{
  "schema_version": "3",
  "candidate_sha": "<40 hex>",
  "run_id": "<GitHub run id>",
  "run_attempt": "<attempt>",
  "workflow_name": "Qualification",
  "producer_job": "normalize-candidate-artifacts",
  "generated_at": "<RFC3339 UTC>",
  "artifacts": [
    {
      "role": "eggfetch",
      "distribution": "eggfetch",
      "version": "<version>",
      "filename": "<wheel filename>",
      "relative_path": "wheels/<filename>",
      "sha256": "<64 hex>",
      "size_bytes": 1,
      "tags": "<wheel tags>"
    },
    {
      "role": "httpx-controlled-replacement",
      "distribution": "httpx",
      "version": "0.28.1",
      "filename": "httpx-0.28.1-py3-none-any.whl",
      "relative_path": "wheels/httpx-0.28.1-py3-none-any.whl",
      "sha256": "<64 hex>",
      "size_bytes": 1,
      "tags": "py3-none-any"
    }
  ]
}
```

Artifact role must be assigned from an explicit command-line input or source directory role, not inferred from whether the filename contains `controlled`.

The generator should accept either:

```text
--eggfetch-wheel <exact path>
--httpx-replacement-wheel <exact path>
```

or two role-specific directories that must each contain exactly one wheel.

Do not flatten unrelated wheel directories and then guess roles by filename.

### 6.2 Manifest CLI contract

Split generation and validation into explicit subcommands:

```text
python scripts/generate_artifact_manifest.py generate \
  --eggfetch-wheel <path> \
  --httpx-replacement-wheel <path> \
  --candidate-sha <sha> \
  --run-id <id> \
  --run-attempt <attempt> \
  --workflow-name Qualification \
  --producer-job normalize-candidate-artifacts \
  --bundle-dir <dir> \
  --output <dir>/artifact-manifest.json
```

```text
python scripts/generate_artifact_manifest.py validate \
  --manifest <dir>/artifact-manifest.json \
  --bundle-root <dir> \
  --expected-sha <sha>
```

Generation-only arguments must not be required by the validation subcommand.

Validation must recompute every recorded file size and SHA-256 from `bundle-root / relative_path`.

### 6.3 Candidate identity schema

Candidate identity must be generated after the final artifact manifest has been written.

Required fields:

```json
{
  "schema_version": "4",
  "candidate_sha": "<40 hex>",
  "artifact_manifest_sha256": "<64 hex of exact manifest bytes>",
  "eggfetch_version": "<version>",
  "reference_httpx_version": "0.28.1",
  "eggfetch_wheel": {
    "filename": "<filename>",
    "sha256": "<64 hex>"
  },
  "httpx_replacement_wheel": {
    "filename": "<filename>",
    "sha256": "<64 hex>"
  },
  "producer": "normalize-candidate-artifacts",
  "run_id": "<id>",
  "run_attempt": "<attempt>",
  "workflow_run_url": "<url>",
  "started_at": "<RFC3339 UTC>",
  "finished_at": "<RFC3339 UTC>",
  "identity_digest": "<64 hex>"
}
```

`identity_digest` is SHA-256 of canonical JSON excluding only `identity_digest`.

The artifact manifest must not embed the identity object. Embedding identity into the manifest while identity also contains the manifest digest creates a circular contract.

### 6.4 Bundle index

Add `bundle-index.json` to bind the two documents without circularity:

```json
{
  "schema_version": "1",
  "candidate_sha": "<sha>",
  "artifact_manifest": {
    "path": "artifact-manifest.json",
    "sha256": "<digest>"
  },
  "candidate_identity": {
    "path": "candidate-identity.json",
    "sha256": "<digest>"
  },
  "identity_digest": "<candidate identity digest>"
}
```

Bundle validation must verify all three files and both wheel hashes.

## 7. Unified release-blocking result contract

Every required job must emit `qualification-result/v1`.

Required top-level fields:

```json
{
  "schema": "qualification-result/v1",
  "suite_id": "<stable id>",
  "producer_job": "<job id>",
  "candidate_sha": "<40 hex>",
  "identity_digest": "<64 hex>",
  "run_id": "<id>",
  "run_attempt": "<attempt>",
  "started_at": "<RFC3339 UTC>",
  "finished_at": "<RFC3339 UTC>",
  "status": "passed",
  "required": true,
  "metrics": {},
  "artifacts": [],
  "diagnostics": []
}
```

Allowed required statuses are only `passed` and `failed`.

A required result with any other status is invalid.

Every producer must use a shared result-contract module for construction and validation.

### 7.1 Pytest adapter

Add a deterministic adapter:

```text
python scripts/normalize_pytest_result.py \
  --input raw-pytest.json \
  --suite-id native-proxy-tls \
  --candidate-identity candidate-identity.json \
  --required \
  --max-skipped 0 \
  --max-xfailed 0 \
  --output result.json
```

The adapter must read pytest-json-report's actual `summary` object and collect:

- collected;
- passed;
- failed;
- errors;
- skipped;
- xfailed;
- xpassed;
- duration.

Required pytest results must enforce:

- collected > 0;
- failed = 0;
- errors = 0;
- skipped = 0;
- xfailed = 0;
- xpassed = 0;
- passed = collected.

### 7.2 Generic result adapter

Non-pytest jobs such as resource monitoring, API comparison, workflow validation, and downstream aggregation must also emit the same envelope.

The payload specific to a suite belongs under `metrics` or a named `details` object, but identity fields remain top-level.

## 8. Downstream portfolio closure

### 8.1 Authoritative category registry

Define the exact Stage C required category set in one shared file or module:

```text
contract-tests
mock-transport-request-matching
framework-test-client
asgi-test-client
sdk-async-client
streaming-sse-consumption
custom-auth-flow
event-hooks-instrumentation
```

The manifest validator, matrix generator, result aggregator, evidence validator, and status generator must import or parse the same registry.

Do not duplicate category sets in multiple scripts.

### 8.2 Required coverage

Every category must have at least one release-blocking producer.

Recommended mapping:

| Category | Required producer |
|---|---|
| contract-tests | facade API oracle plus controlled replacement API oracle |
| mock-transport-request-matching | respx behavioral fixture |
| framework-test-client | pytest-httpx behavioral fixture |
| asgi-test-client | Starlette TestClient fixture |
| sdk-async-client | pinned Anthropic offline async-client fixture |
| streaming-sse-consumption | httpx-sse fixture |
| custom-auth-flow | httpx-auth fixture |
| event-hooks-instrumentation | dedicated event-hook fixture using a real consumer package or a committed instrumentation integration fixture |

If contract proof is provided by API-oracle jobs rather than a downstream package, the category aggregator must explicitly consume those oracle result artifacts. Do not keep an informational `httpx` manifest entry and count its metadata as release proof.

Promote Anthropic to required only after adding a committed offline fixture that materially constructs and uses its async HTTP integration with the controlled replacement. No external API key or network access is permitted.

Replace the current httpx-ws category assignment unless the fixture genuinely exercises event hooks or instrumentation. A category name must describe the behavior actually tested.

### 8.3 Committed behavioral fixture policy

Required package entries must point to committed test files, not inline `python -c` programs.

Example:

```text
pytest -q compat/downstream/fixtures/test_respx.py
```

Each fixture must:

- import the downstream package;
- assert `httpx.__eggfetch_shim__ is True`;
- materially exercise the package's HTTPX integration;
- use only loopback or in-process transports;
- assert request and response behavior;
- run at least one collected test;
- have zero skips and xfails;
- avoid private no-op imports as proof.

Required fixture files should include at minimum:

```text
compat/downstream/fixtures/test_respx.py
compat/downstream/fixtures/test_pytest_httpx.py
compat/downstream/fixtures/test_starlette.py
compat/downstream/fixtures/test_anthropic_async.py
compat/downstream/fixtures/test_httpx_sse.py
compat/downstream/fixtures/test_httpx_auth.py
compat/downstream/fixtures/test_event_hooks.py
```

### 8.4 Immutable downstream artifact declaration

Each required entry must declare the exact artifact to download:

```toml
source-type = "pypi-wheel"
source-filename = "respx-0.21.1-py2.py3-none-any.whl"
source-url = "https://files.pythonhosted.org/.../respx-0.21.1-py2.py3-none-any.whl"
source-sha256 = "<64 hex>"
```

Package name and version alone are insufficient because a release can contain multiple wheels and an sdist.

The acquisition script must download exactly `source-url`, require the final filename to equal `source-filename`, and verify the exact bytes against `source-sha256`.

Record actual URL, filename, byte count, and digest in the result.

### 8.5 Deterministic dependency installation

Do not allow unconstrained optional dependencies to reinstall upstream HTTPX.

For each required package:

1. create a clean virtual environment;
2. install the exact eggfetch wheel;
3. install the exact controlled replacement wheel;
4. verify replacement identity;
5. install the exact downstream wheel using `--no-deps`;
6. install a pinned dependency lock that excludes `httpx`;
7. run `pip check`;
8. verify replacement identity again;
9. run the committed behavioral fixture;
10. verify replacement identity a third time;
11. emit a normalized result.

Each required entry must have a dependency lock or exact dependency list. Bare names such as `pytest`, `httpx`, `anyio`, or `pydantic` are not sufficient for release qualification.

The runner must fail if `importlib.metadata.distribution("httpx")` does not refer to the controlled replacement metadata.

### 8.6 Matrix contract

The generated GitHub Actions matrix should contain identifiers and normalized scalar fields only:

```json
{
  "include": [
    {
      "package_id": "respx",
      "version": "0.21.1",
      "source_sha256": "<hash>",
      "timeout_seconds": 60
    }
  ]
}
```

Do not place shell commands or hyphenated keys in the matrix.

Use compact single-line JSON for `$GITHUB_OUTPUT`:

```bash
MATRIX=$(jq -c . /tmp/downstream-matrix.json)
echo "matrix=$MATRIX" >> "$GITHUB_OUTPUT"
```

The downstream job should call one runner by package ID:

```text
python scripts/run_isolated_downstream.py \
  --package-id "${{ matrix.package_id }}" \
  --manifest compat/downstream/manifest.toml \
  --bundle-root candidate-bundle \
  --candidate-identity candidate-bundle/candidate-identity.json \
  --output result.json
```

The runner, not the workflow YAML, resolves commands, files, dependencies, hashes, and policy.

### 8.7 Downstream aggregation

Each matrix leg uploads a uniquely named result artifact.

A dedicated aggregation job must:

- download every expected matrix result;
- verify exact set equality between expected package IDs and returned package IDs;
- validate every result envelope;
- require every required package status to be `passed`;
- compute category coverage only from passing results;
- merge contract category results from API-oracle jobs when configured;
- require exact equality with the eight-category registry;
- emit one normalized `downstream-portfolio` result.

Missing, duplicate, unexpected, malformed, or identity-mismatched results fail aggregation.

## 9. API-oracle closure

Maintain exact typed difference matching.

Create separate normalized results for:

- `facade-api-oracle` comparing upstream HTTPX 0.28.1 with `eggfetch.compat.httpx`;
- `replacement-api-oracle` comparing upstream HTTPX 0.28.1 with the installed controlled `httpx` replacement.

Both results must include:

- candidate SHA;
- identity digest;
- difference count;
- allowed match count;
- unexplained difference count;
- stale allowance count;
- resolved-in-active count;
- allowed-difference ledger hash.

Required pass criteria:

- unexplained = 0;
- stale = 0;
- resolved-in-active = 0;
- every active allowance matched exactly once unless explicitly marked as version-bounded historical evidence outside the active file.

The contract-tests Stage C category is satisfied only if both oracle results pass.

## 10. Strict native proof

### 10.1 Proxy success

Positive proxy tests must fail on any exception.

Required proof:

- plain HTTP GET through configured proxy reaches backend;
- plain HTTP POST body survives proxy forwarding;
- HTTPS request through CONNECT tunnel completes and returns asserted content;
- proxy observes at least one CONNECT request for HTTPS;
- request and response bodies are fully read;
- client and fixture endpoints close within bounds.

The proxy fixture must expose recorded methods and targets so tests can assert that traffic actually traversed the proxy.

### 10.2 CONNECT failure and stall

Add deterministic fixtures for:

- proxy TCP refusal;
- CONNECT response refusal;
- CONNECT response stall;
- upstream tunnel refusal after CONNECT target parsing.

Each case must assert one exact compatibility exception class and attached request context.

Broad tuples that include base `Exception`, `TransportError`, or multiple unrelated classes are not acceptable.

Where the candidate intentionally differs from HTTPX, record one typed, reviewed compatibility difference and test that exact candidate class.

### 10.3 TLS proof

Required cases:

- trusted self-signed certificate succeeds;
- untrusted certificate fails with the exact intended class;
- hostname mismatch fails with the exact intended class;
- TLS handshake stall fails with the exact timeout class;
- HTTPS through CONNECT with trusted certificate succeeds;
- exception retains originating request.

No positive TLS test may catch and ignore an error.

### 10.4 Timeout classification

Required native timeout classes:

- connect timeout;
- read timeout;
- write timeout where supported;
- pool timeout where supported;
- proxy CONNECT timeout;
- TLS handshake timeout.

Fixtures must model timeouts rather than connection refusal.

Assertions based only on exception strings are not sufficient.

### 10.5 Shutdown and ownership

Add a subprocess-based native shutdown suite with bounded deadlines.

Required scenarios:

- unclosed client with no requests;
- unclosed client after completed request;
- unclosed response with unread body;
- active stalled native request during interpreter shutdown;
- async task cancellation while a native request is active;
- generator/auth-flow cancellation during dispatch;
- repeated client creation followed by process exit.

Each subprocess result must record:

- exit code;
- wall-clock duration;
- stderr excerpt;
- whether a fatal interpreter error occurred;
- whether a timeout or forced kill occurred.

Required pass criteria:

- exit code 0;
- no hang;
- no fatal Python error;
- no panic;
- no unjoined non-daemon thread;
- no leaked child process.

### 10.6 Resource policy

Parse `compat/httpx/0.28.1/resource-thresholds.toml` or its replacement as executable policy.

The resource result must include:

- selected platform profile;
- baseline and final file descriptor counts;
- baseline and final thread counts;
- peak and final RSS;
- request count;
- client-cycle count;
- threshold values;
- pass/fail per metric.

Missing platform profile or unparsed policy is a failure.

### 10.7 Soak policy

Define two explicit modes:

Qualification soak:

- bounded for normal release qualification;
- zero unexpected errors;
- all scheduled operations complete;
- declared minimum request count;
- declared minimum duration if duration-based.

Scheduled retained soak:

- at least 300 seconds or the currently approved stricter policy;
- at least 500 completed requests;
- same candidate identity contract;
- retained result artifact.

The result must state which mode ran and the exact policy values.

## 11. Evidence assembly

### 11.1 No reruns

The evidence job must not execute:

- pytest;
- downstream runners;
- API manifest generation;
- wheel builds;
- resource monitors;
- soak tests.

It may only download, validate, and aggregate retained result artifacts.

### 11.2 Required evidence inputs

The evidence job must consume normalized results for:

- ordinary CI verification;
- artifact bundle validation;
- facade API oracle;
- replacement API oracle;
- compatibility test matrix aggregation;
- downstream portfolio aggregation;
- shim substitution;
- native timeout classification;
- native proxy/TLS;
- native shutdown;
- native resource policy;
- qualification soak;
- workflow-contract validation.

Every result must have the exact same candidate SHA and identity digest.

### 11.3 Artifact verification

Artifact verification must resolve `bundle_root / relative_path` from the artifact manifest.

Do not search guessed locations such as `target/wheels`, `dist`, or repository root.

Recompute:

- manifest byte digest;
- candidate identity byte digest;
- identity digest;
- bundle index digests;
- wheel SHA-256 and size.

### 11.4 Evidence pass calculation

`overall_pass` must be independently computed from all required result statuses and bundle validation.

It must not trust an input's aggregate `overall_pass` without validating its child records.

Required conditions:

- every expected result exists exactly once;
- every result schema validates;
- every result status is `passed`;
- every candidate SHA matches;
- every identity digest matches;
- all eight Stage C categories are proven;
- artifact hashes and sizes match;
- API-oracle unexplained and stale counts are zero;
- required test counts are nonzero;
- required skips and xfails are zero;
- resource thresholds pass;
- soak policy passes;
- workflow validation passes.

### 11.5 Independent validation

The independent validator must recompute the pass decision without importing the evidence generator's `overall_pass` calculation.

It must fail when any single required result is removed, duplicated, changed to skipped, identity-mismatched, or hash-mismatched.

## 12. Workflow rewiring

### 12.1 Job graph

Required job graph:

```text
verify-candidate
  -> build-candidate-artifacts
  -> normalize-candidate-bundle
      -> bundle-contract-test
      -> compat-tests[*]
      -> facade-api-oracle
      -> replacement-api-oracle
      -> prepare-downstream-matrix
          -> downstream[*]
          -> downstream-aggregate
      -> shim-substitution
      -> native-timeout
      -> native-proxy-tls
      -> native-shutdown
      -> native-resource
      -> qualification-soak
      -> workflow-contract
  -> evidence-assemble
  -> evidence-validate
  -> qualification-gate
  -> status-generate
```

Each job referencing `needs.<job>.outputs` must directly list that job in `needs`.

Every checkout after candidate resolution must use the exact candidate SHA, including the qualification-gate and status-generation jobs.

### 12.2 Candidate verification

The verify job must establish that the Required CI Gate belongs to the exact candidate SHA and completed successfully.

Record:

- check run ID;
- workflow run ID, not check-suite ID;
- check URL;
- attempt;
- completed timestamp;
- candidate SHA.

Emit this as a normalized required result.

### 12.3 Artifact retention

All release-blocking result artifacts and final evidence must have a retention period sufficient for release review.

Use one stable artifact name per suite and include the candidate SHA in contained JSON, not necessarily in the artifact name.

### 12.4 Dry-run semantics

`dry_run=true` means:

- execute all qualification jobs;
- build all evidence;
- fail on every qualification defect;
- skip publication or release mutation only.

The final gate must behave identically in dry-run and release mode except for publication steps.

### 12.5 Scheduled runs

A scheduled run must resolve an explicit full commit SHA and satisfy the same Required CI Gate check.

Do not qualify a moving branch reference.

## 13. Workflow contract validator

Expand `validate_qualification_workflow.py` into an actual interface validator.

Required checks:

1. Parse every invoked repository Python script with its real argparse parser or exported command schema.
2. Reject missing required arguments.
3. Reject unknown arguments.
4. Verify subcommand names.
5. Verify every downloaded artifact has a producer.
6. Verify every evidence input artifact has a direct dependency.
7. Verify exact-SHA checkout in every release-blocking job.
8. Verify no suppression patterns.
9. Verify no required job has `continue-on-error`.
10. Verify matrix source is the manifest generator output.
11. Verify matrix does not contain shell command fields.
12. Verify all eight category IDs are enforced as errors when missing.
13. Verify candidate bundle consumers use the canonical bundle.
14. Verify evidence job contains no test or build commands.
15. Verify evidence includes every required result suite.
16. Verify final gate depends on evidence validation.
17. Verify dry-run does not bypass failure handling.
18. Verify resource and soak result producers exist.
19. Verify status generation depends on successful qualification gate.
20. Verify the controlled replacement is installed and checked in every downstream environment.

Add negative workflow fixtures for every class of defect.

The validator documentation must describe only checks that the implementation actually performs.

## 14. Ordered implementation phases

### Phase 0 — Freeze truth and add executable reproductions

1. Record baseline `5681139c9408531ece8f6d354d682d18d2e174a5`.
2. Keep stage at `Stage C candidate`.
3. Add focused tests reproducing:
   - missing manifest `--output`;
   - validate-only parser contamination;
   - replacement wheel misclassification;
   - missing manifest digest in identity;
   - obsolete downstream CLI in evidence;
   - raw pytest summary misparse;
   - bundle-relative artifact lookup failure;
   - missing resource evidence;
   - six-of-eight category false green;
   - catch-and-pass CONNECT false green.
4. Require each reproduction to fail before fixes.

Checkpoint: no implementation changes are accepted without a deterministic failing test for the audited defect.

### Phase 1 — Canonical bundle contracts

1. Split manifest CLI into generate/validate subcommands.
2. Replace filename guessing with role-specific wheel inputs.
3. Write schema v3 manifest.
4. Generate schema v4 identity after manifest creation.
5. Write bundle index.
6. Add independent bundle validator.
7. Add tamper, duplicate-wheel, extra-wheel, missing-wheel, traversal, wrong-role, wrong-version, and digest mismatch tests.

Checkpoint: one local command creates and validates a complete bundle from exactly two wheels.

### Phase 2 — Shared result contract and adapters

1. Implement `qualification-result/v1` module.
2. Implement pytest-json-report adapter.
3. Implement generic result emitter.
4. Convert API oracle, workflow validator, resource monitor, and bundle validator outputs.
5. Add schema and identity mismatch tests.

Checkpoint: every release-blocking producer can emit a validated result envelope.

### Phase 3 — Downstream manifest and artifact acquisition

1. Add the shared category registry.
2. Replace source locator ambiguity with exact source filename/URL/hash.
3. Add dependency locks.
4. Make missing category coverage fatal.
5. Normalize field names to underscore keys in generated matrix output.
6. Remove test commands from matrix output.
7. Make acquisition install the exact verified bytes.

Checkpoint: matrix generation fails unless all required categories have a release-blocking producer.

### Phase 4 — Behavioral downstream suites

1. Replace inline commands with committed pytest files.
2. Repair respx proof.
3. Exercise pytest-httpx fixture behavior directly.
4. Exercise Starlette TestClient end to end.
5. Add offline Anthropic async integration proof.
6. Exercise SSE iteration.
7. Exercise custom auth flow.
8. Add genuine event-hook/instrumentation proof.
9. Verify controlled replacement before, after install, and after tests.
10. Emit normalized per-package results.

Checkpoint: every required package result has collected > 0 and all tests pass with zero skips/xfails.

### Phase 5 — Downstream aggregation and API category proof

1. Aggregate exact matrix result set.
2. Add facade and replacement API-oracle normalized results.
3. Bind contract category to both passing oracle results.
4. Compute category coverage from passing results only.
5. Reject duplicates, missing packages, unexpected packages, and identity mismatches.

Checkpoint: the aggregate proves exact equality with the eight-category registry.

### Phase 6 — Strict native proxy/TLS/timeout proof

1. Add proxy request recording.
2. Remove catch-and-pass blocks.
3. Require successful CONNECT and HTTPS response completion.
4. Add deterministic refusal and stall fixtures.
5. Tighten exact exception classifications.
6. Add hostname mismatch and handshake stall proof.
7. Normalize result artifacts.

Checkpoint: deliberately breaking proxy routing or TLS verification makes the suite fail.

### Phase 7 — Shutdown, resource, and soak proof

1. Add native subprocess shutdown scenarios.
2. Parse executable resource policy.
3. Emit a dedicated resource result.
4. Define qualification and scheduled soak policies.
5. Emit normalized soak result with policy values.
6. Add leak and timeout negative tests.

Checkpoint: resource or shutdown leakage cannot produce a passing result.

### Phase 8 — Workflow rewiring

1. Replace normalization commands with final CLIs.
2. Produce canonical bundle once.
3. Use compact generated matrix output.
4. Run downstream through the package-ID runner.
5. Add aggregation jobs.
6. Normalize every result before upload.
7. Add exact candidate SHA checkout everywhere.
8. Remove reruns from evidence job.
9. Add evidence validation job.
10. Make final gate depend on all required jobs.

Checkpoint: workflow validator and local workflow simulation pass.

### Phase 9 — Evidence and independent validation

1. Assemble evidence only from retained results.
2. Verify bundle-relative files.
3. Recompute every digest.
4. Recompute overall pass across all required suites.
5. Validate all eight categories.
6. Add missing/duplicate/tampered/identity-mismatch negative tests.

Checkpoint: removing any one input causes both generation or validation to fail.

### Phase 10 — Exact-SHA qualification execution

1. Choose a candidate SHA after all release-relevant fixes.
2. Run ordinary CI for that exact SHA.
3. Confirm Required CI Gate success.
4. Dispatch Qualification with the full SHA.
5. Run with dry-run enabled first; dry-run must still fully qualify.
6. Inspect all required job results.
7. Download candidate bundle, evidence, independent validation result, and qualification summary.
8. Verify every embedded SHA and identity digest.
9. Rerun only after a code change creates a new candidate SHA.

Checkpoint: one exact run has `overall_pass=true` and no release-relevant commit follows it.

### Phase 11 — Status reconciliation

1. Generate status from validated evidence.
2. Record candidate SHA, run ID, attempt, run URL, wheel filenames, wheel hashes, identity digest, evidence digest, and qualification summary digest.
3. Remove stale SHA and unsupported completion claims.
4. Mark Stage C released only if all release conditions are mechanically true.

Checkpoint: status generation fails when evidence does not validate.

## 15. Mandatory negative tests

The following negative cases are required. A test that merely observes a nonzero exit without validating the expected structured diagnostic is insufficient.

### Bundle and identity

1. No eggfetch wheel.
2. No replacement wheel.
3. Duplicate eggfetch wheels.
4. Duplicate replacement wheels.
5. Extra unlisted wheel.
6. Replacement distribution named `httpx` but assigned wrong role.
7. Replacement version not 0.28.1.
8. Wheel hash mismatch.
9. Wheel size mismatch.
10. Absolute artifact path.
11. Path traversal.
12. Candidate SHA mismatch.
13. Manifest digest mismatch.
14. Identity digest mismatch.
15. Bundle-index digest mismatch.
16. Manifest generated after identity without rebinding.

### Result contracts

17. Missing candidate SHA.
18. Missing identity digest.
19. Wrong identity digest.
20. Unknown schema.
21. Required result marked skipped.
22. Required result marked informational.
23. Zero collected tests.
24. One skipped test.
25. One xfailed test.
26. One xpassed test.
27. Malformed pytest report.
28. Raw pytest report lacking summary.

### Downstream

29. Missing required category.
30. Category represented only by informational package.
31. Duplicate package ID.
32. Unexpected package result.
33. Missing matrix result.
34. Wrong exact source filename.
35. Wrong source URL.
36. Source hash mismatch.
37. Source artifact verified but different artifact installed.
38. Upstream HTTPX replaces controlled replacement.
39. Bare optional dependency changes environment.
40. `pip check` failure.
41. Import-only required fixture.
42. Fixture imports downstream package but does not exercise it.
43. Respx command syntax failure.
44. pytest-httpx fixture not used.
45. SDK fixture attempts external network.
46. Event-hook category without hook assertion.

### Native

47. Proxy configured but request bypasses proxy.
48. CONNECT never observed.
49. HTTPS request fails but test catches exception.
50. TLS untrusted certificate unexpectedly succeeds.
51. Hostname mismatch unexpectedly succeeds.
52. Handshake stall maps to wrong class.
53. Connection refusal mislabeled as timeout.
54. Exception lacks request context.
55. Shutdown subprocess hangs.
56. Shutdown subprocess exits nonzero.
57. Fatal interpreter error appears on stderr.
58. Resource threshold file missing.
59. Resource profile missing for platform.
60. FD threshold exceeded.
61. Thread threshold exceeded.
62. RSS threshold exceeded.
63. Soak request count below policy.
64. Soak duration below policy.
65. One unexpected soak error.

### Workflow and evidence

66. Workflow invokes unknown script argument.
67. Workflow omits required script argument.
68. Workflow uses obsolete `--wheel-dir` downstream interface.
69. Workflow embeds shell command in matrix.
70. Workflow uses hyphenated matrix expression key.
71. Workflow writes multiline JSON incorrectly to `$GITHUB_OUTPUT`.
72. Evidence job invokes pytest.
73. Evidence job invokes downstream runner.
74. Evidence omits resource result.
75. Evidence omits one API oracle result.
76. Evidence consumes two results with different identity digests.
77. Evidence searches guessed wheel path.
78. Independent validator trusts top-level `overall_pass` after child tampering.
79. Final gate accepts skipped job.
80. Dry-run suppresses a qualification failure.
81. Qualification-gate checks out branch head rather than candidate SHA.
82. Status names a SHA different from evidence.

## 16. Required local validation commands

The implementation should provide or support commands equivalent to:

```bash
python -m pytest scripts/tests/test_artifact_manifest.py -q
python -m pytest scripts/tests/test_candidate_identity.py -q
python -m pytest scripts/tests/test_qualification_result_contract.py -q
python -m pytest scripts/tests/test_downstream_matrix.py -q
python -m pytest scripts/tests/test_downstream_runner.py -q
python -m pytest scripts/tests/test_qualification_workflow.py -q
python -m pytest scripts/tests/test_compatibility_evidence.py -q
```

```bash
python scripts/generate_downstream_matrix.py \
  --manifest compat/downstream/manifest.toml \
  --validate-only
```

```bash
python scripts/validate_qualification_workflow.py \
  .github/workflows/qualification.yml
```

```bash
pytest crates/eggfetch-python/tests/compat/test_native_proxy_tls.py -q
pytest crates/eggfetch-python/tests/compat/test_native_timeout_classification.py -q
pytest crates/eggfetch-python/tests/compat/test_shutdown_native.py -q
pytest crates/eggfetch-python/tests/compat/test_soak.py -q
```

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

## 17. Global acceptance criteria

All criteria are mandatory.

1. Current stage remains candidate until exact-SHA qualification passes.
2. Manifest CLI has separate generate and validate contracts.
3. Validation does not require generation-only arguments.
4. Controlled replacement role is not inferred from `controlled` in filename.
5. Bundle contains exactly two wheels.
6. Manifest records exact relative paths, hashes, sizes, versions, and roles.
7. Identity contains the exact manifest byte digest.
8. Bundle index binds manifest and identity without circularity.
9. Independent bundle validation passes.
10. Every release-blocking job emits `qualification-result/v1`.
11. Every result carries exact candidate SHA and identity digest.
12. Raw pytest reports are normalized from their actual summary schema.
13. Required pytest results have nonzero collection and zero skip/xfail/failure/error.
14. One shared registry defines all eight Stage C categories.
15. Missing required category is an error, not a warning.
16. All eight categories have passing release-blocking proof.
17. Required downstream tests are committed files, not inline shell programs.
18. Every required fixture materially exercises its downstream integration.
19. Anthropic or another pinned SDK provides offline async-client proof.
20. Event-hook category proves actual hook or instrumentation behavior.
21. Exact downstream filename, URL, and hash are declared.
22. Exact verified downstream bytes are the bytes installed.
23. Dependency installation cannot replace controlled HTTPX.
24. Controlled replacement identity is checked three times per downstream run.
25. Matrix is generated from manifest and contains identifiers only.
26. Matrix JSON is emitted safely as compact output.
27. Expected and actual downstream result sets are exactly equal.
28. Category coverage is computed from passing results only.
29. Facade and replacement API oracles both pass.
30. Positive proxy tests fail on any exception.
31. HTTPS CONNECT proof returns and validates a real response.
32. Proxy, TLS, and timeout failures assert exact intended classes.
33. Shutdown proof uses native subprocess scenarios with bounded exit.
34. Resource policy is parsed and enforced.
35. Qualification soak meets declared policy with zero unexpected errors.
36. Evidence job runs no tests and builds no artifacts.
37. Evidence consumes every required retained result.
38. Evidence resolves artifact files from bundle-relative manifest paths.
39. Evidence recomputes all hashes and identity digests.
40. Evidence overall pass includes every required suite.
41. Independent validator recomputes the decision.
42. Workflow validator checks real CLI contracts.
43. Workflow validator checks all required evidence inputs.
44. Workflow validator rejects suppression patterns.
45. Every release-blocking checkout uses exact candidate SHA.
46. Final gate requires evidence validation success.
47. Dry-run remains fully fail-closed.
48. Required CI Gate belongs to the exact candidate SHA.
49. One exact-SHA Qualification run completes successfully.
50. Candidate bundle, evidence, validation, and summary artifacts are retained.
51. All retained artifacts name the same SHA and identity digest.
52. No release-relevant commit follows qualification without requalification.
53. Status is generated from evidence.
54. Status records exact run and artifact identifiers.
55. Stage C released is declared only after all prior criteria pass.

## 18. Suggested commit decomposition

Keep implementation reviewable:

1. `test: reproduce final qualification execution blockers`
2. `fix: make candidate bundle and identity contracts executable`
3. `feat: add unified qualification result contract`
4. `fix: make downstream portfolio immutable and category complete`
5. `test: replace downstream inline checks with behavioral fixtures`
6. `fix: enforce strict native proxy TLS timeout and shutdown proof`
7. `fix: assemble evidence from retained normalized results`
8. `fix: validate workflow interfaces and fail-closed graph`
9. `docs: reconcile exact-SHA qualification status from evidence`

Do not combine all tracks into one opaque commit unless repository constraints require it.

## 19. Stop conditions

Stop and retain `Stage C candidate` if any of the following is true:

- ordinary CI is not green for the exact SHA;
- the bundle cannot independently validate;
- candidate identity cannot bind to the manifest;
- any required category lacks passing proof;
- any downstream environment imports upstream HTTPX;
- any required suite skips or xfails;
- a positive proxy or TLS test catches and ignores errors;
- shutdown or resource proof is incomplete;
- evidence reruns qualification suites;
- evidence omits a required result;
- independent validation disagrees with evidence;
- the final gate accepts a non-success job;
- status names an unqualified SHA;
- a release-relevant commit lands after the qualified SHA.

## 20. Handoff checklist

Before implementation:

- [ ] Confirm baseline SHA.
- [ ] Read this plan and the two prior qualification closure plans.
- [ ] Reproduce all deterministic blockers locally.
- [ ] Do not update release status.

During implementation:

- [ ] Add failing tests before each contract repair.
- [ ] Keep artifact, identity, result, and evidence schemas versioned.
- [ ] Use one category registry.
- [ ] Keep all required paths fail closed.
- [ ] Verify controlled replacement identity repeatedly.
- [ ] Avoid shell-embedded behavioral commands.

Before qualification:

- [ ] Ordinary CI green on exact SHA.
- [ ] Bundle validator green.
- [ ] Workflow contract validator green.
- [ ] All local negative tests green.
- [ ] No stale status update.

After qualification:

- [ ] Download retained candidate bundle.
- [ ] Download every normalized result.
- [ ] Download evidence and independent validation result.
- [ ] Download qualification summary.
- [ ] Verify SHA and identity digest equality.
- [ ] Generate status from evidence.
- [ ] Confirm no subsequent release-relevant commit.

## 21. End condition

This line of work is closed only when the repository has one retained, independently validated Qualification run for one exact candidate SHA, every required result is bound to the same canonical candidate identity, all eight Stage C categories have passing release-blocking proof, strict native lifecycle evidence passes, the final gate is fail closed, and the status document is generated from that evidence.

Until then, the correct repository state is:

`Stage C candidate — qualification closure in progress.`
