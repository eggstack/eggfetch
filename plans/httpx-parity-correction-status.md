# HTTPX Parity Correction — Closure Status

This record is the exact-SHA-bound status for the HTTPX 0.28.1 compatibility
facade. Historical phase and corrective-pass records remain in the git history
and referenced plans; counts below are only from the runs named here.

## Current corrective pass — Corrective 06 final semantic truthfulness

Current designation: **Corrective 06 open**. The previous Stage C qualification
at `c44d4f25ffebc1a792335163ae4bc106076b3963` is retained as historical
evidence only. Executable changes required by Corrective 06 invalidate that
qualification; no new `qualification-sha` is assigned until Corrective 07
runs against a frozen executable commit.

Plan: `plans/httpx-parity-corrective-06-final-semantic-truthfulness.md`.

Scope (narrow):

- Track A: SSLContext translation must be genuinely fail-closed and not
  silently drop unobservable or unmapped state.
- Track B: One native extension parser for all sync/async buffered/streaming
  paths; truthful trace-callback support across all four quadrants.
- Track C: 101 `network_stream` wrapper type must match the caller's API
  mode (sync vs async), not the response conversion path.
- Track D: H2-only policy must propagate through SNI and SOCKS routes, or
  those routes must be explicitly classified as bounded differences.

## Historical corrective pass — Corrective 05 exact-SHA closure (superseded)

**Current bound execution evidence**: `c44d4f25ffebc1a792335163ae4bc106076b3963`,
qualified on 2026-08-19. Corrective 06 is open against this baseline;
the qualification is invalidated as soon as executable changes land for
Corrective 06.

### Corrective 05 evidence

- Focused corrective gate: **143 passed**, 0 failed/skipped/xfail (including
  the compressed-stream cancellation fixture boundary).
- Tier 1 `./scripts/check.sh`: passed; 942 workspace non-doctest Rust tests,
  11 core doctests, 532 Python behavior tests, and 130 compatibility smoke
  tests passed.
- Tier 2 `./scripts/check.sh extended`: passed, including the feature matrix,
  feature-gated tests, docs, FFI, resource monitor, lifecycle, soak, and
  downstream gates; the optional MSRV check was skipped because Rust 1.80 is
  not installed.
- Remote routine CI: passed for documentation/ledger commit `7cf29e1` in
  workflow run `32229114248`, job `95994969763`, in 4m24s; it ran the unchanged
  `./scripts/check.sh` path.
- Full pinned compatibility: three clean runs, each **1798 passed**, with 26
  non-failing warnings and no skips or xfails. The clean runs completed in
  456.07s, 462.35s, and 462.61s.
- API oracle: 71 allowed matches, 0 unexplained, stale, or resolved-active
  differences; the 74-symbol manifest is valid.
- Downstream portfolio: respx, httpx-sse, httpx-auth, and httpx-ws all pass
  against a wheel built from the final executable SHA. Diagnostic pip-check
  dependency warnings do not represent behavioral failures. Candidate wheel
  SHA-256: `c6b8a6b6bdd7812cd56c15411d4d78c734115ac791ad9c86dd1803be106e9049`;
  controlled HTTPX replacement wheel SHA-256:
  `f22dedeff6934ad02dfa8e532fd7ed330a9c2d5a2363b45cb571dd637a3634e3`.

### Retained bounded differences

- SSLContext state that rustls cannot represent (including unsupported cipher,
  ALPN, TLS-version, or client-certificate provenance) fails closed with
  `TypeError`; representable helper-created and passthrough state is supported.
- HTTP/2 `stream_id` remains unavailable because the current Hyper abstraction
  does not expose it as response metadata.
- HTTP/2 origin framing through an HTTP CONNECT proxy remains HTTP/1.1; direct
  TLS, cleartext prior knowledge, direct-specialized, and UDS H2 paths are
  separately covered. This residual is pinned by parity case H2-009.
- HTTPX's four-element null-pointer `socket_options` form is outside the safe
  Rust boundary; the safe three-element form is supported.
