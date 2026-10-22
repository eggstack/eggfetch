# TLS / Proxy / Protocols Milestone 005 — Uncompiled-protocol Fail-Closed at Adapter Boundaries

Status: closed

Source implementation plan:

- `plans/implementation/tls-proxy-protocols/005-uncompiled-protocol-fail-closed.md`

Source subsystem roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-5--uncompiled-protocol-fail-closed-at-adapter-boundaries`

Governing decision:

- `plans/adrs/ADR-0008-uncompiled-protocol-request-semantics.md` (accepted
  2026-10-22)

Repository baseline reviewed: `873ea28541c82817317430c9523fca8525d99c81`.

Implementation commits:

- `d6c41adf4cf8f62ef990cccedf2f86512858fa0b` — feat: implement the eight
  registered milestones as one qualification freeze. This commit carries the
  milestone's implementation together with the other seven milestones
  executed in the same coordinated pass.
- `26fa619299fb8d447f25cbcb5eb713a1f537a30c` — fix(security): patch the four
  dependencies with live RustSec advisories. The bound Stage C SHA; it
  changes only `Cargo.lock` and `fuzz/Cargo.lock` relative to the freeze
  commit above, and no Rust source.

## 1. Executive finding

M005 is implemented. Three defects are closed:

1. The CLI's `--http2` no longer silently downgrades to HTTP/1.1 on a build
   without the capability; it fails closed with an error that names HTTP/2.
2. The CLI's `--http3` no longer reports "HTTP client not available for this
   protocol", which misreported a build-capability gap as a transport
   failure; it fails closed with an error that names HTTP/3.
3. The FFI's `eggfetch_client_builder_http_version` now validates a strict
   policy against the FFI crate's compiled `http2` / `http3` features, and a
   strict policy the build cannot honor is rejected before any connection is
   attempted.

The engine's documented downgrade for **native Rust callers** is unchanged, as
ADR-0008 rule 1 requires. The only engine change is an error-text and
classification change: `Http3Only` on a build without `http3` now produces an
error that names the missing HTTP/3 capability instead of the generic
missing-client message. No new error variant, no `TransportFailureKind`
change, no new public engine surface, no retry, redirect, proxy, pool,
deadline, or TLS-trust change.

The milestone is **closed**. It changes Rust sources, tests, and crate
manifests, which are executable inputs under the exact-SHA rule in
`.skills/verification-qualification.md`; that change invalidated the previous
live Stage C binding, and the binding has since been re-established at the
bound SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c` — the implementation
freeze `d6c41adf4cf8f62ef990cccedf2f86512858fa0b` plus the lockfile-only
dependency patch. The frozen-SHA tier gates were run at that SHA: Tier 1 is
green, Tier 2 is green across every phase (2073 pinned HTTPX compatibility
tests passed, 0 failed; the Rust public-API oracle passes in 8 profiles), and
Tier 3 is green. The live security preflight reports `advisories ok, bans ok,
licenses ok, sources ok`. §10 records the evidence.

