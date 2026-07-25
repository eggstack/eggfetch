# HTTPX Drop-In Qualification Execution Corrective Pass

Status: planning-only corrective handoff

Repository: `eggstack/eggfetch`

Implementation target: PR #12, branch `plans/httpx-release-qualification-final-closure`

Audited implementation head: `8c80e00a6ce58dc6249f38b5c69902d0a93bd457`

Current `main` baseline: `f24b62b552486ac8b35eb8d60210304a5e67cee9`

Compatibility classification before this pass: **Stage C candidate**

Release decision before this pass: **do not merge PR #12 and do not claim Stage C released**

## 1. Purpose

This plan closes the remaining execution and evidence-integrity defects in the HTTPX compatibility qualification work currently staged in PR #12.

The current branch contains useful component-level work, including stronger API-difference governance, downstream behavioral fixtures, artifact and identity schemas, native test files, and a substantially expanded qualification workflow. Ordinary CI, Security, and Benchmarks are green for the audited PR head.

However, the release-qualification path is still non-executable and semantically unsafe. The principal failures are not broad product gaps. They are integration defects between scripts, workflow jobs, downstream environments, result contracts, evidence assembly, and the final release gate.

This pass must therefore remain narrow. It must not reopen unrelated HTTP client semantics, refactor the core architecture, or expand the supported compatibility surface. It must make the existing qualification design executable, fail-closed, identity-preserving, and independently verifiable.

## 2. Required outcome

At completion, one exact candidate SHA must have all of the following retained against the same candidate identity:

1. A green ordinary `Required CI Gate`.
2. A successful non-dry-run `Qualification` workflow.
3. One canonical normalized artifact bundle containing the exact eggfetch wheel and controlled replacement `httpx` wheel.
4. One canonical candidate identity whose digest is embedded in every release-blocking result.
5. Passing facade and top-level shim API oracles.
6. Passing downstream substitution suites that demonstrably execute the controlled replacement rather than upstream HTTPX.
7. Immutable, hash-verified downstream package artifacts installed from the exact bytes that were verified.
8. Release-blocking proof for all eight Stage C downstream categories.
9. Passing native timeout, proxy, TLS, shutdown, resource, concurrency, and soak suites.
10. Compatibility evidence generated from retained result artifacts without rerunning or synthesizing results.
11. Independent evidence validation that recomputes the final decision.
12. A final summary gate that fails on any failed, missing, malformed, skipped, stale, identity-mismatched, or unverified release-blocking input.
13. Status documentation mechanically updated to the exact qualified SHA and workflow run only after all prior criteria pass.

## 3. Confirmed defects to correct

### 3.1 Script and workflow CLI mismatches

The workflow currently calls `generate_artifact_manifest.py` with arguments that the script does not implement.

The workflow currently treats `candidate_identity.py` as a generator, while the current script CLI only validates positional identity files.

The workflow currently calls `validate_qualification_workflow.py` without its required positional workflow path.

The facade and shim API-oracle jobs pass unsupported arguments to `compare_httpx_api_manifest.py`.

These are deterministic argument-parsing failures. They must be corrected before any workflow execution claim is meaningful.

### 3.2 Invalid cross-job output dependencies

`normalize-candidate-artifacts` consumes `needs.verify-candidate.outputs.*` without declaring `verify-candidate` as a direct dependency.

Every job must declare every job whose outputs it references. The workflow validator must reject indirect or undeclared output access.

### 3.3 Downstream suites execute upstream HTTPX

The downstream matrix jobs install the eggfetch wheel and then install downstream packages normally. They do not install the controlled replacement `httpx` wheel.

The `httpx` contract entry explicitly installs upstream `httpx==0.28.1`.

A downstream suite can therefore pass while proving only upstream HTTPX behavior. This invalidates the substitution claim.

### 3.4 Downstream source hashes are descriptive rather than enforced

The matrix copies `source-hash` into result JSON but does not verify the installed artifact bytes against it.

The exact artifact whose hash is verified must be the artifact that is installed. Resolving by package name after verification is prohibited.

### 3.5 Generated downstream matrix is not authoritative

The workflow generates `downstream-matrix.json`, uploads it, then ignores it and executes a separately duplicated static YAML matrix.

There must be one source of truth. The runtime matrix must be generated from `compat/downstream/manifest.toml` and consumed directly by the matrix job.

### 3.6 Downstream result contracts are incomplete

Current inline result construction does not reliably enforce:

- `min-passed`;
- `max-skipped`;
- `max-xfailed`;
- installed version equality;
- verified source hash;
- controlled replacement identity before and after dependency installation;
- required category coverage from passing results only;
- malformed or missing pytest reports as hard failure.

Synthetic zero-count fallback reports are not acceptable release evidence.

### 3.7 Evidence inputs have the wrong shape

The workflow currently passes the same aggregate file as every evidence input.

The evidence generator expects each file to be the contract for that specific result section. A nested aggregate is not interchangeable with a direct suite result.

### 3.8 Failure suppression remains in release-blocking steps

Evidence generation and independent evidence validation currently use `|| echo ...` suppression.

No release-blocking command may suppress a nonzero exit. A missing evidence file must cause artifact upload and final-gate failure, not a warning-only path.

