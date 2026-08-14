# HTTPX 0.28.1 Parity — Final `NO_PROXY` and Qualification Hygiene Closure Pass 05

## Purpose

This is a narrow follow-up closure pass for the remaining issues discovered after pass 04 was marked Stage C qualified.

Current repository state at planning time:

- `main`: `83fbe99b4c39533cc156193a87671e47b84dca9f`;
- currently recorded qualification executable: `64a1e2c3f3cea7ddc6eeabcd85a67a4d7a17cb26`;
- current profile: `stage-c-qualified` / `qualified`;
- pinned reference: `httpx==0.28.1`;
- pinned transport reference: `httpcore==1.0.9`;
- pass 04 plan: `plans/httpx-parity-final-closure-pass-04.md`.

Pass 04 correctly closed the timeout sentinel defect, removed the cross-phase direct/UDS/H3 read wrapper, stabilized Python response/stream runtime ownership, completed substantial proxy differential work, and produced strong aggregate/downstream evidence.

Three closure problems remain:

1. HTTPX-compatible `NO_PROXY` matching is still wrong for ordinary bare domains and related host+port/default-port cases;
2. active `Timeout.as_dict` allowed-difference records still describe the pinned HTTPX API inaccurately;
3. `scripts/check.sh` changed in the documentation commit after the recorded executable qualification SHA, so the current exact-SHA qualification record must be regenerated after the final verification tooling is frozen.

This pass must correct only those items, add the missing reference/candidate edge evidence, and regenerate one clean qualification record.

---

# Scope firewall

## In scope

Only the following work is authorized:

- reopen current Stage C qualification while this correction is active;
- correct HTTPX-facade `NO_PROXY` matching for ordinary domains, leading-dot domains, host+port/default-port behavior, IPv4/IPv6 literals, CIDR-looking values, and scheme-qualified values where required by the pinned reference;
- preserve richer native Rust `NoProxy::parse()` semantics outside the HTTPX compatibility parser;
- add deterministic local HTTPX-vs-EggFetch route-selection tests for every corrected rule;
- correct inaccurate `Timeout.as_dict` allowlist metadata without redesigning the `Timeout` implementation;
- freeze verification scripts before qualification;
- rerun the existing routine, full compatibility, API-oracle, downstream, and documentation qualification gates;
- restore Stage C only against the exact executable/test/verification SHA that produced the final evidence.

## Explicitly out of scope

Do not reopen or redesign:

- direct transport architecture;
- UDS transport architecture;
- SOCKS implementation or pooling;
- HTTP/HTTPS proxy transport architecture;
- proxy TLS layering;
- Python runtime ownership/lifetime fixes from pass 04;
- timeout phase ownership;
- HTTPX timeout sentinel behavior;
- native `Timeout.total`;
- arbitrary Python `ssl.SSLContext` proxy support;
- `Proxy(headers=...)` implementation;
- four-element socket-option support;
- HTTP/2 or HTTP/3 architecture;
- connection-pool behavior;
- dependency versions;
- CI topology;
- release machinery;
- supported Python-version policy;
- unrelated documentation or refactoring.

No transport/runtime implementation file should change unless a new failing reference/candidate test in this pass proves that the remaining defect actually resides there. The expected correction is concentrated in HTTPX environment matching, tests, allowlist metadata, status/profile records, and verification hygiene.

---

# Reference contract

All behavior must be pinned to `httpx==0.28.1` rather than inferred from generic `NO_PROXY` conventions.

HTTPX 0.28.1 obtains proxy environment information through `urllib.request.getproxies()` and converts `NO_PROXY` entries into URL-pattern exclusions.

The important reference distinctions are:

## Ordinary domain entry

For an ordinary non-IP/non-localhost domain such as:

```text
NO_PROXY=example.com
```

HTTPX builds the equivalent of:

```text
all://*example.com
```

That pattern matches:

- `example.com`;
- subdomains such as `www.example.com`;

but must not match a near-name such as:

- `wwwexample.com`;
- `notexample.com` where the URL-pattern regex does not represent a domain suffix boundary.

The current EggFetch HTTPX parser must not use exact-host matching for this case.

