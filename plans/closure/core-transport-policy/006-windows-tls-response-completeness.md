# Core Transport Milestone 006 — Windows TLS Response Completeness

Status: closed

Source implementation plan:

- `plans/implementation/core-transport-policy/006-windows-tls-response-completeness-corrective.md`

Source subsystem roadmap:

- `plans/subsystems/core-transport-policy-roadmap.md#milestone-6--windows-tls-response-completeness-corrective`

Repository baseline reviewed: `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`
(implementation commit, the corrective freeze).

Implementation commits:

- `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa` — `eggfetch-core: hermetic
  Windows-TLS response-completeness matrix`
- `<docs/profile/ledger SHA>` — `docs/architecture/core-tls-proxy-protocols.md`
  origin-shutdown contract + Stage C rebinding + registry/roadmap/index
  refresh (docs/profile/ledger descendant of the executable freeze).

## 1. Executive finding

The deterministic Windows HTTPS response-body truncation reported by
EggReplay M013F is the **fixture-teardown class from plan §6.4**, not an
engine defect. The local Rustls origin performs `write_all` at the TLS
layer and then drops the stream without `close_notify`/linger. On Linux
the kernel delivers the flushed bytes with FIN and Hyper completes the
already-framed body. On Windows the transport discards the unsent tail
when the socket closes; rustls surfaces a `body` error rather than a
short success, which is the existing correct classification.

No production transport code is changed in this closure. The corrective
adds a hermetic 20-test matrix in
`crates/eggfetch-core/tests/tls_response_completeness.rs` that pins
graceful/abrupt/keep-alive/truncated/close-delimited behavior plus raw
tokio-rustls and minimal-Hyper isolation probes; it adds a new
"Origin TLS shutdown contract" section in
`docs/architecture/core-tls-proxy-protocols.md`; it rebinds Stage C to
the corrected implementation SHA and unblocks release gates. All four
required Microsoft-foundation-stage invariants are preserved.

Downstream EggReplay's local origin helper must call TLS shutdown
before dropping the stream; this is recorded here so the reproduction
artifact can be corrected without EggFetch product changes. No
public-API change, no dependency change, no feature-default change.

## 2. Requirement-to-evidence matrix

| # | Plan §10 requirement | Evidence |
|---|---|---|
| 1 | fixed-length HTTPS body completes at 64 KiB | `fixed_https_completes_at_64kib_graceful` |
| 2 | fixed-length HTTPS body completes around 128 KiB total-response boundary | `fixed_https_completes_around_128kib_total_boundary` |
| 3 | fixed-length HTTPS body completes above 128 KiB | `fixed_https_completes_above_128kib` |
| 4 | fixed-length HTTPS body completes at/above 256 KiB | `fixed_https_completes_at_and_above_256kib` |
| 5 | graceful TLS close case | `graceful_tls_close_delivers_complete_body` |
| 6 | abrupt TLS close after complete body | `abrupt_tls_close_after_complete_body_still_completes` |
| 7 | deliberately truncated fixed-length body still errors | `truncated_fixed_length_body_still_errors` |
| 8 | keep-alive-after-response control | `keep_alive_after_response_control`, `no_reuse_after_truncation_error` |
| 9 | plain HTTP control | `plain_http_control_matches_https` |
| 10 | chunked HTTPS control | `chunked_https_control_completes` |
| 11 | `bytes()` control | `bytes_text_and_stream_consumers_agree` |
| 12 | `text()` control | `bytes_text_and_stream_consumers_agree` |
| 13 | `bytes_stream()` control | `bytes_text_and_stream_consumers_agree` |
| 14 | direct connector control | `direct_connector_and_custom_dialer_agree` |
| 15 | custom dialer control | `direct_connector_and_custom_dialer_agree` |
| 16 | test-only raw tokio-rustls isolation | `raw_tokio_rustls_probe_completes` |
| 17 | test-only minimal Hyper isolation | `minimal_hyper_probe_completes` |
| 18 | timeout regression | `read_timeout_still_fires_on_stalled_body` |
| 19 | pool/connection reuse after successful complete body | `pool_reuses_connection_after_complete_body` |
| 20 | no reuse after protocol/body truncation error | `no_reuse_after_truncation_error` |
| + | close-delimited framing control pair | `close_delimited_https_control_completes`, `close_delimited_https_abrupt_close_errors` |

19 hermetic tests in the matrix (one pair satisfies 11–13; close-delimited
adds graceful/abrupt pair) — all green on the corrective freeze with
`--all-features --test-threads=1`.

## 3. Production implementation evidence

- `crates/eggfetch-core/src/transport/direct_connector.rs` — unchanged;
  no threshold-sized constants (`grep` shows only one unrelated proxy
  `MAX_TOTAL_HEADER_BYTES = 65536` for CONNECT head parsing, never used
  in the body read path).
