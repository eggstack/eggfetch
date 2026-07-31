# HTTPX Parity Correction Phase 5 — Focused Differential Closure and Claim Reconciliation

Status: ready for implementation handoff

Depends on:

- `plans/httpx-parity-correction-roadmap.md`
- `plans/httpx-parity-correction-phase-1-entrypoints-client-lifecycle.md`
- `plans/httpx-parity-correction-phase-2-request-response-semantics.md`
- `plans/httpx-parity-correction-phase-3-transport-mount-hook-dispatch.md`
- `plans/httpx-parity-correction-phase-4-redirect-auth-cookie-state.md`

## Objective

Close this corrective line of work with focused, reproducible differential evidence against `httpx==0.28.1`, while keeping the repository’s verification footprint lean.

This phase is not permission to revive the previous large qualification apparatus or add another layer of CI evidence plumbing. It must reuse the existing compatibility suite, API oracle, routine validation command, and currently retained downstream checks.

The result must be one of two truthful outcomes:

- **Parity correction complete:** every required finding is fixed and the existing exact-SHA checks pass; or
- **Stage C candidate with explicit blockers:** each remaining gap is narrowly documented, tested as an intentional difference, and removed from unsupported drop-in claims.

A high test count, generated report, or passing import smoke test does not substitute for behavior-specific evidence.

## Audited baseline and sources

Use:

- reference package: `httpx==0.28.1`;
- candidate import: `eggfetch.compat.httpx`;
- existing profile: `compat/httpx/0.28.1/profile.toml`;
- active differences: `compat/httpx/0.28.1/allowed-differences.toml`;
- resolved ledger: `compat/httpx/0.28.1/resolved-differences.toml`;
- existing compatibility tests: `crates/eggfetch-python/tests/compat/`;
- canonical routine validation: `./scripts/check.sh`;
- existing API manifest generator and comparator.

Do not change the reference version in this phase.

## Scope constraints

This phase may:

- add focused differential tests to the existing compatibility suite;
- add small reusable reference/candidate comparison helpers;
- correct active and resolved difference records;
- update user-facing documentation and compatibility diagnostics;
- run existing retained downstream fixtures that directly exercise corrected behavior;
- add a concise status/closure document for this roadmap.

This phase must not:

- add CI jobs or matrices;
- add a new qualification workflow;
- add an evidence schema or artifact manifest format;
- expand the downstream package portfolio;
- add long soak or resource gates unrelated to the corrected behavior;
- add private HTTPX module compatibility;
- promote to another HTTPX version;
- publish a package named `httpx` publicly;
- convert required failures into skips, xfails, warnings, or allowed broad differences.

# Track 1 — Build a compact parity case registry

## 1.1 Define stable case identifiers

Create one small machine-readable or Python-native registry for the findings in this roadmap. It may be a TOML file, a Python constant, or parametrized test data inside the existing suite. Do not create a new report ecosystem.

Each case should include:

- stable case ID;
- short behavior description;
- sync applicability;
- async applicability;
- reference test function or fixture;
- candidate test function or fixture;
- required result: exact parity or intentional difference;
- linked roadmap phase;
- linked active difference ID if applicable.

Suggested categories and case IDs:

- `ENTRYPOINT-*`;
- `CLIENT-*`;
- `AUTH-*`;
- `REQUEST-*`;
- `RESPONSE-*`;
- `STREAM-*`;
- `TRANSPORT-*`;
- `MOUNT-*`;
- `HOOK-*`;
- `REDIRECT-*`;
- `COOKIE-*`.

Keep the registry limited to this corrective roadmap. Do not inventory all HTTPX behavior again.

## 1.2 Require reference observation for subtle behavior

For ordering, exceptions, stream state, mount priority, cookies, encoding, and redirects, execute the same fixture against installed HTTPX 0.28.1 and the candidate facade.

Avoid copying expected values from implementation comments when direct reference observation is possible.

## 1.3 Make required cases fail closed

A required case fails when:

- the reference package is missing;
- the candidate wheel/module is missing;
- no tests collect;
- either side skips or xfails;
- the reference fixture errors unexpectedly;
- candidate output differs;
- cleanup assertions fail.

The existing `EGGFETCH_COMPAT_REQUIRED=1` mechanism should enforce this. Extend it rather than adding a parallel runner.

