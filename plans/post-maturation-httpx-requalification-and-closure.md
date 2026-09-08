# Post-Maturation HTTPX Requalification and Closure

Planning baseline: `48f4457069d77528bd21fb342ff6ee4f27e3ae4f` (`main`, 2026-09-04)
Parent program: `plans/post-audit-architecture-and-surface-maturation-program.md`
Prior qualified executable SHA: `d24101be6ed7be64463813750da5b4043d9905ec` (2026-09-03)
Reference contract: `httpx==0.28.1` / pinned compatibility requirements, Python 3.10+, asyncio-supported public surface
Live ledger: `plans/httpx-parity-correction-status.md`
Depends on completion of all executable plans in the parent program
Followed by: `plans/post-maturation-documentation-and-plan-hygiene.md`

## Objective

Freeze and requalify one final post-maturation executable/test SHA against the existing HTTPX 0.28.1 Stage C procedure. This pass is the only point in the post-audit maturation program where the repository may renew the current Stage C claim.

The prior qualification at `d24101b...` remains valid historical evidence for that executable snapshot only. It does not establish compatibility for current or future `main` after executable changes.

This plan must reuse the repository's existing qualification machinery rather than inventing a new evidence format, CI workflow or compatibility stage.

## Governing rule

> Every required qualification gate must pass against one exact frozen executable/test SHA. Any later change to source, tests, manifests, dependencies, qualification scripts, package configuration, compatibility-oracle code or downstream qualification behavior invalidates the collected evidence and requires a new freeze.

Documentation-only descendants may follow after qualification if the executable tree remains identical.

# 1. Confirm prerequisite plan closure

Before selecting a candidate SHA, verify that the intended executable work is complete:

- `core-request-and-transport-consolidation.md` closed;
- `http3-lifecycle-and-policy-hardening.md` closed;
- `node-binding-maturation.md` closed with an explicit supported/experimental outcome;
- `native-protocol-observability-and-api-cleanup.md` closed;
- no known correctness defect from those passes remains intentionally deferred without documentation.

Review the resulting change set from the prior qualification SHA to candidate HEAD and group qualification-sensitive changes by semantics:

1. request reconstruction / retry / redirect preservation;
2. transport dispatch and Hyper response conversion;
3. H3 timeout/cache/DNS/resource behavior;
4. trailer/response lifecycle;
5. connection metadata/trace/metrics;
6. native limits/API aliases;
7. Python adapter changes caused by core surfaces;
8. Node/FFI changes affecting shared core build/runtime;
9. compatibility tests or facade changes;
10. validation/package/manifests/dependencies.

Acceptance:

- [ ] every behavioral cluster has focused regression evidence;
- [ ] no known required compatibility regression is deferred into final qualification;
- [ ] current HEAD is not described as newly Stage C qualified before this plan completes.

# 2. Focused pre-freeze semantic gate

Run the directly affected tests before freezing. At minimum include:

## Request-state preservation

- retry preservation of transport hints, proxy/decompression/auth-disable/timeout state;
- redirect clearing of destination-specific hints;
- cross-origin auth/cookie stripping;
- one-shot body replay rejection;
- no-redirect first-hop equivalence.

## Transport consolidation

- standard direct vs specialized direct response equivalence;
- SNI/local-address/socket-option routes;
- UDS H1/H2 behavior;
- proxy/SOCKS route precedence;
- 101 upgrade/network-stream behavior;
- trace callback success/failure/abort boundaries.

## HTTP/3

- multi-address fallback;
- connect vs total timeout classification;
- cache boundedness/eviction/reconnect;
- cancellation;
- request/response streaming timeout behavior;
- pool permit lifecycle.

## Response protocol/observability

- H1/H2/H3 trailers where supported;
- partial-body close/drop;
- real vs unavailable connection metadata;
- transport metrics exactness in deterministic fixtures;
- native concurrency aliases without HTTPX facade signature drift.

## Python compatibility-sensitive boundaries

Run the affected HTTPX files for:

- client/request configuration;
- redirects;
- auth/cookies;
- timeout conversion;
- proxy/SOCKS/TLS;
- response/raw iteration;
- extensions/trace/network stream;
- limits/config objects if native API work touched shared conversion code.

Acceptance:

- [ ] focused gate passes before freeze;
- [ ] failures are fixed rather than waived;
- [ ] any test/build/validation fix is committed before freeze.

# 3. Freeze the executable/test candidate

After the focused gate:

```sh
./scripts/check.sh
git status --short
git rev-parse HEAD
```

Requirements:

- worktree clean;
- record full 40-character `FROZEN_EXECUTABLE_SHA`;
- no uncommitted generated bindings/manifests/declarations;
- all package manifests and lockfiles committed;
- if Node declaration generation or validation scripts changed, those exact versions are part of the frozen evidence.

If any qualification-sensitive file changes afterward, abandon the evidence and create a new freeze.

Acceptance:

- [ ] one exact candidate SHA exists;
- [ ] worktree is clean;
- [ ] the candidate includes every intended executable/test change.

# 4. Tier 1 on the frozen SHA

Run:

```sh
./scripts/check.sh
```

Record in the live status ledger or the existing evidence location:

- exact SHA;
- Rust/Python versions;
- test counts where available;
- skips/warnings;
- Node validation status if it has become part of the existing check path.

Acceptance:

- [ ] Tier 1 passes on the exact frozen SHA.

# 5. Tier 2 extended verification

Install the existing pinned compatibility requirements and run:

```sh
python -m pip install -r compat/httpx/0.28.1/requirements.txt
./scripts/check.sh extended
```

The existing extended path remains authoritative for its current contents. Do not edit `scripts/check.sh` while collecting evidence. If the script is found to be wrong, fix it, commit, select a new freeze and restart from Section 3.

Acceptance:

- [ ] extended validation passes on the same SHA;
- [ ] any policy-permitted optional skip is recorded explicitly;
- [ ] no failure is dismissed based on Tier 1 being green.

# 6. Tier 3 package validation

Run:

```sh
./scripts/check.sh package
```

Do not publish.

Acceptance:

- [ ] package validation passes from the same frozen tree;
- [ ] crate packaging and wheel smoke remain valid;
- [ ] no dirty-tree overrides are used.

# 7. Three consecutive full HTTPX compatibility runs

Run three consecutive times without changing files or dependencies:

```sh
EGGFETCH_COMPAT_REQUIRED=1 python -m pytest \
  crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Record pass/fail/skip/xfail counts and durations for each run.

Rules:

- no selecting only successful runs around an unexplained flake;
- no test modifications between runs;
- any nondeterminism requiring code/test changes creates a new frozen SHA and restarts qualification.

Acceptance:

- [ ] three consecutive runs pass;
- [ ] no unexplained skip/xfail masks required Stage C behavior.

# 8. API oracle and allowed-difference reconciliation

Run the existing generated API manifest/oracle workflow from `scripts/check.sh extended` or its canonical direct commands.

Review active differences, especially any surfaces indirectly touched by this program:

- `Limits` signature/properties in the HTTPX facade must remain pinned even if native Rust naming changes;
- response/trailer additions must not appear as accidental HTTPX public API differences unless deliberately exposed and reference-compatible;
- connection metadata/trace extensions must remain within the documented extension contract;
- Node/native additions must not contaminate Python compatibility manifests.

Acceptance:

- [ ] zero unexplained API differences;
- [ ] retained allowed differences remain intentional, tested and current;
- [ ] no stale resolved row remains active.

# 9. High-risk differential evidence

Ensure direct reference/candidate evidence exists for:

- redirect/auth/cookie state;
- timeout objects and phase behavior;
- proxy environment / NO_PROXY / SOCKS semantics;
- TLS SSLContext translation;
- H2-only behavior;
- response raw/stream consumed/closed transitions;
- network-stream extensions;
- configuration object signatures affected by aliasing work.

New native-only features such as H3 cache metrics do not need HTTPX parity evidence unless they affect the facade.

Acceptance:

- [ ] compatibility-sensitive changes are supported by differential evidence, not candidate-only tests.

# 10. Downstream qualification

Run the repository's existing required downstream portfolio using the same frozen candidate artifact/procedure referenced by current verification policy and status ledger.

If the existing downstream gate requires an artifact manifest, generate/use it according to the established process; do not bypass the gate by treating its absence as qualification evidence.

Acceptance:

- [ ] all currently required downstream fixtures pass;
- [ ] artifact/SHA identity is recorded consistently.

# 11. Remote CI evidence

Push the exact frozen candidate (or its qualification-record docs-only descendant as required by the existing procedure) and confirm the normal GitHub CI run is green for the executable SHA.

Do not create a new workflow solely for this plan.

Acceptance:

- [ ] normal CI is green for the exact executable tree being qualified.

# 12. Renew the qualification ledger/profile

Only after all gates pass:

- update `compat/httpx/0.28.1/profile.toml` qualification SHA/date;
- update `plans/httpx-parity-correction-status.md` with exact evidence;
- update compatibility references that explicitly name the previous current qualification SHA;
- preserve prior qualification SHAs as historical evidence;
- clearly distinguish executable qualification SHA from later docs-only record commits.

This record commit should be documentation/ledger-only. Verify with a diff that no executable/test/build/validation/package file changed.

Acceptance:

- [ ] profile SHA equals the frozen executable SHA;
- [ ] evidence ledger names every required gate and result;
- [ ] prior qualifications remain historical rather than overwritten;
- [ ] record commit is docs/ledger-only.

# 13. Descendant audit

After the qualification-record commit:

```sh
git diff --name-status FROZEN_EXECUTABLE_SHA..HEAD
```

Classify every descendant file. If any qualification-sensitive executable/test/build/validation/package file changed, the qualification is invalid and must be repeated.

Acceptance:

- [ ] descendant audit proves executable identity is unchanged;
- [ ] broad docs cleanup may proceed only after this check.

## Non-goals

- no HTTPX version upgrade;
- no Stage D expansion merely because the suite is green;
- no new CI architecture;
- no release publication;
- no speculative elimination of intentional differences;
- no executable refactor during evidence collection.

## Exit criteria

- [ ] one exact post-maturation executable SHA passes focused, Tier 1, Tier 2 and Tier 3 validation;
- [ ] three consecutive full compatibility runs pass;
- [ ] API oracle and downstream gates pass;
- [ ] remote CI passes;
- [ ] profile/status ledger are rebound to that SHA;
- [ ] descendant audit confirms no executable drift;
- [ ] Stage C claim is once again truthful for the documented exact executable tree.
