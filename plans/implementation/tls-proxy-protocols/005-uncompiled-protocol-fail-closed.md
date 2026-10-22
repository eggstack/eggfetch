# TLS, Proxy, and Protocols Milestone 005 — Uncompiled-Protocol Fail-Closed at Adapter Boundaries

Status: ready

Repository baseline: `873ea28541c82817317430c9523fca8525d99c81`

Source roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-5--uncompiled-protocol-fail-closed-at-adapter-boundaries`

Long-term requirements:

- `plans/000-long-term-specification.md` §4.1, §5
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 items 2 and 4

Applicable ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`
- `plans/adrs/ADR-0008-uncompiled-protocol-request-semantics.md`

Primary class: capability

## 1. Objective

Make a strict protocol request that the built artifact cannot honor fail
closed, with an error that names the missing capability, at every adapter
boundary that exposes a user-facing protocol control.

This covers three observed defects:

- `eggfetch-cli --http2` on a CLI build without `transport-http2` currently
  issues the request over HTTP/1.1 with no warning;
- `eggfetch-cli --http3` on a CLI build without `http3` currently fails with
  `Error::Unsupported("HTTP client not available for this protocol")`, which
  names a missing Hyper client rather than the missing HTTP/3 capability;
- `eggfetch_client_builder_http_version` in `eggfetch-ffi` accepts strict
  protocol values without consulting the compiled feature set.

The engine's documented downgrade for native Rust callers is unchanged.

## 2. Why this milestone is ready

The architectural question is closed by ADR-0008:

- the engine's downgrade for native callers is a deliberate, documented
  contract recorded in `AGENTS.md` and in the `http_version.rs` documentation,
  and must not be changed;
- the adapter boundary is where a silent downgrade is harmful, because that
  is where a user states a protocol expectation;
- capability knowledge must be derived from the feature forwarding rather
  than hand-duplicated, so an adapter's capability cannot drift from the core
  it links;
- `Auto` is not a strict request and must continue to negotiate down;
- the engine's strict-policy-without-feature error text must not misreport a
  build-configuration problem as a transport failure.

No other open decision is a prerequisite. The change touches adapter
boundaries plus one engine error classification, and does not depend on the
H3 graduation program, which remains separately gated.

## 3. Current implementation evidence

At the baseline:

- `crates/eggfetch-core/src/http_version.rs` documents and implements the
  downgrade: `Http2Only`/`Auto` become `Http1Only` without
  `transport-http2`; `Http3Only` becomes `Http1Only` without `http3`;
- `crates/eggfetch-cli/Cargo.toml` enables core features
  `["cookies", "multipart", "proxy"]` only, so a CLI built from that manifest
  has neither `transport-http2` nor `http3`;
- the CLI has `--http2` and `--http3` flags and applies them to the request
  policy without consulting the compiled feature set;
- `crates/eggfetch-core/src/pipeline/mod.rs` sets `use_h3 = false` under
  `#[cfg(not(feature = "http3"))]`, which makes the intended
  "HTTP/3 support is not enabled" guard in the H3 route arm unreachable;
- `crates/eggfetch-core/src/client.rs` leaves `hyper_client` as `None` when
  the policy is `Http3Only` without `http3`;
- the standard-route arm then returns
  `Error::Unsupported("HTTP client not available for this protocol")`;
- `crates/eggfetch-ffi/Cargo.toml` already declares `http2` and `http3`
  features that forward to the corresponding core features;
- `crates/eggfetch-ffi/src/builder.rs` accepts the HTTP-version policy value
  with no feature check;
- `docs/architecture/cli.md` states that "Requesting an uncompiled capability
  fails instead of silently downgrading", which is false today;
- `crates/eggfetch-cli/tests/integration.rs` contains no `--http2` or
  `--http3` coverage.

## 4. Invariants that must not regress

- `eggfetch-core` remains the sole networking implementation, and it remains
  the sole implementation of protocol negotiation.
- The engine's native-caller downgrade contract is unchanged.
- `Auto` never fails closed; it continues to negotiate down to H1.
- No new public engine surface is introduced unless separately proposed under
  ADR-0005 containment.
