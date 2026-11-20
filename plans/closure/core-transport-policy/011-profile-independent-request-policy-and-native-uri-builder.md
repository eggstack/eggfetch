# Core Transport Milestone 011 — Closure Status

Status: conditionally closed

Source implementation plan:

- `plans/implementation/core-transport-policy/011-profile-independent-request-policy-and-native-uri-builder.md`

Source subsystem roadmap:

- `plans/subsystems/core-transport-policy-roadmap.md#12-milestone-status`

Repository baseline reviewed: `db3a553b27f3c3a8a29b77446d7d46449c4be8b8`.

Implementation commits:

- `05f14f442dba697d7c140ead552e87f126ceac6b` — feat(core): make request
  policy and URI construction profile-independent. Carries #31, #32 and #33
  together with the eight regenerated oracle snapshots.
- `b55555ea5feb7abf2ec606f7aa44986c5e8f0c4d` — chore(oracle): advance the Rust
  public-surface baseline. Moves `BASELINE` and the oracle README to
  `05f14f44…`.
- `49f8910d45414b51937a28857e57c68bcf2413d7` — fix(ffi): declare the
  capabilities the C ABI always needs. Repairs the adapter regression that
  #31 exposed (§3, "The FFI regression"). **This is the executable freeze the
  Tier 1 result below was measured at.**

## 1. Executive finding

M011 is implemented, Tier 1 is green at the freeze, and the milestone is
**conditionally closed**. The condition is the exact-SHA Stage C requalification
that the milestone's own executable changes invalidated: Tier 2 and Tier 3 have
not been run at `49f8910d…`, and Tier 2's downstream gate cannot complete on
this checkout because `target/downstream-qualification/artifact-manifest.json`
has no in-repository producer.

The milestone delivers the three capability changes requested by
eggstack/eggfetch#31, #32 and #33:

- **#31** — core's `proxy` feature selects `native-http1` instead of the
  `http1` alias, so proxy support no longer compiles in
  `logical-retry`/`redirects`/`basic-auth`. Measured in an isolated consumer
  crate, the resolved set is now
  `advanced-routing, high-level-url, hyper-rustls, native-http1, proxy,
  standard-route, tls-rustls, transport-http1` — the three policy features are
  absent. Defaults and the `http1`/`http2` aliases are untouched.
- **#32** — `NativeRequestBuilder` constructs a request from an `http::Uri` and
  dispatches through the existing frame-preserving native path, so a
  `native-http1` consumer needs no `url`/`idna`/ICU closure.
- **#33** — `without_retry()` and `redirect_policy()` are callable in every
  profile, backed by an always-present `retry_disabled` flag and an
  always-present request-level `Option<RedirectPolicy>`.

Two things are **not** claimed. First, this record does not assert Tier 2 or
Tier 3. Second, the version is unchanged: advancing the oracle baseline is a
verification-baseline decision, not a release, and §7 records why the next
release carrying this change must be a minor bump.

