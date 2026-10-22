# Python / HTTPX Compat Milestone 004 — httpx2 URL Origin Reachability

Status: closed

Source implementation plan:

- `plans/implementation/python-httpx-compat/004-httpx2-origin-reachability.md`

Source subsystem roadmap:

- `plans/subsystems/python-httpx-compat-roadmap.md#milestone-4--httpx2-url-origin-reachability`

Repository baseline reviewed: `873ea28541c82817317430c9523fca8525d99c81`.

Implementation commits:

- `d6c41adf4cf8f62ef990cccedf2f86512858fa0b` — feat: implement the eight
  registered milestones as one qualification freeze. This commit carries the
  milestone's implementation together with the other seven milestones
  executed in the same coordinated pass.
- `26fa619299fb8d447f25cbcb5eb713a1f537a30c` — fix(security): patch the four
  dependencies with live RustSec advisories. The bound Stage C SHA; it
  changes only `Cargo.lock` and `fuzz/Cargo.lock` relative to the freeze
  commit above, and no Rust source.

## 1. Executive finding

M004 is implemented. The httpx2 headline delta — `URL.origin` — is now
reachable from every URL a user can observe in the `eggfetch.compat.httpx2`
facade: a constructed `Request`, a constructed `Response`, a response returned
by the sync or async client, the final URL after a redirect, and every
history entry.

The root cause was that `eggfetch/compat/httpx2/_request.py` and
`_response.py` were `import *` re-export shims of the 0.28.1 facade's modules,
and `httpx2/__init__.py` imported `Request` / `Response` from the 0.28.1
facade directly. `httpx2.Request is httpx.Request` was `True`, so
`request.url` was a base `httpx._urls.URL` with no `origin` attribute, even
though `httpx2.URL` itself carried one.

The implementation keeps the 0.28.1 facade byte-for-byte unchanged. The httpx2
`Request` and `Response` are now genuine **subclasses** of the 0.28.1 classes,
and the httpx2 facade coerces URLs to its own `URL` class inside its own
modules. Two intentional, recorded public-surface deltas follow from that:
`httpx2.Request is httpx.Request` is now `False`, and the httpx2 API manifest
records a different base class for `Request` and `Response`. Both are
registered in `compat/httpx2/2.12.0/allowed-differences.toml` as
`H2X-NEW-016` and `H2X-NEW-017`, and both API manifest comparisons are green.

