# Core Transport Milestone 009 — Lean Path Single-Sourcing and Full-Suite Profile Coverage

Status: closed

Source implementation plan:

- `plans/implementation/core-transport-policy/009-lean-path-single-sourcing.md`

Source subsystem roadmap:

- `plans/subsystems/core-transport-policy-roadmap.md#milestone-9--lean-path-single-sourcing-and-full-suite-profile-coverage`

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

M009 is implemented, the milestone's own targeted gates are green, and the
milestone **is closed**. The milestone changes Rust sources, test targets, a
crate manifest and a validation script; all of those are executable inputs
under the exact-SHA rule in `.skills/verification-qualification.md`, so the
live Stage C binding was invalidated by it at the time of implementation. The
binding has since been re-established and the frozen-SHA tier gates were run
at the bound SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c`:

- **Tier 1** (`./scripts/check.sh`) is green — 1767 Rust workspace tests
  passed (0 failed, 0 ignored), which includes the full `--all-features`
  `eggfetch-core` suite; 581 Python behaviour tests passed; 134 HTTPX
  compatibility smoke tests passed; the native Python API manifest (66
  exports), the Python typing surface and typing fixture, the
  lint-suppression guard self-test and policy scan, the H3
  qualification-runner self-test (runner logic only), the adapter feature
  ownership check and the release version/ref validation all pass; the stable
  public contract fixtures pass; and the feature-isolated profile
  whole-suite compile passes for all four standard/native profiles — the gate
  this milestone added.
- **Tier 2** (`./scripts/check.sh extended`) is green across every phase —
  the pinned HTTPX compatibility suite reports 2073 passed / 0 failed, the API
  manifest comparison is clean for both facades (74 symbols for httpx 0.28.1,
  76 symbols for httpx2 2.12.0; 0 unexplained, 0 stale), the Rust public-API
  oracle passes in eight profiles and `cargo-semver-checks` 0.49.0 reports
  "no semver update required" against the planning baseline
  `03ecba973010e2858bf16a2b5f84d51ce70adae4`, and the feature matrix, MSRV
  matrix on the exact Rust 1.89.0 toolchain, documentation, FFI, resource,
  lifecycle, soak, downstream behavioral-fixture, lossless-merge and Criterion
  phases all pass. The only skip is the policy-defined optional one: the Node
  JS surface, whose native `eggfetch.node` artifact is absent.
- **Tier 3** (`./scripts/check.sh package`) is green, and the live security
  preflight (`./scripts/check_security.sh`) reports
  `advisories ok, bans ok, licenses ok, sources ok`.

The two items that the previous closure pass left outstanding are therefore
recorded as resolved in §10. The rebinding establishes a compatibility
binding only: it authorizes no publication, and HTTP/3 remains experimental.

What the milestone actually delivered:

- The duplicated first-hop preparation logic is **deleted**, not wrapped. A new
  shared builder in `crates/eggfetch-core/src/pipeline/hop.rs` performs the
  exhaustive request destructuring, the header merge, the timeout merge, the
  hop construction, the cookie injection, the auth resolution and the
  `Set-Cookie` jar update. `lean.rs` and `redirect.rs` both call it.
- The whole `eggfetch-core` test suite now **compiles** under the lean profile.
  Thirteen test targets that could not compile there received a file-level
  `#![cfg(...)]` gate expressing that file's own feature requirement.
- `scripts/check.sh` gained a Tier 1 step that compiles the whole suite under
  the lean profile, so a future test that accidentally depends on a non-lean
  feature is caught by the gate rather than by a future maintainer.
- Two separately-compiled equivalence targets assert that the lean path and the
  redirect path prepare the same request.

One thing is **not** claimed. The lean profile's binary footprint was measured
before and after, but not isolated to this milestone, because the working tree
carries other milestones' changes (§10, the footprint finding).

The plan's §13 criterion about the exhaustive `Request::into_parts()`
destructuring was amended on 2026-10-23 to read "exactly one exhaustive
destructuring in the **first-hop preparation** path, with `prepare_single_request`
and `advance_redirect_hop` remaining as the two transport- and subsequent-hop
sites", which is the property the tree satisfies; the plan and this record now
agree (§10, the "exactly one exhaustive destructuring" finding, resolved).

## 2. Requirement-to-evidence matrix

