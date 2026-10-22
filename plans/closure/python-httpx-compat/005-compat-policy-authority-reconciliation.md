# Python / HTTPX Compat Milestone 005 — Compat Policy Authority Reconciliation

Status: conditionally closed

Source implementation plan:

- `plans/implementation/python-httpx-compat/005-compat-policy-authority-reconciliation.md`

Source subsystem roadmap:

- `plans/subsystems/python-httpx-compat-roadmap.md#milestone-5--compat-policy-authority-reconciliation`

Governing decision:

- `plans/adrs/ADR-0009-compat-policy-authority.md` (accepted 2026-10-22)

Repository baseline reviewed: `873ea28541c82817317430c9523fca8525d99c81`.

Implementation commits:

- None. The milestone is implemented in the working tree and is **not
  committed**; the maintainer reviews the working tree before any commit is
  made. No SHA is claimed for this milestone.

## 1. Executive finding

M005 is implemented in three parts, all of which reconcile the compatibility
facades with the engine under ADR-0009:

**Part A — the cookie jar.** The engine jar and the facade jar disagreed on
`Set-Cookie` headers whose explicit `Domain` attribute names an IP address:
the engine rejected them, the facade (which is `http.cookiejar`, the jar
HTTPX 0.28.1 itself uses) accepted them. The engine was aligned to the
facade's HTTPX-defined verdict, and only where the alignment cannot weaken
cross-host protection. The engine now accepts an IP-literal `Domain` **only
when it names exactly the response host**; the blanket rejection is gone. One
difference remains and is recorded: an explicit IPv6-literal `Domain` is
stored by both jars but only the engine returns it.

**Part B — the redirect policy.** The engine and the facade implement two
different redirect policies. The facade's is HTTPX 0.28.1's and is unchanged;
the engine keeps its stricter, origin-scoped rules. The difference is now
pinned by a 69-test conformance fixture that asserts against explicit tables,
so neither implementation can drift silently.

**Part C — the native environment surface.** Compat-only environment
semantics no longer reach the native Python client. A new keyword-only
`proxy_env` option selects the rule set; the default is the native rule set,
and the compatibility facades opt into the HTTPX rules explicitly.

The milestone cannot be marked `closed` under `plans/closure/README.md`: it
changes Rust sources, Python package sources, tests, and compatibility
fixtures, all of which are executable inputs under the exact-SHA rule in
`.skills/verification-qualification.md`. That invalidates the live Stage C
binding, and the Stage C rebinding plus a full Tier 1 / Tier 2 / Tier 3 run on
the frozen SHA were deliberately not performed at this stage. The milestone is
therefore **conditionally closed**, with the condition, risk, and the exact
future evidence in §10 and §11.

