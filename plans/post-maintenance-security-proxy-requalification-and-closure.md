# Post-Maintenance, Security, and Proxy Requalification and Closure

Planning baseline: `025b1a5a6a94b017b3f3b183c3d562e7ee9bcca7` (`main`, 2026-09-15; eggfetch 0.1.4)
Parent program: `plans/post-audit-maintenance-security-and-proxy-modernization-program.md`
Reference compatibility contracts: `httpx==0.28.1`, `httpx2==2.12.0`, Python 3.10+

## Objective

Perform the single authoritative post-program freeze and qualification after the adapter/dependency, Python dispatch, security/release, and proxy-pooling plans have completed.

This plan does not add features. Its job is to prove that the completed line of work is coherent on one exact executable SHA, correct any integration defects found during qualification, renew compatibility evidence only on the corrected candidate, and close documentation/plan state truthfully.

## Prerequisites

Do not begin final freeze until each sibling plan has either completed its implementation acceptance criteria or has a documented no-go outcome explicitly permitted by that plan:

1. `adapter-feature-and-dependency-boundary-correction.md`;
2. `python-request-dispatch-consolidation.md`;
3. `security-policy-release-and-supply-chain-hardening.md`;
4. `proxy-hyper-pooling-and-upstream-reuse.md`.

A child plan with unresolved executable work is not “close enough” for this qualification. Finish or deliberately defer it before freezing.

## Freeze model

The final qualification must distinguish:

- **executable candidate SHA** — last commit that changes Rust/Python source, manifests, lockfiles, tests/fixtures, scripts that determine validation semantics, package configuration/content, or workflows that determine release/package behavior;
- **documentation descendant SHA** — optional later commit(s) limited to plans, compatibility ledgers/profiles, and prose documentation that cannot alter produced artifacts or test outcomes.

Freeze the executable candidate only after the working tree is clean and all child-plan focused checks are green.

If any qualification failure requires executable/test/build/script/workflow correction, land the fix and create a new executable freeze. Do not bind compatibility evidence to the failed candidate.

## 1. Audit child-plan integration before running expensive gates

Perform a source-level integration review on the final candidate.

### Adapter/feature boundary

Confirm:

- FFI core dependency defaults are disabled explicitly;
- FFI forwards every intended capability deliberately;
- FFI default TLS/native-root behavior is unchanged from effective pre-program behavior unless the child plan documented a deliberate change;
- Node explicitly enables the minimal transport/TLS profile it requires;
- Python direct TLS dependencies have owners or are gone;
- no stale `config.rs` placeholder remains;
- feature documentation matches manifests.

### Python dispatch

Confirm:

- common `PreparedRequest` -> core builder state is applied in one runtime-neutral location;
- sync/async/top-level runtime mechanics remain distinct;
- auth/proxy inherit/disable/override states are preserved;
- trace callback error handling remains outside core and has the same precedence;
- streaming bodies remain one-shot/backpressured where appropriate.

### Security/release

Confirm:

- stale advisory ignores are removed;
- any remaining ignore has a current rationale;
- the canonical security preflight exists and fails closed;
- `SECURITY.md`, dependency policy, verification policy, and release docs agree;
- PyPI publication requires an exact matching version tag in repository-controlled workflow logic;
- build-only dispatch remains available where documented;
- release-critical dynamically installed Python tools are pinned/reviewable;
- no second automatic push/PR workflow was introduced.

### Proxy modernization

Confirm:

- forward/CONNECT successful HTTP framing is under Hyper wherever the child plan qualified that path;
- no second generic origin HTTP parser remains without a documented necessity;
- pooling keys isolate connection-affecting proxy/auth/TLS/pinning/origin policy;
- caches are bounded;
- SOCKS behavior was not regressed by unrelated refactoring;
- proxy TLS and origin TLS remain distinct;
- request-scoped pinned routes remain fail closed;
- timeout/lifecycle policy remains coherent with reused connections.

Record any integration discrepancy before freeze. Do not rely on tests alone to discover ownership mistakes.

## 2. Focused feature/dependency proof

Run the supported core feature matrix already defined by repository policy, plus the new adapter checks.

At minimum:

```sh
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls,tls-native-roots
cargo check -p eggfetch-core --all-features

cargo check -p eggfetch-ffi --no-default-features
cargo check -p eggfetch-ffi --all-features
cargo test -p eggfetch-ffi --all-features
cargo test -p eggfetch-node --all-features
cargo check -p eggfetch-python
```

Capture/review:

```sh
cargo tree -p eggfetch-ffi --no-default-features -e features
cargo tree -p eggfetch-node -e features
cargo tree -p eggfetch-python -e features
cargo tree -d
```

The purpose is not to archive enormous tree output. Verify the child-plan invariants and document only material findings/exceptions.

If the program changed `hyper-util` features/version, specifically confirm only intended client/proxy/pool features entered the graph and no unexpected MSRV or TLS backend appeared.

## 3. Routine validation

On the clean executable candidate:

```sh
./scripts/check.sh
```

This must pass without undocumented skips that affect a supported surface.