- The `ResponseBody` public variant shapes remain frozen.
- No adapter performs direct hyper/tokio TCP.
- Feature-profile isolation is preserved: the lean and native profiles do not
  acquire protocol capability they did not have.
- Redaction rules for `authorization`, `proxy-authorization`, `cookie`, and
  `set-cookie` are unchanged.
- No new cargo dependency is added.

## 5. Scope

### In scope

- CLI: fail closed on `--http2` and `--http3` when the linked core lacks the
  capability, with an error naming the missing capability;
- CLI: express `http2`/`http3` as crate features forwarding to the core
  features, so compiled capability is derived rather than hand-listed;
- FFI: reject a strict HTTP-version policy the linked build cannot honor,
  naming the missing capability;
- engine: classify "this build has no HTTP/3 support" distinctly from "this
  request had no usable client", without adding a new error type;
- the `docs/architecture/cli.md` contract sentence, made true and completed
  with the `Auto` exception;
- focused CLI, FFI and engine tests for each behavior above.

### Explicitly out of scope

- changing the engine's downgrade for native Rust callers;
- graduating H3, or any H3 behavior change;
- enabling `http2`/`http3` in the CLI by default, or changing the CLI
  feature ladder;
- the FFI's missing non-protocol capabilities (proxy setter, multipart,
  custom CA, client identity, UDS, dialer, SNI, resolved addresses, trailers,
  metrics, retry policy);
- the `native-http2`/`standard-http2` profile-matrix gaps, which are tracked
  separately by core-transport-policy M008;
- Node adapter work;
- publication, version bump, or release orchestration.

## 6. Required production changes

### 6.1 CLI capability forwarding

Add to `crates/eggfetch-cli/Cargo.toml`:

```toml
[features]
default = []
http2 = ["eggfetch-core/http2"]
http3 = ["eggfetch-core/http3"]
```

The features stay off by default, so the published default CLI retains its
current capability set. The CLI must then read its compiled capability with
`cfg!(feature = "http2")` / `cfg!(feature = "http3")` rather than repeating a
hand-maintained list of core features.

Correction (2026-10-22, recorded during implementation): the earlier note in
this section stated that Cargo feature unification makes a workspace build
enable these features for the CLI. That is wrong about cargo — unification
applies to *dependency* features, not to a workspace member's own optional
features — so `cargo build --workspace` does **not** enable `eggfetch-cli/http2`
or `eggfetch-cli/http3`. A workspace-built CLI still rejects `--http2` /
`--http3` unless the CLI's own feature is selected (a `--all-features` build
does select it). The artifact still reports its own capability, which is the
intent of the original note; only the mechanism differs. The CLI must not
attempt to suppress unification.

### 6.2 CLI fail-closed guard

When the CLI parses `--http2` or `--http3`:

- if the corresponding capability is not compiled in, fail before any
  connection is attempted;
- the error must name the missing capability explicitly and state that the
  built binary does not support it;
- the error must not reuse the generic "HTTP client not available" wording,
  which would misreport a build capability as a transport failure;
- the failure must use the CLI's existing error/exit-code mapping so the
  exit code stays within the documented set in `docs/architecture/cli.md`;
- `--http2` must not silently fall back to HTTP/1.1.

`Auto` behavior is not reachable from the CLI flags and is unaffected.

### 6.3 FFI policy guard

`eggfetch_client_builder_http_version` must validate the requested policy
against the FFI crate's compiled `http2`/`http3` features:

- a strict policy whose capability is absent must be rejected at builder or
  request time, before any connection attempt;
- the rejection must identify the missing capability;
- `eggfetch_client_builder_http_version` must continue to accept the values
  the linked build can honor, and the FFI's existing default must be
  unchanged.

The FFI already forwards `http2`/`http3` to the core, so no new feature
declaration is required there.

### 6.4 Engine error classification

The engine must distinguish two conditions that currently share one message:

- the build has no HTTP/3 support and the caller asked for HTTP/3;
- the request had no usable client for a different reason.

Both may remain `Error::Unsupported`, but the HTTP/3-without-`http3` case must
carry a message that names the missing HTTP/3 capability rather than the
missing client. The classification must be produced by typed evidence, not by
parsing a `Display` string.

This correction must not introduce a new public error variant, and must not
change the `TransportFailureKind` domain established by ADR-0006.

