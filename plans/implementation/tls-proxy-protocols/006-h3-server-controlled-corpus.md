# TLS, Proxy, and Protocols Milestone 006 — Server-Controlled H3 Corpus

Status: closed

Repository baseline: `f11a46b28df88f43e923d5211f82985784daaf40`

Source roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-6--server-controlled-h3-corpus`

Long-term requirements:

- `plans/000-long-term-specification.md` §3 (production HTTP/3 remains gated on the evidence gate)
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 item 2

Applicable ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`
- `plans/adrs/ADR-0007-caller-owned-h3-datagram-routing.md`

Primary class: infrastructure

## 1. Objective

Make every required ordinary-client HTTP/3 corpus case executable, with an
explicit pass, fail, or unsupported result, against the local Quinn/h3
control and against an optional external server that declares the case.

This milestone builds the missing qualification harness. It does not collect
independent-server evidence and it does not change the experimental label.

## 2. Why this milestone is ready

The corpus, coordinator, and local control suite already exist. The gap is
narrow and local:

- `qualification/http3/corpus.json` lists 18 required cases.
- `scripts/h3_qualification.py` runs one local test target and then one
  external test, `external_h3_interop_servers_if_configured`.
- The external path only executes eight generic endpoint cases (`get`,
  `head`, `buffered_post`, `streaming_upload`, `large_streaming_download`,
  `response_trailers`, `multiplexed_requests`, `connection_reuse`).
- Server-controlled cases (`stream_reset`, `graceful_connection_close`,
  `goaway_reconnect`, `alternative_port`, and the three cancellation cases)
  are recorded `unsupported` even when a manifest lists them.
- The local mapper has no fixture for `streaming_upload`,
  `cancellation_during_upload`, `cancellation_during_download`,
  `graceful_connection_close`, `goaway_reconnect`, `ipv4`, or
  `alternative_port`, so a green local suite still reports those corpus ids
  unsupported.
- GOAWAY draining is already implemented and covered against a local server
  in `h3_alt_svc_discovery.rs` (`graceful_close_does_not_deadlock_and_reconnects`).
  That evidence is not wired into the corpus runner, and it is not an
  independent-server result.

No new architecture is required. The production H3 route, Alt-Svc cache,
fallback rules, and `Http3Only` strictness stay as they are. Historical
plans under `plans/http3-*.md` stay frozen records; this file is the active
handoff.

M008 may later change the Quinn/h3 pins. External evidence is collected only
in M010, after M008 closes, so this harness can land in parallel with the
audit.

## 3. Current implementation evidence

At the baseline:

- local controls live in `crates/eggfetch-core/tests/h3_interop_qualification.rs`
  and are selected by `scripts/h3_qualification.py` `known_local`;
- the external test reads `EGGFETCH_H3_INTEROP_ENDPOINT` and
  `EGGFETCH_H3_INTEROP_CASES` and speaks `Http3Only`;
- `EGGFETCH_H3_INTEROP_DANGER_ACCEPT_INVALID_CERTS` is the only
  qualification-only certificate bypass, and it is opt-in;
- `scripts/test_h3_qualification.py` is the Tier 1 runner self-test and
  makes no network connection;
- the September evidence ledger records zero independent-server passes.
  Do not edit that ledger.

## 4. Invariants that must not regress

- `eggfetch-core` remains the only HTTP/3 implementation. Adapters and
  qualification scripts do not open their own QUIC sockets.
- `Http3Only` never falls back. `Auto { allow_http3: true }` falls back only
  under the existing pre-commit, replayable-body rules.
- One-shot and non-replayable bodies are never duplicated.
- Absence of an external server is `unsupported` or an explicit skip. It is
  never a pass and never a Tier 1 failure.
- The experimental label stays. This milestone does not edit the production
  graduation paragraph except to point operators at the new harness where
  the architecture doc describes the corpus runner.
- Caller-owned datagram routing stays experimental and Rust-only.
- `0-RTT` stays disabled.
- No new public engine type, method, or feature flag.
- Redaction of `authorization`, `proxy-authorization`, `cookie`, and
  `set-cookie` is unchanged.
- The runner does not shell out to a command supplied by the manifest.

## 5. Scope

### In scope

- Local Quinn/h3 fixtures and corpus-runner mappings for every corpus id
  that can be induced by the client or by the in-process test server.
- Optional manifest `controls` URLs for the cases an external peer must
  induce, validated with the same endpoint rules as the main URL.
- External execution of client-driven cancellation without a server-side
  control channel.
- Runner self-tests that reject a manifest which claims a server-controlled
  case without a control URL, and that still reject credentials and query
  strings.

### Explicitly out of scope

- Standing up ngtcp2, quiche, quic-go, or any public origin. That execution
  is M010.