The milestone is **closed**. It changes the Python package's executable
source, its compatibility fixtures, and its API manifest allowlist, all of
which are executable inputs under the exact-SHA rule in
`.skills/verification-qualification.md`; that change invalidated the previous
live Stage C binding, and the binding has since been re-established at the
bound SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c` — the implementation
freeze `d6c41adf4cf8f62ef990cccedf2f86512858fa0b` plus the lockfile-only
dependency patch. The frozen-SHA tier gates were run at that SHA: Tier 1 is
green, Tier 2 is green across every phase (2073 pinned HTTPX compatibility
tests passed, 0 failed; the Rust public-API oracle passes in 8 profiles), and
Tier 3 is green. The live security preflight reports `advisories ok, bans ok,
licenses ok, sources ok`. §10 records the evidence. The milestone's own
scoped findings — the `httpx2.Request` / `httpx2.Response` base-class deltas
(`H2X-NEW-016` / `H2X-NEW-017`) and the `URL` constructor signature freeze —
are unaffected by the binding and remain open by design.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| `httpx2.Request(...).url.origin` resolves to the expected `Origin` (§6.1) | `test_httpx2_origin_reachability.py::test_request_url_is_httpx2_url_with_origin` | Met | `Origin(scheme='https', host='example.com', port=443)` |
| `httpx2.Response(...).url.origin` resolves (§6.1) | `::test_response_url_is_httpx2_url_with_origin` | Met | — |
| A response from the httpx2 sync client exposes `.url.origin` (§6.1) | `::test_sync_client_response_exposes_origin` | Met | Against a real local server |
| The same for the async client (§6.1) | `::test_async_client_response_exposes_origin` | Met | — |
| A redirect response exposes `.url.origin` for the final URL (§6.1) | `::test_redirect_response_exposes_final_origin` | Met | History entries too |
| httpx2 `Request` / `Response` remain subclasses of the 0.28.1 classes (§6.1) | `::test_httpx2_request_response_are_subclasses_with_distinct_identity` | Met | `issubclass` holds for both |
| Type identity with the 0.28.1 classes changes (§6.1) | `httpx2.Request is httpx.Request` → `False` | Met | Intentional; recorded in the allowlist and in `docs/architecture/python-bindings.md` |
| The 0.28.1 facade's URL construction is left unmodified (§6.1) | `git diff --stat -- crates/eggfetch-python/python/eggfetch/compat/httpx/` shows no change to `_urls.py` | Met | The regenerated 0.28.1 API manifest is identical to the baseline |
| The httpx2 `URL` stays a drop-in replacement for the base `URL` (§6.1) | `::test_httpx2_url_is_drop_in_for_base_url` | Met | Every base capability exercised; copies and joins stay httpx2 URLs |
| `Origin.__init__` still accepts both base and httpx2 URLs, with unchanged normalization (§6.1) | `::test_origin_accepts_base_and_httpx2_url`, `::test_url_origin_normalization_retained` | Met | Lowercased host, bracketed IPv6 unwrapped, IDNA decoded, default port |
| Importing httpx2 leaves the 0.28.1 facade unmodified, including `__all__` (§6.1) | `::test_importing_httpx2_leaves_zero_281_facade_unmodified` | Met | `__all__` and `dir()` snapshots identical before/after in a clean interpreter; no PyPI `httpx` import |
| httpx2 does not import the real PyPI `httpx` (§6.1) | same test — `sys.modules['httpx'] is None` | Met | — |
| The parity suite covers observable paths, not only direct construction (§10) | The new module drives the sync client, async client, redirect, and history against a local server | Met | 18 tests |
| Both API manifests are regenerated, not hand-edited, and both comparisons are green (§13) | `generate_httpx_api_manifest.py` run for both packages; `compare_httpx_api_manifest.py` exit 0 for both | Met | The two new allowlist entries were produced by the validated entries and verified by `--validate` |
| The 0.28.1 and httpx2 conformance suites are green (§13) | See §4 | Met | — |
| The httpx2 ledger entry states the real coverage (§13) | Recorded in `plans/httpx-parity-correction-status.md` | Met | Ledger updated in the same pass |
| Tier 1, Tier 2, and Tier 3 pass on the frozen executable SHA (§13) | `./scripts/check.sh`, `./scripts/check.sh extended`, and `./scripts/check.sh package` on `26fa619299fb8d447f25cbcb5eb713a1f537a30c` | Met | All three green; §10 |
| Both HTTPX profiles are rebound to that exact SHA (§13) | `compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml` rebound to `26fa619299fb8d447f25cbcb5eb713a1f537a30c` | Met | §7 and §10 |

## 3. Production implementation evidence

### Facade modules

| File | Change |
|---|---|
| `compat/httpx2/_urls.py` | Added `_as_httpx2_url()` / `_upgrade_httpx2_url()`, which adopt a 0.28.1 URL's parsed parts as an httpx2 `URL`; overrode `URL.copy_with` / `URL.join` so every derived URL keeps `origin`; overrode `URL.__new__` so a foreign 0.28.1 URL handed to `httpx2.URL(...)` is adopted rather than returned unchanged. `Origin` is untouched and still accepts both URL kinds with its existing normalization. |
| `compat/httpx2/_request.py` | Replaced the `import *` shim with `class Request(_BaseRequest)` (`__slots__ = ()`) whose `url` is always an httpx2 `URL`; added `_as_httpx2_request()`, which re-types a 0.28.1 request in place. |
| `compat/httpx2/_response.py` | Replaced the shim with `class Response(_BaseResponse)` whose `url` (and `request` / `next_request`) are httpx2-typed; added `_as_httpx2_response()`. |
| `compat/httpx2/_client.py` | `Client` / `AsyncClient` override `build_request`, `_run_request_hooks`, and `_send_single_request` so requests, redirect hops, history, and responses are httpx2-typed. |
| `compat/httpx2/__init__.py` | `Request` / `Response` now come from the httpx2 modules instead of the 0.28.1 ones. `__all__` is unchanged (76 symbols). |
| `compat/httpx2/_api.py`, `compat/httpx2/websockets/_api.py` | Typing import of `Response` retargeted to the httpx2 class. |
| `compat/httpx2/__init__.pyi` | Declares the httpx2 `Response` (a subclass of the 0.28.1 stub) and `Client.get` / `Client.request` return it. |

### Mechanism

Subclassing plus in-place re-typing. The 0.28.1 base builds its URL with the
base `URL` class inside `Request.__init__` and `Response.__init__`, and the
base **client** builds `Request` / `Response` objects with the base classes in
module-level functions (`_build_redirect_request`, `_wrap_response`) that
cannot be intercepted without editing the base module. Therefore:

- httpx2 `Request` / `Response` are real subclasses; identity with the 0.28.1
  classes is intentionally gone.
- URL coercion happens in httpx2's own modules, adopting a 0.28.1 URL's
  `_parts` / `_raw` unchanged.
- Objects the 0.28.1 client creates (redirect requests, wrapped responses) are
  re-typed **in place** — state, body, history, and elapsed are preserved
  exactly; no base-module change and no copying of base logic.

A second edge case was closed during review: the base `URL.__new__` returns its
argument unchanged when the argument is already a `URL`, so an inherited
`httpx2.URL(httpx.URL(...))` would have handed back a 0.28.1 URL with no
`origin`. `httpx2.URL.__new__` now adopts such a URL instead. This was
verified not to change the recorded API manifest.

### Compatibility fixtures and manifest

| File | Change |
|---|---|
| `crates/eggfetch-python/tests/compat/test_httpx2_origin_reachability.py` (new) | 18 tests covering the observable `origin` paths, the subclass and identity relationships, the 0.28.1 isolation, the cross-facade URL adoption, and the drop-in property. |
| `compat/httpx2/2.12.0/allowed-differences.toml` | Added `H2X-NEW-016` (`Request` base class) and `H2X-NEW-017` (`Response` base class) as `intentional-difference` / `intentional`, each pointing at the new test module. |
| `compat/httpx2/2.12.0/parity-cases.toml` | Case `H2X-API-002` now also lists `test_httpx2_origin_reachability.py` as its test evidence. |

The API manifests are regenerated by `scripts/generate_httpx_api_manifest.py`,
never hand-edited. The committed `compat/*/reference-api.json` files are
upstream goldens and are not produced by that script, so nothing committed was
regenerated or hand-edited; only the candidate manifests the script writes to
`/tmp` were compared.

## 4. Verification executed

### Commands run

```sh
.venv/bin/python scripts/generate_httpx_api_manifest.py --package eggfetch.compat.httpx  --output /tmp/eggfetch-manifest.json
.venv/bin/python scripts/generate_httpx_api_manifest.py --package eggfetch.compat.httpx2 --output /tmp/eggfetch-httpx2-manifest.json
.venv/bin/python scripts/compare_httpx_api_manifest.py --reference compat/httpx/0.28.1/reference-api.json  --candidate /tmp/eggfetch-manifest.json      --allowed compat/httpx/0.28.1/allowed-differences.toml
.venv/bin/python scripts/compare_httpx_api_manifest.py --reference compat/httpx2/2.12.0/reference-api.json --candidate /tmp/eggfetch-httpx2-manifest.json --allowed compat/httpx2/2.12.0/allowed-differences.toml
.venv/bin/python scripts/compare_httpx_api_manifest.py --allowed compat/httpx2/2.12.0/allowed-differences.toml --validate
.venv/bin/python -m pytest crates/eggfetch-python/tests/compat/test_httpx2_origin_reachability.py -v --strict-markers
.venv/bin/python -m pytest crates/eggfetch-python/tests/compat/test_httpx2_api_parity.py -v --strict-markers
.venv/bin/python -m pytest crates/eggfetch-python/tests/compat/test_httpx2_behavior.py -v --strict-markers
.venv/bin/python -m pytest crates/eggfetch-python/tests/compat/test_imports.py -v --strict-markers
.venv/bin/python -m pytest crates/eggfetch-python/tests/compat/ -k "httpx2" -q --strict-markers
.venv/bin/python -m pytest crates/eggfetch-python/tests/compat/test_behavior_cases.py crates/eggfetch-python/tests/compat/test_request_construction_parity.py -q
.venv/bin/python -m pytest crates/eggfetch-python/tests/compat/test_request.py crates/eggfetch-python/tests/compat/test_response.py crates/eggfetch-python/tests/compat/test_url_query.py crates/eggfetch-python/tests/compat/test_client.py crates/eggfetch-python/tests/compat/test_top_level_helpers_parity.py -q
.venv/bin/python -m pytest crates/eggfetch-python/tests/compat/test_httpx2_sse.py crates/eggfetch-python/tests/compat/test_httpx2_websocket.py -q
python scripts/check_python_typing.py
python scripts/check_adapter_features.py
```

### Results (pass/fail/skip with reason; counts without concealment)

| Check | Result |
|---|---|
| `test_httpx2_origin_reachability.py` | **18 passed** (17 from the implementation plus the cross-facade adoption case added during review) |
| `test_httpx2_api_parity.py` | 9 passed |
| `test_httpx2_behavior.py` | 5 passed |
| `test_imports.py` | 3 passed |
| `compat/ -k httpx2` | **50 passed**, 1901 deselected |
| 0.28.1 conformance oracle against real httpx 0.28.1 (`test_behavior_cases.py`, `test_request_construction_parity.py`) | 105 passed |
| 0.28.1 facade suite (`test_request.py`, `test_response.py`, `test_url_query.py`, `test_client.py`) | 212 passed |
| `test_httpx2_sse.py`, `test_httpx2_websocket.py` | 19 passed |
| `check_python_typing.py` | 4 fixtures passed |
| `check_adapter_features.py` | "Adapter feature ownership passed" |
| API manifest comparison, 0.28.1 | exit **0**, 0 unexplained differences |
| API manifest comparison, httpx2 | exit **0** after `H2X-NEW-016` / `H2X-NEW-017` were registered (before registration: exactly 2 unexplained base-class differences, both intended) |
| `--validate` on both allowlists | "Validation passed" for both |

### Observable behavior, before and after

Baseline reproduced from `git HEAD` in an isolated package copy:

```
BASELINE (git HEAD httpx2 facade)
httpx2.Request is httpx.Request : True
request.url type                : eggfetch.compat.httpx._urls.URL
request.url.origin              : AttributeError: 'URL' object has no attribute 'origin'
response.url.origin             : AttributeError: 'URL' object has no attribute 'origin'
httpx2.URL(...).origin          : Origin(scheme='https', host='example.com', port=443)
```

Working tree:

```
AFTER (working tree httpx2 facade)
httpx2.Request is httpx.Request : False
issubclass(httpx2.Request, httpx.Request)   : True
issubclass(httpx2.Response, httpx.Response) : True
request.url type                : eggfetch.compat.httpx2._urls.URL
request.url.origin              : Origin(scheme='https', host='example.com', port=443)
response.url.origin             : Origin(scheme='https', host='example.com', port=443)
sync client  response type      : eggfetch.compat.httpx2._response.Response
sync client  response.url.origin: Origin(scheme='http', host='127.0.0.1', port=40481)
redirect     final url          : http://127.0.0.1:40481/get
redirect     response.url.origin: Origin(scheme='http', host='127.0.0.1', port=40481)
redirect     history[0].url.origin: Origin(scheme='http', host='127.0.0.1', port=40481)
async client response.url.origin: Origin(scheme='http', host='127.0.0.1', port=40481)
async redirect final url        : http://127.0.0.1:40481/get
async redirect response.url.origin: Origin(scheme='http', host='127.0.0.1', port=40481)
```

0.28.1 isolation:

```
hasattr(httpx.URL, 'origin')    : False
hasattr(httpx.URL(...), 'origin'): False
hasattr(httpx, 'Origin')        : False
'Origin' in httpx.__all__       : False
httpx.Request url type          : eggfetch.compat.httpx._urls.URL
httpx.Response url type         : URL
```

The repository-wide gates were subsequently run at the bound SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c`: `./scripts/check.sh` (Tier 1),
`./scripts/check.sh extended` (Tier 2), and `./scripts/check.sh package`
(Tier 3) are all green, together with the live security preflight
(`./scripts/check_security.sh` → `advisories ok, bans ok, licenses ok, sources
ok`). Two host accommodations are recorded rather than hidden: the first
Tier 2 attempt failed with a linker SIGBUS (`ld terminated with signal 7`)
while compiling a test binary, which was host memory pressure from a
concurrent build on the same 16-CPU / 15-GiB host and not a code defect — the
same step re-verified clean immediately afterwards, the full gate then passed
end to end, and the rerun used `CARGO_BUILD_JOBS=6`; and the downstream
behavioral-fixture gate, which fails closed by design, executed rather than
skipping because the operator's artifact manifest was produced for this
qualification. §10 records the qualification evidence.

## 5. Invariant review

- **The 0.28.1 facade is unmodified.** `git diff` over
  `crates/eggfetch-python/python/eggfetch/compat/httpx/` shows no change to
  `_urls.py`, `_request.py`, `_response.py`, or `_client.py`; only the httpx2
  sibling and the httpx2 compatibility fixtures changed. The regenerated 0.28.1
  API manifest is identical to the baseline.
- **HTTPX 0.28.1 parity is preserved.** The 0.28.1 conformance oracle against
  the real upstream package passes (105 tests), as does the 0.28.1 facade
  suite (212 tests).
- **No second engine, no forked transport.** The httpx2 facade reuses the
  0.28.1 facade's client, its transport, and the same native extension. The
  change is typing and URL coercion only.
- **No new Python runtime dependency.** The change uses only the standard
  library and the existing 0.28.1 facade classes.
- **No new public export.** `httpx2.__all__` is unchanged at 76 symbols; the
  httpx2 `URL` constructor signature is unchanged; `Origin` was not altered.
- **The API manifests are generated, never hand-edited.** The only committed
  manifest change is the allowlist, which records an intentional delta.
- **No redaction regression.** No header value, cookie, or credential is
  rendered by the new coercion helpers.

## 6. Failure, timeout, pool, and recovery review

Not applicable in the usual sense: this milestone changes URL typing and
class relationships in the compatibility facade, not the request path,
transport, timeout, pool, or recovery logic.

Two behavioral notes are recorded:

- The in-place re-typing (`obj.__class__ = httpx2Class`) preserves the
  object's state, body, history, and elapsed time exactly. It does not copy
  the base implementation, so there is no second code path that could drift.
- The httpx2 `URL.__new__` override adopts a foreign 0.28.1 URL's `_parts` and
  `_raw` verbatim. It does not re-parse a display string, so no normalization
  difference can be introduced.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

Public-surface deltas, both intentional and both registered:

| Delta | Where recorded |
|---|---|
| `httpx2.Request` / `httpx2.Response` now declare a base class in the API manifest | `compat/httpx2/2.12.0/allowed-differences.toml` — `H2X-NEW-016`, `H2X-NEW-017` |
| `httpx2.Request is httpx.Request` is now `False` (isinstance relationships still hold) | `docs/architecture/python-bindings.md`, httpx2 subsection; and the `migration-impact` field of both allowlist entries |

The `URL` symbol in the httpx2 manifest is **unchanged** — its constructor
signature was deliberately preserved, including after the `__new__` override
added during review, which was verified not to alter the recorded manifest.

**Exact-SHA binding.** This milestone's changes — the Python package's
executable source, the compatibility fixtures, and the compatibility manifest
allowlist — are executable inputs under `.skills/verification-qualification.md`,
so they invalidated the previous live Stage C binding. The binding has since
been re-established: the ledger's Stage C SHA and both HTTPX profiles are
rebound to `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, which carries this
milestone's implementation (`d6c41adf4cf8f62ef990cccedf2f86512858fa0b`) and
the lockfile-only dependency patch on top of it. No Rust source differs
between the two commits.

## 8. Security review

- No transport, TLS, credential, proxy, or trust-store behavior changed. The
  httpx2 facade continues to use the same native engine as the 0.28.1 facade.
- `Origin` continues to normalize the host the same way: lowercased, IPv6
  brackets removed, IDNA-decoded punycode, default port for the scheme. The
  normalization was not loosened.
- The re-typing helpers copy only URL parts and the response/request state;
  they never copy or expose header values beyond what the objects already
  carried. Redaction of `authorization`, `proxy-authorization`, `cookie`, and
  `set-cookie` in `Debug` / `Display` / `__repr__` / errors is unaffected —
  the httpx2 classes inherit the 0.28.1 facade's implementations unchanged.
- The import-isolation test asserts that importing the httpx2 facade does not
  pull the real PyPI `httpx` into `sys.modules`, so the facade cannot silently
  delegate to an unvetted implementation.

## 9. Documentation and operations

| File | Change |
|---|---|
| `docs/architecture/python-bindings.md` | The httpx2 subsection: the shared-semantics list no longer claims `_request` / `_response` are shared with the 0.28.1 facade; two new module-table rows for `httpx2/_request.py` and `httpx2/_response.py`; a new paragraph stating the reachability guarantee and the intentional identity change; the new test module added to the differential-test list. |
| `plans/httpx-parity-correction-status.md` | The httpx2 parity entry now states the real coverage — including that `URL.origin` is reachable from request, response, client, and redirect observables — and records that the executable freeze changed and that the Stage C binding required re-establishment. |

Operations: no new dependency, no network access, no credential. The change is
confined to the Python package's facade modules and its compatibility
fixtures.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| Resolved | The live Stage C binding was invalidated by this milestone's Python source, fixture, and manifest changes, and the Stage C rebinding had not been performed at the time of implementation | The previous binding could not be cited by any publication | **Resolved 2026-10-22.** Both HTTPX profiles and the ledger's Stage C SHA were rebound to `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, and the frozen-SHA qualification was re-run |
| Resolved | Tier 1, Tier 2, and Tier 3 had not been run on the frozen SHA | The repository-wide gates were unproven for this tree | **Resolved 2026-10-22.** Tier 1, Tier 2, and Tier 3 are green on `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, together with the live security preflight |
| Medium | `httpx2.Request` and `httpx2.Response` are no longer identical to their 0.28.1 counterparts | A caller that used `httpx2.Request is httpx.Request` as a feature test now sees `False`. `isinstance` / `issubclass` relationships are preserved | Intentional and recorded. Documented in `docs/architecture/python-bindings.md` and in the `migration-impact` field of `H2X-NEW-016` / `H2X-NEW-017` |
| Low | The httpx2 `URL` constructor's recorded signature must stay identical to the reference; a future change to it would need a new allowlist entry and would invalidate `H2X-INH-060..063` | Any future change to the httpx2 `URL` constructor is a manifest break | Keep the constructor signature unchanged; add a new allowlist entry if it must change |
| Low | The host filesystem had roughly 122 GB free during the frozen-SHA qualification | No space pressure was observed for the Tier 1 / Tier 2 / Tier 3 runs | None. Monitor before future full-workspace builds |

## 11. Roadmap disposition

`plans/subsystems/python-httpx-compat-roadmap.md` milestone 4 moves from
`conditionally closed` to `closed`, with §11 (completion definition) and §12
(milestone status) updated in the same pass.

The milestone's own scope is complete, and the cross-cutting release evidence
it was waiting on — Tier 1 / Tier 2 / Tier 3 on the frozen SHA and the Stage C
rebinding — was delivered on the bound SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c`. The milestone's own scoped
findings — the `httpx2.Request` / `httpx2.Response` base-class deltas
(`H2X-NEW-016` / `H2X-NEW-017`) and the `URL` constructor signature freeze —
remain open and are owned by the python-httpx-compat subsystem; they need a
maintainer decision, not more release evidence. Milestone 5 in the same
subsystem is not blocked by this one.

## 12. Registry updates

`plans/registry.md`:

- The python-httpx-compat row records milestone 4 as closed.
- The implementation-plan table for M004 points at this closure record.
- The blocked / operational work section no longer records the Stage C
  rebinding as pending for the milestones closed in this pass; the rebinding
  was delivered on `26fa619299fb8d447f25cbcb5eb713a1f537a30c`.
