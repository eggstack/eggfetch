# HTTPX 0.28.1 Parity — Follow-up Corrective Closure

Status: ready for implementation handoff

Date: 2026-08-11

Roadmap: `plans/httpx-parity-completion-roadmap.md`

Previous corrective plan: `plans/httpx-parity-corrective-transport-closure.md`

Current `main` at planning time: `b170066e17eb14a3e4d6cb8699f8f0aa79920b62`

Primary corrective implementation commit: `cdbb4c4e6bd3f65ccc81bb09fdaa238d2a78514d`

Follow-up error-classification commit: `b170066e17eb14a3e4d6cb8699f8f0aa79920b62`

Historical pre-corrective qualification candidate: `40beeec09f3e88db8901f39388da665c47ab84f6`

Pinned reference: `httpx==0.28.1`

Compatibility designation during this pass: **Stage C candidate — corrective transport qualification pending**

## Objective

Finish the remaining HTTPX 0.28.1 transport parity work after the first corrective transport pass without reopening the already-corrected direct-connector and UDS architecture.

The previous pass landed the right structural changes:

- HTTPX-style host-only `local_address` values are translated to an ephemeral native bind;
- advanced direct DNS resolution is asynchronous and iterates compatible addresses;
- socket options are classified using the running Python platform's `socket` constants rather than copied Linux numbers;
- UDS now enters Hyper through a `UnixStream` connector and supports optional origin TLS;
- SOCKS now produces a connected stream that enters Hyper rather than using bespoke HTTP framing for production requests;
- local SOCKS DNS failure no longer redirects to loopback;
- direct/SOCKS error classification is separated correctly.

Those architectural corrections should remain intact.

The remaining work is narrower:

1. make SOCKS connection ownership persistent so compatible requests can reuse pooled connections;
2. re-pin and correct SOCKS authentication and DNS behavior against `httpx==0.28.1`/its pinned httpcore stack;
3. finish exact HTTPX proxy-environment and `NO_PROXY` semantics;
4. decide and document the bounded `socket_options` representation gap rather than leaving it implicit;
5. add the missing executable transport differential matrix, especially successful HTTPS-over-UDS and reuse/cancellation cases;
6. produce one clean exact-SHA qualification and only then return the profile to `qualified`.

This is a closure plan, not another architectural rewrite.

---

# Scope firewall

## In scope

- persistent SOCKS Hyper client/pool ownership;
- SOCKS authentication-method negotiation parity;
- `socks5://` / `socks5h://` destination-address behavior as actually exposed by HTTPX 0.28.1;
- SOCKS origin TLS, framing, keep-alive, cancellation, route isolation, and same-route reuse;
- HTTPX environment proxy precedence and normalization;
- uppercase/lowercase proxy-variable precedence;
- scheme-less environment proxy URLs if accepted by HTTPX;
- `NO_PROXY` exact/domain-suffix/port/localhost/wildcard behavior;
- `trust_env=False` and explicit `proxy=` precedence;
- the public `socket_options` tuple forms actually exposed by HTTPX 0.28.1;
- UDS success-path TLS/framing/reuse tests against the already-refactored connector;
- local deterministic fixtures;
- compatibility profile/ledger/docs updates;
- final exact-SHA qualification.

## Explicitly out of scope

- another UDS or direct-connector redesign unless a test proves a real defect;
- Trio/AnyIO;
- Python 3.8/3.9 support;
- HTTPX versions other than 0.28.1;
- SOCKS4/4a;
- SOCKS BIND/UDP ASSOCIATE;
- proxy chaining, PAC/WPAD, SSH proxies, Tor control APIs;
- generalized connector/plugin frameworks;
- private HTTPX module compatibility;
- CI/release redesign;
- unrelated performance cleanup;
- replacing the native Rust networking engine with httpcore, curl, requests, or another side stack.

---

# Correct a mistake in the previous plan before implementation

