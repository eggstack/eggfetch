# Post-Footprint-Reduction Requalification and Closure

Planning baseline: final executable tree after the preceding footprint child plans
Parent program: `plans/linked-binary-footprint-reduction-program.md`
Status: planned

## Objective

Freeze one final executable SHA, prove that the new lean profile reduces linked footprint without regressing the established full eggfetch product, renew the repository's required exact-SHA qualification, and update footprint/feature documentation truthfully.

This is the only plan in the program that should renew final compatibility evidence.

## Preconditions

Before freezing:

- `linked-byte-baseline-and-attribution.md` has current-main baseline evidence;
- `standard-route-advanced-routing-feature-boundary.md` is complete;
- `high-level-policy-footprint-feature-boundary.md` is complete;
- `conditional-tls-and-residual-dependency-footprint-tuning.md` is complete or explicitly closed with no executable changes because residual candidates were not justified;
- no known failing Tier 1 check remains.

If any executable/test/build/dependency change is made after the freeze, choose a new freeze SHA and rerun the affected gates.

## 1. Freeze the executable candidate

Record:

- full commit SHA;
- eggfetch-core version;
- Rust toolchain/MSRV;
- dependency lock state;
- final feature definitions relevant to the lean and compatibility profiles.

Do not bind closure evidence to a pre-commit working tree.

## 2. Final footprint comparison

Run the same fixture and settings used by the baseline plan.

Required comparison table:

```text
aligned reqwest
eggfetch 0.1.5 historical profile
eggfetch program baseline (6093a669 / current-main baseline measurement)
final eggfetch existing high-level compatibility profile
final eggfetch lean standard high-level profile
final eggfetch lean native profile where useful
```

Record:

- raw bytes;
- stripped bytes;
- percentage and absolute delta;
- package count;
- key feature set;
- top crate/symbol attribution.

The principal question is not whether the full compatibility profile becomes smaller. It is whether a consumer can now intentionally select the capabilities it needs and avoid linking the omitted machinery while the full profile remains intact.

## 3. Gregg-like behavior proof

Using the lean profile, prove:

- standard HTTP GET;
- HTTPS with WebPKI;
- Bearer auth;
- redirects not compiled/followed as selected;
- no logical retry as selected;
- explicit pool/connect/write/read/total timeouts;
- response body cap;
- typed DNS failure;
- typed connection-refused failure;
- timeout classification;
- ordinary network error;
- status and body consumption;
- connection reuse;
- cancellation.

No Gregg-specific type or source dependency belongs in eggfetch. The fixture is requirements evidence only.

## 4. Full-capability regression proof

Existing compatibility/default profiles must retain:

- advanced routing (Dialer, resolved target, SNI override, local address/socket options, UDS);
- logical retries;
- redirects;
- Basic and Bearer auth;
- TLS custom CA/additional CA/mTLS/PEM conveniences;
- proxy/CONNECT/SOCKS;
- cookies/compression/multipart/JSON;
- H2;
- experimental H3 behavior;
- Python/CLI/FFI/Node adapter feature expectations.

Run focused tests for each area affected by cfg movement.

## 5. Repository verification

Run the repository's current policy, including as applicable:

```sh
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
./scripts/check_security.sh
```

Tier 2 must include exact Rust 1.89.0 MSRV per current repository policy.

Use the repository's existing feature matrix. Add new lean-profile compile/runtime coverage to the existing appropriate validation location; do not create a new CI workflow/job/matrix solely for footprint.

## 6. Exact-SHA compatibility renewal

Because plans 2-4 change executable core and public Cargo feature composition, renew the exact-SHA compatibility evidence required by the current repository process for:

- HTTPX 0.28.1;
- HTTPX2 2.12.0;
- API oracles/manifests where required;
- the required consecutive compatibility runs.

The compatibility/default profiles, not the new lean profile, are the subject of historical HTTPX behavior parity.

Do not change compatibility behavior merely to make a feature split easier.

## 7. Documentation

Update at minimum:

- `docs/architecture/feature-flags.md`;
- `docs/architecture/dependency-policy.md`;
- `docs/architecture/embedded-footprint.md`;
- relevant Rust API/embedding documentation;
- `AGENTS.md` if canonical feature recipes changed;
- `plans/README.md` program status.

Document:

- the existing full/default recipe;
- the new lean standard high-level recipe;
- the new lean native recipe if distinct;
- which capabilities each intentionally omits;
- measured stripped artifact evidence;
- that existing full capabilities remain available;
- any residual footprint difference that remains.

Do not market a footprint win beyond the measured targets/toolchains.

## Acceptance criteria

- [ ] One final executable SHA is recorded.
- [ ] Final lean profile performs the real Gregg-like request path.
- [ ] Standard-route and policy omissions are compile-time real, not only runtime `None` values.
- [ ] Existing compatibility/default feature surfaces retain their previous APIs/behavior.
- [ ] Full advanced-routing, retry, redirect, auth, TLS, proxy, adapter and protocol regressions pass.
- [ ] Tier 1 passes.
- [ ] Extended verification including Rust 1.89.0 passes.
- [ ] Package validation passes.
- [ ] Security preflight passes where required by release policy.
- [ ] HTTPX 0.28.1 and HTTPX2 2.12.0 exact-SHA evidence is renewed.
- [ ] Final stripped-byte tables use the same measurement conditions as baseline.
- [ ] Documentation distinguishes dependency-count reduction from linked-byte reduction.
- [ ] Any remaining delta versus reqwest is explained rather than hidden.
- [ ] No new routine footprint CI infrastructure was added.

## Closure classification

Classify the program based on evidence:

### Material linked-footprint improvement

Use when the lean profile recovers a meaningful portion of the historical eggfetch overhead while preserving the selected behavior.

### Ownership improvement with modest byte reduction

Use when feature boundaries are materially more truthful/useful but final byte reduction is limited.

### No justified further reduction

Use if the remaining footprint is dominated by shared Hyper/Tokio/Rustls or audited engine behavior whose removal would make the feature surface unsafe/fragmented.

Any of these outcomes is acceptable if measured and documented truthfully.

## Program closure rule

Once the executable freeze passes qualification, only documentation/profile/index changes may follow without selecting a new executable freeze. Any later Rust source, test, Cargo feature/dependency, validation-script, or fixture change that affects the evidence requires re-running the affected closure gates.
