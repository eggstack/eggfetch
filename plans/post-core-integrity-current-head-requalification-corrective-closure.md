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

- [x] Closure notes contain a file-level classification of the post-freeze delta.
- [x] Any executable production-behavior change is identified before qualification begins.
- [x] Any build/dependency/validation change is identified before qualification begins.
- [x] If categories 4 or 5 are non-empty, the candidate is not treated as a documentation/test-only descendant; qualification still may proceed, but all affected gates must be rerun and the reason must be recorded.

## 2. Freeze the current candidate

If `main` still has `1f52d846c186b061481ebb14a7be414f5c78ec7e` as its newest executable/test/build/validation input, use that SHA as the corrective qualification freeze.

The plan/index commits that introduce this handoff are documentation-only descendants and do not change the intended executable/test candidate.

If `main` gains any later executable source, tests, manifest, lockfile, build script, CI workflow, compatibility runner, or validation-script change before qualification is complete, advance the freeze to that later clean SHA and restart the affected gates. Do not bind profiles to `1f52d846...` merely because this plan names it.

Acceptance:

- [x] One exact candidate SHA is recorded before final qualification.
- [x] Worktree is clean at qualification start.
- [x] No later executable/test/build/validation change is silently excluded from the freeze.

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

- [x] All newly added/expanded invariant tests pass on the frozen SHA.
- [x] No test requires weakening a route identity distinction to pass.
- [x] No request-scoped timeout/observer/failure/body state is found in a reusable connector/client.
- [x] No connection-affecting policy dimension is found missing from a mutable route's compatibility identity.

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

- [x] HTTPX 0.28.1 oracle passes under existing policy.
- [x] HTTPX2 2.12.0 oracle passes under existing policy.
- [x] Any changed oracle result is investigated rather than added to an allowlist automatically.

## 6. Repeat full compatibility qualification on the frozen SHA

Use the repository's existing Stage C repetition rule. Run the full compatibility corpus the required number of consecutive times with no file changes between runs.

The prior program used three consecutive full runs; retain that procedure unless the normative verification policy has changed before execution.

The qualification must continue to exercise directly affected areas including proxy/TLS/limits/timeouts/streaming and both compatibility facades.

Acceptance:

- [x] Required consecutive full HTTPX/HTTPX2 compatibility runs pass on one unchanged SHA.
- [x] No new allowed difference is created solely to close the plan.
- [x] Test counts are recorded as evidence but are not treated as durable product-contract numbers.

## 7. Renew both exact-SHA compatibility profiles

Only after all focused, canonical, security, oracle, and repeated compatibility gates pass:

- update `compat/httpx/0.28.1/profile.toml` to the new qualification SHA/date;
- update `compat/httpx2/2.12.0/profile.toml` to the same SHA/date;
- preserve `bfda3889...` as historical prior evidence rather than pretending it never existed;
- state why the prior binding became historical: post-freeze test/invariant hardening changed qualification inputs;
- keep HTTP/3 and Node experimental labels unchanged.

Acceptance:

- [x] Both profiles bind to exactly the same current executable/test qualification SHA.
- [x] The reference versions remain HTTPX 0.28.1 and HTTPX2 2.12.0.
- [x] Profile prose distinguishes API compatibility from experimental H3/Node status.

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

- [x] Final descendant audit is recorded.
- [x] Post-freeze commits are documentation/profile/ledger only.
- [x] Pushed routine CI is green for the final closure head.

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

- [x] Post-`bfda3889` delta is classified and understood.
- [x] One current executable/test qualification SHA is frozen.
- [x] Focused reusable-route/cache invariants pass on that SHA.
- [x] Tier 1 passes on that SHA.
- [x] Extended validation including Rust 1.89.0 MSRV passes on that SHA.
- [x] Package validation passes on that SHA.
- [x] Live security preflight passes with timestamp/tool versions recorded.
- [x] HTTPX 0.28.1 API oracle passes.
- [x] HTTPX2 2.12.0 API oracle passes.
- [x] Required consecutive full compatibility runs pass without an intervening file change.
- [x] Both compatibility profiles are renewed to the same qualification SHA.
- [x] Live parity ledger is renewed to the same qualification SHA.
- [x] HTTP/3 remains experimental.
- [x] Node remains experimental.
- [x] Final pushed descendant contains only documentation/profile/ledger closure changes.
- [x] Routine GitHub CI for the final closure head is green.

## Closure record (executed 2026-09-16)

### 1. Post-freeze delta classification (`bfda3889..1f52d846`)