## 2. Requirement-to-evidence matrix

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| #31: `proxy` does not pull the policy bundle | Isolated consumer probe (`default-features = false`, path dependency) resolved via `cargo tree -e features`; `crates/eggfetch-core/Cargo.toml:78` | Met | A standalone probe is required: inside the workspace, `cargo metadata`/resolver unifies the CLI and Python feature sets and cannot measure one crate's closure |
| #31: defaults and `http1`/`http2` aliases unchanged | `crates/eggfetch-core/Cargo.toml:19,45,46`; CLI/Python adapters still resolve the policy bundle | Met | `proxy` keeps its own `dep:base64` edge, so `Proxy-Authorization` encoding is unaffected |
| #32: `http::Uri` request construction without `high-level-url` | `crates/eggfetch-core/src/native_request.rs`; `tests/native_request_builder_tests.rs` (7 tests) passing in all eight contract profiles | Met as accepted | The issue's acceptance names `RequestBuilder`, which cannot exist in that profile; `NativeRequestBuilder` is the accepted resolution — see "Recorded decision" below |
| #32: existing `high-level-url` entry points keep behavior and signatures | `crates/eggfetch-core/tests/public_api_contracts.rs`; eight-profile oracle with `0` removed lines | Met | The issue's own compatibility constraint |
| #33: both setters available in every profile | `tests/request_policy_assertion_tests.rs` (4 tests) compiled and run in the lean, native, standard and all-features profiles | Met | Contract fixtures also ungated in `public_api_contracts.rs` |
| #33: setter semantics preserved where the feature is enabled | `without_retry()` → 1 attempt against a client retry policy; later `retry()` → 3 attempts; request redirect policy honored | Met | `without_retry()` still stores `Some(RetryPolicy::default())`, so the prior observable state is preserved |
| #33: inert (not failing) where the capability is absent | `tests/request_policy_assertion_tests.rs`, lean profile | Met | Absent capability means the setter is a semantic no-op |
| No public API removals in any profile | Eight-profile oracle diff: `564` added lines, `0` removed | Met | `cargo-semver-checks` 0.49.0 reports `no semver update required` |
| Tier 1 green at the freeze | `./scripts/check.sh` at `49f8910d…` — `CHECK_EXIT=0` | Met | 1828 Rust workspace tests, 0 failed; 541 Python passed, 40 skipped; 134 compatibility smoke passed |
| Tier 2 green at the freeze | not run | **Not met** | Requires the pinned `nightly-2026-05-07` oracle toolchain, the exact 1.89.0 MSRV toolchain, and an operator-supplied downstream artifact manifest |
| Tier 3 green at the freeze | not run | **Not met** | `./scripts/check.sh package` |
| FFI adapter keeps building in its non-default configurations | Eleven-configuration matrix measured against `db3a553b…`; `scripts/check_adapter_features.py` guard | Met | See §3, "The FFI regression" |

### Recorded decision — the #32 acceptance wording (2026-11-20)

Issue #32's acceptance criterion reads "a `RequestBuilder` can be constructed
from an `http::Uri` and dispatched" under
`--no-default-features --features native-http1,tls-rustls`. That literal
criterion cannot be satisfied as written: `pub mod request`, `pub mod
response`, and the `RequestBuilder`/`Response` re-exports are all gated on
`high-level-url` (`crates/eggfetch-core/src/lib.rs:50` and `lib.rs:99`), and
`Request` holds a `url::Url` with URL accessors on the public surface, so
ungating them is not a small edit.

The issue itself allows latitude — its proposed change permits "an equivalent
`RequestBuilder` constructor that takes an `http::Uri`" — and
`NativeRequestBuilder` satisfies the stated intent: a URL-free builder that
constructs from an `http::Uri` and dispatches in a `native-http1` profile.

**Maintainer decision: accept `NativeRequestBuilder` as the resolution of
#32.** No ungating of `request`/`response` is required, and #32's acceptance
criterion should be read as satisfied by the native builder rather than by the
type name. If the issue text is amended, the criterion becomes "a builder can be
constructed from an `http::Uri` and dispatched" without naming the type.

## 3. Production implementation evidence

### The feature graph (#31)

`crates/eggfetch-core/Cargo.toml`:

```toml
proxy = ["native-http1", "tls-rustls", "tokio/io-util", "high-level-url", "dep:eggfetch-http-connect", "dep:base64"]
```

`high-level-url` and `tls-rustls` were already implied by the old `http1`
alias and are still required by the proxy code; only the policy triple moved
out. No file under `proxy.rs`, `transport/proxy.rs` or
`pipeline/proxy_dispatch.rs` refers to `logical-retry`, `redirects` or
`basic-auth`.