Two plan criteria are recorded as not fully met, with reasons, in §10: the FFI
has no strict HTTP/3 policy value to reject, and the CLI's HTTP/2 negotiation
success case could not be exercised without a new dev-dependency. Those two
findings are unrelated to the binding and remain open by design.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| CLI `--http2` fails closed on a build without `transport-http2` (§6.2) | `crates/eggfetch-cli/tests/protocol_capability.rs`; manual run → exit 5, message names HTTP/2 | Met | No connection is opened |
| CLI `--http3` fails closed on a build without `http3` (§6.2) | same test module; manual run → exit 5, message names HTTP/3 and does **not** contain "HTTP client not available" | Met | No connection is opened |
| CLI reads its capability from feature forwarding, not a hand-maintained list (§6.1) | `crates/eggfetch-cli/src/protocol.rs` uses `cfg!(feature = "http2")` / `cfg!(feature = "http3")`; `Cargo.toml` forwards to `eggfetch-core/http2` / `eggfetch-core/http3` | Met | — |
| CLI default capability set unchanged (§6.1) | `crates/eggfetch-cli/Cargo.toml` `default = []`; test asserts the resolved default feature set | Met | `cookies`, `multipart`, `proxy` unchanged |
| CLI exit code stays inside the documented set (§6.2) | `Error::Unsupported` → exit **5**, the existing mapping in `crates/eggfetch-cli/src/main.rs` | Met | No new exit code |
| FFI validates a strict policy against its compiled features (§6.3) | `crates/eggfetch-ffi/src/protocol.rs`; `builder.rs` records the gap; `client.rs` and `streaming.rs` reject before dispatch | Met | Rejection rides the existing `err_out` channel |
| FFI default policy unchanged (§6.3) | `eggfetch_client_new()` builds with `missing_capability: None`; default `Auto` is not strict | Met | — |
| Engine distinguishes "no HTTP/3 support" from "no usable client" (§6.4) | `crates/eggfetch-core/src/http_version.rs` `missing_client_error(policy)`; both dispatch paths call it | Met | Typed evidence (policy variant + compile-time capability); no `Display` parsing |
| Engine `Auto` still downgrades to HTTP/1.1 (§6.4) | `crates/eggfetch-core/tests/http_version_capability_tests.rs` — `Auto` succeeds over HTTP/1.1 | Met | — |
| Native Rust callers' downgrade contract unchanged (ADR-0008 rule 1) | `http_version.rs` `from_policy` untouched; `Http2Only` without `transport-http2` still downgrades | Met | — |
| No new public engine surface | `public_api_contracts` 7 passed; new helpers are `pub(crate)` | Met | — |
| Guards consume no deadline, pool permit, or connection | CLI guard runs before the client is built; FFI guard runs before dispatch | Met | — |
| FFI strict-HTTP/3 rejection at the ABI boundary (§10) | The published FFI policy values contain **no** strict HTTP/3 value | **Not met** | See §10; requires a public ABI extension outside the plan's scope |
| CLI `--http2` negotiates HTTP/2 against a suitable server (§10) | Guard does not reject and a connection is attempted when the feature is enabled | **Partially met** | See §10; a real H2 server fixture needs a new dev-dependency |
| Tier 1 and Tier 2 green on the frozen executable SHA (§13) | `./scripts/check.sh` and `./scripts/check.sh extended` on `26fa619299fb8d447f25cbcb5eb713a1f537a30c` | Met | Both green; §10 |
| Stage C rebinding to the frozen SHA (§13) | The ledger's Stage C SHA and both HTTPX profiles rebound to `26fa619299fb8d447f25cbcb5eb713a1f537a30c` | Met | §7 and §10 |

## 3. Production implementation evidence

### Engine (`eggfetch-core`)

`crates/eggfetch-core/src/http_version.rs` gains a single classification
point:

- `pub(crate) enum Http3Capability { Compiled, NotCompiled }` with
  `Http3Capability::CURRENT` derived from `cfg!(feature = "http3")`.
- `pub(crate) fn missing_client_error(policy) -> Error` — returns the
  capability-naming error for `HttpVersionPolicy::Http3Only` on a build
  without `http3`, and the unchanged generic missing-client error for every
  other policy.

The decision is made from the policy variant and the compile-time capability.
It never parses a rendered string.

Two call sites, and they are the only two places where `inner.hyper_client ==
None` becomes an error — the proxy dispatch never reads `inner.hyper_client`,
and the UDS / direct / SNI / custom arms check their own clients:

- `crates/eggfetch-core/src/pipeline/hyper_dispatch.rs:241` — the high-level
  dispatch path.
- `crates/eggfetch-core/src/pipeline/mod.rs:732` — the native-frame dispatch
  path.

The failure stays at the existing raise point, so its timing relative to the
connect phase is unchanged.

### CLI (`eggfetch-cli`)

- `crates/eggfetch-cli/Cargo.toml`: new `[features]` — `default = []`,
  `http2 = ["eggfetch-core/http2"]`, `http3 = ["eggfetch-core/http3"]`.
