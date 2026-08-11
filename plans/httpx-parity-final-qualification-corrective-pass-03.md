# HTTPX Parity Final Qualification — Corrective Pass 03

## Purpose

This is the next and intended final closure pass for the HTTPX 0.28.1 compatibility line.

It follows `plans/httpx-parity-final-qualification-corrective-pass-02.md` and is written against the implementation currently at:

- repository baseline: `915caa0593d2a6d71a0f2633647a4bd5ce2ab28b`;
- current profile: `stage-c-candidate`;
- current status: `final-corrective-qualification-pending`;
- current qualification SHA: blank.

The previous pass materially improved the implementation:

- HTTP and HTTPS proxy endpoint support exists;
- HTTPS proxy TLS and origin TLS are separate layers;
- multi-phase proxy setup can now consume an optional native request deadline monotonically;
- ordinary three-element `socket_options` now accept `bytearray`;
- non-empty `Proxy(headers=...)` is explicitly rejected instead of silently discarded;
- the proxy-header limitation is represented as an active Stage C difference;
- shared compatibility fixtures now perform more complete server shutdown and deterministic response framing.

However, Stage C qualification must remain open because two categories of work remain:

1. the HTTPX compatibility facade currently synthesizes EggFetch's native `total` timeout from HTTPX scalar/default timeout values, introducing a behavioral deadline that HTTPX 0.28.1 does not have;
2. the final differential and aggregate qualification evidence required by the previous plans is still incomplete.

This pass must close those items without reopening the transport architecture.

---

# Scope firewall

## In scope

Only the following work is permitted:

- HTTPX 0.28.1 timeout-reference pinning;
- compatibility timeout conversion in `eggfetch.compat.httpx`;
- phase-aware timeout ownership in the existing HTTP/HTTPS proxy path;
- preservation of EggFetch's native `Timeout.total` as a native-only outer deadline;
- HTTPX-vs-EggFetch differential proxy endpoint tests;
- HTTPX-vs-EggFetch differential `NO_PROXY` route-selection tests;
- direct reference evidence for the existing bounded `Proxy(headers=...)` difference;
- deterministic cleanup of any remaining aggregate-suite ordering/global-state issue;
- exact-SHA validation, API oracle, downstream portfolio, profile, and status evidence.

## Explicitly out of scope

Do not reopen or redesign:

- direct TCP transport;
- UDS transport;
- SOCKS transport or pooling;
- connection-pool architecture;
- TLS stack outside the existing proxy phase wiring;
- HTTP/2 architecture;
- HTTP/3;
- Trio or AnyIO support;
- private HTTPX modules;
- support for HTTPX versions other than `0.28.1`;
- Python 3.8/3.9 support;
- arbitrary Python `ssl.SSLContext` emulation;
- the safe-Rust four-element socket-option limitation;
- CI or release workflow expansion;
- dependency churn;
- unrelated cleanup/refactoring;
- a generalized proxy middleware/header interception system.

A successful implementation should be small relative to the earlier transport phases.

---

# Reference baseline

The compatibility contract is pinned to:

- `httpx==0.28.1`;
- `httpcore==1.0.9`;
- `socksio==1.0.0` where SOCKS fixtures are already used.

The implementation must derive behavioral expectations from executable reference tests and the pinned source, not from current EggFetch behavior.

Important pinned timeout facts to preserve:

1. HTTPX `Timeout` has four operational timeout dimensions:
   - `connect`;
   - `read`;
   - `write`;
   - `pool`.
2. `Timeout(5.0)` means 5 seconds for each of those operations.
3. HTTPX 0.28.1 does not expose an end-to-end `total` timeout dimension.
4. httpcore's tunnel path reads the request's `connect` timeout for TLS upgrade after CONNECT.
5. The CONNECT request itself carries the ordinary request timeout extensions, so its write/read work remains governed by the normal write/read timeout classes.
6. TLS-to-an-HTTPS-proxy is part of connection establishment and must therefore respect connect timeout semantics.

EggFetch may retain `Timeout.total` in the **native** API. The defect is allowing the HTTPX facade to synthesize and forward it as if it were part of HTTPX semantics.

---

# Track 0 — Preserve pending qualification and freeze the good architecture

Before changing executable behavior:

1. confirm `compat/httpx/0.28.1/profile.toml` remains:
   - `stage = "stage-c-candidate"`;
   - `status = "final-corrective-qualification-pending"`;
   - `qualified = ""`;
   - `qualification-sha = ""`;
2. record `915caa0593d2a6d71a0f2633647a4bd5ce2ab28b` as the starting implementation SHA in the final handoff;
3. do not alter the current direct/UDS/SOCKS architecture;
4. do not reuse old passing counts as current qualification evidence.

Acceptance:

- qualification remains pending until Track 7 completes;
- previous SHA results are clearly historical;
- no implementation commit is labeled qualified merely because focused tests pass.

---

# Track 1 — Re-pin HTTPX timeout semantics before changing code

Create a compact reference fixture dedicated to HTTPX 0.28.1 timeout behavior.

The fixture should execute the reference package directly and record/assert the behaviors that determine this pass.

## 1.1 Timeout object shape

Reference-pin at minimum:

- `Timeout(5.0).connect == 5.0`;
- `Timeout(5.0).read == 5.0`;
- `Timeout(5.0).write == 5.0`;
- `Timeout(5.0).pool == 5.0`;
- no public HTTPX `total` timeout dimension is used for request dispatch;
- explicit overrides such as `Timeout(5.0, connect=1.0)` preserve the other three default values;
- `Timeout(None, connect=...)` semantics match the reference;
- `timeout=None` disables the four operational timeouts as HTTPX defines them.

If EggFetch intentionally exposes a compatibility-only `.total` attribute today, do not use it as a runtime behavior source. Either remove it if doing so is already covered by the public API contract, or leave it inert and explicitly non-dispatching. Do not expand scope into a value-object rewrite solely for this attribute.

## 1.2 Behavioral phase pinning

Use local deterministic servers/proxies to prove reference behavior for:

- proxy TCP connection timeout;
- TLS handshake to an HTTPS proxy;
- CONNECT request write;
- CONNECT response read;
- origin TLS handshake after CONNECT;
- request body write after the tunnel exists;
- response-header/body read after the tunnel exists.

The goal is not to reproduce all of httpcore's internal exception implementation. The goal is to know which public timeout dimension governs each externally observable phase.

## 1.3 No synthetic total envelope

Add a reference test where multiple sequential phases each finish within their own configured phase timeout but their combined wall-clock time exceeds the scalar/default timeout value.

Example shape, with broad deterministic margins:

```text
Timeout(0.40)
proxy/TLS phase A ~0.25s
later independent phase B ~0.25s
each individual phase < 0.40s
combined wall clock > 0.40s
reference request succeeds
```

Choose fixture phases that are deterministic on the local host. Do not depend on external DNS or the public Internet.

This test is the key proof that HTTPX scalar timeout is not an end-to-end request deadline.

## Track 1 acceptance criteria

- reference behavior is captured before candidate behavior is modified;
- the test demonstrates no HTTPX synthetic total deadline;
- timeout phase ownership is recorded in test names/comments or a small table;
- no candidate-only assumption is used as reference evidence.

---

# Track 2 — Stop the HTTPX facade from synthesizing native `total`

## Current defect

The compatibility `Timeout` object currently retains the scalar/default timeout as `_total`, and `_convert_timeout()` forwards that value to `eggfetch.Timeout(total=...)`.

The numeric shortcut path similarly constructs native timeout values with `total=timeout`.

This changes HTTPX semantics because a request can fail after the scalar amount of aggregate wall-clock time even when no individual HTTPX timeout phase exceeded its configured limit.

## Required behavior

For requests entering through `eggfetch.compat.httpx`:

- HTTPX `connect` maps to native connect timeout;
- HTTPX `read` maps to native read timeout;
- HTTPX `write` maps to native write timeout;
- HTTPX `pool` maps to native pool timeout;
- the compatibility facade must **not** synthesize native `total` from the HTTPX default/scalar value;
- ordinary compatibility calls must pass native `total=None` unless the project already has a separately documented non-HTTPX extension path that the caller explicitly selected.

For example:

```text
compat Timeout(5.0)
  -> native pool=5
  -> native connect=5
  -> native write=5
  -> native read=5
  -> native total=None
```

Likewise:

```text
compat timeout=5.0
  -> native pool=5
  -> native connect=5
  -> native write=5
  -> native read=5
  -> native total=None
```

## Native API preservation

Do **not** remove `eggfetch.Timeout.total` from the native Python or Rust API.

Native users who explicitly configure:

```text
Timeout { total: Some(...) }
```

must continue to receive a monotonic end-to-end outer budget.

The compatibility facade simply must not invent this native extension on behalf of HTTPX callers.

## Tests

Add direct conversion tests for:

- `Timeout(5.0)`;
- numeric `timeout=5.0`;
- explicit `Timeout(None, connect=..., read=..., write=..., pool=...)`;
- `timeout=None`;
- per-request timeout overrides;
- sync client;
- async client.

Where possible, inspect native timeout dispatch through a deterministic test seam rather than private implementation state.

## Track 2 acceptance criteria

- HTTPX facade no longer sends synthetic total timeout;
- native total timeout remains supported;
- existing public timeout signatures stay compatible;
- sync and async conversions are identical;
- the no-synthetic-total reference case passes against both HTTPX and EggFetch.

Reject if:

- native `total` is removed project-wide;
- scalar compatibility timeout is simply disabled;
- a second compatibility-specific timeout engine is added;
- tests pass by increasing timeout values until the defect is hidden.

---

# Track 3 — Make the existing proxy path phase-aware

The monotonic native deadline added in the previous pass is useful and should remain for native callers that explicitly set `Timeout.total`.

The proxy path now needs to combine that optional outer deadline with the ordinary phase timeout values.

## Required context

Extend the existing bounded `ProxyRequestContext` or equivalent call chain with the minimum phase information needed:

- `connect` timeout;
- `write` timeout;
- `read` timeout;
- optional native total deadline;
- existing TLS config;
- existing SOCKS client field where already required.

Do not create a generic timeout framework.

## Effective timeout rule

For every blocking phase:

```text
phase_budget = configured phase timeout
outer_budget = remaining native total deadline, if any

effective budget =
  min(phase_budget, outer_budget) if both exist
  phase_budget if only phase exists
  outer_budget if only total exists
  none if neither exists
```

If the native total deadline is exhausted before a phase starts, fail immediately with the existing total/appropriate timeout classification used by the native API.

## Phase ownership for HTTP/HTTPS proxies

Match pinned HTTPX/httpcore behavior as closely as the existing EggFetch error taxonomy permits.

### Proxy connection establishment

Use `connect` timeout for:

- DNS/address resolution owned by the connector where applicable;
- TCP connection to the proxy;
- TLS handshake to an `https://` proxy endpoint.

### CONNECT tunnel setup

For an HTTPS origin through an HTTP or HTTPS proxy:

- CONNECT request write -> `write` timeout;
- CONNECT response/header read -> `read` timeout;
- origin TLS handshake after successful CONNECT -> `connect` timeout.

### Forward proxy requests

For an HTTP origin through an HTTP or HTTPS proxy:

- proxy TCP/TLS setup -> `connect`;
- request write -> `write`;
- response read -> `read`.

### Origin request after CONNECT

After the origin TLS tunnel exists:

- request/body write -> `write`;
- response/header/body read -> `read`.

Preserve the existing response-stream read-timeout mechanism where it already owns incremental body reads. Do not double-wrap a stream with competing read timeout layers.

## Native total behavior

When a native EggFetch caller explicitly set `total`, retain the current monotonic deadline behavior as an outer cap across all phases.

The previous deadline helper may remain, but it must no longer be the only timeout source for proxy connection/setup.

## Error mapping

Preserve the most specific existing native error classification possible:

- connect-phase timeout -> connect-compatible error;
- write-phase timeout -> write-compatible error;
- read-phase timeout -> read-compatible error;
- native total exhaustion -> native total timeout classification.

The HTTPX compatibility exception mapper should then continue producing the corresponding HTTPX exception family.

Do not label origin TLS connect timeout as proxy TLS merely because a proxy is present if the reference behavior and current taxonomy allow a more accurate connect classification.

## Track 3 tests

Create deterministic delayed fixtures proving:

- proxy TCP obeys connect timeout;
- HTTPS-proxy TLS obeys connect timeout;
- CONNECT write obeys write timeout;
- CONNECT response obeys read timeout;
- origin TLS after CONNECT obeys connect timeout;
- tunneled request write obeys write timeout;
- tunneled response read obeys read timeout;
- forward-proxy request write/read obey write/read;
- native total, when explicitly set, remains an outer cap;
- compatibility scalar timeout is not treated as an outer cap.

## Track 3 acceptance criteria

- proxy phases use the same four HTTPX timeout dimensions as the reference;
- optional native total remains monotonic and outermost;
- compatibility callers no longer accidentally enable native total;
- existing direct/UDS/SOCKS timeout behavior is not modified unless a shared helper requires a mechanical signature adjustment;
- no duplicate response-body read timeout wrapper is introduced.

---

# Track 4 — Complete the HTTP/HTTPS proxy differential matrix

The existing candidate-side HTTPS proxy tests must be supplemented with actual reference/candidate differential proof.

Use one local deterministic fixture capable of observing:

- proxy endpoint TLS or plaintext;
- received proxy method;
- received request target;
- CONNECT target;
- proxy headers;
- origin request target;
- whether proxy-only metadata leaked through the tunnel.

Run the same matrix against:

- `httpx==0.28.1`;
- `eggfetch.compat.httpx`.

Required cases:

| Origin | Proxy endpoint | Expected route |
| --- | --- | --- |
| HTTP | `http://` | plaintext proxy + absolute-form |
| HTTPS | `http://` | plaintext proxy + CONNECT + origin TLS |
| HTTP | `https://` | TLS to proxy + absolute-form |
| HTTPS | `https://` | TLS to proxy + CONNECT + origin TLS |

For each applicable case assert:

- response success;
- proxy method;
- proxy target form;
- CONNECT authority;
- origin-form after CONNECT;
- proxy endpoint certificate verification;
- origin certificate verification independently;
- inline proxy credentials where already supported;
- route isolation between `http://proxy` and `https://proxy`.

Also include the relevant timeout cases from Tracks 1–3 in the same fixture where this avoids duplicate infrastructure.

## Track 4 acceptance criteria

- all four combinations execute against reference and candidate;
- HTTPX and EggFetch observations are compared explicitly;
- no external network is used;
- candidate URL acceptance alone is not counted as transport proof;
- proxy and origin TLS identities are independently validated.

---

# Track 5 — Complete `NO_PROXY` reference/candidate route-selection proof

The corrected parser behavior must now be backed by direct routing evidence.

Do not merely assert parser internals.

Use a local origin plus a recording proxy and determine whether each request was sent directly or through the proxy.

Run every supported case against HTTPX 0.28.1 and EggFetch compatibility.

## Required matrix

### Wildcard and list parsing

- `*`;
- wildcard mixed with other entries;
- comma-separated entries;
- surrounding whitespace;
- empty entries.

### Domain behavior

- bare domain vs itself;
- bare domain vs subdomain;
- leading-dot domain vs bare domain;
- leading-dot domain vs subdomain;
- near-match that must not bypass.

### Port behavior

- host + matching explicit port;
- same host + different port;
- HTTP default port behavior;
- HTTPS default port behavior.

### Localhost/IP

- `localhost`;
- `127.0.0.1`;
- bare IPv6 `::1` where available;
- bracketed IPv6 if accepted by the reference runtime;
- another IPv4 literal;
- another IPv6 literal if deterministic.

### Scheme-qualified entries

- `http://host` against HTTP;
- `http://host` against HTTPS;
- `https://host` against HTTPS;
- `https://host` against HTTP;
- scheme-qualified host + port.

### CIDR-looking entries

Pin actual HTTPX behavior rather than assuming CIDR semantics from the string shape:

- `10.0.0.0/8` against exact textual host/address where meaningful;
- an address that lies inside the apparent subnet but is not the same host pattern;
- one IPv6 prefix-looking value if deterministic.

The native Rust `NoProxy::parse()` may keep richer genuine CIDR semantics outside the HTTPX facade.

### Environment precedence

- lowercase proxy variables vs uppercase collision;
- lowercase `no_proxy` vs uppercase `NO_PROXY` collision;
- scheme-less proxy URL normalization;
- `trust_env=False`;
- direct routing when no applicable environment proxy exists.