The previous corrective plan required a fixture in which credentials are configured but the SOCKS server selects `NO AUTH`, with EggFetch proceeding without RFC 1929.

Do **not** implement that requirement blindly.

The pinned HTTPX 0.28.1 stack delegates SOCKS negotiation to httpcore. Re-pin this behavior first. Current source evidence indicates:

- without configured credentials, the client offers only `NO AUTH`;
- with configured credentials, the client offers only `USERNAME/PASSWORD`;
- a proxy-selected method outside the offered method set is an error.

Therefore the follow-up implementation should match the reference result, even where that supersedes wording in `plans/httpx-parity-corrective-transport-closure.md`.

The previous plan remains historical context; this file is authoritative for the remaining work.

---

# Track 0 — Freeze the follow-up baseline and preserve corrected architecture

## 0.1 Starting SHA

Implementation starts from:

`b170066e17eb14a3e4d6cb8699f8f0aa79920b62`

If `main` advances before implementation begins, inspect the delta and record whether it changes any file in:

- `crates/eggfetch-core/src/transport/`;
- `crates/eggfetch-core/src/pipeline.rs`;
- `crates/eggfetch-core/src/client.rs`;
- `crates/eggfetch-core/src/proxy.rs`;
- `crates/eggfetch-python/src/proxy.rs`;
- `crates/eggfetch-python/src/conversion.rs`;
- `crates/eggfetch-python/src/client.rs`;
- `crates/eggfetch-python/src/async_client.rs`;
- compatibility tests/docs/profile.

## 0.2 Preserve already-correct architecture

Do not undo these properties unless a differential test demonstrates a defect:

- UDS -> connected Unix stream -> optional TLS -> Hyper;
- SOCKS -> SOCKS CONNECT tunnel -> optional origin TLS -> Hyper;
- default direct connections remain on the standard connector;
- advanced direct connections use async resolution and compatible-address iteration;
- socket option classification is semantic/platform-derived at the Python boundary.

## 0.3 Qualification remains open

Keep:

`status = "corrective-transport-pending"`

until Track 6 closes.

Do not set a new `qualification-sha` from a partial test run.

## Track 0 acceptance criteria

- starting SHA is recorded;
- no already-correct architecture is replaced without reference evidence;
- old `40beeec...` evidence remains historical;
- profile remains unqualified until final closure.

---

# Track 1 — Make SOCKS connection ownership persistent and pool-compatible

## Problem

At `b170066...`, `send_socks_request()` constructs a new `SocksConnector` and a new Hyper `Client` inside every request.

That fixes HTTP framing but defeats cross-request connection reuse because the Hyper pool is dropped at the end of the request path.

HTTPX/httpcore owns a persistent SOCKS connection pool per transport/client. The EggFetch facade must provide equivalent lifecycle semantics for compatible routes.

## 1.1 Move SOCKS client construction out of the per-request path

Create persistent SOCKS transport/client state owned by `ClientInner` or the smallest existing route-specific transport cache.

Preferred shape:

```text
ClientInner
  direct client
  optional UDS client
  SOCKS clients keyed by effective SOCKS route configuration
```

Do not create a global singleton.

Do not create a new generic transport framework if a small keyed cache is sufficient.

## 1.2 Define the SOCKS route identity explicitly

At minimum, materially different routes must not share one Hyper client/pool:

- SOCKS proxy endpoint host/port;
- proxy scheme if it changes connection semantics;
- authentication identity;
- TLS policy/configuration where it changes origin connection behavior;
- any future setting that changes SOCKS handshake semantics.

Do not place raw passwords into debug-visible/displayable keys.

If a hashed/redacted auth fingerprint is unnecessary because the key can use an opaque internal identity, prefer the smaller design.

## 1.3 Reuse within a compatible route

For the same SOCKS proxy route and same origin, repeated keep-alive-compatible requests must be able to reuse the established pooled connection according to the normal Hyper pool behavior.

Do not force every request to repeat the SOCKS handshake.

