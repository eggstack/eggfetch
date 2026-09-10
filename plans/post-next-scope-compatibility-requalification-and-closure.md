# Post-Next-Scope Compatibility Requalification and Closure

Planning baseline: `4456680361dbfdc2d15b0d43a18cdcb20704f667` (`main`, 2026-09-10)
Parent program: `plans/http3-and-next-httpx-compatibility-program.md`
Prior HTTPX 0.28.1 qualified executable SHA: `d034a1005857a7f403222dda4bda5f2f204a44fe`
Required references: `httpx==0.28.1` and `httpx2==2.12.0`
Depends on completion of all executable/test/validation plans in the parent program
Followed by: `plans/post-next-scope-documentation-and-plan-hygiene.md`

## Objective

Freeze one clean post-program executable/test SHA, renew the existing HTTPX 0.28.1 Stage C qualification on that exact executable tree, independently qualify the declared HTTPX2 2.12.0 surface if its gates pass, and record the HTTP/3 graduation outcome. This is the only plan in the program permitted to make new exact-SHA compatibility qualification claims.

## Governing rule

Every required gate for a claimed compatibility profile must pass against the same frozen executable/test/validation SHA. Any subsequent source, test, dependency, compatibility script, validation script, manifest, lockfile, or packaging change invalidates collected evidence and requires a new freeze. Documentation/profile/ledger-only descendants are permitted after evidence collection when a descendant audit proves executable identity.

## 1. Confirm prerequisite closure

Before freezing, verify closure of:

- HTTP/3 Alt-Svc discovery/fallback/draining;
- HTTP/3 interoperability/graduation decision;
- HTTPX2 profile/delta baseline;
- HTTPX2 core/facade parity;
- HTTPX2 SSE/WebSocket parity for the declared optional surface;
- HTTPX 1.0 preview tracking;
- all test/validation tooling changes required by those plans.

Acceptance:
- [ ] no known required defect remains hidden behind final qualification;
- [ ] HTTP/3 outcome is explicitly `supported` or `experimental with blockers`;
- [ ] HTTPX2 profile declares exactly what is being qualified.

## 2. Audit qualification-sensitive change clusters

Compare `d034a10...` to candidate HEAD and classify every change at least by:

1. H3 discovery/cache/routing/fallback;
2. H3 draining/close/resource semantics;
3. H3 dependencies/interoperability hooks;
4. compatibility tooling/profile generalization;
5. shared Python facade refactors;
6. HTTPX2 auth/URL/headers/method helpers;
7. TLS/proxy/no_proxy differences;
8. compression/multipart/WSGI behavior;
9. SSE;
10. WebSockets/network-stream ownership;
11. package optional dependencies/interpreter metadata;
12. HTTPX 1.0 preview tooling/data;
13. validation/package scripts.

Each changed behavior cluster needs focused regression evidence before freeze.

## 3. Focused pre-freeze gate

Run targeted deterministic suites for all changed clusters, including:

- existing H3 hardening plus new Alt-Svc/fallback/draining tests;
- H3 impairment/resource tests that do not require external Internet;
- all existing HTTPX 0.28.1 high-risk differential clusters;
- HTTPX2 non-streaming differential clusters;
- SSE chunk-boundary/max-event/lifecycle tests;
- WebSocket handshake/framing/cancellation/proxy/TLS tests;
- API-manifest generator/comparator tests for both profile families;
- package/import behavior with and without optional WebSocket dependency where applicable.

Failures must be fixed before freezing. Do not waive a compatibility failure because the other profile passes.

Acceptance:
- [ ] focused gate passes cleanly;
- [ ] any fixture/tooling fix is committed before freeze.

## 4. Freeze candidate

Run Tier 1, ensure a clean worktree, and record full `FROZEN_EXECUTABLE_SHA`.

Requirements:

- all source/tests/scripts/manifests/dependencies committed;
- generated profile/reference artifacts that are part of validation committed as required by repository policy;
- no evidence is collected on a dirty tree;
- no later qualification-sensitive modification without restarting.

## 5. Tier 1 / Tier 2 / Tier 3

On the exact frozen SHA run:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Extended validation must exercise both qualified compatibility profiles through the shared machinery. If optional external H3 interoperability servers are absent, record that separately; mandatory H3 graduation evidence must have been collected according to its plan and cannot be replaced by a skip.

