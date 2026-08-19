# HTTPX Parity Corrective 07 — Final Exact-SHA Requalification

Planning baseline: `25c2c6f01138e2d6a59d1256076ec84972a92d83`
Depends on: `plans/httpx-parity-corrective-06-final-semantic-truthfulness.md`
Reference: `httpx==0.28.1` / `httpcore==1.0.9`

## Objective

Re-qualify the HTTPX 0.28.1 compatibility surface only after Corrective 06 has finished all executable and test changes, and bind the renewed Stage C claim to the exact frozen executable SHA that was actually exercised.

This phase is intentionally evidence-heavy and implementation-light. It exists to prevent the failure mode already seen earlier in the parity program: qualification evidence being collected for one executable tree and then reused after later compatibility code changes.

The governing rule is:

> The SHA recorded in `compat/httpx/0.28.1/profile.toml` must be the final executable/test commit. Every later commit included in the closure may change only documentation, ledgers, status records, or other non-executable evidence files. If executable code, tests, build configuration that changes the produced artifact, or validation scripts change after the frozen SHA, the qualification is invalid and this plan restarts from the freeze step.

Do not add new CI complexity during this phase. Use the repository's existing verification tiers and current manual qualification procedure.

---

# 1. Preconditions

Corrective 07 must not begin until Corrective 06 acceptance is complete.

Required preconditions:

- all intended executable Rust changes are committed;
- all intended Python binding/facade changes are committed;
- all new or corrected semantic tests are committed;
- all fixture changes needed to execute those tests are committed;
- parity ledgers are structurally ready for final classification, although qualification result fields/status may still be updated afterward;
- no known failing semantic item from Corrective 06 is being deferred without an explicit bounded-difference disposition;
- `./scripts/check.sh` passes once on the candidate executable tree before it is frozen.

Do not freeze a SHA while the implementation agent still expects code/test changes.

---

# 2. Define the executable boundary precisely

## 2.1 What counts as executable for qualification

Treat a commit as qualification-invalidating if it changes any of the following after the frozen SHA:

- `crates/**` Rust source;
- Python package/facade source;
- native binding source;
- test fixtures or tests used as qualification evidence;
- `Cargo.toml`, `Cargo.lock`, Python package metadata, build scripts, feature definitions, compiler/linker configuration, or dependency resolution;
- `scripts/check.sh` or any script invoked by the qualification procedure;
- workflow/build configuration if it materially changes what is built or tested;
- compatibility generator/oracle code used to determine pass/fail;
- downstream-runner code used as evidence;
- wheel build configuration or packaging hooks.

Documentation-only descendants are allowed after qualification if they do not alter generated artifacts or validation behavior.

## 2.2 Freeze procedure

Once Corrective 06 is complete:

1. commit all remaining executable/test changes;
2. record the full 40-character SHA as `FROZEN_EXECUTABLE_SHA` in the working notes/status file;
3. verify the worktree/branch contains no uncommitted executable changes;
4. verify subsequent qualification commands are run against that exact commit;
5. do not update `profile.toml` to qualified yet.

If another executable commit is needed, discard the in-progress qualification evidence, designate the newer SHA, and start again.

Acceptance:

- [ ] One unambiguous frozen executable/test SHA exists.
- [ ] All qualification commands can report/verify that SHA before running.
- [ ] No future/placeholder qualification SHA is written into the profile.

---

# 3. Focused semantic closure gate

Before spending time on the full suite, rerun the exact targeted tests that justified Corrective 06.

At minimum include suites covering:

## 3.1 SSLContext safety

- external arbitrary SSLContext rejection or the exact supported subset;
- helper-context provenance/mutation handling;
- real external/helper mTLS mutation cases;
- custom CA content differentiation;
- empty verified trust-store behavior;
- `CERT_REQUIRED + check_hostname=False` wire proof;
- TLS 1.2/1.3 min/max handshake proof;
- proxy SSLContext translation;
- `NetworkStream.start_tls()` translation/rejection.

No test may rely only on kwargs/classifier shape when the acceptance criterion is handshake behavior.

## 3.2 Extensions and trace

- sync buffered;
- sync streaming;
- async buffered;
- async streaming;
- target `OPTIONS *` / custom target;
- SNI override;
- actual sync callback;
- actual `async def` callback when supported;
- explicit pre-dispatch rejection when async trace is a bounded difference;
- callback raises and original exception propagates;
- callback failure prevents later lifecycle work at the declared boundary;
- redirects/retries do not silently lose supported extensions.

