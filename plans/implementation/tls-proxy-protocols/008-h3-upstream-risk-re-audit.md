# TLS, Proxy, and Protocols Milestone 008 — H3 Upstream Risk Re-Audit

Status: closed

Repository baseline: `f11a46b28df88f43e923d5211f82985784daaf40`

Source roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-8--h3-upstream-risk-re-audit`

Long-term requirements:

- `plans/000-long-term-specification.md` §3
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 item 2

Applicable ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`

Primary class: invariant

## 1. Objective

Re-audit the pinned Quinn/h3 stack against the upstream defects that block
ordinary HTTP/3 support, and upgrade only when a published release closes a
defect that eggfetch can reach.

The outcome is a current risk ledger. A decision to stay on `h3` 0.0.8 is a
successful close of this milestone when no published release fixes the
blocking defect. That decision keeps HTTP/3 experimental.

## 2. Why this milestone is ready

The September 2026 ledger is stale relative to this tree and relative to
upstream:

- `Cargo.toml` still pins `h3` 0.0.8 with
  `i-implement-a-third-party-backend-and-opt-into-breaking-changes`,
  `h3-quinn` 0.0.10, and `quinn` 0.11.
- `Cargo.lock` at the baseline resolves `quinn` 0.11.11 and `quinn-proto`
  0.11.18. The September ledger recorded `quinn-proto` 0.11.16.
- Crates.io, as checked on 2026-11-16, still has no `h3` newer than 0.0.8
  and no `h3-quinn` newer than 0.0.10. Quinn 0.11.12 exists and does not
  fix the h3 frame defect.
