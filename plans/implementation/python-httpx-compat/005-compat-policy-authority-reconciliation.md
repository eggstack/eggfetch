# Python Bindings and HTTPX Compatibility Milestone 005 — Compat Policy Authority Reconciliation

Status: ready

Repository baseline: `873ea28541c82817317430c9523fca8525d99c81`

Source roadmap:

- `plans/subsystems/python-httpx-compat-roadmap.md#milestone-5--compat-policy-authority-reconciliation`

Long-term requirements:

- `plans/000-long-term-specification.md` §4.2, §4.3, §8
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 items 2 and 4

Applicable ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`
- `plans/adrs/ADR-0009-compat-policy-authority.md`

Primary class: invariant

## 1. Objective

Establish and enforce a written boundary for which side owns HTTPX-shaped
policy between the engine and the compat facades, remove the two live
disagreements that boundary currently hides, and stop compat-only
environment semantics from reaching the native Python surface.

Three concrete problems are in scope:

1. **Two cookie engines with opposite verdicts.** `eggfetch-core` rejects a
   `Set-Cookie` carrying an explicit `Domain` that names an IP address
   (`crates/eggfetch-core/src/cookie.rs`), while the facades' standard
   library jar accepts it. Both jars observe the same response and both can
   influence what is sent.

2. **Two redirect-policy implementations.** The engine strips
   `Authorization` on every redirect hop; the facade strips it only
   cross-origin and preserves it for same-origin hops and for `https` to
   `http` downgrades. The method-rewrite table exists in both.

3. **Compat-only semantics on the native Python surface.** The native client
   uses the compat `NO_PROXY` parser instead of the native one, and the
   compat `socks5://` to `socks5h://` normalization applies to native users.

## 2. Why this milestone is ready

ADR-0009 records the decision and its rules:

- the engine owns HTTP semantics, and a facade owns HTTPX-shaped behavior;
- where one state is observable from both sides, the two must agree or the
  divergence must be recorded;
- the engine's redirect policy is authoritative for the wire, and a facade
  that loops redirects itself must match it or document the difference;
- compat-only environment semantics must not reach the native surface;
- the engine's typed rebuild helpers remain the only place a hop request is
  constructed.

Each of the three problems has a bounded, executable resolution, and the
measurements needed to choose between the permitted resolutions can be taken
directly from the existing test fixtures. No decision in this milestone
depends on the HTTPX 1.0 program, H3 graduation, or Node maturation.

## 3. Current implementation evidence

At the baseline:

- `crates/eggfetch-core/src/cookie.rs` rejects an explicit `Domain`
  attribute naming an IP address;
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_cookies.py` defines
  `Cookies` on top of `http.cookiejar.CookieJar`;
- the standard library jar accepts `Set-Cookie: q=1; Domain=127.0.0.1` and
  stores it with `domain_specified=True`; the engine jar rejects it;
- the engine's redirect path strips `Authorization` and
  `Proxy-Authorization` on every hop and re-attaches auth through
  `resolve_request_auth`;
- the facade's redirect loop strips credentials only when the origin changes,
  and separately implements the method-rewrite table;
- `crates/eggfetch-python/src/request_preparation.rs` selects the compat
  `NO_PROXY` parser on the shared client-configuration path used by the
  native client;
- the compat SOCKS normalization that rewrites `socks5://` to `socks5h://` is
  applied on the shared path and therefore reaches native users;
- `docs/residual-differences.md` has no cookie section and no
  redirect-policy section;
- `crates/eggfetch-python/tests/compat/test_cookie_scope_parity.py` and
  `test_redirect_state_machine_parity.py` exist and are the natural homes
  for the new conformance fixtures;
- `crates/eggfetch-python/tests/compat/test_no_proxy_differential.py` covers
  the `NO_PROXY` parser difference.

## 4. Invariants that must not regress

- HTTPX 0.28.1 and httpx2 2.12.0 conformance is preserved exactly. Where
  HTTPX's own behavior differs from the engine's, HTTPX wins for the facade.
- The native Rust surface keeps its current behavior except where ADR-0009
  rule 3 requires a deliberate cookie-rule alignment, which must be
  justified explicitly in the closure record.
- The native Python surface keeps using the native `NO_PROXY` parser and does
  not rewrite the SOCKS scheme.
- Redaction of `authorization`, `proxy-authorization`, `cookie` and
  `set-cookie` in all Debug/Display/`__repr__`/error output is unchanged.
- The engine's typed rebuild helpers remain the only place a hop request is
  constructed.
