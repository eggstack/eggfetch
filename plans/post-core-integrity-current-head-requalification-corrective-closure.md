# Post-Core Integrity Current-Head Requalification Corrective Closure

Planning baseline / intended qualification candidate: `1f52d846c186b061481ebb14a7be414f5c78ec7e` (`main`, 2026-09-16)
Prior qualified executable freeze: `bfda3889cbeff5f6fbd98bf3eee12f77fab301c4`
Parent completed program: `plans/post-maintenance-core-integrity-and-verification-program.md`
Prior closure: `plans/post-core-integrity-requalification-and-closure.md`
Reference compatibility contracts: `httpx==0.28.1`, `httpx2==2.12.0`, Python 3.10+
Normative verification policy: `docs/verification-policy.md`

## Objective

Restore exact-SHA qualification truth after the completed post-maintenance core-integrity program received a follow-up route-cache invariant test/documentation hardening commit after its recorded executable freeze.

This is a **qualification corrective only**. Current evidence does not justify another architecture, parity, dependency, transport, HTTP/3, Node, or feature-development pass. The intended outcome is one clean current-head candidate whose focused route-cache tests, canonical repository gates, security preflight, API oracles, and full compatibility runs are all recorded against the same SHA, followed by documentation/profile/ledger closure only.

Do not change production behavior merely to make this closure convenient. If any gate exposes an actual defect, stop this closure, fix the defect in a separately attributable executable commit, select a new freeze SHA, and restart the affected qualification sequence on that new candidate.

## Trigger and current state

The completed program qualified executable freeze `bfda3889cbeff5f6fbd98bf3eee12f77fab301c4` and renewed both compatibility profiles to that SHA after Tier 1, extended, package, live security, both API oracles, and three consecutive full compatibility passes.

Current `main` is `1f52d846c186b061481ebb14a7be414f5c78ec7e`, four commits ahead of that freeze. The post-freeze sequence contains documentation/profile/ledger closure plus follow-up route-cache invariant hardening. The latest commit:

- expands SOCKS / forward / CONNECT route-key equality-isolation matrices;
- adds route/client ownership documentation and reusable-cache invariants;
- adds a forward-proxy trace-observer non-retention regression;
- documents SNI immutable-owner keying and SOCKS deadline ownership;
- reports Tier 1 green;
- has a successful pushed GitHub CI run.

The follow-up appears test/rustdoc/source-comment oriented rather than a production behavior change, but it modifies tests and source files after the recorded freeze. Eggfetch's exact-SHA closure rule treats executable/test/build/validation changes after a freeze as invalidating that qualification binding. Therefore the currently recorded Stage C profiles at `bfda3889...` are historically valid evidence but are not exact-current-head evidence.

## Scope constraints

This corrective must remain narrow:

- no feature acquisition;
- no public API changes;
- no new production dependency;
- no route-cache redesign unless a test proves a correctness defect;
- no pipeline restructuring;
- no HTTP/3 graduation work;
- no Node maturation work;
- no new CI workflow, matrix, scheduler, or publication authority;
- no compatibility reference-version change;
- no broad dependency update;
- no benchmark-driven optimization pass.

The current one-workflow/one-job/no-routine-matrix policy remains authoritative. Live advisory checking remains explicit/release-time rather than routine deterministic CI.

## 1. Reconcile the prior freeze to current head

Before running expensive qualification, compare:

```text
bfda3889cbeff5f6fbd98bf3eee12f77fab301c4
..
1f52d846c186b061481ebb14a7be414f5c78ec7e
```

Classify every changed file into one of:

1. documentation/profile/ledger only;
2. production source comments/rustdoc only;
3. tests/test-only code;
4. executable production behavior;
5. build/dependency/validation behavior.

The expected classification is categories 1-3 only. Explicitly inspect the changed regions in:

- `crates/eggfetch-core/src/client.rs`;
- `crates/eggfetch-core/src/transport/connect.rs`;
- `crates/eggfetch-core/src/transport/hyper_client.rs`;
- `crates/eggfetch-core/src/transport/proxy.rs`;
- `crates/eggfetch-core/src/transport/socks.rs`;
- `crates/eggfetch-core/tests/proxy_tests.rs`.