The requirements are the acceptance criteria of §13 of the implementation plan.

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Exactly one exhaustive `Request::into_parts()` destructuring in the **first-hop preparation** path (§13, as amended 2026-10-23) | `grep -n "RequestParts {" crates/eggfetch-core/src/pipeline/*.rs` | **Met as amended** | The first-hop path has exactly one: `hop.rs:82` (the extracted first-hop builder). Two further sites remain exhaustive by §6.2's own requirement: `prepare.rs:304` (`prepare_single_request`) and `redirect.rs:106` (`advance_redirect_hop`). The baseline had four. See §10, the "exactly one exhaustive destructuring" finding, resolved |
| Header merge, timeout merge, cookie injection, auth resolution and `Set-Cookie` update each exist in exactly one place (§13, §6.3) | `crates/eggfetch-core/src/pipeline/hop.rs`; `lean.rs` and `redirect.rs` contain none of them | Met | `lean.rs` is 94 lines and contains no merge, no cookie injection, no auth resolution and no jar update. `redirect.rs` calls `super::hop::update_cookie_jar` from two sites (the redirects-disabled fast path and the per-hop path), but the implementation exists once |
| The lean path and the redirect path produce identical prepared hops for the same request (§13) | `cargo test -p eggfetch-core --test lean_path_equivalence --no-default-features --features standard-http1,tls-rustls` → 2 passed; `cargo test -p eggfetch-core --test redirect_path_equivalence --all-features` → 2 passed; `cargo test -p eggfetch-core --lib --all-features -- --test-threads=1 pipeline::` → 21 passed | Met | Also covered in-crate by `pipeline::hop::test_support` |
| The lean profile's behavior is unchanged: 3xx not followed, one attempt, Bearer-only auth, body cap and total timeout enforced (§13) | `lean_policy_tests` (lean profile) → 6 passed, 0 failed; `lean_route_tests` (lean profile) → 10 passed, 0 failed; `lean_path_equivalence` → 2 passed | Met | `lean_policy_tests` covers the 503 single-attempt, 302 passthrough, Bearer-header-and-redaction and typed refused / timeout / body-cap cases; the equivalence target asserts 3xx is returned without following |
| The whole test suite compiles under `standard-http1,tls-rustls` (§13) | `RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --all-targets` → exit 0, 0 `missing documentation for the crate` warnings | Met | Measured on the tree fingerprint in §4 |
| All thirteen previously failing targets compile under the lean profile and still pass under the default profile (§13) | Compile half: Met (exit 0 above). Pass half: Met — the full `--all-features` `eggfetch-core` suite ran on the bound SHA as part of Tier 1 (1767 workspace tests passed, 0 failed, 0 ignored) | Met | The plan's §11 default-profile run was delivered by the frozen-SHA qualification; see §10 |
| The Tier 1 gate compiles the whole suite under the lean profile (§13, §6.5) | `scripts/check.sh`, function `tier1_profile_whole_suite_compile`, called from `run_tier1` immediately after `tier1_rust_api_contracts` | Met | The gate **compiles** the suite; it does not run it. The main agent later extended the same gate to the sibling feature-isolated profiles — see §10, the native-profile finding |
| The lean profile's binary footprint does not increase (§13, §15) | `qualification/embedded/eggfetch-min` rebuilt from the same source at the baseline `873ea285…` and on the frozen commit `26fa6192…` with the lean feature set `native-http1,high-level-url,tls-rustls`: unstripped 8 555 504 B → 8 551 576 B, stripped 3 576 216 B → 3 573 128 B | Met for the binary | The total lean binary **shrank** by 3 928 B unstripped and 3 088 B stripped. `eggfetch_core`'s own `.text` grew from 232.7 KiB to 242.9 KiB (+10.2 KiB). The growth is **not attributable to M009 alone** — the frozen commit carries all eight milestones, and a tree carrying only M009 is not constructible from the bundled commit; see §3 and §10, the footprint finding |
| No public API changed in any profile (§13) | The Rust public-API oracle on the bound SHA `26fa6192…`: passed in eight profiles, with `cargo-semver-checks` 0.49.0 reporting "no semver update required" against the planning baseline `03ecba97…` | Met | `pipeline/hop.rs` is a private module, so no public surface is expected to change, and the oracle — the authority — confirms it |
| Tier 1 and Tier 2 are green on the frozen executable SHA (§13) | `./scripts/check.sh` and `./scripts/check.sh extended` on `26fa6192…` — green | Met | Tier 3 (`./scripts/check.sh package`) and the live security preflight are green on the same SHA; see §10, the tier-gate finding |
| Both HTTPX profiles are rebound to that exact SHA (§13) | Both HTTPX profiles and the ledger's Stage C SHA rebound to `26fa6192…`, and the compat qualification was re-run against it | Met | The binding that this milestone invalidated was re-established at the bound SHA; see §7 and §10, the Stage C rebinding finding |

## 3. Production implementation evidence

**New shared builder.** `crates/eggfetch-core/src/pipeline/hop.rs` is 563 lines
and is declared in `crates/eggfetch-core/src/pipeline/mod.rs` as

```rust
#[cfg(feature = "high-level-url")]
mod hop;
```

It is gated on `high-level-url` only and **not** on `redirects`, which is what
makes it usable from the lean path: `lean` requires `not(redirects)` and
`redirect` requires `redirects`, so the two are mutually exclusive at compile
time and a builder gated on `redirects` could not be shared.

The module's crate-private API is:

| Item | Line | Role |
|---|---|---|
| `pub(super) struct HopBuildParams` | 37 | The per-hop parameter set |
| `pub(super) fn prepare_first_hop(client, request) -> HopBuildParams` | 79 | The exhaustive `Request::into_parts()` destructuring, the header merge and the timeout merge |
| `pub(super) fn build_hop_request(client, params) -> Result<Request>` | 159 | The hop construction, transport-hint handling, cookie injection and auth resolution |
| `pub(super) fn update_cookie_jar(client, response)` | 255 | The `Set-Cookie` jar update |
| `pub(super) fn record_prepared_hop(request)` | 281 | The hook the in-crate equivalence fixture observes |
| `pub(crate) mod test_support` | 295 | The crate-private equivalence fixture |

`HopBuildParams` and `build_hop_request` are **not** new code: at the baseline
they already existed inside `redirect.rs` (lines 34 and 76), which is
`redirects`-gated and therefore unreachable from the lean path. The
milestone's contribution is to move them into an ungated module — so the lean
path can call them — and to add `prepare_first_hop` and `update_cookie_jar`
around them. The `SET_COOKIE` and `EXPECTED_COOKIE_HEADER` constants in
`test_support` are gated on `feature = "cookies"`.

**Duplication removed, not wrapped.** `crates/eggfetch-core/src/pipeline/lean.rs`
went from 121 lines at the baseline to 94 lines. Its header merge, timeout
merge, hop construction, cookie injection, auth resolution and `Set-Cookie` jar
update were deleted; the file is now a thin path that calls the shared builder.
`crates/eggfetch-core/src/pipeline/redirect.rs` had its duplicated `Set-Cookie`
update — which appeared **twice** in that file, once on the redirects-disabled
fast path and once per redirect hop — replaced by
`super::hop::update_cookie_jar(client, &response)`. `redirect.rs` retains the
exhaustive destructuring inside `advance_redirect_hop`, whose body is
otherwise unchanged from the baseline (the same comment and the same field
list, at a new line number because the file shrank above it).
`crates/eggfetch-core/src/pipeline/retry.rs` is untouched by this milestone.

**Exhaustiveness — the measured delta.** At the baseline there were four
exhaustive `RequestParts` destructurings in `crates/eggfetch-core/src`:

| File | Baseline | Current |
|---|---|---|
| `pipeline/prepare.rs` | 306 | 304 (unchanged, `prepare_single_request`) |
| `pipeline/redirect.rs` | 247 and 293 | 106 (`advance_redirect_hop`, unchanged) |
| `pipeline/lean.rs` | 27 | — (deleted) |
| `pipeline/hop.rs` | — | 82 (new, the extracted builder) |

