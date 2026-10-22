# Python Bindings and HTTPX Compatibility Milestone 004 — httpx2 URL Origin Reachability

Status: ready

Repository baseline: `873ea28541c82817317430c9523fca8525d99c81`

Source roadmap:

- `plans/subsystems/python-httpx-compat-roadmap.md#milestone-4--httpx2-url-origin-reachability`

Long-term requirements:

- `plans/000-long-term-specification.md` §4.2, §4.3, §8
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 items 2 and 4

Applicable ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`
- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`

Primary class: capability

## 1. Objective

Make the httpx2 `URL.origin` delta reachable from every httpx2 path that
produces a URL, not only from a directly constructed `httpx2.URL`.

At the baseline, `eggfetch.compat.httpx2.URL` subclasses the 0.28.1 facade's
`URL` and adds an `origin` property, but `eggfetch.compat.httpx2._request`
and `eggfetch.compat.httpx2._response` are re-export shims of the 0.28.1
facade's modules. The httpx2 `Request` and `Response` classes are therefore
the 0.28.1 classes, whose `url` attribute is an instance of the *base* URL
class. The base class has no `origin` property.

The result is that the httpx2 headline delta is unreachable from the objects a
user actually holds:

```python
httpx2.Request is httpx.Request          # True
httpx2.Request("GET", "https://example.com/x").url
# -> eggfetch.compat.httpx._urls.URL      # no .origin
httpx2.URL("https://example.com/x").origin
# -> Origin(scheme='https', host='example.com', port=443)
```

`httpx2.Origin(base_url)` does work, because `Origin.__init__` accepts any
base `URL`. But a user following httpx2's documented API writes
`request.url.origin`, and that raises `AttributeError`.

This milestone does not change the 0.28.1 facade, which must not gain an
`origin` attribute that httpx 0.28.1 does not have.

## 2. Why this milestone is ready

- The defect is bounded to the httpx2 facade's Python surface; it needs no
  engine change and no public Rust surface change.
- The required end state is unambiguous: every httpx2 URL observable by a
  user must expose `origin`, and no 0.28.1 URL may.
- ADR-0001's single-engine boundary is untouched; this is facade shaping of
  an already-computed value.
- The httpx2 facade already establishes the precedent of subclassing the
  0.28.1 facade rather than copying it, so the fix extends an existing
  pattern instead of introducing one.
- The httpx2 stage is a sibling facade whose parity is bound to the live
  Stage C SHA, so the fix requires exact-SHA requalification, which is a
  known and bounded cost.

## 3. Current implementation evidence

At the baseline:

- `crates/eggfetch-python/python/eggfetch/compat/httpx2/_urls.py` defines
  `class URL(_BaseURL)` with an `origin` property returning `Origin(self)`,
  and a frozen `Origin` dataclass whose `__init__` accepts a base `_BaseURL`;
- `crates/eggfetch-python/python/eggfetch/compat/httpx2/_request.py` and
  `_response.py` are `import *` re-export shims;
- `crates/eggfetch-python/python/eggfetch/compat/httpx2/__init__.py` imports
  `Request` and `Response` directly from `eggfetch.compat.httpx._request` and
  `eggfetch.compat.httpx._response`;
- the base `Response` stores `self._url` and its `url` setter coerces with
  the base `URL` class;
- the base `Request.__init__` builds its URL with the base `URL` class;
- `crates/eggfetch-python/tests/compat/test_httpx2_api_parity.py` covers
  `Origin` normalization and `httpx2.URL(...).origin`, but constructs the URL
  directly and never asserts `request.url.origin` or `response.url.origin`;
- the ledger records the httpx2 core-facade parity program as covering
  "Origin/URL.origin", and the import-isolation check asserts no `Origin`
  leakage into the 0.28.1 facade.

## 4. Invariants that must not regress

- The 0.28.1 facade must not gain `URL.origin`, `Origin`, or any other httpx2
  delta. httpx 0.28.1 has no `origin` property.