Acceptance:
- [ ] Tier 1 green;
- [ ] Tier 2 green;
- [ ] Tier 3 green;
- [ ] all permitted skips are explicit and do not mask a required qualification surface.

## 6. Requalify HTTPX 0.28.1 independently

Run the existing full pinned HTTPX 0.28.1 suite three consecutive times on the frozen SHA, with no file/dependency changes between runs.

Run its API oracle, allowed/resolved-difference reconciliation, high-risk differential probes, and required downstream portfolio using the established procedure.

Acceptance:
- [ ] three consecutive full suites pass;
- [ ] zero unexplained API differences;
- [ ] retained differences remain current/tested;
- [ ] downstream required portfolio passes;
- [ ] no HTTPX2 behavior leaked into the 0.28.1 contract.

## 7. Qualify HTTPX2 2.12 independently

Install the exact profile requirements and run the full HTTPX2 differential suite three consecutive times on the same frozen executable SHA.

Run:

- HTTPX2 API oracle against `compat/httpx2/2.12.0/reference-api.json`;
- allowed/resolved-difference validation;
- behavior-only upstream-derived/security corpus;
- declared optional WebSocket surface with required dependencies;
- SSE lifecycle/resource corpus;
- downstream portfolio appropriate to HTTPX2, if a stable meaningful set was identified by the baseline plan.

Do not copy the HTTPX 0.28.1 Stage C designation automatically. Stage C is earned only if the declared HTTPX2 profile satisfies the repository's existing Stage C requirements.

Acceptance:
- [ ] three consecutive full HTTPX2 runs pass;
- [ ] zero unexplained public API differences in the declared surface;
- [ ] all required behavior/security cases pass;
- [ ] optional-extra limitations are explicit;
- [ ] any downstream requirement passes or the profile is assigned a lower truthful stage.

## 8. Record HTTP/3 graduation evidence

Attach/refer to the H3 graduation record for the frozen executable tree or an executable-identical ancestor included in the freeze.

If graduation succeeded, verify documentation-visible stable claims correspond exactly to tested scope. If it did not, ensure HTTP/3 remains labeled experimental and blockers are recorded; failed graduation does not prevent HTTPX/HTTPX2 compatibility qualification unless H3 changes broke those profiles.

Acceptance:
- [ ] H3 label matches evidence;
- [ ] no advanced QUIC feature is implied by ordinary H3 graduation.

## 9. Remote CI

Push the frozen candidate or an executable-identical docs/profile record descendant and confirm the repository's existing CI (`./scripts/check.sh`) succeeds. Do not create a special qualification workflow.

## 10. Update independent profile/ledger records

Only after all corresponding gates pass:

- update `compat/httpx/0.28.1/profile.toml` qualification SHA/date and live 0.28.1 ledger;
- update `compat/httpx2/2.12.0/profile.toml` with its independently earned status/SHA/date;
- create/update an HTTPX2 status ledger if the baseline plan established one using the same evidence vocabulary;
- record HTTP/3 supported/experimental outcome in the appropriate status/architecture record;
- leave HTTPX 1.0 preview explicitly unqualified.

The record commit must be documentation/profile/ledger-only.

## 11. Descendant audit

Compare `FROZEN_EXECUTABLE_SHA..HEAD` and classify every file. Any executable/test/build/validation/package drift invalidates exact-SHA claims and requires requalification.

## 12. Final truth checks

Verify mechanically/manually that:

- 0.28.1 and HTTPX2 profiles point at the same intended frozen executable SHA if both are qualified in this program;
- their reference versions/packages differ correctly;
- importing one facade does not mutate the other;
- HTTPX 1.0 preview has no qualification claim;
- H3 label is evidence-consistent.

## Non-goals

- qualifying HTTPX 1.0.dev*;
- inventing a new compatibility stage/evidence schema;
- new CI architecture;
- release publication;
- changing executable code while collecting evidence;
- forcing HTTPX2 Stage C if the declared surface is incomplete.

## Exit criteria

- [ ] one exact frozen executable SHA exists;
- [ ] Tier 1/2/3 pass on it;
- [ ] HTTPX 0.28.1 is freshly Stage C qualified;
- [ ] HTTPX2 2.12 has a truthful independently earned qualification stage;
- [ ] HTTP/3 graduation outcome is recorded truthfully;
- [ ] HTTPX 1.0 preview remains unqualified;
- [ ] remote CI passes;
- [ ] descendant audit proves executable identity after record commits.