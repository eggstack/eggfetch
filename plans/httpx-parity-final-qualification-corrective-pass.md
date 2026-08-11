# HTTPX 0.28.1 Parity — Final Qualification Corrective Pass

Status: ready for implementation handoff

Date: 2026-08-11

Roadmap: `plans/httpx-parity-completion-roadmap.md`

Previous corrective plans:

- `plans/httpx-parity-corrective-transport-closure.md`
- `plans/httpx-parity-follow-up-corrective-closure.md`

Planning baseline (`main`): `d1da2e1bb72f8541e90a9e47e4c8882bddda3d56`

Previously qualified executable SHA: `ace3782ecf825dede595e2660db4905fb9145b40`

Pinned reference stack:

- `httpx==0.28.1`
- `httpcore==1.0.9`
- `socksio==1.0.0` for SOCKS reference cases

Required designation during implementation: **Stage C candidate — final qualification corrective pass pending**

## Objective

Close the remaining small but material HTTPX 0.28.1 parity/evidence defects discovered after the follow-up transport implementation, then regenerate one truthful exact-SHA Stage C qualification record.

This pass does **not** reopen the transport architecture that is already in good shape. The following are considered preserved unless a new differential fixture demonstrates a concrete regression:

- host-only `local_address` compatibility semantics;
- asynchronous direct DNS resolution/address iteration;
- platform-derived three-element socket option handling;
- UDS -> optional TLS -> Hyper framing/pooling;
- persistent route-keyed SOCKS Hyper clients;
- HTTPX-pinned SOCKS auth method negotiation;
- HTTPX-pinned hostname ATYP behavior in the compatibility facade;
- SOCKS origin TLS and origin-form HTTP semantics;
- `urllib.request.getproxies()`-based environment discovery;
- lower-case environment precedence and scheme-less proxy URL normalization;
- object/signature/stream-type parity work from Phases 2 and 3;
- downstream isolated-runner architecture.

The remaining work is intentionally narrow:

1. correct the invalid reference test and inaccurate rationale for HTTPX four-element `socket_options`;
2. add HTTPX-compatible `https://` HTTP proxy endpoint support;
3. finish the uncovered HTTPX `NO_PROXY` forms and remove compatibility-only CIDR overmatching;
4. add only the missing differential tests needed to support the claimed closure matrix;
5. rebuild the qualification/status record so all current evidence is bound to one executable SHA and old evidence is unambiguously historical.

The target end state is a defensible **Stage C qualified** HTTPX 0.28.1 facade for the documented Python >=3.10 asyncio-supported surface, not unrestricted HTTPX replacement.

---

# Reference facts that supersede prior closure wording

These points are authoritative for this pass and must be pinned by executable/reference tests before final qualification.

## Socket option type contract

HTTPX 0.28.1 defines `socket_options` entries as one of:

```text
(level, option, int)
(level, option, bytes-or-bytearray)
(level, option, None, optlen)
```

The previous reference test used `(level, option, 1, 0)` and concluded that four-element tuples fail at socket use. That is not the valid four-element type exposed by HTTPX/httpcore.

Do not preserve the current rationale that "HTTPX accepts four tuples but they fail because Python setsockopt rejects them" without re-testing the valid `(level, option, None, optlen)` form.

Primary reference:

- `https://raw.githubusercontent.com/encode/httpx/0.28.1/httpx/_transports/default.py`
- `https://raw.githubusercontent.com/encode/httpcore/1.0.9/httpcore/_backends/base.py`

## HTTPS proxy endpoint contract

HTTPX 0.28.1 accepts HTTP proxy URLs whose scheme is either `http` or `https`. `https://` means the connection to the proxy itself is TLS protected. For an HTTPS origin, origin TLS is then layered after CONNECT through that proxy connection.

The current EggFetch native proxy parser accepts `http`, `socks5`, and `socks5h`, but rejects `https`. This is a public transport gap and should be treated as must-close unless a bounded blocker is demonstrated.

Primary reference:

- `https://raw.githubusercontent.com/encode/httpx/0.28.1/httpx/_transports/default.py`
- `https://raw.githubusercontent.com/encode/httpcore/1.0.9/httpcore/_sync/http_proxy.py`

## `NO_PROXY` contract

HTTPX 0.28.1 converts `NO_PROXY` entries into URL-pattern exclusions. Important forms include:

- `*`;
- bare domains;
- leading-dot domains;
- IPv4 literals;
- bare IPv6 literals such as `::1`;
- `localhost`;
- scheme-qualified entries containing `://`;
- port-qualified entries;
- CIDR-looking strings such as `192.168.0.0/16`.

Do not assume CIDR-looking strings receive subnet semantics. HTTPX detects the IP portion, constructs a URL pattern, and URL-pattern matching is host/scheme/port based. Re-pin the actual observable behavior before retaining EggFetch's native CIDR semantics in the compatibility path.

Primary reference:

- `https://raw.githubusercontent.com/encode/httpx/0.28.1/httpx/_utils.py`

---

# Scope firewall

## In scope

- qualification state correction;
- valid HTTPX four-element socket-option reference pinning;
- truthful implementation or bounded classification of `(level, option, None, optlen)`;
- `https://` HTTP proxy endpoint parsing and transport;
- TLS-to-proxy identity/verification;
- HTTP origin through HTTPS proxy;
- HTTPS origin through HTTPS proxy with CONNECT and nested origin TLS;
- proxy auth/header preservation on the HTTPS-proxy path where already exposed by the compatibility `Proxy` object;
- route/pool isolation for HTTP-proxy versus HTTPS-proxy endpoints;
- HTTPX-compatible scheme-qualified `NO_PROXY` entries;
- bare IPv6 `NO_PROXY` entries;
- exact HTTPX behavior for CIDR-looking `NO_PROXY` entries;
- environment-selected HTTPS proxy endpoints;
- targeted differential fixtures for the above;
- missing evidence cases that were previously checked off without direct proof;
- API allowlist/profile/status corrections;
- one exact-SHA routine/full/oracle/downstream qualification run.

## Explicitly out of scope

- redesigning the persistent SOCKS pool;
- changing SOCKS auth/ATYP semantics already pinned against HTTPX 0.28.1 unless a new fixture contradicts them;
- redesigning UDS or the direct connector;
- Trio/AnyIO;
- Python 3.8/3.9 support;
- HTTPX versions other than 0.28.1;
- HTTP/3 parity;
- SOCKS4/4a, BIND, UDP ASSOCIATE;
- proxy chaining, PAC/WPAD, SSH proxies;
- generic arbitrary connector/plugin frameworks;
- private HTTPX module compatibility;
- broad Python `ssl.SSLContext` emulation unrelated to the proxy endpoint requirement;
- arbitrary unsafe `setsockopt`/libc escape hatches;
- CI/release redesign;
- unrelated dependency cleanup or performance work.

## Dependency policy

Do not add a dependency merely to avoid writing a small amount of ordinary safe Rust.

A new dependency is acceptable only if all of these are true:

1. it is small and maintained;
2. it materially simplifies safe transport/TLS behavior;
3. the same result is not already available through Tokio, rustls, hyper, hyper-util, or existing project dependencies;
4. the handoff records direct and transitive dependency impact.

No unsafe Rust should be introduced in EggFetch for this pass.

---

# Track 0 — Reopen qualification before touching behavior

## 0.1 Preserve historical evidence but stop presenting it as current closure

The implementation should begin by making the current state truthful.

Until Tracks 1–5 pass:

- `stage` should be a Stage C candidate state, not qualified;
- `status` should indicate final corrective qualification is pending;
- `ace3782ecf825dede595e2660db4905fb9145b40` should remain recorded as the superseded prior qualification attempt, not erased;
- docs must not state that all advanced transport criteria are currently closed.

Do not invent a second compatibility schema. Use the existing profile/status conventions.

## 0.2 Freeze already-correct transport architecture

The first implementation commit should contain no opportunistic transport rewrite.

Explicitly preserve:

```text
Direct advanced socket path -> Hyper
UDS -> Unix stream -> optional origin TLS -> Hyper
SOCKS -> CONNECT tunnel -> optional origin TLS -> persistent Hyper client
HTTP proxy -> existing forward/CONNECT machinery
```

The only architectural addition authorized is a narrow TLS-to-HTTP-proxy stream layer needed for `https://` proxy endpoints.

## Track 0 acceptance criteria

- current qualification is reopened before new behavioral work is described as complete;
- previous SHA evidence remains available as historical evidence;
- no SOCKS/UDS/direct redesign is mixed into the pass;
- the new plan is referenced as the authoritative remaining corrective handoff.

---

# Track 1 — Correct the four-element `socket_options` reference and classification

## Problem

The current reference pin uses the wrong four-element tuple shape:

```python
(level, option, 1, 0)
```

HTTPX/httpcore instead type the four-element form as:

```python
(level, option, None, optlen)
```

The current active difference and documentation therefore rest on an invalid reference case.

## 1.1 Replace the incorrect reference test

Update `test_httpx_reference_pinning.py` so it does not use the invalid `(level, option, value, optlen)` form as evidence for the valid contract.

Pin these forms explicitly:

1. `(level, option, int_value)`;
2. `(level, option, bytes_value)`;
3. `(level, option, bytearray_value)` if HTTPX/httpcore accepts it on the pinned runtime;
4. `(level, option, None, optlen)`;
5. invalid two-element tuple;
6. invalid five-element tuple;
7. invalid four-element tuple where element three is not `None`;
8. invalid value types.

Constructor acceptance and use-time behavior must be recorded separately.

## 1.2 Do not make OS-specific behavior look universal

The four-argument Python `setsockopt` form can still produce platform/option-specific OS errors. The compatibility question is whether EggFetch represents the HTTPX tuple contract correctly, not whether every arbitrary `(level, option, None, optlen)` succeeds on every kernel.