- [h3 #338](https://github.com/hyperium/h3/issues/338) is open.
  [h3 #339](https://github.com/hyperium/h3/pull/339) is open. The defect
  drops buffered frame bytes when a connection close arrives in the same
  receive batch, so a complete response can surface as a connection error.
  The response-header path cannot be repaired in eggfetch, because only h3
  holds those bytes.
- [h3 #361](https://github.com/hyperium/h3/issues/361) is open. On the
  released `h3-quinn` 0.0.10, `stop_sending` during an outstanding read does
  not put the requested HTTP/3 code on the wire. The #331 panic fix is not
  in the released crate.

The audit can be done now. It must finish before M010 collects evidence,
because a later lockfile change invalidates that evidence. M006 and M007
may land beside this milestone; their external runs, if any, are not
graduation evidence until they are repeated on the post-audit pins.

## 3. Current implementation evidence

Draining uses `h3` 0.0.8 `ConnectionState::is_closing()` and
`is_h3_no_error()`, documented in `transport/http3.rs`. Local early-close
tests distinguish EOF from error and do not reproduce #338. Cancellation
tests show the client stays usable; they do not observe the STOP_SENDING
code on the wire.

The previous dispositions live in
`plans/http3-upstream-risk-resource-and-observability-hardening.md` and
`plans/http3-production-qualification-evidence.md`. Those files are
historical. This milestone writes a new ledger.

## 4. Invariants that must not regress

- No private fork, vendored patch, or `[patch]` of `h3`, `h3-quinn`, or
  `quinn` is introduced to force graduation.
- No second QUIC stack is added.
- The unstable h3 feature remains an explicit, documented integration
  contract. If a release removes `is_closing()` / `is_h3_no_error()`, the
  drain path is ported to the replacement API and retested. It is not
  stubbed.
- 0-RTT stays disabled. Datagram routing, WebTransport, and MASQUE stay
  out of scope.
- A dependency bump does not enable H3 by default and does not add it to
  the CLI or FFI default features.
- Certificate verification failure still never retries with another trust
  store.

## 5. Scope

### In scope

- A fresh ledger of exact resolved versions and of every open upstream
  issue that can change ordinary GET, POST, streaming, cancellation, or
  graceful close.
- An upgrade of `h3`, `h3-quinn`, and the Quinn crates they require, only
  when the published crates contain the #338 fix or an equivalent frame
  drain.
- Regression tests that lock the drain API actually used after the audit.
- A written no-upgrade decision when those crates do not exist yet.

### Explicitly out of scope

- Carrying a downstream patch for #339 inside this repository.
- Treating a Quinn-only bump as a fix for #338.
- Closing #361 by matching error strings.
- Independent-server, impairment, or public-origin execution.
- Removing the experimental label. Even a successful upgrade leaves that
  to M010.

## 6. Required production changes

If, at execution time, crates.io still has no release that fixes #338:

- change no dependency and no production code;
- record #338 as a graduation blocker and #361 as an open cancellation
  wire-code limitation;
- re-check the Quinn advisory GHSA-6xvm-j4wr-6v98, or its successor,
  against the resolved `quinn-proto`.

If a published release does fix #338:

- bump the manifests and lockfile together;
- port GOAWAY detection to the published API if the unstable methods moved;
- keep the feature gate and the experimental docs;
- add the regression in §10.

## 7. Ordered work packages

1. Resolve the current graph (`h3`, `h3-quinn`, `quinn`, `quinn-proto`,
   `quinn-udp`, `rustls`, `tokio-rustls`, `tokio`) and record versions and
   the h3 feature flags.
2. Re-read #338, #339, #361, and the reset-observation issues cited by the
   September ledger (#352, #353), plus any newer h3 or Quinn issue that
   affects ordinary client responses, cancellation, GOAWAY, or QPACK.
   Confirm open/closed state on the day of the audit. Do not copy the
   2026-11-16 status into the ledger without looking.
3. For each issue, write one row: reference, affected versions, whether
   eggfetch's recv, cancel, or drain path can reach it, impact on a
   successful response, existing test, and one disposition:
   `upgrade`, `local safe workaround`, `proven not reachable`, or
   `graduation blocker`.
   `local safe workaround` is allowed only when the workaround lives in
   eggfetch code, has a regression test, and does not vendor h3.
   #338 does not qualify for that disposition on the response-header path.
4. Apply §6. Run the H3 suites after any bump. Confirm 0-RTT is still off
   and that `Http3Only` still does not fall back.
5. Write `qualification/http3/upstream-risk-ledger.json` with
   `schema_version` 1, the git SHA, the resolved versions, and the rows.
   Update the dependency table in
   `docs/architecture/core-tls-proxy-protocols.md` so it matches the ledger.
   Add a new dated paragraph. Do not rewrite the 2026-09-11 decision
   paragraph.
6. If the unstable API is still in use, keep the existing re-audit comment
   in `transport/http3.rs` accurate for the pinned version.

## 8. Failure, cancellation, timeout, and pool semantics

An upgrade must preserve phase-correct connect, read, write, and total
timeouts, pool-permit release, and the single pre-commit replayable
fallback. Cancellation must still leave the client usable. If #361 is
unfixed, do not claim a conformant STOP_SENDING code during a pending
read. The ledger says so, and the cancellation tests keep asserting client
usability rather than the wire code.

## 9. Compatibility and migration (incl. feature-profile effects)

A docs-and-ledger-only no-upgrade close does not change executable inputs
and does not require a new Stage C freeze.

A manifest or lockfile change does. Closure of an upgrade requires Tier 1,
Tier 2, Tier 3, and renewed HTTPX 0.28.1 and HTTPX2 2.12.0 qualification
on that exact SHA. M010 must cite that SHA, not an earlier one.

No Python, CLI, or FFI signature changes.

## 10. Required tests

On the no-upgrade path, run the existing H3 suites once to show the audited
pins still pass. Do not add a test that claims to reproduce #338 unless it
fails reliably on the pinned crate. A flaky reproduction is not a gate.

On the upgrade path:

- the existing `h3_hardening`, `h3_alt_svc_discovery`,
  `h3_interop_qualification`, and `h3_datagram_route` suites pass;
- a focused drain test still proves a graceful close refuses new work on
  that generation and the next request reconnects;
- a comment or test name identifies the upstream issue the upgrade closed.

## 11. Required verification commands

No-upgrade path:

```bash
cargo test -p eggfetch-core --all-features --test h3_hardening --test h3_alt_svc_discovery --test h3_interop_qualification --test h3_datagram_route -- --test-threads=1
```

Upgrade path, before closure:

```bash
cargo test -p eggfetch-core --all-features --test h3_hardening --test h3_alt_svc_discovery --test h3_interop_qualification --test h3_datagram_route -- --test-threads=1
./scripts/check.sh
./scripts/check.sh extended
./scripts/check.sh package
```

Plus the HTTPX exact-SHA requalification required by
`.skills/verification-qualification.md`. No new CI job.

## 12. Documentation updates

- `qualification/http3/upstream-risk-ledger.json` is the current ledger.
- `docs/architecture/core-tls-proxy-protocols.md` dependency section matches
  it and still calls #338 a graduation blocker when the issue is open.
- Historical evidence files are not edited.

## 13. Acceptance criteria

- The ledger has one explicit disposition for every audited issue, including
  #338 and #361.
- Either the lockfile is unchanged and #338 is recorded as a blocker, or
  the lockfile cites a published release that contains the frame-drain fix
  and the H3 suites plus Stage C are green on that SHA.
- No `[patch]`, vendor directory, or git dependency on h3 or Quinn exists.
- The experimental label remains.

## 14. Stop conditions

- Stop before adding a fork, a vendored `frame.rs`, or a git dependency.
- Stop if the published fix changes GOAWAY observation and the drain tests
  cannot be made true without a new public API.
- Stop if an upgrade forces 0-RTT, a new public metric, or a default-on
  H3 policy.
- Do not close #338 as `proven not reachable` without a test or a code-level
  argument that eggfetch never polls the affected `recv_response` /
  `recv_data` path. The September audit did not have that proof.

## 15. Closure evidence required

- Closure record under
  `plans/closure/tls-proxy-protocols/008-h3-upstream-risk-re-audit.md`.
- The ledger file and the upstream URLs as fetched, with the fetch date.
- For an upgrade: the freeze SHA and the Stage C record.
- For a no-upgrade: the crates.io versions observed and the sentence that
  M010 cannot clear the upstream row.

## 16. Handoff notes

Look up the issues again at the start of the work. The statuses in §2 are
planning-time facts from 2026-11-16, not a substitute for the audit.

Land this milestone before anyone treats an M006 or M007 external run as
graduation evidence. If this milestone bumps dependencies after those
milestones have merged, their tests are re-run on the new pins as part of
this milestone's H3 suite. Their closure records do not need to be reopened
unless those tests fail.
