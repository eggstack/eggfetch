# Release and Verification Milestone 004 — 0.2.1 Tagged Release Finalization

Status: archived (closed; see plans/closure/release-verification/004-0-2-1-tagged-release-finalization.md)

Repository baseline: `57887520029344c228afe6b537da7e33c6321ef7` (`main`, after M002 closure)

Source roadmap:

- `plans/subsystems/release-verification-roadmap.md`

Parent release train:

- M001 coordinated 0.2.x publication umbrella

Supersedes before execution:

- `plans/implementation/release-verification/001b-0-2-1-coordinated-publication.md`

Long-term requirements:

- `plans/000-long-term-specification.md` §6, §7
- `plans/001-terminology-and-domain-model.md` §10, §11
- `plans/002-long-term-roadmap.md` Phases 1–3

Applicable ADR:

- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`

Primary class: infrastructure / operational release finalization, with bounded documentation polish

## 1. Objective

Complete one coordinated EggFetch `0.2.1` public release across all intended
channels while preserving exact release identity and leaving the repository's
release documentation truthful.

Required public outputs are:

- six crates.io packages at `0.2.1`;
- a signed Git tag `v0.2.1` targeting the exact final candidate;
- the PyPI `eggfetch 0.2.1` distribution with the full 18-wheel + 1-sdist
  matrix, published through the existing Trusted Publishing workflow;
- a non-draft, non-prerelease GitHub Release for `v0.2.1`, with release
  notes derived from the `0.2.1` changelog and links to the public package
  channels;
- post-publication registry/install smoke evidence;
- planning, release-process, and user-facing documentation reconciled so no
  completed prerequisite is still described as pending.

This milestone performs no runtime/API feature work and does not introduce a
new publication workflow.

## 2. Why this milestone is ready

M001A is closed on candidate
`41757569123c0b8038550b956d8b244ab55094a6`.

M002 is closed on the same candidate. PyPI Wheels run `36223505399`
completed successfully with `publish=false`, built all 18 required wheels
plus one sdist, passed smoke/coverage/`twine check`, and selected CPython
3.15.0-rc.2 for all three 3.15 rows. Nothing was published.

The prior M001B plan is unexecuted. It is superseded here because this release
now has two additional truth requirements:

1. a GitHub Release object tied to the signed tag is a required release
   output, not an optional afterthought;
2. the frozen candidate's root README still says Python 3.15 wheels are
   "pending" rehearsal even though M002 is closed. That README is the PyPI
   long description, so publishing it unchanged would knowingly ship stale
   package metadata.

Because the package README must be corrected before publication, the exact
candidate SHA must be refreshed and the build-only wheel rehearsal repeated
against that refreshed docs-only candidate. The version remains `0.2.1`.

## 3. Current implementation and release evidence

At this planning baseline:

- all six publishable Rust crates and `crates/eggfetch-python/pyproject.toml`
  are already versioned `0.2.1`;
- `CHANGELOG.md` already contains a `0.2.1` entry;
- M001A's original candidate SHA is
  `41757569123c0b8038550b956d8b244ab55094a6`;
- M002's successful rehearsal run `36223505399` has
  `head_sha == 41757569123c0b8038550b956d8b244ab55094a6`;
- the historical `v0.2.0` tag/release must remain immutable;
- no `v0.2.1` tag exists at this baseline;
- the latest GitHub Release is `v0.2.0`;
- the public PyPI project still exposes `0.2.0` as the current release;
- `.github/workflows/pypi.yml` already provides manual
  `workflow_dispatch`, `publish=false` rehearsal, `publish=true`
  Trusted Publishing, tag/version validation, live security preflight, the
  18-wheel matrix, sdist validation, and exact 19-distribution cardinality.

Known documentation/control-surface drift at this baseline:

- `README.md` still says Python 3.15 wheels are pending rehearsal;
- `docs/releases/process.md` says a GitHub Release is optional;
- `plans/README.md`, `plans/registry.md`, and the release-verification
  roadmap still describe M001A as ready and M002 as blocked even though both
  have closure records;
- the superseded M001B plan does not require a GitHub Release.

## 4. Invariants that must not regress

- Never move, delete, or reuse `v0.2.0`.
- Never create or move `v0.2.1` until the final refreshed candidate has
  passed its required gates and renewed build-only rehearsal.
- All six crates and the PyPI distribution remain coordinated at `0.2.1`.
- The exact final candidate SHA must equal the renewed rehearsal
  `head_sha`, the signed tag target, and the PyPI publish workflow
  `head_sha`.
- Documentation-only candidate refresh does not authorize runtime, API,
  dependency, feature-default, compatibility-profile, validation-script, or
  workflow-semantic changes.
- If executable or qualification inputs change, stop and apply ADR-0003
  requalification before release.
- crates.io publication remains manual from a trusted maintainer environment.
- PyPI publication remains the existing manual `pypi.yml` dispatch with
  OIDC Trusted Publishing and protected-environment approval.
- GitHub Release creation is manual and must reuse the already-pushed signed
  tag; it must not create or retarget release identity.
- `eggfetch-bench` and `fuzz/` are never published.
- Registry immutability is respected. A partial publication is recorded
  truthfully; versions are never overwritten to simulate atomicity.

## 5. Scope

### In scope

- correct release-facing documentation that is already stale after M002;
- refresh the exact `0.2.1` candidate with docs/planning-only changes;
- rerun the build-only 18-wheel + 1-sdist rehearsal on the refreshed SHA;
- run immediate release/security/package preflight;
- publish six crates.io packages in dependency order;
- create and push a signed `v0.2.1` tag on the exact candidate;
- publish PyPI `eggfetch 0.2.1` from that tag using `publish=true`;
- create the GitHub Release object for `v0.2.1`;
- verify all three public release surfaces;
- reconcile release/process/planning documentation and write closure evidence.

### Explicitly out of scope

- runtime or public API changes;
- dependency upgrades;
- CI redesign or new automatic publication;
- changing the PyPI platform/interpreter matrix;
- abi3/abi3t or free-threaded Python wheels;
- adding GitHub binary assets merely to make the Release page non-empty;
- Node npm publication;
- HTTPX 1.0, H3 graduation, or unrelated roadmap work;
- moving historical tags or republishing an immutable registry version.

## 6. Required production changes

No production-code change is expected.

The only pre-release repository edits allowed are documentation, changelog
date/link truth if needed, and planning-status reconciliation. In particular:

- remove the stale "3.15 wheels pending the build-only rehearsal" qualifier
  from `README.md`;
- update `docs/releases/process.md` so the coordinated release procedure
  requires a manual GitHub Release tied to the signed tag, and place GitHub
  Release creation after successful PyPI verification;
- update `docs/verification-policy.md` only as needed to distinguish
  "no automatic workflow creates releases" from "a coordinated public
  release includes a manual GitHub Release";
- verify the `CHANGELOG.md` `0.2.1` date and compare links match the actual
  release date/tag; change only if needed;
- scan authoritative release docs for stale M001A/M002/pending-rehearsal or
  "GitHub Release optional" language;
- keep historical closure records and historical release notes unchanged.

Because the root README is consumed as the PyPI long description, this
documentation correction is candidate material and therefore triggers the
fresh-candidate/rehearsal sequence below.

## 7. Ordered work packages

### WP1 — Public-version and release-identity preflight

Before modifying release-facing docs, verify:

- `v0.2.1` is absent remotely;
- no GitHub Release for `v0.2.1` exists;
- crates.io does not already contain any unexpected `0.2.1` state for the
  six coordinated packages, except state explicitly created by this release
  attempt;
- PyPI does not already contain `eggfetch 0.2.1`;
- the local checkout descends from the closed M001A/M002 line and contains no
  unreviewed executable changes.

If any `0.2.1` public state exists unexpectedly, stop and classify it before
continuing.

### WP2 — Documentation truth refresh and final candidate freeze

Make only the documentation/planning corrections listed in §6.

Run a focused truth scan such as:

```bash
rg -n 'pending.*rehearsal|GitHub Release is optional|M001A.*sole ready|M002.*blocked' \
  README.md docs plans
