# HTTPX Parity Correction — Closure Status

This record is the exact-SHA-bound status for the HTTPX 0.28.1 compatibility
facade. Historical phase and corrective-pass records remain in the git history
and referenced plans; counts below are only from the runs named here.

## Current corrective pass — evidence record

Current designation: **Stage C candidate — final corrective qualification
pending**.

The corrective executable tree is frozen at
`044e02f3ab5c4bafaab7aa9e91283f109b3675ba`. The profile remains pending because
the full aggregate compatibility run exposed three intermittent fixture-order
timeouts, although the three failures pass when isolated. Proxy headers and
arbitrary Python `ssl_context` objects also remain bounded differences.

Environment: CPython 3.12.3, pytest 9.1.1, pytest-asyncio 1.4.0,
`httpx==0.28.1`, `httpcore==1.0.9`, and `socksio==1.0.0`.

Corrective evidence at that SHA:

- Focused transport differential: **71 passed**, 3 reference deprecation warnings.
- Python behavior suite: **532 passed**.
- API oracle: **76 allowed matches**, 0 stale allowed, 0 unexplained, 0 resolved-in-active.
- Full pinned compatibility: **1479 passed, 3 failed** in aggregate; the three
  failures are `test_read_after_close_returns_data`, `test_read_phase_timeout`,
  and `test_real_proxy_server_forward`, and all three pass in a direct isolated
  invocation.
- Rust formatting, workspace clippy with `-D warnings`, and the Rust portions
  of Tier 1 passed. The Python behavior phase also passed when run directly;
  the first canonical shell session stalled after starting that phase and was
  rerun by its exact command successfully.
- Remote routine CI passed for documentation commit
  `94bb4bf2f0d7b23147cf9a8e06876193a78661cb`: workflow run `31512028521`, job
  `93847967260`, completed 2026-08-11.

The full suite result is intentionally recorded as pending rather than claimed
as qualification evidence until its aggregate fixture-order instability is
resolved or the qualification policy explicitly accepts the isolated rerun.

Native compressed raw body selection, wire metadata parity, native async
cancellation/lease release, planning preservation, and final CI verification
are complete for the documented HTTPX 0.28.1 asyncio-supported surface.

Corrective baseline SHA: `52f540483322a47db11ebff5e17079d21370473f`

Adapter executable baseline SHA:
`1aa5cb986bbdb03b92588eb1c7b7ad7070d9ffe7`

Metadata/cancellation final executable SHA:
`cf4680ac056bf241ca4f4e8fa0e076459bccc9e3`

Archived implementation-plan preservation SHA:
`cf4680ac056bf241ca4f4e8fa0e076459bccc9e3`

Corrective plan: `plans/httpx-final-metadata-ci-hygiene-corrective-pass.md`

Archived implementation plan:
`plans/httpx-native-compressed-raw-adapter-implementation-plan.md`

Closure record: `plans/httpx-native-compressed-raw-adapter-closure.md`.
PR #20 is closed with preservation comment `5195664156`; its conflicting file
was not merged.

Prior documentation-only evidence-binding SHA:
`0ad2275c0fae50140d87c5b2f9b6da07d08dde3c`. This commit is a documentation-only
descendant of the final executable SHA and contains only status/metadata changes.

The core response retains only the original wire `Content-Encoding` and
`Content-Length` values in read-only metadata. Automatic decompression still
removes those headers from visible core response headers; the compatibility
facade overlays the retained values without deriving wire length from decoded
bytes or changing decoder selection.

## Historical validation bound to prior executable SHAs

Focused command:

```sh
python -m pytest \
  crates/eggfetch-python/tests/compat/test_raw_stream_httpx_differential.py \
  crates/eggfetch-python/tests/compat/test_raw_stream_lifecycle.py \
  crates/eggfetch-python/tests/compat/test_response.py \
  crates/eggfetch-python/tests/compat/test_response_metadata_parity.py \
  -q --strict-markers
```

Result: `98 passed, 0 failed, 0 skipped, 0 xfailed`. The final native
differential rerun after test-only assertion tightening was `40 passed`.

Routine command: `./scripts/check.sh`.

Result: passed on the executable SHA. Rust formatting, lint suppression
policy, clippy, 471 core tests, workspace tests/doctests, extension build,
532 collected Python behavior tests (492 passed, 40 skipped), and 117
compatibility smoke tests passed.

Full pinned command:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
  python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Result: `1384 passed, 0 failed, 0 skipped, 0 xfailed` in 57.73 seconds with
`httpx==0.28.1`. The two existing `PytestRemovedIn10Warning` warnings remain
non-failing fixture deprecations.

API-oracle result: 121 allowed matches, 0 stale allowed entries, 0
unexplained differences, 0 resolved-in-active entries, and 0
requires-resolution differences.

The downstream portfolio requires its separate isolated shim-installation
runner; invoking its fixture directory directly against the ordinary
`httpx==0.28.1` environment is not valid shim evidence and was not used to
claim closure.

## Historical final CI and repository hygiene evidence

The existing single Ubuntu `ci` job ran the unchanged `./scripts/check.sh`
routine path.

- Workflow: `CI`
- Run ID: `31034568903`
- Checked-out SHA: `cf4680ac056bf241ca4f4e8fa0e076459bccc9e3`
- Run started: `2026-08-05T18:24:23Z`
- Job: `ci`
- Job ID: `92403300331`
- Job started: `2026-08-05T18:24:30Z`
- Job completed: `2026-08-05T18:28:29Z`
- Conclusion: passed
- Attempt: 1 (no retry)

Documentation-only CI evidence for the consistency correction:

- Substantive correction SHA: `1a4ac12a29ae005d0c4a6dacf0936aab50b72c27`
- Documentation-only CI run ID: `31037562941`
- Documentation-only CI job ID: `92413466674`
- Conclusion: passed (3m41s)
- This commit is documentation-only and does not alter executable evidence.

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

## Historical superseded designation

**Historical Stage C candidate — final deterministic closure remains open pending
metadata, native cancellation, planning-hygiene, and CI evidence.**

The final status updates are documentation-only. No executable files changed
between the implementation SHA and the CI-tested tree; the successful retry
therefore validates the same executable implementation.
