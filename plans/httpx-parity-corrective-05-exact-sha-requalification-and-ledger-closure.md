# HTTPX Parity Corrective 05 — Exact-SHA Requalification and Ledger Closure

Baseline reviewed: `4571cb55bc2ff49822608d750dfef185cff40ebc`
Depends on: Correctives 01-04 complete and executable tree frozen.
Reference: `httpx==0.28.1` / `httpcore==1.0.9`

## Objective

Re-establish a truthful Stage C compatibility qualification after all corrective implementation work lands. This phase does not add compatibility features except for test/evidence fixes needed to verify already-implemented behavior. Its purpose is to freeze the final executable SHA, reconcile all compatibility records with actual behavior, run the existing qualification procedure against that exact SHA, and update the profile only from successful evidence.

The current repository cannot use the existing `48bad19fa1bb7ab7c91bcd67787efb2e41127fff` qualification for HEAD because `4571cb55...` subsequently changed executable Rust/Python compatibility code. That contradiction must be explicitly repaired rather than papered over.

## Precondition

Do not begin final qualification until Correctives 01-04 have landed and no further executable change is expected.

If an executable defect is discovered during this phase:

1. fix it;
2. obtain the new executable SHA;
3. discard qualification results from the prior SHA for closure purposes;
4. rerun the required qualification gates on the new SHA.

Documentation-only commits may follow the frozen executable SHA only if repository policy explicitly permits exact-SHA evidence binding to an executable ancestor. The closure record must identify which later commits are documentation-only and must not claim they were the executable under test.

## Track 1 — Reopen stale qualification before implementation continues

At the start of this corrective program, the compatibility profile/status should not misleadingly present current HEAD as covered by the old exact-SHA qualification.

Follow the repository's existing convention for a reopened/pending compatibility profile. Do not invent a new schema/status mechanism.

Required truth statement during implementation:

- previous qualified executable SHA: `48bad19fa1bb7ab7c91bcd67787efb2e41127fff`;
- executable changes after that SHA invalidated qualification for current HEAD;
- current corrective implementation is qualification-pending until this phase finishes.

Historical evidence remains valid as historical evidence for its original SHA; do not delete it.

## Track 2 — Freeze final executable SHA

After Correctives 01-04:

1. ensure working tree/branch contains all intended executable changes;
2. identify the final executable commit SHA (`FINAL_EXEC_SHA`);
3. record the exact diff boundary from `4571cb55...` through `FINAL_EXEC_SHA` for the closure note;
4. prohibit further executable changes while qualification is running;
5. if docs/tests considered executable under repository policy change, treat them according to that policy rather than improvising.

Tests that affect the qualification harness itself should be finalized before the SHA freeze where practical.

## Track 3 — Reconcile difference ledgers before running the oracle

Audit these files together:

- `compat/httpx/0.28.1/allowed-differences.toml`
- `compat/httpx/0.28.1/resolved-differences.toml`
- `compat/httpx/0.28.1/parity-cases.toml`
- `docs/residual-differences.md`
- `docs/reference/compatibility.md`
- `compat/httpx/0.28.1/README.md`
- `AGENTS.md`
- `.skills/python-bindings.md`
- `plans/httpx-parity-correction-status.md`
- `compat/httpx/0.28.1/profile.toml`

### Required classifications

#### SSLContext

Describe exactly:

- helper-created/representable context behavior that is supported;
- arbitrary external SSLContext behavior that is representable;
- state that fails closed because rustls cannot reproduce it;
- mTLS provenance limitations if any remain.

Do not call arbitrary SSLContext passthrough “resolved” if HTTPX accepts contexts EggFetch intentionally rejects.

#### H2-only

Each remaining difference must be separate enough to identify behavior:

- H2-only TLS ALPN enforcement;
- cleartext H2 prior knowledge;
- direct/local-address/socket-option H2 limitation;
- proxy/UDS limitations if relevant.

If Corrective 04 implements them, move corresponding differences to resolved with test evidence.

#### Extensions

Classify:

- `target`;
- `sni_hostname`;
- trace callback subset;
- `stream_id`.

Do not mark trace resolved based only on the existence of a Rust observer.

#### Network stream

