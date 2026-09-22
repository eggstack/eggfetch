# Post-Private-Architecture API Requalification and Closure

Planning baseline: `c53eebc47569279b61c0611d4523c5132bdbcaeb`
Parent program: `plans/api-preserving-private-architecture-containment-program.md`
Prerequisites:
- `plans/core-client-proxy-private-decomposition-second-pass.md`
- `plans/python-streaming-and-cli-private-decomposition.md`
- `plans/rust-surface-containment-and-experimental-adapter-hygiene.md`
Normative verification policy: `docs/verification-policy.md`

## Objective

Close the private-architecture containment campaign on one exact executable
SHA and prove that the refactors changed implementation ownership only.

This plan is the sole authority for final qualification and closure of the
program. Child plans may run focused/Tier 1/extended checks while executing,
but they must not independently claim final compatibility renewal.

## Part A — audit child-plan completion before freezing

Review each prerequisite against its literal acceptance criteria.

Do not freeze until:

- core client/proxy decomposition is complete or explicitly records a retained
  responsibility because moving it would worsen ownership or require API
  drift;
- Python streaming and CLI decomposition is complete with no public/behavioral
  drift;
- Rust low-level public exposure containment is complete;
- Node metadata/support-status hygiene is complete without capability growth;
- no child plan has an unresolved failing or skipped-required acceptance item.

Retained implementation concentration is acceptable when the child plan
explains why moving it would violate dependency direction, visibility, cfg
boundaries, or semantic ownership.

## Part B — executable/test/validation freeze

Choose one commit after all executable, test, validation-script, package
metadata and qualification-input changes from Plans 1-3 have landed.

Record:

- full SHA;
- UTC date;
- parent SHA;
- list of files changed by this program relative to
  `c53eebc47569279b61c0611d4523c5132bdbcaeb`;
- explicit classification of executable vs tests vs validation vs package
  metadata vs docs/plans;
- current crate/package versions.

After freezing, no Rust/Python/JS executable source, test, validation script,
workflow, package manifest, compatibility fixture/profile, or qualification
input may change without invalidating the freeze and selecting a new one.

Documentation-only closure descendants are allowed after qualification.

## Part C — Rust public API proof

Run the supported-profile compile contracts and exact API oracle.

Required profile coverage:

- default;
- `http1,tls-rustls`;
- `http2,tls-rustls`;
- `standard-http1,tls-rustls`;
- `native-http1,tls-rustls`;
- all-features.

Run the exact `cargo-public-api`/semver oracle exactly as documented in
`compat/rust-public-api/README.md`.

Acceptance:

- zero unexplained addition/removal/signature/path/feature-profile drift;
- snapshots remain unchanged;
- no snapshot regeneration is used to make the refactor pass;
- stable compile contracts pass.

If a public difference exists, the campaign is not API-preserving. Fix the
refactor or split a separate versioned API plan; do not waive it here.

## Part D — native Python runtime and typing proof

Build the extension from the freeze and run:

- native API manifest checker including self-test;
- typing-surface relational checker including self-test;
- mypy/typing fixture;
- full native Python behavior suite;
- sync/async streaming/lifecycle/close-race tests;
- wheel/package typing smoke where required by package tier.

Acceptance:

- `eggfetch.__all__` unchanged;
- constructor/method/property signatures unchanged;
- exception hierarchy unchanged;
- sync/async semantic mirrors unchanged;
- iterator and streaming return shapes unchanged;
- accepted/rejected arguments unchanged;
- no new private-module object leaks into the root public package.

## Part E — HTTPX/HTTPX2 compatibility proof

Run the repository's full pinned compatibility procedure for:

- HTTPX 0.28.1;
- HTTPX2 2.12.0.

Run API-manifest comparison and the full compatibility suite under the
normative exact-SHA rules.

If repository policy requires repeated clean compatibility runs for Stage C,
perform the required count and record each result against the same freeze.

Acceptance:

- zero new unexplained API differences;
- zero new compatibility waiver;
- no residual difference is reclassified merely to accommodate a refactor;
- existing residuals remain bounded and truthfully documented.

