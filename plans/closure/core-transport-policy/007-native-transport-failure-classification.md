# Core Transport Milestone 007 — Native Transport Failure Classification

Status: closed

Source implementation plan:

- `plans/implementation/core-transport-policy/007-native-transport-failure-classification.md`

Source subsystem roadmap:

- `plans/subsystems/core-transport-policy-roadmap.md#milestone-7--native-transport-failure-classification`

Repository baseline reviewed: `3fc58fbd99ecb496749b833ee7b27436fe3b412d`
(implementation freeze; planning baseline `092dcc7a0cdc76451cbca833ef80175ecd7b82b9`).

Implementation commits:

- `326c4a6a4830c842a02ca51a27f910167ced66a9` — `eggfetch-core: native
  transport failure classification (M007)`
- `33e6a7fb332bdbaca7c52e718d850943aaa21184` — `chore: satisfy
  clippy::assert_is_empty on stable 1.99` (one-line test-only fix in the
  `eggfetch-http-connect` wire suite; the floating stable toolchain moved
  1.98.1 → 1.99.0 mid-qualification and 1.99's pedantic clippy adds the
  `assert_is_empty` lint, which fires on pre-existing code that is clean
  under 1.98.1. Zero behavior change.)
- `3fc58fbd99ecb496749b833ee7b27436fe3b412d` — `chore: satisfy pedantic clippy
  drift and pin the oracle toolchain` (the remaining 1.99 pedantic fallout the
  first fix did not reach: `assert_is_empty` in core/CLI/FFI tests, `mut_mut` at
  the 26 CLI mock-server call sites, two load-bearing-laziness
  `unnecessary_lazy_evaluations` sites in `response.rs`, and the MSRV-blocked
  `fetch_update` deprecation in `retry.rs`; plus the real validation-script fix
  in `scripts/check_rust_public_api.py`, which pinned `RUSTUP_TOOLCHAIN` for
  its `cargo-public-api` half only and so let `cargo-semver-checks 0.49.0` run
  on the ambient stable — 1.99.0 emits rustdoc JSON v61, which that tool cannot
  parse, failing the oracle for reasons unrelated to the change. Both halves now
  share one pinned-toolchain env, matching the invocation documented in
  `compat/rust-public-api/README.md`.) The Stage C binding sits on this third
  commit because it is the newest executable-input freeze.
- Stage C rebinding, profile renewals, registry/roadmap refresh, and this
  closure record follow as docs/planning-only descendants of the freeze and
  do not invalidate the binding (no executable, test, validation, workflow,
  package, API snapshot, or compatibility fixture changes in them).

## 1. Executive finding

M007 is complete. `eggfetch_core::Error` gains exactly one additive public
method, `transport_failure_kind()`, returning a new non-exhaustive
`TransportFailureKind::{Connect, Tls, Protocol, Cancelled}` (or `None` when
typed evidence does not prove a category). The `Error` enum itself is
unchanged: no new variants, no `#[non_exhaustive]` on `Error`, unchanged
`kind()` tokens, `Display`, exhaustiveness, and `Result` alias.
`Client::execute_http_body()` and `NativeResponseBody` signatures and shapes
are unchanged, as are timeout, admission, `TransportIoTimeout`,
`DialErrorKind`, and `RequestFailure`/`NetworkFailureKind` authorities. No
retry, redirect, proxy-fallback, pool, deadline, TLS-trust, or route policy
changed; no dependency, feature flag, or adapter behavior changed.

Two implementation facts discovered against the plan are recorded here rather
than hidden. First, `std::io::Error` hides its custom payload from
`source()` (exposed only via `get_ref()`), so nested `rustls::Error`
evidence behind hyper-rustls wrappers is reachable only through `get_ref()`
descent; the classifier descends both edge kinds under a 32-visit bound.
Second, Hyper reports truncated `Content-Length` bodies as `Kind::Body`
with `UnexpectedEof` (not `is_incomplete_message()`) and malformed chunks
as `Kind::Body` with `InvalidInput` (not `is_parse()`), with no public
kind accessor; because `Error::Hyper` is constructed only at the
`IncomingErrorBody` polling boundary, those I/O kinds inside it are treated
as proven body-framing evidence, while a bare `Error::Io` with the same
kinds stays unknown. Both decisions are covered by focused fixtures and
unit tests.

