# HTTPX Parity Corrective 08 — Post-Hardening Requalification and Closure

Planning baseline: `bd78c9a1d2f9aecfc7ee8f2c56bad2b74ec1c3f9` (`main`, 2026-09-03)
Prior qualified executable SHA: `5c7899fefb6df087dfa1b3578fbef9ba64f87742` (Corrective 07, 2026-08-24)
Reference contract: `httpx==0.28.1` / `httpcore==1.0.9`, Python 3.10+, asyncio-supported public surface
Depends on: `plans/httpx-parity-corrective-07-final-exact-sha-requalification.md`
Live ledger: `plans/httpx-parity-correction-status.md`
Followed by: `plans/documentation-broad-truth-refresh-after-requalification.md`

## Objective

Re-qualify the current post-hardening eggfetch executable tree against the pinned HTTPX 0.28.1 compatibility contract and renew the Stage C exact-SHA claim only after all executable changes since Corrective 07 have been exercised by the full qualification procedure.

This is a **requalification and closure pass**, not a feature-expansion phase. The prior Stage C evidence was valid for executable SHA `5c7899f...`, but current `main` is 35 executable commits ahead of that snapshot and has changed core transport behavior, Python bindings, the HTTPX facade, tests, dependencies, and qualification-relevant semantics. Routine CI is green on the planning baseline, but routine CI runs Tier 1 only; it does not renew the exact-SHA qualification.

The governing rule remains:

> The SHA written as the current qualification SHA must be the exact executable/test tree that passed every required qualification gate. If source, tests, build/dependency metadata, validation scripts, packaging configuration, compatibility-oracle code, or downstream-runner code changes after the freeze, discard the qualification evidence, choose a new frozen SHA, and restart this plan from the freeze step.

No new CI workflow, evidence framework, release automation, or speculative compatibility surface is required for this pass.

---

# 1. Why Corrective 08 is required

Corrective 07 qualified SHA `5c7899f...` after three clean full HTTPX runs, the API oracle, extended verification, downstream qualification, focused semantic tests, and remote CI.

Since that freeze, current `main` is 35 commits ahead and changes qualification-sensitive files including:

- `crates/eggfetch-core/src/client.rs`, `pipeline.rs`, `pool.rs`, `proxy.rs`, `redirect.rs`, `retry.rs`, `tls.rs`, compression/response/body/header code, and direct/proxy/SOCKS/H3 transports;
- `crates/eggfetch-python/src/client.rs`, `async_client.rs`, `response.rs`, `streaming.rs`, conversion/TLS/proxy/network-stream code;
- `crates/eggfetch-python/python/eggfetch/compat/httpx/*`, including `_client.py`, `_auth.py`, `_headers.py`, `_response.py`, `_ssl_context.py`, `_timeout.py`, and `_urls.py`;
- compatibility and native Python tests;
- Rust integration/adverse/proxy/retry/TLS/H2 tests;
- `Cargo.lock` and crate manifests;
- FFI and Node adapters.

The recent work is predominantly correctness and hardening, but exact-SHA qualification intentionally makes no distinction between a bug fix and a feature change. Both invalidate the prior executable binding.

Acceptance:

- [ ] The implementation agent treats `5c7899f...` as historical evidence only for current `main`.
- [ ] No current Stage C claim is renewed by inference from routine CI or from the old qualification runs.
- [ ] The new claim is based entirely on evidence collected for one newly frozen executable SHA.

---

# 2. Establish the candidate and audit the post-qualification change set

Before freezing a new candidate, compare the old qualified SHA to the candidate branch head:

```sh
git diff --stat 5c7899fefb6df087dfa1b3578fbef9ba64f87742..HEAD
git diff --name-status 5c7899fefb6df087dfa1b3578fbef9ba64f87742..HEAD
```

Build a short implementation-time audit table in working notes or the status ledger mapping each qualification-sensitive change cluster to its direct regression evidence. At minimum cover:

1. **Pool and lifecycle** — waiter cancellation/RAII, per-origin retention/eviction, permit ownership, shutdown and client-close behavior.
2. **Timeouts and streaming** — read/write timeout wrappers, response/body stream lifecycle, raw-vs-decoded consumption, partial consumption and close semantics.
3. **Compression** — buffered and streaming decompression limits, finite/positive ratio validation, encoded-byte accounting, stacked decoding, raw draining.
4. **Headers and protocol framing** — H2 forbidden-header behavior, H1 Auto behavior, multi-value header extension, target validation, HTTP version reporting.
5. **Redirect/auth/cookies** — sensitive-header stripping, auth reapplication, cookie state, retained-body replay and one-shot body rejection.
6. **Proxy/SOCKS/TLS** — proxy DNS resolution, proxy authentication, proxy-header isolation, proxy endpoint TLS, SNI, ALPN, local-address/socket-option propagation, SOCKS reuse.
7. **Retries** — retry classification, backoff validation, `Retry-After`, elapsed/attempt accounting, total-timeout interaction, body replayability.
8. **Python native API** — sync/async close races, free-threaded locking, response iterators, exception conversion, proxy/TLS conversion.
9. **HTTPX facade** — constructor/signature behavior, auth/config objects, headers/URL/query behavior, timeout conversion, SSLContext translation, redirects, response/raw stream lifecycle, extensions/network stream.
10. **FFI/Node changes** — verify they do not create independent networking behavior and that workspace-wide validation remains green. They are not part of the HTTPX public contract, but they are part of the executable tree and must not destabilize shared core behavior.

The purpose is not to manually re-prove every commit. It is to ensure that newly changed high-risk semantics are represented by direct tests before relying on the broad suite.

Acceptance:

- [ ] Every post-qualification behavioral change cluster has at least one direct test path.
- [ ] Any changed behavior lacking direct regression coverage receives a focused test before freeze.
- [ ] No test is added solely to encode eggfetch behavior where the contract requires HTTPX parity; reference behavior is checked where relevant.

---

# 3. Correct only concrete defects discovered during the audit

If the audit or focused tests reveal a real compatibility defect, fix it before qualification.

Allowed implementation work in this phase:

- correction of a demonstrated HTTPX 0.28.1 compatibility regression;
- correction of a test false positive or nondeterministic fixture that prevents trustworthy qualification;
- correction of a packaging/build defect needed to produce the candidate wheel;
- narrow ledger/test updates required to classify a genuine intentional difference.

Out of scope:

- new HTTPX versions;
- Trio/AnyIO support;
- private HTTPX modules;
- new transport families;
- broad API redesign;
- speculative elimination of already justified bounded differences;
- performance refactors unrelated to a failing qualification gate;
- new CI workflows or matrices.

Any executable/test/build/validation fix changes the candidate SHA. Do not collect final qualification evidence until all such fixes are committed.

Acceptance:

- [ ] No known required-now defect remains open at freeze.
- [ ] Every newly retained difference is intentional, narrow, tested, and represented in the compatibility ledger.

---

# 4. Focused post-hardening semantic gate

Before freezing, run focused tests that directly exercise the high-risk areas changed since Corrective 07. Use the existing tests whenever they already provide strong evidence; add tests only for concrete uncovered behavior.

Required focused coverage:

## 4.1 Pool/cancellation/lifecycle

- cancelled pool waiter releases bookkeeping and cannot orphan per-origin state;
- pool retention/eviction cannot evict an origin with an active waiter;
- sync and async client close remain idempotent;
- close/request races do not panic or use a poisoned/invalid runtime state;
- partially consumed streams release permits only at the documented lifecycle boundary.

## 4.2 Compression/raw-body boundary