Use a layered reference strategy:

- pin the HTTPX/httpcore type/constructor contract;
- execute a valid four-tuple through HTTPX on the qualification platform and record the exact observed exception/success class;
- where an option is platform-specific, gate the behavior test explicitly by platform;
- never infer cross-platform behavior from one Linux constant.

## 1.3 Choose the smallest truthful EggFetch outcome

Preferred outcomes, in order:

### Outcome A — safe support

If the valid four-argument operation can be represented with an existing safe Rust API without exposing arbitrary raw FFI:

- accept `(level, option, None, optlen)`;
- translate only the bounded operations that can be expressed safely;
- test the actual effect/error class against HTTPX where deterministic.

### Outcome B — accurate bounded difference

If the form fundamentally requires an arbitrary null-pointer `setsockopt` operation that the project's safe bounded socket abstraction intentionally does not expose:

- keep three-element forms fully supported;
- reject the valid four-element form deterministically;
- replace `TRANSPORT-SOCKET-OPTIONS-004` with an accurate rationale;
- state that HTTPX forwards `(level, option, None, optlen)` to the platform socket API while EggFetch intentionally does not expose arbitrary raw socket-option pointer semantics;
- classify it as a narrow Stage C limitation with positive safe-Rust rationale;
- do not claim the reference itself fails merely because the old invalid tuple failed.

Outcome B is acceptable for Stage C if it is precise and not used to hide a common supported option.

## 1.4 Preserve platform-derived constants

Do not reopen the previous constant bug.

Requirements remain:

- Python compatibility boundary resolves platform constants from `socket`;
- no copied Linux numeric constants in cross-platform code;
- no silent ignore of unsupported options;
- existing common three-element options continue to work.

## Track 1 acceptance criteria

- invalid old four-tuple test is removed/replaced;
- valid `(level, option, None, optlen)` is reference-pinned;
- bytearray behavior is pinned if part of the HTTPX type contract;
- EggFetch either safely supports the valid form or documents the exact valid form as a narrow intentional difference;
- active allowlist rationale matches the real reference behavior;
- no unsafe code is added;
- existing three-element socket option tests remain green.

---

# Track 2 — Add `https://` HTTP proxy endpoint support

## Problem

HTTPX 0.28.1 treats both `http://proxy` and `https://proxy` as HTTP proxy transports. The latter requires TLS to the proxy itself.

EggFetch currently rejects an HTTPS proxy URL in native `parse_proxy_url()`.

This is separate from using an HTTP proxy to reach an HTTPS origin. EggFetch already has CONNECT logic for that case; the missing capability is TLS on the client-to-proxy leg.

## 2.1 Reference-pin the four routing combinations

Use deterministic local fixtures and run each case against both HTTPX 0.28.1 and EggFetch:

1. HTTP origin through `http://` proxy;
2. HTTPS origin through `http://` proxy;
3. HTTP origin through `https://` proxy;
4. HTTPS origin through `https://` proxy.

Record at least:

- proxy connection scheme;
- whether proxy TLS occurred;
- proxy-observed request method/target;
- CONNECT target where applicable;
- origin-observed request target;
- origin response;
- connection count/reuse where deterministic.

## 2.2 Extend proxy URL parsing narrowly

Native proxy URL validation should accept:

- `http`;
- `https`;
- `socks5`;
- `socks5h`.

Do not broaden to arbitrary schemes.

Default ports should be explicit and correct:

- HTTP proxy: 80;
- HTTPS proxy: 443;
- SOCKS: 1080.

Pool/route identity must include proxy scheme so `http://proxy.example:443` and `https://proxy.example:443` cannot collide.

## 2.3 Add a TLS-to-proxy stream layer without replacing the HTTP proxy implementation

Preferred minimal design:

```text
connect proxy TCP
  -> if proxy scheme == https: TLS(proxy host)
  -> existing forward HTTP request path
       OR existing CONNECT path
            -> if origin == https: TLS(origin host) over tunnel
            -> normal origin HTTP framing
```

A small `ProxyIo`/equivalent stream enum that implements `AsyncRead + AsyncWrite` for:

- plain proxy TCP;
- TLS-to-proxy stream;

is acceptable.

Do not introduce a generalized transport framework.

## 2.4 Keep proxy TLS identity separate from origin TLS identity

Required identity semantics:

- proxy TLS SNI/hostname verification uses the proxy host;
- origin TLS after CONNECT uses the origin host;
- proxy certificate failure maps to a proxy/connect/TLS-compatible exception, not an origin HTTP error;
- origin certificate failure remains an origin TLS/connect error.

For HTTPS origin over HTTPS proxy, there are two separate TLS identities and both must be testable.

## 2.5 Maintain one coherent timeout budget

The existing request total/connect ownership must include:

