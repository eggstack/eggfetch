# Security Policy, Release, and Supply-Chain Hardening

Planning baseline: `025b1a5a6a94b017b3f3b183c3d562e7ee9bcca7` (`main`, 2026-09-15; eggfetch 0.1.4)
Parent program: `plans/post-audit-maintenance-security-and-proxy-modernization-program.md`
Normative verification policy: `docs/verification-policy.md`

## Objective

Make eggfetch's dependency-security and release-security claims match actual enforcement, remove obsolete advisory exceptions, and add a fail-closed release/security preflight without undoing the repository's deliberate local-first CI simplification.

The target is not “more CI.” The target is a small, auditable security path that runs when explicitly requested and before publication, while routine push/PR validation remains the single existing Ubuntu job.

## Current findings

### Documentation contradicts implementation

`SECURITY.md` currently states that:

- `cargo-deny` and `cargo-audit` are wired into CI and run on every push;
- unsafe code is forbidden workspace-wide.

Neither claim describes the current repository. `docs/verification-policy.md` is normative and intentionally keeps one routine workflow; `docs/architecture/dependency-policy.md` describes cargo-deny/audit as manual security-review tools. FFI and Node explicitly permit unsafe code at their ABI boundaries.

The implementation is more defensible than the stale prose. Correct the prose and add a bounded explicit security gate; do not recreate the deleted multi-workflow security system from the pre-simplification era.

### RustSec ignores are stale

`deny.toml` still ignores at least:

- `RUSTSEC-2025-0020`, with a reason saying PyO3 >=0.24.1 is planned;
- `RUSTSEC-2026-0177`, with a reason saying PyO3 >=0.29.0 is planned.

The current lockfile resolves PyO3 0.29.2. The published fix floor for `RUSTSEC-2026-0177` is >=0.29.0. These entries are therefore no longer legitimate exceptions and can hide an accidental future downgrade.

Any advisory exception that remains after a fresh scan must describe the exact dependency path, applicability, mitigation, owner/review trigger, and removal condition.

### PyPI publication is not repository-side tag-gated as documented

The manually dispatched PyPI workflow has several good controls already:

- Actions are commit-SHA pinned;
- default permissions are read-only;
- only the publish job receives `id-token: write`;
- Trusted Publishing/OIDC is used instead of a long-lived PyPI token;
- publication requires the protected `pypi` environment.

However, repository code currently calls:

```sh
python scripts/validate_release_versions.py
```

in `validate-release`, even though that script already supports strict:

```sh
--tag vX.Y.Z --publish
```

The workflow's publish job is gated only on `inputs.publish == true`; it does not itself prove that the selected workflow ref is a `v<SEMVER>` tag matching the package version. Build-only dispatch from branches is useful and should remain supported, but publication must independently fail closed.

### Release tooling can drift between runs

The workflow installs some Python build/release tools with unconstrained `pip install`, including maturin/twine in paths that construct or validate release artifacts. SHA-pinned GitHub Actions reduce one supply-chain axis, but unreviewed tool-version changes can still change artifact behavior between two identical repository SHAs.

Do not freeze every development tool forever. Pin the small set of release-critical tools in one repository-controlled location and update deliberately.

## Design principles

1. Keep the single automatic push/PR workflow.
2. Distinguish deterministic routine regression checks from time-dependent live vulnerability intelligence.
3. Make the explicit security command fail closed when invoked; do not silently skip because tools/databases are missing.
4. Run current advisory checks before publication, when currency matters most.
5. Keep build-only PyPI workflow dispatch useful from branches while making actual publication tag-bound.
6. Prefer repository-owned scripts/configuration to prose-only release instructions.
7. Do not duplicate the same security claim across multiple documents with different meanings.
8. Keep unsafe Rust narrowly scoped and documented instead of claiming it does not exist.

## Required implementation

# 1. Refresh the current dependency advisory state

Run current versions of the repository's chosen security tools against the exact workspace lockfile.

At minimum:

```sh
cargo deny check advisories
cargo deny check licenses bans sources
cargo audit
```

Use a freshly updated advisory database for the live scan. Record the scan date and tool versions in the plan closure/release evidence, not in generated permanent evidence machinery.

Before accepting any ignore:

- confirm the resolved package/version;
- confirm the advisory's affected range;
- inspect whether the vulnerable API/feature is reachable where applicability matters;
- prefer upgrade/removal over ignore;
- if no patched version exists or applicability is demonstrably absent, document the exception and review trigger.

