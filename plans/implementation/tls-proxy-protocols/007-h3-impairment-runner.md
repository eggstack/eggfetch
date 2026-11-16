# TLS, Proxy, and Protocols Milestone 007 — H3 Impairment Runner

Status: conditionally closed

Repository baseline: `f11a46b28df88f43e923d5211f82985784daaf40`

Source roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-7--h3-impairment-runner`

Long-term requirements:

- `plans/000-long-term-specification.md` §3
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 item 2

Applicable ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`
- `plans/adrs/ADR-0004-proxy-fallback-total-deadline.md`

Primary class: infrastructure

## 1. Objective

Provide the missing qualification-only runner that
`scripts/h3_impairment.py` already knows how to invoke, so each scenario in
`qualification/http3/impairment-matrix.json` gets a real pass, fail, or
unsupported result.

The runner proves client invariants under adverse UDP conditions. It does
not benchmark congestion control, and a run that cannot obtain a network
namespace does not count as impairment evidence.

## 2. Why this milestone is ready

`scripts/h3_impairment.py` is a fail-closed coordinator. It executes one
operator-supplied program per scenario and accepts only JSON with status
`pass`, `fail`, or `unsupported` plus every invariant named on that row.
With no runner, all 14 scenarios were recorded `unsupported` on the
September 2026 freeze. That result stays historical.

Local loopback tests already cover a silent connect failure, server
restart, early close, and cancellation against an in-process Quinn server.
They do not apply packet loss, reordering, jitter, duplication, or MTU
impairment. The matrix file already lists the required invariants. No
production API change is needed to start.

This milestone is independent of M006's manifest work. It must be re-run
after any M008 dependency bump before M010 cites it.

## 3. Current implementation evidence

At the baseline:

- the matrix has 14 required scenarios, from `udp_blackhole` through
  `ipv4_failure_ipv6_success`;
- the coordinator's `--runner` argument receives `SCENARIO_ID` and an
  output path, and reads `EGGFETCH_H3_IMPAIRMENT_SCENARIO` and
  `EGGFETCH_H3_IMPAIRMENT_OUTPUT`;
- a pass whose invariant map contains a non-pass is rewritten to fail;
- Tier 1 does not invoke the coordinator;
- the architecture doc already says impairment execution is
  qualification-only and needs a namespace/netem runner.

## 4. Invariants that must not regress

- Routine CI and `./scripts/check.sh` stay network-independent and gain no
  new job, matrix, or workflow.
- `Http3Only` never falls back to H1/H2.
- `Auto { allow_http3: true }` falls back only before commit and only for a
  replayable request.
- A non-replayable body is never sent twice.
- Connect, read, write, and total deadlines stay bounded. Total keeps
  winning ties. The runner must not cache a shrinking total deadline inside
  a connector.
- Pool permits, response bodies, and H3 driver tasks are released after
  bounded cleanup.
- The experimental label is unchanged.
- Impairment setup stays outside `eggfetch-core`. The core does not learn
  netem, namespaces, or packet filters.

## 5. Scope

### In scope

- One Linux runner script that the existing coordinator can call.
- One `http3`-gated Rust test target that performs the client assertions
  for a single scenario selected by the environment.
- A dependency-free contract self-test that checks the JSON shape without
  root and without a network namespace.
- Honest `unsupported` results when the host lacks `unshare`/`ip netns`,
  `tc`, IPv6, or the privileges needed to impair a namespace.

### Explicitly out of scope

- Adding the runner to Tier 1 or to GitHub Actions.
- Independent non-Quinn servers. The impaired peer may be the existing
  loopback Quinn/h3 fixture. Independent-stack evidence stays M006's
  harness plus M010's execution.
- Public-origin checks.
- Claiming a congestion-control algorithm or a throughput number.
- Changing production timeout, fallback, or pooling code unless a scenario
  exposes a defect. A defect stops the milestone; see §14.

## 6. Required production changes

None expected. The runner and the scenario tests are qualification code.
Do not add a production feature flag for impairment.