- invalid `DecompressionLimit` values (`NaN`, infinity, negative, invalid ratios) fail deterministically;
- buffered and streaming paths enforce the same configured policy where their contracts overlap;
- raw iteration returns encoded bytes where the compatibility facade promises raw bytes;
- automatic draining for pool reuse does not trigger decompression-bomb expansion;
- line/text iteration preserves bounds and UTF-8 behavior.

## 4.3 H1/H2 framing

- `Auto` does not strip HTTP/1.1-only headers before H2 is actually selected;
- H2-only and explicit H2 requests reject/strip forbidden headers as required;
- standard TLS, SNI override, direct-specialized, UDS, and SOCKS H2-only routes still behave as documented;
- the HTTP CONNECT H2 residual remains explicit and does not silently downgrade;
- `stream_id` remains absent rather than synthesized if Hyper still cannot expose it.

## 4.4 Redirect/auth/cookie/body replay

- same-origin and cross-origin authorization behavior matches the pinned reference contract or the documented bounded difference;
- proxy authorization never leaks to the origin;
- request-local cookies and explicit `Cookie` handling preserve the facade contract;
- buffered retained bodies replay correctly;
- arbitrary one-shot body iterators are rejected before an unsafe redirect/retry replay;
- empty/unknown-length stream classification does not incorrectly authorize replay.

## 4.5 Proxy/SOCKS/TLS

- proxy DNS/address handling works for the pinned HTTPX forms;
- SOCKS auth and domain/IP ATYP behavior remain pinned;
- route-local SOCKS pooling is reusable and cancellation-safe;
- `Proxy(headers=...)` stays on the proxy leg only;
- HTTPS proxy endpoint TLS trust remains isolated from origin TLS configuration;
- `local_address` and socket options propagate through the intended direct/SNI routes;
- SSLContext translation remains fail-closed for rustls-unrepresentable state and exact for supported helper/live-state cases.

## 4.6 HTTPX object/facade behavior touched post-qualification

At minimum rerun focused files covering:

- auth/config objects;
- headers;
- URL/query semantics;
- timeouts;
- SSLContext translation and network proof;
- redirect state machine;
- response/raw stream lifecycle;
- extensions/trace;
- 101 network stream wrappers.

Acceptance:

- [ ] Focused post-hardening gate is green before the freeze.
- [ ] Tests assert the intended semantic result, not merely a generic exception or unrelated network failure.
- [ ] Required environment-supported behavior has no masking skip/xfail.

---

# 5. Freeze the new executable/test candidate

After all concrete fixes and focused tests are complete:

1. run `./scripts/check.sh` once as a pre-freeze sanity check;
2. commit all executable, tests, manifests, dependency, compatibility-oracle, validation, packaging, and downstream-runner changes;
3. record the full 40-character commit as `FROZEN_EXECUTABLE_SHA`;
4. verify a clean worktree;
5. perform all remaining qualification commands against that exact commit.

Qualification-invalidating files include, at minimum:

- `crates/**` source and tests;
- `Cargo.toml`, `Cargo.lock`, crate manifests/build scripts;
- Python package/facade source and tests;
- `scripts/check.sh` and any script used by qualification;
- compatibility manifest/oracle generators and ledger-validation code;
- downstream qualification runner/fixtures if they affect pass/fail evidence;
- package/wheel configuration;
- CI/build configuration that materially changes the built/tested artifact.

Documentation-only files may change after freeze, but the broad documentation pass is intentionally scheduled after qualification.

Acceptance:

- [ ] Exactly one frozen executable/test SHA is designated.
- [ ] Worktree is clean at freeze.
- [ ] No qualification result is copied from an older SHA.
- [ ] Any later executable change automatically restarts Corrective 08 from this section.

---

# 6. Tier 1 routine verification on the frozen SHA

Run:

```sh
./scripts/check.sh
```

Record:

- exact frozen SHA;
- Rust toolchain and Python version;
- Rust test count;
- Python native behavior test count;
- compatibility smoke-kernel count;
- failures, skips, xfails, and non-failing warning classes;
- elapsed time if useful.

