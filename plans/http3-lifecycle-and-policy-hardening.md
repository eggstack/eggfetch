# HTTP/3 Lifecycle and Policy Hardening

Planning baseline: `48f4457069d77528bd21fb342ff6ee4f27e3ae4f` (`main`, 2026-09-04)
Parent program: `plans/post-audit-architecture-and-surface-maturation-program.md`
Depends on: `plans/core-request-and-transport-consolidation.md`

## Objective

Bring the experimental HTTP/3 transport closer to the lifecycle, timeout, resource-bound and configuration semantics already expected from the H1/H2 paths, while retaining the `http3` feature gate and experimental product label until final validation demonstrates otherwise.

This plan is not a QUIC feature expansion. Do not add 0-RTT, WebTransport, QUIC datagrams or speculative H3 extensions.

## Current risks

The current `H3Connector` is functional and reuses one H3 connection/sender per origin, with `OnceCell` preventing same-origin connection races. However:

- `sender_cache` is unbounded, unlike the deliberately bounded SNI/SOCKS client caches;
- failed/dead cached connections need an explicit eviction/reconnect story;
- DNS resolution uses only the first returned socket address;
- connection establishment does not receive the native connect-phase timeout explicitly;
- QUIC idle timeout is hard-coded to 30 seconds rather than derived from client limits/keepalive configuration;
- maximum H3 stream settings are hard-coded rather than related to documented client policy;
- resource/driver-task lifecycle and cancellation behavior need direct stress evidence.

## 1. Define H3 lifecycle semantics

Before implementation, document in code/tests the intended state machine for a cached origin:

`Vacant -> Connecting -> Ready -> Failed/Closed -> Evicted -> Reconnectable`

Required properties:

- only one connection attempt per origin is in progress at a time;
- concurrent waiters share that attempt;
- a failed initialization does not permanently poison the origin;
- a connection that becomes unusable can be removed and recreated;
- client drop releases cache ownership and background driver tasks;
- bounded cache eviction never makes an in-flight response body invalid.

Acceptance:

- [ ] Unit/integration tests pin initialization failure, reconnect and concurrent-request behavior.
- [ ] No cache cell can permanently trap an origin in a failed state.

## 2. Bound the H3 origin cache

Add a bounded policy for H3 cached origin state. Reuse the spirit of SNI/SOCKS cache bounds but choose a value and eviction strategy appropriate to H3.

Requirements:

- no new LRU dependency unless measurements show simple bounded eviction is insufficient;
- eviction removes only cache ownership, not an in-flight stream's live connection ownership;
- background driver handles terminate when no longer needed;
- boundedness is directly testable by exposing crate-private/test-util cache metrics or deterministic test hooks rather than sleeping/guessing.

Acceptance:

- [ ] Sequential requests to more origins than the configured/internal bound do not grow cache entries without limit.
- [ ] Evicted origins reconnect normally when used again.
- [ ] In-flight requests survive unrelated cache eviction.

## 3. Implement multi-address connection fallback

Replace `lookup_host(...).next()` as the sole strategy.

Minimum acceptable behavior:

- resolve the complete address set returned by Tokio;
- attempt more than one address when earlier candidates fail;
- preserve an overall connect deadline/budget across attempts;
- report a useful final error without leaking sensitive connection detail.

A Happy Eyeballs style race is desirable only if it can be implemented narrowly and deterministically without adding substantial dependency/complexity. Sequential bounded fallback is acceptable for this pass if tested and deadline-aware.

Acceptance:

- [ ] A first unreachable address followed by a reachable address succeeds within the connect budget.
- [ ] All-address failure returns the correct connect/H3-connect taxonomy.
- [ ] Cancellation aborts remaining connection attempts promptly.

## 4. Integrate connect timeout and total deadline

Pass native timeout policy into H3 connection establishment through the prepared transport context introduced by the consolidation plan.

Semantics:

- `Timeout.connect` bounds DNS + QUIC connection establishment to the same degree the architecture can define consistently;
- `Timeout.total` remains the outer logical request deadline and must not restart per address or reconnect attempt;
- read timeout continues to apply to response-body progress at the documented response boundary;
- write timeout applies to streamed request-body progress where feasible in the H3 request stream;
- timeout errors identify the correct phase rather than collapsing everything into generic H3 protocol errors.

