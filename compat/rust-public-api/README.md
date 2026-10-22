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

## Profiles

The oracle freezes eight profiles — every supported `eggfetch-core` profile is
compiled in isolation, so none is covered only as a side effect of
`--all-features`:

| Snapshot | Cargo feature set |
| --- | --- |
| `default.txt` | default features |
| `http1-tls-rustls.txt` | `--no-default-features --features http1,tls-rustls` |
| `http2-tls-rustls.txt` | `--no-default-features --features http2,tls-rustls` |
| `standard-http1-tls-rustls.txt` | `--no-default-features --features standard-http1,tls-rustls` |
| `standard-http2-tls-rustls.txt` | `--no-default-features --features standard-http2,tls-rustls` |
| `native-http1-tls-rustls.txt` | `--no-default-features --features native-http1,tls-rustls` |
| `native-http2-tls-rustls.txt` | `--no-default-features --features native-http2,tls-rustls` |
| `all-features.txt` | `--all-features` |

`standard-http2-tls-rustls.txt` and `native-http2-tls-rustls.txt` are the HTTP/2
members of the lean and native families; they were added alongside the
`standard-http1`/`native-http1` snapshots so that the H2 profiles are frozen
like the H1 ones.  Each H1/H2 pair selects a different transport but exposes
the same public surface — the transport difference is private — so the two
snapshots of each pair are byte-identical.  The lean and native families are
not identical to each other, so the oracle does discriminate profile surfaces.