Six tests in `crates/eggfetch-core/tests/cross_feature_tests.rs` were
proxy tests that relied on redirect or Basic-auth behavior; each gained a
per-test `#[cfg(…)]` gate (`redirects`, `all(redirects, basic-auth)`,
`all(cookies, redirects)`) so the file compiles in the policy-bundle-free
proxy profile.

### The native builder (#32)

`NativeRequestBuilder` exposes `header`, `headers`, `body`, `bytes`,
`version`, `timeout`, `transport_hints`, `without_retry`,
`redirect_policy`, `build() -> Result<http::Request<RequestBody>>` and
`send() -> Result<http::Response<NativeResponseBody>>`. `Client::get_uri`,
`post_uri` and `request_uri` construct it, validating the origin eagerly
through `HttpOrigin::from_uri`. `RequestBody` gained an
`http_body::Body` implementation so the native path accepts it without
exposing `http-body-util` types publicly.

The design is deliberately a separate type rather than a second constructor on
`RequestBuilder`: the issue's literal proposal (a `get_uri` returning a
`RequestBuilder`) would have forced either a union return type on `send()` or
a breaking change to `Request::url()`/`Response::url()`, both barred by the
issue's own "existing `high-level-url` entry points keep their current
behavior and signatures" constraint.

### The profile-independent disable switches (#33)

`redirect.rs` is ungated (its URL helpers were already individually gated on
`high-level-url`), so `RedirectPolicy` and `RedirectDowngradePolicy` exist in
every profile. `Request`/`RequestBuilder` carry an always-present
`redirect: Option<RedirectPolicy>` and an always-present `retry_disabled: bool`;
`pipeline/retry.rs` consults `is_retry_disabled()` before the request and
client policies. `ClientBuilder::redirect_policy` stays `redirects`-gated,
because the client-level policy is the redirect loop's own default rather than
a request-level assertion.

### The FFI regression

Decoupling `proxy` exposed an accidental dependency in `eggfetch-ffi`. Its C
ABI exports `eggfetch_client_builder_follow_redirects`,
`..._max_redirects` and `..._basic_auth` unconditionally, which call
`ClientBuilder::follow_redirects`/`max_redirects` and `AuthScheme::basic`.
The FFI never declared `redirects` or `basic-auth`; the old `proxy`
implication supplied them transitively.

Measured on both trees with `cargo check -p eggfetch-ffi`:

| FFI feature set | `db3a553b` | `49f8910d` |
|---|---|---|
| `--no-default-features --features proxy` | 0 errors | 0 errors |
| `--no-default-features --features proxy,tls-rustls` | 0 | 0 |
| `--no-default-features --features proxy,cookies` | 0 | 0 |
| `--no-default-features --features cookies` | 7 | 0 |
| `--no-default-features` (nothing) | 26 | 25 |
| `--no-default-features --features tls-rustls` | 6 | 6 (out of scope) |
| `http1`, `http2`, `http2,proxy`, `proxy,http1`, default | 0 | 0 |

The `proxy` row was 5 errors before the repair and `cookies` was 4; both are
now 0. `cookies` improved as a side effect of #33 ungating the redirect module.
`tls-rustls`-only remains broken and was equally broken at the baseline: it
selects no transport and no `high-level-url`, and fixing it is unrelated to
this milestone.

The repair names `features = ["redirects", "basic-auth"]` on the FFI's
`eggfetch-core` dependency. The default build is unchanged because its
`http1` feature already implied both. Dropping `logical-retry` has no
behavioral effect: the FFI exposes no retry API and never sets a policy, and
`send_with_retry` dispatches straight to `dispatch_attempt` when no policy is
enabled — which is `send_with_redirects` whenever `redirects` is present.

`scripts/check_adapter_features.py` now asserts that the FFI's core dependency
names those two capabilities. The guard was proven to reject the pre-fix
manifest (it reported `['basic-auth', 'redirects']` missing when run against
the baseline tree).

## 4. Verification executed

### Commands run

