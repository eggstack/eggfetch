# Post Native HTTP Body and TLS Extensibility Qualification and Closure

Planning baseline: `45c08e0e7587eb1e8f713d49e6f7902478c27a35` (`main`, 2026-09-14; eggfetch 0.1.4)
Parent program: `plans/native-http-body-and-tls-extensibility-program.md`
Status: complete; closure freeze `fdfe060`

## Objective

Perform the single integration/qualification pass for the native HTTP-body and TLS-extensibility program, correct only defects found by that integration audit, freeze one exact executable/test/fixture SHA, renew compatibility evidence on that frozen tree, and then reconcile documentation/plan status without changing executable behavior after the freeze.

This plan owns program closure. Individual child plans must not independently claim that current HTTPX/HTTPX2 exact-SHA compatibility evidence remains valid after their executable changes.

## Inputs that must be complete before this plan starts

The following child plans must have implementation and focused tests complete:

1. `plans/native-http-body-interoperability.md`;
2. `plans/tls-crypto-provider-extensibility.md`;
3. `plans/tls-additional-trust-anchors.md`.

Their unchecked acceptance items are blockers. Do not convert missing evidence into a pass merely because ordinary workspace tests are green.

## Closure principles

The program is successful only if eggfetch gained generic native-engine capabilities without becoming specialized for its motivating downstream.

Audit against these questions first:

- Would the new public APIs still make sense for an unrelated Rust gateway, service mesh, private-PKI client or custom-network client?
- Can an ordinary existing eggfetch user ignore every new API and retain current source/behavior semantics?
- Did implementation avoid adding public Hyper client/connector/`Incoming` types?
- Did it avoid new public variants in externally matchable `RequestBody`, `ResponseBody` and `TrustStore` enums?
- Did provider support remain an injected Rustls concept rather than a Synvoid/AWS-LC product mode?
- Did additional trust remain explicit rather than broadening default trust silently?
- Does frame-preserving execution share eggfetch's existing HTTP/TLS/pool/lifecycle engine rather than duplicating it?

A negative answer is an architectural defect to correct before freeze.

## 1. Source/API boundedness audit

Inspect the full executable diff from the program baseline through the candidate closure tree.

Flag and correct:

- downstream names in public types/features/modules/doc examples other than historical planning motivation;
- `synvoid`, `gateway`, `waf`, `site`, `backend`, or reverse-proxy-specific policy embedded in generic core APIs;
- accidental new public enum variants that create exhaustive-match breakage;
- exported Hyper connector/client/`Incoming` internals;
- duplicated route/connector/TLS construction added solely for native-body execution;
- provider-brand branching in generic TLS code where `CryptoProvider`/`KeyProvider` should suffice;
- repurposed custom-CA APIs whose semantics changed for existing callers;
- implicit default-trust broadening;
- new dependencies/features not justified by a real semantic boundary.

Use semver/source-compatibility tools already available to the repository if present, but do not add a new validation framework solely for this audit.

## 2. HTTP-body integration audit

Prove the native frame path and existing high-level byte path coexist correctly.

At minimum verify:

- arbitrary request `Body<Data = Bytes>` is polled incrementally;
- DATA and trailer frames cross the native request boundary intact;
- native response DATA and trailers remain frames;
- body errors retain actionable source/classification information;
- dropping/cancelling request or response bodies does not leak logical pool permits;
- read timeouts apply while waiting for body frames/trailers;
- established transport I/O inactivity remains a separate lower-level control;
- strict Hyper stale-idle retry setting remains independent from logical application retries;
- one-shot native request bodies are never replayed by eggfetch redirect/retry policy;
- `wrap_incoming()` no longer treats an ignorable/future frame as EOF if the child plan identified that defect;
- existing high-level `bytes()`, `bytes_stream()`, `raw_bytes_stream()`, decompression and `trailers()` behavior stays unchanged;
- 101/CONNECT behavior follows the documented supported/unsupported contract with no deadlock.

Exercise all supported native-body route combinations recorded by the child plan. Unsupported combinations must reject before network I/O.

## 3. TLS-provider integration audit

Verify one coherent provider reaches the complete TLS configuration.

At minimum:

- explicit provider selection remains local to the `TlsConfig` and does not mutate the process-global provider;
- ordinary no-explicit-provider clients retain the pre-program default behavior;
- mTLS private-key loading uses the selected provider's `key_provider` or an equivalent standard Rustls builder path;
- verification-disabled and hostname-only-disabled verifier paths report/use algorithms from the selected provider;
- standard, direct/resolved, SNI, custom-dialer and UDS TLS paths share provider semantics;
- origin TLS and HTTPS proxy TLS can intentionally use independent configs/providers;
- H3 behavior matches the documented outcome: supported with the explicit provider where proven, or fail-before-I/O for unsupported provider combinations;
- alternate-provider qualification does not force a second provider into the ordinary default dependency graph.

Inspect `cargo tree -e features` for representative ordinary and alternate-provider builds and record the relevant result in closure evidence.

## 4. Additional-trust integration audit

Verify base trust and additive roots retain distinct meanings.

At minimum:

- no-additional-roots behavior is byte-for-byte/configuration-equivalent where practical;
- replacement CA methods still replace defaults;
- `NativeWithWebPkiFallback` selects/falls back first, then overlays extras;
- `NativeOnly` still requires native roots even when extras exist;
- `WebPkiOnly` retains WebPKI plus explicit extras;
- `Custom` remains an explicit replacement base and can intentionally receive extras;
- malformed extras fail before network I/O;
- private roots do not bypass hostname/SNI validation;
- explicit provider + additional roots works together;
- origin roots never bleed into HTTPS proxy trust unless the proxy leg is explicitly configured with that TLS policy;
- H1/H2 and supported H3 routes observe one authoritative final trust store.

## 5. External-style consumer qualification

Run the generic downstream-style fixture(s) added by the child plans without importing an actual downstream repository.

The combined qualification should prove, in one realistic local scenario where practical:

1. a caller owns an `http_body::Body` request and streams it through eggfetch;
2. eggfetch owns routing/pooling/TLS;
3. an explicit caller-supplied `CryptoProvider` is used;
4. public/default roots are augmented with a private test CA rather than replaced;
5. the response is consumed as an `http_body::Body` with DATA + trailers;
6. cancellation/early body drop releases logical resource permits;
7. custom dialer or resolved-target routing composes with the same API;
8. the fixture uses only published/native `eggfetch-core` APIs.

Do not name the fixture after Synvoid. A name such as `embedded-http-body-tls` or `native-gateway-client` is appropriate if repository conventions permit it.

The fixture exists to prove that the public surface is independently usable. It is not a benchmark or a routine CI matrix.

## 6. Regression and feature-matrix checks

Run focused tests from all three child plans first. Then run the repository's canonical gates.

Minimum ordinary checks:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Feature/provider checks should include at least:

```sh
cargo check -p eggfetch-core --no-default-features --features http1
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,tls-native-roots
cargo check -p eggfetch-core --no-default-features --features http1,http2,tls-rustls,tls-native-roots
```

Run existing HTTP/3 checks required by the repository if TLS-provider internals or QUIC Rustls construction changed. Do not treat passing repository H3 tests as production-graduation evidence.

Run the alternate-provider qualification profile separately. If its dependencies/toolchain are unavailable on one machine, record the missing prerequisite and run it in a suitable environment before declaring this program closed.

## 7. Footprint/dependency evidence

The prior embedded qualification truthfully records that eggfetch is not an intrinsic binary-size win against aligned reqwest builds. Do not erase or reverse that classification simply because this program is motivated by downstream code deletion.

After implementation:

- inspect the ordinary minimal/default dependency trees for accidental new dependencies;
- re-run the existing embedded footprint runner if executable feature/dependency changes plausibly affect its recorded profiles;
- add a bounded native-body/provider fixture measurement only if it answers a useful question;
- distinguish "downstream may delete duplicated code" from "eggfetch binary itself became smaller";
- update the dated footprint record only when comparable measurement was actually rerun.

No acceptance threshold requires eggfetch to beat reqwest on stripped bytes.

## 8. Freeze one executable/test/fixture SHA

After all executable corrections and qualification fixtures are final:

1. ensure the worktree is clean;
2. record the exact commit SHA containing all executable code, tests and qualification fixtures;
3. from this point onward, do not modify Rust/Python/Node/C/fixture behavior until compatibility requalification completes;
4. any executable/test/fixture correction discovered afterward invalidates the freeze and requires a new SHA and repeat of the affected gates.

Documentation/profile/ledger updates may follow the freeze only when they do not change executable/test/fixture behavior.

## 9. Exact-SHA compatibility requalification

The program changes core body/TLS internals even though compatibility facades are intended to remain behaviorally unchanged. Renew the repository's existing HTTPX 0.28.1 and HTTPX2 2.12.0 evidence according to current repository procedure on the frozen SHA.

At minimum follow the current exact-SHA policy in `plans/httpx-parity-correction-status.md`, `compat/httpx/0.28.1/profile.toml`, `compat/httpx2/2.12.0/profile.toml`, and the repository's existing API-oracle/compatibility scripts.

Requirements:

- do not copy forward prior pass status without rerunning required checks;
- bind both compatibility profiles to the same frozen executable SHA if both pass;
- record any skipped external prerequisite explicitly rather than converting it into a pass;
- keep HTTP/3 qualification status separate from HTTPX compatibility status;
- do not broaden facade APIs merely because the native Rust engine gained capabilities.

## 10. Documentation and plan-index closure

Only after the frozen tree has passed required qualification:

Update relevant final-state documentation, including:

- `docs/architecture/core-body-streaming.md`;
- `docs/architecture/core-engine.md`;
- `docs/architecture/core-tls-proxy-protocols.md`;
- `docs/architecture/dependency-policy.md` if provider/dependency boundaries changed;
- `docs/architecture/feature-flags.md` if feature ownership changed;
- `docs/rust/guide.md`;
- `docs/architecture/embedded-footprint.md` only if remeasured;
- `plans/README.md`;
- `plans/ROADMAP.md`.

Mark the parent and child plans complete with the frozen executable SHA and relevant evidence. Keep implementation plans as historical records after closure.

Do not edit executable source after the freeze in the documentation pass. If documentation review reveals an executable defect, fix it and repeat the freeze/requalification sequence.

## 11. Handoff to downstream evaluation

Downstream migration is outside this repository. Program closure should merely state what the generic public engine now supports.

The downstream-facing handoff criteria are:

- generic native frame-preserving request/response bodies are available;
- explicit TLS provider selection is available without a process-global requirement;
- additional private roots can augment base trust explicitly;
- these capabilities compose with the already completed custom-dialer/resolved-target/lifecycle controls;
- no downstream-specific types or policies were introduced.

Do not add Synvoid migration instructions to eggfetch's core architecture docs. A short generic native-embedding example in the Rust guide is sufficient.

## Final acceptance criteria

- [x] All child-plan acceptance criteria are closed or explicitly superseded by documented equivalent implementation.
- [x] Public API review finds no downstream-specific surface and no unnecessary source break.
- [x] Native HTTP-body external fixture passes with frame/trailer/backpressure fidelity.
- [x] Alternate explicit TLS provider fixture passes without mutating the process-global provider.
- [x] mTLS key loading follows the selected provider.
- [x] Additional trust anchors compose with all selected base trust policies without changing replacement CA semantics.
- [x] Existing default high-level clients remain behaviorally compatible.
- [x] Required Tier 1, extended and package checks pass on the candidate executable tree.
- [x] Representative feature matrices pass.
- [x] Dependency/footprint documentation remains truthful.
- [x] One exact executable/test/fixture SHA is frozen.
- [x] HTTPX 0.28.1 and HTTPX2 2.12.0 compatibility evidence is rerun and rebound only if it passes on that SHA.
- [x] Post-freeze changes are documentation/profile/ledger-only.
- [x] `plans/README.md` and `plans/ROADMAP.md` record final status.

## Non-goals

- no downstream migration implementation;
- no performance target invented after the fact;
- no requirement to beat reqwest in binary size;
- no new routine CI matrix;
- no HTTP/3 production graduation;
- no broad compatibility-facade expansion;
- no API redesign unrelated to the three child plans;
- no release/publication step as part of this implementation program.