## Leading-dot domain entry

For:

```text
NO_PROXY=.example.com
```

HTTPX builds the equivalent of:

```text
all://*.example.com
```

This matches subdomains such as `www.example.com` but not the bare `example.com` host.

Do not collapse ordinary-domain and leading-dot-domain behavior into one matcher.

## `localhost`

HTTPX special-cases `localhost` as an exact hostname pattern.

Do not use `localhost` as the sole fixture proving generic domain wildcard behavior.

## IPv4 and IPv6 literals

HTTPX recognizes IP literals separately from ordinary domains and creates exact-host exclusions rather than domain wildcard patterns.

Pin actual behavior for:

- IPv4 loopback;
- IPv6 loopback where the host supports it;
- bracket representation behavior at the environment/parser boundary;
- one nonmatching literal for each family where deterministic.

## CIDR-looking values

HTTPX 0.28.1 does not provide true subnet matching for `NO_PROXY` CIDR-looking strings.

The compatibility facade must continue to avoid leaking EggFetch native CIDR semantics into the HTTPX path.

Native `NoProxy::parse()` may keep true CIDR matching.

## Port behavior

Do not assume generic curl semantics.

Reference-pin HTTPX `URLPattern` behavior for:

- ordinary domain + non-default explicit port;
- ordinary domain + a different port;
- ordinary domain + explicit default HTTP port;
- ordinary domain + explicit default HTTPS port;
- scheme-qualified host + explicit port;
- scheme-qualified host + default port.

HTTPX URL normalization may remove a target URL's default port while an `all://...:port` pattern retains the explicit port. Tests must define the actual contract before the candidate implementation is changed.

## Scheme-qualified entries

Reference-pin:

- `http://host` vs HTTP target;
- `http://host` vs HTTPS target;
- `https://host` vs HTTPS target;
- `https://host` vs HTTP target;
- scheme-qualified host+port.

Existing pass-04 tests may be retained where they already prove a row; do not duplicate fixtures unnecessarily.

---

# Track 0 — Reopen qualification before executable/test corrections

The repository currently advertises Stage C qualification at executable SHA `64a1e2c3f3cea7ddc6eeabcd85a67a4d7a17cb26`, but a public `NO_PROXY` mismatch remains and verification tooling changed after that SHA.

The first implementation commit in this pass must change qualification metadata/status only.

Required in-progress profile state:

```toml
stage = "stage-c-candidate"
status = "final-no-proxy-closure-pending"
qualified = ""
qualification-sha = ""
```

The exact pending status string may follow repository conventions, but it must clearly mean not qualified.

Update the current block in `plans/httpx-parity-correction-status.md` to state:

- qualification was reopened from `64a1e2c3f3cea7ddc6eeabcd85a67a4d7a17cb26`;
- the three `1564 passed` runs, API-oracle result, downstream 4/4 result, and wheel hash remain historical evidence;
- qualification was reopened because ordinary-domain/default-port `NO_PROXY` semantics were not fully covered and because verification script changes landed after the recorded qualification executable;
- pass-04 timeout/runtime/proxy corrections remain accepted and are not being reopened.

Acceptance criteria:

- no executable/test correction is committed while the profile still claims current qualification;
- prior qualification evidence remains auditable and clearly historical;
- pass 05 begins from repository head `83fbe99b4c39533cc156193a87671e47b84dca9f`.

---

# Track 1 — Add failing generic-domain `NO_PROXY` reference/candidate tests first

Before changing the parser, extend the existing route-selection differential harness.

Prefer extending:

```text
crates/eggfetch-python/tests/compat/test_no_proxy_differential.py
```

Do not create a second parallel `NO_PROXY` framework.

Every case must:

1. use a deterministic local origin and recording proxy;
2. run once against `httpx==0.28.1`;
3. run once against `eggfetch.compat.httpx`;
4. assert actual proxy-vs-direct routing;
5. avoid public DNS or external network access.

## 1.1 Generic ordinary-domain rows

Use a deterministic host-resolution fixture or test seam that can route arbitrary local hostnames to loopback without public DNS.

Required rows:

| `NO_PROXY` | Target | Expected HTTPX behavior |
| --- | --- | --- |
| `example.test` | `example.test` | direct |
| `example.test` | `www.example.test` | direct |
| `example.test` | `deep.www.example.test` | direct |
| `example.test` | `wwwexample.test` | proxy |
| `example.test` | `notexample.test` | proxy |
| `.example.test` | `example.test` | proxy |
| `.example.test` | `www.example.test` | direct |
| `.example.test` | `deep.www.example.test` | direct |

Use a reserved/test-only hostname namespace rather than a real public domain.

The first implementation run should demonstrate the current candidate failure for at least the ordinary-domain/subdomain row before the parser is corrected.

## 1.2 Preserve the localhost special case

Keep explicit rows showing:

- `NO_PROXY=localhost` -> exact `localhost` bypass;
- `NO_PROXY=localhost` -> a synthetic subdomain such as `foo.localhost` follows actual HTTPX behavior;
- `.localhost` behavior remains pinned separately.

Do not use those rows as evidence for generic domain semantics.

Acceptance criteria:

- generic domain wildcard behavior is proven independently from `localhost`;
- the reference result is observed, not hard-coded from parser internals alone;
- at least one pre-fix candidate failure is documented in the handoff/status notes.

---

# Track 2 — Correct `NoProxy::parse_httpx()` domain semantics without weakening native semantics

## Current defect

The HTTPX compatibility parser currently maps an ordinary plain host to an exact-host rule.

That is correct for special cases such as localhost/IP literals but incorrect for generic HTTPX environment domains.

## Required implementation direction

Keep separate compatibility rule semantics for:

- generic ordinary domain -> bare-domain-plus-subdomain matching;
- leading-dot domain -> subdomain-only matching;
- localhost -> exact localhost;
- IPv4 -> exact IP;
- IPv6 -> exact IP;
- host+port -> HTTPX URL-pattern-equivalent behavior;
- scheme-qualified values -> scheme/host/port URL-pattern-equivalent behavior;
- CIDR-looking IP text -> HTTPX exact-pattern semantics, not native subnet matching.

Possible implementation approaches include:

- a dedicated HTTPX `DomainWildcard` rule for `*example.com` semantics;
- reusing the existing non-exact host matcher only if it exactly matches HTTPX boundary behavior;
- a dedicated compatibility URL-pattern representation.

Choose the smallest implementation that remains readable and testable.

Do not alter `NoProxy::parse()` native behavior merely to make `parse_httpx()` match HTTPX.

## Required matching boundary

Generic bare-domain matching must enforce a domain-label boundary.

For `example.test`:

- `example.test` -> match;
- `www.example.test` -> match;
- `deep.www.example.test` -> match;
- `wwwexample.test` -> no match;
- `example.test.evil` -> no match.

Leading-dot matching must remain subdomain-only.

## Track 2 acceptance criteria

- all Track 1 reference/candidate domain rows agree;
- native Rust true-CIDR tests remain unchanged and green;
- localhost/IP exact behavior remains unchanged;
- no generic networking/proxy architecture is modified;
- no regex dependency or new dependency is added for this correction unless already present and unavoidable.

---

# Track 3 — Pin and correct host+port/default-port semantics

This is a separate acceptance track because HTTPX's URL normalization makes default ports non-obvious.

## 3.1 Build the reference table first

Using `httpx==0.28.1`, record actual route selection for at minimum:

### Ordinary domain with explicit non-default port

- `NO_PROXY=example.test:<origin-port>` against that exact origin port;
- same host against a different port.

### Ordinary domain with explicit default HTTP port

Reference-pin:

```text
NO_PROXY=example.test:80
http://example.test/
http://example.test:80/
```

Both target forms normalize to the same HTTPX URL; the test must record whether the explicit `NO_PROXY` port pattern matches after URL normalization.

### Ordinary domain with explicit default HTTPS port

Reference-pin:

```text
NO_PROXY=example.test:443
https://example.test/
https://example.test:443/
```

### Scheme-qualified defaults

Reference-pin:

```text
NO_PROXY=http://example.test:80
NO_PROXY=https://example.test:443
```