One factual correction to the milestone's own briefing is recorded here rather
than hidden. The plan and ADR prose assumed the engine strips `Authorization`
on **every** redirect hop and that the facade preserves it across an `https`
to `http` downgrade. Measured against the real HTTPX 0.28.1, the engine's rule
is **origin-based** — `advance_redirect_hop` re-attaches a manually-set
`Authorization` when `origin(previous) == origin(next)` — and the facade
**strips** `Authorization` on an `https` to `http` downgrade, exactly as the
engine does. The real divergences are different from the ones the plan
predicted; §3 records them. ADR-0009's Context and rule 4 were corrected in
the same pass.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| The cookie verdict difference is measured before a resolution is chosen (§6.1) | 19-case shared matrix in `crates/eggfetch-core/tests/cookie_policy_parity_tests.rs` and `crates/eggfetch-python/tests/compat/test_cookie_policy_parity.py` | Met | Engine side in Rust; facade side against a **real local HTTP server** |
| The engine and the facade reach the same accept/reject verdict and the same contributed `Cookie` header (ADR-0009 rule 3) | Rows 7 and 8 of the matrix now agree; `engine_cookie_policy_matches_agreed_matrix` asserts the whole matrix | Met | Engine aligned (Resolution A) |
| The engine's change cannot weaken cross-host cookie protection | `ip_literal_domain_only_matches_the_exact_host`; `domain_matches` already restricts IP literals to exact equality (RFC 6265 §5.2.3) | Met | `Domain=127.0.0.2` for host `127.0.0.1` is still rejected |
| Every remaining difference is recorded in `docs/residual-differences.md` (§6.2) | New "Cookie `Domain` attribute policy" section | Met | Records the IPv6-literal residual and the stored-domain spelling difference |
| The facade's redirect credential rule is compared with the engine's, and the difference is recorded (§6.3, ADR-0009 rule 4) | `crates/eggfetch-python/tests/compat/test_redirect_policy_parity.py` (69 tests) + new "Redirect credential scope: engine vs HTTPX 0.28.1" section | Met | The facade keeps HTTPX 0.28.1 behavior; the engine keeps its stricter rules |
| The method-rewrite tables are compared and the difference pinned (§6.3) | `KNOWN_METHOD_DIVERGENCES = {(302, "PUT"), (302, "DELETE")}` with `test_known_method_divergence_is_exactly_the_pinned_set` | Met | 301/303/307/308 agree |
| The facade matches upstream HTTPX 0.28.1 exactly (§6.3) | Policy layer asserts the facade's `_redirect_headers` / `_redirect_method` against upstream `httpx.Client`'s; the wire layer asserts header-set equality | Met | Asserted on header **sets**, not just the method |
| The native client uses the native `NO_PROXY` parser (§6.4, ADR-0009 rule 5) | `proxy_env` defaults to `native` → `NoProxy::parse` | Met | Observed: `NO_PROXY=localhost` + target `127.0.0.1` goes **direct** natively, **proxied** on the facade |
| The native surface does not rewrite `socks5://` to `socks5h://` (§6.4) | Native `proxy="socks5://…"` → SOCKS5 CONNECT **ATYP 1/4** (IP literal, local DNS); the facade → **ATYP 3** host (remote DNS) | Met | Observed on the wire |
| The compat facades keep both compat behaviors (§6.4) | Facade → HTTPX rules and `socks5h://`; observed identical to upstream httpx | Met | — |
| The compat-only semantics are not inherited by native users (§6.4) | The default is `proxy_env="native"`; the facade opts in explicitly | Met | — |
| `eggfetch.compat.httpx2` keeps the compat semantics without its own configuration (§6.4) | `httpx2.Client.__init__ is httpx._client.Client.__init__` → `True`; no `httpx2/` file was edited; the httpx2 client is observed to proxy for `NO_PROXY=localhost` + target `127.0.0.1` | Met | Inherited through the shared base facade |
| The separation is load-bearing, not incidental | Negative control: removing `kwargs["proxy_env"] = "httpx"]` fails exactly 3 tests | Met | — |
| The native API surface stays in sync (§6.4) | `scripts/check_native_python_api.py --self-test` → "Native API manifest passed (66 exports)"; `scripts/check_python_typing_surface.py --self-test` → passed | Met | `proxy_env` added to the `Client` / `AsyncClient` signatures, the `_native.pyi` stub, and the manifest |
| HTTPX 0.28.1 conformance is preserved exactly | `test_facade_matches_httpx_0281_reference` asserts the facade's verdict equals real `httpx` 0.28.1's for all 19 rows | Met | httpx2 2.12.0 was untouched by this milestone |
| Tier 1, Tier 2, and Tier 3 pass on the frozen executable SHA (§13) | Not run | **Not met** | Deliberately deferred; see §10 |
| Both HTTPX profiles are rebound to that exact SHA (§13) | Not performed | **Not met** | See §7 and §10 |

## 3. Production implementation evidence

### Part A — cookie jar alignment

`crates/eggfetch-core/src/cookie.rs`, `parse_set_cookie`:

```rust
// before
if is_ip_address(&normalized) {
    return None;
}

// after
if is_ip_address(&normalized) && !normalized.eq_ignore_ascii_case(response_host) {
    return None;
}
```

The change is behavioral only: no new `pub` item, no new public surface, no
signature change. It relaxes the blanket rejection of an IP-literal `Domain`
to an exact-response-host acceptance. The safety argument was verified, not
assumed: `domain_matches` already returns `false` for any IP-literal domain
that is not an exact match (`is_ip_address(host) || is_ip_address(domain)` →
false), so the removed blanket check was a strict narrowing layered on an
already-safe matcher.

### Part A — the measured matrix

"engine (before)" = baseline `873ea285`; "engine (after)" = this change;
"facade" = `eggfetch.compat.httpx`'s `http.cookiejar`, measured against a
real local HTTP server. The header column is the `Cookie` header the engine
contributes, or the value the facade's server observed on the wire.