## 2. Requirement-to-evidence matrix

| # | Plan acceptance criterion | Evidence | Result |
|---|---|---|---|
| 1 | Public non-exhaustive generic enum in the error domain | `TransportFailureKind` in `error.rs`, re-exported from crate root; oracle snapshots +12 lines × 6 profiles | Pass |
| 2 | One general classifier method on `Error` | `Error::transport_failure_kind()`; compile-contract tests in `public_api_contracts.rs` and the new integration file | Pass |
| 3 | `Error` variants/kind/Display/exhaustiveness/`Result` unchanged | `error.rs` diff is purely additive (471 insertions, 0 deletions in the enum); `transport_failure_does_not_change_kind_or_display` unit test; oracle + semver green | Pass |
| 4 | `execute_http_body`/`NativeResponseBody` shapes unchanged | No signature/shape edits (`git diff` shows none); `response_body_public_shape` suite green in Tier 1 | Pass |
| 5 | Dispatch TLS/cancellation/protocol/connect classify without consumer downcasts | `native_dispatch_*` fixtures (see §4) | Pass |
| 6 | Body incomplete/malformed framing classifies `Protocol` without timeout/EOF change | `native_body_*` fixtures (see §4) | Pass |
| 7 | Admission/`TimeoutPhase`/`TransportIoTimeout`/`DialErrorKind`/`NetworkFailureKind` authority retained | Unit non-regression tests; `RequestFailure` code untouched; `request_failure_tests` green in Tier 1 | Pass |
| 8 | Unknown/ambiguous returns `None`; no message parsing | `transport_failure_unknown_stays_unknown`, `UnexpectedEof` unit test; `grep` shows no `to_string`/`format!` inspection in the classifier | Pass |
| 9 | Retry/proxy behavior unchanged | Retry delegates to the same evidence (`retry.rs` diff is a pure delegation); `native_transport_failure_retry_and_proxy_policy_unchanged` pins `is_error_retryable()`; proxy parse walk retained (see §6) | Pass |
| 10 | No dependency or feature flag added | `Cargo.toml`/`Cargo.lock` untouched; `cargo tree` delta none (§7) | Pass |
| 11 | `native-http1,tls-rustls` and `standard-http1,tls-rustls` compile; focused tests pass | `cargo check` both profiles clean; new integration file 10/10 under `--all-features` and under `native-http1,tls-rustls` | Pass |
| 12 | Tier 1 + Tier 2 green incl. MSRV and API/semver oracle | §7 | Pass |
| 13 | Exact-SHA Stage C rebinding | Ledger entry + both profile renewals in the closure commit; prior `bb6e0732...` retired to historical | Pass |
| 14 | Docs distinguish `kind()`, `TransportFailureKind`, `NetworkFailureKind`, timeout facts, `DialErrorKind` | `docs/reference/errors.md` subsection; `core-engine.md`; `rust-surface-containment.md`; `.skills/rust-development.md` | Pass |
| 15 | Closure hands publication to release-verification without claiming a release | §11 | Pass |

## 3. Production implementation evidence

- `crates/eggfetch-core/src/error.rs` (+471 lines, additive only):
  `TransportFailureKind` (non-exhaustive, `Connect`/`Tls`/`Protocol`/
  `Cancelled`), `Error::transport_failure_kind()`, private `classify_explicit`
  (TLS family of 7 variants, protocol family of 4, `Connect`/`H3Connect`,
  dialer `Connection` only), private bounded classifier
  (`TransportEvidence` + `classify_transport_failure`: worklist DFS over
  `source()`/`get_ref()` edges, 32-visit bound with cycle guard, `Arc`-aware
  downcasts, `CustomTransport` early return preserving dialer authority,
  `Error::Hyper` body-boundary context, precedence
  TLS > Cancelled > Protocol > Connect), and crate-private
  `hyper_legacy_is_canceled` for the retry delegation.
