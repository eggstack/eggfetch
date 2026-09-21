# Post-Maintenance Closure Evidence Corrective Pass

Planning baseline: `5ced637af7479b95b6454697e638c7a3759d8670` (`main`, 2026-09-21)
Affected maintenance implementation freeze: `df2549f7c64ebfccde61ed36fef785d39e83b38d`
Parent program: `plans/api-preserving-maintenance-and-interop-hardening-program.md`
Prior closure owner: `plans/post-maintenance-api-requalification-and-state-closure.md`
Prior qualified compatibility freeze: `bc4800ee9428f0fd11d7d0b914c489b444fe93fc`
Reference contracts: `httpx==0.28.1`, `httpx2==2.12.0`
Normative verification policy: `docs/verification-policy.md`
Date: 2026-09-21

## Objective

Correct the remaining closure defects from the completed API-preserving
maintenance implementation without reopening the architecture/API campaign.

The implementation work itself is healthy: the Rust exact-surface oracle,
Python relational guardrails, versioned private SSLContext export, private
client/proxy decomposition, and CONNECT conformance changes all landed and
qualified locally. The current repository nevertheless has two evidence
problems and one state-hygiene defect:

1. the live HTTPX/HTTPX2 Stage C profiles and canonical compatibility
   documentation still bind to the pre-maintenance executable freeze
   `bc4800ee9428f0fd11d7d0b914c489b444fe93fc`, even though the maintenance
   campaign changed executable/test/validation inputs and qualified
   `df2549f7c64ebfccde61ed36fef785d39e83b38d`;
2. the newly-added negative evidence is representative but does not yet
   exercise every failure mode promised by the child plans, especially
   relational Python drift fixtures and malformed SSLContext-export schema
   cases;
3. `plans/README.md` still labels earlier second-pass performance work as
   Active even though its own status records completion.

Because strengthening the missing negative tests changes test/validation
inputs, this corrective must first complete those tests, then freeze a new
candidate and requalify it. Only after that new freeze is qualified may the
canonical Stage C records be rebound.

This is a closure/evidence corrective, not a new feature, performance, API,
HTTP/3, Node, compatibility-expansion, or release program.

## Confirmed planning-time state

At planning time:

- current `main` is
  `5ced637af7479b95b6454697e638c7a3759d8670`;
- maintenance implementation/test/tooling freeze
  `df2549f7c64ebfccde61ed36fef785d39e83b38d` is locally qualified;
- documentation descendants
  `9f16730cece5a1dbf6c936079852de165ed21add` and
  `5ced637af7479b95b6454697e638c7a3759d8670` have green remote CI;
- the first closure descendant `fcbcf588289fa7eb68d4ed6ca76c2b83795c8af7`
  failed routine CI because profile contract checks inherited repository-wide
  warning promotion; `df2549f7...` corrected that by clearing `RUSTFLAGS`
  only for the profile contract matrix while retaining workspace/all-features
  `-D warnings` ownership in the normal clippy/Tier 1 path;
- both `compat/httpx/0.28.1/profile.toml` and
  `compat/httpx2/2.12.0/profile.toml` still record
  `qualification-sha = "bc4800ee9428f0fd11d7d0b914c489b444fe93fc"`;
- `docs/residual-differences.md` and
  `docs/reference/compatibility.md` still describe `bc4800ee...` as the
  current Stage C freeze;
- the parent closure record states that canonical exact-SHA records were
  renewed when required, so the live records and the closure claim disagree;
- `plans/README.md` still has the second-pass performance corrective and
  second-pass performance program under Active headings even though both
  entries say they are complete;
- issue #24 remains open and the plan index records publication as pending;
  this is currently truthful and must not be changed unless publication
  actually occurs;
- the Python 3.15 wheel-production rehearsal remains separately pending and is
  outside this corrective.

## Scope constraints

This corrective is allowed to change:

- tests for the existing Python relational guards;
- tests for the private SSLContext export decoder;
- validation self-test helpers only where required to make the promised
  negative cases directly testable;
- compatibility profiles/ledger;
- canonical compatibility/residual-difference documentation;
- plan files and plan index.

This corrective must not change production behavior.

Do not:

- change any public Rust/Python/C/CLI API;
- change Cargo features/defaults;
- change transport/TLS/proxy/timeout/decompression semantics;
- broaden or narrow accepted SSLContext behavior;
- change HTTPX/HTTPX2 reference versions or allowed differences;
- add a new production dependency;
- add a new GitHub Actions job/matrix;
- change Node or HTTP/3 maturity;
- publish/tag/release;
- close issue #24 unless publication has actually completed;
- claim Python 3.15 release qualification unless its separate rehearsal has
  actually completed.

If a negative test exposes a real production defect rather than only an
evidence gap, stop the evidence-only assumption, fix the defect in the same
corrective only if it remains API/behavior-preserving, and record the resulting
new executable freeze explicitly. If the fix would alter supported behavior,
split a separate corrective.

## Part 1 — complete Python relational negative evidence

The runtime and typing checkers now encode useful relational invariants, but
their current `--self-test` paths mainly prove low-level shape helpers detect
one simple mutation.

Strengthen the checker self-tests/fixtures so they prove the full relational
guards fail for representative contract drift.

### Runtime checker

For `scripts/check_native_python_api.py`, add pure/fake-surface tests that
exercise `_check_relational_runtime_contracts()` directly.

At minimum prove failures for:

1. Client constructor gains/drops/reorders a keyword while AsyncClient does
   not;
2. one sync/async mirror method changes a parameter name/order/default;
3. one top-level helper gains a client-only keyword or loses the required
   `limits` relationship;
4. a bodyless convenience helper gains `content`/`data`/`json`/`files`;
5. a body-capable helper loses one of those keywords.

The self-tests should assert on the checker error category/message, not merely
assert two raw parameter lists differ.

Do not create a second runtime API manifest.

### Typing checker

For `scripts/check_python_typing_surface.py`, add focused temporary-package or
AST fixture mutations that exercise `_check_relational_stub_contracts()` and
the existing root-export/semantic checks.

At minimum prove failures for:

1. Client/AsyncClient constructor typing drift;
2. sync method accidentally declared `async def` or async mirror declared
   synchronously;
3. async body-capable method loses `AsyncBody`;
4. sync method gains `AsyncBody`;
5. top-level helper gains `extensions` or otherwise ceases to match the
   reviewed Client subset plus `limits`;
6. root runtime/stub export mismatch through the existing fixture mechanism;
7. semantic return annotation drift for at least one reviewed async method or
   `start_tls`.

Prefer temporary fixture text/AST over editing production stubs in place.

### Acceptance

- [ ] Runtime relational self-tests fail when each promised relationship is
      intentionally mutated.
- [ ] Typing relational self-tests fail for sync/async kind, body annotation,
      top-level-shape, export, and semantic-return drift.
- [ ] Existing production `native_api_manifest.json` remains the only
      reviewed manifest.
- [ ] No public Python signature/stub/export changes are needed.

## Part 2 — complete malformed SSLContext private-contract evidence

The current private export tests cover version, classification,
`verify_mode`, and malformed DER-list examples. Complete the negative matrix
promised by `python-ssl-context-private-contract-hardening.md`.

Add focused tests that monkeypatch `_export_ssl_context_state` or exercise a
pure Rust payload decoder as appropriate.

Required cases:

1. payload is not a mapping;
2. missing `schema_version`;
3. unknown schema version;
4. missing `classification`;
5. invalid classification token;
6. missing `verify_mode`;
7. wrong `verify_mode` type;
8. missing/wrong-type `check_hostname`;
9. missing/wrong-type `ca_certs_der`;
10. malformed individual CA entry;
11. missing/wrong-type `min_version`;
12. missing/wrong-type `max_version`;
13. unsupported numeric minimum TLS version;
14. unsupported numeric maximum TLS version;
15. missing `helper_metadata`;
16. non-mapping non-None `helper_metadata`;
17. malformed helper metadata `verify`, `cert_path`, or `key_path`.

Each must fail before any network dispatch. Preserve current exception classes
where already tested; do not rewrite public errors to make the matrix easier.

Also retain positive controls for:

- default SSLContext;
- verify=False helper context;
- check_hostname=False;
- custom CA;
- TLS 1.2/1.3 bounds;
- helper mTLS provenance;
- mutation invalidation;
- proxy TLS and destination TLS paths.

