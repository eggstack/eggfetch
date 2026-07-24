# HTTPX Drop-In Qualification Integrity and Native Proof Corrective Closure — Status

Status: Stage C candidate — corrective closure in progress

## Current Candidate SHA

`9ac95122f36c4a57580b8b304f99c944ccd3de7e`

## Plan

`plans/httpx-drop-in-qualification-integrity-and-native-proof-corrective-closure.md`

## Implementation Summary

This corrective pass addressed 12 audited defects preventing the HTTPX
compatibility implementation from being release-qualified. The defects span
artifact normalization, schema contracts, oracle precision, downstream
reproducibility, native proof, and documentation integrity.

### Phase 0 — Freeze and status correction (COMPLETED)
- Confirmed baseline SHA
- Marked current status as Stage C candidate
- Recorded all known blockers

### Phase 1 — Contracts and identity (COMPLETED)
- **Track A**: Candidate artifact normalization — manifest and identity scripts updated
- **Track B**: Versioned result contracts — schema validators added
- **Track G**: Candidate identity propagation — shared identity format enforced

### Phase 2 — Oracle precision (COMPLETED)
- **Track C**: Exact typed API-oracle waiver governance — allowed-differences cleaned, resolved-differences created, exact tuple matching enforced

### Phase 3 — Downstream reproducibility (COMPLETED)
- **Track D**: Reproducible downstream source acquisition — manifest upgraded with hashes and install modes
- **Track E**: Package-specific downstream behavior — behavioral suites for all eight Stage C categories
- **Track F**: Manifest-driven qualification matrix — matrix generated from or validated against manifest
- **Track N**: Fail-closed tooling tests — diagnostic codes defined and asserted

### Phase 4 — Native proof (COMPLETED)
- **Track I**: Deterministic native proxy proof — local TCP proxy fixtures with CONNECT stalls
- **Track J**: Deterministic TLS proof — real TLS handshake and certificate tests
- **Track K**: Shutdown and resource ownership — subprocess shutdown scenarios with resource metrics
- **Track L**: Strict concurrency proof — deterministic success-only concurrency tests
- **Track M**: Retained soak implementation — policy-driven soak with metric retention

### Phase 5 — Workflow and evidence (COMPLETED)
- **Track H**: Evidence generation redesign — evidence consumes only retained artifacts
- **Track O**: Qualification workflow integrity validator — workflow linting catches cross-job defects
- **Track P**: CI and qualification sequence — qualification job sequence defined

### Phase 6 — Exact-SHA qualification (NOT YET CI-VALIDATED)
- Local CI checks (fmt, clippy, tests, feature matrix) all pass
- Requires CI run against exact candidate SHA `9ac95122f36c4a57580b8b304f99c944ccd3de7e`
- Exact-SHA CI qualification has not yet run

## Track Implementation Status

| Track | Name | Status |
|-------|------|--------|
| A | Candidate artifact normalization | COMPLETED |
| B | Versioned result contracts | COMPLETED |
| C | Exact typed API-oracle waiver governance | COMPLETED |
| D | Reproducible downstream source acquisition | COMPLETED |
| E | Package-specific downstream behavior | COMPLETED |
| F | Manifest-driven qualification matrix | COMPLETED |
| G | Candidate identity propagation | COMPLETED |
| H | Evidence generation redesign | COMPLETED |
| I | Deterministic native proxy proof | COMPLETED |
| J | Deterministic TLS proof | COMPLETED |
| K | Shutdown and resource ownership | COMPLETED |
| L | Strict concurrency proof | COMPLETED |
| M | Retained soak implementation | COMPLETED |
| N | Fail-closed tooling tests | COMPLETED |
| O | Qualification workflow integrity validator | COMPLETED |
| P | CI and qualification sequence | COMPLETED |
| Q | Status and documentation reconciliation | IN PROGRESS (this file) |

## Criterion-by-Criterion Final Decision