## 1.4 Preserve route isolation

Prove no reuse across:

- direct vs SOCKS;
- HTTP proxy vs SOCKS;
- SOCKS proxy endpoint A vs B;
- materially different auth identities;
- route variants shown by the pinned reference to be semantically distinct.

## 1.5 Cancellation and close ownership

A cancelled in-flight SOCKS connect/handshake/TLS/request must not poison the persistent route client.

With a constrained pool/ownership setup:

1. start a stalled SOCKS request;
2. cancel it;
3. issue a follow-up request on the same compatible client/route;
4. prove the follow-up succeeds.

Closing the EggFetch client must release all persistent SOCKS route clients and their connections.

## Track 1 acceptance criteria

- no Hyper SOCKS client is created per ordinary request;
- same-route requests can reuse a pooled connection;
- handshake count proves reuse when the origin keeps the connection alive;
- route separation is explicit and tested;
- cancellation followed by a constrained request succeeds;
- client close releases persistent SOCKS route state;
- credentials do not appear in logs/debug/displayable route keys.

---

# Track 2 — Re-pin and correct SOCKS authentication and destination-address semantics

This track is reference-first. Do not infer behavior from scheme names or from generic SOCKS conventions.

## 2.1 Build a wire-recording HTTPX 0.28.1 SOCKS fixture

The local fixture must record, for each connection:

- offered SOCKS authentication methods;
- selected authentication method;
- whether RFC 1929 bytes follow;
- username/password bytes where configured;
- CONNECT ATYP;
- CONNECT destination bytes;
- CONNECT destination port;
- request count / connection count;
- for plain HTTP, the origin request target observed after CONNECT.

Run the same fixture against:

1. `httpx==0.28.1` with its normal optional SOCKS dependency installed;
2. `eggfetch.compat.httpx`.

## 2.2 Match credential negotiation exactly

Pin at minimum:

- no credentials;
- username/password credentials;
- username only if accepted;
- empty password if accepted;
- percent-encoded username/password;
- wrong credentials;
- proxy chooses a method not offered by the client;
- proxy returns no acceptable method.

Current expected reference behavior must be verified, not assumed:

- no credentials -> offer only no-auth;
- credentials -> offer only username/password;
- selected method must match the offered set.

Do not advertise both methods merely because both are technically possible if HTTPX does not.

## 2.3 Re-pin `socks5://` versus `socks5h://`

For both schemes, test with:

- hostname destination;
- IPv4 literal;
- IPv6 literal where available;
- unresolved hostname.

Record the actual CONNECT ATYP/destination bytes sent by HTTPX 0.28.1.

Do not retain the current “`socks5` = local DNS, `socks5h` = remote DNS” split merely because it is conventional terminology if the pinned HTTPX/httpcore implementation does not distinguish them that way.

If both schemes are accepted by HTTPX but normalize to the same httpcore behavior, EggFetch should do the same for the compatibility facade while the native API may retain a documented lower-level distinction if useful and non-conflicting.

## 2.4 Preserve no-loopback safety

Regardless of reference detail, unresolved names must never be redirected to loopback or another guessed address.

If the pinned reference passes the domain name to the SOCKS proxy instead of resolving locally, EggFetch should send the same domain form rather than performing a local fallback.

## 2.5 Origin request semantics

After CONNECT succeeds, the destination origin must receive origin-form HTTP targets:

`/path?query`

not forward-proxy absolute-form URLs.

This should already be satisfied by Hyper, but prove it in the fixture.

## 2.6 TLS origin identity

HTTPS-over-SOCKS must prove:

- SNI/server name is the origin host;
- certificate verification is performed for the origin, not the proxy;
- HTTP framing is owned by Hyper after TLS;
- keep-alive reuse is possible after successful TLS.

## Track 2 acceptance criteria