## 3.3 network_stream

All four 101 quadrants:

- sync buffered;
- sync streaming;
- async buffered;
- async streaming.

Each requires correct wrapper type, leading bytes, bidirectional I/O, lifecycle after parent-client close, idempotent close, and no pool reuse.

## 3.4 H2 route policy

- standard H2-only TLS success;
- H1-only TLS rejection under H2-only policy;
- cleartext H2 prior knowledge;
- local-address/socket-option H2;
- UDS H2;
- SNI-override H2-only;
- SOCKS H2-only for every route claimed supported;
- explicit active residual tests for any unsupported SOCKS/SNI path;
- HTTP CONNECT residual `H2-009` remains stable unless intentionally resolved;
- `stream_id` remains actual-or-absent, never synthesized.

Focused gate acceptance:

- [ ] Every Corrective 06 acceptance behavior is directly exercised.
- [ ] No false-positive test remains where an unrelated network error can satisfy the assertion.
- [ ] No capability skip/xfail masks a required environment-supported behavior.

---

# 4. Tier 1 repository verification

Run the repository's unchanged routine gate on the frozen SHA:

```text
./scripts/check.sh
```

Record in `plans/httpx-parity-correction-status.md`:

- exact command;
- frozen SHA;
- date/time/environment;
- Rust test counts;
- doctest counts;
- Python non-compat count;
- compatibility smoke/kernel count;
- failures/skips/xfails;
- any warning classes that are expected and non-failing.

Do not edit `scripts/check.sh` to get a clean run during this phase. If the script itself is wrong, that is an executable/verification change: fix it, commit it, designate a new frozen SHA, and restart.

Acceptance:

- [ ] Tier 1 passes on the exact frozen SHA.
- [ ] No required test is removed or skipped merely to reach green.

---

# 5. Extended verification

Run the repository's current extended verification path exactly as documented by the project, presently conceptually:

```text
./scripts/check.sh extended
```

Use the repository's actual supported invocation at implementation time; do not add new gates unless a missing existing acceptance requirement cannot otherwise be exercised.

Record:

- feature matrix results;
- feature-gated Rust/Python tests;
- docs/doctests;
- FFI checks;
- resource/lifecycle/soak checks currently included by the script;
- downstream gate portions included by the extended mode;
- MSRV disposition if the configured toolchain is unavailable.

MSRV may be recorded as an environment-unavailable optional check only if that is already the repository's documented policy. Do not silently call a required check optional.

Acceptance:

- [ ] Extended verification passes on the same frozen SHA.
- [ ] Any optional omission is explicitly recorded with reason.

---

# 6. Full pinned HTTPX compatibility suite — three clean runs

Run the complete pinned compatibility suite against `httpx==0.28.1` / `httpcore==1.0.9` using the repository's existing environment procedure.

Requirements:

- run it **three consecutive clean times** on the same frozen executable SHA;
- do not reuse cached pass/fail output from a previous SHA;
- do not edit tests between runs;
- record test count and wall time for each run;
- record Python, pytest, pytest-asyncio, HTTPX, httpcore, and relevant optional dependency versions;
- record skips/xfails explicitly;
- required Corrective 06 semantics must have zero capability skips in the qualification environment unless the compatibility contract explicitly excludes that platform capability.

If any run fails nondeterministically:

1. investigate;
2. if only an environment fixture is flaky, fix the fixture/test determinism;
3. commit the fix;
4. assign a new frozen executable SHA;
5. restart **all** Corrective 07 qualification evidence.

Do not cherry-pick a passing run around a known flaky failure.

Acceptance:

- [ ] Three consecutive full compatibility runs pass.
- [ ] Counts are stable or any deterministic count change is explained.
- [ ] Zero unexplained failures, skips, or xfails affect the documented Stage C surface.

---

# 7. Differential semantic spot checks against pinned HTTPX

The full compatibility suite is necessary but not sufficient because prior false-positive tests passed while semantics were wrong.

Perform/retain direct differential fixtures for the high-risk boundaries:

## 7.1 SSLContext

Compare reference vs candidate for:

- helper default context;
- arbitrary external context disposition;
- custom CA trust;
- hostname-verification disabled while chain verification remains enabled;
- TLS version bounds;
- unsupported mTLS/ALPN external state.

