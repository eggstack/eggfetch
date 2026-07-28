# HTTPX 0.28.1 Compatibility Profile

This directory contains the machine-readable compatibility profile for
HTTPX 0.28.1 drop-in compatibility (Stage C candidate).

## Files

- `profile.toml` — Defines the reference package, supported surfaces, and categories
- `allowed-differences.toml` — Registry of reviewed, allowed behavioral differences with exact typed tuples
- `resolved-differences.toml` — Historical ledger of resolved differences (audit trail only)
- `reference-api.json` — Golden manifest of HTTPX 0.28.1 public API (generated)
- `upstream-test-inventory.md` — Catalog of upstream HTTPX tests mapped to eggfetch cases
- `upstream-derived-cases.toml` — Machine-readable mapping of 36 derived upstream test cases
- `performance-budgets.toml` — Latency and throughput budgets for critical paths
- `compat/downstream/` — 12-package consumer portfolio for downstream validation

## Usage

Validate the profile:
```bash
python scripts/validate_httpx_compat_profile.py compat/httpx/0.28.1
```

Generate the reference manifest:
```bash
python scripts/generate_httpx_api_manifest.py --package httpx --output compat/httpx/0.28.1/reference-api.json
```

Generate the eggfetch manifest:
```bash
python scripts/generate_httpx_api_manifest.py --package eggfetch --output /tmp/eggfetch.json
```

Compare manifests:
```bash
python scripts/compare_httpx_api_manifest.py \
  --reference compat/httpx/0.28.1/reference-api.json \
  --candidate /tmp/eggfetch.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml \
  --resolved compat/httpx/0.28.1/resolved-differences.toml \
  --artifact-manifest /tmp/artifact-manifest.json \
  --candidate-identity /tmp/candidate-identity.json
```

Validate qualification workflow and evidence:
```bash
python scripts/validate_qualification_workflow.py
python scripts/validate_compatibility_evidence.py evidence.json
python scripts/candidate_identity.py identity.json
```

Normalize pytest results and generate artifact manifest:
```bash
python scripts/normalize_pytest_result.py --input /tmp/test-results.json --output /tmp/test-results-contract.json
python scripts/generate_artifact_manifest.py --wheel dist/*.whl --output /tmp/artifact-manifest.json
```

## Categories

- `required-now` — Must match for current stage
- `required-later` — Accepted gap assigned to later roadmap phase
- `intentional-difference` — Reviewed and explicitly allowed
- `not-public` — Excluded from contract
- `not-applicable` — Reference feature cannot apply to current product stage
- `resolved` — Previously-allowed differences that have been implemented and verified (tracked in `resolved-differences.toml` as an audit trail)

## Status

This profile targets HTTPX 0.28.1. The compatibility stage is **Stage C candidate**. The corrective closure pass applies typed difference records, lossless merge semantics, native lifecycle proof fixtures, and behavioral downstream fixtures.