- `crates/eggfetch-core/src/lib.rs`: one-line re-export of
  `TransportFailureKind`.
- `crates/eggfetch-core/src/retry.rs`: `is_hyper_canceled` delegates to the
  central helper (net −14/+5 lines); retry predicate semantics unchanged.
- `crates/eggfetch-core/tests/native_transport_failure_classification.rs`
  (new, 10 tests): dispatch + body fixtures (§4), retry/proxy non-regression,
  public contract compile check.
- `crates/eggfetch-core/tests/public_api_contracts.rs`: `TransportFailureKind`
  compile contract (runs under all six Tier 1 profile invocations).
- `compat/rust-public-api/*.txt` (6 files, +12 lines each, additive only):
  regenerated with pinned `cargo-public-api 0.52.0` on
  `nightly-2026-05-07`.
- Final public names match the plan's representative shape exactly, so no
  renaming record is needed.

## 4. Verification executed

### Commands run (all on the freeze unless noted)

Qualification was re-run in full on the final freeze
`3fc58fbd99ecb496749b833ee7b27436fe3b412d` after the toolchain-drift fixes
(§10); the feature-level runs below are from the same tree modulo those
test/lint/validation-script-only changes.

- `cargo test -p eggfetch-core --all-features transport_failure -- --test-threads=1` — 13 passed (9 lib unit + public-contract + retry-integration + 2 new integration matches).
- `cargo test -p eggfetch-core --all-features --test native_transport_failure_classification -- --test-threads=1` — 10/10.
- `cargo test -p eggfetch-core --no-default-features --features native-http1,tls-rustls --test native_transport_failure_classification -- --test-threads=1` — 10/10.
- `cargo check -p eggfetch-core --no-default-features --features native-http1,tls-rustls` — 0 errors (profile-specific dead-code warnings only, pre-existing pattern).
- `cargo check -p eggfetch-core --no-default-features --features standard-http1,tls-rustls` — 0 errors.
- `./scripts/check.sh` (Tier 1) on the freeze — green (`All routine checks passed`; Node JS surface policy skip).
- `./scripts/check.sh extended` (Tier 2) on the freeze — green (`Extended validation passed`; MSRV 1.89.0 matrix, six-profile API oracle + semver, feature matrix, full compat, FFI, lifecycle, soak, bench; Node + downstream-manifest policy skips).
- `EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers` on the freeze — 1934 passed, 0 failed.
- `./scripts/check_security.sh` — `advisories ok, bans ok, licenses ok, sources ok`.
- `cargo tree -p eggfetch-core`, `-e features`, `-d` — no manifest delta (none exists to delta).

### Fixture results

| Fixture | Observed error | Classification |
|---|---|---|
| Loopback refused connection (listener dropped) | `HyperClient` (`is_connect`) | `Connect` |
| Self-signed loopback TLS origin, default trust (verification `UnknownIssuer`) | `HyperClient` wrapping `io::Error(Other)` → `get_ref` → `io::Error(InvalidData)` → `get_ref` → `rustls::Error::InvalidCertificate` | `Tls` (outranks `Connect`) |
| Stale-idle pooled reuse, `retry_canceled_requests(false)` (no sleep; `request_count == 1`) | `HyperClient` wrapping canceled `hyper::Error` | `Cancelled` (6/6 consecutive runs + full-suite runs) |
| `NOT-HTTP` response head | `HyperClient` wrapping `hyper::Error::is_parse` | `Protocol` |
| Truncated `Content-Length` body (`ab` of 4, then drop) | `Error::Hyper` (`Kind::Body`, `UnexpectedEof`) | `Protocol` via body-boundary context |
| Malformed chunk (`ZZZ-not-hex`) | `Error::Hyper` (`Kind::Body`, `InvalidInput`) | `Protocol` via body-boundary context |
| Exact `Content-Length` body | success, `ok` | n/a (EOF succeeds) |
| Stalled body, 50 ms read budget | `timeout_read` | `None` (timeout authority preserved) |
| Truncated body with `max_in_flight_requests(1)` | `Protocol`, lease released, second dispatch `ok` | PoolGuard release preserved |
| Secret-bearing `DialError` source | `Connect` (kind `Connection`) | output `Connect` only; `Debug` redacted |

