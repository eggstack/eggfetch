# Core Request and Transport Consolidation

Planning baseline: `48f4457069d77528bd21fb342ff6ee4f27e3ae4f` (`main`, 2026-09-04)
Parent program: `plans/post-audit-architecture-and-surface-maturation-program.md`

## Objective

Reduce semantic duplication in `eggfetch-core` without changing the documented native or HTTPX-compatible behavior. Centralize request reconstruction/state transformation and common response conversion so transport modules own transport-specific connection/protocol work rather than repeated generic lifecycle policy.

This is a correctness/maintainability refactor, not a public transport-framework redesign.

## Current problem

The current pipeline correctly centralizes many policies in `pipeline.rs`, but request state is still manually reconstructed in several places:

- retry reconstruction uses `rebuild_request(...)` with a long field list;
- the redirects-disabled fast path manually rebuilds a `Request` and reapplies auth/cookies/transport hints/decompression/proxy state;
- redirect hops construct another request and manually choose which state survives;
- transport dispatch then repeatedly converts the normalized request into `http::Request` values for UDS, specialized direct, SNI direct, H3 and standard Hyper paths.

The source already contains corrective comments documenting fields that were once silently dropped. That is evidence that this duplication is semantically risky.

Separately, `transport/direct.rs` contains two nearly identical Hyper response-conversion paths (`send_request` and `send_direct_request`), and UDS repeats much of the same trace/header/body conversion pattern.

## Required design direction

Introduce internal, typed boundaries for request policy and transport execution. The exact names may change during implementation, but the architecture should converge on concepts equivalent to:

- `LogicalRequestState` / enhanced `RequestParts`: complete state needed by retry/redirect policy;
- explicit transformations for retry and redirect rather than ad-hoc reconstruction;
- `PreparedRequest`: one request after defaults/auth/cookies/proxy/timeout/header normalization and content-length/version decisions have been applied;
- common Hyper response conversion helper(s) that can accept different concrete Hyper client types or a closure/future without duplicating response lifecycle code;
- transport dispatch that selects an execution path while preserving one common post-transport response policy.

Do not make these public unless a concrete downstream API requirement exists.

# 1. Establish behavioral invariants before refactoring

Add or identify direct regression tests that pin preservation/clearing rules for every request-state field currently carried by `RequestParts`:

- method;
- URL;
- headers;
- body and replayability;
- HTTP version;
- timeout;
- redirect policy;
- auth plus auth-disabled state;
- decompression override;
- proxy override;
- retry override;
- transport hints (`target`, `sni_hostname`, trace observer and any current extension-backed hints).

Required transformation semantics:

### Retry

A retry of the same logical request preserves all request-local configuration that is valid for another attempt, including transport hints, proxy override, decompression override and auth-disable state. The total deadline shrinks rather than restarting. Non-replayable stream bodies remain non-retryable.

### Redirect

Redirects preserve client/request policy that logically survives the redirect, but destination-specific hints are cleared after the first hop. Existing cross-origin auth/cookie stripping and method/body rewrite semantics must remain unchanged.

### No-redirect fast path

Disabling redirects must not produce a semantically different request construction path from the first hop of the redirect-enabled path except for redirect-loop behavior itself.

Acceptance:

- [ ] Every request-state field has a direct preservation/transformation assertion.
- [ ] Tests fail if a new field is added to `RequestParts` but omitted from the transformation path, where practical using constructor/struct exhaustiveness rather than wildcard destructuring.
- [ ] Existing HTTPX compatibility tests for target/SNI/trace/proxy/auth/cookies/retry remain green.

# 2. Replace manual retry reconstruction

Replace the long `rebuild_request(...)` field-copy function with a typed operation on request state.

Preferred characteristics:

- consume or borrow one complete saved request-state object;
- replay the body through one explicit replay operation;
- apply remaining total deadline through a dedicated timeout transformation;
- do not accept a dozen unrelated parameters;
- make non-replayable state return the existing error classification rather than panicking or silently dropping fields.

Do not change retry policy semantics in this pass.

Acceptance:

- [ ] `rebuild_request(...)` is removed or reduced to a thin wrapper over one typed transformation.
- [ ] Adding a new request-state field does not require editing an unrelated long parameter list.
- [ ] Retry error/attempt accounting and `Retry-After` behavior remain unchanged.

# 3. Unify first-hop and redirect-hop request construction

Extract common request assembly for:

- merged/default headers;
- cookies;
- auth resolution;
- decompression override;
- proxy override;
- timeout propagation;
- transport hints.

The redirects-disabled path should call the same first-hop builder as the redirect-enabled path. Redirect-specific state transitions should occur in one transformation step between hops.

Avoid overgeneralizing cookie/auth behavior into transport modules; they remain request-policy concerns.

Acceptance:

