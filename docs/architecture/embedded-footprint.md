# Embedded Footprint Evidence

Latest manual qualification for representative embedded Rust consumers.
This document records one measured profile with exact toolchain/SHA
metadata. Do not duplicate these byte counts elsewhere; link here.

See also: [overview.md](overview.md).

Parent program: `plans/embedded-rust-client-footprint-and-routing-program.md`.
Fixture profiles and runner: `qualification/embedded/README.md`,
`scripts/qualify-embedded-footprint.sh` (manual, never a CI gate).

The extensible embedded transport program also includes the external-style
compile/runtime fixture at `qualification/embedded-custom-dialer/`. It is a
public-API qualification artifact, not a replacement for this footprint
measurement and does not change the classification below.

Status note: the native JSON helper track is now implemented. The recorded
JSON fixture uses `eggfetch-core/json`; static resolved-destination routing is
an independent native transport capability and is not part of the size
workload.

## Linked footprint reduction program: lean standard-route measurement (2026-09-18)

Parent program: `plans/linked-binary-footprint-reduction-program.md`.
Executable freeze: policy boundary `263e7749` plus the standard-route
boundary `f1988fa0a39a057946dd0e73bfd81065612b7711` (see `plans/standard-route-advanced-routing-feature-boundary.md`;
docs-only descendants after that freeze do not change the executable
binding). Fixture: `qualification/embedded/eggfetch-min`
source (streaming HTTPS GET, same source for all eggfetch profiles below) plus
temporary downstream-style copies with lean feature sets (same source, same
release shape `lto="thin"`, `codegen-units=1`, `panic="unwind"`, `strip=false`;
`stripped` is an explicit `strip` copy).

| Item | Value |
|---|---|
| rustc | `1.98.1 (48a229cea 2026-09-01)` |
| cargo | `1.98.1 (797e8a9bc 2026-08-05)` |
| target | `x86_64-unknown-linux-gnu` |
| linker | `cc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0` |
| reqwest | `0.12.28` (resolved via crates.io at run time) |
| build isolation | isolated `CARGO_TARGET_DIR` per profile (clean) |

| Profile | Features | Unstripped | Stripped |
|---|---|---:|---:|
| reqwest-min (aligned baseline) | `default-features=false`, `stream,rustls-tls` | 7,873,176 | 3,079,840 |
| eggfetch full compatibility | `default-features=false`, `http1,tls-rustls` | 8,683,256 | 3,669,840 |
| eggfetch policy-lean (advanced retained) | `native-http1,high-level-url,tls-rustls` (no `logical-retry`/`redirects`/`basic-auth`) | 8,609,648 | 3,600,816 |
| eggfetch lean standard Bearer client | `standard-http1,tls-rustls` (no `advanced-routing`, no policy bundle) | 7,967,632 | 3,111,904 |

Deltas (stripped):

- Full compatibility vs reqwest: +590,000 (+19.2%). Unchanged direction from
  the historical record below: the full client retains retry/redirect/Basic
  plus advanced routing (Dialer, resolved-target/SNI caches, direct/UDS).
- Policy-lean vs full: −69,024 (−1.9%). Matches the policy-boundary closure
  (`eggfetch_core` .text 262.0 KiB → 236.9 KiB; `httpdate`/`base64` absent
  from the lean link; `getrandom` 748 B remains via ring/Rustls).
- Lean standard vs full: −557,936 (−15.2%). `eggfetch_core` .text 262.0 KiB
  → 133.9 KiB (−128.1 KiB eggfetch-owned, −49%). `hyper` 124.5 KiB → 78.6 KiB,
  `hyper-util` 132.7 KiB → 68.3 KiB (advanced connector monomorphizations gone).
- Lean standard vs reqwest: +32,064 (+1.0%). The Gregg-like gap is
  essentially closed for standard DNS/TCP/TLS Bearer clients under this
  toolchain/target/profile.
- Unique packages (`cargo tree --prefix none | sort -u | wc -l`): full 110 →
  lean standard 105 (−5). Tree lines 193 → 187 (−6). Lean drops core's direct
  `base64`/`httpdate`/`getrandom` edges (transitive `getrandom` via ring and
  `base64ct` via `pem-rfc7468` remain where TLS needs them); dependency-count
  reduction is modest — the win is linked bytes, not package count.

Attribution (unstripped companions, `cargo bloat --crates` / `-n 30`):