### 3.9 Final gate tolerates incomplete qualification

The current summary gate:

- treats `skipped` as acceptable for required jobs;
- omits evidence generation and independent validation from its required result set;
- weakens failures when `dry_run` is true;
- derives status from opportunistically discovered JSON files rather than authoritative `needs.<job>.result` and validated evidence.

Dry-run may disable publication or attestation. It must not weaken qualification correctness.

### 3.10 Native proxy, TLS, and shutdown tests remain too weak

The current CONNECT test opens a client but does not send a request through the proxy.

TLS handshake and invalid-proxy tests accept broad `Exception` classes.

The shutdown suite tests normal context-manager closure but does not prove interpreter teardown, abandoned resources, partial native streams, cancellation, or bounded subprocess exit.

### 3.11 Status is stale and overstates completion

The existing status file names an older SHA and marks tracks complete despite the current workflow being non-executable.

Status must remain Stage C candidate until exact-SHA qualification artifacts exist and independently validate.

## 4. Non-goals

Do not add new HTTPX public APIs unless a failing oracle or downstream fixture proves they are required for the declared Stage C surface.

Do not broaden support beyond HTTPX 0.28.1 in this pass.

Do not add new downstream categories beyond the existing eight.

Do not replace the Rust transport stack or redesign the Python compatibility facade.

Do not optimize performance unless a qualification threshold fails and a targeted correction is necessary.

Do not commit generated qualification evidence, wheel files, or transient pytest reports to the repository.

Do not merge PR #12 merely because ordinary CI remains green.

## 5. Implementation rules

1. Continue implementation on PR #12 or a replacement branch created from audited head `8c80e00a6ce58dc6249f38b5c69902d0a93bd457`.
2. Preserve the current ordinary CI baseline.
3. Make script CLIs authoritative before editing workflow call sites.
4. Add parser-contract tests for every release-facing script.
5. Prefer committed Python helper scripts over complex inline workflow Python.
6. Every release-blocking result must use one versioned result contract.
7. Every result must carry the exact `candidate_sha` and `identity_digest`.
8. Every artifact path must be relative to one downloaded normalized bundle root.
9. No required workflow step may use `continue-on-error`, `|| true`, `|| echo`, broad fallback data, or warning-only failure semantics.
10. Required jobs may not be accepted as skipped.
11. All release assertions must be derived from retained artifacts, not commit messages or status prose.
12. Any release-relevant commit after qualification invalidates the prior qualification and requires a new run.

## 6. Canonical contracts

### 6.1 Normalized candidate bundle

The normalization job must upload one artifact named `candidate-bundle` with this structure:

```text
candidate-bundle/
  artifact-manifest.json
  candidate-identity.json
  wheels/
    eggfetch-<version>-<tags>.whl
    httpx-0.28.1-py3-none-any.whl
```

No later job should separately download the two original build artifacts. All consumers must download `candidate-bundle`.

### 6.2 Artifact manifest

`artifact-manifest.json` must include:

```json
{
  "schema_version": "3",
  "candidate_sha": "<40-char SHA>",
  "producer": {
    "workflow": "Qualification",
    "job": "normalize-candidate-artifacts",
    "run_id": "<run id>",
    "run_attempt": "<attempt>",
    "run_url": "<URL>"
  },
  "artifacts": [
    {
      "artifact_type": "eggfetch",
      "distribution": "eggfetch",
      "version": "<version>",
      "filename": "<wheel filename>",
      "path": "wheels/<wheel filename>",
      "sha256": "<64-char SHA-256>",
      "size_bytes": 123
    },
    {
      "artifact_type": "httpx-controlled-replacement",
      "distribution": "httpx",
      "version": "0.28.1",
      "filename": "httpx-0.28.1-py3-none-any.whl",
      "path": "wheels/httpx-0.28.1-py3-none-any.whl",
      "sha256": "<64-char SHA-256>",
      "size_bytes": 123
    }
  ]
}
```

The manifest generator must copy the exact selected wheels into the bundle and reject zero or multiple matching controlled replacement wheels.

For eggfetch ABI wheels, either build exactly one Python 3.12 ABI-compatible wheel for the release qualification job or define deterministic wheel selection by interpreter and platform. Do not pass a shell glob that may expand to several positional arguments where one file is expected.

### 6.3 Candidate identity

`candidate-identity.json` must include:

```json
{
  "schema_version": "3",
  "candidate_sha": "<40-char SHA>",
  "eggfetch_version": "<version>",
  "reference_httpx_version": "0.28.1",
  "artifact_manifest_sha256": "<64-char SHA-256>",
  "eggfetch_wheel": {
    "filename": "<filename>",
    "sha256": "<64-char SHA-256>"
  },
  "httpx_replacement_wheel": {
    "filename": "<filename>",
    "sha256": "<64-char SHA-256>"
  },
  "run_id": "<run id>",
  "run_attempt": "<attempt>",
  "workflow_run_url": "<URL>",
  "producer": "Qualification/normalize-candidate-artifacts",
  "started_at": "<UTC timestamp>",
  "finished_at": "<later UTC timestamp>",
  "identity_digest": "<64-char SHA-256>"
}
```