Remove the obsolete PyO3 ignores once the live scan confirms the current lockfile is outside the affected ranges.

Acceptance:

- [x] No ignore remains merely because a historical upgrade used to be pending.
- [x] Every remaining exception has a current, specific rationale.
- [x] The live current lockfile has no unacknowledged known vulnerability finding at plan closure.

# 2. Add one explicit fail-closed security preflight

Create a small repository-owned command, for example:

```text
./scripts/check_security.sh
```

Exact filename is implementation choice, but there must be one canonical entry point.

The command should:

- require `cargo-deny` and `cargo-audit` rather than silently skip them;
- operate on the checked-in lockfile (`--locked` where the tool supports the relevant behavior);
- run advisory, license, bans, and source policy checks with the existing `deny.toml`;
- run the independent cargo-audit RustSec check;
- fail on a stale/unavailable advisory database rather than reporting a false clean result when used as a release gate;
- print tool versions and scan date/time for operator records;
- make no source changes;
- produce no retained evidence artifact by default.

Do not add this command to routine `./scripts/check.sh` if doing so would make every push dependent on a live external advisory database and violate the current deterministic/latency budget.

Decide explicitly whether `./scripts/check.sh extended` should call it. The default target for this plan is **no**: keep the live security preflight a distinct release/security command so the normative verification policy remains truthful about time-dependent network checks.

Acceptance:

- [x] There is one documented security command with fail-closed prerequisites.
- [x] Routine CI remains one workflow/job and does not fetch live vulnerability databases.
- [x] Release documentation requires the security command before crates.io publication.

# 3. Run the security preflight in the manual PyPI publication path

Integrate the security preflight into the existing manually dispatched PyPI workflow's validation stage when publication is requested.

Build-only mode may either run it or skip it explicitly; publication mode must not.

Use a pinned installation mechanism for the audit tools. Acceptable approaches include SHA-pinned upstream actions in the existing manual workflow or exact-version `cargo install --locked` commands. Prefer the option with the lower ongoing maintenance/runner cost while keeping versions reviewable in repository history.

Do not create `.github/workflows/security.yml` or another automatic workflow.

Acceptance:

- [x] `publish=true` cannot reach artifact publication without a successful live dependency-security preflight.
- [x] The security tool versions used by publication are reviewable/pinned.
- [x] Routine push/PR workflow complexity is unchanged.

# 4. Enforce immutable publication identity

Update the manually dispatched PyPI workflow so build-only and publish modes have different identity requirements.

### Build-only mode

`publish=false` may run from a branch, commit, or tag. It validates internal version coherence with the existing normal mode:

```sh
python scripts/validate_release_versions.py
```

### Publish mode

`publish=true` must fail before expensive artifact construction unless all are true:

- `github.ref_type == 'tag'`;
- `github.ref_name` matches the repository's `v<SEMVER>` tag contract;
- `scripts/validate_release_versions.py --tag "$GITHUB_REF_NAME" --publish` succeeds;
- the checked-out commit is the commit identified by the selected tag.

The final publish job should repeat a lightweight ref-type/tag guard in its `if:` expression or an explicit step. Do not rely solely on the protected GitHub Environment because environment rules are account configuration rather than version-controlled repository policy.

Do not weaken the existing environment approval or OIDC controls.

Acceptance:

- [x] A branch dispatch with `publish=false` remains valid.
- [x] A branch dispatch with `publish=true` fails before publication.
- [x] A malformed tag fails.
- [x] A valid tag whose version differs from Cargo/pyproject metadata fails.
- [x] A correct matching tag may proceed to the existing protected environment and OIDC publish step.

# 5. Pin release-critical Python tooling

Inventory Python tools installed dynamically by `.github/workflows/pypi.yml` and any local release command that can change produced distributions or package validation results.

At minimum inspect:

- maturin;
- twine;
- package-validation helpers pulled from PyPI;
- the validation job's Python test/build tools where their behavior can affect release acceptance.

Create one small checked-in constraints/requirements file or equivalent repository-owned version declaration for release-critical tools. Use exact versions; hashes are preferred where practical without making cross-platform wheel resolution brittle.