Acceptance:

- [ ] Tier 1 passes on `FROZEN_EXECUTABLE_SHA`.
- [ ] No required test is removed, weakened, or skipped to reach green.

---

# 7. Tier 2 extended verification on the frozen SHA

Install the pinned compatibility dependencies and run:

```sh
python -m pip install -r compat/httpx/0.28.1/requirements.txt
./scripts/check.sh extended
```

This must exercise the repository's existing extended path, including full compatibility, API oracle, feature matrix, feature-gated tests, docs/doctests, FFI, resource/lifecycle/soak checks, lossless merge, benchmarks, and the current downstream gate when its artifact prerequisite is present.

Do not change `scripts/check.sh` during evidence collection. If the script is wrong, fix it, commit, choose a new freeze, and restart.

Optional MSRV handling follows `docs/verification-policy.md`: an unavailable configured toolchain may be an explicit recorded skip only where the current policy already allows that.

Acceptance:

- [ ] Extended verification passes on the same frozen SHA.
- [ ] Every optional omission is named and justified.
- [ ] No extended failure is dismissed because routine CI is green.

---

# 8. Tier 3 package validation on the frozen SHA

Because the post-qualification changes include native bindings, manifests, dependency resolution, and packaging-sensitive code, run the existing package gate on the same clean frozen tree:

```sh
./scripts/check.sh package
```

This is local validation only and must not publish.

Record the wheel filename and SHA-256 of the produced candidate artifact if the script or working procedure exposes it. If the downstream qualification later builds a separate wheel, record that artifact independently rather than assuming hashes match.

Acceptance:

- [ ] Package validation passes from a clean frozen tree.
- [ ] No `--allow-dirty` or publication action is used.
- [ ] Candidate wheel can be installed and smoke-tested by the existing package procedure.

---

# 9. Full pinned HTTPX compatibility suite — three consecutive clean runs

Run the complete compatibility suite three consecutive times on the frozen SHA:

```sh
EGGFETCH_COMPAT_REQUIRED=1 python -m pytest \
  crates/eggfetch-python/tests/compat/ -q --strict-markers
```

For each run record:

- test count;
- pass/fail/skip/xfail counts;
- duration;
- Python, pytest, pytest-asyncio, HTTPX, httpcore, socksio and other relevant optional dependency versions.

Rules:

- no test changes between the three runs;
- no cherry-picking only successful runs around a known flake;
- any nondeterministic failure must be investigated;
- if a fixture/test determinism fix is required, commit it, designate a new frozen SHA, and restart all qualification evidence from Section 5.

Acceptance:

- [ ] Three consecutive full runs pass on one SHA.
- [ ] Counts are stable, or any deterministic count difference is explained.
- [ ] Zero unexplained failures/skips/xfails affect the documented Stage C contract.

---

# 10. Differential high-risk spot checks against pinned HTTPX

Retain or run direct reference/candidate comparisons for the areas most likely to have regressed during hardening:

- Headers and multi-value behavior.
- URL/query normalization and request-target behavior.
- Redirect state transitions, auth stripping/reapplication, and retained bodies.
- Timeout object conversion and connect/read/write/pool behavior.
- SSLContext supported/rejected states and wire-level TLS-version/trust behavior.
- Proxy environment precedence and `NO_PROXY` edge forms.
- SOCKS hostname/IP behavior and authentication.
- H2-only routes and the HTTP CONNECT residual.
- Raw response iteration and stream-consumed/closed state transitions.
- 101 `network_stream` ownership and sync/async wrapper selection.

Intentional differences may remain different from HTTPX, but the reference result and candidate result must be deliberate and linked to a stable allowed-difference/parity-case ID.

Acceptance:

- [ ] Every materially changed high-risk boundary has direct reference/candidate evidence or a linked existing differential case.
- [ ] Candidate-only expectation tests are not counted as parity proof.

