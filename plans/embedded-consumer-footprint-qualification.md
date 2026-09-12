# Embedded Consumer Footprint Qualification

Planning baseline: `445149bd2af3c3ce5c0d34daacbde1bf8de57297` (`main`, 2026-09-11; eggfetch 0.1.3)
Parent program: `plans/embedded-rust-client-footprint-and-routing-program.md`
Depends on: closure of `core-feature-dependency-and-tls-boundary-hardening.md`, `static-resolution-and-pinned-destination-routing.md`, and `native-rust-json-and-response-ergonomics.md`

## Objective

Measure the actual dependency, compile, and stripped-binary footprint of representative embedded Rust consumers of `eggfetch-core`, compare it with an equivalently scoped reqwest configuration, and use the evidence to correct any remaining avoidable feature/dependency ownership before the parent program claims that eggfetch is a beneficial low-footprint choice.

This is qualification and bounded tuning, not a benchmark-marketing exercise. If eggfetch is equal or larger for a given profile, record that honestly and use the data to decide whether further cleanup is justified.

## Why this plan is required

The existing benchmark suite is strong for latency, throughput, streaming, resource shape and peak RSS, but it intentionally builds a broad capability set. It does not answer the downstream embedding question: what does a small application pay in dependency count, enabled features, clean compile work, and final stripped release artifact size?

Both eggfetch and reqwest sit on substantial overlapping Hyper/Tokio/Rustls foundations. Therefore manifest inspection alone cannot establish a final-binary win. Thin LTO/dead-code elimination, root-store features, wrapper dependencies and duplicate versions all affect the result.

The parent program must not claim a slimmer build solely because reqwest disappears from a manifest.

# 1. Add a tiny downstream-style qualification fixture

Create a deliberately small Rust fixture outside the public core API implementation, preferably under `qualification/embedded/` or another existing non-published qualification/benchmark location.

The fixture should build independently enough that Cargo's dependency graph reflects what an ordinary downstream binary would select.

Keep the application behavior simple and equivalent across clients. At minimum exercise:

- construct one reusable client;
- HTTPS GET;
- request JSON serialization or equivalent serialized body where selected;
- response JSON deserialization;
- streaming response iteration.

Do not exercise cookies, proxies, compression, multipart, H2 or H3 in the minimal profile unless a specific comparison profile includes them.

No external request needs to run for the size measurement; the binary can compile paths without network access. Runtime smoke may use a loopback fixture if needed.

Acceptance:

- [ ] Fixture source remains tiny enough that most artifact weight comes from the HTTP stack, not benchmark scaffolding.
- [ ] Eggfetch and reqwest variants perform semantically equivalent operations.
- [ ] Qualification code is clearly non-product and does not create a second HTTP implementation.

# 2. Define comparison profiles before measuring

At minimum define these profiles:

### Profile A — minimal HTTPS streaming client

Eggfetch target concept:

```toml
eggfetch-core = { default-features = false, features = ["http1", "tls-rustls"] }
```

Reqwest comparison concept:

```toml
reqwest = { version = "0.12", default-features = false, features = ["stream", "rustls-tls"] }
```

### Profile B — HTTPS + streaming + JSON

Eggfetch:

```toml
features = ["http1", "tls-rustls", "json"]
```

Reqwest:

```toml
features = ["stream", "json", "rustls-tls"]
```

If eggfetch's explicit root-store feature design distinguishes WebPKI-only from native-root defaults, record both a deterministic embedded WebPKI profile and the ordinary default-root profile. Do not compare different trust behavior and label them equivalent without saying so.

Optional additional profile:

### Profile C — ordinary default Rust client

Compare each library's intended ordinary Rust experience, but keep this separate from the minimal embedding comparison because defaults intentionally include different conveniences.

Acceptance:

- [ ] Comparison profiles are documented before results.
- [ ] Protocol/TLS/JSON/streaming capabilities are aligned closely enough for the comparison to be meaningful.
- [ ] Any unavoidable semantic difference is written beside the result.

# 3. Record dependency and feature shape

For each profile capture:

```sh
cargo tree
cargo tree -e features
cargo tree -d
```

Summarize at least:

- total unique packages in the resolved downstream tree;
- major duplicate package/version families;
- direct dependencies introduced uniquely by eggfetch or reqwest;
- TLS/root-store stack differences;
- whether two major versions of a dependency such as DashMap are present when embedding in a representative larger workspace;
- optional dependencies successfully absent from the minimal eggfetch profile.

Do not use raw package count as the sole quality metric. A smaller number of large/complex crates can cost more than several tiny crates.

Acceptance:

- [ ] Evidence makes it possible to explain major size differences rather than only report bytes.
- [ ] Feature ownership corrections from prior plans are verifiably reflected in the tree.

# 4. Measure stripped release artifact size

Use reproducible release settings and record toolchain/target.

At minimum build on the primary available Linux target and, when practical, macOS. Cross-platform results are informative but not required to block the program if the environment lacks a target.

Prefer the fixture's own release profile or a documented common profile equivalent to likely downstream release settings. Record:

