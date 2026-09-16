# Reusable Route-Cache Invariants and Hardening

Planning baseline: post-corrective tree after the proxy-TLS identity and Hyper idle-pool plans
Parent program: `plans/post-maintenance-core-integrity-and-verification-program.md`
Predecessor corrective: `plans/proxy-cached-total-deadline-corrective-pass.md`

## Objective

Turn reusable-client/cache compatibility from an implicit collection of key fields into an explicit, tested architectural contract.

The immediate proxy modernization has now produced two classes of regression at the same boundary: request-scoped total deadline captured by a reusable connector, and incomplete TLS policy identity permitting false-compatible cache reuse. This plan prevents equivalent failures from reappearing in SNI, custom-SNI, SOCKS, forward-proxy, CONNECT, resolved-target, or future route clients.

This is invariant/test hardening, not a new generic cache framework.

## Core invariants

For every reusable client or connector cache:

1. **Connection-scoped state may be cached.** Examples: logical proxy endpoint, proxy auth/headers used during connection/tunnel establishment, pinned physical peer, origin/proxy TLS policy, SNI, HTTP protocol policy, connect-phase timeout policy when it changes connector behavior.
2. **Every connection-affecting distinction must participate in compatibility identity.** False hits are correctness/security defects.
3. **Request-scoped state must never be retained by a reusable connector/client.** Examples: shrinking total deadline, read/write request budgets, retry attempt, redirect state, body, cookies, request auth headers, decompression, trace observer, failure context.
4. **Request-scoped state must not fragment cache identity merely to avoid ownership bugs.** Different `Timeout.total` values for compatible routes should reuse the same configured client while each request retains its own deadline.
5. **Safe false misses are acceptable.** If exact semantic equality is expensive or secret-bearing, use opaque configuration identity rather than weakening isolation.

## Required work

### 1. Build a route/cache inventory

Document in code comments/tests or closure notes each reusable/isolated client family:

| Family | Lifetime | Key/identity | Physical connection reuse owner |
|---|---|---|---|
| standard Hyper client | client lifetime | immutable client config | Hyper |
| direct advanced client | client lifetime | immutable client config | Hyper |
| UDS client | client lifetime | client UDS/TLS config | Hyper |
| custom-dialer client | client lifetime | dialer/client config | Hyper |
| SNI client | bounded route cache | SNI + immutable client policy | Hyper |
| custom-SNI client | bounded route cache | SNI + dialer/client policy | Hyper |
| resolved-target client | request scoped / isolated | no retained route cache | Hyper within request-owned client only |
| SOCKS client | bounded route cache | `SocksRouteKey` | Hyper |
| HTTP forward proxy | bounded route cache | `ForwardRouteKey` | Hyper |
| HTTPS CONNECT | bounded route cache | `ConnectRouteKey` | Hyper |
| H3 | separate QUIC cache | H3 origin/Alt-Svc policy | H3 connector |

Correct the table if implementation differs at execution time.

### 2. Add a connection-vs-request policy matrix

For each policy knob currently carried through request/client configuration, explicitly classify whether it may affect reusable physical connection compatibility.

At minimum include:

- proxy URI/scheme;
- proxy auth;
- proxy-only headers;
- proxy TLS config;
- proxy pinned addresses;
- proxied target address;
- origin TLS config;
- SNI override;
- local address/socket options;
- custom dialer identity;
- HTTP version/ALPN policy;
- connect timeout and proxy TLS/connect phase policy;
- total/read/write/pool request timeout;
- retry policy;
- redirect policy;
- request body/replayability;
- trace/failure context;
- decoded-body/decompression policy.

The classification should live close enough to route-key code that future contributors can update it when adding a field.

### 3. Add direct equality/isolation tests for route keys

For each reusable route-key type, build table-driven tests proving:

- same connection policy => equal key;
- each connection-affecting mutation => non-equal key;
- request-only mutation => key remains equal or is not represented in key construction;
- secret-bearing values never require a `Debug`/`Display` implementation on the key.

At minimum cover `SocksRouteKey`, `ForwardRouteKey`, and `ConnectRouteKey`.

If SNI caches continue using simple string keys, assert/document why the remaining relevant policy is immutable at `ClientInner` scope.

### 4. Add permanent deadline ownership regressions

Retain and extend the recently added cached-total regressions so the invariant is route-generic:

- short-total request followed by long-total request on same cached route with forced reconnect;
- long-total followed by short-total with forced reconnect;
- differing totals do not produce distinct route keys;
- connector structs contain no logical total deadline/remaining-total field.

Prefer behavior tests over source-shape tests, but a small source/unit invariant is acceptable where deterministic delayed establishment is impractical.

### 5. Add request-context non-retention tests

Where feasible, alternate requests on the same reusable route with distinct:

- trace observers;
- failure contexts;
- read/write budgets;
- decompression/body limits;
- request auth/cookies.

Prove the current request receives its own policy and no previous request's observer/context is invoked.

Do not force policy through cache keys merely to make the test pass; correct ownership instead.

### 6. Audit lock scope and cache construction

Current route caches use async mutexes and construct clients while holding the cache lock. Confirm that construction is CPU/local configuration only and does not await network I/O. If a future construction path performs I/O, restructure to avoid holding the global route-cache lock across I/O.

Also verify bounded eviction remains safe under concurrent clones. Arbitrary eviction is acceptable for this pass if correctness is preserved; do not introduce LRU without measured need.

### 7. Define a reusable regression checklist for future route additions

Add a concise crate-private comment or contributor-facing architecture note requiring any new reusable route/client cache to answer:

- What is connection identity?
- What request state is explicitly excluded?
- How is cache growth bounded?
- How are secrets kept out of diagnostics?
- Who owns physical pooling?
- How are current-request deadlines enforced on reconnect?
- Which regression proves false-hit isolation?

## Validation

Run focused route-key/proxy/SOCKS/SNI tests and then:

```sh
./scripts/check.sh
```

No exact-SHA compatibility renewal occurs in this plan.

## Non-goals

- no universal public cache-key trait;
- no custom socket pool;
- no mandatory LRU;
- no new metrics solely to inspect Hyper internals;
- no H3 redesign;
- no request-policy semantic change.

## Exit criteria

- [ ] Every reusable H1/H2 client family has an explicit connection-identity ownership statement.
- [ ] `SocksRouteKey`, `ForwardRouteKey`, and `ConnectRouteKey` have table-driven compatibility tests.
- [ ] Current request total/read/write state cannot be captured by reusable connectors.
- [ ] Connection-affecting TLS/proxy/routing state cannot be omitted from compatibility identity without a documented immutable-owner reason.
- [ ] Secret-bearing route keys remain non-renderable.
- [ ] Cache construction holds no async lock across network I/O.
- [ ] Tier 1 remains green.