| # | case | host | `Set-Cookie` | engine (before) | engine (after) | facade |
|---|---|---|---|---|---|---|
| 1 | `no_domain` | example.test | `a=1` | accept / `example.test` / host_only=true / `/` / false / session / `a=1` | same | accept / `example.test` / host_only=true / `/` / false / session / `a=1` |
| 2 | `domain_exact` | example.test | `a=1; Domain=example.test` | accept / `example.test` / host_only=false / `/` / false / session / `a=1` | same | accept / `.example.test` / host_only=false / `/` / false / session / `a=1` |
| 3 | `domain_leading_dot` | example.test | `a=1; Domain=.example.test` | accept / `example.test` / host_only=false / `/` / false / session / `a=1` | same | accept / `.example.test` (initial-dot=true) / host_only=false / `/` / false / session / `a=1` |
| 4 | `domain_parent` | sub.example.test | `a=1; Domain=example.test` | accept / `example.test` / host_only=false / `/` / false / session / `a=1` | same | accept / `.example.test` / host_only=false / `/` / false / session / `a=1` |
| 5 | `domain_mismatch` | example.test | `a=1; Domain=other.test` | **reject** / no header | reject | **reject** (not stored) / `""` |
| 6 | `domain_unrelated_suffix` | example.test | `a=1; Domain=evil.test` | **reject** / no header | reject | **reject** (not stored) / `""` |
| 7 | `domain_ipv4_literal` | 127.0.0.1 | `a=1; Domain=127.0.0.1` | **REJECT** / no header | **accept** / `127.0.0.1` / host_only=false / `/` / false / session / **`a=1`** | accept / `.127.0.0.1` / host_only=false / `/` / false / session / **`a=1`** |
| 8 | `domain_ipv4_leading_dot` | 127.0.0.1 | `a=1; Domain=.127.0.0.1` | **REJECT** / no header | **accept** / `127.0.0.1` / host_only=false / `/` / false / session / **`a=1`** | accept / `.127.0.0.1` / host_only=false / `/` / false / session / **`a=1`** |
| 9 | `domain_ipv4_mismatch` | 127.0.0.1 | `a=1; Domain=127.0.0.2` | reject / no header | reject | reject (not stored) / `""` |
| 10 | `domain_ipv6_literal` | [::1] | `a=1; Domain=[::1]` | accept / `[::1]` / host_only=false / `/` / false / session / **`a=1`** | accept / `[::1]` / host_only=false / `/` / false / session / **`a=1`** | accept / `.[::1]` / host_only=false / `/` / false / session / **`""` (stored, not returned)** |
| 11 | `ipv6_host_only` | [::1] | `a=1` | accept / `[::1]` / host_only=true / `/` / false / session / `a=1` | same | accept / `[::1].local` / host_only=true / `/` / false / session / `a=1` |
| 12 | `domain_ipv6_unbracketed` | [::1] | `a=1; Domain=::1` | reject / no header | reject | reject (not stored) / `""` |
| 13 | `path_api` | example.test | `a=1; Path=/api` | accept / `example.test` / host_only=true / path=`/api` / false / session / `a=1` | same | accept / `example.test` / host_only=true / path=`/api` / false / session / `a=1` |
| 14 | `path_api_no_match` | example.test | `a=1; Path=/api` (req `/other`) | accept / true / path=`/api` / **`""`** | same | accept / true / path=`/api` / **`""`** |
| 15 | `path_root` | example.test | `a=1; Path=/` (req `/api/v1`) | accept / true / path=`/` / `a=1` | same | accept / true / path=`/` / `a=1` |
| 16 | `secure_set` | example.test | `a=1; Secure` | accept / secure=true / **`""`** (http) | same | accept / secure=true / **`""`** (http) |
| 17 | `max_age` | example.test | `a=1; Max-Age=3600` | accept / persistent=true / expires=now+3600 / `a=1` | same | accept / persistent=true / expires=now+3600 / `a=1` |
| 18 | `expires_future` | example.test | `a=1; Expires=Wed, 21 Oct 2035 07:28:00 GMT` | accept / persistent=true / expires=2076564480 / `a=1` | same | accept / persistent=true / expires=2076564480 / `a=1` |
| 19 | `expires_past` | example.test | `a=1; Expires=Thu, 01 Jan 1970 00:00:00 GMT` | reject (parsed, then evicted) | same | reject (deleted at parse) / `""` |

Rows 7 and 8 were the divergence; the engine is now aligned. Row 10 is the
remaining, recorded residual.

### Part B — redirect policy

Measured header matrix (wire level; the first hop carried all six headers):

| scenario | engine | facade | upstream httpx 0.28.1 |
|---|---|---|---|
| same-origin | auth **KEEP**, proxy-auth **DROP**, cookie **KEEP**, referer **KEEP**, origin **KEEP**, host KEEP | auth KEEP, proxy-auth **KEEP**, cookie **DROP**, referer KEEP, origin KEEP, host KEEP | identical to facade |
| cross-origin | auth DROP, proxy-auth DROP, cookie DROP, referer **DROP**, origin **DROP**, host→dest | auth DROP, proxy-auth **KEEP**, cookie DROP, referer **KEEP**, origin **KEEP**, host→dest | identical to facade |
| http→https upgrade (local, non-canonical ports) | auth DROP, proxy-auth DROP, cookie DROP, referer DROP, origin DROP, host→dest | auth DROP, proxy-auth **KEEP**, cookie DROP, referer KEEP, origin KEEP, host→dest | identical to facade |
| https→http downgrade | auth DROP, proxy-auth DROP, cookie DROP, referer DROP, origin DROP, host→dest | auth DROP, proxy-auth **KEEP**, cookie DROP, referer **KEEP**, origin **KEEP**, host→dest | identical to facade |
| **canonical** http:80→https:443 same host (policy layer only) | DROP (origin-based rule; http:80 and https:443 are different origins) | **KEEP** | **KEEP** |