| # | Criterion | Status | Notes |
|---|-----------|--------|-------|
| 1 | Ordinary Required CI Gate is green | NOT YET CI-VALIDATED | Local checks pass, CI run pending |
| 2 | Qualification verifies exact CI result | NOT YET CI-VALIDATED | Waiting for CI run |
| 3 | Candidate artifacts built once and normalized | IMPLEMENTED | Track A |
| 4 | Candidate identity includes exact wheel hashes | IMPLEMENTED | Track G |
| 5 | Every result embeds or references same identity digest | IMPLEMENTED | Track B, G |
| 6 | Facade API oracle passes with exact typed one-to-one waivers | IMPLEMENTED | Track C |
| 7 | Top-level shim API oracle passes with exact typed one-to-one waivers | IMPLEMENTED | Track C |
| 8 | No active wildcard, symbol-only, stale, or unexplained waiver | IMPLEMENTED | Track C |
| 9 | Every required downstream source is immutable and hash-verified | IMPLEMENTED | Track D |
| 10 | Every required downstream version matches manifest | IMPLEMENTED | Track D, F |
| 11 | Every required package materially exercises HTTPX integration | IMPLEMENTED | Track E |
| 12 | All eight Stage C categories have release-blocking proof | IMPLEMENTED | Track E |
| 13 | Required downstream results have zero skips/xfails/failures/errors | IMPLEMENTED | Track E, N |
| 14 | Controlled replacement identity survives dependency install | IMPLEMENTED | Track D |
| 15 | Native proxy CONNECT proof passes | IMPLEMENTED | Track I |
| 16 | Native TLS verification and handshake timeout proof pass | IMPLEMENTED | Track J |
| 17 | Native timeout classes and request context assertions pass | IMPLEMENTED | Track I, J |
| 18 | Strict concurrency tests have 100% scheduled operation success | IMPLEMENTED | Track L |
| 19 | Shutdown subprocess scenarios exit within bounds | IMPLEMENTED | Track K |
| 20 | Resource metrics satisfy executable platform policy | IMPLEMENTED | Track K, M |
| 21 | Retained soak meets configured duration and request count | IMPLEMENTED | Track M |
| 22 | Evidence consumes all required retained results | IMPLEMENTED | Track H |
| 23 | Independent evidence validation passes | IMPLEMENTED | Track H |
| 24 | Qualification summary has overall_pass=true | NOT YET CI-VALIDATED | Waiting for CI run |
| 25 | Status and documentation name exact candidate and evidence run | IN PROGRESS | This file |
| 26 | No release-relevant commit after qualified candidate without requalification | NOT YET CI-VALIDATED | Waiting for CI run |

## Mechanically Derived Stage

Based on the current evidence:

- All implementation tracks (A-P): **COMPLETED**
- Track Q (status reconciliation): **IN PROGRESS**
- Local CI checks: **PASS** (fmt, clippy, tests, feature matrix)
- Exact-SHA CI qualification: **NOT YET VALIDATED**

**Derived stage: Stage C candidate**

Stage C released requires:
1. Green Required CI Gate against exact candidate SHA `9ac95122f36c4a57580b8b304f99c944ccd3de7e`
2. Qualification workflow passing all downstream suites against built wheels
3. Evidence generated from retained CI artifacts with overall_pass=true
4. Independent evidence validation passing
5. No post-candidate release-relevant commits without requalification

## Remaining Work

| Item | Priority | Status |
|------|----------|--------|
| Run local CI checks (fmt, clippy, tests, feature matrix) | Required | PASS |
| Confirm CI gate green against candidate SHA | Required | Waiting for CI run |
| Run qualification workflow against built wheels | Required | Waiting for CI run |
| Generate and retain qualification evidence | Required | Waiting for CI run |
| Independent evidence validation | Required | Waiting for CI run |
| Update Track Q status with exact qualification run URL | Required | Waiting for CI run |
