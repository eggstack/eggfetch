# Rust Surface Containment and Experimental Adapter Hygiene

Planning baseline: `c53eebc47569279b61c0611d4523c5132bdbcaeb`
Parent program: `plans/api-preserving-private-architecture-containment-program.md`
Prerequisite: `plans/python-streaming-and-cli-private-decomposition.md`
Normative verification policy: `docs/verification-policy.md`

## Objective

Prevent further accidental expansion of low-level Rust implementation APIs that
are already public, and reconcile experimental-adapter metadata/support-status
drift without removing, renaming, moving, or broadening any existing public
surface or capability.

This is a containment plan, not an API redesign. Several low-level items are
already part of the mechanically captured `eggfetch-core` public surface.
They must now be treated as compatibility obligations even when they are more
implementation-oriented than ideal.

## Part A — inventory existing low-level public exposure

Create a reviewed inventory document or machine-readable section under the
existing compatibility/documentation structure that names low-level public
areas whose exposure should not grow accidentally.

At minimum audit:

- `eggfetch_core::transport::alt_svc`;
- `eggfetch_core::transport::direct_connector`;
- `eggfetch_core::transport::lifecycle`;
- `eggfetch_core::transport::metrics`;
- `eggfetch_core::transport::dialer`;
- `Pool`, `PoolConfig`, `PoolMetrics`, `PoolGuard`;
- transport metrics/counter fields and public record/snapshot methods;
- feature-gated H3 diagnostics/types;
- historical compatibility re-export paths recorded by the Rust API oracle.

The inventory must distinguish:

1. intended user-facing native API;
2. historical low-level exposure that is supported because it already exists;
3. test-only exposure gated by `test-util`;
4. private/`pub(crate)` internals.

Do not change visibility while creating the inventory.

## Part B — freeze Alt-Svc/H3 support boundaries

`transport::alt_svc` is publicly addressable and exposes cache/state types,
routing-suppression types and constants. HTTP/3 itself remains experimental.

Requirements:

- preserve every existing public item/path/signature/feature relationship;
- do not add new public Alt-Svc/H3 cache mutation, parser, route-selection,
  connection-driver, or test-injection API;
- keep H3 transport implementation modules private where they are currently
  private;
- do not move currently public Alt-Svc items behind the `http3` feature if
  they are currently available in broader profiles;
- do not make additional H3 implementation types public merely to improve
  internal module boundaries;
- keep `test-util`-gated public test hooks exactly as currently captured.

Document that experimental protocol maturity does not erase Rust semver/source
compatibility obligations for already-public low-level types.

## Part C — contain mutable metrics/public counter growth

Audit `TransportMetrics`, `TransportSnapshot`, `PoolMetrics` and related
public methods/fields.

Existing public mutable counters/methods must remain unchanged under this
program. The goal is to prevent new ones from appearing accidentally.

Add a targeted source/contract check only if the exact public API oracle alone
does not provide sufficiently fast/local feedback. Prefer one of:

- expand `public_api_contracts.rs` with representative path/type checks;
- add a small reviewed denylist/allowlist assertion to
  `scripts/check_rust_public_api.py`;
- add a focused source-visibility guard integrated into an existing validation
  tier.

Do not create a second full Rust API manifest framework.

The guard should fail if a refactor introduces new public implementation
helpers in the audited modules without an explicit versioned API plan.

## Part D — verify feature-profile containment

Re-run the exact public API snapshots for all already-supported profiles:

- default;
- `http1,tls-rustls`;
- `http2,tls-rustls`;
- `standard-http1,tls-rustls`;
- `native-http1,tls-rustls`;
- all-features.

Audit that decomposition work did not accidentally:

- expose a type under a leaner feature profile;
- make an existing public type disappear when a feature is absent;
- transitively enable `advanced-routing`, retry, redirects, Basic auth or
  proxy capability in a profile where it was previously absent;
- change default features.

No feature alias/recipe simplification belongs in this plan.

## Part E — FFI exposure sanity check

The C ABI is currently explicit and reasonably bounded. This plan should only
verify, not redesign it.

Confirm:

- exported symbol set is unchanged;
- handle ownership/consumption rules are unchanged;
- panic guards still prevent unwind across the C ABI;
- null/error sentinels are unchanged;
- FFI feature defaults/forwarding do not accidentally expand core exposure;
- no new C ABI symbol is introduced as a convenience for other refactors.

