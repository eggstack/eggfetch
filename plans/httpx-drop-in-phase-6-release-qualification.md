# HTTPX Drop-In Phase 6: Compatibility Release Qualification

Status: ready for implementation handoff

## Purpose

Convert a passing implementation and downstream compatibility program into defensible, immutable release evidence for the exact compatibility stage claimed.

This phase does not assume that completion of the earlier implementation phases automatically authorizes a drop-in claim. It validates built artifacts, packaging and import behavior, security and resource properties, supported platforms, compatibility manifests, canary operation, and rollback procedures against one frozen candidate commit.

## Dependencies

The intended release stage determines prerequisites:

- Stage A requires Phase 1 plus production validation.
- Stage B requires Phases 0 through 3 and the relevant Phase 5 evidence.
- Stage C requires Phases 0 through 5 for asyncio and extension surfaces.
- Stage D requires all prior phases, including supported Trio/AnyIO and optional-extra decisions.

The release workflow and required CI gate must be green before an immutable candidate dry run is accepted.

## Non-goals

- Publishing before dry-run evidence is complete.
- Expanding API or protocol scope during release qualification.
- Changing compatibility target from HTTPX 0.28.1.
- Waiving a failed supported platform without updating support claims and restarting qualification.
- Treating source-tree tests as artifact tests.
- Claiming full drop-in behavior when packaging cannot transparently satisfy downstream dependency resolution.
- Enabling experimental HTTP/3 as a compatibility default.

## Deliverables

1. Final compatibility packaging and import policy.
2. Version and compatibility metadata policy.
3. Built artifact matrix and clean-install smoke suite.
4. Immutable compatibility and production evidence manifests.
5. Security, fuzzing, fault-injection, and soak evidence.
6. Platform and Python support evidence.
7. Canary deployment and rollback procedure.
8. Documentation and package metadata aligned to the achieved stage.
9. A successful immutable non-publishing release dry run.
10. A release decision and phase status file.

## Track A — Finalize package architecture

### A1. Native distribution

Keep `eggfetch` as the primary native distribution and top-level module. It may expose eggfetch-native additions and a stable compatibility submodule.

Validate that ordinary installation:

- never creates a top-level `httpx` module;
- does not conflict with upstream HTTPX;
- supports using eggfetch and upstream HTTPX in the same environment;
- reports the native eggfetch version accurately;
- includes the compatibility profile and runtime version helpers needed by users.

### A2. Compatibility distribution

If the achieved stage justifies a top-level substitution distribution, define a separate artifact, for example an explicitly named replacement package that installs the `httpx` module.

The final name and metadata must be reviewed against package-index policy. The design must account for the fact that a differently named distribution does not automatically satisfy another package's dependency on the project name `httpx`.

The release decision must distinguish:

- import drop-in after explicit environment preparation;
- source drop-in after changing one import;
- transparent package dependency replacement;
- fully transparent installation replacement.

Only claim the level actually proven.

### A3. Conflict policy

Prove behavior when:

- upstream HTTPX is already installed;
- compatibility distribution is installed first;
- either package is upgraded;
- either package is uninstalled;
- editable installs are involved;
- user-site packages shadow virtual-environment packages;
- zipapp or packaged application environments are used where supported.

Do not permit two distributions to write the same top-level files silently.

### A4. Import-origin diagnostics

Expose a safe runtime diagnostic showing:

- compatibility provider;
- eggfetch implementation version;
- emulated HTTPX version/profile;
- native extension build/version;
- active compatibility stage.

Do not overload `httpx.__version__` with the eggfetch implementation version if consumers expect the emulated HTTPX version. Provide a separate implementation-version attribute or module.

## Track B — Versioning and compatibility metadata

### B1. Version dimensions

Maintain separate dimensions for:

- eggfetch release version;
- Rust crate versions;
- native extension version;
- compatibility profile version;
- emulated HTTPX version;
- compatibility stage;
- evidence schema version.