against their normalized target URLs.

### Scheme and port mismatch

Include at least:

- HTTP-qualified pattern against HTTPS target;
- HTTPS-qualified pattern against HTTP target;
- correct scheme but wrong explicit port.

## 3.2 Candidate implementation

Only after the reference rows are captured, correct `HostPortExact`, a new compatibility-specific port rule, or `SchemeHostPort` behavior as required.

Do not blindly substitute `default_port_for_scheme()` for a normalized `None` port unless the reference proves that behavior.

The implementation should model HTTPX `URLPattern.matches()` semantics rather than generic proxy conventions.

## Track 3 acceptance criteria

- default-port behavior is defined by executable HTTPX evidence;
- EggFetch matches every pinned row;
- ordinary host+port and scheme-qualified host+port may differ where HTTPX differs;
- no native `NoProxy::parse()` port semantics are weakened unless an existing native test proves they were already intended to match the compatibility path.

---

# Track 4 — Complete IPv6 and CIDR-looking route evidence

Pass 04 improved this area, but pass 05 should leave the matrix unambiguous.

## IPv6

Where IPv6 loopback is available, prove reference/candidate routing for:

- `NO_PROXY=::1` against `[::1]` target;
- bracketed environment representation if HTTPX accepts it;
- IPv6 + explicit non-default port;
- one nonmatching IPv6 literal;
- one IPv6 prefix-looking/CIDR-looking value that must not gain native subnet semantics.

If the platform lacks IPv6 loopback, use one explicit capability skip shared by reference and candidate and record that skip in qualification status.

## IPv4/CIDR-looking

Pin:

- exact IPv4 literal bypass;
- different IPv4 literal -> proxy;
- `10.0.0.0/8` or another CIDR-looking value against the exact textual/IP host behavior HTTPX produces;
- an address inside the apparent subnet must not bypass merely because it belongs to that subnet.

Do not require external interfaces or routing to private networks. A parser+route fixture may use deterministic host translation as long as the final assertion remains proxy-vs-direct dispatch rather than only calling `should_bypass()`.

Acceptance criteria:

- HTTPX compatibility never performs native CIDR subnet matching;
- native `NoProxy::parse()` retains real CIDR support;
- IPv6 behavior is either fully exercised or explicitly capability-skipped with a reason.

---

# Track 5 — Preserve and link environment-precedence evidence

Do not rewrite working environment logic.

Ensure the final focused matrix explicitly includes or links existing reference/candidate tests for:

- lowercase `http_proxy` over uppercase `HTTP_PROXY`;
- lowercase `https_proxy` where applicable;
- lowercase `no_proxy` over uppercase `NO_PROXY`;
- scheme-less proxy URL normalization;
- `trust_env=False`;
- no applicable proxy variable -> direct route;
- wildcard `NO_PROXY=*` disabling proxy use;
- comma-separated/whitespace/empty-entry parsing.

Acceptance criteria:

- existing passing behavior is not disturbed while domain/port matching is fixed;
- every environment-precedence assertion observes actual routing or effective dispatch, not only object state.

---

# Track 6 — Correct `Timeout.as_dict` allowlist metadata

No `Timeout` behavior change is requested in this track.

## Pinned reference fact

HTTPX 0.28.1 `Timeout` **does have** an `as_dict()` method.

EggFetch compatibility currently exposes `as_dict` as a property returning the same four operational phase values.

Therefore active allowed-difference records must not say:

```text
HTTPX Timeout does not have an as_dict method
```

or describe EggFetch as having an `as_dict` method if the candidate actually exposes a property.

## Required cleanup

Inspect all active `TIMEOUT-AS-DICT-001-*` records.

For each retained entry:

- `reference-behavior` must describe the actual HTTPX 0.28.1 member type/shape;
- `eggfetch-behavior` must describe the actual candidate member type/shape;
- `difference-type` must agree with the oracle result;
- rationale must explain the actual additive/member-shape difference;
- migration impact must be truthful;
- test references must point to a test that exercises the relevant shape.

Where an active record is stale, resolved, duplicated, or logically impossible, remove/move it according to the repository's resolved-difference process.