```sh
source .venv/bin/activate && ./scripts/check.sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
python3 scripts/check_adapter_features.py
python3 scripts/check_doc_links.py
python3 scripts/check_doc_examples.py
cargo check -p eggfetch-ffi <eleven feature configurations>     # both trees
TC=nightly-2026-05-07 CARGO=… RUSTC=… RUSTDOC=… python3 scripts/check_rust_public_api.py
```

### Results (pass/fail/skip with reason; counts without concealment)

- **Tier 1** at `49f8910d…`: `CHECK_EXIT=0`, "All routine checks passed."
  1828 Rust workspace tests, **0 failed, 0 ignored**, which includes the
  `--all-features` core suite. 541 Python behaviour tests passed with 40
  documented skips. 134 HTTPX compatibility smoke tests passed. The native
  Python API manifest (66 exports), the Python typing surface (66 runtime
  exports, 32 exception bases, 24 reviewed member contracts), the four typing
  fixtures, the lint-suppression guard self-test (33 cases) and policy scan,
  the downstream-manifest self-test, the adapter feature ownership check, the
  release version/ref validation and the H3 qualification-runner self-test all
  pass. The eight-profile `public_api_contracts` matrix and the lean
  behavioural suites pass.
- **Rust public-surface oracle**: `Rust public API oracle passed (8 profiles)`
  with `cargo-semver-checks` `no semver update required`
  (`223 checks: 223 pass, 30 skip`) against baseline `05f14f44…`.
- **clippy**: exit 0. The one `unused dependency` note
  (`rustls-native-certs`) is present identically at the baseline.
- **fmt**: clean.
- **Skipped**: the Node JS surface, because
  `crates/eggfetch-node/eggfetch.node` is not built on this host. This is the
  policy-defined optional Tier 1 skip, not a failure.
- **Not run**: Tier 2 and Tier 3, and the live security preflight. Tier 2
  needs the pinned nightly oracle toolchain, the exact 1.89.0 MSRV toolchain and
  the operator-supplied downstream artifact manifest; the manifest is absent
  from this checkout, and the gate fails closed rather than skipping.

## 5. Invariant review

- **Single engine.** All new behaviour lives in `eggfetch-core`; no adapter
  gained HTTP logic. The FFI change is a manifest capability declaration.
- **Exhaustive typed rebuilds.** `RequestParts` gained `retry_disabled`, so
  `retry_request` and `advance_redirect_hop` both had to handle it and the
  destructuring in `pipeline/hop.rs`, `pipeline/prepare.rs` and
  `pipeline/redirect.rs` was updated. Omitting it is a compile error, which is
  the intended containment.
- **Frozen body surface.** No public `ResponseBody` variant was added, removed
  or made `#[non_exhaustive]`. `BodyTimeoutStream` remains the only
  high-level timeout owner.
- **Total deadline ownership.** The native builder dispatches exactly one
  attempt through `execute_http_body`; it never resets or caches a deadline.
- **`unsafe_code`.** No `unsafe` was added. The FFI keeps its existing
  `allow`.
- **`set_*` setters, not `#[cfg]` on statements.** `retry()` clears the flag
  through a setter so the cfg-split assignment rule is respected.

## 6. Failure, timeout, pool, and recovery review

- `without_retry()` suppresses a client-level policy: measured as exactly one
  logical attempt in `request_policy_assertion_tests.rs`.
- `retry()` after `without_retry()` re-enables retries: measured as three
  attempts, so setter order semantics are preserved rather than documented.
- The native builder is single-attempt by construction and does not interact
  with the retry or redirect loops.
- `Client::get_uri`/`post_uri`/`request_uri` validate the URI origin eagerly,
  so an unusable URI fails before any connection is opened.
- No pool keying changed. The known IPv6 pool-key divergence is untouched and
  remains pending maintainer action 6.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

