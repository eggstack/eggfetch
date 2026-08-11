# HTTPX Parity Final Qualification — Corrective Pass 02

## Purpose

This is the final narrow follow-up to `plans/httpx-parity-final-qualification-corrective-pass.md`.

It exists only because the implementation at `044e02f3ab5c4bafaab7aa9e91283f109b3675ba` closed most of the intended transport work but did **not** satisfy the final qualification contract. The repository correctly remains a Stage C candidate.

Planning baseline:

- repository head when this plan was written: `dd22ebb958610e278660cd03831bf7ad91398737`;
- current corrective executable SHA: `044e02f3ab5c4bafaab7aa9e91283f109b3675ba`;
- current profile state: `stage-c-candidate` / `final-corrective-qualification-pending`;
- current recorded full pinned result: `1479 passed, 3 failed` in aggregate, with the three failures passing when isolated.

This pass must **not** reopen the now-correct direct connector, UDS transport, persistent SOCKS route architecture, general TLS stack, CI/release policy, or the broader HTTPX roadmap.

The remaining closure items are:

1. make HTTPS-proxy multi-phase timeout ownership use one coherent deadline rather than restarting the same remaining duration per phase;
2. accept `bytearray` in the ordinary three-element HTTPX socket-option form;
3. settle `Proxy(headers=...)` behavior so public metadata is either propagated or explicitly bounded and differential-pinned, never silently discarded;
4. add the missing HTTPX-vs-EggFetch differential proof for HTTPS proxy routing and the corrected `NO_PROXY` edge semantics;
5. diagnose and remove the three aggregate-suite ordering/state failures;
6. rerun one exact-SHA qualification and only then restore Stage C qualified status.

---

# Scope firewall

## In scope

- `crates/eggfetch-core/src/transport/proxy.rs` and `connect.rs` only as required for coherent proxy deadline ownership;
- the smallest supporting timeout/deadline utility if one is already the project pattern;
- Python compatibility socket-option conversion/validation for `bytearray`;
- compatibility `Proxy` metadata conversion for `headers`, if representable through existing core request/proxy machinery without a generalized transport redesign;
- HTTPX 0.28.1 differential fixtures for HTTP/HTTPS proxy endpoint combinations;
- HTTPX 0.28.1 differential fixtures for scheme-qualified, IPv6, CIDR-looking, domain, leading-dot, port, wildcard, and environment-precedence `NO_PROXY` behavior;
- fixture/global-state cleanup required to make the aggregate pinned compatibility suite deterministic;
- compatibility profile/allowed-difference/status documentation required by the observed result;
- one final exact-SHA qualification run.

## Explicitly out of scope

- Trio/AnyIO support;
- Python 3.8/3.9 support;
- private HTTPX modules;
- HTTPX versions other than 0.28.1;
- HTTP/3 parity changes;
- redesign of direct TCP, UDS, or SOCKS transports;
- new generalized proxy/mount architecture;
- arbitrary Python `ssl.SSLContext` emulation for `Proxy.ssl_context`;
- unsafe Rust or raw arbitrary `setsockopt` escape hatches solely to support the HTTPX four-element socket-option form;
- CI or release workflow expansion;
- unrelated cleanup, dependency changes, or performance refactors.

---

# Track 0 — Preserve pending qualification and freeze the baseline

Before code changes:

1. confirm `compat/httpx/0.28.1/profile.toml` remains:
   - `stage = "stage-c-candidate"`;
   - `status = "final-corrective-qualification-pending"`;
   - blank qualification date/SHA;
2. preserve `plans/httpx-parity-correction-status.md` as a pending evidence record;
3. record the actual implementation starting SHA in the final handoff;
4. do not reuse old qualification counts as current evidence.

Acceptance:

- no commit in the implementation sequence marks Stage C qualified before all final gates pass;
- old `ace3782...`, `044e02f...`, and prior qualification attempts remain historical evidence only.

---

# Track 1 — Fix HTTPS-proxy timeout ownership to one wall-clock deadline

## Current defect

The current proxy path receives one `remaining_total` duration and applies that same full duration independently to multiple phases. In the HTTPS-proxy path this can effectively grant fresh time to:

1. proxy TCP connect;
2. proxy TLS handshake;
3. CONNECT exchange;
4. origin TLS handshake;
5. request setup/response-header work.

This contradicts the previous plan's requirement that a request total budget be consumed monotonically across phases.

## Required design

Use one request-scoped deadline or equivalent monotonic elapsed-time accounting.