Measured method matrix (wire level):

| status | GET | HEAD | POST | PUT | DELETE |
|---|---|---|---|---|---|
| 301 engine | GET | HEAD | GET | PUT | DELETE |
| 301 facade/upstream | GET | HEAD | GET | PUT | DELETE |
| **302 engine** | GET | HEAD | GET | **PUT** | **DELETE** |
| **302 facade/upstream** | GET | HEAD | GET | **GET** | **GET** |
| 303 all | GET | HEAD | GET | GET | GET |
| 307 / 308 all | GET | HEAD | POST | PUT | DELETE |

The five recorded differences (engine ≠ facade):

1. `Proxy-Authorization` — the engine strips it on **every** hop; the facade
   keeps it on every hop.
2. `Cookie` — the engine keeps an explicit `Cookie` on **same-origin** hops;
   the facade pops it **unconditionally**.
3. `Referer` / `Origin` — the engine strips them **cross-origin**; the facade
   keeps them.
4. `Authorization` on the **canonical** `http:80` → `https:443` same-host
   upgrade — the facade keeps it, the engine drops it.
5. Method — **302 with a non-POST/HEAD method**: the engine preserves the
   method *and* body; the facade rewrites to `GET` and drops the body.
   301/303/307/308 agree.

Everything else agrees: `Authorization` same-origin (both keep),
`Authorization` cross-origin and on the downgrade (both drop), `Cookie`
cross-origin (both drop), `Host` (both keep same-origin / rewrite
cross-origin).

### Part B — the fixture

`crates/eggfetch-python/tests/compat/test_redirect_policy_parity.py` (69
tests), two layers, both asserting against **explicit pinned tables**:

- **policy layer** — the facade's `_redirect_headers` / `_redirect_method`
  against upstream `httpx.Client._redirect_headers` / `._redirect_method`, over
  7 origin/target cases × the 5×5 method matrix.
- **wire layer** — the same chain driven through `eggfetch.Client`,
  `eggfetch.compat.httpx.Client`, and upstream `httpx.Client` against local
  HTTP/TLS servers; the outgoing hop's method and header set are read off the
  wire.

Pinned tables: `HTTPX_METHOD_TABLE`, `ENGINE_METHOD_TABLE`,
`KNOWN_METHOD_DIVERGENCES = {(302, "PUT"), (302, "DELETE")}`, and
`test_known_method_divergence_is_exactly_the_pinned_set`, which guards the
divergence set itself.

### Part C — native environment isolation

A new **keyword-only** native constructor option `proxy_env`:

- `proxy_env=None` or `"native"` → **default**: `NoProxy::parse` and
  `socks5://` passed through unchanged.
- `proxy_env="httpx"` → `NoProxy::parse_httpx` and `socks5://` → `socks5h://`.
- any other value → `ValueError` (never a silent fallback).