- offered auth methods match HTTPX 0.28.1 exactly;
- selected-method handling matches the reference;
- credential encoding edge cases match the reference or are explicitly excluded with positive evidence;
- `socks5`/`socks5h` CONNECT ATYP behavior is pinned and matched;
- unresolved names never target loopback;
- origin-form request target is observed at the destination;
- HTTPS uses origin TLS identity;
- no legacy SOCKS HTTP parser/framing path is used in production.

---

# Track 3 — Finish exact HTTPX environment proxy and `NO_PROXY` semantics

The previous pass fixed the architectural issue of one global environment proxy. The remaining work is exact policy compatibility.

## 3.1 Use HTTPX's actual environment interpretation as the reference

HTTPX 0.28.1 derives environment proxy state through its environment helper path, which relies on Python/stdlib proxy environment behavior.

Build a subprocess-based or carefully isolated differential fixture so process-global environment state cannot leak between tests.

For every case, run against HTTPX and EggFetch.

## 3.2 Uppercase/lowercase precedence

Pin and match collisions such as:

- `HTTP_PROXY=A`, `http_proxy=B`;
- `HTTPS_PROXY=A`, `https_proxy=B`;
- `ALL_PROXY=A`, `all_proxy=B`;
- `NO_PROXY=A`, `no_proxy=B`.

Current source evidence indicates lowercase values take precedence when both cases are present. Verify this directly and implement the observed rule.

Do not keep the current uppercase-first fallback if the differential fixture disagrees.

## 3.3 Scheme-less proxy environment values

Pin values such as:

- `HTTP_PROXY=127.0.0.1:8080`;
- `HTTPS_PROXY=127.0.0.1:8080`;
- `ALL_PROXY=127.0.0.1:1080`.

If HTTPX normalizes a missing scheme to `http://`, EggFetch must do the same in the compatibility facade.

Do not weaken the native Rust proxy parser merely to accept ambiguous values globally; normalize at the compatibility/environment boundary.

## 3.4 Scheme-specific precedence

Prove with distinct local proxy fixtures:

- HTTP destination selects `HTTP_PROXY` over `ALL_PROXY`;
- HTTPS destination selects `HTTPS_PROXY` over `ALL_PROXY`;
- `ALL_PROXY` is fallback when scheme-specific value is absent;
- one scheme's proxy is not used for the other scheme merely because it was configured first.

## 3.5 `NO_PROXY` matching semantics

Differentially pin at minimum:

- exact hostname;
- bare domain entry;
- leading-dot domain entry;
- subdomain;
- bare domain versus subdomain distinction;
- host + port;
- `localhost`;
- `127.0.0.1`;
- `[::1]` where supported;
- wildcard `*`;
- comma-separated combinations;
- whitespace handling;
- uppercase/lowercase variable collision.

Current EggFetch logic likely differs from HTTPX in at least these cases:

- bare `example.com` should match both the bare domain and subdomains if the reference does;
- `.example.com` may match subdomains while excluding the bare domain.

Do not preserve the current matcher if the reference fixture disproves it.

## 3.6 Explicit proxy and `trust_env`

Pin and match:

- explicit `proxy=` + environment values;
- explicit `proxy=None` semantics where observable;
- per-request proxy disable/override if exposed by the facade;
- `trust_env=False` ignoring every proxy and `NO_PROXY` environment variable.

## 3.7 SOCKS through environment

If HTTPX accepts a SOCKS URL in `ALL_PROXY`/scheme-specific environment state for this pinned surface, prove and match it.

Do not special-case environment SOCKS differently from explicit SOCKS after route resolution.

## Track 3 acceptance criteria

- lowercase/uppercase precedence matches the pinned reference;
- scheme-less proxy values are normalized exactly as HTTPX does;
- HTTP/HTTPS/ALL proxy fallback is request-scheme-correct;
- `NO_PROXY` bare-domain and leading-dot semantics match HTTPX;
- port/localhost/loopback/wildcard cases match;
- explicit proxy precedence matches;
- `trust_env=False` ignores all proxy environment state;
- SOCKS environment routes use the same persistent SOCKS machinery;
- tests isolate process-global environment state deterministically.