The identity digest must be the SHA-256 of canonical JSON excluding `identity_digest`.

The identity generator must have an explicit CLI. Recommended interface:

```text
python scripts/candidate_identity.py generate \
  --artifact-manifest candidate-bundle/artifact-manifest.json \
  --candidate-sha "$CANDIDATE_SHA" \
  --run-id "$GITHUB_RUN_ID" \
  --run-attempt "$GITHUB_RUN_ATTEMPT" \
  --workflow-run-url "$RUN_URL" \
  --output candidate-bundle/candidate-identity.json

python scripts/candidate_identity.py validate \
  candidate-bundle/candidate-identity.json \
  --artifact-manifest candidate-bundle/artifact-manifest.json \
  --expected-sha "$CANDIDATE_SHA"
```

If a different CLI is chosen, update the plan examples, script tests, and workflow together in one commit.

### 6.4 Common result contract

Every release-blocking suite must emit a direct result contract:

```json
{
  "schema_version": "3",
  "suite": "<stable suite id>",
  "candidate_sha": "<40-char SHA>",
  "identity_digest": "<64-char SHA-256>",
  "overall_pass": true,
  "status": "passed",
  "started_at": "<UTC timestamp>",
  "finished_at": "<later UTC timestamp>",
  "metrics": {},
  "diagnostics": [],
  "inputs": {},
  "outputs": {}
}
```

Allowed `status` values for required release suites are only `passed` and `failed`.

`skipped`, `unavailable`, `unknown`, `partial`, `warning`, or absent results must fail release validation.

Create or extend one shared helper, for example `scripts/result_contract.py`, to:

- load candidate identity;
- normalize pytest-json-report output;
- validate counts;
- enforce suite thresholds;
- embed candidate SHA and identity digest;
- emit structured diagnostic codes;
- validate a finished contract.

Do not keep separate ad hoc inline JSON emitters for each job.

## 7. Phase 0 — Freeze and executable contract inventory

### Tasks

1. Record PR #12 head at the start of implementation.
2. Re-run ordinary CI before changes if the head has moved.
3. Capture `--help` output for:
   - `generate_artifact_manifest.py`;
   - `candidate_identity.py`;
   - `compare_httpx_api_manifest.py`;
   - `generate_downstream_matrix.py`;
   - `normalize_pytest_result.py`;
   - `generate_compatibility_evidence.py`;
   - `validate_compatibility_evidence.py`;
   - `validate_qualification_workflow.py`.
4. Add parser tests that instantiate each `argparse` parser or execute each script with `--help` and known valid/invalid argument sets.
5. Decide and document one authoritative CLI for every script before editing workflow YAML.

### Files

- `scripts/*.py`
- `crates/eggfetch-python/tests/compat/test_qualification_tooling.py`
- new focused tests under `tests/scripts/` if that is the repository convention

### Acceptance criteria

- Every workflow-invoked argument is declared by the called script.
- Every required positional argument appears in the workflow call.
- Unknown arguments produce exit code 2 in negative tests.
- Required missing arguments produce exit code 2.
- The workflow validator has enough information to compare workflow calls against parser contracts.

## 8. Phase 1 — Repair artifact normalization and identity generation

### Tasks

1. Make `generate_artifact_manifest.py` accept one clear generation interface.
2. Select exact wheel files deterministically.
3. Copy wheels into `candidate-bundle/wheels/`.
4. Write paths relative to the bundle root.
5. Validate manifest schema immediately after generation.
6. Add `generate` and `validate` modes to `candidate_identity.py`.
7. Ensure `started_at < finished_at`; do not use the same timestamp for both.
8. Validate manifest digest, wheel hashes, candidate SHA, version, and run metadata.
9. Upload the complete bundle as one artifact.
10. Remove downstream dependence on separately named wheel artifacts after the normalization job.

### Workflow dependency correction

`normalize-candidate-artifacts` must declare:

```yaml
needs:
  - verify-candidate
  - build-candidate-artifacts
```

Any other job using `needs.verify-candidate.outputs.*` must also declare `verify-candidate` directly.

### Acceptance criteria

- The normalization job succeeds from a clean checkout and downloaded build artifacts.
- The bundle contains exactly two wheel files plus both JSON records.
- Recomputed hashes match manifest and identity.
- Changing one wheel byte causes validation failure.
- Changing one manifest field causes identity validation failure.
- Changing the candidate SHA causes validation failure.
- Missing run ID, attempt, or run URL causes release-mode validation failure.
- No later job needs shell globs over nested artifact-download directories.

## 9. Phase 2 — Repair API-oracle execution contracts

### Tasks

1. Decide whether `compare_httpx_api_manifest.py` will:
   - emit a result contract directly; or
   - emit raw comparison JSON that a shared helper wraps in a result contract.
2. Add only the CLI options actually required by the workflow.
3. Support an explicit output path instead of relying on stdout redirection where possible.
4. Validate active and resolved difference files separately.
5. Include candidate SHA and identity digest in the final facade and shim oracle result contracts.
6. Preserve exact tuple matching by:
   - symbol;
   - difference type;
   - member;
   - reference value;
   - candidate value.