| file | change |
|---|---|
| `crates/eggfetch-python/src/proxy.rs` | New `pub(crate) enum ProxyEnvSemantics { Native, Httpx }`, `parse_proxy_env_semantics`, `parse_no_proxy`, `apply_proxy_semantics`; `normalize_compat_proxy_url` → `normalize_httpx_proxy_url`; `parse_proxy` no longer rewrites the scheme |
| `crates/eggfetch-python/src/request_preparation.rs` | `PreparedClientConfig.proxy_env_semantics`, `RequestDispatchDefaults.proxy_env_semantics`, `prepare_client_config(..., proxy_env: Option<&str>)`; environment `NO_PROXY` via `proxy::parse_no_proxy`; the client/request proxy URL via `proxy::apply_proxy_semantics` |
| `crates/eggfetch-python/src/client.rs` | `proxy_env=None` in the `#[pyo3(signature=…)]`, the parameter, the struct field, and forwarding into `RequestDispatchDefaults` (2 sites) |
| `crates/eggfetch-python/src/async_client.rs` | The same, for `AsyncClient` |
| `crates/eggfetch-python/src/lib.rs` | The one-shot helpers use `ProxyEnvSemantics::Native` |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py` | `kwargs["proxy_env"] = "httpx"]` at both native-client construction sites |
| `crates/eggfetch-python/python/eggfetch/compat/httpx/_transports.py` | The same, at the `HTTPTransport` / `AsyncHTTPTransport` construction sites |
| `crates/eggfetch-python/python/eggfetch/_native.pyi` | The `Client` / `AsyncClient` `__init__` stub gains `proxy_env: str \| None = ...` |
| `crates/eggfetch-python/tests/native_api_manifest.json` | `signatures.Client` and `signatures.AsyncClient` gain `proxy_env=None` |
| `crates/eggfetch-python/tests/compat/test_redirect_policy_parity.py` | New |
| `crates/eggfetch-python/tests/compat/test_environment_isolation.py` | New |
| `crates/eggfetch-python/tests/compat/test_cookie_policy_parity.py` | New |

Two files were edited outside the milestone's original file split and are
recorded here so the deviation is explicit:

- `_native.pyi` — `crates/eggfetch-python/python/eggfetch/__init__.pyi` is a
  pure re-export shim and carries no constructor signature, so it needed no
  change; the constructor signature lives in `_native.pyi`, and
  `scripts/check_python_typing_surface.py` compares the manifest's
  `signatures.Client` against `_native.pyi`'s `Client.__init__` shape, so the
  check can only pass if `_native.pyi` is updated.
- `compat/httpx/_transports.py` — `HTTPTransport` / `AsyncHTTPTransport` are
  public compat exports that construct their own native client; without the
  opt-in they would have silently lost the compat `NO_PROXY` / SOCKS
  semantics.

Observed separation:

| `NO_PROXY` | target host | native | native `proxy_env="httpx"` | facade | upstream httpx |
|---|---|---|---|---|---|
| `localhost` | `127.0.0.1` | **direct** | **PROXY** | **PROXY** | **PROXY** |
| `localhost` | `localhost` | direct | direct | direct | direct |
| `127.0.0.1` | `127.0.0.1` | direct | direct | direct | direct |
| `*` | `127.0.0.1` | direct | direct | direct | direct |
| `.localhost` | `127.0.0.1` | PROXY | PROXY | PROXY | PROXY |

| client | SOCKS5 CONNECT |
|---|---|
| native `proxy="socks5://…"` | **ATYP 1/4 (IP literal)** — local DNS |
| native `proxy="socks5://…"`, `proxy_env="httpx"` | **ATYP 3, host `localhost`** — remote DNS |
| facade `proxy="socks5://…"` | **ATYP 3, host `localhost`** |
| upstream httpx `proxy="socks5://…"` (httpcore) | **ATYP 3, host `localhost`** |

Negative control: removing `kwargs["proxy_env"] = "httpx"]` makes exactly three
tests fail —
`test_native_and_compat_use_different_no_proxy_rules[localhost-127.0.0.1]`,
`test_native_leaves_socks5_scheme_alone_and_compat_rewrites_it`, and
`test_httpx2_client_inherits_the_compat_environment_semantics` — which proves
the fixture is load-bearing.

## 4. Verification executed

### Commands run

```sh
.venv/bin/python -m maturin develop -m crates/eggfetch-python/Cargo.toml --offline

cargo test --offline -p eggfetch-core --all-features --test cookie_policy_parity_tests --test http_version_capability_tests -- --test-threads=1
cargo test --offline -p eggfetch-core --all-features cookie -- --test-threads=1
cargo test --offline -p eggfetch-core --no-default-features --features http1,tls-rustls --test cookie_policy_parity_tests -- --test-threads=1
cargo clippy --offline -p eggfetch-core --all-features -- -D warnings
cargo clippy --offline -p eggfetch-core --all-features --all-targets -- -D warnings
cargo clippy --offline -p eggfetch-python --all-features -- -D warnings