---

# Track 4 — Close the remaining `socket_options` public representation question

The platform-constant bug is fixed. Do not reopen that implementation unless tests fail.

The remaining question is public tuple-shape coverage.

## 4.1 Pin all tuple forms accepted by HTTPX 0.28.1

Test at minimum:

- `(level, option, int_value)`;
- `(level, option, bytes_value)`;
- four-element forms exposed by HTTPX/httpcore type contracts, including `(level, option, None, optlen)` if runtime behavior supports it;
- invalid tuple lengths;
- invalid values.

Constructor acceptance alone is insufficient if use-time behavior differs.

## 4.2 Choose one truthful Stage C outcome

If a four-element option can be implemented safely using existing Tokio/std APIs for the bounded supported option set, implement and test it.

If it fundamentally represents generic `getsockopt`/platform-specific behavior that cannot be expressed under the project's safe-Rust and bounded-option constraints, then:

- add a precise compatibility difference;
- document the exact unsupported tuple form;
- keep common three-element options working;
- do not claim unrestricted `socket_options` parity.

This is one of the few items that may remain a positive, documented Stage C limitation because the project explicitly does not expose an unsafe arbitrary `setsockopt` escape hatch.

## Track 4 acceptance criteria

- accepted HTTPX tuple forms are pinned by executable reference tests;
- supported forms are implemented correctly;
- any retained four-tuple/platform limitation is explicit, narrow, and positively justified;
- no unsafe Rust or raw libc/FFI is added;
- no copied platform numeric constants return.

---

# Track 5 — Complete the missing transport evidence matrix

This track is mandatory. The previous corrective implementation changed transport behavior without adding enough proof.

## 5.1 UDS success-path qualification

Use a deterministic local UDS server backed by Hyper or another proper HTTP server implementation, not a one-response read/write loop that always closes.

Required cases:

1. plain HTTP over UDS;
2. HTTPS over UDS with a local TLS origin and trusted test certificate;
3. correct Host header / HTTP authority;
4. origin-form request target;
5. fixed `Content-Length` response while server leaves connection open;
6. chunked response;
7. incremental response streaming;
8. request body streaming for the supported facade surface;
9. partial response close;
10. total/connect timeout;
11. cancellation followed by constrained request;
12. same-path keep-alive reuse with observed connection count;
13. different UDS path isolation;
14. non-Unix deterministic behavior.

A test named `test_uds_https_rejected` that only points at a nonexistent socket is not HTTPS qualification and should be renamed/replaced.

## 5.2 Direct connector closure proof

Retain existing tests, and add/confirm:

- compatibility facade `HTTPTransport(local_address="127.0.0.1")` sync;
- async equivalent;
- server-observed source address;
- OS-selected nonzero source port;
- IPv6 where available;
- family mismatch/unavailable bind error class;
- Python runtime socket constants for all supported semantic kinds;
- unsupported tuple deterministic error.

Native tests using `"127.0.0.1:0"` are fine for the native Rust API, but they do not replace compatibility-layer host-only tests.

## 5.3 SOCKS qualification matrix

At minimum:

1. HTTP over SOCKS;
2. HTTPS over SOCKS;
3. no-auth negotiation;
4. credential negotiation;
5. wrong credentials;
6. proxy-selected unsupported method;
7. no acceptable method;
8. `socks5` hostname wire form;
9. `socks5h` hostname wire form;
10. IPv4 literal;
11. IPv6 literal where available;
12. unresolved hostname behavior;
13. connect refusal;
14. CONNECT reply rejection;
15. malformed/truncated handshake;
16. one coherent total timeout across multiple delayed phases;
17. cancellation + constrained follow-up;
18. origin-form target observed by destination;
19. HTTPS SNI/certificate target is origin;
20. same-route keep-alive reuse with handshake/connection count;
21. different proxy endpoints isolated;
22. different auth identities isolated;
23. direct/HTTP-proxy/SOCKS isolation;
24. credential redaction in errors/debug/display;
25. environment-selected SOCKS route if reference-supported.

