# Free-Threaded Python Support Investigation

Planning baseline: `272c82215fbf2f0193748e9d862931f8c2b4169a` (main, post-0.4.0 publication)

Subsystem: Python bindings and HTTPX compatibility
(`plans/subsystems/python-httpx-compat-roadmap.md`)

Normative verification policy: `docs/verification-policy.md`

## Objective

Determine how much work would be required to run `eggfetch`'s Python bindings on
a free-threaded (PEP 703, `Py_GIL_DISABLED`) CPython build, and whether the
resulting capability justifies registering a milestone.

This document started as a research record and now also tracks the work that
followed the maintainer decisions in §9: free-threaded wheels for CPython
3.14t and 3.15t (Option A), the in-tree contract tests, and the qualification
evidence for both. It publishes no wheel and cuts no release; publication
remains a manual, tagged step.

## Scope

In scope:

- A source-level audit of every place the binding depends on the GIL.
- The upstream support matrix (PyO3, `pyo3-async-runtimes`, maturin,
  `setup-python`, CPython).
- The packaging and CI surface that a free-threaded wheel would change.
- A ranked risk inventory with the evidence each risk rests on.

Out of scope: publication, release cutting, and any statement that the binding
is safe under `Py_GIL_DISABLED` beyond what the recorded qualification
establishes. Sub-interpreter support remains out of scope (§10).

## 1. Upstream support matrix