The three remaining sites each list every `RequestParts` field explicitly with
no rest pattern. The plan's §6.2 requires `advance_redirect_hop` to remain a
destructuring site, and `prepare.rs`'s `prepare_single_request` destructuring
was pre-existing and outside the blocks §6.3 lists for deletion. The plan's
§13 criterion and its §6.2 wording were therefore mutually inconsistent once
the tree was read literally; §13 was amended on 2026-10-23 to state the
first-hop scoping explicitly, and the plan and this record now agree (§10,
the "exactly one exhaustive destructuring" finding, resolved). What the tree
satisfies is that the **first-hop preparation** is single-sourced: one
builder, one header merge, one timeout merge, one cookie injection, one auth
resolution, one `Set-Cookie` update.

**Test-target gating.** Thirteen test targets under `crates/eggfetch-core/tests/`
that could not compile under the lean profile received a file-level
`#![cfg(...)]` gate. The gate states the feature requirement of that file's own
content, not a blanket profile check:

| Target | File-level gate |
|---|---|
| `adverse_tests.rs` | `all(feature = "redirects", feature = "logical-retry")` |
| `custom_dialer_tests.rs` | `all(feature = "advanced-routing", feature = "logical-retry")` |
| `direct_transport_tests.rs` | `all(feature = "advanced-routing", feature = "redirects")` |
| `integration.rs` | `feature = "redirects"` |
| `native_tower_service_tests.rs` | `feature = "redirects"` |
| `network_stream_tests.rs` | `all(feature = "advanced-routing", feature = "high-level-url")` |
| `redirect_strict_tests.rs` | `feature = "redirects"` |
| `request_failure_tests.rs` | `all(feature = "advanced-routing", feature = "high-level-url")` |
| `resolved_route_cache_tests.rs` | `all(feature = "advanced-routing", feature = "redirects")` |
| `retry_integration.rs` | `all(feature = "logical-retry", feature = "redirects")` |
| `streaming_decompression_tests.rs` | `any(compression-gzip, compression-brotli, compression-zstd, compression-deflate)` |
| `total_body_deadline_tests.rs` | `feature = "redirects"` |
| `transport_metrics_tests.rs` | `all(feature = "advanced-routing", feature = "high-level-url")` |

Each gated file also carries a comment naming the API it exercises and why the
lean profile compiles it out. `streaming_decompression_tests.rs` is the codec
case: without a file-level gate every test in that file is stripped under the
lean profile and the file compiles with an entirely dead body.

`tls_integration.rs` and `tls_response_completeness.rs` are **not** in this
list. Both were already file-level gated at the baseline (`#![cfg(feature =
"tls-rustls")]`) and needed no change for this milestone.

**Attribute order.** In the test tree, 26 of the 27 files that carry both an
`#![allow(...)]` block and a file-level `#![cfg(...)]` attribute now place the
allow block first. The one exception is the new
`crates/eggfetch-core/tests/hop_equivalence_support.rs`, which keeps
`#![cfg(feature = "high-level-url")]` above `#![allow(dead_code)]`. That file
is both a standalone integration-test target in its own right (cargo compiles
every `tests/*.rs` file as its own target) and a `#[path]`-included module in
the two equivalence targets; its `cfg` is satisfied in every profile that
compiles it, so the ordering is benign there (§10, the `hop_equivalence_support.rs` finding). The order is
load-bearing: when `#![cfg(false)]` strips a crate, every attribute below it is
stripped too, so an `#![allow(missing_docs)]` placed below the `cfg` never
applies and the emptied crate emits "missing documentation for the crate".
`native_json.rs` was cfg-first at the baseline and is allow-first now.

**Two inline unit tests.** `crates/eggfetch-core/src/request.rs`
(`request_builder_validates_resolved_target_port`) and
`crates/eggfetch-core/src/client.rs` (`resolved_route_cache_is_bounded`) use
`advanced-routing`-only APIs and received a matching `#[cfg(feature =
"advanced-routing")]` attribute.

**Manifest.** `crates/eggfetch-core/Cargo.toml` gained

```toml
[[example]]
name = "quickstart"
required-features = ["redirects", "basic-auth"]
```

so the `quickstart` example is excluded from the lean-profile `--all-targets`
compile instead of failing it.

**Tier 1 gate.** `scripts/check.sh` gained

```sh
tier1_lean_profile_compile() {
    info "Lean profile whole-suite compile"
    # … the bare RUSTFLAGS= prefix overrides CI's -D warnings …
    RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --all-targets
}
```

called from `run_tier1` immediately after `tier1_rust_api_contracts`. The bare
`RUSTFLAGS=` prefix matches the existing contract matrix: the lean and native
profiles retain private helpers for other supported profiles, and CI's global
`-D warnings` turns profile-specific dead-code warnings into errors. The gate
**compiles** the suite; it does not run it.

**Equivalence fixture.** The fixture is a shared Rust expectation module, not a
snapshot-artifact comparison:
- `crates/eggfetch-core/tests/hop_equivalence_support.rs` — the shared
  expectation (`expected_request()`) and the wire-shape assertion, included by
  both targets through `#[path]`.
- `crates/eggfetch-core/tests/lean_path_equivalence.rs` — gated
  `#![cfg(all(feature = "high-level-url", not(feature = "redirects")))]`.
- `crates/eggfetch-core/tests/redirect_path_equivalence.rs` — gated
  `#![cfg(all(feature = "high-level-url", feature = "redirects"))]`.

Because the lean and redirect paths are mutually exclusive at compile time, the
fixture is split into two separately-compiled targets, each asserting against
the same shared expectation. The comparison covers method, URL, headers,
timeout, body, auth and transport hints, and the post-response cookie-jar
state. The same properties are also asserted in-crate by
`pipeline::hop::test_support`, which compares the prepared hop field by field
and checks the jar state through `EXPECTED_COOKIE_HEADER`.