- No new cargo dependency, and no new Python runtime dependency.
- The two facades continue to coexist without cross-mutation.
- The native `eggfetch` surface and the compat facades remain independently
  importable and independently mutable.

## 5. Scope

### In scope

- measurement of the observable difference between the engine's cookie rules
  and the facade's cookie rules;
- resolution of that difference under ADR-0009 rule 3, with the outcome
  recorded in `docs/residual-differences.md`;
- alignment or documented justification of the facade's redirect credential
  rule against the engine's rule;
- confirmation of whether the facade's duplicated method-rewrite table is
  equivalent to the engine's, and a shared conformance fixture if it is;
- native Python surface isolation for `NO_PROXY` parsing and SOCKS scheme
  normalization;
- conformance fixtures for the cookie and redirect rules;
- `docs/residual-differences.md` sections for cookies and redirect policy.

### Explicitly out of scope

- merging the engine's cookie jar and the facade's jar into one type;
- changing the engine's cookie rules for reasons other than the ADR-0009
  rule 3 alignment;
- changing the facade's `Cookies` public type or its `http.cookiejar`
  backing;
- changing HTTPX compatibility in either direction;
- the httpx2 `origin` reachability work tracked separately by
  python-httpx-compat M004;
- HTTPX 1.0 support;
- CLI, FFI, Node and engine transport work;
- publication, version bump, or release orchestration.

## 6. Required production changes

### 6.1 Measure before changing

The first deliverable is a measurement, not a code change. Build a fixture
that feeds one identical `Set-Cookie` header sequence through:

- the engine's jar, and
- the facade's `http.cookiejar`-backed jar,

and record, per case, whether each engine accepts the cookie, the stored
domain, the host-only/domain-specified flag, and the path scoping.

Cases must include at minimum: no `Domain`; an explicit matching `Domain`;
an explicit non-matching `Domain`; an explicit `Domain` naming an IPv4
literal; an explicit `Domain` naming an IPv6 literal; a leading-dot `Domain`;
a `Domain` with an unrelated registrable suffix; `Path` scoping variants;
and `Secure`/`Max-Age`/`Expires` variants.

The fixture must report the *observable* effect as well as the internal
verdict, because the engine's jar and the facade's jar influence the request
through different paths. The fixture must therefore also record, for the
same response, what `Cookie` header each engine would contribute.

This measurement is required evidence for the closure record regardless of
which resolution is chosen.

### 6.2 Resolve the cookie disagreement

Choose exactly one of the two resolutions ADR-0009 rule 3 permits:

- **Resolution A — align the engine.** Relax the engine's rule so that an
  explicit `Domain` naming an IP address is accepted, matching the standard
  library's verdict. The engine's rule is then consistent with the facade's
  for every measured case.
- **Resolution B — subordinate the facade.** In the facade request path,
  ensure only one jar ever decides what is sent, so the engine's jar cannot
  contradict the facade's.

If the measurement shows that the two engines already agree on every case
that matters, record that finding and record any residual difference in
`docs/residual-differences.md` instead of changing behavior.

The chosen resolution must not change the facade's public `Cookies` type and
must not change HTTPX conformance.

If Resolution A is chosen, the engine's behavior change affects the native
Rust surface and must be justified in the closure record as a deliberate
alignment with the facade's HTTPX-defined rule, not as a side effect.

### 6.3 Align or justify the redirect credential rule

The engine strips `Authorization` and `Proxy-Authorization` on every redirect
hop. The facade strips them only when the origin changes, and preserves them
for same-origin hops and for `https` to `http` downgrades.

The implementation must:

- determine whether preserving credentials across an `https` to `http`
  downgrade is required for HTTPX conformance, or is an artifact of the
  facade's implementation;
- if HTTPX requires it, keep it and record the difference, with the
  downgrade case called out explicitly because it sends credentials in clear
  text, in `docs/residual-differences.md`;
- if HTTPX does not require it, adopt the engine's rule in the facade;
- add a conformance fixture that drives the same redirect chain through the
  engine and through the facade and asserts an agreed credential outcome.

The method-rewrite table must be compared between the engine and the facade.
If they are equivalent, a shared fixture must prove it so the two cannot
drift silently. If they are not equivalent, the difference must be recorded
and the HTTPX-conformance requirement identified.

### 6.4 Isolate the native surface

- The native `eggfetch` Python client must use the native `NO_PROXY` parser.
  The compat facades keep the compat parser. The CLI already uses the native
  parser and is unchanged.
- The compat `socks5://` to `socks5h://` normalization must apply only to the
  compat facades. The native surface must pass the scheme through unchanged.