- Full top eggfetch symbol: `pipeline::send_single_request::{closure#0}`
  55.8 KiB, plus `pipeline::redirect::send_with_redirects` 19.0 KiB and four
  `hyper_util::Client<...>::send_request` monomorphizations ~18 KiB each for
  standard, Dialer, Direct, and UDS connectors, plus
  `DirectConnector::call` 14.2 KiB and `ClientBuilder::build` 13.1 KiB.
- Lean standard top eggfetch symbol: `pipeline::lean::send_lean::{closure#0}`
  26.1 KiB. Exactly one `hyper_util::Client<...>::send_request` closure
  remains (standard route, 21.7 KiB); no Dialer/UDS/Direct variants.
  `ClientBuilder::build` 7.8 KiB. Zero `send_with_retry`/
  `send_with_redirects`/`RetryPolicy`/`redirect_method` and zero
  `uds`/`dialer`/`direct`/`sni`/`resolved` route symbols (remaining "retry"
  hits are rustls `HelloRetryRequest` only).
- `nm --size-sort` confirms the above; `cargo tree` confirms the lean
  resolved set loses `eggfetch-http-connect` (proxy-owned since the residual
  tuning plan) and core's direct `base64`/`httpdate`/`getrandom` edges.

What the lean profile omits (opt-in only; full/default/Python/CLI/HTTPX
behavior unchanged):

- Advanced routing (`advanced-routing`): custom `Dialer`, caller-supplied
  resolved addresses, SNI override, local-address/socket-option route, UDS,
  and their caches/dispatch arms. Pinned/SNI hints fail closed with
  `Unsupported` in lean; builder methods and `Dialer`/`SocketOption`
  re-exports are absent without the feature.
- Policy (`logical-retry`, `redirects`, `basic-auth`): logical retry loop,
  redirect following/history, Basic auth. Lean dispatches once and returns
  3xx without following.

Gregg-like behavior proof (lean `standard-http1,tls-rustls`):
`crates/eggfetch-core/tests/lean_route_tests.rs` (9 tests) plus
`lean_policy_tests.rs` (6 tests), each passing under both `--all-features`
and `--no-default-features --features standard-http1,tls-rustls`: HTTP/HTTPS
loopback, Bearer + redaction, DNS/refused typed failures, total timeout,
body cap, keep-alive reuse, cancellation, 3xx passthrough with empty history,
single-attempt 503, and lean rejection of advanced hints.

Classification update: the full compatibility profile is still **not a
footprint win** versus aligned reqwest (same direction as below). The new
lean standard-route Bearer profile **is a material linked-footprint
improvement**, closing the measured stripped delta to ~+32 KiB (+1%) on this
host/target/profile without changing default capabilities, TLS verification,
roots, typed failures, pooling, timeouts, or body limits. Do not generalize
beyond the measured toolchain/target/profile; cross-host deltas are not exact
regressions.

## Latest native Tower service measurement (2026-09-15)

```sh
scripts/qualify-embedded-footprint.sh --output-dir /tmp/eggfetch-embedded-footprint
```

| Item | Value |
|---|---|
| eggfetch SHA | `490320f6e99fbb7916280d6bcdd21660bd74f858` |
| reqwest | `0.12.28` (resolved via crates.io at run time) |
| rustc | `1.98.1 (48a229cea 2026-09-01)` |
| cargo | `1.98.1 (797e8a9bc 2026-08-05)` |
| target | `x86_64-unknown-linux-gnu` |
| linker | `cc (Ubuntu 13.3.0-6ubuntu2~24.04.1) 13.3.0` |
| release profile | `lto="thin"`, `codegen-units=1`, `strip=false`, `debug=false`, `panic="unwind"`; `stripped` is an explicit `strip` copy |
| build isolation | isolated `CARGO_TARGET_DIR` per profile (clean) |

Ordinary full-compatibility profiles on this host remain larger than aligned reqwest profiles
(pre-lean record; the 2026-09-18 lean section above supersedes this for
standard-route Bearer clients):

| Profile | eggfetch stripped | reqwest stripped | Delta |
|---|---:|---:|---:|
| minimal WebPKI | 3,654,432 | 3,079,840 | +574,592 |
| minimal native roots | 3,688,968 | 3,116,968 | +572,000 |
| JSON WebPKI | 3,780,016 | 3,223,256 | +556,760 |
| JSON native roots | 3,814,552 | 3,256,288 | +558,264 |

