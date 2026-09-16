# Post-Core Integrity Requalification and Closure

Parent program: `plans/post-maintenance-core-integrity-and-verification-program.md`
Reference compatibility contracts: HTTPX 0.28.1, HTTPX2 2.12.0, Python 3.10+
Normative verification policy: `docs/verification-policy.md`

## Objective

Close the post-maintenance integrity program on one exact executable/test/build candidate after all corrective, invariant, refactor, and validation-tooling work has landed.

This plan owns the only final compatibility renewal for the program. Earlier child plans should keep Tier 1 and focused tests green but must not repeatedly advance the Stage C ledger.

## Preconditions

Do not start final freeze until all prior child plans are implementation-complete:

1. proxy TLS route-cache identity corrective;
2. Hyper idle-pool policy corrective;
3. reusable route-cache invariant hardening;
4. Hyper client construction/cache consolidation;
5. pipeline responsibility decomposition;
6. dependency graph/validation reproducibility hardening.

Any unchecked correctness acceptance item in those plans is a blocker. Documentation-only cleanup may remain for after the freeze, but no executable/test/build/dependency/validation change may remain pending.

## 1. Perform final source audit before freezing

Review the candidate tree specifically for the program's high-risk boundaries.

### TLS/cache compatibility

Confirm:

- `TlsConfig` connection identity represents complete connection-affecting policy or an opaque immutable policy token;
- unchanged clones remain compatible;
- policy mutations cannot retain an unsafe identity;
- proxy and origin TLS route keys use the corrected identity;
- secret-bearing cache keys are not renderable.

### Reusable connector ownership

Confirm:

- no reusable connector/client stores request `remaining_total`, absolute request deadline, request read/write budgets, trace observer, failure context, body, redirect/retry state, or other logical request state;
- connection-scoped timeout/TLS/routing/pinning policy is represented by route identity or immutable client ownership;
- multi-target fallback remains request-local where required.

### Hyper idle policy

Confirm:

- idle timeout installs the required Hyper timer;
- effective idle cap/timeout is applied to persistent standard/direct/UDS/custom/SNI/SOCKS/forward/CONNECT clients;
- logical pool permits remain separate from physical idle policy.

### Refactor boundedness

Confirm:

- Hyper client construction has one common policy owner;
- route-specific connectors remain explicit;
- top-level pipeline is orchestration-oriented;
- retry/redirect/preparation/route/proxy/H3/finalization boundaries remain private and testable;
- native body execution did not inherit high-level policy accidentally.

### Verification policy

Confirm:

- cargo-deny graph covers intended optional features/targets;
- routine CI tools are pinned;
- one automatic workflow/job/no-matrix policy remains intact;
- live advisory scan remains explicit/release-time, not deterministic Tier 1.

Record this audit in the closure section of this file.

## 2. Freeze one exact candidate SHA

After source audit and local focused tests are green:

- require a clean worktree;
- record the exact commit SHA as the executable/test/build/validation freeze;
- do not modify executable code, tests, manifests, lockfile, scripts, workflows, compatibility runners, or package configuration after freezing without invalidating the candidate.

Documentation/profile/plan-index updates after qualification may descend from the freeze only if they are demonstrably non-executable.

## 3. Run focused corrective/invariant gates on the freeze

Run the exact tests introduced by the program for:

- TLS clone/mutator cache identity;
- weak -> strict and strict -> weak proxy TLS behavior for forward and CONNECT routes;
- idle timeout timer/physical reconnect;
- route-specific idle policy;
- route-key equality/isolation matrices;
- short->long and long->short total deadlines with cached route reconnect;
- cache bounds;
- H2-only/canceled-request retry/lifecycle wrappers;
- retry/redirect state preservation;
- route precedence;
- H3 discovery/fallback/suppression after module movement;
- native-body policy isolation.

Record exact commands and results.

## 4. Run canonical repository gates

On the frozen SHA run:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
./scripts/check_security.sh
```

All required checks are fail closed. Optional prerequisites/skips must remain exactly those permitted by current normative policy; do not silently convert a missing required gate into a skip.

For `check_security.sh`, record UTC scan time plus cargo-deny/cargo-audit versions and any non-failing duplicate/license warnings.

## 5. Verify feature/dependency claims

Explicitly confirm:

- MSRV Rust 1.89.0 gate passes;
- default/core minimal profiles still compile as documented;
- all intended optional feature families resolve under the updated cargo-deny graph;
- Windows conditional dependency graph is covered by policy;
- no production dependency was added solely for refactor/cache machinery unless a prior plan explicitly justified it;
- HTTP/3 remains feature-gated and experimental;
- Node remains experimental.

## 6. Run compatibility qualification

Use the repository's existing exact-SHA HTTPX qualification procedure against the frozen candidate.

Required:

- full HTTPX 0.28.1 compatibility suite;
- full HTTPX2 2.12.0 compatibility suite;
- existing API oracles;
- the repository's existing repetition policy for Stage C qualification;
- directly affected proxy/TLS/limits/timeout/streaming cases included in the compatibility result.

Do not change reference package versions in this closure.

If a compatibility difference appears:

1. classify it against existing documented contract;
2. fix executable behavior if it is a regression;
3. create a new freeze SHA;
4. rerun all affected closure gates;
5. only then update profiles/ledger.

## 7. Renew exact-SHA profiles and live ledger

Only after all gates pass:

- update `compat/httpx/0.28.1/profile.toml` to the frozen SHA/result;
- update `compat/httpx2/2.12.0/profile.toml` likewise;
- update `plans/httpx-parity-correction-status.md` with the new recorded state;
- record any existing permitted skips truthfully;
- do not claim H3 or Node support beyond their existing experimental labels.

## 8. Documentation and plan closure

Update only documentation/profile/plan files after the freeze:

- `plans/README.md` program status;
- `plans/ROADMAP.md` current product position if warranted;
- affected architecture docs for cache identity, idle pool policy, and pipeline/module ownership;
- child-plan closure records.

Perform a final descendant audit confirming all commits after the executable freeze are documentation/profile/ledger only.

If executable or validation code changes during documentation closure, invalidate the freeze and return to step 2.

## Final acceptance criteria

- [ ] All six executable child plans are complete.
- [ ] One clean exact executable/test/build/validation SHA is recorded.
- [ ] Focused TLS/cache/idle/deadline/refactor regressions pass on that SHA.
- [ ] Tier 1 passes.
- [ ] Extended validation passes.
- [ ] Package validation passes.
- [ ] Live security preflight passes with scan metadata recorded.
- [ ] MSRV/feature/dependency claims remain truthful.
- [ ] HTTPX 0.28.1 full qualification and API oracle pass under existing Stage C procedure.
- [ ] HTTPX2 2.12.0 full qualification and API oracle pass under existing Stage C procedure.
- [ ] Both compatibility profiles and live ledger bind to the same final executable SHA.
- [ ] HTTP/3 remains experimental unless a separate future qualification program changes that decision.
- [ ] Node remains experimental.
- [ ] Post-freeze descendants are documentation/profile/ledger only.

## Non-goals

No new compatibility reference version, feature expansion, release publication, H3 graduation, Node graduation, new CI topology, or unrelated dependency update belongs in this closure.