- If the native surface needs an equivalent capability, it is added as an
  explicit native-surface option, never as an inherited compat default.

Both changes are user-visible for native Python users and must be documented
in the native surface's documentation and in
`docs/residual-differences.md`.

### 6.5 Residual-difference documentation

`docs/residual-differences.md` must gain:

- a **cookie** section stating, per measured case, whether the engine and the
  facade agree, and describing the resolution;
- a **redirect policy** section stating the credential-stripping rule on each
  side, the method-rewrite rule on each side, and any agreed difference.

`docs/residual-differences.md` is a canonical qualification record. It must
not be used to paper over a difference that should have been fixed; it
records differences that are intentional, required by HTTPX, or explicitly
accepted.

## 7. Ordered work packages

1. Build the cookie conformance fixture and record the measurement.
2. Build the redirect conformance fixture and record the measurement.
3. Choose and implement the cookie resolution, or record that no change is
   required.
4. Align or justify the facade's redirect credential rule.
5. Prove method-rewrite equivalence, or record the difference.
6. Isolate the native surface's `NO_PROXY` parsing and SOCKS scheme.
7. Update `docs/residual-differences.md`.
8. Run the full 0.28.1 and httpx2 conformance suites.
9. Regenerate and compare both API manifests if the public Python surface
   changed.
10. Run Tier 1, Tier 2, then Tier 3 where required.
11. Renew the exact-SHA Stage C binding for both profiles.
12. Write `plans/closure/python-httpx-compat/005-compat-policy-authority-reconciliation.md`.

## 8. Failure, cancellation, timeout, and pool semantics

- No request may be issued with a cookie header that contradicts the rules
  that produced it. Where a resolution changes which jar is consulted, the
  change must be atomic with respect to a single request: a request must not
  observe a partially-updated jar.
- The engine's cookie jar must not gain a second copy of every cookie just
  because the facade's jar also tracks it.
- Redirect credential handling must not consume or alter the request's total
  deadline, and must not change the number of hops performed.
- Native-surface `NO_PROXY` and SOCKS changes affect route selection only;
  they must not change timeouts, pool behavior, or connection reuse rules
  beyond which destination is selected.
- No cancellation or streaming behavior changes.

## 9. Compatibility and migration

- Compat facades: HTTPX conformance must be preserved exactly. Any facade
  behavior change requires an HTTPX conformance argument recorded in the
  closure record.
- Native Rust callers: affected only if Resolution A is chosen. That change
  is a cookie-rule alignment and must be stated explicitly in the closure
  record and in the release note.
- Native Python callers: environment `NO_PROXY` parsing and SOCKS scheme
  handling change. Users who relied on the compat `NO_PROXY` parser must
  migrate to the native behavior; the release note must say so, and the
  native surface's documentation must state the native semantics.
- Feature-profile effects: none.
- The engine and the facades are executable inputs. All changes require
  renewed exact-SHA Stage C qualification of both the 0.28.1 and 2.12.0
  profiles under `.skills/verification-qualification.md`.

## 10. Required tests

### Cookie conformance

- the fixture covers every case listed in section 6.1;
- per case, the engine's and the facade's verdicts are asserted to agree,
  except for cases explicitly recorded in `docs/residual-differences.md`;
- the observable `Cookie` header contribution of each engine is asserted;
- the facade's `client.cookies` still reflects the server's `Set-Cookie`
  exactly as HTTPX does.

### Redirect conformance

- one redirect chain, driven through the engine and through the facade,
  asserts an agreed credential outcome;
- same-origin redirect credential handling is asserted explicitly;
- cross-origin redirect credential handling is asserted explicitly;
- the `https` to `http` downgrade case is asserted explicitly, whatever the
  chosen resolution;
- the method-rewrite sequence is asserted on both sides and shown equivalent
  or the difference is recorded;
- the redirect hop count and the total deadline are unchanged.

### Native surface isolation

- the native Python client parses `NO_PROXY` with the native parser, proven
  by a `NO_PROXY` value where the two parsers disagree;
- the native Python client leaves a `socks5://` scheme unchanged;
- the compat facades retain the compat `NO_PROXY` behavior and the compat
  SOCKS normalization;
- the CLI's `NO_PROXY` behavior is unchanged.

### Regression

- the full 0.28.1 and httpx2 conformance suites remain green;
- the native Python surface test suite remains green;
- redaction tests still pass;
- both API manifests are regenerated if the surface changed, and both
  comparisons are green.

## 11. Required verification commands

