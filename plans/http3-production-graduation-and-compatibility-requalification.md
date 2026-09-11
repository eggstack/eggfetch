# HTTP/3 Production Graduation and Compatibility Requalification

Planning baseline: `be7bf441661a923637b92ba8e449342868c56aa5`
Parent: `plans/http3-production-qualification-program.md`
Depends on:
- `plans/http3-independent-interop-and-impairment-qualification.md`
- `plans/http3-upstream-risk-resource-and-observability-hardening.md`

## Objective

Perform the final audit, exact-SHA freeze, objective HTTP/3 graduation decision, and compatibility requalification after all qualification-sensitive H3 work has landed. This plan must not hide missing H3 evidence behind successful HTTPX tests, nor invalidate existing HTTPX Stage C claims without renewing them on the final executable tree.

## 1. Prerequisite closure audit

Before freezing, audit every acceptance item in both child plans and classify it as:

- satisfied with named evidence;
- not applicable with a technical rationale consistent with the declared stable scope;
- unsatisfied and therefore an H3 graduation blocker.

Required named evidence includes:

- at least two independent non-Quinn server implementations passing the required corpus;
- independent GOAWAY/drain/reconnect evidence;
- public-origin spot-check ledger;
- impairment matrix;
- upstream dependency/issue ledger;
- resource/soak results;
- diagnostic/platform evidence.

Do not freeze while known product defects or qualification-script defects are still being corrected.

Acceptance:
- [ ] no child-plan checkbox is silently waived;
- [ ] all executable corrections are committed before freeze;
- [ ] working tree is clean.

## 2. Change-cluster audit

Audit the complete executable delta from the previously qualified executable SHA `65beb675a5380d3ff4291da6833b91ebf12c769a` to the candidate.

Group changes by risk:

- H3 independent interop harness/test hooks;
- impairment/fallback behavior;
- upstream dependency upgrades/workarounds;
- cancellation/reset/drain behavior;
- cache/resource lifecycle;
- diagnostics/metrics;
- fuzz/validation/package changes.

For every product behavior change identify focused regression evidence. Confirm that H1/H2, proxy/SOCKS, TLS, request replay, Python bindings and compatibility facades were not accidentally altered outside intended boundaries.

Acceptance:
- [ ] every executable cluster has focused evidence;
- [ ] no hidden second transport path was introduced;
- [ ] HTTPX facade behavior changes, if any, are intentional and separately qualified.

## 3. Pre-freeze focused gate

On the candidate executable tree run at minimum:

- all H3 hardening tests;
- Alt-Svc discovery/fallback/draining tests;
- local H3 interop corpus;
- independent-server corpus against the two graduation implementations;
- impairment matrix;
- upstream-defect regression cases;
- resource/soak qualification;
- H3 fuzz/property smoke or bounded qualification run;
- transport metrics/metadata tests;
- high-risk HTTPX 0.28.1 and HTTPX2 differential tests that cover transport/TLS/proxy/streaming/cancellation;
- API oracles for both compatibility profiles.

Any executable correction after this gate requires rerunning affected focused gates before freeze.

## 4. Freeze exact executable SHA

Once all executable work is committed and focused gates are green:

- record `FROZEN_EXECUTABLE_SHA`;
- verify clean working tree;
- do not change Rust/Python source, tests, Cargo manifests/lockfile, scripts, workflows, package metadata or qualification tooling during evidence collection;
- if any such file changes, invalidate the freeze and restart qualification on the new SHA.

## 5. Full repository verification on frozen SHA

Run existing repository tiers without inventing a special permanent CI path:

1. `./scripts/check.sh`;
2. `./scripts/check.sh extended`;
3. `./scripts/check.sh package`.

Record all skips. A skip is evidence only of absence, never of H3 support.

Run the full pinned compatibility suite three consecutive times on the unchanged frozen SHA under the existing required-compatibility mode. Require zero failures; apply existing qualification policy to skips/xfails/warnings.

Run both API oracles and the required downstream compatibility portfolio.

Acceptance:
- [ ] Tier 1 green;
- [ ] extended green with all skips explained;
- [ ] package validation green;
- [ ] three consecutive full compatibility runs green without intervening changes;
- [ ] HTTPX 0.28.1 oracle clean under its existing allowed-difference policy;
- [ ] HTTPX2 2.12.0 oracle clean under its existing allowed-difference policy;
- [ ] required downstream portfolio green.

## 6. HTTP/3 graduation decision

Evaluate the parent program gate literally.

### Promote only if all required evidence exists

If the gate passes, ordinary HTTP/3 becomes **supported** under the narrow documented scope. Update documentation to state exactly what was qualified and what remains excluded.

Do not automatically change default `Auto` behavior. H3 support status and default enablement are separate policy decisions.

### Retain experimental if any blocker remains

If the gate does not pass, keep HTTP/3 **experimental** and record each blocker with the evidence that failed or remains unavailable. Do not phrase partial interop as production qualification.

A retained-experimental result is a successful execution of this plan if the decision is truthful and all available evidence is recorded.

## 7. Compatibility profile renewal

Because this program is expected to change qualification-sensitive tests/tooling and may change dependencies/product code, renew both existing compatibility profiles on `FROZEN_EXECUTABLE_SHA` after all gates pass:

- `compat/httpx/0.28.1/profile.toml`;
- `compat/httpx2/2.12.0/profile.toml`.

Update `plans/httpx-parity-correction-status.md` with a new current pass. Demote the prior `65beb67` binding to historical evidence rather than rewriting history.

HTTP/3 graduation status is not itself part of either HTTPX API parity claim; record it adjacent to, but distinct from, compatibility qualification.

## 8. Remote CI and descendant audit

Push the frozen executable SHA and obtain successful existing CI evidence if repository workflow timing permits the freeze commit itself to run. If qualification records are committed afterward, prove every post-freeze changed file is documentation/profile/ledger-only.

Record:

- frozen SHA CI run/result;
- qualification-record descendant SHA/run if applicable;
- explicit descendant file audit.

Any executable change after freeze invalidates qualification and requires a new freeze.

## 9. Documentation truth pass

After the executable qualification is fixed, update only documentation/profile/ledger files as needed:

- README feature/status wording;
- Rust/Python guides;
- architecture protocol deep dive;
- troubleshooting;
- security/threat model;
- verification documentation;
- plan index and roadmap;
- H3 evidence ledger;
- compatibility profiles/status ledger.

If H3 graduates, document the exact supported scope and advanced-feature exclusions. If not, keep experimental wording and list remaining blockers consistently.

## Non-goals

- using HTTPX qualification to substitute for H3 interop evidence;
- changing H3 defaults merely because qualification passes;
- adding 0-RTT/WebTransport/datagrams/MASQUE/migration;
- creating a new permanent CI matrix without separate approval;
- weakening exact-SHA qualification rules.

## Exit criteria

- [ ] prerequisite child plans are fully audited;
- [ ] one clean exact executable SHA is frozen;
- [ ] independent interop, impairment, upstream-risk, resource and diagnostic evidence is recorded;
- [ ] objective H3 graduation decision is made without waiver-by-wording;
- [ ] HTTPX 0.28.1 and HTTPX2 2.12.0 Stage C are renewed on the frozen executable SHA if qualification-sensitive changes occurred;
- [ ] remote CI and post-freeze descendant audit are recorded;
- [ ] documentation and plan index reflect the final truth;
- [ ] no active blocker is hidden behind a completed-plan label.