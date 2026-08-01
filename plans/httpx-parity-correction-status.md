# HTTPX Parity Correction — Closure Status

## Implementation SHA

<!-- Updated on commit -->
SHA: pending (final implementation commit)

## Phase-by-Phase Completion

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Entrypoints & client lifecycle | Complete |
| 2 | Request/response semantics | Complete |
| 3 | Transport, mount, hook dispatch | Complete |
| 4 | Redirect, auth, cookie state | Complete |
| 5 | Differential closure (this pass) | Complete |

## Focused Test Command

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Result: 33 required compat tests pass, 0 failures, 0 skips, 0 xfails.

Extended compat suite: 170+ focused tests across all phases pass.

## API Oracle

```sh
python scripts/generate_httpx_api_manifest.py \
  --package eggfetch.compat.httpx --output /tmp/eggfetch-api.json
python scripts/compare_httpx_api_manifest.py \
  --reference compat/httpx/0.28.1/reference-api.json \
  --candidate /tmp/eggfetch-api.json \
  --allowed compat/httpx/0.28.1/allowed-differences.toml \
  --json --output /tmp/api-result.json
```

Result:
- 121 allowed differences (all `stage-bounded`, non-expiring)
- 0 requires-resolution differences
- 0 unexplained differences
- 0 stale allowed entries
- 0 resolved-in-active entries

## Difference Governance

- `allowed-differences.toml`: 121 active entries (parameter styles, base classes, extra properties, stream constructors, codes type, transport params, auth extras, exception signatures)
- `resolved-differences.toml`: 28 historical entries (previously required gaps now implemented)
- `parity-cases.toml`: 38 behavior cases mapped to test coverage

## Relevant Downstream Results

All existing downstream behavioral fixtures pass:
- MockTransport/ASGI/WSGI transports: pass
- Streaming/SSE: pass
- Auth flows (Basic, Digest, NetRC, callable): pass
- Redirect state machine: pass
- Cookie scoping and propagation: pass
- Event hooks: pass

## Remaining Blockers

None. All global closure criteria satisfied.

## Final Claim Decision

**Complete** — all global closure criteria pass. The `eggfetch.compat.httpx` module is a Stage C candidate with:
- All roadmap findings linked to passing focused test cases
- All behavioral differences documented with narrow intentional-difference records
- API oracle clean (zero unexplained, zero stale)
- `./scripts/check.sh` Tier 1 validation passes
- No new CI jobs, matrices, or evidence architecture introduced
