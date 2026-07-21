# HTTPX Drop-In Phase 5: Downstream Compatibility and Substitution Validation

Status: ready for implementation handoff

## Purpose

Prove that the compatibility layer works for real HTTPX consumers, not only for hand-selected eggfetch tests.

This phase expands the pinned manifest and differential oracle into a representative substitution program. It runs unmodified consumer code, exercises framework test clients, mock and custom transports, SDK patterns, streaming, proxying, custom auth, and error handling, and validates package substitution in clean environments.

A passing native suite is not sufficient. A passing API manifest is not sufficient. The exit criterion is evidence that representative downstream users can run without eggfetch-specific branches.

## Dependencies

Phase 0 provides the oracle and allowed-difference policy. Phase 1 provides production semantics. Phase 2 provides the object model and client surface. Phase 3 provides streaming. Phase 4 provides transports, extensions, auth, and backend behavior.

Phase 5 fixtures may be added incrementally earlier, but this phase cannot close until the required prior-phase gates are green.

## Non-goals

- Claiming compatibility with every package that imports private HTTPX internals.
- Modifying downstream packages to recognize eggfetch-specific types.
- Treating monkeypatches that alter assertions as successful compatibility.
- Running arbitrary untrusted downstream test suites with production credentials or network access.
- Depending on live public endpoints for required CI.
- Chasing unpinned latest versions on every CI run.
- Benchmark optimization that weakens correctness.

## Deliverables

1. A versioned downstream compatibility inventory.
2. A broad deterministic HTTPX differential behavior corpus.
3. Selected upstream HTTPX public-contract tests or equivalent derived cases.
4. Pinned representative downstream package suites run unmodified.
5. Clean-environment substitution and import-origin tests.
6. Compatibility resource and performance budgets.
7. A machine-readable downstream evidence report.
8. A compatibility-stage decision document.
9. A phase status file.

## Track A — Define the downstream compatibility portfolio

### A1. Consumer categories

Select at least one maintained, pinned representative for each applicable category:

- ordinary synchronous API client;
- ordinary asyncio API client;
- framework test client using ASGI transport;
- WSGI test client or integration;
- mock transport user;
- custom transport subclass;
- event-hook instrumentation;
- custom auth flow;
- streaming upload and download;
- proxy and environment configuration;
- HTTP/2-capable consumer;
- SDK that inspects request/response exceptions;
- package using `base_url`, params, headers, cookies, and timeouts heavily;
- Trio/AnyIO consumer for Stage D.

Do not choose only trivial clients that call `httpx.get()`.

### A2. Selection criteria

Each fixture must record:

- package and exact version;
- license;
- why it represents a compatibility category;
- public versus private HTTPX usage;
- test subset executed;
- expected network isolation;
- optional dependencies;
- known incompatibilities;
- update owner and review cadence.

Prefer packages that use documented public APIs. A package that relies on private internals may be included as informational evidence but cannot define the public contract automatically.

### A3. Pinning and updates

Commit lock files or exact constraints for downstream fixtures. Updates require a dependency-delta review and must not silently change compatibility acceptance.

Add scheduled discovery that reports newer versions without updating required CI automatically.

## Track B — Expand the behavior corpus

### B1. Request construction cases

Cover:

- all methods and arbitrary methods;
- base URL resolution;
- repeated query parameters;
- duplicate headers;
- cookies and conflicts;
- all body types;
- auto headers;
- explicit host and content headers;
- auth and auth disable;
- timeout and extensions;
- invalid inputs and exception classes.

### B2. Redirect cases

Cover status-specific method and body handling for 301, 302, 303, 307, and 308, including:

- relative and absolute locations;
- scheme-relative locations;
- malformed locations;
- fragments;
- cross-origin auth stripping;
- cookie updates;
- proxy/mount rerouting;
- history bodies;
- maximum redirects;
- non-replayable streams;
- total timeout budget.

### B3. Protocol and body cases

Cover:

- HTTP/1.0 and HTTP/1.1;
- keep-alive and close;
- chunked bodies;
- content length mismatch;
- interim responses where supported;
- HTTP/2 negotiation and multiplexing;
- connection close at each protocol point;
- compressed and multiply encoded bodies where supported;
- malformed encodings;
- slow headers, slow chunks, and stalled uploads;
- early consumer close.

### B4. TLS and proxy cases

Cover:

- trusted and untrusted roots;
- hostname mismatch;
- custom CA;
- client certificates;
- minimum/maximum TLS policy where exposed;
- HTTP proxy forwarding;
- CONNECT;
- proxy auth;
- environment proxies;
- `NO_PROXY` cases;
- SOCKS when included;
- proxy and origin failures separately.

### B5. Transport and extension cases

Cover:

- custom sync and async transports;
- mounts and route priority;
- mock transport;
- WSGI and ASGI;
- hooks and hook failures;
- custom extensions;
- custom auth multi-exchange flows;
- backend cancellation.

### B6. Exception normalization

For every failure case compare:

- class;
- superclass relationships;
- message after approved normalization;
- request attachment;
- response attachment;
- URL and method context;
- cause/context behavior where public;
- resource cleanup.

## Track C — Upstream HTTPX test leverage

### C1. Inventory upstream tests

At the pinned HTTPX tag, classify tests into:

- public contract and suitable to run unchanged;
- public contract requiring only fixture adaptation;
- httpcore-internal and not applicable;
- packaging or implementation-internal;
- private behavior intentionally excluded.

Record license attribution and source commit.

### C2. Run unchanged where possible

Prefer executing selected tests against an import-substituted compatibility package with no assertion changes.

Allowed harness changes include:

- selecting the package under test through environment or isolated installation;
- providing equivalent local certificate fixtures;
- adapting test collection paths;
- excluding explicitly classified private implementation tests.

Do not rewrite expected values.

### C3. Derived cases

Where an upstream test is too coupled to HTTPX internals but describes a public behavior, reproduce the behavior in the repository's own differential schema and cite the source test identifier in fixture metadata.

### C4. Coverage report

Generate a report showing which public HTTPX modules, symbols, and behavior categories are covered by upstream-derived or local tests. Uncovered required symbols must block the stage or receive an explicit plan.

## Track D — Unmodified downstream suites

### D1. Isolation

Run each downstream fixture in an isolated environment with:

- no upstream HTTPX installed when testing the compatibility distribution;
- no live credentials;
- network disabled except deterministic local fixtures;
- temporary home/config directories;
- sanitized proxy and certificate environment;
- strict time limits;
- captured package and import metadata.

### D2. No source modifications

The downstream package source and tests must remain byte-identical to the pinned fixture, except for a generic package-under-test selection mechanism that does not contain eggfetch-specific behavior.

Patches that change expected exception types, skip failures, or branch on eggfetch are prohibited for required evidence.

### D3. Framework test clients

Include representative use of:

- Starlette/FastAPI-style test client integration where compatible versions use HTTPX public APIs;
- ASGI lifespan policy if the target transport does or does not handle it;
- streaming request/response tests;
- app exception propagation;
- base URL and client-address configuration.

### D4. SDK fixtures

Select SDKs that exercise:

- sync and async clients;
- streaming responses;
- multipart uploads;
- custom timeouts and limits;
- request/response exception inspection;
- retry behavior implemented above the HTTP layer;
- event hooks or custom transports where available.

Mock remote APIs locally. Do not use production service credentials.

### D5. Mocking ecosystem

Test at least one public-API mocking pattern. If common mocking packages rely primarily on private HTTPX internals, classify that explicitly and decide whether public transport compatibility is sufficient or a narrow optional adapter is justified.

## Track E — Package substitution strategy tests

### E1. Compatibility module mode

Test consumers with an explicit compatibility import mode during development, such as import injection or a dedicated module path, while preserving consumer code otherwise.

### E2. Top-level compatibility distribution

Once Phase 5 is near closure, build a separate candidate distribution that provides the `httpx` top-level module.

Validate:

- installation into a clean environment without upstream HTTPX;
- declared conflict or mutually exclusive install policy;
- `import httpx` origin points to the compatibility distribution;
- emulated HTTPX version is available where consumers inspect it;
- eggfetch implementation version is separately available;
- uninstall removes the shim cleanly;
- installing upstream HTTPX afterward fails clearly or replaces the shim according to documented package-manager behavior;
- ordinary `eggfetch` installation does not shadow `httpx`.

### E3. Metadata and dependency resolution

Test pip resolution for:

- a package requiring `httpx>=0.27,<0.29`;
- direct compatibility-distribution installation;
- extras such as HTTP/2, SOCKS, or CLI where relevant;
- editable and wheel installs;
- constraints files;
- uninstall/reinstall cycles.

A distribution with a different project name may not satisfy a dependency declared on `httpx` automatically. Document this packaging limitation honestly and determine whether an explicit replacement workflow, wheel name, or controlled environment image is required. Do not claim transparent dependency resolution unless it is proven.

## Track F — Compatibility performance and resource budgets

### F1. Purpose

Performance is not the primary parity oracle, but catastrophic regressions can make an otherwise compatible client unusable.

Measure eggfetch compatibility mode against pinned HTTPX for:

- import time;
- client construction;
- one-shot request;
- reused HTTP/1.1 request;
- HTTP/2 multiplexing;
- small and large body throughput;
- sync and async streaming;
- multipart upload;
- proxy request;
- mock/ASGI transport overhead;
- memory and allocations;
- threads and tasks.

