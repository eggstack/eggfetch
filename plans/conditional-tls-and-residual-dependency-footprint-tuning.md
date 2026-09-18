# Conditional TLS and Residual Dependency Footprint Tuning

Planning baseline: current tree after `high-level-policy-footprint-feature-boundary.md`
Parent program: `plans/linked-binary-footprint-reduction-program.md`
Status: complete (2026-09-18; executable change: Candidate A only, B–E skipped on evidence)

## Objective

After the two high-leverage feature-boundary plans land, inspect the remaining linked-byte delta and make only residual changes that have a clear ownership boundary and measurable benefit.

This plan is deliberately conditional. It must not become a micro-feature proliferation exercise. If the remaining candidates are link-pruned or individually negligible, record that result and make no executable change.

## Entry requirement

Before editing production code, rerun the Gregg-like footprint fixture and attribution.

Record:

- stripped bytes after standard-route separation;
- stripped bytes after retry/redirect/Basic-auth separation;
- top remaining eggfetch-owned crates/symbols;
- dependency tree of the lean profile.

Proceed only for candidates that remain either:

1. incorrectly resolved under a disabled capability; or
2. measurably linked and large enough to justify a coarse feature boundary.

## Candidate A: make eggfetch-http-connect proxy-owned

`eggfetch-http-connect` is a generic CONNECT wire primitive used by built-in proxy CONNECT logic. Core currently declares it unconditionally.

If the lean non-proxy profile still resolves it:

- make the dependency optional;
- have `proxy` enable it;
- cfg any imports/types accordingly;
- retain the separate crate and release-order contract;
- preserve every proxy CONNECT/auth/bounded-head test.

Do not duplicate CONNECT serialization back into core.

This change is expected to improve dependency/compile footprint more than linked bytes; measure rather than assume.

## Candidate B: TLS PEM convenience boundary

Current `tls-rustls` owns `pem-rfc7468` and its Base64 closure for custom CA/mTLS PEM parsing.

If symbol attribution shows material linked cost in a simple WebPKI HTTPS client, introduce a compatibility-preserving lower TLS transport slice.

A possible additive shape:

```toml
tls-rustls-transport = [
  "dep:hyper-rustls",
  "dep:rustls",
  "dep:tokio-rustls",
  "dep:webpki-roots",
  "hyper-rustls/ring",
  "hyper-rustls/tls12",
]

tls-pem = ["tls-rustls-transport", "dep:pem-rfc7468"]

# Historical compatibility surface remains full.
tls-rustls = ["tls-rustls-transport", "tls-pem", "tls-rustls-logging"]
```

Exact naming is implementation-selected.

The lower transport profile must retain:

- secure certificate + hostname verification;
- WebPKI roots;
- TLS 1.2/1.3;
- SNI;
- custom TLS config using non-PEM/DER APIs that do not require the parser, where already representable;
- typed TLS failures.

The existing `tls-rustls` profile must retain all current custom CA, additional CA, mTLS, and PEM convenience APIs.

Do not remove PEM capability from defaults.

## Candidate C: Rustls logging feature

The current TLS feature activates `hyper-rustls/logging`.

If the lean transport does not require this for correctness and measurement shows a nontrivial closure:

- keep logging enabled in the historical `tls-rustls` compatibility feature;
- omit it from a new lower transport feature;
- verify no diagnostics/security behavior relies on the dependency.

Do not remove logs from existing profiles silently.

## Candidate D: transport metrics/observability

Transport metrics are currently constructed for every client and instrument standard/advanced routes.

Only investigate a compile-time metrics boundary if attribution shows that the metrics/lifecycle observability code is a material linked contributor after the earlier splits.

If pursued:

- existing compatibility/default profiles must retain metrics;
- a new lean profile may omit metric counters/accessors;
- timeout/lifecycle correctness must not depend on metrics being enabled;
- do not fork lifecycle implementations;
- no diagnostic/security event required by existing APIs may disappear from existing profiles.

This is a higher-complexity candidate than A-C. Skip it unless the byte evidence is strong.

## Candidate E: other unconditional small dependencies

Audit remaining direct dependencies, but do not feature-gate foundational crates simply because they appear in `cargo tree`.

Examples to classify:

- `tower-service`;
- `futures-core` / `futures-util`;
- `pin-project-lite`;
- `http-body-util`;
- `thiserror`.

Default expectation: leave these alone unless attribution proves a coarse semantic owner.

## Security constraints

This plan must never reduce footprint by:

- changing root certificates without a security/version review;
- disabling certificate or hostname verification;
- disabling SNI;
- using a weaker crypto provider;
- removing TLS 1.2/1.3 support from an existing profile;
- parsing PEM/Base64 with new handwritten security-sensitive code;
- changing proxy authentication validation/redaction;
- dropping typed failures.

## Measurement/stop rule

For every candidate actually implemented, record:

- package-count delta;
- stripped-byte delta;
- symbol/crate attribution before/after;
- cfg/public-surface complexity added.

If a change saves only negligible bytes and exists solely for footprint, revert it unless it independently fixes dependency ownership.

No fixed byte threshold is mandated, but the closure note must make the tradeoff explicit.

## Validation

Any executable change requires:

```sh
./scripts/check.sh
```

Run the affected TLS/proxy/feature slices from extended verification.

If TLS public feature/API availability changes in a new profile, run package/dry-run verification before closure.

Do not renew final exact-SHA compatibility profiles here.

## Non-goals

