# Release-Candidate Validation Follow-Up Plan

## Purpose

This plan closes the remaining validation gaps discovered after the initial release-candidate closure implementation. The repository now contains release workflows, benchmark automation, differential tests, cross-platform CI, package validation, and fuzz-corpus cleanup, but the implementation history exposed several unresolved concerns:

- repeated CI repairs against Rust 1.97 and platform-specific environments;
- broad lint suppression in some test and benchmark targets;
- incomplete proof that the release workflow produces installable artifacts;
- incomplete evidence for crates.io and PyPI publication dry runs;
- documentation execution paths that may succeed when dependencies are absent;
- loss of allocation and memory-regression measurement after preserving the workspace-wide `unsafe_code = forbid` policy;
- no single, explicit commit-level release gate proving all required jobs are green.

The objective is not to add features. The objective is to produce auditable evidence that one exact commit is suitable for tagging as `0.1.0-rc1`.

## Scope

This pass covers:

1. lint-policy cleanup and CI normalization;
2. release-workflow dry-run execution;
3. Rust package validation;
4. Python wheel validation;
5. CLI and native artifact validation;
6. checksums, SBOM, and provenance validation;
7. differential and compatibility validation;
8. memory and resource-regression coverage;
9. documentation execution validation;
10. final release-candidate evidence and sign-off.

Out of scope:

- new HTTP features;
- new bindings;
- performance tuning unrelated to a demonstrated regression;
- HTTP/3 graduation from experimental status;
- public registry publication before all acceptance criteria pass.

## Operating rules

- No new public API unless required to fix a release-blocking defect.
- No broad warning suppression in production crates.
- Test-only lint exemptions must be explicit, narrow, and documented.
- Release validation must run against one immutable commit SHA.
- A passing workflow with skipped critical checks does not count as a successful release validation.
- Artifact installation tests must use the produced artifacts, not source-tree builds.
- Publishing remains disabled until the final release gate is satisfied.

# Track A: Lint and CI policy cleanup

## Goals

Ensure that the repository passes the declared Rust toolchain without masking actionable warnings.

## Tasks

1. Audit all crate roots, test targets, examples, benchmarks, and build scripts for:
   - `#![allow(warnings)]`;
   - `#![allow(clippy::all)]`;
   - `#![allow(clippy::pedantic)]`;
   - overly broad module-level lint suppression;
   - duplicate lint exemptions that can be removed.
2. Replace broad suppressions with the smallest applicable lint exemption.
3. Add comments for non-obvious exemptions explaining why the pattern is intentional.
4. Keep test-only exemptions local to the specific test module or function where practical.
5. Standardize CI commands so local and CI lint behavior match.
6. Pin or explicitly declare the supported Rust version and MSRV policy in one authoritative location.
7. Add an automated grep/check script that fails CI when forbidden broad suppressions are introduced.
8. Confirm formatting, clippy, docs, tests, and examples all use the same toolchain expectations.

## Acceptance criteria

- No production source file contains `allow(warnings)` or `allow(clippy::all)`.
- No benchmark or test crate uses `allow(warnings)`.
- Any remaining `allow(clippy::...)` is targeted to named lints and includes a justification when not self-evident.
- A CI check rejects future additions of forbidden broad suppressions.
- `cargo fmt --all -- --check` passes.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes, excluding only targets that require separately configured language runtimes and are covered by dedicated jobs.
- The exact lint command is documented in `CONTRIBUTING.md`.

# Track B: Release workflow dry-run mode

## Goals

Prove that the release workflow can build and validate all intended artifacts without publishing them.

## Tasks

1. Add or verify a `workflow_dispatch` dry-run mode in `.github/workflows/release.yml`.
2. Ensure dry-run mode:
   - builds all release artifacts;
   - runs all validation jobs;
   - uploads artifacts for inspection;
   - does not publish to crates.io, PyPI, npm, or GitHub Releases;
   - does not require production publication secrets.
3. Make all publish jobs depend on a single validated release manifest.
4. Add explicit input validation for version, tag, prerelease status, and publish toggle.
5. Ensure release jobs operate on the triggering commit SHA and do not silently rebuild from a moving branch.
6. Add a summary job that reports every artifact, platform, checksum, and validation result.
7. Fail the workflow if an expected artifact is missing.
8. Fail the workflow if any critical validation job is skipped unexpectedly.

## Acceptance criteria

- A manually triggered dry run completes successfully from one immutable commit SHA.
- The workflow uploads all expected artifacts without publishing any package.
- The workflow summary lists each expected artifact and its validation status.
- No production registry credentials are required for dry-run execution.
- The workflow fails when an expected artifact is deliberately removed from the manifest.
- Every publish job is gated behind the successful validation summary.