- The two facades must continue to coexist without cross-mutation.
- Importing `eggfetch.compat.httpx2` must continue to leave
  `eggfetch.compat.httpx` unmodified, and the import-isolation check must
  remain green.
- `eggfetch.compat.httpx2` must not import the real PyPI `httpx`.
- The native `eggfetch` surface is untouched by this milestone.
- No engine change, no public Rust surface change, no new dependency.
- Existing httpx2 behavior for redirects, cookies, auth, proxies, timeouts
  and retries is unchanged.

## 5. Scope

### In scope

- httpx2 `Request` and `Response` must expose httpx2 `URL` instances
  wherever a user can observe `.url`;
- `httpx2.Client` and `httpx2.AsyncClient` responses must expose an httpx2
  `URL` through `.url`;
- httpx2 redirect responses must expose an httpx2 `URL` through `.url`;
- parity tests covering `request.url.origin`, `response.url.origin`, and the
  client-returned response path;
- the API manifest regenerated for both facades if the public Python surface
  changes shape;
- correction of the httpx2 parity ledger entry so it states what is actually
  covered.

### Explicitly out of scope

- adding `origin` to the 0.28.1 facade;
- adding httpx2 deltas beyond `origin` (the `query()`, `sse()` and
  `websocket()` deltas already exist and are not in scope);
- changing `Origin`'s normalization rules;
- the compat policy-authority work tracked separately by python-httpx-compat
  M005;
- HTTPX 1.0 support;
- engine, CLI, FFI and Node work;
- publication, version bump, or release orchestration.

## 6. Required production changes

### 6.1 httpx2 URL type at every observation point

The httpx2 facade must ensure that every `url` a user can observe is an
instance of the httpx2 `URL` class, so that `.origin` is present.

The implementer may choose the mechanics, but the design constraints are:

- the httpx2 `Request` and `Response` must remain **subclasses** of the
  0.28.1 facade's `Request` and `Response`, so that existing isinstance
  relationships and any code accepting a 0.28.1 request/response keeps
  working;
- type **identity** with the 0.28.1 classes is not required and is expected
  to change: `httpx2.Request is httpx.Request` must not remain true, because
  that identity is the cause of the defect;
- the httpx2 `Client` and `AsyncClient` must produce httpx2 `Response`
  instances, and those responses must carry an httpx2 `URL`;
- redirect-following responses must carry an httpx2 `URL` as well;
- the base 0.28.1 facade must be left unmodified in its URL construction, so
  that httpx 0.28.1 parity is untouched.

If subclassing the base `Request`/`Response` proves insufficient because the
base classes construct their URL in a way that cannot be intercepted without
modifying the base module, the implementation must introduce the httpx2
classes in httpx2's own module and construct httpx2 URLs there, without
changing the base module's behavior.

### 6.2 `Origin` acceptance of httpx2 URLs

`Origin.__init__` already accepts a base `_BaseURL`. It must continue to
accept both the base and the httpx2 `URL`, and must keep its current
normalization: lowercased host, bracketed IPv6 hosts unwrapped and
re-normalized, IDNA-decoded punycode, and the default port for the scheme.

### 6.3 Parity test coverage

The httpx2 parity suite must assert the delta through the *observable* paths,
not only through a directly constructed URL:

- `httpx2.Request(...).url.origin` resolves to the expected `Origin`;
- `httpx2.Response(...).url.origin` resolves to the expected `Origin`;
- a response returned by `httpx2.Client(...).get(...)` exposes
  `response.url.origin`;
- the same for `httpx2.AsyncClient(...)`;
- a response obtained after a redirect exposes `response.url.origin` for the
  final URL;
- `httpx2.Origin` is not reachable from the 0.28.1 facade: `httpx.URL` has
  no `origin` attribute and `httpx` does not export `Origin`;
- the 0.28.1 facade's own URL behavior is unchanged.

The existing direct-construction coverage must be retained.

### 6.4 API manifest and ledger truth

