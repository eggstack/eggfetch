# TLS, Proxy and Protocols Milestone 004 — Closure Status

Status: closed

Source implementation plan:

- `plans/implementation/tls-proxy-protocols/004-caller-owned-h3-datagram-routing.md`

Source subsystem roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-4--caller-owned-h3-datagram-routing`

Source architecture decision:

- `plans/adrs/ADR-0007-caller-owned-h3-datagram-routing.md` (accepted)

Repository baseline reviewed: `c8e51bfd27f83ab9059c431b241ff411ed35d156`

Implementation commits:

- `42a9c96db5b25354843bd83bd458c71757e95651` — feat(h3): add caller-owned fixed-target datagram routing

## 1. Executive finding

M004 is closed. A caller can install a client-scoped `DatagramDialer` and every
HTTP/3 connection generation — explicit `Http3Only` or Alt-Svc discovered — is
then established over one fixed-target `DatagramRoute` the caller owns. Eggfetch
still owns the whole QUIC/TLS/H3 stack: the private bridge builds the QUIC
client configuration, so a provider supplies a datagram carrier and never takes
over authentication. Direct H3 is unchanged when no dialer is configured, and
H3 remains retained-experimental.

Closure is **unqualified**: every M004 requirement is implemented and every Tier 2
gate passes. The full pinned HTTPX compatibility suite (1934 passed, 0 failed)
and the API-manifest comparison for both facades were executed on
`ubuntu-latest` / Python 3.12.14 at run
[`37511735210`](https://github.com/eggstack/eggfetch/actions/runs/37511735210)
(§10). Those two gates were previously recorded as blocked on this `darwin`
workstation; that attribution was partly wrong and is corrected in §10. H3
graduation remains separately gated and is not claimed here.

Two requirements needed an interpretation, both recorded in §10 rather than
silently resolved:

1. "Fail route setup before beginning the handshake" for a carrier that cannot
   carry a 1200-byte QUIC payload. A carrier's capacity is only observable by
   attempting the Initial packet, so the bridge converts a refused first
   transmit into typed route evidence and the generation is refused during
   establishment — before any H3 stream is opened, and before any direct UDP
   socket could exist. `a_route_that_cannot_carry_quic_payloads_fails_establishment`
   pins this, including the control that the same server is healthy over a
   direct socket.
2. "Reported exactly once" for route terminal I/O. A `RouteShared` latch records
   the first failure for the generation, so a later send or receive cannot
   overwrite the cause with a different one.

The API delta is additive and Rust-only. `H3RouteKind` deliberately keeps
describing *discovery* (`Explicit` vs `AltSvc`) rather than physical routing, so
no public enum or struct changed; only the all-features oracle snapshot changed.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Compile-time fixtures precede public API | `tests/public_api_contracts.rs::caller_owned_h3_datagram_route_contracts_compile` | pass | Written before internals; proves object safety and the builder hook |
| Public contract is minimal and documented | `transport/datagram.rs` module docs; two traits + three boxed-future aliases | pass | Reuses `DialError`/`DialErrorKind`; no Quinn type is public |
| Builder hook, Rust-only | `ClientBuilder::datagram_dialer` under `#[cfg(feature = "http3")]` | pass | No Python/FFI/Node exposure; API manifests unchanged |
| Exactly one datagram per `send`/`recv` | `transport::datagram::tests` | pass | `recv` delivers one datagram per call |
| Fixed target per route; no re-resolution | `a_refused_destination_is_refused`, `a_foreign_destination_is_refused` | pass | `try_send` rejects a foreign peer with `InvalidInput` |
| Explicit H3 routes through the caller's route | `explicit_h3_request_completes_over_a_caller_route` | pass | Dialer sees the logical origin target |
| Alt-Svc H3 routes through the same seam | `alt_svc_h3_uses_the_same_caller_route_seam` | pass | Real CA fixture (Alt-Svc requires an authenticated hop) |
| `Http3Only` never falls back to TCP | `http3_only_never_selects_a_tcp_fallback_for_a_route_failure` | pass | Asserts on metrics, not text |
| No direct-UDP fallback when configured | `a_refused_route_never_falls_back_to_a_direct_socket` | pass | Control proves the same URL works directly |
| Connect budget spans route establishment | `a_stalled_route_dial_is_bounded_by_the_connect_budget` | pass | Never-resolving dial is bounded by the connect timeout |
| Undersized carrier fails establishment | `a_route_that_cannot_carry_quic_payloads_fails_establishment` | pass | Connect-class, fast, no downgrade |
| Provider rejection/auth preserves broad kind | `explicit_provider_rejection_preserves_the_broad_kind`, `provider_authentication_failure_preserves_the_broad_kind` | pass | Matched structurally from `CustomTransport`, never parsed |
| Terminal route I/O evicts only its generation | `terminal_route_io_evicts_only_the_current_generation` | pass | Per-origin routes prove isolation both directions |
| Bounded queues back-pressure, never drop | `a_slow_carrier_backpressures_instead_of_dropping_datagrams` | pass | 512 KiB body arrives byte-for-byte |
| Bridge `Debug` never renders the provider | `socket_debug_never_renders_the_route_implementation`, `route_debug_output_never_escapes_into_errors` | pass | Provider marker and type name both absent |
| Route/worker release on drop | `the_route_is_released_with_the_socket`, `dropping_the_client_releases_every_route` | pass | No `JoinHandle` cycle; both workers aborted |
| Cancellation-safe provider contract | Documented on `DatagramRoute`; drop aborts workers | pass | No partial datagram is reported on truncation |
| Routed diagnostics separate path from origin | `alt_svc_h3_uses_the_same_caller_route_seam` | pass | `remote_address` is the route peer; SNI stays origin |
| Direct-H3 non-regression | `h3_integration` 9, `h3_alt_svc_discovery` 17, `h3_hardening` 12, `h3_interop_qualification` 20 | pass | Whole workspace suite green |
| Alt-Svc suppression still applies | `repeated_route_failures_suppress_the_broken_alt_svc_route` | pass | Connect-class ⇒ suppressible |
| Rust public API oracle update | Six-profile `check_rust_public_api.py` + semver | pass | Only `all-features` snapshot changed, additively |
| Tier 1 green | `./scripts/check.sh` | pass | 581 Python + full Rust workspace |
| Tier 2 — Rust public API oracle + semver | `check_rust_public_api.py` (pinned `nightly-2026-05-07`, `cargo-semver-checks` 0.49.0) | pass | 6 profiles; 223 semver checks pass, 30 skip; "no semver update required" |
| Tier 2 — feature matrix | 5 `cargo check` invocations | pass | Bare, `http1`, `+tls-rustls`, `+tls-native-roots`, `--all-features` |
| Tier 2 — MSRV | `rustup run 1.89.0 cargo check` × 5 | pass | Exact 1.89.0 toolchain; includes `--workspace --all-targets --all-features` |
| Tier 2 — docs/doctests | `cargo doc`, `cargo test --doc`, doc example + link checks | pass | |
| Tier 2 — FFI | `cargo test -p eggfetch-ffi --all-features` | pass | 38 tests |
| Tier 2 — resource monitor | `resource_monitor` release binary | pass | |
| Tier 2 — lifecycle / soak / merge-lossless | pytest suites | pass | 49 / 11 / 12 |
| Tier 2 — full pinned HTTPX compat suite | `EGGFETCH_COMPAT_REQUIRED=1 pytest tests/compat` | **PASS** | 1934 passed / 0 failed on `ubuntu-latest`, Python 3.12.14, run `37511735210`; see §10 |
| Tier 2 — API manifest comparison | `generate`/`compare_httpx_api_manifest.py` | **PASS** | Both facades, 0 unexplained / 0 stale, same run; see §10 |
| Tier 3 green | `./scripts/check.sh package` | pass | Crate packaging + wheel build/smoke |
| Live security preflight | `./scripts/check_security.sh` | pass | `advisories ok, bans ok, licenses ok, sources ok` |
| Exact-SHA Stage C renewal | `plans/httpx-parity-correction-status.md` + both `compat/*/profile.toml` | pass | Rebound to `42a9c96d…` |
| H3 remains experimental | `docs/architecture/core-tls-proxy-protocols.md` | pass | No graduation claim anywhere |
| No Quinn/Eggress dependency added | `Cargo.toml`/`Cargo.lock` unchanged | pass | Quinn was already in the `http3` line |