7. Reject wildcard, symbol-only, duplicate, expired, stale, and resolved-in-active entries.
8. Run both oracles against installed wheels from `candidate-bundle`, not source-tree imports.

### Recommended workflow calls

```text
python scripts/compare_httpx_api_manifest.py compare \
  --reference compat/httpx/0.28.1/reference-api.json \
  --candidate /tmp/facade-api-manifest.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml \
  --resolved compat/httpx/0.28.1/resolved-differences.toml \
  --candidate-identity candidate-bundle/candidate-identity.json \
  --suite facade-api-oracle \
  --output /tmp/facade-api-result.json
```

Use the actual implemented interface consistently.

### Acceptance criteria

- Facade oracle job executes without parser errors.
- Shim oracle job executes without parser errors.
- Both results contain exact candidate SHA and identity digest.
- Any unexplained difference fails.
- Any stale active allowance fails.
- Any mismatch in one tuple field fails.
- A symbol-level allowance cannot suppress a different difference on the same symbol.

## 10. Phase 3 — Make the downstream manifest the sole matrix source

### Tasks

1. Keep all required package metadata only in `compat/downstream/manifest.toml`.
2. Have `generate_downstream_matrix.py` emit compact JSON to both:
   - a retained artifact for inspection;
   - a GitHub Actions job output.
3. Consume the output through `fromJSON(needs.prepare-downstream-matrix.outputs.matrix)`.
4. Delete the duplicated static matrix from `.github/workflows/qualification.yml`.
5. Validate that every required manifest entry has:
   - package name;
   - exact version;
   - immutable locator;
   - SHA-256;
   - install mode;
   - committed test file or command;
   - minimum collected and passed counts;
   - zero skip and xfail limits;
   - one or more valid category IDs;
   - timeout.
6. Validate exactly the eight declared Stage C categories are covered by required entries.
7. Reject informational entries from the release matrix unless explicitly placed in a separate non-blocking job.

### Required categories

- `contract-tests`
- `mock-transport-request-matching`
- `framework-test-client`
- `asgi-test-client`
- `sdk-async-client`
- `streaming-sse-consumption`
- `custom-auth-flow`
- `event-hooks-instrumentation`

### Acceptance criteria

- Editing a required manifest package changes the runtime matrix without editing YAML.
- Removing one category from the manifest causes matrix generation or workflow validation failure.
- Adding a static required-package matrix to the workflow causes workflow validation failure.
- Every release matrix entry maps back to one manifest record and manifest digest.

## 11. Phase 4 — Prove controlled replacement installation downstream

### Environment construction sequence

Every downstream matrix job must perform this sequence:

1. Download `candidate-bundle`.
2. Validate artifact manifest and candidate identity.
3. Create a fresh virtual environment.
4. Install the eggfetch wheel from `candidate-bundle/wheels/`.
5. Install the controlled replacement `httpx` wheel from `candidate-bundle/wheels/`.
6. Run `compat/httpx-controlled-replacement/verify_identity.py`.
7. Download the exact downstream distribution artifact into a local cache without installing it.
8. Verify its SHA-256 against the manifest.
9. Install that exact downloaded artifact, not a package-name resolver expression.
10. Install any separately declared pinned auxiliary dependencies.
11. Run `pip check`.
12. Re-run controlled replacement identity verification.
13. Record installed package versions and `httpx.__file__`.
14. Run the package-specific behavioral fixture.
15. Re-run controlled replacement identity verification after tests.

### Exact downstream artifact acquisition

Use a helper such as:

```text
python scripts/acquire_downstream_artifact.py \
  --package "$PACKAGE" \
  --version "$VERSION" \
  --expected-sha256 "$SOURCE_HASH" \
  --output-dir /tmp/downstream-artifact
```

The helper must:

- use an exact version;
- reject source ambiguity;
- select one supported wheel deterministically;
- hash the downloaded bytes;
- fail before installation on mismatch;
- emit artifact filename and actual digest.

Install using the downloaded path:

```text
pip install /tmp/downstream-artifact/<exact-file>.whl
```

Do not verify one artifact and install another by package name.

### HTTPX identity assertions

The result must record:

```json
{
  "httpx_distribution_version": "0.28.1",
  "httpx_module_path": "<installed path>",
  "replacement_identity_before_install": true,
  "replacement_identity_after_install": true,
  "replacement_identity_after_tests": true
}
```

The identity verifier must distinguish the controlled replacement from upstream HTTPX even when both report version `0.28.1`.

### Acceptance criteria

- Removing the controlled replacement wheel causes every required downstream job to fail.
- Replacing it with upstream HTTPX causes identity verification to fail.
- A downstream dependency install that overwrites the controlled replacement causes failure before tests.
- A source-hash mismatch causes failure before installation.
- An installed downstream version mismatch causes failure.
- `pip check` failure blocks qualification.
- Every package-specific fixture imports `httpx` from the controlled replacement environment.

## 12. Phase 5 — Standardize downstream result emission and aggregation

### Tasks

1. Replace inline downstream JSON construction with a committed helper.
2. Parse pytest-json-report strictly.
3. Treat missing, malformed, or zero-test reports as failure.
4. Enforce all thresholds:
   - `collected >= min_collected`;
   - `passed >= min_passed`;
   - `failures == 0`;
   - `errors == 0`;
   - `skipped <= max_skipped`;
   - `xfailed <= max_xfailed`.
