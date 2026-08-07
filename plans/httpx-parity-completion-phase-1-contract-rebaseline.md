# HTTPX 0.28.1 Parity Completion — Phase 1: Contract Rebaseline

Status: ready for implementation handoff

Date: 2026-08-07

Roadmap commit: `1f8247f2c78f26fc80a000c26cc735a938e7c5da`

Audited repository baseline: `c66360827489c988f37b4aa9bd615b612258825d`

Last exact executable compatibility evidence SHA: `cf4680ac056bf241ca4f4e8fa0e076459bccc9e3`

Pinned reference: `httpx==0.28.1`

Compatibility designation: `Stage C candidate`

## Objective

Rebaseline the remaining HTTPX 0.28.1 compatibility contract before any new runtime changes are made.

The previous closure proves that the current active allowlist is internally explained, but it does not prove that all 121 active differences are worth retaining. This phase converts the current broad allowlist into a finite implementation inventory for Phases 2–5 and corrects stale or inaccurate compatibility documentation.

This phase is evidence and documentation work only. It must not modify Python or Rust runtime behavior.

## Why this phase exists

The current repository contains three different kinds of information that must be reconciled before further implementation:

1. exact-SHA closure evidence showing the current supported asyncio surface is stable;
2. an active API allowlist containing 121 stage-bounded differences, including several cheap public-contract mismatches;
3. historical inventory/documentation text that predates the August corrective passes and is no longer an accurate description of present behavior.

The implementation phases must be driven by the current reference and current main tree, not by stale July inventory text or by assumptions carried over from earlier plans.

## Scope firewall

### In scope

- `compat/httpx/0.28.1/profile.toml`;
- `compat/httpx/0.28.1/allowed-differences.toml` classification metadata where needed;
- `compat/httpx/0.28.1/upstream-test-inventory.md` or its current replacement if the repo already has one;
- generated candidate API manifest/output using the existing scripts;
- `docs/reference/compatibility.md`;
- the HTTPX compatibility wording in `README.md`;
- exact-SHA status/planning records needed to bind this rebaseline;
- a finite mapping from active difference IDs to Phases 2–5 or to an explicitly reviewed retained/deferred category.

### Out of scope

Do not modify:

- Python runtime code;
- Rust runtime code;
- native bindings;
- tests except a documentation/evidence helper test already required by an existing repository mechanism;
- CI workflows or `scripts/check.sh`;
- dependencies;
- release automation;
- compatibility semantics.

Do not add a new manifest generator, inventory framework, dashboard, registry, CI job, scheduled workflow, or evidence format.

## Track 0 — Freeze and verify the starting state

### 0.1 Record the implementation baseline

Record the current `main` SHA at the start of implementation.

Confirm whether executable files changed after the last exact executable compatibility evidence SHA:

`cf4680ac056bf241ca4f4e8fa0e076459bccc9e3`

The known descendants through the audited baseline are documentation-oriented. Do not assume that remains true at implementation time. Inspect commits/files since the evidence SHA.

If executable compatibility behavior has changed, stop treating `cf4680ac...` as the active implementation baseline and create a short exact-SHA delta note before proceeding.

### 0.2 Preserve accepted closure evidence

Treat the following previous results as historical evidence, not as proof for future executable changes:

- routine `./scripts/check.sh`: passed on `cf4680ac...`;
- full pinned compatibility suite: `1384 passed, 0 failed, 0 skipped, 0 xfailed`;
- API oracle: 121 allowed matches, zero unexplained differences, zero stale allowed entries, zero resolved-in-active entries, zero requires-resolution entries.

Do not rewrite these historical counts simply because later phases will change the active allowlist.

### 0.3 Confirm the pinned reference

Confirm the differential environment is using exactly:

```text
httpx==0.28.1
```

Do not silently update the target to a later HTTPX release during this program.

### Track 0 acceptance criteria

- Starting `main` SHA is recorded.
- Executable ancestry since `cf4680ac...` is understood.
- Historical evidence is preserved as historical evidence.
- The reference version is pinned exactly to 0.28.1.

## Track 1 — Regenerate the current candidate API view

### 1.1 Generate the candidate manifest

Use the existing repository command:

```sh
python scripts/generate_httpx_api_manifest.py \
  --package eggfetch.compat.httpx \
  --output /tmp/eggfetch-api.json
```

Do not create a new generator.

### 1.2 Compare against the pinned reference

Use the existing comparator:

```sh
python scripts/compare_httpx_api_manifest.py \
  --reference compat/httpx/0.28.1/reference-api.json \
  --candidate /tmp/eggfetch-api.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml \
  --json \
  --output /tmp/api-result.json
```

Capture:

- active allowed-match count;
- unexplained difference count;
- stale allowed-entry count;
- resolved-in-active count;
- requires-resolution count.

If these differ from the historical 121/0/0/0/0 result, explain the exact delta before classifying differences.

### 1.3 Group the active allowlist by public behavior

Do not review 121 records as 121 unrelated tasks. Group them by symbol/behavior case so one implementation change can close all related tuples.