.venv/bin/python -m pytest crates/eggfetch-python/tests/compat/test_cookie_policy_parity.py crates/eggfetch-python/tests/compat/test_redirect_policy_parity.py crates/eggfetch-python/tests/compat/test_environment_isolation.py -q --strict-markers
.venv/bin/python -m pytest crates/eggfetch-python/tests/compat/test_cookie_scope_parity.py crates/eggfetch-python/tests/compat/test_redirect_state_machine_parity.py crates/eggfetch-python/tests/compat/test_no_proxy_differential.py crates/eggfetch-python/tests/compat/test_imports.py crates/eggfetch-python/tests/compat/test_cookies.py -q --strict-markers
.venv/bin/python -m pytest crates/eggfetch-python/tests/compat/ -k "httpx2" -q --strict-markers
.venv/bin/python scripts/check_native_python_api.py --self-test
.venv/bin/python scripts/check_python_typing_surface.py --self-test
.venv/bin/python scripts/check_doc_links.py
cargo fmt --all -- --check
```

### Results (pass/fail/skip with reason; counts without concealment)

| Command | Result |
|---|---|
| `maturin develop --offline` | exit 0 — "Finished dev profile … in 1m 04s"; "Built wheel … eggfetch-0.2.2"; "Installed eggfetch-0.2.2" |
| `cargo test -p eggfetch-core --all-features --test cookie_policy_parity_tests --test http_version_capability_tests` | exit 0 — 3 passed + 2 passed, 0 failed |
| `cargo test -p eggfetch-core --all-features cookie` | exit 0 — lib 65 passed, 0 failed; `cookie_policy_parity_tests` 2 passed; `cross_feature_tests` 1 passed |
| `cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls --test cookie_policy_parity_tests` | exit 0 — 0 passed, 0 failed. The lean profile does not enable `cookies`, so the file's `#![cfg(feature = "cookies")]` compiles it out. Only the named target was compiled, so the known M009 lean gap did not surface |
| `cargo clippy -p eggfetch-core --all-features -- -D warnings` | exit 0 |
| `cargo clippy -p eggfetch-core --all-features --all-targets -- -D warnings` | exit 0 (after fixing one `clippy::cloned_ref_to_slice_refs` hit in the new test file) |
| `cargo clippy -p eggfetch-python --all-features -- -D warnings` | exit 0 (after fixing one `clippy::implicit_clone` hit: `url.to_owned()` → `url`) |
| `cargo clippy -p eggfetch-core -p eggfetch-cli -p eggfetch-ffi -p eggfetch-python --all-targets --all-features -- -D warnings` | see §4.1 below |
| `cargo fmt --all -- --check` | clean — no output |
| new compat suites (cookie + redirect + environment isolation) | **121 passed**, 2 upstream `httpx` deprecation warnings, 57.86 s |
| compat regression sweep (cookie scope, redirect state machine, no-proxy differential, imports, cookies) | **196 passed**, 74.24 s |
| `compat/ -k httpx2` | 50 passed, 1901 deselected |
| `check_native_python_api.py --self-test` | "Native API manifest passed (66 exports)" — exit 0 |
| `check_python_typing_surface.py --self-test` | "Python typing surface passed (66 runtime exports, 32 exception bases, 24 reviewed member contracts)" — exit 0 |
| `check_doc_links.py` | "All internal links OK." across 59 markdown files |
| negative control (opt-in removed) | 3 expected failures — confirms the environment-isolation fixture is load-bearing |

Not run, with reason: `./scripts/check.sh` (Tier 1), `./scripts/check.sh
extended` (Tier 2), and `./scripts/check.sh package` (Tier 3) — deliberately
deferred at this stage per the maintainer's instruction to keep verification
scoped.

### 4.1 Workspace clippy and format

`cargo fmt --all -- --check` produced no output (clean).

`cargo clippy --offline -p eggfetch-core -p eggfetch-cli -p eggfetch-ffi -p eggfetch-python --all-targets --all-features -- -D warnings`
exited 0 with no findings, across the four crates this milestone's sibling
milestones also changed. Two findings were fixed during the milestone rather
than suppressed: one `clippy::implicit_clone` in `crates/eggfetch-python/src/proxy.rs`
(`url.to_owned()` → `url`) and one `clippy::cloned_ref_to_slice_refs` in the
new core cookie fixture (`std::slice::from_ref`).

## 5. Invariant review

- **All networking stays in `eggfetch-core`.** The Python changes are
  constructor arguments, proxy-URL selection, and cookie-jar policy inside the
  engine; no transport, socket, or HTTP parsing was added to an adapter.
- **No new public engine surface.** The cookie change is behavioral; the
  Python-side `ProxyEnvSemantics` is `pub(crate)`. `public_api_contracts`
  passes.
- **The native surface is HTTPX-shaped only where the repo already made it
  so.** `proxy_env` is a keyword-only option with a native default, matching
  the existing native-client design, which already exposes `follow_redirects`,
  `trust_env`, and `proxy`.
- **The 0.28.1 facade's HTTPX conformance is untouched.** The facade's
  redirect rules and its `http.cookiejar` are unchanged; the facade only
  gained an explicit `proxy_env="httpx"]` opt-in.
- **The httpx2 facade needed no change.** It inherits the base facade's
  client construction, so the opt-in propagates. No `httpx2/` file was edited
  by this milestone.
- **No new Python runtime dependency and no new cargo dependency.**
- **The typed rebuild helpers remain the only place a hop request is built.**
  The facade's own redirect loop is a Python-level concern and was not
  restructured.
