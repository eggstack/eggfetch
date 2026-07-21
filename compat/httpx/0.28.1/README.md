# HTTPX 0.28.1 Compatibility Profile

This directory contains the machine-readable compatibility profile for
HTTPX 0.28.1 drop-in compatibility (Phase 0).

## Files

- `profile.toml` — Defines the reference package, supported surfaces, and categories
- `allowed-differences.toml` — Registry of reviewed, allowed behavioral differences
- `reference-api.json` — Golden manifest of HTTPX 0.28.1 public API (generated)

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
  --allowed compat/httpx/0.28.1/allowed-differences.toml
```

## Categories

- `required-now` — Must match for current stage
- `required-later` — Accepted gap assigned to later roadmap phase
- `intentional-difference` — Reviewed and explicitly allowed
- `not-public` — Excluded from contract
- `not-applicable` — Reference feature cannot apply to current product stage

## Status

This profile targets HTTPX 0.28.1. The compatibility stage is **Phase 0**.