- `crates/eggfetch-cli/src/protocol.rs` (new): `ProtocolCapability`, with
  `is_compiled_in()` derived from `cfg!`, `protocol_name()`, `feature_name()`,
  `missing_capability_message()`, and `require()`.
- `crates/eggfetch-cli/src/main.rs`: `require_capability(ProtocolCapability::Http2)`
  / `…::Http3` runs before the policy is applied and before the client is
  built.

### FFI (`eggfetch-ffi`)

- `crates/eggfetch-ffi/src/protocol.rs` (new): `MissingProtocolCapability`,
  `missing_capability(policy)` — records a gap for strict policies only —
  `message()` and `error()`.
- `crates/eggfetch-ffi/src/handle.rs`: `ClientBuilderHandle` and
  `ClientHandle` became braced structs carrying `missing_capability`; a new
  `reject_missing_capability(client, err_out)` helper serves both send paths.
  The fields remain `pub(crate)`.
- `crates/eggfetch-ffi/src/builder.rs`: `eggfetch_client_builder_http_version`
  records the gap for the accepted policy. The last accepted policy wins.
- `crates/eggfetch-ffi/src/client.rs` and `streaming.rs`: the guard runs
  before dispatch, on both the blocking and the streaming send paths.

### Error text, before and after

| Surface | Before | After |
|---|---|---|
| CLI `--http2` without the capability | request silently issued over HTTP/1.1, exit 0 | `unsupported: HTTP/2 is not supported by this build: the built binary was compiled without the http2 feature`, exit **5** |
| CLI `--http3` without the capability | `unsupported: HTTP client not available for this protocol`, exit 5 | `unsupported: HTTP/3 is not supported by this build: the built binary was compiled without the http3 feature`, exit **5** |
| FFI strict HTTP/2 without the capability | builder → 0; request proceeded over HTTP/1.1 | builder → 0 (value understood); request → NULL response, `err_out` kind `unsupported`, message `unsupported: HTTP/2 is not supported by this build: the linked library was compiled without the http2 feature` |
| FFI strict HTTP/3 | **not reachable** — the published value set has no strict HTTP/3 value (3 = `Auto{allow_http3:true}`) | unchanged; `Auto` is still accepted and still negotiates down. The strict classification exists in `protocol::missing_capability` and is unit-tested |
| Engine `Http3Only` without `http3` | `unsupported: HTTP client not available for this protocol` | `unsupported: HTTP/3 is not supported by this build: the http3 feature was not compiled in` |
| Generic missing-client error | `unsupported: HTTP client not available for this protocol` | **unchanged** |

## 4. Verification executed

### Commands run

```sh
cargo test -p eggfetch-cli -- --test-threads=1
cargo test -p eggfetch-cli --features http2,http3 -- --test-threads=1
cargo test -p eggfetch-ffi -- --test-threads=1
cargo test -p eggfetch-core --all-features http_version -- --test-threads=1
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls -- --test-threads=1
cargo clippy -p eggfetch-cli -p eggfetch-ffi --all-targets --all-features -- -D warnings
cargo clippy -p eggfetch-core --all-features -- -D warnings
cargo clippy -p eggfetch-core --all-targets --all-features -- -D warnings
cargo test -p eggfetch-ffi --all-features --test protocol_capability_tests
RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts -- --test-threads=1
cargo check -p eggfetch-node --all-features
rustfmt --edition 2021 --check <the touched .rs files>
```

### Results (pass/fail/skip with reason; counts without concealment)