If the public Python surface of the httpx2 facade changes shape, the API
manifest must be **regenerated** with
`scripts/generate_httpx_api_manifest.py` and compared with
`scripts/compare_httpx_api_manifest.py`. The manifests must never be
hand-edited.

The httpx2 parity ledger entry must be corrected so it states that `origin`
is covered through `request.url` and `response.url`, not only through a
directly constructed `httpx2.URL`. The ledger is the live qualification
record; a case that claims coverage it did not have must be corrected, not
silently extended.

## 7. Ordered work packages

1. Add failing parity tests for `request.url.origin`, `response.url.origin`,
   and the client-returned response path.
2. Introduce httpx2 `Request`/`Response` classes that carry httpx2 `URL`
   instances, leaving the 0.28.1 facade unmodified.
3. Ensure the httpx2 clients and the redirect path produce httpx2 responses
   with httpx2 URLs.
4. Add the 0.28.1 isolation assertions to the same test module.
5. Run the httpx2 parity, behavior, SSE and WebSocket suites.
6. Regenerate and compare both API manifests if the surface changed.
7. Correct the httpx2 parity ledger entry.
8. Run Tier 1 and Tier 2, then Tier 3 where required.
9. Renew the exact-SHA Stage C binding for both profiles.
10. Write `plans/closure/python-httpx-compat/004-httpx2-origin-reachability.md`.

## 8. Failure, cancellation, timeout, and pool semantics

- This milestone changes only Python-level type construction. No request
  preparation, dispatch, timeout, pool or cancellation behavior changes.
- The httpx2 `URL` must remain a drop-in replacement for the base `URL` for
  every operation the base class supports. If the httpx2 class narrows any
  base capability, that is a defect.
- No request body, stream, or connection lifecycle behavior may change.
- The 0.28.1 facade's request lifecycle must be bit-for-bit unchanged.

## 9. Compatibility and migration

- httpx2 users gain `request.url.origin` and `response.url.origin`.
- Code that relied on `httpx2.Request is httpx.Request` or
  `httpx2.Response is httpx.Response` stops working. This identity was an
  accident of re-export, not a documented contract, and it is the cause of
  the defect. The release note must state the change explicitly.
- The 0.28.1 facade is unchanged, so 0.28.1 users are unaffected.
- Feature-profile effects: none. This is Python-surface only.
- The httpx2 API manifest may change; it must be regenerated, not edited.
- Both HTTPX profiles are executable inputs and require renewed exact-SHA
  qualification under `.skills/verification-qualification.md`.

## 10. Required tests

### httpx2 delta reachability

- `httpx2.Request("GET", url).url` is an httpx2 `URL`;
- `httpx2.Request("GET", url).url.origin` equals the expected `Origin`;
- `httpx2.Response(200, request=...).url.origin` equals the expected
  `Origin`;
- a `httpx2.Client` GET response exposes `.url.origin`;
- a `httpx2.AsyncClient` GET response exposes `.url.origin`;
- a redirect response exposes `.url.origin` for the final URL;
- `httpx2.URL(...).origin` retains its existing normalization behavior.

### 0.28.1 isolation

- `httpx.URL` has no `origin` attribute;
- `httpx` does not export `Origin`;
- `httpx.Request`/`httpx.Response` behavior is unchanged;
- importing `eggfetch.compat.httpx2` leaves `eggfetch.compat.httpx`
  unmodified, including its `__all__`;
- the existing import-isolation test remains green.

### Regression

- the full httpx2 parity, behavior, SSE and WebSocket suites remain green;
- the 0.28.1 conformance suite remains green;
- the native `eggfetch` surface is unchanged.

## 11. Required verification commands

Follow `docs/verification-policy.md` and
`.skills/verification-qualification.md`.

Rebuild the native module first, because the Python surface changed:

```sh
maturin develop -m crates/eggfetch-python/Cargo.toml
```

Focused first:

```bash
python -m pytest crates/eggfetch-python/tests/compat/test_httpx2_api_parity.py -v --strict-markers
python -m pytest crates/eggfetch-python/tests/compat/test_httpx2_behavior.py -v --strict-markers
python -m pytest crates/eggfetch-python/tests/compat/test_imports.py -v --strict-markers
```

