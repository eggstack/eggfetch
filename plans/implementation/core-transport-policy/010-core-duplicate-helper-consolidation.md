# Core Transport and Request Policy Milestone 010 — Core Duplicate-Helper Consolidation

Status: ready

Repository baseline: `873ea28541c82817317430c9523fca8525d99c81`

Source roadmap:

- `plans/subsystems/core-transport-policy-roadmap.md#milestone-10--core-duplicate-helper-consolidation`

Long-term requirements:

- `plans/000-long-term-specification.md` §4.1, §5
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 items 2 and 4

Applicable ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0002-pipeline-route-finalize.md`
- `plans/adrs/ADR-0004-proxy-fallback-total-deadline.md`
- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`
- `plans/adrs/ADR-0006-native-transport-failure-classification.md`

Primary class: infrastructure

## 1. Objective

Consolidate the confirmed clusters of duplicated helper logic in
`eggfetch-core` so that each rule is implemented once, and remove the
literal-duplicated error strings that let a rule change land in one copy and
not another.

Seven clusters are in scope. They are grouped here because each is an
internal consolidation with no user-visible behavior change, and because
fixing them together keeps the engine's helper inventory reviewable in a
single pass.

| # | Cluster | Evidence | Copies |
|---|---|---|---|
| A | resolved-target and SNI client builders | `client/routes.rs` `build_resolved_client` (~144-198) vs `sni_client` (~207-259); the pair differs in three respects only — the resolved-target binding, the SNI binding (optional versus mandatory) and the enabler computed twice versus once (the same value both times) | 2 |
| B | resolved-destination effective-port validation | six port-comparison sites — `pipeline/prepare.rs:348` (resolved) and `:363` (proxied), `pipeline/mod.rs:496` (native resolved), `request.rs:853` (resolved) and `:869` (proxied), `proxy.rs:664` (resolved proxy peer) — plus `pipeline/hyper_dispatch.rs:161`, which carries a third copy of the `"resolved destinations require an HTTP or HTTPS URL"` message and compares no port | 6 comparison sites over 7 call sites |
| C | custom-dialing incompatibility ladder | `pipeline/prepare.rs:456,461,466,475` (the high-level preparation ladder, preceded by two non-ladder messages at `:433` and `:439`) and `pipeline/mod.rs:512,517,522,531` (the native ladder, with no built-in-proxy arm) — **not** two identical ladders; `pipeline/mod.rs:260` is the high-level dispatch-boundary arm, which the native path does not have | 2 ladders of 4 arms plus 1 dispatch-boundary arm |
| D | origin normalization | `pool.rs` `OriginKey::from_url` / `from_url_with_proxy_scheme` vs `http_origin.rs` `HttpOrigin::from_url`; `HttpOrigin::from_uri` strips the IPv6 brackets (`::1`) while `HttpOrigin::from_url` and `OriginKey::from_url` keep them (`[::1]`) | 3 |
| E | post-handshake ALPN `h2` detection | `transport/connect.rs`, `transport/uds.rs`, `transport/direct_connector.rs`, `transport/socks.rs` | 4 |
| F | `HyperClientPolicy` constructor duplication | `transport/hyper_client.rs` `persistent` vs `cached_route` are byte-identical bodies | 2 |
| G | `"target extension is not valid UTF-8"` literal | `pipeline/prepare.rs` (2), `transport/connect.rs` (2) and `transport/proxy.rs` (4) — not `src/proxy.rs` — with `Error::InvalidUrl` and identical text at all eight sites | 8 |

## 2. Why this milestone is ready

- Each cluster was confirmed by reading the code, not by matching names. The
  copies currently agree; the risk is that a future change does not.
- Every cluster is internal. None requires a public API change, a new
  dependency, or a change to a documented contract.
- ADR-0002's phase ownership already assigns these concerns to private
  owners. Consolidation moves logic toward those owners rather than
  introducing a new layer.
- Cluster D has a live divergence, not just a duplication: the two origin
  normalizations do not apply the same validation. That makes it worth
  resolving rather than merely deduplicating.
- The lean-path duplication that motivated a separate milestone
  (core-transport-policy M009) is excluded here so that this milestone stays
  a pure consolidation pass.
- No decision in this milestone depends on the H3 graduation program, the
  HTTPX 1.0 program, or Node maturation.

## 3. Current implementation evidence

The figures in this section were corrected against the closure record
`plans/closure/core-transport-policy/010-core-duplicate-helper-consolidation.md`
on 2026-10-23, once the implementation had been measured against the tree.
Where this section and that record disagree, the record is the authority.

At the baseline:

- `crates/eggfetch-core/src/client/routes.rs` contains
  `build_resolved_client` and `sni_client`, whose bodies share the TLS
  materialization, Hyper client policy derivation, and lifecycle
  configuration steps. The two differ in exactly three respects: the
  resolved-target binding, the SNI binding (optional in one, mandatory in
  the other), and the enabler being computed twice versus once (the same
  value both times). The "neither" case — no resolved target and no SNI — is
  the *persistent* direct singleton built in `client.rs`, with a different
  Hyper policy source and different TLS-failure handling, and is not part
  of this cluster;
- six sites compare a resolved destination's port against the URL's
  effective port: `crates/eggfetch-core/src/pipeline/prepare.rs:348`
  (resolved) and `:363` (proxied),
  `crates/eggfetch-core/src/pipeline/mod.rs:496` (native resolved),
  `crates/eggfetch-core/src/request.rs:853` (resolved) and `:869` (proxied),
  and `crates/eggfetch-core/src/proxy.rs:664` (resolved proxy peer). The
  three resolved-destination sites emit the identical message
  `"all resolved destinations must use the URL's effective port {expected_port}"`,
  the two proxied sites emit the proxied variant, and the proxy-peer site
  emits the proxy-peer variant. The five destination sites raise
  `Error::InvalidResolvedTarget`; the proxy-peer site raises
  `Error::InvalidProxyUrl`;
- a seventh call site, `crates/eggfetch-core/src/pipeline/hyper_dispatch.rs:161`,
  carries a third copy of `"resolved destinations require an HTTP or HTTPS
  URL"` — the same error kind and the same text — without comparing a port;