Acceptance:

- [ ] Closure notes contain a file-level classification of the post-freeze delta.
- [ ] Any executable production-behavior change is identified before qualification begins.
- [ ] Any build/dependency/validation change is identified before qualification begins.
- [ ] If categories 4 or 5 are non-empty, the candidate is not treated as a documentation/test-only descendant; qualification still may proceed, but all affected gates must be rerun and the reason must be recorded.

## 2. Freeze the current candidate

If `main` still has `1f52d846c186b061481ebb14a7be414f5c78ec7e` as its newest executable/test/build/validation input, use that SHA as the corrective qualification freeze.

The plan/index commits that introduce this handoff are documentation-only descendants and do not change the intended executable/test candidate.

If `main` gains any later executable source, tests, manifest, lockfile, build script, CI workflow, compatibility runner, or validation-script change before qualification is complete, advance the freeze to that later clean SHA and restart the affected gates. Do not bind profiles to `1f52d846...` merely because this plan names it.

Acceptance:

- [ ] One exact candidate SHA is recorded before final qualification.
- [ ] Worktree is clean at qualification start.
- [ ] No later executable/test/build/validation change is silently excluded from the freeze.

## 3. Run the focused route-cache/invariant qualification first

Run the narrow tests that motivated the post-freeze change before the full repository suite.

At minimum cover:

- TLS policy clone identity and policy mutation fragmentation;
- SOCKS route-key connection-policy isolation and request-policy non-fragmentation;
- forward-proxy route-key isolation for endpoint/auth/headers/pinning/origin/connect/proxy-TLS timeout dimensions;
- CONNECT route-key isolation for proxy identity, origin, pinned target, origin TLS identity, SNI, HTTP-version policy, and connect-phase timeout dimensions;
- different `Timeout.total` values sharing a compatible reusable route;
- short -> long and long -> short total-budget reconnect regressions;
- per-request read budget not retained by reusable proxy clients/connectors;
- forward-proxy trace observer not retained by a reusable connector;
- reusable-cache size bounds;
- SNI cache assumptions that depend on immutable client-owned connection policy.

Prefer exact test names from the current tree and record them in closure notes rather than inventing a second test harness.

Acceptance:

- [ ] All newly added/expanded invariant tests pass on the frozen SHA.
- [ ] No test requires weakening a route identity distinction to pass.
- [ ] No request-scoped timeout/observer/failure/body state is found in a reusable connector/client.
- [ ] No connection-affecting policy dimension is found missing from a mutable route's compatibility identity.

## 4. Re-run canonical repository gates on the freeze

