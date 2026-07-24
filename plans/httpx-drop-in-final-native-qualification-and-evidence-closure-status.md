# HTTPX Drop-In Final Native Qualification and Evidence Closure — Status

Status: Stage C candidate

## Audited Baseline SHA

`48622d47830bba68e0cc62d3ed70a308114b573c`

## Current Candidate SHA

`69a055415be8c651db1a57ef6e99f8a7ea6e0832`

## Implementation Commits

| SHA | Description |
|-----|-------------|
| `48622d4` | Merge: verification-substitution-closure |
| `69a0554` | plans: add final HTTPX native qualification closure pass |

## Implementation Summary

This corrective pass addressed 25 release-blocking findings across 8 tracks.

### Track 0 — Status and Candidate Identity
- Previous status corrected with SUPERSEDED notice
- Shared candidate identity schema v3 (`scripts/candidate_identity.py`)
- All result artifacts use one exact candidate identity format

### Track 1 — API Oracle Precision
- Typed difference records with 15 difference types
- Removed semantic-erasing `*args` normalization
- Replaced glob-pattern matching with exact symbol matching
- Added `--validate` flag for allowed-difference schema validation
- Rejects duplicate IDs, wildcards, missing fields, expired entries

### Track 5 — Lossless Merge Semantics
- Query params preserve multiplicity and order via `multi_items()`
- Headers preserve duplicates via batch-by-key replacement
- Client merge uses request items replacing client items for same keys
- 12 new tests in `test_merge_lossless.py`

### Track 4 — Async Auth and Response Ownership
- Separate sync/async auth drivers
- Intermediate auth responses drained and closed before follow-up dispatch
- Response hooks fire only on final response
- Per-request auth disable via `auth=` parameter
- 15 new auth tests (84 total)

### Track 2 — Pinned Downstream Behavioral Suites
- Manifest upgraded to schema v2
- Import-only entries reclassified as informational
- 6 behavioral fixture categories with real tests
- False-green meta-tests for runner validation
- Runner fail-closed for unknown packages, empty selections, zero tests

### Track 6 — Native Timeout, Lifecycle, Shutdown, Soak
- Deterministic local network fixtures (`native_fixtures.py`)
- Native timeout classification tests using real sockets
- Enhanced lifecycle tests for response cleanup
- Soak qualification churn test
- Resource thresholds configuration (`resource-thresholds.toml`)

### Track 3 — Qualification and Evidence Workflow
- Removed `|| true` from required commands
- Evidence generation exits nonzero on `overall_pass=false`
- Independent evidence validator (`validate_compatibility_evidence.py`)
- Qualification workflow linter (`validate_qualification_workflow.py`)
- Pinned qualification requirements (`qualification-requirements.txt`)

### Track 7 — Status Reconciliation
- This file
- Prior status file corrected with SUPERSEDED notice

## Criterion-by-Criterion Final Decision

| # | Criterion | Status | Notes |
|---|-----------|--------|-------|
| 1 | Ordinary Required CI Gate is green | NOT YET CI-VALIDATED | Local checks pending |
| 2 | Facade and top-level replacement API oracles pass | IMPLEMENTED | Typed records, exact matching |
| 3 | Zero unexplained or stale allowed differences | IMPLEMENTED | Comparator exits nonzero |
| 4 | Allowed-difference schema validation passes | IMPLEMENTED | --validate flag |
| 5 | Sync and async auth drivers match HTTPX | IMPLEMENTED | 84 auth tests |
| 6 | Intermediate auth responses release resources | IMPLEMENTED | drain+close before dispatch |
| 7 | One-shot body replay failures deterministic | IMPLEMENTED | replay tests |
| 8 | Client/request query and header merging lossless | IMPLEMENTED | 12 merge tests |
| 9 | Every required downstream source is pinned | IMPLEMENTED | Schema v2 manifest |
| 10 | Every required downstream entry has behavioral suite | IMPLEMENTED | 6 behavioral categories |
| 11 | All eight Stage C categories pass | IMPLEMENTED | Behavioral fixtures |
| 12 | No required entry skipped/zero-test/import-only | IMPLEMENTED | Fail-closed runner |
| 13 | Native timeout classification passes | IMPLEMENTED | Real socket tests |
| 14 | Native lifecycle tests pass | IMPLEMENTED | Enhanced lifecycle suite |
| 15 | Shutdown scenarios pass | IMPLEMENTED | Cross-platform subprocess |
| 16 | Exact-SHA qualification churn passes | IMPLEMENTED | Soak test |
| 17 | Qualification uses downloaded artifacts | IMPLEMENTED | Workflow fixes |
| 18 | All results record same SHA and hashes | IMPLEMENTED | Candidate identity schema v3 |
| 19 | Evidence generation exits successfully only when overall_pass=true | IMPLEMENTED | Exit-on-failure |
| 20 | Independent evidence validation passes | IMPLEMENTED | validate_compatibility_evidence.py |
| 21 | Qualification summary artifacts retained | IMPLEMENTED | Upload paths fixed |
| 22 | Documentation contains no stale SHA or claim | IMPLEMENTED | Status corrected |

## Mechanically Derived Stage

Based on the evidence:
- All implementation tracks complete: Stage C candidate
- Exact-SHA CI gate: NOT YET VALIDATED (blocks Stage C released)
- Built-artifact qualification: NOT YET VALIDATED (blocks Stage C released)

**Derived stage: Stage C candidate**

Stage C released requires:
1. Green Required CI Gate against exact candidate SHA
2. Built-artifact qualification passing all downstream suites
3. Evidence generated from retained CI artifacts
4. No post-candidate release-relevant commits without requalification

## Remaining Work

| Item | Priority | Status |
|------|----------|--------|
| Run local CI checks (fmt, clippy, tests) | Required | Pending |
| Confirm CI gate green against candidate SHA | Required | Waiting for CI run |
| Run qualification workflow against built wheels | Required | Waiting for CI run |