## Track 5 acceptance criteria

- every row asserts actual route selection;
- every row runs against reference and candidate where the platform supports it;
- native Rust CIDR behavior is not weakened;
- compatibility behavior matches the observed HTTPX 0.28.1 URL-pattern semantics;
- unsupported platform-specific IPv6 cases are documented with a specific reason, not silently skipped.

---

# Track 6 — Finish `Proxy(headers=...)` evidence as a bounded Stage C difference

The current implementation disposition is acceptable for this stage:

- HTTPX accepts `Proxy(headers=...)` and applies the headers to the proxy leg;
- EggFetch explicitly raises `NotImplementedError` before native dispatch;
- the limitation is recorded as `PROXY-HEADERS-001`;
- silent metadata loss is no longer possible.

Do **not** build a generalized proxy-header channel in this pass unless it is truly a trivial bounded extension of existing proxy request writing.

The remaining task is evidence.

## Reference fixture

Prove HTTPX 0.28.1 behavior using a local recording proxy:

- custom proxy header appears on forward HTTP proxy request;
- custom proxy header appears on CONNECT request where reference does so;
- custom proxy header does not leak to the origin after CONNECT;
- interaction with proxy authentication is recorded;
- behavior is tested for an HTTPS proxy endpoint as well where practical.

## Candidate fixture

Prove EggFetch:

- accepts/stores the `Proxy` value object as currently designed;
- rejects non-empty `Proxy(headers=...)` before native dispatch;
- does not silently issue a request without the header;
- raises the documented exception consistently for sync and async clients.

## Allowlist hygiene

Verify `PROXY-HEADERS-001`:

- matches the actual observed reference and candidate behavior;
- is active, not stale;
- has accurate migration impact;
- does not claim proxy headers are implemented;
- is the only difference required for this behavior.

## Track 6 acceptance criteria

- reference behavior is executable evidence, not source-only inference;
- candidate behavior is executable evidence;
- API oracle recognizes the bounded difference cleanly;
- no silent drop remains;
- no broad proxy redesign occurs.

---

# Track 7 — Aggregate stability and exact-SHA qualification

Qualification is the final step, not a documentation assumption.

## 7.1 First establish one frozen executable SHA

After all executable/test changes are complete:

1. commit them;
2. record that commit as the candidate executable SHA;
3. do not modify executable files during qualification;
4. documentation-only evidence commits may follow, but they must identify the executable SHA explicitly.

If any executable fix is required after qualification starts, restart qualification on the new executable SHA.

## 7.2 Routine project gate

Run the repository's existing validation path only:

```sh
./scripts/check.sh
```

Also ensure the equivalent explicit gates remain clean where the script does not expose enough detail:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Do not add CI jobs.

## 7.3 Focused corrective suite

Run all timeout/proxy/environment/config fixtures changed by this pass together in one process.

At minimum include:

- timeout reference/candidate tests;
- HTTP/HTTPS proxy differential tests;
- HTTPS proxy TLS tests;
- environment/`NO_PROXY` differential tests;
- proxy-header bounded-difference tests;
- stream/resource tests implicated in the prior aggregate failures.

Result must be:

- 0 failures;
- 0 unexpected skips;
- 0 xfails used to mask this pass.

## 7.4 Full pinned compatibility — three consecutive clean runs

On the same executable SHA run:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Run it **three consecutive times** without changing code between runs.

All three runs must have:

- 0 failed;
- 0 unexpected skipped;
- 0 xfailed corrective cases;
- no retry plugin masking failures;
- no isolated rerun substituted for aggregate success.

Record all three counts and durations.

The prior aggregate failures:

- `test_read_after_close_returns_data`;
- `test_read_phase_timeout`;
- `test_real_proxy_server_forward`;

must remain green in the aggregate runs.

If any one of the three runs fails, qualification remains open until the root cause is fixed and a fresh three-run sequence passes.

## 7.5 API oracle

Regenerate and compare the API manifest using the existing scripts.

Required result:

- 0 unexplained differences;
- 0 stale allowed differences;
- 0 resolved-in-active entries;
- 0 requires-resolution entries;
- `PROXY-HEADERS-001` present only if still intentionally bounded;
- four-element socket-option difference remains accurately classified;
- no timeout difference is introduced merely to excuse the synthetic-total defect.

