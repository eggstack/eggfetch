# eggfetch Active Planning Registry

This is the control surface for current planning. Detailed requirements and
completed evidence remain in subsystem roadmaps, closure records, and the
historical plan archive.

Canonical direction:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`

Live compatibility binding:

- `plans/httpx-parity-correction-status.md`
- `compat/httpx/0.28.1/profile.toml`
- `compat/httpx2/2.12.0/profile.toml`

## Current release

The current public version is **0.4.1** across the six publishable crates and
the Python distribution. The publication evidence is in
`plans/closure/release-verification/010-0-4-1-publication.md`; the 0.4.0 record
is `plans/closure/release-verification/009-0-4-0-publication.md`.

**0.4.1 is published.** Released from `dc0f580f…` under signed tag `v0.4.1`.
All four gates were green before publication — Tier 1, Tier 2
(`./scripts/check.sh extended`, 6915 Rust test results, 0 failed, one
policy-defined Node skip), Tier 3, and the live security preflight. All six
crates resolve at 0.4.1 on crates.io; PyPI carries 33 distributions for the
version — 32 wheels including the free-threaded 3.14t and 3.15t rows, plus
one sdist; the GitHub Release is live.

**The Stage C binding has since been renewed.** The 0.4.1 release did not
advance it — the free-threaded wheel work, the benchmark harness changes and
the version bump invalidate a binding, so a fresh exact-SHA requalification
was outstanding. That requalification ran green on 2026-11-24 and also
corrected the required-profile skip auditor, which had
printed a FAIL line for an unapproved skip without changing the session exit
status. The binding is live again; the record is in
`plans/httpx-parity-correction-status.md`. The build-only PyPI rehearsal
also caught a Windows wheel-smoke defect in the unreleased free-threaded
work; it was fixed before any wheel was published, and the record is in the
0.4.1 closure document.

Validation tiers are defined in `.skills/verification-qualification.md`; the
normative policy is `docs/verification-policy.md`.

## Status vocabulary

- **proposed:** not approved for execution.
- **ready:** dependencies and interfaces are satisfied; the plan may be handed off.
- **active:** implementation or closure work is in progress.
- **blocked:** a named dependency or evidence requirement prevents progress.
- **closing:** implementation landed and closure evidence is being gathered.
- **closed:** the closure record is accepted.
- **conditionally closed:** implementation is complete, but a named condition remains.
- **superseded:** replaced by another document.
- **archived:** no longer active and retained for traceability.

## Subsystem roadmaps

| Subsystem | Roadmap status | Roadmap | Current state |
|---|---|---|---|
| Core transport and request policy | active | `plans/subsystems/core-transport-policy-roadmap.md` | M011 (profile-independent request policy and native `http::Uri` builder) is conditionally closed; the oracle baseline advanced to `05f14f44`, and the exact-SHA Stage C requalification it required has since run green (2026-11-24), leaving the closure acceptance with the maintainer. |
| Python bindings and HTTPX compatibility | active | `plans/subsystems/python-httpx-compat-roadmap.md` | No implementation milestone is registered. HTTPX 1.0 remains gated on its upstream trigger. Free-threaded (PEP 703) wheel support was adopted (Option A) and shipped in 0.4.1: `cp314-cp314t` and `cp315-cp315t` wheels ship for 3.14t and 3.15t on the four release platforms, and the release set grew from 24 to 32 wheels (25 to 33 distributions). The evidence is in `plans/python-free-threaded-support-investigation.md` and `docs/architecture/free-threading-performance.md`; the separate `abi3t`/PEP 803 question is still open. |
| TLS, proxy, and protocols | active | `plans/subsystems/tls-proxy-protocols-roadmap.md` | M006–M012 closed (H3 experimental retained). The Stage C binding was renewed on 2026-11-24 after the free-threaded wheel work invalidated the previous binding. |
| Release and verification | active | `plans/subsystems/release-verification-roadmap.md` | 0.4.1 publication is closed (`plans/closure/release-verification/010-0-4-1-publication.md`); 0.4.0 is `009-0-4-0-publication.md`. |
| Performance and footprint | closed | `plans/subsystems/performance-footprint-roadmap.md` | Completed campaigns and repaired fixtures. |

Closed plans are historical evidence, not active requirements or release gates.

## Dependency-ready implementation plans

None.

No new plan for upstream h3 #338 — remains a gated graduation blocker (M008/M010).

## Conditionally closed implementation plans

| Plan | Status | Condition |
|---|---|---|
| `plans/implementation/core-transport-policy/011-profile-independent-request-policy-and-native-uri-builder.md` | conditionally closed | The exact-SHA Stage C requalification this change invalidated has run green (2026-11-24) on a freeze whose tree contains this milestone. Tier 2 and Tier 3 have not run at the milestone's own freeze `49f8910d…`; accepting the later green freeze as covering it is the remaining maintainer decision. Closure record: `plans/closure/core-transport-policy/011-profile-independent-request-policy-and-native-uri-builder.md` |
| `plans/implementation/tls-proxy-protocols/006-h3-server-controlled-corpus.md` | closed | Stage C rebound on `cef0e7aa5c23ef92f4a378f370516d9dfbaa466d` |
| `plans/implementation/tls-proxy-protocols/007-h3-impairment-runner.md` | closed | Stage C rebound; privileged Linux matrix remains operational evidence for graduation |
| `plans/implementation/tls-proxy-protocols/009-h3-resource-soak-and-platform-scope.md` | closed | Stage C rebound; Darwin + ubuntu-latest CI Linux soak rows recorded |

## Recently closed implementation plans

| Plan | Status | Closure |
|---|---|---|
| `plans/implementation/tls-proxy-protocols/011-h3-dial-error-sanitization-and-datagram-recv-trust.md` | closed | `plans/closure/tls-proxy-protocols/011-h3-dial-error-sanitization-and-datagram-recv-trust.md` |
| `plans/implementation/tls-proxy-protocols/012-h3-qualification-security-hygiene.md` | closed | `plans/closure/tls-proxy-protocols/012-h3-qualification-security-hygiene.md` |
| `plans/implementation/tls-proxy-protocols/008-h3-upstream-risk-re-audit.md` | closed | `plans/closure/tls-proxy-protocols/008-h3-upstream-risk-re-audit.md` |
| `plans/implementation/tls-proxy-protocols/010-ordinary-h3-graduation-decision.md` | closed (experimental retained) | `plans/closure/tls-proxy-protocols/010-ordinary-h3-graduation-decision.md` |

## Blocked implementation plans

None.

## Gated futures

- **HTTPX 1.0:** wait for an upstream release candidate or stable release with a
  frozen API, then perform a fresh delta review.
- **H3 graduation:** M010 closed with experimental retained. Ordinary HTTP/3
  stays experimental until a later decision fills independent-server,
  impairment, public-origin, and upstream (#338) rows on one post-audit SHA.
- **Node.js maturation:** requires an explicit scope decision.

None of these is unblocked by the closed milestones.

## Pending maintainer actions

| # | Action | Owner | Reference |
|---|---|---|---|
| 2 | Decide whether CLI work gets its own subsystem roadmap or belongs under an existing roadmap. | Project maintainers | `plans/implementation/release-verification/007-planning-and-documentation-hygiene.md` §5 |
| 6 | Decide whether the IPv6 pool-key divergence needs its own milestone. The high-level and native paths currently produce different keys for the same IPv6 origin. | Project maintainers | `plans/closure/core-transport-policy/010-core-duplicate-helper-consolidation.md` §10 |
| H3 | Supply two non-Quinn servers, a privileged Linux impairment pass, and three public-origin checks before any future support decision. | Project maintainers | M010 closure |
| FT | Decide whether `abi3t` / PEP 803 should be revisited as a separate question from free-threading. The free-threaded wheel-support decision itself is closed: Option A was adopted on 2026-11-23. | Project maintainers | `plans/python-free-threaded-support-investigation.md` §9 |

## Operational blockers

| Area | Blocker |
|---|---|
| HTTPX qualification | HTTPX 1.0 has not met its upstream trigger. The Stage C binding is live as of 2026-11-24; any further executable-input change invalidates it again and requires a fresh exact-SHA qualification before it can be cited. |
| H3 | Experimental retained (M010). Open rows: independent non-Quinn servers, privileged impairment, public origins, upstream h3 #338. TLS/proxy M011–M012 closed. The Stage C binding was renewed on 2026-11-24. |
| Tier 2 downstream fixtures | The gate requires `target/downstream-qualification/artifact-manifest.json` and fails closed if it is absent. |

Closure records hold milestone-specific evidence and decisions. The live
binding is recorded in the ledger, profiles, and canonical compatibility pages:
`docs/residual-differences.md`, `docs/reference/compatibility.md`, and
`docs/reference/compatibility-stage-decision.md`. Other planning documents
must not invent a second live SHA.