- **Redaction is preserved.** The new `proxy_env` option accepts only the
  literal values `"native"` and `"httpx"`; an unknown value raises `ValueError`
  echoing the caller's own input, which is not a credential.

## 6. Failure, timeout, pool, and recovery review

- The cookie change is a **parse-time** decision. It neither accepts nor
  rejects anything on a new path: `parse_set_cookie` already existed and
  already returned `None` to reject. No timeout, pool, or deadline behavior
  is touched.
- The redirect change is documentation and a fixture. Neither the engine's nor
  the facade's redirect loop was restructured, so the hop count, the
  `max_redirects` ceiling, the total-deadline span, and the pool-permit
  lifecycle are unchanged.
- The `proxy_env` change selects a rule set at construction time. It does not
  add a connection attempt, a retry, or a fallback: a `ValueError` for an
  unknown value is raised while the constructor arguments are normalized, so
  no client is built and no connection is attempted.
- The SOCKS change alters only the **destination form** of the CONNECT
  request (ATYP 1/4 versus ATYP 3). It does not change the retry, fallback,
  or connection-failure policy; a SOCKS failure still follows the existing
  typed-fallback rules.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

- The **native Python surface's** `NO_PROXY` semantics change for native
  users: with `NO_PROXY=localhost`, a request to `127.0.0.1` now goes direct
  where it previously went through the proxy. This is the intended effect of
  ADR-0009 rule 5 and is called out as a release note in
  `docs/migration/from-httpx.md`. `proxy_env="httpx"]` restores the old
  behavior.
- The **native Python surface's** explicit `socks5://` proxy now keeps its
  local-DNS meaning where it previously became `socks5h://`. Environment
  proxies (`ALL_PROXY=socks5://…`) were never rewritten by the compat path
  either, so environment-proxy behavior is unchanged for both surfaces; that
  was deliberately left alone to avoid an unrequested compat behavior change.
- The **CLI** is unaffected: it uses `NoProxy::parse` and its own proxy path,
  and `crates/eggfetch-cli/**` was not touched.
- The **engine's** cookie behavior changes only for an IP-literal `Domain`
  that names exactly the response host. The native Rust surface is affected
  by the same rule; this is the deliberate alignment recorded in ADR-0009
  rule 3 and in `docs/residual-differences.md`.
- The **FFI and Node** surfaces are unaffected.

**Exact-SHA binding.** The live Stage C binding is **invalidated** by this
milestone: Rust sources, Python package sources, tests, and compatibility
fixtures are executable inputs under `.skills/verification-qualification.md`.
No SHA is claimed here because the milestone is not committed. Publication
must not cite the previous binding until the Stage C binding is
re-established at a frozen SHA that includes this milestone, and until the
ledger's Stage C SHA and both HTTPX profiles are rebound to that same commit.

The measurement in this record was taken against the **working tree**, not a
frozen SHA. Per `.skills/verification-qualification.md`, it counts as
implementation evidence, not as Tier 1 evidence; Tier 1 evidence requires the
frozen-SHA run.

## 8. Security review

**The cookie relaxation was verified not to weaken cross-host protection.**
`domain_matches` already implements RFC 6265 §5.2.3 — an IP-literal domain
matches only by exact equality — so `Domain=127.0.0.1` is still rejected for
any host other than `127.0.0.1`, and the cookie is only ever returned to the
exact IP it names. The removed check was a strict narrowing layered on an
already-safe matcher. The new test `ip_literal_domain_only_matches_the_exact_host`
asserts exactly this, and the pre-existing rejection test was retargeted to a
*different* IP literal so its original intent is preserved.

**The engine's stricter redirect rules are kept, and the reasons are recorded
rather than silently dropped:**

- `Proxy-Authorization` is a proxy credential. Forwarding it to a different
  origin discloses the proxy secret, so the engine drops it on every hop.
- `Referer` and `Origin` disclose the source URL to the destination origin;
  the engine drops them whenever the origin changes.
- `Authorization` is replayed only within one origin. HTTPX replays a bearer
  token across the canonical `http` → `https` upgrade; the engine does not,
  because a scheme change is a new origin.
- The `302` method difference is a browser-emulation rule in HTTPX versus the
  engine's RFC 9110 table. The engine preserves the method and body for
  non-POST methods, so the next origin receives what the client actually sent.

The facade's HTTPX behavior is unchanged and remains the compatibility
facade's specification.

**The native environment isolation removes a compat-only default from the
native surface.** A native user now gets eggfetch's own `NO_PROXY` rules and
eggfetch's own SOCKS destination resolution by default, and must opt in
explicitly for HTTPX's. `proxy_env` accepts only two literal values and raises
`ValueError` otherwise, so no env-derived or caller-derived string is
interpreted.