- [ ] No-redirect and redirect first-hop construction share one code path for common request state.
- [ ] Cross-origin stripping remains tested and centralized.
- [ ] Transport hints are cleared only where existing destination-change semantics require it.
- [ ] One-shot body redirect rejection remains before unsafe replay.

# 4. Introduce an internal prepared-request boundary

After request policy and body/header normalization, represent transport-ready state in one internal structure rather than rebuilding the same `http::Request` scaffolding in each branch.

The prepared form should carry at minimum:

- method;
- logical URL;
- resolved request URI/target;
- validated headers;
- body;
- selected HTTP version/policy information required by transports;
- transport hints;
- effective proxy/route selection where relevant;
- remaining total/connect/write/read budgets or a shared deadline representation where already established.

The prepared representation must preserve ownership semantics for one-shot request streams.

Acceptance:

- [ ] `send_single_request` has a clearly separated preparation phase and execution phase.
- [ ] Request-size/content-length/version/header policy is not reimplemented independently in transport modules.
- [ ] No body is cloned merely to satisfy the abstraction.

# 5. Consolidate direct Hyper response conversion

Refactor `transport/direct.rs` so the standard Hyper client and specialized direct client share one response-lifecycle implementation.

Common behavior includes:

- trace start/complete/failure events;
- request dispatch error mapping;
- response status/version/header capture;
- 101 upgrade detection;
- Hyper incoming body conversion;
- upgraded-stream attachment;
- response construction.

Use a generic helper, future-returning closure or narrowly scoped macro only if it makes the code clearer. Avoid exposing Hyper client generics throughout the rest of the core.

Acceptance:

- [ ] Standard direct and specialized-direct paths no longer contain near-copy response-conversion functions.
- [ ] 101 upgrade behavior is unchanged and still preserves leading data.
- [ ] Trace callback abort/failure behavior remains identical.
- [ ] Existing direct/SNI/socket-option/local-address tests remain green.

# 6. Reuse common Hyper response handling for UDS where semantics match

UDS should continue to own Unix socket connection establishment and TLS-over-UDS behavior, but common Hyper response conversion should be shared with direct paths wherever semantics are identical.

Do not force 101 upgrade support into UDS unless the current connector can expose it safely and tests justify it; if UDS intentionally differs, keep that difference explicit rather than hiding it behind a generic helper.

Acceptance:

- [ ] UDS no longer repeats generic trace/header/body conversion unnecessarily.
- [ ] Any remaining UDS-specific response behavior is documented in code and directly tested.
- [ ] H2-only UDS ALPN/`negotiated_h2()` behavior remains intact.

# 7. Simplify `send_single_request` dispatch

After preparation, make transport selection visibly declarative. The intended routing precedence must remain:

- configured UDS;
- specialized direct connector when applicable;
- effective proxy/SOCKS path;
- SNI override direct path;
- H3 where selected;
- standard Hyper direct path.

Refactoring must not reorder proxy/SNI/H3 precedence or accidentally make H3 bypass proxy rules.

Common post-response operations such as read-timeout wrapping and pool-lease attachment should occur once where possible. If a transport legitimately has different lease timing, preserve that explicitly.

Acceptance:

- [ ] `send_single_request` is materially shorter and separates policy preparation from transport selection.
- [ ] Route precedence has direct tests.
- [ ] Read timeout and pool lease semantics remain consistent across supported streaming transports.

# 8. Regression and compatibility validation

Required focused tests:

- retry preserves every request-local override;
- redirects clear only destination-specific hints and sensitive credentials per policy;
- no-redirect first hop matches redirect-enabled first-hop wire behavior;
- standard direct and specialized direct produce equivalent response metadata/body behavior for common requests;
- 101 upgrade behavior remains intact;
- UDS H1/H2 behavior remains intact;
- SOCKS/proxy/SNI/H3 route selection is unchanged.

Run:

```sh
./scripts/check.sh
```

Also run directly affected HTTPX compatibility files for extensions/trace, redirects, auth/cookies, proxy, timeout and network-stream behavior.

At closure, run `./scripts/check.sh extended` if the environment supports the existing prerequisites. Do not renew the Stage C exact-SHA claim here; this plan is intentionally followed by further executable work.

## Non-goals

- no new public `Transport` trait;
- no Hyper fork;
- no new proxy or protocol family;
- no retry/redirect behavior redesign;
- no HTTPX parity expansion;
- no new CI workflow;
- no performance rewrite unrelated to duplication removal.

## Exit criteria

- [ ] Request reconstruction is typed and centralized.
- [ ] First-hop/no-redirect construction shares one policy path.
- [ ] Direct Hyper response lifecycle is shared.
- [ ] UDS reuses shared behavior where valid.
- [ ] Transport dispatch is simpler without changing precedence or semantics.
- [ ] Tier 1 and focused compatibility tests pass.
- [ ] No compatibility qualification ledger is advanced yet.
