# HTTPX 0.28.1 Parity — Final IPv6 `NO_PROXY` and Differential Evidence Closure Pass 06

## Purpose

This is the final narrow corrective closure pass for the HTTPX 0.28.1 parity line.

Repository state at planning time:

- `main`: `84b05ce97702b79f2488ee82f2bb614687b39b5d`;
- current Stage C qualification SHA: `6c1013a554483f51023a0b7d534198b1c0a9229a`;
- pinned reference: `httpx==0.28.1`;
- pinned transport reference: `httpcore==1.0.9`;
- pass 05 plan: `plans/httpx-parity-final-closure-pass-05.md`;
- current aggregate pass-05 evidence: focused 208 passed, full compat 1599 passed three consecutive times, API oracle 71 allowed / 0 unexplained / 0 stale / 0 resolved-active, required downstream 4/4.

Pass 05 correctly fixed ordinary bare-domain `NO_PROXY` matching, leading-dot domain behavior, compatibility host+port/default-port handling, `Timeout.as_dict` allowlist truthfulness, and exact-SHA verification hygiene.

Two closure issues remain:

1. the HTTPX compatibility parser accepts IPv6 `NO_PROXY` forms that the pinned HTTPX environment parser may reject during proxy configuration, notably bracketed and prefix-looking forms;
2. several pass-05 `NO_PROXY` edge cases are proven through direct `httpx._utils.URLPattern.matches()` and Rust `should_bypass()` tests rather than the reference/candidate pre-dispatch or route-observable evidence explicitly required by the closure plan.

This pass exists only to resolve those points and regenerate final exact-SHA qualification. It must not reopen previously closed transport, timeout, runtime, proxy, pooling, dependency, CI, or release work.

---

# Scope firewall

## In scope

Only the following work is authorized:

- reopen current Stage C qualification before executable/test corrections;
- pin actual HTTPX 0.28.1 behavior for bracketed IPv6 and IPv6 CIDR/prefix-looking `NO_PROXY` environment values;
- make EggFetch compatibility behavior match the pinned reference for those forms, or record a narrowly justified intentional difference only if exact matching is technically impossible without disproportionate architecture changes;
- add reference/candidate tests that observe either:
  - pre-dispatch construction/configuration failure and zero network dispatch, or
  - actual direct-vs-proxy routing;
- strengthen the pass-05 generic-domain/default-port/CIDR/IPv6 evidence so corrected compatibility semantics are not supported only by parser-self-tests;
- preserve native Rust `NoProxy::parse()` behavior, including richer native CIDR support;
- freeze all executable code, tests, fixtures, allowlist, reference manifest, and verification scripts before selecting the qualification SHA;
- rerun the existing focused, routine, full compat, API-oracle, documentation, Rust, downstream, and optional routine-CI gates;
- restore Stage C only against the exact SHA that produced the new evidence.

## Explicitly out of scope

Do not change or redesign:

- direct Hyper transport;
- UDS transport;
- SOCKS transport or pooling;
- HTTP/HTTPS proxy transport architecture;
- proxy TLS layering or certificate policy;
- Python response/runtime ownership or stream lifetime;
- timeout phase ownership;
- HTTPX timeout `UNSET` behavior;
- native `Timeout.total`;
- connection-pool behavior;
- request/response object models unrelated to this defect;
- arbitrary Python `ssl.SSLContext` proxy support;
- `Proxy(headers=...)` implementation;
- four-element socket-option support;
- HTTP/2 or HTTP/3 architecture;
- dependencies;
- CI topology;
- release workflows;
- supported Python versions;
- general documentation cleanup;
- broad API parity work.

Any change outside HTTPX environment matching, its tests/fixtures, allowlist/status/profile records, or verification evidence requires a concrete failing public-surface differential from this pass.

---

# Reference contract: prove before implementing

Do not infer behavior from generic `NO_PROXY`, curl, requests, urllib, or RFC conventions. The only compatibility target in this pass is `httpx==0.28.1` using its actual environment proxy path.