## 5. Invariant review

- `Error` exhaustive enum unchanged; `kind()`/`Display` unchanged (unit-pinned).
- `TransportFailureKind` non-exhaustive from birth.
- `NativeResponseBody`/`ResponseBody` shapes frozen (no edits; public-shape suite green).
- Total-deadline ownership untouched (no pipeline/pool/stream edits).
- Timeout/admission/`TransportIoTimeout`/`DialErrorKind`/`NetworkFailureKind` authorities preserved (unit + existing suites).
- `unsafe` unchanged (none added); pedantic clippy `-D warnings` clean; `cargo fmt --check` clean; lint-suppression policy clean (one specific `struct_excessive_bools` allow with justification).
- No new public helpers beside the error domain (ADR-0005 containment holds; ADR-0006 exception recorded in `rust-surface-containment.md`).

## 6. Failure, timeout, pool, and recovery review

- Retry predicate unchanged: `Connect`/`Io`/`Hyper`/connect-pool-proxy timeouts/`H3Connect`/dialer connection+timeout/`REFUSED_STREAM` retryable; `Tls`/`Protocol`/other dialer kinds not. Pinned by existing retry tests plus the new `native_transport_failure_retry_and_proxy_policy_unchanged` regression test. The delegation only bounds the previously unbounded source walk (32 visits); all realistic chains are far shorter.
- Proxy fallback unchanged: `map_send_error_for_proxy`'s private parse walk is RETAINED (not deduplicated) because it maps to the proxy-specific `MalformedProxyResponse`, not the generic `Protocol` category. Consolidating it would change proxy error taxonomy.
- Timeouts win over classification everywhere (timeouts return `None`).
- Pool leases release on body error/drop as before (lease fixture).

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

- Additive Rust API only; Python/CLI/FFI/Node/HTTPX surfaces untouched.
- Six-profile oracle regenerated (+12 lines each, additive); `check_rust_public_api.py` green including the default-profile semver cross-check (`223 checks pass, 30 skip; no semver update required` against planning baseline `03ecba973010e2858bf16a2b5f84d51ce70adae4`).
- No `Cargo.toml`/`Cargo.lock` change; `cargo tree` delta none.
- Full pinned HTTPX 0.28.1 / HTTPX2 2.12.0 suites: 1934 passed, 0 failed on the freeze.
- Stage C rebound from `bb6e07320f61736702e37f335fd1d48c7c3dfbaf` (now historical) to the M007 freeze `3fc58fbd99ecb496749b833ee7b27436fe3b412d` via the ledger entry and both profile renewals in the closure commit.

## 8. Security review

- `./scripts/check_security.sh` green on the freeze.
- Classifier output is enum-only; no URL/query/userinfo, proxy credentials, auth/cookies, TLS certificate text, bodies, OS messages, or nested `Display`/`Debug` strings flow into it (only typed downcasts and `ErrorKind` comparisons; a `grep` for string inspection in the classifier finds none).
- Secret-bearing nested-error unit test proves a credential-carrying dialer source still classifies by kind without surfacing secrets; existing `DialError` `Debug` redaction untouched.
- Fixture servers bind `127.0.0.1` with ephemeral ports/CA; no public sockets, no internet.

## 9. Documentation and operations