---

# 11. API oracle and compatibility-ledger reconciliation

Run the existing manifest/oracle procedure against the frozen candidate. Do not hand-edit generated manifests.

Required result:

- zero unexplained public API differences;
- zero stale active allowed rows;
- zero resolved differences left in the active ledger;
- every retained active difference has a stable ID, rationale, migration impact, and named tests;
- public symbols remain scoped to the documented HTTPX 0.28.1 contract.

Review the currently retained bounded differences rather than assuming they are unchanged:

- Trio/AnyIO remains out of scope;
- Python 3.8/3.9 remain out of scope;
- rustls-unrepresentable arbitrary SSLContext state fails closed;
- HTTP/2 `stream_id` metadata is absent unless Hyper now exposes it truthfully;
- HTTP/2 origin framing through the hand-built HTTP CONNECT path remains bounded unless genuinely resolved;
- the four-element null-pointer socket-option form remains outside the safe Rust boundary;
- ordinary pooled `network_stream` remains unavailable unless ownership semantics genuinely changed;
- internal CONNECT tunnel non-exposure remains explicit;
- coroutine trace callbacks remain rejected unless the core can now await them safely without a new parallel networking path.

Acceptance:

- [ ] Oracle reports zero unexplained/stale/resolved-active differences.
- [ ] No allowed-difference row exists merely to make the oracle green.
- [ ] `allowed-differences.toml`, `resolved-differences.toml`, and `parity-cases.toml` agree with tested behavior.

---

# 12. Required downstream portfolio qualification

Build a fresh wheel from `FROZEN_EXECUTABLE_SHA` and run the repository's current required downstream portfolio with the existing controlled replacement procedure:

```sh
python scripts/run_downstream_compat.py \
  --artifact-manifest target/downstream-qualification/artifact-manifest.json \
  --required-only
```

Do not hardcode the historical four-package list if the checked-in required portfolio has legitimately changed; use the current repository manifest/configuration and record every required package that runs.

Record:

- candidate wheel filename and SHA-256;
- controlled HTTPX shim/replacement artifact hash if used;
- downstream package versions;
- exact behavioral test results;
- dependency-resolution diagnostics separately from behavioral failures;
- confirmation that no editable checkout or source tree shadows the installed candidate wheel.

`httpx-ws`/upgrade behavior, mounted/mock transports, event hooks, auth integrations, and other downstream fixtures that exercise the compatibility facade deserve particular attention.

Acceptance:

- [ ] Every current required downstream target passes.
- [ ] Candidate artifact hash is tied to `FROZEN_EXECUTABLE_SHA`.
- [ ] The environment proves the candidate wheel, not local source shadowing, supplied the HTTPX facade.

---

# 13. Remote CI evidence

Push the frozen executable commit normally and allow the existing single routine CI job to run. Do not create a special qualification workflow.

Record:

- workflow name;
- run ID;
- job ID if useful;
- head SHA;
- conclusion;
- relationship to `FROZEN_EXECUTABLE_SHA`.

A documentation-only descendant may also receive a green routine run later, but that is secondary evidence and cannot substitute for the executable freeze run.

Acceptance:

- [ ] Existing CI is green on the frozen executable tree or a provably executable-identical descendant.
- [ ] No required routine failure is ignored.

---

# 14. Renew the qualification records only after all gates pass

After Sections 6–13 pass, update the live qualification records. At minimum reconcile:

- `compat/httpx/0.28.1/profile.toml`;
- `compat/httpx/0.28.1/allowed-differences.toml`;
- `compat/httpx/0.28.1/resolved-differences.toml`;
- `compat/httpx/0.28.1/parity-cases.toml`;
- `compat/httpx/0.28.1/README.md`;
- `plans/httpx-parity-correction-status.md`;
- `docs/reference/compatibility.md` only to the extent needed to state the renewed qualification truthfully before the broader docs pass.

