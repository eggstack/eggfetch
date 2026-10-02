# Release and Verification Milestone 005 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/release-verification/005-0-2-2-m007-publication.md`

Source subsystem roadmap:

- `plans/subsystems/release-verification-roadmap.md#milestone-5--022-m007-publication-and-release-polish`

Repository baseline reviewed: `44c6477b6f23d407e9989c496e48915619e7d633`
(planning baseline; M007 closure line)

Implementation commits:

- `e508992c` — `fix(ffi): make the bounded C-string scan c_char-signedness agnostic`
  (the release-blocking defect that Tier 1 found on the versioned candidate)
- `015a56d7` — `release: bump coordinated versions to 0.2.2 for the M007 publication`
  (**frozen release candidate**; signed `v0.2.2` targets this commit)
- `f8e9ecd9` — `plans: renew Stage C binding to the 0.2.2 release candidate`
  (docs/planning-only descendant; does not invalidate the binding)

## 1. Executive finding

M005 is complete. The coordinated **EggFetch 0.2.2** release is published on
frozen candidate `015a56d7ec3edf186eec8ebccff01cbf5584274e`: all six
publishable crates are on crates.io at 0.2.2, the signed `v0.2.2` tag
dereferences to that exact commit, PyPI `eggfetch` 0.2.2 has exactly the
expected 19 files (18 wheels + 1 sdist), and the non-draft, non-prerelease
GitHub Release `401746856` targets the same tag. Fresh-consumer smokes confirm
both registries resolve and work.

The milestone performed no new runtime feature work. It published the
already-qualified M007 capability and, in doing so, **surfaced and fixed one
genuine release blocker** that the previous qualification could not have
caught: `eggfetch-ffi` did not compile on any target where `c_char` is `u8`
(§3). That fix is the only executable change in the candidate beyond the
release identity, and Stage C was renewed to the resulting SHA per the
exact-SHA rule rather than left pointing at the pre-release freeze.