## 3. Production implementation evidence

`crates/eggfetch-core/src/transport/datagram.rs` (new) holds the whole public
contract and the crate-private bridge in one place, mirroring how
`transport/dialer.rs` already pairs a public seam with its private adapter.

Public surface:

- `DatagramDialer::connect(DialTarget) -> Result<Arc<dyn DatagramRoute>, DialError>`
- `DatagramRoute` — `send`, `recv`, `local_addr`, `peer_addr`
- `DatagramDialFuture`, `DatagramSendFuture`, `DatagramRecvFuture`
- blanket `impl DatagramDialer for Arc<T>`
- `ClientBuilder::datagram_dialer` (`http3` only)

Private bridge (`RouteUdpSocket` implements `quinn::AsyncUdpSocket`):

- Both datagram queues are bounded `mpsc` channels (depth 64). `try_send`/
  `try_recv` are the synchronous halves Quinn needs; `recv`/`reserve` are the
  awaiting halves the two workers need, so queue depth bounds memory exactly
  with no custom queue.
- The receive worker reserves capacity *before* calling the route, so a
  saturated receive pauses the route instead of dropping packets.
- `EndpointConfig::max_udp_payload_size` is pinned to `ROUTE_MTU` (1200) and
  `may_fragment()` returns `true`, which clears Quinn's `allow_mtud`; the stack
  never attempts PMTUD over a carrier whose real MTU cannot be measured.
