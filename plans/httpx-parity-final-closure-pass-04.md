# HTTPX 0.28.1 Parity — Final Semantic Closure Pass 04

## Purpose

This is a narrow closure pass for the remaining issues discovered after the repository was marked Stage C qualified.

Current repository state at planning time:

- `main`: `c8c768e46f07225d0ffc391d1639cbcd9cfbb785`;
- current qualified executable: `52b187744d062840879f6e7752c87753021e2415`;
- current profile: `stage-c-qualified` / `qualified`;
- pinned reference: `httpx==0.28.1`;
- pinned transport reference: `httpcore==1.0.9`.

The previous pass closed the synthetic native-total defect, added phase-aware HTTP/HTTPS proxy timeouts, added meaningful proxy/`NO_PROXY` differential fixtures, stabilized the aggregate suite, and produced clean exact-SHA qualification evidence.

That qualification must nevertheless be reopened because two public semantic defects remain and several acceptance rows from the previous plan were not actually exercised:

1. the new generic response-header timeout wrapper applies the HTTPX `read` timeout around whole direct/UDS/H3 request futures, which can include connection establishment and request transmission;
2. the compatibility `Timeout` constructor still uses `None` for both omitted and explicitly disabled phase values, so forms such as `Timeout(5.0, pool=None)` do not match HTTPX 0.28.1;
3. the final differential matrices are useful but incomplete relative to the explicit closure criteria.

This pass must correct only those issues and then regenerate qualification evidence on one new executable SHA.

---

# Scope firewall

## In scope

Only the following work is authorized:

- reopen the current Stage C qualification while corrections are in progress;
- correct direct/UDS/H3 timeout phase ownership introduced by `52b1877...`;
- preserve the correctly implemented HTTP/HTTPS proxy phase timeout behavior;
- implement HTTPX-compatible omitted-vs-explicit-`None` timeout constructor semantics;
- correct timeout allowlist entries whose current rationale hides real behavior;
- fill the missing `NO_PROXY` differential rows required by pass 03;
- fill the missing HTTP/HTTPS proxy differential assertions required by pass 03;
- complete bounded `Proxy(headers=...)` reference evidence;
- run the existing project, compatibility, API-oracle, and downstream qualification gates;
- restore Stage C qualification only after one frozen executable SHA satisfies every criterion below.

## Explicitly out of scope

Do not reopen or redesign:

- direct connection-pool architecture;
- UDS architecture;
- SOCKS implementation or pooling;
- HTTP/HTTPS proxy route architecture;
- proxy TLS layering already implemented;
- HTTP/2 architecture;
- HTTP/3 feature design beyond correcting the same timeout-wrapper regression if the feature is enabled;
- native `Timeout.total` as an EggFetch extension;
- arbitrary Python `ssl.SSLContext` compatibility;
- `Proxy(headers=...)` implementation beyond the already-approved bounded difference;
- safe-Rust four-element socket-option limitation;
- CI topology or release machinery;
- supported Python version policy;
- dependency upgrades or dependency slimming;
- unrelated refactoring, formatting churn, or documentation cleanup.

The intended implementation should be materially smaller than pass 03.

---

# Reference contract

Pin behavior to the already selected references:

- `httpx==0.28.1`;
- `httpcore==1.0.9`.

Relevant reference semantics that must drive implementation:

## HTTPX timeout object

HTTPX 0.28.1 distinguishes an omitted timeout argument from an explicitly supplied `None` using an `UNSET` sentinel.

Examples that must match:

```python
httpx.Timeout(5.0)
# connect=5, read=5, write=5, pool=5

httpx.Timeout(5.0, connect=None)
# connect=None, read=5, write=5, pool=5

httpx.Timeout(5.0, pool=None)
# connect=5, read=5, write=5, pool=None

httpx.Timeout(None, connect=1.0)
# connect=1, read=None, write=None, pool=None
```

HTTPX also requires either:

- one scalar/default timeout; or
- all four phase values explicitly.

The candidate may retain additive properties such as `.as_dict` or `.total` only if those properties do not alter HTTPX dispatch semantics.

## HTTPX/httpcore timeout phase ownership

For ordinary direct requests, httpcore uses:

- `connect` for TCP/UDS connection establishment and TLS startup;
- `write` for request headers/body transmission;
- `read` for receiving response headers and response body data;
- `pool` for connection-pool acquisition.