## 7.6 Downstream portfolio

Run the repository's existing downstream/shim qualification runner exactly as currently documented.

Required packages remain those already designated as required by the profile/runner.

Record:

- package name;
- exact command/runner;
- pass/fail;
- any informational private-module limitation separately from required public-surface results.

Do not broaden the downstream portfolio in this pass.

## 7.7 Optional routine CI evidence

If the existing CI naturally runs on the final executable or a documentation-only descendant, record it.

Do not:

- add a new workflow;
- add platforms;
- make CI responsible for the full compatibility matrix;
- block closure merely because no GitHub-attached status exists if all required local qualification evidence is complete.

---

# Track 8 — Qualification/profile/status cleanup

Only after Track 7 is entirely green:

## Profile

Update `compat/httpx/0.28.1/profile.toml` to:

- `stage = "stage-c-qualified"` or the repository's existing exact qualified stage spelling;
- `status = "qualified"`;
- `qualified = "2026-08-11"` or the actual qualification date if implementation occurs later;
- `qualification-sha = "<exact executable SHA>"`.

Do not point `qualification-sha` at a documentation-only descendant.

## Status record

Rewrite the top/current section of `plans/httpx-parity-correction-status.md` so it contains one unambiguous current evidence block:

- exact executable SHA;
- reference package versions;
- `./scripts/check.sh` result;
- focused differential result;
- three full pinned run results;
- API oracle result;
- downstream result;
- any routine CI evidence with its exact checked-out SHA;
- active intentional differences.

Move or retain old counts only under clearly labeled historical/superseded sections.

Do not leave phrases such as "being re-qualified" after qualification is complete.

## Documentation

Update only documents that currently describe the affected behavior:

- compatibility timeout semantics;
- native-only `total` timeout distinction;
- proxy header bounded difference;
- HTTPS proxy endpoint support;
- current Stage C status.

Do not rewrite unrelated architecture documentation.

---

# Required acceptance matrix

The pass is complete only if every row below is satisfied.

| Area | Acceptance criterion |
| --- | --- |
| Baseline | starts from `915caa0593d2a6d71a0f2633647a4bd5ce2ab28b` or explicitly records any newer pre-existing head |
| HTTPX timeout model | reference fixture proves only connect/read/write/pool operational dimensions |
| Scalar timeout | candidate no longer synthesizes native total |
| Numeric timeout shortcut | candidate no longer synthesizes native total |
| Native total | remains supported for explicit native callers |
| Proxy TCP | obeys connect timeout |
| HTTPS proxy TLS | obeys connect timeout |
| CONNECT write | obeys write timeout |
| CONNECT read | obeys read timeout |
| Origin TLS after CONNECT | obeys connect timeout |
| Tunneled write | obeys write timeout |
| Tunneled read | obeys read timeout |
| Total + phase | native explicit total acts only as outer cap; effective budget is bounded by the smaller applicable timeout |
| No synthetic envelope | multi-phase HTTPX scalar-timeout case matches reference and may exceed scalar wall-clock total if each phase is within limit |
| HTTP/HTTP proxy | reference/candidate differential passes |
| HTTPS/HTTP proxy | reference/candidate differential passes |
| HTTP/HTTPS proxy | reference/candidate differential passes |
| HTTPS/HTTPS proxy | reference/candidate differential passes |
| `NO_PROXY` | required wildcard/domain/port/IP/IPv6/scheme/CIDR-looking/environment cases are route-selection differential tests |
| Proxy headers | HTTPX proxy-leg behavior proven; EggFetch explicit bounded rejection proven |
| Proxy header leakage | reference fixture proves proxy-only header does not become origin header after CONNECT |
| Socket options | bytearray remains green; four-element limitation remains truthful |
| Aggregate stability | three consecutive full pinned compatibility runs pass |
| Prior flaky tests | all three prior aggregate failures remain green in each run |
| API oracle | zero unexplained/stale/requires-resolution |
| Downstream | required portfolio passes using existing runner |
| CI | no workflow expansion |
| Qualification SHA | exact executable SHA recorded |
| Profile | qualified only after all gates pass |
| Status record | current evidence separated cleanly from historical evidence |
| Scope | no direct/UDS/SOCKS/general transport redesign |