- `poll_recv` wakes only for queued data or a terminal failure. It never polls
  the route's future, so a provider cannot observe or stall the QUIC poller.
- Receive metadata reports the fixed peer with no ECN and no destination IP,
  because neither is observable through this contract.
- Terminal route I/O is reported to Quinn *and* closes the QUIC connection with
  transport error `0x100`, so a request never parks until the idle timeout.
- The socket owns the workers' `JoinHandle`s, and the workers hold only
  `Arc<RouteShared>` — never the socket — so no handle cycle can retain a
  generation. `Drop` aborts both.
- Poller ownership uses `Weak`, so a lingering poller reports a closed
  transport instead of keeping a route alive.
- Provider evidence is rebuilt from the retained `DialErrorKind` with fixed
  text; the caller's message and `Debug` never reach Quinn or an error.

`crates/eggfetch-core/src/transport/http3.rs`:

- `H3Connector` holds `endpoint: Option<quinn::Endpoint>` plus
  `datagram_dialer`. `with_datagram_route` creates **no** direct endpoint when
  a dialer is configured — that is what makes "no direct-UDP fallback"
  structural rather than a policy check.
- `CachedH3Sender` carries `RouteOwnedChain` (bridge + route-owned endpoint +
  terminal watcher) so the whole chain is created once and released as one
  unit on eviction, terminal close, or drop.
- The h3 driver task selects on the route's terminal watcher and closes the
  QUIC connection, then falls into the ordinary close path so drain state and
  metrics stay in one place.
- `route_error_override` is the single place that replaces a QUIC symptom with
  caller evidence, applied at every dispatch failure site and in the response
  body stream.

Tests: `crates/eggfetch-core/tests/h3_datagram_route.rs` (15 loopback tests),
crate-private `transport::datagram` unit tests (8), and one compile contract.

## 4. Verification executed

### Commands run

All commands ran in the repository's pinned venv with
`RUSTUP_TOOLCHAIN=stable-x86_64-apple-darwin` for the ambient toolchain.

```sh
./scripts/check.sh                  # Tier 1
./scripts/check.sh extended         # Tier 2
./scripts/check.sh package          # Tier 3
./scripts/check_security.sh         # live RustSec/license/source preflight
cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1
RUSTUP_TOOLCHAIN=nightly-2026-05-07 python scripts/check_rust_public_api.py
```

### Results (pass/fail/skip with reason; counts without concealment)