- `docs/reference/errors.md`: new `TransportFailureKind` subsection with category semantics, precedence, non-retryability, and authority table.
- `docs/architecture/core-engine.md`: module-map row + classifier subsection (central evidence, `get_ref` descent, retry reuse, proxy retention).
- `docs/architecture/rust-surface-containment.md`: ADR-0006 narrow-exception record.
- `.skills/rust-development.md`: classifier constraints for future work.
- `docs/residual-differences.md`, `docs/reference/compatibility.md`, `docs/reference/compatibility-stage-decision.md`: live-SHA renewal (canonical-record edits only).
- `plans/registry.md`, `plans/subsystems/core-transport-policy-roadmap.md`, `plans/README.md`: status refresh (see §11).

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| Informational | One Criterion micro-benchmark (`request_building/build_post_json_request`, ~750 ns) printed a +20% timing note during a Tier 2 run; the gate passed. | None on the engine; request-building code untouched (classifier runs only on error paths). | None; timing noise on shared CI hardware. Re-observe in the next Tier 2 run. |
| Informational (resolved) | Floating stable toolchain drifted 1.98.1 → 1.99.0 during qualification; 1.99's pedantic clippy adds `assert_is_empty` (and surfaces `mut_mut`, `unnecessary_lazy_evaluations`, and a `fetch_update` deprecation), failing `-D warnings` on pre-existing code proven clean under 1.98.1 and unrelated to M007. | Tier 1 red under current stable until fixed. | Fixed by the two test/lint-only `chore:` commits in §1; the freeze (and this binding) moved to the last of them and all gates re-ran green on it. |
| Informational (resolved) | `scripts/check_rust_public_api.py` pinned `RUSTUP_TOOLCHAIN` for the snapshot half of the oracle only, so the semver half ran on the ambient stable; once stable reached 1.99.0 the oracle failed with `unsupported rustdoc format v61` on code the oracle had already accepted. | The extended gate would fail for a reason unrelated to the change under test, on any future stable that outruns `cargo-semver-checks 0.49.0`'s parser. | Fixed in the third implementation commit: both oracle halves share one pinned-toolchain env (`nightly-2026-05-07` by default, `RUST_API_RUSTUP_TOOLCHAIN` override), matching the invocation already documented in `compat/rust-public-api/README.md`. The oracle is now independent of ambient stable drift. |
| Operational (by design) | Exact-pin downstream consumers cannot adopt the API until a release containing it is published. | Adoption blocked until publication. | Route through release-verification M003 standing intake (maintainer decision); M007 claims no release. |

No high- or critical-severity findings. No stop-condition triggered (no `Error`/body shape change, no Display parsing, no public transport-library types, no new dependency/flag, no policy change, oracle/semver clean, qualification green).

## 11. Roadmap disposition

- `plans/subsystems/core-transport-policy-roadmap.md`: M007 moves to **closed**; M001–M006/M006C1/M006C2 remain closed. The subsystem returns to steady state: future changes require normal corrective/ADR gates.
- `plans/registry.md`: M007 row moves to closed with this closure record; core-transport current milestone cleared; Stage C rebound to the M007 freeze.
- Future-plan unblocking analysis: M007 unblocks no other registered plan. Publication/version selection is explicitly out of scope and hands to the release-verification M003 standing intake (proposed, maintainer-gated). The gated futures (HTTPX 1.0 program, H3 graduation, Node maturation) are unregistered and unaffected. Exact-pin downstream adoption remains operationally blocked until publication, as the plan requires.
- `plans/implementation/core-transport-policy/007-native-transport-failure-classification.md` stays in place as the handoff record (per the M006 convention, closed plans are not rewritten).

## 12. Registry updates

- `plans/registry.md`: subsystem table (core transport → closed M007, no current milestone), implementation-plan table (M007 → closed + closure link), core-transport gate paragraph (M007 closed under ADR-0006; publication via M003 intake), closure-work paragraph + Stage C binding refreshed to the M007 freeze.
- `plans/subsystems/core-transport-policy-roadmap.md`: header status, M007 section status, §11 completion definition, §12 milestone table.
- `plans/httpx-parity-correction-status.md`: new top entry binding Stage C to the M007 freeze.
- `compat/httpx/0.28.1/profile.toml`, `compat/httpx2/2.12.0/profile.toml`: `qualification-sha`, `previous-qualification-sha`, date, and renewal comment.
- `plans/README.md`, `docs/residual-differences.md`, `docs/reference/compatibility.md`, `docs/reference/compatibility-stage-decision.md`: live-SHA renewal.

## 13. Closure statement

M007 is **closed**. The native transport failure classification API is implemented, proven by deterministic fixtures, qualified by Tier 1, Tier 2 (incl. MSRV and API/semver oracles), renewed exact-SHA Stage C, and security-reviewed, with publication explicitly handed to release-verification.
