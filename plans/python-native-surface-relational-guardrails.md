# Python Native Surface Relational Guardrails

Planning baseline: 03ecba973010e2858bf16a2b5f84d51ce70adae4
Parent program: plans/api-preserving-maintenance-and-interop-hardening-program.md
Predecessor work: plans/python-interop-api-hygiene-program.md, plans/python-pep561-method-contract-corrective-pass.md
Normative verification policy: docs/verification-policy.md

## Objective

Strengthen the native Python API checks so the supported sync Client, AsyncClient, top-level helper, runtime manifest, package export, and PEP 561 surfaces cannot drift independently while preserving every existing Python signature and behavior.

The existing Python architecture already centralizes request/client semantics in request_preparation.rs and already has strong per-symbol checks in scripts/check_native_python_api.py and scripts/check_python_typing_surface.py. This plan must build on those checks rather than replace them or create a second transport/request implementation.

The primary gap is relational: a snapshot can prove that one symbol matches its reviewed signature, but it does not by itself express that two families are intentionally mirrors except for a bounded set of known differences.

## Part A — inventory the current relational contracts

At the planning baseline, derive and document the intended relationships among:

- Client and AsyncClient constructors;
- Client.request and AsyncClient.request;
- Client.stream and AsyncClient.stream;
- common verb methods get/post/put/patch/delete/head/options;
- top-level request/get/post/put/patch/delete/head/options helpers;
- __init__.py exports, __init__.pyi exports, _native.pyi declarations, native_api_manifest.json, and PyO3 module registration.

Do not infer that every method must be identical. Record intentional deltas explicitly, including:

- sync versus async method kind;
- AsyncClient acceptance of asynchronous content iterables in typing;
- top-level helpers owning limits/verify/cert behavior differently from reusable-client methods where the current public contract already differs;
- extensions being client-method-only where currently documented;
- lifecycle methods close/aclose and context-manager shapes;
- any signature positional-only marker difference caused by PyO3 that is already reviewed.

The result should be a short source-of-truth table in code comments or documentation owned by the checker, not a new user-facing API document.

## Part B — extend the existing checker rather than create a parallel manifest

Prefer extending scripts/check_native_python_api.py and/or scripts/check_python_typing_surface.py using the existing native_api_manifest.json.

Add relational assertions such as:

1. Client and AsyncClient constructor runtime parameter names/order/default presence are identical.
2. Client.request and AsyncClient.request runtime signatures are identical except method kind.
3. Client.stream and AsyncClient.stream runtime signatures are identical except method kind.
4. The sync/async common verb methods share the same keyword inventory and defaults for corresponding operations.
5. Top-level helper keyword sets match the reviewed subset/superset relationship to Client methods.
6. request/post/put/patch body-capable methods remain aligned with one another, while get/delete/head/options retain their currently body-less convenience signatures.
7. Root runtime __all__, __init__.pyi __all__, and reviewed manifest export order remain identical.
8. Every supported runtime class/function exported at the root has a reviewed stub declaration and every supported stub declaration maps back to an approved runtime export, excluding the existing intentional typing-only aliases.
9. PyO3 registration exposes the implementation objects required by the root package but does not establish _native.__all__ as a second support contract.

If a relational invariant cannot be expressed from the current manifest, add the smallest possible reviewed metadata to the existing manifest. Do not create another JSON/YAML manifest that can drift from native_api_manifest.json.

## Part C — reduce only low-risk declaration duplication

Review the concrete Rust forwarding code in:

- crates/eggfetch-python/src/lib.rs;
- crates/eggfetch-python/src/client.rs;
- crates/eggfetch-python/src/async_client.rs.

The goal is not to force deduplication. The current concrete PyO3 functions make the public signatures auditable.

Permitted cleanup:

- private helper functions for repeated argument forwarding that do not change PyO3 signatures;
- private local structs for internal normalized arguments where they reduce maintenance burden and do not affect extraction/default behavior;
- narrowly scoped macros only when the generated surface remains straightforward to audit and inspect.signature remains byte-for-byte identical.

Do not replace the public method declarations with a large metaprogramming layer merely to reduce line count.

If the relational checker provides the needed protection and executable deduplication would reduce readability, record a deliberate no-change result for the forwarding code.

## Part D — strengthen semantic typing relationships

Use the existing typing-surface checker to assert the bounded semantic differences that runtime signatures cannot encode.

At minimum retain/prove:

- Client content accepts the current synchronous body types;
- AsyncClient content accepts the same synchronous types plus the current async body type;
- sync request/stream return Response/StreamingResponse;
- async request awaits to Response and async stream awaits to StreamingResponse;
- NetworkStream.start_tls returns NetworkStream;
- AsyncNetworkStream.start_tls awaits to AsyncNetworkStream;
- close/aclose and context-manager return annotations remain as reviewed;
- verify/cert/headers/cookies/auth/retry/limits annotations match runtime-supported shapes and are not broadened speculatively.

No typing-only widening is allowed simply because a Python protocol could theoretically accept more values.

## Part E — failure-mode tests

Add focused checker tests or fixture mutations proving the guard fails when:

- one constructor gains/drops/reorders a keyword;
- one sync verb changes while its async mirror does not;
- a top-level helper gains a client-only keyword;
- a root export is present at runtime but absent from __init__.pyi;
- a stub method changes sync/async kind;
- a semantic return annotation drifts while runtime inspect.signature remains unchanged.