- The impairment matrix and its netem runner. That is M007.
- Upgrading `h3`, `h3-quinn`, or `quinn`. That is M008.
- Removing the experimental label, enabling H3 by default, or enabling
  `http3` in the CLI or FFI default features.
- A manifest field that names a restart shell command.
- WebTransport, MASQUE, QUIC migration, application datagrams, and 0-RTT.

## 6. Required production changes

No production behavior change is expected. If a fixture shows that GOAWAY
draining, cancellation, or reset handling is wrong, stop and record the
defect. Do not widen this milestone into a transport rewrite. A narrow
regression fix is allowed only when an existing local test already defines
the contract and the new fixture caught a break in that contract.

## 7. Ordered work packages

1. Extend the local interop server in `h3_interop_qualification.rs` so one
   process can serve the ordinary corpus and three controlled behaviors:
   - reset the response stream after response headers or mid-body;
   - complete one response and then graceful-close the connection;
   - advertise an authenticated Alt-Svc `h3` alternative on a second local
     port and serve that port.
   Keep the server loopback-only. Reuse the existing self-signed test
   certificate pattern already in that file.
2. Add named tests, and map them from `known_local`, for the corpus ids that
   currently have no local fixture:
   - `streaming_upload` — one streaming request body is sent once and the
     response completes;
   - `cancellation_before_headers`, `cancellation_during_upload`,
     `cancellation_during_download` — cancellation returns promptly, the
     client stays usable, and a later request on the same client succeeds;
   - `graceful_connection_close` — the in-flight response completes or
     returns the real connection error, and the next request reconnects;
   - `goaway_reconnect` — after the controlled close, the next request is
     counted in `TransportMetrics` `h3_reconnected` and succeeds;
   - `alternative_port` — `Auto { allow_http3: true }` learns the Alt-Svc
     entry and the following eligible request uses H3 on the alternative
     port, with origin identity unchanged;
   - `ipv4` — the loopback IPv4 fixture succeeds. `ipv6` stays the existing
     capability-detection test and must not start failing on a host without
     IPv6.
   `server_restart` keeps its existing local test. Do not add an external
   restart command.
3. Teach `external_h3_interop_servers_if_configured` the client-driven
   cases. Cancellation must be initiated by the client: drop the request
   before reading headers, cancel an upload the client has not finished,
   and abandon a download after the first chunk when the body is large
   enough to still be open. A later request on the same client must
   succeed. Do not require the peer to stall.
4. Add an optional `controls` object on each server manifest entry:
   - `stream_reset`: HTTPS URL that aborts the response stream;
   - `graceful_connection_close`: HTTPS URL that completes one response and
     then closes the connection;
   - `goaway_reconnect`: HTTPS URL that completes one response and then
     sends a graceful GOAWAY or equivalent connection close. The following
     request uses the manifest `endpoint` on the same origin and must
     succeed with `h3_reconnected` increased;
   - `alternative_port`: HTTPS discovery URL that returns an authenticated
     Alt-Svc `h3` entry for a different port on the same authority. The
     following request uses the manifest endpoint in `Auto { allow_http3:
     true }` mode.
   Validate every control URL with the existing endpoint rules: `https`,
   hostname, no userinfo, no query, no fragment. A capability listed in
   `capabilities` that needs a control is `unsupported` when the control
   URL is absent. It is a manifest error when the control URL is present
   but the capability is not declared. Unknown control keys are a manifest
   error.
5. Pass control URLs to the external test through environment variables
   that the runner sets and the test reads. Keep
   `EGGFETCH_H3_INTEROP_DANGER_ACCEPT_INVALID_CERTS` as the only certificate
   bypass, and keep it off unless the operator passed the existing flag.
6. Bump `qualification/http3/corpus.json` or the server-manifest schema
   version if the new fields would be misread by `schema_version` 1.
   Both coordinators fail closed on an unsupported version. Update
   `qualification/http3/servers.example.json` so its example controls are
   obviously placeholders on `h3.example.invalid` and are not evidence.
7. Extend `scripts/test_h3_qualification.py` for the new manifest rules.
   The self-test must not open a socket and must not run `cargo test`.
8. Point the module docs in `h3_interop_qualification.rs` at this plan as
   the active harness plan. Leave the historical program files in place
   and do not mark them complete.

## 8. Failure, cancellation, timeout, and pool semantics

Cancellation tests assert prompt return and a usable client afterwards.
They use the existing timeout types. They must not add a new timeout field
or a new `ResponseBody` variant.

GOAWAY behavior asserted here is the behavior already implemented in
`crates/eggfetch-core/src/transport/http3.rs`: a draining generation accepts
no new work, and the next eligible request opens a fresh generation. The
test observes that through `h3_reconnected` and a successful follow-up
response. It does not string-match an implementation-specific error when
`H3DispatchError.draining` or the metrics counter is available.

