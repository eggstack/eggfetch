# Issue #24 — release, qualification rebinding, and closure pass

Status: **active / ready for handoff**

Parent corrective: `issue-24-streaming-decompression-chunk-boundary-corrective.md`

Issue: [#24](https://github.com/eggstack/eggfetch/issues/24)

Corrective executable freeze:
`37ab02b3873a4f0ce7018bd716e326bcf0595230`

Prepared coordinated release commit:
`0ddcecc5e59e82c0cbf2d3d0647fd58b3d534dff`

Planning/closure baseline:
`29e8c1c52298341699d28485195d74ada7449144`

Target coordinated release: **0.1.9**

## Objective

Close the already-implemented issue #24 streaming-decompression corrective without reopening its code path.

This pass owns the remaining release and evidence work only:

1. prove the prepared 0.1.9 release commit contains no unqualified executable/test/feature/dependency drift relative to the qualified freeze;
2. renew the canonical HTTPX 0.28.1 and HTTPX2 2.12.0 exact-SHA records to the issue #24 executable freeze;
3. perform the coordinated crates.io publication in the repository's required dependency order;
4. create and push the signed `v0.1.9` tag;
5. run the required PyPI build-only rehearsal and, if clean, the explicit publication dispatch;
6. verify registry/package availability;
7. normalize the parent corrective and plan index from active/pending to completed;
8. close issue #24 only after the published artifact containing the fix is externally consumable;
9. leave eggsearch workaround removal to a separate downstream dependency-bump/corrective pass.

No production source refactor, API change, dependency upgrade, feature change, timeout change, decoder change, or new CI/release automation belongs in this pass.

## Current state at handoff

The implementation has already landed correctly at
`37ab02b3873a4f0ce7018bd716e326bcf0595230`.

Recorded evidence in the parent corrective includes:

- deterministic baseline-red reproduction against 0.1.8 baseline `9ecdd04`;
- one-item and buffered controls green on the baseline;
- fragmented gzip/Brotli red on the baseline;
- raw/decompression-disabled transport green on the baseline;
- post-fix 20/20 streaming decompression regression tests green;
- fragmented gzip, Brotli, deflate and zstd green;
- empty-source-chunk behavior green;
- awkward gzip framing green;
- decoded-size and decompression-ratio limits green;
- malformed/unsupported error-kind compatibility green;
- high-level `Content-Length` and HTTP/1.1 chunked gzip/Brotli green;
- raw chunked encoded bytes exact;
- public `ResponseBody` exhaustive-shape compatibility green;
- timeout/pool/native/lean-route regressions green;
- Tier 1, extended, package, security and Rust 1.89.0 MSRV recorded green;
- both API oracles recorded green;
- three consecutive 1,871-test compatibility runs recorded green.

The coordinated 0.1.9 release metadata is already prepared in
`0ddcecc5e59e82c0cbf2d3d0647fd58b3d534dff`.

At planning time, the remaining inconsistencies are:

- no `v0.1.9` Git tag exists;
- crates.io/PyPI publication is not recorded complete;
- `compat/httpx/0.28.1/profile.toml` still binds to
  `82f3f38631b44a9a5c5ec5b40790e5015aeb40f8`;
- `compat/httpx2/2.12.0/profile.toml` still binds to the same prior freeze;
- `plans/httpx-parity-correction-status.md` still describes the
  2026-09-18 total-deadline freeze as current;
- the parent issue #24 plan header still says `active / ready for handoff`;
- `plans/README.md` correctly says implementation/qualification are complete
  but publication and profile/ledger renewal remain pending;
- GitHub issue #24 remains open, which is correct until a published fixed
  artifact exists.

## Freeze and descendant policy

### Authoritative executable freeze

Keep
`37ab02b3873a4f0ce7018bd716e326bcf0595230`
as the issue #24 executable/test qualification freeze unless this pass discovers
that a later commit changed executable/test/feature/dependency semantics.

Do not move the freeze merely because release metadata, compatibility profiles,
plan records, tags, or registry publication occur later.

### Release commit audit

Before publication, compare:

```text
37ab02b3873a4f0ce7018bd716e326bcf0595230
    ..
0ddcecc5e59e82c0cbf2d3d0647fd58b3d534dff
```

Classify every changed file.

Expected bounded release-only delta:

- coordinated crate versions;
- Python package version;
- lockfile package-version self references if required by the coordinated bump;
- `CHANGELOG.md`;
- release-facing metadata required solely for 0.1.9 identity.

The release commit may remain a descendant of the qualified executable freeze
only if the audit proves there is **no** change to:

- production Rust/Python/Node/FFI behavior;
- tests or qualification fixtures;
- Cargo dependency versions/sources;
- feature definitions or defaults;
- `rust-version` / MSRV;
- compiler/profile/build flags;
- compression behavior;
- timeout/body/pool behavior;
- public API surfaces;
- packaging workflow semantics beyond version identity;
- HTTPX/HTTPX2 compatibility behavior.

If any unexpected executable/test/build-policy change exists, stop publication.
Create a new executable freeze and rerun the required qualification instead of
silently treating the old evidence as current.

## Part A — reconcile exact-SHA compatibility records

The compatibility tests and API oracles have already been run against the issue
#24 freeze; the canonical records must now reflect that evidence.

Update:

- `compat/httpx/0.28.1/profile.toml`;
- `compat/httpx2/2.12.0/profile.toml`;
- `plans/httpx-parity-correction-status.md`.

For each profile:

1. mark the preceding `82f3f386...` qualification as historical;
2. bind `qualification-sha` to
   `37ab02b3873a4f0ce7018bd716e326bcf0595230`;
3. set the qualification date to 2026-09-19;
4. set the immediate previous qualification SHA to
   `82f3f38631b44a9a5c5ec5b40790e5015aeb40f8`;
5. preserve the existing Stage C designation;
6. record issue #24 as a private streaming-decompression implementation
   correction with no HTTPX facade/API semantic delta;
7. do not alter allowed-difference inventories merely to make counts match.

Update the live ledger to state exactly what changed and what did not:

- private compressed-stream adapter replaced;
- public APIs unchanged;
- HTTPX timeout semantics unchanged;
- HTTPX/HTTPX2 decompression behavior remains compatible;
- API oracle counts remain 71 and 79 respectively with zero unexplained,
  stale or resolved-active differences;
- three consecutive full pinned compatibility runs passed 1,871 tests;
- H3 and Node experimental status is unchanged.

The profile/ledger update is documentation/evidence binding. It must not modify
runtime code or tests.

### Acceptance

- [ ] Both profiles bind to `37ab02b...`.
- [ ] Both retain `stage-c-qualified` / `qualified`.
- [ ] `82f3f386...` is explicitly historical, not erased.
- [ ] Ledger counts match the evidence recorded in the parent corrective.
- [ ] No compatibility exception is added for issue #24.
- [ ] Profile/ledger changes are evidence-only descendants.

## Part B — pre-publication release audit and validation

Operate from the prepared 0.1.9 release commit
`0ddcecc5e59e82c0cbf2d3d0647fd58b3d534dff`
or a documentation-only descendant whose tree is otherwise publication-equivalent.

First verify coordinated version identity:

- `eggfetch-http-connect == 0.1.9`;
- `eggfetch-core == 0.1.9`;
- `eggfetch-cli == 0.1.9`;
- `eggfetch-ffi == 0.1.9`;
- `eggfetch-python == 0.1.9`;
- `eggfetch-node == 0.1.9`;
- Python distribution version == 0.1.9;
- internal dependency constraints reference the intended coordinated version;
- changelog contains the issue #24 correction without overstating scope.

Re-run the release-time gates required by current policy immediately before
publication:

```sh
./scripts/check.sh
./scripts/check.sh package
./scripts/check_security.sh
```

Extended qualification does not need to be gratuitously repeated if the
release-commit audit proves the executable/test tree remains represented by the
already-qualified freeze and no time-sensitive extended prerequisite changed.
If the audit finds meaningful build/test/executable drift, rerun
`./scripts/check.sh extended` and establish a new freeze instead.

Do not weaken or skip the live security preflight because it was green earlier;
it is intentionally time-sensitive.

### Acceptance

- [ ] Release commit diff is classified and bounded.
- [ ] All six publishable crates and Python metadata are exactly 0.1.9.
- [ ] Changelog accurately describes the chunk-boundary correction.
- [ ] Tier 1 passes immediately before publication.
- [ ] Package validation passes immediately before publication.
- [ ] Live security preflight passes immediately before publication.
- [ ] No new executable freeze is needed, or a new one is established and fully qualified if required.

## Part C — crates.io coordinated publication

Follow `docs/releases/process.md` exactly.

Publish manually from a trusted maintainer environment in dependency order:

```sh
cargo publish -p eggfetch-http-connect
# verify registry visibility/resolution

cargo publish -p eggfetch-core
# verify registry visibility/resolution

cargo publish -p eggfetch-cli
cargo publish -p eggfetch-ffi
cargo publish -p eggfetch-python
cargo publish -p eggfetch-node
```

Do not use fixed sleeps as a substitute for checking crates.io propagation.

After every publish, verify the actual 0.1.9 package is visible/resolvable before
moving to a dependent crate when dependency ordering requires it.

If publication becomes partial:

- do not overwrite/yank merely to make the sequence look atomic;
- record exactly which packages published;
- correct any release defect with a new patch version where required;
- never claim coordinated 0.1.9 completion while one required crate is missing.

At minimum verify `eggfetch-core 0.1.9` is publicly resolvable before declaring
the downstream bug fix consumable.

### Acceptance

- [ ] eggfetch-http-connect 0.1.9 published and visible.
- [ ] eggfetch-core 0.1.9 published and visible.
- [ ] eggfetch-cli 0.1.9 published and visible.
- [ ] eggfetch-ffi 0.1.9 published and visible.
- [ ] eggfetch-python 0.1.9 published and visible.
- [ ] eggfetch-node 0.1.9 published and visible.
- [ ] Publication order and any propagation waits are recorded truthfully.

## Part D — signed tag and PyPI release

Only after successful crates.io publication, create and push the signed tag:

```sh
git tag -s v0.1.9 -m "Release v0.1.9"
git push origin v0.1.9
```

Verify that `v0.1.9` resolves to the intended coordinated release commit.

Then use the manual `.github/workflows/pypi.yml` workflow from the
**v0.1.9 tag**.

### Required first dispatch: build-only rehearsal

Run:

```text
publish=false
```

Verify the expected current matrix:

- Linux x86_64: CPython 3.10–3.15;
- macOS arm64: CPython 3.10–3.15;
- Windows x86_64: CPython 3.10–3.15;
- 18 wheels total;
- 1 sdist;
- 19 distributions assembled.

This build-only run also closes the separately pending Python 3.15 wheel
production rehearsal if and only if all expected artifacts are actually
produced and validated. Record that linkage explicitly rather than silently
marking the other plan complete.

### Publication dispatch

After inspecting the rehearsal artifacts, dispatch the same tag with:

```text
publish=true
```

Approve the protected `pypi` environment deployment when prompted.

Verify:

- PyPI shows `eggfetch 0.1.9`;
- representative installation succeeds from PyPI;
- wheel filenames/tags match the expected platform/Python matrix;
- no stale 0.1.8 metadata appears in the installed package.

A GitHub Release is optional and must not be treated as a closure gate.

### Acceptance

- [ ] Signed `v0.1.9` tag exists and resolves to the intended release commit.
- [ ] Build-only PyPI rehearsal succeeds from that exact tag.
- [ ] 18 wheels + 1 sdist are assembled.
- [ ] Python 3.15 rows are present on all three configured platforms.
- [ ] Publish dispatch succeeds.
- [ ] PyPI 0.1.9 is externally installable.
- [ ] Any Python 3.15 plan status update is evidence-backed.

## Part E — post-publication issue #24 smoke proof

After crates.io publication, verify the **published** `eggfetch-core 0.1.9`,
not a path/git checkout.

Use a small disposable consumer or equivalent clean registry-resolved fixture
that depends on:

```toml
eggfetch-core = "=0.1.9"
```

Enable the same relevant HTTP/compression features needed by the issue
reproducer.

Run a bounded loopback proof using the published crate:

- gzip + `Content-Length` decodes to plaintext;
- gzip + HTTP/1.1 chunked decodes to identical plaintext;
- Brotli + `Content-Length` decodes to plaintext;
- Brotli + HTTP/1.1 chunked decodes to identical plaintext;
- raw/decompression-disabled chunked mode returns exact encoded bytes.

This is not a new product test suite. It is a release smoke proving the
registry artifact actually contains the qualified correction.

If registry-resolved 0.1.9 fails while the repository freeze passes, stop
closure and investigate package/release composition before closing the issue.

### Acceptance

- [ ] Clean external consumer resolves crates.io `eggfetch-core =0.1.9`.
- [ ] Published gzip Content-Length/chunked parity passes.
- [ ] Published Brotli Content-Length/chunked parity passes.
- [ ] Published raw mode remains exact.
- [ ] No git/path override is present in the smoke consumer.

## Part F — normalize parent plan and plan index

After publication and smoke verification, update the parent:

`plans/issue-24-streaming-decompression-chunk-boundary-corrective.md`

Change its header/status from active/handoff language to completed, and append
the publication evidence without rewriting the historical implementation
record.

Record:

- final executable freeze SHA;
- release commit SHA;
- coordinated published version;
- crates.io status for all six crates;
- `v0.1.9` tag SHA/target;
- PyPI rehearsal run/result;
- PyPI publication run/result;
- compatibility profile/ledger rebinding;
- external registry-resolved smoke result;
- issue closure;
- downstream eggsearch handoff status.

Update `plans/README.md`:

- move issue #24 from **Active corrective** to **Completed corrective**;
- state that the implementation freeze is `37ab02b...`;
- state that coordinated 0.1.9 is published;
- state that both Stage C profiles are rebound to the issue #24 freeze;
- state the exact registry/tag/PyPI outcome;
- retain any genuine limitation instead of implying it vanished.

If the Python 3.15 build-only rehearsal completed successfully during this
release, update the Python 3.15 plan/index status in the same documentation-only
closure pass using the actual workflow evidence.

Do not edit executable/test/build files during the final documentation closure.

## Part G — close GitHub issue #24

Close issue #24 only when all of these are true:

1. `eggfetch-core 0.1.9` is published on crates.io;
2. the registry-resolved smoke reproducer passes;
3. the canonical compatibility profiles/ledger point to the correct freeze;
4. the parent plan and plan index truthfully record closure.

Add a concise closure comment containing:

- fixing commit `37ab02b...`;
- released version `eggfetch-core 0.1.9`;
- root cause: private stream-to-`AsyncRead` adapter re-emitted consumed
  compressed chunks / mishandled empty chunks;
- fix: `tokio_util::io::StreamReader` with existing `BufReader` retained;
- regression coverage: fragmented codecs plus high-level Content-Length vs
  chunked gzip/Brotli;
- downstream note: consumers can adopt 0.1.9.

Do not close the issue merely because the repository code is fixed.

## Part H — downstream eggsearch handoff boundary

Eggsearch is explicitly out of scope for this repository pass.

Once `eggfetch-core 0.1.9` is published and the registry smoke is green,
record that eggsearch may begin its own dependency-bump/workaround-removal pass.

The downstream pass should:

- bump to the published eggfetch version;
- remove the narrow HTML-engine `.decompress(false)` workaround;
- restore normal automatic decompression only where the workaround was added;
- run its own live/loopback engine coverage;
- avoid adding a second local decompression stack.

Do not modify eggsearch from this plan.

## Files expected to change

Evidence/closure only:

- `compat/httpx/0.28.1/profile.toml`;
- `compat/httpx2/2.12.0/profile.toml`;
- `plans/httpx-parity-correction-status.md`;
- `plans/issue-24-streaming-decompression-chunk-boundary-corrective.md`;
- this plan;
- `plans/README.md`;
- possibly the Python 3.15 wheel plan/index entry if the 18-wheel rehearsal
  completes during this release.

Tag/registry state changes occur outside repository file content.

Unexpected changes to these paths are stop-and-review signals:

- `crates/eggfetch-core/src/**`;
- `crates/eggfetch-core/tests/**`;
- dependency versions/sources;
- Cargo feature definitions;
- `rust-version`;
- request/response/public API definitions;
- `.github/workflows/**` other than executing the existing manual PyPI
  workflow.

## Explicit non-goals

Do not use this pass to:

- refactor compression code;
- remove the retained `BufReader`;
- upgrade `tokio-util`, Tokio, async-compression, flate2, brotli or zstd;
- alter decoded chunk sizing;
- redesign body/error/timeout APIs;
- change HTTPX or HTTPX2 facade semantics;
- add CI jobs or release automation;
- add provenance/SBOM/evidence-manifest infrastructure;
- reopen HTTP/3 graduation;
- mature the Node prototype;
- change Python wheel targets beyond executing the already-planned 3.15
  matrix;
- modify eggsearch.

## Completion criteria

This closure pass is complete only when:

- [ ] `37ab02b...` remains the truthful executable/test freeze, or a newly
  qualified freeze supersedes it for an explicitly recorded reason.
- [ ] The 37ab→0dd release delta is audited as release-only metadata/version
  change.
- [ ] HTTPX 0.28.1 profile binds to the issue #24 freeze.
- [ ] HTTPX2 2.12.0 profile binds to the issue #24 freeze.
- [ ] Live parity ledger describes the issue #24 qualification.
- [ ] Tier 1 is green immediately before publication.
- [ ] Package validation is green immediately before publication.
- [ ] Live security preflight is green immediately before publication.
- [ ] All six coordinated crates are published as 0.1.9.
- [ ] `eggfetch-core 0.1.9` is externally resolvable from crates.io.
- [ ] Signed `v0.1.9` tag exists and targets the intended release commit.
- [ ] PyPI build-only rehearsal succeeds.
- [ ] Expected 18-wheel + 1-sdist matrix is verified.
- [ ] PyPI 0.1.9 publication succeeds and is installable.
- [ ] Registry-resolved issue #24 gzip/Brotli smoke passes.
- [ ] Parent issue #24 plan is marked complete with publication evidence.
- [ ] Plan index moves the corrective to completed.
- [ ] GitHub issue #24 is closed with fixing commit/version/root-cause summary.
- [ ] Eggsearch handoff points to published 0.1.9 but does not modify eggsearch.
- [ ] Final closure descendant changes only evidence/documentation/profile
  records.

## Closure record template

Append when complete:

```text
Parent corrective:
  issue-24-streaming-decompression-chunk-boundary-corrective.md

Qualified executable freeze:
Prepared release commit:
Release-delta audit:
Unexpected executable/test/build delta:

HTTPX 0.28.1 profile:
HTTPX2 2.12.0 profile:
Parity ledger:
API oracle counts:
Full compat runs:

Pre-publication Tier 1:
Pre-publication package:
Pre-publication security:
Extended rerun required?:
MSRV status:

Published version:
eggfetch-http-connect crates.io:
eggfetch-core crates.io:
eggfetch-cli crates.io:
eggfetch-ffi crates.io:
eggfetch-python crates.io:
eggfetch-node crates.io:

Signed tag:
Tag target:

PyPI rehearsal run:
Rehearsal wheel count:
Rehearsal sdist count:
Python 3.15 matrix:
PyPI publication run:
PyPI install smoke:

Registry-resolved eggfetch-core source:
Published gzip Content-Length:
Published gzip chunked:
Published Brotli Content-Length:
Published Brotli chunked:
Published raw chunked:

Parent plan normalized:
Plan index normalized:
Python 3.15 plan linkage:
Issue #24 closure comment:
Issue #24 state:

eggsearch handoff version:
eggsearch workaround removal status:

Final documentation/profile descendant SHA:
Known limitations:
```

Missing evidence remains missing. A version bump commit, green repository tests,
or a successful tag alone is not evidence that the registry artifact contains
the fix.

## Exit criterion

Issue #24 is genuinely closed when the qualified private-adapter correction is
bound into the canonical compatibility records, coordinated eggfetch 0.1.9 is
published and tagged, the published `eggfetch-core 0.1.9` passes the
registry-resolved chunked gzip/Brotli smoke proof, PyPI release state is
truthfully recorded, the parent/index are normalized, and the downstream
eggsearch team can depend on a published fixed version rather than an
unpublished commit.
