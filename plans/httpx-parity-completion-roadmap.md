# HTTPX 0.28.1 Parity Completion Roadmap

Status: ready for implementation handoff

Date: 2026-08-07

Audited baseline: `c66360827489c988f37b4aa9bd615b612258825d`

Last exact executable compatibility evidence SHA: `cf4680ac056bf241ca4f4e8fa0e076459bccc9e3`

Pinned reference: `httpx==0.28.1`

Current designation: `Stage C candidate`

## Purpose

Finish the remaining practical HTTPX 0.28.1 parity work after the earlier request/response, redirect, cookie, auth, transport-routing, streaming, compressed-raw, cancellation, and evidence closure passes.

This is not a restart of the HTTPX compatibility project. The high-risk behavioral core is already substantially closed. The remaining work is a bounded combination of:

1. correcting stale or inaccurate compatibility records;
2. eliminating inexpensive public Python-contract mismatches that are currently allowlisted;
3. making signatures and stream/type relationships match the pinned public API where doing so is low-risk;
4. implementing currently accepted-but-rejected low-level transport options (`uds`, `local_address`, `socket_options`);
5. adding HTTPX's optional SOCKS proxy capability without contaminating the default transport path;
6. rerunning exact differential/API evidence and reducing the active allowlist to differences that are genuinely intentional or explicitly deferred.

The program is split into six implementation plans:

- `plans/httpx-parity-completion-phase-1-contract-rebaseline.md`
- `plans/httpx-parity-completion-phase-2-python-object-contracts.md`
- `plans/httpx-parity-completion-phase-3-signatures-stream-types.md`
- `plans/httpx-parity-completion-phase-4-advanced-direct-transport.md`
- `plans/httpx-parity-completion-phase-5-socks-proxy.md`
- `plans/httpx-parity-completion-phase-6-differential-closure.md`

## Current state motivating this roadmap

The existing exact-SHA evidence records:

- routine `./scripts/check.sh`: passed;
- full pinned compatibility suite: `1384 passed, 0 failed, 0 skipped, 0 xfailed` against HTTPX 0.28.1;
- API oracle: 121 allowed matches, zero unexplained differences, zero stale entries, zero requires-resolution entries;
- current public designation: Stage C candidate for the documented asyncio-supported surface.

Those results establish that the existing allowlist is internally consistent. They do **not** establish that every allowlisted difference is desirable to retain.

The current active difference ledger still includes source-visible differences such as:

- `Headers` not inheriting `MutableMapping`, with `setdefault()` and `popitem()` absent;
- `QueryParams` not inheriting `Mapping`;
- `StreamError` inheriting `Exception` instead of `RuntimeError`;
- no exact `NetRCAuth(file=...)` keyword;
- missing `URL.raw`;
- `codes` implemented as a plain constant namespace rather than `IntEnum`;
- helper and method signatures represented by broad `*args`/`**kwargs` forms;
- constructor/default differences for configuration objects;
- stream class and transport base-class/type-shape differences;
- transport parameters that are accepted by the compatibility facade but deliberately rejected before network activity.

The current documentation also contains at least one factual defect: `docs/reference/compatibility.md` says SOCKS is not in the HTTPX 0.28.1 public API, while HTTPX documents SOCKS as an optional public proxy capability (`httpx[socks]`). The README correctly lists SOCKS as unsupported, but its short feature headline calls the facade an "HTTPX drop-in" while later text correctly bounds that claim.

## Program principles

### 1. Preserve the accepted behavioral core

Do not reopen already-proven redirect, cookie, auth replay, compressed raw-stream, cancellation, lifecycle, mount routing, WSGI/ASGI, custom transport, or response metadata behavior unless a new pinned differential test demonstrates a real regression.

### 2. Prefer deleting allowlist entries over rationalizing cheap incompatibilities

If the mismatch is public, source-visible, easy to reproduce, and inexpensive to match, fix it. Do not retain it merely because existing code has already been written around it.

### 3. Keep architectural boundaries intact