- TCP connect to proxy;
- proxy TLS handshake;
- CONNECT exchange where applicable;
- origin TLS handshake where applicable;
- normal request setup.

Do not grant a fresh full total timeout to each phase.

Add a deterministic delayed-proxy fixture proving the budget cannot multiply across proxy TLS + CONNECT + origin TLS.

## 2.6 Proxy authentication and headers

The compatibility `Proxy` object currently stores:

- URL;
- headers;
- auth;
- `ssl_context`.

The current `_convert_proxy()` mostly collapses that object to a URL and only encodes SOCKS userinfo.

For the HTTPS proxy path, do not silently lose public proxy metadata.

At minimum differential-pin:

- `Proxy(url, auth=(user, pass))` for HTTP/HTTPS proxy schemes;
- explicit proxy headers sent to the proxy;
- URL userinfo behavior if accepted by HTTPX;
- redaction of proxy credentials in errors/debug output.

If the existing native proxy auth/header machinery can already represent these, wire the compatibility object into it rather than inventing a second mechanism.

If a separate mismatch is discovered for ordinary `http://` proxies while doing this, fix it in the same narrow metadata path because it is the same public `Proxy` contract.

## 2.7 `Proxy.ssl_context` boundary

HTTPX passes `Proxy.ssl_context` specifically to proxy TLS. EggFetch currently accepts/stores the compatibility property but does not propagate it into native proxy behavior.

Do **not** turn this pass into full Python SSLContext emulation.

Required decision:

### Minimum must-close

- `https://` proxies work with normal/default trusted proxy certificates;
- proxy TLS verification is enabled by default;
- proxy SNI is correct;
- verification can be tested deterministically without globally disabling TLS validation.

### Optional narrow closure

If a small safe adapter can translate the subset needed for trust roots/verification from the existing compatibility `Proxy.ssl_context`, implement it.

### Acceptable Stage C limitation

If arbitrary Python SSLContext features (custom ciphers, callbacks, client keys, etc.) cannot be translated safely without broad scope:

- retain `Proxy.ssl_context` as an explicit Stage C difference;
- do not claim it is honored;
- make the HTTPS proxy endpoint itself functional independently;
- document the exact boundary.

Do not weaken proxy TLS verification merely to make a local fixture pass.

## 2.8 Pooling and cancellation

Required tests:

- repeated HTTP-origin requests through one HTTPS proxy reuse when keep-alive permits;
- repeated HTTPS-origin requests through one HTTPS proxy reuse the established tunnel/origin connection when normal pool semantics permit;
- cancelling a stalled request does not poison the proxy route;
- follow-up request succeeds under a constrained client;
- HTTP proxy and HTTPS proxy routes cannot share one underlying route key incorrectly.

## Track 2 acceptance criteria

- native parser accepts `https://` HTTP proxies;
- HTTP origin through HTTPS proxy succeeds;
- HTTPS origin through HTTPS proxy succeeds;
- proxy TLS verifies proxy identity;
- nested origin TLS verifies origin identity;
- forward proxy requests use correct absolute-form semantics;
- tunneled origin requests use origin-form semantics;
- proxy authentication/headers are not silently dropped from compatibility `Proxy` objects;
- credentials remain redacted;
- total/connect timeout ownership remains coherent;
- route identity includes proxy scheme;
- no SOCKS/UDS architecture is changed to implement this feature.

---

# Track 3 — Finish exact HTTPX `NO_PROXY` matching

## Problem

The compatibility parser now handles common domain/port/localhost cases, but the follow-up qualification did not directly cover all HTTPX forms. Current `parse_httpx()` also inherits behavior that can differ materially from HTTPX URL-pattern matching.

The native Rust `NoProxy` API may keep broader semantics for native callers. This track is about the **HTTPX compatibility facade**.

## 3.1 Build one table-driven differential fixture

For each `NO_PROXY` input, execute the same request-selection scenario against:

1. `httpx==0.28.1`;
2. `eggfetch.compat.httpx`.

Use local direct and proxy fixtures so the test can tell which route was actually selected.

Required cases:

### Wildcard

- `*`;
- wildcard mixed with other entries.

### Domains

- `example.test` vs bare domain;
- `example.test` vs `api.example.test`;
- `.example.test` vs bare domain;
- `.example.test` vs subdomain;
- near-match `badexample.test`.

### Ports

- `example.test:8080` matching the same port;
- same host different port;
- default HTTP/HTTPS ports.

### Localhost and IP literals

- `localhost`;
- `127.0.0.1`;
- bare `::1`;
- bracketed IPv6 form only if the reference accepts it as an environment entry;
- non-loopback IPv4 literal;
- non-loopback IPv6 literal where the platform supports it.

### Scheme-qualified entries

- `http://example.test`;
- `https://example.test`;
- scheme + port;
- prove that an HTTP-only exclusion does not incorrectly bypass HTTPS and vice versa.

### CIDR-looking entries

At minimum:

- `10.0.0.0/8` against `10.0.0.0`;
- `10.0.0.0/8` against `10.42.1.9`;
- an IPv6 prefix-looking entry if reference parsing accepts it.

Do not assume subnet membership. Record the actual HTTPX behavior.

### Parsing hygiene

- comma-separated combinations;
- surrounding whitespace;
- empty elements;
- lower/uppercase `NO_PROXY`/`no_proxy` collision through `urllib.request.getproxies()`.

## 3.2 Represent HTTPX patterns rather than native policy

Preferred implementation approach:

Keep the native parser's broader semantics if they are useful for Rust callers, but make `NoProxy::parse_httpx()` represent HTTPX pattern behavior exactly.

A small set of HTTPX-specific rule variants is acceptable, for example:

```text
ExactHost
SuffixIncludingBare
SuffixSubdomainsOnly
SchemeHostPort
ExactIp
```

Do not force native CIDR matching onto the HTTPX compatibility path.

Do not copy the whole HTTPX `URLPattern` class into Rust; implement only the observed environment exclusion semantics needed here.

## 3.3 Scheme-qualified exclusions

Entries containing `://` must not be sent through the current CIDR parser.

Parse them as URL-pattern exclusions with:

- optional scheme constraint;
- host constraint;
- optional port constraint.

Reject only inputs that HTTPX rejects or ignores in the same observable way.

## 3.4 Bare IPv6

HTTPX explicitly recognizes bare IPv6 hostnames before turning them into bracketed URL patterns internally.

The compatibility path must correctly handle a `NO_PROXY=::1` style entry without treating the last colon as a host/port separator.

## 3.5 Native behavior remains separate

Do not regress the native `NoProxy::parse()` API merely to match HTTPX.

If native callers currently receive true CIDR matching and localhost-to-loopback alias behavior, that may remain documented native behavior.

The compatibility facade should continue using `parse_httpx()` or an equivalent dedicated compatibility path.

## Track 3 acceptance criteria

- scheme-qualified exclusions match HTTPX;
- bare IPv6 exclusions match HTTPX;
- CIDR-looking entries match HTTPX's actual URL-pattern behavior rather than assumed subnet behavior;
- bare/leading-dot domain behavior remains correct;
- port behavior remains correct;
- lower-case environment precedence remains correct;
- native `NoProxy` semantics are not needlessly reduced;
- every listed public compatibility case is differential, not self-asserting.

---

# Track 4 — Close the remaining evidence gaps without rebuilding the entire matrix

The previous pass added useful UDS and SOCKS tests. Do not repeat already-proven cases merely to increase counts.

This track exists because the prior plan marked several cases complete without direct executable evidence in the new files.

## 4.1 Audit previous checked boxes against actual tests

Create a short matrix in the handoff/closure record mapping each claimed transport criterion to a concrete test name.

For any checked criterion with no concrete executable test, do one of:

1. add the smallest deterministic test;
2. point to an existing test elsewhere that actually proves it;
3. uncheck/reword the claim if it was stronger than the evidence.

Do not manufacture test-count parity by duplicating cases.

## 4.2 UDS remaining proof

The new UDS suite already proves successful HTTP reuse and HTTPS/chunked reuse. Confirm whether the repository has concrete tests for each of these claimed items:

- sync host-only `local_address`;
- async host-only `local_address`;
- different UDS path isolation;
- cancellation + constrained follow-up;
- request streaming if claimed;
- partial response close if claimed;
- source-port behavior if claimed.

Add only missing tests needed to support documentation/checklist claims.

If a behavior is not required for the Stage C claim, remove the overbroad claim rather than broadening implementation.

## 4.3 SOCKS remaining proof

The new SOCKS differential suite already covers key auth/ATYP/reuse/cancellation cases. Confirm concrete tests for each claim that remains in final docs:

- wrong credentials;
- CONNECT rejection;
- malformed/truncated handshake;
- IPv6 literal where available;
- unresolved hostname behavior;
- coherent multi-phase total timeout;
- different proxy endpoint isolation;
- different auth identity isolation;
- direct/HTTP-proxy/SOCKS isolation;
- credential redaction;
- environment-selected SOCKS if documented as supported.

Again, add only the missing cases required by the final claim.

## 4.4 HTTPS proxy differential suite

Track 2's tests become part of the transport differential qualification suite.

The minimum public proxy matrix at closure is:

| Origin | Proxy endpoint | Required |
| --- | --- | --- |
| HTTP | HTTP | yes |
| HTTPS | HTTP | yes |
| HTTP | HTTPS | yes |
| HTTPS | HTTPS | yes |
| HTTP/HTTPS | SOCKS5 | already qualified |

## 4.5 Environment selection with HTTPS proxy URL

Once `https://` proxy endpoints work, add a differential environment case proving that a selected environment proxy value may itself use `https://` when HTTPX accepts it.

Do not confuse `HTTPS_PROXY` (proxy used for HTTPS destinations) with an `https://` proxy URL (TLS to proxy). They are independent dimensions and should be tested separately.

