# Python Public API and Package Correctness

Status: **implemented; final qualification pending**
Parent: `python-interop-api-hygiene-program.md`  
Date: 2026-09-15

## Scope

Establish one authoritative native Python package contract and eliminate release metadata drift before deeper binding refactors begin.

This plan owns:

- runtime/package version consistency;
- top-level native export policy;
- exception/network-stream export decisions;
- native Python API manifest/oracle coverage; and
- wheel/package validation required to keep those contracts correct.

It does not own SSL interop refactoring, async request-body support, PyO3 upgrades, or PEP 561 stubs beyond making their future source of truth explicit.

## Baseline Defects

### Version drift

`crates/eggfetch-python/src/lib.rs` currently exports `_native.__version__ = "0.1.0"`, while `crates/eggfetch-python/Cargo.toml` and `pyproject.toml` are `0.1.4`.

Current wheel smoke only asserts that `eggfetch.__version__` exists and is non-empty. The package-content validator cannot catch the mismatch because top-level `__init__.py` imports the native version rather than assigning a literal.

### Public export drift

`_native.__all__` includes symbols not exported by `eggfetch.__init__`, including at least:

- `NetworkStream`;
- `AsyncNetworkStream`;
- `DecompressionError`;
- `UnsupportedContentEncoding`;
- `H3Error`;
- `H3ConnectError`;
- `H3ProtocolError`.

The architecture documentation already describes some of these as part of the native exception or network-stream surface.

There is no native API manifest comparable to the compatibility-facade API oracles.

## Design Decisions

### 1. Top-level package is authoritative

Define `eggfetch.__all__` as the supported native Python API contract.

`eggfetch._native` is an implementation module. It may contain implementation-visible classes/functions needed by the pure-Python facade, but its `__all__` must not create a second independent support promise.

Preferred implementation choices, in order:

1. remove `_native.__all__` entirely if nothing legitimately consumes wildcard import from the extension; or
2. generate/derive it from the same explicit source used by top-level `eggfetch.__all__`; or
3. keep it as a strict superset only if that distinction is documented and mechanically validated.

Do not leave two hand-maintained lists that can drift silently.

### 2. Exception exposure

Top-level native Python should export the documented exception hierarchy, including decompression and HTTP/3 exceptions, unless there is a specific reason to make one private.

Any intentional non-export must be removed from public docs and tested as private. Do not retain accidental mismatches.

### 3. Network-stream exposure

Make an explicit decision for `NetworkStream` and `AsyncNetworkStream`.

Preferred outcome: export them at top level because public responses may expose these concrete objects through `extensions["network_stream"]` for 101 upgrades and because the classes have usable public methods (`read`, `write`, `close`/`aclose`, `start_tls`, `get_extra_info`, `is_upgraded`).

If implementation instead decides these should remain nominally private, remove them from any public/native export declaration and document/type the returned object through an explicit protocol. Do not leave the current ambiguous state.

## Version Ownership

Replace the hard-coded native version with a build-derived value:

```rust
m.add("__version__", env!("CARGO_PKG_VERSION"))?;
```

or an equally mechanical single-source solution.

Because Rust crate and Python distribution versions are release-locked today, this is preferred over adding another generated version file.

Update release/version validation so a version bump fails if any of these disagree:

- `crates/eggfetch-python/Cargo.toml`;
- `crates/eggfetch-python/pyproject.toml`;
- installed distribution metadata;
- `eggfetch.__version__` at runtime.

### Wheel smoke assertion

Inside the isolated installed-wheel test, add:

```python
from importlib.metadata import version
assert eggfetch.__version__ == version("eggfetch")
```

Do not merely compare source files; validate the installed artifact.

## Native API Oracle

Add a small checked native API manifest/oracle independent of the HTTPX facades.

Recommended location:

```text
crates/eggfetch-python/tests/native_api_manifest.json
scripts/generate_native_python_api_manifest.py
scripts/compare_native_python_api_manifest.py
```

A simpler checked Python constant/test is acceptable if it provides equivalent deterministic coverage.

At minimum record/verify:

- top-level exported names (`eggfetch.__all__`);
- symbol kind (class/function/singleton/exception where useful);
- exception inheritance/MRO for public exception types;
- public function signatures for top-level helpers;
- constructor signatures for `Client`, `AsyncClient`, `Timeout`, `Limits`, auth/retry/file classes;
- request/stream method signatures where accidental drift would materially affect users;
- explicit presence/absence of `NetworkStream` and `AsyncNetworkStream` according to the chosen policy.

Do not turn the oracle into an exhaustive dump of every method descriptor or implementation detail. The goal is support-contract drift detection.

## Public API Tests

Add direct tests that verify:

- every name in `eggfetch.__all__` exists;
- `from eggfetch import *` produces the intended surface;
- no documented public exception is absent;
- exception hierarchy matches the native contract;
- `_native` implementation extras cannot accidentally become top-level public merely by being registered in Rust;
- runtime version equals installed distribution version in wheel/package smoke;
- public classes report expected `__module__` / discoverability behavior where relevant.

## Documentation

Update native Python documentation to state explicitly:

- `eggfetch` is the supported import surface;
- `_native` is private;
- the public exception hierarchy;
- whether network-stream classes are public concrete classes or protocol-only returned objects;
- version ownership/release behavior if documented elsewhere.

Do not conflate this with HTTPX compatibility exports. `eggfetch.compat.httpx` and `eggfetch.compat.httpx2` retain their own versioned contracts.

## Validation

During implementation run:

```text
./scripts/check.sh
./scripts/check.sh package
```

plus the new native API oracle and isolated wheel-smoke version assertion.

If top-level exports are intentionally expanded, add direct native tests before changing the compatibility facade. Compatibility requalification is deferred to the parent program's final closure plan.

## Acceptance Criteria

- [x] `_native.__version__` is not hard-coded independently of package release metadata.
- [x] installed wheel runtime version equals `importlib.metadata.version("eggfetch")`.
- [x] wheel/package validation fails on version mismatch.
- [x] `eggfetch.__all__` is documented as the authoritative native public surface.
- [x] `_native.__all__` is removed, derived, or otherwise prevented from becoming an independent drifting contract.
- [x] documented decompression and H3 exceptions are intentionally exported or deliberately reclassified/private with matching documentation.
- [x] network-stream public/private status is explicit and tested.
- [x] native API oracle exists and covers root exports, important signatures, and exception hierarchy.
- [x] native API oracle runs in repository validation at an appropriate tier.
- [x] package and wheel smoke tests pass locally.
- [ ] no HTTP behavior changes are introduced.
- [x] no `eggfetch-core` API change is required.

## Handoff

Once this pass is green, continue with `python-interop-boundary-and-async-body.md`. Do not perform exact-SHA HTTPX/HTTPX2 profile rebinding yet; final closure owns that work.
