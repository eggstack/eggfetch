# Broad Documentation Truth Refresh — Post-Requalification

Planning baseline: `bd78c9a1d2f9aecfc7ee8f2c56bad2b74ec1c3f9` (`main`, 2026-09-03)
Depends on: `plans/httpx-parity-corrective-08-post-hardening-requalification-and-closure.md`
Qualification source of truth: `compat/httpx/0.28.1/profile.toml` + `plans/httpx-parity-correction-status.md`
Normative verification source: `docs/verification-policy.md`
Normative release source: `docs/releases/process.md`

## Objective

Perform a broad repository-wide documentation truth pass after Corrective 08 has renewed the exact-SHA HTTPX qualification. Reconcile public, contributor, architecture, compatibility, migration, release, and internal-agent documentation with the actual current implementation and normative policy, while keeping this phase documentation-only so it remains a valid descendant of the newly qualified executable SHA.

This pass is deliberately broader than fixing the already identified HTTPX migration-guide errors. The repository has accumulated substantial implementation changes across transport behavior, compression, pooling, redirects, retries, TLS, Python bindings, FFI/Node adapters, CLI behavior, and compatibility semantics. Documentation must be audited systematically against code/tests and the live compatibility/verification records rather than patched opportunistically.

The governing rule is:

> Source code and executable tests define behavior; the current exact-SHA compatibility profile/status defines qualified HTTPX claims; normative verification/release policy defines process; documentation must describe those truths without inventing broader guarantees.

This plan must land as a **documentation/ledger-only descendant** of the Corrective 08 frozen executable SHA. If the audit discovers a source-code or source-doc-comment defect that requires editing an executable file, stop this pass, make that correction before qualification or reopen Corrective 08 and requalify a new SHA.

---

# 1. Preconditions and documentation-only boundary

Do not begin this pass until Corrective 08 has completed and the repository has:

- one newly qualified `FROZEN_EXECUTABLE_SHA` in `compat/httpx/0.28.1/profile.toml`;
- a matching current section in `plans/httpx-parity-correction-status.md`;
- clean API-oracle/compatibility/downstream evidence for that exact SHA;
- a documented post-freeze descendant audit showing qualification-record changes are non-executable.

Files permitted in this phase are documentation and non-executable compatibility/status prose/ledgers only. Typical allowed paths:

- `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md` where a documentation correction is appropriate;
- `AGENTS.md`;
- `.skills/*.md`;
- `docs/**/*.md`;
- `compat/httpx/0.28.1/*.md` and TOML ledgers/profile only when reconciling already-qualified facts without changing the executable contract;
- `plans/*.md` status/roadmap/history records.

Do **not** edit in this pass:

- `crates/**` Rust/Python/Node/FFI source or tests;
- Rust doc comments if they live in `.rs` files;
- Python source/docstrings in package files;
- manifests, lockfiles, build scripts, package metadata that affect artifacts;
- `scripts/**` validation or generation logic;
- workflows;
- compatibility test fixtures or oracle generators.

Acceptance:

- [ ] Corrective 08 is complete before documentation changes begin.
- [ ] Final docs commits remain non-executable descendants of the new qualified SHA.
- [ ] Any discovered executable correction is explicitly routed back through requalification instead of being smuggled into the docs pass.

---

# 2. Define source-of-truth precedence

For every disputed claim, resolve truth in this order:

1. **Executable implementation and direct tests** for runtime/API behavior.
2. **Pinned HTTPX reference tests and differential cases** for compatibility behavior.
3. **`compat/httpx/0.28.1/profile.toml`, active/resolved difference ledgers, parity cases, and `plans/httpx-parity-correction-status.md`** for qualified compatibility scope/evidence.
4. **`docs/verification-policy.md`** for CI/local verification policy.
5. **`docs/releases/process.md`** for release/publication procedure.
6. Current package manifests/workflows only to verify descriptive facts such as versions, feature flags, wheel matrix, and dependencies.
7. Architecture/reference docs.
8. Historical completed plans last; they are evidence/history, not normative requirements.

Never resolve a contradiction by copying language from an older completed plan when current code or normative policy disagrees.

Acceptance:

- [ ] Every corrected compatibility/process claim can be traced to a current authoritative source.
- [ ] Historical plan wording is not treated as current product truth.

---

# 3. Build a documentation inventory and contradiction map

At implementation time, enumerate all Markdown documentation and group it by audience/scope. Use repository-native tools such as `find`/`rg`; do not rely on memory of filenames.

Minimum inventory groups:

## 3.1 Top-level and contributor guidance

- `README.md`
- `CONTRIBUTING.md`
- `SECURITY.md`
- `CHANGELOG.md`
- `AGENTS.md`
- `.skills/*.md`

## 3.2 User documentation

- `docs/README.md`
- `docs/getting-started/**`
- `docs/concepts/**`
- `docs/rust/**`
- `docs/python/**`
- `docs/cli/**`
- `docs/cookbook/**`
- `docs/migration/**`
- `docs/reference/**`
- `docs/security/**`

## 3.3 Maintainer/internal documentation

- `docs/architecture/**`
- `docs/ffi/**`
- `docs/releases/**`
- `docs/verification-policy.md`

## 3.4 Compatibility-specific documentation

- `compat/httpx/0.28.1/README.md`
- `docs/reference/compatibility.md`
- `docs/residual-differences.md`
- current compatibility/profile/ledger comments that make user-facing claims
- `plans/httpx-parity-correction-status.md`

For each file, classify it as:

- current and verified;
- stale wording only;
- behaviorally incorrect;
- internally contradictory;
- duplicates another canonical document;
- historical and should be clearly labeled as such;
- missing a newly important behavior/limitation.

Maintain a temporary audit table during implementation with columns similar to:

`file | claim/topic | authoritative source | disposition | changed?`

The audit table need not become a permanent new evidence format unless it materially improves maintainability. Prefer updating existing docs over inventing another permanent truth ledger.

Acceptance:

- [ ] Every current Markdown documentation file is considered, not only migration docs.
- [ ] Known contradictions are recorded before editing so they are not fixed piecemeal and forgotten elsewhere.

---

# 4. Correct the known HTTPX migration and compatibility drift

The following known issues must be explicitly checked and corrected against the renewed qualification.

## 4.1 `docs/migration/from-httpx.md`

Audit and fix at minimum:

- **Raw iteration:** current text says HTTPX `iter_raw()` is unavailable and recommends `iter_bytes()`, while the compatibility facade now implements raw streaming semantics. Describe native eggfetch and `eggfetch.compat.httpx` separately where they differ.
- **WSGI/ASGI transports:** current summary says they are unavailable even though the qualified HTTPX compatibility facade implements `WSGITransport` and `ASGITransport`.
- **Mock/custom transports and mounts:** ensure implemented HTTPX facade support is represented accurately.
- **Proxy environment variables:** current text says eggfetch does not read `HTTP_PROXY`/`HTTPS_PROXY`; this must distinguish native explicit proxy behavior from the HTTPX compatibility facade, which honors HTTPX-compatible environment discovery when `trust_env=True`.
- **Redirect summary:** remove the contradictory row that says HTTPX follows redirects by default. HTTPX 0.28.1 and eggfetch's HTTPX facade default to `follow_redirects=False`.
- **Timeouts:** distinguish HTTPX's four operational phases from eggfetch-native `total`; do not claim HTTPX has a request-wide total timeout.
- **Compression:** distinguish decoded iteration/content from raw encoded iteration rather than calling one library simply “manual” and the other “automatic.”
- **Backend/concurrency:** retain the important asyncio-only vs AnyIO/Trio boundary.
- **SSLContext:** describe supported translation and fail-closed rustls-unrepresentable state without implying arbitrary OpenSSL context passthrough.
- **Network stream/upgrades:** document only the qualified 101 ownership behavior and retained ordinary pooled-stream difference if relevant to migration users.

## 4.2 `docs/reference/compatibility.md`

Ensure the feature table and narrative agree with the newly qualified profile. Specifically distinguish:

- native Python API;
- HTTPX compatibility facade;
- Rust API;
- CLI;
- optional/experimental support.

Do not let a feature implemented only in the HTTPX facade appear as an unconditional native Python capability.

Review every “Yes” involving:

- proxy env vars;
- custom transports/mounts;
- Mock/WSGI/ASGI transports;
- Digest/NetRC/custom auth flows;
- raw iteration;
- UDS/local address/socket options;
- SOCKS;
- timeout phases;
- HTTP/2 and HTTP/3;
- retries.

## 4.3 `compat/httpx/0.28.1/README.md` and residual differences

Synchronize:

- current qualification SHA/date;
- supported Python/asyncio scope;
- current bounded differences;
- qualification invalidation rule;
- relationship between native API and HTTPX facade.

Do not duplicate long stale historical evidence if the live status ledger already owns it; link instead.

Acceptance:

- [ ] No migration or compatibility doc contains a known contradiction with the renewed profile.
- [ ] Native eggfetch behavior and HTTPX-facade behavior are clearly separated where they differ.
- [ ] No HTTPX default is described incorrectly.

---

# 5. Re-audit Requests migration documentation

`docs/migration/from-requests.md` must be treated as a migration guide, not a loose feature comparison.

Verify at minimum:

- `Session` vs `Client` lifecycle and pooling behavior;
- module-level helper behavior and cost/connection reuse implications;
- request method and body-kwarg compatibility;
- auth tuple shorthand versus typed auth support in the current native API;
- cookie jar operations and request-local cookie precedence;
- redirect defaults and method/body rewrite behavior;
- cross-origin credential stripping, described accurately for both Requests and eggfetch without unsupported claims about Requests hooks/default forwarding;
- timeout float/tuple/object semantics, including whether connect/read/write/pool/total are independently enforced in current code;
- proxy dict/single-proxy/native env behavior and `NO_PROXY` differences;
- TLS custom CA semantics and mTLS forms;
- streaming API differences, including raw/decoded behavior;
- multipart tuple/file/path forms;
- exception mapping;
- retry behavior: distinguish built-in eggfetch retry policy from Requests' urllib3 adapter configuration rather than simply saying “Requests has no retry.”

Known stale statements to verify closely include the old claim that eggfetch's `connect` phase is accepted but not independently enforced. Current implementation/architecture documentation indicates connect setup is actively bounded on direct and proxy routes; the migration guide must reflect the current tested behavior.

Acceptance:

- [ ] Requests migration examples run or syntax-check under the current package API.
- [ ] Differences are migration-relevant rather than marketing-oriented.
- [ ] Requests behavior is not misstated merely to make eggfetch look stronger.

---

# 6. Refresh README and product-position claims

Audit the top-level `README.md` as the highest-visibility source.

Required checks:

- feature list matches current native/core behavior;
- HTTP/1.1, HTTP/2, HTTP/3 wording preserves HTTP/3 experimental status;
- retry claims match actual default/opt-in behavior;
- proxy wording distinguishes native environment behavior from the HTTPX facade where necessary;
- compatibility claim uses the newly qualified exact SHA/date or points to the live profile/status rather than embedding stale closure language;
- retained HTTPX differences are concise but not omitted where they materially affect migration;
- Python support versions match manifests/release policy;
- Rust MSRV matches workspace metadata;
- Node support remains explicitly experimental;
- C FFI maturity is described accurately;
- package/release badges and installation commands correspond to actual published package/crate names and current versioning;
- no comparative performance claim is made without a current apples-to-apples benchmark.

Prefer concise top-level claims with links to canonical detailed docs rather than duplicating multi-paragraph compatibility internals that will drift again.

Acceptance:

- [ ] README contains no stale qualification SHA/status.
- [ ] README does not overclaim drop-in compatibility, performance, protocol support, or platform support.
- [ ] Detailed compatibility caveats are linked to one canonical reference.

---

# 7. Refresh contributor and agent guidance

Audit `AGENTS.md` and `.skills/*.md` for instructions invalidated by the new closure pass or by current verification/release policy.

Known items:

- `AGENTS.md` currently tells agents that the package is Stage C qualified and that any executable change requires “restarting Corrective 07.” After Corrective 08, replace pass-number-specific operational guidance with the current live rule: executable changes invalidate the qualification and require a fresh exact-SHA requalification using the current closure plan/status procedure.
- `.skills/documentation.md` contains the same Corrective 07-specific wording and must be generalized or updated to Corrective 08/current procedure.
- validation commands must match `scripts/check.sh` and `docs/verification-policy.md`.
- release guidance must match the current single automatic CI workflow and manually dispatched PyPI process.
- crate-boundary and “one networking implementation” rules must match current FFI/Node architecture.
- unsafe-code wording must reflect the actual workspace/crate lint configuration; do not claim workspace-wide `forbid` if FFI/Node explicitly override it.

The goal is to make future automated implementation agents less likely to recreate documentation drift.

Acceptance:

- [ ] No active agent instruction points to a superseded corrective pass as the permanent procedure.
- [ ] Contributor commands match actual checked-in scripts/policy.
- [ ] Architecture invariants are stated consistently across AGENTS and architecture docs.

---

# 8. Reconcile verification, release, version, and platform documentation

Cross-check these documents as one system:

- `docs/verification-policy.md`
- `docs/releases/process.md`
- `docs/releases/compatibility-policy.md`
- `docs/reference/versioning.md`
- `docs/getting-started/installation.md`
- `CONTRIBUTING.md`
- relevant `.skills/release-process.md`

Known contradiction to fix:

- `docs/releases/compatibility-policy.md` currently says “CI runs the full test suite against the declared MSRV,” while the normative verification policy says MSRV is an extended local check and may be skipped when the 1.80 toolchain is unavailable. Make the compatibility policy describe the actual extended-check model rather than a nonexistent routine CI matrix.

Verify additionally:

- current MSRV from workspace metadata;
- Python 3.10–3.13 support;
- ABI/wheel model from current PyO3/maturin configuration;
- supported platform tiers and actual release wheel matrix;
- whether source distributions are produced;
- crates.io publication remains manual;
- PyPI workflow remains manually dispatched and does not run on push/PR;
- package-validation behavior matches `./scripts/check.sh package`;
- no doc implies routine CI is a release-authority or full cross-platform qualification system.

Acceptance:

- [ ] One consistent verification/release story exists across every maintainer/user doc.
- [ ] Normative policy is referenced instead of duplicated where possible.
- [ ] No nonexistent CI matrix/check is claimed.

---

# 9. Audit concepts and architecture docs against post-hardening behavior

Review every current concept/architecture deep dive against implementation changes since the prior documentation pass.

Priority topics:

## 9.1 Pooling and lifecycle

Verify:

- per-origin concurrency/permit model;
- waiter cancellation and retention/eviction behavior;
- H2 multiplexing semantics;
- response/stream permit ownership;
- client close/shutdown behavior;
- bounded caches and their eviction semantics where documented.

## 9.2 Streaming and body model

Verify:

- known vs unknown stream length semantics;
- `RequestBody::is_empty`/replayability concepts exposed in docs;
- buffered vs streaming response paths;
- raw vs decoded body distinction;
- line/text iteration bounds;
- 101 upgraded stream lifecycle.

## 9.3 Compression

Verify:

- supported codecs and feature gates;
- decompression-limit validation;
- size/ratio policy;
- stacked encodings;
- streaming/buffered enforcement boundaries;
- header restoration/removal behavior in the HTTPX facade versus native core;
- no implication that decompression is unbounded.

## 9.4 Redirect/auth/cookies/retries

Verify:

- credential stripping/reapplication;
- cookie propagation;
- replayability restrictions;
- retry cause/status behavior;
- retry budget/elapsed accounting;
- `Retry-After` handling;
- default retry opt-in state.

## 9.5 TLS/proxy/protocols

Verify:

- rustls trust model and custom roots;
- mTLS configuration;
- TLS version policy;
- SNI overrides;
- proxy endpoint TLS isolation;
- HTTP forward vs CONNECT behavior;
- SOCKS route pooling/address behavior;
- H1/H2 policy and H2 forbidden headers;
- HTTP/3 remains experimental and separately routed.

## 9.6 Python bindings

Verify:

- sync facade runtime/GIL behavior;
- async client lifecycle and locking;
- response/stream wrappers;
- compatibility facade architecture;
- error hierarchy and conversion;
- network-stream sync/async wrapper selection.

## 9.7 FFI and Node

Verify:

- handle ownership and panic/error boundaries;
- runtime bridge;
- streaming support;
- Node maturity remains experimental;
- adapters do not contain independent HTTP engines.

Acceptance:

- [ ] Architecture docs describe current invariants and ownership/lifecycle semantics.
- [ ] Historical bug-fix implementation details are included only when they teach a stable invariant.
- [ ] Architecture docs do not become a second incompatible API reference.

---

# 10. Audit Python, Rust, CLI, FFI guides and cookbook examples

For each guide/example, verify both signatures and semantics against current public APIs.

## Python guide

Check:

- constructor kwargs;
- sync/async parity;
- streaming methods and context-manager lifecycle;
- auth/cookies/proxy/TLS/timeouts/retry parameters;
- HTTP2/HTTP3 switches;
- exception names;
- native-vs-compat facade examples.

## Rust guide

Check:

- current builder methods;
- feature-gate names/defaults;
- timeout/retry/config types;
- streaming examples;
- proxy/TLS/H2/H3 examples;
- no example relies on private APIs.

Rust source doc comments are out of scope for this docs-only pass; if Markdown reveals contradictory rustdoc that must be corrected in source, route it back through requalification.

## CLI guide

The CLI changed materially after the prior qualification. Verify:

- current flags and aliases;
- request-body modes;
- output schemas/machine-readable modes;
- HTTP version options;
- proxy/TLS/auth/cookie/retry options;
- exit codes;
- streamed upload/download behavior;
- shell completion/install examples.

## FFI guide

Verify:

- exported ownership model;
- null/error handling contract;
- request/response lifetime;
- streaming functions;
- runtime behavior.

## Cookbook

Run/syntax-check examples where practical and ensure examples prefer current recommended APIs.

Acceptance:

- [ ] Public guide examples match current signatures.
- [ ] No guide recommends a superseded or compatibility-only API as native behavior.
- [ ] CLI examples match actual current argument parsing.

---

# 11. Normalize terminology and claim strength

Apply consistent terminology across all docs:

Use:

- **native Python API** for `eggfetch.Client` / `eggfetch.AsyncClient`;
- **HTTPX compatibility facade** for `eggfetch.compat.httpx`;
- **Stage C qualified** only when tied to the live exact-SHA profile and scoped to the documented HTTPX 0.28.1 asyncio surface;
- **experimental** for HTTP/3 and Node where current policy says so;
- **supported and tested**, **partially supported**, **intentional difference**, or **out of scope** for compatibility claims.

Avoid:

- unrestricted “drop-in replacement for HTTPX” language;
- “drop-in replacement for Requests” language;
- “faster than HTTPX/Requests” without current comparative benchmark data;
- “full parity” without scope qualifiers;
- ambiguous “eggfetch supports X” where X is facade-only;
- calling routine CI “release qualification.”

Acceptance:

- [ ] Same feature is not described with materially different maturity/support labels in different docs.
- [ ] Marketing-strength wording never exceeds tested evidence.

---

# 12. Reduce future drift through canonical references, not new machinery

Where the same volatile facts are copied into many documents, reduce duplication.

Preferred canonical homes:

- exact HTTPX qualification SHA/scope: `compat/httpx/0.28.1/profile.toml` + `plans/httpx-parity-correction-status.md`;
- user-visible feature compatibility: `docs/reference/compatibility.md`;
- residual differences: `docs/residual-differences.md`;
- verification tiers: `docs/verification-policy.md`;
- release workflow: `docs/releases/process.md`;
- platform/version support: `docs/releases/compatibility-policy.md` / `docs/reference/versioning.md` with one clearly designated source if overlap is unnecessary;
- architecture invariants: `docs/architecture/overview.md` + focused deep dives.

Other docs should link to these canonical sources rather than duplicating test counts, workflow internals, or long lists of bounded differences that will drift.

Do not add a new docs CI workflow, evidence schema, generated-doc system, or doc metadata database for this pass.

Acceptance:

- [ ] Volatile exact-SHA/test-count data has one primary home.
- [ ] Duplicate process descriptions are shortened to references where appropriate.
- [ ] Documentation maintenance remains simpler than the behavior being documented.

---

# 13. Documentation validation

Run the documentation-specific checks from `.skills/documentation.md`:

```sh
python scripts/check_doc_examples.py
python scripts/check_doc_links.py
cargo doc --workspace --all-features --no-deps
cargo test --doc -p eggfetch-core --all-features
```

Also run routine validation after the docs edits:

```sh
./scripts/check.sh
```

The routine command is expected to pass without rebuilding qualification evidence because this phase is docs-only. If a documentation edit unexpectedly changes an executable path, stop and enforce the requalification boundary.

Perform targeted textual audits for stale high-risk phrases, for example:

```sh
rg -n "Corrective 07|5c7899f|follow(s|ed)? redirects by default|does not read.*HTTP_PROXY|iter_raw.*not available|WSGI.*not available|ASGI.*not available|full test suite.*MSRV|drop-in|full parity|faster than" README.md AGENTS.md .skills docs compat plans
```

Treat search hits as review prompts, not automatic errors; historical plans may legitimately contain historical SHAs/wording if clearly labeled.

Acceptance:

- [ ] Python documentation example checker passes.
- [ ] Internal link checker passes.
- [ ] Rustdoc builds and doctests pass.
- [ ] Routine repository validation passes.
- [ ] Remaining stale-phrase search hits are either corrected or intentionally historical/scoped.

---

# 14. Final descendant audit

After all documentation commits:

1. compare the Corrective 08 `FROZEN_EXECUTABLE_SHA` to final `main`;
2. list all changed files;
3. prove every post-freeze change is documentation/ledger-only;
4. confirm no Rust/Python source, tests, manifests, scripts, workflows, dependency lockfiles, or package configuration changed;
5. add a concise post-documentation descendant note to `plans/httpx-parity-correction-status.md` if needed so future maintainers can distinguish the executable freeze from docs-only descendants.

If any executable file appears, the exact-SHA qualification is invalid until Corrective 08 is rerun for the newer tree.

Acceptance:

- [ ] Final `main` is an explicitly audited documentation/ledger-only descendant of the qualified SHA.
- [ ] The profile still points to the correct executable freeze.

---

# Final acceptance criteria

The broad documentation truth refresh is complete only when:

- [ ] Corrective 08 has renewed the exact-SHA HTTPX qualification first.
- [ ] Every current Markdown documentation file has been inventoried and reviewed for relevance/accuracy.
- [ ] `docs/migration/from-httpx.md` no longer contains the known raw-stream, WSGI/ASGI, proxy-env, redirect-default, timeout, or compatibility contradictions.
- [ ] `docs/migration/from-requests.md` reflects current auth, timeout, retry, proxy, streaming, redirect, and exception behavior accurately.
- [ ] `README.md` describes current capabilities and compatibility scope without stale SHA/status or unsupported performance/drop-in claims.
- [ ] `docs/reference/compatibility.md`, `docs/residual-differences.md`, and the compatibility README match the newly qualified profile/ledgers.
- [ ] `AGENTS.md` and `.skills/*.md` no longer hardcode superseded Corrective 07 as the permanent requalification procedure.
- [ ] Verification/release/version/platform docs agree with normative policy and actual workflows/manifests.
- [ ] Concepts/architecture docs reflect current pool, streaming, compression, retry, redirect/auth, TLS/proxy, H2/H3, Python, FFI, and Node behavior.
- [ ] Python/Rust/CLI/FFI guides and cookbook examples match current public signatures and semantics.
- [ ] Terminology consistently distinguishes native Python from the HTTPX facade and preserves experimental/bounded-difference labels.
- [ ] Volatile facts have canonical homes and unnecessary duplication is reduced.
- [ ] Documentation example/link/rustdoc/doctest checks pass.
- [ ] `./scripts/check.sh` passes after the docs-only changes.
- [ ] Final descendant audit proves no executable/test/build/validation/packaging file changed after the qualified SHA.

## Closure statement

After this plan closes, the repository should have one coherent documentation story: eggfetch is a Rust-native HTTP client platform with a native Python sync/async API and an exact-SHA-qualified, explicitly bounded HTTPX 0.28.1 asyncio compatibility facade. Requests migration should be documented as a practical migration path rather than a drop-in guarantee, and process/platform/release claims should match the checked-in normative policies and tooling.