Classify separately:

- 101 owned upgrade stream;
- ordinary pooled HTTP/1.1 response;
- ordinary HTTP/2 response;
- internal proxy CONNECT handling;
- `start_tls` limitations by stream type if any.

#### Proxy

Confirm:

- proxy headers are supported on proxy leg only;
- proxy authorization precedence is tested;
- proxy TLS trust is independent from origin TLS;
- unrepresentable proxy SSLContext behavior is classified consistently with origin SSLContext behavior.

### Ledger hygiene requirements

- No entry remains in active allowed differences if its behavior is now resolved.
- No resolved entry remains if tests show the difference still exists.
- Every active difference has one or more concrete tests.
- No contradictory duplicate records exist across allowed/resolved ledgers.
- Rationale text describes current code, not historical implementation state.
- `migration-impact` is realistic rather than automatically “None.”
- Security-sensitive bounded differences state the consequence clearly.

## Track 4 — Focused corrective test gate

Before the full suite, run focused local gates for the new work.

At minimum include:

### TLS/proxy trust

- SSLContext classifier/unit tests;
- helper mutation tests;
- custom CA network proof;
- mTLS provenance tests;
- proxy CA/origin CA separation matrix;
- proxy header redaction tests.

### Extensions/metadata

- target buffered/streaming sync/async;
- SNI buffered/streaming sync/async;
- trace success/failure callbacks;
- custom reason phrase;
- H2 streaming metadata.

### Network stream

- 101 leading bytes;
- sync/async read/write/close;
- runtime/client-close lifecycle;
- ordinary response absence;
- internal CONNECT non-exposure;
- start_tls supported/unsupported cases.

### H2-only

- TLS H2 server;
- TLS H1-only server;
- cleartext H2 prior-knowledge server;
- cleartext H1 server;
- specialized transport rows;
- stream_id presence/absence truth test.

There must be no network calls to public internet services in correctness tests.

## Track 5 — Existing Tier 1 gate

Run the repository's canonical existing Tier 1 command, currently documented as:

```sh
./scripts/check.sh
```

Use the existing serialized/routine validation apparatus. Do not add another CI job or matrix for this corrective work.

Record:

- exact SHA;
- command;
- pass/fail;
- relevant collected/passed counts;
- environment details required by existing repository convention.

If Tier 1 fails, fix and restart qualification at the new executable SHA.

## Track 6 — Full pinned compatibility qualification

Run the existing required compatibility command with the repository's required-mode environment, e.g. the currently documented form:

```sh
EGGFETCH_COMPAT_REQUIRED=1 \
python -m pytest crates/eggfetch-python/tests/compat/ -q --strict-markers
```

Requirements:

- pinned `httpx==0.28.1`;
- pinned/expected `httpcore==1.0.9`;
- no capability skip that hides a supported required behavior;
- no xfail used to turn a required unresolved defect into green evidence;
- local fixtures for protocol/TLS/proxy tests;
- environment versions recorded.

Follow the repository's existing repeated-run policy. If current Stage C convention requires multiple consecutive clean full-suite runs, perform the same number; do not weaken the standard for this closure.

## Track 7 — API oracle and manifest validation

Run the existing API comparison/oracle tooling against the frozen candidate.

Acceptance:

- zero unexplained differences;
- zero stale allowed differences;
- zero resolved entries still active;
- zero invalid manifest/ledger entries;
- every intentional difference maps to a reviewed active ledger record.

Review oracle results manually for semantic category mistakes. A mechanically “allowed” entry with stale rationale is not closure.

## Track 8 — Required downstream qualification

Run the existing isolated downstream compatibility runner using a wheel built from `FINAL_EXEC_SHA`.

Required packages currently include the repository's established portfolio:

- `respx`
- `httpx-sse`
- `httpx-auth`
- `httpx-ws`

Record:

- exact candidate wheel SHA-256;
- source executable SHA;
- package versions if existing runner reports them;
- test counts/results;
- any diagnostic dependency warnings separately from behavioral failures.

If downstream behavior fails because of a newly corrected metadata/upgrade/TLS path, treat it as an executable defect and restart qualification after the fix.

## Track 9 — Documentation truth pass