- target triple;
- Rust version;
- Cargo version;
- linker where materially relevant;
- release profile options (`lto`, `codegen-units`, `strip`, panic strategy if changed);
- exact eggfetch SHA;
- exact reqwest version/resolution;
- final file size before and after explicit strip if the profile does not already strip.

Where available, run `cargo bloat --release --crates` or an equivalent non-invasive analyzer and record major crate/code contributors. This tool is optional; absence must not block the plan.

Acceptance:

- [ ] Results are reproducible from documented commands.
- [ ] Raw binary sizes are attached to exact profile/toolchain/SHA metadata.
- [ ] No marketing conclusion is written without the underlying numbers.

# 5. Measure compile cost as secondary evidence

If practical, measure one clean build of each tiny fixture after clearing only that fixture's target directory or using isolated `CARGO_TARGET_DIR` values.

Record wall-clock duration and peak disk/target-dir size only as directional evidence; compile timings are noisy and should not become pass/fail criteria.

The main objective remains final artifact/dependency footprint, not winning a compile benchmark.

# 6. Bounded corrective tuning

If evidence shows avoidable eggfetch overhead, inspect causes before proceeding to compatibility freeze.

Acceptable corrections include:

- a dependency still unconditional despite being owned by a disabled capability;
- a crate feature still enabled globally despite a disabled eggfetch feature;
- duplicated TLS/root-store machinery left after the prior plan;
- dead compatibility helper dependencies accidentally present in the native minimal profile;
- avoidable duplicate major versions caused by eggfetch's own manifest choices.

Do not make corrections that:

- remove existing eggfetch capabilities from users who select them;
- weaken security defaults;
- create numerous obscure micro-features;
- replace a well-audited dependency with custom code solely to shave a small binary delta;
- change core semantics merely to beat reqwest in a benchmark.

Every corrective executable change must run normal focused tests and `./scripts/check.sh`. Because this plan occurs before the final freeze, such corrections are allowed here.

# 7. Define the parent-program success interpretation

The parent program does not require eggfetch to be smaller than reqwest in every profile.

Use these outcome categories:

### Beneficial

Eggfetch materially reduces final artifact/dependency footprint for the representative embedded profile while preserving equivalent required behavior.

### Neutral / strategically acceptable

Artifact size is close enough that ownership/control/API consolidation may justify adoption, but size is not a meaningful benefit. Documentation must say so.

### Not a footprint win

Eggfetch is materially larger for the relevant profile. Do not describe migration as slimming. Identify whether the difference is an inherent consequence of eggfetch capabilities/architecture or an avoidable issue requiring a future plan.

Do not set an arbitrary percentage threshold before observing variance. The implementation report should provide absolute bytes and percentage deltas so downstream projects can set their own policy.

# 8. Keep qualification lightweight

Add a script only if it improves reproducibility, for example:

```text
scripts/qualify-embedded-footprint.sh
```

or a script under the qualification directory.

The script should:

- build the fixed profiles;
- gather `cargo tree` outputs;
- gather artifact sizes;
- optionally invoke `cargo bloat` when installed without making it mandatory;
- write a small machine-readable result file plus a human-readable summary if that matches repository conventions.

Do not add a scheduled workflow, per-commit binary-size gate, external dashboard, or noisy CI comparison. Run this manually at relevant release/architecture review points.

# 9. Documentation/evidence record

Create or update a bounded architecture/qualification document that records the latest measured profile rather than embedding time-sensitive numbers in many README locations.

The final docs plan may link to this evidence and describe the supported minimal feature recipe.

Evidence should include exact command lines and enough metadata to distinguish results from historical measurements after dependencies/toolchains change.

## Non-goals

- no public benchmark claims without context;
- no network throughput re-benchmark unless a footprint correction plausibly affects runtime performance;
- no new routine CI matrix;
- no requirement that eggfetch beat reqwest on every target/profile;
- no custom allocator/linker requirement solely to improve numbers;
- no CodeGG migration in this plan;
- no dependency replacement whose only justification is a few kilobytes.

## Required validation

For any executable correction made during this plan:

```sh
./scripts/check.sh
```

Run the embedded qualification script/commands on a clean target directory. Run `./scripts/check.sh extended` if feature/dependency corrections touch broad supported combinations and the environment provides existing prerequisites.

Do not renew compatibility exact-SHA evidence here; this plan may still make corrective executable changes. Freeze occurs only in the next child plan.

## Exit criteria

- [ ] Tiny downstream-style fixtures exist for equivalent eggfetch/reqwest profiles.
- [ ] Minimal HTTPS and HTTPS+JSON/streaming profiles are explicitly defined.
- [ ] Dependency trees, feature trees, duplicates and stripped artifact sizes are recorded with exact toolchain/SHA metadata.
- [ ] Avoidable eggfetch overhead discovered by measurement is corrected or deliberately documented.
- [ ] The result is classified as beneficial, neutral/strategic, or not a footprint win without overstating evidence.
- [ ] Qualification remains manual/bounded rather than becoming a large CI subsystem.
- [ ] Parent program can make a truthful downstream recommendation based on measured evidence.
