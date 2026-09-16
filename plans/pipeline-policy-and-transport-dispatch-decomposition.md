# Pipeline Policy and Transport Dispatch Decomposition

Planning baseline: tree after Hyper-client/cache consolidation
Parent program: `plans/post-maintenance-core-integrity-and-verification-program.md`
Predecessor architecture work: `plans/core-request-and-transport-consolidation.md`

## Objective

Decompose the current large `pipeline.rs` by responsibility without changing the request-state model, transport precedence, timeout ownership, retry/redirect semantics, or public API.

The previous consolidation already solved the dangerous part: typed `RequestParts`, `PreparedRequest`, shared first-hop construction, explicit route selection, and common post-response policy. This plan must preserve those boundaries and move them into maintainable internal modules rather than redesigning them.

## Current maintenance problem

`pipeline.rs` currently owns, in one module:

- retry budget/backoff and response draining;
- redirect-loop request transformation;
- first-hop auth/cookie/default policy;
- single-request preparation and request-size/header/body normalization;
- logical pool acquisition;
- route compatibility checks;
- proxy selection/context assembly;
- H3 discovery, suppression, fallback, and metrics;
- H1/H2 transport dispatch;
- decompression/decoded-body/read-timeout/pool-lease finalization;
- native-body execution variants and route helpers.

The implementation is no longer mostly duplicate code, but the module has too many reasons to change. The goal is lower review blast radius.

## Required target shape

Exact module names are implementation choice, but converge on narrow private responsibilities equivalent to:

- `pipeline/retry.rs` — retry loop, retry delay, bounded discard drain;
- `pipeline/redirect.rs` — redirect state/hop transformation and first-hop request assembly;
- `pipeline/prepare.rs` — `PreparedRequest`, header/body/version/limits normalization, pool acquisition;
- `pipeline/route.rs` — route enum/precedence and compatibility checks;
- `pipeline/hyper_dispatch.rs` — direct/custom/UDS/SNI ordinary H1/H2 dispatch;
- `pipeline/proxy_dispatch.rs` — proxy route client selection and request-context construction;
- `pipeline/h3_dispatch.rs` — H3 discovery/fallback/suppression logic behind feature gate;
- `pipeline/finalize.rs` — common response decompression, limits, read timeout, pool lease;
- `pipeline/mod.rs` — short orchestration entry points.

Prefer fewer modules if some of these are naturally small. Do not create one-file-per-function fragmentation.

## Required work

### 1. Freeze behavioral invariants before moving code

Identify direct tests for:

- retry preserves request-local state and shrinks total deadline;
- redirect clears/preserves the correct hints/auth/cookies;
- no-redirect first hop matches redirect-enabled first hop;
- route precedence: UDS/custom/direct/proxy/SNI/H3/standard exactly as current source defines it;
- H3 never bypasses proxy/custom/UDS restrictions;
- proxy total deadline remains request-scoped;
- common read timeout/decompression/body limit/pool lease applies to all routes;
- pool permit survives until response body consumption/drop.

Add missing narrow tests before moving code.

### 2. Move retry policy intact

Move retry-specific functions and constants as a unit:

- `send_with_retry`;
- `sleep_if_budget_allows`;
- `has_budget`;
- `compute_retry_delay`;
- bounded response draining constants/helper.

Do not alter backoff, Retry-After, replayability, retryable status/error classification, or total deadline behavior in this plan.

### 3. Move redirect/hop construction intact

Keep `HopBuildParams` and `build_hop_request` together with redirect-loop state. Preserve exhaustive request-state handling and same-origin resolved-target semantics.

Do not reintroduce independent no-redirect construction.

### 4. Isolate preparation from dispatch

Move `PreparedRequest` plus `prepare_single_request` and tightly coupled helpers into a preparation module.

Preparation owns:

- effective decompression/body limits;
- proxy resolution decision;
- resolved-target compatibility checks;
- logical pool-key construction/acquisition;
- write-timeout body wrapping;
- content length;
- user-agent/accept-encoding;
- H2 forbidden-header policy;
- request target/size validation;
- remaining total/deadline calculation.

It must not perform transport I/O.

### 5. Isolate route selection/compatibility

Keep route precedence in one small, testable function/type. Avoid scattering boolean route predicates across dispatch modules.

Route selection should consume a compact context or prepared request facts rather than the full `ClientInner` when practical.

### 6. Split proxy and H3 dispatch from ordinary Hyper dispatch

Proxy dispatch owns only proxy-specific preparation after the common prepared request exists:

- SOCKS/forward/CONNECT cached client acquisition;
- proxy auth conflict check;
- `ProxyRequestContext` construction;
- legacy multi-target fallback route selection.

H3 dispatch owns:

- Alt-Svc discovery lookup;
- explicit-vs-discovered H3 distinction;
- replay-safe pre-commit fallback;
- suppression generation/failure classification;
- H3-specific metrics.

Ordinary Hyper dispatch owns direct/custom/UDS/SNI/standard request building and send calls.

No module should duplicate common response finalization.

### 7. Extract common response finalization

Move post-transport policy applied to every successful route into one function that receives the response plus prepared policy:

- decompression wrapper;
- decoded-body size/ratio limits;
- read-timeout stream;
- pool-lease attachment;
- any common metadata finalization.

Transport-specific response metadata must already be present before this step.

### 8. Keep native-body execution semantics explicit

If the native `execute_http_body` path shares route/preparation code, reuse only behavior that is actually common. It intentionally bypasses redirects, retries, cookies, auth, decompression, and decoded-body policy. Do not accidentally broaden it by moving code under a shared helper.

### 9. Module visibility and error boundaries

Keep new modules crate-private. Avoid `pub(crate)` proliferation beyond what orchestration needs.

Do not introduce a new error taxonomy. Moving code must preserve existing error variants/phases and detailed failure-context behavior.

### 10. Quantitative maintainability target

The success criterion is not an arbitrary LOC threshold, but the top-level `pipeline/mod.rs` should become visibly orchestration-oriented. No resulting child module should simply become another 100+ KB catch-all.

If one proposed split merely moves a large block without clarifying ownership, revise the boundary rather than mechanically extracting files.

## Validation

After each movement step run targeted unit/integration tests. At closure:

```sh
./scripts/check.sh
```

Also run focused tests for retry, redirect, proxy, custom dialer, resolved routing, UDS, SNI, HTTP/2, H3 (when feature-enabled), streaming/body limits, and native body execution.

Run the directly affected HTTPX/HTTPX2 compatibility subsets. Final exact-SHA requalification remains deferred.

## Non-goals

- no retry/redirect redesign;
- no new public transport trait;
- no Tower middleware architecture;
- no new route family;
- no H3 feature/graduation work;
- no compatibility expansion;
- no performance rewrite;
- no new dependency solely for module decomposition.

## Exit criteria

- [ ] Top-level pipeline entry points read as orchestration, not protocol implementation.
- [ ] Retry, redirect, preparation, route selection, proxy dispatch, H3 dispatch, and finalization have explicit private owners.
- [ ] Existing typed request/prepared-request invariants remain exhaustive.
- [ ] Route precedence and timeout ownership are unchanged and directly tested.
- [ ] Native-body semantics remain intentionally narrower than high-level requests.
- [ ] H3 suppression/fallback behavior remains green under feature tests.
- [ ] No public API or dependency change is required.
- [ ] Tier 1 and focused compatibility tests pass.