### Track 1 acceptance criteria

- [ ] Every roadmap finding has at least one stable focused case ID.
- [ ] Subtle semantics use direct HTTPX 0.28.1 reference observation.
- [ ] Required cases cannot pass through skip, xfail, no-collection, or missing reference.
- [ ] The registry does not duplicate the entire compatibility profile.
- [ ] Each remaining intentional difference links to one exact active difference record.

# Track 2 — Add mandatory regression coverage by phase

## 2.1 Phase 1 cases

Required coverage includes:

- top-level `stream()` yields an open response;
- top-level proxy, verify, timeout, and trust-env argument routing;
- method-specific top-level signatures;
- stream-level auth, redirect, and timeout overrides;
- tuple, callable, URL, explicit-none, and object auth;
- closed-client permanence;
- property setters and default headers;
- protocol boolean validation;
- unsupported UDS/local-address/socket-option failure.

## 2.2 Phase 2 cases

Required coverage includes:

- direct Request params in URL with duplicates;
- `data` plus `files` multipart;
- compact JSON encoding;
- explicit stream auto-header behavior;
- empty POST/PUT/PATCH content length;
- RequestNotRead and ResponseNotRead;
- HTTP/2 metadata through the compatibility Response;
- reason phrase extension;
- elapsed state;
- `raise_for_status()` for 1xx, 3xx, 4xx, and 5xx;
- `next_request` public state;
- callable default encoding and encoding setter restriction;
- raw/decoded/text/line streaming boundaries;
- buffered custom response body preservation.

## 2.3 Phase 3 cases

Required coverage includes:

- native one-hop redirect-disabled dispatch;
- sync and async custom transport stream typing;
- mounted buffered and streaming body preservation;
- HTTPTransport and AsyncHTTPTransport streaming;
- mount wildcard and priority behavior;
- explicit `None` mount bypass;
- request/response hook per-hop order;
- hook exception cleanup;
- request versus response extension separation;
- duplicate mounted transport close-once behavior.

## 2.4 Phase 4 cases

Required coverage includes:

- redirect method/body rewriting for 301/302/303/307/308;
- cross-origin auth stripping;
- manual `next_request`;
- max redirect boundary;
- replayable versus one-shot body redirects;
- Basic, Digest, NetRC, callable, custom sync, and custom async auth;
- auth through native, mounted, custom, Mock, WSGI, and ASGI transports;
- domain/path/secure/expiry cookie selection;
- duplicate-name CookieConflict;
- multiple Set-Cookie processing;
- cookie-before-redirect and cookie-before-auth-follow-up behavior;
- hook/auth/cookie/redirect event ordering;
- intermediate response cleanup and cancellation.

## 2.5 Prove cleanup, not only return values

For cases involving streaming or multiple hops, assert where feasible:

- response closed state;
- stream consumed state;
- custom transport close count;
- no outstanding iterator producer task;
- native pool permit released or subsequent constrained request succeeds;
- history response content remains readable after resources close.

Use bounded deterministic assertions. Do not add a long-running resource monitor.

### Track 2 acceptance criteria

- [ ] Every listed phase behavior has sync/async coverage where applicable.
- [ ] Previously silent argument loss has explicit negative regression tests.
- [ ] Previously lost custom transport content has explicit regression tests.
- [ ] Cleanup is asserted for streaming, redirects, auth challenges, and hook errors.
- [ ] Required cases pass without broad retries or timing sleeps.

# Track 3 — Reconcile API oracle and difference governance

## 3.1 Regenerate candidate manifest

Generate the public API manifest for the actual compatibility facade, not root `eggfetch`.

Compare against the committed HTTPX 0.28.1 reference manifest using the existing typed comparator.

Do not normalize away:

- signature differences;
- keyword-only distinctions;
- properties versus methods;
- sync versus async callables;
- base class differences that affect public protocols;
- missing public exports.

## 3.2 Resolve corrected difference records

For every implemented parity fix:

- remove the active allowed difference;
- add or update a historical record in `resolved-differences.toml` if the repository convention requires it;
- link the regression test;
- record the implementation milestone or roadmap phase.

A resolved record must not remain active.

## 3.3 Correct inaccurate active records

Audit active records touching:

- Cookies mapping and scope;
- Headers and QueryParams protocol methods;
- auth flow methods;
- Response properties;
- transport constructor parameters;
- top-level functions;
- Request/Response stream errors;
- missing exports.

Reject rationales claiming “behavior is equivalent” when the public behavior differs materially.

## 3.4 Keep genuine bounded differences narrow

Examples likely to remain intentional or unsupported after this roadmap:

- Trio/AnyIO backend support;
- SOCKS;
- UDS;
- local address;
- socket options;
- Python 3.8/3.9;
- private modules.

Each record must identify exact affected symbols/parameters and migration impact. Do not use one broad waiver for a whole class or module.

### Track 3 acceptance criteria

- [ ] Candidate manifest targets `eggfetch.compat.httpx`.
- [ ] Zero unexplained required-now differences remain.
- [ ] Corrected active records move to the resolved ledger.
- [ ] No resolved record remains active.
- [ ] Material behavior differences are not mislabeled as equivalent inheritance details.
- [ ] Remaining unsupported advanced features have exact narrow records.

# Track 4 — Run only relevant retained downstream checks

## 4.1 Select existing consumers by corrected behavior

Use only already retained downstream fixtures that exercise this roadmap, such as categories involving:

- MockTransport/custom transport;
- ASGI or WSGI TestClient;
- streaming/SSE;
- auth;
- redirects;
- cookies;
- event hooks.

Do not add packages merely to increase the number of downstream checks.

## 4.2 Require behavioral assertions

An import-only downstream result is informational. For closure, a selected downstream fixture must execute behavior affected by this roadmap.

Examples:

- a mocked request returns and streams its body;
- a TestClient follows or exposes redirects;
- SSE consumes streaming lines;
- an SDK applies auth and request hooks;
- cookies persist across a redirect.

## 4.3 Do not block on private-module consumers

If a retained package imports `httpx._content`, `httpx._models`, or another private module, keep that failure classified outside the public Stage C claim unless the package was already explicitly approved for narrow private compatibility.

Do not expand this roadmap to emulate the HTTPX internal module tree.

### Track 4 acceptance criteria

- [ ] Selected retained downstream fixtures directly exercise corrected public behavior.
- [ ] Import-only checks are not described as behavioral proof.
- [ ] No new downstream package is added.
- [ ] Private-module failures do not cause undocumented scope expansion.
- [ ] Any remaining downstream failure is mapped to a precise public gap or excluded private dependency.

# Track 5 — Reconcile documentation and runtime diagnostics

## 5.1 Correct README and compatibility matrix

Update claims to match the final implementation:

- describe `eggfetch.compat.httpx` as Stage C candidate or achieved supported drop-in profile based on actual closure;
- remove claims that cookies, transports, mounts, hooks, or auth are fully equivalent unless their focused cases pass;
- distinguish implemented advanced features from constructor-signature acceptance;
- retain explicit asyncio-only, no-SOCKS, no-UDS, and supported-Python limitations.

## 5.2 Correct Python guide examples

Ensure examples are executable and use the correct APIs for:

- top-level stream;
- sync and async client stream;
- per-request timeout/auth/redirect override;
- manual redirects through `next_request`;
- scoped cookies;
- custom transports and mounts;
- unsupported-option errors.

Do not show APIs the facade does not implement.

## 5.3 Correct diagnostics and stage status

Runtime diagnostics must report:

- emulated HTTPX version: 0.28.1;
- eggfetch implementation version;
- asyncio-only backend limitation;
- current stage/status;
- exact high-level unsupported surfaces.

The stage status must remain `Stage C candidate` unless all required roadmap cases and existing required qualification checks pass on the same exact SHA.

## 5.4 Add a concise roadmap status file

Create:

`plans/httpx-parity-correction-status.md`

It should contain:

- implementation SHA;
- phase-by-phase completion table;
- focused test command and result counts;
- API oracle result;
- relevant downstream results;
- remaining exact blockers;
- final claim decision.

Do not embed transient generated evidence or large logs in the repository.

### Track 5 acceptance criteria

- [ ] README claims match focused and existing qualification evidence.
- [ ] Compatibility matrix no longer equates accepted parameters with implemented behavior.
- [ ] Python guide examples execute against the facade.
- [ ] Runtime diagnostics identify version, stage, backend, and unsupported surfaces.
- [ ] Status file is concise, exact-SHA-bound, and free of placeholders.
- [ ] No release claim is promoted from documentation alone.