Four commits: three documentation/profile/ledger closures
(`19522953`, `52219326`, `9e5c68c3`) plus the route-cache invariant
hardening commit (`1f52d846`). File-level classification:

| File(s) | Category |
|---|---|
| `plans/*`, `compat/*/profile.toml` (prior binding), `compat/*/README.md`, `docs/architecture/*` (prior SHA wording), `docs/reference/*`, `plans/ROADMAP.md`, `.skills/*`, `AGENTS.md` | 1 — documentation/profile/ledger only |
| `crates/eggfetch-core/src/client.rs` (`sni_clients` field docs), `crates/eggfetch-core/src/transport/hyper_client.rs` (route/client inventory rustdoc), `crates/eggfetch-core/src/transport/socks.rs` (`SocksRouteKey`/`SocksConnector` docs) | 2 — production source comments/rustdoc only (verified: hunks are `///` doc lines) |
| `crates/eggfetch-core/src/transport/connect.rs`, `proxy.rs`, `socks.rs` (`mod tests` matrices), `crates/eggfetch-core/tests/proxy_tests.rs` (new trace-observer regression) | 3 — tests/test-only code (verified: all non-doc hunks are inside `mod tests` or the integration test file) |
| Executable production behavior (category 4) | empty — no production statement changed |
| Build/dependency/validation behavior (category 5) | empty — no manifest, lockfile, workflow, or script changed |

The candidate is therefore a test/documentation-only descendant of the
prior freeze; nevertheless all affected gates were rerun below per the
plan's freeze rule.

### 2. Freeze

- Candidate executable/test freeze:
  `1f52d846c186b061481ebb14a7be414f5c78ec7e`.
- Qualification ran on `5a08c87ea8ada43a2646680e9231e9e3984e2f4b`
  (`main` at execution start), which is exactly one documentation-only
  plan-handoff file ahead of the freeze
  (`plans/post-core-integrity-current-head-requalification-corrective-closure.md`,
  +260 lines, no other content). Worktree was clean at qualification
  start and no later executable/test/build/validation change occurred
  during qualification.

### 3. Focused route-cache/invariant qualification

- `connection_identity` lib filter: 5 passed (TLS clone identity and
  policy-mutation fragmentation, incl.
  `unchanged_clone_shares_connection_identity`,
  `connection_identity_covers_policy_dimensions`).
- `route_key` lib filter: 4 passed
  (`connect_route_key_isolates_tls_policy_and_reuses_compatible`,
  `forward_route_key_isolates_tls_policy_and_reuses_compatible`,
  `forward_route_key_fragments_on_connection_policy`,
  `socks_route_key_compatibility_matrix`).
- `bounded_cache` lib filter: 3 passed (reusable-cache size bounds).
- `tls` lib filter: 47 passed.
- `proxy_tests` focused set: 9 passed
  (`hyper_forward_proxy_reuses_keep_alive_connection`,
  `hyper_connect_proxy_reuses_keep_alive_tunnel`,
  `hyper_connect_proxy_does_not_reuse_short_total_on_reconnect`,
  `hyper_connect_proxy_reconnect_honors_short_current_total`,
  `forward_proxy_per_request_read_budget_is_not_retained`,
  `forward_proxy_per_request_trace_observer_is_not_retained` (new),
  `forward_proxy_weak_then_strict_does_not_reuse_weak_route`,
  `connect_proxy_weak_then_strict_does_not_reuse_weak_route`,
  `pinned_connect_target_uses_ip_but_preserves_origin_tls_identity`).
- `pool_tests::test_sni_cached_client_obeys_idle_policy`: passed.
- Deterministic H3 suites re-passed on the current tree:
  `h3_hardening` 12/12, `h3_alt_svc_discovery` 17/17,
  `h3_interop_qualification` 20/20.
- No test weakened a route identity distinction; no request-scoped
  timeout/observer/failure/body state was found in a reusable
  connector/client; no connection-affecting policy dimension was found
  missing from a mutable route's compatibility identity.

### 4. Canonical repository gates

- `./scripts/check.sh` (Tier 1): passed, including compat smoke kernel
  (133 passed). One explicit skip: Node JS surface (native artifact not
  built), per prototype policy.
- `./scripts/check.sh extended` (Tier 2): passed, including the exact
  Rust 1.89.0 MSRV gate and the full compatibility suite. Two explicit
  skips, both allowed by current policy and unchanged from the prior
  qualification: missing Node JS artifact and downstream artifact
  manifest.