The wheel-building `PyO3/maturin-action` is already SHA-pinned. Do not install a second unrelated maturin version in the same path without a reason. Align the validation/sdist tool version with the version expected by the build action when practical.

Acceptance:

- [x] Re-running a release workflow at the same repository SHA does not silently pick a newer maturin/twine simply because PyPI changed.
- [x] Tool updates are ordinary reviewed repository diffs.

# 6. Reconcile security documentation

Make `SECURITY.md` describe the final policy accurately.

Required corrections:

- remove the claim that cargo-deny/audit run on every push unless implementation genuinely changes to that model (not the target here);
- state where/when the explicit live security preflight runs;
- describe the difference between routine CI and release security checks;
- replace “safe Rust only / unsafe forbidden workspace-wide” with the actual policy: core/default workspace code forbids unsafe, while FFI/Node ABI boundaries have explicit local allowances and must contain/document unsafe blocks;
- keep private reporting, disclosure, CVE/GHSA, supported-version, redaction, and response-process material that remains true;
- do not promise response automation or release behavior the project cannot guarantee.

Reconcile cross references in:

- `docs/architecture/dependency-policy.md`;
- `docs/verification-policy.md`;
- `docs/releases/process.md`;
- release checklist(s) if present.

Choose one normative statement for each policy and link to it from other documents rather than copying conflicting prose.

# 7. Advisory-ignore governance

Update `deny.toml` comments/policy so future ignores have a minimum standard.

Each ignore should carry or link to:

- advisory ID;
- affected package/path;
- why upgrade/removal is not currently possible;
- why the advisory is accepted in eggfetch's actual use;
- a review/removal trigger (patched release, dependency removal, date, or feature change).

Do not create an issue solely because a comment can capture a short-lived exception, but long-lived/high-severity exceptions should have an externally trackable owner if they ever become necessary.

# 8. Keep fuzzing claims truthful

The repository has a substantial cargo-fuzz target set, but routine CI does not run fuzz campaigns. Documentation should say exactly that.

Do not add indefinite fuzzing to the single push workflow. If release/security docs require fuzzing, define a bounded/manual command or refer to recorded targeted campaigns rather than implying continuous fuzzing exists.

## Focused tests / workflow validation

Add a lightweight test or script coverage for `validate_release_versions.py` publication mode and any new workflow guard logic that can be tested without invoking GitHub Actions.

At minimum test:

- matching tag/version;
- mismatched tag/version;
- invalid tag syntax;
- `--publish` without tag;
- ordinary non-publish mode.

Validate workflow YAML syntax with the repository's existing approach; do not add a workflow-meta-validation framework.

## Required plan closure checks

Before this plan closes:

```sh
./scripts/check.sh
./scripts/check_security.sh   # or final canonical name
python scripts/validate_release_versions.py
```

Run package validation if release/tooling files affect package construction:

```sh
./scripts/check.sh package
```

Record the live security scan date and tool versions in the plan closure section. The parent program's final exact-SHA qualification will run the security preflight again against the frozen candidate.

## Non-goals

- no second automatic CI workflow;
- no restoration of the deleted qualification/evidence orchestration system;
- no automatic crates.io publication;
- no long-lived PyPI API token;
- no removal of protected-environment approval;
- no claim that a RustSec scan proves absence of vulnerabilities;
- no dependency upgrade unrelated to an actual finding or required tool compatibility;
- no generic SBOM/provenance program in this pass;
- no cargo-geiger score gate.

## Exit criteria

This plan is complete when security documentation, advisory configuration, and release enforcement tell the same story: routine CI remains intentionally small; explicit/release security scanning is current and fail closed; obsolete advisory exceptions are gone; bounded unsafe ABI exceptions are documented truthfully; and PyPI publication cannot occur from an unversioned or mismatched ref even if external environment configuration is permissive.

## Closure record — complete (2026-09-16)

`deny.toml` has no stale advisory ignores. `scripts/check_security.sh` is the
canonical fail-closed live preflight and is invoked by the publish path without
adding a second automatic workflow. Publication now requires an exact matching
`v<SEMVER>` tag and HEAD identity, while build-only branch dispatch remains
valid. Release-critical maturin/twine versions and the pinned security tool
versions are repository-controlled. The final preflight passed at
2026-09-16T03:59:54Z with cargo-deny 0.19.0 and cargo-audit 0.22.2; the only
reported findings were non-failing duplicate getrandom/hashbrown warnings.
