# HTTPX Parity Correction — Closure Status

This record is the exact-SHA-bound status for the HTTPX 0.28.1 compatibility
facade. Historical phase and corrective-pass records remain in the git history
and referenced plans; counts below are only from the runs named here.

## Current corrective pass — qualified

Current designation: **Stage C qualified** for the documented Python 3.10+
asyncio-supported HTTPX 0.28.1 surface.

The final executable qualification SHA is
`64a1e2c3f3cea7ddc6eeabcd85a67a4d7a17cb26`. The pass-04 corrections keep
direct/UDS/H3 read budgets on response-body chunks after transport setup,
preserve HTTPX's omitted-versus-explicit-`None` timeout phases, stabilize
NO_PROXY and proxy endpoint matrices, and keep Python response and streaming
body work on the runtime that owns the transport. Live sync streams retain a
runtime lease so they remain readable after `Client.close()`.

Environment: CPython 3.12.3, pytest 9.1.1, pytest-asyncio 1.4.0,
`httpx==0.28.1`, `httpcore==1.0.9`, and `socksio==1.0.0`. IPv6 loopback was
available; no capability-based skips were used.

Evidence bound to the exact SHA:

- `./scripts/check.sh`: passed, including serialized Rust workspace tests,
  clippy, doctests, extension build, 532 Python behavior tests, and the 130
  test compatibility smoke kernel.
- Full pinned compatibility command: three consecutive clean runs, each
  **1564 passed**, in **148.78s**, **128.20s**, and **130.62s**, with 11
  non-failing warnings.
- API oracle: 0 unexplained, 0 stale, and 0 resolved-in-active differences;
  the manifest is valid.
- Documentation examples and internal links: passed (122 Python blocks across
  55 Markdown files; all internal links valid).
- `cargo doc --workspace --all-features --no-deps` and core doctests: passed;
  rustdoc emitted only pre-existing FFI/private-link warnings.
- Required downstream runner: **4/4 packages passed**, with no failed, error,
  or skipped required suites (`respx` 5/5, `httpx-sse` 4/4, `httpx-auth` 5/5,
  `httpx-ws` 4/4). Its reported pip-check dependency warnings are diagnostic
  only; behavioral suites passed. The refreshed candidate wheel is SHA-256
  `69c177d4d7fa0384da99a2a3fed316f544804cc3945fa398e65f92d959b543ef`.

Retained bounded differences are unchanged: non-empty `Proxy(headers=...)`
is rejected at conversion, arbitrary Python `ssl_context` objects are not
forwarded into the Rust proxy engine, the valid four-element socket-option
pointer form is outside the safe-Rust boundary, and direct Hyper/UDS/H3 header
acquisition is not separately exposed from the transport future. These are
documented in `compat/httpx/0.28.1/allowed-differences.toml`.

## Historical corrective pass — superseded qualified evidence record

Current designation: **Stage C qualified**.

Qualification was completed on 2026-08-11 from executable commit
`52b187744d062840879f6e7752c87753021e2415`. The working tree started from
`dfcf518` (the plan's earlier `915caa` baseline was already superseded by that
plan-only commit). The pass fixes timeout phase accounting and shared proxy
fixture response framing while retaining explicitly bounded proxy-header and
arbitrary Python `ssl_context` differences.

Environment: CPython 3.12.3, pytest 9.1.1, pytest-asyncio 1.4.0,
`httpx==0.28.1`, `httpcore==1.0.9`, and `socksio==1.0.0`.

Evidence bound to the executable SHA:

- Canonical `./scripts/check.sh`: passed, including Rust formatting, lint
  suppression policy, clippy, workspace tests/doctests, extension build,
  **532** Python behavior tests, and the **130**-test compatibility smoke
  kernel.
- Focused corrective/transport evidence: **207 passed**; the proxy-header
  and timeout/NO_PROXY differential subsets were also rerun independently
  with no failures.
- Full pinned compatibility command
  (`EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers`):
  three consecutive clean runs, each **1526 passed**, in **108.09s**,
  **106.65s**, and **106.79s**.
- API oracle: **76** differences, all allowed; **0** unexplained, **0** stale
  allowed, **0** resolved-in-active, and no requires-resolution entries.
- Downstream command
  (`python scripts/run_downstream_compat.py --artifact-manifest target/downstream-qualification/artifact-manifest.json --required-only`):
  **4/4 required packages passed**, with 0 failures, errors, or skips:
  `respx` 5/5, `httpx-sse` 4/4, `httpx-auth` 5/5, and `httpx-ws` 4/4.
  The local manifest used the candidate wheel SHA
  `ef07b468114f2db144699ea3dca33dc7d6555ff70deeb7576bf04d742912c419` and
  controlled replacement SHA
  `11914ce75c418d2c75acce35d12973087543b1a8a7ba4dbb9daf827c05ff2f7f`.

Remote routine CI passed for documentation commit
`1bb07cf5ed9d4b31d66dd5f85f1993400479ade5`: workflow run `31536326327`, job
`93928168428`, checked out the exact pushed SHA, and completed successfully in
4m04s. The workflow ran the unchanged `./scripts/check.sh` routine path.

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