## 5.4 Environment matrix

Add dedicated tests for every Track 3 case.

These should be subprocess-based if necessary to avoid environment races in parallel pytest/Rust test execution.

## 5.5 Differential rather than self-asserting tests

For public HTTPX behavior, every new fixture should either:

- execute the same scenario against HTTPX 0.28.1 and EggFetch; or
- be generated from a clearly recorded reference result and assert that exact observable behavior.

Do not add tests that only encode EggFetch's current implementation choice.

## Track 5 acceptance criteria

- successful HTTPS-over-UDS is directly proven;
- UDS persistent framing/reuse is directly proven;
- SOCKS same-route reuse is directly proven;
- SOCKS auth and address-type behavior is wire-recorded against HTTPX;
- environment precedence/matching is differential;
- compatibility-layer host-only local bind is tested sync and async;
- no required case is represented only by constructor acceptance;
- local fixtures terminate cleanly and require no external network/proxy service.

---

# Track 6 — Final exact-SHA qualification and documentation closure

Do not begin final qualification until Tracks 1–5 are complete.

## 6.1 Remove dead legacy transport code after proof

`send_socks_request_legacy` and its associated bespoke SOCKS HTTP response helpers may remain temporarily during fixture migration, but final closure should delete dead production-legacy paths if they are no longer called.

Do not keep large alternate HTTP framing code under `#[allow(dead_code)]` after the normal Hyper path is proven.

Similarly, remove stale comments such as “UDS creates fresh connections per request” if pooling now exists.

## 6.2 Routine validation

Run on the exact final executable SHA:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
./scripts/check.sh
```

No known failures may be waived as flaky for qualification.

## 6.3 Full pinned compatibility suite

Run exactly:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ \
  -q --strict-markers
```

Required result:

- zero failures;
- zero unexpected skips;
- warnings may remain only if they are known non-failing framework deprecations and are recorded separately.

## 6.4 API oracle

Regenerate the candidate manifest and run the established comparator.

Required result:

- zero unexplained differences;
- zero stale allowed entries;
- zero resolved-in-active entries;
- zero requires-resolution entries.

If Track 4 leaves a bounded `socket_options` tuple-form limitation, it must appear as a reviewed active difference with explicit rationale and not as an unexplained gap.

## 6.5 Downstream isolated runner

Use the repository's intended downstream compatibility runner rather than directly executing fixture directories in an environment where the real `httpx` module is installed.

Required command/path:

```sh
python scripts/run_downstream_compat.py
```

For the previously observed SSE failures:

- identify whether the isolated shim runner reproduces them;
- if yes and they are within the public Stage C surface, fix them;
- if they require deliberately excluded private integration, document the exact dependency and exclude them from the claim;
- do not write “all behavioral tests pass” while an unexplained public failure remains.

## 6.6 Exact-SHA evidence discipline

One executable SHA must be named for:

- targeted direct tests;
- targeted UDS tests;
- targeted SOCKS tests;
- environment differential tests;
- routine validation;
- full pinned compatibility suite;
- API oracle;
- downstream isolated runner.

Documentation-only descendants may update prose, but `qualification-sha` must point to the tested executable tree.

## 6.7 Profile and docs

Only after all executable criteria pass:

- set profile status back to `qualified`;
- replace the historical `qualification-sha` with the new executable SHA;
- retain `40beeec...` as historical superseded evidence;
- accurately document SOCKS DNS/auth behavior discovered from the reference;
- accurately document environment proxy precedence and `NO_PROXY` rules;
- document any bounded socket-option tuple limitation;
- remove stale claims that contradict actual pooling/reuse behavior.

## Track 6 acceptance criteria