There remains exactly one real networking implementation: `eggfetch-core` on Tokio/Hyper. The HTTPX compatibility facade may adapt API semantics but must not grow a second Python networking stack.

### 4. Advanced transport additions must not tax the default path

UDS, explicit local binding, socket options, and SOCKS are opt-in. Their implementation must preserve the existing direct HTTP/HTTPS fast path and pooling behavior when these options are absent.

### 5. Verification stays proportional

Routine CI remains the existing single lightweight path invoking `./scripts/check.sh`. The complete pinned HTTPX suite and API oracle remain extended/manual qualification gates. Do not add matrices, scheduled jobs, new release automation, or evidence dashboards for this work.

## Explicit scope

### In scope

- `crates/eggfetch-python/python/eggfetch/compat/httpx/` public compatibility facade;
- compatibility tests under `crates/eggfetch-python/tests/compat/`;
- `compat/httpx/0.28.1/` profile, allowlists, manifests, inventories, and evidence records;
- narrowly required `eggfetch-core` connector/transport changes for UDS, local binding, socket options, and SOCKS;
- narrowly required Python native-binding plumbing for those transport options;
- documentation wording needed to keep the compatibility claim truthful;
- dependency changes only when a transport primitive cannot be implemented safely with the existing dependency set.

### Explicitly deferred / out of scope

- Trio backend support;
- a general AnyIO abstraction layer;
- Python 3.8 or 3.9 support;
- HTTPX versions other than 0.28.1;
- private HTTPX module emulation solely for private import compatibility;
- reproducing httpcore internals;
- a Python TLS/network stack alongside the Rust engine;
- changing the project's async runtime away from Tokio;
- release cadence or publication automation;
- expanding routine CI beyond the existing lightweight job;
- performance redesign unrelated to the opt-in transport features;
- broad refactors of accepted redirect, cookie, auth, decompression, retry, or pool logic.

Trio/AnyIO and Python 3.8/3.9 may be reconsidered only if a concrete downstream consumer demonstrates that they are required for the project's intended replacement use case. Do not implement them merely to increase a parity percentage.

## Phase 1 — Contract rebaseline and truthfulness

Implement `httpx-parity-completion-phase-1-contract-rebaseline.md` first.

Goals:

- freeze the current exact baseline;
- regenerate or refresh the API/inventory view from current main;
- classify each active difference as `must-close`, `intentional`, or `deferred`;
- correct stale compatibility documentation before implementation starts;
- produce a finite implementation inventory for Phases 2–5.

This phase is documentation/evidence work only. It must not change runtime semantics.

Gate to Phase 2:

- the active implementation inventory names every public mismatch intended for closure;
- SOCKS is no longer described as absent from HTTPX's public API;
- the README short claim no longer overstates unrestricted drop-in compatibility;
- stale upstream inventory material is regenerated or clearly labeled historical;
- no difference is classified as intentional solely because it is inconvenient to fix.

## Phase 2 — Python object and configuration contracts

Implement `httpx-parity-completion-phase-2-python-object-contracts.md`.

Primary targets:

- `Headers` / `QueryParams` collection contracts;
- exception inheritance and stream-error constructors;
- `NetRCAuth(file=...)`;
- `URL.raw`;
- `codes` type behavior;
- `Timeout`, `Limits`, `Proxy`, and `default_encoding` public semantics where the mismatch can be corrected without a transport redesign.

Gate to Phase 3:

- every targeted object-level difference has a direct HTTPX 0.28.1 differential test;
- targeted allowlist records are moved to the resolved ledger only after those tests pass;
- no duplicate-header, query ordering, exception mapping, or response-decoding regression appears.

## Phase 3 — Exact signatures, transport inheritance, and stream type surface

Implement `httpx-parity-completion-phase-3-signatures-stream-types.md`.

Goals:

- replace broad helper/method signatures with the pinned HTTPX public signature shape;
- align keyword-only/default behavior where semantically supported;
- make transport and stream base classes reflect HTTPX's public relationships where doing so does not require private-module emulation;
- preserve all current dispatch behavior.

