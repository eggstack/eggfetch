# Release and Verification M001B — 0.2.1 Coordinated Publication

Status: superseded before execution

Repository planning baseline: `36ab862dabf8f30fdcf05900a68fdf9f93060879`

Superseded by:

- `plans/implementation/release-verification/004-0-2-1-tagged-release-finalization.md`

Reason: M001A and M002 closed successfully, but the final publication pass now
requires a published GitHub Release and a truthful pre-release README/PyPI
long-description refresh. The replacement milestone preserves this plan's
publication-order and recovery guidance while renewing the exact candidate
and wheel rehearsal.

Parent milestone:

- M001 coordinated 0.2.x publication

Depends on:

- M001A release candidate closed
- M002 Python 3.15 wheel rehearsal closed

Primary class: infrastructure / operational publication

## 1. Objective

Publish the exact M001A candidate already qualified by M002 as coordinated
EggFetch `0.2.1`.

Outputs:

- six crates.io packages at `0.2.1`;
- signed `v0.2.1` tag targeting the candidate SHA;
- PyPI `eggfetch 0.2.1` built from that exact tag through Trusted
  Publishing;
- external registry-resolution/install smoke;
- closure records and release-status reconciliation.

No source change belongs in this plan.

## 2. Immutable candidate rule

The candidate SHA from M001A and M002 is authoritative.

Before publication verify:

- current release worktree matches that candidate;
- all versions remain `0.2.1`;
- M002 run `head_sha` equals that candidate;
- `v0.2.1` does not already exist;
- no new executable/test/dependency/workflow change has landed into the
  candidate.

If any change is required, stop and return to M001A. Do not publish a
different tree under M002's evidence.

## 3. Immediate pre-publication gates

From a trusted maintainer environment and clean candidate checkout run:

```bash
./scripts/check.sh extended
./scripts/check.sh package
./scripts/check_security.sh
git diff --check
```

The security preflight must be live at publication time; earlier green scans
do not substitute.

## 4. crates.io publication order

Publish manually in dependency order:

```text
eggfetch-http-connect 0.2.1
eggfetch-core         0.2.1
eggfetch-cli          0.2.1
eggfetch-ffi          0.2.1
eggfetch-python       0.2.1
eggfetch-node         0.2.1
```

For each dependent crate:

1. verify preceding internal dependencies are visible/resolvable on crates.io;
2. run the appropriate full `cargo publish --dry-run -p <crate>` once its
   dependencies can resolve from the registry;
3. publish manually;
4. verify the exact `0.2.1` version is visible before continuing where
   dependency order requires it.

Do not use fixed sleeps as release policy.

## 5. Partial-publication handling

crates.io versions are immutable.

If publication stops after one or more crates succeed:

- record exactly which versions are live;
- do not yank merely to make the sequence appear atomic;
- do not retry an already-published version;
- determine whether the remaining crates can safely continue;
- if package contents/version identity must change, stop and plan a new patch
  release rather than overwriting `0.2.1`.

The closure must truthfully represent partial state until all required
packages are available.

## 6. Signed tag

Only after all six crates are successfully published and verified, create a
signed tag on the exact candidate SHA:

```bash
git tag -s v0.2.1 <CANDIDATE_SHA> -m "Release v0.2.1"
git push origin v0.2.1
```

Verify the remote tag resolves to exactly the M001A/M002 candidate.

Do not move or replace historical `v0.2.0`.

A GitHub Release is optional and is not a closure gate.

## 7. PyPI publication

Dispatch `.github/workflows/pypi.yml` from the **v0.2.1 tag** with:

```text
publish=true
```

The workflow must:

- validate that the selected ref is a `v<SEMVER>` tag;
- prove checkout commit equals the tag commit;
- validate release version `0.2.1`;
- run the live security preflight;
- run routine/package validation;
- rebuild the same 18-wheel + 1-sdist matrix;
- validate the assembled set;
- publish only the 19 validated distributions;
- use the protected `pypi` environment and OIDC Trusted Publishing.

Approve the protected environment only after the assemble job is green.

M002's rehearsal is evidence for the candidate but does not permit skipping
the publish workflow's own rebuild/validation.

## 8. External verification

After publication verify:

### crates.io

- all six `0.2.1` packages resolve without path/git overrides;
- at minimum a disposable consumer can resolve/build
  `eggfetch-core = "=0.2.1"`.

### PyPI

- `eggfetch==0.2.1` installs from PyPI into a clean supported interpreter;
- import/version smoke passes;
- release files include the intended 18 wheels + 1 sdist;
- representative Python 3.15 installation succeeds when the current
  interpreter tooling permits it.

### Release identity

- remote `v0.2.1` tag == M001A candidate SHA == M002 run head SHA ==
  PyPI publish workflow head SHA.

Any mismatch is a release-integrity failure.

## 9. Documentation / closure

Create child closure:

`plans/closure/release-verification/001b-0-2-1-coordinated-publication.md`

Then create/update umbrella closure:

`plans/closure/release-verification/001-coordinated-0-2-1-publication.md`

Update:

- `plans/registry.md`;
- `plans/subsystems/release-verification-roadmap.md`;
- `plans/README.md`;
- release/changelog links if needed;
- any still-open legacy issue/release record whose only blocker was public
  artifact availability.

Do not rewrite historical closure evidence.

## 10. Acceptance criteria

- [ ] M001A candidate SHA unchanged;
- [ ] M002 rehearsal closed on that exact SHA;
- [ ] Tier 2 green immediately before publication;
- [ ] Tier 3 green immediately before publication;
- [ ] live security preflight green;
- [ ] six crates.io `0.2.1` packages published in dependency order;
- [ ] registry propagation/resolution verified;
- [ ] signed `v0.2.1` tag exists and targets candidate SHA;
- [ ] PyPI `publish=true` workflow runs from that tag;
- [ ] all 19 distributions validated and published;
- [ ] clean PyPI install/import smoke passes;
- [ ] clean crates.io consumer smoke passes;
- [ ] candidate/tag/rehearsal/publish SHA identity matches;
- [ ] M001 umbrella closure written;
- [ ] registry leaves no stale `0.2.0` publication instruction.

## 11. Stop conditions

Stop rather than weaken policy if:

- any pre-publication gate is red;
- a security advisory/license/source check fails;
- `v0.2.1` appears unexpectedly;
- tag identity differs from candidate;
- M002 evidence is from a different SHA;
- publication becomes partial and package contents must change;
- PyPI OIDC/environment validation fails;
- any attempt would require moving `v0.2.0` or overwriting an immutable
  registry version.

## 12. Non-goals

- No runtime changes.
- No dependency upgrades.
- No workflow redesign.
- No matrix reduction.
- No HTTPX 1.0/H3/Node maturation.
- No automatic crates.io publication.
