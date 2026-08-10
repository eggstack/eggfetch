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
  --allowed compat/httpx/0.28.1/allowed-differences.toml
```

## Categories

- `required-now` — Must match for current stage
- `required-later` — Accepted gap assigned to later roadmap phase
- `intentional-difference` — Reviewed and explicitly allowed
- `not-public` — Excluded from contract
- `not-applicable` — Reference feature cannot apply to current product stage
- `resolved` — Previously-allowed differences that have been implemented and verified (tracked in `resolved-differences.toml` as an audit trail)

## Status

This profile targets HTTPX 0.28.1. The compatibility stage is **Stage C candidate** — high-fidelity HTTPX 0.28.1 compatibility for the documented Python ≥3.10 asyncio-supported surface, including the qualified low-level transport features documented here. Phase 6 qualification SHA: `40beeec09f3e88db8901f39388da665c47ab84f6`.