One residual finding is recorded at low severity (§10) and one operational
deviation — the temporary release branch used to obtain a dispatchable ref —
is documented rather than hidden (§9).

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| WP0 planning/control-surface reconciliation | `1d571b57` | Pass | core-transport → closed; registry M005 dependency-ready; README execution gate updated |
| WP1 public identity preflight | No remote `v0.2.2` tag; `gh release view v0.2.2` → "release not found"; all six crates.io `max_version` 0.2.1; PyPI latest 0.2.1; `v0.2.1` dereferences to `fe4d596c` per its closure; `44c6477b` and `3fc58fbd` both ancestors of HEAD; only `plans/**` differed between baseline and HEAD | Pass | No unexpected public state; no unreviewed executable drift |
| WP2 coordinated 0.2.2 version identity | `validate_release_versions.py` → "All versions consistent: 0.2.2" (six crates + pyproject); `validate_publishable_internal_dependencies.py` → every internal edge `^0.2.2` | Pass | Also aligned `package.json`, `test_sync.py` version pin, and two release-facing docs |
| WP2 truthful changelog | `CHANGELOG.md` `## [0.2.2]` Added/Fixed/Changed sections | Pass | M007 scoped explicitly to Rust core; no adapter-classifier claim; MSRV 1.89 retained |
| WP2 Python 3.15 truthfulness | python.org FTP lists `3.15.0/` with newest file `Python-3.15.0rc2.tgz` (no `3.15.0` final); `actions/python-versions` manifest newest 3.15 entry is `3.15.0-rc.2` | Pass | Time-bound "until 2026-10-01" wording removed; 3.15 rows and `allow-prereleases` **unchanged**; no matrix or coverage weakening |
| WP3 Tier 1 | `./scripts/check.sh` on the candidate tree | Pass | Explicit Node-JS-artifact skip |
| WP3 Tier 2 | `./scripts/check.sh extended` | Pass | Explicit Node-JS-artifact and downstream-manifest skips |
| WP3 Tier 3 | `./scripts/check.sh package` | Pass | Crate package validation, `cargo publish --dry-run -p eggfetch-http-connect`, wheel smoke, package-content validation, installed-wheel typing smoke |
| WP3 Rust 1.89.0 MSRV | `rustup run 1.89.0 cargo check --locked --workspace --all-targets --all-features` inside Tier 2; `rustc 1.89.0 (29483883e 2025-08-04)` | Pass | MSRV unchanged at 1.89 |
| WP3 six-profile API oracle + semver | `Rust public API oracle passed (6 profiles)`; `223 checks: 223 pass, 30 skip`, `Summary no semver update required`, `eggfetch-core v0.1.9 -> v0.2.2` vs planning baseline `03ecba973010e2858bf16a2b5f84d51ce70adae4` | Pass | Snapshots unchanged by the bump (no version strings in them) |
| WP3 full compatibility suites | `EGGFETCH_COMPAT_REQUIRED=1 ... 1934 passed, 26 warnings` inside Tier 2; standalone re-run `1934 passed` | Pass | Both facades |
| WP3 live security preflight | `./scripts/check_security.sh` at 2026-10-02 05:39 UTC and re-run 10:05 UTC: `advisories ok, bans ok, licenses ok, sources ok`; cargo-deny 0.20.2, cargo-audit 0.22.2, 1280 advisories loaded, 273 crate dependencies scanned | Pass | Re-run immediately before publication per WP5 |
| WP3 version/tree evidence | `validate_release_versions.py`; `cargo tree -p eggfetch-core`; `cargo tree -p eggfetch-core -e features`; `cargo tree -p eggfetch-core -d` | Pass | No new duplicates introduced |
| WP3 Stage C rebinding | `f8e9ecd9`: live ledger + both `compat/*/profile.toml` + `docs/residual-differences.md` + `docs/reference/compatibility.md` + `docs/reference/compatibility-stage-decision.md` all moved to `015a56d7` | Pass | Binding names the candidate that is actually released |
| WP3 CI on the candidate | Run [36969894716](https://github.com/eggstack/eggfetch/actions/runs/36969894716), `head_sha=015a56d7`, conclusion `success` | Pass | Exact candidate, not a descendant |
| WP4 build-only rehearsal | Run [36991057953](https://github.com/eggstack/eggfetch/actions/runs/36991057953), `head_sha=015a56d7`, `publish=false`, conclusion `success` | Pass | 18 wheels + 1 sdist; all rows smoked; `validate_wheel_coverage.py` and `twine check` green |
| WP4 Python 3.15 interpreter | All three 3.15 rows logged `Successfully set up CPython (3.15.0-rc.2)` (linux-x86_64, macos-arm64, windows-x86_64) | Pass | 3.15.0 final is still unreleased; prerelease fallback confined to the 3.15 rows |
| WP5 crates.io, dependency order | `eggfetch-http-connect` → `eggfetch-core` → `eggfetch-cli` → `eggfetch-ffi` → `eggfetch-python` → `eggfetch-node`, each awaited until available before the next | Pass | All six `newest_version=0.2.2`, `max=0.2.2` |
| WP5 registry propagation | crates.io dependency API for 0.2.2: `eggfetch-core→eggfetch-http-connect ^0.2.2 (optional)`, `eggfetch-cli→eggfetch-core ^0.2.2`, `eggfetch-ffi→eggfetch-core ^0.2.2`, `eggfetch-python→eggfetch-core ^0.2.2`, `eggfetch-node→eggfetch-ffi ^0.2.2` | Pass | No fixed sleeps; cargo awaited propagation |
| WP6 signed tag | Tag object `33d1dfcf09e6f1fca1f5c621a10e335c61596f55` dereferences to `015a56d7`; local `git verify-tag` → `Good "git" signature ... ED25519 key SHA256:+u+s27HZnCLx4APmzjnS0F9LCbJqj40KuaYbnVkN6k4` | Pass | Created only after every pre-publication gate and the rehearsal |
| WP6 PyPI publication | Run [36994291293](https://github.com/eggstack/eggfetch/actions/runs/36994291293), `head_sha=015a56d7`, `publish=true`, conclusion `success`; publish job `Distributions to publish: 19`; protected `pypi` environment approved before publish | Pass | OIDC Trusted Publishing only; no token used |
| WP6 assembled-set inspection | 19 artifacts downloaded and re-validated locally before environment approval: `validate_wheel_coverage.py` "All 18 wheels present with expected coverage", `twine 6.1.0 check` PASSED on all 19, `validate_package_content.py` PASSED | Pass | Approval was not blind |
| WP7 GitHub Release | Release ID `401746856`, tag `v0.2.2`, `targetCommitish=015a56d7`, `draft=false`, `prerelease=false`, published 2026-10-02T11:07:10Z | Pass | Reuses the already-pushed signed tag (`--verify-tag`) |
| WP7 PyPI file set | PyPI JSON for `eggfetch/0.2.2`: exactly 19 files, CPython 3.10–3.15 × {macosx_11_0_arm64, manylinux_2_17_x86_64, win_amd64} + `eggfetch-0.2.2.tar.gz`; `requires_python >=3.10` | Pass | Identical shape to 0.2.1 |
| WP7 fresh Rust smoke | Disposable crate with `eggfetch-core = "=0.2.2"`, no path/git override; `cargo run` → `kind()=hyper_client`, `transport_failure_kind()=connect` against a real refused loopback connect | Pass | M007 API compiles and returns the expected category from the published crate |
| WP7 fresh Python smoke | Clean venv, `pip install eggfetch==0.2.2` (resolved the CPython 3.12 manylinux x86_64 wheel); `__version__=0.2.2`, `importlib.metadata.version=0.2.2`; sync `GET` → `200 {'ok': True, ...}` and async `GET` → `200 ok` against a local server | Pass | Fresh registry install, not a local build |
| WP8 closure + reconciliation | This record; `plans/registry.md`, release-verification roadmap, `plans/README.md`, `docs/architecture/overview.md` | Pass | All four returned to truthful steady state |
| Historical v0.2.0 / v0.2.1 immutability | `git tag -l 'v0.2*'` → `v0.2.0 v0.2.1 v0.2.2`; `v0.2.1` still dereferences to `fe4d596c`; crates.io 0.2.1 and PyPI 0.2.1 both still present | Pass | No tag moved, no version overwritten |
| `eggfetch-bench` / `fuzz/` never published | Not in `PUBLISHABLE_CRATES`; not published | Pass | Unchanged |

## 3. The release blocker Tier 1 found

`cstr_to_string()` in `crates/eggfetch-ffi/src/handle.rs` was hardened
after `v0.2.1` (commit `52589716`) to a bounded `strnlen`-style scan. The scan
read each character through `(*ptr.add(len)).cast_unsigned()`. `cast_unsigned`
exists **only on signed integer types**, but `c_char` is `i8` on
x86_64-linux/windows/macOS and `u8` on `aarch64-unknown-linux-gnu`. On an
unsigned-`c_char` target the crate therefore failed to compile:

```text
error[E0599]: no method named `cast_unsigned` found for type `u8` in the current scope
   --> crates/eggfetch-ffi/src/handle.rs:134:49
```

Three facts made this invisible until now:

1. the line only exists after `v0.2.1`, so published 0.2.1 was unaffected
   (verified by extracting the published `eggfetch-ffi` 0.2.1 crate, whose
   `cstr_to_string` still uses `CStr::from_ptr`);
2. CI runs exactly one job on `ubuntu-latest` (x86_64), where `c_char == i8`
   and the call type-checks;
3. every prior qualification — including M007's — ran on x86_64.

Publishing `eggfetch-ffi` 0.2.2 unchanged would have shipped a crate that
does not build for aarch64 Linux consumers. The fix compares the C character
against `0` directly, which is correct for both signedness variants and keeps
the identical fail-closed `MAX_CSTR_LEN` bound. Four unit tests now pin the
scan contract (ordinary input, leading NUL, over-long input rejected, null
pointer). This is the only executable change in the candidate, which is why
Stage C was rebound to `015a56d7` rather than left on the M007 freeze
`3fc58fbd`.

## 4. Verification executed

### Commands run on the frozen candidate `015a56d7`

- `./scripts/check.sh`
- `./scripts/check.sh extended`
- `./scripts/check.sh package`
- `./scripts/check_security.sh` (twice: qualification, then immediately
  before crates.io publication)
- `python scripts/validate_release_versions.py`
- `python scripts/validate_publishable_internal_dependencies.py`
- `cargo tree -p eggfetch-core` / `-e features` / `-d`
- `rustup run 1.89.0 rustc --version`
- `python scripts/check_doc_links.py`

### Results

| Gate | Result | Counts / notes |
|---|---|---|
| Tier 1 | Pass | Workspace tests green; stable six-profile contract fixtures green; native API manifest, typing surface, typing fixture, behavior tests, compat smoke kernel, Node binding build all green. One explicit skip: Node JS surface (`eggfetch.node` native artifact absent) |
| Tier 2 | Pass | Compatibility suites **1934 passed, 0 failed**; API manifest comparison 74 symbols; six-profile oracle **223 pass / 30 skip**, no semver update; feature matrix (incl. `native-http1`/`standard-http1` lean profiles); feature-gated tests; Rust 1.89.0 MSRV; docs; FFI; lifecycle; soak; lossless merge; benchmarks. Two explicit skips: Node JS surface, downstream fixtures (manifest absent) |
| Tier 3 | Pass | Crate package validation for all six crates; `cargo publish --dry-run -p eggfetch-http-connect`; built `eggfetch-0.2.2-cp312-cp312-manylinux_2_34_aarch64.whl`; all 12 wheel smoke checks PASSED (version assertion plus 11 request/behavior checks); package-content validation PASSED; installed-wheel typing smoke PASSED |
| Security preflight | Pass | `advisories ok, bans ok, licenses ok, sources ok` |
| CI | Pass | Run `36969894716`, `head_sha=015a56d7` |

No required gate was substituted or skipped for convenience. The two skips in
each tier are the policy-defined optional ones present on every historical run.

## 5. Invariant review

| Invariant | Result |
|---|---|
| Never move, delete, or reuse v0.2.0 or v0.2.1 | Held — both tags still dereference to their original targets; both registries still serve 0.2.1 |
| v0.2.2 created only after every pre-publication gate and the rehearsal | Held — tag created after Tier 1/2/3, security, Stage C renewal, CI, and rehearsal run `36991057953` |
| All six crates and PyPI share the 0.2.2 identity | Held — verified on all three registries |
| All internal publishable constraints moved coherently to 0.2.2 | Held — manifest edges and published crates.io dependency metadata both show `^0.2.2` |
| Rust MSRV remains 1.89 | Held — exact 1.89.0 matrix green; no manifest MSRV edit |
| M007 behavior contract unchanged | Held — `TransportFailureKind::{Connect,Tls,Protocol,Cancelled}`, `Error::transport_failure_kind()`, and existing Error/timeout/dialer/`NetworkFailureKind` semantics untouched |
| No new Error variant, feature flag, dependency, or policy | Held — only the one pre-existing M007 additive enum/method is public; the six API snapshots are byte-identical to the M007 ones |
| Version/package/config changes invalidate the freeze | Honored — Stage C rebound to `015a56d7`, not left on `3fc58fbd` |
| crates.io publication manual from a trusted environment | Held — local `cargo publish` |
| PyPI publication manual `pypi.yml` dispatch with OIDC | Held — run `36994291293`; no PyPI token exists or was used |
| GitHub Release manual, reuses the pushed signed tag | Held — created with `--verify-tag` against `v0.2.2` |
| `eggfetch-bench` and `fuzz/` never published | Held |
| No registry overwrite, no tag moved to fake atomicity | Held — nothing was overwritten or retargeted |

## 6. Failure, timeout, pool, and recovery review

No transport, timeout, pool, or recovery code changed in this milestone. The
candidate's only executable delta is the FFI C-string scan (§3), which is a
pure read-side bound and cannot affect deadline, admission, or pool behavior.
The post-0.2.1 hardening already in the candidate (saturating timeout
arithmetic, total-gated backoff, `Total`-wins-tie breaking, bounded retry
drain) is covered by the unchanged Tier 1/2 timeout, lifecycle, and soak
suites, all green on the candidate.

## 7. Compatibility and feature-profile review

- Both facades remain Stage C qualified; the binding moved from
  `3fc58fbd99ecb496749b833ee7b27436fe3b412d` to
  `015a56d7ec3edf186eec8ebccff01cbf5584274e` because executable inputs
  changed (release identity + the FFI fix). The canonical records were renewed
  together, as the exact-SHA rule requires.
- `compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml`
  now read `qualification-sha = 015a56d7...` with
  `previous-qualification-sha = 3fc58fbd...`; the superseded SHAs remain
  listed as historical.
- Full pinned compatibility suites: 1934 passed, 0 failed.
- Feature graph and defaults unchanged; the feature matrix (including the lean
  `native-http1` / `standard-http1` profiles) is green.
- `Error` remains exhaustively matchable; `TransportFailureKind` is
  non-exhaustive from birth. The fresh Rust smoke compiled a match over the
  four named variants plus a `Some(_)` future arm.
- No new residual difference. `docs/residual-differences.md` was renewed for
  the new SHA, not rewritten in substance.

## 8. Security review

Complete release checklist executed; no new security behavior was authorized
or added.

| Item | Result |
|---|---|
| M007 classifier is enum-only, surfaces no nested secret text | Confirmed — `TransportFailureKind` carries four unit variants only; the classifier reads typed evidence and returns a category, never a message or payload |
| Dependency/advisory/license/source checks current | `advisories ok, bans ok, licenses ok, sources ok` on the candidate, re-run immediately before publication |
| TLS/proxy/auth/redaction regression suites green | Yes — Tier 1/2 include the TLS, proxy, SOCKS, auth, redirect-credential, and redaction suites; all green on the candidate |
| No release credential committed | Confirmed — `git show --stat` for both candidate commits shows no secret files; crates.io token remains in `~/.cargo/credentials.toml` (outside the repo); PyPI uses OIDC only and no token exists in the environment |
| Tag signed | Yes — verified locally; see §2 |
| Published package contents contain no generated/private qualification artifacts | Verified — `cargo package --list` for all six crates contains no `plans/`, `compat/`, `.skills/`, or audit files; the sdist's top level is `Cargo.lock Cargo.toml PKG-INFO README.md crates pyproject.toml python`; wheel contents are the `eggfetch` package plus `dist-info` (incl. the intended CycloneDX SBOM) |

The FFI fix also removes a signedness-dependent read path rather than adding
one, so it is security-neutral.

## 9. Documentation and operations

Reviewed and corrected for the 0.2.2 state:

- `CHANGELOG.md` — new `## [0.2.2]` section; `[Unreleased]` compare link moved
  to `v0.2.2...HEAD`; `[0.2.2]` release link added.
- `docs/reference/versioning.md` — coordinated version example now 0.2.2.
- `docs/architecture/rust-surface-containment.md` — Node package metadata now
  described as aligned to 0.2.2.
- `docs/releases/compatibility-policy.md` — the stale claim that Python 3.15
  publication "remains gated on the final package and wheel qualification
  pass" was replaced with the truthful pre-GA description (3.15 rows resolve
  the current prerelease; 3.10–3.14 always stable; the row self-upgrades at
  GA).
- `docs/architecture/overview.md` — current-status paragraph updated to 0.2.2.
- `.github/workflows/pypi.yml` — the time-bound "until 3.15.0 final
  (2026-10-01)" comment was replaced with a statement that is true today and
  stays true after GA. **No matrix, row, or `allow-prereleases` behavior was
  changed**, so interpreter coverage was not weakened.
- `crates/eggfetch-node/package.json` — version aligned to 0.2.2 (metadata
  alignment is a documented intent, not npm publication; the Node package
  remains an unpublished experimental prototype).

Historical 0.2.1 plan and closure records were not rewritten. The M005
implementation plan itself was **not** moved to `archive/`: the most recent
closed plans in this repository (`001a`, `002`, and M007) stay in place with
`Status: closed`, and both the registry and the subsystem roadmap cite this
file by its current path.

Two truth fixes outside M005's own surface were made while reconciling, and
are recorded here rather than applied silently:

- `plans/implementation/core-transport-policy/007-native-transport-failure-classification.md`
  still read `Status: ready` even though its closure record was accepted and
  the registry already listed it as closed. It now reads
  `Status: closed — see plans/closure/core-transport-policy/007-...`.
- `docs/architecture/overview.md` still described the 0.2.1 publication as the
  latest state.

**Operational deviation, recorded deliberately.** `workflow_dispatch`
requires a branch or tag ref, and the candidate `015a56d7` was no longer a
branch head by the time the rehearsal was dispatched (the docs-only Stage C
renewal `f8e9ecd9` had landed on `main`). A temporary branch
`release/0.2.2-candidate` was therefore pushed at the candidate SHA and used
as the rehearsal dispatch ref, so the rehearsal `head_sha` is the exact
candidate rather than `main`. The same branch was used for the build-only
rehearsal only; the publication run was dispatched from the `v0.2.2` tag as
the workflow requires. The temporary branch was deleted after publication and
no stray ref remains. Every workflow run listed in §2 reports
`head_sha=015a56d7`, so no gate was evaluated on the wrong tree.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| Low | `crates/egfetch-python/tests/compat/test_socks_transport.py` `_SocksHandler._relay` decodes the relayed request line with `fields[1].decode("ascii")` and does not catch `UnicodeDecodeError` (a `ValueError`, outside its `except (AssertionError, ConnectionError, OSError, socket.timeout)`). Under whole-suite load one Tier 2 attempt raised it from TLS bytes, failing `test_socks_https_uses_origin_tls_and_reuses_connection[candidate]` and killing the relay thread. | Fixture robustness only. The candidate changes no proxy/SOCKS code; the module passed 6/6 in isolation and the full 1934-test suite passed on rerun. No product impact. | Route through the standing corrective intake (release-verification M003). No action required for 0.2.2. |
| Informational | `eggfetch-ffi` and `eggfetch-node` are built and tested only on the CI job's x86_64 architecture; unsigned-`c_char` and other cross-target build breakage is invisible to routine CI (this is exactly how §3 escaped). | A repeat class of portability defect could reach a release again. | Owner decision, not taken here: the verification policy forbids adding CI jobs or matrices without an explicit request. Raise as a roadmap item if the maintainer wants matrix coverage. |
| Informational | The `v0.2.2` tag is signed with a different SSH key than `v0.2.1`. `v0.2.1` was signed by `wr3n-ai <wr3n@autonomouscollective>` with ED25519 `SHA256:gwin+vF6C6TSm44DJkRShuNILg8fPah7e832ujvYeKw`; `v0.2.2` is signed by this environment's configured identity `eggfetch-agent <agent@eggfetch.dev>` with ED25519 `SHA256:+u+s27HZnCLx4APmzjnS0F9LCbJqj40KuaYbnVkN6k4`. | Consumers pinning a trusted-signer list must add the new key. The signature itself is valid and the tag was never moved. | Publish the new key fingerprint as the release signing key; do not attempt to re-sign or replace the immutable tag. |

No high-severity finding remains open. Nothing here blocks the release or
its downstream consumers.

## 11. Roadmap disposition

- Release and verification M005: **closed**.
- Core transport and request policy M007: stays closed. Its API is now
  available from a published registry version (§12).
- Release and verification M003 (standing corrective intake): remains
  proposed/standing and now carries the one low-severity finding in §10.
- HTTPX 1.0, H3 graduation, Node maturation: unchanged and still gated on
  their own external evidence; none is unblocked by this milestone.
- No new milestone is created by this closure.

## 12. Registry updates

Recorded in `plans/registry.md`, `plans/subsystems/release-verification-roadmap.md`,
and `plans/README.md`:

- release-verification roadmap status → closed/steady state; M005 → closed
  with its closure record and public identities;
- registry release-train paragraph → M001/M004 and M005 all closed;
- registry execution gate → no active release task;
- `plans/README.md` execution gate → no active release task, with the
  0.2.2 identities recorded as current public state.

## 13. Downstream handoff

**Explicit statement.** M007's `TransportFailureKind` /
`Error::transport_failure_kind()` API is now available from the published
registry version: `eggfetch-core = "0.2.2"` on crates.io, verified by a fresh
consumer build that resolved 0.2.2 from crates.io and compiled and ran the
classifier. Exact-pin downstream consumers (including EggPool) **may begin
their own adoption milestone now** that public 0.2.2 verification is complete.

This milestone does **not** claim that any downstream consumer has migrated.
No downstream implementation was performed, attempted, or verified here; an
exact-pin consumer must pin and verify 0.2.2 itself and record its own
adoption evidence. Because 0.2.2 is additive, a consumer that pins an older
version is unaffected and may adopt independently of this milestone's
schedule.

## 14. Final recommendation

**Closed.**

All fifteen acceptance criteria in the implementation plan are satisfied, with
one acceptance criterion satisfied by evidence that exceeded its literal
requirement: §7's "renew the Stage C binding" turned out to be load-bearing
rather than clerical, because the release candidate contained a real
executable fix. No partial-publication incident occurred — all six crates,
the tag, all 19 PyPI files, and the GitHub Release published successfully on
the first attempt and were verified from outside the publishing environment.