If the repository lacks a lightweight exported-symbol regression check and one
can be added without platform-specific fragility, add it through existing
validation conventions. Otherwise rely on existing FFI tests and document the
decision.

## Part F — reconcile Node prototype metadata without maturing Node

At baseline:

- Rust crate `eggfetch-node` is version `0.1.9`;
- `crates/eggfetch-node/package.json` reports `0.1.0`;
- workspace/repository license is MIT;
- `package.json` reports `MIT OR Apache-2.0`;
- `index.d.ts` intentionally exports nothing;
- npm publication is not supported.

Reconcile metadata so it truthfully represents the existing prototype without
creating a release promise.

Default target:

- align package version with the coordinated workspace release version unless
  repository release policy explicitly defines Node as independently
  versioned;
- align license metadata with the repository's actual MIT license;
- retain the package description's experimental/unsupported wording;
- retain declaration-free `index.d.ts`;
- retain no npm publish workflow;
- retain manual native-artifact loading behavior.

Add a small validation check only if it can reuse the existing
release/package-validation framework. Do not invent a Node release subsystem.

## Part G — preserve Node support boundary

Do not implement any of the deferred supported-binding work:

- direct async `eggfetch-core` request dispatch;
- Rust-owned `Arc` in-flight lifetime redesign;
- binary/stream request bodies;
- incremental response streaming;
- cancellation/AbortSignal;
- structured stable errors;
- generated TypeScript declarations;
- npm publication.

Those are capability/API additions and require a separate product plan.

Tier 1 may continue treating the JS runtime test as an explicit skip when no
native artifact is present, provided the Rust-side Node crate still builds/
tests and the skip remains visible.

## Part H — reaffirm HTTP/3 experimental status

Review the current H3 qualification evidence and documentation only to ensure
the maintenance campaign has not accidentally changed the status claim.

Retain experimental status unless a separate graduation program proves all of
its existing gate requirements. In particular, this plan does not satisfy
independent interoperability, public-origin Alt-Svc, network impairment or
upstream-risk graduation requirements merely by running normal tests.

Do not change the protocol label based on refactoring success.

## Validation

Required:

```sh
./scripts/check.sh
./scripts/check.sh extended
```

The extended tier already invokes `scripts/check_rust_public_api.py`; use its
exact baseline and do not update snapshots.

Also run:

```sh
cargo test -p eggfetch-ffi --all-features
cargo test -p eggfetch-node --all-features
cargo test -p eggfetch-core --test public_api_contracts --all-features
```

Run package validation if Node metadata or package-content validation changes.

## Acceptance criteria

- [ ] Existing Rust public API snapshots have zero unexplained difference.
- [ ] No audited low-level module gains a new public implementation helper.
- [ ] Existing Alt-Svc/H3 public paths remain exactly available as before.
- [ ] HTTP/3 remains explicitly experimental.
- [ ] Existing mutable metric/counter surface is unchanged; no adjacent public
      counter/mutator is added.
- [ ] Feature-profile public exposure remains unchanged.
- [ ] C ABI exported surface and ownership semantics remain unchanged.
- [ ] Node package metadata is internally consistent with the repository and
      coordinated versioning policy.
- [ ] Node remains explicitly experimental and unsupported as a stable npm
      binding.
- [ ] No generated TypeScript declarations or npm publish pipeline is added.
- [ ] Tier 1 and extended validation pass.
- [ ] No compatibility/API waiver is introduced.

## Stop conditions

Stop and split a separate versioned API/product plan if the work would require:

- hiding/removing/moving an existing public Rust item;
- changing feature exposure;
- adding a new public low-level Rust helper;
- changing C ABI symbols or semantics;
- defining a supported Node API;
- changing Node request/response capability;
- graduating HTTP/3;
- regenerating API snapshots to make a refactor pass.

Containment means preserving today's surface while making further accidental
growth harder.

## Implementation record

Completed in the current qualification candidate:

- the audited low-level public surfaces are inventoried in
  `docs/architecture/rust-surface-containment.md`;
- the exact six-profile Rust API oracle and semver cross-check pass without
  snapshot regeneration;
- FFI and Node Rust tests pass;
- Node package metadata now aligns with coordinated version `0.1.9` and the
  repository MIT license while retaining experimental/no-npm/no-types status;
- HTTP/3 remains experimental and no new low-level public helper was added.

The final executable freeze SHA and remote CI result are recorded by the
closure plan after the qualification commit.