Define which changes require major, minor, or patch increments under the pre-1.0 and post-1.0 policy.

### B2. Compatibility manifest

Generate a signed or checksummed immutable manifest containing:

- candidate SHA;
- eggfetch version;
- emulated HTTPX version;
- achieved stage;
- supported Python versions;
- supported platforms and architectures;
- supported async backends;
- supported optional extras;
- public API comparator totals;
- differential behavior totals;
- downstream fixture results;
- allowed differences;
- known limitations;
- build workflow and run IDs;
- artifact hashes;
- generation timestamp.

The manifest's overall pass must be computed fail-closed.

### B3. Runtime compatibility query

Provide a supported API or metadata file allowing applications to determine the compatibility profile without parsing documentation.

## Track C — Artifact matrix

### C1. Python artifacts

Define and build the declared matrix for:

- Python 3.10 through 3.13 or the then-approved support range;
- Linux x86_64;
- Linux aarch64 if claimed;
- macOS x86_64;
- macOS arm64;
- Windows x86_64;
- source distribution;
- optional compatibility distribution artifacts;
- optional extras that alter native dependencies.

If abi3 is adopted, prove compatibility on every declared interpreter rather than assuming one wheel tag is sufficient.

### C2. Rust and CLI artifacts

Continue validating publishable Rust crates and native CLI artifacts, but keep HTTPX compatibility qualification focused on Python artifacts. Cross-language releases must identify whether they share the same candidate SHA.

### C3. Artifact provenance

For every artifact record:

- filename;
- target and Python tag;
- size;
- SHA-256;
- build job and run attempt;
- candidate SHA;
- toolchain versions;
- SBOM or package dependency manifest;
- signature/attestation status;
- smoke-test result.

### C4. No post-candidate changes

Any release-relevant change after the validated candidate SHA requires a new candidate and complete rerun. Documentation that changes the compatibility claim is release relevant.

## Track D — Clean-install artifact smoke tests

### D1. Native eggfetch wheel

Install each built wheel into a new environment and test:

- import and version;
- native compatibility submodule;
- sync and async request;
- HTTP/1.1 and HTTP/2 where available;
- timeout and limits;
- TLS verification;
- proxy;
- cookies and auth;
- streaming upload and download;
- multipart;
- close and interpreter shutdown;
- coexistence with upstream HTTPX.

### D2. Compatibility wheel

For each compatibility wheel, test:

- `import httpx` origin;
- public API manifest from installed files;
- representative request and response behavior;
- transport, ASGI, WSGI, mock, hooks, auth, and backend extras required by the stage;
- downstream fixture subset;
- version/profile diagnostics;
- uninstall cleanliness;
- upstream conflict behavior.

### D3. Source distribution

Build wheels from the sdist in an isolated environment without repository files. Run the required smoke subset and compare generated artifact metadata to directly built wheels.

### D4. Package content

Reject:

- source tests or corpora not intended for distribution;
- secrets or local paths;
- duplicate native libraries;
- unexpected top-level modules;
- stale compatibility manifests;
- generated artifacts with mismatched versions;
- missing type information or marker files if promised;
- non-deterministic file ownership between distributions.

## Track E — Security qualification

### E1. Dependency policy

Run and retain:

- Rust advisory audit;
- dependency license/source policy;
- Python dependency audit;
- lockfile review;
- SBOM generation;
- action pin verification;
- artifact provenance checks.

A known advisory requires an explicit severity and exposure decision. Critical reachable advisories block release.

### E2. Compatibility threat review

Review at least:

- environment proxy injection;
- netrc credential discovery;
- redirect credential stripping;
- URL display redaction;
- proxy authorization leakage;
- TLS verify/cert combinations;
- custom transport trust boundary;
- WSGI/ASGI app exception handling;
- event-hook secret exposure;
- request/response exception reprs;
- decompression and stream limits;
- SOCKS credential handling;
- compatibility distribution shadowing and dependency confusion.

### E3. Fuzzing