Do not change `Timeout` semantics merely to make the text easier to describe.

## Required executable/reference checks

Add or retain a small reference/candidate test proving:

- `callable(httpx.Timeout(1).as_dict)` is true;
- calling reference `as_dict()` returns the four phases;
- EggFetch's chosen additive/property form returns the equivalent phase dictionary;
- the API oracle classifies the difference exactly as the allowlist says.

## Track 6 acceptance criteria

- no active timeout difference contains a factually false HTTPX description;
- no active timeout difference contains a factually false EggFetch description;
- API oracle reports 0 stale/unexplained/resolved-in-active entries;
- timeout runtime semantics from pass 04 remain untouched.

---

# Track 7 — Freeze verification tooling before qualification

The previous record points to executable SHA `64a1e2c3...`, while commit `83fbe99...` later changed `scripts/check.sh` in addition to documentation/profile files.

This pass must remove that ambiguity.

## Required sequence

Before selecting the new qualification SHA:

1. finish all code changes;
2. finish all test changes;
3. finish any required `scripts/check.sh` correction;
4. finish allowlist/reference manifest changes;
5. finish any qualification runner changes;
6. commit that complete executable/test/verification tree;
7. declare that commit the qualification candidate SHA.

After that freeze:

- documentation-only evidence/profile commits may follow;
- `scripts/`, `crates/`, `compat/.../allowed-differences.toml`, reference manifests, test fixtures, or qualification runners must not change without restarting qualification at a new SHA.

## `scripts/check.sh` disposition

The current `check.sh` changes that use the pinned reference manifest and required-only downstream artifact runner may remain if they are correct.

Do not revert them solely because they landed after the previous qualification SHA.

Instead:

- validate them as part of the new frozen tree;
- ensure the pinned reference manifest is immutable for HTTPX 0.28.1 unless intentionally regenerated from the exact reference package;
- ensure the downstream step cannot silently convert a required qualification into success when the required artifact manifest is absent.

The routine Tier 1 script may record an optional Tier 2 skip where historically intended, but the final manual qualification record must independently run the required downstream command and record 4/4 required success.

Acceptance criteria:

- final qualification SHA contains the exact verification scripts used to generate the evidence;
- any later qualification-record commit is documentation/profile only;
- no verification script is changed in the same commit that merely claims to record already-completed qualification.

---

# Track 8 — Focused corrective suite

Before freezing the final SHA, run one focused process containing all changed behavior.

At minimum include:

```text
crates/eggfetch-python/tests/compat/test_no_proxy_differential.py
crates/eggfetch-python/tests/compat/test_environment.py
crates/eggfetch-python/tests/compat/test_timeout_reference_differential.py
crates/eggfetch-python/tests/compat/test_config_objects.py
crates/eggfetch-python/tests/compat/test_proxy_differential.py
crates/eggfetch-python/tests/compat/test_native_proxy_tls.py
```

Also include existing proxy parser/core tests covering native CIDR semantics.

Required focused result:

- 0 failures;
- 0 unexpected skips;
- 0 corrective xfails;
- no retry-until-green plugin/mechanism;
- any IPv6 capability skip is explicit and documented.

The focused result must demonstrate the corrected generic-domain/subdomain and default-port cases specifically.

---

# Track 9 — Freeze one final SHA and re-qualify

After Tracks 1–8 are complete, freeze one exact SHA.

If any code, test, fixture, allowlist, reference manifest, or verification script changes after qualification begins, discard the qualification run and start again at the new SHA.

## 9.1 Routine project gate

Run the repository's current routine gate:

```sh
./scripts/check.sh
```

Also preserve explicit visibility for:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

Use the repository's existing test-serialization policy where `./scripts/check.sh` requires it for RSS-sensitive tests. Do not add CI jobs.

## 9.2 Full pinned compatibility — three consecutive clean runs

On the same frozen SHA:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Run three consecutive times with no code/test/fixture changes between runs.

Each run must have:

- 0 failed;
- 0 unexpected skipped;
- 0 corrective xfails;
- no retry plugin masking failures.

Record:

- test count;
- duration;
- warning count;
- any explicit IPv6 capability skip.