Preferred narrow pattern:

```text
request starts
  -> deadline = start + total_timeout
  -> before each blocking phase:
       remaining = deadline - now
       if exhausted: timeout
       else timeout(remaining, phase)
```

The deadline must be created at the existing request-total ownership boundary, not independently inside each transport helper.

If the core already has an established deadline/remaining helper, reuse it. Do not introduce a second timeout framework.

## Required phases

For an HTTPS origin through an HTTPS proxy, the same deadline must cover at least:

- pool acquisition where total timeout already owns it;
- DNS/connect to proxy;
- proxy TLS handshake;
- sending CONNECT;
- reading CONNECT response;
- origin TLS handshake;
- request write/setup;
- response-header acquisition, consistent with the existing total-timeout contract.

Phase-specific timeout classification should remain accurate even though the remaining wall-clock budget is shared.

## Deterministic tests

Add a fixture that deliberately spends measurable portions of the budget in more than one phase.

At minimum prove:

- proxy TCP delay + proxy TLS delay cannot each consume a full total timeout;
- proxy TLS delay + CONNECT delay cannot each consume a full total timeout;
- CONNECT delay + origin TLS delay cannot each consume a full total timeout;
- a fast path still succeeds;
- timeout error keeps the correct request context and compatible exception family;
- cancelling a timed-out/stalled proxy request leaves the client usable for a follow-up request.

Use broad-enough timing margins to avoid scheduler flakiness. The assertion should prove an upper envelope, not depend on millisecond precision.

## Track 1 acceptance criteria

- one monotonic deadline owns the multi-phase proxy path;
- no phase receives the original full total duration after previous phases consumed time;
- HTTP-over-HTTPS-proxy and HTTPS-over-HTTPS-proxy still succeed;
- HTTP-proxy and SOCKS behavior are not regressed;
- timeout classification remains coherent;
- deterministic test proves budget non-multiplication.

Reject if:

- each helper simply receives the same original duration;
- the fix adds sleeps to production code;
- total timeout is disabled around proxy TLS to avoid the problem;
- a new general timeout subsystem is introduced.

---

# Track 2 — Close ordinary three-element socket-option `bytearray` parity

## Current defect

The corrected reference fixture establishes that HTTPX 0.28.1 accepts `bytearray` as the value in the ordinary three-element socket-option form.

EggFetch's compatibility conversion currently accepts `int` and `bytes`, but rejects `bytearray` before native dispatch.

This is separate from the intentionally bounded four-element `(level, option, None, optlen)` form.

## Required change

At the Python compatibility boundary:

- accept `bytes`;
- accept `bytearray` and convert it losslessly to bytes/native byte storage;
- continue accepting supported integer values;
- keep the four-element arbitrary null-pointer form explicitly unsupported unless a safe existing abstraction already supports it.

Do not broaden the native socket API beyond the current bounded safe option set.

## Differential tests

Run the same safe three-element byte-array option through:

1. HTTPX 0.28.1;
2. EggFetch compatibility facade.

Prefer an option/value that is valid on the test platform. If platform behavior differs, reference-pin constructor/dispatch behavior separately from OS acceptance.

## Track 2 acceptance criteria

- `bytearray` is accepted wherever HTTPX accepts the equivalent ordinary three-tuple;
- conversion is lossless;
- existing `bytes` and integer behavior stays green;
- four-element difference remains accurately documented;
- no unsafe socket API is introduced.

---

# Track 3 — Settle `Proxy(headers=...)` without silent data loss

## Current defect

The compatibility `Proxy` object stores public proxy metadata including `headers`, but `_convert_proxy()` currently collapses the object into a URL plus encoded authentication. Proxy headers are therefore not propagated through the native path.

`Proxy.ssl_context` may remain a bounded Stage C limitation under the prior plan. Proxy headers require a specific disposition.

## 3.1 Reference-pin HTTPX behavior first

Create a deterministic local HTTP/HTTPS proxy fixture and establish how HTTPX 0.28.1 applies `Proxy(headers=...)` for:

- HTTP origin through HTTP proxy;
- HTTPS origin through HTTP proxy/CONNECT;
- HTTP origin through HTTPS proxy;
- HTTPS origin through HTTPS proxy/CONNECT;
- interaction with `Proxy(auth=...)` / `Proxy-Authorization`;
- duplicate/conflicting header behavior where observable;
- whether headers appear only on the proxy leg and never leak to the origin.

Do not infer behavior from constructor storage alone.

