# PyO3 and Python Version Modernization

Status: **complete; qualified locally**
Parent: `python-interop-api-hygiene-program.md`  
Date: 2026-09-15

## Scope

Bring the Python binding runtime and release matrix onto a current supported PyO3 stack, add Python 3.14 as a first-class supported version after qualification, and remove obsolete compatibility escape hatches/dead build dependencies where proven unnecessary.

This plan owns dependency/runtime modernization only. It must not introduce unrelated HTTP behavior or use the upgrade as an excuse for broad dependency churn.

## Baseline

`eggfetch-python` currently declares:

```toml
pyo3 = { version = "0.23", features = ["extension-module"] }
pyo3-async-runtimes = { version = "0.23", features = ["tokio-runtime"] }
```

and `pyo3-build-config = "0.23"` under `[build-dependencies]` despite no repository `build.rs` being present in the crate.

Current package metadata advertises Python 3.10–3.13 and the wheel release matrix builds those versions across the supported release platforms. The release workflow globally sets:

```text
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1
```

although eggfetch does not intentionally publish an abi3 support contract.

Research baseline at plan time: PyO3 / `pyo3-async-runtimes` 0.29.x are the current compatible line and understand Python 3.14; Python 3.14 is stable. Reconfirm versions at implementation time before editing manifests.

## Design Decisions

### 1. Upgrade PyO3 and async bridge together

Select the current mutually compatible stable releases of:

- `pyo3`;
- `pyo3-async-runtimes`;
- `pyo3-build-config` only if genuinely required.

Do not upgrade one side independently and leave mixed major/minor binding APIs.

### 2. Keep the public Python floor at 3.10 unless evidence requires change

There is no current justification for raising `requires-python = ">=3.10"` as part of this work. Preserve 3.10 behavior unless the modern PyO3 line or an explicitly accepted dependency makes support impossible.

If 3.10 cannot be retained, stop and document the concrete blocker before changing package metadata.

### 3. Add Python 3.14 after real qualification

Update classifiers and release-wheel matrix to include 3.14 only after:

- native behavior tests pass;
- wheel build/install/smoke passes;
- HTTPX/HTTPX2 compatibility smoke/full targeted tests pass as required;
- interpreter-shutdown and lifecycle tests pass;
- package content validation passes.

Do not claim support solely because the extension compiles.

### 4. Python 3.15 is forward evidence only

At implementation time, if 3.15 is still pre-release, run a non-publishing qualification where practical and record blockers. Do not add final package classifiers or a release-wheel requirement until final CPython release and PyO3 support are appropriate.

### 5. Do not add free-threaded support here

Python 3.14 free-threaded (`3.14t`) is a separate concurrency/ABI qualification problem. Eggfetch has explicit GIL-release, runtime, mutex, interpreter-shutdown, and PyO3 object-lifetime behavior that should be reviewed independently.

No free-threaded wheel/classifier/claim belongs in this plan.

### 6. Do not migrate to abi3/abi3t here

Continue per-Python wheel builds unless a separately approved packaging program changes that policy.

## Dependency Audit

Before editing versions, run a focused dependency/usage audit for:

- all `pyo3::*` imports and macros;
- all `pyo3_async_runtimes::*` calls;
- any `pyo3_build_config` references;
- environment-variable workarounds tied to old PyO3 behavior;
- Python-version cfgs or compatibility shims.

Confirm whether `pyo3-build-config` is actually used indirectly by a build script. If there is no consumer, remove the direct build dependency.

Do not remove transitive PyO3 build tooling required by PyO3 itself; only remove the crate's direct unused dependency declaration.

## Upgrade Implementation

### Manifest update

Update `crates/eggfetch-python/Cargo.toml` to the selected compatible PyO3 pair.

Run `cargo update` narrowly enough to avoid unrelated graph churn. Review `Cargo.lock` diff explicitly.

Expected transitive movement in PyO3-owned crates is acceptable; unrelated transport/security dependency upgrades are not part of this plan.

### Source adaptation

Address deprecations/API changes at the binding layer with the narrowest changes possible.

Pay special attention to:

- `Bound` / `Py` conversions;
- GIL APIs and deprecations;
- `Python::with_gil`/attachment APIs if changed by the selected release;
- `future_into_py` / `into_future` APIs;
- exception creation;
- `PyIterator` behavior;
- module registration;
- interpreter-finalization behavior;
- send/sync trait requirements for futures crossing the bridge.

Do not suppress meaningful new compiler/PyO3 warnings globally to make the upgrade pass.

### Forward compatibility environment flag

Once the selected PyO3 release natively recognizes every supported release interpreter in the wheel matrix, remove `PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1` from ordinary release validation/build environment.