The candidate may intentionally reject a context HTTPX accepts, but that exact case must appear as a bounded difference and must fail before transport dispatch.

## 7.2 Trace

Compare:

- callback type expectations;
- event ordering for the supported subset;
- callback exception behavior;
- sync/async differences.

Do not claim full trace parity if coroutine callback support remains intentionally bounded.

## 7.3 H2

Compare:

- H2-only TLS vs H1 peer;
- cleartext H2 prior knowledge;
- SNI route;
- SOCKS route;
- HTTP CONNECT route residual.

Wire fixtures should prove the protocol actually transmitted rather than trusting response metadata alone.

## 7.4 Network stream

Compare the observable ownership contract for 101 upgrades. Keep ordinary pooled raw-stream absence explicitly bounded if Hyper cannot expose it safely.

Acceptance:

- [ ] Every high-risk boundary has at least one reference/candidate differential fixture or a clearly linked existing parity case.
- [ ] Candidate-only “expected difference” tests are not mislabeled as parity evidence.

---

# 8. API oracle and ledger validation

Run the repository's existing API manifest/oracle procedure against the frozen candidate.

Required outcome:

- zero unexplained public-surface differences;
- zero stale active rows;
- zero resolved differences remaining in the active ledger;
- all retained differences map to stable IDs and named tests;
- all current public symbols in the supported contract are represented in the manifest.

Review especially the rows touched by Corrective 06:

- SSLContext bounded acceptance/rejection;
- trace callback behavior;
- H2 SNI/SOCKS route classification;
- network-stream ordinary/upgrade classification if represented in the ledger.

Do not use the oracle to hide semantic differences as “allowed” merely because the surface signature matches.

Acceptance:

- [ ] Oracle reports zero unexplained/stale/resolved-active differences.
- [ ] Every new active difference is intentional, narrowly stated, and tested.

---

# 9. Downstream portfolio qualification

Run the existing required downstream compatibility portfolio against a wheel built from the frozen executable SHA.

Current required portfolio from the prior qualification:

- `respx`;
- `httpx-sse`;
- `httpx-auth`;
- `httpx-ws`.

Use the same controlled replacement procedure already present in the repository unless it must change for a real packaging defect. If downstream-runner code changes, that changes the evidence mechanism and requires a new frozen SHA if the runner is considered executable qualification infrastructure.

Record:

- wheel filename;
- wheel SHA-256;
- controlled HTTPX-replacement wheel SHA-256 if that procedure remains in use;
- package versions/commits;
- exact test counts/results;
- diagnostic dependency warnings separately from behavioral failures.

Pay particular attention to `httpx-ws`, because 101/network-stream wrapper mode changed in Corrective 06 and this downstream is likely to exercise that boundary.

Acceptance:

- [ ] All required downstream packages pass.
- [ ] Wheel hash corresponds to the frozen executable SHA.
- [ ] No local editable/source tree accidentally shadows the candidate wheel.

---

# 10. Remote CI evidence

The frozen executable commit should be pushed normally so the repository's existing push CI runs against it.

Record the workflow run associated with the frozen executable SHA when available:

- workflow name;
- run ID;
- job ID(s);
- head SHA;
- conclusion;
- duration if useful.

If the final profile/status update is a documentation-only descendant and CI also runs there, that later green run is useful secondary evidence, but it does not replace the executable-SHA run.

Do not add a new qualification workflow solely for this line of work.

Acceptance:

- [ ] Existing routine CI is green for the frozen executable tree or an exactly equivalent unchanged executable tree, with the relationship documented.
- [ ] No failing required check is ignored.

---

# 11. Final documentation and ledger reconciliation

Only after all gates above pass, update the documentation/ledger state.

Review and reconcile at minimum:

- `compat/httpx/0.28.1/profile.toml`;
- `compat/httpx/0.28.1/allowed-differences.toml`;
- `compat/httpx/0.28.1/resolved-differences.toml`;
- `compat/httpx/0.28.1/parity-cases.toml`;
- `compat/httpx/0.28.1/README.md`;
- `docs/reference/compatibility.md`;
- `docs/residual-differences.md`;
- `plans/httpx-parity-correction-status.md`;
- README/architecture text only where it currently states a behavior corrected by Corrective 06.

