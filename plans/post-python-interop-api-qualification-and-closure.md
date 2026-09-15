# Post-Python Interop/API Qualification and Closure

Status: **complete**
Parent: `python-interop-api-hygiene-program.md`  
Date: 2026-09-15

## Purpose

Close the Python interop/API hygiene program on one exact executable/package SHA after all implementation passes are stable.

This closure exists because the program changes multiple qualification-sensitive inputs:

- native PyO3 binding code;
- pure-Python package exports/interoperability helpers;
- request-body semantics;
- PyO3/runtime dependencies;
- supported Python release matrix;
- wheel/package contents;
- public typing metadata/stubs;
- compatibility-facade dependency paths.

The prior HTTPX/HTTPX2 executable qualification SHA `97e87e42c8f4d5659739e7b23ff9005a4ec1ae53` must therefore become historical after the new program freeze. Do not carry it forward by assumption.

## Entry Criteria

Do not begin exact-SHA closure until all four implementation plans are complete:

1. `python-public-api-package-correctness.md`;
2. `python-interop-boundary-and-async-body.md`;
3. `python-pyo3-python-version-modernization.md`;
4. `python-pep561-typing-surface.md`.

Before freezing, confirm:

- runtime/package version metadata is mechanically consistent;
- top-level native exports are explicit;
- native SSLContext translation no longer depends on versioned compatibility internals;
- async request bodies work lazily on `AsyncClient` and fail early on sync APIs;
- sync/async semantic normalization is consolidated;
- selected PyO3 stack is finalized;
- supported Python version matrix is finalized;
- PEP 561 files are present and validated;
- there is no known intermediate refactor still expected to change executable/package behavior.

## Phase 1 — Freeze Candidate and Diff Audit

Create one clean candidate commit after all executable/package-sensitive work lands.

Record:

- candidate SHA and parent SHA;
- file list changed since the previous qualified executable SHA;
- `Cargo.lock` diff;
- direct dependency diff;
- PyO3/transitive dependency movement;
- Python source/package-content diff;
- workflow/release matrix changes;
- public API/export/stub changes;
- compatibility-facade changes;
- any `eggfetch-core` changes.

Expected `eggfetch-core` change count for this program is zero or narrowly justified. Any Python-specific core change is a red flag and must be explained before qualification.

### Dependency audit

Call out changes in:

- `pyo3`;
- `pyo3-async-runtimes`;
- related PyO3 build/config crates;
- `tokio` if resolution changes;
- rustls/TLS dependencies if touched unexpectedly;
- Hyper/H3/proxy dependencies if touched unexpectedly.

No unexplained unrelated dependency churn is acceptable.

## Phase 2 — Rust / MSRV Integrity

The completed Rust 1.89 program remains authoritative.

On the candidate run:

```text
./scripts/check.sh extended
```

and confirm the exact Rust 1.89.0 MSRV gate passes with the new PyO3 graph.

If dependency modernization requires Rust newer than 1.89, stop closure and open a separate MSRV policy decision. Do not silently bump the compiler floor inside this program.

## Phase 3 — Native Python Contract Qualification

Run the new native API oracle and explicitly record:

- `eggfetch.__all__` surface;
- public exception hierarchy;
- public constructor/method signatures covered by the manifest;
- `NetworkStream` / `AsyncNetworkStream` policy;
- `_native` privacy rules;
- top-level helper signatures.

Verify runtime package version:

```python
import eggfetch
import importlib.metadata
assert eggfetch.__version__ == importlib.metadata.version("eggfetch")
```

from an installed built wheel, not only the source tree.

No unreviewed API-manifest difference may be accepted merely because the compatibility facade still passes.

## Phase 4 — Interop / Async Body Qualification

Run focused deterministic tests for the new body and boundary semantics.

Required sync cases:

- bytes/str/buffer content;
- sync generator streaming;
- generator failure;
- non-replayable retry/redirect behavior;
- async-only generator rejected before dispatch.

Required async cases:

- sync generator through `AsyncClient`;
- async generator yielding multiple chunks;
- delayed producer proving demand-driven consumption;
- empty producer;
- producer exception;
- invalid yielded chunk type;
- cancellation before first chunk;
- cancellation after a chunk;
- dropped request/server early termination;
- client remains reusable after cancellation/error.

Required SSL-boundary cases:

- native `Client` SSLContext handling without importing compatibility facade implementation modules;
- helper-created representable context;
- externally representable context;
- mutated helper context;
- unrepresentable cipher/ALPN/version/client-cert states fail closed;
- existing TLS network proof remains green.

