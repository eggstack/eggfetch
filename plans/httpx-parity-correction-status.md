# HTTPX Parity Correction — Closure Status

This record is the exact-SHA-bound status for the HTTPX 0.28.1 compatibility
facade. Historical phase and corrective-pass records remain in the git history
and referenced plans; counts below are only from the runs named here.

## Current corrective pass

The prior raw-stream completion statement below is superseded by the corrective
closure in `plans/httpx-parity-raw-stream-final-corrective-closure.md`. The
redirect closure remains accepted. This record is reopened while the raw-stream
state, accounting, modality, and finalization behavior are corrected and
re-verified.

Corrective baseline SHA: `eb397395f8a2a0bf0621fbcd9deece98647a85cb`

Status: raw-stream corrective closure in progress; the pure-Python raw-stream
semantics are corrected, but final native compressed-raw parity is blocked by a
missing core raw-body boundary.

Plan: `plans/httpx-parity-raw-stream-final-corrective-closure.md`

Executable corrective implementation SHA:
`20a6a2c66ba8d10449d36d1fcc9f575cd6660554`

Validation bound to that implementation:

- Focused raw-stream differential, lifecycle, metadata, and response tests:
  `86 passed`.
- Tier 1 compatibility smoke kernel: `117 passed`.
- API oracle: `0 unexplained`, `0 stale`, and `0 unresolved` differences;
  all 121 reported differences match the active documented allowlist.
- Rust formatting, lint-suppression policy, clippy, Rust workspace tests and
  doctests, and the Python extension rebuild passed through the canonical
  `./scripts/check.sh` run. The aggregate 532-test non-compat Python phase
  stalled in this environment during local-server tests and was interrupted;
  isolated reruns of the affected auth file passed (`31 passed`).
- The full pinned compatibility suite was attempted but not completed after
  the same environment-level stall; it is not claimed as passing evidence.

### Stop-condition blocker

The native Python response is created from
`eggfetch_core::Response::bytes_stream()` after the core pipeline has applied
content decompression. That is the only current native stream owner exposed to
`PyStreamingResponse`; consequently native `iter_raw()` and `aiter_raw()` can
only observe decoded chunks for a compressed response. The Python facade cannot
recover the encoded bytes without adding a second decompressor, and sending a
request with decompression disabled would break the decoded iterator on the
same response.

The smallest separately reviewable adapter surface is a core-owned response
boundary that preserves the pre-decompression raw stream (or an authoritative
raw stream/counter) alongside the existing decoded stream, with the Python
binding selecting the raw stream for `iter_raw()` and `aiter_raw()`. A broad
transport redesign is not justified by this pass. The focused differential
suite proves the mismatch against a gzip loopback response before any such
adapter is claimed complete.

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

## Final designation

**Stage C candidate — final deterministic closure complete for the documented
HTTPX 0.28.1 asyncio-supported surface.**

The final status updates are documentation-only. No executable files changed
between the implementation SHA and the CI-tested tree; the successful retry
therefore validates the same executable implementation.
