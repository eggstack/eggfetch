# HTTPX 0.28.1 Parity Completion — Phase 2: Python Object and Configuration Contracts

Status: ready for implementation handoff

Date: 2026-08-07

Roadmap: `plans/httpx-parity-completion-roadmap.md`

Prerequisite: Phase 1 contract rebaseline complete

Audited pre-roadmap baseline: `c66360827489c988f37b4aa9bd615b612258825d`

Pinned reference: `httpx==0.28.1`

Compatibility designation: `Stage C candidate`

## Objective

Close the remaining inexpensive, source-visible Python object and configuration mismatches in `eggfetch.compat.httpx` without changing the native networking architecture.

This phase targets public contracts that downstream code can observe through normal calls, `isinstance`/ABC checks, enum construction, constructor keywords, object properties, URL representation, and exception handling. These differences should not remain permanently allowlisted when matching HTTPX 0.28.1 is straightforward and low risk.

The Phase 1 implementation inventory is authoritative for exact difference IDs. If the IDs below changed during rebaseline, use the Phase 1 mapping rather than resurrecting stale IDs.

## Primary target areas

Expected target groups include:

- `Headers` collection contract (`MUTABLE-MAPPING-003*` family or rebaselined equivalent);
- `QueryParams` mapping contract (`MAPPING-001` family or equivalent);
- stream exception hierarchy/signatures (`STREAM-ERROR-BASE-*`, `STREAM-ERROR-SIG-*` families or equivalent);
- `NetRCAuth(file=...)` (`NETRC-PARAM-NAME-*` or equivalent);
- `URL.raw` (`URL-RAW-*` family or equivalent);
- `codes` enum/type behavior (`CODES-KIND-*` family or equivalent);
- `Timeout`, `Limits`, `Proxy`, and related configuration constructor/default semantics assigned by Phase 1;
- `Client`/`AsyncClient.default_encoding` semantics if Phase 1 confirms a real observable mismatch.

## Likely implementation files

Primary Python facade files:

- `crates/eggfetch-python/python/eggfetch/compat/httpx/_headers.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_urls.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_exceptions.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_auth.py`
- status-code module used by the facade
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_timeout.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_limits.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_proxy.py`
- `crates/eggfetch-python/python/eggfetch/compat/httpx/_client.py` only for default/value-object integration
- `crates/eggfetch-python/python/eggfetch/compat/httpx/__init__.py` only for public export parity

Primary tests:

- existing compatibility tests for headers, URLs/query params, exceptions, auth, status codes, config objects, response/client defaults;
- narrowly added direct differential tests where current tests are candidate-only.

## Scope firewall

### In scope

- public Python ABC inheritance where it is observable and part of HTTPX's public object surface;
- public methods inherited from those ABCs;
- exception inheritance and exact normal constructor behavior;
- public constructor keyword names/defaults for targeted value/configuration objects;
- public URL/status-code representation semantics;
- response/client default-encoding behavior;
- API-oracle allowlist removal for differences actually resolved by this phase;
- moving resolved records to `resolved-differences.toml` using the existing ledger conventions.

### Out of scope

Do not implement:

- exact top-level/client method signatures assigned to Phase 3;
- stream base-class/type hierarchy assigned to Phase 3, except exception classes;
- UDS/local-address/socket-option behavior assigned to Phase 4;
- SOCKS assigned to Phase 5;
- Trio/AnyIO;
- Python 3.8/3.9 support;
- private HTTPX modules;
- a native transport redesign;
- a new URL parser or general collections framework unless existing facade code cannot express the reference behavior safely.

## Track 0 — Reference-first failing tests

Before changing each target group, add or identify a direct HTTPX 0.28.1 comparison that demonstrates the mismatch.

For every test:

1. run the case against imported `httpx` pinned to 0.28.1;
2. run the equivalent case against `eggfetch.compat.httpx`;
3. assert the public result/type/error, not private implementation details;
4. prove the current candidate fails before the fix when practical;
5. keep the test after implementation as regression coverage.

Do not use the API manifest alone as proof for behavioral semantics.

### Track 0 acceptance criteria

- Every targeted group has reference-derived coverage.
- Candidate-only assertions are not the sole evidence for a disputed contract.
- Tests avoid private HTTPX imports unless the compatibility contract explicitly includes that symbol.

## Track 1 — Align `Headers` collection behavior

### 1.1 Match the public ABC relationship

If Phase 1 confirms HTTPX 0.28.1 publicly exposes `Headers` as a `MutableMapping` implementation, make the facade satisfy that relationship through normal inheritance/protocol implementation.

Use `collections.abc.MutableMapping` or the exact appropriate public ABC. Do not fake the MRO through metadata.

### 1.2 Implement inherited mapping methods correctly

Close the current missing-method differences for at least:

- `setdefault()`;
- `popitem()`.

The implementation must preserve HTTP header semantics rather than blindly inheriting dict behavior where HTTPX behaves differently.

Differential cases must cover:

- case-insensitive key matching;
- normalized return values;
- duplicate-header storage;
- setting a missing key;
- preserving an existing key through `setdefault`;
- `popitem` on non-empty and empty headers;
- deletion after duplicate values;
- ordering behavior if reference-observable.

### 1.3 Preserve EggFetch duplicate-header behavior

Do not regress:

- `multi_items()`;
- comma-joining behavior where HTTPX joins values;
- raw/multi-value access;
- case-insensitive update semantics;
- header validation already proven by existing tests.

### 1.4 Review additive methods separately

If EggFetch exposes additive methods such as `append` that HTTPX does not expose, do not remove them automatically. Phase 1 should have classified them as intentional/additive or must-close.

An additive method may remain if it does not alter valid HTTPX calls or the intended substitution behavior.

### Track 1 acceptance criteria

- `isinstance(headers, MutableMapping)` matches the reference expectation.
- Targeted inherited methods exist and match reference results/errors.
- Duplicate/header-normalization behavior does not regress.
- All resolved Headers difference records leave the active allowlist.

## Track 2 — Align `QueryParams` mapping behavior

### 2.1 Match Mapping ABC behavior

If confirmed by Phase 1, make `QueryParams` satisfy `collections.abc.Mapping` in the same observable way as HTTPX 0.28.1.

Do not make it mutable merely because a mapping ABC is added.

### 2.2 Preserve multi-value and immutability semantics

Differential tests must cover:

- `isinstance(params, Mapping)`;
- key iteration;
- length;
- `__getitem__` behavior with repeated keys;
- `get`, `items`, and inherited helpers that become visible through the ABC;
- `multi_items()` ordering;
- immutable mutation-helper behavior already exposed by the facade;
- equality against supported mapping/query representations if public and currently tested.

### Track 2 acceptance criteria

- Mapping ABC checks match HTTPX.
- No accidental mutable-dict semantics are introduced.
- Repeated parameter ordering and value selection remain reference-compatible.
- Targeted QueryParams allowlist entries are resolved.

## Track 3 — Correct exception hierarchy and constructor contracts

### 3.1 Make `StreamError` derive from `RuntimeError`

HTTPX's stream-state errors are observable through the `RuntimeError` hierarchy. Align the facade hierarchy while preserving the existing HTTPX-compatible exception subclasses.

Required checks include:

```python
issubclass(StreamError, RuntimeError)
issubclass(StreamConsumed, StreamError)
issubclass(StreamClosed, StreamError)
issubclass(ResponseNotRead, StreamError)
```

### 3.2 Match no-argument stream exception constructors

Where HTTPX's stream-state exception constructors take no explicit arguments, make the normal call shape match.

Differential tests should cover:

- `inspect.signature` only to the extent this phase owns constructor semantics;
- `ResponseNotRead()`;
- `StreamClosed()`;
- `StreamConsumed()`;
- passing unsupported positional/keyword arguments and the resulting `TypeError` behavior where stable and public.

Phase 3 owns broad public method/helper signature work. Do not duplicate that effort here.

### 3.3 Revisit other cheap exception constructor differences

Phase 1 may classify optional-message mismatches such as `InvalidURL` or `CookieConflict` as `must-close`.

If so, align their required/default message parameter behavior now. Do not retain a looser constructor merely because it is convenient.

### 3.4 Preserve native exception mapping

Changing facade inheritance/signatures must not alter which compatibility exception is produced from native failures.

Run existing timeout, proxy, redirect, stream-state, and request-error mapping tests after the hierarchy change.

### Track 3 acceptance criteria

- Stream exception hierarchy matches the reference.
- Targeted exception constructor behavior matches the reference.
- Native exception mapping remains unchanged for supported failures.
- Resolved exception differences are removed from the active ledger.

## Track 4 — Align `NetRCAuth` constructor keyword

### 4.1 Support exact HTTPX `file=` syntax

HTTPX-valid source such as:

```python
httpx.NetRCAuth(file=path)
```

must be valid with `eggfetch.compat.httpx.NetRCAuth`.

Use the exact public keyword name even though it shadows a common built-in name. Built-in-shadow avoidance is not a compatibility justification.

### 4.2 Decide `auth_file=` only from Phase 1 classification

Preferred compatibility behavior is an exact HTTPX public signature.

If the existing `auth_file=` keyword is retained as an EggFetch-only alias for backwards compatibility, it must:

- not change the reference-compatible normal path;
- be explicitly classified as additive/intentional;
- not prevent Phase 3 from exposing the exact HTTPX signature if signature parity is required.

Do not keep two ambiguous sources simultaneously.

### 4.3 Differential coverage

Cover:

- default netrc lookup;
- explicit path with `file=`;
- nonexistent/unreadable file behavior;
- host credential selection;
- behavior when credentials are absent;
- type validation where the reference has stable behavior.

### Track 4 acceptance criteria

- `NetRCAuth(file=...)` works exactly for normal supported use.
- Existing auth-flow behavior is unchanged.
- The parameter-name difference is removed from the active allowlist.

## Track 5 — Implement `URL.raw`

### 5.1 Match the reference public property

Add the public `URL.raw` representation with the same externally observable structure/types as HTTPX 0.28.1.

Do not approximate the raw representation by decoding to Unicode and re-encoding if that loses canonical percent-encoded information.

### 5.2 Pin byte-preservation semantics with differential tests

Cover at minimum:

- `http://example.com/path?x=1`;
- explicit and default ports;
- IPv4;
- bracketed IPv6;
- percent-encoded path bytes;
- non-ASCII input and resulting encoded raw form;
- empty path/query;
- fragments if the raw property exposes/excludes them in the reference;
- user-info only if it is part of the public URL raw structure and safe to expose exactly as HTTPX does.

