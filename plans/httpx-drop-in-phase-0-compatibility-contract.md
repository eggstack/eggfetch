# HTTPX Drop-In Phase 0: Compatibility Contract and Mandatory Oracle

Status: ready for implementation handoff

## Purpose

Create a pinned, executable, fail-closed definition of HTTPX compatibility before adding more compatibility code.

The repository currently has useful native and differential tests, but the comparison dependencies are optional, reference tests may be skipped, and the existing compatibility matrix contains at least some incorrect statements about HTTPX defaults. This phase replaces informal compatibility claims with a versioned compatibility profile and machine-readable evidence.

The target reference for this phase is `httpx==0.28.1`.

## Baseline

Relevant existing areas include:

- `crates/eggfetch-python/tests/test_differential.py`
- `crates/eggfetch-python/tests/`
- `scripts/check_api_surface.py`
- `docs/reference/compatibility.md`
- `docs/migration/from-httpx.md`
- `.github/workflows/ci.yml`
- `plans/ROADMAP.md`
- the Python package exports under `crates/eggfetch-python/python/eggfetch/`

Before implementation, record the exact baseline SHA and current test counts in a phase status file.

## Non-goals

- Implementing missing HTTPX API objects.
- Correcting timeout, pooling, or lifecycle behavior beyond what is needed to construct the oracle.
- Adding transports, hooks, Trio support, or downstream package fixtures.
- Declaring any new compatibility stage.
- Treating private HTTPX modules as required public compatibility surface.
- Updating the reference target beyond HTTPX 0.28.1.

## Deliverables

1. A compatibility profile file pinning HTTPX 0.28.1 and defining the supported public surface.
2. Required CI installation of the pinned reference package.
3. A dedicated compatibility test job that fails on skips, missing dependencies, malformed manifests, or unexplained differences.
4. Generated reference and eggfetch public API manifests.
5. A machine-readable allowed-differences registry.
6. Corrected compatibility and migration documentation.
7. A deterministic local behavior fixture framework.
8. An implementation status file mapping every acceptance criterion to evidence.

## Track A — Define the compatibility profile

### A1. Add a machine-readable profile

Create a versioned profile, for example:

- `compat/httpx/0.28.1/profile.toml`
- `compat/httpx/0.28.1/allowed-differences.toml`
- `compat/httpx/0.28.1/README.md`

The profile must record:

- reference distribution name and exact version;
- supported Python versions;
- expected sync backends;
- expected async backends for the current phase;
- public modules included in the compatibility contract;
- public symbols excluded because they are private or deprecated;
- feature extras needed for optional HTTPX capabilities;
- manifest schema version;
- generator version;
- date and commit that established the profile.

The initial contract should include the public top-level `httpx` module and documented public APIs. Private modules beginning with `_` are excluded unless a later downstream compatibility decision explicitly includes one.

### A2. Define compatibility categories

Every measured item must be classified as one of:

- `required-now`: must match for the current stage;
- `required-later`: accepted gap assigned to a later roadmap phase;
- `intentional-difference`: reviewed and explicitly allowed;
- `not-public`: excluded from the contract;
- `not-applicable`: reference feature cannot apply to the selected product stage.

`required-later` is not a pass. It is a measured open gap that blocks the relevant stage.

### A3. Add allowed-difference schema validation

Each allowed difference must include:

- stable ID;
- category;
- affected symbol or behavior case;
- reference behavior;
- eggfetch behavior;
- rationale;
- compatibility-stage impact;
- security impact;
- migration impact;
- owner;
- review milestone or review date;
- tests that bound the difference.

Add a schema validator that fails on:

- duplicate IDs;
- missing fields;
- unknown categories;
- references to nonexistent symbols or behavior cases;
- expired review dates;
- an allowed difference with no associated test;
- an item marked `intentional-difference` that blocks the claimed stage.

## Track B — Generate public API manifests

### B1. Reference manifest generator

Create a Python script that imports the pinned HTTPX package in an isolated environment and records, at minimum:

- public top-level names;
- object kind: module, class, function, constant, enum, exception;
- `inspect.signature()` output where available;
- parameter kind, name, default representation, and annotation representation;
- class bases and MRO names;
- public methods and properties;
- documented instance attributes that can be observed from safe fixture construction;
- return-object type for approved constructor fixtures;
- module of origin;
- deprecation markers where detectable;
- reference package version.

The manifest must be stable across runs. Normalize memory addresses, object reprs, platform-dependent paths, and ordering.

