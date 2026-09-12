# Post-Embedded-Engine Compatibility Requalification and Closure

Planning baseline: `445149bd2af3c3ce5c0d34daacbde1bf8de57297` (`main`, 2026-09-11; eggfetch 0.1.3)
Parent program: `plans/embedded-rust-client-footprint-and-routing-program.md`
Prior qualified executable SHA: `78a77ea153aae239ce2693fec406fa2a61865dad3` for the currently documented HTTPX 0.28.1 / HTTPX2 2.12.0 qualification state, subject to the live ledger's descendant rules
Depends on completion of all executable/test/validation child plans in the parent program
Followed by: `plans/post-embedded-engine-documentation-and-plan-hygiene.md`

## Objective

Freeze one clean executable/test/validation SHA after the embedded-Rust engine work, run the repository's existing verification/package/compatibility procedures on that exact tree, renew HTTPX 0.28.1 and HTTPX2 2.12.0 qualification claims only if their existing gates pass, and close the executable portion of the parent program without inventing a new evidence system.

This is the only child plan in the program permitted to renew exact-SHA compatibility claims.

## Governing rule

> All evidence for a claimed compatibility profile must describe one exact frozen executable/test/validation SHA. Any later source, test, manifest, lockfile, compatibility harness, validation script, package configuration, or qualification-fixture change invalidates that evidence and requires a new freeze.

Documentation/profile/ledger-only descendants are permitted after evidence collection when a descendant audit proves executable identity.

# 1. Confirm prerequisite closure

Before selecting a candidate SHA, verify closure of:

- `core-feature-dependency-and-tls-boundary-hardening.md`;
- `static-resolution-and-pinned-destination-routing.md`;
- `native-rust-json-and-response-ergonomics.md`;
- `embedded-consumer-footprint-qualification.md`.

Confirm specifically that:

- the feature/dependency matrix is stable and documented;
- TLS default/native/WebPKI/custom trust semantics have focused regression coverage;
- static routing is fail-closed across direct/retry/redirect/proxy/H3 selection;
- physical connection reuse cannot bypass static route constraints;
- JSON helpers preserve body replay/limit/single-consumption semantics;
- footprint qualification has either completed bounded corrections or explicitly accepted remaining overhead;
- no executable correction is intentionally postponed into the documentation-only descendant.

Acceptance:

- [ ] each prerequisite plan has explicit closure evidence;
- [ ] no known required correctness/security defect is hidden behind final qualification;
- [ ] current HEAD is not described as freshly qualified before this plan completes.

# 2. Audit qualification-sensitive change clusters

Compare the previously qualified executable baseline/current live-ledger SHA to candidate HEAD and classify changes by at least:

1. Cargo feature/dependency ownership;
2. TLS root-store construction/default behavior;
3. standard/direct/proxy/H3 TLS connector construction;
4. request transport state and static resolution;
5. retry state preservation;
6. redirect state clearing/preservation;
7. proxy/SOCKS route selection;
8. H3/Alt-Svc interaction with static routing;
9. client/connector cache and connection-reuse identity;
10. native JSON/body/error behavior;
11. response metadata/body limits;
12. benchmark/qualification scripts/manifests;
13. Python/CLI/FFI/Node build-feature changes caused by core manifest edits;
14. package metadata/lockfile changes.

Every changed behavioral cluster needs focused regression evidence before freeze.

# 3. Focused pre-freeze semantic gate

Run directly affected deterministic tests before freezing.

## Feature/dependency/TLS

At minimum exercise supported combinations for:

- no-default-features compile contract;
- H1 cleartext profile if supported;
- H1 + Rustls WebPKI-only profile;
- H1 + Rustls + native-root/default profile;
- H2 selected profile;
- proxy/compression/multipart combinations retained by existing feature validation;
- all-features;
- custom CA/client cert/default trust paths;
- default standard vs specialized direct trust equivalence.

## Static routing

Run the full security regression corpus from the child plan, especially:

- no resolver invocation in static mode;
- logical Host/SNI/cert identity;
- candidate fallback under one budget;
- no DNS fallback after all candidates fail;
- retry snapshot preservation;
- same-origin/cross-origin redirect behavior;
- strict redirect behavior;
- proxy rejection/interaction;
- H3/Alt-Svc fail-closed behavior;
- incompatible pooled-route isolation.

## Native JSON/body behavior

Exercise:

- request serialization/header precedence;
- request replay on retries/allowed redirects;
- response deserialization/single consumption;
- invalid JSON error classification/redaction;
- decompression/body-limit interaction;
- feature disabled/enabled compile behavior.

## Compatibility-sensitive adapters

Run affected HTTPX and HTTPX2 tests for:

- TLS/verify/cert behavior;
- proxy/no_proxy routing;
- redirects and retries;
- response streaming/raw/body limits;
- request body/header behavior;
- package/import/build behavior across facade surfaces.

Native-only static resolution/JSON helpers must not leak accidental symbols into compatibility facades.

Acceptance:

- [ ] focused gate passes before freeze;
- [ ] failures are corrected rather than waived;
- [ ] every source/test/fixture correction is committed before freeze.

# 4. Freeze one executable/test/validation candidate

Run the normal Tier 1 check and establish a clean candidate:

```sh
./scripts/check.sh
git status --short
git rev-parse HEAD
```

Record the full 40-character `FROZEN_EXECUTABLE_SHA`.

Requirements:

- clean worktree;
- all manifests/lockfiles/qualification fixtures/scripts committed;
- embedded-footprint evidence that affects conclusions committed if repository policy stores it;
- no generated binding/profile artifacts left dirty;
- no modification of qualification-sensitive files after evidence collection starts.

Acceptance:

- [ ] one exact clean frozen SHA exists.

# 5. Run Tier 1 / extended / package validation

On the frozen SHA run the existing authoritative commands:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Do not edit validation scripts while collecting evidence. If a script is incorrect, fix it, commit the fix, select a new frozen SHA, and restart from Section 4.

Record permitted environment-dependent skips explicitly. A missing prerequisite is unsupported evidence, not a pass.

Acceptance:

- [ ] Tier 1 passes;
- [ ] extended validation passes;
- [ ] package validation passes;
- [ ] default and selected minimal core feature builds remain represented truthfully in the existing feature-matrix policy.

# 6. Requalify HTTPX 0.28.1

Use the repository's existing pinned HTTPX 0.28.1 requirements/profile and full qualification procedure on the frozen SHA.

At minimum:

- full compatibility suite three consecutive times without changing files/dependencies;
- API oracle/reference comparison;
- allowed/resolved-difference reconciliation;
- high-risk differential/security corpus;
- required downstream portfolio under existing policy.

Rules:

- no selecting only successful runs around unexplained flakes;
- no new allowed difference solely to excuse a regression introduced by this program;
- native-only APIs must not alter the facade contract accidentally.

Acceptance:

- [ ] three consecutive full HTTPX 0.28.1 runs pass;
- [ ] no unexplained API difference remains;
- [ ] retained differences remain current/tested;
- [ ] required downstream evidence passes.

# 7. Requalify HTTPX2 2.12.0 independently

Run the existing HTTPX2 2.12.0 profile on the same frozen executable SHA.

At minimum include:

- three consecutive full declared-surface runs;
- API oracle/reference comparison;
- allowed/resolved-difference reconciliation;
- existing behavior/security corpus;
- SSE and optional WebSocket surfaces where required by the current profile;
- required downstream portfolio if the live profile requires it.

Do not infer HTTPX2 qualification from HTTPX 0.28.1 results.

Acceptance:

- [ ] HTTPX2 receives only the qualification stage actually earned by the frozen tree;
- [ ] no native JSON/static-routing API leaks into the facade unexpectedly.

# 8. Re-run embedded footprint qualification on the frozen SHA

Because the footprint plan may have performed corrective tuning, run its final bounded measurement on the exact frozen SHA or prove the recorded measurement was taken on an executable-identical ancestor.

Record:

- exact eggfetch SHA;
- reqwest comparison version;
- feature profiles;
- toolchain/target;
- stripped artifact sizes;
- dependency/feature summary;
- outcome category: beneficial, neutral/strategic, or not a footprint win.

This evidence does not need to become a compatibility gate, but the documentation descendant must have final numbers tied to the frozen implementation.

# 9. Remote CI

Push the frozen candidate or an executable-identical record descendant and confirm the repository's existing remote CI succeeds. Do not add a special program-specific workflow.

# 10. Update compatibility profile/ledger records

Only after corresponding gates pass:

- update `compat/httpx/0.28.1/profile.toml` SHA/date/status as required by current conventions;
- update `compat/httpx2/2.12.0/profile.toml` independently;
- update the live compatibility ledger/status records;
- leave HTTPX 1.0 preview explicitly unqualified;
- leave HTTP/3 maturity status unchanged unless separately qualified by its own governing process.

Record commits after freeze must be profile/ledger/documentation-only.

# 11. Descendant audit

Compare:

```text
FROZEN_EXECUTABLE_SHA..HEAD
```

Classify every file. Any source/test/build/validation/package/manifest/lockfile drift invalidates the exact-SHA evidence and requires a new freeze/requalification.

# 12. Final executable closure

Before handing to the documentation-only child plan verify:

- selected core profiles compile as documented;
- current default behavior is preserved;
- static destination guarantees remain tested;
- JSON feature remains optional;
- footprint evidence describes the frozen implementation;
- compatibility profiles/ledger point to the intended frozen SHA;
- remote CI is green;
- HEAD descendants after the freeze contain no executable drift.

## Non-goals

- no new feature work during qualification;
- no new compatibility stage/evidence schema;
- no HTTPX 1.0.dev qualification;
- no HTTP/3 production-graduation attempt;
- no release publication;
- no CodeGG migration;
- no special CI architecture for this program.

## Exit criteria

- [ ] one exact frozen executable/test/validation SHA exists;
- [ ] Tier 1, extended and package validation pass on it;
- [ ] HTTPX 0.28.1 is freshly and truthfully requalified;
- [ ] HTTPX2 2.12.0 has a freshly and independently earned status;
- [ ] final embedded footprint evidence is tied to the frozen implementation;
- [ ] remote CI passes;
- [ ] profile/ledger-only descendants are proven executable-identical;
- [ ] executable work in the parent program is closed and only docs/registry hygiene remains.