Then:

```bash
python -m pytest crates/eggfetch-python/tests/ -q --ignore=crates/eggfetch-python/tests/compat
EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Regenerate the API manifests when the public surface changed, then compare:

```bash
python scripts/generate_httpx_api_manifest.py --package eggfetch.compat.httpx --output /tmp/eggfetch-manifest.json
python scripts/generate_httpx_api_manifest.py --package eggfetch.compat.httpx2 --output /tmp/eggfetch-httpx2-manifest.json
python scripts/compare_httpx_api_manifest.py
```

## 12. Documentation updates

Implementation must update:

- `docs/architecture/python-bindings.md` (httpx2 facade section, describing
  where the httpx2 delta types are reachable from);
- `docs/reference/compatibility.md` if the httpx2 delta surface description
  changes;
- `docs/residual-differences.md` if any residual difference is introduced or
  removed;
- `plans/httpx-parity-correction-status.md` (the httpx2 parity ledger entry
  correction, and the renewed exact-SHA binding);
- `plans/registry.md`, the subsystem roadmap and `plans/README.md` at
  closure.

## 13. Acceptance criteria

M004 closes only when:

- `httpx2.Request(...).url.origin` and `httpx2.Response(...).url.origin`
  resolve to the expected `Origin`;
- a response returned by the httpx2 sync and async clients exposes
  `.url.origin`;
- a redirect response exposes `.url.origin` for the final URL;
- `httpx.URL` still has no `origin` attribute and `httpx` still exports no
  `Origin`;
- importing `eggfetch.compat.httpx2` leaves the 0.28.1 facade unmodified;
- the httpx2 parity suite covers the observable paths, not only direct
  construction;
- the httpx2 ledger entry states the real coverage;
- both API manifests are regenerated, not hand-edited, and both comparisons
  are green;
- the 0.28.1 and httpx2 conformance suites are green;
- Tier 1, Tier 2 and Tier 3 pass on the frozen executable SHA;
- both HTTPX profiles are rebound to that exact SHA.

## 14. Stop conditions

Stop and revise this plan rather than improvising if:

- making the httpx2 URL observable everywhere requires modifying the 0.28.1
  facade's URL construction, which would put httpx 0.28.1 parity at risk;
- the httpx2 `Request`/`Response` cannot remain subclasses of the 0.28.1
  classes without breaking an existing httpx2 consumer;
- the httpx2 `URL` class cannot carry the base class's full behavior, which
  would make the delta a narrowing;
- the regenerated API manifest shows a difference that cannot be explained
  as the intended httpx2 delta;
- a facade change would require an engine change to remain correct.

## 15. Closure evidence required

Create `plans/closure/python-httpx-compat/004-httpx2-origin-reachability.md` containing:

- implementation commit(s) and the final frozen SHA;
- before/after evidence for each observable `origin` path;
- evidence that the 0.28.1 facade gained no `origin`/`Origin`;
- the regenerated API manifests and both comparison outcomes;
- the corrected httpx2 parity ledger entry;
- the exact Tier 1/2/3 outcomes;
- renewed HTTPX Stage C binding evidence for both profiles;
- the documented `httpx2.Request is httpx.Request` identity change and its
  downstream impact;
- known limitations.

## 16. Handoff notes

The fix is Python-surface only. Do not touch the engine, and do not add an
engine API that reports a URL type.

The httpx2 facade intentionally subclasses the 0.28.1 facade rather than
copying it. Extend that pattern; do not fork the base modules.

This milestone changes `httpx2.Request is httpx.Request` from `True` to
`False`. That is intended, but it is a user-visible identity change and must
appear in the release note and the closure evidence.

The ledger correction is not optional. A parity entry that claims coverage
of "Origin/URL.origin" while only covering a directly constructed URL is the
root cause of this defect escaping, and leaving it in place would let the
same class of gap recur.