Only renew Stage C/profile/ledger SHA bindings if the normative policy says the
executable/test/qualification-input delta invalidated the previous binding.
When renewed, all canonical records must name the same freeze.

## Part F — C ABI, CLI and Node adapter proof

### C ABI

Run all FFI tests with applicable features and verify:

- exported symbol set unchanged;
- handle ownership and consumption behavior unchanged;
- panic guards/sentinels unchanged;
- buffered/streaming behavior unchanged;
- feature ownership unchanged.

### CLI

Run the CLI crate tests and direct behavior/smoke fixtures used by repository
validation.

Verify:

- help/argument contract unchanged;
- exit-code mapping unchanged;
- stdout/stderr and machine-output behavior unchanged;
- redaction unchanged;
- file naming/no-clobber behavior unchanged;
- streaming/destructor-safe error paths unchanged.

### Node

Run Rust-side Node tests.

If the optional JS test cannot run because no native artifact is present,
record the ordinary explicit skip exactly as repository policy allows. Do not
turn optional prototype validation into a new release requirement during
closure.

Verify package metadata reflects the intended experimental status and no new
supported Node API appeared.

## Part G — feature/dependency/package proof

Run existing feature/dependency validation to prove:

- no default-feature drift;
- no lean-profile capability leakage;
- no new production dependency unless a child plan separately justified it
  (default expectation: none);
- adapter feature ownership remains correct;
- package-content validation remains correct.

Run package tier when required by source-layout or manifest changes.

If Node metadata changes package validation, ensure the validator checks truth
without assuming npm publication support.

## Part H — MSRV, lint, docs and security proof

Required final gates include:

- exact Rust 1.89.0 MSRV validation;
- formatting;
- Clippy with warnings denied;
- lint-suppression policy;
- docs/rustdoc/doctests;
- dependency/security policy;
- release/version validation;
- applicable resource/lifecycle tests.

Do not close on a newer Rust toolchain alone.

## Part I — HTTP/3/Node status truth check

This program does not graduate either experimental surface.

At closure explicitly confirm:

- HTTP/3 is still labeled experimental in user-facing and architecture docs;
- no child refactor accidentally created new H3 capability or support claim;
- current H3 graduation blockers remain owned by the separate H3 program;
- Node remains an experimental prototype;
- no npm publication/declaration/support promise was introduced.

Do not rerun independent H3 graduation qualification merely to close this
program unless a child plan unexpectedly touched H3 protocol semantics. Such a
touch should normally trigger the stop condition instead.

## Part J — issue #24 and Python 3.15 independence

Do not conflate this campaign with independent maintainer/release actions.

At closure record current truth for:

- issue #24 publication/tag/PyPI status;
- Python 3.15 18-wheel `publish=false` rehearsal status.

They do not block this architecture campaign unless a child plan directly
changes the relevant release artifacts, but they must not be falsely marked
complete here.

## Part K — closure documentation

After all gates pass on the freeze:

1. record the exact freeze and results in this plan;
2. update the parent program to Completed;
3. update `plans/README.md` from Active to Completed;
4. update architecture docs only where file ownership/module maps changed;
5. keep compatibility/profile records synchronized if exact-SHA renewal was
   required;
6. distinguish later documentation-only commit SHAs from the executable
   freeze.

If a documentation pass discovers stale executable truth, correct the
executable/test/validation issue and re-freeze rather than papering over it.

## Required validation command groups