- no alternate TLS backend;
- no root-store weakening;
- no crypto rewrite;
- no removal of PEM/mTLS/custom-CA capability from existing profiles;
- no CONNECT duplication;
- no dozens of dependency-specific features;
- no arbitrary binary-size target;
- no downstream migration code.

## Exit criteria

- [x] Post-policy-split measurement is recorded before changes.
- [x] `eggfetch-http-connect` is proxy-owned if it remained incorrectly unconditional.
- [x] TLS PEM/logging boundaries are split only if linked-byte evidence justifies them.
- [x] Metrics are gated only if material and cleanly separable.
- [x] Existing/default profiles retain all capabilities and security semantics.
- [x] Every retained residual change has measured benefit or independent ownership value.
- [x] Tier 1 and affected feature/TLS/proxy checks pass.

## Closure record (2026-09-18)

Parent plans 2 (`standard-route-advanced-routing-feature-boundary.md`) and 3
(`high-level-policy-footprint-feature-boundary.md`) remain `planned`: they had
not landed when this plan executed, so there is no "after standard-route
separation" or "after retry/redirect separation" delta to record. The entry
measurement below is the current-`main` lean-profile baseline, taken with the
existing `qualification/embedded/eggfetch-min` fixture (`http1,tls-rustls`;
real streaming-GET request path, not construction-only).

### Entry measurement (before changes)

- Fixture: `qualification/embedded/eggfetch-min` release profile
  (`lto="thin"`, `codegen-units=1`, `panic="unwind"`), target
  `x86_64-unknown-linux-gnu`, `rustc 1.98.1 (48a229cea 2026-09-01)`.
- Unstripped: 8,682,080 B; stripped (`strip` copy): 3,668,792 B.
- Lean resolved set (`http1,tls-rustls`): 152 `cargo tree` lines; the
  `+proxy` tree was byte-identical (zero package delta), proving
  `eggfetch-http-connect` was incorrectly resolved under the disabled proxy
  capability.
- `cargo bloat --crates` (unstripped companion): `eggfetch_core` .text
  261.6 KiB; top eggfetch-owned symbol
  `pipeline::send_single_request::{closure#0}` 55.8 KiB with further
  advanced-route monomorphizations (UDS/Dialer/Direct connectors ~12–18 KiB
  each) — all plans-2/3 territory, out of scope here.
- Residual candidates in the lean link: `httpdate` 6.4 KiB, `base64`
  1.3 KiB, `getrandom` 748 B, `tracing` 430 B, `log` 140 B.
  `nm --size-sort` shows zero symbols from `eggfetch_http_connect`,
  `pem_rfc7468`, or `base64ct`: all three are fully link-pruned when their
  APIs go uncalled, so no TLS-PEM/logging split can buy lean-profile bytes.

### Change retained: Candidate A (proxy-owned `eggfetch-http-connect`)

- `crates/eggfetch-core/Cargo.toml`: dependency is now `optional = true`;
  `proxy = [..., "dep:eggfetch-http-connect"]`. No source cfg was needed:
  the sole consumer (`transport::connect`, plus its unit tests) was already
  `#[cfg(feature = "proxy")]`, so no CONNECT serialization was duplicated
  and the separate-crate/release-order contract is unchanged
  (`validate_publishable_internal_dependencies.py` still sees the edge via
  `cargo metadata`; leaf-first publish order retained).
- After-measurement (same toolchain/profile/fixture): unstripped 8,682,168 B
  (+88 B build noise), stripped 3,668,856 B (+64 B noise), identical `.text`
  attribution. Linked-byte delta is effectively zero, as predicted: the
  crate was already link-pruned.
- Dependency delta: lean resolved set loses exactly `eggfetch-http-connect`
  (verified by tree diff; `thiserror`/`base64` remain via core's direct
  ownership), and the lean vs `+proxy` trees now differ by that package.
  Retained per the stop rule: negligible bytes, but an independent
  dependency-ownership fix (proxy feature now truthfully owns its wire
  crate). Compile footprint improves accordingly (proxy-gated crate no
  longer builds for non-proxy profiles).
- Proxy CONNECT/auth/bounded-head tests preserved: full `proxy` feature
  test slice re-run (see Tier 1 + `http1,tls-rustls,proxy` feature tests);
  `cargo check` passes for lean, `+proxy`, default, and `--all-features`.

### Candidates skipped on evidence (stop rule)

- **B (TLS PEM boundary):** skipped. `pem-rfc7468` links zero bytes into
  the lean profile; a `tls-rustls-transport`/`tls-pem` split would add
  public feature complexity with no measured lean-profile benefit.
  `tls-rustls` retains all custom-CA/additional-CA/mTLS/PEM APIs.
- **C (Rustls logging):** skipped. `hyper-rustls/logging` closure in the
  lean link is ~140 B (`log`) — negligible; no diagnostics change.
- **D (metrics boundary):** skipped. No attribution evidence that
  transport metrics/lifecycle observability is a material linked
  contributor; higher complexity than A–C, so skipped per the plan's
  explicit bias.
- **E (small deps):** left alone. `tower-service`, `futures-core`/
  `futures-util`, `pin-project-lite`, `http-body-util`, `thiserror` are
  foundational (Service impls, Body, error taxonomy); `base64`/`httpdate`/
  `getrandom` ownership belongs to the still-planned retry/auth policy
  boundaries, not to this plan.

### Security posture

No change to roots, verification, SNI, TLS versions, crypto provider, proxy
auth validation/redaction, PEM parsing (no new code), or typed failures.
The `proxy` feature still implies `http1` + `tls-rustls`; existing/default
profiles resolve and behave exactly as before.
