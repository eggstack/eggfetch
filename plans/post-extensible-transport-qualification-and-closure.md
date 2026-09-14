# Post-Extensible-Transport Qualification and Closure

Planning baseline: `475bd50f6f9b9f66b95eea814f06b4adb22ede93` (`main`, 2026-09-13; eggfetch 0.1.4)
Parent program: `plans/extensible-embedded-transport-consumer-program.md`
Depends on: executable completion of the custom-dialer, strict-attempt-control, and physical-connection/I/O-guardrail plans
Status: complete; final qualification and documentation closure recorded 2026-09-14

## Objective

Close the extensible embedded transport program with evidence that the new native engine surface is independently usable, preserves existing compatibility/default behavior, does not introduce unjustified dependency or footprint growth, and is documented accurately.

This is the only plan in the program that should freeze an executable SHA and renew exact-SHA compatibility evidence. Earlier child plans intentionally leave the compatibility ledger stale while executable work is still moving.

## Closure principles

1. Qualify eggfetch itself, not a specific downstream repository.
2. Use a tiny synthetic external-style Rust consumer to prove the public API; do not add Eggpool, Eggress or another application as an eggfetch dependency.
3. Treat size/dependency results as measurements, never as assumed benefits.
4. Do not add routine CI jobs or external services for one qualification event.
5. Freeze one clean executable/test/fixture SHA only after all behavior changes are complete.
6. Any post-freeze executable change invalidates the freeze and requires rerunning the affected closure gates.
7. Documentation/profile/ledger-only descendants may follow the freeze if they are audited as non-executable.

## 1. Audit child-plan closure before freezing

Review the final implementation against every cross-program invariant in `extensible-embedded-transport-consumer-program.md`.

At minimum verify from source and tests that:

- custom dialing is a generic raw-stream boundary;
- no downstream/proxy protocol names entered the public engine API;
- custom dialing preserves eggfetch-owned HTTP/TLS identity;
- custom dial failure cannot fall back to direct networking;
- ambiguous route combinations fail before I/O;
- Hyper canceled-request retry control exists and preserves the previous default;
- all Hyper client construction paths receive the configured lower-level retry policy;
- physical admission is distinct from logical request concurrency;
- transport I/O inactivity is distinct from existing request/body timeout semantics;
- HTTP/3 is not claimed to support features that remain Hyper-only;
- no unrelated dependency upgrade was smuggled into the implementation.

Run targeted source searches for route/build-policy drift and record any intentional exception in this plan's closure evidence rather than leaving an unexplained path.

## 2. Add an external-style native consumer fixture

Create a small fixture under the existing qualification/fixture conventions, for example:

```text
qualification/embedded-custom-dialer/
    Cargo.toml
    src/main.rs
    README.md
```

The fixture should depend on `eggfetch-core` through the repository path exactly as an external Rust application would, using only public exports.

It should exercise:

1. build a reusable client with `default-features = false`, `http1,tls-rustls`;
2. install a synthetic `Dialer` that maps a logical hostname to a local test endpoint;
3. perform a finite HTTPS or HTTP request and stream the response incrementally;
4. set strict canceled-request retry control;
5. configure a physical connection cap;
6. configure transport I/O inactivity guardrails;
7. inspect only documented public error/query APIs;
8. compile without reaching into `eggfetch_core::transport` crate-private types.

The fixture is a compile/runtime qualification artifact, not a new supported example surface or a downstream-specific adapter.

If running a local TLS endpoint makes the fixture disproportionately complex, keep the fixture HTTP-only and rely on core integration tests for HTTPS/SNI; the public compile boundary is the primary purpose here.

## 3. Public API review

Perform a focused source/API audit before freezing.

Check:

- new public types have complete rustdoc and safe `Debug`;
- no public API exposes Hyper/Tower connector implementation details unnecessarily;
- builder method names distinguish logical retry/pooling from physical controls;
- no existing method changed signature;
- no existing public struct field changed meaning;
- no existing enum variant payload changed;
- any newly added enum variant or exhaustiveness change has an explicit compatibility rationale;
- custom dialer error/source inspection is usable without parsing display strings;
- native APIs remain idiomatic Rust rather than reflecting one downstream's configuration schema.

Run whatever existing public-surface/API oracle the repository already uses; do not invent a second API snapshot framework.

## 4. Feature/dependency audit

Record before/after dependency shape for the relevant minimal profiles.

At minimum:

```sh
cargo tree -p eggfetch-core --no-default-features --features http1
cargo tree -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo tree -p eggfetch-core --all-features
cargo tree -d
```

Expected outcome:

- custom dialer/physical admission/I/O guardrails should require no new runtime dependency;
- no proxy/H2/H3/compression/cookie/multipart dependencies appear in the minimal HTTP/1 + TLS profile merely because the new API exists;
- any new dependency must be explicitly justified in the closure record and `docs/architecture/dependency-policy.md` if policy requires it.

Do not treat fewer packages as proof of a smaller binary.

## 5. Footprint qualification

Re-run the existing embedded footprint runner or extend its fixture minimally so the record can answer two separate questions:

1. Did the new always-available engine surface materially change an ordinary minimal consumer that does **not** use the new controls?
2. What is the incremental stripped size of a tiny consumer that actually uses custom dialing plus the strict resource controls?

Use the same target/toolchain/profile methodology as `docs/architecture/embedded-footprint.md` when available so results are comparable. If the exact prior host/toolchain is unavailable, record the new environment and do not present cross-host byte deltas as exact regression numbers.

No hard byte gate is required unless a clearly accidental large regression appears. The expected outcome is truthful evidence, not a forced slimming result.

Do not rewrite the existing conclusion that eggfetch is not presently a footprint win against aligned reqwest unless new equivalent measurements genuinely change it.

## 6. Focused behavior qualification

Before broad gates, run the new deterministic regressions together:

- custom dialer direct/no-fallback tests;
- HTTPS logical Host/SNI/certificate tests;
- route-conflict tests;
- stale-idle default/strict/explicit-retry tests;
- physical admission/reuse/idle-retention tests;
- transport I/O progress/timeout tests;
- cancellation/resource-release tests;
- HTTP/2 physical-vs-logical concurrency test where feature is available;
- H3 incompatibility/fail-closed tests.

A failure here should be fixed and rerun before freezing, rather than hidden inside the broad suite.

## 7. Canonical repository validation

Run the repository's normative gates from a clean worktree:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Respect existing explicit skips documented by `docs/verification-policy.md`; do not convert a missing optional prerequisite into a fabricated pass.

Package validation must remain local/non-publishing.

## 8. Compatibility requalification

Because Plans 1–3 change native executable transport behavior and potentially common client construction, renew the existing HTTPX 0.28.1 and HTTPX2 2.12.0 exact-SHA qualification on one frozen executable/test/fixture commit using the repository's current established procedure.

Use the existing compatibility profiles and oracles. Do not change compatibility target versions as part of this program.

Required sequence:

1. establish a clean candidate commit after all executable/test/fixture changes;
2. run Tier 1/extended/package as above;
3. run the existing full pinned compatibility suite according to current policy/plan convention;
4. run the existing API oracles;
5. if current repository convention requires consecutive successful compatibility runs for an executable freeze, perform the same number used by the live profile rather than inventing a weaker criterion;
6. bind the live compatibility status/profile evidence to the exact candidate SHA;
7. only then call that SHA the closure freeze.

If any executable fix lands after this point, discard the binding and repeat from the new candidate.

## 9. Documentation truth pass

After executable evidence is green, update documentation to describe exactly what landed.

At minimum audit/update:

- `README.md` only if the native capability belongs in the concise feature overview;
- `docs/rust/guide.md` with custom-dialer/resource-control examples;
- `docs/architecture/core-engine.md` with final route/build/lifecycle boundaries;
- `docs/architecture/core-timeout-pool.md` with logical vs physical vs I/O timeout semantics;
- `docs/architecture/core-tls-proxy-protocols.md` with custom dialer composition/fail-closed exclusions;
- `docs/architecture/embedded-footprint.md` with dated measurement evidence if rerun;
- `docs/architecture/feature-flags.md` if feature ownership changed;
- `plans/README.md` and `plans/ROADMAP.md` to mark the program complete/current;
- compatibility status/profile files to point at the frozen SHA.

Do not advertise a downstream integration as complete. The closure claim is only that eggfetch now exposes and qualifies the general-purpose engine surface needed by embedded consumers.

## 10. Descendant audit

If documentation/profile/plan updates are committed after the frozen executable SHA, inspect every descendant commit before final closure and prove it changes only documentation/profile/ledger/plan data.

Any Rust/Python/JS executable source, tests that affect qualification fixtures, manifests that change dependencies/features, build scripts or generated executable assets invalidate the freeze.

Record the final executable SHA and final documentation descendant SHA separately if they differ.

## 11. Downstream handoff note

Add a short native integration note for downstream maintainers describing the intended boundary without naming one application as normative architecture:

```text
application routing policy
    -> construct one client per route/isolation domain as needed
    -> optional custom Dialer supplies raw stream
    -> eggfetch owns destination HTTP/TLS/pool/timeout behavior
```

The note should call out the controls an orchestrator typically needs:

- explicit lower-level canceled-request retry setting;
- physical connection policy;
- transport I/O inactivity policy;
- ordinary eggfetch `RetryPolicy` remains separately configurable.

