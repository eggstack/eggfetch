# HTTPX Parity Correction — Closure Status

This record is the exact-SHA-bound status for the HTTPX 0.28.1 compatibility
facade. Historical phase and corrective-pass records remain in the git history
and referenced plans; counts below are only from the runs named here.

## Current corrective pass

Native compressed raw body selection is implemented; final metadata, native
cancellation, CI evidence, and planning-hygiene closure remain open.

Corrective baseline SHA: `52f540483322a47db11ebff5e17079d21370473f`

Adapter executable baseline SHA:
`1aa5cb986bbdb03b92588eb1c7b7ad7070d9ffe7`

Corrective plan: `plans/httpx-final-metadata-ci-hygiene-corrective-pass.md`

Open planning-hygiene item: PR #20 must be preserved under a unique plan path
on `main` and then closed without merging its conflicting closure-record path.

The prior local evidence below is provisional for this corrective pass. It is
retained as evidence for the adapter executable baseline, but it does not
close the metadata, native cancellation, planning-hygiene, or CI obligations.
The existing decoded-header policy remains deliberately bounded: automatic
core decompression removes `Content-Encoding` and `Content-Length` as before.

Provisional validation bound to the adapter executable baseline:

- Canonical `./scripts/check.sh`: passed. Rust formatting, lint policy,
  clippy, workspace tests/doctests, extension build, 532 Python behavior
  tests, and the 117-test compatibility smoke kernel all passed.
- Full pinned compatibility suite: `1379 passed, 0 failed, 0 skipped,
  0 xfailed` with `httpx==0.28.1`.
- Native raw differential module and lifecycle/response focused tests passed,
  including sync/async gzip parity, one-shot selection, source accounting,
  immediate source failure, cancellation, and close behavior.
- API oracle: `0 unexplained`, `0 stale`, and `0 unresolved` differences;
  all 121 reported differences match the active documented allowlist.
- Feature-gated core suites passed: gzip, brotli, zstd, deflate, and proxy.
  The timeout test server was made deterministic for feature configurations
  that do not enable `test-util`.
- Extended supporting checks passed: documentation examples/links, FFI (30
  tests), resource monitor, lifecycle (44), soak (11), merge-lossless (12),
  and benchmarks. Rust 1.80 MSRV was skipped because that toolchain is not
  installed locally.

The downstream portfolio requires its separate isolated shim-installation
runner; invoking its fixture directory directly against the ordinary
`httpx==0.28.1` environment is not valid shim evidence and was not used to
claim closure.

## Historical superseded final closure evidence

Starting SHA: `6ae10308b9db1e215eca19027d4ca9b7575900f6`

Redirect implementation SHA: `aec56c0` (`fix: redirect cookie security and body replay parity`)

Raw-stream implementation SHA: `11eb77a` (`fix: raw stream lifecycle parity (final closure 02)`)

Final implementation SHA: `11eb77a7e121d4b83e75a1bd87ebf7ac240e9046`

Final pushed tree SHA (CI-tested): `fd2c0bd141eb284e14b52af929eb949d62483dfb`

Final-closure plan: `plans/httpx-parity-final-closure-03-verification-status-hygiene.md`

Related implementation plans:

- `plans/httpx-parity-final-closure-01-redirect-security-replay.md`
- `plans/httpx-parity-final-closure-02-raw-stream-lifecycle.md`

Historical status: complete (superseded evidence; retained for history).

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

CI run ID: `30969985491`

CI checked-out SHA: `fd2c0bd141eb284e14b52af929eb949d62483dfb`

CI conclusion: passed. Workflow `CI`, job `ci` (`92192018602`), duration
3m35s. The immediately preceding documentation-only tree had one flaky RSS
resource-test attempt and a successful retry (`30969660204`, job
`92191448150`); no code changes were made.

The final run executed the existing `./scripts/check.sh` routine path. GitHub
reported only the platform-level Node.js 20 action deprecation annotation.

PR #16: closed with a supersession comment; obsolete planning branch not merged.

PR #17: closed with a supersession comment; obsolete planning branch not merged.

## Provisional designation

**Stage C candidate — final deterministic closure remains open pending
metadata, native cancellation, planning-hygiene, and CI evidence.**

The final status updates are documentation-only. No executable files changed
between the implementation SHA and the CI-tested tree; the successful retry
therefore validates the same executable implementation.
