# Embedded Footprint Qualification

Manual, bounded downstream-size qualification for `eggfetch-core`.
Not a CI gate. Run at release/architecture review points; copy headline
numbers into `docs/architecture/embedded-footprint.md`.

## Fixtures

Tiny downstream-style binaries under this directory. Each has its own
`[workspace]` so Cargo's graph reflects an ordinary downstream binary, not
the eggfetch workspace. No fixture creates a second HTTP implementation;
all eggfetch fixtures drive `eggfetch-core` only.

| Fixture | Profile | `eggfetch-core` / `reqwest` features |
|---|---|---|
| `eggfetch-min` | A minimal HTTPS streaming (WebPKI primary) | `default-features=false`, `http1,tls-rustls` |
| `reqwest-min` | A minimal HTTPS streaming (WebPKI primary) | `default-features=false`, `stream,rustls-tls` |
| `eggfetch-json` | B HTTPS + JSON + streaming (WebPKI primary) | `http1,tls-rustls,json` (no default) |
| `reqwest-json` | B HTTPS + JSON + streaming (WebPKI primary) | `stream,json,rustls-tls` (no default) |
| `eggfetch-default` | C ordinary default + JSON workload (informational) | default (`http1,tls-rustls,tls-native-roots`) + `json` |
| `reqwest-default` | C ordinary default + JSON workload (informational) | default (`default-tls,charset,http2,system-proxy`) + `json,stream` |

`native` builds add the fixture `native` feature:

- eggfetch: `eggfetch-core/tls-native-roots` (system store preferred,
  WebPKI construction fallback).
- reqwest: `reqwest/rustls-tls-native-roots` alongside `rustls-tls`
  (both root sets present, approximating fallback).

All fixtures share one release profile (`lto="thin"`, `codegen-units=1`,
`strip=false`, `debug=false`, `panic="unwind"`); the runner strips an
explicit copy so both raw and stripped sizes are recorded.

Each binary exercises the same workload for its profile:

- construct one reusable client;
- HTTPS GET;
- JSON request serialization where the profile selects it
  (eggfetch fixtures serialize via `serde_json` directly because
  `eggfetch-core/json` is currently reserved; reqwest fixtures use
  `.json()` — the difference is recorded, not hidden);
- JSON response deserialization from raw bytes;
- streaming response iteration via `bytes_stream()`.

No fixture exercises cookies, proxies, compression, multipart, H2, or H3
unless its profile selects them. `EGGFETCH_FIXTURE_NOOP=1` constructs a
client (plus a serde round-trip for JSON fixtures) without I/O for smoke.

## Commands

```sh
# Full qualification (trees + 10 isolated release builds + sizes):
scripts/qualify-embedded-footprint.sh --output-dir /tmp/eggfetch-embedded-footprint

# Tree evidence only (fast, no builds):
scripts/qualify-embedded-footprint.sh --output-dir /tmp/eggfetch-embedded-footprint --skip-build

# Single fixture checks:
cargo check --manifest-path qualification/embedded/eggfetch-min/Cargo.toml
cargo check --manifest-path qualification/embedded/eggfetch-min/Cargo.toml --features native
cargo tree --manifest-path qualification/embedded/eggfetch-min/Cargo.toml
cargo tree --manifest-path qualification/embedded/eggfetch-min/Cargo.toml -e features
cargo tree --manifest-path qualification/embedded/eggfetch-min/Cargo.toml -d
```

`cargo bloat` is optional: the runner uses it when installed and never
fails when absent.

## Result interpretation

See `docs/architecture/embedded-footprint.md` for the latest measured
numbers with toolchain/SHA metadata and the
beneficial / neutral / not-a-footprint-win classification. Do not copy
time-sensitive byte counts into README or guides; link to that document.

## Non-goals

- No benchmark-marketing claims without context.
- No network throughput re-benchmark.
- No routine CI matrix, size gate, dashboard, or scheduled workflow.
- No requirement that eggfetch beat reqwest on every target/profile.
- No custom allocator/linker solely for numbers.
- No dependency replacement whose only justification is a few kilobytes.
