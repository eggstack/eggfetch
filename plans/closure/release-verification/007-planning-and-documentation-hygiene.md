# Release and Verification Milestone 007 — Planning and Documentation Truthfulness Hygiene

Status: conditionally closed

Source implementation plan:

- `plans/implementation/release-verification/007-planning-and-documentation-hygiene.md`

Source subsystem roadmap:

- `plans/subsystems/release-verification-roadmap.md#milestone-7--planning-and-documentation-truthfulness-hygiene`

Repository baseline reviewed: `873ea28541c82817317430c9523fca8525d99c81`.

Implementation commits:

- None. The milestone is implemented in the working tree and is **not
  committed**; the maintainer reviews the working tree before any commit is
  made. No SHA is claimed for this milestone.

## 1. Executive finding

M007 is implemented for the plan's §6.1 through §6.5, each of which is present
in the tree and measured below. It cannot be marked `closed` under
`plans/closure/README.md`. The milestone changes executable inputs —
`crates/eggfetch-python/src/**` and other Rust sources gained `reason =`
justification fields, `fuzz/Cargo.lock` was refreshed, `fuzz/fuzz_targets/**`
were fixed, `compat/**` packages and profile files changed, and `plans/**`
changed — and all of those are executable inputs under the exact-SHA rule in
`.skills/verification-qualification.md`. That invalidates the live Stage C
binding, and neither the Stage C rebinding nor a full Tier 1 / Tier 2 / Tier 3
run on the frozen SHA was performed: the maintainer deliberately deferred the
full tier gates. The milestone is therefore **conditionally closed**, with the
condition, the risk, and the exact future evidence stated in §10.

What the milestone delivered:

- **The unticked acceptance criteria in `plans/` are classified and
  dispositioned.** The baseline census at `873ea285…` was 2,267 unticked
  `- [ ]` boxes across 88 files (83 legacy flat `plans/<name>.md` files
  holding 2,197 boxes; 5 taxonomy files under `plans/implementation/` holding
  70). The tree now holds **1,257** unticked boxes in **50** files, and
  **0 acceptance violations**: every remaining unticked box is covered by
  either a dated `## Acceptance-criteria disposition (2026-10-22)` note or a
  dated `## Open items (2026-10-22)` pointer in its own file.
- **The priority file was resolved individually.**
  `plans/core-private-module-decomposition.md` now carries **0** unticked
  boxes; all ten of its criteria were re-verified against the tree and are
  met, with inline evidence. Its prerequisite citation in
  `plans/connect-wire-overlap-conformance.md:5` was checked and is **accurate**;
  no correction was needed, and the accuracy is recorded in that file's own
  disposition note.
- **The compat profiles' machine-readable content is provably unchanged.**
  The non-comment, non-blank content of both profiles is byte-identical before
  and after, and neither profile retains any SHA in its comments.
- **The two httpx replacement packages now carry the runtime code once.**
  `compat/httpx-replacement-source/httpx/__init__.py` is the single source;
  both distributions map their `httpx` package at it and both set
  `__eggfetch_shim__ = True`.
- **The fuzz lock records the current core version and the fuzz tree
  compiles** against it.
- **The lint-suppression justification pass A (six non-core crates) is
  complete**, with the lint set of every attribute unchanged.

Two things are **not** claimed. The plan's §6.6 **pass B over
`crates/eggfetch-core/src`** was still in flight while this record was
written; its count is left open in §2 and §10 rather than asserted. And the
full Tier 1 / Tier 2 / Tier 3 gates, the workspace suite, the compat pytest
suites and the Rust public-API oracle were **not** run at the frozen SHA — the
public-API oracle result quoted below is the implementer's reported run, not
one taken by this closure pass.

## 2. Requirement-to-evidence matrix

The requirements are the acceptance criteria of §13 of the implementation plan.

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Every unticked acceptance criterion in `plans/` is classified and dispositioned, with the per-class counts recorded (§13) | Baseline census recomputed at HEAD: 2,267 unticked boxes across 88 files (83 legacy / 2,197; 5 taxonomy / 70). Current tree: 1,257 unticked boxes across 50 files. `git diff -- plans/` shows exactly **1,010** newly ticked boxes, matching the reported Tier-1 count | Met | Per-class counts are the implementer's report: met 760, superseded 250, deferred 1,257, never applicable 0 (sums to 2,267). The **totals** are independently confirmed; the 760/250 split inside the 1,010 is measured as 758/252 by citation wording (§4, rows 5–6) |
| No plan file carries a complete execution record while retaining unticked boxes that describe unmet conditions (§13) | Scan of all 88 baseline files and all 50 current files: **0** files combine a completed-execution marker with unticked boxes that lack a dated open-items section | Met | The single file with a completion marker and unticked boxes is `plans/httpx-drop-in-phase-1-status.md` (`Status: partially complete`), and it carries a dated open-items section |
| `plans/core-private-module-decomposition.md` is individually resolved and its prerequisite citation in `plans/connect-wire-overlap-conformance.md` is accurate (§13) | The file has **0** unticked boxes; its ten criteria are ticked with inline evidence. `ResolvedRouteKey` + its impl are byte-identical between `d2c8b137:client.rs` and `crates/eggfetch-core/src/client/routes.rs` (209 B each, `diff` clean). Zero `pub ` items in the six private module files. One `build_hyper_client`. `NoProxy::parse` → `false` at `proxy.rs:132`, `NoProxy::parse_httpx` → `true` at `:145` | Met | `plans/connect-wire-overlap-conformance.md:5` reads `Prerequisite: plans/core-private-module-decomposition.md`; the citation is accurate and the file's own disposition note records that no correction was required |
| `plans/README.md` carries the pending maintainer actions section and `AGENTS.md` is accurate about it (§13) | `plans/README.md:22` is `## Pending maintainer actions` with six numbered items; `AGENTS.md:4` points at it accurately | Met | **Verified, not edited** — the section already existed before this milestone; the plan's §1 claim that it was missing was already false against this tree |
| `fuzz/Cargo.lock` records `eggfetch-core 0.2.2` and the fuzz tree builds (§13) | `grep -A3 'name = "eggfetch-core"' fuzz/Cargo.lock` → `version = "0.2.2"`; `cargo check --manifest-path fuzz/Cargo.toml --all-targets --offline` → **exit 0**, all 12 declared fuzz-target binaries checked | Met | The build became possible only after the **main agent's** two API-drift fixes (§3.3). The plan's §17 still says the fuzz tree does not compile; that statement is now stale |
| The machine-readable content of both compat profiles is byte-identical before and after, and neither retains a historical SHA in its comments (§13) | `diff` of `grep -v '^[[:space:]]*#' \| grep -v '^[[:space:]]*$'` before/after → **IDENTICAL** for both; 41 non-blank non-comment lines each. SHA count on comment lines: **1 → 0** full 40-char, **16 → 0** abbreviated 8-char, per profile. `tomllib.load` parses both | Met | The plan's §3/§6.4 figure of "eleven distinct 40-character SHA strings in those comments" is **wrong** — see §10 |
| The httpx replacement packages are reconciled, and the controlled replacement's publish guard and identity verification still pass (§13) | `check_no_publish.py` → **PASS**; `verify_identity.py` → **PASS**; the downstream identity check (`httpx.__eggfetch_shim__` + `Client`/`AsyncClient` module provenance + no-upstream-httpx) → **PASS** for the shim and **PASS** for the controlled replacement, each in its own isolated venv | Met | The shim was **not** deleted: §14 requires explicit maintainer confirmation and none exists. The resolution is "two packages, one source" |
| Every `allow`/`expect` attribute in `crates/*/src` carries a justification, with no lint set changed (§13) | Pass A (six non-core crates, 71 attributes): 58 python + 4 cli + 3 ffi + 1 node + 1 http-connect + 4 bench. Independent lint-set comparison against HEAD across all 53 files: **0** lint multisets changed | **Partially met** | Pass B over `crates/eggfetch-core/src` was **in flight** at the fingerprint measured (78 of 119 core attributes carry a `reason =`; baseline 41 of 118). The count is left open — see §10 |
| Tier 1 and Tier 2 are green on the frozen executable SHA (§13) | Not run | **Not met** | Deliberately deferred by the maintainer; see §10 |
| Both HTTPX profiles remain bound to the recorded live SHA (§13) | The machine-readable binding fields are **unchanged**; the live Stage C SHA is **invalidated** by this milestone's executable changes and was not rebound | **Not met** | See §7 and §10 |

