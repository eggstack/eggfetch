# Embedded Footprint Evidence

Latest manual qualification for representative embedded Rust consumers.
This document records one measured profile with exact toolchain/SHA
metadata. Do not duplicate these byte counts elsewhere; link here.

Parent program: `plans/embedded-rust-client-footprint-and-routing-program.md`.
Fixture profiles and runner: `qualification/embedded/README.md`,
`scripts/qualify-embedded-footprint.sh` (manual, never a CI gate).

Status note: the sibling tracks for static resolved-destination routing
(`plans/static-resolution-and-pinned-destination-routing.md`) and native
Rust JSON ergonomics (`plans/native-rust-json-and-response-ergonomics.md`)
have not landed. The `json` flag below is therefore the reserved no-op
described in `feature-flags.md`; JSON fixtures serialize via downstream
`serde_json` directly. This measurement reflects the current tree, not the
intended post-JSON surface.

## Measurement

```sh
scripts/qualify-embedded-footprint.sh --output-dir /tmp/eggfetch-embedded-footprint
```

| Item | Value |
|---|---|
| eggfetch SHA | `cc4468598320f129aa36b99400238d36fddcb577` |
| reqwest | `0.12.28` (resolved via crates.io at run time) |
| rustc | `1.98.1 (48a229cea 2026-09-01)` |
| cargo | `1.98.1 (797e8a9bc 2026-08-05)` |
| target | `aarch64-unknown-linux-gnu` |
| linker | `cc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0` |
| release profile | `lto="thin"`, `codegen-units=1`, `strip=false`, `debug=false`, `panic="unwind"`; `stripped` is an explicit `strip` copy |
| build isolation | isolated `CARGO_TARGET_DIR` per profile (clean) |

Each binary remains tiny: one reusable client, HTTPS GET, JSON
serialize/deserialize where the profile selects it, and streaming
iteration. No cookies, proxy, compression, multipart, H2, or H3 in the
minimal profiles.

## Stripped sizes (primary)

| Profile | eggfetch stripped | reqwest stripped | Delta |
|---|---|---|---|
| A minimal WebPKI (`http1,tls-rustls` vs `stream,rustls-tls`) | 3,086,256 | 2,758,528 | +327,728 (+11.88%) |
| A minimal native (`+tls-native-roots` vs `+rustls-tls-native-roots`) | 3,151,792 | 2,824,064 | +327,728 (+11.60%) |
| B JSON WebPKI (`+json` both) | 3,217,328 | 2,889,600 | +327,728 (+11.34%) |
| B JSON native | 3,217,328 | 2,889,600 | +327,728 (+11.34%) |

Raw (unstripped) deltas are +6.6–7.0% in the same direction
(eggfetch-min 7,571,376 vs reqwest-min 7,076,776, etc.).
Build wall-clock is directional only (81–100 s per clean release build
on this host); compile time is not a pass/fail criterion.

Profile C (ordinary defaults, informational, different conveniences):

| Profile | eggfetch stripped | reqwest stripped |
|---|---|---|
| C default + JSON workload | 3,217,328 | 2,241,192 |

Profile C is not an equivalent comparison: reqwest default uses
dynamically linked system OpenSSL (`default-tls`) plus `charset`/`http2`/
`system-proxy`, while eggfetch default bundles static Rustls/ring. The
smaller reqwest-default artifact reflects different crypto linkage, not a
slimmer equivalent static stack.

## Dependency and feature shape

Unique resolved packages (`cargo tree --prefix none | sort -u | wc -l`):

| Profile | eggfetch | reqwest |
|---|---|---|
| A minimal WebPKI | 116 | 121 |
| A minimal native | 118 | 124 |
| B JSON WebPKI | 122 | 126 |
| B JSON native | 124 | 129 |
| C default | 124 | 141 |

`cargo tree -d` shows only the expected low-risk duplicates (`syn`
2.x/3.x via derive macros; `webpki-roots` 0.26 shim over 1.0 in the
eggfetch tree vs 1.0-only in reqwest). No second DashMap major version
appears in any isolated fixture tree (all eggfetch profiles resolve
`dashmap 6.2.1` once).

TLS/root-store stack:

- eggfetch WebPKI: `hyper-rustls 0.27.9`, `rustls 0.23`, `tokio-rustls
  0.26`, `webpki-roots 0.26.11` (+ `1.0.9` via the 0.26 shim),
  `pem-rfc7468` for custom CA/mTLS parsing.
- reqwest WebPKI: same `hyper-rustls`/`rustls`/`tokio-rustls` lineage
  with `webpki-roots 1.0.9` only, no `pem-rfc7468`.
- Native variants both add `rustls-native-certs 0.8.4`; eggfetch keeps
  WebPKI as construction fallback per `TlsConfig` policy.

Direct-dependency differences (minimal WebPKI):

- Only in eggfetch: `dashmap` (+ `crossbeam-utils`/`hashbrown`/
  `lock_api`/`parking_lot_core` for pool state), `httpdate`
  (Retry-After), `pem-rfc7468` + `base64ct` (custom CA/mTLS),
  `getrandom` (retry jitter + multipart boundaries, intentionally
  unconditional).
- Only in reqwest: `tower`/`tower-http`/`tower-layer`, `tokio-util`,
  `serde_urlencoded`, `sync_wrapper`, `ipnet`, `bitflags`.
- Neither minimal tree contains `cookie`, `async-compression`,
  `brotli`/`zstd`, `quinn`/`h3`, `native-tls`/`openssl`, or `h2`.
  Prior feature-ownership corrections are verifiably reflected:
  disabling a capability removes its crates.
- `serde`/`serde_json` are absent from `eggfetch-min` and present only
  where JSON is selected (currently via the fixture, not core).

## Corrective tuning (bounded)

No executable correction was made. Inspection found no allowed fix that
would not violate the plan's prohibitions:

- No unconditional dependency owned by a disabled capability remains;
  minimal trees exclude cookies/proxy/compression/multipart/H2/H3.
- `getrandom` stays unconditional with documented justification (retry
  jitter shares it with multipart boundaries).
- `pem-rfc7468` ships with `tls-rustls` because custom CA/mTLS parsing
  is part of that API; splitting it would be a micro-feature.
- The `webpki-roots` 0.26 shim (vs reqwest's direct 1.0) is a small,
  audited indirection; bumping the major root bundle solely for bytes
  would change trust content without a security review and is out of
  scope here.
- No dead compat helper or duplicate DashMap major is introduced by
  eggfetch's own manifest.

## Classification: not a footprint win

Eggfetch is materially larger than the equivalently scoped reqwest
configuration in every Rustls-aligned profile (+327,728 stripped bytes,
+11–12%). Do not describe migration as slimming.

The difference is inherent to the current engine rather than an
avoidable wiring error: both stacks share Hyper/Tokio/Rustls
foundations, eggfetch resolves fewer unique packages but carries its own
client/pipeline/retry/redirect/pool/metrics code plus always-on TLS
policy helpers (`pem-rfc7468`, `httpdate`, pool state). Raw package
count alone would have predicted the wrong winner, as the plan warned.

Adoption remains justifiable on ownership/control/API consolidation
grounds where those matter, but size is not a benefit. Revisit only if a
future functional change (for example native JSON landing, or a reviewed
root-bundle update) plausibly moves the artifact, then re-run the manual
runner and update this record.
