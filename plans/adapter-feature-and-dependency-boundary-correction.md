# Adapter Feature and Dependency Boundary Correction

Planning baseline: `025b1a5a6a94b017b3f3b183c3d562e7ee9bcca7` (`main`, 2026-09-15; eggfetch 0.1.4)
Parent program: `plans/post-audit-maintenance-security-and-proxy-modernization-program.md`

## Objective

Make the Cargo feature/dependency boundaries of the FFI, Node, and Python adapters accurately describe the capabilities they use, without changing their effective default behavior or moving HTTP/TLS policy out of `eggfetch-core`.

This is a corrective ownership pass. It should reduce accidental dependency activation and stale adapter residue, not create a new feature taxonomy.

## Current findings

### FFI default-feature leakage

`crates/eggfetch-ffi/Cargo.toml` currently declares feature forwarding such as:

```toml
http1 = ["eggfetch-core/http1"]
http2 = ["eggfetch-core/http2"]
tls-rustls = ["eggfetch-core/tls-rustls"]
http3 = ["eggfetch-core/http3"]
```

but depends on core as:

```toml
eggfetch-core = { version = "0.1.4", path = "../eggfetch-core" }
```

Because `default-features = false` is absent, Cargo activates the core default profile (`http1`, `tls-rustls`, `tls-native-roots`) even when FFI defaults are disabled. Thus the FFI feature table is not authoritative.

The FFI also has no forwarding feature for `eggfetch-core/tls-native-roots`, even though that core feature is active today through the leak.

### Node relies on the same accidental activation

`eggfetch-node` depends on `eggfetch-ffi` with `default-features = false`. This suppresses the FFI defaults, but it does not suppress `eggfetch-core` defaults inside FFI. The Node prototype therefore currently gets H1/Rustls/native roots accidentally.

If the FFI dependency is corrected without updating Node explicitly, the Node build may lose the very transport profile it currently relies on.

### Python carries likely redundant direct TLS dependencies

`eggfetch-python` directly lists `rustls`, `tokio-rustls`, and `webpki-roots` in addition to enabling the core TLS/H3 feature graph. Current Python TLS translation code delegates trust/certificate policy to `eggfetch_core::TlsConfigBuilder`, and upgraded-stream TLS obtains its connector from the core TLS configuration.

Source search alone is not sufficient proof of redundancy: a direct dependency may exist only to unify Cargo features of a transitive crate. The implementation must therefore compare feature graphs before removal and prove that the wheel's effective Rustls provider/TLS1.2/logging/root behavior remains unchanged where required.

### Stale core placeholder

`crates/eggfetch-core/src/config.rs` is an orphaned milestone-era placeholder, is not exported from `lib.rs`, and still describes redirect configuration as future work. It has no current ownership role.

## Required implementation

# 1. Capture the pre-change feature graphs

Before editing manifests, record the relevant effective graphs with repository-local command output or plan notes sufficient for implementation review.

At minimum inspect:

```sh
cargo tree -p eggfetch-ffi -e features
cargo tree -p eggfetch-ffi --no-default-features -e features
cargo tree -p eggfetch-node -e features
cargo tree -p eggfetch-python -e features
cargo tree -p eggfetch-python -i rustls -e features
cargo tree -p eggfetch-python -i tokio-rustls -e features
cargo tree -p eggfetch-python -i webpki-roots -e features
```

Explicitly identify which path currently activates:

- `eggfetch-core/http1`;
- `eggfetch-core/tls-rustls`;
- `eggfetch-core/tls-native-roots`;
- Rustls crypto-provider features;
- TLS 1.2 support;
- Hyper-rustls logging/ALPN support.

Acceptance:

- [ ] The accidental FFI -> core default-feature path is demonstrated before correction.
- [ ] Node's actual pre-change transport/TLS feature set is recorded.
- [ ] Python direct TLS dependencies are classified as behavioral owners, feature-unification-only dependencies, or residue.

# 2. Make FFI forwarding authoritative

Change the FFI's dependency on core to disable core defaults explicitly:

```toml
eggfetch-core = {
    version = "0.1.4",
    path = "../eggfetch-core",
    default-features = false,
}
```

Then make every intended FFI capability explicit through FFI features.

Add an explicit native-root forwarding feature equivalent to:

```toml
tls-native-roots = ["tls-rustls", "eggfetch-core/tls-native-roots"]
```

Preserve the *effective current FFI default trust behavior*. Because native roots are currently present through core defaults, removing the leak must not silently change default FFI certificate-store policy. Unless source-backed review demonstrates that the intended FFI default was deliberately WebPKI-only, include `tls-native-roots` in the FFI default profile.

Audit all other FFI forwarded features for the same issue. Do not add an FFI feature merely because core has one; add it only if the FFI either exposes behavior that needs it or must faithfully forward an existing documented FFI capability.

Acceptance:

- [ ] `eggfetch-ffi --no-default-features` does not activate core H1/Rustls/native-root defaults.
- [ ] FFI default HTTPS behavior retains its current secure root policy.
- [ ] Each FFI Cargo feature maps to an explicit core feature edge.
- [ ] `cargo tree -p eggfetch-ffi --no-default-features -e features` proves the corrected graph.

# 3. Make Node's transport profile explicit

After the FFI correction, update `eggfetch-node` so its dependency declaration requests exactly the transport/TLS capabilities that the prototype actually uses.

The default target should preserve today's effective Node behavior without accidentally enabling unrelated FFI features such as cookies, multipart, proxying, or compression that the Node API does not expose.

A likely profile is conceptually:

```toml
eggfetch-ffi = {
    version = "0.1.4",
    path = "../eggfetch-ffi",
    default-features = false,
    features = ["http1", "tls-rustls", "tls-native-roots"],
}
```