### 6.5 Documentation contract

`docs/architecture/cli.md` must state, in the protocol-support section:

- `--http2`/`--http3` require a CLI build with the corresponding core feature
  compiled in;
- requesting an uncompiled capability fails with an error naming the missing
  capability, and does not silently downgrade;
- the default CLI build compiles neither `http2` nor `http3`;
- a build compiled with those features supports the flags.

The `Auto`-style negotiation downgrade is an engine-level contract and must be
documented as such rather than described as CLI behavior.

## 7. Ordered work packages

1. Add failing CLI integration tests for `--http2` and `--http3` on a build
   without the capability, asserting the error names the missing capability.
2. Add the CLI `http2`/`http3` forwarding features.
3. Implement the CLI guard and its error mapping.
4. Add failing FFI tests for a strict policy on a build without the
   capability.
5. Implement the FFI policy guard.
6. Implement the engine error-classification correction.
7. Add engine tests for the classified HTTP/3-without-feature error.
8. Update `docs/architecture/cli.md` and the adapter boundary documentation.
9. Run Tier 1, then Tier 2, then Tier 3 where the roadmap phase requires.
10. Renew the exact-SHA Stage C binding, because the engine error
    classification is an executable change.
11. Write `plans/closure/tls-proxy-protocols/005-uncompiled-protocol-fail-closed.md` and return the
    subsystem to steady state if evidence is green.

## 8. Failure, cancellation, timeout, and pool semantics

- The CLI and FFI guards must run before any connection is attempted, so no
  pool permit, connection attempt, or deadline is consumed by a request that
  cannot be honored.
- The guards must not consume or alter the request's total deadline.
- The engine correction is confined to error classification and must not
  change when a failure is raised relative to the connect phase.
- No cancellation, backpressure, or body-lifecycle behavior changes.
- The CLI and FFI guards must be deterministic and side-effect free apart
  from returning the failure.

## 9. Compatibility and migration

- Native Rust callers: no behavioral change except the corrected message for
  `Http3Only` on a build without `http3`. That is a message change on an
  existing error kind.
- CLI: `--http2`/`--http3` on a build without the feature now fail with a
  named cause instead of downgrading or misreporting. Builds with the feature
  are unaffected.
- FFI: a strict policy the linked build cannot honor now fails with a named
  cause instead of downgrading.
- Feature-profile effects: the CLI gains two new opt-in crate features that
  forward to existing core features. The default CLI feature set is
  unchanged. No core profile changes.
- HTTPX 0.28.1 and httpx2 2.12.0 facades expose no strict protocol control,
  so their behavior is unchanged; the engine error-message correction is
  still an executable input and requires renewed exact-SHA qualification.
- No crate version bump is implied. Publication is handled by
  release-verification intake.

## 10. Required tests

### CLI

- `--http2` on a build without `transport-http2` fails, and the message names
  HTTP/2;
- `--http3` on a build without `http3` fails, and the message names HTTP/3
  and does not contain the generic "HTTP client not available" wording;
- neither failure opens a connection;
- with the corresponding feature enabled, `--http2` against a suitable
  server negotiates HTTP/2, proving the guard does not block a supported
  protocol;
- the default CLI feature set is unchanged, asserted by inspecting the
  crate's resolved feature set.

### FFI

- `eggfetch_client_builder_http_version` with the strict HTTP/2 value on a
  build without `http2` is rejected and names HTTP/2;
- the same for HTTP/3 without `http3`;
- a supported policy value is accepted;
- the FFI default is unchanged.

### Engine

- `Http3Only` on a build without `http3` produces the capability-naming
  error, not the missing-client error;
- the same request with `http3` compiled in is unaffected;
- `Auto` without `http3` still negotiates down to HTTP/1.1 without failing;
- `Auto` with `http3` compiled in still behaves as before;
- no `Display`-string parsing is introduced in the classification path.

### Regression

- existing CLI exit-code mapping tests remain green;
- existing FFI surface tests remain green;
- the public API oracle is unchanged.

## 11. Required verification commands

Follow `docs/verification-policy.md` and
`.skills/verification-qualification.md`.

Focused first:

```bash
cargo test -p eggfetch-cli -- --test-threads=1
cargo test -p eggfetch-ffi -- --test-threads=1
cargo test -p eggfetch-core --all-features http_version -- --test-threads=1
cargo test -p eggfetch-core --no-default-features --features http1,tls-rustls -- --test-threads=1
```

Then:

```bash
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

The CLI feature variants must also be exercised explicitly, because the
default feature set does not include them:

```bash
cargo test -p eggfetch-cli --features http2,http3 -- --test-threads=1
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

Tier 2 requires the exact Rust 1.89.0 toolchain and fails rather than skips.
Do not create a new automatic workflow or a permanent network-dependent CI
job.

## 12. Documentation updates

Implementation must update:

- `docs/architecture/cli.md` (protocol and compression support section);
- `docs/architecture/ffi-and-node.md` (FFI HTTP-version policy behavior);
- `docs/architecture/feature-flags.md` if the new CLI crate features are
  documented there;
- `docs/architecture/core-tls-proxy-protocols.md` for the engine error
  classification;
- `docs/reference/errors.md` if the error-kind mapping changes;
- `plans/registry.md`, the subsystem roadmap and `plans/README.md` at
  closure.

## 13. Acceptance criteria

M005 closes only when:

- `eggfetch-cli --http2` on a build without the capability fails with an
  error naming HTTP/2 and does not issue an HTTP/1.1 request;
- `eggfetch-cli --http3` on a build without the capability fails with an
  error naming HTTP/3 and does not report a missing client;
- the FFI rejects a strict policy the linked build cannot honor, naming the
  missing capability;
- the engine distinguishes "no HTTP/3 support" from "no usable client"
  without a new error variant;
- `Auto` still downgrades to HTTP/1.1 without failing;
- the engine's native-caller downgrade contract is unchanged;
- `docs/architecture/cli.md` states the implemented behavior;
- the default CLI feature set is unchanged and the new features are opt-in;
- the public API oracle is unchanged;
- Tier 1, Tier 2 and Tier 3 pass on the frozen executable SHA;
- both HTTPX profiles are rebound to that exact SHA;
- closure evidence records the before/after CLI and FFI behavior, the
  feature-graph delta, and the known limitations.

## 14. Stop conditions

Stop and revise ADR-0008 rather than improvising if:

- the CLI cannot distinguish its compiled core capability without adding a
  new public engine API, and the maintainer does not accept adding one under
  ADR-0005 containment;
- making the FFI reject an unsupported strict policy would break an existing
  published FFI consumer that relies on the downgrade;
- the engine error-classification correction would require a new public error
  variant or would change `TransportFailureKind`;
- the corrected HTTP/3 message would break an HTTPX conformance assertion
  that asserts on error text, in which case the compat mapping must be
  revisited explicitly rather than the message reverted;
- a fail-closed guard would consume request deadlines or pool permits.

## 15. Closure evidence required

Create `plans/closure/tls-proxy-protocols/005-uncompiled-protocol-fail-closed.md` containing:

- implementation commit(s) and the final frozen SHA;
- before/after CLI and FFI behavior for every strict protocol value;
- the engine error-classification change and its exact message mapping;
- the feature-graph delta for `eggfetch-cli` and `eggfetch-ffi`;
- evidence that `Auto` still downgrades;
- evidence that no connection is opened by a rejected request;
- the public API oracle delta (expected: none);
- exact Tier 1/2/3 outcomes;
- renewed HTTPX Stage C binding evidence for both profiles;
- known limitations, including that H3 remains experimental and that the
  CLI's protocol support still depends on how the artifact was built;
- downstream handoff notes for FFI consumers that relied on the downgrade.

## 16. Handoff notes

The CLI and FFI guards are boundary checks, not a second implementation of
protocol negotiation. Do not move negotiation logic into an adapter.

The default CLI binary's protocol support must remain exactly what it is
today; the fix is about honesty, not about adding capability.

Do not enable `http2`/`http3` for the CLI by default in this milestone. If a
maintainer wants the default CLI to support HTTP/2, that is a separate
capability decision with its own size and footprint consequences, not a
side effect of this correction.

The engine's `Http3Only`-without-`http3` message correction changes a
user-visible string on an existing error kind. Expect downstream consumers
that match on that text to need an update, and record that in the closure
evidence.