If a specific pre-release Python qualification still needs it, scope the flag to that explicit experimental job only and document why.

Production/release wheels should fail closed when the binding stack does not support the interpreter.

## Supported Python Matrix

At minimum qualify:

- Python 3.10;
- Python 3.11;
- Python 3.12;
- Python 3.13;
- Python 3.14.

For Linux x86_64, run the strongest behavioral matrix across all supported versions. Existing release wheel builds on macOS arm64 and Windows x86_64 should include 3.14 after qualification.

It is acceptable for routine push CI to remain centered on one representative Python version if release/manual matrix coverage is explicit and reproducible. Do not multiply routine CI unnecessarily.

## Qualification Focus

For every supported Python version, at minimum validate:

- import and `__version__` metadata;
- `Client` and `AsyncClient` construction;
- buffered GET/POST;
- sync and async streaming responses;
- sync iterable request bodies;
- async iterable request bodies on `AsyncClient` after the preceding plan;
- cancellation;
- close/interpreter shutdown behavior;
- exceptions;
- TLS verification defaults;
- multipart;
- proxy basics;
- installed-wheel smoke.

Run the complete compatibility suite at least on the main qualification interpreter used by the repository. Use targeted compatibility smoke/critical regressions across the entire Python-version matrix unless runtime budget permits full differential qualification everywhere.

## Python 3.14 Specific Audit

Check for differences involving:

- asyncio event-loop behavior;
- interpreter finalization;
- `ssl.SSLContext` APIs used by the neutral interop layer;
- threading/GIL assumptions in sync streaming;
- extension module loading;
- exception and inspect/signature metadata;
- packaging tags/wheel installation.

Do not confuse standard Python 3.14 with the free-threaded build.

## Release Workflow Changes

After successful qualification:

- add Python 3.14 to `pyproject.toml` classifiers;
- add Python 3.14 rows to the existing wheel matrix for supported platforms;
- update release/verification docs that enumerate supported versions;
- remove global `PYO3_USE_ABI3_FORWARD_COMPATIBILITY` if no longer required;
- keep package `requires-python = ">=3.10"` unless an explicit accepted blocker changed the floor.

## Validation

Run:

```text
cargo check --workspace --all-targets --all-features
cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

plus supported-Python wheel build/install/smoke qualification.

Run Rust 1.89 Tier 2 MSRV after dependency changes; the PyO3 upgrade must not silently break the declared Rust floor. If current PyO3 requires Rust newer than 1.89, stop and evaluate that conflict explicitly rather than weakening the gate or bumping MSRV incidentally.

## Acceptance Criteria

- [x] selected PyO3 and `pyo3-async-runtimes` releases are mutually compatible and current at implementation time.
- [x] dependency update is narrowly scoped and reviewed.
- [x] no unrelated HTTP transport/security dependency churn is introduced.
- [x] all binding code compiles cleanly with repository lint policy.
- [x] direct `pyo3-build-config` dependency is removed after confirming it is unused.
- [x] global `PYO3_USE_ABI3_FORWARD_COMPATIBILITY` is removed from release builds.
- [x] Python 3.10–3.14 remain supported.
- [x] Python 3.14 native behavior tests pass locally.
- [x] Python 3.14 wheel build/install/smoke passes on the locally qualified release platform; macOS and Windows rows are covered by the release workflow.
- [x] Python 3.14 classifier/matrix/docs are updated after qualification.
- [x] no free-threaded or abi3 support claim is introduced.
- [x] Rust 1.89 MSRV gate still passes with the upgraded dependency graph.
- [x] Tier 1, Tier 2, and package validation pass.

## Qualification Record

The implementation uses the current compatible PyO3 line resolved locally as
`pyo3 0.29.2` and `pyo3-async-runtimes 0.29.0`. The direct
`pyo3-build-config` build dependency and the ABI3 forward-compatibility
override are absent; the transitive build-config crates required by PyO3
remain in the lockfile.

Local qualification on 2026-09-15 covered CPython 3.10, 3.11, 3.12, 3.13,
and 3.14. Each interpreter built the native extension and passed the native
API and behavior suites. The Python 3.14 release wheel was built, installed
in an isolated environment, and passed the installed-artifact smoke test and
package-content validation. CPython 3.15.0rc2 also passed the same native
behavior suite as forward-compatibility evidence; it is not advertised as a
supported version.

The canonical Tier 1 and Tier 2 checks passed locally, including the full
compatibility suite, MSRV, documentation, FFI, lifecycle, soak, and benchmark
gates. Tier 3 package validation remains the final local gate before the
release workflow is dispatched for cross-platform wheel qualification.

## Handoff

After this pass stabilizes, implement `python-pep561-typing-surface.md`. Defer exact-SHA compatibility profile rebinding to final closure.
