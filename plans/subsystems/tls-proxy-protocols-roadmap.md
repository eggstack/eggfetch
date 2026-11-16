# TLS, Proxy, and Protocols Roadmap

Status: active — M005–M010 closed (H3 experimental retained). M011–M012 registered (security-audit correctives; ready). Stage C renewed on `8af32d7ca4f1140e266875798522f5da09c17850`.

Long-term references:

- `plans/000-long-term-specification.md` §4.4, §5, §8
- `plans/001-terminology-and-domain-model.md` §6, §7
- `plans/002-long-term-roadmap.md` Phase 0 + Phase 4 item 2

Related ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0004-proxy-fallback-total-deadline.md`
- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`
- `plans/adrs/ADR-0007-caller-owned-h3-datagram-routing.md`
- `plans/adrs/ADR-0008-uncompiled-protocol-request-semantics.md`

## 1. Purpose and ownership boundary

Owns transport security and connectivity: TLS configuration and trust,
CONNECT/SOCKS proxying and pooling, H1/H2 negotiation, experimental H3/QUIC,
Alt-Svc discovery, UDS and dialer transports shared with the core
workstream, and the ProxyOverride/`NoProxy` model.

Consumes: pipeline stages, pool/timeout ownership. Must not own: facade
semantics, publication, a parallel connection pool, downstream route policy.

## 2. Work classification

### Invariants

- `additional_ca_*` augments; `ca_certificate_*` replaces. Fallback to
  WebPKI on construction failure only; verification failure never retries
  another store.
- Typed proxy fallback; no request-total state in reusable connectors.
- Forward proxying H1-only; H3 never bypasses proxy selection.

### Capabilities

- HTTP forwarding, CONNECT tunneling, SOCKS5, proxy auth, per-request
  override, `NO_PROXY` bypass, resolved proxy peers/destinations.
- H1/H2 with three-layer `Http2Only`; experimental H3 behind `http3`.

### Infrastructure

- Hyper-owned forward/CONNECT reuse; per-route SOCKS pools; bounded H3
  per-origin cache; authenticated Alt-Svc cache with suppression/drain.

### Polish

- Diagnostics counter semantics; route/client ownership documentation.

## 3. Non-goals

- Removing the experimental label except by the M010 decision after the
  full evidence gate. 0-RTT, WebTransport, H3 application datagrams,
  MASQUE, QUIC migration, and automatic H3-by-default stay out of M006–M010.
  Environment-proxy activation and upstream proxy helpers where eggfetch
  contracts would regress stay out of this roadmap.
- M004 permits only fixed-target transport datagrams beneath origin QUIC; it
  does not expose QUIC DATAGRAM or an application datagram API.

## 4. Current state

Proxy framing back under Hyper with safe reuse; typed fallback; route
pinning for proxy peers and ultimate destinations; H3 hardened (bounded
cache, multi-address fallback, phase-correct timeouts, keepalive idle,
GOAWAY drain) and qualified as retained-experimental. Graduation blockers
recorded in `docs/architecture/core-tls-proxy-protocols.md` and the
machine-readable evidence ledger. The direct-only H3 route boundary is now
reopened narrowly by ADR-0007/M004 so embedders can supply a fixed-target
physical datagram route without becoming QUIC/H3 owners. That seam is now
implemented and closed: `ClientBuilder::datagram_dialer` routes every H3
generation over one caller-owned fixed-target route, with direct H3 unchanged
when unset. Ordinary H3 graduation is registered as M006–M010 and has not
started.

## 5. Target architecture

Invariants permanent. H3 graduates only through M010, on a fresh executable
freeze, after M006–M009 have landed and the external evidence rows are
actually pass. Until that decision, every H3 change keeps the experimental
label and must not convert missing evidence into a pass.
M004 adds one client-scoped datagram-routing contract below QUIC while keeping
origin QUIC/TLS/H3 in Eggfetch and preserving the direct-H3 path when unset.

## 6. Dependency graph