| Component | Version in this repository | Free-threading status |
|---|---|---|
| PyO3 | `0.29` (`crates/eggfetch-python/Cargo.toml:27`) | Supports free-threaded builds since 0.23. Since 0.28 support is **opt-out**, not opt-in (PyO3 PR #5564). 0.29 **dropped Python 3.13t**; 3.14t and later are supported (PR #5865). 0.29 added the PEP 803 `abi3t` / `abi3t-py315` features (PR #5807). |
| `pyo3-async-runtimes` | `0.29` (`crates/eggfetch-python/Cargo.toml:28`) | Uses the PyO3 0.26+ GIL-optional API (`Python::attach`) throughout `src/generic.rs` and `src/tokio.rs`. The crate contains no free-threading-specific code and documents no free-threading limitation. |
| maturin | `1.14.1` (pinned in `scripts/release-requirements.txt`, used by `.github/workflows/pypi.yml`) | Detects free-threaded interpreters via the `Py_GIL_DISABLED` build flag and accepts `python3.13t` when cross-compiling. |
| `actions/setup-python` | `v5.6.0` (`.github/workflows/pypi.yml`) | Provides free-threaded builds through `t`-suffixed version specifiers (for example `3.14t`, `3.14t-dev`). |
| CPython | 3.13 / 3.14 / 3.15 | 3.13t is the first free-threaded build. CPython **declared free-threading supported starting with 3.14**. 3.15 adds the `abi3t` Stable ABI (PEP 803). |

## 2. Current repository state

### 2.1 The module already declares itself GIL-free

`crates/eggfetch-python/src/lib.rs:614` declares a bare `#[pymodule]`.

In PyO3 0.29 the macro resolves the `gil_used` option as
`options.gil_used.is_some_and(|op| op.value.value)`
(`pyo3-macros-backend-0.29.2/src/module.rs:394`). With no `gil_used = true`
option present this evaluates to `false`, and the generated module slots call
`.with_gil_used(false)` (`pyo3-macros-backend-0.29.2/src/module.rs:551`).
PyO3 maps that to the `Py_mod_gil` slot value `Py_MOD_GIL_NOT_USED`
(`pyo3-ffi-0.29.2/src/moduleobject.rs:121`).

Under CPython's rules, `Py_MOD_GIL_NOT_USED` is how an extension states that it
supports running with the GIL disabled; without it, importing the module in a
free-threaded build emits a warning and re-enables the GIL at runtime.

**Consequence:** the binding does not need a module-level opt-in. If it is
imported on a free-threaded interpreter today, the GIL stays disabled rather
than being silently re-enabled. This is the single most important enabling
fact in this investigation.

### 2.2 The binding already uses the GIL-optional API shape

The crate uses PyO3 0.26+`s `py.detach(...)` and `Python::attach(...)`
rather than the older `allow_threads` / `with_gil` spelling. No
`allow_threads`, `with_gil`, `acquire_gil`, `try_attach`, or
`check_signals` call exists in `crates/eggfetch-python/src`. The
`detach`/`attach` pair is the API PyO3 introduced for GIL-optional builds.

### 2.3 Packaging and CI surface

- `crates/eggfetch-python/pyproject.toml` declares `requires-python = ">=3.10"`,
  `[tool.maturin] features = ["pyo3/extension-module"]`, `python-source = "python"`,
  and `module-name = "eggfetch._native"`. There is no `abi3`, no
  `abi3t`, and no free-threaded maturin option.
- `.github/workflows/pypi.yml` builds 24 wheels: 4 platforms (Linux x86_64,
  Linux aarch64, macOS arm64, Windows x86_64) x 6 GIL-enabled interpreters
  (3.10-3.15), via
  `maturin --release --out dist <target> -i python<ver> -m crates/eggfetch-python/Cargo.toml`.
  Every row selects a GIL-enabled interpreter; there is no `t` row.
- `scripts/validate_wheel_coverage.py` hard-codes
  `EXPECTED_PYTHON_VERSIONS = {cp310, cp311, cp312, cp313, cp314, cp315}`
  and `EXPECTED_PLATFORMS` (4 platforms). Although its comment says it rejects
  `abi3`, the validator currently never reads the ABI-tag field, so that
  rejection does not work for normal `cp310-abi3-...` wheel filenames.
  `.github/workflows/pypi.yml` additionally asserts exactly 24 wheels and 25
  distributions.
- `crates/eggfetch-python/tests/native_api_manifest.json` and the typing
  guards describe the public surface, not the interpreter ABI, so they are
  ABI-agnostic.

## 3. GIL-dependency inventory

### 3.1 GIL release points

| Location | API | Context |
|---|---|---|
| `client.rs:163`, `lib.rs:119` | `py.detach` | Tokio runtime creation in sync constructors and short-lived dispatch. |
| `client.rs:257`, `lib.rs:154` | `py.detach` | Sync request dispatch: the entire blocking engine call. |
| `client.rs:714` | `py.detach` | Sync streaming dispatch. |
| `streaming.rs:596,602` | `py.detach` | Sync body drain for `read()` / `text()`. |
| `streaming/sync_iterators.rs:148,268,391,532` | `py.detach` | Blocking receive in the four sync iterator `__next__` paths. |
| `network_stream.rs:268,316,345,469,491` | `py.detach` | Sync upgraded-stream read, write, close. |
| `conversion.rs:217,253,305,316,335` | `Python::attach` | Request-body iterator close/advance; async-body chunk build; `StopAsyncIteration` classification. |
| `trace_bridge.rs:151` | `Python::attach` | Trace callback invocation and event-dict construction. |
| `streaming.rs:591,601,608,634,657,703`, `async_client.rs:648`, `network_stream.rs:898` | `Python::attach` | Python-object construction at async boundaries. |

### 3.2 Sync bridge mechanics

`Client.request` (`client.rs:192`) converts every argument to owned Rust
state first through `request_preparation.rs`, clones the core client, takes a
runtime guard and handle, and only then releases the GIL:

```rust
let result = py.detach(|| {
    runtime_handle.block_on(async {
        let mut response = Box::pin(builder.send()).await.map_err(map_err)?;
        let content = response.bytes().await.map_err(map_err)?;
        Ok::<_, PyErr>((response, content))
    })
});
```
(`client.rs:257-267`)

The `Python<'py>` token and all borrowed `Bound<'py, PyAny>` arguments are
**not** captured by the detached closure. No `Python<'py>`-typed field is
stored in any struct in `crates/eggfetch-python/src`; the token appears only as
a function parameter. Python response objects are constructed after the detach
returns (`client.rs:278-288`).

This is the correct shape for free-threaded execution: the blocking wait holds
no interpreter state.

### 3.3 Shared mutable state is already GIL-independent

Every piece of shared mutable state in the binding is guarded by Rust
synchronization rather than by the GIL:

- `client.rs:16-24,86-87` and `async_client.rs:20`: client/runtime state in
  `std::sync::Mutex`, plus an `AtomicBool` shutdown flag.
- `streaming.rs:86-104`: body state in `AtomicU8`, response and cached
  content/text in `std::sync::Mutex`.
- `streaming/sync_bridge.rs:20-24`: `Mutex` + `Condvar` queue.
- `streaming/sync_iterators.rs:24,410` and
  `streaming/async_iterators.rs:12-13,22`: pending buffers and receivers.
- `network_stream.rs:89,125,623`: shared upgraded-stream IO behind
  `std::sync::Mutex` / `tokio::sync::Mutex`.
- `trace_bridge.rs:61,67`: `Arc<Mutex<Option<TraceBridgeError>>>`.
- `response.rs:94`: lazily decoded text in a `OnceLock<Arc<str>>`.

No `RefCell`, `Cell`, `UnsafeCell`, `static mut`, or `thread_local!`
appears in `crates/eggfetch-python/src`. The sibling `eggfetch-ffi` and
`eggfetch-node` crates contain no PyO3 or GIL dependency at all; their
`unsafe impl Send`/`Sync` surfaces are justified by their own mutexes.

### 3.4 The one process-global Python object

`crates/eggfetch-python/src/response.rs:20`:

```rust
static NOOP_ACLOSE: OnceLock<Py<PyAny>> = OnceLock::new();
```

It caches a compiled `async def _noop_aclose()` coroutine used by
`Response.aclose` (`response.rs:22-34`). The `OnceLock` makes
initialization thread-safe, but the cached object belongs to whichever
interpreter created it and the cache is **not keyed by interpreter**.

### 3.5 Callbacks that lose implicit serialization

`PyTraceObserver::on_event` (`trace_bridge.rs:138-176`) invokes a user
Python callable from inside the transport stack, and
`python_iterable_to_request_body` (`conversion.rs:246-273`) advances a user
Python iterator from a Tokio blocking worker. Under a GIL these calls were
implicitly serialized against every other Python execution. Under
`Py_GIL_DISABLED` two in-flight requests on the same `Client` can enter the
same Python callable concurrently.

## 4. Findings

Ranked by consequence. "Fact" marks something established by reading the
source or a pinned upstream artifact; "inference" marks a runtime consequence
that still requires qualification.

### F1 - The module already declares GIL-not-used (enabling fact)

Established in 2.1 from the pinned PyO3 0.29 macro backend. Consequence: no
`gil_used` opt-in is needed, and a free-threaded interpreter will not
silently re-enable the GIL on import.

### F2 - The global `NOOP_ACLOSE` cache is not interpreter-keyed (fact)

Established in 3.4. `Py<PyAny>` objects are interpreter-affine. Inference: a
sub-interpreter that calls `Response.aclose()` could observe the
cross-interpreter object, which PyO3's own borrow checker rejects in debug
builds. The single-interpreter case is not affected. Sub-interpreter support is
not currently claimed by the binding, so this is a latent hazard rather than a
present defect.

### F3 - User callbacks and iterators lose the GIL's implicit serialization (inference)

Established in 3.5. The Rust-side state is already correctly locked, but a
Python callable or iterator shared across concurrent requests on one
`Client` would now be entered concurrently. This is a **behavior change**
relative to the GIL build, not a memory-safety defect, and it is the kind of
difference that user code may not have been written to tolerate.

### F4 - `Python::attach` is called from non-Python runtime threads (fact)

`trace_bridge.rs:151` and `conversion.rs:253` attach from Tokio worker and
blocking-pool threads that Python did not create. Inference: the attached
interpreter is the thread's interpreter if it has one, otherwise the main
interpreter. This is correct for the single-interpreter deployment the binding
targets and is a sub-interpreter concern only.

### F5 - The packaging and CI gates currently forbid free-threaded wheels (fact)

Established in 2.3. The workflow currently builds 24 GIL-enabled wheels
and asserts exactly 24 wheels (25 distributions including the sdist).
Version-specific free-threaded wheel filenames use tags such as
`cp314-cp314t` and `cp315-cp315t`: `cp314`/`cp315` is the Python tag; the
`cp314t`/`cp315t` part is the ABI tag. The validator reads `info["py"]`
but never inspects `info["abi"]`, so it collapses GIL and free-threaded
wheels for the same platform/Python version into one coverage key. Its
claimed `abi3` rejection is also ineffective: it looks for `abi3` in the
Python tag rather than the ABI tag, so a normal `cp310-abi3-...` filename
passes that check. A 32-wheel release set would fail the current count;
changing the count alone would still not validate ABI coverage. The validator
needs explicit Python-plus-ABI coverage and tests for allowed/rejected ABI
tags before adding free-threaded wheels.

### F6 - 3.13t is not a viable target for this PyO3 version (fact)

PyO3 0.29 dropped 3.13t support (PR #5865) because CPython declared
free-threading supported starting with 3.14. The realistic free-threaded
matrix is 3.14t and 3.15t.

### F7 - `abi3t` is available but requires a separate policy decision (fact)

PyO3 0.29 exposes the PEP 803 `abi3t` / `abi3t-py315` feature. PEP 803
distinguishes the free-threaded-only `abi3t` tag from the combined
`abi3.abi3t` wheel tag for extensions built to support both GIL-enabled and
free-threaded CPython 3.15+. The `abi3t` Stable ABI does not cover 3.14t,
which still needs a version-specific `cp314-cp314t` wheel. The current
validator's intended no-`abi3` policy is not actually enforced because it
never checks the ABI field. Reconsidering a Stable-ABI strategy is therefore
a separate policy and packaging decision, and the pinned PyO3/maturin toolchain
must be shown to emit and install the intended tags.

## 5. Work classification

**Invariant** (must remain true)

- All blocking waits release the GIL; no `Python<'py>` token is stored beyond
  a call; shared mutable state stays behind Rust synchronization.
- The private `_native` module boundary and the public surface in
  `__init__.py` + `__all__` are unchanged by any ABI work.

**Capability** (user-visible)

- A free-threaded interpreter can `import eggfetch` with the GIL disabled and
  complete sync and async requests.

**Infrastructure**

- A free-threaded wheel build path (interpreter selection in `pypi.yml`).
- A `t`-aware wheel-coverage rule and updated wheel/distribution counts.
- A free-threaded test lane.

**Polish**

- Documentation of the supported free-threaded interpreter range and of the
  concurrency semantics change (F3).

## 6. Feasibility verdict

**Feasible, and cheaper than expected.** The binding is already written in the
GIL-optional API shape, already keeps its shared state behind Rust
synchronization, and already declares `Py_MOD_GIL_NOT_USED` through PyO3
0.29's default. No core-engine change and no architectural change is implied.

The remaining work is concentrated in packaging and CI (F5) plus two
correctness questions that only a runtime qualification can settle (F2, F3).

What the investigation does **not** establish: that the binding is free-thread
safe today. Source inspection shows no GIL-dependent memory-safety assumption
in the binding itself, but that is an argument, not evidence.

## 7. Options

**Decision (2026-11-23): Option A adopted.** Free-threaded wheels ship
alongside the GIL-enabled ones for CPython 3.14t and 3.15t on the four
release platforms. Options B and C are kept below for the record; C remains
an open question in its own right (§9.4).

### Option A - Free-threaded wheels alongside the GIL wheels (adopted)

Keep the 24 GIL-enabled wheels unchanged and add a parallel free-threaded set
for 3.14t and 3.15t on the same 4 platforms.

- Preserves the existing wheel contract; PyPI resolves a `t` wheel only for a
  free-threaded interpreter.
- Requires: `t` rows in `pypi.yml`, a `t`-aware rule in
  `validate_wheel_coverage.py`, updated wheel/distribution counts, and a
  free-threaded test lane.
- Costs 8 additional wheels per release (2 interpreters x 4 platforms) and
  therefore 8 more CI jobs, which the current one-job CI policy and the
  "no new matrix combinations without maintainer approval" rule make an
  explicit decision rather than a default.
- **Adopted as that explicit decision** (2026-11-23). The extra rows live in
  the release workflow; the one-job CI is unchanged.

### Option B - Defer

Keep the binding on GIL-enabled interpreters only: no `t` wheels are
published and the wheel matrix is unchanged.

- Cost: zero. Nothing else in the repository moves.
- Consequence: `pip install eggfetch` on a free-threaded interpreter finds no
  matching tag, so it falls back to a source build. Such a build would still
  work and — because of F1 — would keep the GIL disabled, but that is a side
  effect of the declaration rather than a supported configuration.
- The binding therefore cannot *claim* free-threaded support, and there is no
  free-threaded test lane that would keep such a claim honest if it were made.

### Option C - PEP 803 Stable ABI (`abi3t`)

Evaluate PyO3's `abi3t` feature and the combined `abi3.abi3t` wheel tag for
CPython 3.15+; `abi3t` alone is free-threaded-only, while `abi3.abi3t` is
intended to cover both GIL-enabled and free-threaded interpreters. This does
not cover 3.14t, which still needs a `cp314-cp314t` wheel, nor does it
replace the existing GIL-enabled 3.10-3.14 coverage by itself.

- Potentially reduces per-version builds for the 3.15+ portion of the matrix.
- Requires reversing or refining the existing no-`abi3` policy, validating
  the exact `abi3.abi3t` support of the pinned PyO3/maturin toolchain,
  confirming the binding is compatible with the Stable ABI, and redesigning
  the matrix/validator to cover the distinct ABI ranges.
- Treat as a separate packaging strategy, not a shortcut to proving
  free-threaded runtime correctness.

## 8. Evidence required before any support claim

The repository's verification policy requires evidence, not reasoning. A
free-threaded claim would need, at minimum:

1. A wheel built against each supported free-threaded interpreter/target,
   with the expected Python, ABI, and platform tags; install the wheel in a
   clean environment and confirm the installer selected that artifact.
2. Confirm `sysconfig.get_config_var("Py_GIL_DISABLED") == 1` and that
   `sys._is_gil_enabled()` is false both before and after importing
   `eggfetch._native` / `eggfetch`. The latter check proves the import did not
   silently re-enable the GIL; the module slot declaration alone is not
   runtime evidence.
3. Run the existing Python suite on that interpreter, including sync and
   async requests, streaming, cookies, and network-stream paths.
4. Deterministic concurrency tests that exercise F3: multiple Python threads
   issuing requests through one `Client`, with shared trace-callback state and
   (where the API permits sharing) a request-body iterator. Define and assert
   the intended contract: callbacks/iterators may be entered concurrently
   unless the binding explicitly serializes them, so shared user state must
   provide its own synchronization. Test for lost/duplicated body items,
   exceptions, deadlocks, and unintended Rust-side races.
5. A sub-interpreter test covering F2, or explicit documentation that
   sub-interpreters are unsupported; this is a separate compatibility axis
   from free-threaded execution.
6. Apply `.skills/verification-qualification.md`'s exact-SHA rule: record the
   source/build inputs, interpreter, wheel tags, commands, and results against
   the exact candidate SHA; rerun qualification after executable-input changes.

Status against the evidence in §10 (macOS arm64, CPython 3.14.10t and
3.15.2t):

| # | Requirement | Status |
|---|---|---|
| 1 | Wheel per free-threaded interpreter/target, tags checked, clean install | Met on all four release platforms for 3.14t and 3.15t; Linux free-threaded wheels are manylinux_2_34 (§10) |
| 2 | `Py_GIL_DISABLED == 1`, GIL stays off across import | Met (in-tree test, clean subprocess) |
| 3 | Existing Python suite on that interpreter | Met inside Tier 1 locally (native 545 passed / 40 skipped; compat 134 passed) and on Linux CI (free-threaded suite jobs) |
| 4 | Deterministic concurrency tests for F3 | Met (`crates/eggfetch-python/tests/test_free_threading.py`) |
| 5 | Sub-interpreter test, or explicit non-support documentation | Met by documentation (see §10) |
| 6 | Exact-SHA record against the candidate SHA | Met for the recorded tree; re-run required after any further executable-input change |

Performance is not a precondition for a support claim, so it is not a row
above. It is measured separately, and §10 records the GIL-versus-free-
threaded concurrency comparison.

## 9. Maintainer decisions

1. Adopt, defer, or scope free-threaded wheel support (F5, Options A/B/C).
   **Resolved 2026-11-23: Option A adopted.** Free-threaded wheels for 3.14t
   and 3.15t ship on the four release platforms; the release set grows from
   24 to 32 wheels.
2. Whether the 8 extra wheel builds and CI jobs are acceptable under the
   one-job CI policy and the no-new-matrix-combinations rule.
   **Resolved 2026-11-23: accepted** as part of Option A. The extra rows are
   release-workflow rows; the one-job CI stays as it is, and the
   free-threaded test lane is the scratch-branch qualification recorded in
   §10.
3. Whether F3's concurrency semantics change is acceptable to document, or
   whether the binding should add explicit serialization around user
   callbacks. The current behaviour is now pinned by an in-tree contract test
   (§10); explicit serialization would change that test.
   **Resolved 2026-11-23: document the current semantics.** The binding does
   not serialize user callbacks; the contract is written up in
   `docs/architecture/python-bindings.md` and `docs/python/guide.md`. The
   measured consequence is in §10: that missing serialization is also what
   limits GIL-enabled scaling once the application does Python work per
   response.
4. Whether `abi3t` / PEP 803 should be revisited as a separate question from
   free-threading (F7). **Still open.** Option A does not depend on it.

## 10. Runtime validation follow-up (evidence only)

This follow-up records local runtime evidence without changing the document's
no-support-claim status. No runtime code or CI configuration was changed. The
host was macOS 26.6.2 arm64. The qualified interpreters are CPython 3.14.10t
and CPython 3.15.2t — *release* free-threaded builds from
python-build-standalone `20261123` — with PyO3 0.29.2 and maturin 1.14.1.
The same interpreter versions were qualified on Linux and Windows runners
through `setup-python` (§10).

Executable source tree: HEAD `edf098ccb466491a041ee3b7607e5b3fd4fdfb91`
plus the changes recorded below. No Rust source changed, so the built
artifact is identical to the HEAD build; the exact-SHA rule still covers the
added test module and the packaging changes, and the gates below were re-run
against that tree. The changed inputs are:

- `crates/eggfetch-python/tests/test_free_threading.py` (added): the in-tree
  free-threading contract tests.
- `scripts/validate_wheel_coverage.py`: ABI-aware coverage plus a
  `--self-test`, wired into Tier 1 through `scripts/check.sh`.
- `scripts/wheel_smoke.py`: free-threaded-aware wheel resolution.
- `.github/workflows/pypi.yml`: the eight free-threaded rows, the
  `container: "off"` setting for the free-threaded Linux rows, and the
  updated wheel/distribution counts.
- `README.md`, `docs/architecture/python-bindings.md`,
  `docs/getting-started/installation.md`, `docs/python/guide.md`: the
  supported free-threaded range and the F3 concurrency contract.

### Correction: the earlier CPython 3.15.0b1t typing-gate failure

An earlier round of this investigation recorded a CPython **3.15.0b1** *beta*
build whose full Tier 1 attempt stopped at the Python typing-fixture step,
because the pinned `mypy==2.3.1` process died with SIGSEGV even on
`mypy --version`. That failure is **not** an eggfetch defect and it is not
evidence about the binding on 3.15:

- `mypy` ships a mypyc-compiled extension for free-threaded builds
  (`__mypyc.cpython-315t-darwin.so`). A `faulthandler` run places the crash
  inside that extension — `CPyExec_mypy___util`, during the import of
  `mypy.util` — before any eggfetch source is read. The same crash reproduces
  on a one-line file, so it is independent of the binding's stubs.
- The wheel is tagged `cp315-cp315t-macosx_11_0_arm64`: it is built against
  the 3.15 *release* ABI, which the `3.15.0b1+freethreaded` beta interpreter
  predates. The two disagree, and the disagreement is fatal at import.
- On CPython 3.15.2t — the 3.15 *release* free-threaded build — the same
  pinned `mypy==2.3.1` runs normally (`mypy 2.3.1 (compiled: yes)`) and the
  strict typing fixtures pass.

Consequence: the typing-gate blocker was an artifact of the beta interpreter.
It is recorded here so the beta result is not later read as a binding defect,
and so the qualification is not re-run against a 3.15 beta.

### CPython 3.14.10t and CPython 3.15.2t

- `./scripts/check.sh` exited **0** on both interpreters: all routine checks
  passed. The Node JS surface was explicitly skipped on both because
  `crates/eggfetch-node/eggfetch.node` is absent on this host.
- The Python suites run inside that gate: native tests `545 passed, 40
  skipped`, compat tests `134 passed`.
- GIL state: `sysconfig.get_config_var("Py_GIL_DISABLED") == 1` and
  `sys._is_gil_enabled() == False` before and after importing `eggfetch` and
  `eggfetch._native`.

### New in-tree free-threading contract tests

`crates/eggfetch-python/tests/test_free_threading.py` turns the ad-hoc
concurrency probes into a repeatable in-tree contract (§8.4):

| Test | Asserts | GIL build | Free-threaded build |
|---|---|---|---|
| `test_import_does_not_reenable_the_gil` | In a clean subprocess, importing the binding leaves the interpreter's GIL state unchanged, and a free-threaded interpreter stays free-threaded across the import. | pass | pass |
| `test_concurrent_requests_through_one_client` | Eight threads issuing requests through one `Client` each receive their own response body: no cross-talk, no hung threads. | pass | pass |
| `test_trace_callback_is_entered_concurrently_on_free_threaded_builds` | A trace callback that blocks until a second invocation is in flight completes, proving user callbacks are **not** serialized (F3). | skipped — a GIL build cannot express this without deadlocking | pass |
| `test_shared_request_body_iterator_neither_loses_nor_duplicates_chunks` | Two concurrent requests sharing one body iterator deliver every chunk exactly once across both bodies: no loss, no duplication. | pass | pass |

On this host the module reports 4 passed on each free-threaded interpreter and
3 passed / 1 skipped on the GIL-enabled CPython 3.14.2 build. The
trace-callback test pins the F3 contract; if the binding later adds explicit
serialization around user callbacks, that test is the place to update.

### Cross-platform qualification (scratch branch)

The one-job CI runs GIL-enabled interpreters on one platform, so the
cross-platform free-threaded evidence was collected on a scratch branch
(scratch/free-threaded-crossplatform) carrying a temporary workflow. The
branch and its workflow were deleted after the runs; no permanent CI lane
was added. Run 45276582176 passed every job:

| Job | Combinations | Result |
|---|---|---|
| contract | Linux x86_64, Linux aarch64, macOS arm64, Windows x86_64, each on 3.14t and 3.15t | 8/8 passed |
| suite | ubuntu-latest and ubuntu-24.04-arm, each on 3.14t and 3.15t | 4/4 passed |
| release path | Linux x86_64 on 3.14t, Linux aarch64 on 3.15t | 2/2 passed |

Every contract job asserted Py_GIL_DISABLED == 1 on the interpreter
setup-python installed for the t-suffixed version spec, that the versioned
name python3.14t / python3.15t resolves (the release workflow selects the
interpreter by that name), that the GIL stayed disabled across the imports
of eggfetch and eggfetch._native, that the four contract tests passed, and
that a release wheel carried the free-threaded tag. The suite jobs ran the
native Python suite and the focused compat suite. The free-threaded wheel
names produced were:

- eggfetch-0.4.0-cp314-cp314t-manylinux_2_34_x86_64.whl
- eggfetch-0.4.0-cp314-cp314t-manylinux_2_34_aarch64.whl
- eggfetch-0.4.0-cp314-cp314t-macosx_11_0_arm64.whl
- eggfetch-0.4.0-cp314-cp314t-win_amd64.whl
- eggfetch-0.4.0-cp315-cp315t-manylinux_2_34_x86_64.whl
- eggfetch-0.4.0-cp315-cp315t-manylinux_2_34_aarch64.whl
- eggfetch-0.4.0-cp315-cp315t-macosx_11_0_arm64.whl
- eggfetch-0.4.0-cp315-cp315t-win_amd64.whl

### The free-threaded Linux wheels need glibc 2.34

maturin-action builds Linux wheels inside a manylinux container, and the
container glibc - not the interpreter one - decides the platform tag. The
free-threaded CPython interpreters require glibc 2.34, and no available
manylinux container provides it: the 2014 image has glibc 2.17. The
free-threaded Linux rows therefore set container: off and build on the host
runner, which lets maturin derive the tag from the interpreter.

The consequence is that the free-threaded Linux wheels are manylinux_2_34,
not manylinux2014: they need a newer glibc than the GIL-enabled wheels and
so cover a narrower set of Linux distributions. The release path jobs
verified this through the real maturin-action invocation (manylinux: auto,
container: off).


### Superseded: CPython 3.15.0b1t

The beta-build results (extension built as `cp315-cp315t`; GIL disabled
before and after import; native tests 541 passed / 40 skipped; focused compat
tests 21 passed; concurrency probes passed) are superseded by the 3.15.2t
release-build results above. The beta's Tier 1 attempt did **not** pass — it
exited 245 at the typing-fixture step for the upstream reason recorded in the
correction above — and it is not counted as a Tier 1 pass.

### Local wheel build and clean-install evidence

Release wheels were built with `maturin build --release` and installed with
`uv pip install --no-deps` into fresh project-local venvs:

- `eggfetch-0.4.0-cp314-cp314t-macosx_11_0_arm64.whl` (CPython 3.14.10t)
- `eggfetch-0.4.0-cp315-cp315t-macosx_11_0_arm64.whl` (CPython 3.15.2t)

In each clean environment the installed distribution's tag matched the target
interpreter, the installed extension filename matched
`sysconfig.get_config_var("EXT_SUFFIX")`
(`_native.cpython-314t-darwin.so` / `_native.cpython-315t-darwin.so`), the GIL
stayed disabled across the import, and a loopback HTTP GET returned 200. The
wheels were not uploaded or published. Build and test logs are under
`target/free-threaded-*` in the local worktree.

### Sub-interpreters (F2)

Sub-interpreters are **not supported** by the binding and are not qualified
here, so the §8.5 qualifier is the documentation branch rather than a test:
F2's `NOOP_ACLOSE` cache is not interpreter-keyed, and the binding makes no
sub-interpreter claim. The qualified interpreters ship neither `interpreters`
(PEP 734) nor `_xxsubinterpreters`, so no sub-interpreter exercise was
possible on this host either. Sub-interpreter behaviour remains a separate
compatibility axis from free-threaded execution.

### Remote CI readback and remaining limits

The existing [CI run 45242054629](https://github.com/eggstack/eggfetch/actions/runs/45242054629)
passed at the same source SHA using the regular GIL-enabled CI configuration.
The existing [build-only PyPI workflow 45246499235](https://github.com/eggstack/eggfetch/actions/runs/45246499235)
also passed at that SHA; publication was skipped. Its Python matrix uses
GIL-enabled interpreters, so neither run is a remote free-threaded test. No
free-threaded CI lane or wheel-matrix row was added.

Evidence now covers all four release platforms. The permanent CI is still a
single GIL-enabled job on one platform, so the free-threaded lanes live only
in the release workflow and in the scratch-branch qualification above; no
permanent free-threaded test lane was added (§9.2). The release set grows
from 24 to 32 wheels, and no wheel has been published: publication remains a
manual, tagged step.

### Concurrency scaling: GIL versus free-threaded

The free-threaded qualification above establishes correctness. It does not
establish performance, and nothing in the repository measured it: the
Criterion suites in `crates/eggfetch-bench` measure `eggfetch-core` in Rust,
so the interpreter's GIL state cannot appear in them, and the API-safe
campaign harness was single-client and sequential.

The `--concurrency` group of `scripts/performance_benchmark.py` closes that
gap. Several threads share one `Client` against a minimal loopback server in
a separate process of the same interpreter. Two workloads are measured: `io`
(request plus body read) and `io_cpu` (the same, plus repeated hashing of the
body, modelling CPU-bound Python work around network calls). The method and
its reading rules are in `docs/architecture/benchmarks.md`.

Measured on this repository, macOS arm64 (Apple silicon), CPython 3.14.2
(GIL-enabled) against CPython 3.14.10t and 3.15.2t (free-threaded), all three
on the same commit, with `--repeats 3 --threads 1,2,4,8`:

| Workload | Threads | GIL 3.14.2 | FT 3.14.10 | FT 3.15.2 |
|---|---|---|---|---|
| `io` | 1 | 11,392 req/s (1.00x) | 11,143 req/s (1.00x) | 10,491 req/s (1.00x) |
| `io` | 8 | 38,002 req/s (3.34x) | 47,861 req/s (4.30x) | 56,420 req/s (5.38x) |
| `io_cpu` | 1 | 9,802 req/s (1.00x) | 9,149 req/s (1.00x) | 10,713 req/s (1.00x) |
| `io_cpu` | 4 | 25,010 req/s (2.55x) | 25,364 req/s (2.77x) | 32,092 req/s (3.00x) |
| `io_cpu` | 8 | 20,047 req/s (2.05x) | 36,861 req/s (4.03x) | 48,445 req/s (4.52x) |

The scaling factor is relative to the 1-thread result of the same run.

Three findings:

- **Single-client cost is the same on both builds.** The existing campaign
  cases move within run-to-run noise between a GIL-enabled and a
  free-threaded interpreter, so there is no per-request penalty for the
  free-threaded build.
- **The engine already parallelizes on a GIL-enabled build.** The `io`
  workload — where the Python side does almost nothing — reaches 3.34x at 8
  threads under the GIL, because the binding releases the GIL for the
  network wait. Free-threading does not change that; it is not a throughput
  fix for I/O-bound use.
- **Free-threading pays off in the Python work around the network call.**
  Under the GIL, adding per-response Python work makes throughput *fall*
  from 4 to 8 threads (25,010 to 20,047 req/s): the Python side serializes,
  so the extra threads add contention without adding throughput. The
  free-threaded builds keep scaling to 4.03x and 4.52x, which is 1.84x and
  2.42x the GIL-enabled throughput at 8 threads.

A server ceiling would show up as the same plateau in every run, so a
free-threaded run that out-scales the GIL-enabled run is evidence that the
ceiling was the client rather than the server.

These are host-specific local measurements, not performance budgets: the
machine was under load from an unrelated project, the GIL interpreter is
3.14.2 against free-threaded 3.14.10, and the server is loopback with no
network latency. The repository rule stands — `scripts/performance_benchmark.py`
stays non-CI timing evidence, and only a fresh measurement on the same
commit, toolchains, target, feature set, server fixture and repeat settings
is the basis for a comparison.

### Where the work landed

The implementation landed on `main` in `06d21f93` (`feat(python): add
free-threaded CPython wheel support`), with the documentation and packaging
metadata in the follow-up commit. That work changed executable inputs — the
release workflow, the wheel-coverage validator, the wheel-smoke script, the
Tier 1 entry point and a new Python test target — so it invalidated the
exact-SHA Stage C binding. `plans/httpx-parity-correction-status.md` records
the resulting state. The evidence collected here is free-threading and
packaging evidence; it is not an exact-SHA renewal of the HTTPX Stage C
binding.

## 11. References

- [PyO3 0.29 free-threading guide](https://pyo3.rs/v0.29.0/free-threading).
- [PEP 803](https://peps.python.org/pep-0803/) (`abi3t`, compatibility tags,
  and the distinction between `abi3t` and `abi3.abi3t`).
- [PyPA platform compatibility tags](https://packaging.python.org/en/latest/specifications/platform-compatibility-tags/)
  (wheel tag fields).
- [CPython free-threading HOWTO](https://docs.python.org/3/howto/free-threading-python.html)
  (checking `Py_GIL_DISABLED` and `sys._is_gil_enabled()`).
- [Maturin bindings guide](https://www.maturin.rs/bindings) and [Python
  Free-Threading Guide CI notes](https://py-free-threading.github.io/ci/)
  (free-threaded interpreter and wheel-tag examples).
- PyO3 0.29.0 CHANGELOG: PR #5564 (opt-out free-threading), PR #5865 (drop
  3.13t), PR #5807 (`abi3t`).
- `pyo3-macros-backend-0.29.2/src/module.rs:394,551`; `pyo3-ffi-0.29.2/src/moduleobject.rs:121`;
  `pyo3-0.29.0/src/impl_/pymodule.rs:349`; `pyo3-0.29.0/src/marker.rs:413`.
- `docs/verification-policy.md`, `.skills/verification-qualification.md`.
- `docs/architecture/python-bindings.md` (the GIL release/acquire contract).
