# Post-Embedded-Engine Documentation and Plan Hygiene

Planning baseline: `445149bd2af3c3ce5c0d34daacbde1bf8de57297` (`main`, 2026-09-11; eggfetch 0.1.3)
Parent program: `plans/embedded-rust-client-footprint-and-routing-program.md`
Depends on: successful closure of `plans/post-embedded-engine-compatibility-requalification-and-closure.md`
Scope: documentation/profile/ledger/plan-index only after the frozen executable SHA

## Objective

Reconcile eggfetch documentation, plan navigation, architecture descriptions, examples, and qualification evidence with the final embedded-Rust engine implementation without modifying executable/test/build/validation/package behavior after compatibility requalification.

This is the final truth/hygiene pass for the parent program.

## Governing rule

> This plan must not change executable semantics after the frozen qualification SHA.

Allowed descendants include documentation, plan indexes, compatibility profile/ledger metadata, and static evidence files that do not affect build/test/qualification behavior. If a required executable correction is discovered, stop this plan, make the correction, and repeat the parent requalification plan with a new frozen SHA.

# 1. Audit documentation against the frozen implementation

Review all documents that describe:

- Cargo features/defaults;
- dependency ownership;
- TLS/root-store behavior;
- ordinary/default connector behavior;
- request transport hints/routing;
- retries and redirects;
- proxy interaction;
- HTTP/3 routing/maturity;
- native Rust JSON support;
- response/body limit behavior;
- connection pooling/reuse;
- performance/footprint claims;
- public native Rust examples.

At minimum inspect:

- `README.md`;
- `docs/architecture/feature-flags.md`;
- `docs/architecture/dependency-policy.md`;
- `docs/architecture/core-tls-proxy-protocols.md`;
- `docs/architecture/core-engine.md`;
- `docs/architecture/core-body-streaming.md` where body-limit behavior is described;
- native Rust API/quickstart/cookbook docs;
- `docs/architecture/benchmarks.md` if embedded qualification is referenced;
- `plans/ROADMAP.md`;
- `plans/README.md`.

Do not update unrelated historical plan text merely because terminology evolved; historical plans may remain accurate records of their prior baseline.

# 2. Document the final feature profiles precisely

Publish the supported Cargo recipes established by the implementation, including:

- ordinary/default eggfetch-core profile;
- minimal H1 cleartext profile if supported;
- H1 + Rustls deterministic/WebPKI profile;
- H1 + Rustls + native-root/default trust profile;
- H1 + Rustls + native JSON profile;
- H2/H3 feature relationships where relevant.

For each profile, state what is included/excluded rather than using ambiguous labels such as "lite" or "minimal" without a capability list.

If `--no-default-features` still has intentionally limited utility, document exactly what it means after refactoring.

Acceptance:

- [ ] examples compile against the frozen feature contract;
- [ ] native-root/WebPKI differences are explicit;
- [ ] optional capabilities are not described as free/default when they are feature-gated.

# 3. Reconcile TLS trust documentation

Document the single authoritative trust-policy model after the TLS refactor.

Required clarity:

- default native-root preference/fallback behavior;
- WebPKI-only deterministic behavior;
- native-only behavior;
- custom CA replacement/augmentation semantics as actually implemented;
- client certificate behavior;
- SNI/hostname verification;
- difference between native-root loading failure and certificate verification failure;
- proxy endpoint trust vs origin trust;
- H3 trust semantics where shared/different.

Ensure no document still describes a separate standard-Hyper fallback implementation if that code was removed.

# 4. Document static/resolved destination routing

Add native Rust documentation and at least one concise example showing:

```text
logical URL/Host/SNI identity
        !=
physical remote SocketAddr selection
```

Document guarantees and limitations:

- caller supplies/validates addresses;
- no DNS fallback in static mode;
- retries preserve the snapshot;
- redirect policy and same-origin/cross-origin behavior;
- strict pinned mode if implemented;
- proxy interaction;
- HTTP/3/Alt-Svc supported/rejected behavior;
- connection-pool route isolation;
- this is a transport primitive, not automatic SSRF validation.