This is a Python-surface pass. It must not be used to smuggle in native transport features assigned to Phases 4–5.

Gate to Phase 4:

- `inspect.signature` parity is exact for the targeted public symbols or the remaining tuple is explicitly reviewed as intentional/deferred;
- invalid positional/keyword calls fail with reference-compatible exception classes;
- stream lifecycle behavior remains unchanged from the already-accepted raw/decoded closure.

## Phase 4 — Advanced direct transport parity

Implement `httpx-parity-completion-phase-4-advanced-direct-transport.md`.

Goals:

- make `HTTPTransport` and `AsyncHTTPTransport` `uds`, `local_address`, and `socket_options` parameters functional rather than accepted-then-rejected;
- preserve pool isolation between incompatible connector configurations;
- keep default direct HTTP/HTTPS behavior unchanged;
- define platform behavior explicitly.

Implementation should use existing Tokio/Hyper primitives where practical. A small focused socket dependency may be added only if required for correct socket-option semantics; do not introduce a general networking abstraction.

Gate to Phase 5:

- UDS works end to end on supported Unix targets;
- `local_address` demonstrably binds the outbound connection;
- supported socket options demonstrably affect the created socket before connect;
- invalid/unsupported option behavior matches the pinned reference closely enough to remove the active functional-gap entries;
- pool reuse cannot cross connector configurations.

## Phase 5 — SOCKS proxy parity

Implement `httpx-parity-completion-phase-5-socks-proxy.md`.

Goals:

- support the SOCKS proxy capability that HTTPX exposes through its optional `httpx[socks]` surface;
- support the exact schemes/authentication behavior required by HTTPX 0.28.1 rather than inventing a wider proxy product;
- integrate with existing proxy selection, `NO_PROXY`, timeout, TLS, error mapping, and pool ownership;
- keep SOCKS code feature-gated or otherwise absent from the normal direct path when unused.

Do not implement SOCKS4, UDP ASSOCIATE, proxy chaining, or a generic proxy framework unless the pinned HTTPX reference requires them.

Gate to Phase 6:

- an end-to-end local SOCKS fixture proves traffic traverses the proxy;
- username/password behavior is differentially verified when supported by the reference;
- DNS-resolution behavior for the accepted scheme(s) is pinned by tests rather than assumption;
- HTTPS through SOCKS completes TLS to the origin correctly;
- errors map to the compatibility exception family without credential leakage;
- default builds/requests remain unaffected when SOCKS is unused.

## Phase 6 — Differential closure and final designation

Implement `httpx-parity-completion-phase-6-differential-closure.md` last.

Required work:

- rerun routine validation;
- rerun the full pinned HTTPX compatibility suite;
- regenerate and compare the API manifest;
- refresh the upstream test inventory/evidence records;
- reduce `allowed-differences.toml` to only reviewed intentional/deferred entries;
- update `resolved-differences.toml` for newly closed differences;
- reconcile README/reference wording with the final proved surface;
- record exact implementation and evidence SHAs without conflating documentation descendants with executable test SHAs.

Do not promote to Stage D automatically. The implementation may justify stronger wording about the supported HTTPX 0.28.1 asyncio surface, but any stage change must be explicit and evidence-backed.

## Global acceptance criteria

The roadmap is complete only when all of the following are true.

### Public Python contract

- No known inexpensive source-visible mismatch remains allowlisted merely for implementation convenience.
- `Headers` and `QueryParams` satisfy the targeted HTTPX collection contracts.
- stream exception inheritance and call behavior match the reference for the targeted classes.
- `NetRCAuth(file=...)` works.
- `URL.raw` matches the reference representation and byte-preservation semantics.
- `codes` matches HTTPX's public enum behavior for the covered status codes.
- targeted configuration/default semantics and function signatures match the pinned reference.

### Advanced transports