Do not silently alter HTTPX facade timeout mapping. HTTPX phases remain connect/read/write/pool only; native total is not synthesized.

Acceptance:

- [ ] Slow/unreachable H3 connect produces connect-phase timeout when connect is tighter than total.
- [ ] Total wins when total is tighter.
- [ ] Streaming request and response timeout behavior has focused H3 tests.

## 5. Derive idle behavior from client configuration

Remove the unconditional 30-second QUIC idle timeout as the only policy source.

Map existing native limits/keepalive policy to H3 where semantics are meaningful. If Quinn requires a protocol transport timeout that cannot exactly equal Hyper's pool idle policy, document the mapping explicitly.

Requirements:

- no idle lifetime longer than intended because the H3 path ignored client configuration;
- no misuse of `Timeout.pool` as an idle-connection lifetime;
- defaults remain conservative and compatible with current behavior where no explicit keepalive expiry is configured.

Acceptance:

- [ ] Configured keepalive/idle policy affects H3 deterministically.
- [ ] Default behavior is documented.
- [ ] Pool acquisition timeouts remain separate from connection idle lifetime.

## 6. Reconcile logical concurrency and H3 stream limits

The native `Pool` limits logical in-flight requests, while Quinn/H3 also have stream limits. Keep those concepts separate.

Do not claim `max_connections` controls QUIC physical connections if it does not. For this plan:

- ensure H3 honors the same logical pool permit model as other transports;
- configure H3/QUIC stream limits conservatively so they do not contradict configured logical concurrency;
- avoid hard-coded values where existing client policy can provide a sensible upper bound;
- leave public naming cleanup to `native-protocol-observability-and-api-cleanup.md`.

Acceptance:

- [ ] H3 requests still acquire/release logical pool permits correctly.
- [ ] Concurrent H3 streams do not exceed an explicitly configured logical per-origin request limit.
- [ ] Physical QUIC connection and stream settings are documented separately.

## 7. Harden connection failure and replay behavior

When a cached H3 sender fails because the connection is closed or unusable:

- classify whether the request can be retried through the existing retry policy;
- invalidate stale cached route state so a subsequent eligible attempt can reconnect;
- never automatically replay a non-replayable request body outside the existing retry policy;
- avoid hidden transport-level retries that bypass retry accounting, method policy or total deadline.

Acceptance:

- [ ] Stale connection state is evicted after terminal connection failure.
- [ ] Replayable idempotent requests can reconnect only through existing retry machinery where applicable.
- [ ] One-shot bodies are never duplicated.

## 8. Resource and cancellation stress tests

Add focused tests covering:

- many sequential origins beyond cache capacity;
- many concurrent requests to one origin during initial connect;
- cancellation while DNS is pending;
- cancellation while QUIC connect is pending;
- cancellation while sending a streaming request body;
- partial response consumption/drop;
- client drop with cached H3 origins;
- repeated connect-fail-reconnect cycles;
- no monotonically growing cache/driver-task count after stabilization.

Use deterministic local fixtures wherever possible. Do not depend on public internet H3 endpoints for required tests.

## 9. Validation

Run feature-specific checks during implementation:

```sh
cargo test -p eggfetch-core --all-features -- --test-threads=1
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,http3
./scripts/check.sh
```

Run Python H3-focused tests because the Python binding enables H3. If the extended environment is available, run `./scripts/check.sh extended` at plan closure.

Do not renew HTTPX exact-SHA qualification in this plan.

## Non-goals

- no 0-RTT;
- no Alt-Svc automatic upgrade policy unless already present and required for correctness;
- no WebTransport;
- no QUIC datagrams;
- no browser H3 emulation;
- no new public QUIC configuration API beyond what is needed to honor existing client policy;
- no removal of the experimental label solely because this plan passes.

## Exit criteria

- [ ] H3 cache growth is bounded.
- [ ] Dead/failed cached origins are reconnectable.
- [ ] Multiple resolved addresses receive fallback behavior.
- [ ] Connect and total deadlines are phase-correct.
- [ ] Configured idle policy governs H3 appropriately.
- [ ] Logical request limits and QUIC stream limits are explicitly separated.
- [ ] Cancellation/resource stress evidence is green.
- [ ] Tier 1 is green with all features.
