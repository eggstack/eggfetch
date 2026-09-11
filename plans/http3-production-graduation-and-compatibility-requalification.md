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
- [x] no child-plan checkbox is silently waived; the unresolved items are
  recorded as blockers in `http3-production-qualification-evidence.md`;
- [x] all executable corrections are committed before freeze;
- [x] working tree was clean at the executable freeze.

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
- [x] every executable cluster in this final delta has focused evidence;
- [x] no hidden second transport path was introduced;
- [x] HTTPX facade behavior changes, if any, are intentional and separately qualified.

Frozen-tree audit record: the executable delta from prior qualified SHA
`65beb675a5380d3ff4291da6833b91ebf12c769a` to frozen SHA
`639bf186a71c054e11278d1b160ffe7a6f172c02` is limited to the timeout
qualification fixture's wall-clock assertion margin. The focused timeout
file, H3 suites, full compatibility suites, and repository tiers were rerun;
no Rust transport, dependency, manifest, or qualification-runner behavior
changed in this final delta.

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
- [x] Tier 1 green;
- [x] extended green with all skips explained;
- [x] package validation green;
- [x] three consecutive full compatibility runs green without intervening changes;
- [x] HTTPX 0.28.1 oracle clean under its existing allowed-difference policy;
- [x] HTTPX2 2.12.0 oracle clean under its existing allowed-difference policy;
- [x] required downstream portfolio green.

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

Execution record: the frozen executable commit `639bf18` and the
documentation/profile/ledger descendant `c053fe0` were pushed in one
fast-forward. Existing CI run `34578376515` passed on `c053fe0`; the
descendant diff contains no executable files, so it is valid routine-CI
evidence for the frozen executable tree. A final record-only descendant is
being created after this run and will receive its own routine CI verification.

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

## Execution record (2026-09-11)

The literal gate was evaluated on frozen executable SHA
`639bf186a71c054e11278d1b160ffe7a6f172c02`. HTTP/3 remains experimental:
the deterministic local suites and all repository/compatibility gates pass,
but the required independent-server, independent-drain, public-origin,
realistic impairment, and upstream-risk evidence does not. Full details are
in `plans/http3-production-qualification-evidence.md` and the machine-readable
interop ledger. Both compatibility profiles were renewed on this SHA; this
does not constitute HTTP/3 graduation.

## Non-goals

- using HTTPX qualification to substitute for H3 interop evidence;
- changing H3 defaults merely because qualification passes;
- adding 0-RTT/WebTransport/datagrams/MASQUE/migration;
- creating a new permanent CI matrix without separate approval;
- weakening exact-SHA qualification rules.

## Exit criteria

- [x] prerequisite child plans are fully audited;
- [x] one clean exact executable SHA is frozen;
- [x] independent interop, impairment, upstream-risk, resource and diagnostic evidence is recorded;
- [x] objective H3 graduation decision is made without waiver-by-wording;
- [x] HTTPX 0.28.1 and HTTPX2 2.12.0 Stage C are renewed on the frozen executable SHA if qualification-sensitive changes occurred;
- [x] remote CI and post-freeze descendant audit are recorded;
- [x] documentation and plan index reflect the final truth;
- [x] no active blocker is hidden behind a completed-plan label.