- Ordinary pooled responses and internal CONNECT tunnels expose no writable
  network stream. Only 101 responses own an upgraded stream, with
  `start_tls` limited to safe inner TCP variants.

### Resolved in this closure

Proxy-leg headers, proxy/origin TLS trust isolation, target and SNI wire
metadata, the supported trace-observer subset, 101 network-stream ownership,
H2-only TLS ALPN enforcement, cleartext H2 prior knowledge, and direct/UDS H2
typing/enforcement are implemented and covered by the focused and full gates.

### Environment and boundary

Evidence used CPython 3.12.3, pytest 9.1.1, pytest-asyncio 1.4.0,
`httpx==0.28.1`, `httpcore==1.0.9`, and `socksio==1.0.0`, with IPv6 loopback
available and no capability skips. The executable diff boundary is
`4571cb55bc2ff49822608d750dfef185cff40ebc` through the final SHA above.

## Historical corrective pass — Phase 06 final rebaseline qualified (superseded)

Current designation: **Stage C qualified** for the documented Python 3.10+
asyncio-supported HTTPX 0.28.1 surface.

Phase 06 completes the HTTPX parity remaining-parity program. It rebaselines
the compatibility profile after Phases 01-05 implementation, eliminates stale
or contradictory difference records, classifies residual gaps narrowly, and
re-runs the repository's exact-SHA qualification procedure.

Phase 06 qualification is bound to executable SHA
`48bad19fa1bb7ab7c91bcd67787efb2e41127fff`, qualified on 2026-08-18. Only
documentation and status records follow that frozen executable SHA.

### What changed in Phases 01-05

- **Phase 01 (TLS):** Safe SSLContext translation boundary — `create_ssl_context()` returns a real `ssl.SSLContext`; arbitrary contexts are classified and rejected before dispatch if unrepresentable by rustls.
- **Phase 02 (HTTP/2):** HTTP/2-only prior-knowledge mode enabled; ALPN protocol negotiation.
- **Phase 03 (Transport hints):** `target`, `sni_hostname`, `trace` observer, `stream_id` via extensions dict.
- **Phase 04 (Network stream):** `NetworkStream`, `ConnectionMetadata`, `UpgradedStream` for 101/CONNECT upgrade lifecycle.
- **Phase 05 (Proxy):** `Proxy(headers=...)` forwarded on the proxy leg; `Proxy(ssl_context=...)` translated to native `TlsConfig` for proxy endpoint TLS; proxy TLS config separated from origin TLS config.

### Evidence bound to the exact SHA

- `./scripts/check.sh` (Tier 1): passed, including serialized Rust workspace tests,
  clippy, doctests, extension build, 532 Python behavior tests, and the 130
  test compatibility smoke kernel.
- Full pinned compatibility command: **1735 passed**, 0 failed, 0 skipped,
  0 xfailed, in ~230s. Environment: CPython 3.12.3, pytest 9.1.1,
  pytest-asyncio 1.4.0, `httpx==0.28.1`, `httpcore==1.0.9`, `socksio==1.0.0`.
  No capability-based skips were used.
- API oracle: **71** allowed matches, 0 unexplained, 0 stale, and 0
  resolved-in-active differences; the manifest is valid.
- Required downstream runner: **4/4 packages passed** (`respx` 5/5,
  `httpx-sse` 4/4, `httpx-auth` 5/5, `httpx-ws` 4/4). Candidate wheel SHA-256:
  `16dba105efd8cdc2461e9fb3fb6cf67d62577e7fc6efa99183fbc7d9b59951f5`.
- `resolved-differences.toml`: `PROXY-SSL-CONTEXT-001` updated to reflect
  Phase 05 implementation; `PROXY-HEADERS-001` resolved in earlier pass.
- All 532 non-compat Python tests pass. All core Rust tests pass (the
  pre-existing `ordinary_response_has_no_network_stream` test was fixed to
  use a local server instead of hitting external httpbin.org).

### Retained bounded differences (unchanged)