# Track C: Rust package validation

## Goals

Verify that all publishable Rust crates package correctly from registry contents rather than relying on workspace-only files.

## Tasks

1. Identify every publishable crate and every intentionally non-publishable crate.
2. For each publishable crate:
   - run `cargo package --list`;
   - inspect included files;
   - run `cargo package`;
   - unpack the `.crate` archive;
   - run tests and docs from the unpacked package where feasible;
   - run `cargo publish --dry-run` in dependency order.
3. Verify crate metadata:
   - version;
   - license expression;
   - repository URL;
   - readme path;
   - categories and keywords;
   - rust-version;
   - feature documentation.
4. Ensure path dependencies resolve correctly for publication.
5. Ensure generated artifacts, fuzz corpora, local plans, and development-only files are excluded.
6. Add CI automation for package-content validation.

## Acceptance criteria

- `cargo package --list` contains only intended release files for every publishable crate.
- `cargo package` succeeds for every publishable crate.
- `cargo publish --dry-run` succeeds in documented publication order.
- Tests or smoke builds succeed from unpacked `.crate` archives.
- No crate depends on unpublished path-only metadata.
- No large fuzz corpus, local cache, virtual environment, benchmark output, or secret-bearing file appears in a package.
- Crate metadata matches the documented compatibility and licensing policy.

# Track D: Python wheel validation

## Goals

Prove that produced wheels install and function across the declared Python and platform matrix.

## Tasks

1. Define the exact supported Python versions, operating systems, and architectures for `0.1.0-rc1`.
2. Build wheels using the release workflow, not ad hoc local commands.
3. Run `twine check` on all wheel and source-distribution artifacts.
4. For every wheel:
   - create a clean environment;
   - install the wheel with `pip --no-index --find-links`;
   - import `eggfetch`;
   - verify version metadata;
   - run a sync request against a local server;
   - run an async request against a local server;
   - exercise streaming response iteration;
   - exercise one TLS configuration path;
   - exercise one retry path;
   - exercise multipart upload;
   - verify named exception mapping.
5. Build and install the source distribution in a clean environment.
6. Confirm abi3 compatibility claims against actual wheel tags.
7. Ensure no test uses the source tree through `PYTHONPATH` accidentally.
8. Add TestPyPI publication as a separate optional pre-release rehearsal if credentials are available.

## Acceptance criteria

- `twine check` passes for every Python artifact.
- Every declared wheel installs in a clean environment without building from source.
- Installed-wheel smoke tests pass on every declared platform/Python combination.
- The source distribution builds and installs successfully in a clean environment.
- `eggfetch.__version__`, package metadata, and release version agree.
- Wheel tags match the documented ABI and platform support.
- Tests fail if the installed wheel is removed, proving they are not importing from the source tree.
- TestPyPI rehearsal succeeds, or is explicitly documented as deferred because credentials are unavailable; deferral does not permit skipping local artifact installation tests.

# Track E: CLI and native artifact validation

## Goals

Ensure that distributed binaries and native libraries are usable without a development checkout.

## Tasks

1. Build CLI binaries for every declared release target.
2. Package binaries with license, README, shell completion files if supported, and checksums.
3. Install/extract each artifact in a clean directory.
4. Run:
   - `eggfetch --version`;
   - `eggfetch --help`;
   - a local HTTP GET;
   - a streamed download;
   - JSON output mode;
   - one error/exit-code case.
5. Validate C ABI artifacts:
   - shared/static library naming;
   - exported symbol presence;
   - generated C header;
   - simple C consumer compilation and execution;
   - panic containment across exported calls.
6. Validate Node artifacts only if they are included in the RC support claim. Otherwise mark the Node binding experimental and exclude it from the required release gate.
7. Verify binary runtime dependencies on Linux, macOS, and Windows.

## Acceptance criteria

- Every declared CLI binary runs from an extracted release archive without access to the source tree.
- CLI version output matches the release version.
- CLI local-server smoke tests pass on every declared target.
- Exit codes match the documented contract.
- The C header and library compile and run with a minimal external C program.
- Exported C symbols match the documented ABI surface.
- No panic crosses the FFI boundary in tested failure cases.
- Experimental Node artifacts are either explicitly excluded from RC support or independently validated and labeled.

# Track F: Checksums, SBOM, and provenance

## Goals

Create verifiable release metadata for every artifact.

## Tasks

1. Generate SHA-256 checksums for every release artifact.
2. Produce a machine-readable release manifest containing:
   - artifact filename;
   - platform/architecture;
   - package type;
   - version;
   - source commit SHA;
   - checksum;
   - build job identifier.