A shorter `read` timeout must not terminate a still-valid longer `connect` phase.

Likewise, a shorter `connect` timeout must not be silently replaced by a broader wrapper derived from `read`.

The correctly implemented proxy rules from pass 03 remain authoritative:

- proxy TCP -> `connect`;
- TLS to `https://` proxy -> `connect`;
- CONNECT write -> `write`;
- CONNECT response -> `read`;
- origin TLS after CONNECT -> `connect`;
- tunneled/forward request write -> `write`;
- response acquisition -> `read`;
- explicitly configured native `total` -> optional monotonic outer cap only.

---

# Track 0 — Reopen qualification before executable changes

The current profile is prematurely qualified relative to the defects above.

The first implementation commit in this pass must change only qualification state/evidence metadata as needed to reflect that correction is active.

Required state while implementation is in progress:

```toml
stage = "stage-c-candidate"
status = "final-semantic-closure-pending"
qualified = ""
qualification-sha = ""
```

The exact status spelling may follow repository conventions, but it must unambiguously mean **not currently qualified**.

Update the top/current block in `plans/httpx-parity-correction-status.md` so that:

- `52b187744d062840879f6e7752c87753021e2415` remains recorded as the superseded qualification executable;
- its three clean aggregate runs and downstream results remain historical evidence;
- the record explicitly states why qualification was reopened;
- old evidence is not deleted or rewritten as though it never occurred.

Acceptance criteria:

- no executable correction is committed while the profile still claims current qualification;
- previous qualification evidence remains auditable;
- the new pass begins from `c8c768e46f07225d0ffc391d1639cbcd9cfbb785`.

---

# Track 1 — Correct the direct/UDS/H3 read-timeout phase regression

## Current defect

Pass 03 introduced a helper equivalent to:

```text
send_with_header_timeout(send_future, timeout.read, remaining_total)
```

and applied it around whole direct, UDS, and H3 request futures.

For the direct Hyper path, the wrapped future includes `hyper_client.request(...)`, which may perform:

1. pool acquisition;
2. connection establishment;
3. TLS establishment;
4. request transmission;
5. response-header acquisition.

Wrapping that entire future with the `read` timeout crosses phase boundaries.

Concrete must-close case:

```text
connect = 1.0 s
read = 0.10 s
actual connection establishment = ~0.20 s
response arrives promptly after connection
```

The reference must not raise `ReadTimeout` during the 0.20 s connection because the connection remains within the 1.0 s connect budget.

## Required implementation direction

Remove or narrow the generic wrapper so `read` does not govern connection establishment or request transmission.

Preferred order of solutions:

1. use an existing transport seam that begins only when response headers are being awaited;
2. if the current Hyper abstraction does not expose that boundary cleanly, introduce the smallest internal signal/state needed to arm the read-header timer only after connection/request-write completion;
3. if that would require a broad transport redesign, restore the pre-`52b1877...` direct/UDS/H3 behavior rather than retaining a knowingly incorrect cross-phase `read` wrapper, then keep any remaining bounded limitation explicit and evidenced.

Do **not** solve this by:

- using `max(connect, read)`;
- using `min(connect, read)` around the whole request;
- adding a second request-wide compatibility deadline;
- mapping all whole-request timeouts to `ReadTimeout`;
- weakening the already-correct proxy timeout implementation;
- changing connection pooling.

## Preserve native total

If a native caller explicitly configures `Timeout.total`, it remains an outer cap.

For any phase-aware wait:

```text
effective = min(phase_budget, remaining_native_total)
```

when both are present.

Compatibility callers must continue to send `total=None` unless they deliberately enter a separately documented native API.

## Required differential tests

Add deterministic reference/candidate tests for at minimum:

1. `connect > read`, connection duration between the two -> succeeds if response read is prompt;
2. `connect < read`, connection stalls beyond connect -> `ConnectTimeout` family;
3. connection succeeds, response headers stall beyond read -> `ReadTimeout` family;
4. request body/write stalls beyond write -> `WriteTimeout` family where the existing fixture can deterministically create backpressure;
5. `timeout=None` disables all four operational compatibility timeouts;
6. explicit native `total` still terminates a request even when every configured phase budget is larger.

Cover sync and async compatibility paths where they do not share one proven native implementation path.