- low-level direct transport parameters are not silently ignored and are not rejected if this roadmap marks them implemented;
- UDS, local binding, and socket options have real end-to-end proof;
- SOCKS has real end-to-end proxy traversal proof;
- pool keys/connector ownership prevent reuse across incompatible transport configurations;
- TLS verification, proxy auth, timeout, cancellation, and close semantics remain correct;
- credentials never appear in Debug/Display/error output.

### Evidence and repository hygiene

- `./scripts/check.sh` passes on the final executable tree;
- the full pinned compatibility suite passes against `httpx==0.28.1`;
- API oracle reports zero unexplained differences and zero stale active entries;
- every newly resolved difference is absent from the active allowlist and represented in the resolved ledger;
- every remaining active difference has a truthful rationale and explicit scope status;
- compatibility documentation does not claim HTTPX lacks a feature that it publicly exposes;
- short marketing wording and detailed qualification wording do not contradict each other;
- no new CI job, matrix, scheduled workflow, dashboard, or release automation is added.

## Rejection criteria

Reject an implementation if any of the following occurs:

- a public mismatch is hidden by editing only the oracle/allowlist instead of matching the behavior;
- signatures are cosmetically forged through `__signature__` while runtime argument acceptance remains incompatible, unless the plan explicitly permits that narrow technique for a non-callable descriptor;
- `uds`, `local_address`, or `socket_options` are still accepted and ignored/rejected while documentation claims support;
- SOCKS support bypasses the existing timeout, TLS, pool, cancellation, or proxy error paths;
- UDS or SOCKS connections are pooled under keys that can collide with ordinary TCP proxy/direct connections;
- credentials are included in errors or debug representations;
- arbitrary file/network behavior is added to the Python facade rather than the native engine;
- a broad socket/proxy abstraction is introduced without being required by the pinned parity target;
- new routine CI infrastructure is added to prove a feature that can be covered by the existing test path plus extended qualification;
- stale evidence from a previous executable SHA is presented as final proof.

## Stop conditions

Stop the affected phase and record a bounded blocker rather than expanding scope if:

- correct socket option support would require unsafe code contrary to the workspace `unsafe_code = "forbid"` policy and no safe focused dependency is acceptable;
- UDS requires a broad Hyper transport rewrite rather than a contained connector path;
- SOCKS requires replacing the existing proxy subsystem rather than adding a contained connector variant;
- exact HTTPX behavior depends on private httpcore state that is not observable through public behavior and cannot be reproduced without a second networking stack;
- matching a public signature would break an established non-compat EggFetch public API outside `eggfetch.compat.httpx`;
- a pinned-reference behavior conflicts with an existing security invariant and cannot be normalized safely.

A stop report must contain the exact missing primitive, minimal reproducer, affected acceptance criterion, proposed bounded retained behavior, and a separate follow-up proposal. Do not silently convert the blocker into an "intentional difference".

## Suggested commit decomposition

A reasonable implementation sequence is:

1. `docs: rebaseline remaining HTTPX parity contract`
2. `fix: align HTTPX object and exception contracts`
3. `fix: align HTTPX signatures and stream type surface`
4. `feat: implement advanced direct transport options`
5. `feat: add SOCKS proxy transport parity`
6. `test: complete remaining HTTPX differential coverage`
7. `docs: record final HTTPX parity completion evidence`

Adjacent commits may be combined when review remains clear. Do not collapse all six phases into one opaque change.

## Final handoff checklist

The implementer must report:

- starting SHA;
- final executable SHA;
- final documentation/evidence SHA if different;
- files changed by phase;
- active-difference count before and after;
- list of differences intentionally retained/deferred;
- `./scripts/check.sh` result;
- full pinned compatibility command/result;
- API oracle command/result;
- focused UDS/local-address/socket-options test results;
- focused SOCKS test results;
- CI run ID and checked-out SHA if CI ran;
- confirmation that Trio/AnyIO and Python 3.8/3.9 were not pulled into scope;
- confirmation that CI/release architecture was not expanded;
- final compatibility designation and exact wording used in user-facing documentation.
