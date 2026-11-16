# TLS, Proxy, and Protocols Milestone 010 — Ordinary H3 Graduation Decision

Status: closed

Repository baseline: `f11a46b28df88f43e923d5211f82985784daaf40`

Source roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-10--ordinary-h3-graduation-decision`

Long-term requirements:

- `plans/000-long-term-specification.md` §3 (a production HTTP/3 stack exists only when the evidence gate closes)
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 item 2

Applicable ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`
- `plans/adrs/ADR-0007-caller-owned-h3-datagram-routing.md`
- `plans/adrs/ADR-0008-uncompiled-protocol-request-semantics.md`

Primary class: capability

## 1. Objective

Decide whether ordinary HTTP/3 request/response operation can be described
as supported, using the gate already written in
`docs/architecture/core-tls-proxy-protocols.md` and
`plans/http3-production-qualification-program.md`.

Supported, if the gate passes, means the ordinary client path only. The
decision is evidence-backed. A retained experimental label is a valid close
when any required row is missing.

## 2. Why this milestone is ready

It is not ready to execute. Hard predecessors:

- M006 closed, so server-controlled corpus cases can be run and cannot be
  counted as passes while `unsupported`;
- M007 closed, so the impairment coordinator has a real runner;
- M008 closed, so the upstream ledger matches the pins under test;
- M009 closed, so the soak and the platform table exist before this
  decision extends them.

Operational predecessors, supplied by the operator and not by those
milestones:

- two pinned non-Quinn servers;
- one of them able to induce GOAWAY or an equivalent graceful drain,
  stream reset, and a restart;
- three public origins on unrelated operators or CDNs;
- a privileged Linux run of the impairment matrix;
- any extra OS row that the support claim will name.

Do not start this milestone by editing the experimental wording.

## 3. Current implementation evidence

The September 2026 decision retained experimental. Its ledger is
historical and its SHA is not this tree. The current blockers, until this
milestone replaces them with a new run, are:

- zero independent non-Quinn passes;
- no independent GOAWAY/drain evidence;
- an empty public-origin ledger;
- an impairment matrix left `unsupported`;
- h3 #338 open, with no eggfetch workaround on the response-header path;
- runtime evidence recorded for Linux aarch64 only, on that older freeze.

M006 through M009 prepare harnesses and local evidence. Their closure
records are not this decision.

## 4. Invariants that must not regress

- Missing evidence is not a pass. `unsupported`, an empty ledger, and a
  skipped public host do not satisfy a gate row.
- `Http3Only` never falls back. Auto fallback stays pre-commit and
  replayable-only.
- No second H3 stack, no vendored h3, and no default `allow_http3: true`.
- Caller-owned datagram routing stays experimental even if ordinary H3
  graduates.
- 0-RTT, WebTransport, application datagrams, MASQUE, and connection
  migration stay experimental or unimplemented.
- CLI and FFI default features do not gain `http3`. ADR-0008 still applies:
  a strict HTTP/3 request on a build without the feature fails closed.
- The Stage C binding is renewed only when this milestone changes
  executable inputs. A documentation decision does not invalidate it. A
  product fix discovered here does not land inside this plan.

## 5. Scope

### In scope

- One evidence run on the post-M008 pins, using the M006 and M007 tools.
- A public-origin ledger with the fields already specified in
  `plans/http3-public-origin-spot-check-ledger.json`.
- A written decision: supported ordinary client, or experimental retained,
  with the failing rows named.
- Documentation edits that match that decision and nothing broader.

### Explicitly out of scope

- Implementing the harness, the netem runner, the soak, or the dependency
  audit. Those are M006–M009.
- Enabling H3 discovery by default, or compiling `http3` into the default
  CLI or FFI build.
- Graduating datagram routing or any advanced QUIC feature.
- New CI, a new evidence schema, or a new workflow.
- Fixing a product defect found during the run. Stop and open a corrective
  plan. Re-freeze afterwards. Do not promote on a tree that still has the
  defect.