## 11.1 Profile update

Set the current qualification fields to the exact frozen executable SHA and current qualification date only after evidence is complete.

The renewed Stage C claim must be no broader than the tested contract:

- pinned HTTPX 0.28.1;
- documented Python versions actually supported by EggFetch;
- documented asyncio-supported surface;
- explicit retained differences.

Do not describe arbitrary OpenSSL `SSLContext` acceptance if Corrective 06 intentionally narrowed it.

Do not describe coroutine trace callbacks as supported if they are a bounded difference.

Do not describe SOCKS/SNI H2-only as closed unless the differential route tests prove it.

## 11.2 Residual-difference minimum set

Retain the already justified differences unless Corrective 06 genuinely resolved them:

- HTTP/2 `stream_id` unavailable through Hyper;
- HTTP/2 origin framing through the hand-rolled HTTP CONNECT proxy path (`H2-009`);
- safe Rust rejection of HTTPX's four-element null-pointer socket-option form;
- ordinary pooled HTTP/1/H2 raw `network_stream` absence / no writable shared socket;
- internal proxy CONNECT tunnel non-exposure;
- unsupported arbitrary SSLContext/OpenSSL state;
- async trace callback difference if true awaiting was intentionally not implemented;
- SNI/SOCKS H2 route difference if any route remains unsupported.

Each retained difference must state migration impact and concrete evidence.

---

# 12. Post-qualification descendant audit

After writing the final qualification/profile/status documentation:

1. compare `FROZEN_EXECUTABLE_SHA` to final `main`;
2. inspect every changed file in descendants;
3. confirm descendants are documentation/ledger-only;
4. confirm no test, source, build metadata, validation script, or packaging config changed;
5. record this comparison in the closure status.

If executable content changed, the profile must not remain qualified. Reopen Corrective 07 with the new executable SHA.

Acceptance:

- [ ] Final main is a documentation/ledger-only descendant of the frozen executable SHA.
- [ ] `profile.toml` points exactly at that executable SHA.

---

# 13. Required closure evidence record

`plans/httpx-parity-correction-status.md` should contain one concise current section for this final pass with:

- final executable SHA;
- qualification date;
- Corrective 06 baseline/range;
- focused semantic test result/count;
- Tier 1 result/count;
- extended result;
- three full compatibility run counts/times;
- API oracle result;
- downstream result and wheel hashes;
- remote CI run reference;
- current retained differences;
- statement that later descendants are documentation/ledger-only;
- environment versions.

Historical qualification sections may remain below it, clearly marked superseded. Avoid contradictory duplicate “current designation” language in historical sections.

---

# Final acceptance criteria

Corrective 07 and the HTTPX remaining-parity line are closed only when:

- [ ] Corrective 06 is complete with no known unclassified semantic defect.
- [ ] One final executable/test SHA is frozen.
- [ ] Focused Corrective 06 semantic tests pass on that exact SHA.
- [ ] `./scripts/check.sh` passes on that exact SHA.
- [ ] Existing extended verification passes on that exact SHA.
- [ ] The full pinned compatibility suite passes three consecutive clean runs on that exact SHA.
- [ ] High-risk SSLContext, trace, H2, and 101 behaviors have direct differential/wire evidence.
- [ ] API oracle reports zero unexplained, stale, or resolved-active differences.
- [ ] Required downstream portfolio passes against a wheel built from that SHA.
- [ ] Candidate wheel hashes are recorded.
- [ ] Existing remote CI is green and tied to the frozen executable tree/equivalent unchanged tree.
- [ ] `profile.toml` `qualification-sha` equals the frozen executable SHA exactly.
- [ ] The current status section reports the new qualification and does not conflate it with superseded evidence.
- [ ] Active/resolved/parity ledgers reflect the final implementation truthfully.
- [ ] No post-qualification descendant contains executable/test/build/validation changes.
- [ ] Retained differences are narrow, tested, and user-visible where they affect migration.
- [ ] No new CI/release machinery was introduced merely to close the parity program.

## Closure statement

Only after every item above passes should the repository again state that the documented HTTPX 0.28.1 surface is **Stage C qualified**.

At that point this remaining-parity line should be considered complete. Future HTTPX work should be triggered by a new pinned HTTPX version, a newly discovered concrete compatibility defect, or an intentionally expanded compatibility contract — not by further speculative parity expansion.