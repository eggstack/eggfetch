# Core Transport Policy M006C1 — Windows qualification and downstream closure corrective

Status: closing (native-Windows EggFetch matrix green — run 36193582776, 19/19; corrected EggReplay M013F Windows proof pending)

Repository baseline: `b001790b44ce1c455e3b2cc97d30e9e14217060c`

Corrects:

- `plans/implementation/core-transport-policy/006-windows-tls-response-completeness-corrective.md`
- `plans/closure/core-transport-policy/006-windows-tls-response-completeness.md`

Source roadmaps:

- `plans/subsystems/core-transport-policy-roadmap.md`
- `plans/subsystems/release-verification-roadmap.md`
- `plans/002-long-term-roadmap.md` Phase 3

Applicable policy:

- `plans/003-planning-process.md`
- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`
- `docs/verification-policy.md`

Primary class: invariant / qualification corrective

## 1. Objective

Close the remaining evidence gap from M006 before EggFetch publication work
resumes.

M006 produced a strong fixture-teardown diagnosis and a comprehensive hermetic
response-completeness matrix, but its closure advanced beyond two acceptance
conditions from the source plan:

1. the new matrix was not shown running on native Windows;
2. the corrected downstream EggReplay M013F large-body Windows reproduction
   had not yet passed at the original envelope.

This corrective does **not** reopen EggFetch transport implementation. It owns
only the missing platform/downstream evidence and the release-gate disposition
that depends on that evidence.

The historical M006 closure remains immutable evidence of the investigation.
M006C1 becomes the current release-gating authority.

## 2. Why this corrective is required

The M006 plan explicitly required:

- native Windows x86_64 as the mandatory defect-closure platform;
- Linux/macOS as regression controls;
- downstream EggReplay confirmation after correcting the fixture;
- the original large-body envelope rather than a reduced threshold.

The current M006 closure records:

- Tier 1/2/3 and security checks on the EggFetch freeze;
- 19 hermetic tests green;
- a fixture-teardown verdict;
- Stage C rebound to
  `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`;
- an unresolved downstream action requiring EggReplay's local TLS origin helper
  to perform async shutdown + brief linger.

EggFetch's automatic CI runs only on `ubuntu-latest`. Therefore the green
head run does not establish the mandatory Windows result.

The closure also states that EggReplay will continue to fail on Windows until
its origin helper is corrected. No subsequent green EggReplay Windows M013F
run is currently cited.

Those are evidence gaps, not evidence that the fixture-teardown diagnosis is
wrong.

## 3. Current technical baseline

Preserve the M006 production verdict unless new evidence contradicts it:

- no EggFetch production transport change;
- no dependency change;
- no public API change;
- no feature-default change;
- Stage C remains bound to
  `5247ff0e01e309d9b408b8b4b4c90151ee5ce9fa`;
- `crates/eggfetch-core/tests/tls_response_completeness.rs` remains the
  canonical local isolation matrix.

Do not re-open the root-cause investigation merely to collect evidence.

## 4. Release gate

Until M006C1 closes:

- release-verification M001 publication is **blocked**;
- release-verification M002 Python 3.15 wheel rehearsal is **blocked**;
- do not publish/tag/PyPI from the prior candidate;
- do not claim the Windows response-completeness issue fully closed.

This is an evidence gate, not an executable-input invalidation. Stage C does
not need to move unless EggFetch executable/test/qualification inputs change.

## 5. Required work package A — native Windows EggFetch proof

Run the M006 hermetic matrix on native Windows x86_64 against the exact
implementation freeze or a docs-only descendant.

Required environment record:

```text
git rev-parse HEAD
rustc --version
cargo --version
rustup show active-toolchain
Windows version / runner identity
```

Required focused command:

```powershell
cargo test -p eggfetch-core --all-features --test tls_response_completeness -- --test-threads=1 --nocapture
```

Record all test names and pass/fail counts.

Mandatory Windows assertions include at minimum:

- 64 KiB graceful fixed-length completes;
- near/exact/above 128 KiB graceful fixed-length completes;
- 256 KiB and 300 KiB graceful fixed-length complete;
- graceful `close_notify` completes;
- deliberately truncated fixed-length response errors;
- keep-alive complete response succeeds;
- chunked graceful response succeeds;
- close-delimited graceful response succeeds;
- buffered/text/stream consumers agree;
- direct/custom dialers agree;
- raw tokio-rustls probe succeeds;
- minimal Hyper probe succeeds;
- read timeout still fires;
- pool reuse after success works;
- no reuse after truncation error.

If any graceful Windows case fails, stop. M006's fixture-teardown verdict is
not yet sufficient and the root-cause investigation must reopen before
publication.

## 6. Required work package B — Windows abrupt-close characterization

The abrupt-close tests are diagnostic, not a requirement that Windows match
Linux transport buffering.

On Windows, record whether:

- a complete body followed by bare TLS/TCP drop is fully delivered;
- rustls/Hyper reports a body error because queued bytes were lost;
- raw tokio-rustls and minimal Hyper agree with EggFetch.

The acceptance condition is consistency with the documented teardown
classification, not cross-platform byte-for-byte equivalence for a malformed
origin teardown.

A genuine short-body success remains forbidden.

## 7. Required work package C — downstream EggReplay fixture correction

Coordinate with EggReplay M013F so its local TLS origin helper performs a
well-defined graceful shutdown after writing the complete response:

1. write response head/body;
2. `flush().await`;
3. `TlsStream::shutdown().await` so rustls emits `close_notify`;
4. brief bounded linger sufficient for the client to drain before handler
   teardown.

The exact linger may follow the M006 fixture baseline; do not encode it into
EggFetch production code.

EggFetch does not own the downstream code change, but M006C1 owns recording
its resulting evidence before release gates reopen.

## 8. Required work package D — EggReplay hosted Windows confirmation

Rerun EggReplay M013F on hosted Windows using the corrected origin helper.

Do **not** reduce the original qualification envelope.

Required downstream proof:

- direct proxy-less EggFetch request receives the complete 300 KiB TLS body;
- Eggress-direct dialer receives the same complete body;
- MITM path returns the complete body to the client;
- durable EggReplay flow records the complete body length;
- no upstream response-body error event is recorded;
- Windows Rust verification lane is green;
- Windows interception lane is green.

Retain the exact EggReplay commit SHA and Actions run/job URLs in the closure
record.

If the corrected downstream origin still reproduces truncation, M006C1 must
remain open and M006 root-cause work reopens. Do not paper over it by lowering
the body size.

## 9. Required work package E — control evidence

Confirm the current EggFetch freeze remains green on the normal supported
control environment:

```bash
./scripts/check.sh
```

Tier 2/3 do not need to be repeated merely because external evidence was added
and no executable/qualification input changed.

If any EggFetch executable, dependency, fixture, validation script, profile, or
workflow changes during M006C1, the ordinary invalidation rules apply:

- rerun the appropriate Tier 1/2/3 gates;
- renew Stage C if qualification inputs/executable code changed;
- run the public API/semver oracle as required.

## 10. CI policy

Do not add a permanent Windows matrix to routine CI solely for this
corrective.

Accepted native-Windows evidence:

- maintainer-run Windows host with captured command/toolchain output;
- an already-approved manual workflow surface if one can execute the focused
  test without expanding automatic CI;
- another reproducible native Windows executor whose SHA/toolchain/output are
  retained.

EggReplay's hosted Windows result is required downstream confirmation but does
not replace the direct EggFetch Windows proof.

## 11. Documentation and status updates

After both Windows proofs pass:

- create
  `plans/closure/core-transport-policy/006c1-windows-qualification-and-downstream-closure.md`;
- mark M006C1 closed in the core-transport roadmap;
- mark M001 and M002 ready again;
- update the registry release gate;
- add a current-authority note referencing the EggReplay confirmation;
- leave the historical M006 closure unchanged.

Do not add another Stage C entry if no executable/qualification input changed.
The existing binding remains authoritative.

## 12. Acceptance criteria

- [ ] M006 hermetic matrix runs on native Windows x86_64.
- [ ] all graceful fixed-length cases through 300 KiB pass on Windows.
- [ ] raw tokio-rustls Windows probe passes with graceful shutdown.
- [ ] minimal Hyper Windows probe passes with graceful shutdown.
- [ ] EggFetch direct and custom-dialer Windows paths pass.
- [ ] genuine truncated response still errors.
- [ ] no silent short-body success occurs.
- [ ] abrupt-close behavior is recorded and consistent with the documented
      fixture-teardown contract.
- [ ] EggReplay local TLS origin performs graceful shutdown + bounded linger.
- [ ] EggReplay proxy-less direct 300 KiB Windows test passes.
- [ ] EggReplay Eggress-direct 300 KiB Windows test passes.
- [ ] EggReplay MITM 300 KiB Windows test passes with full durable body.
- [ ] EggReplay Windows Rust/interception qualification lanes are green.
- [ ] exact EggReplay SHA/run/job evidence is retained.
- [ ] normal EggFetch Tier 1 remains green.
- [ ] release M001/M002 stay blocked until every item above is satisfied.

## 13. Stop conditions

Stop and reopen the M006 technical investigation if:

- graceful Windows EggFetch matrix fails;
- raw rustls succeeds but minimal Hyper fails;
- minimal Hyper succeeds but EggFetch fails;
- corrected EggReplay graceful origin still truncates;
- a response with fewer than declared bytes is ever reported as success.

Do not reopen transport implementation merely because abrupt-close Windows
behavior differs from Linux when all graceful controls pass.

## 14. Closure evidence required

Create:

`plans/closure/core-transport-policy/006c1-windows-qualification-and-downstream-closure.md`

Record:

- EggFetch exact SHA;
- Windows OS/toolchain;
- complete focused-test output/count;
- graceful/abrupt Windows matrix result;
- raw-rustls/minimal-Hyper result;
- EggReplay corrected fixture commit;
- EggReplay Actions run/job links;
- 300 KiB direct/Eggress/MITM body-length evidence;
- Tier 1 control result;
- whether Stage C remained unchanged or was renewed;
- final M001/M002 disposition.

M006C1 may close only when both the direct EggFetch Windows proof and corrected
EggReplay Windows proof are present.

## 15. Handoff

The expected outcome is evidence-only closure with no EggFetch production
change.

If the expected fixture-teardown diagnosis survives native-Windows and
downstream confirmation, this corrective should be small: collect evidence,
record closure, and reopen release gates.

If either proof contradicts the diagnosis, publication remains blocked and the
technical investigation reopens with the new layer-isolation evidence.