**No credential is rendered anywhere in the new code.** The new error message
echoes only the caller's own option value.

## 9. Documentation and operations

| File | Change |
|---|---|
| `docs/residual-differences.md` | New "Cookie `Domain` attribute policy" section: the alignment, the IPv6-literal residual, and the stored-domain spelling difference. New "Redirect credential scope: engine vs HTTPX 0.28.1" section: the full case table, the origin-based rule, the four security reasons, and the `302` method difference. |
| `docs/architecture/python-bindings.md` | New "Proxy-environment semantics" subsection documenting the `proxy_env` option, its two rule sets, the `ValueError` on an unknown value, the native default, and the facades' explicit opt-in including the httpx2 inheritance. |
| `docs/concepts/proxy.md` | New "Environment semantics: native vs HTTPX" subsection: the rule comparison table, the worked `NO_PROXY=localhost` example, and the opt-in. |
| `docs/migration/from-httpx.md` | New "Redirect credentials and proxy environment" subsection under "Redirects": the two HTTPX behaviors the native client does not reproduce, with the pointer to `docs/residual-differences.md` and the `proxy_env="httpx"]` opt-in. |
| `plans/adrs/ADR-0009-compat-policy-authority.md` | The Context's characterization of the two redirect policies and rule 4's wording were corrected against the measured behavior, with a dated correction note. The decisions in rules 1–6 are unchanged. |

Operations: one new keyword-only option on the native Python `Client` and
`AsyncClient`. No new dependency, network access, credential, or environment
variable is introduced.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| High | The live Stage C binding is invalidated by this milestone's Rust, Python, test, and fixture changes, and the Stage C rebinding was deliberately not performed at this stage | No publication may cite the previous Stage C binding | Rebind both HTTPX profiles and the ledger's Stage C SHA to the frozen commit that includes this milestone, then re-run the compat qualification |
| High | Tier 1, Tier 2, and Tier 3 were not run on the frozen SHA (deliberately deferred) | The milestone's own conformance suites are green; the repository-wide gates are unproven for this tree | Run `./scripts/check.sh`, `./scripts/check.sh extended`, and `./scripts/check.sh package` on the frozen SHA before release |
| Medium | The native Python surface's `NO_PROXY` and SOCKS behavior changes for native users; `proxy_env="httpx"]` restores the previous behavior | Existing native users relying on `NO_PROXY=localhost` also covering loopback IPs, or on remote SOCKS DNS, see a behavior change | Release note. The wording is in `docs/migration/from-httpx.md`; the option itself is documented in `docs/architecture/python-bindings.md` |
| Medium | An explicit IPv6-literal `Domain` (`Domain=[::1]`) is stored by both jars but returned only by the engine | The facade does not send that cookie. The difference is a `http.cookiejar` host-mangling artifact, not a safety gap | Recorded in `docs/residual-differences.md`. Matching the facade would mean reproducing a standard-library artifact in the native engine; recommended action is to keep the difference |
| Low | The canonical `http:80` → `https:443` same-host upgrade could not be observed end to end: the process runs as uid 1000 and cannot bind ports 80/443 | The facade's and upstream's side is proven at the policy layer; the engine's side is proven as the origin rule via a real http→https hop on non-canonical ports | Stated explicitly in the fixture docstring and test name. No action unless a privileged environment becomes available |
| Low | The two jars spell the stored domain differently (`.example.test` vs `example.test`) | A caller comparing a stored domain string across the two surfaces sees different spellings for the same cookie | Recorded in `docs/residual-differences.md`. The accept/reject verdict and the contributed `Cookie` header are identical |
| Low | Filesystem at 98% with ~22 GB free | Future full-workspace builds may run out of space | Reclaim space before Tier 2 / Tier 3 |

## 11. Roadmap disposition

`plans/subsystems/python-httpx-compat-roadmap.md` milestone 5 moves from
`ready` to `conditionally closed`, with §11 (completion definition) and §12
(milestone status) updated in the same pass.

The milestone's own scope is complete. What remains is cross-cutting release
evidence — Tier 1 / Tier 2 / Tier 3 on the frozen SHA and the Stage C
rebinding — which belongs to the release-verification subsystem.

## 12. Registry updates

`plans/registry.md`:

- The python-httpx-compat row records milestone 5 as conditionally closed,
  with the Stage C rebinding named as the outstanding condition.
- The implementation-plan table for M005 points at this closure record.
- The blocked / operational work section records the Stage C rebinding as
  pending for all four milestones closed in this pass.