Run relevant fuzz targets for a defined budget and candidate SHA, including:

- URLs and query parameters;
- headers;
- cookies;
- redirect locations;
- proxy URLs and no-proxy matching;
- auth challenges;
- multipart encoding;
- content decoding;
- timeout and stream state machines;
- transport routing patterns;
- compatibility manifest parsing.

No reproducible crash, panic, memory-safety issue, credential leak, or unbounded allocation may remain open.

### E4. Malformed peer tests

Retain candidate evidence for malformed HTTP/1.1, HTTP/2, TLS, proxy, compression, and stream behavior. The client must fail safely and release resources.

## Track F — Resource, concurrency, and soak qualification

### F1. Required short profiles

Run on every release candidate:

- repeated client create/close;
- repeated connection failure;
- timeout/cancellation churn;
- concurrent sync thread use;
- concurrent asyncio task use;
- concurrent Trio use for Stage D;
- many active streams;
- early-close streams;
- proxy and TLS churn;
- ASGI/mock transport churn.

### F2. Long soak profiles

Run defined-duration workloads for:

- keep-alive HTTP/1.1;
- HTTP/2 multiplexing;
- mixed-origin traffic;
- streaming uploads/downloads;
- cancellation under load;
- proxy traffic;
- repeated auth challenges;
- downstream SDK workload;
- compatibility distribution import/client churn.

### F3. Stability thresholds

Commit thresholds for:

- resident memory trend;
- open descriptors/handles;
- threads;
- runtime tasks;
- active connections;
- pool permits/waiters;
- request latency percentiles;
- error-rate consistency;
- shutdown duration.

A plateau within a defined envelope is required. Merely avoiding process failure is insufficient.

## Track G — Performance qualification

### G1. Regression baseline

Compare the candidate to:

- the previous eggfetch release;
- the approved Phase 5 baseline;
- pinned HTTPX for contextual comparison.

### G2. Required workloads

Include:

- import and startup;
- one-shot sync request;
- reused sync request;
- concurrent async requests;
- HTTP/2 multiplexing;
- streaming throughput;
- multipart upload;
- proxy;
- mock and ASGI transport;
- custom auth;
- object construction and URL/query operations.

### G3. Gate policy

Fail on severe regression relative to the previous approved eggfetch baseline. Do not fail solely because a microbenchmark is slower than HTTPX if the committed product budget is satisfied.

## Track H — Canary and operational validation

### H1. Canary applications

Run the candidate in controlled applications representing:

- sync service worker;
- asyncio service;
- framework test suite;
- streaming client;
- proxy-enabled client;
- SDK consumer;
- Trio consumer for Stage D.

### H2. Observability

Collect:

- request counts and errors;
- timeout classes;
- connection reuse indicators;
- resource metrics;
- shutdown behavior;
- compatibility fallback or unsupported-surface errors;
- performance relative to the existing client.

### H3. Rollback criteria

Define immediate rollback triggers, including:

- credential leakage;
- TLS validation regression;
- deadlock or process hang;
- unbounded resource growth;
- materially elevated timeout/error rate;
- downstream incompatibility not present in qualification;
- package import conflict.

Canary evidence must identify candidate SHA and artifact hashes.

## Track I — Documentation and claims

### I1. Stage-specific language

Use only the achieved claim:

- `production-grade eggfetch client`;
- `HTTPX-compatible network-client subset`;
- `HTTPX-compatible asyncio drop-in`;
- `HTTPX 0.28.1-compatible drop-in for the supported profile`.

Avoid unqualified `full HTTPX replacement` unless Stage D passes with no stage-blocking differences.

### I2. Compatibility documentation

Publish:

- target HTTPX version;
- supported modules and extras;
- backend support;
- packaging/import procedure;
- allowed differences;
- unsupported private APIs;
- migration and rollback instructions;
- runtime profile diagnostic;
- security implications of `trust_env`, proxies, netrc, and verification options.

