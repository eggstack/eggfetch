# Core Private Module Decomposition

Planning baseline: 03ecba973010e2858bf16a2b5f84d51ce70adae4
Parent program: plans/api-preserving-maintenance-and-interop-hardening-program.md
Prerequisite: plans/rust-public-api-regression-oracle.md
Normative verification policy: docs/verification-policy.md

## Objective

Reduce private maintenance concentration in eggfetch-core without changing any public Rust path, signature, type shape, feature exposure, protocol semantics, or supported capability.

The target is not arbitrary line-count reduction. The target is clearer private ownership boundaries in client.rs and proxy.rs, where public models currently coexist with substantial private construction/parsing/cache machinery.

All moves must be protected by the Rust public-surface oracle established by the prerequisite plan.

## Part A — decompose client.rs around private responsibilities

Keep the public Client and ClientBuilder type declarations and their documented public methods at their existing canonical paths.

Extract only private/pub(crate) machinery into submodules under crates/eggfetch-core/src/client/ as appropriate.

Candidate boundaries:

### client/config.rs

Own private ClientConfig state, defaults, Debug/redaction support, and configuration-only normalization helpers.

Requirements:

- ClientConfig remains pub(crate), never public;
- existing public builder defaults and precedence are unchanged;
- redaction behavior remains unchanged;
- feature-gated fields remain behind exactly the same cfg expressions.

### client/routes.rs or client/cache.rs

Own private route/cache identities and bounded per-route client caches, including ResolvedRouteKey and route-specific cached-client construction where it is currently concentrated in ClientInner.

Requirements:

- cache-key identity remains byte/field equivalent;
- cache bounds/eviction/reuse behavior remain unchanged;
- proxy/TLS/SNI/resolved-target/HTTP-version identity isolation stays exact;
- no total request deadline enters persistent cache identity/state;
- no new shared global cache is introduced.

### client/connectors.rs

Own private standard/custom connector construction and ALPN configuration helpers where this can be moved without creating a second transport owner.

Requirements:

- transport/hyper_client.rs remains the centralized Hyper client construction owner where AGENTS.md currently requires it;
- this module may prepare connector inputs but must not duplicate hyper_client.rs policy;
- standard-route, advanced-routing, UDS, custom dialer, SNI, resolved-target and socket-option cfg boundaries remain exact.

If these candidate boundaries would create circular or less coherent dependencies, choose a smaller decomposition. Do not move code solely to hit a target file size.

## Part B — keep public Client/ClientBuilder implementation auditable

Public method definitions may remain in client.rs even if their private implementation delegates to extracted helpers.

Prefer:

- public method body is small and delegates to private typed helper;
- private helper arguments make ownership/config dependencies explicit;
- no macro generation of the public builder surface;
- no trait indirection introduced solely to split files.

Do not create a public internal ClientConfig-like abstraction.

## Part C — decompose proxy.rs without moving public proxy types

Keep these existing public types at their current public paths:

- NoProxyRule;
- NoProxy;
- ProxyAuth;
- ProxyRule;
- ProxyDecision;
- ProxyConfig;
- Proxy;
- ProxyEnvironment.

Move only private helpers behind proxy submodules.

Candidate boundaries:

### proxy/no_proxy.rs

Private parsing/matching primitives:

- parse_with_localhost_mode;
- parse_entry;
- HTTPX-specific internal entry parsing;
- domain/host/IP/CIDR/port matching helpers;
- scheme default-port helper.

The public NoProxy::parse / parse_httpx / should_bypass methods remain in proxy.rs and delegate privately.

Do not unify native and HTTPX NO_PROXY semantics. AGENTS.md explicitly records that they differ.

### proxy/environment.rs

Private environment variable normalization/snapshot helpers:

- proxy URL normalization;
- environment precedence helpers;
- no_proxy environment extraction/normalization;
- scheme-specific resolution internals.

Public ProxyEnvironment construction/resolution stays unchanged.