5. Require `job_status == success`.
6. Require exact source artifact hash verification.
7. Require exact installed version.
8. Require all three controlled replacement identity checks.
9. Embed manifest record digest in each package result.
10. Aggregate only validated package result contracts.
11. Count a category as covered only when at least one required package result for that category passes.
12. Require every required package result to be present exactly once.
13. Require all eight categories.
14. Fail on duplicate package results or unknown package results.

### Acceptance criteria

- A missing package result fails aggregation.
- A duplicate package result fails aggregation.
- A skipped test fails when `max_skipped = 0`.
- An xfail fails when `max_xfailed = 0`.
- One passed test with `min_passed = 2` fails.
- A passing report with failed replacement identity fails.
- Category metadata from a failed package does not count as coverage.
- Aggregate result contains the candidate SHA and identity digest.

## 13. Phase 6 — Complete native proxy and TLS proof

### Proxy requirements

Add deterministic loopback fixtures for:

1. Plain HTTP proxy forwarding.
2. HTTPS request through an HTTP proxy using CONNECT.
3. Proxy TCP connection refusal.
4. Proxy accepts TCP but stalls before CONNECT response.
5. Proxy returns non-2xx CONNECT response.
6. Tunnel established, then origin closes.

The CONNECT success test must make an actual request through `Client(proxy=...)` to a TLS origin and prove the proxy observed a CONNECT request.

The CONNECT-stall test must assert the exact exception class selected by reference HTTPX behavior. Capture reference behavior in a focused oracle test if uncertain. Do not accept base `Exception`.

### TLS requirements

Add deterministic tests for:

1. Successful verification against a generated local CA or pinned certificate.
2. Verification failure for an untrusted certificate.
3. Hostname mismatch.
4. TLS server accepts TCP but never completes handshake.
5. HTTPS through CONNECT proxy.
6. Exception retains request context.

Use exact exception classes and bounded elapsed-time assertions. Avoid internet access and nondeterministic unroutable-address assumptions.

### Acceptance criteria

- The proxy records a CONNECT request in the success test.
- No proxy success test merely constructs an unused client.
- No required proxy/TLS test uses `pytest.raises(Exception)`.
- Exact exception types match the reference HTTPX contract.
- Every timeout/error exception retains the originating request where HTTPX does.
- All fixtures shut down within bounded time and leave no server threads alive.

## 14. Phase 7 — Complete native shutdown, concurrency, resource, and soak proof

### Shutdown subprocess scenarios

Create subprocess tests for:

1. Unused sync client without explicit close.
2. Used sync client without explicit close.
3. Unread native response at interpreter exit.
4. Partially consumed native response at interpreter exit.
5. Active stalled native request during client close.
6. Client close racing with request completion.
7. Async client with cancelled request.
8. Async client without explicit `aclose()`.
9. Proxy-backed native request at shutdown.
10. TLS-backed native request at shutdown.

Each subprocess must:

- have a hard timeout;
- exit with code 0 only for the expected outcome;
- reject panic, unhandled task, event-loop-closed, unclosed resource, and thread-pool warning patterns;
- emit a structured result.

### Concurrency requirements

- Restore strict 100% scheduled-operation success.
- Do not swallow thread or task exceptions.
- Report every operation failure with index and exception.
- Use a barrier to begin concurrent operations deterministically.
- Keep thresholds practical for CI without reducing required success count.

### Resource requirements

- Read `compat/httpx/0.28.1/resource-thresholds.toml` directly.
- Emit measured before/after values and deltas for file descriptors, threads, and RSS where supported.
- Apply platform-specific thresholds.
- Fail if the platform has no declared policy in release mode.
- Retain the raw resource report.

### Soak requirements

Separate two modes:

1. PR qualification smoke soak: bounded request count, zero errors, short enough for required CI.
2. Scheduled/release long soak: policy-defined minimum duration and request count, zero errors, retained metrics.

The exact release policy must be executable, not documentary. If the current policy requires 300 seconds and 500 requests, the release workflow must prove both values or update the policy with an explicit reviewed rationale.

### Acceptance criteria

- All shutdown subprocess scenarios are native, not MockTransport-only.
- Any subprocess timeout fails.
- Any forbidden warning fails.
- Concurrency requires all operations to succeed.
- Resource thresholds are loaded and recorded.
- Soak result records duration, request count, success count, error count, and policy values.
- Release soak cannot pass with one error.

## 15. Phase 8 — Rebuild workflow validation around actual contracts

### Tasks

Enhance `validate_qualification_workflow.py` to validate the actual workflow, not only string presence.

It must check:

1. Required positional workflow path is supplied.
2. Every script invocation uses supported CLI arguments.
3. Every job directly declares jobs whose outputs it references.
4. Every required checkout uses the exact candidate SHA.
5. Artifact producers and consumers match by exact artifact name.
6. All post-normalization jobs consume `candidate-bundle`.
7. No required job consumes original wheel artifacts directly.
8. The runtime downstream matrix comes from the manifest-generated output.
9. No duplicated static required-package matrix exists.
10. No obsolete `--wheel-dir` interface exists.
11. No `continue-on-error` on required steps or jobs.
12. No `|| true`, `|| echo`, or equivalent failure suppression.
13. Every required pytest invocation installs the required plugins.
14. Every required suite emits and uploads one result contract.
15. Evidence generation consumes direct result artifacts, not one aggregate file repeated under several flags.
16. Evidence generation runs with `--release` for release qualification.
17. Independent evidence validation is unsuppressed.
18. Final gate requires evidence generation and independent validation.
19. Required jobs are not accepted as skipped.
20. Dry-run affects only publication/attestation, not qualification pass/fail.
21. Status update is downstream of successful final gate or performed manually after retained proof.

### Parser-contract validation

Prefer importing parser builders from scripts. Each script should expose a function such as `build_parser()` so workflow validation tests can inspect supported options without duplicating parser definitions.

Where shell parsing is difficult, constrain workflow commands to one argument per line and add focused command extraction tests.

### Acceptance criteria

- The validator rejects the audited PR #12 workflow before correction.
- Each confirmed defect in Section 3 has a dedicated negative fixture.
- The corrected workflow passes the validator.
- Removing one direct `needs` dependency fails.
- Adding one unsupported script argument fails.
- Adding one suppression operator fails.
- Reintroducing a static downstream matrix fails.

## 16. Phase 9 — Rewire evidence generation to direct artifacts

### Required direct evidence inputs

Evidence generation must consume separate validated result files for:

- compatibility suite aggregate;
- facade API oracle;
- shim API oracle;
- downstream aggregate;
- shim substitution;
- native timeout classification;
- proxy/TLS;
- shutdown;
- resource;
- soak;
- workflow validation;
- candidate identity;
- artifact manifest.

Do not pass `qualification-aggregate.json` repeatedly.

A high-level aggregate may be retained for convenience, but it is not a substitute for direct section contracts.

### Compatibility matrix aggregation

The Python-version compatibility matrix produces several results. Add a dedicated aggregate helper that:

- requires every configured Python version;
- validates each contract;
- requires all to pass;
- sums counts correctly;
- preserves per-version records;
- emits one direct `compat-suite-aggregate` result contract.

### Evidence generation behavior

In release mode, the generator must:

1. Validate candidate identity and manifest.
2. Recompute artifact hashes from bundle paths.
3. Validate every direct result contract.
4. Require matching candidate SHA and identity digest for every result.
5. Require every section once.
6. Require all section `overall_pass` values true.
7. Recompute downstream category coverage.
8. Recompute compatibility totals.
9. Recompute final `overall_pass`.
10. Refuse to write evidence when any check fails.

### Acceptance criteria

- Evidence generation has no suppression operator.
- Missing any direct result file fails.
- Passing the wrong section file under a flag fails by suite ID.
- A nested aggregate passed as a direct result fails schema validation.
- Any identity mismatch fails.
- Any artifact byte mismatch fails.
- Evidence includes the exact candidate identity and manifest.

## 17. Phase 10 — Make independent validation and final gate fail closed

### Independent validation

The independent validator must:

- run in a clean job;
- download only retained evidence and candidate bundle, plus source checkout for the validator;
- recompute candidate identity digest;
- recompute artifact hashes;
- validate every result section;
- recompute `overall_pass` without trusting the generator’s boolean;
- exit nonzero on any discrepancy.

Do not append `|| echo` or use `continue-on-error`.

### Final gate

The final gate must combine:

1. Authoritative GitHub job results from `needs.<job>.result`.
2. The independently validated evidence result contract.
3. Exact candidate SHA and identity digest.
4. Confirmation that required jobs were neither skipped nor cancelled.

Required jobs must include:

- verify candidate;
- build candidate artifacts;
- normalize candidate artifacts;
- validate workflow;
- all compatibility matrix jobs and aggregate;
- facade API oracle;
- shim API oracle;
- downstream matrix preparation;
- all downstream jobs and aggregate;
- shim substitution;
- native timeout;
- proxy/TLS;
- shutdown;
- concurrency if separate;
- resource;
- soak;
- evidence generation;
- independent evidence validation.

The summary job must fail if any required result is not `success`.

`dry_run` may skip publishing packages, attestations, or release tags. It may not allow failed qualification to exit 0.

### Acceptance criteria

- Default dry-run fails when qualification fails.
- A skipped required job fails.
- A cancelled required job fails.
- Missing evidence fails.
- Failed independent validation fails.
- Evidence with `overall_pass=false` fails.
- A successful final gate emits one retained `qualification-summary.json` with exact SHA, identity digest, run URL, and all required job conclusions.

## 18. Phase 11 — Required negative test matrix

Implement at least the following negative cases. Each must assert a stable diagnostic code or exact validation message category.

### Script CLI negatives

1. Unknown artifact-manifest argument.
2. Missing artifact-manifest output directory.
3. Candidate identity generation without manifest.
4. Candidate identity validation without expected SHA in release mode.
5. API comparator unknown option.
6. Workflow validator missing positional workflow path.
7. Evidence generator missing one mandatory release section.

### Artifact and identity negatives