### Acceptance

- [ ] Every required/malformed field has direct negative evidence.
- [ ] Unknown schema versions fail closed.
- [ ] No malformed payload reaches transport dispatch.
- [ ] Existing accepted/rejected SSLContext semantics remain unchanged.
- [ ] No private key material or secret-bearing payload is introduced.

## Part 3 — freeze a new corrective candidate

Because Parts 1–2 change tests and/or validation tooling, the old
`df2549f7...` maintenance freeze becomes historical for exact-SHA purposes.

After those changes:

1. identify the last test/tooling/executable commit in this corrective;
2. record it as the new final maintenance-corrective freeze;
3. ensure all later descendants are documentation/profile/ledger/index only;
4. record stable Rust/Python/tool versions and any policy-defined optional
   skips.

Do not bind Stage C to the documentation head.

### Acceptance

- [ ] One exact corrective freeze is named.
- [ ] No test/build/validation change exists after that freeze.
- [ ] `df2549f7...` remains historical evidence for the original maintenance
      implementation, not the live Stage C binding.

## Part 4 — requalify the new freeze

Run the canonical current-policy gates against the corrective freeze.

Required:

- `./scripts/check.sh`;
- `./scripts/check.sh extended`;
- `./scripts/check.sh package` when required by current policy for these
  validation/test changes;
- `./scripts/check_security.sh`;
- exact Rust 1.89.0 MSRV checks owned by extended validation;
- `python scripts/check_rust_public_api.py` with the pinned
  `cargo-public-api 0.52.0` / `nightly-2026-05-07` and
  `cargo-semver-checks 0.49.0` environment already documented by the repo;
- native Python API/typing checks including the new negative self-tests;
- the complete SSLContext focused suite;
- full pinned HTTPX 0.28.1 and HTTPX2 2.12.0 compatibility suites;
- both compatibility API oracles;
- FFI, feature matrix, docs/doctests, lifecycle/resource/soak/native frame and
  proxy controls already owned by extended validation;
- Node Rust checks;
- Node JS/downstream only when their required artifacts exist; otherwise record
  the existing policy-defined skip rather than PASS.

Do not reintroduce a historical three-run requirement unless the current
verification policy now requires it.

### Acceptance

- [ ] Exact Rust six-profile API snapshots remain byte-identical to planning
      baseline `03ecba973010e2858bf16a2b5f84d51ce70adae4`.
- [ ] Semver check passes with no breaking result.
- [ ] Native Python public surface has zero drift.
- [ ] HTTPX/HTTPX2 have zero new unexplained API or behavioral differences.
- [ ] SSLContext accepted/rejected behavior is unchanged.
- [ ] Tier 1, extended, applicable package, security, MSRV and docs gates pass.
- [ ] Optional artifacts are truthfully skipped when absent.

## Part 5 — renew the canonical Stage C exact-SHA records

Only after Part 4 passes, update the live compatibility evidence to the new
corrective freeze.

Update:

- `compat/httpx/0.28.1/profile.toml`;
- `compat/httpx2/2.12.0/profile.toml`;
- `plans/httpx-parity-correction-status.md`;
- `docs/residual-differences.md`;
- `docs/reference/compatibility.md`;
- any other canonical compatibility reference explicitly identified by
  `AGENTS.md` / current verification policy.

For both profiles:

- keep `stage = "stage-c-qualified"`;
- keep `status = "qualified"`;
- set `qualification-sha` to the new corrective freeze;
- set the qualification date to the actual qualification date;
- set `previous-qualification-sha` to
  `bc4800ee9428f0fd11d7d0b914c489b444fe93fc` or otherwise preserve the
  profile's established predecessor-history convention;
- add concise commentary that the maintenance campaign plus this corrective
  changed private implementation/test/validation inputs without public
  compatibility drift;
- keep `df2549f7...` as intermediate maintenance evidence in the corrective
  plan rather than pretending it was the prior live Stage C profile binding.

Do not change allowed-difference files, compatibility categories, reference
versions, Python-version support claims, or feature-extra mappings.

### Acceptance

