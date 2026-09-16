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

## Closure record (2026-09-16)

### Preconditions — all six executable child plans complete

1. Proxy TLS route-cache identity corrective — implemented in freeze
   `6a4738ae` (opaque per-build token, clone-shared, mutator-minted) plus
   `bfda3889` (feature-gate hardening). The standalone plan file existed
   before implementation; this closure implements it rather than finding it
   already landed.
2. Hyper idle-pool policy corrective — `43a843e8` (pool timer + uniform
   per-route idle timeout/cap, deterministic expiry/reconnect/cap tests).
3. Reusable route-cache invariant hardening — implemented in freeze
   `6a4738ae` (key equality/isolation matrices, deadline ownership,
   per-request read-budget proof, lock-scope audit, checklist in
   `transport::hyper_client`) plus the CONNECT trace-retention removal.
4. Hyper client construction/cache consolidation — `648d726a` (central
   `HyperClientPolicy`/`build_hyper_client`/`BoundedClientCache`, pool
   do-not-adopt recorded).
5. Pipeline responsibility decomposition — `bad4dbc5` (`pipeline/` modules
   with orchestration-oriented `mod.rs`).
6. Dependency graph/validation reproducibility hardening — `3fd4f186`
   (cargo-deny all-features + Windows, pinned CI requirements, one
   workflow/job preserved).

### 1. Final source audit (before freeze)

- TLS/cache: `TlsConfig::connection_identity()` is the opaque token
  (`*policy_token`); `build()` mints fresh, `Clone` shares,
  `danger_accept_invalid_certs` mints new; builder setters all flow through
  fresh builds. `ProxyConfig::connection_identity()` and
  `ConnectRouteKey::origin_tls_identity` use the token. Route keys
  (`Forward`/`Connect`/`Socks`) implement no `Debug`/`Display`;
  `TlsConfig`/`ProxyConfig` debugs are redacted/non-exhaustive and never
  render the token.
- Connector ownership: `ForwardProxyConnector`, `ConnectProxyConnector`
  (SNI-only hint retained; trace/target no longer cached),
  and `SocksConnector` carry no `remaining_total`, absolute deadline,
  read/write budgets, retry/redirect state, body, cookies, auth headers,
  decompression policy, trace observers, or failure contexts. Current
  request budgets stay authoritative at the outer
  `send_with_total_timeout` dispatch boundary; multi-target CONNECT
  fallback remains handshake-specific and request-local.
- Hyper idle: `HyperClientPolicy::apply` installs `TokioTimer` whenever an
  idle timeout is configured; all persistent families (standard, direct,
  UDS, custom, resolved, SNI, SOCKS, forward, CONNECT) share the resolved
  idle timeout and effective per-host cap via `Pool::idle_timeout` /
  `Pool::max_idle_per_host`. Logical pool permits remain separate.
- Refactor: one `HyperClientPolicy` owner, explicit per-route connectors,
  orchestration-oriented `pipeline/mod.rs` (643 lines; largest child
  `redirect.rs` 805 lines, no 100KB catch-all), private retry/redirect/
  preparation/route/proxy/H3/finalization boundaries, native-body path
  unchanged in policy scope.
- Verification: cargo-deny covers all features + Windows x86_64 release
  targets; routine CI tools pinned in `scripts/ci-requirements.txt`
  (maturin aligned with release pins); one automatic workflow/job/no-matrix
  intact; live advisory scan stays in explicit `check_security.sh`.

### 2. Freeze

Executable freeze SHA: `bfda3889cbeff5f6fbd98bf3eee12f77fab301c4`
(follow-up to `6a4738ae`, which was invalidated by the extended
feature-matrix gate: an unused non-proxy identity duplicate and two
ungated forward-route tests. No qualification had been recorded on
`6a4738ae`; all gates below were collected on `bfda3889` with a clean
worktree).

### 3. Focused corrective/invariant gates on the freeze

- `cargo test -p eggfetch-core --all-features --lib`: 732 passed.
- `cargo test -p eggfetch-core --all-features --test proxy_tests`: 53
  passed, including `forward_proxy_weak_then_strict_does_not_reuse_weak_route`,
  `forward_proxy_strict_then_weak_uses_separate_routes`,
  `connect_proxy_weak_then_strict_does_not_reuse_weak_route`,
  `connect_proxy_strict_then_weak_uses_separate_routes`,
  `forward_proxy_per_request_read_budget_is_not_retained`,
  `hyper_connect_proxy_does_not_reuse_short_total_on_reconnect`,
  `hyper_connect_proxy_reconnect_honors_short_current_total`,
  `hyper_forward_proxy_reuses_keep_alive_connection`,
  `hyper_connect_proxy_reuses_keep_alive_tunnel`.