HTTPX 0.28.1 obtains environment values through `urllib.request.getproxies()` and converts `NO_PROXY` entries into mount keys/`URLPattern` objects. The conversion distinguishes ordinary domains, IP literals, localhost, scheme-qualified values, and values that fail URL-pattern parsing.

The current EggFetch compatibility parser has explicit acceptance paths for bracketed IPv6 such as:

```text
[::1]
[::1]:8080
```

and for prefix-looking forms by splitting on `/` before deciding whether an IP literal is present.

That may be broader than the reference. The implementation must not be changed based only on source inspection; first capture the actual reference behavior below.

---

# Track 0 — Reopen qualification before corrections

The first implementation commit in pass 06 must contain qualification/status changes only.

Required profile state while this pass is active:

```toml
stage = "stage-c-candidate"
status = "final-ipv6-no-proxy-closure-pending"
qualified = ""
qualification-sha = ""
```

The exact status wording may follow repository conventions, but it must unambiguously mean not qualified.

Update the current block in `plans/httpx-parity-correction-status.md` to state:

- qualification was reopened from `6c1013a554483f51023a0b7d534198b1c0a9229a`;
- pass-05 evidence remains historical and auditable;
- pass-04 timeout/runtime/proxy work and pass-05 generic domain/default-port fixes are not being reopened;
- pass 06 is limited to IPv6 environment-form parity and route/pre-dispatch evidence completion.

Acceptance criteria:

- no executable or test correction lands while the profile still claims Stage C qualification;
- the 1599 x3 pass-05 runs remain recorded as historical evidence, not current evidence;
- the reopening commit itself changes no runtime behavior.

---

# Track 1 — Build an executable HTTPX IPv6 environment-form truth table

Add a focused reference test module or extend `test_no_proxy_differential.py` without creating a parallel compatibility framework.

The test must exercise **HTTPX client environment configuration**, not only instantiate `URLPattern` directly.

Use `mock.patch.dict(os.environ, ..., clear=True)` or the existing deterministic environment harness. No public network is allowed.

For every invalid-reference row, create a recording local proxy and local origin where useful and assert that neither receives a request when HTTPX fails before dispatch.

## Required reference rows

Pin the exact outcome for at least:

1. bare IPv6 loopback:

```text
NO_PROXY=::1
```

2. bracketed IPv6 loopback:

```text
NO_PROXY=[::1]
```

3. bare IPv6 + non-default port:

```text
NO_PROXY=::1:<port>
```

If that textual form is ambiguous/invalid in HTTPX, record that exact outcome rather than inventing an interpretation.

4. bracketed IPv6 + non-default port:

```text
NO_PROXY=[::1]:<port>
```

5. IPv6 prefix/CIDR-looking text:

```text
NO_PROXY=::1/128
```

6. bracketed IPv6 prefix-looking text where the reference accepts the environment string far enough to parse it:

```text
NO_PROXY=[::1]/128
```

7. a non-loopback IPv6 literal or deterministic synthetic value, sufficient to prove exact-vs-nonmatching behavior without external routing.

## Outcomes to record

Each row must classify the reference as exactly one of:

- client/environment configuration succeeds and target bypasses proxy;
- configuration succeeds and target uses proxy;
- configuration succeeds but request later fails for an unrelated deterministic network reason;
- client/environment configuration raises before any request dispatch.

For failure rows, record:

- exact exception class;
- stable message substring only if stable enough to assert;
- whether failure occurs at client construction, mount generation, or first request setup;
- zero recorded proxy/origin requests.

Do not normalize or sanitize the environment value before giving it to HTTPX. The point is to capture the public observed behavior of the pinned version.

## Track 1 acceptance criteria

- all required IPv6 forms have executable HTTPX reference evidence;
- no result is inferred solely from `is_ipv6_hostname()` or `URLPattern` source;
- malformed/reference-rejected forms prove zero dispatch;
- bare `::1` behavior remains separately pinned from bracketed/prefix-looking behavior.

---

# Track 2 — Add candidate tests against the same truth table

Run the same semantic rows against `eggfetch.compat.httpx`.

