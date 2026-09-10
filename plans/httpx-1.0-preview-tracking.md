# HTTPX 1.0 Preview Tracking

Planning baseline: `4456680361dbfdc2d15b0d43a18cdcb20704f667` (`main`, 2026-09-10)
Parent program: `plans/http3-and-next-httpx-compatibility-program.md`
Current upstream preview observed during planning: `httpx==1.0.dev6` (2026-08-31)
Stable production contract retained: `httpx==0.28.1`

## Objective

Track the original HTTPX 1.0 redesign closely enough that EggFetch can make an informed migration decision when the upstream API reaches an RC/stable boundary, without spending implementation effort chasing unstable dev-release APIs or weakening the existing 0.28.1 qualification.

This plan is reconnaissance and compatibility architecture preparation. It must not advertise HTTPX 1.0 parity.

## 1. Establish a preview reference area

Create a clearly non-qualified preview area, for example `compat/httpx/1.0-preview/`, containing:

- README naming the exact observed upstream dev release;
- generated reference API manifest;
- a machine-readable delta/classification record if the existing schema fits;
- notes on public redesign areas and removed/reintroduced concepts.

Do not reuse `profile.toml` fields in a way that could be read as Stage C qualification. If a profile is useful, give it an explicit preview/unqualified status.

Acceptance:
- [ ] no `qualification-sha` or Stage C claim is attached to a dev release;
- [ ] exact upstream dev version/date are recorded;
- [ ] preview artifacts cannot be confused with `compat/httpx/0.28.1/` production evidence.

## 2. Generate API deltas using shared tooling

Use the generalized API manifest tooling from the HTTPX2 baseline plan to compare:

- HTTPX 0.28.1;
- current HTTPX 1.0 preview;
- EggFetch's 0.28.1 facade;
- HTTPX2 2.12 where useful for lineage comparison.

Classify public removals/additions/signature redesign rather than immediately implementing them.

Acceptance:
- [ ] all top-level public API and public class-method/property changes are classified;
- [ ] package-size/private-implementation churn is ignored unless it changes the public contract.

## 3. Track architectural redesign themes

For each meaningful 1.0 preview release, review changes in these buckets:

- client vs toolkit/server scope;
- sync/async API organization;
- transport customization;
- request/response/URL/header models;
- streaming and upgrade/network-stream semantics;
- TLS/proxy configuration;
- timeout/limits semantics;
- auth/cookies/redirects;
- optional protocol helpers;
- exception taxonomy;
- public/private module boundary.

Record whether EggFetch already has an equivalent native capability, would need a compatibility-only adapter, or would require real engine work.

Acceptance:
- [ ] each redesign area has an impact classification (`reuse`, `adapter`, `engine`, `not-applicable`, `unknown`);
- [ ] no implementation work is started solely because a dev release temporarily exposes an API.

## 4. Use upstream tests as reconnaissance, not qualification

Identify a small set of high-signal upstream tests/probes for newly stabilized-looking behavior. It is acceptable to run them manually or in a preview-only local harness, but they must not become required Stage C gates while upstream is still dev-status.

Acceptance:
- [ ] preview failures do not invalidate HTTPX 0.28.1 qualification;
- [ ] preview tests are clearly separated from required compatibility suites.

## 5. Define migration trigger

Create a concrete trigger for opening an implementation/qualification program for HTTPX 1.0. Recommended trigger:

- upstream publishes an RC with an explicitly frozen/public API, or a stable 1.0 release;
- no release notes indicate another major compatibility reset before stable;
- a fresh delta inventory shows the target is sufficiently stable to justify implementation.

At trigger time, write a new version-specific implementation program. Do not simply rename this preview plan into a Stage C plan.

Acceptance:
- [ ] trigger is recorded in `plans/README.md`/roadmap;
- [ ] future target is pinned to an exact RC/stable release before coding begins.

## 6. Preview refresh discipline

A preview refresh should normally update only generated/reference/docs data. If shared scripts need modification, make those changes before the parent program's final exact-SHA freeze and rerun all affected compatibility validation.

Do not create automated polling, a new CI job, or a release watcher as part of this plan.

## Non-goals

- implementing `1.0.dev6` parity;
- Stage C/production claims for any dev release;
- replacing the 0.28.1 facade;
- mirroring HTTPX's new server/toolkit scope unless a future stable contract makes that strategically desirable;
- adding new CI workflows;
- repeatedly refactoring EggFetch around pre-release churn.

## Exit criteria

- [ ] a non-qualified HTTPX 1.0 preview baseline exists;
- [ ] public/architectural deltas are classified against EggFetch;
- [ ] a clear RC/stable implementation trigger is documented;
- [ ] no unstable preview API has become an EggFetch compatibility promise;
- [ ] any qualification-sensitive tooling changes occur before final parent-program freeze.