Avoid security wording that implies eggfetch decides whether an address is safe. The caller owns address validation/policy.

# 5. Document native JSON support

Update native Rust docs for:

- `json` Cargo feature;
- request `.json(...)` behavior;
- Content-Type precedence;
- response `.json::<T>()` consumption;
- error classification;
- body/decompression limits;
- replayability on retry/redirect.

Keep Python HTTPX-style JSON documentation separate and do not imply the Python facade uses native Serde types at its public boundary.

# 6. Record embedded footprint evidence truthfully

Publish or link the final evidence produced by `embedded-consumer-footprint-qualification.md`.

At minimum state:

- frozen eggfetch SHA;
- comparison client/version;
- exact compared feature profiles;
- target/toolchain/release settings;
- stripped artifact sizes;
- key dependency-tree explanation;
- outcome category (`beneficial`, `neutral/strategic`, or `not a footprint win`).

Do not put a timeless marketing statement such as "eggfetch is X% smaller" in top-level docs unless the comparison context/date/profile is immediately visible. Prefer a dated qualification record.

# 7. Correct connection pooling/reuse language

Audit stale statements about TCP reuse.

The final documentation must distinguish:

- eggfetch's logical request-concurrency pool/semaphore limits;
- Hyper's physical connection reuse for standard/direct Hyper paths;
- any separate proxy transport connection behavior;
- HTTP/2 multiplexing;
- HTTP/3 QUIC connection caching;
- static-route connection/client-cache identity constraints.

Remove broad statements that every request opens a fresh TCP connection if they are no longer true for ordinary direct paths.

# 8. Update roadmap and plan index

When all executable qualification gates are complete:

- move the embedded Rust program from active to completed status in `plans/README.md`;
- update `plans/ROADMAP.md` current product position/date;
- record the ordered child-plan outcomes and frozen SHA;
- state the footprint outcome without exaggeration;
- note whether the engine is now ready for downstream migration evaluation, while explicitly keeping any downstream migration outside eggfetch scope;
- retain all child plans as historical implementation records.

Do not delete older plans solely to shorten the directory.

# 9. Compatibility/profile truth pass

Verify that documentation agrees with the live profile/ledger records produced by the requalification plan:

- HTTPX 0.28.1 exact-SHA status;
- HTTPX2 2.12.0 exact-SHA/status;
- HTTPX 1.0 preview remains unqualified;
- HTTP/3 remains at its separately established maturity level;
- no docs imply that native static routing or JSON are part of HTTPX public compatibility unless they truly are reference-compatible facade behavior.

# 10. Descendant audit

Before closing the plan, compare the frozen executable SHA to final HEAD and classify every changed file.

Allowed classes:

- `.md` documentation/plans;
- profile/ledger metadata explicitly allowed by the requalification process;
- static result/evidence files that do not affect validation execution.

Any source, test, manifest, lockfile, script, workflow, packaging, compatibility harness, or executable qualification fixture change means the descendant is not documentation-only and requires a new requalification freeze.

Record this audit in the program/index closure note.

## Non-goals

- no executable bug fixes;
- no new Cargo features;
- no dependency changes;
- no CodeGG migration plan in the eggfetch repo;
- no HTTP/3 graduation;
- no HTTPX 1.0 implementation work;
- no new benchmark/CI framework;
- no deletion/rewrite of historical plan records simply for cleanup.

## Exit criteria

- [ ] All current architecture/docs describe the frozen implementation accurately.
- [ ] Supported embedded feature profiles are documented with explicit capabilities.
- [ ] TLS trust behavior has one consistent documented model.
- [ ] Static destination routing guarantees/limitations are precise and security wording is bounded.
- [ ] Native JSON semantics are documented.
- [ ] Footprint evidence is dated/profile-specific and does not overclaim.
- [ ] Connection reuse/pooling docs distinguish logical limits from physical transport reuse correctly.
- [ ] `plans/README.md` and `plans/ROADMAP.md` record final program status.
- [ ] Compatibility documents agree with live exact-SHA profiles/ledger.
- [ ] Descendant audit proves no executable drift after final qualification.
- [ ] Parent embedded-Rust engine program is ready to close.
