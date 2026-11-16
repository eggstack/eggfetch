# TLS, Proxy, and Protocols Milestone 011 — H3 Dial-Error Sanitization and Datagram Recv Trust

Status: closed

Repository baseline: `8af32d7ca4f1140e266875798522f5da09c17850`

Closure: `plans/closure/tls-proxy-protocols/011-h3-dial-error-sanitization-and-datagram-recv-trust.md`

Source roadmap:

- `plans/subsystems/tls-proxy-protocols-roadmap.md#milestone-11--h3-dial-error-sanitization-and-datagram-recv-trust`

Long-term requirements:

- `plans/000-long-term-specification.md` §3, §5, §8
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 item 2

Applicable ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`
- `plans/adrs/ADR-0007-caller-owned-h3-datagram-routing.md`

Primary class: invariant

## 1. Objective

Close two security-audit gaps on the experimental caller-owned H3 datagram
path so eggfetch matches its own threat-model and ADR-0007 redaction rules:

1. **Dial-path error text:** `Error::H3Connect` must not embed raw
   `DialError::message()` (or other provider-controlled strings) for
   connection-kind failures. Terminal route I/O already uses fixed
   `ROUTE_IO_TEXT`; connect mapping must be consistent.
2. **Recv source trust:** the private Quinn bridge must not blindly trust
   provider datagrams as coming from `peer_addr()`. Reject or drop datagrams
   whose reported peer does not match the fixed route peer (defense in depth
   beyond AEAD), without changing the public `DatagramDialer` /
   `DatagramRoute` shapes.

This milestone does **not** address upstream h3 #338 (already a graduation
blocker under M008/M010). It does not graduate H3 or expose datagram routing
beyond Rust.

## 2. Why this milestone is ready

M004 is closed. A post-M010 security audit of
`feat/h3-caller-owned-datagram-routing` recorded:

- High: `route_dial_error` / `route_error_override` embed
  `failure.message()` into `Error::H3Connect` for
  `DialErrorKind::Connection`, contradicting the threat-model claim that
  provider messages are not re-exposed and that classification uses typed
  `DialErrorKind` only.
- Medium: the bridge tags `RecvMeta.addr` as `self.peer` without validating
  what the provider delivered, so a compromised provider can inject off-path
  UDP into the QUIC stack (mostly dropped by AEAD; still a DoS / contract
  gap).

Dependencies and interfaces exist. No upstream crate release is required.

## 3. Current implementation evidence

- `crates/eggfetch-core/src/transport/http3.rs` — `route_dial_error` and the
  connect-path override format `QUIC handshake failed: {}` with
  `failure.message()` for Connection; Authentication / Rejected / Other map
  to `Error::CustomTransport` preserving the dial error (same as stream
  `Dialer`).
- `crates/eggfetch-core/src/transport/datagram.rs` — `try_send` refuses
  foreign destinations; Quinn-facing I/O uses fixed `ROUTE_IO_TEXT`;
  `recv_worker` does not compare provider source addresses to `peer_addr()`.
- `docs/architecture/threat-model.md` § Malicious Datagram Route documents
  fixed Quinn I/O text and typed classification.

## 4. Invariants that must not regress

- Public `DatagramDialer` / `DatagramRoute` shapes stay frozen; no new public
  helpers beside the existing dialer surface.
- Direct H3 (no datagram dialer) behavior unchanged.
- `Http3Only` still fails closed when a dialer is set and the route fails;
  no silent direct-UDP fallback.
- Authentication / Rejected / Other kinds may still surface as
  `CustomTransport` with provider text (embedder evidence path), matching
  stream `Dialer` — document that consciously; do not silently broaden
  redaction of those kinds without a paired Dialer change.
- Bounded queues, fixed peer, SNI/cert on logical origin unchanged.
- H3 remains experimental; no Python/CLI/FFI/Node exposure of the dialer.
- Workspace `unsafe_code = forbid` in core.

## 5. Scope

### In scope

- Sanitize Connection-kind (and any other kind currently mapped into
  `H3Connect` with provider text) to fixed, non-provider strings; keep
  typed kind / metrics behavior.
- Enforce fixed-peer recv: datagrams not from `peer_addr()` are rejected
  without feeding Quinn (log/metrics only if an existing counter fits;
  no new public metric required).
- Unit/integration tests under `http3` proving (a) malicious/careless
  Connection messages never appear in `H3Connect` Display/Debug, and
  (b) off-peer recv is dropped and the client stays usable or fails closed
  without panic.
- Threat-model and ADR-0007 cross-check prose if wording drifts.

### Explicitly out of scope

- Fixing or working around h3 #338 / #339.
- Changing `CustomTransport` / stream `Dialer` redaction policy.
- Graduating H3; enabling datagram dialer on non-Rust adapters.
- Ledger/lockfile hygiene (M012).
- New CI jobs or Stage C schema changes.

## 6. Required production changes

Expected in `eggfetch-core` only (`transport/http3.rs`,
`transport/datagram.rs`, and tests). No `ResponseBody` change. No public
API oracle delta unless an existing error string is part of a frozen
contract (prefer fixed strings already used elsewhere).

## 7. Ordered work packages

1. Inventory every path that builds `Error::H3Connect` from dialer failures;
   replace provider message interpolation with fixed text keyed by
   `DialErrorKind` where Connection (and siblings on that path) apply.
2. Add bridge-side recv peer check before enqueue/to-Quinn; define behavior
   on mismatch (drop + continue vs terminate route) and stick to one.
3. Tests: provider that returns Connection with a credential-like message;
   provider that yields foreign-source datagrams.
4. Align threat-model wording with the chosen CustomTransport exception.
5. Tier 1; renew exact-SHA Stage C because executable inputs change.

## 8. Failure, cancellation, timeout, and pool semantics

Dial failures remain non-retryable at the transport layer unless existing
classification already says otherwise. Recv drops must not hang the route
worker or leak queue capacity. Pool/cache caps stay at 64.

## 9. Compatibility and migration (incl. feature-profile effects)

Rust-only, `http3` feature. Callers that parsed `H3Connect` strings for
provider diagnostics will see fixed text — that is intentional. HTTPX
facades unchanged. Profile default features unchanged.

## 10. Required tests

- Connection-kind dial error with embedded secret/URL never appears in
  `format!("{e}")` / `Debug` for `H3Connect`.
- Foreign-peer recv never reaches a successful QUIC accept of attacker
  plaintext; route remains bounded.
- Existing `h3_datagram_route` / hardening suites still pass.

## 11. Required verification commands

```bash
./scripts/check.sh
cargo test -p eggfetch-core --all-features --test h3_datagram_route -- --test-threads=1
cargo test -p eggfetch-core --all-features http3 -- --test-threads=1
# After freeze: Tier 2/3 + ./scripts/check_security.sh and Stage C rebind
```

## 12. Documentation updates

- `docs/architecture/threat-model.md` § Malicious Datagram Route if needed.
- Architecture H3 datagram section only if behavior notes change.
- No Stage C SHA hardcoding outside canonical qualification records.

## 13. Acceptance criteria

- Audit High (H3Connect provider text) and Medium (recv source) closed with
  tests.
- Threat model and implementation agree on what is and is not redacted.
- Stage C rebound after the executable change.
- H3 still experimental.

## 14. Stop conditions

- Any change that requires exposing Quinn types or widening the public
  dialer surface.
- Discovery that fixing recv check breaks a supported embedder contract
  that relied on lying about peer (document and escalate; do not silently
  waive).

## 15. Closure evidence required

- Requirement-to-evidence matrix for both findings.
- Tier 1 green; exact-SHA Stage C renewal recorded in the live ledger.
- Closure record under `plans/closure/tls-proxy-protocols/`.

## 16. Handoff notes

Security audit was report-only; this plan is the authorized corrective
work. Do not bundle M012 lockfile/ledger edits into this change set unless
they are docs-only on the same freeze (prefer separate commits).