## 3. Production implementation evidence

### 3.1 The unticked acceptance criteria

**Baseline, recomputed at `873ea285…`**: 2,267 unticked `- [ ]` boxes across 88
files under `plans/` — 83 legacy flat `plans/<name>.md` files holding 2,197
boxes, and 5 taxonomy files under `plans/implementation/` holding 70 boxes.

**Final**: 1,257 unticked boxes across 50 files. The `git diff -- plans/` delta
is exactly **1,010** newly ticked boxes, zero unticked boxes, and 1,080
removed / 70 added unticked lines — the 70 are the deferred open-item
annotations, so the net is 2,267 − 1,010 = 1,257.

Three treatments were applied:

| tier | files | boxes | treatment |
|---|---|---|---|
| Tier 1 — an execution/completion record exists (in-file, or in a linked sibling status record the plan names) | 39 | 1,010 | dispositioned individually; the box is ticked and the evidence is cited inline |
| Tier 2 — no execution record at all | 45 | 1,195 | classified **uniformly by rule**; no box ticked, no file rewritten |
| Deferred (open items) | 5 | 62 | dated open-items section; every unticked box carries a `— Deferred (OI-N).` pointer |

The Tier-2 rule is stated in the file itself, so it is auditable. The
canonical wording, from
`plans/core-hyper-client-construction-and-cache-consolidation.md`:

> This file records **no completed execution**. Its 11 unticked acceptance
> criteria describe work that was never carried through to completion, so they
> remain open exactly as written and are left unticked. The file is a frozen
> historical record: it is retained for context only and is not a current
> gate.

The five deferred files, each carrying `## Open items (2026-10-22)` and an
`— Deferred (OI-N).` pointer on every unticked box:

| File | Unticked boxes |
|---|---|
| `plans/http3-independent-interop-and-impairment-qualification.md` | 26 |
| `plans/http3-upstream-risk-resource-and-observability-hardening.md` | 14 |
| `plans/httpx-drop-in-phase-1-production-semantics.md` | 9 |
| `plans/httpx-drop-in-phase-1-status.md` | 7 |
| `plans/release-candidate-final-evidence-and-enforcement-status.md` | 6 |

One file, `plans/httpx-drop-in-phase-1-production-semantics.md`, carries **both**
a disposition note and an open-items section; that is why the per-tier file
counts sum to 89 against an 88-file baseline.