### F2. Budgets

Commit threshold policy that distinguishes:

- correctness blockers;
- severe regression blockers;
- informational differences;
- expected Rust-engine advantages.

Do not require eggfetch to beat HTTPX in every microbenchmark. Require no unbounded resource growth and no unacceptable compatibility-facade overhead.

### F3. Reproducibility

Record hardware/runner class, Python version, build profile, warmup, sample count, confidence interval, and baseline commit. Avoid comparing noisy shared-runner values with narrow thresholds.

## Track G — Failure triage and allowed differences

### G1. Stable failure IDs

Every downstream or differential failure must map to:

- compatibility case ID;
- public symbol or behavior;
- roadmap phase owner;
- severity;
- stage impact;
- allowed-difference record if approved.

### G2. No blanket skips

Do not skip an entire downstream suite because one optional feature is missing. Split profiles or mark individual cases with machine-readable policy.

### G3. Flake policy

A flaky compatibility test is a test defect or product defect to investigate. Required jobs must not use unrestricted retries. If one rerun is temporarily allowed for diagnosis, retain both attempts and keep the gate failed until the cause is resolved or a time-bounded quarantine is approved.

## Track H — Evidence report

Generate `compatibility-evidence.json` containing:

- schema version;
- eggfetch commit and package version;
- reference HTTPX version;
- compatibility stage under evaluation;
- platform/Python/backend matrix;
- API manifest summary;
- differential case totals and failures;
- upstream test inventory and result;
- downstream package versions and results;
- package substitution result;
- resource/performance summary;
- allowed differences;
- overall pass determined fail-closed.

Also generate a concise Markdown report for review.

## Track I — Compatibility-stage decision

At phase end, write a decision record stating the highest justified stage:

- Stage A: production-grade eggfetch;
- Stage B: HTTPX-compatible network subset;
- Stage C: asyncio drop-in;
- Stage D: full supported drop-in.

The decision must list blockers to the next stage and must be generated from evidence, not roadmap completion labels.

## Expected files

Likely additions include:

- `compat/downstream/manifest.toml`;
- per-package fixture metadata and lock files;
- `compat/httpx/0.28.1/behavior-cases/`;
- scripts to create isolated environments and run fixtures;
- selected upstream test inventory;
- compatibility report generators;
- benchmark compatibility profiles;
- CI workflow jobs or reusable workflows;
- `docs/reference/downstream-compatibility.md`;
- `plans/httpx-drop-in-phase-5-status.md`;
- a compatibility-stage decision record.

## Acceptance criteria

This phase is complete only when:

- [ ] A versioned downstream portfolio covers every required consumer category.
- [ ] Every fixture records exact package version, license, public API use, and test scope.
- [ ] Required fixtures run without live public network or credentials.
- [ ] The differential corpus covers request construction, redirects, protocol failures, TLS, proxies, streaming, transports, hooks, auth, and cancellation.
- [ ] Exception class and context are compared for every failure case.
- [ ] Public-contract tests from the pinned HTTPX source are inventoried and appropriately reused or derived.
- [ ] Public API coverage has no unexplained required gaps.
- [ ] Representative sync and asyncio consumers run unmodified.
- [ ] Framework test-client fixtures pass with unmodified consumer code.
- [ ] Mock/custom transport fixtures pass with unmodified consumer code.
- [ ] Streaming and multipart SDK fixtures pass with unmodified consumer code.
- [ ] Exception-inspecting SDK fixtures receive compatible request and response context.
- [ ] Trio/AnyIO downstream fixtures pass before Stage D is claimed.
- [ ] No required fixture branches on eggfetch or changes expected assertions.
- [ ] Clean-environment compatibility-module substitution succeeds.
- [ ] The top-level compatibility distribution installs and imports according to documented policy.
- [ ] Ordinary eggfetch installation never shadows upstream `httpx`.
- [ ] Dependency-resolution limitations are explicitly proven and documented.
- [ ] Compatibility mode has committed resource and severe-regression budgets.
- [ ] Required compatibility jobs do not use blanket skips or unrestricted reruns.
- [ ] `compatibility-evidence.json` is generated and fails closed.
- [ ] A compatibility-stage decision is committed and matches the evidence.
- [ ] `plans/httpx-drop-in-phase-5-status.md` links exact CI, fixture, manifest, and package evidence.

## Handoff notes

Choose the downstream portfolio for breadth of public API behavior, not brand recognition. A few large SDKs that all use the same simple `AsyncClient.post(json=...)` pattern provide less evidence than a smaller set spanning transports, test clients, streaming, auth, hooks, proxies, exceptions, and multiple async backends.