At minimum, produce groups for:

- Headers collection/MRO/method surface;
- QueryParams collection/MRO surface;
- exception hierarchy and constructor signatures;
- NetRCAuth keyword shape;
- URL raw representation;
- status `codes` enum/type surface;
- Timeout/Limits/Proxy configuration objects;
- Client/AsyncClient and top-level helper signatures;
- transport constructor signatures and base classes;
- byte/sync/async stream base classes and method signatures;
- advanced transport parameters (`uds`, `local_address`, `socket_options`);
- SOCKS/proxy capability and environment proxy semantics;
- genuinely additive EggFetch-only public members;
- private/internal-reference differences that should remain excluded.

### Track 1 acceptance criteria

- Candidate manifest is generated from current main.
- Existing comparator runs without new tooling.
- Every active difference is assigned to a grouped public behavior area.
- Any delta from the historical 121-entry result is explained.

## Track 2 — Classify every active difference

Each grouped difference must be assigned to exactly one of the following buckets.

### 2.1 `must-close`

Use this category when all are true or substantially true:

- the symbol/member is public in HTTPX 0.28.1;
- the difference can affect valid downstream source code, runtime type checks, argument validation, or observable behavior;
- matching it does not require a second networking stack or a disproportionate architectural rewrite;
- the incompatibility is not justified by a security boundary.

Expected examples include:

- `Headers.setdefault()` and `Headers.popitem()`;
- mapping inheritance where downstream `isinstance`/ABC behavior differs;
- `StreamError` base class;
- `NetRCAuth(file=...)`;
- `URL.raw`;
- `codes` enum behavior;
- exact public helper/method signatures;
- supported constructor/default semantics;
- advanced direct transport parameters;
- SOCKS proxy support.

### 2.2 `intentional`

Use this only when the difference is reviewed and there is a positive reason to preserve it, for example:

- an EggFetch-only additive property/method that does not invalidate HTTPX-compatible calls;
- a security-hardening difference with documented safer semantics;
- an internal implementation detail that is not part of the public contract and cannot affect supported public behavior.

`intentional` is not a synonym for "already implemented differently" or "inconvenient to fix".

### 2.3 `deferred`

Use this for compatibility work deliberately outside this roadmap's intended scope, including:

- Trio backend support;
- a general AnyIO abstraction;
- Python 3.8/3.9 interpreter support;
- private HTTPX modules;
- HTTPX versions other than 0.28.1.

A deferred record must state the trigger for reconsideration. For Trio/AnyIO and Python 3.8/3.9, the trigger is a concrete intended downstream consumer that requires the feature/version.

### 2.4 Map `must-close` groups to implementation phases

Expected assignment:

- Phase 2: Python object/configuration contracts;
- Phase 3: exact signatures, transport inheritance, stream type surface;
- Phase 4: UDS, local address, socket options;
- Phase 5: SOCKS and related proxy-environment semantics.

If a difference spans phases, identify one owning phase and list prerequisites rather than duplicating ownership.

### Track 2 acceptance criteria

- Every active difference is `must-close`, `intentional`, or `deferred`.
- Every `must-close` group has one owning implementation phase.
- Every `intentional` difference has a positive technical rationale.
- Every `deferred` difference has an explicit reconsideration trigger.
- No public source-visible mismatch is retained merely because it is cheap to allowlist.

## Track 3 — Refresh stale compatibility inventory

### 3.1 Audit `upstream-test-inventory.md`

The existing July-era inventory contains descriptions that predate later corrective passes. Compare its assertions with current tests and current implementation.

Specifically check for stale statements involving:

- redirect default behavior;
- redirect Cookie regeneration/security;
- multipart/replay behavior;
- proxy environment handling;
- mounts and custom transports;
- WSGI/ASGI/MockTransport;
- raw streaming lifecycle;
- compressed raw streaming;
- cancellation and response metadata.

### 3.2 Regenerate using the existing process, if one exists

If the repository has an existing inventory generation/update command, use it.

If it does not, update the inventory manually or mark the old document prominently as historical and add a current bounded inventory using the same established format. Do not build a new generator solely for this phase.

### 3.3 Bind inventory to an exact SHA

The current inventory must state:

- source tree SHA inspected;
- pinned HTTPX version;
- generation/update date;
- whether rows are direct reference-derived, test-derived, or manually classified.

### Track 3 acceptance criteria

- Current documentation no longer presents July-era gap statements as present facts when they were closed in August.
- The inventory is exact-SHA-bound or clearly labeled historical.
- No new inventory framework is introduced.

## Track 4 — Correct user-facing compatibility claims

### 4.1 Correct the SOCKS statement

Current `docs/reference/compatibility.md` says the SOCKS proxy feature is "Not in HTTPX 0.28.1 public API, deferred."

That statement must be corrected. HTTPX 0.28.1 exposes SOCKS proxy support as an optional public capability through its SOCKS extra.

Use bounded wording equivalent to:

```text
SOCKS proxy | HTTPX 0.28.1 optional public feature; currently unsupported by EggFetch and scheduled for Phase 5.
```