### B2. Eggfetch manifest generator

Generate the same schema from the eggfetch compatibility surface. Do not maintain a second hand-written expected-name list as the primary oracle.

The generator must work against:

- an editable development install;
- a built wheel installed into a clean environment.

### B3. Manifest comparator

Create a comparator that reports:

- missing symbols;
- extra symbols;
- kind mismatches;
- signature mismatches;
- default mismatches;
- inheritance mismatches;
- property/method mismatches;
- return-type mismatches;
- allowed-difference matches;
- unexplained differences.

The comparator must exit nonzero on every unexplained difference and on stale allowed-difference entries that no longer correspond to a real delta.

Produce both JSON and concise human-readable reports.

### B4. Golden-manifest policy

Commit the normalized HTTPX 0.28.1 reference manifest. Do not regenerate it automatically on every ordinary CI run from an unpinned environment.

CI should:

1. verify the installed reference version;
2. optionally regenerate the reference manifest and compare it byte-for-byte with the committed golden file;
3. generate the eggfetch manifest;
4. compare eggfetch against the golden reference and allowed-difference registry.

A reference-manifest update requires an explicit compatibility-profile commit.

## Track C — Make differential tests mandatory

### C1. Pin reference dependencies

Add a dedicated compatibility dependency group or requirements lock containing at least:

- `httpx==0.28.1`;
- `requests` at an explicitly pinned version where requests comparison remains useful;
- pytest and required plugins;
- optional HTTPX extras only in dedicated jobs that exercise those extras.

The required compatibility job must install these dependencies directly. It must not rely on a developer's environment.

### C2. Remove optional comparison behavior

Refactor `test_differential.py` so required compatibility tests do not use `HAS_HTTPX` or skip when HTTPX is absent.

The required job must fail immediately if:

- HTTPX cannot be imported;
- the imported version is not exactly 0.28.1;
- Requests cannot be imported for a required Requests comparison;
- a required test is skipped;
- pytest reports deselected cases that should belong to the required profile.

Optional extra-specific tests may skip only in jobs whose profile explicitly excludes that extra.

### C3. Add skip auditing

Add a pytest plugin or post-run evaluator that records:

- skipped tests;
- xfailed tests;
- deselected tests;
- collection errors;
- reference version;
- profile name.

The required profile must fail on any skip or xfail not listed in a dedicated, machine-readable conditional-test policy.

### C4. Separate native and compatibility suites

Keep native eggfetch tests independently useful. Define explicit commands such as:

- native Python tests;
- HTTPX surface manifest tests;
- HTTPX behavioral differential tests;
- Requests migration comparisons;
- package-artifact compatibility smoke tests.

The required CI gate must depend on both native and compatibility jobs. A compatibility failure cannot be hidden by a green native matrix.

## Track D — Correct compatibility documentation

### D1. Audit defaults and documented claims

Compare every existing row in `docs/reference/compatibility.md` and every migration statement against the pinned reference.

At minimum, verify and correct:

- redirect defaults;
- pool timeout support;
- default timeout behavior;
- resource-limit defaults;
- proxy environment behavior;
- constructor signatures;
- response URL type;
- `raise_for_status()` return behavior;
- client thread-sharing guarantees;
- transport, hook, mount, and auth surfaces.

### D2. Generate documentation inputs

Where practical, generate compatibility tables from the machine-readable profile and current comparator result. Hand-written narrative may remain, but status labels must come from executable evidence.

### D3. Add claim linting

Add a lightweight documentation check that rejects unqualified phrases such as:

- `drop-in replacement`;
- `fully compatible`;
- `identical to HTTPX`;

unless the current profile status file records the required achieved stage.

## Track E — Build deterministic behavior fixtures

### E1. Local protocol server

Refactor the differential test server into reusable fixtures capable of deterministic cases for:

- all common methods;
- duplicate and unusual headers;
- query encoding and repeated parameters;
- redirects and redirect chains;
- auth challenge and credential echo;
- cookies and multiple `Set-Cookie` fields;
- JSON, form, raw, and multipart bodies;
- fixed and chunked responses;
- compression formats;
- delayed headers and delayed chunks;
- abrupt close and truncated bodies;
- invalid content length;
- malformed status line and headers;
- connection reuse observation;
- proxy forwarding and CONNECT;
- TLS and client certificate scenarios;
- HTTP/2 cases where the existing test harness supports them.

### E2. Behavior case schema

Represent cases with stable IDs and structured expected observations. Each case should identify:

- setup fixture;
- reference invocation;
- eggfetch invocation;
- normalized result fields;
- exception expectations;
- sync/async applicability;
- platform applicability;
- allowed-difference ID if any.

Do not compare only status codes. Normalize and compare request construction, response metadata, body state, exception type, exception attributes, and cleanup outcome where applicable.

### E3. Malformed-peer fixtures

Add a small raw TCP fixture layer for behavior that a compliant `http.server` cannot produce. It must be deterministic, bounded by test timeouts, and shut down cleanly after failures.

## Track F — CI and evidence

### F1. Dedicated compatibility jobs

Add stable CI jobs for:

- manifest generation and comparison;
- sync behavioral differential tests;
- asyncio behavioral differential tests;
- documentation truth checks;
- wheel-installed compatibility smoke tests when built artifacts are available.

The aggregate `Required CI Gate` must treat these jobs as required once this phase lands.

### F2. Artifact reports

Upload, even on failure:

- reference manifest;
- eggfetch manifest;
- comparator JSON;
- human-readable delta report;
- pytest skip audit;
- compatibility profile metadata.

### F3. Implementation status

Create `plans/httpx-drop-in-phase-0-status.md` during implementation. It must map every acceptance criterion below to:

- pass, fail, blocked, or not applicable;
- exact file or test evidence;
- CI run URL and commit SHA;
- remaining allowed differences.

## Expected files

Likely additions or changes include:

- `compat/httpx/0.28.1/profile.toml`
- `compat/httpx/0.28.1/reference-api.json`
- `compat/httpx/0.28.1/allowed-differences.toml`
- `compat/httpx/0.28.1/README.md`
- `scripts/generate_httpx_api_manifest.py`
- `scripts/compare_httpx_api_manifest.py`
- `scripts/validate_httpx_compat_profile.py`
- `scripts/check_compatibility_claims.py`
- `crates/eggfetch-python/tests/compat/`
- `.github/workflows/ci.yml`
- `docs/reference/compatibility.md`
- `docs/migration/from-httpx.md`
- `plans/httpx-drop-in-phase-0-status.md`

The implementation may choose different paths, but the separation between profile, generators, behavior fixtures, tests, and evidence must remain clear.

## Validation commands

The implementation status file must record exact working commands. Expected command shapes include:

```bash
python scripts/validate_httpx_compat_profile.py compat/httpx/0.28.1
python scripts/generate_httpx_api_manifest.py --package httpx --output /tmp/httpx.json
python scripts/generate_httpx_api_manifest.py --package eggfetch.compat.httpx --output /tmp/eggfetch.json
python scripts/compare_httpx_api_manifest.py \
  --reference compat/httpx/0.28.1/reference-api.json \
  --candidate /tmp/eggfetch.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml
pytest crates/eggfetch-python/tests/compat -q --strict-markers
```

## Acceptance criteria

This phase is complete only when all of the following are true:

- [ ] HTTPX 0.28.1 is pinned in a dedicated compatibility dependency definition.
- [ ] Required compatibility CI verifies the imported HTTPX version exactly.
- [ ] Required comparison tests cannot silently skip because HTTPX or Requests is absent.
- [ ] The required profile fails on unapproved skip, xfail, deselection, or collection error.
- [ ] A normalized HTTPX public API golden manifest is committed.
- [ ] An eggfetch manifest using the same schema is generated in CI.
- [ ] The comparator reports symbol, signature, default, inheritance, attribute, and kind differences.
- [ ] Every unexplained manifest difference fails CI.
- [ ] Allowed-difference records are schema validated and linked to tests.
- [ ] Stale allowed-difference records fail CI.
- [ ] Current compatibility documentation has been audited against the pinned reference.
- [ ] Incorrect redirect-default and pool-timeout statements are corrected.
- [ ] Unqualified drop-in claims are rejected until the required stage is achieved.
- [ ] Deterministic local behavior fixtures cover compliant and malformed peer cases.
- [ ] Sync and asyncio differential suites have stable case identifiers.
- [ ] Compatibility reports are retained as CI artifacts.
- [ ] The aggregate required gate includes the compatibility jobs.
- [ ] The implementation status file links a green CI run and exact candidate SHA.

## Handoff notes

Implementation should prioritize truth and measurement over closing gaps in this phase. A large red comparator report is an acceptable Phase 0 outcome if every delta is correctly classified and the oracle fails closed. Converting open gaps into false passes is not acceptable.
