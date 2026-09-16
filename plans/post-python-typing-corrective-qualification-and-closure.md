# Post-Python Typing Corrective Qualification and Closure

Status: **complete; qualified and closed**
Date: 2026-09-15
Parent corrective plan: `python-pep561-method-contract-corrective-pass.md`
Prior qualified executable/package SHA: `2281345f3eaf636c62ec21d2c963d6f90ea764a8`

## Purpose

Close the narrow PEP 561 method-contract correction on one exact candidate SHA, renew qualification evidence invalidated by the typing/package/test changes, and prove that the stronger runtime↔stub contract is actually enforced from both source and installed wheels.

This closure is intentionally smaller than the preceding Python interop/API-hygiene program. It must not reopen already-qualified runtime architecture unless the corrective implementation discovers a real runtime defect.

## Final closure record — 2026-09-16

The corrective implementation froze at
`c28bbcad6bf9c420721731e8b7a18c2ec1707dd1` (parent `1bc90362`). The final
qualification tree is that same executable commit; profile, ledger, plan, and
documentation updates are a documentation-only descendant. The change was
limited to native PEP 561 stubs, the reviewed member-contract manifest,
typing/runtime-oracle scripts, no-network consumer fixtures, and
documentation. No Rust executable source, `pyproject.toml`, maturin
configuration, package-content logic, Cargo.lock, dependency, or
compatibility-facade behavior changed.

The source and installed-wheel checks passed with 66 runtime exports, 32
exception bases, and 24 reviewed member contracts. Synthetic regression tests
covered missing methods/properties, sync/async kind drift, signature drift,
semantic return drift, and unreviewed public stub members. Positive and
expected-negative typing fixtures passed; the release-style wheel contained
`py.typed` and both public stubs, and package-content/version validation
passed. Runtime spot checks confirmed existing stream, lifecycle,
verify-boundary, and context-manager behavior; this was a stub/checker
correction only.

Tier 1, extended, and package gates passed. Extended/package retained only
the existing local skips for the unbuilt Node JS artifact and absent
downstream artifact manifest. The Rust 1.89.0 MSRV gate passed. Because no
PyO3 Rust source, packaging configuration, or platform-sensitive behavior
changed, prior Python 3.10–3.14 cross-platform wheel evidence remains
applicable; no Python 3.15, abi3, or free-threaded claim is made.

The HTTPX 0.28.1 oracle passed with 71 allowed matches, zero stale allowed,
zero unexplained, and zero resolved-in-active entries. The HTTPX2 2.12.0
oracle passed with 79 allowed matches and the same zero counts. Three
consecutive exact-SHA full pinned compatibility runs each passed 1,870 tests
with 26 existing non-failing warnings, in 246.45s, 243.57s, and 244.52s.
No files or dependencies changed between runs, and no new allowed difference
or compatibility-facade behavior change was introduced. Both profiles and the
live ledger now bind to the final SHA; the previous `2281345f...` binding is
historical.

## Entry Criteria

Do not freeze a candidate until `python-pep561-method-contract-corrective-pass.md` is implementation-complete and focused validation is green.

Before freezing, confirm:

- every confirmed stub/runtime mismatch has been resolved;
- method/property drift checking is wired into canonical validation;
- consumer typing fixtures cover the corrected contracts;
- installed-wheel typing validation exercises the strengthened checks;
- no known follow-up stub correction remains pending;
- no unrelated dependency/protocol/runtime work is mixed into the candidate.

## Phase 1 — Freeze and Diff Audit

Freeze one exact candidate commit after the corrective implementation stabilizes.

Record:

- candidate SHA and parent SHA;
- files changed from `2281345f3eaf636c62ec21d2c963d6f90ea764a8`;
- whether any Rust executable source changed;
- whether any package metadata/build configuration changed;
- whether `Cargo.lock` changed;
- typing/stub/manifest/checker/test changes;
- compatibility-facade changes, if any;
- `eggfetch-core` changes, if any.

Expected scope is primarily:

- `crates/eggfetch-python/python/eggfetch/*.pyi`;
- `crates/eggfetch-python/tests/native_api_manifest.json` or a narrowly related reviewed contract file;
- typing validation scripts/fixtures;
- focused Python tests proving return/property semantics;
- plan/documentation closure records.