---

# Rejection criteria

Reject the implementation as incomplete if any of the following occurs:

- the HTTPX facade still forwards scalar/default timeout as native `total`;
- native `Timeout.total` is removed to solve a compatibility-only problem;
- proxy setup continues to ignore connect/read/write phase timeout values;
- an HTTPX scalar timeout is treated as a request-wide wall-clock deadline;
- proxy phase tests assert only EggFetch behavior without a reference pin where differential proof is required;
- `NO_PROXY` tests assert parser internals instead of actual route selection;
- `Proxy(headers=...)` remains only documented without executable HTTPX reference evidence;
- proxy headers are silently dropped;
- the full pinned suite is qualified based on isolated reruns after an aggregate failure;
- fewer than three consecutive clean aggregate runs are recorded;
- retries, xfails, timing inflation, or skips are used to mask nondeterminism;
- qualification points to a docs-only commit instead of the executable SHA;
- CI/release machinery is expanded;
- direct, UDS, SOCKS, or unrelated architecture is refactored without necessity;
- a new generalized timeout framework is introduced instead of extending existing bounded timeout plumbing.

---

# Suggested implementation sequence

Keep the implementation easy to review.

## Commit 1 — timeout reference + compatibility translation

Suggested message:

```text
fix: align HTTPX compatibility timeout semantics
```

Contents:

- reference timeout fixture;
- remove synthetic native total from compatibility conversion;
- conversion tests;
- no proxy architecture changes yet beyond signatures required for the next commit.

## Commit 2 — proxy phase timeout ownership

Suggested message:

```text
fix: apply HTTPX phase timeouts across proxy setup
```

Contents:

- bounded proxy context fields;
- connect/write/read effective-timeout helper using optional native total deadline;
- proxy TCP/TLS/CONNECT/origin-TLS phase wiring;
- deterministic timeout tests.

## Commit 3 — differential evidence and fixture stability

Suggested message:

```text
test: complete HTTPX proxy and environment differential coverage
```

Contents:

- four proxy endpoint/origin combinations against reference and candidate;
- `NO_PROXY` route-selection matrix;
- proxy-header reference/candidate evidence;
- any genuinely necessary fixture cleanup.

If the implementation is naturally smaller, Commits 1–3 may be combined. Do not split code merely to satisfy this suggested decomposition.

## Commit 4 — qualification evidence only

Suggested message:

```text
docs: record final HTTPX 0.28.1 qualification
```

Only after all qualification gates pass.

Contents:

- profile qualification date/SHA;
- status record;
- minimal affected docs;
- no executable behavior changes.

---

# Required handoff evidence

The implementer must leave a concise final handoff containing:

1. starting SHA;
2. final executable SHA;
3. any documentation-only evidence SHA;
4. files changed;
5. HTTPX/httpcore reference versions;
6. exact timeout semantic change;
7. proxy phase timeout mapping table;
8. whether `Proxy(headers=...)` remains bounded or was trivially implemented;
9. focused test command/result;
10. three full pinned compatibility commands/results;
11. API oracle result;
12. downstream runner result;
13. routine CI result if naturally available;
14. final profile designation;
15. active intentional differences remaining after qualification.

If any qualification gate is not green, the handoff must state **Stage C candidate / qualification pending** and must not partially fill the qualification SHA.

---

# Closure condition

This HTTPX parity corrective line is closed when:

- the compatibility facade no longer creates a non-reference total timeout;
- proxy phases honor HTTPX connect/write/read semantics while preserving native explicit total as an outer cap;
- HTTPS proxy routing is differential-proven for all four HTTP/HTTPS origin/proxy combinations;
- `NO_PROXY` edge semantics are differential-proven by route selection;
- the bounded proxy-header difference has executable reference/candidate evidence;
- the aggregate suite is deterministic across three consecutive full pinned runs;
- API oracle and downstream portfolio are green;
- the exact executable SHA is recorded as qualified;
- no broader transport or CI architecture was reopened.

Once these conditions are met, do not create another general HTTPX parity corrective pass for this roadmap. Future findings should be treated as isolated bugs or explicitly scoped compatibility enhancements rather than another re-audit of the already-closed direct/UDS/SOCKS/transport architecture.