Do not imply SOCKS is already implemented.

### 4.2 Correct the short README claim

The README currently uses a short headline equivalent to "HTTPX drop-in" while its detailed section correctly says the facade is a bounded Stage C candidate and is not an unrestricted replacement for every HTTPX transport/concurrency backend.

Change the short claim to wording such as:

```text
HTTPX compatibility facade — compatible asyncio surface targeting HTTPX 0.28.1
```

or an equivalent bounded statement.

Do not weaken the detailed compatibility information.

### 4.3 Audit proxy environment claims

HTTPX's documented environment behavior includes the standard proxy environment surface, including `ALL_PROXY` and `NO_PROXY` in addition to scheme-specific variables.

Determine whether current EggFetch behavior matches that surface. Do not infer support from documentation alone.

If `ALL_PROXY` or lowercase variants are missing, record the exact gap as `must-close` under Phase 5 rather than silently editing documentation to claim parity.

### 4.4 Audit SSL-context wording

Distinguish these separate concepts:

- client `verify=` accepting an SSLContext-like value;
- `Proxy(..., ssl_context=...)` constructor/public state;
- low-level HTTP transport TLS configuration.

Do not collapse them into one "SSL context unsupported" statement. Assign any real source-visible constructor mismatch to Phase 2 and any native transport behavior requirement to Phase 4 or 5 as appropriate.

### Track 4 acceptance criteria

- No active document says SOCKS is absent from HTTPX's public API.
- README short wording and detailed qualification are consistent.
- Proxy environment support is stated from verified behavior, not assumption.
- SSL-context differences are described at the correct API layer.

## Track 5 — Produce the implementation handoff inventory

Create or update one concise section in the parity status/planning records that lists the implementation groups assigned to each later phase.

For every group include:

- owning phase;
- relevant active difference IDs;
- primary symbols/files;
- reference behavior summary;
- current EggFetch behavior summary;
- whether native/core changes are expected;
- focused differential test that must exist before the allowlist entry is removed.

Do not duplicate the full later plan files. The purpose is traceability from the active difference ledger to the implementation program.

### Track 5 acceptance criteria

- Phases 2–5 each have a finite input list.
- No active `must-close` difference is ownerless.
- No implementation phase needs to rediscover its intended scope from scratch.

## Validation

Because this phase is documentation/evidence-only, validation is intentionally bounded.

Required:

1. candidate API manifest generation;
2. API comparison against the active allowlist;
3. review of changed Markdown/TOML for exact-SHA and reference correctness;
4. `./scripts/check.sh` only if the repository's normal workflow executes it for the change or if non-documentation files were unexpectedly touched.

Do not rerun expensive transport/end-to-end tests solely for prose changes.

## Phase acceptance criteria

Phase 1 is complete only when:

- current main has an exact compatibility rebaseline;
- all active differences are classified and assigned;
- stale upstream inventory claims are corrected or clearly historical;
- SOCKS is correctly described as an HTTPX optional public feature not yet supported by EggFetch;
- README no longer overclaims unrestricted drop-in status;
- `ALL_PROXY`/proxy environment and SSL-context gaps are explicitly audited;
- Phases 2–5 have a finite difference-ID inventory;
- no runtime, dependency, CI, or release changes were introduced.

## Rejection criteria

Reject the pass if:

- it changes runtime behavior;
- it deletes allowlist entries without implementation/reference evidence;
- it converts public incompatibilities into `intentional` merely to reduce planned work;
- it updates the HTTPX target version;
- it claims SOCKS, UDS, local binding, or socket-option support before implementation;
- it creates a new compatibility-evidence framework instead of using existing scripts;
- it rewrites historical exact-SHA evidence as if it described the new baseline.

## Stop conditions

Stop and report a bounded blocker if:

- the current main tree contains executable compatibility changes after `cf4680ac...` that have not been validated;
- the reference manifest and installed `httpx==0.28.1` disagree materially;
- active allowlist records cannot be traced to current oracle output;
- an apparently public mismatch actually depends on private httpcore behavior and cannot be classified from public HTTPX behavior.

The blocker report must name the exact symbol/difference IDs and the smallest reproducer. Do not resolve ambiguity by broadening scope.

## Suggested commit decomposition

1. `docs: rebaseline active HTTPX compatibility differences`
2. `docs: refresh HTTPX upstream parity inventory`
3. `docs: correct bounded HTTPX compatibility claims`

These may be one commit if the diff remains reviewable and no generated artifact is mixed opaquely with manual claims.

## Handoff checklist

Report:

- starting SHA;
- final Phase 1 SHA;
- current allowed-difference count;
- counts by `must-close`, `intentional`, and `deferred`;
- grouped difference IDs assigned to Phases 2–5;
- API oracle command/result;
- inventory file updated and baseline SHA recorded;
- exact README/compatibility wording changed;
- result of the `ALL_PROXY` audit;
- result of the SSL-context surface audit;
- confirmation that no executable code, dependency, CI, or release changes were made.
