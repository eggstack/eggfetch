# HTTPX Parity Correction — Closure Status

## Implementation SHA

<!-- Updated on commit -->
SHA: 429ebe4

Follow-up starting SHA: 40c036f
Follow-up audited baseline: 4d46edc0b1609430d7b053e6376121b746ba0cd1
Follow-up plan: `plans/httpx-parity-follow-up-corrective-pass.md`

## Corrective closure baseline

- Audited baseline: `7de195716ef64787535d089020a99891bae4aa8e`.
- Corrective plan: `plans/httpx-parity-narrow-corrective-closure.md`.
- Current result: follow-up corrective implementation complete locally; remote CI verification pending. Prior corrective evidence below remains historical.
- The earlier phase counts below are historical evidence, not closure proof.

## Phase-by-Phase Completion

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Entrypoints & client lifecycle | Complete |
| 2 | Request/response semantics | Complete |
| 3 | Transport, mount, hook dispatch | Complete |
| 4 | Redirect, auth, cookie state | Complete |
| 5 | Differential closure (historical phase) | Complete |
| Corrective closure | Timeout, state, cookie, replay, transport, and Tier 1 parity corrections | Complete locally |

## Focused Test Command

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Result: full pinned suite passes — 1,314 tests, 0 failures, 2 pytest deprecation warnings.

Tier 1: `./scripts/check.sh` passes with 532 non-compat tests and 95 routine compatibility-kernel tests.

Extended validation also passes the Rust workspace, feature and API checks locally; MSRV remains skipped when the configured toolchain is unavailable.

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

Prior corrective closure completed at `d419267`; its CI run `30688525760` is historical and does not validate this follow-up pass.

## Final Claim Decision

**Stage C candidate — corrective closure complete.** Do not infer complete parity from the historical Phase 5 evidence above. The `eggfetch.compat.httpx` module is a Stage C candidate with:
- All roadmap findings linked to passing focused and full-suite test cases
- All behavioral differences documented with narrow intentional-difference records
- API oracle clean (zero unexplained, zero stale)
- `./scripts/check.sh` Tier 1 validation passes
- No new CI jobs, matrices, or evidence architecture introduced
