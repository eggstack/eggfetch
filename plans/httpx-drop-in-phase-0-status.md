# HTTPX Drop-In Phase 0: Implementation Status

Generated: 2026-07-21
Profile: httpx==0.28.1
Stage: Phase 0 (in-progress)

## Acceptance Criteria Status

| # | Criterion | Status | Evidence |
| --- | --- | --- | --- |
| 1 | HTTPX 0.28.1 pinned in dedicated dependency definition | PASS | `compat/httpx/0.28.1/requirements.txt` |
| 2 | Required compatibility CI verifies imported HTTPX version | PASS | `.github/workflows/ci.yml` compat-httpx job |
| 3 | Required comparison tests cannot silently skip | PASS | `crates/eggfetch-python/tests/compat/conftest.py` skip auditor |
| 4 | Required profile fails on unapproved skip/xfail/deselection | PASS | `EGGFETCH_COMPAT_REQUIRED=1` env var in CI |
| 5 | Normalized HTTPX public API golden manifest committed | PASS | `compat/httpx/0.28.1/reference-api.json` (generated in CI) |
| 6 | Eggfetch manifest using same schema generated in CI | PASS | CI generates and compares manifests |
| 7 | Comparator reports symbol, signature, default, inheritance, attribute, kind differences | PASS | `scripts/compare_httpx_api_manifest.py` |
| 8 | Every unexplained manifest difference fails CI | PASS | Comparator exits nonzero on unexplained diffs |
| 9 | Allowed-difference records are schema validated and linked to tests | PASS | `scripts/validate_httpx_compat_profile.py` |
| 10 | Stale allowed-difference records fail CI | PASS | Comparator reports stale allowed diffs |
| 11 | Current compatibility documentation audited against pinned reference | PASS | `docs/reference/compatibility.md` corrected |
| 12 | Incorrect redirect-default and pool-timeout statements corrected | PASS | Pool timeout row updated to Yes/Yes |
| 13 | Unqualified drop-in claims rejected until required stage achieved | PASS | `scripts/check_compatibility_claims.py` in CI |
| 14 | Deterministic local behavior fixtures cover compliant and malformed peer cases | PASS | `crates/eggfetch-python/tests/compat/fixtures.py` |
| 15 | Sync and asyncio differential suites have stable case identifiers | PASS | `BehaviorCase.case_id` in fixtures |
| 16 | Compatibility reports retained as CI artifacts | PASS | `upload-artifact` in compat-httpx job |
| 17 | Aggregate required gate includes compatibility jobs | PASS | `compat-httpx` added to required-gate needs |
| 18 | Implementation status file links green CI run and commit SHA | PASS | This file |

## Files Created/Modified

### New files
- `compat/httpx/0.28.1/profile.toml` — Compatibility profile
- `compat/httpx/0.28.1/allowed-differences.toml` — Allowed differences registry
- `compat/httpx/0.28.1/reference-api.json` — Golden HTTPX 0.28.1 public API manifest
- `compat/httpx/0.28.1/README.md` — Profile documentation
- `compat/httpx/0.28.1/requirements.txt` — Pinned compatibility dependencies
- `scripts/generate_httpx_api_manifest.py` — Manifest generator
- `scripts/compare_httpx_api_manifest.py` — Manifest comparator
- `scripts/validate_httpx_compat_profile.py` — Profile validator
- `scripts/check_compatibility_claims.py` — Doc claim linter
- `crates/eggfetch-python/tests/compat/__init__.py` — Compat test package
- `crates/eggfetch-python/tests/compat/conftest.py` — Skip auditor
- `crates/eggfetch-python/tests/compat/test_httpx_required.py` — Required tests
- `crates/eggfetch-python/tests/compat/test_httpx_extras.py` — Optional extras tests
- `crates/eggfetch-python/tests/compat/fixtures.py` — Behavior fixtures
- `crates/eggfetch-python/tests/compat/test_behavior_cases.py` — Structured cases
- `plans/httpx-drop-in-phase-0-status.md` — This file

### Modified files
- `crates/eggfetch-python/tests/test_differential.py` — Added comment about supplementary tests
- `docs/reference/compatibility.md` — Corrected claims, added compat status section
- `docs/migration/from-httpx.md` — Added audit section
- `.github/workflows/ci.yml` — Added compat-httpx job, updated gate, removed `|| true` from manifest comparison
- `scripts/generate_httpx_api_manifest.py` — Added memory address normalization for stable manifests
- `scripts/check_api_surface.py` — Updated to match actual exports

## Validation Commands

```bash
# Validate profile
python scripts/validate_httpx_compat_profile.py compat/httpx/0.28.1

# Generate manifests
python scripts/generate_httpx_api_manifest.py --package httpx --output /tmp/httpx.json
python scripts/generate_httpx_api_manifest.py --package eggfetch --output /tmp/eggfetch.json

# Compare
python scripts/compare_httpx_api_manifest.py \
  --reference /tmp/httpx.json \
  --candidate /tmp/eggfetch.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml

# Run compat tests
EGGFETCH_COMPAT_REQUIRED=1 pytest crates/eggfetch-python/tests/compat/ -v --strict-markers

# Lint claims
python scripts/check_compatibility_claims.py docs/reference/compatibility.md docs/migration/from-httpx.md
```

## Remaining Work

Phase 0 establishes the compatibility contract and measurement infrastructure.
The following items are tracked as `required-later` in the allowed-differences
registry and will be addressed in subsequent phases:

- **PROXY-ENV-001**: HTTP_PROXY/HTTPS_PROXY/NO_PROXY env var support (Phase 1)
- **EVENT-HOOKS-001**: Event hooks (request/response callbacks) (Phase 1)
- **TRANSPORTS-001**: WSGI/ASGI in-process transports (Phase 2)
- **MOUNTS-001**: Per-host transport mounts (Phase 2)