**Footprint.** The plan's §13 makes "the lean profile's binary footprint does
not increase" an acceptance criterion and §15 lists the before/after pair as
required closure evidence. It was measured with the repository's own recorded
method (`plans/high-level-policy-footprint-feature-boundary.md`, §"Footprint
measurement"): the `qualification/embedded/eggfetch-min` fixture, release
profile `lto = "thin"`, `codegen-units = 1`, `strip = false`, `debug = false`,
`panic = "unwind"`, and the lean feature set
`native-http1,high-level-url,tls-rustls`. The fixture was rebuilt twice from
the same source — once against `crates/eggfetch-core` at the baseline SHA in a
detached worktree, once against `crates/eggfetch-core` on the current tree —
in two scratch directories outside the repository, so nothing in the working
tree was modified. The result is in §4 and in §10, the footprint finding.

**Footprint re-measurement at the frozen commit (2026-10-23).** The pair was
re-run on the frozen commit `26fa619299fb8d447f25cbcb5eb713a1f537a30c` against
the same baseline `873ea28541c82817317430c9523fca8525d99c81`, with the same
method: two detached worktrees, two scratch copies of the fixture outside the
repository with the path dependency repointed at each worktree's
`crates/eggfetch-core`, the same lean feature set, and an isolated
`CARGO_TARGET_DIR` per side. The fixture's `Cargo.lock` is byte-identical at
the two SHAs, so one lock isolates the core-source difference. Sizes came from
`stat -c %s` on the binary and on an explicit `strip` copy; the per-crate
`.text` came from `cargo bloat --release --crates`.

| Measurement | Baseline `873ea285…` | Frozen `26fa6192…` | Delta |
|---|---|---|---|
| Fixture binary, unstripped | 8 555 504 B | 8 551 576 B | −3 928 B |
| Fixture binary, stripped | 3 576 216 B | 3 573 128 B | −3 088 B |
| `eggfetch_core` `.text` | 232.7 KiB | 242.9 KiB | +10.2 KiB |
| `hyper_util` `.text` | 133.1 KiB | 131.7 KiB | −1.4 KiB |
| `http` `.text` | 48.1 KiB | 47.1 KiB | −1.0 KiB |

The baseline side reproduces the previously recorded figures (stripped
3 576 216 B, `eggfetch_core` 232.7 KiB, `hyper_util` 133.1 KiB, `http` 48.1 KiB);
the unstripped size differs from the recorded 8 555 488 B by 16 B. The frozen
side was rebuilt into a second target directory and came out byte-identical
(sha256 `9adf5cc0…`), reproducing 242.9 KiB, so the measurement is
deterministic. Toolchain: `rustc 1.99.1 (58cf8f9c3 2026-10-13)`.

The option the finding offered first — a tree carrying **only** M009 — is not
constructible. M009's changes were bundled into `d6c41adf…` with seven other
milestones, and `crates/eggfetch-core/src/pipeline/` between the baseline and
that commit also carries M010's `custom_dialing.rs` and `hyper_dispatch.rs`
plus changes to `finalize.rs`, `h3_dispatch.rs`, `proxy_dispatch.rs` and
`retry.rs`, so a faithful "only M009" tree cannot be reconstructed from the
committed history. The +10.2 KiB `eggfetch_core` `.text` growth is therefore
cumulative across the whole freeze. §13's binary criterion is met at the
frozen SHA; the per-crate attribution to M009 is not establishable from the
tree and is not claimed.

## 4. Verification executed

### Commands run

The working tree is shared with another agent that was editing
`crates/eggfetch-core/src/**` throughout this closure pass. Every measurement
below is therefore pinned to a tree fingerprint, and the two snapshots taken
five minutes apart are reported separately rather than merged.

```sh
# tree fingerprint used by the measurements
git rev-parse HEAD                              # 873ea28541c82817317430c9523fca8525d99c81
git diff | sha256sum                            # worktree fingerprint

# lean-profile whole-suite compile (M009's own gate)
RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --all-targets

# the sibling lean profile, and the native profile that still fails
RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features standard-http2,tls-rustls --all-targets
RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features native-http1,tls-rustls --all-targets

# lean behavioral suites
RUSTFLAGS= cargo test -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --test lean_policy_tests -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --test lean_route_tests -- --test-threads=1

# equivalence fixtures
cargo test -p eggfetch-core --test redirect_path_equivalence --all-features -- --test-threads=1
RUSTFLAGS= cargo test -p eggfetch-core --test lean_path_equivalence --no-default-features --features standard-http1,tls-rustls -- --test-threads=1
cargo test -p eggfetch-core --lib --all-features -- --test-threads=1 pipeline::

# lint gate on the crate
cargo clippy -p eggfetch-core --all-targets --all-features -- -D warnings

# baseline "before" state, measured in a detached worktree at the baseline SHA
git worktree add --detach /tmp/eggfetch-baseline-wt 873ea285…
cd /tmp/eggfetch-baseline-wt
RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --all-targets
git worktree remove --force /tmp/eggfetch-baseline-wt

# exhaustiveness delta, baseline vs current
git show 873ea285…:crates/eggfetch-core/src/pipeline/{prepare,redirect,lean}.rs | grep -n "RequestParts {"
grep -n "RequestParts {" crates/eggfetch-core/src/pipeline/{prepare,redirect,hop}.rs

# lean-profile footprint, baseline vs current, using the repository's own fixture
# (two scratch copies of qualification/embedded/eggfetch-min outside the repository,
#  each pointing at a different eggfetch-core; the baseline one at the worktree)
cargo build --release          # in /tmp/fpfx-base and /tmp/fpfx-cur
strip <binary>                 # a stripped copy for the second size
cargo bloat --release --crates # the eggfetch_core .text figure
```

### Results (pass/fail/skip with reason; counts without concealment)

Snapshot A — 2026-10-22 17:14:04Z, `HEAD=873ea285…`,
`git diff | sha256sum = e573c729bbb78f2a5fa7729280fa41dc1591d7403d2142605ff1e7c6ad1b1f4a`:

| # | Invocation | Result |
|---|---|---|
| 1 | lean whole-suite `cargo check --all-targets` | exit 0; **0** `missing documentation for the crate` warnings; 23 lib warnings and 17 lib-test warnings (duplicates), all of them the pre-existing profile-specific dead-code / unused-import category |
| 2 | `cargo clippy -p eggfetch-core --all-targets --all-features -- -D warnings` | exit 0 |
| 3 | `redirect_path_equivalence` (`--all-features`) | 2 passed, 0 failed |
| 4 | `lean_path_equivalence` (lean profile) | 2 passed, 0 failed |
| 5 | `cargo test -p eggfetch-core --lib --all-features -- --test-threads=1 pipeline::` | 21 passed, 0 failed, 811 filtered out |

The sibling lean profile `standard-http2,tls-rustls` has the same feature
shape as the gated one (`transport-http2` + `standard-route` +
`high-level-url`) and was also checked: `RUSTFLAGS= cargo check -p eggfetch-core
--no-default-features --features standard-http2,tls-rustls --all-targets` exits
0 with 0 errors and 0 `missing documentation for the crate` warnings. It is
not itself a Tier 1 gate.

The **native** profile does not compile the whole suite, before or after this
milestone:

| # | Invocation | Result |
|---|---|---|
| 6 | `RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features native-http1,tls-rustls --all-targets` on the current tree | **exit 101**, 93 errors — 70 × `E0599` "no method named `get` found for struct `Client`", 11 × the same for `eggfetch_core::Client`, 2 × "no method named `request`", 2 × "no method named `post`". **Resolved after this snapshot** by the main agent's native-profile gating extension; see §10, the native-profile finding |
| 7 | The same command at the baseline SHA, in the detached worktree | **exit 101**, 174 errors |

`native-http1` is `transport-http1` + `standard-route` + `advanced-routing` and
does **not** carry `high-level-url`, so the high-level `Client::get` /
`Client::post` / `Client::request` API does not exist in that profile while the
test targets that use it are not gated against it. This is the same class of
gap M009 closed for the lean profile, in a profile that is not a Tier 1
`--all-targets` gate. See §10, the native-profile finding.

Baseline "before" state, in a detached worktree at `873ea285…`:

| # | Invocation | Result |
|---|---|---|
| 8 | lean whole-suite `cargo check --all-targets` at the baseline | **exit 101**, 62 errors: 47 `E0599` (methods such as `ClientBuilder::follow_redirects` and `RequestBuilder::resolved_addresses` absent in this profile), 3 `E0432` (unresolved imports of `Dialer`, `RetryPolicy`, `RedirectPolicy`), 1 `E0425`, 1 `E0433`, 1 `E0308`. Cargo named 7 of the failing targets — `redirect_strict_tests`, `request_failure_tests`, `resolved_route_cache_tests`, `retry_integration`, `tls_integration`, `tls_response_completeness`, `total_body_deadline_tests` — before it stopped scheduling the rest. The plan and the M008 closure record both record the full failing set as thirteen |

The baseline worktree was removed afterwards; `git worktree list` shows only the
main tree.

Snapshot B — 2026-10-22 17:19:36Z,
`git diff | sha256sum = 75407191669a2a3e4f1a29c7cd098d086a8a09d4db6d77217f20f2d14b892011`:

| # | Invocation | Result |
|---|---|---|
| 9 | lean whole-suite `cargo check --all-targets` | **exit 101**, 2 × `E0308` in `crates/eggfetch-core/src/pipeline/prepare.rs:337` and `:345` (`validate_url_effective_port` expected `&Url`, found `Url`) |
| 10 | `cargo clippy -p eggfetch-core --all-targets --all-features -- -D warnings` | **exit 101** (same two errors) |
| 11 | `lean_policy_tests`, `lean_route_tests` (lean profile) | **exit 101** (same two errors) |

Snapshot B's failure is **not** attributable to M009. `http_origin.rs` was
modified at 17:16:37 and `pipeline/prepare.rs` at 17:18:04, i.e. inside the
measurement window, by a concurrent edit to
`validate_url_effective_port`'s signature. Eight core source files were
modified after 17:14, including `pipeline/prepare.rs` and `pipeline/mod.rs`,
which are M009 files. The M009-owned change set therefore cannot be measured in
isolation on the shared tree at the time of writing; snapshot A is the
measurement of the milestone's own code, taken when the tree compiled.

Snapshot C — 2026-10-22 17:35:45Z,
`git diff | sha256sum = ac3a552c5c33c5a94d7cfa1502ebaedd07e10cd473b02ed9bbd9a706dc402ed0`,
after the concurrent edit was completed:

| # | Invocation | Result |
|---|---|---|
| 12 | `lean_policy_tests` (lean profile) | 6 passed, 0 failed, 0 ignored |
| 13 | `lean_route_tests` (lean profile) | 10 passed, 0 failed, 0 ignored |

These are the two lean behavioral suites the plan's §11 lists, and they cover
the §13 "lean profile's behavior is unchanged" list: 503 single attempt, 302
returned with an empty history, Bearer header plus redaction, and the typed
refused / timeout / body-cap failures.

Lean-profile footprint, baseline vs current tree. The `eggfetch-min` fixture
was rebuilt from the same source in two scratch directories outside the
repository, one against `crates/eggfetch-core` at the baseline SHA and one
against the current tree (fingerprint
`5b4a57ebec9cf18030f0070bb061048b385b1d3a9dba892c4ac80069bc91a7fb`,
2026-10-22 17:23:10Z), with the lean feature set
`native-http1,high-level-url,tls-rustls`:

| Metric | Baseline `873ea285…` | Current tree | Delta |
|---|---|---|---|
| Fixture binary, unstripped | 8 555 488 B | 8 550 648 B | **−4 840 B** |
| Fixture binary, stripped | 3 576 216 B | 3 572 480 B | **−3 736 B** (−0.10 %) |
| `cargo bloat --crates`, `eggfetch_core` `.text` | 232.7 KiB | 242.3 KiB | **+9.6 KiB** (+4.1 %) |
| `cargo bloat --crates`, `hyper_util` `.text` | 133.1 KiB | 131.7 KiB | −1.4 KiB |
| `cargo bloat --crates`, `http` `.text` | 48.1 KiB | 47.1 KiB | −1.0 KiB |

The total lean binary did **not** increase; it shrank slightly. The
`eggfetch_core`-owned `.text` grew by 9.6 KiB while `hyper_util` and `http`
shrank. The measurement is a before/after over the **whole** current tree,
which carries other milestones' changes as well as M009's, so it does not
isolate M009's own contribution; the plan's recorded reference figures
(8 605 056 B unstripped, 3 600 816 B stripped, 236.9 KiB `.text`) were taken
on `rustc 1.98.1` and are not directly comparable with these absolute
numbers, which are on `rustc 1.99.1`.

Out of scope at the time of the milestone's own verification, and since run on
the bound SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c`:

- `./scripts/check.sh` (Tier 1), `./scripts/check.sh extended` (Tier 2) and
  `./scripts/check.sh package` (Tier 3) — the targeted equivalents in the
  table above were the substitute evidence at the time; the full gates are
  green on the bound SHA (§10).
- The full workspace test suite (`cargo test --workspace --exclude
  eggfetch-python --all-features`) — green on the bound SHA: 1767 passed,
  0 failed, 0 ignored.
- The Python HTTPX compat pytest suites — green on the bound SHA: 2073
  passed, 0 failed.
- The Rust public-API oracle (`scripts/check_rust_public_api.py`) — green on
  the bound SHA: passed in eight profiles, with `cargo-semver-checks` 0.49.0
  reporting "no semver update required" against the planning baseline
  `03ecba97…`.
- The MSRV gate — green on the bound SHA, on the exact Rust 1.89.0
  toolchain (five checks).
- The default-profile execution of the thirteen newly gated targets —
  covered by the `--all-features` core suite on the bound SHA.

Every cargo invocation used the shell assignment prefix (`RUSTFLAGS= cargo …`).
The `env` binary on this machine is a shim that ignores its arguments, so
`env RUSTFLAGS= cargo …` succeeds without running `cargo` at all; this hazard is
recorded in the M008 closure record and was avoided here.

## 5. Invariant review

- **The `RUSTFLAGS=` rationale is preserved.** The new Tier 1 gate uses the bare
  prefix, matching the existing contract matrix. Profile-specific dead code is
  still reported as a warning; it is only CI's `-D warnings` that would turn it
  into an error.
- **`--test-threads=1` is preserved** on every cargo test invocation, per the
  repository rule that workspace Rust tests are never parallelized.
- **The lean path's behavior, defaults and footprint target were not changed.**
  The shared builder performs the same operations in the same order: cookie
  injection still happens after the hop's headers are set and before dispatch,
  and the `Set-Cookie` update still happens after the response is received and
  before the response is returned.
- **No second timeout owner and no second pool admission point** was
  introduced; the shared builder only merges the client timeout with the
  request timeout into the hop request.
- **The `retry_request` and `advance_redirect_hop` typed rebuild helpers were
  not touched.** They remain the only places a *subsequent* hop request is
  constructed.
- **No public API surface was added.** `pipeline/hop.rs` is a private module;
  `HopBuildParams`, `build_hop_request` and `test_support` are crate-private.
  The oracle that confirms this ran on the bound SHA and reported no delta in
  any of the eight profiles (§2).
- **No cargo feature was added, removed or renamed.** The `[[example]]` entry
  adds a `required-features` declaration for the existing `redirects` and
  `basic-auth` features.
- **No dependency changed.** `Cargo.lock` is untouched by this milestone.
- **No CI job, matrix or workflow was added.** `.github/workflows/` still holds
  `ci.yml` (one job, `ubuntu-latest`) and the pre-existing manually dispatched
  `pypi.yml`.
- **The lean profile still exists for its recorded footprint reason.** The
  decision in `plans/high-level-policy-footprint-feature-boundary.md` was not
  reopened; this milestone removes the duplication that decision caused.

## 6. Failure, timeout, pool, and recovery review

No new request path, transport, timeout, pool or recovery logic was added. The
shared builder is a pure refactor of existing preparation code: the same
header merge, the same timeout merge, the same cookie injection, the same auth
resolution, the same `Set-Cookie` update, in the same order, from the same
inputs.

The one behavioral surface the milestone could have disturbed is the
divergence between the lean path and the redirect path. That is what the two
equivalence targets exist to catch, and both passed (2 passed each).

One operational hazard is carried forward because it can silently invalidate
future verification in this environment: `env` on `PATH` resolves to
`/home/sugarwookie/.local/bin/env`, a shim that ignores its arguments and only
prepends `PATH`. An invocation of the form `env RUSTFLAGS= cargo …` therefore
succeeds without running `cargo` at all and produces a false pass. Every
result in §4 comes from the shell assignment prefix form, which is also how
`scripts/check.sh` is written.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

Supported core profiles and their gates after this milestone:

| Profile | Contract fixtures | Behavioral suite | Whole-suite compile | Public-surface snapshot |
|---|---|---|---|---|
| default | Tier 1 | yes | — | `default.txt` |
| http1 + tls-rustls | Tier 1 | yes | — | `http1-tls-rustls.txt` |
| http2 + tls-rustls | Tier 1 | yes | — | `http2-tls-rustls.txt` |
| standard-http1 + tls-rustls | Tier 1 | yes (`lean_policy_tests`, `lean_route_tests`) | **Tier 1 (new)** | `standard-http1-tls-rustls.txt` |
| standard-http2 + tls-rustls | Tier 1 | yes | — | `standard-http2-tls-rustls.txt` |
| native-http1 + tls-rustls | Tier 1 | yes | — | `native-http1-tls-rustls.txt` |
| native-http2 + tls-rustls | Tier 1 | yes | — | `native-http2-tls-rustls.txt` |
| all features | Tier 1 | yes | — | `all-features.txt` |

**Exact-SHA binding.** This milestone changed Rust sources, test targets,
`crates/eggfetch-core/Cargo.toml` and `scripts/check.sh` — all executable
inputs under `.skills/verification-qualification.md` — and therefore
invalidated the then-live Stage C binding
`42a9c96db5b25354843bd83bd458c71757e95651`
(`compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml`,
and the ledger's Stage C entry in
`plans/httpx-parity-correction-status.md`), which is an ancestor of the
baseline `873ea285…` but did not cover the milestone's changes. The binding
has since been re-established at the frozen SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c`, which carries this milestone's
implementation in the freeze commit
`d6c41adf4cf8f62ef990cccedf2f86512858fa0b`. Both HTTPX profiles and the
ledger's Stage C SHA point at that SHA, and the frozen-SHA qualification is
green (§10). The rebinding establishes a compatibility binding only: it
authorizes no publication, and HTTP/3 remains experimental.

**Lean-profile containment.** The lean profile
(`standard-http1,tls-rustls`) is a Bearer-only, single-attempt, 3xx-returned-
without-following client. This milestone does not change that contract. The
new shared builder references no `redirects`-gated item, and `hop.rs` compiles
under the lean profile — which is exactly what the new Tier 1 gate now checks
on every run.

**Documentation consistency.** The stale statement this milestone surfaced
outside its documentation set was corrected in the same coordinated pass:
`docs/architecture/build-ci.md`'s Tier 1 table now carries the
"Rust feature-isolated profile whole-suite compile" row, and the parenthetical
that said the lean gate "is not widened to `--all-targets`, because the
remaining core test targets do not compile under the lean profile
(core-transport-policy M009)" is gone. See §10.

## 8. Security review

No security-relevant behavior changed. The shared builder moves existing code
without changing it. No trust store, certificate handling, credential, redaction,
proxy, cookie or TLS configuration was touched, and no new dependency was
introduced.

The auth block is one of the blocks that moved. It remains
`#[cfg(feature = "cookies")]`-gated in the same way as before, resolves the
same way as before, and applies to the same requests; the redirect path's
manual `Authorization` handling for a same-origin redirect hop is untouched in
`advance_redirect_hop`. No `authorization` / `cookie` value is logged, printed
or embedded in an error message by the new code.

## 9. Documentation and operations

| File | Change |
|---|---|
| `docs/architecture/core-engine.md` | The pipeline phase ownership is updated to name the shared `pipeline/hop` builder as the owner of first-hop preparation, and the lean path's relationship to the redirect path is restated |
| `docs/architecture/core-timeout-pool.md` | The preparation builder's timeout ownership is described against the shared builder rather than the lean path |
| `docs/architecture/feature-flags.md` | The lean profile's supported scope and its test coverage are updated to reflect that the whole suite now compiles under it and that Tier 1 gates it |
| `docs/architecture/core-auth-redirect-retry.md` | The single preparation builder and the exhaustive `Request::into_parts()` destructuring are documented |
| `docs/architecture/overview.md` | Four places that described the lean path as duplicating the hop builder are corrected |

Operations: Tier 1 gains one cargo invocation, a `cargo check` of the whole
`eggfetch-core` test suite under the lean profile. It is a compile, not a run,
so it adds no test runtime; it does add compile time for the lean feature set on
every Tier 1 run. No new external dependency, no network access and no
credential is required.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| Resolved | The live Stage C binding (`42a9c96d…`) was invalidated by this milestone's Rust-source, test-target, manifest and validation-script changes, and the Stage C rebinding had not been performed at the time of implementation | The previous binding could not be cited by any publication | **Resolved 2026-10-22.** Both HTTPX profiles and the ledger's Stage C SHA were rebound to `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, and the frozen-SHA qualification was re-run |
| Resolved | Tier 1, Tier 2, and Tier 3 had not been run on the frozen SHA at the time of implementation, and neither had the Rust public-API oracle or the full workspace suite | The repository-wide gates and the public-surface oracle were unproven for this tree | **Resolved 2026-10-22.** Tier 1, Tier 2, and Tier 3 are green on `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, together with the live security preflight; the public-API oracle passed in eight profiles and the full workspace suite reported 1767 passed, 0 failed, 0 ignored |
| Resolved | The plan's §13 criterion "exactly one exhaustive `Request::into_parts()` destructuring exists in the engine's preparation path" is not met as literally worded. Three exhaustive destructurings remain in `crates/eggfetch-core/src`: `pipeline/hop.rs:82`, `pipeline/prepare.rs:304` and `pipeline/redirect.rs:106`. The baseline had four. Because all three list every field with no rest pattern, a newly added `RequestParts` field would fail to compile at each of the three sites, not at exactly one. The implementer reported a temporary experiment that produced exactly one compile error; that experiment does not reproduce against the tree as it stands and was reverted, so no reproducible artifact exists | The criterion's stated purpose — "a new request field must produce exactly one compile error when it is not threaded through, not one per copy" — is not achieved at the level of the whole engine. The *first-hop* duplication the milestone targeted is gone, and the two remaining sites are the two the plan's §6.2 requires to remain, so the plan's §13 and §6.2 are mutually inconsistent as written | **Resolved 2026-10-23.** The first option above was taken: the plan's §13 criterion now reads "exactly one exhaustive destructuring in the **first-hop preparation** path, with `prepare_single_request` and `advance_redirect_hop` remaining as the two transport- and subsequent-hop sites", so §13 and §6.2 now agree. No Rust source changed, and the reverted temporary experiment is still not a reproducible artifact and is not claimed as one. The three exhaustive destructurings are unchanged — `pipeline/hop.rs:82` (`prepare_first_hop`), `pipeline/prepare.rs:304` (`prepare_single_request`) and `pipeline/redirect.rs:106` (`advance_redirect_hop`) — and each still lists every `RequestParts` field with no rest pattern, so a newly added field fails to compile at all three sites |
| Resolved | The working tree was under concurrent modification by another agent throughout the milestone's implementation pass. Snapshot A (17:14:04Z) is green on every M009 check; snapshot B (17:19:36Z) fails to compile on two `E0308` errors in `pipeline/prepare.rs` caused by a concurrent change to `validate_url_effective_port`'s signature in `http_origin.rs`. Eight core source files changed after 17:14 | The §4 measurements were pinned to tree fingerprints rather than to a commit, so M009's own change set could not be isolated from the concurrent edits at the time | **Closed by the freeze.** The eight milestones were committed together as `d6c41adf4cf8f62ef990cccedf2f86512858fa0b`, and the frozen-SHA qualification (Tier 1 / Tier 2 / Tier 3) ran green on `26fa6192…`, re-running the workspace suite and the feature-isolated profile compiles against one immutable tree. The footprint measurement below was closed separately on 2026-10-23 by a re-measurement on the frozen commit |
| Resolved | The thirteen newly gated targets were compiled under the lean profile but were not executed under the default profile during the milestone's own pass | The plan's §13 requires both halves ("compile under the lean profile **and** still pass under the default profile"). The compile half was measured; the pass half was outstanding | **Resolved 2026-10-22.** The full `--all-features` core suite ran on the bound SHA `26fa6192…` as part of Tier 1 — 1767 workspace tests passed, 0 failed, 0 ignored — which executes the thirteen targets under the default profile |
| Resolved | The lean profile's binary footprint was measured before and after, but not **isolated to M009**: the total lean binary did not increase (unstripped −4 840 B, stripped −3 736 B) while `eggfetch_core`'s own `.text` grew by 9.6 KiB, over a tree that carries other milestones' changes as well | The plan's §13 criterion "the lean profile's binary footprint does not increase" is met for the binary as measured. Whether any part of the 9.6 KiB `.text` growth is M009's own is not established | **Resolved 2026-10-23.** The second option was taken: the `eggfetch-min` footprint pair was re-run on the frozen commit `26fa6192…` against the same baseline `873ea285…`, using the repository's own recorded method — the `qualification/embedded/eggfetch-min` fixture, release profile `lto = "thin"`, `codegen-units = 1`, `strip = false`, `debug = false`, `panic = "unwind"`, lean feature set `native-http1,high-level-url,tls-rustls`, and `cargo bloat --release --crates` for the per-crate `.text`. Baseline: unstripped 8 555 504 B, stripped 3 576 216 B, `eggfetch_core` `.text` 232.7 KiB. Frozen: unstripped 8 551 576 B, stripped 3 573 128 B, `eggfetch_core` `.text` 242.9 KiB. The lean binary **shrank** by 3 928 B unstripped and 3 088 B stripped, so §13's criterion is met at the frozen SHA, and the frozen side was confirmed deterministic by a rebuild into a second target directory that produced a byte-identical binary. The **attribution** question is answered as far as the tree allows: the first option — a tree carrying only M009 — is **not constructible**, because M009's changes were bundled into `d6c41adf…` with seven other milestones, and `crates/eggfetch-core/src/pipeline/` between the baseline and that commit also carries M010's `custom_dialing.rs` and `hyper_dispatch.rs` plus changes to `finalize.rs`, `h3_dispatch.rs`, `proxy_dispatch.rs` and `retry.rs`. The +10.2 KiB `eggfetch_core` `.text` growth is therefore cumulative across the whole freeze and is **not** attributable to M009 from the tree. §13's binary criterion is met; the per-crate attribution is not claimable and is not claimed |
| **Resolved after the closure pass (main agent)** | The **native** profile (`native-http1,tls-rustls`) could not compile the whole test suite: 93 errors on the snapshot-C tree, down from 174 at the baseline. `native-http1` carries `advanced-routing` but not `high-level-url`, so the high-level `Client::get` / `Client::post` API does not exist in that profile while the test targets that use it were not gated against it | The same class of gap M009 closed for the lean profile was open for the native profile, and because the native profile was not a Tier 1 `--all-targets` gate the gap was invisible to the gates. **Resolved**: six further test targets that are wholly high-level-dependent (`timeout_tests`, `trailer_tests`, `pool_tests`, `resource_stabilization`, `lean_policy_tests`, `lean_route_tests`) received a file-level `#![cfg(feature = "high-level-url")]` gate, the three high-level tests in `http_version_capability_tests` received a per-test gate (its native-frame test is deliberately left ungated so native coverage is not lost), and two inline unit tests in `crates/eggfetch-core/src/auth.rs` received the same gate. `RUSTFLAGS= cargo check -p eggfetch-core --no-default-features --features native-http1,tls-rustls --all-targets` now exits 0, as does the `native-http2` equivalent. The Tier 1 gate was renamed `tier1_profile_whole_suite_compile` and extended to all four feature-isolated profiles (`standard-http1`, `standard-http2`, `native-http1`, `native-http2`), all four of which now exit 0. **This is an extension beyond M009's stated §6.5 scope, which names the lean profile only**; it is recorded here because it closes the finding the record itself raised | None outstanding. The extension changes only test-target gating and the Tier 1 gate's command list; it changes no runtime behavior and no public surface |
| **Resolved after the closure pass (main agent)** | `crates/eggfetch-core/tests/hop_equivalence_support.rs` kept its `#![cfg(feature = "high-level-url")]` above its `#![allow(dead_code)]` — the only file in the test tree with that order | Benign today: the `cfg` is satisfied in every profile that compiles the module, so the allow is never stripped. It would have become a "missing documentation for the crate" source if the `cfg` were ever made false | **Done**: the allow block now sits above the `cfg`, matching the other test files. The file's file-level gate was also widened to name the feature it needs alongside the features it already named |
| Resolved | `docs/architecture/build-ci.md`'s Tier 1 table said the lean gate "is not widened to `--all-targets`, because the remaining core test targets do not compile under the lean profile (core-transport-policy M009)", and had no row for the whole-suite compile step | The build/CI reference was false about a gate M009 introduced | **Resolved 2026-10-22.** The Tier 1 table now carries the "Rust feature-isolated profile whole-suite compile" row and the M009 parenthetical is gone. The file is outside `plans/closure/` and was corrected in the coordinated freeze pass, not by this record |
| Low | The plan's §6.4 required a file-level gate that is "a file-level `#![cfg(...)]` attribute expressing the feature requirement of that file's content". Four of the thirteen gates are a bare single-feature gate (`feature = "redirects"`) and do not name the specific API the file exercises | The gate is still per-file and precise enough to hide nothing under the default profile, and every gated file carries a comment naming the API it exercises; but four files share the identical bare `redirects` gate | Acceptable as implemented; the per-file comment is what makes the requirement legible |

## 11. Roadmap disposition

`plans/subsystems/core-transport-policy-roadmap.md` milestone 9 moves from
`conditionally closed` to `closed`, with §11 (completion definition) and §12
(milestone status) updated in the same pass.

The milestone's own scope — the single-sourcing of the first-hop preparation,
the lean-profile compile coverage of the whole test suite, the new Tier 1 gate
and the equivalence fixture — is complete and measured, and the cross-cutting
release evidence (Tier 1 / Tier 2 / Tier 3 on the frozen SHA and the Stage C
rebinding) was delivered on the bound SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c`. The "exactly one exhaustive
destructuring" wording finding was closed on 2026-10-23 by the amendment of the
plan's §13 recorded in §10, and the footprint finding was closed the same day
by the frozen-commit re-measurement recorded in §3 and §10. **No finding in §10
remains scoped to this milestone.** What remains is the maintainer decisions
listed in `plans/README.md`.

The successor milestone (M010, core duplicate-helper consolidation) is not
blocked by this one; on the contrary, M009 removes the duplication M010 was
scoped to address.

## 12. Registry updates

`plans/registry.md`:

- The core-transport-policy row records milestone 9 as closed.
- The implementation-plan table for M009 points at this closure record.
- The blocked / operational work section no longer records the Stage C
  rebinding as pending for the milestones closed in this pass: it was
  delivered on the bound SHA `26fa6192…`, together with the Tier 1 / Tier 2 /
  Tier 3 run.
