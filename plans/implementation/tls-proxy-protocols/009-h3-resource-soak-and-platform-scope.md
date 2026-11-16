# TLS, Proxy, and Protocols Milestone 009 — H3 Resource Soak and Platform Scope

Status: closed

Repository baseline: `f11a46b28df88f43e923d5211f82985784daaf40`

Source roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-9--h3-resource-soak-and-platform-scope`

Long-term requirements:

- `plans/000-long-term-specification.md` §3
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 item 2

Applicable ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`
- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`

Primary class: infrastructure

## 1. Objective

Extend the deterministic HTTP/3 resource evidence from short loopback
cycles to a bounded soak, and replace the prose platform claim with a
table that records only what was actually run.

The table is allowed to say that a platform was not executed. That is the
support limit. It is not a failure, and it is not a support claim.

## 2. Why this milestone is ready

The short controls already pass and stay in place: 100 reused requests, 10
fail/reconnect cycles, cancellation storms, construction/drop loops, 70
origins against the 64-entry cache, and 20-way multiplexing. The
architecture doc names long-duration descriptor and memory soak as future
work and forbids fragile exact-RSS assertions.

`crates/eggfetch-core/tests/resource_stabilization.rs` is the existing
pattern: warm up, repeat, and compare against a bounded envelope. On
non-Linux it skips the RSS number instead of inventing one. H3 soak should
follow that pattern inside the `http3` test target. It does not belong in
the H1 stabilization file.

No upstream fix and no external server is required to measure the local
client. If M008 later changes pins, these tests are part of the suite that
must be re-run; they do not freeze a support claim by themselves.

## 3. Current implementation evidence

`h3_hardening.rs` and `h3_interop_qualification.rs` contain the short
cycles listed above. `h3_many_distinct_origins_stay_bounded` covers the
cache cap. `TransportMetrics::h3_diagnostics` retains at most 64 copied
snapshots and no live connection handle.

The September evidence record says runtime qualification was Linux aarch64
only, and that macOS, Windows, and other architectures were not executed.
This tree has no platform table that a later reader can check.

## 4. Invariants that must not regress

- No exact RSS equality and no single-sample peak as a pass/fail line.
- The 64-entry H3 connection cache and the 64-entry diagnostic store stay
  bounded. Active references are not evicted out from under a live body.
- Dropping the client releases owned QUIC resources after in-flight
  request references are gone.
- Tests stay `--test-threads=1` safe and loopback-only.
- `ResponseBody` gains no variant. No new public metric is added to force
  the soak to be observable. Use existing `TransportMetrics` and process
  observations.
- The experimental label stays. A green soak is not graduation.

## 5. Scope

### In scope

- Longer deterministic H3 cycles for reuse, fail/reconnect, cancellation,
  partial body drop, and client construction/drop.
- Descriptor or socket observations where the OS exposes them, skipped
  with a recorded reason where it does not.
- A platform table in the architecture doc.

### Explicitly out of scope

- A multi-hour CI soak or a new benchmark product.
- Running the soak on every OS inside this milestone. Fill a row only for
  the host that actually ran it. Additional hosts are M010 operational
  evidence.
- Impairment, independent servers, and the upstream ledger.
- Changing cache eviction policy. If the soak shows unbounded growth,
  stop and report it. Do not retune the cache in this milestone unless the
  growth is a leak of a resource the existing tests already require to be
  released.

## 6. Required production changes

None expected. A leak fix is in scope only when the new test shows the
client retains per-origin state, tasks, or sockets after the existing
drop and eviction contracts say they should be gone. Any such fix keeps
the public surface unchanged and gets the new test as its regression.

## 7. Ordered work packages

1. Add soak tests to the existing `http3` interop or hardening target,
   still loopback Quinn/h3:
   - repeated sequential requests on one reused connection, well beyond
     the current 100, in a fixed count that finishes in a local test run
     (use 1 000 unless measurement shows that count is unreasonable, and
     record the count);
   - repeated fail/reconnect cycles beyond the current 10, asserting the
     cache does not grow by one live generation per failure;
   - a cancellation storm and a partial-body-drop loop, followed by one
     successful request;
   - a construction/drop loop of many clients, each of which completes
     one request and is dropped.
2. Sample resource use with the same restraint as
   `resource_stabilization.rs`. On Linux, read `VmRSS` from
   `/proc/self/status` and the count of `/proc/self/fd`. Assert an
   envelope against the post-warmup baseline: the ending sample is not an
   unbounded step above the baseline across the whole loop. Do not assert
   a byte equality. On other operating systems, skip the numeric RSS check
   and still assert the behavioral contracts (later request succeeds,
   origin count stays within the cache cap, diagnostics length stays
   within 64).
3. Assert `h3_diagnostics()` length stays at or below 64 and that entries
   contain no origin hostname, header, or close-reason text. The existing
   redaction tests remain the authority if they already cover this.
4. Add a platform table under "Resource and Platform Scope" in
   `docs/architecture/core-tls-proxy-protocols.md` with one row per
   execution: OS, architecture, build features, deterministic H3 suites
   run, soak run, IPv6 executed or skipped, impairment executed or not,
   and whether that row is support evidence. The row for the host that
   runs this milestone is filled. Other rows are absent, or present with
   "not executed". A package build is not a runtime row.
5. Leave `eggfetch-bench` `resource_monitor` on its current H1/H2 workload.
   Do not make the bench binary an H3 graduation gate.

## 8. Failure, cancellation, timeout, and pool semantics

The soak uses the same timeout and cancellation paths as `h3_hardening.rs`.
Fail/reconnect uses a refused or blackholed UDP target and must not poison
a different origin. Partial body drop must leave the connection usable for
a later request on that origin, matching
`h3_partial_response_drop_keeps_connection_usable`.

Pool permits are acquired and released as they are today. The soak does not
raise pool limits to hide a leak.

## 9. Compatibility and migration (incl. feature-profile effects)

Tests only, plus the architecture doc. The new tests are behind `http3`
and do not run in lean or native profiles. Changing Rust tests invalidates
the Stage C binding. Closure includes renewed HTTPX 0.28.1 and HTTPX2
2.12.0 qualification on the freeze SHA.

No default-feature change. No Python API change.

## 10. Required tests

- The new soak tests pass at `--test-threads=1` with `http3` enabled.
- Existing short H3 resource tests still pass.
- A host without `/proc` still passes the behavioral assertions and skips
  the numeric sample without a false pass of the Linux envelope.

## 11. Required verification commands

```bash
cargo test -p eggfetch-core --all-features --test h3_hardening --test h3_interop_qualification -- --test-threads=1
./scripts/check.sh
```

Tier 2, Tier 3, and both HTTPX profiles on the freeze SHA are required
before closure. Do not add the soak to a new CI job. Tier 1 already runs
the `http3` tests through the workspace test command when features are on;
keep that shape.

## 12. Documentation updates

- The platform table and a short description of the soak counts in
  `docs/architecture/core-tls-proxy-protocols.md`.
- The experimental graduation section stays. The soak is listed as local
  evidence, not as the missing independent-server or impairment evidence.

## 13. Acceptance criteria

- Soak counts and the envelope rule are written down and enforced by tests.
- Diagnostics and the connection cache remain capped at 64.
- The platform table has a filled row only for an execution that happened,
  and it does not call an unexecuted OS supported.
- Stage C is renewed.
- HTTP/3 remains experimental.

## 14. Stop conditions

- Stop if the envelope can be satisfied only by an exact RSS number or by
  disabling the test under load.
- Stop if the soak requires a public metrics API or a new `ResponseBody`
  variant.
- Stop if growth is unbounded and the cause is not a straightforward
  failure to honor the existing drop contract. Report the measurement
  instead of raising the cache cap.
- Do not mark macOS or Windows supported because the binary compiles there.

## 15. Closure evidence required

- Closure record under
  `plans/closure/tls-proxy-protocols/009-h3-resource-soak-and-platform-scope.md`.
- The host row that was filled, the soak counts, and the envelope result.
- The renewed Stage C SHA.
- A note that M010 still owns any additional platform rows and the
  graduation decision.

## 16. Handoff notes

Read `current_rss_bytes` in `resource_stabilization.rs` and the cache-bound
test `h3_many_distinct_origins_stay_bounded` before choosing the envelope.
Keep the soak in the `http3` test binary so a default non-H3 build does
not compile it.

Choose a request count that a developer can run locally. A multi-hour soak
is not required for this milestone; M010 may attach a longer operator run
later, and this plan does not promise one.