Any unrelated transport, TLS, H3, proxy, retry, pooling, or dependency movement is a stop-and-review condition.

## Phase 2 — Corrected Typing Contract Proof

Run the source-tree typing gates on the frozen candidate.

Required checks include the repository's canonical commands plus the strengthened typing checker(s), for example:

```text
python scripts/check_python_typing_surface.py
python scripts/check_python_typing.py
```

If the corrective pass introduces a distinct method-contract checker, invoke it explicitly here and ensure `./scripts/check.sh` also runs it.

Record proof that the gate catches at least these classes of regression:

- missing public method;
- missing public property;
- sync/async method-kind mismatch;
- manifest parameter/signature mismatch where introspection is reliable;
- incorrect `start_tls` semantic return type;
- missing client lifecycle/cookie members;
- incorrect `Verify` DER-list typing;
- unreviewed public stub-only runtime member.

A useful implementation technique is to include unit tests for the checker against synthetic bad stub fragments or temporary fixtures. Do not rely on manually editing the real stub to demonstrate failure.

## Phase 3 — Runtime Semantics Spot Check

Run the focused native tests added or used by the corrective pass and record results for:

- `NetworkStream.start_tls` returns `NetworkStream`;
- awaiting `AsyncNetworkStream.start_tls` returns `AsyncNetworkStream`;
- both stream classes expose boolean `is_upgraded`;
- `Client.is_closed` and `Client.cookies`;
- `AsyncClient.close`, `AsyncClient.is_closed`, and `AsyncClient.cookies`;
- accepted DER `list[bytes]` verify input and rejection of unsupported sequence shapes;
- context-manager exit return values represented by the stubs.

If runtime behavior changed during the correction, expand this phase to cover that exact behavior and document why the change was necessary. Otherwise explicitly record that this was a stub/checker correction only.

## Phase 4 — Consumer Type-Checking Proof

Run representative positive and negative consumer fixtures from an environment that sees the source package typing surface.

Required positive cases:

- sync client construction and lifecycle/property access;
- async client construction and lifecycle/property access;
- sync `NetworkStream.start_tls` result assignment;
- async `AsyncNetworkStream.start_tls` result assignment;
- `is_upgraded: bool` access;
- DER-list `verify=` usage;
- normal sync and async request bodies.

Required negative cases:

- async-only body passed to sync `Client`;
- unsupported `verify=` input form that runtime rejects;
- assignment of `start_tls` result to `None` or another incompatible type where the checker can prove the error.

The fixtures must not depend on external network access.

## Phase 5 — Built Wheel Typing and Metadata

Build the release-style Python wheel on the candidate and validate the installed artifact in an isolated environment.

At minimum prove:

- `eggfetch/py.typed` is installed;
- `eggfetch/__init__.pyi` and `eggfetch/_native.pyi` are installed;
- installed distribution version matches `eggfetch.__version__`;
- consumer typing fixtures resolve the installed wheel rather than the source tree;
- corrected `NetworkStream`/`AsyncNetworkStream`, client lifecycle, `Verify`, and constructor contracts are visible to the type checker;
- package-content validation passes;
- no tests/private qualification artifacts leak into the wheel.

Run the repository package gate:

```text
./scripts/check.sh package
```

## Phase 6 — Canonical Repository Gates

On the same exact candidate SHA run:

```text
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

The exact Rust 1.89.0 MSRV gate must remain green. The PyO3 stack must remain the already-qualified line unless the corrective implementation explicitly required otherwise; dependency movement is not expected.

Document optional unrelated skips separately. No typing, package, MSRV, native Python, or compatibility failure may be converted into a skip.

## Phase 7 — Python Version / Platform Scope

Because this correction is expected to affect `.pyi`, validation scripts, and tests rather than ABI/runtime implementation, a full 15-wheel rebuild is not automatically required solely to prove method annotations.

However:

- if any PyO3 Rust source, `pyproject.toml`, maturin configuration, wheel-content logic, or platform-sensitive package behavior changes, rerun the build-only 3.10–3.14 Linux/macOS/Windows release matrix;
- if only stubs/checkers/tests change and one representative release-style wheel proves packaged typing, retain the prior cross-platform wheel evidence and state why it remains applicable.

Do not claim Python 3.15, abi3, or free-threaded support in this closure.

## Phase 8 — HTTPX / HTTPX2 Compatibility Requalification

Repository policy currently treats changes to tests, validation scripts, packaging inputs, and packaged Python surfaces as qualification-sensitive. Therefore do not carry forward `2281345f...` as the active binding by assumption.

On the exact candidate:

1. run the HTTPX 0.28.1 API oracle;
2. run the HTTPX2 2.12.0 API oracle;
3. run the full pinned compatibility suite according to the current live Stage C procedure;
4. run the required repeated exact-SHA passes if the live ledger still requires three consecutive full passes;
5. record exact pass counts and zero failures;
6. confirm the typing correction introduced no new allowed-difference entries and no compatibility-facade behavior change.

If repository policy has been explicitly revised before implementation so typing-only/test-only changes no longer invalidate behavioral compatibility evidence, cite that new policy instead of mechanically repeating unnecessary work. Absent such a policy change, follow the current exact-SHA rule.

## Phase 9 — Profile / Ledger Renewal

After exact-SHA qualification succeeds:

- bind `compat/httpx/0.28.1/profile.toml` to the new candidate SHA/date;
- bind `compat/httpx2/2.12.0/profile.toml` to the same candidate SHA/date;
- mark `2281345f...` historical;
- update `plans/httpx-parity-correction-status.md` and current compatibility documentation;
- state that the triggering delta was a narrow PEP 561 method-contract/checker correction;
- do not alter generated API manifests by hand.

Profile/ledger/documentation updates may be a descendant of the frozen candidate only if they do not change executable/package inputs.

## Phase 10 — Plan and Documentation Closure

Update the corrective plan with:

- implementation SHA(s);
- final frozen qualification SHA;
- exact corrected stub mismatches;
- checker design and any narrow allowlists;
- typing fixture results;
- installed-wheel evidence;
- Tier 1/Tier 2/package results;
- whether a full cross-platform wheel rerun was required;
- HTTPX/HTTPX2 oracle and compatibility results;
- renewed profile/ledger bindings;
- any remaining intentional typing imprecision.

Update the original `python-pep561-typing-surface.md` only with a short historical note pointing to this corrective closure if useful. Do not rewrite its historical implementation record as though the original checker had provided method-level coverage that it did not.

## Closure Acceptance Criteria

### Stub/runtime contract

- [x] every confirmed method/property/type mismatch is corrected;
- [x] all supported native class members have an explicit typing decision;
- [x] method/property drift is mechanically checked;
- [x] sync/async member classification is checked where reliable;
- [x] reviewed semantic return types are checked;
- [x] unsupported input forms are not widened in stubs;
- [x] `AsyncClient` constructor is explicitly typed.

### Typing behavior

- [x] corrected positive consumer fixtures pass;
- [x] expected-negative consumer fixtures fail for the intended reason;
- [x] source-tree typing gate passes;
- [x] installed-wheel typing gate passes;
- [x] `py.typed` and stubs are present in the wheel.

### Runtime/package safety

- [x] focused runtime semantics tests pass;
- [x] installed runtime version still equals distribution metadata;
- [x] no unintended native Python API behavior change is introduced;
- [x] no unintended dependency/MSRV/platform change is introduced;
- [x] Rust 1.89.0 MSRV remains green.

### Repository/compatibility

- [x] Tier 1 passes;
- [x] Tier 2 passes;
- [x] package validation passes;
- [x] HTTPX 0.28.1 API oracle passes;
- [x] HTTPX2 2.12.0 API oracle passes;
- [x] current Stage C exact-SHA compatibility procedure passes;
- [x] both compatibility profiles and live ledger bind to the new candidate when required by policy;
- [x] any post-freeze descendant is documentation/profile/ledger-only.

## Final Outcome Required

The corrective line is closed only when the shipped PEP 561 surface describes the actual native member contract—not merely the export set—and the repository has a durable gate capable of detecting the same class of drift that allowed the first closure to miss these discrepancies.
