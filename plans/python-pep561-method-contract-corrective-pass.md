# Python PEP 561 Method Contract Corrective Pass

Status: **ready for implementation**
Date: 2026-09-15
Parent historical program: `python-interop-api-hygiene-program.md`
Preceding typing plan: `python-pep561-typing-surface.md`
Closure handoff: `post-python-typing-corrective-qualification-and-closure.md`

## Purpose

Correct the narrow residual left after the completed Python interop/API-hygiene program: the packaged PEP 561 stubs cover the public export set, but the current structural checker does not prove that public class members, properties, argument contracts, and return annotations match the actual PyO3 runtime API.

This is a typing-contract correction, not a Python runtime redesign and not a new HTTP feature program.

## Current Qualified Baseline

The current executable/package qualification freeze before this corrective work is:

`2281345f3eaf636c62ec21d2c963d6f90ea764a8`

The current `main` descendant is documentation/profile/ledger-only relative to that freeze.

The completed interop program already established:

- `eggfetch.__all__` as the supported native Python package surface;
- a native API manifest with exports, symbol kinds, exception inheritance, and PyO3 runtime signatures;
- `py.typed`, `__init__.pyi`, and `_native.pyi` in the package;
- consumer typing fixtures and installed-wheel typing checks;
- PyO3 0.29.x and Python 3.10–3.14 qualification;
- exact-SHA HTTPX 0.28.1 and HTTPX2 2.12.0 Stage C qualification.

Preserve all of those decisions.

## Confirmed Residuals

The following current stub/runtime mismatches are already confirmed and must be corrected unless a source audit proves the runtime contract has changed before implementation.

### Network stream return types

Runtime behavior:

- `NetworkStream.start_tls(...)` returns a new `NetworkStream`;
- awaiting `AsyncNetworkStream.start_tls(...)` returns a new `AsyncNetworkStream`.

Current `_native.pyi` incorrectly annotates both as returning `None`.

Correct the stubs to the actual runtime return values. Do not change runtime behavior merely to match the current stubs.

### Missing network-stream properties

Both runtime stream classes expose the `is_upgraded` property. The current stubs omit it.

Add:

```python
@property
def is_upgraded(self) -> bool: ...
```

or the equivalent valid stub representation to both stream classes.

### Missing client members

The native runtime exposes members that are not represented in the current stubs:

- `Client.is_closed: bool`;
- `Client.cookies: Cookies`;
- `AsyncClient.close() -> None`;
- `AsyncClient.is_closed: bool`;
- `AsyncClient.cookies: Cookies`.

Represent these accurately. Preserve existing `Client.close()` and `AsyncClient.aclose()` typing.

### Verify input under-typing

The native TLS boundary accepts:

- `bool`;
- CA path `str`;
- `ssl.SSLContext`;
- a concrete Python `list[bytes]` of DER certificates.

The current `Verify` alias omits the DER-list form. Add only the form actually accepted by the runtime. Do not widen this to arbitrary `Sequence[bytes]`, tuple, file-like object, or path-like object unless runtime code is intentionally changed and separately reviewed.

### Context-manager return precision

Audit the actual runtime return values for:

- `Client.__exit__`;
- `AsyncClient.__aexit__`;
- `StreamingResponse.__exit__`;
- `StreamingResponse.__aexit__`.

The current stubs use `None` in several places even where PyO3 returns a boolean false value. Type the real runtime contract. Do not normalize these to an HTTPX idealized signature.

### AsyncClient constructor precision

The runtime native API manifest has an explicit `AsyncClient` constructor signature matching the supported constructor surface, while `_native.pyi` currently collapses it to `**kwargs: Any`.

Expand the stub constructor to the actual accepted keyword set and types, reusing the same aliases as `Client` where appropriate. Keep runtime-specific differences explicit if any exist.

## Full Stub Audit Required

The confirmed mismatches above are the minimum correction set, not permission to stop after those edits.

Perform a complete audit of every public native class and helper represented by `_native.pyi` against:

1. `crates/eggfetch-python/tests/native_api_manifest.json`;
2. current PyO3 implementation source;
3. runtime introspection where reliable;
4. focused behavior tests for return/value semantics that signatures alone cannot prove.

At minimum review:

- constructor parameter names and keyword-only structure;
- public methods;
- public properties/getters;
- sync versus async method classification;
- return types;
- context-manager protocols;
- iterator protocols;
- streaming APIs;
- top-level helper body/input types;
- exception types;
- `Response`/`StreamingResponse` mutable/read-only attribute assumptions;
- `NetworkStream`/`AsyncNetworkStream` members;
- `Client`/`AsyncClient` lifecycle members.

Do not add private PyO3 implementation helpers to the supported typing surface.

## Strengthen the Drift Checker

The existing `scripts/check_python_typing_surface.py` intentionally checks structure at export/exception level and explicitly is not a second signature oracle. Preserve that useful role, but extend the validation system so the repository also detects the class-member drift that escaped the first closure.

### Required validation layers

Keep the existing checks for:

- package typing files present;
- all runtime exports represented;
- `__init__.pyi.__all__` equality;
- intentional typing-only aliases;
- exception base classes.

Add a method/property contract check that proves, for the supported native API:

- every manifest-tracked public runtime method exists in the stub;
- sync runtime methods are not declared async and vice versa;
- manifest parameter names/order/kind/default structure match where introspection is reliable;
- required public properties are represented;
- public stub members absent from runtime are rejected unless explicitly allowlisted as typing-only protocol conveniences;
- selected semantic return annotations are checked against a reviewed expectation table for contracts runtime signatures cannot encode.