Run, on the same exact candidate:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
./scripts/check_security.sh
```

Required evidence:

- Tier 1 passes;
- extended validation passes, including Rust 1.89.0 MSRV and feature combinations;
- package validation passes;
- live security preflight passes;
- security record includes UTC scan time and cargo-deny / cargo-audit versions;
- any permitted optional skips are exactly those allowed by current verification policy and are recorded truthfully.

Do not reinterpret an unexpected missing prerequisite as an optional skip.

## 5. Re-run both compatibility API oracles

Run the repository's existing API-oracle process for:

- HTTPX 0.28.1;
- HTTPX2 2.12.0.

Expected outcome is no new unexplained or stale difference introduced by the route-cache test/documentation follow-up.

Acceptance:

- [ ] HTTPX 0.28.1 oracle passes under existing policy.
- [ ] HTTPX2 2.12.0 oracle passes under existing policy.
- [ ] Any changed oracle result is investigated rather than added to an allowlist automatically.

## 6. Repeat full compatibility qualification on the frozen SHA

Use the repository's existing Stage C repetition rule. Run the full compatibility corpus the required number of consecutive times with no file changes between runs.

The prior program used three consecutive full runs; retain that procedure unless the normative verification policy has changed before execution.

The qualification must continue to exercise directly affected areas including proxy/TLS/limits/timeouts/streaming and both compatibility facades.

Acceptance:

- [ ] Required consecutive full HTTPX/HTTPX2 compatibility runs pass on one unchanged SHA.
- [ ] No new allowed difference is created solely to close the plan.
- [ ] Test counts are recorded as evidence but are not treated as durable product-contract numbers.

## 7. Renew both exact-SHA compatibility profiles

Only after all focused, canonical, security, oracle, and repeated compatibility gates pass:

- update `compat/httpx/0.28.1/profile.toml` to the new qualification SHA/date;
- update `compat/httpx2/2.12.0/profile.toml` to the same SHA/date;
- preserve `bfda3889...` as historical prior evidence rather than pretending it never existed;
- state why the prior binding became historical: post-freeze test/invariant hardening changed qualification inputs;
- keep HTTP/3 and Node experimental labels unchanged.

Acceptance:

- [ ] Both profiles bind to exactly the same current executable/test qualification SHA.
- [ ] The reference versions remain HTTPX 0.28.1 and HTTPX2 2.12.0.
- [ ] Profile prose distinguishes API compatibility from experimental H3/Node status.

## 8. Renew the live ledger and closure records

Update:

- `plans/httpx-parity-correction-status.md`;
- this corrective closure plan with exact commands/results;
- `plans/README.md` status for this corrective;
- any compatibility README whose current-SHA wording would otherwise be stale.

The final closure record must include:

- frozen SHA;
- focused invariant result;
- Tier 1 result;
- extended/MSRV result;
- package result;
- security result with scan/tool metadata;
- API-oracle results;
- repeated compatibility results;
- pushed GitHub CI result for the final documentation/profile descendant when available;
- explicit statement that H3 and Node status did not change.

## 9. Enforce a clean post-freeze descendant rule

After the new executable/test qualification freeze is established, only documentation/profile/ledger/plan-index changes may descend from it without another requalification.

Perform a final diff from the frozen SHA to the pushed closure head. If it includes executable code, tests, manifests, lockfiles, build scripts, CI workflows, compatibility runners, or validation scripts, the closure is invalid and must return to the freeze step.

Acceptance:

- [ ] Final descendant audit is recorded.
- [ ] Post-freeze commits are documentation/profile/ledger only.
- [ ] Pushed routine CI is green for the final closure head.

## Expected files changed by implementation

Qualification-only execution should primarily change:

- `compat/httpx/0.28.1/profile.toml`;
- `compat/httpx2/2.12.0/profile.toml`;
- `plans/httpx-parity-correction-status.md`;
- `plans/post-core-integrity-current-head-requalification-corrective-closure.md`;
- `plans/README.md`;
- narrowly affected compatibility README text if needed.

Unexpected changes to Rust/Python production source, tests, manifests, lockfiles, workflow files, or validation scripts are a scope-expansion signal. If such changes are required by a discovered defect, make the corrective explicit, select a new freeze, and rerun affected qualification.

## Final acceptance criteria

- [ ] Post-`bfda3889` delta is classified and understood.
- [ ] One current executable/test qualification SHA is frozen.
- [ ] Focused reusable-route/cache invariants pass on that SHA.
- [ ] Tier 1 passes on that SHA.
- [ ] Extended validation including Rust 1.89.0 MSRV passes on that SHA.
- [ ] Package validation passes on that SHA.
- [ ] Live security preflight passes with timestamp/tool versions recorded.
- [ ] HTTPX 0.28.1 API oracle passes.
- [ ] HTTPX2 2.12.0 API oracle passes.
- [ ] Required consecutive full compatibility runs pass without an intervening file change.
- [ ] Both compatibility profiles are renewed to the same qualification SHA.
- [ ] Live parity ledger is renewed to the same qualification SHA.
- [ ] HTTP/3 remains experimental.
- [ ] Node remains experimental.
- [ ] Final pushed descendant contains only documentation/profile/ledger closure changes.
- [ ] Routine GitHub CI for the final closure head is green.

## Closure rule

Once all acceptance criteria are checked, this corrective and the post-maintenance core-integrity line are closed. Do not create another follow-up architecture/parity program merely because the qualification SHA advanced. Reopen only for a newly demonstrated defect, a deliberate product-scope change, a reference-compatibility version change, or a future explicit H3/Node maturation decision.
