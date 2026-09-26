# Core Transport Policy M006C1 — Windows qualification and downstream closure

Status: closed

Source implementation plan:

- `plans/implementation/core-transport-policy/006c1-windows-qualification-and-downstream-closure.md`

Source subsystem roadmaps:

- `plans/subsystems/core-transport-policy-roadmap.md`
- `plans/subsystems/release-verification-roadmap.md`

Repository baseline reviewed: `6616d061aba523182ad00a37a64a4919b44bd73d`
(documentation-only closure descendant; EggFetch executable, test,
validation, workflow, and qualification inputs are unchanged).

## 1. Executive finding

M006C1 closes the remaining native-Windows and downstream qualification gaps
from M006. The native-Windows EggFetch response-completeness matrix passed
19/19, and the corrected EggReplay origin passed both hosted Windows Rust
verification and interception qualification with the original 300 KiB direct,
Eggress-direct, and MITM response envelope.

The EggReplay TLS origin now flushes the response, sends TLS `close_notify`
with `shutdown()`, and performs a bounded two-second peer-close linger. The
Windows tests confirmed that EggFetch receives all 307,200 response bytes
through both direct dialers, returns the complete MITM response, durably records
307,200 bytes, and reports no upstream body error.

No EggFetch transport, dependency, API, feature-default, or qualification input
changed. Stage C remains bound to
`5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`. No additional transport change or
Stage C rebinding is required.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result |
|---|---|---|
| Native Windows EggFetch matrix | [Actions run 36193582776](https://github.com/eggstack/eggfetch/actions/runs/36193582776), [Windows job](https://github.com/eggstack/eggfetch/actions/runs/36193582776/job/108264181282), EggFetch SHA `fc269b93cb4b5b54801758ff4e064bdfb18df18c` | 19 passed, 0 failed |
| Windows environment | Windows Server 2025, `Microsoft Windows NT 10.0.26100.0`, `x86_64-pc-windows-msvc`, stable `rustc 1.98.1`, `cargo 1.98.1` | Recorded in job log |
| EggFetch graceful response completeness through 300 KiB | `fixed_https_completes_at_and_above_256kib`, graceful TLS close, raw tokio-rustls and minimal Hyper probes, direct/custom dialer agreement | Passed |
| EggFetch malformed/truncated response behavior | `truncated_fixed_length_body_still_errors`, `close_delimited_https_abrupt_close_errors`, `no_reuse_after_truncation_error` | Passed; no short-body success |
| Corrected EggReplay Windows Rust lane | [Actions run 36211265347](https://github.com/eggstack/eggreplay/actions/runs/36211265347), [verify Windows job](https://github.com/eggstack/eggreplay/actions/runs/36211265347/job/108318122662), EggReplay SHA `5efc6f9c892bb5b1c2e84330a0c40a38f1de0de7` | Passed formatting, check, Clippy, and workspace tests (313 passed, 0 failed) |
| Corrected EggReplay Windows interception lane | [interception Windows job](https://github.com/eggstack/eggreplay/actions/runs/36211265347/job/108318122612) | Passed; all 163 interception test cases and CLI interception tests passed |
| EggFetch direct 300 KiB download | `eggfetch_direct_downloads_full_large_tls_response` in `tests/mitm.rs`; asserts full 300 KiB body from plain-TCP and Eggress-direct dialers | Passed |
| EggReplay MITM 300 KiB transfer and durable recording | `mitm_streams_large_bodies_both_directions`; asserts complete wire body, exact durable body length, and no upstream error | Passed |
| EggFetch Tier 1 control | `./scripts/check.sh` passed on 2026-09-25, recorded in M006 closure; no EggFetch executable or validation input changed in M006C1 | Remains valid |
| Stage C binding | `plans/httpx-parity-correction-status.md` | Unchanged at `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa` |

## 3. Production implementation evidence

EggFetch production code is unchanged. The downstream fixture correction is in
EggReplay commit `5efc6f9c892bb5b1c2e84330a0c40a38f1de0de7`:

- shared MITM/direct-download origin performs one response per TLS connection;
- response bytes are written in bounded chunks;
- the origin shuts down TLS cleanly and lingers, bounded by two seconds, for
  the peer's TLS close;
- MITM and direct-download response lengths are restored to 300 KiB;
- stale Windows truncation diagnoses were removed.

This is test-fixture lifecycle behavior only. EggFetch continues to own
outbound HTTP/TLS transport; EggReplay retains its existing transport
boundaries.

## 4. Verification executed

### EggFetch native-Windows matrix

Command:

```text
cargo test -p eggfetch-core --all-features --test tls_response_completeness -- --test-threads=1 --nocapture
```

Result: 19 passed, 0 failed on Windows Server 2025, stable Rust 1.98.1, SHA
`fc269b93cb4b5b54801758ff4e064bdfb18df18c`. The complete focused test output is
retained in [the Windows job log](https://github.com/eggstack/eggfetch/actions/runs/36193582776/job/108264181282).

### EggReplay hosted Windows qualification

Qualifying EggReplay SHA:
`5efc6f9c892bb5b1c2e84330a0c40a38f1de0de7`.

- [Rust workspace Windows verification](https://github.com/eggstack/eggreplay/actions/runs/36211265347/job/108318122662): fmt, check, Clippy, and tests passed; 313 tests passed, 0 failed. The `mitm` integration target passed 24/24, including `eggfetch_direct_downloads_full_large_tls_response` and `mitm_streams_large_bodies_both_directions`.
- [Interception Windows qualification](https://github.com/eggstack/eggreplay/actions/runs/36211265347/job/108318122612): dedicated interception and CLI-interception jobs passed; the `mitm` integration target passed 24/24, and the interception crate test targets reported 163 passed, 0 failed.
- The complete [EggReplay Actions run](https://github.com/eggstack/eggreplay/actions/runs/36211265347) concluded successfully, including its Windows Python bindings job.
- Both large-response tests use `300 * 1024` bytes. The direct test exercises plain TCP and `EggressDialer::direct()` and asserts the complete body. The MITM test asserts complete wire delivery, exact persisted response-body length, and no upstream error.

The shared Actions run also passed Linux stable, Linux Rust 1.89, macOS stable,
and dependency-boundary jobs.

### EggFetch control gates and exact-SHA binding

EggFetch Tier 1 remains green from the M006 implementation freeze recorded in
`plans/closure/core-transport-policy/006-windows-tls-response-completeness.md`.
M006C1 changed no EggFetch executable, dependency, fixture, validation script,
profile, or workflow, so Tier 2/3 and Stage C were not invalidated. Stage C
remains `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`.

## 5. Abrupt-close characterization

The EggFetch matrix preserves the documented teardown distinction. Graceful
TLS shutdown delivers complete fixed-length bodies through the 300 KiB
envelope. Abrupt-close and explicitly truncated cases remain separately
characterized; a genuinely short declared body continues to error and is not
reported as success. Raw tokio-rustls and minimal Hyper probes pass their
graceful cases. This evidence confirms the fixture-teardown diagnosis without
requiring Windows to match Linux behavior for malformed abrupt teardown.

## 6. Unresolved findings

None for M006C1. M006's existing documented limitations and the existing
release procedures remain in force.

## 7. Roadmap disposition

- M006 remains closed as the historical technical diagnosis.
- M006C1 is closed with both native-Windows and downstream Windows evidence.
- Core-transport policy roadmap is closed for this corrective.
- Release-verification M001 publication and M002 wheel rehearsal are reopened
  as **ready**; each retains its own execution and maintainer approval gates.
- No EggFetch transport work is reopened or required.

## 8. Registry and release-gate updates

- `plans/registry.md`: M006C1 closed; M001 and M002 ready; M006C1 release gate
  removed from blocked work.
- `plans/subsystems/core-transport-policy-roadmap.md`: M006C1 closed with this
  record linked.
- `plans/subsystems/release-verification-roadmap.md`: M001 and M002 ready.
- `plans/README.md`: current execution gate updated with both hosted Windows
  results.
- The historical M006 closure record is unchanged.