3. Generate an SBOM for Rust and packaged native dependencies using a documented tool.
4. Generate Python dependency metadata for build and runtime dependencies.
5. Add GitHub artifact attestations or another provenance mechanism where practical.
6. Verify checksums after artifact download in a separate job.
7. Ensure signing/provenance failures are release-blocking when the feature is declared supported.
8. Document what is signed, attested, checksummed, and not yet covered.

## Acceptance criteria

- Every uploaded artifact appears exactly once in the release manifest.
- Every artifact has a SHA-256 checksum.
- A separate verification job downloads artifacts and validates all checksums.
- The release manifest records the exact source commit SHA.
- SBOM generation succeeds and the SBOM is attached to the dry-run artifacts.
- Provenance/attestation is generated for all supported GitHub-built release artifacts, or the absence is explicitly documented as a release blocker rather than silently omitted.
- The release process documentation accurately describes the implemented guarantees.

# Track G: Differential and compatibility validation

## Goals

Confirm that documented compatibility claims match tested behavior.

## Tasks

1. Review `test_differential.py` for deterministic, local-server-based scenarios.
2. Cover high-value semantic comparisons with requests and HTTPX:
   - query encoding;
   - repeated headers;
   - redirects;
   - cookie persistence;
   - Basic/Bearer auth;
   - multipart encoding;
   - timeout categories;
   - streaming behavior;
   - decompression;
   - exception categories.
3. Separate intentional differences from regressions.
4. Record intentional differences in the compatibility matrix.
5. Ensure differential tests do not rely on public internet services.
6. Add a release report listing passed comparisons and documented divergences.

## Acceptance criteria

- Differential tests pass against pinned supported versions of requests and HTTPX.
- All tests use local deterministic servers.
- Every known divergence is documented in `docs/reference/compatibility.md`.
- No documentation claims drop-in compatibility.
- A generated or checked-in compatibility report identifies tested versions and scenarios.
- Any unexplained differential failure blocks the RC tag.

# Track H: Memory and resource regression coverage

## Goals

Restore useful memory-regression visibility without violating `unsafe_code = forbid` in production crates.

## Tasks

1. Implement an external process-level resource harness outside production crates.
2. Measure at minimum:
   - peak RSS during large buffered download;
   - peak RSS during streamed download;
   - peak RSS during streamed upload;
   - repeated connection reuse workload;
   - repeated failed/cancelled requests;
   - decompression-limit failure behavior;
   - proxy workload if supported in the harness.
3. Prefer OS-level process metrics or a standalone benchmark helper rather than a custom global allocator.
4. Record baseline results for a controlled environment.
5. Define initial regression thresholds generously enough to avoid noisy failures but strict enough to detect unbounded growth.
6. Run deterministic resource tests in scheduled or manual CI if hosted-runner noise makes PR gating unreliable.
7. Add leak/lifecycle assertions for pool permits, tasks, file handles, and streams where direct measurement is available.

## Acceptance criteria

- Resource tests require no unsafe code in production crates.
- Large streamed transfers remain bounded relative to configured chunk/window sizes.
- Repeated cancellation and failure workloads do not show monotonic unbounded RSS growth beyond the documented tolerance.
- Buffered and streamed workloads are measured separately.
- Baseline environment, command, and results are documented.
- A scheduled or manual CI workflow publishes resource reports.
- A clear policy states which resource regressions block an RC and which require manual review.

# Track I: Documentation execution validation

## Goals

Ensure documentation examples are genuinely validated rather than silently skipped.

## Tasks

1. Split documentation validation into explicit jobs:
   - syntax-only extraction/checking;
   - doctests;
   - installed-package Python example execution;
   - installed CLI example execution;
   - link validation.
2. Remove success-on-missing-package behavior from jobs intended to execute examples.
3. Permit explicit skip only in syntax-only jobs, with a clear log message and job name.
4. Install release artifacts before runtime example execution.
5. Mark examples requiring external credentials or internet access as non-executable and explain why.
6. Validate generated CLI help against checked-in/reference documentation.
7. Ensure version and feature references match the RC configuration.

## Acceptance criteria

- Runtime Python example jobs fail when the wheel is absent.
- Runtime CLI example jobs fail when the binary is absent.
- Syntax-only and execution jobs are separately named and reported.
- Rust doctests pass.
- Internal documentation links pass validation.
- Generated CLI help matches the documented CLI reference or differences are explicitly approved.
- No critical example is silently skipped because a dependency was not installed.

# Track J: Cross-platform and feature-matrix closure

## Goals

Prove the supported matrix rather than relying on individual ad hoc fixes.

## Tasks

