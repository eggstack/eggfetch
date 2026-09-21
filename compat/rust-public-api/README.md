# Rust public API oracle

These snapshots freeze the supported `eggfetch-core` Rust surface at planning
baseline `03ecba973010e2858bf16a2b5f84d51ce70adae4`.  They are verification
artifacts, not an additional supported API or a source of feature semantics.

The generator is `cargo-public-api 0.52.0`, run with the pinned repository
nightly toolchain `nightly-2026-05-07` and `-sss --color never`.  The
complementary semver check uses `cargo-semver-checks 0.49.0` on a supported
stable toolchain and compares the default profile to the same baseline.

Run the exact oracle from the repository root with:

```sh
RUSTUP_TOOLCHAIN=nightly-2026-05-07 \
  CARGO_PUBLIC_API=/path/to/cargo-public-api \
  CARGO_SEMVER_CHECKS=cargo-semver-checks \
  python scripts/check_rust_public_api.py
```

The stable compile contracts are in
`crates/eggfetch-core/tests/public_api_contracts.rs` and are exercised by the
normal profile checks.  Do not regenerate a snapshot to accept an accidental
API change; an intentional public change requires a separate versioned API
plan.