| Command | Result |
|---|---|
| `cargo test -p eggfetch-cli` | exit 0 — 31 lib + 69 integration + 3 `protocol_capability` passed, 0 failed |
| `cargo test -p eggfetch-cli --features http2,http3` | exit 0 — 31 + 69 + 3 passed (the 3 are the capability-enabled variants) |
| `cargo test -p eggfetch-ffi` | exit 0 — 9 lib + 34 `ffi_tests` + 5 `protocol_capability_tests` passed, 0 failed |
| `cargo test -p eggfetch-core --all-features http_version` | exit 0 — 10 lib unit tests + 2 new integration tests passed |
| `cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls` | exit 0 — the whole lean suite compiled and passed (510 lib, 90 integration, 3 new in `http_version_capability_tests`, plus the rest). The M009 lean compile gap did **not** materialize for this feature set. One pre-existing, unrelated warning: `unused import: decompress_buffered` in `tests/streaming_decompression_tests.rs` |
| `cargo clippy -p eggfetch-cli -p eggfetch-ffi --all-targets --all-features -- -D warnings` | exit 0 — after fixing two findings in the new code: `unnecessary_debug_formatting` in the CLI test and `too_many_lines` on `eggfetch_client_send_streaming`, resolved by extracting `handle::reject_missing_capability` |
| `cargo clippy -p eggfetch-core --all-features -- -D warnings` | exit 0 |
| `cargo clippy -p eggfetch-core --all-targets --all-features -- -D warnings` | exit 0 |
| `cargo test -p eggfetch-ffi --all-features --test protocol_capability_tests` | 5 passed |
| `RUSTFLAGS= cargo test -p eggfetch-core --test public_api_contracts` | 7 passed — public surface unchanged |
| `cargo check -p eggfetch-node --all-features` | exit 0 — the Node adapter is unaffected by the `ClientHandle` reshape |
| `rustfmt --edition 2021 --check` on all touched files | clean |
| Manual `./target/debug/eggfetch --http2 http://127.0.0.1:1/` | exit 5, capability message naming HTTP/2 |
| Manual `./target/debug/eggfetch --http3 http://127.0.0.1:1/` | exit 5, capability message naming HTTP/3 |
| Manual `./target/debug/eggfetch --http1 http://127.0.0.1:1/` | proceeds to the request (no capability rejection) |

The repository-wide gates were subsequently run at the bound SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c`: `./scripts/check.sh` (Tier 1),
`./scripts/check.sh extended` (Tier 2), and `./scripts/check.sh package`
(Tier 3) are all green, together with the live security preflight
(`./scripts/check_security.sh` → `advisories ok, bans ok, licenses ok, sources
ok`). Two host accommodations are recorded rather than hidden: the first
Tier 2 attempt failed with a linker SIGBUS (`ld terminated with signal 7`)
while compiling a test binary, which was host memory pressure from a
concurrent build on the same 16-CPU / 15-GiB host and not a code defect — the
same step re-verified clean immediately afterwards, the full gate then passed
end to end, and the rerun used `CARGO_BUILD_JOBS=6`; and the downstream
behavioral-fixture gate, which fails closed by design, executed rather than
skipping because the operator's artifact manifest was produced for this
qualification. The targeted equivalents listed above remain the milestone's own
implementation evidence; §10 records the qualification evidence.

## 5. Invariant review

- **All networking stays in `eggfetch-core`.** The CLI and FFI adapters only
  read a compiled capability and map it to an error; neither opens a socket,
  parses HTTP, or talks to hyper/tokio directly.
- **No new public engine surface.** `Http3Capability` and
  `missing_client_error` are `pub(crate)`. `public_api_contracts` passes.
- **No new error variant and no `TransportFailureKind` change.** The
  classification reuses `Error::Unsupported`; the ADR-0006 transport-failure
  domain is untouched.
- **No new cargo dependency.** The CLI's H2 test was approximated rather than
  adding `hyper` / `hyper-util` as a dev-dependency.
- **No new CLI exit code.** The failure reuses the existing
  `Error::Unsupported` → 5 mapping.
- **The native Rust downgrade contract is preserved.** `from_policy` is
  untouched; `Http2Only` without `transport-http2` still downgrades for native
  callers.
- **No redaction regression.** The new error messages contain only a protocol
  name and a feature name; no header value, cookie, or credential is
  rendered.
- **`unsafe_code = "forbid"` respected.** The new modules contain no `unsafe`.
- **Clippy pedantic is clean** on every touched crate, with specific lint
  names only and no blanket suppression.

## 6. Failure, timeout, pool, and recovery review

- The CLI guard runs **before** the client is constructed, so a rejected
  request consumes no connection, no pool permit, and no request deadline.
- The FFI guard runs **before** dispatch, on both the blocking and the
  streaming send path, so a rejected request likewise consumes no connection
  and no deadline.
- The engine classification is evaluated at the existing raise point. The
  failure timing relative to the connect phase is unchanged; the guard does
  not introduce an earlier abort that could strand a pool permit.
- Neither guard is a retry, redirect, or proxy-fallback trigger: an
  unsupported protocol is a permanent, typed condition and is never retried
  with another route or another store.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

Feature-profile impact:

- The CLI gains two **opt-in** crate features. The published default CLI
  capability set is unchanged (`default = []`), so no published artifact
  changes capability by default.
- `eggfetch-core`'s feature graph is unchanged. No core feature was added,
  removed, or renamed; `public_api_contracts` passes under the default
  profile and the two new CLI profiles were exercised with the features on
  and off.
- The FFI's feature forwarding was already present (`http2` → `eggfetch-core/http2`,
  `http3` → `eggfetch-core/http3`); no new FFI feature was declared.

**Behavior changes visible to consumers** (both mandated by ADR-0008 and
plan §9):

1. A published FFI consumer that requests `Http2Only` (policy value 1) on a
   default FFI build without the `http2` feature previously got a silent
   downgrade to HTTP/1.1; it now receives a named rejection. There is no
   in-repo consumer — the Node adapter never calls
   `eggfetch_client_builder_http_version`.
2. A CLI user passing `--http2` or `--http3` to a default build previously
   got a silent downgrade (for `--http2`) or a misleading error (for
   `--http3`); both now name the missing capability.

**Exact-SHA binding.** This milestone's changes — Rust sources, tests, and crate
manifests — are executable inputs under `.skills/verification-qualification.md`,
so they invalidated the previous live Stage C binding. The binding has since
been re-established: the ledger's Stage C SHA and both HTTPX profiles are
rebound to `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, which carries this
milestone's implementation (`d6c41adf4cf8f62ef990cccedf2f86512858fa0b`) and
the lockfile-only dependency patch on top of it. No Rust source differs
between the two commits.