### 5.3 Avoid URL parser duplication

Use the facade's existing parsed/canonical URL state where possible. Add the minimum stored byte representation necessary to reproduce the reference.

Do not introduce a second full URL parser.

### Track 5 acceptance criteria

- `URL.raw` type and value match HTTPX for the differential corpus.
- `str(url)`, equality, copying, joining, parameter mutation, and existing URL tests remain correct.
- URL raw differences are removed from the active allowlist.

## Track 6 — Align `codes` with HTTPX enum behavior

### 6.1 Use the correct enum form

If Phase 1 confirms HTTPX 0.28.1 exposes `codes` as an `IntEnum`-compatible type, implement the facade accordingly rather than as a plain constants namespace.

### 6.2 Preserve member/alias behavior

Generate or declare members from the existing canonical status-code table rather than maintaining two manually divergent status lists.

Differential tests should cover:

- `codes.OK == 200`;
- `int(codes.OK) == 200`;
- `codes(200)`;
- enum member name/value;
- alias members where HTTPX exposes multiple names for one status;
- iteration/member lookup behavior if public;
- repr/str only where stable and used by the API oracle;
- existing helper predicates on `codes` if HTTPX exposes them.

### 6.3 Do not alter response status-code storage

Responses may continue to expose integer `status_code` if that matches HTTPX. This track concerns the `codes` public object, not a broad status representation rewrite.

### Track 6 acceptance criteria

- Enum construction/type checks match HTTPX.
- Canonical and alias status members match the pinned reference.
- No duplicate hand-maintained status table is introduced.
- Targeted `codes` differences are removed from the active ledger.

## Track 7 — Configuration objects and defaults

Phase 1 must provide the exact IDs/symbols assigned here. Do not broaden this track beyond current public mismatches.

### 7.1 `Timeout`

Differentially specify and then align:

- `Timeout(5.0)`;
- `Timeout(None)`;
- explicit `connect`, `read`, `write`, `pool` fields;
- partial-component constructor validity/invalidity;
- default/sentinel semantics;
- copy-construction if public;
- repr/equality only if reference-visible and already in the manifest/tests.

The facade may convert the resulting object to EggFetch's native timeout model afterward. The public constructor must not leak native convenience semantics into HTTPX-compatible source.

### 7.2 `Limits`

Align targeted keyword-only/default constructor behavior.

Preserve the actual native limit mapping already proven by pooling tests.

Do not add transport behavior in this phase.

### 7.3 `Proxy`

If Phase 1 assigns `Proxy(..., ssl_context=...)` constructor parity here, accept and store the public parameter exactly as HTTPX does.

Do not claim functional TLS proxy transport behavior unless the native path already supports the required semantic. If using the value requires Phase 4/5 native work, the constructor may be aligned here while functional transport use remains an explicitly linked later acceptance criterion.