```

Classify historical matches rather than blindly rewriting them.

Then run from a clean worktree:

```bash
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
git diff --check
python scripts/validate_release_versions.py
python scripts/validate_publishable_internal_dependencies.py
```

Audit the delta from
`41757569123c0b8038550b956d8b244ab55094a6` to the refreshed candidate.
It must be documentation/planning/release-metadata only. If the audit finds
runtime source, tests with changed semantics, validation scripts, dependency
versions/sources, feature defaults, compatibility profiles, or PyPI workflow
semantics, stop and requalify under ADR-0003.

Commit the docs-only release truth pass and record its exact SHA as
`FINAL_CANDIDATE_SHA`. After freeze, do not fold any further change into
that candidate.

### WP3 — Renew build-only PyPI rehearsal on the final candidate

Dispatch `.github/workflows/pypi.yml` from a ref resolving exactly to
`FINAL_CANDIDATE_SHA` with:

```text
publish=false
```

Require:

- workflow `head_sha == FINAL_CANDIDATE_SHA`;
- all 18 wheels green;
- all three CPython 3.15 rows green;
- one sdist green and rebuild/install smoke green;
- exactly 19 distributions assembled;
- wheel-coverage validator green;
- `twine check` green;
- publish job skipped;
- exact run ID recorded.

This renewed run supersedes M002's old candidate binding for publication
purposes; the original M002 closure remains valid historical evidence.

### WP4 — Immediate publication preflight

From a trusted maintainer checkout of `FINAL_CANDIDATE_SHA`, with a clean
worktree:

```bash
./scripts/check.sh extended
./scripts/check.sh package
./scripts/check_security.sh
git diff --check
```

Verify versions and internal dependency requirements are still exactly
`0.2.1`. Verify the renewed rehearsal still points at this same SHA.

The live security preflight must run immediately before crates.io
publication. Do not substitute the earlier rehearsal or candidate scan.

### WP5 — crates.io publication

Publish in dependency order:

```text
eggfetch-http-connect 0.2.1
eggfetch-core         0.2.1
eggfetch-cli          0.2.1
eggfetch-ffi          0.2.1
eggfetch-python       0.2.1
eggfetch-node         0.2.1
```

For each dependent crate:

1. confirm already-published internal prerequisites resolve from crates.io;
2. run the full `cargo publish --dry-run -p <crate>` against registry
   resolution;
3. publish manually;
4. verify exact `0.2.1` visibility/resolution before moving to dependents.

Use observed registry availability, not fixed sleeps.

### WP6 — Signed tag

Only after all six crates are visible and resolvable, create the signed tag
on the exact final candidate:

```bash
git tag -s v0.2.1 "${FINAL_CANDIDATE_SHA}" -m "Release v0.2.1"
git push origin v0.2.1
```

Verify the remote tag dereferences to `FINAL_CANDIDATE_SHA`. Do not create
the GitHub Release object yet.

### WP7 — PyPI publication

Dispatch `.github/workflows/pypi.yml` from the `v0.2.1` tag with:

```text
publish=true
```

Require:

- workflow `head_sha == FINAL_CANDIDATE_SHA`;
- tag/version validation green;
- workflow live security preflight green;
- all 18 wheels + 1 sdist rebuilt and validated;
- assemble job green before environment approval;
- protected `pypi` environment approval occurs only after assembled-set
  validation;
- Trusted Publishing/OIDC succeeds;
- all 19 distributions publish without skip/overwrite behavior.

Record the workflow run ID and PyPI attestation/source identity.

### WP8 — Public GitHub Release

After PyPI publication and initial registry smoke are green, create a manual
GitHub Release using the existing signed `v0.2.1` tag.

The release must be:

- tag: `v0.2.1`;
- title: `v0.2.1`;
- non-draft;
- non-prerelease;
- based on release notes derived from the `CHANGELOG.md` `0.2.1` entry;
- explicit that the release is coordinated across GitHub, crates.io, and
  PyPI;
- linked to crates.io and PyPI package pages;
- truthful about Python 3.10–3.15 wheel coverage and the interpreter used for
  the pre-GA 3.15 rehearsal where relevant.

A command such as the following is acceptable when the maintainer's GitHub
CLI authentication is trusted:

```bash
gh release create v0.2.1 --verify-tag --title "v0.2.1" --notes-file <notes-file>
```

The GitHub UI is also acceptable. Do not create a second tag through the
Release UI.

No attached binary assets are required unless a separate distribution plan
explicitly requires them; GitHub's source archives plus registry links are
sufficient for this milestone.

### WP9 — External smoke and documentation/registry closeout

Verify:

- all six crates.io packages resolve at `0.2.1` without path/git overrides;
- a disposable Rust consumer resolves/builds
  `eggfetch-core = "=0.2.1"`;
- `pip install eggfetch==0.2.1` works in a clean supported environment;
- import/version smoke passes;
- PyPI exposes exactly the intended 18 wheels + 1 sdist;
- representative Python 3.15 installation succeeds when current interpreter
  tooling permits it;
- GitHub Release `v0.2.1` exists, is published, and points through the
  signed tag to `FINAL_CANDIDATE_SHA`;
- refreshed README/release docs contain no stale pending-rehearsal or
  optional-GitHub-Release language.

Then create the closure record and reconcile the active planning control
surfaces. Historical M001A/M002 closure records remain immutable.

## 8. Failure, cancellation, and partial-publication semantics

Before any registry publication, any failure is fail-closed: fix the
documentation-only defect, freeze a new candidate, and rerun WP2–WP4 as
needed.

After crates.io publication begins, registry immutability controls recovery:

- never retry an already-published crate version as if it were absent;
- do not yank merely to make the release appear atomic;
- record partial state exactly;
- continue only if remaining artifacts are byte/content compatible with the
  frozen `0.2.1` candidate;
- if package contents must change, stop and plan a new patch release.

If the signed tag is pushed but PyPI fails, do not move the tag. Diagnose
whether PyPI can safely be retried from the same immutable tag. If source
contents must change, `0.2.1` is consumed and a new patch version is
required.

If PyPI succeeds but GitHub Release creation fails, do not republish or
retag. Retry only GitHub Release creation against the existing signed tag.

If the GitHub Release is accidentally created as draft, it may be published
after verifying tag identity. If it points to the wrong tag/SHA, stop; do not
retarget an already-public release identity without an explicit corrective
decision.

## 9. Compatibility and migration

No public Rust, Python, CLI, C ABI, HTTPX facade, feature-profile, protocol,
timeout, retry, pooling, or dependency behavior may change.

The candidate refresh is documentation/planning-only. Stage C remains bound
to `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa` only if the final delta audit
proves no executable or qualification-input drift.

The package minimums remain Rust 1.89 and Python 3.10. The release matrix
remains CPython 3.10–3.15 on Linux x86_64, macOS arm64, and Windows x86_64.

## 10. Required tests

No new behavioral test is expected.

Required release correctness evidence is provided by existing validators and
smokes:

- release-version validator;
- publishable-internal-dependency validator;
- routine/extended/package checks;
- live dependency-security preflight;
- 18-wheel matrix build/install/smoke;
- sdist rebuild/install smoke;
- wheel-coverage validator;
- `twine check`;
- clean external crates.io consumer build;
- clean external PyPI install/import;
- GitHub tag/release identity verification.

If a documentation change breaks link/example validation, fix the
documentation; do not weaken the validator.

## 11. Required verification commands

At minimum:

```bash
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
git diff --check
python scripts/validate_release_versions.py
python scripts/validate_publishable_internal_dependencies.py
```

Immediately before crates.io publication:

```bash
./scripts/check_security.sh
```

The build-only and publish PyPI workflow runs are separate evidence and must
be recorded with exact run IDs and head SHAs.

## 12. Documentation updates

Pre-release candidate documentation:

- `README.md`: remove the completed-rehearsal pending qualifier;
- `docs/releases/process.md`: make the manual GitHub Release a required
  coordinated-release step and document its ordering;
- `docs/verification-policy.md`: preserve the prohibition on automatic
  release creation while clarifying the manual GitHub Release expectation;
- `CHANGELOG.md`: verify release date and compare links.

Post-publication planning/closure documentation:

- `plans/registry.md`;
- `plans/README.md`;
- `plans/subsystems/release-verification-roadmap.md`;
- M001 umbrella status;
- this milestone's closure record;
- any legacy release-status record that is active solely because public
  artifact availability was pending.

Do not rewrite historical release notes or closure evidence merely to remove
old version numbers.

## 13. Acceptance criteria

- [x] release-facing doc truth pass is complete before final candidate freeze;
- [x] final candidate delta from the original M001A candidate is
      docs/planning/release-metadata only;
- [x] Tier 1, Tier 2, Tier 3, release validators, and `git diff --check`
      are green on the final candidate;
- [x] renewed `publish=false` run is green on exactly the final candidate;
- [x] renewed rehearsal builds 18/18 wheels + 1 sdist and publishes nothing;
- [x] live security preflight is green immediately before crates.io publish;
- [x] all six crates.io `0.2.1` packages publish in dependency order and
      resolve from the public registry;
- [x] signed `v0.2.1` exists and dereferences to the final candidate;
- [x] PyPI `publish=true` runs from that exact tag/SHA;
- [x] all 19 PyPI distributions validate and publish through OIDC;
- [x] clean crates.io and PyPI consumer smokes pass;
- [x] published GitHub Release `v0.2.1` exists and reuses the signed tag;
- [x] GitHub Release is non-draft/non-prerelease and release notes are
      consistent with `CHANGELOG.md`;
- [x] final candidate SHA == renewed rehearsal head SHA == signed tag target
      == PyPI publish head SHA;
- [x] README/release-process docs no longer claim M002 is pending or GitHub
      Release is optional;
- [x] M001 umbrella closure is written;
- [x] active registry/roadmap/README contain no stale M001A/M002 execution
      gate.

## 14. Stop conditions

Stop and do not improvise if:

- an unexpected public `0.2.1` version/tag/release exists before this
  release attempt;
- the docs truth pass requires runtime/API/dependency/workflow-semantic
  changes;
- the refreshed-candidate delta cannot be proven docs/planning-only;
- any required local gate, renewed rehearsal row, security scan, package
  validation, or registry smoke is red;
- tag signing cannot be verified;
- the remote tag resolves to any SHA other than the final candidate;
- PyPI environment/OIDC/tag validation fails in a way requiring source
  changes;
- partial publication requires different package contents;
- continuing would require moving a tag or overwriting an immutable version.

## 15. Closure evidence required

Create:

`plans/closure/release-verification/004-0-2-1-tagged-release-finalization.md`

Record at minimum:

- original M001A candidate SHA;
- final refreshed candidate SHA;
- candidate delta classification;
- Tier 1/2/3 and security-preflight outcomes;
- renewed build-only workflow run ID/head SHA and 18+1 results;
- each crates.io package/version and verification result;
- signed tag verification and dereferenced commit;
- PyPI publish run ID/head SHA, 19-file result, OIDC/attestation identity;
- GitHub Release URL/ID, draft/prerelease flags, and tag identity;
- external Rust/Python smoke commands and results;
- documentation truth-scan result;
- partial failures/retries, if any;
- Stage C disposition;
- final M001 umbrella/registry/roadmap status.

Closure record: `plans/closure/release-verification/004-0-2-1-tagged-release-finalization.md`.

## 16. Handoff notes

This is now the sole executable release task.

The original M001A/M002 evidence remains useful historical qualification, but
publication must bind to the refreshed candidate because the PyPI README
truth fix changes package-source bytes. Do not shortcut the renewed
build-only rehearsal merely because the change is documentation-only.

The old M001B plan is superseded before execution. Its publication-order and
partial-publication guidance are incorporated here, with the additional
requirements for truthful package documentation and a real GitHub Release
object.