Do not add downstream config parsing, provider/account models or protocol implementations to eggfetch.

## Non-goals

- no migration of Eggpool, CodeGG or another repository;
- no new routine CI workflow/matrix;
- no external-network qualification requirement;
- no release/tag/publication as an automatic consequence of closure;
- no change to HTTPX target versions;
- no H3 graduation;
- no binary-size promise;
- no dependency bump solely to improve cosmetic tree counts.

## Completion record to fill during execution

At closure, append a concise evidence section containing:

- executable freeze SHA;
- documentation descendant SHA if applicable;
- focused test commands/results;
- Tier 1/extended/package results;
- compatibility profile/oracle results;
- fixture compile/runtime result;
- minimal and custom-dialer footprint measurements with environment metadata;
- dependency-tree conclusion;
- any retained limitations or deferred follow-ups.

## Exit criteria

- [x] all three executable child plans satisfy their exit criteria;
- [x] external-style public consumer fixture compiles and runs without private APIs;
- [x] minimal feature profiles remain clean and expected dependencies are absent;
- [x] footprint impact is measured and recorded truthfully;
- [x] focused transport regressions pass;
- [x] Tier 1, extended and package validation pass under existing policy;
- [x] exact-SHA HTTPX/HTTPX2 compatibility evidence is renewed on the final executable tree;
- [x] documentation reflects the actual public API and limitations;
- [x] post-freeze descendants are audited as non-executable or the freeze is rerun;
- [x] plan index/roadmap mark the program complete only after evidence exists.

## Closure evidence (2026-09-14)

- **Executable freeze:** `43c68bd1bcff45301fc8b6b163b6b6e06d98a786`.
  The only post-`6eb9f3f` executable correction was the RSS monitor's delta
  headroom, widened from 50 MiB to 64 MiB while retaining the independent
  100 MiB absolute peak cap. The final clean Tier 1/2/3 runs are bound to this
  SHA.
- **Focused behavior:** the custom dialer, route-conflict, TLS identity,
  stale-idle retry, physical admission/reuse/idle-retention, transport-I/O
  progress/timeout, cancellation, H2, and H3 incompatibility suites passed.
  In the final all-features run, the relevant counts were retry 10, transport
  metrics 8, direct transport 17, H2 22, H3 hardening 12, H3 Alt-Svc 17, and
  H3 interop controls 20; the full core test run passed 691 unit tests.
- **Canonical gates:** Tier 1 passed; Tier 2 passed with only the documented
  Node artifact, local MSRV/Cargo, and downstream artifact skips; Tier 3
  passed package dry-runs, wheel build/smoke, and content validation. The
  local MSRV check is explicitly not an MSRV pass because installed Rust 1.80
  Cargo cannot parse the current crates.io resolution.
- **Compatibility and API oracles:** three consecutive full runs for both
  versioned facades passed, each with 1,870 tests and 26 warnings. HTTPX 0.28.1
  generated 71 allowed differences and httpx2 2.12.0 generated 79; both API
  oracles were clean. The profiles now bind to the executable freeze above.
- **External-style fixture:**
  `qualification/embedded-custom-dialer/` compiled and ran using only public
  `eggfetch-core` exports, with HTTP streaming, strict retry, physical
  admission, and transport-I/O controls. The fixture remains intentionally
  HTTP-only; TLS/SNI behavior is covered by core integration tests.
- **Footprint:** on `x86_64-unknown-linux-gnu`, `rustc/cargo 1.98.1`, linker
  `cc 13.3.0`, isolated release builds measured eggfetch/reqwest stripped
  minimal WebPKI at 3,643,952/3,079,840 bytes, native roots at
  3,678,296/3,116,968, JSON WebPKI at 3,769,600/3,223,256, and JSON native
  at 3,804,008/3,256,288. The control fixture was 4,369,848 stripped bytes
  (6,077,008 unstripped; 117 unique packages). Classification remains **not a
  footprint win**; values are not cross-host regressions.
- **Dependency conclusion:** the new controls add no runtime dependency;
  minimal trees remain free of optional proxy/H2/H3/compression/cookie/
  multipart crates. No unrelated dependency pins or upgrades were retained.
- **Descendant audit:** documentation/profile/plan closure is a separate
  descendant of the executable freeze and contains no Rust/Python/JS source,
  tests, manifests, build scripts, or generated executable assets. The final
  documentation-only closure began at descendant
  `e81d9e0453ee872a279b27ffacb5739e6349fad8`; this plan record is also part
  of the audited documentation-only descendant.

Retained limitations are unchanged: HTTP/3 remains experimental, the local
MSRV environment is unsupported for a definitive Rust 1.80 graph check, and
no downstream application migration is included.
