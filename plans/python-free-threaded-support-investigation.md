# Free-Threaded Python Support Investigation

Planning baseline: `272c82215fbf2f0193748e9d862931f8c2b4169a` (main, post-0.4.0 publication)

Subsystem: Python bindings and HTTPX compatibility
(`plans/subsystems/python-httpx-compat-roadmap.md`)

Normative verification policy: `docs/verification-policy.md`

## Objective

Determine how much work would be required to run `eggfetch`'s Python bindings on
a free-threaded (PEP 703, `Py_GIL_DISABLED`) CPython build, and whether the
resulting capability justifies registering a milestone.

This document is a research record. It proposes no implementation, changes no CI
matrix combination, publishes no wheel, and makes **no** free-threaded
qualification claim.

## Scope

In scope:

- A source-level audit of every place the binding depends on the GIL.
- The upstream support matrix (PyO3, `pyo3-async-runtimes`, maturin,
  `setup-python`, CPython).
- The packaging and CI surface that a free-threaded wheel would change.
- A ranked risk inventory with the evidence each risk rests on.

Out of scope: implementation, CI edits, wheel builds, publication, and any
statement that the current binding is or is not safe under `Py_GIL_DISABLED`
without a runtime qualification.

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
  and `EXPECTED_PLATFORMS` (4 platforms), and rejects `abi3` tags
  explicitly. `.github/workflows/pypi.yml` additionally asserts exactly 24
  wheels and 25 distributions.
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

Established in 2.3. `scripts/validate_wheel_coverage.py` accepts only
`cp310`-`cp315` and rejects `abi3`; `pypi.yml` asserts exactly 24 wheels
and 25 distributions. A free-threaded wheel set (`cp314t`, `cp315t`) would
fail the coverage validator and the count assertions. Note that a `t` tag
also fails `normalize_python`, which only recognises `cpNNN` of length 5 -
so the validator needs an explicit `t`-aware rule, not just a wider
`EXPECTED_PYTHON_VERSIONS` set.

### F6 - 3.13t is not a viable target for this PyO3 version (fact)

PyO3 0.29 dropped 3.13t support (PR #5865) because CPython declared
free-threading supported starting with 3.14. The realistic free-threaded
matrix is 3.14t and 3.15t.

### F7 - `abi3t` is available but conflicts with current policy (fact)

PyO3 0.29 exposes `abi3t` / `abi3t-py315` for the PEP 803 Stable ABI.
However `scripts/validate_wheel_coverage.py` rejects `abi3` tags by
explicit plan decision, and a single `abi3t` wheel would replace the
per-version wheel matrix rather than extend it. Adopting `abi3t` would
reverse an existing documented decision and is a separate decision from
"support free-threaded builds".

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

Presented for a maintainer decision. None has been implemented.

### Option A - Free-threaded wheels alongside the GIL wheels (recommended)

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

### Option C - `abi3t` Stable ABI

Target PEP 803 via PyO3's `abi3t` feature so one wheel per platform covers
both GIL-enabled and free-threaded CPython 3.15+.

- Attractive in principle: one wheel instead of N per platform.
- Requires reversing the existing no-`abi3` decision, confirming that the
  binding's use of the full CPython API (not the limited API) is
  `abi3t`-compatible, and reworking the wheel matrix and validator around a
  different tag shape.
- Out of proportion to the problem unless the release process wants to move
  toward the Stable ABI generally.

## 8. Evidence required before any support claim

The repository's verification policy requires evidence, not reasoning. A
free-threaded claim would need, at minimum:

1. A wheel built against a free-threaded interpreter, imported, and shown to
   report the GIL as disabled at runtime (rather than relying on the
   `Py_MOD_GIL_NOT_USED` declaration alone).
2. The existing Python suite run on that interpreter, including the sync and
   async request, streaming, cookie, and network-stream paths.
3. A concurrency test that exercises F3 directly: two threads issuing requests
   against one `Client` with a shared trace callback and a shared request-body
   iterator, asserting the callback's own synchronization rather than relying
   on the GIL.
4. A sub-interpreter test (or an explicit statement that sub-interpreters are
   unsupported) covering F2.
5. The exact-SHA qualification rules in `.skills/verification-qualification.md`
   applied to the SHA that carries the change.

## 9. Open maintainer decisions

1. Adopt, defer, or scope free-threaded wheel support (F5, Options A/B/C).
2. Whether the 8 extra wheel builds and CI jobs are acceptable under the
   one-job CI policy and the no-new-matrix-combinations rule.
3. Whether F3's concurrency semantics change is acceptable to document, or
   whether the binding should add explicit serialization around user
   callbacks.
4. Whether `abi3t` / PEP 803 should be revisited as a separate question from
   free-threading (F7).

## 10. References

- PyO3 user guide, "Supporting Free-Threaded CPython".
- PyO3 0.29.0 CHANGELOG: PR #5564 (opt-out free-threading), PR #5865 (drop
  3.13t), PR #5807 (`abi3t`).
- `pyo3-macros-backend-0.29.2/src/module.rs:394,551`; `pyo3-ffi-0.29.2/src/moduleobject.rs:121`;
  `pyo3-0.29.0/src/impl_/pymodule.rs:349`; `pyo3-0.29.0/src/marker.rs:413`.
- CPython HOWTO: "Python support for free threading" and "C API Extension
  Support for Free Threading".
- maturin user guide, "Bindings"; `PyO3/maturin` PR #2310.
- `docs/verification-policy.md`, `.skills/verification-qualification.md`.
- `docs/architecture/python-bindings.md` (the GIL release/acquire contract).