### proxy/identity.rs, only if justified

Private connection identity construction can move if doing so improves auditability without obscuring which proxy/TLS/resolved-address fields fragment connection identity.

Do not hash away behavior or change identity ordering/encoding under this maintenance plan.

## Part D — preserve public canonical paths exactly

Do not implement decomposition by moving public structs into child modules and re-exporting them unless the Rust exact-surface oracle proves canonical path/rustdoc identity is byte-for-byte unchanged and the repository explicitly accepts that source-layout tradeoff.

The default implementation should leave public declarations in client.rs/proxy.rs and move only private internals.

Historical public re-export paths outside these files remain untouched.

## Part E — tests follow ownership

Move private unit tests with their implementation only when that improves locality. Keep cross-module behavior tests/integration tests at their existing higher-level boundary.

Maintain focused tests for:

- ClientBuilder defaults and timeout merging;
- TLS/root/provider behavior;
- HTTP version policy;
- direct/custom connector ALPN;
- route-cache key isolation and cache bounds;
- content-length handling;
- URL credential rejection/redaction;
- proxy URL parsing/redaction;
- native versus HTTPX NO_PROXY differences;
- environment proxy precedence;
- proxy auth redaction/validation;
- resolved proxy addresses;
- opaque connection identity.

Do not reduce coverage because a helper changed files.

## Part F — dependency and visibility audit

After moving code:

- run a visibility search for newly public pub items;
- ensure child modules are private or pub(crate) only as required;
- ensure no new crate dependency was introduced;
- ensure no optional feature became unconditional;
- ensure url/idna/high-level policy still disappears from native/lean profiles where currently promised;
- ensure eggfetch-http-connect remains owned only by the proxy feature.

## Part G — documentation

Architecture docs should change only if the new private ownership boundary is useful to future maintainers.

Potential updates:

- docs/architecture/core-engine.md for client private composition;
- docs/architecture/core-tls-proxy-protocols.md for proxy private ownership;
- AGENTS.md only if agents need to know a new canonical private file location.

Do not present internal modules as public extension points.

## Focused validation

At each decomposition stage run:

- cargo fmt --all -- --check;
- cargo clippy -p eggfetch-core --all-targets --all-features -- -D warnings;
- cargo test -p eggfetch-core --all-features -- --test-threads=1;
- documented no-default/lean/native profile checks;
- the exact Rust public-surface oracle from the prerequisite plan;
- ./scripts/check.sh.

After both client and proxy decomposition, run ./scripts/check.sh extended before handing off to the final program closure.

## Acceptance criteria