1. Define the required matrix for:
   - Linux, macOS, Windows;
   - MSRV and stable Rust;
   - supported Python versions;
   - default features;
   - no-default-features;
   - HTTP/2;
   - HTTP/3 experimental build;
   - proxy;
   - compression combinations;
   - multipart;
   - FFI;
   - CLI.
2. Eliminate duplicate jobs where one matrix job can provide equivalent proof.
3. Keep HTTP/3 failures release-blocking only if the experimental feature is shipped in the RC artifact; otherwise require compile/test proof but not default-path installation proof.
4. Ensure platform-specific setup is explicit and does not depend on shell assumptions.
5. Add a matrix summary artifact.

## Acceptance criteria

- Every declared supported platform has a passing Rust core job.
- Every declared Python wheel target has a passing installed-wheel job.
- Default and no-default-feature builds pass.
- Required feature combinations compile and test successfully.
- HTTP/2 integration tests pass.
- HTTP/3 experimental tests pass in the declared environment or the feature is excluded from RC artifacts with documentation updated accordingly.
- FFI tests pass on all declared FFI targets.
- The matrix summary identifies every required job and result for the candidate commit.

# Track K: Final release-candidate gate

## Goals

Create one unambiguous decision point for tagging `0.1.0-rc1`.

## Tasks

1. Select a candidate commit SHA after all fixes are merged.
2. Freeze feature work during validation.
3. Run the complete dry-run release workflow against that SHA.
4. Collect:
   - CI matrix summary;
   - release manifest;
   - checksums;
   - SBOM/provenance;
   - Rust package dry-run results;
   - Python artifact installation results;
   - CLI/native smoke results;
   - differential test report;
   - benchmark/resource report;
   - security workflow results;
   - documentation validation results.
5. Add a release-candidate checklist document tied to the exact SHA.
6. Record all accepted limitations and experimental features.
7. Prohibit tagging if any required job is missing, skipped unexpectedly, or failing.
8. Tag only the validated SHA.

## Final acceptance criteria

All conditions below are mandatory unless explicitly marked optional:

### Code quality

- Formatting passes.
- Clippy passes with warnings denied.
- No forbidden broad lint suppressions remain.
- Rust tests pass on all supported platforms.
- Python tests pass on all supported Python versions.
- CLI, FFI, docs, security, and feature-matrix jobs pass.

### Packages and artifacts

- All publishable Rust crates pass `cargo publish --dry-run`.
- All Python artifacts pass `twine check`.
- Every wheel installs and passes smoke tests in a clean environment.
- Every CLI binary runs and passes smoke tests from the packaged archive.
- C ABI artifacts compile and run with an external C consumer.
- Artifact filenames and versions are internally consistent.

### Supply chain

- Every artifact has a verified checksum.
- The release manifest references the exact source commit.
- SBOM output is present and validated.
- Provenance/attestation is present for supported artifacts or treated as a blocking omission.
- Security scans pass with no unaccepted high- or critical-severity findings.

### Semantics and compatibility

- Differential tests pass.
- Intentional differences are documented.
- Streaming, retries, redirects, TLS, compression, multipart, proxy, cookies, and auth have passing release-path smoke coverage.
- HTTP/3 remains explicitly experimental and non-default.

### Documentation

- Doctests pass.
- Runtime examples execute against installed artifacts.
- Link validation passes.
- Compatibility, feature, versioning, installation, and security documentation match the candidate artifacts.

### Operational evidence

- One complete dry-run release workflow is green for the candidate SHA.
- No required job is skipped unexpectedly.
- The candidate checklist names the exact SHA and records every required result.
- The repository remains frozen except for release-blocking fixes after validation begins.
- The `0.1.0-rc1` tag is created only after all criteria above pass.

## Deliverables

The implementation agent should produce:

- cleaned lint policy and enforcement script;
- normalized CI matrix;
- working release dry-run mode;
- Rust package validation jobs;
- Python artifact installation jobs;
- CLI and FFI artifact smoke jobs;
- checksum and release manifest generation;
- SBOM and provenance jobs;
- restored process-level resource benchmarks;
- strict documentation execution jobs;
- compatibility report;
- candidate checklist tied to an exact commit SHA;
- updated release-process documentation.

## Recommended execution order

1. Track A: lint and CI cleanup.
2. Track I: documentation validation cleanup.
3. Track J: matrix definition and normalization.
4. Track B: release dry-run mode.
5. Tracks C–F: package, artifact, checksum, SBOM, and provenance validation.
6. Track G: differential closure.
7. Track H: resource-regression restoration.
8. Track K: final candidate run and sign-off.

## Completion definition

This follow-up is complete only when one exact commit SHA has a fully green release dry run and all final acceptance criteria are represented by concrete, reviewable evidence. A collection of locally passing commands, partially green workflows, or manually inferred success is insufficient.