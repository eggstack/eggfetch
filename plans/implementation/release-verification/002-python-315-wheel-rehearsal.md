# Release and Verification Milestone 002 — Python 3.15 Wheel Rehearsal

Status: closed

Repository planning baseline: `36ab862dabf8f30fdcf05900a68fdf9f93060879`

Depends on:

- M001A 0.2.1 release-candidate preparation

Unblocks:

- M001B coordinated 0.2.1 publication

Source roadmap:

- `plans/subsystems/release-verification-roadmap.md` §7 Milestone 2

Applicable ADR:

- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`

Primary class: capability / packaging qualification

## 1. Objective

Prove the exact M001A `0.2.1` candidate builds, installs, and assembles the
full CPython 3.10–3.15 distribution set without publishing anything.

Required release set:

- Linux manylinux2014 x86_64: CPython 3.10–3.15;
- macOS arm64: CPython 3.10–3.15;
- Windows x86_64: CPython 3.10–3.15;
- 18 wheels total;
- 1 sdist;
- 19 distributions total.

## 2. Current Python 3.15 baseline

As of this planning pass, Python 3.15.0rc2 is the final planned release
candidate and 3.15.0 final is scheduled for 2026-10-01.

The checked-in workflow intentionally permits prerelease resolution only for
the 3.15 rows. The repository already advertises the 3.15 classifier, so this
milestone is a hard predecessor to M001B publication.

## 3. Candidate identity

Use the exact release-candidate SHA recorded by M001A.

Do not rehearse an arbitrary newer `main`.

Operationally, `workflow_dispatch` may be launched from `main` only while
`main` still resolves to the candidate SHA. Immediately verify the resulting
Actions run `head_sha` equals the M001A candidate. If it does not, cancel the
run and do not treat it as evidence.

No tag is required for `publish=false`.

## 4. Local pre-dispatch gates

From the exact candidate:

```bash
./scripts/check.sh
./scripts/check.sh package
git diff --check
```

Verify again that:

- six Rust package versions are `0.2.1`;
- Python package version is `0.2.1`;
- release validators pass;
- the worktree is clean.

## 5. Required workflow dispatch

Dispatch:

`.github/workflows/pypi.yml`

with:

```text
publish=false
```

Do not use `publish=true`.

The workflow must execute its existing:

- release-version validation;
- internal-dependency validation;
- routine validation;
- package validation;
- 18 wheel jobs;
- wheel install/smoke;
- wheel metadata validation;
- sdist build and isolated rebuild/install;
- package-content validation;
- assembled release-set validation;
- matrix coverage validation;
- `twine check`.

## 6. Python 3.15-specific evidence

For all three 3.15 rows, record the interpreter actually selected by
`actions/setup-python`.

At the current date the expected interpreter is a 3.15 prerelease, currently
3.15.0rc2. Do not hard-code rc2 as a permanent acceptance requirement; the
workflow is intentionally designed to resolve stable 3.15 automatically after
GA.

Required 3.15 evidence:

- Linux x86_64 wheel green;
- macOS arm64 wheel green;
- Windows x86_64 wheel green;
- each wheel installs into its matching interpreter;
- `scripts/wheel_smoke.py` passes;
- metadata reports package/version `eggfetch 0.2.1`;
- native extension is present.

## 7. Release-set evidence

The `assemble` job must prove:

- exactly 18 wheels;
- exactly 1 sdist;
- no duplicate filenames;
- `scripts/validate_wheel_coverage.py` green;
- `twine check` green;
- `pypi-distributions` artifact uploaded.

Retain the workflow run ID and relevant job/artifact metadata in the closure.

## 8. Failure policy

A failed row is a real release blocker.

Do not:

- drop the failed platform/version;
- change `allow-prereleases` for 3.10–3.14;
- bypass wheel smoke;
- publish to discover whether the artifact works;
- rerun until green without understanding deterministic failures.

Transient hosted-runner failures may be re-dispatched, but both the failed
and successful runs must be recorded if the first run exposed a real
infrastructure condition.

Any repository change needed to fix the rehearsal invalidates the M001A
candidate. Return to M001A, create a new candidate, rerun its required gates,
then rehearse again.

## 9. Acceptance criteria

- [ ] workflow `head_sha` equals the M001A candidate SHA;
- [ ] `publish=false`;
- [ ] validate-release green;
- [ ] all 18 wheel jobs green;
- [ ] all three Python 3.15 rows green;
- [ ] sdist green and rebuilds outside the repo;
- [ ] exactly 19 distributions assembled;
- [ ] wheel coverage validator green;
- [ ] `twine check` green;
- [ ] no publish job executed;
- [ ] exact run ID and candidate SHA recorded;
- [ ] Python 3.15 release-backed support is now evidence-backed for the
      candidate.

## 10. Closure

Create:

`plans/closure/release-verification/002-python-315-wheel-rehearsal.md`

Record:

- candidate SHA;
- run ID and conclusion;
- 18 per-wheel results;
- three 3.15 interpreter versions;
- sdist result;
- assemble counts;
- smoke/coverage/twine results;
- any retries and why;
- confirmation that nothing was published.

After closure, M001B becomes the sole ready release task.
