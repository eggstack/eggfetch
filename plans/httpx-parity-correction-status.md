# HTTPX Parity Correction — Closure Status

This record is the exact-SHA-bound status for the HTTPX 0.28.1 compatibility
facade. Historical phase and corrective-pass records remain in the git history
and referenced plans; counts below are only from the runs named here.

## Final closure evidence

Starting SHA: `6ae10308b9db1e215eca19027d4ca9b7575900f6`

Redirect implementation SHA: `aec56c0` (`fix: redirect cookie security and body replay parity`)

Raw-stream implementation SHA: `11eb77a` (`fix: raw stream lifecycle parity (final closure 02)`)

Final implementation SHA: `11eb77a7e121d4b83e75a1bd87ebf7ac240e9046`

Final pushed tree SHA (CI-tested): `d4967f110247db659e5c48bbfca0acb3fb1e84bc`

Final-closure plan: `plans/httpx-parity-final-closure-03-verification-status-hygiene.md`

Related implementation plans:

- `plans/httpx-parity-final-closure-01-redirect-security-replay.md`
- `plans/httpx-parity-final-closure-02-raw-stream-lifecycle.md`

Current status: complete.

The implementation SHA contains all executable changes for this closure. The
verification/status commits after it are documentation-only unless explicitly
noted. A CI result for the implementation tree therefore remains applicable to
later documentation-only trees, but is not a new execution of those commits.

## Routine validation

SHA checked: `11eb77a7e121d4b83e75a1bd87ebf7ac240e9046`

Command: `./scripts/check.sh`

Result: passed.

- Python behavior tests: 532 passed.
- Compatibility smoke kernel: 115 passed.
- Rust workspace tests, doctests, clippy, formatting, and Python extension build: passed.
- No routine tests were skipped.
- Environment: CPython 3.12.3, pytest 9.1.1, pytest-asyncio 1.4.0, maturin 1.14.1.

The smoke kernel is the only compatibility suite in Tier 1. The complete
pinned suite and API oracle remain extended/manual evidence.

## Full pinned HTTPX compatibility

SHA checked: `11eb77a7e121d4b83e75a1bd87ebf7ac240e9046`

Pinned version: `httpx==0.28.1`

Command:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Result: `1389 passed, 0 failed, 0 skipped, 0 xfailed` in 52.25 seconds.

Warnings: 2 `PytestRemovedIn10Warning` deprecation warnings from the existing
class-scoped fixture in `test_httpx_required.py`; they are not test failures.

## API oracle

SHA checked: `11eb77a7e121d4b83e75a1bd87ebf7ac240e9046`

Commands:

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

- 121 allowed matches, all `stage-bounded`.
- 0 unexplained differences.
- 0 stale allowed entries.
- 0 resolved-in-active entries.
- 0 requires-resolution differences.

The active allowlist contains 121 documented stage-bounded differences;
`resolved-differences.toml` remains the historical ledger of resolved entries.

## CI scope and repository hygiene

The existing CI workflow runs only routine Tier 1 validation through
`./scripts/check.sh`. It does not prove the complete pinned compatibility
matrix or API oracle; those are the manual/local results above.

CI run ID: `30969434477`

CI checked-out SHA: `d4967f110247db659e5c48bbfca0acb3fb1e84bc`

CI conclusion: passed on retry. Workflow `CI`, retry job `ci`
(`92191448150`), duration 3m34s. The initial attempt in the same run failed
only in the existing RSS-based `test_repeated_client_creation_stabilizes`
resource test; the retry passed without code changes.

The successful retry executed the existing `./scripts/check.sh` routine path.
GitHub reported only the platform-level Node.js 20 action deprecation
annotation.

PR #16: closed with a supersession comment; obsolete planning branch not merged.

PR #17: closed with a supersession comment; obsolete planning branch not merged.

## Final designation

**Stage C candidate — final deterministic closure complete for the documented
HTTPX 0.28.1 asyncio-supported surface.**

The final status updates are documentation-only. No executable files changed
between the implementation SHA and the CI-tested tree; the successful retry
therefore validates the same executable implementation.