For each row, assert the same externally visible outcome category as HTTPX unless the row is explicitly approved as an intentional Stage C difference.

Preferred outcome is exact parity.

## For reference-rejected forms

If HTTPX rejects a `NO_PROXY` entry before dispatch, EggFetch should also reject that form at the compatibility environment boundary before native proxy routing is constructed.

Do not silently:

- strip IPv6 brackets;
- strip CIDR/prefix text;
- reinterpret invalid HTTPX input as an exact host;
- fall back to native true-CIDR matching;
- route the request anyway.

The candidate test must prove:

- same broad lifecycle point: pre-dispatch;
- compatible exception family/type where feasible;
- no recording proxy request;
- no recording origin request.

## For reference-accepted forms

If HTTPX accepts a form, candidate tests must observe actual direct-vs-proxy behavior rather than only call `NoProxy::should_bypass()`.

## Track 2 acceptance criteria

- every Track 1 row has a candidate counterpart;
- invalid reference forms are not broadened into accepted candidate forms;
- accepted reference forms match route selection;
- native `NoProxy::parse()` remains unaffected.

---

# Track 3 — Correct only `parse_httpx()` / compatibility environment validation

The expected implementation area is `crates/eggfetch-core/src/proxy.rs` and/or the Python compatibility environment conversion layer that calls it.

Choose the smallest boundary that can reproduce HTTPX behavior accurately.

## Required design constraints

1. Keep native parsing separate.

`NoProxy::parse()` is a native EggFetch API and may retain:

- bracketed IPv6 support;
- native IPv6 host+port syntax;
- true CIDR matching;
- default-port behavior documented for native rules.

Do not regress native functionality merely because HTTPX's environment syntax is narrower.

2. HTTPX compatibility must reject or match based on the reference truth table.

If a bracketed or prefix-looking form is rejected by HTTPX, the compatibility parser should fail explicitly instead of accepting a broader native representation.

3. Do not add a generic URL-pattern engine.

The remaining mismatch does not justify a new proxy-routing abstraction, regex framework, or generalized matcher.

4. Keep error handling bounded.

If the compatibility layer must translate a native `InvalidProxyUrl` into HTTPX's `InvalidURL`/configuration exception, perform that translation at the existing Python exception boundary. Do not contaminate native error taxonomy unnecessarily.

5. Preserve all pass-05 semantics.

The following must remain unchanged:

- `example.test` matches bare host and subdomains at a label boundary;
- `.example.test` matches subdomains only;
- localhost remains special/exact per reference;
- IPv4 literals remain exact;
- ordinary host+explicit-port follows HTTPX normalized-port behavior;
- CIDR-looking IPv4 text does not acquire native subnet semantics in compat;
- scheme-qualified patterns retain reference scheme/host/port behavior.

## Track 3 acceptance criteria

- the full Track 1/2 truth table matches HTTPX;
- native parser tests remain green and unchanged except for new tests documenting preserved native behavior;
- no transport/proxy architecture change is made;
- no new dependency is added;
- no unrelated error behavior changes.

---

# Track 4 — Close pass-05 route/pre-dispatch evidence gaps

Pass 05 added useful reference `URLPattern` truth tests and Rust parser tests, but its acceptance criteria required reference/candidate route-observable evidence for corrected rules.

Do not delete the parser/unit tests; they remain useful. Add the smallest deterministic external-behavior layer needed to make the evidence complete.

## 4.1 Generic domain behavior

Provide reference/candidate evidence for:

- bare generic domain -> bare host bypass;
- bare generic domain -> subdomain bypass;
- bare generic domain -> near-match uses proxy;
- leading-dot generic domain -> bare host uses proxy;
- leading-dot generic domain -> subdomain bypass.

Do not use `localhost` as the only domain fixture because HTTPX special-cases it.

### Deterministic host resolution

No public DNS is allowed.

Use the smallest test-only mechanism that can make the same synthetic hostname reach loopback for both engines. Acceptable approaches include:

- an existing repository resolver/connector seam if one already exists;
- a deterministic local test fixture that maps synthetic hostnames without modifying production transport logic;
- separate but equivalent reference/candidate local-resolution adapters in the test harness if necessary.

