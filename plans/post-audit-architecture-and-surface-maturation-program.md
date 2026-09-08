# Post-Audit Architecture and Surface Maturation Program

Planning baseline: `48f4457069d77528bd21fb342ff6ee4f27e3ae4f` (`main`, 2026-09-04)
Audit date: 2026-09-08
Reference compatibility contract: `httpx==0.28.1`, Python 3.10+, asyncio-supported public surface
Live compatibility ledger: `plans/httpx-parity-correction-status.md`

## Objective

Move eggfetch from broad feature acquisition into architectural consolidation and surface maturation. The current engine already implements the major HTTP-client capability set; this program focuses on reducing duplicated lifecycle logic, hardening the experimental HTTP/3 path, deciding and completing the Node surface, filling narrow protocol/observability gaps, and then requalifying one exact executable SHA against HTTPX 0.28.1.

This is not a new parity-expansion phase. Do not add speculative HTTPX APIs, new transport families, new language bindings, or new CI/evidence frameworks as part of this program.

## Why this program exists

The audit found five high-value concerns:

1. Request reconstruction and transport dispatch repeat policy/lifecycle plumbing across retry, redirect, direct, specialized-direct, UDS, SOCKS/proxy, SNI and HTTP/3 paths.
2. HTTP/3 is functional but still has experimental lifecycle characteristics: an unbounded sender cache, first-address-only DNS selection, hard-coded stream/idle settings and incomplete integration with native timeout/limit policy.
3. The Node crate is a prototype despite being described in some roadmap language as a completed additional binding. It currently wraps the blocking C ABI inside `spawn_blocking`, has a narrow string-body API, no real TypeScript declaration surface, and no normal validation-tier coverage.
4. Trailers and connector-level transport observability remain explicit gaps, while native pool naming conflates logical request concurrency with physical connection counts.
5. The HTTPX Stage C evidence is bound to `d24101be6ed7be64463813750da5b4043d9905ec`, while current `main` contains later executable changes. Exact-SHA qualification must therefore be renewed only after this executable line of work is complete.

## Architectural invariants

The implementation must preserve all existing repository invariants:

- `eggfetch-core` remains the sole owner of HTTP/network behavior.
- Rust remains async-first.
- Python sync continues to block on the Rust async engine while releasing the GIL.
- The CLI, FFI and Node layers do not grow independent HTTP implementations.
- HTTPX compatibility remains a bounded facade over the native engine, not a second transport stack.
- Unsafe code remains limited to the FFI/Node boundaries already allowed by repository policy.
- Existing intentional HTTPX differences remain explicit; refactoring must not silently erase or broaden them.
- No compatibility claim is renewed by inference from old evidence.

## Ordered implementation plans

Execute the following plans in order unless a plan explicitly states it may overlap:

1. `plans/core-request-and-transport-consolidation.md`
   - centralize request state preservation/transformation;
   - reduce duplicate Hyper/UDS response conversion and transport dispatch plumbing;
   - establish internal prepared-request / transport-response boundaries without creating a public abstraction prematurely.

2. `plans/http3-lifecycle-and-policy-hardening.md`
   - bound H3 cache growth;
   - improve multi-address connection behavior;
   - integrate connect/idle/concurrency policy;
   - add cancellation, reconnect and resource-lifecycle evidence.

3. `plans/node-binding-maturation.md`
   - make an explicit product decision: supported binding or clearly experimental prototype;
   - if supported, remove the blocking-C-ABI request path from the normal Node request surface and expose streaming/cancellation/structured errors/TypeScript declarations;
   - add Node validation through existing local validation conventions without inventing a new release framework.

4. `plans/native-protocol-observability-and-api-cleanup.md`
   - add HTTP trailer support at the core response boundary;
   - improve connection metadata/transport metrics where the connector architecture can expose them truthfully;
   - clarify logical concurrency naming in the native API before stabilization, while preserving compatibility-facade terminology.

5. `plans/post-maturation-httpx-requalification-and-closure.md`
   - freeze one final executable/test SHA;
   - run the existing Tier 1, Tier 2, Tier 3 and exact-SHA HTTPX qualification procedure;
   - renew Stage C only if every existing gate passes on that SHA.

6. `plans/post-maturation-documentation-and-plan-hygiene.md`
   - documentation-only descendant after requalification;
   - reconcile current architecture, Node/H3 maturity labels, native limits terminology and compatibility status;
   - archive/index historical plans without changing normative verification semantics.

## Sequencing rules

Plans 1–4 are executable work and therefore intentionally invalidate the old HTTPX exact-SHA evidence. Do not repeatedly requalify after each plan. Routine Tier 1 validation must remain green throughout, and targeted compatibility tests must accompany any change that touches compatibility-sensitive behavior.

Plan 5 is the only plan allowed to renew the HTTPX Stage C exact-SHA claim for this program. Any executable/test/build/validation/packaging change after its freeze invalidates that evidence and requires a new freeze.

Plan 6 must remain documentation/ledger/plan-hygiene only. If it discovers a required executable correction, reopen Plan 5 after the fix rather than editing executable code under a docs-only descendant.

## Cross-program acceptance criteria

The program is complete only when all of the following are true:

- [ ] Retry, redirect and no-redirect request reconstruction use explicit shared state-preservation/transformation operations rather than independent manual field-copy lists.
- [ ] Direct standard, specialized-direct and UDS Hyper response conversion share common lifecycle helpers where semantics are identical.
- [ ] Transport-specific modules own transport-specific I/O; generic request policy remains centralized.
- [ ] HTTP/3 per-origin state is bounded and recoverable after connection failure; client limits/timeouts govern it consistently where semantically applicable.
- [ ] HTTP/3 does not rely on a permanently first-address-only strategy without explicit fallback behavior.
- [ ] Node is either a deliberately documented prototype or a supported binding with native async dispatch, byte/stream body support, response streaming, cancellation, structured errors, declarations and validation.
- [ ] HTTP trailers are either implemented and tested or retained as an explicit, narrowly justified limitation with a deliberate deferral decision. The default target of this program is implementation.
- [ ] Transport observability reports only information the engine can actually observe; no synthetic socket/H2/H3 metadata is invented.
- [ ] Native concurrency APIs no longer imply physical-connection control where the implementation only limits logical in-flight requests, or a compatibility-preserving alias/deprecation path is documented.
- [ ] `./scripts/check.sh` remains green after each executable plan.
- [ ] Final exact-SHA requalification passes all existing qualification gates and updates the compatibility ledger/profile truthfully.
- [ ] Documentation and roadmap describe the final implementation rather than historical milestone assumptions.

## Non-goals

Do not expand this program into:

- Trio/AnyIO support;
- a new HTTPX reference version;
- browser-grade cookie policy;
- OAuth/OIDC or broad new authentication frameworks;
- outgoing content compression unless required by a concrete regression found here;
- a new public transport trait solely for abstraction aesthetics;
- new GitHub Actions matrices or release automation;
- new language bindings beyond Node;
- performance micro-optimization that is unrelated to duplication/resource-lifecycle fixes.

## Validation policy

Use the existing repository validation tiers and focused tests. `./scripts/check.sh` is mandatory for every executable commit. Use `./scripts/check.sh extended` at major plan closure points when practical, but the authoritative full exact-SHA qualification occurs in Plan 5.

If a refactor changes compatibility-sensitive code, run the directly affected HTTPX compatibility tests immediately; do not defer discovering semantic drift until final qualification.