At minimum, using the repository's documented environment:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
./scripts/check_security.sh
```

Run exact Rust 1.89.0/MSRV and Rust public API-oracle commands per repository
documentation.

Run full HTTPX/HTTPX2 exact-SHA qualification per
`docs/verification-policy.md` and the current compatibility profiles.

## Final acceptance criteria

- [x] One exact executable/test/validation/package freeze SHA is recorded.
- [x] All prerequisite plans satisfy their literal acceptance criteria.
- [x] Rust public API oracle reports zero unexplained drift across all
      supported profiles and no snapshots were regenerated.
- [x] Native Python manifest and PEP 561 typing surfaces have zero drift.
- [x] Full native Python behavior/streaming/lifecycle tests pass.
- [x] HTTPX 0.28.1 and HTTPX2 2.12.0 compatibility/API gates have zero new
      unexplained drift.
- [x] C ABI exported surface and behavior are unchanged.
- [x] CLI public behavior and exit/output contracts are unchanged.
- [x] Node remains experimental and has no new supported capability.
- [x] HTTP/3 remains experimental and has no new support claim.
- [x] Feature/default/dependency ownership is unchanged.
- [x] Tier 1, extended, package, security, docs and exact MSRV gates pass.
- [x] No API/compatibility waiver was added for this campaign.
- [x] Canonical exact-SHA records all agree if renewal was required.
- [x] Issue #24 publication and Python 3.15 rehearsal state are reported
      independently and truthfully.
- [x] Parent program and `plans/README.md` are reconciled only after all
      required evidence is complete.

## Stop conditions

Reopen/fix and select a new freeze if any post-freeze change touches
executable/test/validation/package/qualification input.

Do not close if any public-surface difference, behavioral drift, compatibility
waiver, feature-profile drift, new Node/H3 capability, or unresolved required
gate remains.

## Final closure record

Status: complete on executable freeze `d4979f1dac53f30f07900f54b01a88de06956c1c`
(2026-09-22 UTC). The freeze is the first commit containing the executable,
test, package-metadata, architecture, skill, and qualification changes from
the three prerequisite plans. The planning baseline is
`c53eebc47569279b61c0611d4523c5132bdbcaeb`; this record is a documentation
descendant and does not reopen the freeze.

Freeze contents are classified as follows:

- executable/private ownership: core client/proxy modules, Python streaming
  modules, CLI modules, and unchanged adapter wiring;
- package metadata: Node package version/license alignment (`0.1.9`, `MIT`);
- validation evidence: no validation scripts, snapshots, compatibility
  fixtures, or workflows were changed;
- documentation/plans: architecture maps, README/AGENTS guidance, project
  skills, and the prerequisite implementation records.

Coordinated versions at the freeze are `eggfetch-core`, `eggfetch-http-connect`,
`eggfetch-cli`, `eggfetch-ffi`, `eggfetch-node`, and `eggfetch-python` 0.1.9;
`eggfetch-bench` remains 0.1.0.

Qualification evidence:

- Tier 1 `./scripts/check.sh`: passed;
- extended `./scripts/check.sh extended`: passed on the final idle-host run;
  the earlier contention-sensitive timeout failures were isolated and passed
  on rerun without source changes;
- package `./scripts/check.sh package`: passed through the repeated Tier 1,
  API, Python, HTTPX, and Node/package stages; the Node JavaScript artifact
  remained the policy-defined skip;
- security `./scripts/check_security.sh`: passed; cargo-deny advisories,
  bans, licenses, and sources plus cargo-audit passed. The existing duplicate
  `getrandom` lock warning is informational and not a security failure;
- exact Rust API oracle: six profiles passed, with 223 semver checks passing
  and 30 policy-defined skips; no snapshots were regenerated;
- native Python API/typing: 66 exports, 32 exception bases, and 24 reviewed
  member contracts passed; native behavior passed 578 tests;
- HTTPX 0.28.1 smoke passed 134 tests and the full pinned suite passed 1,928;
- FFI tests passed 34; Node Rust tests passed with JavaScript surface skipped
  because `crates/eggfetch-node/eggfetch.node` is absent;
- the exact Rust 1.89.0/MSRV and feature-profile stages passed in the final
  extended qualification.
- remote CI run `35674749373` passed for documentation head
  `e70ae121d314715c3ab4489a297b83b9300d8c2c` (9m12s); its single validation
  job completed successfully. GitHub's Node.js 20 and Ubuntu 26 runner
  migration notices are advisory annotations only.

Issue #24 publication/tag/PyPI work and the Python 3.15 wheel rehearsal remain
independent pending maintainer/release actions. HTTP/3 and Node remain
experimental; no capability or compatibility waiver was added.