## 6. Required production changes

None on the decision path. Documentation changes follow the decision in
§7. Production code changes are a stop condition.

## 7. Ordered work packages

1. Confirm M006, M007, and M008 closures exist and that the checkout SHA
   contains them. Read `qualification/http3/upstream-risk-ledger.json`. If
   #338 is still a graduation blocker, the decision is already "experimental
   retained" for that row. Continue the other evidence runs so the record
   is complete, and do not clear the upstream row by argument.
2. Independent servers. Copy `qualification/http3/servers.example.json` to
   an untracked private manifest. Fill at least two implementations that
   are not Quinn/h3, chosen from ngtcp2/nghttp3, quiche, quic-go, or
   MsQuic. Each entry needs a version and a `source_revision` or
   `image_digest`. Run:

   ```bash
   python scripts/h3_qualification.py --manifest <private-manifest> --output target/h3-qualification.json
   ```

   At least two servers must pass the generic cases. At least one must
   pass `goaway_reconnect` through its control URL. Reset and graceful
   close on that server are required when the server can induce them;
   `unsupported` does not fill the independent-drain row. Store the
   manifest outside git if it contains local endpoints. Commit only a
   redacted result that keeps implementation, version, identity, case
   status, and SHA.
3. Impairment. On a Linux host that can create a namespace, run:

   ```bash
   python scripts/h3_impairment.py --runner scripts/h3_impairment_netem.py --output target/h3-impairment.json
   ```

   Every required scenario is `pass`, or the decision stays experimental
   and lists the ids that are `fail` or `unsupported`. A Quinn peer is
   acceptable for this row. It is not acceptable for the independent-server
   row.
4. Public origins. Follow the manual procedure in
   `docs/architecture/core-tls-proxy-protocols.md`. Record at least three
   successful origins from unrelated operators or CDNs in a new file
   `qualification/http3/public-origin-ledger.json`, using the entry fields
   from the historical ledger schema: time, hostname only, operator or CDN,
   authenticated Alt-Svc category, whether a later request used H3, body
   integrity, address family, UDP-unavailable fallback, and whether any
   failure was transient. Transient Internet failure does not fail the
   deterministic suites and does not count as a success. Do not put host
   names into Rust tests. Do not edit the historical empty ledger.
5. Platform scope. Start from the M009 table. Add a row only for a host
   that ran the deterministic H3 suites on this SHA. Name IPv4 and IPv6
   only where those cases actually ran. Leave every other OS unexecuted.
6. Decide. Ordinary HTTP/3 may be described as supported only when all of
   the following are true on this SHA:
   - deterministic hardening, Alt-Svc, fallback, lifecycle, and soak suites
     are green;
   - two independent non-Quinn servers passed the required corpus;
   - one independent server demonstrated drain and reconnect;
   - the public ledger has three successful current origins;
   - the impairment matrix is pass on the required invariants;
   - the upstream ledger has no open issue that corrupts an ordinary
     successful response, and #361 is either fixed or explicitly outside
     the supported cancellation claim;
   - the platform table matches the wording of the support claim;
   - Tier 1 is green. Tier 2, Tier 3, and both HTTPX profiles are green on
     this SHA if this milestone, or an unclosed predecessor, changed
     executable inputs since the last binding.
7. Write the decision. Add a new dated section in
   `docs/architecture/core-tls-proxy-protocols.md`. Do not rewrite the
   2026-09-11 section.
   - If the gate passes: call ordinary request/response supported and list
     the scope in §1. Update the feature-flag page, the Rust and Python
     guides, the README line, the compatibility tables, and the
     troubleshooting page so they use that scope. Keep datagram routing,
     0-RTT, WebTransport, MASQUE, and migration marked experimental or
     absent. Leave `allow_http3: false` as the default.
   - If the gate fails: keep every experimental label. Name the open rows
     in the new section and in `plans/registry.md`.

## 8. Failure, cancellation, timeout, and pool semantics