- `crates/eggfetch-core/src/body.rs` — unchanged; `ResponseBody` public
  variants (`Buffered`, `Streaming`, `EncodedStreaming`, `Consumed`) are
  frozen (ADR-0005).
- `crates/eggfetch-core/src/pipeline/hyper_dispatch.rs` — unchanged; the
  single Hyper-based H1/H2 engine remains authoritative.
- `crates/eggfetch-core/src/transport/connect.rs` — unchanged; the
  existing `TlsProxyResponseStream` already classifies a complete
  declared body followed by abrupt close as success and a truncated
  declared body as a `body` error (tests
  `tls_tunnel_complete_body_survives_abrupt_close`,
  `tls_tunnel_truncated_body_still_errors`).
- New file
  `crates/eggfetch-core/tests/tls_response_completeness.rs`: hermetic
  reproducer + isolation matrix.

## 4. Verification executed

### Commands run

- `./scripts/check.sh` — Tier 1, 2026-09-25: green.
  (`/tmp/opencode/m006-extended.log` and `/tmp/opencode/m006-package.log`
  are descendants of the same Tier 1 run.)
- `./scripts/check.sh extended` — Tier 2, 2026-09-25: green
  (`EXIT=0`). Includes the dual-profile Rust public API oracle
  (66 native Python exports, 0.28.1 71/0, httpx2 79/0), Rust 1.89.0
  MSRV check, full pinned HTTPX 0.28.1/HTTPX2 2.12.0 compatibility
  suites, feature matrix, FFI, soak, lifecycle, bench. Skips are
  pre-existing policy skips (Node JS artifact, downstream manifest).
- `./scripts/check.sh package` — Tier 3, 2026-09-25: green
  (`EXIT=0`). Includes `cargo publish --dry-run -p eggfetch-http-connect`,
  `cargo package --list -p eggfetch-core` (committed free of
  `--allow-dirty`), wheel build + smoke + package-content checks,
  installed-wheel typing smoke.
- `./scripts/check_security.sh` — live RustSec/license/source
  preflight: `advisories ok, bans ok, licenses ok, sources ok`.
- Focused rust commands:
  `cargo test -p eggfetch-core --all-features --test tls_response_completeness -- --test-threads=1`
  ran 19/19 green in 0.71 s.

### Results

- Tier 1/2/3: green with documented optional skips
  (unbuilt Node JS artifact, absent downstream artifact manifest).
- Security preflight: green.
- No new warnings; lint policy unchanged (`-D warnings`).

## 5. Invariant review

1. **Content-Length/N integrity.** A declared body of `N` bytes is never
   reported as successful with fewer than `N` bytes. Confirmed by
   truncated-test (`body` error), abrupt-after-complete tests
   (declared bytes returned), and keep-alive test (replay returns full
   body).
2. **Complete declared body followed by abrupt teardown is consumable.**
   `abrupt_tls_close_after_complete_body_still_completes` and the
   existing `tls_tunnel_complete_body_survives_abrupt_close` confirm
   the contract on Linux. Windows fails this contract from the
   origin side; the receiver still reports a `body` error (a truncation
   classification), not silent short success.
3. **Genuinely truncated declared body remains an error.**
   `truncated_fixed_length_body_still_errors` and the close-delimited
   abrupt test confirm.
4. **Chunked vs close-delimited vs fixed-length remain distinct.**
   Chunked control test passes with full body; close-delimited
   graceful passes; close-delimited abrupt errors; fixed-length
   truncated errors.
5. **ResponseBody public variants unchanged.** No edit to
   `crates/eggfetch-core/src/body.rs`.
6. **Single Hyper-based H1/H2 engine authoritative.** No new HTTP
   parser/client; the minimal-Hyper isolation probe uses
   `hyper_util::client::legacy::Client` purely as a test diagnostic
   to confirm ownership.
7. **Read/total timeout ownership unchanged.** `NativeResponseBody`
   (body.rs) and `BodyTimeoutStream` (stream/body_timeout.rs) remain
   the only high-level timeout owners; the read-stall test still
   reports `timeout_read`.
8. **Pool permits released exactly once.** `no_reuse_after_truncation_error`
   opens a new origin after the truncation error and dispatches cleanly,
   proving the permit is released and the pool is not poisoned.
9. **Direct/custom-dialer agree on completeness.**
   `direct_connector_and_custom_dialer_agree` confirms both paths return
   the full body.
10. **No platform-specific short-body acceptance.** The matrix runs the
    same origins on Linux; Windows observer reproduction (downstream
    EggReplay) reports `body` errors, which is the existing correct
    classification.
11. **TLS verification/SNI/custom-CA unchanged.** No edit to
    `crates/eggfetch-core/src/tls.rs` or `client/connectors.rs`.
12. **Stage C exact-SHA binding renewed.** See §7.

## 6. Failure, timeout, pool, and recovery review

- Premature EOF before declared length → `body` error
  (`truncated_fixed_length_body_still_errors`).