## Track 4 acceptance criteria

- every final transport claim maps to a concrete test;
- no checklist item is marked complete solely because related code exists;
- no duplicate test explosion;
- HTTPS-proxy matrix is differential;
- remaining UDS/SOCKS tests are only those needed to substantiate final documentation.

---

# Track 5 — API profile, allowed differences, and documentation correction

## 5.1 Fix socket-option difference record

If the valid four-element form remains intentionally unsupported, update the existing active difference so it states the real contract:

Reference:

```text
HTTPX/httpcore accept (level, option, None, optlen) and forward it to the platform socket API.
```

Candidate:

```text
EggFetch supports bounded safe three-element socket operations but intentionally rejects arbitrary null-pointer four-argument socket operations.
```

Do not retain the claim that the valid HTTPX form fails because Python rejects it.

If the valid form is implemented safely, remove the now-resolved active difference and move it into the resolved ledger using the repository's normal process.

## 5.2 HTTPS proxy difference

The goal is to resolve the missing `https://` proxy endpoint capability, not add it to the allowlist.

If a stop condition prevents implementation, record a must-close blocker rather than silently classifying the entire HTTPS proxy feature intentional.

A narrower custom `Proxy.ssl_context` limitation may remain intentional if accurately documented and already outside the Stage C claim.

## 5.3 `NO_PROXY` documentation

Document exact compatibility semantics discovered from the differential fixture, especially:

- scheme-qualified entries;
- IPv6 form;
- domain-leading-dot distinction;
- CIDR-looking behavior.

Keep native Rust `NoProxy` behavior separately documented if it intentionally differs.

## 5.4 Correct the compatibility matrix

The compatibility table must distinguish:

- HTTP proxy endpoint support;
- HTTPS proxy endpoint support;
- HTTPS CONNECT tunneling;
- SOCKS proxy support;
- custom proxy SSL context support if still bounded.

Do not label generic "HTTP proxy: Yes" if an important public endpoint scheme remains missing.

## 5.5 Preserve bounded Stage C scope

Final docs must continue to state that the claim excludes:

- Trio/AnyIO;
- Python 3.8/3.9;
- private HTTPX modules;
- other HTTPX versions;
- any retained explicitly allowed Stage C differences.

## Track 5 acceptance criteria

- active difference rationale is technically correct;
- no unresolved HTTPS-proxy public gap is hidden as intentional;
- compatibility docs distinguish proxy endpoint TLS from CONNECT-to-origin TLS;
- `NO_PROXY` docs match reference fixtures;
- native-versus-compat differences are explicit;
- Stage C scope does not expand.

---

# Track 6 — One clean exact-SHA qualification

Do not set the profile back to qualified until Tracks 1–5 are complete.

## 6.1 Freeze one executable qualification SHA

Create the final executable commit containing all code and executable-test changes.

Record its full 40-character SHA before documentation-only qualification edits.

Every result below must be from that exact executable tree.

## 6.2 Targeted corrective tests

Run and record exact counts for at least:

```sh
python -m pytest \
  crates/eggfetch-python/tests/compat/test_httpx_reference_pinning.py \
  crates/eggfetch-python/tests/compat/test_environment.py \
  crates/eggfetch-python/tests/compat/test_socks_transport.py \
  crates/eggfetch-python/tests/compat/test_uds_transport.py \
  <new HTTPS proxy differential test file> \
  -q --strict-markers
```

Also run any targeted Rust proxy/socket tests modified by this pass.

Required result:

- zero failures;
- no unexpected skips;
- platform-conditioned skips must be explicitly justified.

## 6.3 Routine repository validation

Run on the same executable SHA:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
./scripts/check.sh
```

No known failure may be waived as flaky for qualification.

## 6.4 Full pinned compatibility suite

Run exactly against the pinned reference environment:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Record:

- exact pass/fail/skip/xfailed count;
- warnings separately;
- Python version;
- `httpx`, `httpcore`, and `socksio` versions.

Required result:

- zero failures;
- zero unexpected skips.

## 6.5 API oracle

Regenerate and compare the candidate manifest using the repository's established commands.

Required result:

- zero unexplained differences;
- zero stale allowed entries;
- zero resolved-in-active entries;
- zero requires-resolution entries.

Record the final active allowed-difference count and category breakdown.

Do not reuse the previous "76 allowed" number if this pass changes the contract.

## 6.6 Downstream qualification

Use the **current** isolated downstream runner contract in the repository.

The previous documentation now requires an artifact manifest. Inspect the script help/current docs and record the exact command actually used, for example:

```sh
python scripts/run_downstream_compat.py \
  --artifact-manifest /path/to/artifact-manifest.json \
  --required-only