## 7. Ordered work packages

1. Add `crates/eggfetch-core/tests/h3_impairment_scenarios.rs`, gated on
   `http3`. Each scenario is its own test, named exactly as the matrix id.
   The test is skipped with a printed `unsupported` reason when
   `EGGFETCH_H3_IMPAIRMENT_SCENARIO` is unset or names a different id, so a
   normal `cargo test` of the file does not require netem and does not
   pretend to have passed the matrix. When selected, the test runs the
   client assertions below and writes the coordinator JSON to
   `EGGFETCH_H3_IMPAIRMENT_OUTPUT` if that variable is set.
2. Add `scripts/h3_impairment_netem.py`. For each scenario it creates a
   Linux network namespace (or uses `unshare`), places the loopback Quinn
   fixture and the eggfetch test in that namespace, applies the impairment,
   runs the one matching cargo test with `--test-threads=1`, and writes the
   JSON artifact. If namespace setup is unavailable, write
   `status: unsupported` and a `reason`. Do not exit 0 from the coordinator
   in that case: the existing coordinator already returns blocked unless
   every selected scenario is pass.
3. Implement the matrix as client invariants, using these impairments:
   - `udp_blackhole`: drop UDP to the server port. `Http3Only` ends within
     the configured connect or total deadline and does not fall back.
     Resources release.
   - `udp_rejected`: reject the UDP port. `Auto` on a replayable GET falls
     back only before commit when an H1 path exists in the fixture; a
     non-replayable body is not retried.
   - `packet_loss_low` and `packet_loss_high`: `tc netem loss` at two fixed
     nonzero rates chosen in the script and recorded in the JSON. Assert
     deadline, no body duplication, and prompt cancellation. Do not assert
     a throughput.
   - `latency_jitter`: delay plus jitter. A request with a total deadline
     shorter than the induced delay terminates. A later request with a
     usable deadline succeeds. The Alt-Svc cache is not permanently poisoned.
   - `packet_reordering` and `packet_duplication`: body bytes match the
     fixture exactly and the request body is sent once.
   - `reduced_mtu_pmtu_blackhole`: clamp MTU or drop packets above a small
     size. The attempt stays inside its deadline. `Auto` fallback, if it
     happens, is pre-commit only.
   - `path_loss_after_headers` and `path_loss_during_upload`: impair the
     path after the transfer has started. The read or write ends by
     cancellation or error inside the deadline. The upload body is not
     retried.
   - `server_restart`: stop the fixture, start it again on the same port,
     and show the next request succeeds on fresh H3 state.
   - `first_address_unreachable`: the fixture advertises one unreachable
     address and one reachable address. Both attempts share one connect
     budget, and the reachable address succeeds.
   - `ipv6_failure_ipv4_success` and `ipv4_failure_ipv6_success`: report
     `unsupported` when the host has only one family. When both exist, the
     failed family does not prevent the working family from returning an
     intact body.
4. Keep certificate material inside the existing test fixture. The runner
   must not disable certificate verification in production code.
5. Add a unittest module beside `scripts/test_h3_qualification.py` that
   loads the runner's result-writing helpers without root. Cover: missing
   privileges become `unsupported` with every required invariant key
   present; a pass with a failed invariant is rejected; an unknown scenario
   id is rejected. Wire that self-test into the Tier 1 runner self-test
   path only if it stays socket-free. Do not register the netem execution
   in `./scripts/check.sh`.
6. Document the privilege requirement and the two example rates next to
   the matrix description in
   `docs/architecture/core-tls-proxy-protocols.md`. State that an
   `unsupported` matrix is not graduation evidence.

## 8. Failure, cancellation, timeout, and pool semantics

Every scenario that names a deadline invariant uses a `Timeout` small
enough to fail the test if the impairment hangs, and large enough that a
healthy loopback request still passes. Cancellation uses the client's
existing drop/abort path.

Fallback assertions use a replayable GET for the positive fallback case and
a one-shot body for the negative case. The one-shot body must be observed
once at the fixture.