- the custom-dialing incompatibility messages are not two identical ladders.
  The high-level preparation ladder is
  `crates/eggfetch-core/src/pipeline/prepare.rs:456,461,466,475`, preceded
  by two messages that are not ladder arms (`prepare.rs:433`,
  "caller-supplied resolved destinations require direct routing; disable the
  proxy explicitly", and `prepare.rs:439`, "caller-supplied resolved
  destinations are incompatible with Unix-domain routing"). The native
  ladder is `crates/eggfetch-core/src/pipeline/mod.rs:512,517,522,531`,
  preceded by the native proxy rejection at `mod.rs:504`, and it has **no**
  built-in-proxy arm. The built-in-proxy message
  `"custom dialing is incompatible with built-in proxy routing"` at
  `crates/eggfetch-core/src/pipeline/mod.rs:260` is the high-level
  dispatch-boundary arm, which the native path does not have. All five
  arms raise `Error::Unsupported`;
- `crates/eggfetch-core/src/pool.rs` `OriginKey::from_url` accepts any
  scheme string and does not validate the host, while
  `crates/eggfetch-core/src/http_origin.rs` `HttpOrigin::from_url` rejects
  a non-HTTP(S) scheme and an empty host; `OriginKey::from_url_with_proxy_scheme`
  rebuilds the struct directly rather than going through the shared
  constructor. The two constructors also disagree on the IPv6 host form:
  `HttpOrigin::from_uri` strips the brackets (`::1`), while
  `HttpOrigin::from_url` and `OriginKey::from_url` keep them (`[::1]`) — a
  residual divergence that this milestone records but does not fix, because
  fixing it would change pool keying;
- the post-handshake ALPN `h2` check is repeated in
  `crates/eggfetch-core/src/transport/connect.rs`,
  `transport/uds.rs`, `transport/direct_connector.rs` and
  `transport/socks.rs`;
- `crates/eggfetch-core/src/transport/hyper_client.rs` `persistent` and
  `cached_route` have identical bodies;
- the `"target extension is not valid UTF-8"` message appears eight times
  across `crates/eggfetch-core/src/pipeline/prepare.rs` (two),
  `crates/eggfetch-core/src/transport/connect.rs` (two) and
  `crates/eggfetch-core/src/transport/proxy.rs` (four). At all eight sites
  the error kind is `Error::InvalidUrl` and the text is identical.

## 4. Invariants that must not regress

- No public API changes in any profile. Every cluster is private.
- The origin identity used for pooling, SNI, certificate verification, Host,
  cookies, auth, redirects and Alt-Svc trust remains the logical origin.
  A route peer address never becomes origin identity.
- The effective-port validation keeps its current strictness: a resolved
  destination whose port differs from the URL's effective port is rejected,
  including the proxied variants.
- The custom-dialing incompatibility ladder keeps its current ordering and
  its current set of incompatible combinations.
- The `TransportFailureKind` domain established by ADR-0006 is unchanged;
  consolidation must not collapse distinct failure classes.
- Timeout ownership, pool admission, and body-lifecycle semantics are
  unchanged.
- The `ResponseBody` public variant shapes remain frozen.
- No new cargo dependency, and no new error variant.
- Redaction rules are unchanged.
- The `eggfetch-http-connect` boundary is unchanged: that crate owns generic
  CONNECT wire bytes only.

## 5. Scope

### In scope

- one shared implementation for the resolved-target and SNI client builders;
- one shared implementation of effective-port validation, including the
  proxied and proxy-peer variants;
- one shared implementation of the custom-dialing incompatibility ladder;
- one shared, consistently-validating origin normalization;
- one shared post-handshake ALPN `h2` check;
- removal of the byte-identical `HyperClientPolicy` constructor duplication;
- replacement of the eight duplicated UTF-8 error literals with one shared
  constant or one shared helper.

### Explicitly out of scope

- the lean-path preparation duplication, tracked by core-transport-policy
  M009;
- the redirect method-rewrite table's location, which is tracked by
  python-httpx-compat M005 where it crosses the facade boundary;
- the profile-matrix coverage gaps tracked by core-transport-policy M008;
- any change to `eggfetch-http-connect`;
- any change to public API, error taxonomy, or documented semantics;
- H3 behavior, the HTTPX 1.0 program, Node maturation;
- publication, version bump, or release orchestration.

## 6. Required production changes

### 6.1 Cluster A — one client builder

`build_resolved_client` and `sni_client` must share one implementation for
the steps they have in common: TLS materialization, the HTTP-version policy
enabler derivation, the Hyper client policy, and the lifecycle
configuration.

The two must still differ where they are semantically different: the
resolved-target builder binds a resolved address, the SNI builder binds a
logical SNI hostname. The difference must be expressed as a parameter or a
small enum, not as two bodies.

Design constraint: the shared builder must not become a function with a
boolean per difference. If the two call sites need more than a small
discriminating parameter, the shared part must be a helper that each call
site composes.

### 6.2 Cluster B — one effective-port validation

The resolved-destination, proxied-destination and proxy-peer effective-port
checks must share one implementation, with the message text expressed once.

The three currently differ in subject ("resolved destinations", "proxied
destinations", "resolved proxy peers") and in which port is compared. The
shared implementation must preserve those distinctions in the message while
keeping the comparison logic in one place.

The validation must keep failing closed. A resolved destination whose port
differs from the expected effective port is rejected before a connection is
attempted, and the rejection must remain an `Error` of the same kind as
today.

### 6.3 Cluster C — one incompatibility ladder

The custom-dialing incompatibility ladder must be implemented once and
called from both the preparation path and the dispatch path.

The five incompatibilities are: built-in proxy routing, caller-supplied
resolved destinations, local-address or socket-option routing, Unix-domain
routing, and HTTP/3. The shared implementation must preserve the current
precedence, so that a request violating more than one combination still
produces the same first error as today.

The `pipeline/mod.rs:260` built-in-proxy case must join the same ladder
rather than remain a separate literal.

### 6.4 Cluster D — one consistently-validating origin normalization

The three origin normalizations must agree on:

- the scheme handling: whether a non-HTTP(S) scheme is accepted;
- the empty-host handling;
- the host lowercase normalization;
- the effective-port derivation;
- the default-port mapping.

The implementer must first decide, and record, which validation set is
authoritative. Two acceptable outcomes:

- the stricter validation (`HttpOrigin::from_url`) becomes the single rule,
  and `OriginKey` adopts it; or
- the permissive validation is adopted, and the stricter rejection is
  removed.

The choice must be justified by tracing every current caller of both
normalizations and confirming that the chosen rule does not change pool
keying for any supported URL shape.

This is the one cluster with a live behavioral consequence: the two
normalizations currently disagree about which URLs are valid origins, and a
URL accepted by one and rejected by the other produces different pool keys
depending on which path built them. The closure evidence must record the
chosen rule and the caller trace.

Host lowercase normalization, IDNA handling and default-port derivation must
remain identical to today in the chosen rule.

### 6.5 Cluster E — one ALPN check

The post-handshake ALPN `h2` detection must be one shared helper used by
`connect.rs`, `uds.rs`, `direct_connector.rs` and `socks.rs`.

The helper must not change the detection rule, the failure mapping, or the
transport-specific context attached to a failure. The four call sites
differ in what they do with the result, and that difference must remain at
the call site.

### 6.6 Cluster F — one `HyperClientPolicy` constructor

`persistent` and `cached_route` must not remain byte-identical bodies. Either
one becomes the other by delegation, or they are collapsed into a single
constructor with a parameter that expresses the real difference.

The implementer must determine whether the two constructors genuinely have
different intent. If they do not, one must be removed and its callers
updated. If they do, the difference must be made explicit in the API shape
rather than in an identical body.

### 6.7 Cluster G — one UTF-8 error

The eight occurrences of `"target extension is not valid UTF-8"` must become
one shared constant or one shared helper that produces the error.

The error kind, the message text, and the behavior on a non-UTF-8 target
extension must all be unchanged. The shared helper must not begin parsing
the extension further than today.

## 7. Ordered work packages

1. Record the before state: the per-cluster copy counts, and the two origin
   normalization rule sets side by side.
2. Cluster G: introduce the shared constant or helper and replace the eight
   occurrences.
3. Cluster F: resolve the `HyperClientPolicy` constructor duplication.
4. Cluster E: introduce the shared ALPN check and update the four call
   sites.
5. Cluster B: introduce the shared effective-port validation and update the
   seven call sites.
6. Cluster C: introduce the shared incompatibility ladder and update the
   two ladders and the built-in-proxy case.
7. Cluster D: trace the callers of both origin normalizations, decide the
   authoritative rule, and implement it once.
8. Cluster A: introduce the shared client builder and update both call
   sites.
9. Verify the per-cluster copy counts are all one.
10. Run the full workspace test suite, including the profile matrix.
11. Run Tier 1 and Tier 2.
12. Renew the exact-SHA Stage C binding, because the engine is an
    executable input.
13. Write `plans/closure/core-transport-policy/010-core-duplicate-helper-consolidation.md`.

## 8. Failure, cancellation, timeout, and pool semantics

- Cluster B's validation must still run before any connection attempt, and
  must not consume request deadlines or pool permits.
- Cluster C's ladder must preserve the current precedence exactly, so that a
  request violating several combinations produces the same first error.
- Cluster D's chosen rule must not change pool keying for any supported URL
  shape, and must not change the origin used for SNI, certificate
  verification, Host, cookies, auth, redirects or Alt-Svc trust.
- Cluster E's shared helper must not change which transports report an
  negotiated HTTP/2 connection, nor the failure kind attached to it.
- Cluster A's shared builder must not change the resolved-target client's
  idle policy, pool binding, or the SNI client's logical-origin binding.
- No timeout, pool, cancellation, retry, redirect or body-lifecycle
  semantics change.

## 9. Compatibility and migration

- No public API change in any profile.
- No documented behavior change, with one exception: cluster D may change
  which URLs are accepted as origins. That change must be justified by the
  caller trace, must not change pool keying for any supported URL shape,
  and must be recorded in the closure evidence and in
  `docs/residual-differences.md` if it is observable.
- Error message text may change for the clusters that centralize a literal,
  but the error **kind** must not change, and the shared messages must be
  byte-identical to the current text so that no consumer matching on the
  text breaks.
- Feature-profile effects: none. Every consolidation must compile in every
  profile, and the per-profile public API oracle must be unchanged.
- The engine is an executable input, so the exact-SHA Stage C binding must
  be renewed under `.skills/verification-qualification.md`.
- No crate version bump is implied.

## 10. Required tests

### Per-cluster equivalence

- Cluster A: the resolved-target client and the SNI client are proven to
  bind the same TLS materialization, enabler, Hyper policy and lifecycle
  configuration as before;
- Cluster B: for each of the three subject variants, a mismatched-port
  destination is rejected with the same error kind and the same message as
  before, and a matching-port destination is accepted;
- Cluster C: for each of the five incompatibility combinations, the
  resulting error is identical to before, including the precedence when
  several apply;
- Cluster D: for a table of URL shapes — HTTP, HTTPS, non-HTTP(S) scheme,
  empty host, uppercase host, IPv4 literal, IPv6 literal, explicit default
  port, explicit non-default port, punycode host — both origin types are
  proven to produce an agreed verdict, and the pool key is proven stable
  for each accepted shape;
- Cluster E: each of the four transports is proven to detect a negotiated
  HTTP/2 connection exactly as before;
- Cluster F: the surviving `HyperClientPolicy` constructor is proven to
  produce the same policy as both predecessors;
- Cluster G: a non-UTF-8 target extension is rejected with the same error
  kind and message as before, in each of the affected call paths.

### Regression

- the full workspace test suite is green;
- the profile matrix, including the lean and native profiles, is green;
- the public API oracle is unchanged for every profile;
- Tier 1 and Tier 2 are green.

## 11. Required verification commands

Follow `docs/verification-policy.md` and
`.skills/verification-qualification.md`.

Focused first:

```bash
cargo test -p eggfetch-core --all-features -- --test-threads=1
cargo test -p eggfetch-core --test pool_tests -- --test-threads=1
cargo test -p eggfetch-core --test resolved_route_cache_tests -- --test-threads=1
cargo test -p eggfetch-core --test custom_dialer_tests -- --test-threads=1
cargo test -p eggfetch-core --test proxy_tests -- --test-threads=1
```

Then:

```bash
cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1
./scripts/check.sh
./scripts/check.sh extended
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

Never parallelize Rust workspace tests; `--test-threads=1` is required.

Do not create a new automatic workflow or a permanent CI job.

## 12. Documentation updates

Implementation must update:

- `docs/architecture/core-engine.md` (the private helper inventory and the
  phase owners);
- `docs/architecture/core-tls-proxy-protocols.md` (the ALPN detection and
  the client builder consolidation);
- `docs/architecture/core-timeout-pool.md` if the origin-normalization
  description changes;
- `docs/residual-differences.md` if cluster D produces an observable
  difference;
- `plans/registry.md`, the subsystem roadmap and `plans/README.md` at
  closure.

## 13. Acceptance criteria

M010 closes only when:

- each of the seven clusters has exactly one implementation of its rule;
- the effective-port validation rejects the same inputs with the same error
  kind and message as before — `Error::InvalidResolvedTarget` at the five
  destination sites, `Error::InvalidProxyUrl` at the proxy-peer site — and
  all seven call sites resolve to that one implementation, including the
  route-key site that compares no port;
- the custom-dialing ladder preserves its precedence and its five
  incompatibilities: the four preparation arms on the high-level path, the
  same four arms on the native path, and the built-in-proxy arm at the
  high-level dispatch boundary, which the native path does not have;
- the origin normalizations are consolidated to one consistently validating
  rule — the stricter `HttpOrigin` rule — and the chosen authoritative rule
  is recorded with its caller trace; the legacy permissive rule and the
  adopted strict rule differ on exactly one URL shape in the test table (a
  non-HTTP(S) scheme), and that difference is proven unreachable;
- pool keying is unchanged for every supported URL shape;
- no public API changed in any profile;
- every error **kind** is unchanged — cluster B keeps
  `Error::InvalidResolvedTarget` and `Error::InvalidProxyUrl`, cluster G
  keeps `Error::InvalidUrl` at all eight sites, and the ladder keeps
  `Error::Unsupported` — and the shared error messages are byte-identical to
  the current text;
- the full workspace test suite is green on the bound SHA, as delivered by
  the frozen-SHA qualification rather than by the milestone's own gates;
- Tier 1 and Tier 2 are green on the frozen executable SHA;
- both HTTPX profiles are rebound to that exact SHA.

## 14. Stop conditions

Stop and revise this plan rather than improvising if:

- cluster D's caller trace shows that neither validation set can be adopted
  without changing pool keying for a supported URL shape, in which case the
  divergence must be escalated rather than papered over;
- a shared helper would require a boolean parameter per difference, which
  the plan forbids;
- consolidating the client builders would require changing the resolved
  target's or the SNI client's binding semantics;
- the shared ALPN check would change which transports report a negotiated
  HTTP/2 connection;
- the shared effective-port validation would change when the rejection
  happens relative to connection attempt;
- any consolidation would change a public API, an error kind, or a
  documented message.

## 15. Closure evidence required

Create `plans/closure/core-transport-policy/010-core-duplicate-helper-consolidation.md` containing:

- implementation commit(s) and the final frozen SHA;
- the before/after per-cluster copy count table;
- the cluster D caller trace and the chosen authoritative rule;
- the origin-verdict agreement table for the URL-shape test matrix;
- the pool-key stability evidence;
- the per-cluster equivalence test output;
- the public API delta (expected: none) for every profile;
- the exact Tier 1 and Tier 2 outcomes;
- renewed HTTPX Stage C binding evidence for both profiles;
- known limitations, including the remaining lean-path work tracked by M009
  and the remaining redirect-policy work tracked by python-httpx-compat M005.

## 16. Handoff notes

This is a consolidation milestone. The expected diff is mostly deletions. A
large addition here usually means a helper was generalized too far.

Cluster D is the one with a behavioral consequence and needs a real
decision, recorded in the closure evidence. Do not pick the stricter or the
permissive rule by default; pick it from the caller trace.

The shared error messages must be byte-identical to the current text.
Centralizing a message is the point; changing it is a separate decision
that belongs in its own plan.

Do not widen this milestone to the lean-path preparation duplication. That is
M009, and it must land first so that the remaining duplication inventory is
accurate.