Do not silently discard a provided `ssl_context` value.

### 7.4 `default_encoding`

Confirm the actual HTTPX 0.28.1 behavior rather than relying on manifest default text alone.

If `AsyncClient.default_encoding` or the corresponding sync client currently exposes `None` where HTTPX exposes `'utf-8'`, align the stored/public default while preserving the existing response wrapper's UTF-8 fallback.

Test both string and callable default encodings if the reference supports callable detection.

### Track 7 acceptance criteria

- Targeted value/config objects accept/reject the same normal constructor forms as HTTPX.
- No provided public value is silently ignored.
- Native timeout/limit behavior remains correct.
- Response decoding behavior remains reference-compatible.
- Resolved config/default difference records are removed from the active ledger.

## Track 8 — Ledger and evidence update

### 8.1 Rerun focused tests

Run the relevant object/configuration compatibility modules plus every new direct differential test.

### 8.2 Regenerate candidate API manifest

Use the existing manifest generator and comparator.

Expected result:

- targeted differences disappear from oracle output;
- their active allowlist entries become stale if not removed;
- after ledger cleanup, zero stale and zero unexplained entries remain.

### 8.3 Move resolved entries to historical ledger

For each resolved record:

- remove it from `allowed-differences.toml`;
- add/update the corresponding record in `resolved-differences.toml` according to existing conventions;
- bind resolution to tests and implementation SHA where the ledger format supports it.

Do not delete history.

## Required validation

At minimum:

```sh
python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

may be run as the full pinned qualification if environment/time permits during the phase; focused tests are mandatory before that.

Also run:

```sh
./scripts/check.sh
```

before Phase 2 is declared complete because this phase modifies executable Python code.

Run the existing API-oracle commands after implementation.

Do not add CI jobs.

## Phase acceptance criteria

Phase 2 is complete only when:

- every Phase 1 difference assigned to Phase 2 has either been resolved or stopped under an explicit blocker;
- Headers and QueryParams collection contracts match the pinned reference for targeted behavior;
- stream exception hierarchy/constructors match;
- `NetRCAuth(file=...)` works;
- `URL.raw` matches reference differential cases;
- `codes` has the correct public enum/type semantics;
- targeted Timeout/Limits/Proxy/default-encoding semantics match;
- focused differential tests pass;
- `./scripts/check.sh` passes;
- API oracle reports zero unexplained/stale differences after ledger cleanup;
- no transport, SOCKS, Trio/AnyIO, CI, or release scope is added.

## Rejection criteria

Reject the implementation if:

- an allowlist entry is removed without reference-derived tests;
- `Headers` gains generic dict behavior that breaks duplicate-header semantics;
- `QueryParams` becomes mutable unintentionally;
- exception hierarchy changes break native error mapping;
- `URL.raw` is implemented through a lossy decode/re-encode shortcut;
- a second status-code table can drift independently;
- `Proxy.ssl_context` is accepted and silently discarded;
- runtime call signatures are cosmetically altered with metadata while actual constructor behavior remains incompatible;
- transport features assigned to later phases are partially implemented here without their end-to-end acceptance tests.

## Stop conditions

Stop the affected sub-track and record a bounded blocker if:

- reproducing `URL.raw` requires replacing the existing URL parsing model rather than adding a narrow preserved representation;
- matching `Proxy.ssl_context` requires immediate native transport changes that cannot be separated from Phase 4/5;
- an apparent API-oracle difference is proven to be private/non-public and Phase 1 classification is wrong;
- exact constructor behavior conflicts with an established non-compat EggFetch API outside the compatibility facade.

A blocker affects only its sub-track unless it invalidates the Phase 1 inventory. Continue independent Phase 2 work.

## Suggested commit decomposition

1. `test: pin remaining HTTPX object contract differences`
2. `fix: align header query and exception contracts`
3. `fix: align HTTPX auth URL and status value objects`
4. `fix: align HTTPX configuration object semantics`
5. `test: resolve HTTPX object contract allowlist entries`

## Handoff checklist

Report:

- starting SHA;
- final Phase 2 executable SHA;
- exact difference IDs resolved;
- exact difference IDs blocked/retained and why;
- focused differential commands/results;
- `./scripts/check.sh` result;
- full pinned suite result if run;
- API oracle before/after allowed counts;
- active/resolved ledger changes;
- confirmation that no Phase 4/5 transport implementation, Trio/AnyIO, CI, or release scope was introduced.