8. Missing eggfetch wheel.
9. Missing controlled replacement wheel.
10. Multiple controlled replacement wheels.
11. Wheel hash mismatch.
12. Manifest path traversal.
13. Manifest candidate SHA mismatch.
14. Identity manifest digest mismatch.
15. Identity wheel digest mismatch.
16. Identity timestamps not ordered.
17. Identity digest tampering.

### Downstream negatives

18. Downstream source hash mismatch.
19. Downstream installed version mismatch.
20. Upstream HTTPX installed instead of controlled replacement.
21. Controlled replacement overwritten during dependency install.
22. `pip check` failure.
23. Missing pytest report.
24. Malformed pytest report.
25. Zero collected tests.
26. Fewer than minimum passed tests.
27. One skipped test with zero skip allowance.
28. One xfailed test with zero xfail allowance.
29. Duplicate package result.
30. Missing required package result.
31. Unknown package result.
32. Failed package attempting to cover a category.
33. Missing Stage C category.
34. Static workflow matrix diverging from manifest.

### Native negatives

35. CONNECT stall classified as broad or wrong exception.
36. TLS handshake stall classified as broad or wrong exception.
37. Certificate verification unexpectedly succeeds.
38. Missing request context on timeout/error.
39. Shutdown subprocess exceeds deadline.
40. Shutdown subprocess emits forbidden warning.
41. Concurrency operation exception is swallowed.
42. Resource policy missing for release platform.
43. Resource threshold exceeded.
44. Soak has one request error.
45. Soak duration below policy.
46. Soak request count below policy.

### Evidence and final-gate negatives

47. Direct result candidate SHA mismatch.
48. Direct result identity digest mismatch.
49. Wrong suite result supplied under evidence flag.
50. Nested aggregate supplied as direct suite result.
51. Missing direct result artifact.
52. Result `overall_pass=false`.
53. Evidence overall pass tampered to true.
54. Artifact changed after evidence generation.
55. Independent validator suppressed by shell operator.
56. Evidence generator suppressed by shell operator.
57. Required job skipped.
58. Required job cancelled.
59. Dry-run with failed qualification exits 0.
60. Release-relevant commit after qualified SHA without requalification.

## 19. Phase 12 — Local and CI validation sequence

### Local tooling validation

Run from the implementation branch:

```text
python scripts/generate_artifact_manifest.py --help
python scripts/candidate_identity.py --help
python scripts/compare_httpx_api_manifest.py --help
python scripts/generate_downstream_matrix.py --help
python scripts/normalize_pytest_result.py --help
python scripts/generate_compatibility_evidence.py --help
python scripts/validate_compatibility_evidence.py --help
python scripts/validate_qualification_workflow.py --help
```

Run focused tests:

```text
python -m pytest crates/eggfetch-python/tests/compat/test_qualification_tooling.py -v
python -m pytest crates/eggfetch-python/tests/compat/test_downstream_portfolio.py -v
python -m pytest crates/eggfetch-python/tests/compat/test_native_proxy_tls.py -v
python -m pytest crates/eggfetch-python/tests/compat/test_native_shutdown.py -v
python -m pytest crates/eggfetch-python/tests/compat/test_soak.py -v
```

Run workflow validation explicitly:

```text
python scripts/validate_qualification_workflow.py \
  .github/workflows/qualification.yml
```

Run the repository’s existing formatting, linting, Rust, Python, and wheel-smoke commands used by ordinary CI.

### Required CI sequence

1. Push corrective commits to PR #12.
2. Wait for ordinary CI, Security, and Benchmarks on the new head.
3. Confirm exact `Required CI Gate` success for the PR head SHA.
4. Dispatch `Qualification` with:
   - `candidate_sha` equal to the exact PR head;
   - `dry_run=true` for the first execution.
5. Dry-run must still fail on any qualification defect.
6. Correct defects and repeat until the entire qualification workflow succeeds.
7. After no release-relevant commit is added, dispatch a non-dry release qualification for the same exact SHA if publication/attestation behavior differs.
8. Download and inspect:
   - candidate bundle;
   - direct result artifacts;
   - compatibility evidence;
   - independent validation result;
   - qualification summary.
9. Confirm all artifacts name the same SHA and identity digest.
10. Only then merge the implementation PR.
11. If merge creates a new release-relevant merge SHA, qualify the merge SHA before release or use a merge method that preserves the qualified commit as the release candidate according to repository policy.

## 20. Status and documentation reconciliation

Do not update status to Stage C released during implementation.

After exact-SHA qualification succeeds, update or replace the status file with mechanically grounded fields:

```text
Status: Stage C released
Candidate SHA: <exact qualified SHA>
Required CI run: <URL>
Qualification run: <URL>
Qualification attempt: <number>
Candidate identity digest: <SHA-256>
Artifact manifest digest: <SHA-256>
Evidence artifact name: compatibility-evidence
Evidence overall_pass: true
Independent validation: passed
Qualification summary: passed
Post-qualification release-relevant commits: none
```

Do not retain an older candidate SHA as “current.”

Do not mark tracks completed solely because files exist. Completion requires executable acceptance criteria and retained evidence.

## 21. Suggested commit decomposition