The custom-control fixture is measured separately because reqwest has no
equivalent first-party profile:

| Profile | stripped | unstripped | unique packages |
|---|---:|---:|---:|
| `eggfetch-custom-dialer` | 4,380,200 | 6,086,232 | 117 |

The runner also measured `eggfetch-default` at 3,774,552 stripped bytes and
`reqwest-default` at 2,401,392; that pair is informational and not equivalent.
These x86_64 values supersede neither the historical aarch64 record below nor
the classification: eggfetch is still **not a footprint win** in the aligned
profiles. Cross-host byte deltas are not exact regressions.

Latest unique package counts were 116/118/124/126 for the four eggfetch
minimal/native/JSON profiles and 121/124/126/129 for their reqwest peers;
`eggfetch-default` was 126, `reqwest-default` 144, and the custom fixture 117.

## Previous comparable measurement (2026-09-12, aarch64)

Each binary remains tiny: one reusable client, HTTPS GET, JSON
serialize/deserialize where the profile selects it, and streaming
iteration. No cookies, proxy, compression, multipart, H2, or H3 in the
minimal profiles.

## Stripped sizes (historical primary record)

| Profile | eggfetch stripped | reqwest stripped | Delta |
|---|---|---|---|
| A minimal WebPKI (`http1,tls-rustls` vs `stream,rustls-tls`) | 3,086,256 | 2,758,528 | +327,728 (+11.88%) |
| A minimal native (`+tls-native-roots` vs `+rustls-tls-native-roots`) | 3,151,792 | 2,824,064 | +327,728 (+11.60%) |
| B JSON WebPKI (`+json` both) | 3,217,328 | 2,889,600 | +327,728 (+11.34%) |
| B JSON native | 3,217,328 | 2,889,600 | +327,728 (+11.34%) |

Raw (unstripped) deltas are +6.6–7.0% in the same direction:
`eggfetch-min` 7,574,048 vs `reqwest-min` 7,076,776,
`eggfetch-min-native` 7,681,424 vs 7,189,528, `eggfetch-json` 7,797,184
vs 7,305,064, and `eggfetch-json-native` 7,839,192 vs 7,352,520.
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

## Dependency and feature shape (historical pre-lean tree)

The tree below describes the pre-lean dependency shape at the recorded
SHAs (includes `dashmap`; `getrandom` unconditional). Since then DashMap
was removed, `getrandom`/`httpdate`/`base64` became optional behind
`logical-retry`/`multipart`/`basic-auth`, and `eggfetch-http-connect`
became `proxy`-owned; see the 2026-09-18 lean section above for the
current ownership.

Unique resolved packages (`cargo tree --prefix none | sort -u | wc -l`):

| Profile | eggfetch | reqwest |
|---|---|---|
| A minimal WebPKI | 116 | 121 |
| A minimal native | 118 | 124 |
| B JSON WebPKI | 124 | 126 |
| B JSON native | 126 | 129 |
| C default | 126 | 141 |

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
  where the native `json` feature is selected.

## Corrective tuning (bounded)

No additional executable correction was made during measurement. Inspection found no allowed fix that
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

## Historical classification (pre-lean): full profiles not a footprint win

This section preserves the pre-lean assessment for full compatibility
profiles. It is superseded for standard-route Bearer clients by the
2026-09-18 lean section above.

The historical aarch64 record below showed eggfetch materially larger than the
equivalently scoped reqwest configuration by +327,728 stripped bytes in every
Rustls-aligned profile. The latest x86_64 record shows the same direction with
host-specific deltas of +546,344 to +564,112 bytes. Do not describe migration
as slimming beyond the lean record above.

The difference is inherent to the current engine rather than an
avoidable wiring error: both stacks share Hyper/Tokio/Rustls
foundations, eggfetch resolves fewer unique packages but carries its own
client/pipeline/retry/redirect/pool/metrics code plus always-on TLS
policy helpers (`pem-rfc7468`, `httpdate`, pool state). Raw package
count alone would have predicted the wrong winner, as the plan warned.

Adoption remains justifiable on ownership/control/API consolidation
grounds where those matter, but size is not a benefit. Revisit after a
future functional or dependency change plausibly moves the artifact, then
re-run the manual runner and update this dated record.