### Source of truth

Prefer extending the existing native API manifest rather than creating an unrelated second manifest.

A reasonable design is to add reviewed data such as:

```json
{
  "members": {
    "NetworkStream": {
      "properties": {"is_upgraded": "bool"},
      "returns": {"start_tls": "NetworkStream"}
    }
  }
}
```

The exact schema is an implementation choice. It must remain small, reviewable, and tied to the existing public native API oracle.

Do not attempt to synthesize Python typing from Rust types automatically. Runtime PyO3 signatures do not contain enough information for unions such as `SyncBody | AsyncBody`, and the reviewed stub remains authoritative for Python type semantics.

### Avoid broad allowlists

If PyO3 introspection cannot represent a detail reliably, allowlist only the exact member/detail. Do not suppress whole classes or all signatures.

## Consumer Typing Fixtures

Add or extend downstream-style fixtures so the corrected types are actually exercised by the type checker.

Required positive examples include:

```python
stream2: eggfetch.NetworkStream = stream.start_tls(ctx, "example.com")
upgraded: bool = stream.is_upgraded
```

and:

```python
stream2: eggfetch.AsyncNetworkStream = await stream.start_tls(ctx, "example.com")
closed: bool = client.is_closed
cookies: eggfetch.Cookies = client.cookies
```

Also cover:

- `AsyncClient(...)` constructor options without falling back to untyped `Any`;
- DER certificate list accepted by `verify=`;
- context-manager return/control-flow typing;
- negative sync-client async-body fixture remains rejected statically;
- an unsupported `verify=` sequence form remains rejected statically if runtime rejects it.

Fixtures must not perform network I/O.

## Runtime Regression Tests

This pass should normally require no runtime behavior change. Add only narrow runtime tests needed to prove the documented semantics used by the stubs, especially:

- sync/async `start_tls` result type;
- `is_upgraded` existence/type;
- client `is_closed` and `cookies` access;
- concrete DER-list `verify` acceptance/type rejection boundary;
- context-manager return values where required for typing precision.

If a runtime defect is discovered while reconciling the stubs, stop and record it explicitly before broadening this corrective pass. Do not silently change public behavior to make a typing assertion convenient.

## Packaging and Validation Integration

The corrected checker must remain part of routine validation. The installed-wheel typing gate must exercise the same corrected public contract from an isolated installed artifact.

At minimum confirm:

```text
python scripts/check_python_typing_surface.py
python scripts/check_python_typing.py
./scripts/check.sh
./scripts/check.sh package
```

If a new helper script is introduced, wire it into the existing canonical validation path rather than creating an undocumented optional check.

## Compatibility / Qualification Sensitivity

Changes to `.pyi` files, native API manifests, typing validation scripts, tests, or packaged Python artifacts are qualification-sensitive under the repository's exact-SHA policy.

Therefore the current `2281345f...` Stage C binding becomes historical after implementation. Do not update compatibility profiles during intermediate edits. Finish this corrective pass, then execute `post-python-typing-corrective-qualification-and-closure.md` once on a frozen candidate.

## Non-Goals

- no new runtime HTTP features;
- no `eggfetch-core` API change unless an independently proven runtime defect requires it;
- no change to HTTPX/HTTPX2 public compatibility semantics;
- no broad compatibility-facade annotation campaign;
- no PyO3 upgrade beyond the already-qualified 0.29 line;
- no Python floor/matrix change;
- no free-threaded or abi3/abi3t work;
- no automatic stub generation replacement;
- no widening accepted Python inputs merely to simplify annotations.

## Acceptance Criteria

- [ ] all confirmed stub/runtime mismatches listed above are corrected;
- [ ] `NetworkStream.start_tls` is typed as returning `NetworkStream`;
- [ ] `AsyncNetworkStream.start_tls` is typed as awaiting to `AsyncNetworkStream`;
- [ ] both stream classes expose typed `is_upgraded: bool`;
- [ ] `Client` and `AsyncClient` lifecycle/cookie members match runtime;
- [ ] `Verify` includes exactly the supported DER-list form without unsupported widening;
- [ ] `AsyncClient.__init__` no longer collapses the supported constructor to `**kwargs: Any`;
- [ ] context-manager exit return annotations match actual runtime values;
- [ ] every manifest-tracked public method is represented in `_native.pyi`;
- [ ] public runtime/stub member drift is mechanically detected;
- [ ] selected semantic return types are mechanically checked through a reviewed contract table/manifest extension;
- [ ] allowlists, if any, are member-specific and documented;
- [ ] positive and negative consumer typing fixtures exercise the corrected contracts;
- [ ] installed-wheel typing validation catches the same drift;
- [ ] no unintended runtime API change is introduced;
- [ ] no `eggfetch-core` Python-specific coupling is introduced;
- [ ] Tier 1 and package validation pass before final freeze;
- [ ] final exact-SHA closure is handed off to `post-python-typing-corrective-qualification-and-closure.md`.

## Handoff

Once implementation and focused validation are stable, freeze a single candidate and execute `post-python-typing-corrective-qualification-and-closure.md`. Do not mark the earlier `python-pep561-typing-surface.md` historical record false; record this corrective as a follow-up that strengthens the originally incomplete method-level drift guarantee.