The new ordinary-domain and default-port `NO_PROXY` rows must be included in all three aggregate runs.

## 9.3 API oracle

Run the current repository API-manifest comparison against the pinned HTTPX 0.28.1 reference manifest.

Required result:

- 0 unexplained differences;
- 0 stale active differences;
- 0 resolved-in-active entries;
- 0 requires-resolution entries;
- all `TIMEOUT-AS-DICT-*` descriptions match actual reference/candidate behavior;
- `PROXY-HEADERS-001` remains unchanged unless independently affected;
- the four-element socket-option difference remains accurately classified.

## 9.4 Required downstream qualification

Run the existing required-only downstream qualification workflow using the candidate artifact manifest produced from the frozen SHA.

Required packages remain the existing release-blocking set only.

Required result:

- `respx`: pass;
- `httpx-sse`: pass;
- `httpx-auth`: pass;
- `httpx-ws`: pass;
- 0 required failures;
- 0 required errors;
- 0 required skips.

Record the candidate wheel/artifact SHA-256.

Do not broaden the downstream package set.

## 9.5 Documentation and repository checks

Run the existing documentation/example/link checks and any existing all-features rustdoc/doctest gates already used by the repository.

Do not add new documentation infrastructure.

## 9.6 Existing CI

If routine CI naturally runs on the frozen SHA or a documentation-only descendant, record it as supplementary evidence.

Do not expand CI to perform this manual qualification.

---

# Track 10 — Restore Stage C only after every gate is green

After Track 9 succeeds, update:

```text
compat/httpx/0.28.1/profile.toml
plans/httpx-parity-correction-status.md
```

Required profile state:

```toml
stage = "stage-c-qualified"
status = "qualified"
qualified = "<actual qualification date>"
qualification-sha = "<exact frozen SHA>"
```

The qualification SHA must include:

- final `NO_PROXY` implementation;
- final differential tests;
- final allowlist;
- final reference manifest if changed;
- final verification scripts.

It must not point to a prior executable commit that predates verification-tool changes.

## Current status block

Record one unambiguous current evidence section containing:

- exact frozen qualification SHA;
- exact Python/httpx/httpcore/socksio versions;
- focused corrective suite count/result;
- all three full compatibility counts and durations;
- IPv6 capability status;
- API-oracle counts;
- downstream 4/4 result;
- candidate artifact hash;
- routine/documentation/rustdoc result;
- existing CI run/job only if actually observed;
- retained bounded differences.

Keep both previous qualification SHAs (`52b1877...` and `64a1e2c...`) clearly historical rather than deleting them.

---

# Global acceptance criteria

This pass is complete only if every statement below is true:

1. Stage C qualification was reopened before corrections;
2. generic `NO_PROXY=example.test` matches the bare host in both HTTPX and EggFetch;
3. generic `NO_PROXY=example.test` matches subdomains in both HTTPX and EggFetch;
4. generic bare-domain matching does not match near-name hosts without a label boundary;
5. leading-dot domain matching remains subdomain-only where HTTPX does so;
6. `localhost` remains separately pinned to HTTPX's special-case behavior;
7. IPv4 matching remains exact under the HTTPX facade;
8. IPv6 matching is reference-pinned and either exercised or explicitly capability-skipped;
9. CIDR-looking compatibility values do not gain native subnet semantics;
10. native `NoProxy::parse()` retains richer CIDR behavior;
11. non-default host+port behavior matches HTTPX;
12. default HTTP port behavior matches executable HTTPX evidence;
13. default HTTPS port behavior matches executable HTTPX evidence;
14. scheme-qualified host/port behavior matches HTTPX;
15. lowercase/uppercase environment precedence remains correct;
16. scheme-less proxy normalization remains correct;
17. `trust_env=False` remains correct;
18. all `NO_PROXY` acceptance assertions are actual route observations;
19. no external network is required by the corrective tests;
20. every active `Timeout.as_dict` allowlist record describes HTTPX 0.28.1 accurately;
21. every active `Timeout.as_dict` allowlist record describes EggFetch accurately;
22. timeout runtime semantics from pass 04 are unchanged;
23. proxy/UDS/SOCKS/runtime architecture is unchanged;
24. final verification scripts are frozen before qualification begins;
25. focused corrective tests pass in one process;
26. full pinned compatibility passes three consecutive times on one SHA;
27. API oracle is clean;
28. all four required downstream packages pass;
29. final qualification SHA contains the verification tooling used for qualification;
30. only documentation/profile evidence commits follow the frozen SHA;
31. no CI/release/dependency expansion enters the pass.