**Per-class counts over the 2,267 baseline boxes** (implementer's report):
met 760, not met / superseded 250, not met / deferred 1,257, not met / never
applicable 0. The totals are independently confirmed. The 1,010 newly ticked
boxes split, by the citation each tick carries, into 758 that do not cite a
supersession and 252 that do (`supersed*` or `overtaken`); the plan's report of
760/250 is the same total with a two-box wording difference.

### 3.2 `plans/core-private-module-decomposition.md`

Handled first and individually, because `plans/connect-wire-overlap-conformance.md`
cites it as a prerequisite. All ten criteria were re-verified against the
tree and are **met**; the file now has **0** unticked boxes and carries a dated
disposition note recording that its execution history is unchanged. The
evidence, per criterion, as cited in the file and re-checked here:

| Criterion | Evidence |
|---|---|
| Public `Client`/`ClientBuilder` paths unchanged | `crates/eggfetch-core/src/client.rs:76` (`Client`) and `:483` (`ClientBuilder`); re-exported from `crates/eggfetch-core/src/lib.rs:71` |
| Public proxy types unchanged | `crates/eggfetch-core/src/proxy.rs:50,113,187,270,285,298,446,829`; re-exported from `lib.rs:86` |
| Private separation holds | `client/config.rs`, `client/routes.rs`, `client/connectors.rs`, `proxy/no_proxy.rs`, `proxy/environment.rs`, `proxy/identity.rs` — **zero** `pub ` items in all six files |
| Native and HTTPX `NO_PROXY` semantics stay distinct | `NoProxy::parse` → `parse_with_localhost_mode(s, false)` at `proxy.rs:132`; `NoProxy::parse_httpx` → `parse_with_localhost_mode(s, true)` at `proxy.rs:145`; the two rule sets live in `proxy/no_proxy.rs` |
| Route/client cache identity and bounds unchanged | The `ResolvedRouteKey` struct + impl block is **byte-identical** (209 B) between `d2c8b137:crates/eggfetch-core/src/client.rs` and `crates/eggfetch-core/src/client/routes.rs:53` |
| Hyper client construction stays centralized | Exactly one `fn build_hyper_client` in `crates/eggfetch-core/src` (`transport/hyper_client.rs`) |
| No new public helper/type/module | `grep -rnE '^\s*pub (fn\|struct\|enum\|trait\|type\|mod\|const\|static\|use)' crates/eggfetch-core/src/client/ crates/eggfetch-core/src/proxy/` returns nothing |
| No new production dependency | `git diff 03ecba97 -- crates/eggfetch-core/Cargo.toml` shows only the two workspace-internal version fields (`eggfetch-core` 0.1.9→0.2.2, `eggfetch-http-connect` 0.1.9→0.2.2) and no new dependency name. The same file also carries an `[[example]]` stanza added by core-transport M009, which adds no dependency |
| Supported profiles expose the planning-baseline Rust surface | `scripts/check_rust_public_api.py` → `Rust public API oracle passed (8 profiles)`; the complementary `cargo-semver-checks` half reports 223/223 checks. **Implementer-reported**, not re-run by this pass (§10) |
| Focused and canonical tests green | `cargo test -p eggfetch-core --all-features -- --test-threads=1` → **1,489 passed, 0 failed**. **Implementer-reported**, not re-run by this pass |

The prerequisite citation in `plans/connect-wire-overlap-conformance.md:5`
was checked and is **accurate**; no correction was made. The file's own
disposition note records that fact.

### 3.3 `fuzz/Cargo.lock`

Before: `eggfetch-core 0.1.0` in `fuzz/Cargo.lock`, plus `dashmap` and six
other stale transitive packages. After
`cargo update --manifest-path fuzz/Cargo.toml --offline -p eggfetch-core`:

- `eggfetch-core` 0.1.0 → **0.2.2**; `eggfetch-http-connect` 0.2.2 **added**;
- `base64` 0.22.1 → 0.23.1 (required by the current core);
- **removed**: `crossbeam-utils`, `dashmap`, `hashbrown`, `lock_api`,
  `parking_lot_core`, `redox_syscall`, `scopeguard`;
- the remaining **134 of the 143** baseline lock packages are unchanged in
  version.

A full `cargo generate-lockfile --offline` was **tried and rejected**: it moved
85 packages including major-version bumps (`syn` 2→3, `http` 1.4→1.5,
`hyper` 1.10→1.12, `brotli` 8→9, `zstd` 0.13→0.14), which is a plan stop
condition (fuzz-target behavior change).

**The main agent's edits.** Two pre-existing fuzz-target API drifts had
prevented the tree from building, so that the refreshed lock would be usable.
These are the main agent's edits, not the milestone worker's:

- `fuzz/fuzz_targets/fuzz_headers.rs:34` — `headers.extend(other)` →
  `headers.extend(&other)`, matching the current
  `Headers::extend(&mut self, other: &Self)` signature.
- `fuzz/fuzz_targets/fuzz_multipart.rs:37–50` — `content_length()` returns
  `Option<u64>`, not `usize`; the invariant is now asserted only when the
  length is known.

With those in place, `cargo check --manifest-path fuzz/Cargo.toml --all-targets
--offline` exits **0** and checks all twelve declared fuzz-target binaries.
`fuzz/Cargo.toml` declares `eggfetch-core` as a **path dependency**, so the
lock's version comes from the workspace crate version.

### 3.4 The compat profiles

`compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml`:
the historical SHA log was removed from the **comments** only.

| Measurement | `compat/httpx/0.28.1` | `compat/httpx2/2.12.0` |
|---|---|---|
| Total lines, before → after | 207 → 54 | 206 → 58 |
| Comment lines, before → after | 161 → 8 | 160 → 12 |
| Non-blank, non-comment lines, before → after | 41 → 41 | 41 → 41 |
| Non-comment content diff before/after | **IDENTICAL** | **IDENTICAL** |
| Full 40-char SHAs on comment lines, before → after | 1 → **0** | 1 → **0** |
| Abbreviated 8-char prefixes on comment lines, before → after | 16 → **0** | 16 → **0** |
| `tomllib.load` | OK (6 keys) | OK (6 keys) |

The machine-readable keys (`rebaseline-sha`, `qualification-sha`,
`previous-qualification-sha`, `historical-current-qualification-sha`,
`historical-qualification-sha`) are untouched, and each profile keeps a short
comment pointing at `plans/httpx-parity-correction-status.md`.

The one SHA the removed comments named that the HTTPX ledger does not hold,
`639bf186a71c054e11278d1b160ffe7a6f172c02`, is **not** an HTTPX binding: it
is an HTTP/3 qualification binding, held in
`plans/http3-public-origin-spot-check-ledger.json:5`,
`plans/http3-production-qualification-evidence.md:4` and
`plans/http3-post-freeze-diagnostics-requalification-corrective-closure.md:5`.
The main agent recorded that provenance in `docs/architecture/overview.md:266`.
It still needs a canonical ledger entry — see §10.

### 3.5 The httpx replacement packages

Resolution: **two packages, one source**. The shim was **not** deleted — the
plan requires explicit maintainer confirmation for that, and none exists.

- Single canonical source:
  `compat/httpx-replacement-source/httpx/__init__.py` (new).
- Both `pyproject.toml` files map their `httpx` package at it via setuptools
  `package-dir = {httpx = "../httpx-replacement-source/httpx"}`.
- The two in-package `httpx/__init__.py` copies were **deleted**, so the
  runtime code exists once. The stale tracked artifact
  `compat/httpx-shim/build/lib/httpx/__init__.py` was removed and
  `compat/httpx-shim/.gitignore` added, mirroring the controlled replacement.
- Both packages now set `__eggfetch_shim__ = True`, so the qualification
  identity check passes regardless of which is installed.
- Both wheels were rebuilt and verified **by artifact content**, not source
  inspection: `compat/httpx-controlled-replacement/dist/httpx-0.28.1-py3-none-any.whl`
  → sha256 `391b02c8…`; `compat/httpx-shim/dist/httpx_eggfetch_shim-0.28.1-py3-none-any.whl`
  → sha256 `422b1f5c…`. The old hash is not pinned anywhere in the repository.
- `compat/httpx-controlled-replacement/build.sh` now uses
  `PYTHON="${PYTHON:-python3}"` (was bare `python`).
- `compat/httpx-shim/README.md` was corrected: it previously claimed the
  package "declares a conflict with the real `httpx` package", which the
  manifest does not do.

### 3.6 The lint suppressions

**Pass A — the six non-core crates** (71 attributes):

| Crate | Attributes |
|---|---|
| `crates/eggfetch-python/src` | 58 |
| `crates/eggfetch-cli/src` | 4 |
| `crates/eggfetch-ffi/src` | 3 |
| `crates/eggfetch-node/src` | 1 |
| `crates/eggfetch-http-connect/src` | 1 |
| `crates/eggfetch-bench/src` | 4 |

The implementer reports that 57 gained a `reason =`, 3 already carried one in
HEAD, and 10 keep an existing explanatory adjacent comment. Measured at the
fingerprint below, the python crate holds 58 attributes of which **51** carry
a `reason =` and 7 do not; 6 of those 7 keep an explanatory adjacent comment.
The implementer's per-file split does not reconcile arithmetically with the
crate's 58 attributes (57 + 3 > 58), so the measured figures are used here
(§10). **1 attribute is recorded as a case for code change**:
`crates/eggfetch-python/src/response.rs:697` —
`#[allow(dead_code)]` on `response_extensions_from_core`, which has **no
caller** anywhere in `crates/` and whose trailing comment ("Wired into the
HTTPX compatibility facade") is factually wrong. The attribute was left
unchanged; deleting the function, or calling it from the `extensions` getter,
is recommended as a follow-up.

The lint set of every attribute was checked against HEAD across all 53 files
in these six crates and **no lint multiset changed** — only the justification
was added, as §8 of the plan requires.

Also recorded from pass A: `crates/eggfetch-cli/src/main.rs:24` carries a
now-stale `clippy::cast_precision_loss` entry — the `delay as f64` cast that
motivated it is gone (`grep -n 'as f64' crates/eggfetch-cli/src/main.rs`
returns nothing; only `as_secs_f64()` calls remain) — and the lint set is
frozen for this milestone, so it stays and is flagged for removal in a
follow-up.

**Pass B — `crates/eggfetch-core/src`** was being completed concurrently with
this closure record. Measured at fingerprint B, the core tree holds 119
`allow`/`expect` attributes, of which **78** carry a `reason =` field
(baseline at HEAD: 41 of 118); 41 do not. Across `crates/*/src` as a whole
the tree holds **190** attributes, **138** with a `reason =` and 52 without,
against a baseline of 189 / 44 / 145 at HEAD. The pass was still in flight —
it had advanced between fingerprints A and B, and had not advanced further
between B and the final reading — so the count is left open rather than
asserted (§10).

## 4. Verification executed

### Commands run

The working tree is shared with another agent that was editing
`crates/eggfetch-core/src/**` throughout this closure pass, and the
`crates/eggfetch-core/src` lint-suppression pass was itself in flight. Every
measurement below is therefore pinned to a tree fingerprint, and the two
fingerprints taken during the pass are reported separately rather than merged.

```sh
# tree fingerprint used by the measurements
git rev-parse HEAD                              # 873ea28541c82817317430c9523fca8525d99c81
git diff | sha256sum                            # worktree fingerprint

# §6.1 census — baseline at HEAD and current tree
git ls-tree -r --name-only HEAD plans/           # then git show HEAD:<file> per file
grep -rn '^\s*- \[ \]' plans/ --include='*.md' | wc -l
git diff -- plans/ | grep -cE '^\+.*- \[x\]'    # newly ticked boxes

# §6.3 fuzz lock and fuzz build
grep -A3 'name = "eggfetch-core"' fuzz/Cargo.lock
cargo check --manifest-path fuzz/Cargo.toml --all-targets --offline

# §6.4 compat profiles
diff <(git show HEAD:<profile> | grep -v '^[[:space:]]*#' | grep -v '^[[:space:]]*$') \
     <(grep -v '^[[:space:]]*#' <profile> | grep -v '^[[:space:]]*$')
grep '^[[:space:]]*#' <profile> | grep -oE '[0-9a-f]{40}' | sort -u | wc -l
grep '^[[:space:]]*#' <profile> | grep -oE '`[0-9a-f]{8}\.\.\.`' | sort -u | wc -l
.venv/bin/python -c "import tomllib; tomllib.load(open(<profile>,'rb'))"

# §6.5 replacement packages
.venv/bin/python compat/httpx-controlled-replacement/check_no_publish.py
PYTHONPATH=<isolated install> .venv/bin/python compat/httpx-controlled-replacement/verify_identity.py
sha256sum compat/httpx-controlled-replacement/dist/*.whl compat/httpx-shim/dist/*.whl

# §6.6 lint suppressions
bash scripts/check_lint_suppressions.sh
bash scripts/check_lint_suppressions.sh --self-test

# Python surface guards
.venv/bin/python scripts/check_native_python_api.py --self-test
.venv/bin/python scripts/check_python_typing_surface.py --self-test
```

### Results (pass/fail/skip with reason; counts without concealment)

Fingerprint A — `git diff | sha256sum =
65277dc6c2cc89b03dc7469a18a5e9d975287470439f0d67bde3318bd8b3c10e` (read at
the start of the pass):

| # | Invocation | Result |
|---|---|---|
| 1 | Baseline unticked-box census recomputed at HEAD | 2,267 boxes across 88 files — 83 legacy flat files / 2,197 boxes; 5 taxonomy files / 70 boxes |
| 2 | `grep -rn '^\s*- \[ \]' plans/ --include='*.md' \| wc -l` | **1,257** |
| 3 | Files under `plans/` still holding unticked boxes | **50** |
| 4 | Acceptance scan — every unticked box covered by a dated disposition note or an open-items pointer | **1,257 / 1,257 covered; 0 violations**. 46 files carry the disposition note, 5 carry the open-items section, one file carries both |
| 5 | `git diff -- plans/ \| grep -cE '^\+.*- \[x\]'` | **1,010** newly ticked boxes; 0 unticked boxes removed outright; 1,080 removed / 70 added unticked lines (net −1,010) |
| 6 | Class split of the 1,010 newly ticked boxes, by the citation each carries | 758 do not cite a supersession, 252 cite one (`supersed*` / `overtaken`). The plan reports 760 / 250 — same total, two-box wording difference |
| 7 | Files combining a completed-execution marker with unticked boxes lacking a dated open-items section | **0** |
| 8 | `plans/core-private-module-decomposition.md` unticked boxes | **0**; ten criteria ticked with inline evidence |
| 9 | `ResolvedRouteKey` block, `d2c8b137:client.rs` vs `client/routes.rs:53` | **IDENTICAL** (209 B each) |
| 10 | `pub ` items in `client/{config,routes,connectors}.rs` and `proxy/{no_proxy,environment,identity}.rs` | **0** in all six files |
| 11 | `fn build_hyper_client` definitions in `crates/eggfetch-core/src` | **1** |
| 12 | `git diff 03ecba97 -- crates/eggfetch-core/Cargo.toml` | only the two workspace-internal version fields, plus an `[[example]]` stanza from core-transport M009; **no new dependency name** |
| 13 | `plans/README.md` pending-maintainer-actions section | `## Pending maintainer actions` at line 22, **six** numbered items; `AGENTS.md:4` points at it accurately |
| 14 | `fuzz/Cargo.lock` core version | `eggfetch-core` **0.2.2**; `eggfetch-http-connect` 0.2.2 present |
| 15 | `cargo check --manifest-path fuzz/Cargo.toml --all-targets --offline` | **exit 0** — `Checking eggfetch-core v0.2.2`, `Checking eggfetch-fuzz v0.0.0`, `Finished` |
| 16 | Fuzz lock package diff, HEAD → current | 143 → 137 packages: 1 added (`eggfetch-http-connect` 0.2.2), 7 removed, 2 version-changed (`eggfetch-core` 0.1.0→0.2.2, `base64` 0.22.1→0.23.1), **134 unchanged** |
| 17 | Compat profile non-comment content diff, HEAD → current | **IDENTICAL** for both profiles; 41 non-blank non-comment lines each |
| 18 | 40-char SHAs on comment lines, HEAD → current | **1 → 0** (httpx), **1 → 0** (httpx2) |
| 19 | Abbreviated 8-char prefixes on comment lines, HEAD → current | **16 → 0** (httpx), **16 → 0** (httpx2) |
| 20 | `tomllib.load` on both profiles | OK, 6 keys each |
| 21 | `compat/httpx-controlled-replacement/check_no_publish.py` | **PASS** — "no publish attempts targeting the httpx controlled replacement" |
| 22 | `verify_identity.py` against the rebuilt controlled-replacement wheel | **PASS** — "all identity checks passed" |
| 23 | Downstream identity check (`__eggfetch_shim__`, `Client`/`AsyncClient` module provenance, no-upstream-httpx) in an isolated venv per package | **PASS** for the shim and **PASS** for the controlled replacement. `httpx.__file__` resolves inside the isolated site-packages in both; `Client.__module__` and `AsyncClient.__module__` are `eggfetch.compat.httpx._client` in both |
| 24 | Wheel sha256 | `httpx-0.28.1-py3-none-any.whl` → `391b02c8…`; `httpx_eggfetch_shim-0.28.1-py3-none-any.whl` → `422b1f5c…` |
| 25 | `bash scripts/check_lint_suppressions.sh` | exit 0 — "OK: No forbidden broad lint suppressions found." over `crates/` and `fuzz/` |
| 26 | `bash scripts/check_lint_suppressions.sh --self-test` | exit 0 — "lint-suppression guard self-test passed (33 cases)" |
| 27 | Lint multiset comparison, HEAD → current, across the six non-core crates | **0** of 53 files changed a lint set |
| 28 | `crates/eggfetch-python/src` attributes | 58 total, 51 with `reason =`; 7 without, of which 6 carry an adjacent explanatory comment and 1 is the `response.rs:697` code-change case |
| 29 | `crates/eggfetch-core/src` attributes | 119 total, 78 with `reason =` (baseline 41 of 118) — **pass B in flight** |
| 30 | `crates/eggfetch-cli/src/main.rs:24` | carries `clippy::cast_precision_loss`; `grep -n 'as f64'` over the file returns **no match** — the motivating cast is gone |
| 31 | `crates/eggfetch-python/src/response.rs:697` | `#[allow(dead_code)]` on `response_extensions_from_core`; `grep -rn 'response_extensions_from_core' crates/` finds **no caller** |
| 32 | `grep -rn '639bf186' plans/ docs/ compat/` | present in the H3 records and in `docs/architecture/overview.md:266`; **not** in the HTTPX ledger |
| 33 | `.venv/bin/python scripts/check_native_python_api.py --self-test` | exit 0 — "Native API manifest passed (66 exports)" |
| 34 | `.venv/bin/python scripts/check_python_typing_surface.py --self-test` | exit 0 — "Python typing surface passed (66 runtime exports, 32 exception bases, 24 reviewed member contracts)" |

Fingerprint B — `git diff | sha256sum =
3cacb0a13f7badce373975c054cd0b525b2085c2d6316bf30cb0ea422b259c57` (read at
17:52:13Z, after the pass-A measurements). The tree moved twice more during
the pass — `32baf20fdb90e2cbdc0d50b7563c07f514f55b52c9c2df58d67fdec6ce0751b4`
at 17:54:54Z and
`c86574bef2220a4ab81a71fe461c7772b25b8b1b0ccfda9628f0bb557be69f63` at
17:55:57Z (fingerprint C, the final reading):

| # | Invocation | Result |
|---|---|---|
| 35 | `git diff \| sha256sum` | the fingerprint **changed** during the pass (`65277dc6…` → `3cacb0a1…`, then `32baf20f…` and `c86574be…`): the tree was under concurrent edit by the core-consolidation milestone throughout this closure pass |
| 36 | `crates/eggfetch-core/src` lint multiset comparison, HEAD → current | 4 files changed a lint multiset: `client/routes.rs`, `pipeline/custom_dialing.rs`, `pipeline/hop.rs`, `pipeline/redirect.rs` — all of them files **added or moved by core-transport M009/M010**, not by the justification pass |
| 37 | `crates/eggfetch-core/src` attributes, fingerprint A → fingerprint B | 119 total, **43** with `reason =` at fingerprint A → **78** with `reason =` at fingerprint B: pass B advanced during the pass. The count did not move again between fingerprint B and the final reading (`c86574be…`) |
| 38 | `crates/*/src` attributes, HEAD → final reading | 190 total (baseline 189), **138** with `reason =` (baseline 44), 52 without (baseline 145) |
| 39 | `crates/eggfetch-core/src` attributes, final reading | 119 total, 78 with `reason =`, 41 without |

Not run, with reason:

- `./scripts/check.sh` (Tier 1), `./scripts/check.sh extended` (Tier 2),
  `./scripts/check.sh package` (Tier 3) — deliberately deferred by the
  maintainer. The targeted equivalents in the table above are the substitute
  evidence.
- The full workspace test suite
  (`cargo test --workspace --exclude eggfetch-python --all-features`) — same
  reason. The `eggfetch-core` focused suite (1,489 passed, 0 failed) is the
  implementer's reported substitute.
- The Python HTTPX compat pytest suites — same reason; they are also network
  dependent.
- The Rust public-API oracle (`scripts/check_rust_public_api.py`) — it is a
  Tier 2 requirement and must be run at the frozen SHA. The oracle result
  quoted in §3.2 (8 profiles green, 223/223 semver checks) is the
  implementer's reported run. `cargo-public-api 0.52.0`,
  `cargo-semver-checks 0.49.0` and `nightly-2026-05-07` are installed in this
  environment, so the oracle is re-runnable at the frozen commit.
- The MSRV gate — requires the exact Rust 1.89.0 toolchain and is a Tier 2
  requirement.

Every environment-variable assignment used the shell prefix
(`PYTHONPATH=… cmd`). The `env` binary on this machine is a shim that ignores
its arguments, so `env PYTHONPATH=… cmd` would silently produce a false pass;
this hazard is recorded in the M006 and M009 closure records and was avoided
here. All Python tooling ran under `.venv/bin/python`, because the system
`python` resolves to a different eggfetch install.

## 5. Invariant review

- **The machine-readable content of both compat profiles is unchanged.** The
  non-comment, non-blank content diffs clean against HEAD for both files
  (§4, row 17), and the five binding keys are byte-identical.
- **The live SHA remains recorded only in the canonical qualification
  records.** No SHA remains in either profile's comments (§4, rows 18–19);
  each profile points at `plans/httpx-parity-correction-status.md`.
- **The lint set of every attribute is unchanged in the six non-core crates.**
  Independently verified across all 53 files (§4, row 27). The four core
  files whose lint multisets differ are M009/M010's new and moved modules
  (§4, row 36), not justification-pass edits.
- **No `allow`/`expect` attribute was removed** in the six non-core crates;
  the attribute count is unchanged per crate and only the justification was
  added.
- **The controlled-replacement package keeps its publish guard and its
  identity verification**, and both pass (§4, rows 21–23).
- **The downstream qualification path keeps working and keeps requiring
  `__eggfetch_shim__`.** The identity check passes in an isolated venv for
  both packages (§4, row 23).
- **No runtime behavior changed.** The milestone touches Rust attributes
  (justification only), a lock file, a fuzz target's two API-drift fixes,
  Python packaging metadata, and documentation. The `reason =` field does not
  alter what any suppression permits.
- **`unsafe_code = "forbid"` remains workspace-wide** apart from the existing
  FFI and Node overrides. This milestone added no `unsafe` and no dependency.
- **No CI job, matrix or workflow was added.** `.github/workflows/` still holds
  `ci.yml` and the pre-existing manually dispatched `pypi.yml`.
- **The canonical long-term documents `plans/000-` through `plans/003-` were
  not modified by this milestone.** The `git diff -- plans/` delta touches
  plan records and the registry, not the canonical direction documents.
- **No plan file was rewritten to hide history.** The Tier-2 treatment is an
  appended dated note; the Tier-1 treatment ticks boxes and cites evidence
  inline; the deferred treatment appends an open-items section. No execution
  record was removed — `git diff -- plans/` shows **0** removed ticked boxes.
- **The shim was not deleted**, so the plan's §14 stop condition on deleting a
  possibly published distribution was not triggered.

## 6. Failure, timeout, pool, and recovery review

Not applicable. This milestone adds no request path, no transport, no timeout,
no pool behavior and no recovery logic. It changes Rust lint attributes, a
lock file, two fuzz targets, Python packaging metadata, and documentation.

Two operational points are carried forward:

- **The `env` shim hazard.** `env` on `PATH` resolves to a shim that ignores
  its arguments and only prepends `PATH`. An invocation of the form
  `env PYTHONPATH=… cmd` therefore succeeds without running `cmd` at all and
  produces a false pass. Every result in §4 comes from the shell prefix form.
- **The shared-tree hazard.** The tree was under concurrent edit throughout
  this closure pass (fingerprint moved between the two readings), and the
  `crates/eggfetch-core/src` justification pass was itself in flight. The
  measurements that matter — the box census, the profile byte-identity, the
  fuzz build, the lint-set invariance — are pinned to the fingerprints above;
  the core lint count is explicitly left open.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

This milestone changes no cargo feature, no supported profile and no public
API surface. The profile matrix in `docs/architecture/feature-flags.md` and
the snapshot set in `compat/rust-public-api/` are unaffected by it. The
Rust public-API oracle result quoted in §3.2 is the implementer's reported
run.

**Exact-SHA binding.** The live Stage C binding is **invalidated** by this
milestone. The live binding is `42a9c96db5b25354843bd83bd458c71757e95651`
(`compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml`,
and the ledger's Stage C entry in
`plans/httpx-parity-correction-status.md`), which is an ancestor of the
baseline `873ea285…` but does not cover the working tree. This milestone
changes `crates/eggfetch-python/src/**` and other Rust sources (the `reason =`
justification fields), `fuzz/Cargo.lock`, `fuzz/fuzz_targets/**`,
`compat/**` and `plans/**` — all executable inputs under
`.skills/verification-qualification.md`. Until the Stage C binding is
re-established at a frozen SHA that includes these changes, no publication may
cite the previous binding. No SHA is claimed here because the milestone is not
committed.

The machine-readable binding **fields** are unchanged, which is what the
milestone's scope required; that is a different statement from the binding
being current.

**Profile parse.** Both profiles still parse as TOML with 6 top-level keys
each (§4, row 20), so the qualification runner's input is unchanged.

## 8. Security review

No security-relevant runtime behavior changed. No request path, transport,
credential, trust store, certificate handling, redaction, proxy, cookie or TLS
configuration was touched.

- The `reason =` justification fields are Rust lint metadata. They do not
  change what any suppression permits, and the lint set comparison (§4, row
  27) confirms no suppression widened or narrowed.
- The replacement-package consolidation changes **packaging**, not the runtime
  code: both wheels package the same `httpx/__init__.py` from
  `compat/httpx-replacement-source/httpx/`, and both set
  `__eggfetch_shim__ = True`. The publish guard
  (`check_no_publish.py`) still passes, so the controlled replacement cannot
  be published to public PyPI as `httpx`.
- The fuzz-lock refresh narrows the dependency set rather than widening it:
  `dashmap` and six other packages were **removed** and only `base64` was
  version-bumped (0.22.1 → 0.23.1, required by the current core). The
  rejected `cargo generate-lockfile` alternative would have moved 85 packages
  including major-version bumps and was not taken.
- The compat-profile comment cleanup removed SHA strings from a machine-read
  file and added none. The one SHA that would have been lost from the
  repository (`639bf186…`) is held in three H3 records and is now also
  recorded in `docs/architecture/overview.md:266`.

## 9. Documentation and operations

| File | Change |
|---|---|
| `plans/**` (88 files) | Acceptance-criteria disposition notes and open-items sections; 1,010 boxes ticked with inline evidence; per-file and per-class counts recorded in the plan's §17 |
| `plans/README.md` | **Not changed by this milestone** — the `## Pending maintainer actions` section already existed and was verified, not edited |
| `AGENTS.md` | **Not changed by this milestone** — line 4's pointer was already accurate |
| `docs/architecture/overview.md` | The compat directory description now names `compat/httpx-replacement-source/`, states that both packages carry identical runtime code and set `__eggfetch_shim__ = True`, and records the provenance of `639bf186a71c054e11278d1b160ffe7a6f172c02` as an HTTP/3 qualification binding rather than an HTTPX one |
| `compat/httpx/0.28.1/profile.toml`, `compat/httpx2/2.12.0/profile.toml` | Comment cleanup only; the machine-readable content is unchanged |
| `compat/httpx-shim/README.md` | The shared-source mapping is documented; the false "declares a conflict with the real `httpx` package" claim is corrected |
| `compat/httpx-controlled-replacement/README.md` | The shared-source mapping is documented |

Operations:

- No new external dependency, no network access and no credential is
  required by anything this milestone changed.
- The `fuzz/` tree now builds against its refreshed lock, which it did not
  before.
- The httpx replacement packages must be rebuilt as **wheels, not sdists**,
  because the shared source lives outside each project directory and is
  mapped in through setuptools' `package-dir`. This is documented in
  `compat/httpx-shim/README.md` and in `build.sh`.
- The two rebuilt wheels carry new sha256 digests
  (`391b02c8…`, `422b1f5c…`). The old digests are not pinned anywhere in the
  repository, so no manifest needs updating for them.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| High | The live Stage C binding (`42a9c96d…`) is invalidated by this milestone's Rust-source, `fuzz/`, `compat/` and `plans/` changes, and the Stage C rebinding was deliberately not performed | No publication may cite the previous Stage C binding; `compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml` still point at the superseded SHA | Rebind both HTTPX profiles and the ledger's Stage C SHA in `plans/httpx-parity-correction-status.md` to the frozen commit that includes this milestone, then re-run the compat qualification |
| High | Tier 1, Tier 2 and Tier 3 were not run on the frozen SHA (deliberately deferred), and neither was the full workspace suite, the compat pytest suites or the Rust public-API oracle | The milestone's own targeted checks are green, but the repository-wide gates and the public-surface oracle are unproven for this tree. The plan's §13 criterion "Tier 1 and Tier 2 are green" and "both HTTPX profiles remain bound to the recorded live SHA" are **not met** | Run `./scripts/check.sh`, `./scripts/check.sh extended` and `./scripts/check.sh package` on the frozen SHA, plus `scripts/check_rust_public_api.py` and the workspace suite, before release |
| High | The plan's §6.6 **pass B** over `crates/eggfetch-core/src` was still in flight when this record was written. At the fingerprint measured, 78 of 119 core attributes carry a `reason =` (baseline 41 of 118) | The plan's §13 criterion "every `allow`/`expect` attribute in `crates/*/src` carries a justification" is **not yet satisfied** for the core crate. The count is deliberately left open rather than asserted | Finish pass B, then re-run the lint-guard self-test, the lint-set comparison of §4 row 27 over `crates/eggfetch-core/src`, and `bash scripts/check_lint_suppressions.sh`, and record the final counts |
| Medium | The plan's §6.4 evidence is wrong: it claims "eleven distinct 40-character SHA strings" in the profile comments. Measured: **1** full 40-char SHA and **16** abbreviated 8-char prefixes per profile, before the change | The plan's §1 and §2 reasoning ("eleven distinct historical SHA bindings", "161 comment lines retaining eleven distinct historical SHA bindings") is not supported by the tree. The real historical log was 16 abbreviated prefixes plus one full SHA | Amend the plan's §1/§3/§6.4 figures, or record the correction in the plan's implementation-outcome section alongside §17 |
| Medium | The plan's §1 claim that "most of those files also carry a completion record" does not hold under an in-file-marker rule. The implementer reports 31 of 83 under the in-file-marker rule and 40 of 83 under the broader "in-file **or** linked sibling status record the plan names" rule | The tier assignment is not independently reproducible from the tree without the implementer's exact marker list: a scan for explicit `## Execution record` / `## Status:` headings finds 15 and 24 of the 83 respectively. The substantive outcome is unaffected — 1,010 boxes newly ticked, 1,257 remain, 0 violations — but the per-file tiering is not re-derivable by a reader | Either publish the marker list used for the in-file rule, or restate the tier counts in terms of the measured outcome rather than the marker rule |
| Medium | The plan's §6.4 line counts ("46 / 41 non-comment lines") are counts **including** blanks; the non-blank non-comment content is 41 lines per profile | A reader diffing "46 vs 41" would see an apparent change in machine-readable content that does not exist | Amend the plan's §1/§6.4 figures to state the non-blank count |
| Medium | The plan's §6.3 implied that the fuzz tree builds after the lock refresh. It did not — it failed on two pre-existing API drifts (`fuzz_headers.rs`, `fuzz_multipart.rs`) until the main agent fixed them. The plan's §17 still says "The fuzz tree still does **not** compile" | The plan text and the tree now disagree; the tree is green (`cargo check --manifest-path fuzz/Cargo.toml --all-targets --offline` → exit 0) | Update the plan's §17 fuzz subsection to record the two API-drift fixes as the main agent's edits and the resulting green build |
| Medium | The plan's §17 fuzz-lock figure "66 dependencies unchanged" is not reproducible from the lock. Measured: 143 baseline packages → 137 current; 1 added, 7 removed, 2 version-changed, **134** unchanged | A reader cannot reconcile "66" with the lock. The substantive content — one added, seven removed, one version bump — is correct | Amend the plan's §17 figure to the measured 134 unchanged (or state the basis on which 66 was counted) |
| Medium | `crates/eggfetch-python/src/response.rs:697` — `#[allow(dead_code)]` on `response_extensions_from_core`, which has no caller anywhere in `crates/` and whose trailing comment ("Wired into the HTTPX compatibility facade") is false | A dead function with a false justification is a truthfulness defect of exactly the kind this milestone exists to remove, and it is the one attribute in pass A that could not be justified | Delete the function, or call it from the `extensions` getter, and remove the attribute. The plan's §14 assigns that code change to its own plan |
| Low | `crates/eggfetch-cli/src/main.rs:24` carries a stale `clippy::cast_precision_loss` entry — the `delay as f64` cast that motivated it is gone | The lint set is frozen for this milestone, so the entry stays; it is dead weight in a `reason =` justification that now names a cast that does not exist | Remove the lint entry and correct the `reason =` text in a follow-up |
| Low | The SHA `639bf186a71c054e11278d1b160ffe7a6f172c02` — an HTTP/3 qualification binding, not an HTTPX one — is not held by the canonical HTTPX ledger. It is held by `plans/http3-public-origin-spot-check-ledger.json:5`, `plans/http3-production-qualification-evidence.md:4` and `plans/http3-post-freeze-diagnostics-requalification-corrective-closure.md:5`, and its provenance is now recorded in `docs/architecture/overview.md:266` | The fact is not lost, but the canonical ledger does not carry it, and the plan's §14 stop condition on losing history from the ledger is only partly discharged | The ledger's owner should add a canonical entry for the H3 binding in `plans/httpx-parity-correction-status.md`, or record it in the H3 ledger's own canonical place and cross-reference it |
| Low | The working tree is under concurrent modification by another agent throughout this closure pass; the fingerprint moved from `65277dc6…` to `3cacb0a1…` during the pass, and the core lint-suppression pass was in flight | No measurement of the core justification pass can be isolated from the concurrent work, and the core tree's compile state may alternate during a verification pass | Re-run the §4 command set against the frozen commit after the concurrent work lands, and record the result against that SHA |
| Low | The plan's §12 documentation list is satisfied for `plans/README.md`, `docs/architecture/overview.md` and the two profiles, but §12 also names `plans/registry.md`, the subsystem roadmap and `plans/README.md` at closure, which are outside this record's scope | The registry and roadmap updates listed in §12 are not made by this record | The main agent must update `plans/registry.md`, `plans/subsystems/release-verification-roadmap.md` and `plans/README.md` |

## 11. Roadmap disposition

`plans/subsystems/release-verification-roadmap.md` milestone 7 moves from
`ready` to `conditionally closed`, with §11 (completion definition) and §12
(milestone status) updated in the same pass.

The milestone's own scope — the acceptance-criteria disposition, the
verified pending-maintainer-actions section, the refreshed fuzz lock, the
comment-only compat profile cleanup, the two-packages-one-source
reconciliation, and the pass-A lint justifications — is complete and measured.
What remains is the cross-cutting release evidence: the Stage C rebinding and
the Tier 1 / Tier 2 / Tier 3 run on the frozen SHA, which belong to the
release-verification subsystem's release gates rather than to this milestone,
plus the core lint-suppression pass B and the three code/documentation
follow-ups in §10.

The successor milestone (if any) is not blocked by this one.

## 12. Registry updates

`plans/registry.md`:

- The release-verification row records milestone 7 as **conditionally closed**,
  with the Stage C rebinding and the frozen-SHA tier gate run named as the
  outstanding condition.
- The implementation-plan table for M007 moves from `ready` to
  `conditionally closed` and points at this closure record.
- The blocked / operational work section records the Stage C rebinding as
  pending for every milestone closed in this pass, not for M007 alone, and
  records the outstanding pass-B lint justification over
  `crates/eggfetch-core/src`.