**The live Stage C binding is invalidated by this milestone.** It changes Rust
sources, tests and a validation script, all executable inputs under
`.skills/verification-qualification.md`. The binding at
`cef0e7aa5c23ef92f4a378f370516d9dfbaa466d` no longer describes the tree; that
SHA remains the **last completed qualification**, and the current tree is
**unqualified**. The invalidation is recorded in
`plans/httpx-parity-correction-status.md`; the two compat profiles are
deliberately unchanged, because their `qualification-sha` names the last SHA
that was actually qualified.

**Oracle baseline advance.** The `proxy` implication change is classified
`feature_no_longer_enables_feature`, a **major** lint under both
`--release-type patch` and `--release-type minor`. A per-lint
`check-settings.toml` downgrade was tested at both repository and crate level
and does not suppress it. The baseline therefore advanced to `05f14f44…`
rather than bumping the coordinated version or weakening the gate. This is the
established path rather than a new exception: the previous feature-graph
change, the `standard-http1`/`native-http1` split, already predates the old
baseline and was absorbed the same way. The Rust surface only gained items.

**The change is not patch-compatible.** Defaults are unchanged, but a
downstream crate that relied on `proxy` implying `http1` observably loses that
closure. Under `docs/reference/versioning.md` ("patch versions are always
backward-compatible"), the next release carrying this change must be a
**minor** bump. The current version stays 0.3.0, and nothing here authorizes a
release.

## 8. Security review

- Credential handling is untouched: `AuthScheme` hashing/redaction is
  unchanged, and `Proxy-Authorization` keeps its own `base64` edge.
- Gating the six proxy tests on their real feature dependencies means the
  policy-bundle-free profile no longer compiles tests that assert behavior it
  cannot have — they are skipped, not silently weakened.
- `without_retry()` becoming profile-independent adds no bypass: it can only
  reduce attempts, and only where a policy exists.
- The reminder that a lean profile omits redirect following and returns 3xx
  directly is now documented on the ungated `redirect`/policy surfaces rather
  than implied by feature names.
- No new dependency, no new network surface, no new `unsafe`.

## 9. Documentation and operations

Updated: `docs/architecture/feature-flags.md` (proxy-only recipe as a manual
profile, not a declared matrix row), `core-auth-redirect-retry.md`,
`rust-surface-containment.md`, `overview.md`, `docs/ffi/README.md`,
`CHANGELOG.md` (new `[Unreleased]` section), and the plan and roadmap status
rows. The proxy-only recipe is deliberately **not** added to the "Supported
Core Profiles" table: adding a CI matrix row needs maintainer approval, and
declaring a gated profile the eight-profile matrix does not cover would
overstate coverage.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| High | Tier 2 and Tier 3 have not run at `49f8910d…` | The freeze has Tier 1 evidence only; no release may cite it | Run `./scripts/check.sh extended` and `./scripts/check.sh package` at the freeze, supplying the downstream artifact manifest |
| High | The live Stage C binding is invalidated | The tree is unqualified for HTTPX Stage C claims | Run a fresh exact-SHA qualification and record the renewal in the ledger |
| Medium | `eggfetch-ffi --no-default-features --features tls-rustls` does not compile | Pre-existing at `db3a553b…`, not caused by this milestone; it selects no transport and no `high-level-url` | Decide whether unsupported adapter combinations should fail with a clearer manifest error or be documented as unsupported |
| Low | The next release must be a minor bump | A patch release carrying this change would break a `proxy`-implying-`http1` consumer | Hold this change for 0.4.0; do not ship it in a 0.3.x patch |

## 11. Roadmap disposition

M011 moves from `closing` to **conditionally closed** in
`plans/subsystems/core-transport-policy-roadmap.md`. The subsystem stays
`active`. The condition is the exact-SHA requalification in §10, not new
implementation.

## 12. Registry updates

`plans/registry.md`: the core-transport row and the M011 implementation-plan row
now record the baseline advance and the outstanding requalification condition.