# Track 6 — Execute bounded closure validation

## 6.1 Focused compatibility suite

Run all newly added and directly affected existing compatibility tests under required mode:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Required result:

- zero failures;
- zero collection errors;
- zero required skips;
- zero required xfails.

## 6.2 API oracle

Generate and compare the facade manifest using the repository’s canonical commands. Required result:

- zero unexplained required-now differences;
- zero stale active records;
- zero duplicate difference IDs/tuples;
- zero resolved entries in the active file.

## 6.3 Routine validation

Run:

```sh
./scripts/check.sh
```

Do not require `extended` or `package` mode unless implementation changes packaging or native features outside the normal compatibility path.

## 6.4 Existing relevant downstream fixtures

Run the existing commands for selected retained behavioral fixtures. Record exact package/version, command, and result in the status file.

## 6.5 Exact implementation SHA

All closure commands must run after the final implementation commit, with a clean working tree. If fixes are committed afterward, rerun the affected closure commands.

### Track 6 acceptance criteria

- [ ] Focused/full existing compatibility suite passes in required mode.
- [ ] API oracle passes with exact difference governance.
- [ ] Routine validation passes.
- [ ] Relevant retained downstream behavioral fixtures pass or have precise blockers.
- [ ] Status references the exact final implementation SHA.
- [ ] No new CI or evidence architecture was required.

# Global closure criteria

This roadmap can be marked complete only when:

- all phase completion criteria are satisfied;
- every roadmap finding is linked to a passing focused case or exact intentional difference;
- top-level streaming and argument routing are correct;
- per-request stream overrides are preserved;
- auth inputs and lifecycle are correct;
- Request and Response object semantics match the pinned reference for the scoped public surface;
- response protocol metadata and elapsed timing are real;
- custom/mounted transport bodies are preserved;
- mount matching and explicit `None` behavior are correct;
- hooks run once per actual hop in reference order;
- redirects, auth flows, cookies, history, and cleanup are aligned;
- one authoritative scoped cookie jar is used;
- required differential cases pass without skips/xfails;
- API oracle and difference ledgers are coherent;
- relevant retained downstream behaviors pass;
- user-facing claims match evidence;
- `./scripts/check.sh` passes on the final SHA;
- no new CI job, matrix, evidence schema, or release automation was introduced.

## Rejection conditions

Do not close this roadmap if any of the following remains:

- top-level `stream()` returns a closed Response;
- valid top-level proxy/TLS/environment arguments raise unexpected keyword errors;
- streaming silently ignores auth, redirect, or timeout overrides;
- a raw auth tuple reaches an auth-flow method;
- closed clients can reopen;
- unsupported transport options silently no-op;
- `data` plus `files` is rejected;
- unread stream state raises generic errors instead of compatibility exceptions;
- HTTP/2 responses report HTTP/1.1;
- manual redirects lack `next_request`;
- custom transport streaming loses body bytes;
- hooks run only once for a multi-hop request;
- scoped cookies are flattened by name;
- active difference records claim equivalence for material behavior gaps;
- documentation says “drop-in” without the supported-surface qualification;
- required tests pass only by skip, xfail, warning, retry suppression, or missing reference package.

## Suggested implementation commit decomposition

Keep commits reviewable and scoped:

1. top-level helpers and stream override routing;
2. auth normalization and lifecycle/property semantics;
3. Request construction and body encoding;
4. Response metadata, status, encoding, and stream state;
5. one-hop native/custom transport boundary;
6. mount and hook ordering;
7. redirect and auth state machine;
8. scoped cookie jar integration;
9. focused differential tests and difference-ledger cleanup;
10. documentation and final status.

Do not combine all behavior changes and all tests into one opaque commit.

## Handoff decision rule

The implementation agent should report exactly one of:

- **Complete:** all global closure criteria pass on the stated SHA;
- **Blocked:** one or more rejection conditions remain, with exact files, tests, and architectural blocker described;
- **Partially implemented:** some phases landed, but Stage C candidate status remains and no closure claim is made.

“Mostly complete” without mapped remaining cases is not an acceptable handoff state.