The Node JS artifact remains an explicitly optional prototype check under current policy; absence of a manually built Node artifact may remain a documented skip. Do not reinterpret that historical prototype policy during final qualification.

## 4. Extended validation

Run:

```sh
./scripts/check.sh extended
```

Satisfy all required prerequisites first, including exact Rust 1.89.0 and pinned HTTPX/HTTPX2 compatibility dependencies.

Review every optional skip printed by the command. A skip is acceptable only if it matches current normative policy and does not cover a surface modified by this program.

Because this program changes proxy transport behavior, downstream/artifact fixtures relevant to proxy/TLS use should be present for final qualification where the repository's current process marks them required. If a nominally optional artifact would be the only coverage of a changed required downstream, generate it rather than accepting a misleading skip.

## 5. Package validation

Run from the same clean candidate:

```sh
./scripts/check.sh package
```

Verify especially:

- FFI/Node manifest topology after feature correction;
- Python wheel content after direct dependency cleanup;
- native extension import;
- PEP 561 stubs/marker;
- runtime version coherence;
- sdist/wheel package content;
- no orphaned source/config files accidentally included.

Do not publish during qualification.

## 6. Live dependency-security preflight

Run the canonical command created by the security plan, for example:

```sh
./scripts/check_security.sh
```

Use current advisory data. Record in this plan's closure section:

- UTC/local date of scan;
- `cargo-deny --version`;
- `cargo-audit --version`;
- advisory database freshness/commit if the tool exposes it conveniently;
- any accepted warnings/exceptions with links to the current documented rationale.

The security scan binds only what was known to the selected advisory data at that time. Do not phrase it as proof of vulnerability absence.

A new high/critical applicable advisory discovered before release reopens the executable candidate if remediation changes dependencies/code.

## 7. Focused proxy reuse and isolation qualification

The proxy child plan changes connection lifecycle and therefore requires evidence beyond generic HTTPX parity.

Use deterministic local proxy fixtures that count accepted physical sockets/tunnels.

Required evidence includes:

### Reuse

- sequential forward-proxy requests to a compatible route reuse a physical proxy connection;
- sequential HTTPS requests to the same compatible CONNECT origin reuse a tunnel/connection where protocol semantics permit;
- H2-over-CONNECT multiplex/reuse remains correct if H2 is enabled for that path;
- idle-close/stale pooled connection recovery opens a new connection and completes/fails according to existing retry policy.

### Non-reuse/isolation

Prove a new connection/client route is selected when changing relevant:

- proxy endpoint;
- proxy credential/config identity where reuse would be unsafe;
- proxy TLS policy;
- CONNECT origin;
- pinned proxy address set;
- pinned proxied target;
- origin TLS/SNI/ALPN policy;
- direct vs proxy route.

### Lifecycle/security

Prove:

- incomplete/cancelled response bodies do not return corrupted connections to the pool;
- `Connection: close` is honored;
- proxy auth and proxy-only headers never leak into the CONNECT origin request or a later direct request;
- pinning never falls back to DNS;
- timeout phases remain correctly classified on newly established routes;
- reuse does not fabricate connect/proxy-TLS timeout events for phases that did not occur;
- body-size/decompression limits still apply above the modernized route.

If any test demonstrates an ambiguous ownership condition, fix it before exact-SHA compatibility binding.

## 8. Python sync/async dispatch qualification

Run the focused tests owned by the dispatch plan plus the API/type oracles.

Required categories:

- sync/async argument parity;
- auth inherit/disable/override;
- proxy inherit/disable/override;
- redirect partial overrides;
- retry and timeout overrides;
- sync/async streamed uploads;
- streaming responses and close/cancellation;
- extension target/SNI/static route handling;
- trace callback failures;
- top-level request helper behavior;
- installed-wheel typing/member contract.

Do not accept “full suite passes” as the only evidence if a focused regression can pinpoint a drift-prone branch more directly.

## 9. HTTPX and HTTPX2 exact-SHA requalification

Renew the repository's pinned compatibility profiles only after all prior gates are green.

Run the full compatibility process against the exact versions declared by the live repository:

- `httpx==0.28.1`;
- `httpx2==2.12.0`.

Run both API-manifest/oracle comparisons and the full behavioral suite. Preserve the repository's current convention of repeated successful runs where the live compatibility process requires three consecutive passes.

Pay special attention to:

- HTTP/HTTPS proxies;
- proxy auth and NO_PROXY;
- proxy TLS/custom CA;
- streaming;
- timeout classification;
- redirects with proxy route changes;
- sync/async request construction;
- network-stream/upgrade behavior;
- exception hierarchy and messages where contractually checked.

Update compatibility profile/ledger binding only to the exact final executable SHA.

Any executable/test/build/dependency change after the bound SHA invalidates the qualification and requires rerunning this section.

## 10. MSRV and platform implications

Run the existing exact Rust 1.89.0 gate through extended validation.

If proxy modernization enables newer `hyper-util` features/version, prove the resolved graph remains compatible with Rust 1.89. Do not rely only on the dependency's advertised lower MSRV; compile the actual workspace profiles.