- no dead legacy SOCKS HTTP framing code remains in production paths;
- no stale UDS/SOCKS lifecycle comments remain;
- routine validation is clean;
- full pinned suite is clean;
- API oracle is clean;
- downstream isolated runner has zero unexplained Stage C failures;
- all qualification evidence points to one exact executable SHA;
- profile status returns to `qualified` only after the above;
- Stage C scope remains Python >=3.10 asyncio-oriented and does not claim Trio/AnyIO or private-module parity.

---

# Global acceptance criteria

The follow-up corrective pass is complete only when all of the following are true:

- [ ] SOCKS Hyper clients/pools persist across compatible requests rather than being created per request;
- [ ] same-route SOCKS requests demonstrably reuse a connection when keep-alive permits;
- [ ] direct/HTTP-proxy/SOCKS/different-SOCKS-route isolation is proven;
- [ ] SOCKS auth method advertisement and selection match HTTPX 0.28.1 exactly;
- [ ] `socks5://` / `socks5h://` address semantics are wire-pinned against HTTPX and matched;
- [ ] unresolved destinations never fall back to loopback;
- [ ] HTTPS-over-SOCKS proves origin TLS/SNI/certificate identity and normal HTTP framing;
- [ ] SOCKS cancellation followed by a constrained request succeeds;
- [ ] SOCKS total timeout consumes one coherent request budget rather than restarting per handshake operation;
- [ ] lowercase/uppercase environment variable precedence matches HTTPX;
- [ ] scheme-less environment proxy values are normalized like HTTPX if the reference accepts them;
- [ ] `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` fallback is request-scheme-correct;
- [ ] `NO_PROXY` bare-domain, leading-dot, port, localhost, loopback, and wildcard behavior matches HTTPX;
- [ ] `trust_env=False` ignores all environment proxy state;
- [ ] explicit proxy precedence matches HTTPX;
- [ ] public `socket_options` tuple forms are pinned and either implemented or narrowly documented as a positive Stage C limitation;
- [ ] successful HTTPS-over-UDS is proven against a real local TLS UDS fixture;
- [ ] UDS fixed-length/chunked/streaming behavior does not depend on EOF;
- [ ] UDS same-path reuse and different-path isolation are proven;
- [ ] compatibility facade host-only `local_address` is tested sync and async;
- [ ] dead legacy SOCKS HTTP framing code is removed after migration;
- [ ] routine validation passes on the final executable SHA;
- [ ] the full pinned HTTPX compatibility suite passes with zero failures;
- [ ] the API oracle has zero unexplained/stale/resolved-active/requires-resolution differences;
- [ ] downstream isolated qualification has zero unexplained public Stage C failures;
- [ ] profile `qualification-sha` identifies the exact executable tree that produced all final evidence;
- [ ] profile status returns to `qualified` only after every item above closes.

---

# Rejection criteria

Reject the implementation as incomplete if any of the following is true:

- a new Hyper SOCKS client is still constructed for every request;
- SOCKS pooling/reuse is claimed without connection/handshake-count evidence;
- credentials cause EggFetch to offer auth methods different from the pinned reference;
- `socks5`/`socks5h` DNS behavior is retained based on naming convention rather than differential evidence;
- unresolved names can reach loopback or another guessed destination;
- HTTPS-over-SOCKS does not verify the origin identity;
- SOCKS timeout steps each receive a fresh full total timeout;
- cancellation leaves the route unusable under a constrained follow-up;
- uppercase proxy environment variables override lowercase ones when the pinned reference does the opposite;
- scheme-less environment proxy values fail while HTTPX accepts/normalizes them;
- `NO_PROXY` bare-domain/leading-dot semantics remain different from HTTPX;
- explicit proxy or `trust_env=False` precedence differs from HTTPX;
- the socket-option tuple contract is left ambiguous;
- UDS HTTPS “qualification” consists only of an expected error to a nonexistent socket;
- UDS/SOCKS reuse is not tested with persistent origin connections;
- legacy bespoke SOCKS HTTP framing remains an active production path;
- dead legacy transport code is retained under suppressions without a migration reason;
- final qualification contains known flaky failures;
- downstream fixture directories are run directly and treated as shim qualification instead of using `scripts/run_downstream_compat.py`;
- a public mismatch is moved to `intentional` solely to restore a green oracle/profile;
- CI/release scope is expanded for this pass;
- unsafe Rust/libc code is added to chase arbitrary socket option parity.