## 8. Security review

- The change removes a **downgrade** path: an adapter could previously accept
  a strict protocol request and silently perform it over a different protocol.
  Failing closed removes that class of misdirected traffic.
- The FFI guard prevents a strict-policy request from being sent over an
  unintended protocol with credentials attached.
- No new trust store, certificate, hostname-verification, proxy, or cookie
  behaviour was introduced. The native-root construction fallback rule and the
  "never retry a cert/hostname verification failure with another store" rule
  are untouched.
- No credential, header value, or cookie is rendered in the new messages.
  `authorization`, `proxy-authorization`, `cookie`, and `set-cookie` redaction
  is unaffected.
- The new FFI module reaches `err_out` only with a static, capability-naming
  string. It does not echo caller-supplied data.

## 9. Documentation and operations

| File | Change |
|---|---|
| `docs/architecture/cli.md` | The protocol section now states that `--http2` / `--http3` require a CLI build with the corresponding core feature compiled in, that an uncompiled capability fails with an error naming the missing capability and does not silently downgrade, that the default CLI build compiles neither `http2` nor `http3`, that a build with those features supports the flags, and that the `Auto`-style negotiation downgrade is an engine-level contract rather than CLI behaviour. The previously false sentence claiming an uncompiled capability "fails instead of silently downgrading" was replaced with the true, complete statement. |
| `docs/architecture/ffi-and-node.md` | New "HTTP Version Policy" section describing the validation and the request-time rejection; a `protocol.rs` row in the module map. |
| `docs/architecture/core-tls-proxy-protocols.md` | New "Error Classification" section under HTTP/3 describing the typed classification and the two dispatch paths. |
| `docs/reference/errors.md` | **Not changed** — the error-kind mapping (`Unsupported` → `unsupported`) is unchanged, so no edit was warranted. |
| `docs/architecture/feature-flags.md` | The CLI crate features are documented in the "CLI crate features (`eggfetch-cli`)" subsection: the two opt-in features, the empty default set, the `cfg!`-derived capability read, the fail-closed behavior with exit code 5, and the separation between the adapter's strict flags and the engine's negotiation downgrade. |
| `plans/implementation/tls-proxy-protocols/005-uncompiled-protocol-fail-closed.md` | §6.1's note about Cargo feature unification was corrected: unification applies to dependency features, not to a workspace member's own optional features, so a workspace build does **not** enable `eggfetch-cli/http2` / `http3`. The correction is recorded in the plan itself. |