**Tier 1 — `./scripts/check.sh`: PASS.**
Clippy `-D warnings` clean across all targets; `cargo fmt --check` clean;
lint-suppression policy clean; adapter feature ownership clean; release
version/ref clean. Full Rust workspace green, including the new
`h3_datagram_route` suite (15/15) and every direct-H3 control
(`h3_integration` 9, `h3_alt_svc_discovery` 17, `h3_hardening` 12,
`h3_interop_qualification` 20, plus `h3_hardening`/`h3_interop` unit tests).
Six-profile stable contract fixtures pass. Python extension built and
installed; native API manifest, typing surface, and typing fixture all pass;
581 Python behavior tests pass; HTTPX compatibility smoke kernel passes;
Node binding 10/10.

**Tier 2 — `./scripts/check.sh extended`: PARTIAL.** Every gate runnable on
this host passes:

| Tier 2 gate | Result |
|---|---|
| Rust public API oracle (6 profiles, pinned `nightly-2026-05-07`) | pass |
| `cargo-semver-checks` 0.49.0 vs baseline `03ecba97…` | pass — "no semver update required" |
| Feature matrix (5 invocations) | pass |
| Rust 1.89.0 MSRV (5 invocations, incl. `--workspace --all-targets --all-features`) | pass |
| Docs: `cargo doc`, `cargo test --doc`, doc examples, doc links | pass |
| FFI tests | pass (38) |
| Resource regression monitor | pass |
| Lifecycle (timeout/proxy-TLS/shutdown) | pass (49) |
| Soak | pass (11) |
| Lossless merge | pass (12) |
| Feature-gated tests (gzip/brotli/zstd/deflate/proxy) | pass |
| Full pinned HTTPX 0.28.1 / HTTPX2 2.12.0 suite | pass — 1934 passed, 0 failed, `--strict-markers`, `EGGFETCH_COMPAT_REQUIRED=1` (see below) |
| API manifest comparison (both facades) | pass — 0 unexplained, 0 stale (see below) |

The two gates that first made this closure conditional have since been
executed green and were misattributed when they were recorded. The
correction matters more than the gate results, because the earlier
record sent the next maintainer after a bug that does not exist.

1. **`codes (is_integer): ref=present cand=absent` was not a Python
   3.11-versus-3.12 difference.** `int.is_integer` has existed since
   Python 2.6 and is present in every stock CPython. The local `.venv`
   was built on a **damaged** interpreter
   (`/Library/Frameworks/Python.framework/Versions/3.11`, python.org
   3.11.9) that lacks exactly that one public `int` method — the other
   six (`as_integer_ratio`, `bit_count`, `bit_length`, `conjugate`,
   `from_bytes`, `to_bytes`) are all present — and whose
   `etc/openssl/` directory is **empty**. CI's `ubuntu-latest`
   interpreter is 3.12.14 with `int.is_integer = True` and 121 default
   CA certificates. The earlier note also contradicted itself by
   claiming the reference snapshot was generated on an interpreter
   where the symbol is absent while the comparator reported
   `ref=present`.