- [ ] Both profile files bind to the same new corrective freeze.
- [ ] The live parity ledger names the same freeze as current Stage C.
- [ ] Canonical compatibility/residual docs name the same freeze.
- [ ] `bc4800ee...` is clearly historical after the renewal.
- [ ] No new compatibility waiver/allowed difference is introduced.
- [ ] No documentation calls the later docs-only head the executable freeze.

## Part 6 — repair closure records and plan-index truth

Update the maintenance program and closure plan with a corrective addendum.

Required statements:

- original maintenance implementation landed on `df2549f7...`;
- the first closure record incorrectly marked canonical exact-SHA renewal
  complete while live profiles/docs still pointed to `bc4800ee...`;
- this corrective added the missing negative evidence, created a new final
  test/tooling freeze, requalified it, and renewed the canonical records;
- no production API/capability change occurred.

Update `plans/README.md`:

1. mark this corrective Active while implementation is in progress, then
   Completed only after qualification and remote CI;
2. mark the parent maintenance program complete only after this corrective
   closes;
3. change the second-pass performance corrective heading from Active to
   Completed because its own recorded status is complete;
4. change the second-pass performance ownership program heading from Active to
   Completed because its own recorded status is complete;
5. retain issue #24 as active/fixed-but-unpublished unless release publication
   has actually completed;
6. retain Python 3.15 as active/pending rehearsal unless that separate evidence
   has actually completed.

Do not rewrite historical execution details solely for cosmetic consistency.

### Acceptance

- [ ] Plan headings agree with their recorded status.
- [ ] The maintenance parent does not claim final closure before this
      corrective passes.
- [ ] Issue #24 and Python 3.15 remain truthful independent pending work.
- [ ] Historical freeze versus live freeze terminology is unambiguous.

## Part 7 — remote CI and descendant audit

Push the final qualified evidence/index descendant and wait for the normal
single-job GitHub CI result.

After CI succeeds:

- record workflow/run ID and checked-out SHA;
- verify the descendant after the corrective freeze contains only
  documentation/profile/ledger/index changes;
- mark this corrective complete;
- mark the parent maintenance program fully closed.

If remote CI fails, do not record closure until the failure is understood.
Any fix touching tests/validation/executable inputs creates a new corrective
freeze and requires the relevant requalification again.

## Final acceptance criteria

- [ ] Python relational negative evidence covers the promised drift classes.
- [ ] SSLContext malformed-contract evidence covers every required field and
      fails before dispatch.
- [ ] One new exact corrective freeze is recorded.
- [ ] Six-profile Rust public API oracle remains identical to
      `03ecba973010e2858bf16a2b5f84d51ce70adae4`.
- [ ] Rust semver check passes.
- [ ] Native Python manifest/typing/API surface remains unchanged.
- [ ] HTTPX 0.28.1 and HTTPX2 2.12.0 full compatibility/API oracles have zero
      new unexplained drift.
- [ ] Canonical Stage C profiles, ledger, residual differences, and
      compatibility docs all bind to the same new freeze.
- [ ] Tier 1, extended, applicable package, security, exact MSRV, docs and
      adapter/resource gates pass.
- [ ] No compatibility waiver, feature/default, public API, or supported
      behavior changes.
- [ ] Remote CI is green on the final documentation/evidence descendant.
- [ ] Stale performance Active headings are corrected.
- [ ] Issue #24 remains fixed-but-unpublished unless publication actually
      completes.
- [ ] Python 3.15 remains separately pending unless its rehearsal actually
      completes.
- [ ] Parent maintenance program is marked fully complete only after every item
      above is satisfied.

## Stop conditions

Stop and split new work rather than expanding this corrective if:

- a negative test reveals a real public behavior/API defect requiring a
  compatibility change;
- any public Rust/Python/C/CLI surface must change;
- any Cargo feature/default must change;
- SSLContext translation must accept/reject a case differently;
- a new compatibility exception/allowed difference is proposed;
- HTTP/3 or Node maturity changes;
- new production dependencies or CI topology changes are required;
- issue #24 publication or Python 3.15 release work becomes necessary to make
  this corrective pass.

This corrective succeeds by making the already-implemented maintenance work
fully evidenced and the repository's live exact-SHA state truthful.