After all executable evidence is clean, make a final consistency pass.

### `profile.toml`

Set:

- `qualified` to actual qualification date;
- `qualification-sha` to `FINAL_EXEC_SHA`;
- Stage/status only if all required gates passed.

Do not set qualification SHA to the final documentation commit if the executable tested was an earlier ancestor unless repository convention explicitly defines it that way.

### Correction status

Rewrite the top/current section of `plans/httpx-parity-correction-status.md` so it does not contain contradictory current claims.

Historical sections may retain old facts, but they must be clearly labeled historical/superseded. Remove stale current-language statements such as proxy headers being rejected after they are supported.

The current section must list all retained bounded differences, including H2/network-stream/SSLContext residuals that Correctives 01-04 intentionally keep.

### Compatibility docs

Ensure the user-facing claim is scoped to:

- pinned HTTPX 0.28.1;
- supported Python versions/backends;
- Stage C supported surface;
- explicit residuals.

Avoid phrases like “full parity” or “complete prior-knowledge mode” where active differences remain.

### AGENTS/skills

Update implementation guidance to the final architectural truth:

- typed extensions;
- safe SSLContext boundary;
- proxy/origin TLS separation;
- network-stream ownership limitations;
- H2 residuals.

Do not turn status history into long agent instructions; keep operational invariants concise.

## Track 10 — Remote CI evidence

Use the existing routine CI only if the repository's current qualification policy requires/records remote CI. Do not add workflows.

If the final executable SHA itself has a CI run, record it.

If a documentation-only descendant triggers CI, record that separately and do not imply it validated a different executable SHA unless the workflow checked out/tested that exact executable state.

## Final closure report structure

The closure record should contain:

1. final executable SHA;
2. qualification date/environment;
3. corrective scope summary;
4. exact focused test results;
5. Tier 1 result;
6. full compatibility repeated-run results;
7. API oracle result;
8. downstream portfolio result + wheel hash;
9. remote CI result if applicable;
10. list of resolved differences;
11. list of retained bounded differences;
12. confirmation that no executable commits follow the recorded qualified SHA at the time of closure, or explicit documentation-only descendants with hashes;
13. statement that future executable changes require fresh qualification.

## Acceptance criteria

Corrective 05 and the entire post-Phase-06 corrective program are complete only when:

- [ ] Existing stale qualification has been truthfully reopened/superseded for current HEAD.
- [ ] Correctives 01-04 are complete before qualification begins.
- [ ] A single `FINAL_EXEC_SHA` is frozen.
- [ ] No executable changes occur after qualification begins without restarting qualification.
- [ ] Active/resolved ledgers match actual current behavior.
- [ ] SSLContext residuals are classified accurately.
- [ ] H2-only residuals are classified accurately.
- [ ] trace/target/SNI/stream_id classifications match differential evidence.
- [ ] network_stream classifications distinguish 101, ordinary pooled responses, and internal CONNECT.
- [ ] proxy TLS/header claims match corrected implementation.
- [ ] Focused corrective suites pass on `FINAL_EXEC_SHA`.
- [ ] `./scripts/check.sh` passes on `FINAL_EXEC_SHA`.
- [ ] Full pinned compatibility suite satisfies the repository's repeated clean-run policy with no unapproved skips/xfails.
- [ ] API oracle reports zero unexplained, stale, and resolved-active differences.
- [ ] Required downstream packages all pass against a wheel from `FINAL_EXEC_SHA`.
- [ ] Candidate wheel hash is recorded.
- [ ] `profile.toml` records exactly `FINAL_EXEC_SHA` as qualification SHA.
- [ ] Current correction/status documentation contains no stale contradictory claims.
- [ ] User-facing compatibility docs list retained residuals explicitly.
- [ ] No new CI matrix/workflow/release apparatus was introduced.
- [ ] Final repository status can truthfully be described as Stage C qualified for its documented HTTPX 0.28.1 surface.

## Out of scope

Do not use this phase to chase unreleased HTTPX master changes, add FunctionAuth ahead of the next stable rebaseline, expand Python-version support, or introduce new release automation. Any executable feature discovered after the freeze belongs in a later plan and requires its own qualification cycle.