### I3. Classifiers and metadata

Update development-status classifiers only when the release evidence supports them. Ensure project descriptions do not overstate compatibility.

## Track J — Immutable release dry run

### J1. Freeze candidate

Select one full candidate SHA after required CI is green. Use an immutable non-publishing validation ref. Verify every job checks out the same commit.

### J2. Run with publishing disabled

The dry run must build and validate every declared artifact while being technically unable to publish to package registries or create the final release/tag.

### J3. Aggregate evidence

Produce a final release evidence bundle containing:

- compatibility manifest;
- production resource report;
- API and behavior comparator reports;
- downstream evidence;
- artifact hashes and SBOMs;
- security and fuzz reports;
- platform smoke results;
- canary report;
- CI job result map;
- overall pass.

### J4. Prove no side effects

Record that no package, release, final tag, or production deployment occurred because of the dry run.

## Track K — Release decision

Create a decision record with:

- candidate SHA;
- requested version;
- achieved compatibility stage;
- exact claim text approved for release;
- all allowed differences;
- all unsupported surfaces;
- artifact/platform coverage;
- canary outcome;
- known risks;
- rollback owner and procedure;
- approve or reject decision.

A rejected candidate is a valid outcome and must not be relabeled as a partial success release.

## Expected files

Likely changes or additions include:

- compatibility distribution packaging metadata;
- runtime compatibility-profile module;
- release and CI workflows;
- artifact smoke scripts;
- compatibility and release evidence generators;
- security/fuzz/soak scripts and reports;
- package conflict tests;
- release documentation;
- canary runbook;
- compatibility-stage decision record;
- `plans/httpx-drop-in-phase-6-status.md`.

## Acceptance criteria

This phase is complete only when:

- [ ] Native eggfetch and upstream HTTPX coexist cleanly in one environment.
- [ ] Ordinary eggfetch installation never shadows the `httpx` module.
- [ ] The compatibility distribution has a tested, explicit conflict and replacement policy.
- [ ] Runtime diagnostics report provider, implementation version, emulated version, profile, and stage.
- [ ] Version fields across crates, Python packages, extension, profile, and release input are consistent.
- [ ] The compatibility manifest is immutable, checksummed, and fail-closed.
- [ ] Every declared Python/platform artifact is built from the same candidate SHA.
- [ ] Every wheel passes clean-install native and compatibility smoke tests appropriate to its stage.
- [ ] The sdist builds and passes smoke tests without repository-only files.
- [ ] Package contents contain no unexpected top-level modules, secrets, stale manifests, or duplicate native libraries.
- [ ] Artifact hashes, SBOMs, provenance, toolchains, and smoke results are recorded.
- [ ] Rust and Python dependency security checks satisfy release policy.
- [ ] Compatibility threat review has no unresolved release-blocking finding.
- [ ] Candidate fuzz runs have no unresolved reproducible failure.
- [ ] Malformed peer tests fail safely and release resources.
- [ ] Required concurrency and cancellation profiles stay within committed limits.
- [ ] Long soak profiles plateau within memory, descriptor, thread, task, and connection thresholds.
- [ ] Severe performance regressions against the approved eggfetch baseline are absent.
- [ ] Canary applications pass without rollback triggers.
- [ ] Documentation and metadata use only the achieved compatibility-stage claim.
- [ ] One complete immutable non-publishing release dry run is green.
- [ ] The evidence bundle reports overall pass and is internally consistent.
- [ ] No release-relevant changes exist after the validated candidate SHA.
- [ ] A release decision explicitly approves or rejects the candidate and records the exact claim.
- [ ] `plans/httpx-drop-in-phase-6-status.md` links all immutable evidence and workflow runs.

## Handoff notes

The final claim should be narrower than the evidence only by choice, never broader. Packaging constraints are part of drop-in compatibility: matching Python behavior does not make a differently named distribution transparently satisfy dependencies on `httpx`. The release decision must state exactly what substitution workflow is supported.