---

# Stop conditions

Stop and write a bounded blocker report rather than changing scope if:

1. exact HTTPX SOCKS behavior depends on an httpcore primitive that cannot be reproduced with the existing safe connector model without a broad networking rewrite;
2. persistent route-specific SOCKS pooling cannot be represented without destabilizing the default direct pool architecture;
3. a public socket-option tuple form requires unsafe generic `setsockopt` behavior and no small safe abstraction can express it;
4. HTTPS-over-UDS fails because the shared Hyper/rustls connector cannot support the pinned reference behavior without replacing a major subsystem;
5. HTTPX environment semantics depend on platform behavior that cannot be made deterministic across the project's supported targets;
6. the isolated downstream SSE failure is proven to depend on a public Stage C behavior whose implementation would materially expand into excluded private/Trio scope.

A blocker report must include:

- exact HTTPX reference case;
- exact EggFetch result;
- missing primitive;
- affected acceptance criterion;
- smallest next corrective step;
- why it cannot be completed in this pass.

Do not downgrade a blocker to an intentional difference solely to close qualification.

---

# Suggested implementation sequence

Keep commits reviewable and bisectable. A good sequence is:

1. `test: pin HTTPX SOCKS auth DNS and env semantics`
2. `fix: persist SOCKS transport pools across requests`
3. `fix: align SOCKS auth and address semantics with HTTPX`
4. `fix: align HTTPX proxy environment and NO_PROXY semantics`
5. `test: complete UDS SOCKS direct transport closure matrix`
6. `fix: close or document socket option tuple-form parity`
7. `refactor: remove superseded SOCKS legacy framing path`
8. `docs: record exact-SHA HTTPX transport qualification`

Adjacent commits may be combined if the result stays narrowly reviewable.

Do not combine unrelated cleanup.

---

# Required handoff report

The implementing agent must report:

- starting SHA;
- final executable SHA;
- documentation-only SHA if different;
- exact `httpx` version;
- exact httpcore version used by the pinned reference environment;
- SOCKS auth-method reference matrix;
- SOCKS `socks5`/`socks5h` ATYP/destination reference matrix;
- SOCKS persistent pooling design;
- same-route handshake/connection reuse evidence;
- route-isolation evidence;
- SOCKS cancellation/follow-up result;
- SOCKS coherent-timeout result;
- HTTPS-over-SOCKS origin TLS/SNI result;
- HTTP/HTTPS/ALL proxy precedence matrix;
- uppercase/lowercase collision results;
- scheme-less environment proxy result;
- `NO_PROXY` domain/leading-dot/port/localhost/wildcard matrix;
- `trust_env=False` result;
- explicit proxy precedence result;
- socket-option tuple-form decision and rationale;
- HTTPS-over-UDS success result;
- UDS fixed-length/chunked/streaming/reuse results;
- host-only `local_address` sync/async result;
- targeted transport test counts;
- `cargo fmt` result;
- `cargo clippy` result;
- `cargo test --workspace` result;
- `./scripts/check.sh` result;
- full pinned compatibility suite result;
- API oracle counts;
- downstream isolated runner result;
- SSE disposition;
- active allowed-difference count/classification;
- confirmation that no CI/release, Trio/AnyIO, Python 3.8/3.9, private-module, or unsafe-socket scope was added;
- final compatibility designation.

The intended end state is a defensible **Stage C qualified** HTTPX 0.28.1 compatibility profile whose advanced transport claims are supported by reference-driven executable evidence and whose SOCKS lifecycle matches persistent client semantics rather than per-request tunneling.