This milestone does not change them. Evidence runs use the client as M006
and M007 left it. A failure that shows a deadline, replay, or pool-permit
bug stops the decision. The corrective plan owns the fix.

## 9. Compatibility and migration (incl. feature-profile effects)

Graduation does not add `http3` to a profile that lacks it. Python
`http3=True` remains opt-in. The CLI `--http3` flag remains available only
on a build that compiled the feature, and still fails closed otherwise.

A docs-only decision keeps the current Stage C SHA. Say that in the closure.
Any executable change means this milestone stops; the corrective plan owns
the requalification.

H3 support and HTTPX Stage C stay separate sentences in the docs. A green
compat suite is not an H3 pass.

## 10. Required tests

No new product tests. The evidence commands in §7 are the tests. Deterministic
suites are re-run on the decision SHA:

```bash
cargo test -p eggfetch-core --all-features --test h3_hardening --test h3_alt_svc_discovery --test h3_interop_qualification --test h3_datagram_route --test h3_impairment_scenarios -- --test-threads=1
```

`h3_impairment_scenarios` without the netem environment must skip rather
than pass the matrix. The matrix result comes only from the coordinator
JSON.

## 11. Required verification commands

The §7 commands, plus `./scripts/check.sh` on the decision SHA. Extended
and package tiers run when the binding has to move. No new CI.

## 12. Documentation updates

Listed in §7.6–§7.7. Also update the TLS/protocols roadmap status row and
`plans/registry.md` gated-future line so they match the decision.

Do not edit `plans/000-long-term-specification.md`,
`plans/001-terminology-and-domain-model.md`, or
`plans/002-long-term-roadmap.md` in this milestone. On a passing gate, the
closure record asks the maintainer to amend §3 of the long-term
specification in its own architecture edit. The architecture doc carries
the operational decision until that edit lands. On a retained-experimental
close, leave the long-term documents untouched.

## 13. Acceptance criteria

- One new decision section, dated, with either a supported ordinary-client
  scope or a blocker list.
- Machine-readable results for the corpus run and the impairment run, and
  a public ledger with three successes or an explicit shortfall.
- The upstream ledger SHA matches the decision SHA.
- No historical evidence file was overwritten.
- Datagram routing and advanced QUIC features are still not described as
  supported.
- The default version policy is still `Auto { allow_http3: false }`.

## 14. Stop conditions

- Stop on a product defect. Open a corrective plan. Do not promote.
- Stop if the only way to fill a row is to treat `unsupported` as pass, to
  reuse the September 2026 JSON, or to run public hosts from Tier 1.
- Stop if promotion would require a fork of h3 or Quinn.
- Stop if the support wording would cover a platform, an address family, or
  a QUIC feature the evidence did not run.
- Stop before editing `plans/000-long-term-specification.md`,
  `plans/001-terminology-and-domain-model.md`, or
  `plans/002-long-term-roadmap.md`.

## 15. Closure evidence required

- Closure record under
  `plans/closure/tls-proxy-protocols/010-ordinary-h3-graduation-decision.md`
  with the requirement-to-evidence matrix for every gate row.
- Paths or attached redacted JSON for the corpus, impairment, public
  ledger, upstream ledger, and platform table.
- The decision SHA.
- Recommendation: closed with ordinary H3 supported, or closed with
  experimental retained. Both need the matrix. A missing predecessor is
  not a close; leave this plan blocked.

## 16. Handoff notes

This plan becomes the handoff only after M006, M007, and M008 are closed.
M009 should be closed as well so the platform table and soak exist; if
M009 is still open, do not invent a parallel soak here.

Private manifests and public host lists stay out of git unless they are
redacted to hostname, implementation identity, and pass/fail. No
credentials, no query strings, no raw diagnostic dumps that contain
cookies or authorization headers.

The canonical gate text remains the architecture doc plus the historical
program's graduation-gate section. This plan sequences the run. It does
not replace that gate with a weaker one.