`Http3Only` remains the policy for generic external endpoint cases. The
alternative-port case is the one external case that uses
`Auto { allow_http3: true }`, because discovery is the behavior under test.

## 9. Compatibility and migration (incl. feature-profile effects)

The `http3` feature gate is unchanged. Lean, native, CLI, Python, FFI, and
Node profiles do not gain H3. Python `http3=True` behavior is unchanged.

This milestone changes Rust tests and a Tier 1 validation script
(`scripts/test_h3_qualification.py`). Those are executable inputs. Closure
requires a new exact-SHA Stage C binding for HTTPX 0.28.1 and HTTPX2
2.12.0. Do not cite the September 2026 H3 freeze
`639bf186a71c054e11278d1b160ffe7a6f172c02`.

## 10. Required tests

- New and mapped cases in `h3_interop_qualification.rs`, loopback only,
  `--test-threads=1`.
- Existing `h3_hardening` and `h3_alt_svc_discovery` suites still pass.
- `scripts/test_h3_qualification.py` covers: control URL hygiene, a
  server-controlled capability without a control URL recorded as
  unsupported rather than pass, and rejection of an unknown control key.
- One local-only coordinator run
  (`scripts/h3_qualification.py --local-only`) reports pass for every
  corpus id that now has a local fixture. `ipv6` may remain a
  capability-detection pass on a host without IPv6. No external server is
  required for acceptance.

## 11. Required verification commands

```bash
python scripts/test_h3_qualification.py
python scripts/h3_qualification.py --local-only --output target/h3-qualification-local.json
cargo test -p eggfetch-core --all-features --test h3_interop_qualification -- --test-threads=1
cargo test -p eggfetch-core --all-features --test h3_hardening --test h3_alt_svc_discovery -- --test-threads=1
./scripts/check.sh
```

Tier 2 and Tier 3, including both HTTPX profiles on one frozen SHA, are
required before closure because executable inputs change. Do not add a CI
job, a matrix, or a new workflow.

## 12. Documentation updates

- `docs/architecture/core-tls-proxy-protocols.md`: the interop-corpus
  paragraph names which cases the external runner can now execute, and
  states that server-controlled cases without a control URL stay
  unsupported. The experimental verdict stays.
- `qualification/http3/servers.example.json` comments stay inside JSON
  `notes` strings and state that the file is a template.
- Do not edit `plans/http3-*-evidence.json`,
  `plans/http3-production-qualification-evidence.md`, or the historical
  program files.

## 13. Acceptance criteria

- Every corpus id except an unavailable `ipv6` host has a local result of
  pass from the coordinator, produced by a named test the runner actually
  observed as `... ok`.
- A manifest that declares `goaway_reconnect` without `controls.goaway_reconnect`
  yields `unsupported` for that case and cannot make
  `independent_servers.status` a pass on the strength of that case.
- The external test executes client-driven cancellation and, when control
  URLs are present, reset, graceful close, GOAWAY reconnect, and
  alternative-port discovery.
- Tier 1 is green, and Stage C is renewed on the freeze that contains this
  work.
- HTTP/3 is still documented as experimental.

## 14. Stop conditions

- Stop if the only way to observe GOAWAY is to fork or vendor `h3`.
- Stop if a fixture requires a new public client API or a new error variant.
- Stop if making the local corpus pass requires weakening an existing
  assertion in `h3_hardening.rs` or `h3_alt_svc_discovery.rs`.
- Stop if the manifest needs to carry credentials, query secrets, or a
  shell command.
- A product defect in draining or cancellation is a stop, not a silent
  scope expansion. Write the failing test down in the closure notes and
  hand the defect back as a separate corrective plan.

## 15. Closure evidence required

- Closure record under
  `plans/closure/tls-proxy-protocols/006-h3-server-controlled-corpus.md`.
- Requirement-to-evidence matrix: each corpus id, the local test name, and
  the external trigger (client-driven or control URL).
- The local-only coordinator JSON, with no independent-server pass claimed.
- Exact SHA of the requalified tree and the Stage C ledger reference.
- An explicit statement that M010 is still blocked on two real non-Quinn
  servers.

## 16. Handoff notes

Inspect `QuicTestServer` in `h3_interop_qualification.rs` and the draining
test in `h3_alt_svc_discovery.rs` before adding fixtures. Prefer extending
the existing server over a second fixture stack.

The coordinator currently marks every attempted external case pass when the
single external test returns 0. Keep one test process if that is simpler,
but the JSON must not mark a case pass when that case was not enabled and
did not print its own `H3_CORPUS_CASE <id>=pass` line. Parse those lines.
A panic in one case is a failure of that case, not a pass of the others.

Do not run this harness against live public hosts.