Follow `docs/verification-policy.md` and
`.skills/verification-qualification.md`.

Rebuild the native module first:

```sh
maturin develop -m crates/eggfetch-python/Cargo.toml
```

Focused first:

```bash
python -m pytest crates/eggfetch-python/tests/compat/test_cookie_scope_parity.py -v --strict-markers
python -m pytest crates/eggfetch-python/tests/compat/test_redirect_state_machine_parity.py -v --strict-markers
python -m pytest crates/eggfetch-python/tests/compat/test_no_proxy_differential.py -v --strict-markers
python -m pytest crates/eggfetch-python/tests/compat/test_socks_transport.py -v --strict-markers
```

Then:

```bash
python -m pytest crates/eggfetch-python/tests/ -q --ignore=crates/eggfetch-python/tests/compat
EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

## 12. Documentation updates

Implementation must update:

- `docs/residual-differences.md` (new cookie and redirect-policy sections);
- `docs/architecture/python-bindings.md` (the facade-versus-engine ownership
  boundary, the native surface's environment semantics);
- `docs/concepts/proxy.md` if the native `NO_PROXY` semantics are documented
  there;
- `docs/migration/from-httpx.md` if the native-surface change affects a
  documented migration path;
- `plans/httpx-parity-correction-status.md` (renewed exact-SHA binding and
  any parity-case corrections);
- `plans/registry.md`, the subsystem roadmap and `plans/README.md` at
  closure.

## 13. Acceptance criteria

M005 closes only when:

- the cookie conformance fixture exists, covers every case in section 6.1,
  and its measurement is recorded in the closure record;
- the engine and the facade agree on every measured cookie case, or each
  remaining difference is explicitly recorded in
  `docs/residual-differences.md` with its HTTPX or safety justification;
- the facade's redirect credential rule either matches the engine's or is
  documented with the `https` to `http` downgrade case called out;
- the method-rewrite tables are shown equivalent by a shared fixture, or the
  difference is recorded;
- the native Python client uses the native `NO_PROXY` parser and leaves the
  SOCKS scheme unchanged;
- the compat facades retain their compat `NO_PROXY` and SOCKS behavior;
- `docs/residual-differences.md` has cookie and redirect-policy sections;
- HTTPX 0.28.1 and httpx2 2.12.0 conformance is preserved exactly;
- both API manifests are regenerated, not hand-edited, and both comparisons
  are green;
- Tier 1, Tier 2 and Tier 3 pass on the frozen executable SHA;
- both HTTPX profiles are rebound to that exact SHA.

## 14. Stop conditions

Stop and revise ADR-0009 rather than improvising if:

- the measurement shows the two cookie engines disagree on a case where
  neither resolution is safe, in which case the disagreement must be
  escalated rather than papered over;
- adopting the engine's redirect credential rule would break HTTPX
  conformance in a way that cannot be reconciled;
- isolating the native surface's `NO_PROXY` behavior would break a published
  native Python consumer that depends on the compat parser;
- the two cookie jars cannot be made to agree without merging them, which
  ADR-0009 explicitly defers;
- any change would require a second engine or a second request path.

## 15. Closure evidence required

Create `plans/closure/python-httpx-compat/005-compat-policy-authority-reconciliation.md` containing:

- implementation commit(s) and the final frozen SHA;
- the full cookie measurement table, before and after;
- the chosen cookie resolution and its justification;
- the redirect credential and method-rewrite comparison;
- the native-surface isolation evidence;
- the regenerated API manifests and both comparison outcomes;
- the exact Tier 1/2/3 outcomes;
- renewed HTTPX Stage C binding evidence for both profiles;
- the `docs/residual-differences.md` sections as written;
- known limitations, including that the two cookie jars remain two
  implementations and that the facade's redirect loop remains in Python;
- the native-Python user-visible change, stated for the release note.

## 16. Handoff notes

Measure before changing. The measurement fixture is the deliverable that
makes the rest of this milestone defensible, and it is the artifact a
maintainer should read first.

Do not merge the two cookie jars in this milestone. ADR-0009 explicitly
defers that, and merging it would change the facade's public `Cookies` type.

Do not resolve a disagreement by relaxing a safety rule. The engine's
always-strip redirect credential behavior is conservative on purpose; if the
facade's behavior must differ, the difference belongs in
`docs/residual-differences.md` with an explicit safety argument, not in a
relaxed engine rule.

The native-surface `NO_PROXY` change is a user-visible behavior change for
native Python users. It is correct under ADR-0009, but it needs a release
note, not only a code change.