- `./scripts/check.sh package` (Tier 3): passed (crate dry-run, wheel
  build `eggfetch-0.1.4-cp312-cp312-manylinux_2_34_x86_64.whl`, wheel
  smoke, package-content, installed-wheel typing).
- `./scripts/check_security.sh`: passed at 2026-09-16T19:00:19Z with
  cargo-deny 0.19.0 and cargo-audit 0.22.2; advisories, bans, licenses,
  and sources ok.

### 5. API oracles

- HTTPX 0.28.1: 71 differences / 71 allowed matches, 0 stale allowed,
  0 unexplained, 0 resolved-in-active. Passes under existing policy.
- HTTPX2 2.12.0: 79 differences / 79 allowed matches, 0 stale allowed,
  0 unexplained, 0 resolved-in-active. Passes under existing policy.
- No oracle result changed relative to the prior binding, so no
  allowlist change was needed or made.

### 6. Repeated full compatibility qualification

`EGGFETCH_COMPAT_REQUIRED=1 python -m pytest
crates/eggfetch-python/tests/compat/ -q --strict-markers`, three
consecutive runs on the unchanged tree (HEAD pinned before run 1 and
verified unchanged after run 3; worktree clean throughout):

- Run 1/3: 1870 passed, 26 warnings in 256.24s.
- Run 2/3: 1870 passed, 26 warnings in 244.63s.
- Run 3/3: 1870 passed, 26 warnings in 246.60s.

Warnings are the existing non-failing HTTPX/SQL/TLS deprecations.
Zero skips/xfails/failures. No new allowed difference was created.
Counts are evidence only, not product-contract numbers.

### 7. Profile renewal

- `compat/httpx/0.28.1/profile.toml` and
  `compat/httpx2/2.12.0/profile.toml` both bind
  `qualification-sha = "1f52d846c186b061481ebb14a7be414f5c78ec7e"` /
  `qualification-date = "2026-09-16"` with
  `previous-qualification-sha = "bfda3889cbeff5f6fbd98bf3eee12f77fab301c4"`
  preserved as historical prior evidence and prose stating why the prior
  binding became historical.
- Reference versions unchanged: HTTPX 0.28.1 and HTTPX2 2.12.0.
- Profile prose keeps API compatibility separate from experimental
  H3/Node status; both compat READMEs were renewed to the new SHA.

### 8. Ledger and index renewal

- `plans/httpx-parity-correction-status.md`: new live section bound to
  `1f52d846`, preserving `bfda3889` history.
- `plans/README.md`: new corrective entry; the completed-program entry
  notes its `bfda3889` binding is superseded.
- Compatibility reference docs (`docs/reference/compatibility.md`,
  `docs/reference/compatibility-stage-decision.md`), the H3 graduation
  record (`docs/architecture/core-tls-proxy-protocols.md`), both skills
  (`.skills/rust-development.md`, `.skills/documentation.md`), and
  `plans/ROADMAP.md` were renewed to the current freeze SHA. `README.md`
  and `AGENTS.md` needed no change: both already reference the live
  ledger instead of hardcoding a SHA (volatile counts were pruned in the
  prior pass).

### 9. Post-freeze descendant rule and remote CI

- HTTP/3 remains experimental; the Node binding remains an experimental
  prototype. Neither status changed in this corrective.
- Final descendant audit: `1f52d846..9112f6ef` contains only
  documentation/profile/ledger changes (2 compat `profile.toml`, 2 compat
  READMEs, `plans/*` ledger/index/closure, `docs/reference/*`,
  `docs/architecture/core-tls-proxy-protocols.md`, `.skills/*`,
  `plans/ROADMAP.md`). No Rust/Python source, test, manifest, lockfile,
  build script, CI workflow, compatibility runner, or validation script
  changed after the freeze.
- Pushed routine CI for the final closure head is green: workflow `CI`,
  run `35140415963`, head
  `9112f6ef1be0f7069ef82398a3c3ddf1e6f8058c` (a
  documentation/profile/ledger descendant of the executable freeze, so the
  run covers the frozen executable tree): conclusion success
  (19:24–19:34Z, ~10min). The only annotation is the repository's
  existing GitHub Actions Node.js 20 deprecation notice; no validation
  step failed.

## Closure rule

Once all acceptance criteria are checked, this corrective and the post-maintenance core-integrity line are closed. Do not create another follow-up architecture/parity program merely because the qualification SHA advanced. Reopen only for a newly demonstrated defect, a deliberate product-scope change, a reference-compatibility version change, or a future explicit H3/Node maturation decision.