```text
M001 proxy Hyper pooling + upstream reuse (hard predecessor for M002)
    |
    +--> M002 route pinning + egress interop (soft after M001)
    |
    `--> M003 H3 hardening + retained-experimental qualification
              (soft after M001; independent evidence)
              |
              `--> M004 caller-owned H3 datagram routing
                    (hard: M003; interface: ADR-0007 + Quinn 0.11.11 abstract socket)

M006 server-controlled corpus (closed; Stage C rebound)
M007 impairment runner       (closed; Stage C rebound; Linux netem operational for graduation)
M008 upstream risk re-audit  (closed; #338 remains graduation blocker)
M009 resource soak + platform table (closed; Stage C rebound; Darwin row only)
    |
    `--> M010 ordinary H3 graduation decision
           (closed 2026-11-16: experimental retained)
           (open rows: independent servers, public origins, privileged Linux impairment, #338)
```

M001–M003 are closed. M004 is closed under accepted ADR-0007: implementation and
every Tier 2 gate pass, including the full pinned HTTPX compatibility suite and
the API-manifest comparison, which are green on `ubuntu-latest` / Python 3.12.14 at
run `37511735210` (1934 passed / 0 failed; both facades 0 unexplained, 0 stale).
H3 graduation was not an M004 exit condition. The M006–M010 sequence has
closure records: all closed, with Stage C rebound on
`6b571028767dde4b0a78f36e6ecef525c6d68482`; M008 retains the no-upgrade/#338
path and M010 retains experimental. Ordinary HTTP/3 remains experimental
until a later decision fills the open operational and upstream rows.

## 7. Milestones

### Milestone 1 — Proxy Hyper pooling and upstream reuse

Class: infrastructure. Status: closed.

Legacy: `plans/proxy-hyper-pooling-and-upstream-reuse.md`,
`plans/post-audit-maintenance-security-and-proxy-modernization-program.md`
(proxy track), `plans/security-policy-release-and-supply-chain-hardening.md`.

Exit conditions: framing under Hyper with reuse; TLS/pinning/timeout/auth/
error contracts preserved; no parallel pool. Met.

### Milestone 2 — Proxy route pinning and egress interoperability

Class: capability. Status: closed.

Legacy: `plans/proxy-route-pinning-and-egress-interoperability-program.md`,
`plans/pinned-proxy-peer-routing.md`,
`plans/pinned-proxy-destination-routing.md`,
`plans/post-proxy-route-pinning-qualification-and-closure.md`.

Exit conditions: `Proxy::resolved_addresses()` +
`RequestBuilder::proxy_target_addresses()`; no DNS fallback; no Egress
dependency; exact-SHA requalification. Met.

### Milestone 3 — H3 hardening and retained-experimental qualification

Class: capability (experimental) + infrastructure. Status: closed with
retained-experimental verdict.

Legacy: `plans/http3-production-qualification-program.md`,
`plans/http3-post-freeze-diagnostics-requalification-corrective-closure.md`,
`plans/http3-independent-interop-and-impairment-qualification.md`,
`plans/http3-upstream-risk-resource-and-observability-hardening.md`,
`plans/http3-production-graduation-and-compatibility-requalification.md`,
`plans/http3-alt-svc-discovery-fallback-and-draining.md`,
`plans/http3-lifecycle-and-policy-hardening.md`,
`plans/http3-interoperability-and-production-graduation.md`.

Exit conditions: hardening landed; qualification attempted with truthful
retained-experimental outcome; blockers named, not waived. Met as a valid
negative graduation verdict.

### Milestone 4 — Caller-owned H3 datagram routing

Class: capability (experimental). Status: closed.

Implementation:
`plans/implementation/tls-proxy-protocols/004-caller-owned-h3-datagram-routing.md`.

Objective: add one Rust-only client-scoped fixed-target datagram route beneath
experimental H3, privately adapted to Quinn, without exposing Quinn or
downstream route-product types.

Dependencies:

- hard: M003 H3 hardening/retained-experimental closure;
- interface: accepted ADR-0007;
- interface: Quinn 0.11.11 abstract-socket API already present in the H3
  dependency line;
- downstream Eggress is not a dependency.

Deliverable boundary: custom-routed H3 works through an arbitrary fixed-target
datagram route; direct H3 remains unchanged; H3 remains experimental.

User/integration value: embedders can route origin HTTP/3 over a listener-free
UDP provider (for example a SOCKS5 UDP association) while Eggfetch retains TLS,
SNI, H3 and retry/fallback authority.

Exit conditions: bounded/cancel-safe bridge, no direct-UDP fallback when the
route is configured, direct-H3 non-regression, public API oracle update,
Tier 1/2/3 green, exact-SHA Stage C renewal, closure record accepted.

Deferred: per-request route override, MASQUE/CONNECT-UDP, QUIC migration,
0-RTT, H3 application datagrams and graduation.

### Milestone 5 — Uncompiled-protocol fail-closed at adapter boundaries

Class: capability. Status: closed.

Implementation:
`plans/implementation/tls-proxy-protocols/005-uncompiled-protocol-fail-closed.md`.

Closure record:
`plans/closure/tls-proxy-protocols/005-uncompiled-protocol-fail-closed.md`.

Accepted ADR:
`plans/adrs/ADR-0008-uncompiled-protocol-request-semantics.md` (accepted
2026-10-22).

Objective: make a strict protocol request that the built artifact cannot
honor fail closed, with an error that names the missing capability, at every
adapter boundary that exposes a user-facing protocol control.

Dependencies:
- hard: none;
- interface: accepted ADR-0008;
- interface: the engine's documented native-caller downgrade, which stays
  unchanged.

Deliverable boundary: `eggfetch-cli --http2` and `--http3` fail with a
capability-naming error on a build without the capability instead of
silently downgrading or reporting a missing client; the CLI expresses
`http2`/`http3` as crate features forwarding to the core features;
`eggfetch_client_builder_http_version` rejects a strict policy the linked
build cannot honor; the engine distinguishes "no HTTP/3 support" from "no
usable client" without a new error variant; `docs/architecture/cli.md` states
the implemented behavior.

User or operator value: a user who asks for HTTP/2 or HTTP/3 through the CLI
or the FFI either gets that protocol or a clear error naming what the build
lacks. At the baseline `--http2` silently issues HTTP/1.1 and `--http3`
reports "HTTP client not available for this protocol", which misreports a
build capability as a transport failure.

Exit conditions: strict requests fail closed with a named cause in both
adapters; `Auto` still downgrades to HTTP/1.1 without failing; the engine's
native-caller downgrade contract unchanged; the default CLI feature set
unchanged and the new features opt-in; public API oracle unchanged; Tier
1/2/3 green with renewed exact-SHA Stage C binding for both profiles.

Deferred: H3 graduation, which is M010 and remains blocked; enabling `http2` or
`http3` in the CLI by default, which is a separate capability decision; the
FFI's missing non-protocol capabilities.

### Milestone 6 — Server-controlled H3 corpus

Class: infrastructure. Status: ready.

Implementation:
`plans/implementation/tls-proxy-protocols/006-h3-server-controlled-corpus.md`.

Objective: make every ordinary-client corpus case executable locally, and
make server-controlled cases executable against an optional external
manifest, with unsupported kept distinct from pass.

Dependencies:

- hard: none;
- soft: M008, because a later dependency bump requires the new tests to be
  re-run before M010 cites them.

Deliverable boundary: local Quinn fixtures and the qualification coordinator
cover the corpus ids that the client or the in-process server can induce.
External control URLs exist for reset, graceful close, GOAWAY reconnect, and
alternative-port discovery. No independent server is claimed.

User or operator value: a later qualification run can exercise GOAWAY, reset,
and cancellation instead of recording those cases unsupported by construction.

Exit conditions: local coordinator maps each inducible corpus id to a named
passing test; manifest hygiene tests reject credentials and do not count a
missing control URL as a pass; Tier 1 is green; Stage C is renewed because
Rust tests and a Tier 1 script change; HTTP/3 stays experimental.

Deferred: running ngtcp2, quiche, or quic-go; the impairment matrix; the
experimental-label decision.

### Milestone 7 — H3 impairment runner

Class: infrastructure. Status: ready.

Implementation:
`plans/implementation/tls-proxy-protocols/007-h3-impairment-runner.md`.

Objective: supply the Linux namespace/netem runner that
`scripts/h3_impairment.py` already calls, so each matrix scenario is pass,
fail, or unsupported for a real reason.

Dependencies:

- hard: none;
- soft: M008, same re-run rule as M006;
- operational: a privileged Linux namespace, which is required for a pass
  of the matrix and is not required to land the runner.

Deliverable boundary: one opt-in runner and one `http3` scenario test
target. Tier 1 does not impair the network. An unprivileged host records
unsupported and does not satisfy M010.

User or operator value: loss, reordering, jitter, duplication, MTU, and
address-family failures can be executed without adding them to CI.

Exit conditions: the existing coordinator accepts the runner's JSON; the
unprivileged contract self-test passes; Stage C is renewed because a Rust
test target is added; HTTP/3 stays experimental.

Deferred: independent-server impairment; a CI job for netem.

### Milestone 8 — H3 upstream risk re-audit

Class: invariant. Status: ready.

Implementation:
`plans/implementation/tls-proxy-protocols/008-h3-upstream-risk-re-audit.md`.

Objective: write a current Quinn/h3 risk ledger and upgrade only if a
published release closes the buffered-data-on-close defect.

Dependencies:

- hard: none;
- this milestone is a hard predecessor of any evidence M010 cites.

Deliverable boundary: `qualification/http3/upstream-risk-ledger.json` plus
either an unchanged lockfile with #338 still blocking, or a published
upgrade, drain tests, and a renewed Stage C binding. No fork and no vendored
patch.

User or operator value: graduation cannot proceed on a stale September 2026
dependency story, and it cannot proceed by forking h3.

Exit conditions: every audited issue has one disposition; the architecture
dependency section matches the ledger; the experimental label remains.

Deferred: clearing the experimental label; fixing #361 inside eggfetch when
the published crates still defer STOP_SENDING.

### Milestone 9 — H3 resource soak and platform scope

Class: infrastructure. Status: ready.

Implementation:
`plans/implementation/tls-proxy-protocols/009-h3-resource-soak-and-platform-scope.md`.

Objective: add a bounded H3 reuse and churn soak, and a platform table that
records only executions that happened.

Dependencies:

- hard: none;
- soft: M008, so the soak that M010 quotes is the post-audit one.

Deliverable boundary: longer loopback cycles with an envelope rather than an
exact RSS figure, plus one filled platform row for the host that ran them.
Other operating systems stay "not executed".

User or operator value: resource growth and the support boundary are
visible before anyone writes a support claim.

Exit conditions: cache and diagnostic caps still hold; the table does not
infer runtime support from a package build; Stage C is renewed; HTTP/3 stays
experimental.

Deferred: multi-OS execution, which M010 may attach when those hosts are
actually run.

### Milestone 10 — Ordinary H3 graduation decision

Class: capability. Status: blocked.

Implementation:
`plans/implementation/tls-proxy-protocols/010-ordinary-h3-graduation-decision.md`.

Objective: run the existing graduation gate on the post-audit tree and
either describe ordinary request/response HTTP/3 as supported or retain the
experimental label with the open rows named.

Dependencies:

- hard: M006, M007, M008, and M009 closed;
- operational: two pinned non-Quinn servers, one independent drain source,
  three public origins, and a privileged Linux impairment run.

Deliverable boundary: a new dated decision section and redacted evidence.
The supported scope, if the gate passes, is ordinary client operation only.
Discovery stays off by default. Datagram routing and advanced QUIC features
stay experimental. Long-term specification files are not edited here.

User or operator value: the experimental label matches current evidence
instead of the September 2026 freeze.

Exit conditions: every gate row is pass on one SHA, or the decision records
the failing rows and leaves the label in place. A product defect stops the
milestone and opens a corrective plan.

Deferred: H3-by-default, CLI/FFI default `http3`, 0-RTT, WebTransport,
MASQUE, migration, and graduating the datagram dialer.

### Milestone 11 — H3 dial-error sanitization and datagram recv trust

Class: invariant. Status: ready.

Implementation:
`plans/implementation/tls-proxy-protocols/011-h3-dial-error-sanitization-and-datagram-recv-trust.md`.

Objective: close the post-M010 audit High/Medium gaps on the experimental
datagram path — stop embedding provider `DialError` text into
`Error::H3Connect`, and enforce fixed-peer recv checks in the private Quinn
bridge — without changing public dialer shapes or graduating H3.

Dependencies: M004 closed; audit findings recorded.

Deliverable boundary: core transport + tests + threat-model alignment.
Upstream h3 #338 is out of scope (remains a graduation blocker).

Exit conditions: both findings closed with tests; Stage C rebound after the
executable change; H3 still experimental.

### Milestone 12 — H3 qualification security hygiene

Class: infrastructure. Status: ready.

Implementation:
`plans/implementation/tls-proxy-protocols/012-h3-qualification-security-hygiene.md`.

Objective: refresh the upstream-risk ledger and embedded qualification
lockfiles to the post-advisory Quinn pins, harden netem `scenario_id`
handling, and reinforce opt-in cert-bypass / operator-URL boundaries.

Dependencies: M008 ledger exists; Quinn advisory bump on the workspace lock.

Deliverable boundary: `qualification/`, H3 scripts, docs. No H3 graduation.
No new CI job.

Exit conditions: ledger/locks not stale vs workspace authority; shell and
cert-bypass hygiene accepted; Tier 1 runner self-test green.

## 8. Cross-cutting requirements

Trust construction explicit per profile; `Proxy(headers=…)` rides the proxy
leg only; proxy setup uses one monotonic deadline; redaction at every
boundary; `crypto_provider()` per-config.

## 9. Verification strategy

Proxy/TLS/route matrices; H3 deterministic loopback controls in the normal
suite; independent-server, impairment, and public-origin evidence manual and
opt-in, never routine gates. M006 and M007 add harness code only. M010 is
the evidence run. Tier 2 and the HTTPX oracles renew on any milestone that
changes executable inputs. No new CI job or matrix.

## 10. Risks and decision points

Risk: silent H3 capability growth. Control: M006–M009 keep the experimental
label, and M010 is the only milestone allowed to change it, and only when
every gate row is pass. A failed row is a recorded blocker, not a waiver.

Risk: citing the September 2026 freeze after the lockfile moved. Control:
M008 writes a new ledger, and M010 cites only the post-audit SHA.

## 11. Completion definition

M001–M003 evidence is accepted. The subsystem is active.
M004 is closed on an unconditional exact-SHA Stage C binding because it changed
executable/public Rust inputs; its two outstanding Tier 2 gates were then run green
on `ubuntu-latest` / Python 3.12.14 (run `37511735210`). The subsystem holds at
steady state with H3 still experimental. Graduation remains controlled by the separate Phase
4 evidence trigger, and no future milestone is unblocked by this closure.

The subsystem reopens for M005, registered from a repository audit that found
the CLI and FFI silently downgrading a strict protocol request the linked
build cannot honor, and the CLI reporting a missing Hyper client where the
real cause is a missing HTTP/3 capability. M005 changes no engine
negotiation logic and does not affect H3's experimental status.

M005 was implemented and is **closed** on 2026-10-22: the CLI
`--http2` silent downgrade and the CLI `--http3` misleading error are closed,
and the FFI's unguarded strict policy now fails closed. The engine's
documented native-caller downgrade is unchanged; only the `Http3Only` error
text and classification changed, with no new error variant and no new public
surface. It was first recorded as conditionally closed because the milestone
changes Rust sources, tests, and crate manifests — all executable inputs —
which invalidated the live Stage C binding and left the frozen-SHA Tier
1/2/3 gates outstanding. That condition was met on bound SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c`, where the Stage C binding was
renewed and the tier gates ran green; M005 moved from `conditionally closed`
to `closed` there. The closure authorizes neither H3 graduation nor
publication, and the maintainer decision on a strict-HTTP/3 FFI policy value
remains open.

The ordinary-H3 graduation sequence registered on 2026-11-16 now has closure
records. M006–M010 are closed; Stage C is rebound on
`6b571028767dde4b0a78f36e6ecef525c6d68482`. M008 retains the #338 blocker and
M010 retains experimental. A future support decision still needs independent
servers, privileged impairment, public origins, and a published fix for h3
#338.

## 12. Milestone status

| Milestone | Status | Implementation plan | Closure record | Blockers |
|---|---|---|---|---|
| M001 proxy pooling/reuse | closed | legacy plans §7 | legacy closures | — |
| M002 route pinning/egress | closed | legacy program §7 | legacy closure | — |
| M003 H3 retained-experimental | closed | legacy program §7 | legacy + evidence ledger | Graduation evidence (gated future) |
| M004 caller-owned H3 datagram routing | closed | `implementation/tls-proxy-protocols/004-caller-owned-h3-datagram-routing.md` | `closure/tls-proxy-protocols/004-caller-owned-h3-datagram-routing.md` | ADR-0007 accepted; Tier 1/2/3 green including full compat suite (1934 passed) + API manifests on `ubuntu-latest`/3.12.14, run `37511735210`; H3 stays experimental |
| M005 uncompiled-protocol fail-closed | closed | `implementation/tls-proxy-protocols/005-uncompiled-protocol-fail-closed.md` | `closure/tls-proxy-protocols/005-uncompiled-protocol-fail-closed.md` | ADR-0008 accepted; closed on bound SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c` with the Stage C binding renewed there; maintainer decision on a strict-HTTP/3 FFI policy value |
| M006 server-controlled H3 corpus | closed | `implementation/tls-proxy-protocols/006-h3-server-controlled-corpus.md` | `closure/tls-proxy-protocols/006-h3-server-controlled-corpus.md` | Stage C rebound on `6b571028…` |
| M007 H3 impairment runner | closed | `implementation/tls-proxy-protocols/007-h3-impairment-runner.md` | `closure/tls-proxy-protocols/007-h3-impairment-runner.md` | Privileged Linux matrix still operational for graduation |
| M008 H3 upstream risk re-audit | closed | `implementation/tls-proxy-protocols/008-h3-upstream-risk-re-audit.md` | `closure/tls-proxy-protocols/008-h3-upstream-risk-re-audit.md` | No-upgrade; #338 blocker |
| M009 H3 resource soak and platform scope | closed | `implementation/tls-proxy-protocols/009-h3-resource-soak-and-platform-scope.md` | `closure/tls-proxy-protocols/009-h3-resource-soak-and-platform-scope.md` | Darwin row only; Stage C rebound |
| M010 ordinary H3 graduation decision | closed (experimental retained) | `implementation/tls-proxy-protocols/010-ordinary-h3-graduation-decision.md` | `closure/tls-proxy-protocols/010-ordinary-h3-graduation-decision.md` | Gate failed on independent servers, impairment, public origins, #338 |
| M011 dial-error sanitization + datagram recv trust | ready | `implementation/tls-proxy-protocols/011-h3-dial-error-sanitization-and-datagram-recv-trust.md` | — | Post-M010 security audit High/Medium; Stage C required after implement |
| M012 H3 qualification security hygiene | ready | `implementation/tls-proxy-protocols/012-h3-qualification-security-hygiene.md` | — | Stale ledger/embed locks; netem/cert-bypass hygiene |