Operations: the CLI gains two crate features that are off by default, so no
published binary changes. The FFI gains no new exported symbol and no new C
ABI value. No new dependency, network access, or credential is required.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| Resolved | The live Stage C binding was invalidated by this milestone's Rust, test, and manifest changes, and the Stage C rebinding had not been performed at the time of implementation | The previous binding could not be cited by any publication | **Resolved 2026-10-22.** Both HTTPX profiles and the ledger's Stage C SHA were rebound to `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, and the frozen-SHA qualification was re-run |
| Resolved | Tier 1, Tier 2, and Tier 3 had not been run on the frozen SHA | The milestone's own gates are green; the repository-wide gates were unproven for this tree | **Resolved 2026-10-22.** Tier 1, Tier 2, and Tier 3 are green on `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, together with the live security preflight |
| Medium | The FFI has **no strict HTTP/3 policy value** to reject. The published values are `0`=`Http1Only`, `1`=`Http2Only`, `2`=`Auto{allow_http3:false}`, `3`=`Auto{allow_http3:true}`; ADR-0008 rule 3 requires `Auto` to keep negotiating down, so the reachable strict value is HTTP/2 only | The plan's §10 FFI bullet for strict HTTP/3 cannot be satisfied as written. The classification itself exists and is unit-tested; only the ABI value is missing | Maintainer decision: either add a strict-HTTP/3 FFI policy value (the guard path is already written and unit-tested, and `docs/ffi/README.md` would need the new value documented), or record the ABI surface as deliberately excluding strict HTTP/3 |
| Medium | The CLI's `--http2` negotiation-success case could not be exercised end to end: an HTTP/2 server fixture needs `hyper` / `hyper-util` as a new dev-dependency in `eggfetch-cli`, which the milestone's constraints forbid | The guard's *rejection* path is fully covered; the *acceptance* path is covered only as "the guard does not reject and a connection is attempted" | Either authorise a dev-dependency in a follow-up plan, or accept the coverage as-is and record it as a known limitation of the CLI test surface |
| Low | A published FFI consumer relying on `Http2Only` downgrading to HTTP/1.1 on a default FFI build now receives a named rejection | Intentional behavior change per ADR-0008; no in-repo consumer exists | None. Recorded in ADR-0008 and in this closure record |
| Low | The host filesystem had roughly 122 GB free during the frozen-SHA qualification | No space pressure was observed for the Tier 1 / Tier 2 / Tier 3 runs | None. Monitor before future full-workspace builds |

## 11. Roadmap disposition

`plans/subsystems/tls-proxy-protocols-roadmap.md` milestone 5 moves from
`conditionally closed` to `closed`, with §11 (completion definition) and §12
(milestone status) updated in the same pass.

The milestone's own scope is complete, and the cross-cutting release evidence
it was waiting on — Tier 1 / Tier 2 / Tier 3 on the frozen SHA and the Stage C
rebinding — was delivered on the bound SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c`. What remains is the milestone's
own scoped findings: the maintainer decision on the FFI's strict-HTTP/3 policy
value, the CLI's `--http2` negotiation-success coverage, and the intentional
FFI `Http2Only` behavior change. The strict-HTTP/3 decision is recorded as a
pending maintainer action in `plans/registry.md`.

## 12. Registry updates

`plans/registry.md`:

- The tls-proxy-protocols row records milestone 5 as closed.
- The implementation-plan table for M005 points at this closure record.
- The pending maintainer actions table gains the FFI strict-HTTP/3 policy-value
  decision.
- The blocked / operational work section no longer records the Stage C
  rebinding as pending for the milestones closed in this pass; the rebinding
  was delivered on `26fa619299fb8d447f25cbcb5eb713a1f537a30c`.