The profile must record the exact frozen SHA and qualification date. Historical `5c7899f...` evidence should remain clearly labeled historical/superseded, not deleted.

The current status section must include:

- new qualification SHA/date;
- planning/prior qualification baseline;
- focused post-hardening gate result;
- Tier 1 result;
- Tier 2 result;
- Tier 3 result;
- three full compatibility run results and durations;
- API-oracle result;
- downstream result and artifact hashes;
- remote CI evidence;
- environment versions;
- current retained differences.

Acceptance:

- [ ] `profile.toml` `qualification-sha` equals `FROZEN_EXECUTABLE_SHA` exactly.
- [ ] The current status section contains no contradictory active qualification SHA.
- [ ] The renewed claim is no broader than the tested Python 3.10+ asyncio-supported HTTPX 0.28.1 surface.

---

# 15. Qualification descendant boundary and handoff to documentation refresh

After the qualification records are committed:

1. compare `FROZEN_EXECUTABLE_SHA` to the qualification-record commit;
2. confirm every post-freeze changed file is documentation/ledger-only;
3. record the descendant audit in `plans/httpx-parity-correction-status.md`;
4. begin `plans/documentation-broad-truth-refresh-after-requalification.md` only after this audit is clean.

The documentation refresh may update Markdown/status/ledger prose without invalidating the executable SHA. It must not edit Rust/Python source, tests, manifests, scripts, workflows, or packaging configuration. If the docs audit discovers a source-code/doc-comment correction that requires an executable file change, stop the docs-only pass, make the source correction in a new executable commit, and re-run Corrective 08 from the freeze step.

Acceptance:

- [ ] The qualification-record commit is a documentation/ledger-only descendant of the frozen executable SHA.
- [ ] Broad documentation work starts only after the renewed Stage C evidence is bound and recorded.

---

# Final acceptance criteria

Corrective 08 is complete only when:

- [ ] The old `5c7899f...` qualification is treated as historical for current `main`.
- [ ] The 35+ post-qualification executable commits are audited by behavior cluster.
- [ ] Every changed high-risk cluster has direct regression coverage.
- [ ] Any concrete compatibility defects found during the audit are fixed before freeze.
- [ ] One exact executable/test SHA is frozen with a clean worktree.
- [ ] Focused post-hardening semantic tests pass on the final candidate.
- [ ] `./scripts/check.sh` passes on the frozen SHA.
- [ ] `./scripts/check.sh extended` passes on the same SHA, with only policy-permitted optional skips explicitly recorded.
- [ ] `./scripts/check.sh package` passes on the same clean SHA.
- [ ] The full pinned compatibility suite passes three consecutive clean runs on the same SHA.
- [ ] High-risk differential/reference spot checks are green or intentionally bounded.
- [ ] API oracle reports zero unexplained, stale, or resolved-active differences.
- [ ] Every current required downstream target passes against a wheel built from the frozen SHA.
- [ ] Candidate artifact hashes are recorded.
- [ ] Existing remote CI is green and tied to the frozen executable tree.
- [ ] `compat/httpx/0.28.1/profile.toml` is rebound to the exact frozen SHA and current date.
- [ ] `plans/httpx-parity-correction-status.md` contains one unambiguous current qualification section and preserves old evidence as historical.
- [ ] Active/resolved/parity ledgers reflect actual tested behavior.
- [ ] No post-freeze executable/test/build/validation/packaging change exists.
- [ ] The broad documentation refresh is handed off as a docs-only descendant phase.

## Closure statement

Only after every item above passes may the repository again describe the current executable tree as **Stage C qualified** for the documented Python 3.10+ asyncio-supported HTTPX 0.28.1 surface.

After that point, HTTPX parity work should reopen only for a newly discovered concrete defect, an intentionally expanded supported surface, or a newly pinned HTTPX version. Documentation cleanup proceeds separately so it cannot blur or invalidate the executable qualification boundary.