Prefer testing the checker itself with temporary fixture copies rather than mutating production source in test setup.

## Part F — validation integration

Keep the existing Tier 1 entry points:

- scripts/check_native_python_api.py;
- scripts/check_python_typing_surface.py;
- scripts/check_python_typing.py.

If a new helper module is introduced for relational logic, invoke it through these existing checks rather than adding another independent validation lane unless separation materially improves maintainability.

Do not add a new GitHub Actions job.

## Focused validation

Required:

- maturin develop -m crates/eggfetch-python/Cargo.toml;
- python scripts/check_native_python_api.py;
- python scripts/check_python_typing_surface.py;
- python scripts/check_python_typing.py;
- python -m pytest crates/eggfetch-python/tests/ -q --ignore=crates/eggfetch-python/tests/compat;
- ./scripts/check.sh.

If executable binding code changes, also run the relevant HTTPX/HTTPX2 compact compatibility tests immediately.

## Acceptance criteria

- [x] Client/AsyncClient constructor parity is mechanically enforced. — `scripts/check_native_python_api.py:42-46` compares the `Client` and `AsyncClient` constructor contracts as one reviewed contract, and `scripts/check_native_python_api.py --self-test` (green on 2026-10-22) mutates each constructor to prove the check fires.
- [x] request/stream sync-async mirror relationships are mechanically enforced. — the reviewed `mirror_methods` set (`request`, `stream`, `get`, `post`, `put`, `patch`, `delete`, `head`, `options`) is compared across `Client` and `AsyncClient` at `scripts/check_native_python_api.py:46-51`, with matching mutation self-tests.
- [x] common HTTP verb method relationships are mechanically enforced. — the same `mirror_methods` comparison covers the nine common HTTP verb methods; `scripts/check_native_python_api.py --self-test` proves a single changed parameter name/order/default is rejected.
- [x] top-level helper versus reusable-client intentional differences are explicit and tested. — the intentional differences are encoded at `scripts/check_native_python_api.py:53-74` (top-level helpers omit the reusable-client-only keywords) and each difference is asserted by its own mutation self-test (`:186`, `:192`, `:201`, `:212`).
- [x] runtime exports, root package exports, reviewed stubs, and native manifest remain one coherent support contract. — `crates/eggfetch-python/tests/native_api_manifest.json` is the single reviewed contract (66 exports, symbol kinds, exception bases and MRO, signatures, members, semantic contracts); `scripts/check_native_python_api.py` reports `Native API manifest passed (66 exports)` on 2026-10-22.
- [x] semantic typing differences that inspect.signature cannot express are retained and tested. — the manifest's `semantic_contracts` section records them explicitly, and `scripts/check_python_typing_surface.py --self-test` reports `Python typing surface passed (66 runtime exports, 32 exception bases, 24 reviewed member contracts)` on 2026-10-22.
- [x] No native Python public signature, property, exception hierarchy, or return shape changed. — for this plan's own scope at its recorded freeze `df2549f7c64ebfccde61ed36fef785d39e83b38d`, yes: the guardrails were added as checks, not as signature edits. Note for the reader: the *current uncommitted* tree has since gained one symmetric keyword-only constructor parameter (`proxy_env=None`) on both `Client` and `AsyncClient` under a later, separately planned milestone — see the diff in `crates/eggfetch-python/tests/native_api_manifest.json`. The parity condition still holds, because both constructors changed identically and the checker is green.
- [x] No new public export was added. — the manifest export count is unchanged at 66 (`Native API manifest passed (66 exports)`); the guardrails introduced no runtime export.
- [x] No large code-generation layer obscures the supported signatures. — the enforcement is a plain declarative manifest plus a Python checker; the PyO3 declarations remain concrete, as the execution record below states.
- [x] Tier 1 remains green. — the execution record below names the native API, typing, 578 routine Python tests, and extended compatibility qualification as passing at freeze `df2549f7c64ebfccde61ed36fef785d39e83b38d`. The manifest and typing checkers were re-run green on 2026-10-22; the full Tier 1 gate was **not** re-run in this session (the maintainer deferred it), so this criterion is recorded as green as of the named freeze.

## Stop conditions

If a proposed deduplication changes inspect.signature, argument extraction order, default semantics, exception timing/type, sync/async body acceptance, return types, or root exports, retain the explicit declarations and rely on stronger relational checks instead.

Do not modify HTTPX/HTTPX2 facade contracts as part of this plan except to prove they remain unaffected.

## Execution record — complete (2026-09-21)

Implemented on executable freeze `df2549f7c64ebfccde61ed36fef785d39e83b38d`.
The existing native manifest and typing checkers now enforce the reviewed
sync/async/top-level relationships with explicit intentional deltas and
mutation self-tests. Concrete PyO3 declarations were retained. Native API,
typing, 578 routine Python tests, and the extended compatibility qualification
passed without public-surface drift.

## Acceptance-criteria disposition (2026-10-22)

All ten acceptance criteria above were re-verified against the current tree and
are **met**; none is superseded, deferred, or never applicable. Evidence is
cited inline with each criterion. The execution record above is unchanged.

Two criteria are explicitly bounded to the plan's recorded freeze
`df2549f7c64ebfccde61ed36fef785d39e83b38d` — the public-signature criterion and
the Tier 1 criterion — because the current uncommitted tree carries later
milestone work. The full Tier 1 gate was not re-run in this session.