2. **`test_ca_count_heuristic_removed` was the same damaged
   interpreter, not macOS trust-store semantics.** With a default CA
   count of 0 the test generated 0 CAs, so the "custom" store was
   empty and classified as `verify=True` — correct behavior for an
   empty store. Against an interpreter with a real CA store (121 on CI,
   136 on this host's healthy 3.14.2) the test passes unmodified.
3. **`test_resource_assertions.py` is a genuine pre-existing repo
   defect and was reported accurately.** `compat/*/resource-thresholds.toml`
   declares `[platform.linux]`, `[platform.macos]` and
   `[platform.windows]`, while the test keys on
   `platform.system().lower()`, which returns `darwin` on macOS. No
   macOS host can pass this test at any commit; Linux passes only
   because `[platform.linux]` happens to match. Left unfixed here —
   see the residual-risk table.
4. **A third defect surfaced only once the suite could run to
   completion.** `test_helper_context_then_min_version_detected`
   assigned `minimum_version = ssl.TLSVersion.TLSv1_2` to a helper that
   already floors at TLSv1_2, so the context was byte-for-byte
   unchanged (`771` → `771`). A value-based construction fingerprint
   cannot observe a no-op write, so no implementation could satisfy the
   assertion; it failed on every host and interpreter tried. Fixed in
   `e675b2e7` by mutating an observable value and adding a positive
   control. The sibling `load_verify_locations` test, which performs a
   genuinely observable mutation, is unchanged.

**Evidence.** `ubuntu-latest`, Python 3.12.14, Actions run
[`37511735210`](https://github.com/eggstack/eggfetch/actions/runs/37511735210)
at `e675b2e7`: compat suite **1934 passed, 0 failed** (266 s), manifest
generate/compare clean for both facades (74 and 76 symbols), no
unexplained and no stale allowed-difference entries.

**Exact-SHA note.** The gates ran at `e675b2e7`, not at the frozen
`42a9c96d`. `git diff 42a9c96d e675b2e7` touches no Rust source,
manifest or lockfile — only this closure record, the Stage C ledger and
docs, the two `compat/*/profile.toml` notes, the temporary evidence
workflow (since removed), and the one Python test described above. The
shipped artifact under qualification is identical to the freeze.

M004 changed **no** Python, FFI, Node, CLI, or `compat/` file at the
freeze — the entire diff outside `crates/eggfetch-core` is one additive
line block in `compat/rust-public-api/all-features.txt`. None of the
defects above are reachable from this change.

**Tier 3 — `./scripts/check.sh package`: PASS.** Crate
`cargo publish --dry-run` packaging for the published crates, wheel build
(`eggfetch-0.2.2-cp311-cp311-macosx_10_12_x86_64.whl`), wheel smoke tests,
package-content validation, and installed-wheel typing smoke (positive and
negative fixtures) all pass.

**Live security preflight — `./scripts/check_security.sh`: PASS.**
`advisories ok, bans ok, licenses ok, sources ok`.

**Skips** are the policy-defined optional ones only: the Node JS surface (native
`eggfetch.node` artifact absent) and downstream behavioral fixtures (artifact
manifest absent). Both are explicit skips, not failures.

### Condition on this closure

**Satisfied.** The two Tier 2 gates were executed on `ubuntu-latest`
(or CI's single `ubuntu-latest` job) against the same executable freeze:
Actions run
[`37511735210`](https://github.com/eggstack/eggfetch/actions/runs/37511735210)
at `e675b2e7`, whose Rust tree is identical to the frozen `42a9c96d`:

```sh
EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -v --strict-markers
python scripts/generate_httpx_api_manifest.py --package eggfetch.compat.httpx  --output "$tmp/a.json"
python scripts/compare_httpx_api_manifest.py --reference compat/httpx/0.28.1/reference-api.json  --candidate "$tmp/a.json" --allowed compat/httpx/0.28.1/allowed-differences.toml
python scripts/generate_httpx_api_manifest.py --package eggfetch.compat.httpx2 --output "$tmp/b.json"
python scripts/compare_httpx_api_manifest.py --reference compat/httpx2/2.12.0/reference-api.json --candidate "$tmp/b.json" --allowed compat/httpx2/2.12.0/allowed-differences.toml
```

Result: **1934 passed, 0 failed** in 266 s; both facades 0 unexplained and
0 stale. A throwaway, dispatch-only workflow carried this evidence and was
deleted afterwards, so no CI job was added to the repository.

### Environment note (recorded, not hidden)

Two local accommodations were required and are recorded rather than suppressed:

1. **Architecture.** This machine's shell and Python venv are `x86_64` while
   rustup's default host is `aarch64-apple-darwin`. `maturin develop` under the
   default toolchain produced an `arm64` wheel that the `x86_64` interpreter
   rejected (`is not a supported wheel on this platform`), failing Tier 1 at the
   release version/ref step. All gates were therefore run with
   `RUSTUP_TOOLCHAIN=stable-x86_64-apple-darwin`. Only the build architecture
   differs: the MSRV gate still resolved `rustup run 1.89.0` explicitly, and
   `rustup run` ignores `RUSTUP_TOOLCHAIN`, so the exact required 1.89.0
   toolchain ran; the API oracle ran the pinned `nightly-2026-05-07`. CI runs one
   `ubuntu-latest` job and is unaffected.
2. **Intermittent pre-existing flakes.** Two tests fail intermittently on this
   host, both measured as pre-existing rather than regressions:
   - `crates/eggfetch-bench/src/bench_proxy.rs::forwards_complete_origin_form_requests_and_repeated_gets`
     (~25% of runs). Measured with an interleaved A/B of 16 runs per arm on the
     true pre-M004 baseline commit `c8e51bfd` in a separate git worktree, same
     toolchain and build conditions: **5/16 failures on the baseline vs 4/16 on
     M004** — statistically indistinguishable. Three candidate fixture
     hardenings were trialled (bound the origin's per-connection read and stop
     abandoning the run on one short read; drain and forward the upstream
     response before giving up; drain the proxy's accept backlog with a blocking
     accept after a bounded spin) and all three were **reverted** because none
     changed the failure rate; shipping an unrelated half-fix inside an M004
     commit would have been wrong.
   - `crates/eggfetch-core/tests/direct_transport_tests.rs::test_pool_isolation_uds_vs_tcp`
     (also intermittent under load). It concerns UDS/TCP pool isolation and
     shares no code path with HTTP/3 datagram routing. It failed 6/6 on the
     baseline under concurrent load and passed 6/6 on this branch when run
     without competing builds, which is the signature of load sensitivity
     rather than a behavioral difference.

   Neither has any product or fixture code changed for it. Both are routed to
   the standing corrective intake alongside the existing low-severity
   compatibility-fixture finding.

## 5. Invariant review

| Invariant | How it is held |
|---|---|
| Eggfetch owns QUIC/TLS/H3 | The bridge builds the QUIC client configuration; the provider supplies datagrams only |
| Direct H3 unchanged when unset | `datagram_dialer` is `None`; the connector builds the same shared direct endpoint as before |
| No direct-UDP fallback | No direct endpoint is constructed when a dialer is set |
| SNI/cert bound to the logical origin | The dialer receives the logical `DialTarget`; `connect_with` uses `sni_host` |
| Bounded memory per generation | Both queues hard-bounded at 64 datagrams |
| No silent packet loss | Receive back-pressures; saturated sends surface `WouldBlock` |
| No worker/task leak | `Weak` poller ownership; drop aborts both workers; no handle cycle |
| Bounded request lifetime | Connect budget wraps dial + handshake; terminal route I/O closes the connection |
| No parsed-string classification | Only `DialErrorKind` variants; asserted structurally in tests |
| No secrets in diagnostics | Provider `Debug` never rendered; fixed I/O text; message rebuilt from kind |

## 6. Failure, timeout, pool, and recovery review

- Route establishment: bounded by the remaining connect budget; typed
  `DialError` mapped to connect-class / timeout / preserved-evidence.
- Handshake: bounded by the same budget; a refused first transmit becomes route
  evidence rather than a generic QUIC error.
- Terminal send/receive I/O: connection closed with transport error, generation
  evicted, next attempt dials a fresh route. Unrelated origins keep their
  cached generation.
- Read/write/total phases: unchanged. The plan's "no native total synthesis"
  rule is untouched; this milestone adds no timeout.
- Pool interaction: none. The seam is below QUIC and does not touch `Pool`
  permits or `PoolMetrics`.
- Cancellation: dropping a request future drops the `recv` future; the
  documented provider cancellation contract applies. Dropping a generation
  aborts workers under that contract.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

- Public API: additive only. `cargo-public-api` diff shows exactly one builder
  method, two traits, three type aliases, and one blanket impl; nothing removed,
  renamed, or narrowed. The other five profiles are byte-identical.
- `cargo-semver-checks` (0.49.0) reports no semver update required against
  planning baseline `03ecba973010e2858bf16a2b5f84d51ce70adae4`.
- Profiles: `http3` implies `advanced-routing` through `http1`, so the seam is
  available exactly where intended. Lean `standard-http1`/`standard-http2`
  profiles compile the new module out entirely; verified by the six-profile
  contract fixture matrix.
- HTTPX/httpx2 facades: no manifest, typing, or parity change. The change is
  Rust-only by construction and the adapter feature-ownership gate confirms no
  Python/FFI/Node edge was added.
- Exact-SHA: Stage C renewed from `015a56d7ec3edf186eec8ebccff01cbf5584274e`
  (the published 0.2.2 candidate) to `42a9c96db5b25354843bd83bd458c71757e95651`
  in `plans/httpx-parity-correction-status.md` and both `compat/*/profile.toml`.
  The prior binding is retained as `previous-qualification-sha`. No new residual
  difference; the Rust-only addition is documented in
  `docs/residual-differences.md` as native-only.

## 8. Security review

Threat-model entry added: `docs/architecture/threat-model.md` § "Malicious
Datagram Route". Controls are single fixed peer (foreign destination refused),
provider-unreachable crypto (Eggfetch builds the QUIC config), no silent
corruption (back-pressured receives), no silent downgrade (no direct endpoint),
no diagnostic leakage (never renders the provider; fixed I/O text), and bounded
termination (transport-error close). A buggy or compromised provider cannot
downgrade the QUIC connection's authentication or identity.

Redaction tests assert both that a provider `Debug` marker and the provider
type name are absent from rendered errors.

Live security preflight green: advisories, bans, licenses, and sources all ok.
No new dependency, license, or source change: Quinn was already in the `http3`
line and no Eggress dependency was added.

## 9. Documentation and operations

- `docs/architecture/core-tls-proxy-protocols.md` — new § "Caller-owned
  datagram routing (experimental)" with the contract, the bridge-invariant
  table, error mapping, explicit non-scope, diagnostics truthfulness, and
  evidence pointers; H3 feature-gating and stress-evidence sections updated.
- `docs/architecture/overview.md` — `transport` module map and the H3 dispatch
  arm now name the seam.
- `docs/architecture/feature-flags.md` — `http3` section records the seam,
  the no-direct-socket rule, and Rust-only scope.
- `docs/architecture/rust-surface-containment.md` — records the one authorized
  addition beside `dialer` and forbids adjacent public helpers.
- `docs/architecture/threat-model.md` — new attacker-capability entry.
- `docs/residual-differences.md` — new native-only section; no parity claim.
- Public API rustdoc on every new item, including the cancellation contract and
  the `Debug` prohibition for providers.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| Low | "Fail route setup before beginning the handshake" cannot be literal: a carrier's capacity is only observable by attempting the ≥1200-byte Initial. Implemented as refusal during establishment with typed evidence, never a downgrade. | Slight semantic narrowing versus the plan's wording | Accepted interpretation; pinned by `a_route_that_cannot_carry_quic_payloads_fails_establishment`. Recorded here rather than silently resolved. |
| Low | The 1200-byte ceiling is a conservative hard-coded constant, not derived from a carrier's real MTU. | Routed H3 does not use path MTU discovery | Deliberate: PMTUD is disabled on this seam because Eggfetch cannot measure a carrier's real MTU. Documented; revisit with the CONNECT-UDP carrier milestone. |
| Low | Local verification ran with the `x86_64` stable toolchain because this host's shell/venv is `x86_64` while rustup's default host is `aarch64`. | Architecture of the local build differs from a native `aarch64` run | Recorded in §4. No code or version consequence; MSRV and the API oracle used their exact pinned toolchains. |
| Low | The bridge copies each outgoing datagram once (`Bytes::copy_from_slice`). | One bounded copy per datagram on the experimental seam | Accepted and documented as the one allocation the seam allows; queue depth bounds it to a constant per generation. |
| Low | `crates/eggfetch-bench/src/bench_proxy.rs::forwards_complete_origin_form_requests_and_repeated_gets` is intermittently flaky here (~25% of runs). Measured pre-existing: interleaved A/B on the true baseline `c8e51bfd` in a separate worktree failed 5/16 (baseline) vs 4/16 (M004). Three fixture hardenings were trialled (bound the origin's read, drain-and-forward the
upstream response, drain the proxy's accept backlog) and all were reverted because none changed
the rate. | Tier 2 can fail on an unrelated pre-existing harness race; no M004 behavior is implicated | Recorded in §4 and in the Stage C ledger; routed to the standing corrective intake. Tier 2 was run to a clean pass. |
| **Resolved** | Tier 2's full pinned HTTPX compatibility suite and the API-manifest comparison could not be executed on the `darwin` workstation. Recorded cause was partly wrong: `codes.is_integer` was attributed to a Python 3.11-vs-3.12 change and the CA-count test to "macOS trust-store semantics", when both were caused by a damaged local interpreter, and a third defect (an unsatisfiable no-op mutation test) had gone unnoticed because the suite never ran to completion. | None remaining — both gates are green | Executed on `ubuntu-latest` / Python 3.12.14, run [`37511735210`](https://github.com/eggstack/eggfetch/actions/runs/37511735210) at `e675b2e7`: 1934 passed / 0 failed, both manifests clean. Causes corrected in §10 so the next maintainer does not chase a non-existent 3.12 regression. |
| **Low** | `compat/*/resource-thresholds.toml` declares `[platform.macos]` but `test_resource_assertions.py` keys on `platform.system()` (`darwin`), so the full compat suite still cannot pass on **any** macOS host at any commit. Linux passes only because `[platform.linux]` matches. | Tier 2's full compat suite remains macOS-unrunnable; the gate above was satisfied on Linux instead | Pre-existing defect, deliberately not fixed here: it is unrelated to M004 and would be a repo-wide test-infra change. Routed to the standing corrective intake. Recorded in §10. |
| Low | `direct_transport_tests.rs::test_pool_isolation_uds_vs_tcp` is intermittently load-sensitive (failed 6/6 on baseline under concurrent load, passed 6/6 on this branch when run alone). | UDS/TCP pool isolation evidence is intermittently noisy on this host | Routed to the standing corrective intake. No M004 code path is involved. |
| Info | `h3_route_attempted` describes Alt-Svc discovery, not explicit `Http3Only`. | The strict-H3 test asserts on `h3_fallback_selected` plus the dial count instead | Not a defect; the counter's existing semantics are unchanged. |

## 11. Roadmap disposition

`plans/subsystems/tls-proxy-protocols-roadmap.md` Milestone 4 moves
**ready → closed**. Every M004 exit condition is met and evidenced:
bounded/cancel-safe bridge, no direct-UDP fallback when the route is configured,
direct-H3 non-regression, additive API with the oracle snapshot updated, Tier 1
green, Tier 3 green, live security preflight green, all runnable Tier 2 gates
green, the full pinned HTTPX compatibility suite and the API-manifest comparison
green on `ubuntu-latest` / Python 3.12.14 (run
[`37511735210`](https://github.com/eggstack/eggfetch/actions/runs/37511735210)),
and a renewed exact-SHA Stage C binding.

The two gates that made this closure conditional are now executed, and their
recorded causes were partly wrong (§10): two were a damaged local interpreter
rather than a Python version difference, and a third defect had gone unnoticed
precisely because the suite never ran to completion. The one defect left standing
— the `[platform.macos]` / `darwin` key mismatch that makes the full suite
macOS-unrunnable — is unrelated to M004 and is recorded in §10.

Deferred exactly as the roadmap specified: per-request route override,
MASQUE/CONNECT-UDP carrier, QUIC migration, 0-RTT, H3 application datagrams, and
H3 graduation. No future milestone is unblocked by this closure:

- **H3 graduation** stays gated on independent interop, GOAWAY/drain,
  public-origin, impairment, and upstream-risk evidence. A caller-owned route is
  an embedder feature, not graduation evidence, and this closure makes no
  graduation claim.
- **Concrete carriers** (MASQUE CONNECT-UDP datagram stream, inter-process
  channel) remain the natural next milestone for this subsystem. They are
  unblocked in the sense that the contract they must implement now exists and is
  proven end to end, but no such milestone is registered and none is implied.

The subsystem may therefore return to closed/steady state with H3 still
experimental.

## 12. Registry updates

- `plans/registry.md`: M004 marked **closed** with its closure record; TLS/proxy
  and protocols roadmap status moved to closed; the TLS/protocol execution gate
  cleared; the M004 gate note replaced with the closure outcome.
- `plans/subsystems/tls-proxy-protocols-roadmap.md`: Milestone 4 closed, §3
  non-goals unchanged (M004 still permits only fixed-target transport datagrams
  and no QUIC DATAGRAM API), §11 completion definition updated, §12 status table
  updated, and §4 current state updated.
- `plans/README.md`: execution gate line replaced with the closed outcome;
  H3 graduation remains explicitly separate and still gated.
- `plans/httpx-parity-correction-status.md` + `compat/httpx/0.28.1/profile.toml`
  + `compat/httpx2/2.12.0/profile.toml`: Stage C rebound to
  `42a9c96db5b25354843bd83bd458c71757e95651`.