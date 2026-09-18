# Linked Byte Baseline and Attribution

Planning baseline: `6093a66959165f132f02102ffb727ac3e710917c` (`main`, 2026-09-17; eggfetch-core 0.1.6)
Parent program: `plans/linked-binary-footprint-reduction-program.md`
Status: complete (2026-09-18; entry + attribution recorded; see Closure record)

## Objective

Establish a current, reproducible linked-binary baseline and symbol-level attribution before changing the engine for footprint.

The existing `docs/architecture/embedded-footprint.md` record is historically useful but predates two relevant changes now on `main`:

- DashMap and its unique transitive closure were removed;
- native HTTP transport can now omit `url` / IDNA / ICU.

The native-URI closure also proved that package-count reduction alone can produce effectively zero linked-size reduction. This plan therefore measures a real request path and attributes linked bytes to crates and symbols.

This plan should not change production behavior. If the measurement harness reveals a correctness bug, record it and create/extend the appropriate executable plan rather than fixing unrelated code inside the baseline commit.

## Profiles to measure

Use a standalone downstream-style fixture outside the core library's own test binary so dead-code elimination reflects an ordinary consumer.

At minimum measure four profiles under the same target/toolchain/release settings:

### A. Aligned reqwest baseline

Equivalent behavior where practical:

- reqwest 0.12;
- no default features;
- HTTP/1;
- Rustls/WebPKI;
- streaming/bounded body consumption;
- no redirect following;
- no automatic logical retry;
- Bearer Authorization header;
- comparable timeout behavior as far as reqwest exposes it.

### B. eggfetch 0.1.5 historical compatibility baseline

Use the feature recipe that motivated the Gregg migration:

```toml
eggfetch-core = {
  version = "=0.1.5",
  default-features = false,
  features = ["http1", "tls-rustls"]
}
```

Exercise the real high-level request path, including `send_detailed()`.

### C. current-main high-level profile

Use current `main` with the compatibility-equivalent high-level profile:

```text
default-features = false
features = ["http1", "tls-rustls"]
```

This isolates the effect of DashMap removal and other 0.1.6 changes before new work.

### D. current-main native transport control

Use:

```text
default-features = false
features = ["native-http1", "tls-rustls"]
```

Exercise a real `http::Request` through `execute_http_body`, not a construction-only fixture.

This profile is diagnostic. It is not necessarily the downstream target because Gregg-like clients benefit from the high-level URL/request API and typed `send_detailed` response surface.

## Gregg-like request behavior

The high-level fixture must keep the following behavior reachable in the linked binary:

- one HTTP GET;
- one HTTPS GET;
- default DNS / standard route only;
- Rustls with packaged WebPKI roots;
- redirects disabled;
- no logical retry configured;
- Bearer auth construction and application;
- explicit pool/connect/write/read/total timeout configuration;
- per-client or per-request decoded-body cap;
- `send_detailed()` typed failure path;
- response status and bounded bytes consumption.

Use loopback fixtures for runtime smoke where network behavior is needed. No external network dependency belongs in the measurement.

## Release-build comparability

Record:

- exact Rust version;
- target triple;
- linker;
- CPU architecture;
- Cargo.lock state for each standalone fixture;
- release profile fields;
- whether `lto` is fat/thin/off;
- `codegen-units`;
- `panic`;
- `strip`;
- raw and stripped bytes.

For direct Gregg relevance, include a measurement using Gregg's release shape where practical:

```toml
lto = "fat"
codegen-units = 1
strip = true
panic = "abort"
```

If the generic embedded qualification keeps its historical profile for continuity, run both rather than silently changing the historical series.

## Attribution

For an unstripped companion build of each eggfetch profile, collect:

```sh
cargo bloat --release --crates
cargo bloat --release -n 100
cargo tree -e normal
cargo tree -e features
cargo tree -d
```

Use the output to classify material linked contributors into:

- shared Hyper/Tokio/Rustls foundation;
- standard resolver/connector;
- eggfetch logical pool;
- lifecycle/transport metrics;
- direct connector / socket-option path;
- custom dialer;
- UDS;
- SNI/resolved-route caches;
- retry;
- redirect;
- auth/Base64;
- TLS PEM/custom identity policy;
- response/decompression body wrappers;
- other.

Do not infer byte ownership solely from source-file size.

## Dependency checks

For current main, explicitly record whether the measured profile resolves:

```text
dashmap
url / idna / ICU / percent-encoding
getrandom
httpdate
base64
eggfetch-http-connect
pem-rfc7468 / base64ct
tower-service
```

Explain whether each dependency is:

- required by the exercised path;
- merely resolved but link-pruned;
- reachable and linked;
- already absent.

## Output

Add a dated subsection to `docs/architecture/embedded-footprint.md` or a tightly scoped companion record with:

- exact baseline SHAs/versions;
- byte table;
- package/feature table;
- top linked crates/symbol families;
- a ranked list of candidate upstream boundaries.

The ranked list is an implementation input, not a promise. Plans 2-4 may be narrowed if attribution disproves an expected source of overhead.

## Stop/decision rules

- If current main already removes most of the historical Gregg-like delta, do not implement broad new cfg boundaries merely because this program exists.
- If advanced routing dominates linked bytes, proceed with the standard-route split.
- If retry/redirect dominate, prioritize policy gating.
- If TLS/crypto dominates and eggfetch-owned code is minor, record the limit; do not weaken crypto or roots.
- If `cargo bloat` shows an unexpected dominant owner, amend the later child plan before implementation rather than forcing the prewritten hypothesis.

## Validation

This plan should require only the fixture/runtime checks needed to prove the measurement exercises real request paths. If no production code changes, Tier 1 is not required solely to record measurements, though any committed script/document changes should follow normal repository lint/document checks.

## Exit criteria

- [ ] 0.1.5, current-main high-level, current-main native, and aligned reqwest profiles are measured under documented equivalent settings.
- [ ] The high-level fixture performs a real request path and typed-failure path.
- [ ] Stripped and unstripped sizes are recorded.
- [ ] Crate and symbol attribution is captured.
- [ ] The expected contribution of advanced routing, retry/redirect, auth, TLS helpers, and residual dependencies is evidence-backed.
- [ ] Later executable plans are confirmed, narrowed, or explicitly amended based on the evidence.

## Closure record (2026-09-18)

Entry measurement taken on current `main` (`263e7749`, with the policy
boundary already landed, before the standard-route split) using the
`qualification/embedded/eggfetch-min` source (streaming HTTPS GET, same
source for all eggfetch profiles) with isolated `CARGO_TARGET_DIR`s, release
shape `lto="thin"`, `codegen-units=1`, `panic="unwind"`, `rustc 1.98.1`,
`x86_64-unknown-linux-gnu`:

- Full compatibility (`http1,tls-rustls`): unstripped 8,683,256 B, stripped
  3,669,840 B. `cargo bloat --crates`: `eggfetch_core` .text 262.0 KiB.
  Top symbol `pipeline::send_single_request::{closure#0}` 55.8 KiB with
  advanced-route monomorphizations (UDS/Dialer/Direct `send_request`
  closures ~18 KiB each), `send_with_redirects` 19.0 KiB,
  `DirectConnector::call` 14.2 KiB, `ClientBuilder::build` 13.1 KiB.
- Policy-lean (`native-http1,high-level-url,tls-rustls`, no policy bundle):
  unstripped 8,609,648 B, stripped 3,600,816 B (−69,024 stripped vs full).
- Aligned reqwest (`reqwest-min`, `stream,rustls-tls`): unstripped
  7,873,176 B, stripped 3,079,840 B (matches the historical x86_64 record).
  Full-vs-reqwest stripped delta: +590,000 (+19.2%).
- Unique packages: full 110 → lean-standard 105 (see closure plan for the
  final lean numbers). Dependency-count reduction is modest; the material
  win is linked bytes (see standard-route and closure plans).
- Residual candidates in the full link: `httpdate` 6.4 KiB, `base64`
  1.3 KiB, `getrandom` 748 B (via ring), `tracing`/`log` negligible;
  `eggfetch_http_connect`/`pem_rfc7468`/`base64ct` zero linked bytes when
  uncalled (link-pruned). Advanced-route machinery dominates eggfetch-owned
  bytes, confirming the standard-route split as the highest-value boundary;
  retry/redirect policy is second (already split). No unexpected dominant
  owner; later plans confirmed as written.

Exit criteria met: four profiles measured under documented equivalent
settings (0.1.5 historical via the prior x86_64/aarch64 records linked from
`docs/architecture/embedded-footprint.md`); real request + typed-failure
paths exercised; stripped/unstripped recorded; crate/symbol attribution
captured; advanced-routing vs policy contributions evidence-backed.