Do **not** add production DNS abstraction solely for these tests.

If direct synthetic-host resolution cannot be achieved without changing production architecture, use a pre-dispatch route-selection seam that exercises the full environment-to-routing decision for both reference and candidate, and document why actual socket dispatch is impractical. This is a fallback, not the preferred path.

## 4.2 Host+port/default-port behavior

Add reference/candidate externally observable evidence for:

- non-default matching port;
- non-default different port;
- explicit default HTTP port behavior;
- explicit default HTTPS port behavior;
- scheme-qualified default-port behavior.

Where binding a local server to port 80/443 is unavailable or inappropriate, use a deterministic pre-dispatch route-selection seam for the normalized URL rather than requiring privileged ports.

The test must still compare HTTPX and EggFetch behavior through their environment routing layers; a standalone candidate `should_bypass()` assertion is insufficient.

## 4.3 IPv4 CIDR-looking behavior

Add reference/candidate evidence that:

- exact textual/IP behavior matches the pinned reference;
- an address merely inside an apparent subnet does not bypass because of native CIDR semantics.

No external private-network route is required. A deterministic route-selection fixture is acceptable.

## 4.4 IPv6 evidence

The Track 1/2 matrix supplies the final IPv6 evidence and should be linked from this track rather than duplicated.

## Track 4 acceptance criteria

- every behavior changed in pass 05 has at least one reference/candidate external-behavior test;
- parser-self-tests are supplementary rather than sole evidence;
- no public DNS or privileged system modification is required;
- no production transport architecture is introduced just to satisfy testing.

---

# Track 5 — Focused regression suite

Before selecting a qualification SHA, run one focused process containing all pass-05/pass-06 semantic closure areas.

At minimum include:

- `test_no_proxy_differential.py`;
- any new IPv6 environment-form test module;
- environment precedence/scheme-less normalization tests;
- `test_timeout_reference_differential.py` to ensure pass-04/pass-05 timeout ledger semantics remain intact;
- proxy differential tests sufficient to prove the routing subsystem was not disturbed;
- native `NoProxy::parse()` Rust unit tests;
- downstream-runner unit/fixture coverage if `scripts/run_isolated_downstream.py` is unchanged, only as a regression guard.

Required result:

- 0 failures;
- 0 unexpected skips;
- 0 corrective xfails;
- no retry-until-green behavior;
- any IPv6 capability skip must be explicit, shared by reference/candidate where applicable, and recorded in qualification evidence.

Reference-rejected malformed IPv6 forms are not capability-dependent and must run even when IPv6 loopback sockets are unavailable, because they should fail before network dispatch.

---

# Track 6 — Freeze one exact executable/evidence SHA

After all code, tests, fixtures, allowlist, manifest, and verification-script changes are complete:

1. commit them;
2. record that commit as the candidate qualification SHA;
3. make no executable/test/fixture/allowlist/reference-manifest/verification-script changes during qualification;
4. if any such file changes, select a new SHA and restart qualification.

The qualification SHA must include:

- final `NO_PROXY` compatibility implementation;
- all IPv6 truth-table tests;
- completed route/pre-dispatch differential evidence;
- active allowed-difference ledger;
- pinned reference manifest;
- `scripts/check.sh`;
- downstream qualification scripts;
- any fixture used by the focused/full qualification runs.

Only status/profile/general documentation commits may follow without restarting qualification.

---

# Track 7 — Qualification gates

## 7.1 Routine repository gate

Run the existing project validation unchanged:

```sh
./scripts/check.sh
```