- [x] Public Client/ClientBuilder canonical paths/signatures/docs exposure are unchanged. — `Client` and `ClientBuilder` are still declared in `crates/eggfetch-core/src/client.rs:76` and `:483` and still re-exported from `crates/eggfetch-core/src/lib.rs:71`; the exact-surface oracle (`scripts/check_rust_public_api.py`, planning baseline `03ecba973010e2858bf16a2b5f84d51ce70adae4`) is green on all eight profiles.
- [x] Public proxy type canonical paths/signatures/docs exposure are unchanged. — `NoProxyRule`, `NoProxy`, `ProxyAuth`, `ProxyRule`, `ProxyDecision`, `ProxyConfig`, `Proxy`, and `ProxyEnvironment` are still declared in `crates/eggfetch-core/src/proxy.rs:50,113,187,270,285,298,446,829` and still re-exported from `crates/eggfetch-core/src/lib.rs:86`; the same eight-profile oracle is green.
- [x] Private config/cache/connector responsibilities are separated where doing so reduces maintenance coupling. — The private modules `client/config.rs`, `client/routes.rs`, `client/connectors.rs` (declared at `crates/eggfetch-core/src/client.rs:53,55,56`) and `proxy/no_proxy.rs`, `proxy/environment.rs`, `proxy/identity.rs` (declared at `crates/eggfetch-core/src/proxy.rs:41,42,43`) hold the extracted machinery; every item in them is `pub(crate)` or private.
- [x] Native and HTTPX NO_PROXY semantics remain deliberately distinct. — `NoProxy::parse` still selects `parse_with_localhost_mode(s, false)` (`crates/eggfetch-core/src/proxy.rs:132`) and `NoProxy::parse_httpx` still selects `parse_with_localhost_mode(s, true)` (`:145`); the two rule sets stay separate in `crates/eggfetch-core/src/proxy/no_proxy.rs`.
- [x] Route/client cache identity and bounds are unchanged. — `ResolvedRouteKey`, its `impl`, and the `BoundedClientCache` bounds moved byte-identically out of `crates/eggfetch-core/src/client.rs` at `d2c8b137` into `crates/eggfetch-core/src/client/routes.rs:53` (the three extracted regions `diff` clean against the pre-split file).
- [x] Hyper client construction remains centralized under existing architecture rules. — `build_hyper_client` has exactly one definition, `crates/eggfetch-core/src/transport/hyper_client.rs:261`; every call site imports it from that module.
- [x] No new public helper/type/module is introduced. — `crates/eggfetch-core/src/lib.rs` declares no new public module, and `grep -rnE '^\s*pub (fn|struct|enum|trait|type|mod|const|static|use)' crates/eggfetch-core/src/client/ crates/eggfetch-core/src/proxy/` returns nothing; the eight-profile oracle is green.
- [x] No new production dependency is introduced. — `crates/eggfetch-core/Cargo.toml` differs from planning baseline `03ecba97` only in the two workspace-internal version fields (`eggfetch-core` 0.1.9→0.2.2, `eggfetch-http-connect` 0.1.9→0.2.2); no dependency name was added.
- [x] Supported feature profiles expose the exact planning-baseline Rust surface. — `scripts/check_rust_public_api.py` reports `Rust public API oracle passed (8 profiles)` against `compat/rust-public-api/` at baseline `03ecba973010e2858bf16a2b5f84d51ce70adae4`, and the complementary `cargo-semver-checks` half reports 223/223 checks passing.
- [x] All focused and canonical tests remain green. — See the disposition note below for the focused re-verification recorded on 2026-10-22 and the gate evidence named in the execution record.

## Stop conditions

Do not continue a file split if it requires public re-export churn, cfg duplication that can drift, cyclic module layering, a new abstraction trait with no behavioral value, or duplicated transport/client-construction policy.

A smaller coherent extraction is preferable to a complete decomposition that makes ownership harder to audit.

## Execution record — complete (2026-09-21)

Implemented on executable freeze `df2549f7c64ebfccde61ed36fef785d39e83b38d`.
Private client configuration and proxy environment/NO_PROXY helpers now have
focused internal module boundaries. Public paths, cfg exposure, cache and
transport ownership, and native-versus-HTTPX NO_PROXY behavior remain
unchanged; the exact Rust oracle and extended feature/compatibility gates pass.

## Acceptance-criteria disposition (2026-10-22)

All ten acceptance criteria above were re-verified against the current tree and
are **met**; none is superseded, deferred, or never applicable. Evidence is
cited inline with each criterion. The execution record above is unchanged.

Focused re-verification recorded on 2026-10-22:

- `scripts/check_rust_public_api.py` — `Rust public API oracle passed (8 profiles)`,
  plus `223/223` semver checks from the complementary `cargo-semver-checks` half.
- `cargo test -p eggfetch-core --all-features -- --test-threads=1` —
  **1489 passed, 0 failed** across the unit, integration and doc-test suites.

The full Tier 1 and extended gates were not re-run in this session — the
maintainer deferred them — so the gate claim in the execution record above is
carried forward as of the named freeze
`df2549f7c64ebfccde61ed36fef785d39e83b38d` rather than re-certified against the
current uncommitted tree.