UDS/H3 requirements:

- UDS must not acquire a cross-phase read wrapper;
- if H3 is compiled/tested in the normal workspace, apply the same phase correction;
- do not expand qualification solely to create a new H3 environment if H3 is not part of the existing routine profile.

## Track 1 acceptance criteria

- a short `read` timeout cannot expire during a valid longer connect phase;
- response-header stalls still honor `read` where the implementation claims support;
- write stalls honor `write` where currently supported;
- native total remains an outer cap;
- direct/UDS/SOCKS architecture is unchanged;
- proxy timeout tests from pass 03 remain green.

---

# Track 2 — Implement HTTPX `UNSET` vs explicit `None` timeout semantics

## Current defect

The compatibility class currently has constructor defaults similar to:

```python
def __init__(self, timeout=5.0, *, connect=None, read=None, write=None, pool=None):
    if connect is None:
        connect = timeout
```

This makes these two calls indistinguishable:

```python
Timeout(5.0)
Timeout(5.0, connect=None)
```

HTTPX 0.28.1 intentionally distinguishes them.

## Required implementation

Add a private compatibility sentinel, e.g. `_UNSET`, used only for constructor argument presence.

The public signature should match HTTPX's behavior as closely as the project's oracle representation permits.

Required decision table:

| Input | connect | read | write | pool |
| --- | ---: | ---: | ---: | ---: |
| `Timeout(5.0)` | 5 | 5 | 5 | 5 |
| `Timeout(5.0, connect=None)` | None | 5 | 5 | 5 |
| `Timeout(5.0, read=None)` | 5 | None | 5 | 5 |
| `Timeout(5.0, write=None)` | 5 | 5 | None | 5 |
| `Timeout(5.0, pool=None)` | 5 | 5 | 5 | None |
| `Timeout(None)` | None | None | None | None |
| `Timeout(None, connect=1.0)` | 1 | None | None | None |
| all four explicitly supplied without scalar | exact supplied values |

Also pin/reference-test:

- `Timeout()` with no scalar and incomplete phases should behave as HTTPX 0.28.1 does;
- tuple/copy construction if supported by the compatibility contract;
- equality/repr must remain coherent after sentinel introduction;
- `_convert_timeout()` must preserve explicit `None` rather than fill it back in;
- per-request timeout override conversion must preserve explicit disabled phases.

## Native PyO3 boundary

The native `eggfetch.Timeout` constructor may keep native semantics.

Do not force HTTPX's `UNSET` sentinel into the Rust-native API.

The compatibility object should resolve omission vs explicit `None` **before** constructing the native timeout object.

## Allowlist cleanup

The current `TIMEOUT-AS-DICT-001-*` records conflate constructor default differences with the additive `.as_dict` property and incorrectly state that the default mismatch has no migration impact.

After the semantic fix:

- remove resolved constructor-default differences from the active allowlist;
- keep only genuinely additive `Timeout` properties/signature differences that remain intentional;
- move resolved records to the repository's resolved-difference ledger if that is the established process;
- ensure no rationale claims that an `UNSET`/`None` semantic difference is merely an `as_dict` convenience.

Do not add a new intentional difference to excuse `Timeout(5.0, pool=None)` behavior.

## Track 2 acceptance criteria

- explicit `None` disables exactly the selected phase;
- omission inherits the scalar/default;
- four explicit phase values without a scalar follow HTTPX validation rules;
- sync and async clients preserve the same values into request extensions/native conversion;
- API oracle has no unexplained timeout constructor mismatch;
- active timeout allowlist rationales are truthful.

---

# Track 3 — Finish `NO_PROXY` route-selection differential coverage

Retain `test_no_proxy_differential.py` and extend it rather than creating another parallel harness.

Every added row must assert actual direct-vs-proxy routing using a recording local proxy and must run against:

- HTTPX 0.28.1 reference;
- EggFetch compatibility candidate.

## Required remaining rows

### Domain suffix behavior

Add deterministic host-resolution fixtures or a local resolver/test seam sufficient to exercise:

- bare domain vs itself;
- bare domain vs subdomain;
- leading-dot domain vs bare domain;
- leading-dot domain vs subdomain;
- near-match that must not bypass.

Do not use public DNS.

### Port/default-port behavior

Pin:

- explicit matching non-default port;
- same host with different port;
- HTTP default port normalization;
- HTTPS default port normalization.

### IPv6

Where the host supports IPv6 loopback, test:

- bare `::1`;
- bracketed `[::1]` if accepted by HTTPX environment parsing;
- at least one nonmatching IPv6 literal/prefix-looking entry.

If IPv6 loopback is unavailable, use one explicit platform-capability skip with a reason shared by reference and candidate. Do not silently omit the matrix.

### Scheme-qualified patterns

Pin both directions:

- `http://host` with HTTP target -> bypass;
- `http://host` with HTTPS target -> proxy;
- `https://host` with HTTPS target -> bypass;
- `https://host` with HTTP target -> proxy;
- scheme-qualified host + explicit port.

### CIDR-looking text

Demonstrate that HTTPX compatibility does **not** acquire native true-CIDR semantics:

- apparent CIDR exact textual pattern behavior;
- address inside the apparent subnet but not exact host pattern -> does not bypass merely because it is in the subnet;
- one IPv6 prefix-looking row where deterministic.

### Environment precedence/completeness

Add or explicitly link existing differential tests for:

- lowercase proxy var vs uppercase proxy var;
- lowercase `no_proxy` vs uppercase `NO_PROXY`;
- scheme-less proxy normalization;
- `trust_env=False`;
- no applicable proxy variable -> direct route.

## Track 3 acceptance criteria

- every required semantic category above has executable reference/candidate evidence;
- all assertions are route observations, not parser-self-tests;
- no native true-CIDR behavior leaks into the compatibility parser;
- no external network is used.

---

# Track 4 — Complete proxy differential evidence without changing proxy architecture

Keep the existing four-way routing matrix:

| Origin | Proxy |
| --- | --- |
| HTTP | HTTP |
| HTTPS | HTTP |
| HTTP | HTTPS |
| HTTPS | HTTPS |

The current matrix proves basic success/method/target. Extend it only with the missing acceptance assertions.

## Required additions

### Independent TLS identity

Prove with deterministic local certificates that:

- HTTPS proxy certificate verification is against the proxy identity;
- HTTPS origin certificate verification after CONNECT is against the origin identity;
- trusting only one side does not accidentally trust the other;
- proxy and origin SNI/verification identities do not collapse into one value.

A deliberately wrong proxy certificate and a deliberately wrong origin certificate should fail at the correct layer where practical.

### Inline proxy credentials

For HTTP and HTTPS proxy endpoints where the current compatibility API supports URL/auth credentials:

- reference and candidate send equivalent proxy auth;
- credentials are not forwarded to the tunneled origin;
- redaction behavior remains unchanged in user-visible representations/errors.

### Route isolation

Using the same host/port fixture shape where practical, prove that route identity distinguishes:

- `http://proxy`;
- `https://proxy`.

No connection/client cache entry may be reused across proxy schemes.

## Track 4 acceptance criteria

- all four route combinations still pass for reference and candidate;
- proxy TLS and origin TLS identities are independently proven;
- proxy auth behavior is differential where supported;
- proxy scheme participates in route isolation;
- no direct/UDS/SOCKS change is made.

---

# Track 5 — Complete `Proxy(headers=...)` bounded-difference evidence

Do **not** implement proxy headers in this pass unless implementation is truly trivial and does not expand the native proxy model.

The accepted Stage C disposition remains:

- HTTPX accepts proxy headers;
- EggFetch stores the value object;
- EggFetch raises `NotImplementedError` before native dispatch when non-empty proxy headers are supplied;
- no silent drop occurs.

Expand the existing local recording fixture to pin HTTPX reference behavior for:

1. custom header on a forward HTTP proxy request;
2. custom header on CONNECT;
3. custom proxy header absent from the tunneled origin request;
4. custom header combined with proxy authentication;
5. HTTPS proxy endpoint behavior where the existing local TLS proxy fixture makes this practical.

Candidate evidence must prove:

- sync rejection occurs before dispatch;
- async rejection occurs before dispatch;
- recording proxy observes no request after rejection;
- exception type/message matches the documented bounded difference.

Update `PROXY-HEADERS-001` test references only after those cases exist.

## Track 5 acceptance criteria

- HTTPX reference proxy-leg behavior is executable;
- origin non-leakage is executable;
- candidate never silently discards headers;
- bounded-difference documentation exactly matches observed behavior;
- no generalized proxy metadata framework is introduced.