Non-empty `Proxy(headers=...)` is now forwarded to the proxy leg (resolved).
Arbitrary Python `ssl_context` objects that cannot be represented by rustls
are rejected at construction time with a clear TypeError (resolved for safe
subset, residual for unrepresentable). The valid four-element socket-option
pointer form is outside the safe-Rust boundary. Direct Hyper/UDS/H3 header
acquisition is not separately exposed from the transport future. These are
documented in `compat/httpx/0.28.1/allowed-differences.toml`.

### FunctionAuth forward-drift note

HTTPX master contains commit `ae1b9f66238f75ced3ced5e4485408435de10768`
(`Expose FunctionAuth in __all__`, 2025-12-10). EggFetch already has an
internal `_FunctionAuth` adapter. For this 0.28.1 closure, public `FunctionAuth`
is not added. The next stable HTTPX rebaseline should evaluate public
export/signature/behavior.

Environment: CPython 3.12.3, pytest 9.1.1, pytest-asyncio 1.4.0,
`httpx==0.28.1`, `httpcore==1.0.9`, and `socksio==1.0.0`. IPv6 loopback was
available; no capability-based skips were used.

Evidence bound to the exact SHA:

- `./scripts/check.sh`: passed, including serialized Rust workspace tests,
  clippy, doctests, extension build, 532 Python behavior tests, and the 130
  test compatibility smoke kernel.
- Focused closure command covering the NO_PROXY, environment, timeout, and
  proxy differential suites: **146 passed**, with no failures, skips, or
  xfails. Native `proxy::tests` also passed **80 tests**. IPv6 loopback was
  available, and malformed-form rows ran without capability skips.
- Full pinned compatibility command: three consecutive clean runs, each
  **1623 passed**, in **151.69s**, **152.89s**, and **149.86s**, with 11
  non-failing warnings and no skips or xfails. IPv6 loopback was available;
  no capability-based skips were used.
- IPv6 truth-table evidence matches HTTPX 0.28.1: bare `::1` bypasses;
  bare `::1:8080` and synthetic `2001:db8::1` are accepted and route through
  the proxy for the loopback target; `[::1]`, `[::1]:8080`, `::1/128`,
  `[::1]/128`, and `2001:db8::1/128` fail pre-dispatch with `InvalidURL` and
  zero origin/proxy requests. Generic bare/leading-dot domain, near-match,
  explicit-port, default-port, scheme-qualified, and IPv4 CIDR-looking route
  evidence also passed.
- API oracle: **71** allowed matches, 0 unexplained, 0 stale, and 0
  resolved-in-active differences; the manifest is valid.
- Documentation examples and internal links: passed (122 Python blocks across
  55 Markdown files; all internal links valid).
- `cargo doc --workspace --all-features --no-deps` and core doctests: passed;
  rustdoc emitted only pre-existing FFI/private-link warnings.
- Required downstream runner: **4/4 packages passed**, with no failed, error,
  or skipped required suites (`respx` 5/5, `httpx-sse` 4/4, `httpx-auth` 5/5,
  `httpx-ws` 4/4). Its reported pip-check dependency warnings are diagnostic
  only; behavioral suites passed. The candidate wheel is SHA-256
  `3a64c303b319fa26234cc872c335c081d0c9b3f77a7c52bc63bc5f2cc2d10a2a` and
  the controlled replacement wheel is
  `a7507fe4fb76693c1ed5022012f396c77f86b5853c9926949341c333bfbe0649`.

- Explicit serialized `cargo test --workspace --all-features` passed with
  **942 non-doctest tests** across the workspace plus 11 core doctests.
  Documentation checks passed: 122 Python blocks across 55 Markdown files,
  all internal links, all-features rustdoc, and core doctests. The canonical
  `./scripts/check.sh` routine gate also passed on this SHA.
- Remote routine CI passed for documentation/status commit `5d3cf37`: workflow
  run `31847177274`, job `94915879545`, checked out the exact pushed SHA, and
  completed successfully in approximately 4m31s. The workflow ran the
  unchanged `./scripts/check.sh` routine path.

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
