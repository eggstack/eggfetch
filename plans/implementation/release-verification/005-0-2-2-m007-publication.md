# Release and Verification Milestone 005 — 0.2.2 M007 Publication and Release Polish

Status: ready

Repository baseline: `44c6477b6f23d407e9989c496e48915619e7d633` (`main`, M007 closure head)

Source roadmap:

- `plans/subsystems/release-verification-roadmap.md`, Milestone 5

Upstream capability closure:

- `plans/closure/core-transport-policy/007-native-transport-failure-classification.md`

Applicable ADRs:

- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`
- `plans/adrs/ADR-0006-native-transport-failure-classification.md`

Long-term requirements:

- `plans/000-long-term-specification.md` §6, §7
- `plans/001-terminology-and-domain-model.md` §10, §11
- `plans/002-long-term-roadmap.md` Phases 1–3

Primary class: infrastructure / operational release, with bounded corrective
planning and release-documentation polish

Target coordinated version: **0.2.2**

## 1. Objective

Publish the already-implemented and qualified M007 native transport failure
classification as one coordinated EggFetch 0.2.2 release, while correcting
the small post-M007 planning/release-documentation drift discovered at closure.

Required public outputs:

- all six publishable crates at 0.2.2:
  - eggfetch-http-connect
  - eggfetch-core
  - eggfetch-cli
  - eggfetch-ffi
  - eggfetch-python
  - eggfetch-node
- PyPI eggfetch 0.2.2 with the existing 18-wheel + 1-sdist matrix;
- a signed `v0.2.2` tag targeting the exact frozen release candidate;
- a non-draft, non-prerelease GitHub Release for `v0.2.2`;
- public registry/install smoke proving the coordinated release is usable;
- release/planning documentation reconciled to the published state.

The release must expose M007's additive
`TransportFailureKind` / `Error::transport_failure_kind()` API without
changing the M007 behavior contract.

This milestone performs no new runtime feature work.

## 2. Version-selection rationale

0.2.2 is the correct target under the current compatibility policy:

- 0.2.1 is the current coordinated public release;
- M007 is additive: it adds one non-exhaustive enum and one Error method while
  leaving the exhaustive Error enum, Error::kind(), existing error kinds,
  feature defaults, and MSRV unchanged;
- the M007 semver oracle passed with no breaking-semver update required;
- no minor-release-only change is planned (notably no MSRV increase, error-kind
  removal/reclassification, feature-default change, or breaking API change).

If preflight discovers that any 0.2.2 registry/tag identity already exists,
stop. Do not overwrite or retarget immutable public state; select a new patch
version in a fresh corrective plan.

## 3. Current evidence and release baseline

At the planning baseline:

- M007 is closed and qualified on executable freeze
  `3fc58fbd99ecb496749b833ee7b27436fe3b412d`;
- the closure/planning descendant on main is
  `44c6477b6f23d407e9989c496e48915619e7d633`;
- final M007 qualification recorded Tier 1 and Tier 2 green, Rust 1.89.0 MSRV
  green, six Rust API profiles green, 223 semver checks green, 1,934 facade
  compatibility tests green, and security preflight green;
- the latest coordinated public release is v0.2.1;
- all six publishable Cargo packages and the Python project remain versioned
  0.2.1;
- internal publishable dependency version constraints remain coordinated at
  0.2.1;
- the PyPI workflow remains manual `workflow_dispatch`, with 18 wheels
  (Linux x86_64, macOS arm64, Windows x86_64 × Python 3.10–3.15) plus one
  sdist and Trusted Publishing for `publish=true`;
- the current release process requires manual crates.io publication,
  build-only PyPI rehearsal, signed tag, PyPI publish, registry smoke, and
  manual GitHub Release.

Known polish/reconciliation debt at this baseline:

1. the core-transport roadmap and registry still label the subsystem
   `active` even though M007 is closed and its own closure says the subsystem
   returned to steady state;
2. the release-verification roadmap still describes the pre-M007 Stage C
   state and has no active post-M007 publication milestone;
3. the planning README still says there is no active release task;
4. `.github/workflows/pypi.yml` contains a time-bound Python 3.15 prerelease
   comment tied to 2026-10-01. At execution, verify actual Python 3.15 GA
   availability and make the comment truthful without changing the wheel
   matrix or weakening interpreter coverage.

The registration commit for M005 may reconcile items 1–3 immediately because
they are planning control-surface truth, not release execution.

## 4. Invariants that must not regress

- Never move, delete, or reuse v0.2.0 or v0.2.1.
- Never create or move v0.2.2 until the exact final candidate has completed
  every required pre-publication gate and build-only wheel rehearsal.
- All six publishable crates and PyPI eggfetch use the same 0.2.2 identity.
- All internal publishable dependency constraints move coherently to 0.2.2.
- Rust MSRV remains 1.89.
- M007 public behavior remains unchanged:
  `TransportFailureKind::{Connect,Tls,Protocol,Cancelled}`,
  `Error::transport_failure_kind()`, and existing Error/timeout/dialer/
  NetworkFailureKind semantics.
- No new Error variant, feature flag, dependency, transport policy, retry
  policy, proxy behavior, or adapter behavior is authorized.
- Version/package/config changes invalidate the exact-SHA release candidate:
  Tier 2 and the Stage C binding must be renewed on the final candidate after
  all candidate-material edits are complete.
- crates.io publication remains manual from a trusted maintainer environment.
- PyPI publication remains the existing manual `pypi.yml` dispatch using OIDC
  Trusted Publishing and protected-environment approval.
- GitHub Release creation remains manual and must reuse the already-pushed
  signed tag.
- `eggfetch-bench` and `fuzz/` are never published.
- Partial registry publication is recorded truthfully. Never overwrite a
  registry version or move a tag to simulate atomic publication.

## 5. Scope

### In scope

- reconcile the stale post-M007 planning/control-surface status;
- select and apply the coordinated 0.2.2 version;
- update all internal coordinated dependency pins to 0.2.2;
- update `crates/eggfetch-python/pyproject.toml` to 0.2.2;
- create a truthful 0.2.2 changelog section focused on M007 plus any already
  landed post-0.2.1 fixes that are part of the candidate;
- review release-facing documentation for stale 0.2.1/current-version claims;
- verify Python 3.15 GA state and correct only stale release-workflow prose/
  comments if needed;
- freeze one exact final candidate;
- run Tier 1, Tier 2, Tier 3, and live security preflight;
- run a build-only 18-wheel + 1-sdist PyPI rehearsal on the exact candidate;
- manually publish crates.io packages in dependency order with registry
  propagation checks;
- create/push signed v0.2.2 on the exact candidate;
- publish PyPI from that exact tag;
- create the GitHub Release;
- verify public package/install surfaces;
- write M005 closure and reconcile planning/release docs.

### Explicitly out of scope

- M007 behavior changes or classifier redesign;
- new errors/categories;
- dependency upgrades unrelated to release identity;
- Rust MSRV changes;
- Python/HTTPX/CLI/FFI/Node API expansion;
- wheel matrix changes, new architectures, abi3/abi3t, or free-threaded
  Python support;
- new CI workflows or automatic publication;
- Node npm publication;
- HTTPX 1.0 work;
- H3 graduation;
- performance work unrelated to a release-blocking regression;
- moving historical tags or overwriting immutable registry versions.

## 6. Required candidate edits

The final candidate should contain only release-identity/documentation/package
changes beyond the already-closed M007 executable freeze, unless a release
gate proves a real blocker.

At minimum update:

- package version in:
  - `crates/eggfetch-http-connect/Cargo.toml`
  - `crates/eggfetch-core/Cargo.toml`
  - `crates/eggfetch-cli/Cargo.toml`
  - `crates/eggfetch-ffi/Cargo.toml`
  - `crates/eggfetch-python/Cargo.toml`
  - `crates/eggfetch-node/Cargo.toml`
- coordinated internal dependency version requirements in those manifests;
- `crates/eggfetch-python/pyproject.toml`;
- `Cargo.lock` through normal Cargo resolution;
- `CHANGELOG.md` with a 0.2.2 section;
- release/version references that are meant to describe current public state,
  not historical evidence.

Do not rewrite historical 0.2.1 plans/closure records.

For Python 3.15, verify the public interpreter state at execution. If 3.15.0
final is available, remove or rewrite the stale time-bound workflow comment so
it no longer describes 3.15 as prerelease. Keep the existing Python 3.15 row
and `allow-prereleases` behavior unless evidence proves a behavioral change is
necessary; stable interpreters should naturally satisfy the requested version
once available.

## 7. Ordered work packages

### WP0 — Planning/control-surface reconciliation

Before release edits:

- core-transport roadmap -> closed/steady state after M007;
- registry core-transport row -> closed with no active implementation;
- release-verification roadmap -> M005 ready/active workstream;
- registry -> M005 dependency-ready;
- plans README execution gate -> M005 is the current release handoff.

This planning-registration commit performs this reconciliation. Historical
closure records remain immutable.

### WP1 — Public identity preflight

Verify immediately before versioning:

- no remote v0.2.2 tag;
- no GitHub Release for v0.2.2;
- no crates.io 0.2.2 release for any of the six publishable packages;
- no PyPI eggfetch 0.2.2 release;
- public v0.2.1 identities point where the historical closure says they do;
- current main descends from the M007 closure line;
- no unreviewed executable change has landed after the planning baseline.

If unexpected public state or executable drift exists, stop and classify it
before editing release identity.

### WP2 — Version/changelog/release polish

Bump every coordinated version and internal version edge to 0.2.2, update
pyproject and lockfile, and draft the 0.2.2 changelog.

The changelog must distinguish:

- Added: M007 native transport failure classification;
- Fixed/Changed: any already-landed post-0.2.1 correctness/tooling changes
  actually present in the candidate;
- no claim of new Python/CLI/FFI/Node error API unless such a surface exists;
- MSRV remains 1.89;
- no feature-default/dependency change unless the final diff proves otherwise.

Review release-facing docs and workflow comments for stale version/Python 3.15
language. Do not turn documentation polish into policy expansion.

Run the release-version validator before freezing.

### WP3 — Freeze and exact-SHA qualification

Once candidate-material edits are complete, freeze the exact candidate SHA.

Required on that exact candidate:

    ./scripts/check.sh
    ./scripts/check.sh extended
    ./scripts/check.sh package
    ./scripts/check_security.sh

Also explicitly run/record:

    python scripts/validate_release_versions.py
    cargo tree -p eggfetch-core
    cargo tree -p eggfetch-core -e features
    cargo tree -p eggfetch-core -d

Tier 2 must renew the six-profile public API/semver evidence and exact Rust
1.89.0 MSRV matrix.

Because manifests/package identity changed after the M007 freeze, renew the
Stage C exact-SHA binding to this final candidate using the canonical ledger
and both facade profile files. The binding must name the candidate that is
actually released.

Any executable/package/validation/compat-fixture edit after this qualification
invalidates the freeze and requires the affected gates to be rerun before
publication.

### WP4 — Build-only PyPI rehearsal

From the exact frozen candidate, manually dispatch PyPI Wheels with
`publish=false`.

Required evidence:

- workflow head SHA equals the frozen candidate;
- 18 wheels + 1 sdist assemble;
- all matrix rows pass smoke;
- Python versions 3.10–3.15 are covered on Linux x86_64, macOS arm64, and
  Windows x86_64;
- record the exact interpreter selected for the 3.15 rows;
- `twine check` and package-content validation pass.

If any candidate file changes after this rehearsal, rerun the rehearsal.

### WP5 — crates.io publication

Immediately before publication, rerun the live security preflight if the
recorded scan is no longer contemporaneous.

Publish manually from a trusted environment in dependency order:

    cargo publish -p eggfetch-http-connect
    cargo publish -p eggfetch-core
    cargo publish -p eggfetch-cli
    cargo publish -p eggfetch-ffi
    cargo publish -p eggfetch-python
    cargo publish -p eggfetch-node

Before each dependent publish, verify the preceding 0.2.2 dependency is
visible to crates.io resolution. Do not encode fixed sleeps.

For dependents, run the full `cargo publish --dry-run -p <crate>` when their
new internal dependencies are resolvable, as required by the release process.

Record each immutable publication result.

### WP6 — Signed tag and PyPI publication

After successful crates.io publication:

- create signed `v0.2.2` on the exact frozen candidate;
- push the tag without moving it;
- manually dispatch PyPI Wheels from that tag with `publish=true`;
- verify the workflow's tag/commit/version checks and live security preflight;
- inspect the assembled 19-distribution set before environment approval;
- approve the protected `pypi` environment;
- verify PyPI 0.2.2 and representative import/version behavior.

If PyPI publication fails after crates.io publication, do not delete/yank
healthy crates solely to recreate atomicity. Preserve public truth and prepare
a corrective patch if necessary.

### WP7 — GitHub Release and public smoke

Create a non-draft, non-prerelease GitHub Release from the existing signed
v0.2.2 tag.

Release notes should be derived from the 0.2.2 changelog and call out the new
Rust-native transport classifier without implying that Python/CLI/FFI/Node
received equivalent new classifier APIs.

Verify:

- tag target == frozen candidate;
- GitHub Release tag == v0.2.2;
- all six crates.io packages report 0.2.2;
- PyPI reports eggfetch 0.2.2;
- fresh representative Rust install resolves eggfetch-core 0.2.2 and can
  compile use of `TransportFailureKind` /
  `Error::transport_failure_kind()`;
- fresh Python install reports 0.2.2 and basic sync/async smoke remains green.

### WP8 — Closure and downstream handoff

Write:

`plans/closure/release-verification/005-0-2-2-m007-publication.md`

Then reconcile:

- release-verification roadmap -> M005 closed/steady state;
- registry -> M005 closed and public identities recorded;
- plans README execution gate -> no active release task;
- current-version documentation -> 0.2.2 where appropriate.

The closure must explicitly state that M007's API is now available from the
published registry version so exact-pin downstream consumers may start their
own adoption milestone. It must not claim downstream migration has occurred.

## 8. Verification and evidence requirements

### Required before publication

- Tier 1 green on final candidate.
- Tier 2 green on final candidate.
- Tier 3 green on final candidate.
- live security preflight green.
- exact Rust 1.89.0 MSRV green.
- six-profile public API oracle and semver checks green with expected M007
  additive API.
- full compatibility suites green.
- version validator proves all coordinated package/project identities are
  0.2.2.
- build-only 19-distribution rehearsal green on the same candidate.
- current head CI green for the candidate or an exact docs-only descendant
  whose relationship is documented.

### Required after publication

- six crates.io 0.2.2 versions visible;
- signed v0.2.2 tag resolves to frozen candidate;
- PyPI 0.2.2 has exactly the expected release set;
- GitHub Release exists and targets v0.2.2;
- representative Rust and Python fresh-install smoke green.

## 9. Security review

No new security behavior is authorized. Still run the complete release
security checklist because publication changes the supported public artifact.

Specifically confirm:

- M007 classifier remains enum-only and does not surface nested secret text;
- dependency/advisory/license/source checks are current;
- TLS/proxy/auth/redaction regression suites remain green;
- no release credential is committed;
- crates.io credentials remain local;
- PyPI uses OIDC only;
- tag is signed;
- published package contents contain no generated/private qualification
  artifacts.

## 10. Failure and partial-publication semantics

Before any public publication, a red required gate means stop and fix/re-freeze.

After the first immutable registry publication:

- never retarget the candidate tag to hide drift;
- never overwrite a crates.io/PyPI version;
- record exactly which channels succeeded;
- if a defect requires source/package changes, select the next patch version
  and open a corrective release plan;
- yank/deprecate only when the defect itself warrants it, not merely because
  another channel failed.

## 11. Acceptance criteria

1. Post-M007 planning status is reconciled before execution handoff.
2. All coordinated package/project versions and internal constraints are
   0.2.2.
3. 0.2.2 changelog accurately describes M007 and other included post-0.2.1
   changes without overstating adapter APIs.
4. Rust MSRV remains 1.89; no unrelated dependency/feature/default change.
5. Final candidate passes Tier 1, Tier 2, Tier 3, security, API/semver, and
   MSRV checks.
6. Stage C is rebound to the exact final release candidate.
7. Build-only PyPI rehearsal produces 18 wheels + 1 sdist from that candidate.
8. Six crates publish to crates.io in dependency order with propagation
   verified.
9. Signed v0.2.2 points to the exact candidate.
10. PyPI 0.2.2 publishes from that tag with all 19 distributions.
11. GitHub Release v0.2.2 exists and is non-draft/non-prerelease.
12. Fresh Rust smoke can compile the M007 classifier API from 0.2.2.
13. Fresh Python smoke reports 0.2.2 and basic request behavior works.
14. Closure record captures immutable public identities, qualification/rehearsal
    evidence, residual findings, and downstream-unblock statement.
15. Registry/roadmap/README return to truthful steady-state status after
    publication.

## 12. Stop conditions

Stop before publication if:

- v0.2.2 or any coordinated 0.2.2 registry version already exists unexpectedly;
- version bump reveals incoherent internal dependency edges;
- release preparation requires runtime/API behavior changes;
- M007 semantics differ from its closure evidence;
- any required Tier 1/2/3/security/MSRV/API/compat gate is red;
- the 19-distribution build-only rehearsal is incomplete or from a different
  SHA;
- Stage C cannot be rebound cleanly to the candidate;
- Python 3.15 selection exposes an unsupported interpreter/package issue that
  cannot be resolved without changing the promised matrix;
- package contents or registry dry-runs reveal unintended artifacts;
- signing/tag identity cannot be verified.

After partial publication, stop using this plan as though the release were
atomic; record the partial state and open a patch corrective.

## 13. Closure evidence

The closure record must include:

- final candidate SHA and Stage C binding;
- version-file/internal-dependency coherence evidence;
- changelog/release-doc review;
- Python 3.15 interpreter selected in the rehearsal;
- Tier 1/Tier 2/Tier 3 results;
- Rust 1.89.0 MSRV result;
- public API + semver oracle result;
- full compatibility result;
- live security-preflight timestamp/tool versions;
- build-only PyPI workflow run ID and 19-artifact result;
- each crates.io package/version publication and propagation evidence;
- signed tag object/target;
- PyPI publish run ID and public release evidence;
- GitHub Release evidence;
- fresh Rust classifier API smoke;
- fresh Python install/import/request smoke;
- unresolved findings by severity;
- partial-publication incidents, if any;
- final recommendation: closed, conditionally closed, corrective required, or
  blocked;
- explicit downstream handoff: EggPool/other exact-pin consumers may begin
  adoption only after public 0.2.2 verification is complete.

## 14. Non-goals

- no new transport/error capability;
- no dependency modernization;
- no MSRV bump;
- no wheel-matrix expansion;
- no CI/publish automation redesign;
- no HTTPX 1.0/H3/Node maturation work;
- no downstream EggPool implementation;
- no historical plan/tag rewriting.