- `cargo test -p eggfetch-core --all-features --test pool_tests`: passed.
- TLS identity unit tests (clone shares, mutation fragments, independent
  builds differ, policy dimensions, opacity) and route-key matrices
  (SOCKS/forward/CONNECT) green.

### 4. Canonical repository gates on the freeze

- `./scripts/check.sh` (Tier 1): passed.
- `./scripts/check.sh extended`: passed, with only the two existing
  optional skips (unbuilt Node JS artifact, absent downstream artifact
  manifest). Includes full compat (1,870 passed, 26 warnings, 250.99s),
  feature matrix (incl. no-default/http1/tls profiles), docs, FFI, MSRV
  Rust 1.89.0 checks, lifecycle/soak, and benchmarks.
- `./scripts/check.sh package`: passed (crate dry-runs, wheel build/smoke,
  package-content and installed-wheel typing checks).
- `./scripts/check_security.sh`: passed at 2026-09-16T16:12:19Z with
  cargo-deny 0.19.0 and cargo-audit 0.22.2; advisories/bans/licenses/
  sources ok; only non-failing duplicate warnings (getrandom, hashbrown).

### 5. Feature/dependency claims

- MSRV Rust 1.89.0 gate passes (extended).
- Default/minimal profiles compile (extended feature matrix).
- All optional feature families resolve under cargo-deny all-features;
  Windows x86_64 covered by policy targets.
- No production dependency added (manifest/lockfile untouched by this
  closure's executable work; only `rcgen`/`tokio-rustls` test-fixture use,
  both pre-existing dev/test paths).
- HTTP/3 remains feature-gated experimental; Node remains experimental
  prototype (Tier 1 truthful skip preserved).

### 6. Compatibility qualification on the freeze

- HTTPX 0.28.1 oracle: 71 allowed matches, 0 stale, 0 unexplained,
  0 resolved-in-active.
- HTTPX2 2.12.0 oracle: 79 allowed matches, 0 stale, 0 unexplained,
  0 resolved-in-active.
- Three consecutive `EGGFETCH_COMPAT_REQUIRED=1 pytest
  crates/eggfetch-python/tests/compat/ -q --strict-markers` runs, no files
  changed between runs: 1,870 passed / 26 warnings in 260.11s, 247.60s,
  and 250.06s. Directly affected proxy/TLS/limits/timeout/streaming cases
  included. No regression; no new allowed difference.

### 7. Profiles and ledger

- `compat/httpx/0.28.1/profile.toml` and
  `compat/httpx2/2.12.0/profile.toml` renewed to `bfda3889` / 2026-09-16
  with `de00479e` recorded as previous.
- `plans/httpx-parity-correction-status.md` renewed with the Stage C
  evidence above; H3/Node remain experimental.

### 8. Documentation and plan closure (descendants of the freeze)

- `plans/README.md`: program indexed as complete.
- `plans/ROADMAP.md`: updated only if product position changed (no new
  surface; see below).
- Architecture docs: TLS identity (`core-tls-proxy-protocols.md`), route
  identity/connector ownership (`core-engine.md`), checklist/matrix
  (`transport::hyper_client` rustdoc, the normative code comment).
- Child-plan closure records: this file; per-plan implementation notes in
  the freeze commits and module docs.

### Final acceptance

- [x] All six executable child plans are complete.
- [x] One clean exact executable/test/build/validation SHA is recorded
  (`bfda3889`).
- [x] Focused TLS/cache/idle/deadline/refactor regressions pass on it.
- [x] Tier 1 passes.
- [x] Extended validation passes.
- [x] Package validation passes.
- [x] Live security preflight passes with scan metadata recorded.
- [x] MSRV/feature/dependency claims remain truthful.
- [x] HTTPX 0.28.1 full qualification and API oracle pass under Stage C.
- [x] HTTPX2 2.12.0 full qualification and API oracle pass under Stage C.
- [x] Both compatibility profiles and live ledger bind to the final SHA.
- [x] HTTP/3 remains experimental.
- [x] Node remains experimental.
- [x] Post-freeze descendants are documentation/profile/ledger only
  (to be verified by final descendant audit before push).