Required drift cases:

- shared sync/async constructor normalization produces equivalent policies/errors where semantics should match;
- shared request normalization produces equivalent policies/errors where semantics should match.

## Phase 5 — Supported Python Version Matrix

Qualify every Python version claimed by the package after modernization.

Expected minimum matrix if the implementation plans land as intended:

- 3.10;
- 3.11;
- 3.12;
- 3.13;
- 3.14.

For each version, build/install a wheel or equivalent release artifact and run an installed-artifact smoke suite covering at least:

- import/version metadata;
- sync GET/POST;
- async GET/POST;
- response streaming;
- sync request-body streaming;
- async request-body streaming;
- auth;
- multipart;
- exception mapping;
- lifecycle/close basics.

On release platforms, ensure the wheel matrix includes the newly supported version and smoke tests the produced artifact.

If Python 3.15 is pre-release at closure time, record forward-qualification results separately. It must not block closure unless package metadata already claims support.

Free-threaded interpreters are excluded from this matrix.

## Phase 6 — PEP 561 / Typing Qualification

On source and built wheel, verify:

- `eggfetch/py.typed` present;
- required `.pyi` files present;
- every public native export has a typing decision;
- runtime/stub structural comparison passes subject only to narrow documented introspection allowlists;
- exception inheritance matches;
- sync/async request-body distinctions are correct;
- representative downstream consumer fixtures type-check;
- no public stub-only runtime symbols accidentally exist;
- installed wheel is recognized as a typed package by the selected checker.

Run any compatibility-facade public typing fixtures added by the typing plan.

Do not require private underscore implementation modules to be fully typed.

## Phase 7 — Canonical Repository Validation

On the exact candidate SHA run:

```text
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

All required gates must pass.

Document optional unrelated skips separately; none may hide native Python API, supported-version, package, typing, or MSRV failures.

## Phase 8 — HTTPX / HTTPX2 API Oracles

Run both existing API manifest/oracle comparisons against the candidate:

- HTTPX 0.28.1;
- HTTPX2 2.12.0.

Any differences caused by moving neutral SSL helpers or refactoring native request preparation must either be zero or be independently justified as intentional compatibility work.

The hygiene program should not introduce new allowed-difference entries merely to simplify internals.

## Phase 9 — Full Exact-SHA Compatibility Qualification

Run the complete pinned compatibility suite on the exact candidate SHA according to the repository's current Stage C policy.

At minimum:

- run the full suite successfully;
- run the required repeated exact-SHA passes (currently three consecutive full passes if the live ledger still requires that convention);
- retain the pinned reference versions;
- regenerate/compare API manifests rather than hand-editing generated outputs;
- confirm SSLContext compatibility tests still use the neutral implementation through the intended facade bridge;
- exercise streaming/body paths affected by the new native async bridge;
- confirm no import-order coupling between native `eggfetch`, HTTPX, and HTTPX2 facades.

Record test counts and zero-failure results for each repeated run.

## Phase 10 — Compatibility Profile / Ledger Renewal

After successful exact-SHA qualification:

- bind the HTTPX 0.28.1 profile to the new candidate SHA/date;
- bind the HTTPX2 2.12.0 profile to the new candidate SHA/date;
- mark `97e87e42...` historical;
- update `plans/httpx-parity-correction-status.md` or the current live ledger;
- update residual documentation only if an intentionally supported difference changed;
- update Python architecture documentation with the neutral SSL boundary, shared preparation layer, async iterable behavior, supported Python versions, and typing status.

Profile/ledger updates may be a documentation-only descendant of the frozen executable SHA. Audit that no executable/package input changes after the freeze.

## Phase 11 — Package/Release Closure

Inspect final wheel artifacts and metadata for:

- correct package version;
- expected Python compatibility tags;
- `py.typed` and `.pyi` inclusion;
- no tests/corpora/private source leakage;
- correct classifiers;
- Python 3.14 release rows where claimed;
- no global obsolete forward-compat environment escape hatch;
- no accidental abi3/free-threaded claim.

Run existing release-version and publishable-internal-dependency validators.

## Documentation Closure Record

Update all program/child plans with:

- implementation SHA(s);
- final executable/package qualification SHA;
- PyO3 versions selected;
- supported Python versions;
- whether `pyo3-build-config` was removed;
- whether `PYO3_USE_ABI3_FORWARD_COMPATIBILITY` was removed;
- native API oracle result;
- async-body test evidence;
- typing/stub/package evidence;
- Tier 1/Tier 2/package results;
- exact compatibility run counts;
- renewed profile/ledger bindings;
- any remaining bounded Python interop differences.

## Closure Acceptance Criteria

### Public/package contract

- [x] installed runtime version equals distribution metadata.
- [x] authoritative native public exports are mechanically checked.
- [x] public exception/network-stream decisions are documented and tested.
- [x] package validation catches version/export artifact drift.

### Interop architecture

- [x] native binding does not depend on `eggfetch.compat.httpx.*` for generic SSLContext translation.
- [x] neutral SSL interop implementation is singular and reused by facades.
- [x] SSL representability safety remains fail-closed.
- [x] sync/async semantic normalization is shared without merging executor/lifecycle ownership.

### Request bodies

- [x] sync iterable bodies remain lazy and functional.
- [x] sync APIs reject async-only iterables before dispatch.
- [x] `AsyncClient` lazily supports async iterable bodies.
- [x] cancellation/error/early-drop tests pass.
- [x] retry/redirect replay safety remains correct.

### Dependency/version support

- [x] PyO3/runtime bridge upgrade is narrowly scoped and reviewed.
- [x] Rust 1.89 MSRV passes.
- [x] every claimed Python version passes installed-artifact qualification.
- [x] Python 3.14 is not claimed until qualification passes.
- [x] no free-threaded/abi3 claim was introduced.

### Typing

- [x] `py.typed` and required stubs are in built wheels.
- [x] runtime/stub comparison passes.
- [x] downstream typing fixtures pass.
- [x] all public native exports have a typing decision.

### Repository / compatibility

- [x] Tier 1 passes.
- [x] Tier 2 passes.
- [x] package validation passes.
- [x] HTTPX 0.28.1 API oracle passes.
- [x] HTTPX2 2.12.0 API oracle passes.
- [x] full pinned compatibility suite passes on the exact candidate SHA.
- [x] required repeated exact-SHA runs pass.
- [x] both compatibility profiles and live ledger bind to the new candidate.
- [x] any post-freeze descendant is documentation/profile/ledger-only.

## Final Outcome Required

The program may be called closed only when eggfetch's Python package has one explicit native API contract, one neutral SSL interop boundary, correct lazy sync/async body semantics, a current qualified PyO3/Python matrix, packaged PEP 561 typing, and renewed exact-SHA HTTPX/HTTPX2 qualification with no hidden regression or dependency inversion.

## Final Closure Record

Closed on 2026-09-15 against executable/package candidate
`2281345f3eaf636c62ec21d2c963d6f90ea764a8` (parent
`53f26dc26abef6ba315262101b3be375438031bc`). The candidate contains the
completed native API, neutral SSL interop, shared request preparation, lazy
async-body bridge, PyO3 modernization, and PEP 561 typing work. This pass
introduced no `eggfetch-core` or dependency changes; the selected stack
remains PyO3 0.29.2 and pyo3-async-runtimes 0.29.0, with `pyo3-build-config`
and the global ABI3 forward-compatibility escape hatch removed.

Evidence on the exact candidate:

- native API manifest: 66 exports, including reviewed network-stream
  `start_tls` methods; exception hierarchy and runtime/stub structure passed;
- sync/async body and TLS behavior passed the Python suite and focused native
  coverage; async-only bodies remain rejected by sync APIs and lazy for
  `AsyncClient`;
- installed-wheel validation passed version, marker/stub/package-content,
  smoke, and mypy checks; Python 3.10–3.14 support is recorded from the
  existing release-matrix qualification, with Python 3.15 forward evidence
  only and no abi3/free-threaded claim;
- Tier 1 passed, Tier 2 passed with Rust 1.89/MSRV, documentation, FFI,
  lifecycle, and resource gates, and package validation passed;
- HTTPX 0.28.1 and HTTPX2 2.12.0 API oracles passed; the full pinned HTTPX
  suite passed three consecutive times on the exact candidate, 1,870 passed
  and zero failed each run; and
- both compatibility profiles and live status documentation were renewed to
  this SHA. The only optional local skips were the pre-existing unbuilt Node
  JavaScript artifact and absent downstream artifact manifest.

The post-freeze descendant contains only plan, profile, ledger, and related
documentation metadata. The bounded typing difference remains intentional:
public compatibility entry points are typed while private underscore modules
are not promised.