Also explicitly record, if not already evident in the script output:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features -- --test-threads=1
```

Do not expand CI or add new release gates.

## 7.2 Focused closure suite

Run the Track 5 focused command on the frozen SHA.

Record:

- exact command;
- passed count;
- duration;
- warnings;
- skips/xfails;
- IPv6 capability state.

## 7.3 Full pinned compatibility suite — three consecutive runs

Run exactly:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

three consecutive times on the frozen SHA.

Each run requires:

- 0 failed;
- 0 unexpected skipped;
- 0 corrective xfails;
- no retry plugin masking failures;
- the new IPv6 malformed-form tests included in the aggregate count.

Record count and duration for all three runs.

## 7.4 API oracle

Run the existing manifest/oracle comparison using the frozen reference manifest and active allowed-difference file.

Required:

- 0 unexplained;
- 0 stale allowed differences;
- 0 resolved-in-active;
- 0 requires-resolution;
- no new allowed difference used merely to excuse broader IPv6 syntax unless exact parity is technically impossible and the difference is explicitly reviewed;
- existing `Timeout.as_dict`, `Proxy(headers=...)`, Python proxy `ssl_context`, and four-element socket-option records remain truthful.

## 7.5 Downstream qualification

Run the existing required-only isolated downstream qualification with the artifact manifest.

Required:

- 4/4 currently required packages pass;
- 0 required failures;
- 0 required errors;
- 0 required skips;
- candidate/replacement artifact hashes recorded.

Do not add packages or change downstream policy in this pass.

## 7.6 Documentation

Run existing documentation checks:

- Python Markdown examples;
- internal links;
- all-features rustdoc;
- core doctests.

Only documentation directly affected by the final compatibility statement should need updates.

## 7.7 Routine CI

If existing CI runs naturally on the frozen SHA or a documentation-only descendant, record it as supplementary evidence.

Do not add workflows, platforms, or release automation.

---

# Track 8 — Restore Stage C only after every gate is green

After Track 7 succeeds, restore:

```toml
stage = "stage-c-qualified"
status = "qualified"
qualified = "<actual date>"
qualification-sha = "<frozen pass-06 executable/evidence SHA>"
```

Update `plans/httpx-parity-correction-status.md` with a new current evidence block containing:

- exact qualification SHA;
- CPython/pytest/pytest-asyncio/httpx/httpcore/socksio versions;
- focused-suite result;
- all three aggregate full-suite counts and durations;
- malformed/bracketed/prefix-looking IPv6 reference/candidate outcomes;
- IPv6 loopback capability state;
- API-oracle result;
- Rust workspace result;
- documentation result;
- downstream 4/4 result and artifact hashes;
- optional routine CI run/job if actually observed;
- retained bounded differences.

Keep pass-05 `6c1013a...` qualification clearly historical after this pass.

---

# Global acceptance criteria

Pass 06 is complete only if all of the following are true:

1. Stage C qualification was reopened before corrections;
2. HTTPX 0.28.1 behavior for `NO_PROXY=::1` is executable and pinned;
3. HTTPX behavior for `NO_PROXY=[::1]` is executable and pinned;
4. HTTPX behavior for bare/bracketed IPv6 + port forms is executable and pinned;
5. HTTPX behavior for IPv6 prefix/CIDR-looking forms is executable and pinned;
6. EggFetch compatibility matches the reference outcome for every pinned IPv6 form, including pre-dispatch rejection where applicable;
7. invalid compatibility inputs do not silently normalize into broader accepted native forms;
8. malformed-form tests prove zero network dispatch;
9. accepted IPv6 forms have actual route-selection evidence when the platform supports IPv6 loopback;
10. native `NoProxy::parse()` retains its documented richer IPv6/CIDR behavior;
11. generic bare-domain pass-05 behavior retains reference/candidate external-behavior evidence;
12. leading-dot domain pass-05 behavior retains reference/candidate external-behavior evidence;
13. near-match domain behavior retains reference/candidate external-behavior evidence;
14. non-default host+port behavior retains reference/candidate external-behavior evidence;
15. HTTP/HTTPS default-port behavior is compared through the environment routing layer, not only parser-unit tests;
16. IPv4 CIDR-looking compatibility behavior cannot gain native subnet semantics;
17. environment precedence, scheme-less normalization, wildcard, and `trust_env=False` remain green;
18. timeout, proxy, SOCKS, UDS, runtime ownership, and pooling behavior are unchanged;
19. no new dependency is added;
20. no CI/release expansion occurs;
21. the focused closure suite is clean;
22. the full pinned compatibility suite passes three consecutive times on one frozen SHA;
23. API oracle is clean;
24. all required downstream packages pass;
25. final `qualification-sha` contains the exact code, tests, fixtures, allowlist, manifest, and verification scripts that produced the evidence;
26. only documentation/status commits follow the frozen qualification SHA;
27. the status record clearly marks pass-05 evidence historical and pass-06 evidence current.

---

# Explicit rejection criteria

Reject the implementation if any of the following occurs:

- Stage C remains advertised while known corrections are being made;
- bracketed/prefix-looking IPv6 behavior is guessed rather than executed against HTTPX 0.28.1;
- a malformed HTTPX environment form is silently accepted by EggFetch without an intentional-difference record;
- a new allowed difference is added simply to avoid a small compatibility validation fix;
- native `NoProxy::parse()` loses true CIDR or documented IPv6 behavior to simplify compatibility;
- IPv6 compatibility starts using native subnet matching;
- generic domain/default-port evidence remains only `URLPattern.matches()` plus candidate `should_bypass()` assertions when a deterministic external-behavior comparison is feasible;
- public DNS is required;
- production DNS/transport architecture is added solely for testing;
- transports, proxy TLS, timeout ownership, runtime ownership, SOCKS, UDS, pooling, or HTTP/3 are modified without a new failing public-surface differential;
- aggregate failures are waived with xfail, retries, or isolated reruns;
- qualification is restored after fewer than three clean aggregate runs;
- the qualification SHA predates any test, fixture, allowlist, manifest, or verification script used to claim qualification;
- a later non-documentation commit lands without requalification;
- CI/release machinery is expanded.

---

# Expected implementation footprint

Likely files are limited to a subset of:

```text
compat/httpx/0.28.1/profile.toml
compat/httpx/0.28.1/allowed-differences.toml
compat/httpx/0.28.1/resolved-differences.toml
crates/eggfetch-core/src/proxy.rs
crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py
crates/eggfetch-python/python/eggfetch/compat/httpx/_exceptions.py
crates/eggfetch-python/tests/compat/native_fixtures.py
crates/eggfetch-python/tests/compat/test_no_proxy_differential.py
crates/eggfetch-python/tests/compat/test_environment.py
plans/httpx-parity-correction-status.md
docs/reference/compatibility.md
README.md
```

A new narrowly named IPv6 environment test file is acceptable if extending `test_no_proxy_differential.py` would make it materially less readable.

Changes to transport modules, streaming/runtime code, timeout modules, proxy tunnel code, SOCKS code, pool code, Cargo dependencies, workflows, or release files are unexpected and require explicit failing evidence.

---

# Handoff sequence

Implement in this order:

1. reopen Stage C qualification/profile status;
2. add HTTPX-only executable IPv6 environment-form truth-table tests;
3. add candidate counterparts and document the failing rows;
4. make the smallest compatibility parser/validation correction;
5. add/complete route/pre-dispatch evidence for pass-05 generic domain, port/default-port, CIDR-looking, and IPv6 semantics;
6. run focused pass-05/pass-06 regression suite;
7. verify native `NoProxy::parse()` behavior remains intact;
8. freeze one executable/evidence SHA containing code, tests, fixtures, allowlist, manifest, and verification scripts;
9. run `./scripts/check.sh` and explicit serialized Rust workspace verification;
10. run the full pinned compatibility suite three consecutive times;
11. run API oracle;
12. run required-only downstream qualification;
13. run documentation checks;
14. restore Stage C against the frozen SHA only if every gate passes;
15. record optional existing CI evidence if it naturally runs.

If any executable, test, fixture, allowlist, reference-manifest, or verification-script change occurs after step 8, return to step 8 and restart qualification.

---

# Closure declaration

If all global acceptance criteria pass, mark the HTTPX 0.28.1 parity corrective line closed.

Do not create pass 07 or another broad parity roadmap merely to chase theoretical edge cases. Reopen the line only if a new reproducible public-surface differential against the pinned HTTPX 0.28.1 contract is demonstrated.