## 3.2 Preferred implementation

If the existing native proxy request writer can accept an additional bounded header collection without redesign:

- convert compatibility `Proxy.headers` into a native safe header representation;
- apply those headers only to proxy-facing requests;
- preserve explicit authentication conflict/precedence according to the reference;
- ensure proxy-only headers are not copied to tunneled origin requests;
- preserve credential/header redaction in error/debug output.

This should be a small metadata plumbing path, not a generalized interceptor system.

## 3.3 Allowed stop condition

If supporting arbitrary proxy headers requires a broad new cross-language request-routing abstraction, do not expand scope.

Instead:

- add a direct reference/candidate behavioral fixture;
- create or correct an explicit active Stage C allowed-difference entry describing exactly what is ignored;
- document migration impact;
- ensure no documentation claims `Proxy(headers=...)` parity;
- retain the profile as Stage C candidate until the final allowed-difference/oracle gate accepts the bounded difference.

Silent ignore with no explicit difference is not acceptable.

## Track 3 acceptance criteria

One of these two outcomes must be true:

### Preferred closure

- proxy headers propagate correctly on all supported proxy endpoint schemes;
- proxy-only headers never reach the destination after CONNECT;
- auth/header precedence matches HTTPX;
- redaction remains intact.

### Bounded closure

- unsupported header behavior is differential-pinned;
- exact behavior is represented in `allowed-differences.toml`;
- docs do not claim support;
- API/difference oracle is clean with the intentional Stage C limitation.

---

# Track 4 — Make HTTPS proxy proof genuinely differential

The existing live HTTPS-proxy tests are valuable candidate tests, but the final qualification plan required reference/candidate differential evidence.

Create one compact table-driven test/fixture that runs the same cases against:

- `httpx==0.28.1`;
- `eggfetch.compat.httpx`.

Required routing matrix:

| Origin | Proxy endpoint | Required behavior |
| --- | --- | --- |
| HTTP | `http://` | forward absolute-form |
| HTTPS | `http://` | CONNECT, then origin TLS/origin-form |
| HTTP | `https://` | TLS to proxy, then forward absolute-form |
| HTTPS | `https://` | TLS to proxy, CONNECT, origin TLS, origin-form |

For each applicable case record/assert:

- successful response;
- proxy-observed method;
- proxy-observed target;
- whether TLS-to-proxy occurred;
- CONNECT target;
- origin-observed request target;
- proxy authentication behavior where configured;
- proxy custom-header behavior according to Track 3 disposition;
- proxy certificate verification failure on an untrusted certificate;
- origin certificate verification failure independently of proxy verification;
- route-key isolation between HTTP and HTTPS proxy endpoint schemes.

Use local deterministic certificates/fixtures. Never disable verification merely to turn tests green.

## Track 4 acceptance criteria

- all four routing combinations execute against both reference and candidate;
- target forms match the reference;
- proxy and origin TLS identities are independently proven;
- no test passes solely because the candidate parser accepts the URL;
- no new external-network dependency is added.

---

# Track 5 — Make `NO_PROXY` closure genuinely differential

The implementation now has candidate-side rules for scheme-qualified values, bare IPv6, and CIDR-looking entries. The missing closure is route-selection proof against HTTPX itself.

Build one table-driven fixture that places a direct local server and a recording local proxy in front of the same request and determines which route each runtime chose.

Run every row against:

- HTTPX 0.28.1;
- EggFetch compatibility facade.

Required cases:

## Wildcard / parsing hygiene

- `*`;
- wildcard mixed with other entries;
- comma-separated values;
- surrounding whitespace;
- empty entries.

## Domain rules

- `example.test` vs bare domain;
- `example.test` vs subdomain;
- `.example.test` vs bare domain;
- `.example.test` vs subdomain;
- `badexample.test` near-match.

## Port rules

- same host/same explicit port;
- same host/different port;
- default HTTP port behavior;
- default HTTPS port behavior.

## Localhost/IP

- `localhost`;
- `127.0.0.1`;
- bare `::1`;
- bracketed IPv6 only if reference environment accepts it;
- non-loopback IPv4 literal;
- non-loopback IPv6 where available.

## Scheme-qualified

- `http://example.test`;
- `https://example.test`;
- scheme + port;
- prove HTTP-only exclusion does not bypass HTTPS and vice versa.

## CIDR-looking