---

# Explicit rejection criteria

Reject the implementation if any of the following occurs:

- qualification remains marked current while known `NO_PROXY` corrections are being made;
- generic bare domains remain mapped to exact-host matching;
- `localhost` tests are used as the only evidence for generic domain wildcard semantics;
- ordinary and leading-dot domains are collapsed into the same behavior;
- near-match hostnames such as `wwwexample.test` bypass incorrectly;
- EggFetch substitutes default ports without executable HTTPX evidence;
- native true-CIDR matching leaks into `parse_httpx()`;
- native CIDR support is removed to simplify compatibility;
- tests inspect only parser internals rather than route selection;
- public DNS or external internet access is required;
- `Timeout.as_dict` records continue to claim HTTPX lacks `as_dict()`;
- timeout behavior is changed as part of metadata cleanup;
- direct/UDS/SOCKS/proxy/runtime code is refactored without a new failing differential proving necessity;
- `scripts/check.sh` or another qualification runner changes after the frozen SHA without restarting qualification;
- a verification-script change is hidden inside a docs-only qualification-record commit;
- aggregate failures are waived with xfail, retries, timeout inflation, or isolated reruns;
- fewer than three clean aggregate runs are used for qualification;
- required downstream packages are skipped because an artifact manifest is absent;
- qualification SHA points to a commit that predates the final tests/allowlist/verification scripts;
- CI/release machinery is expanded.

---

# Expected implementation footprint

Likely files are limited to a subset of:

```text
compat/httpx/0.28.1/profile.toml
compat/httpx/0.28.1/allowed-differences.toml
compat/httpx/0.28.1/resolved-differences.toml
crates/eggfetch-core/src/proxy.rs
crates/eggfetch-core/tests/proxy_tests.rs
crates/eggfetch-python/tests/compat/native_fixtures.py
crates/eggfetch-python/tests/compat/test_no_proxy_differential.py
crates/eggfetch-python/tests/compat/test_environment.py
crates/eggfetch-python/tests/compat/test_timeout_reference_differential.py
crates/eggfetch-python/tests/compat/test_config_objects.py
scripts/check.sh
plans/httpx-parity-correction-status.md
docs/reference/compatibility.md
```

Changes to transport connection code, streaming/runtime code, SOCKS code, UDS code, or proxy tunnel/TLS code should be treated as a scope violation unless a newly added failing reference/candidate acceptance test proves such a change is necessary.

---

# Handoff sequence

Implement in this order:

1. reopen qualification/profile status;
2. add failing generic bare-domain/subdomain differential rows;
3. add HTTPX reference rows for ordinary/leading-dot/default-port/scheme-port behavior;
4. correct `parse_httpx()` matching rules only as required by those rows;
5. complete IPv4/IPv6/CIDR-looking route evidence;
6. rerun environment-precedence regressions;
7. correct `Timeout.as_dict` allowlist descriptions and stale entries;
8. finalize `scripts/check.sh` and all qualification runners;
9. run the focused corrective suite;
10. commit/freeze the final executable-test-verification SHA;
11. run the routine project gate;
12. run three consecutive full pinned compatibility suites;
13. run the API oracle;
14. run required-only downstream qualification and record artifact hash;
15. run existing documentation/rustdoc checks;
16. restore Stage C qualification against the frozen SHA;
17. record optional existing CI evidence only if it naturally exists.

If any executable code, tests, fixtures, allowlist, reference manifest, or verification script changes after step 10, return to step 10 and restart qualification.

This should be the final closure pass for the HTTPX 0.28.1 parity line. Do not create another broad parity roadmap unless a new public-surface differential independently demonstrates a separate material defect.