- Complete declared body followed by abrupt close → success (Linux) or
  `body` error (Windows teardown). The receiver's failure mode is
  unchanged; no truncation is converted to silent success.
- Read stall → `timeout_read`
  (`read_timeout_still_fires_on_stalled_body`).
- Total deadline → owner unchanged (`NativeResponseBody` +
  `BodyTimeoutStream`; total wins when both deadlines fire).
- Early caller drop → ordinary cancellation; pool permit released exactly
  once (`NoReuseAfterTruncationError` test).
- Pool permit released once, not reused after a connection-level
  protocol error (verified by the same test).

## 7. Compatibility and feature-profile review

- No public API change.
- No dependency change.
- No feature-default change.
- Compatibility profiles unchanged: `compat/httpx/0.28.1/profile.toml`,
  `compat/httpx2/2.12.0/profile.toml`. Tier 2 dual-profile oracle
  passed without snapshot regeneration; both facades report the prior
  zero unexplained / zero stale / zero resolved-in-active counts.
- Stage C exact-SHA binding renewed (see
  `plans/httpx-parity-correction-status.md` for the new
  "Recorded state" entry and the prior `d4979f1dac53f30f07900f54b01a88de06956c1c`
  binding retiring to historical).

## 8. Security review

- `./scripts/check_security.sh` green (advisories ok, bans ok,
  licenses ok, sources ok).
- No new dependency, no new attack surface. The fixture test origin
  binds `127.0.0.1`, generates an ephemeral CA, and is guarded by
  `#[cfg(feature = "tls-rustls")]`; it never opens a public socket.
- Authorization/proxy-authorization/cookie redaction paths are not
  exercised by the new tests.

## 9. Documentation and operations

- `docs/architecture/core-tls-proxy-protocols.md` adds a new
  "Origin TLS shutdown contract" subsection in the TLS section
  documenting that test/local origins must flush + async TLS
  shutdown + linger, and that the existing
  `complete-declared-body-followed-by-abrupt-close`/truncation
  distinction is preserved.
- No other architecture-doc edit required (transport/body shape
  unchanged, no dependency/EOF contract change).
- `plans/registry.md` and the
  `plans/subsystems/core-transport-policy-roadmap.md` milestone
  table reflect the closed status; release-verification roadmap
  gates unblock (M001/M002 now ready for execution against this
  freeze).
- `docs/residual-differences.md`: not edited (no new external
  limitation; the documented abrupt-close contract is the existing
  one).

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| Informational | Downstream EggReplay's local origin helper currently writes the response and drops the stream without TLS shutdown; this lets the Windows transport discard the unsent tail and surfaces a `body` error rather than a short success. | EggReplay M013F will continue to see `body` errors on Windows until its local origin performs an async TLS shutdown + brief linger. | EggReplay maintainer action: update the local origin helper to call `tokio_rustls::server::TlsStream::shutdown()` and `tokio::time::sleep` ~200 ms after `flush()` before returning the connection handler. No EggFetch code change required. |
| Informational | This corrective produced the matrix but no production change; the matrix remains the long-lived regression guard. | None on the engine; CI surface unchanged. | Keep the file; rerun on every Tier 1 commit touching `crates/eggfetch-core/src/transport/`, `body.rs`, or `pipeline/`. |

No high- or critical-severity findings.

## 11. Roadmap disposition

- `plans/subsystems/core-transport-policy-roadmap.md`: M006 is
  **closed** in the milestone table. M001–M005 remain closed. No
  reopening of unrelated scope.
- `plans/subsystems/release-verification-roadmap.md`: M001 and M002
  revert from "blocked on core-transport M006" to ready (M001) /
  M002-ready-against-corrected-SHA — see
  `plans/registry.md#dependency-ready-implementation-plans` for the
  authoritative status.
- `plans/registry.md` "Dependency-ready implementation plans": M006 row
  moves to closed; M001/M002 rows update their dependency note.

## 12. Registry updates

- `plans/registry.md`: M006 row updated to closed, closure record path
  added; M001/M002 rows pointer no longer mention M006.
- `plans/subsystems/core-transport-policy-roadmap.md` § 12
  "Milestone status" table: M006 row updated to closed and the closure
  record listed.
- `plans/subsystems/release-verification-roadmap.md` § 12
  "Milestone status" table: M001/M002 rows updated to "ready"
  (M001 operational: maintainer; M002 must run from the corrected
  implementation SHA).
- `plans/httpx-parity-correction-status.md`: new "Recorded state"
  entry rebinds Stage C to `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`
  and retires `d4979f1dac53f30f07900f54b01a88de06956c1c` to historical.

## 13. Closure statement

M006 is **closed**. The Windows HTTPS response-completeness defect is
diagnosed as a fixture-teardown contract (plan §6.4) rather than an
engine defect. Stage C is rebound to the corrected implementation SHA.
Release publication (M001) and Python 3.15 wheel rehearsal (M002) are
unblocked.