Python cross-platform wheel builds are release-workflow concerns, but any changed Cargo cfg/feature behavior affecting Windows/macOS must have source/build coverage before final closure. The adapter feature plan in particular must not create Unix-only assumptions in FFI/Node/Python manifests.

## 11. Release-workflow dry logic checks

Do not publish packages merely to test the workflow.

Test the version/ref validation script directly for:

- ordinary no-tag validation;
- correct matching tag + `--publish`;
- mismatched tag;
- invalid tag;
- publish mode without tag.

Review the YAML guard to prove:

- `publish=false` permits a branch/ref build;
- `publish=true` requires a version tag;
- only the publish job has `id-token: write`;
- protected `pypi` environment remains;
- Actions remain SHA pinned.

If a safe GitHub manual build-only dispatch is performed during closure, record it as supplemental evidence, not as a substitute for source review.

## 12. Freeze and closure record

Once all checks pass, add a closure section to this plan containing:

```text
Executable freeze SHA:
Candidate date:
Tier 1:
Extended:
Package:
Security preflight:
Feature/dependency proof:
Proxy reuse/isolation proof:
Python dispatch proof:
HTTPX 0.28.1:
HTTPX2 2.12.0:
API oracles:
MSRV:
Known optional skips:
Residual limitations:
```

Do not mark a field passed without having run it on the bound candidate or an explicitly equivalent immutable artifact from that candidate.

## 13. Documentation-only descendant

After executable freeze and compatibility binding, update documentation/index state without changing executable semantics.

At minimum reconcile:

- `plans/README.md`;
- parent program status;
- compatibility ledger/profile references;
- `docs/architecture/dependency-policy.md`;
- `docs/architecture/feature-flags.md`;
- `docs/architecture/ffi-and-node.md` as needed;
- proxy architecture docs;
- `SECURITY.md`;
- `docs/verification-policy.md` and release docs where the security preflight/tag policy is described;
- roadmap/current-product-position text if it still claims HTTP proxy sockets are always one-shot.

The descendant must not edit manifests, lockfiles, source, tests, scripts, workflows, package metadata, or fixtures. If documentation review reveals an executable discrepancy, reopen the candidate instead.

## Final program acceptance criteria

- [x] All four child plans have completed or documented an explicitly permitted bounded no-go outcome.
- [x] One exact executable SHA is identified.
- [x] Tier 1 passes on that SHA.
- [x] Extended validation passes on that SHA with only truthful permitted skips.
- [x] Package validation passes on that SHA.
- [x] Security preflight passes against current advisory data on that SHA.
- [x] FFI/Node/Python feature graphs satisfy the new adapter ownership invariants.
- [x] Proxy connection reuse and isolation are demonstrated by physical-socket/tunnel tests.
- [x] Python sync/async/top-level dispatch semantics remain qualified.
- [x] HTTPX 0.28.1 and HTTPX2 2.12.0 full exact-SHA qualification passes according to the live repository process.
- [x] API/type oracles pass.
- [x] Rust 1.89 MSRV passes.
- [x] PyPI publication guards are fail closed and build-only behavior remains available.
- [x] Documentation-only closure contains no executable changes.
- [x] Current `main` CI is green on the final descendant.

## Non-goals

- no additional feature development during qualification;
- no HTTP/3 graduation;
- no Node maturation;
- no new compatibility target;
- no publication as part of qualification;
- no performance claim without measurement;
- no transformation of historical plan records into normative requirements.

## Exit criterion

The parent program is closed only when the final repository state has one auditable executable freeze whose adapter feature graph, Python dispatch, dependency security, release guardrails, proxy lifecycle, package artifacts, MSRV, and HTTPX/HTTPX2 behavior have all been proven together. Historical evidence from earlier SHAs does not satisfy this criterion.

## Final closure record — complete (2026-09-16)

Executable freeze: `d87be1b780a41dc8ff5f3ba8a14f8d74de5814d0`.

- Tier 1, extended, and package validation passed. Extended/package reported
  only the existing optional Node native-artifact and downstream artifact
  manifest skips; Rust 1.89.0 MSRV checks passed.
- Adapter feature/dependency proof passed; native Python API/typing checks
  passed (66 exports, 32 exception bases, 24 reviewed member contracts).
- The explicit security preflight passed at 2026-09-16T03:59:54Z with
  cargo-deny 0.19.0 and cargo-audit 0.22.2.
- Core proxy tests passed 46/46, including Hyper forward reuse and HTTPS
  CONNECT tunnel reuse, with route isolation and fallback contracts intact.
- Three consecutive full pinned compatibility runs passed 1,870 tests each,
  with 26 existing non-failing warnings, in 242.98s, 241.34s, and 243.98s.
  HTTPX 0.28.1 and HTTPX2 2.12.0 API oracles remained at 71 and 79 allowed
  matches, with no unexplained, stale, or resolved-active differences.
- `compat/httpx/0.28.1/profile.toml` and `compat/httpx2/2.12.0/profile.toml`
  now bind Stage C to the exact executable SHA. Everything after that freeze
  in this closure is documentation/profile/index prose only.