- `10.0.0.0/8` against exact `10.0.0.0` host text;
- `10.0.0.0/8` against an address inside the apparent subnet but not exact host text;
- one IPv6 prefix-looking entry if deterministic.

## Environment precedence

- lowercase `no_proxy` vs uppercase `NO_PROXY` collision;
- lowercase proxy variables remain preferred;
- `trust_env=False` bypasses environment discovery entirely.

Do not convert this into a test of the native Rust `NoProxy::parse()` semantics. This is the compatibility facade only.

## Track 5 acceptance criteria

- every listed public compatibility case is reference/candidate differential;
- route selection, not just parser state, is asserted;
- native Rust CIDR semantics remain intact;
- no HTTPX URL-pattern class is copied wholesale;
- all candidate rules match the observed HTTPX 0.28.1 behavior.

---

# Track 6 — Resolve the three aggregate-suite ordering/state failures

Current aggregate failures recorded at `044e02f...`:

- `test_read_after_close_returns_data`;
- `test_read_phase_timeout`;
- `test_real_proxy_server_forward`.

All reportedly pass in isolated invocation. That is not qualification evidence.

## 6.1 Reproduce deterministically

For each failing test:

1. run it alone;
2. run its containing file alone;
3. run it immediately after likely polluting files/fixtures;
4. run the full suite with deterministic ordering;
5. if needed, use pytest's collected order and binary-search the preceding test set to identify the contaminating fixture/state.

Do not add random retries or automatic reruns.

## 6.2 Audit likely shared-state sources

Inspect specifically for:

- environment variables not restored;
- global proxy settings;
- global event-loop/runtime handles;
- shared executor/thread state;
- server threads not joined;
- listener sockets not closed;
- stale monkeypatch/mock state;
- process-global socket defaults;
- reused ports or readiness races;
- shared caches/pools retained across tests;
- tests that alter timeouts/defaults and fail to restore them;
- fixture cleanup that depends on normal test completion only.

## 6.3 Fix root cause, not symptom

Acceptable fixes include:

- fixture context managers that always close/join resources;
- deterministic ready/stop synchronization;
- proper restoration of environment/global state;
- explicit per-test client/resource lifetime;
- eliminating order-sensitive singleton state;
- making local fixture shutdown wait for completion.

Not acceptable:

- `pytest-rerunfailures`;
- retry loops around the failing assertions;
- increasing every timeout globally;
- marking the failures flaky/xfail/skip;
- splitting the qualification suite into isolated commands solely to hide aggregate order dependence.

## 6.4 Stability proof

After fixing the root cause, run the full pinned suite at least three consecutive times from the same built executable/environment.

Required:

- zero failures on every run;
- no new unexpected skips;
- no port/resource leakage between runs.

If a platform-specific nondeterminism remains, write a bounded blocker report instead of restoring qualified status.

## Track 6 acceptance criteria

- all three previously failing tests pass in aggregate;
- root cause is documented in the handoff;
- no retry/xfail/skip workaround is used;
- three consecutive aggregate runs are clean.

---

# Track 7 — Reconcile profile, allowed differences, and documentation

Before final qualification:

1. ensure the four-element socket-option record states the real `(level, option, None, optlen)` reference contract;
2. if `bytearray` is implemented, no difference record may imply ordinary three-element byte-like values are unsupported;
3. record the final `Proxy(headers=...)` outcome from Track 3;
4. keep `Proxy.ssl_context` bounded if still unsupported and do not imply it is honored;
5. ensure docs distinguish:
   - HTTP proxy endpoint (`http://`);
   - HTTPS proxy endpoint (`https://`);
   - HTTPS CONNECT to origin;
   - SOCKS;
6. ensure `NO_PROXY` documentation matches the differential fixture rather than implementation assumptions;
7. keep Stage C exclusions explicit.

Run the API oracle after these changes and remove any stale/resolved-in-active entries using the existing ledger process.

Acceptance:

- zero unexplained differences;
- zero stale allowed entries;
- zero resolved-in-active entries;
- zero requires-resolution entries;
- final active difference count is recorded fresh.

---

# Track 8 — One exact-SHA final qualification

This track begins only after Tracks 1–7 are complete.

## 8.1 Freeze one executable SHA

Create one final executable commit containing all code and executable-test changes.

Record its full SHA. All qualification commands below must use artifacts/tree state from this SHA.

Documentation-only evidence commits may follow, but they must identify the executable SHA explicitly.

## 8.2 Targeted corrective suite

Run at minimum:

```sh
python -m pytest \
  crates/eggfetch-python/tests/compat/test_httpx_reference_pinning.py \
  crates/eggfetch-python/tests/compat/test_environment.py \
  crates/eggfetch-python/tests/compat/test_native_proxy_tls.py \
  crates/eggfetch-python/tests/compat/test_socks_transport.py \
  crates/eggfetch-python/tests/compat/test_uds_transport.py \
  -q --strict-markers
```

Include any new file used for the HTTPS-proxy / `NO_PROXY` differential table if separate.

Record exact counts.

Required:

- zero failures;
- zero unexpected skips.

## 8.3 Routine validation

Run on the same executable SHA:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
./scripts/check.sh
```

Required: all clean.

## 8.4 Full pinned compatibility stability gate

Environment must record:

- Python version;
- `httpx==0.28.1`;
- exact `httpcore` version;
- exact `socksio` version;
- pytest/pytest-asyncio versions if material.

Run:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Run the same command three consecutive times after the aggregate-order fix.

Required:

- zero failures every run;
- zero unexpected skips every run.

Record all three counts.

## 8.5 API oracle

Regenerate and compare the manifest using the current repository commands.

Required:

- zero unexplained;
- zero stale allowed;
- zero resolved-in-active;
- zero requires-resolution.

Record the new allowed-difference count rather than reusing `76` automatically.

## 8.6 Downstream qualification

Use the current repository's isolated downstream runner contract and artifact manifest requirements.

Record:

- exact command;
- executable/artifact SHA or hashes;
- release-blocking package results;
- informational failures separately;
- any exclusion tied explicitly to private HTTPX behavior or excluded concurrency backends.

Required:

- no unexplained public Stage C downstream failure.

## 8.7 CI evidence

Do not modify CI.

If routine CI runs on the final executable SHA, record it.

If it runs only on a documentation descendant, state that accurately.

If no exact-SHA CI exists, local exact-SHA qualification remains the source of truth under the existing project policy.

Never present `94bb4bf...` as direct validation of a later executable tree.

## 8.8 Restore qualification

Only after all gates pass:

- set `qualified` date;
- set `qualification-sha` to the final executable SHA;
- set stage to the repository's Stage C qualified value;
- set status to `qualified`;
- rewrite the current evidence block in `plans/httpx-parity-correction-status.md` so it contains only fresh results attributable to this qualification;
- retain older evidence under clearly historical headings.

---

# Global acceptance criteria

This corrective pass is complete only when **all** of the following are true:

- [ ] profile remains pending during implementation;
- [ ] HTTPS proxy TCP/TLS/CONNECT/origin-TLS phases consume one coherent total-timeout deadline;
- [ ] deterministic tests prove the timeout budget does not multiply across phases;
- [ ] safe three-element socket options accept `bytearray` with HTTPX-compatible behavior;
- [ ] valid four-element `(level, option, None, optlen)` remains either safely implemented or explicitly bounded without unsafe escape hatches;
- [ ] `Proxy(headers=...)` is either propagated correctly or represented as an explicit, differential-pinned Stage C limitation;
- [ ] proxy-only metadata never leaks to the origin after CONNECT;
- [ ] HTTP/HTTPS origin × HTTP/HTTPS proxy routing matrix is executed against both HTTPX and EggFetch;
- [ ] HTTPS proxy identity verification and origin identity verification are independently proven;
- [ ] corrected `NO_PROXY` scheme/IPv6/CIDR/domain/port/wildcard semantics are route-selection differential tests against HTTPX;
- [ ] native Rust `NoProxy` semantics are not weakened solely for compatibility;
- [ ] the three previously aggregate-only failures are root-caused and fixed;
- [ ] no retry/xfail/skip workaround masks those failures;
- [ ] full pinned compatibility suite passes three consecutive aggregate runs;
- [ ] routine Rust/Python checks pass on one executable SHA;
- [ ] API oracle is clean on that SHA;
- [ ] downstream release-blocking qualification is clean for the documented Stage C surface;
- [ ] current evidence block contains only fresh exact-SHA-attributable evidence;
- [ ] old qualification attempts remain historical;
- [ ] no transport redesign, CI/release expansion, Trio/AnyIO work, private-module parity, or unrelated cleanup was introduced;
- [ ] profile returns to Stage C qualified only after every required gate passes.

---

# Rejection criteria

Reject the implementation as incomplete if any of the following is true:

- the same original timeout duration is restarted independently for proxy TCP, proxy TLS, CONNECT, or origin TLS;
- a timing test only asserts that some timeout happened without proving the shared wall-clock envelope;
- production sleeps are introduced;
- `bytearray` remains rejected in an otherwise valid three-element socket option;
- unsafe Rust/raw libc is added solely for four-element socket-option parity;
- `Proxy(headers=...)` remains silently ignored with no explicit behavioral difference;
- proxy headers leak to the destination through a CONNECT tunnel;
- HTTPS proxy tests are candidate-only rather than reference/candidate differential;
- `NO_PROXY` tests inspect only parser objects rather than actual route selection;
- HTTPX compatibility mode starts using native true-CIDR subnet bypass where HTTPX does not;
- any of the three aggregate failures is handled by rerun, xfail, skip, or blanket timeout inflation;
- qualification relies on isolated passing invocations while the aggregate suite still fails;
- old `1479`, `76`, or other historical counts are copied forward without rerunning;
- CI evidence from an older executable SHA is labeled as current exact-SHA evidence;
- profile is marked qualified before all final gates pass;
- direct, UDS, SOCKS, CI/release, or unrelated architecture is rewritten.

---

# Stop conditions

Stop and write a bounded blocker report instead of expanding scope if:

1. coherent deadline ownership would require replacing the existing request timeout architecture rather than threading/reusing an existing deadline primitive;
2. proxy headers require a generalized cross-language middleware framework instead of a small proxy-leg metadata field;
3. a `NO_PROXY` behavior depends on platform-specific `urllib` semantics that cannot be made deterministic in the supported qualification environment;
4. aggregate failure root cause is an external interpreter/runtime defect that remains reproducible after repository fixture state is clean;
5. a downstream failure requires private HTTPX modules, Trio/AnyIO, Python 3.8/3.9, or another excluded surface.

A blocker report must include:

- exact reference scenario;
- exact EggFetch behavior;
- smallest reproducer;
- missing primitive/root cause;
- affected acceptance criterion;
- why closure would violate the scope firewall;
- smallest future follow-up.

Do not convert a blocker to an intentional difference merely to restore green status.

---

# Suggested implementation sequence

Keep commits narrow and bisectable. A suitable sequence is:

1. `fix: preserve one deadline across proxy handshake phases`
2. `fix: accept bytearray socket option values`
3. `test: differential-pin proxy headers and HTTPS proxy routing`
4. `fix: propagate or explicitly bound proxy headers`
5. `test: differential-pin HTTPX NO_PROXY edge routing`
6. `fix: remove HTTPX aggregate fixture-order state leaks`
7. `docs: reconcile final HTTPX differences and qualification scope`
8. `docs: record exact-SHA HTTPX Stage C qualification`

Adjacent commits may be combined if reviewability remains high. Do not combine unrelated cleanup.

---

# Required handoff report

The implementing agent must report:

- planning baseline SHA;
- implementation starting SHA;
- final executable SHA;
- documentation-only SHA if different;
- root cause of each of the three previous aggregate failures;
- exact proxy timeout/deadline design used;
- multi-phase timeout test result;
- bytearray socket-option reference/candidate result;
- four-element socket-option final disposition;
- HTTPX `Proxy(headers=...)` observed reference behavior;
- final EggFetch proxy-header disposition;
- proxy auth/header precedence result;
- proxy-header leak-prevention result;
- HTTP/HTTP, HTTPS/HTTP, HTTP/HTTPS, HTTPS/HTTPS proxy differential results;
- proxy TLS verification/SNI result;
- origin TLS verification/SNI result;
- HTTP/HTTPS proxy route-isolation result;
- complete `NO_PROXY` differential matrix result;
- native `NoProxy` preservation confirmation;
- targeted corrective test count;
- all three consecutive aggregate pinned-suite counts;
- `cargo fmt --all -- --check` result;
- clippy result;
- `cargo test --workspace` result;
- `./scripts/check.sh` result;
- API oracle counts;
- final active allowed-difference count/classifications;
- downstream runner command, artifact manifest/hash identity, and release-blocking result;
- CI result and exact SHA relationship;
- confirmation that no unsafe Rust, transport rewrite, CI/release expansion, Trio/AnyIO, Python 3.8/3.9, private-module, or unrelated scope was added;
- final compatibility designation.

The intended outcome is a small closure pass: fix the remaining correctness/evidence defects around the already-landed transport work, eliminate aggregate test-order instability, and leave EggFetch with one auditable HTTPX 0.28.1 Stage C qualification bound to one clean executable SHA.