but implementation must confirm the exact required list against the Node tests and supported prototype behavior.

Do not use this plan to mature Node. Keep the existing N-API -> FFI architecture and experimental label unchanged.

Acceptance:

- [ ] Node no longer depends on transitive Cargo defaults for HTTPS capability.
- [ ] Node does not gain unused FFI feature families during correction.
- [ ] Rust-side Node tests and the optional JS smoke surface retain existing behavior.

# 4. Audit and remove redundant Python TLS dependencies

For each of these direct dependencies in `eggfetch-python`:

- `rustls`;
- `tokio-rustls`;
- `webpki-roots`;

answer both questions before removal:

1. Does Python Rust source directly use the crate?
2. Does the direct dependency materially activate a feature that the `eggfetch-core` dependency graph would otherwise omit?

Use `cargo tree -e features` rather than grep alone.

If a dependency is only duplicating features already guaranteed by the core profile, remove it. After each removal or as a single coherent edit, compare the effective feature graph and run the Python wheel/API tests.

If a direct dependency is intentionally retained solely for feature unification, document that rationale next to the manifest entry and in `docs/architecture/dependency-policy.md`; an unexplained direct dependency with no source use is not acceptable.

Do not weaken or change:

- default TLS verification;
- TLS 1.2/1.3 support exposed by the Python package;
- Rustls provider availability;
- native-root/WebPKI policy;
- mTLS;
- `ssl.SSLContext` translation;
- proxy TLS behavior;
- `NetworkStream.start_tls()`.

Acceptance:

- [ ] Every direct Python TLS dependency has a source-backed owner or is removed.
- [ ] The before/after effective feature graph is equivalent for supported Python behavior.
- [ ] No wheel/API/HTTPX behavior is changed by dependency cleanup.

# 5. Delete obsolete core configuration placeholder

Confirm `crates/eggfetch-core/src/config.rs` has no module declaration, include, generated-code dependency, documentation link, or external package-content contract.

If it is truly orphaned, delete it. Do not repurpose it into a generic configuration module merely to avoid deletion; `ClientConfig`, `PoolConfig`, `Timeout`, `TlsConfig`, `RetryPolicy`, and the other current typed owners should remain where they are.

Search docs for stale references to the placeholder and update only those directly affected.

Acceptance:

- [ ] No milestone-era placeholder remains in the core source tree.
- [ ] No public API is removed as a consequence.

# 6. Decide the Python HTTP/3 build-cost issue explicitly

The Python crate currently enables core HTTP/3 at compile time so `Client(http3=True)` / `AsyncClient(http3=True)` are available from the ordinary wheel even though HTTP/3 remains experimental.

Do not casually remove H3 from the wheel in this corrective pass: Cargo features are compile-time, while the Python API currently presents H3 as a runtime option. Removing the feature would make an existing constructor option fail or require a separate wheel/package strategy.

Instead:

- measure/record the H3 contribution to the Python dependency tree and, where existing tooling permits, artifact size;
- verify that the documentation labels H3 experimental despite being compiled into the wheel;
- retain it unless a backward-compatible packaging strategy is already available;
- if retained, record it as an intentional package-cost decision rather than accidental dependency residue.

A future optional-wheel/extras architecture is outside this plan.

Acceptance:

- [ ] The always-compiled Python H3 surface is explicitly classified as intentional or changed through a separately justified compatible design.
- [ ] No unsupported binary-size claim is made.

# 7. Add bounded feature-graph regression checks

Extend existing local validation rather than creating a new workflow.

At minimum ensure the repository can validate:

```sh
cargo check -p eggfetch-ffi --no-default-features
cargo check -p eggfetch-ffi --all-features
cargo check -p eggfetch-node
cargo check -p eggfetch-python
```

Add targeted metadata/tree assertions only for important invariants that Cargo compile checks cannot prove, especially:

- FFI no-default must not activate `eggfetch-core/tls-rustls` or `tls-native-roots`;
- Node must explicitly receive its required protocol/TLS features;
- removed Python dependencies must not reappear as direct dependencies accidentally.

Prefer a small stdlib/Python metadata checker over brittle textual grep of `cargo tree` output if an executable assertion is needed.

Do not create a combinatorial adapter feature matrix.

# 8. Documentation

Update only documents directly affected by the corrected ownership:

- `docs/architecture/dependency-policy.md`;
- `docs/architecture/feature-flags.md`;
- `docs/architecture/ffi-and-node.md` if the effective Node/FFI profile is described there;
- package/readme feature tables if they make a contradictory claim.

Do not reopen the broad documentation program.

## Focused validation

Required before this plan closes:

```sh
./scripts/check.sh
cargo check -p eggfetch-ffi --no-default-features
cargo check -p eggfetch-ffi --all-features
cargo test -p eggfetch-ffi --all-features
cargo test -p eggfetch-node --all-features
cargo check -p eggfetch-python
```

Also run the repository's existing Python native API, typing, behavior, and compact compatibility checks through Tier 1. If manifest changes alter wheel composition, run `./scripts/check.sh package` before closure.

Where the exact Rust 1.89 toolchain is available, include the corrected adapter graph in the normal MSRV qualification rather than silently relying on stable.

## Non-goals

- no core feature redesign beyond what the adapter correction requires;
- no Node maturation or npm publication;
- no Python API additions;
- no H3 graduation;
- no new TLS backend;
- no removal of native roots from current default users;
- no dependency upgrade solely to make `cargo tree` shorter;
- no exhaustive feature cross-product.

## Exit criteria

This plan is complete when adapter manifests describe reality: disabling FFI defaults truly disables core defaults, Node explicitly asks for the capabilities it uses, Python has no unexplained TLS dependency duplication, stale core placeholder code is gone, and routine/package validation proves the corrections without an API regression.