1. `fix: align qualification script CLI contracts`
2. `fix: normalize candidate bundle and identity generation`
3. `fix: make downstream matrix manifest-authoritative`
4. `fix: install and verify exact downstream artifacts against replacement httpx`
5. `fix: standardize qualification result contracts`
6. `test: complete native proxy tls shutdown and policy proof`
7. `fix: make workflow validator enforce execution contracts`
8. `fix: rewire evidence generation to direct retained results`
9. `fix: make independent validation and final gate fail closed`
10. `test: add qualification negative fixture matrix`
11. `docs: reconcile exact-sha qualification status`

Keep commits reviewable. Do not combine workflow rewiring, native semantic changes, and status claims in one opaque commit.

## 22. Stop conditions

Stop and leave the repository at Stage C candidate if any of these remain true:

- any workflow-invoked script rejects its arguments;
- any job references outputs from an undeclared dependency;
- downstream tests execute upstream HTTPX;
- verified downstream bytes differ from installed bytes;
- any required package result is missing, skipped, malformed, or identity-mismatched;
- any Stage C category lacks passing release-blocking proof;
- required proxy or TLS tests accept base `Exception`;
- shutdown proof is only normal context-manager closure;
- resource thresholds are not executable;
- release soak does not satisfy policy;
- evidence generation or validation failure is suppressed;
- the final gate accepts skipped required jobs;
- dry-run weakens qualification correctness;
- exact-SHA qualification artifacts are absent;
- status names a SHA other than the qualified candidate;
- a release-relevant commit exists after qualification without a new qualification run.

## 23. Final acceptance criteria

The corrective pass is complete only when all criteria below are true.

1. PR #12 or its replacement contains no unsupported workflow script arguments.
2. Every workflow script call passes parser-contract tests.
3. Every job declares direct dependencies for all consumed outputs.
4. Candidate bundle generation succeeds from clean downloaded build artifacts.
5. Candidate bundle contains exactly the expected wheels, manifest, and identity.
6. Candidate identity digest validates independently.
7. Every release-blocking job consumes the same candidate bundle.
8. Facade API oracle passes against the installed eggfetch wheel.
9. Shim API oracle passes against the installed controlled replacement wheel.
10. Exact typed API allowances remain one-to-one and fail closed.
11. Downstream runtime matrix is generated from the manifest.
12. No duplicated static required-package matrix remains.
13. Every required downstream source artifact is exact-version and hash-verified.
14. The exact verified artifact is installed.
15. Controlled replacement identity passes before downstream install.
16. Controlled replacement identity passes after downstream install.
17. Controlled replacement identity passes after downstream tests.
18. Installed downstream versions match the manifest.
19. `pip check` passes in every downstream environment.
20. Every required downstream behavioral fixture collects and passes its minimum tests.
21. Required downstream suites have zero failures, errors, skips, and xfails.
22. All eight Stage C categories are covered by passing required results.
23. Native HTTP proxy forwarding passes.
24. Native HTTPS CONNECT proxying passes and records CONNECT.
25. CONNECT stall produces the exact expected timeout class.
26. TLS verification success, rejection, hostname mismatch, and handshake stall pass with exact classes.
27. Native timeout and error exceptions retain request context where required.
28. Shutdown subprocess scenarios exit cleanly within deadlines.
29. No forbidden shutdown warnings or panics occur.
30. Concurrency requires 100% operation success.
31. Resource metrics satisfy loaded platform policy.
32. PR soak satisfies its strict zero-error policy.
33. Release or scheduled long soak satisfies duration and request-count policy.
34. Workflow validator rejects all mandatory negative fixtures.
35. Corrected workflow passes workflow validation.
36. Compatibility matrix results aggregate across every configured Python version.
37. Evidence generator consumes direct result artifacts.
38. Evidence generator has no failure suppression.
39. Every evidence section has matching candidate SHA and identity digest.
40. Artifact hashes are recomputed from the candidate bundle.
41. Evidence `overall_pass` is recomputed from all mandatory sections.
42. Independent evidence validation has no failure suppression.
43. Independent validation recomputes and agrees with `overall_pass=true`.
44. Final gate includes evidence generation and independent validation.
45. Final gate rejects failed, cancelled, missing, and skipped required jobs.
46. Dry-run does not weaken pass/fail behavior.
47. Ordinary `Required CI Gate` is green for the exact candidate SHA.
48. Qualification succeeds for the exact candidate SHA.
49. Candidate bundle, evidence, validation, and summary artifacts are retained.
50. Qualification summary names the exact candidate SHA and identity digest.
51. Status documentation names the exact qualified SHA and run URLs.
52. No release-relevant commit follows the qualified SHA without requalification.
53. The implementation PR is merged only after the above evidence exists.
54. Stage C released is claimed only after all criteria pass.

## 24. Handoff decision rule

An implementing agent may declare this corrective pass complete only after producing a retained exact-SHA qualification run and independently validated evidence.

A green ordinary CI run alone is insufficient.

The presence of scripts, fixtures, workflow jobs, or status prose is insufficient.

If the workflow remains unable to execute from candidate verification through final summary without suppressed failures, the correct decision is:

> Stage C candidate — qualification execution corrective pass incomplete.