`server_restart` and the address-family scenarios must leave the origin
usable afterwards. A suppressed route, if suppression fires, must recover
when the path returns, using the existing generation rules. Do not invent a
new suppression class.

## 9. Compatibility and migration (incl. feature-profile effects)

No profile gains `http3`. The new test target is `http3`-gated and is not
part of the lean or native profiles.

Adding a Rust test target and a Tier 1 self-test changes executable inputs.
Closure requires a renewed exact-SHA Stage C binding. A docs-only note of
an `unsupported` macOS run does not by itself satisfy that requirement
once the test target exists.

The runner is Linux-specific. macOS and Windows runs of the script record
`unsupported`. They are not failures of the client.

## 10. Required tests

- The new scenario file passes under plain `cargo test` by skipping every
  scenario that is not selected, without claiming matrix evidence.
- The contract self-test passes with no privileges.
- On a Linux host that can create a namespace, one coordinator run with
  `--runner scripts/h3_impairment_netem.py` writes a result for all 14 ids.
  Scenarios the host cannot execute are `unsupported`, not pass.
- Existing `h3_hardening` blackhole, restart, and cancellation tests still
  pass and remain the Tier 1 controls.

## 11. Required verification commands

```bash
python scripts/test_h3_qualification.py
cargo test -p eggfetch-core --all-features --test h3_impairment_scenarios -- --test-threads=1
cargo test -p eggfetch-core --all-features --test h3_hardening -- --test-threads=1
./scripts/check.sh
```

Privileged execution, only where the host allows it, and not part of Tier 1:

```bash
python scripts/h3_impairment.py --runner scripts/h3_impairment_netem.py --output target/h3-impairment.json
```

Tier 2 and Tier 3 plus both HTTPX profiles are required before closure
because a new Rust test target is an executable input. Do not add CI.

## 12. Documentation updates

- Architecture doc: how to invoke the runner, the privilege requirement,
  and the rule that `unsupported` is not a pass.
- Matrix `description` may gain one sentence pointing at the runner script.
- Do not rewrite `plans/http3-independent-interop-and-impairment-qualification-evidence.json`.

## 13. Acceptance criteria

- `scripts/h3_impairment.py --runner scripts/h3_impairment_netem.py` is a
  sufficient command. No second coordinator is introduced.
- Every matrix id is implemented or explicitly `unsupported` with a reason
  the contract test can distinguish from a pass.
- Invariants in the JSON are exactly the matrix invariants, each `pass`,
  `fail`, or a status that forces the scenario off `pass`.
- Tier 1 stays green without root and without UDP impairment.
- HTTP/3 remains experimental. The closure says whether a privileged Linux
  run happened. If it did not, M010's impairment row stays open.

## 14. Stop conditions

- Stop if the scenario can only be expressed by teaching `eggfetch-core`
  about netem or by adding a public test hook on `Client`.
- Stop if a scenario fails because of a product defect in deadlines,
  fallback, or replay. Record the scenario id and the observed behavior.
  Do not loosen the invariant to obtain a pass.
- Stop if the runner needs a new published crate or a new CI service.
- Do not vendor a packet simulator when `tc netem` can express the
  scenario. A scenario `tc` cannot express on that host is `unsupported`.

## 15. Closure evidence required

- Closure record under
  `plans/closure/tls-proxy-protocols/007-h3-impairment-runner.md`.
- The contract-test result.
- The coordinator JSON if a privileged run was possible; otherwise an
  explicit "not executed" entry naming the host OS and the missing
  capability.
- The renewed Stage C SHA.
- A statement that this Quinn-peer impairment run is not independent-server
  interop evidence.

## 16. Handoff notes

The development host for the planning baseline is macOS. Implement the
runner and the unprivileged tests there. Do not block the code on a Linux
namespace you cannot create. Do not describe a macOS `unsupported` result
as a completed impairment matrix.

Read `scripts/h3_impairment.py` `execute` before choosing field names. The
artifact must already match that parser: `status`, `scenario`, and an
`invariants` object whose keys include every name on the matrix row.