---

# Track 6 — Focused regression and acceptance suite

Before full qualification, run one focused process containing all changed semantic areas.

At minimum include:

- timeout constructor/reference tests;
- timeout dispatch/reference tests;
- direct connect-vs-read phase regression;
- proxy timeout classification tests from pass 03;
- four-way proxy differential matrix;
- proxy-header bounded-difference tests;
- complete `NO_PROXY` differential matrix;
- the three historical aggregate-order regressions:
  - `test_read_after_close_returns_data`;
  - `test_read_phase_timeout`;
  - `test_real_proxy_server_forward`.

Required result:

- 0 failures;
- 0 unexpected skips;
- 0 corrective xfails;
- no retry-until-green mechanism.

If the expanded IPv6 matrix has a platform capability skip, it must be the one explicit capability skip described in Track 3 and must be documented in the status evidence.

---

# Track 7 — Freeze one executable SHA and re-qualify

After all executable and test changes are complete:

1. commit them as one candidate executable SHA;
2. record that SHA before qualification begins;
3. do not modify executable files during qualification;
4. restart qualification if any executable change is required.

## 7.1 Routine project gate

Run the existing repository validation only:

```sh
./scripts/check.sh
```

Also verify, where not already visible in the script output:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

No CI expansion is authorized.

## 7.2 Full pinned compatibility — three consecutive runs

On the frozen executable SHA:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Run three consecutive times without code changes.

Each run must have:

- 0 failed;
- 0 unexpected skipped;
- 0 corrective xfails;
- no retry plugin masking failures.

Record count and duration for all three runs.

The direct connect-vs-read regression and explicit-`None` timeout tests must be included in these aggregate runs.

## 7.3 API oracle

Regenerate/compare the public API oracle using the existing repository scripts.

Required:

- 0 unexplained differences;
- 0 stale active differences;
- 0 resolved-in-active entries;
- 0 requires-resolution entries;
- timeout `UNSET`/`None` behavior no longer hidden behind misleading `TIMEOUT-AS-DICT-*` rationale;
- `PROXY-HEADERS-001` remains only if still intentionally bounded;
- existing four-element socket-option difference remains accurately classified.

## 7.4 Downstream qualification

Run the existing required-only downstream command using the repository's artifact-manifest workflow.

Do not add packages.

Required:

- all currently required downstream packages pass;
- 0 required failures;
- 0 required errors;
- 0 required skips.

Record candidate artifact SHA(s) in the status file as before.

## 7.5 Routine CI

If existing CI runs naturally on the implementation or a docs-only descendant, record it as supplementary evidence.

Do not:

- add workflows;
- add platforms;
- make CI responsible for the full local qualification matrix.

---

# Track 8 — Restore qualification only after every gate is green

After Track 7 succeeds, update `compat/httpx/0.28.1/profile.toml`:

```toml
stage = "stage-c-qualified"
status = "qualified"
qualified = "<actual qualification date>"
qualification-sha = "<frozen executable SHA>"
```

The qualification SHA must point to the executable commit, not to a documentation-only descendant.

Rewrite the current evidence block in `plans/httpx-parity-correction-status.md` with:

- exact executable SHA;
- exact reference versions;
- focused-suite result;
- all three aggregate run counts/durations;
- API-oracle result;
- downstream result and artifact hashes;
- any explicit IPv6 capability skip;
- routine CI run/job only if it actually occurred;
- retained bounded differences.

Keep the superseded `52b1877...` qualification clearly historical.

---

# Global acceptance criteria

This pass is complete only if all statements below are true:

1. qualification was reopened before executable corrections;
2. HTTPX compatibility no longer applies `read` timeout around whole direct/UDS/H3 request lifecycles;
3. a valid longer connect phase cannot be cut short by a shorter read timeout;
4. connect, write, read, and pool semantics remain independently phase-owned;
5. native explicit `total` remains an optional outer cap;
6. compatibility calls do not synthesize native total;
7. `Timeout(5.0, connect=None)` preserves `connect=None`;
8. `Timeout(5.0, read=None)` preserves `read=None`;
9. `Timeout(5.0, write=None)` preserves `write=None`;
10. `Timeout(5.0, pool=None)` preserves `pool=None`;
11. omission still inherits the scalar timeout;
12. timeout validation/construction matches HTTPX 0.28.1 for all covered public forms;
13. timeout allowlist entries are semantically truthful;
14. `NO_PROXY` domain, port, IPv6, scheme, CIDR-looking, and environment-precedence rows have direct reference/candidate route evidence;
15. all four HTTP/HTTPS origin/proxy combinations retain differential transport proof;
16. proxy and origin TLS identities are independently verified;
17. supported proxy auth behavior has differential evidence and does not leak to the origin;
18. `Proxy(headers=...)` has forward, CONNECT, non-leakage, auth-interaction, and applicable HTTPS-proxy reference evidence;
19. EggFetch proxy-header rejection remains explicit and pre-dispatch if still bounded;
20. focused corrective tests pass in one process;
21. the full pinned compatibility suite passes three consecutive times on one executable SHA;
22. API oracle is clean under the repository's current Stage C policy;
23. all required downstream packages pass;
24. no unrelated architecture, CI, release, dependency, or feature work enters the pass;
25. final `qualification-sha` points to the exact executable SHA that produced the recorded evidence.

---

# Explicit rejection criteria

Reject the implementation if any of the following occurs:

- current Stage C qualification is left in place while known corrections are being made;
- `read` is still an outer timeout over connection establishment;
- whole-request wrappers are renamed rather than phase-corrected;
- HTTPX scalar timeout becomes a native total deadline again;
- native `Timeout.total` is removed to simplify compatibility;
- explicit `None` is still treated as argument omission;
- a new allowed difference is added merely to excuse `Timeout(5.0, pool=None)` or equivalent forms;
- proxy timeout behavior regresses while fixing direct transport;
- direct/UDS/SOCKS/pool architecture is redesigned without a failing differential proving necessity;
- `NO_PROXY` evidence falls back to parser-self-tests instead of route observation;
- CIDR-looking compatibility entries gain native subnet semantics;
- proxy TLS and origin TLS identities are conflated;
- proxy headers are silently discarded;
- missing matrix rows are declared unnecessary without executable reference evidence;
- aggregate failures are waived with retries, xfail, or isolated reruns;
- qualification is restored after fewer than three clean aggregate runs;
- qualification SHA points to a docs-only commit;
- CI/release machinery is expanded.

---

# Expected implementation footprint

Likely files include only a subset of:

```text
compat/httpx/0.28.1/profile.toml
compat/httpx/0.28.1/allowed-differences.toml
compat/httpx/0.28.1/resolved-differences.toml
crates/eggfetch-core/src/pipeline.rs
crates/eggfetch-core/src/stream/read_timeout.rs
crates/eggfetch-core/src/transport/direct.rs
crates/eggfetch-python/python/eggfetch/compat/httpx/_timeout.py
crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py
crates/eggfetch-python/tests/compat/test_timeout_reference_differential.py
crates/eggfetch-python/tests/compat/test_corrective_kernel.py
crates/eggfetch-python/tests/compat/test_no_proxy_differential.py
crates/eggfetch-python/tests/compat/test_proxy_differential.py
crates/eggfetch-python/tests/compat/test_native_proxy_tls.py
plans/httpx-parity-correction-status.md
docs/concepts/timeouts.md
docs/reference/compatibility.md
```

Changes outside these areas require a concrete failing acceptance test and should remain exceptional.

---

# Handoff sequence

Implement in this order:

1. reopen qualification/profile status;
2. add failing reference/candidate timeout tests for connect-vs-read and explicit `None`;
3. correct timeout phase ownership without reopening transport architecture;
4. implement compatibility sentinel semantics;
5. clean timeout allowlist entries;
6. complete `NO_PROXY` differential matrix;
7. complete proxy TLS/auth/header differential evidence;
8. run focused corrective suite;
9. freeze executable SHA;
10. run routine project gate;
11. run full pinned compatibility three consecutive times;
12. run API oracle;
13. run required downstream qualification;
14. restore Stage C qualification against the frozen executable SHA;
15. record optional existing CI evidence if it naturally runs.

If any executable change occurs after step 9, return to step 9 and start qualification again.

This pass should close the HTTPX 0.28.1 parity line. Do not create another broad parity roadmap unless a new failing public-surface differential demonstrates an independent defect outside this scope.