```

Do not use an obsolete no-argument command copied from an older plan.

Required result:

- all release-blocking packages pass;
- informational failures are listed separately and tied to excluded/private/old-version behavior;
- no unexplained public Stage C failure remains.

## 6.7 CI evidence policy

Do not add or expand CI for this pass.

If the existing CI runs on the final executable SHA, record the run and result.

If there is no CI run for the exact SHA:

- state that clearly;
- do not recycle CI evidence from an older executable SHA as though it validated the new tree;
- rely on the exact-SHA local qualification commands above as the qualification evidence if that remains the repository's established policy.

Absence of a new CI run is not a reason to redesign the workflow.

## 6.8 Rewrite the current evidence block cleanly

`plans/httpx-parity-correction-status.md` currently mixes old detailed counts and CI evidence with the new qualification designation.

At closure, structure it so there is exactly one clearly labeled **current qualification** block containing:

- executable SHA;
- documentation SHA if different;
- date;
- reference versions;
- targeted corrective result;
- routine validation result;
- full pinned suite result;
- API oracle result;
- downstream result;
- CI result or explicit "not run for this SHA";
- final allowed-difference count;
- final Stage C designation.

Older evidence may remain below under explicitly historical headings, but must not be interleaved with the current evidence.

## 6.9 Restore qualification only after all gates pass

Only after 6.1–6.8 succeed:

- set `status = "qualified"`;
- set the stage to the repository's qualified Stage C value;
- update `qualification-sha` to the new executable SHA;
- update the qualification date;
- mark this plan complete;
- retain `ace3782...` and `40beeec...` as superseded historical evidence.

## Track 6 acceptance criteria

- one executable SHA backs all current test/oracle/downstream results;
- current evidence does not contain stale counts from older SHAs;
- full pinned suite has zero failures;
- oracle is clean;
- release-blocking downstream set is clean;
- CI evidence is truthful and exact-SHA-specific if present;
- no new CI/release machinery is added;
- profile is restored to qualified only after all criteria pass.

---

# Global acceptance criteria

The corrective pass is complete only when every item below is true:

- [ ] current premature qualification is reopened while implementation is active;
- [ ] the invalid `(level, option, 1, 0)` four-tuple reference case is no longer used to characterize HTTPX's valid four-element socket contract;
- [ ] `(level, option, None, optlen)` is reference-pinned;
- [ ] four-element socket support is either safely implemented or accurately retained as a narrow safe-Rust Stage C difference;
- [ ] `https://` HTTP proxy URLs are accepted by EggFetch's HTTPX-compatible transport;
- [ ] HTTP origin through HTTPS proxy succeeds;
- [ ] HTTPS origin through HTTPS proxy succeeds;
- [ ] proxy TLS identity/SNI is the proxy host;
- [ ] nested origin TLS identity/SNI is the origin host;
- [ ] proxy auth/headers exposed by the compatibility `Proxy` object are not silently dropped on the corrected path;
- [ ] proxy credentials remain redacted;
- [ ] HTTP and HTTPS proxy endpoint routes remain isolated;
- [ ] proxy TLS/CONNECT/origin TLS consume one coherent timeout budget;
- [ ] scheme-qualified `NO_PROXY` entries match HTTPX 0.28.1;
- [ ] bare IPv6 `NO_PROXY` entries match HTTPX 0.28.1;
- [ ] CIDR-looking `NO_PROXY` entries match actual HTTPX pattern behavior, not assumed subnet behavior;
- [ ] existing bare-domain/leading-dot/port/localhost/wildcard matching remains correct;
- [ ] native `NoProxy` semantics are not unnecessarily weakened;
- [ ] every final UDS/SOCKS/proxy claim maps to a concrete executable test;
- [ ] missing evidence is filled narrowly rather than rebuilding the transport test corpus;
- [ ] active difference records describe the true reference contract;
- [ ] routine validation passes on one final executable SHA;
- [ ] full pinned HTTPX suite passes with zero failures on that SHA;
- [ ] API oracle is clean on that SHA;
- [ ] release-blocking downstream qualification passes on artifacts built from that SHA;
- [ ] current status/evidence block contains only results attributable to that SHA;
- [ ] previous qualification attempts remain clearly historical;
- [ ] no CI/release redesign, Trio/AnyIO, Python 3.8/3.9, private-module, or unrelated transport scope is introduced;
- [ ] final profile returns to Stage C qualified only after all items above close.

---

# Rejection criteria

Reject the implementation as incomplete if any of the following is true:

- the old invalid four-tuple is still cited as proof that HTTPX rejects its valid four-element socket form;
- a valid `(level, option, None, optlen)` is silently rewritten into a three-element tuple;
- unsafe Rust/libc is introduced solely to chase arbitrary socket-option parity;
- `https://` proxy URLs are merely accepted at construction but fail before TLS-to-proxy is attempted;
- HTTPS-proxy testing only checks a parser instead of a live local TLS proxy;
- proxy TLS verification is disabled to make tests pass;
- proxy SNI uses the origin hostname;
- origin TLS after CONNECT uses the proxy hostname;
- HTTP-over-HTTPS-proxy sends origin-form instead of the reference's forward-proxy form;
- HTTPS-over-HTTPS-proxy fails to use CONNECT before origin TLS;
- proxy metadata from `Proxy(...)` is dropped without an explicit bounded difference;
- HTTP and HTTPS proxy pool identities can collide;
- each proxy handshake phase receives a fresh full total timeout;
- scheme-qualified `NO_PROXY` entries still fall into CIDR parsing;
- bare `::1` is parsed as host/port incorrectly;
- HTTPX compatibility mode applies true CIDR subnet bypass when the reference does not;
- existing native `NoProxy` behavior is removed solely to simplify compat matching;
- a missing public behavior is reclassified intentional only to get a green oracle;
- the follow-up plan is marked complete without concrete tests for the final public claims;
- old `1384`, `1450`, `1475`, `121`, or `76` counts are reused without rerunning and recording the new actual counts;
- CI from an older SHA is described as current executable validation;
- qualification is restored while any public corrective test is failing or unexplained;
- unrelated networking architecture is rewritten;
- CI/release infrastructure is expanded.

---

# Stop conditions

Stop and write a bounded blocker report rather than expanding scope if any of these occur:

1. valid HTTPX four-element socket behavior requires unsafe arbitrary `setsockopt` semantics and no existing small safe abstraction can express it;
2. TLS-to-proxy cannot be layered onto the existing HTTP proxy stream without replacing the entire proxy transport subsystem;
3. nested TLS for HTTPS-origin-over-HTTPS-proxy cannot be represented safely with the existing Tokio/rustls stack;
4. deterministic proxy TLS verification requires full arbitrary Python SSLContext emulation rather than a bounded trust/verification mechanism;
5. a `NO_PROXY` form depends on platform-specific `urllib` behavior that cannot be made deterministic for the project's supported compatibility environment;
6. a downstream failure is proven to require private HTTPX modules or excluded concurrency backends.

A blocker report must include:

- exact HTTPX reference scenario;
- exact EggFetch behavior;
- missing primitive;
- affected acceptance criterion;
- smallest feasible follow-up;
- why the issue cannot be completed without violating this pass's scope firewall.

Do not downgrade a blocker to intentional merely to restore `qualified` status.

---

# Suggested implementation sequence

Keep commits narrow and bisectable. A suitable sequence is:

1. `docs: reopen HTTPX qualification for final corrective pass`
2. `test: correct socket option and NO_PROXY reference pins`
3. `fix: add TLS HTTP proxy endpoint support`
4. `fix: align HTTPX NO_PROXY pattern edge cases`
5. `test: close remaining transport evidence gaps`
6. `docs: correct HTTPX transport differences and compatibility claims`
7. `docs: record exact-SHA final HTTPX qualification`

Combining adjacent commits is acceptable if the result remains reviewable.

Do not combine unrelated cleanup.

---

# Required implementation handoff report

The implementing agent must report all of the following:

- planning baseline SHA;
- implementation starting SHA if different;
- final executable SHA;
- final documentation-only SHA if different;
- Python version;
- exact `httpx`, `httpcore`, and `socksio` versions;
- valid four-element socket-option reference result;
- final four-element socket-option decision and rationale;
- confirmation that old invalid four-tuple rationale was removed;
- HTTPS proxy parser result;
- HTTP-origin-over-HTTPS-proxy result;
- HTTPS-origin-over-HTTPS-proxy result;
- proxy TLS SNI/verification result;
- origin TLS SNI/verification result;
- proxy authentication/header propagation result;
- proxy credential redaction result;
- HTTP/HTTPS proxy endpoint route-isolation result;
- proxy multi-phase timeout-budget result;
- scheme-qualified `NO_PROXY` matrix;
- IPv4/IPv6 `NO_PROXY` matrix;
- CIDR-looking `NO_PROXY` reference/candidate result;
- domain/leading-dot/port/wildcard regression result;
- native `NoProxy` behavior preservation confirmation;
- UDS/SOCKS evidence audit mapping final claims to tests;
- targeted corrective test count;
- `cargo fmt --all -- --check` result;
- clippy result;
- `cargo test --workspace` result;
- `./scripts/check.sh` result;
- full pinned compatibility result;
- API oracle counts;
- final active allowed-difference count and classifications;
- exact downstream runner command and artifact hashes/manifest path;
- downstream release-blocking result;
- informational downstream exclusions/failures;
- CI result for the final executable SHA, or explicit statement that no such CI run exists;
- confirmation that no old CI result is presented as current evidence;
- confirmation that no unsafe Rust, CI/release redesign, Trio/AnyIO, Python 3.8/3.9, private-module, or unrelated architecture scope was added;
- final compatibility designation.

The intended closure is a narrow final correction: preserve the now-solid direct/UDS/SOCKS architecture, close the remaining HTTP proxy and environment-pattern contract gaps, correct the socket-option reference record, and leave the repository with one auditable Stage C qualification bound to one executable SHA.
