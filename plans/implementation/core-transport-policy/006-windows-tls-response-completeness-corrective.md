# Core Transport Policy Milestone 006 — Windows TLS response completeness corrective

Status: ready

Repository baseline: `c0ff57918af945755e2e595cd91276e81e7bb8ff`

Source roadmaps:

- `plans/subsystems/core-transport-policy-roadmap.md` — response-body lifecycle,
  Hyper client construction, custom dialers, and transport ownership.
- `plans/subsystems/tls-proxy-protocols-roadmap.md` — TLS transport/trust
  boundary consumed by this corrective.
- `plans/002-long-term-roadmap.md` Phase 3 — bounded corrective maintenance.

Long-term requirements:

- `plans/000-long-term-specification.md` — single engine, truthful streaming,
  bounded lifecycle, supported-platform correctness.
- `plans/001-terminology-and-domain-model.md` — transport vs response-body
  ownership and exact-SHA qualification vocabulary.

Applicable ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0003-exact-sha-stage-c-binding.md`
- `plans/adrs/ADR-0004-proxy-fallback-total-deadline.md`
- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`

Primary class: invariant / corrective

## 1. Objective

Investigate and correct a deterministic Windows-only HTTPS response-body
truncation observed by EggReplay while using `eggfetch-core 0.2.0`.

The corrective must determine the actual losing layer before changing
production code:

- local TLS origin / TLS shutdown semantics;
- `tokio-rustls` / rustls Windows read behavior;
- Hyper / hyper-util H1 framing over the EggFetch direct connector;
- EggFetch transport wrappers;
- EggFetch `ResponseBody` timeout/decode/lease adapters.

Do not assume the bug is EggFetch-owned merely because the public EggFetch
request reports the failure.

If EggFetch owns the defect, fix it without threshold-specific or
Windows-specific body truncation workarounds and requalify the exact SHA.
If the defect belongs to an upstream crate or the reproducer's TLS teardown,
record that evidence explicitly and do not patch around it.

This corrective blocks the pending 0.2.x publication and Python 3.15 release
rehearsal until disposition is complete.

## 2. Why this milestone is ready

EggReplay M013F produced a deterministic cross-repository reproducer on hosted
Windows runners.

Latest downstream diagnostic evidence:

- EggReplay commit
  `7cae08f4c4002aad3f7dd39acecda956b49f6b3b`;
- Actions run `36167771359`;
- Windows interception and ordinary Rust verification jobs fail;
- Linux/macOS, dependency-boundary, and Python lanes pass.

The failure reproduces **without MITM/proxy recording** using a plain EggFetch
client and the default TCP/TLS path.

For a 128 KiB response body the local origin reports writing every byte, while
EggFetch fails while reading the body. The observed successful body prefix is
130,954 bytes; together with the test response head this lands at an apparent
128 KiB transport boundary. Earlier 300 KiB runs failed near an apparent
256 KiB total-response boundary.

The same failure reproduces with the proxy/MITM path and the recorded flow
truthfully contains the same partial body plus an upstream body error. That
strongly exonerates EggReplay storage/MITM logic and EggServe.

There is no existing EggFetch plan for this exact Windows TLS response
completeness defect.

## 3. Current implementation evidence

Current ownership relevant to the defect:

- Hyper/hyper-util owns HTTP/1 framing and pooled ordinary response bodies.
- EggFetch direct/custom-dialer routes feed async IO into Hyper.
- rustls/tokio-rustls owns TLS record processing.
- EggFetch wraps transport response bodies in its frozen `ResponseBody`
  model, body timeout/deadline adapter, optional decompression, and pool lease.
- `Response::bytes()` / `text()` collect from that stream; a body error is
  surfaced rather than silently returning a short body.
- `ResponseBody` public variant shapes are frozen by ADR 0005.
- Existing proxy-tunnel code already distinguishes a complete declared body
  followed by abrupt TLS EOF from a genuinely truncated declared body; this
  corrective must not accidentally regress that contract.

The downstream reproducer currently uses a local rustls origin that proves
`write_all` completion but does not yet, by that fact alone, prove identical
TLS `close_notify` semantics across operating systems. That must be tested
before production changes.

## 4. Invariants that must not regress

1. A response with `Content-Length: N` must never be reported as successful
   with fewer than N decoded wire-body bytes.
2. A complete declared body followed by connection teardown must remain
   consumable when HTTP framing has already completed.
3. A genuinely truncated declared body must remain an error.
4. Chunked and close-delimited response semantics must remain distinct from
   fixed-length semantics.
5. `ResponseBody` public variants remain unchanged.
6. The single Hyper-based H1/H2 engine remains authoritative; do not add a
   second HTTP parser/client.
7. Read/total timeout ownership remains in the existing response-body deadline
   path.
8. Pool permits are released exactly once on EOF, error, timeout, or drop.
9. Direct, custom-dialer, and proxy routes must not diverge silently on body
   completeness.
10. No platform-specific "accept short body" behavior is allowed.
11. Existing TLS verification/SNI/custom-CA semantics remain unchanged.
12. Stage C exact-SHA binding is invalidated by executable changes and must be
    renewed before release work resumes.

## 5. Scope

### In scope

- Hermetic Windows reproducer in EggFetch itself.
- Graceful vs abrupt TLS-origin shutdown matrix.
- HTTP vs HTTPS controls.
- Fixed-length, chunked, and close-delimited response controls.
- Buffered `bytes()/text()` and streaming `bytes_stream()` consumption.
- Default/direct connector and custom-dialer paths.
- Layer-isolation probes below and above Hyper.
- Owned production fix if the fault is inside EggFetch.
- Dependency bump if a proven upstream rustls/tokio-rustls/Hyper bug is already
  fixed in a compatible version and the bump is the narrowest safe remedy.
- Exact-SHA requalification and downstream EggReplay confirmation.

### Explicitly out of scope

- EggReplay interception changes.
- Changing response-size qualification thresholds to hide the failure.
- Windows-only body-size caps.
- Disabling TLS close validation globally.
- New public response-body variants or compatibility surface.
- New CI matrices/workflows unless separately approved under
  `docs/verification-policy.md`.
- HTTP/3 work.
- Proxy feature expansion.
- Performance tuning unrelated to the defect.
- 0.2.x publication while the corrective is unresolved.

## 6. Required production changes

Production changes are evidence-dependent.

### 6.1 First prove the fault layer

No production edit is permitted until the isolation matrix in §7 identifies
the first layer that loses or rejects bytes.

### 6.2 If EggFetch-owned

Fix the narrowest owning component. Candidate ownership surfaces include:

- direct/custom connector IO wrapping;
- Hyper client construction;
- response-body stream adaptation;
- timeout/deadline wrapper;
- EOF/error classification after HTTP framing completes.

The fix must be semantic, not threshold-based. No constants matching 64 KiB,
128 KiB, 256 KiB, or observed downstream byte counts may be introduced as a
correctness workaround.

### 6.3 If dependency-owned

If raw tokio-rustls or a minimal Hyper client reproduces independently:

- identify the exact upstream crate/version and issue/fix;
- verify a compatible fixed version on Windows;
- update only the needed dependency floor/pin;
- run the full EggFetch qualification;
- do not reimplement rustls/Hyper internals locally.

If no compatible upstream fix exists, leave this milestone blocked or
conditionally closed with an explicit Windows support limitation. Do not
publish the current support claim as though the defect were resolved.

### 6.4 If fixture teardown-owned

If proper TLS `close_notify` makes the reproducer fully pass and EggFetch's
behavior matches rustls/Hyper contract:

- do not change production transport code;
- add a regression fixture that distinguishes graceful close from intentionally
  abrupt TLS EOF;
- document the exact abrupt-close contract;
- verify whether EggReplay's local origin helper must be corrected downstream.

The closure must show why this is not an EggFetch product defect.

## 7. Ordered work packages

### WP1 — Import a minimal hermetic reproducer

Add a focused EggFetch test/qualification fixture using only local loopback.

Use a generated CA/server certificate and HTTP/1.1 TLS origin.

Exercise fixed response-body sizes around suspected boundaries, including at
least:

- 64 KiB;
- 128 KiB minus headroom;
- exactly 128 KiB body;
- 128 KiB plus one transport chunk;
- 256 KiB;
- 300 KiB.

The test must record:

- declared `Content-Length`;
- response-head byte length;
- body bytes produced by the origin;
- body bytes observed by the consumer;
- final error classification.

Do not assert on platform-specific magic byte counts as expected behavior.

### WP2 — TLS shutdown matrix

Run every fixed-length boundary case with both:

1. **graceful TLS shutdown** — server flushes and performs async TLS shutdown so
   rustls emits `close_notify`;
2. **abrupt transport close** — server writes/flushed response bytes then drops
   without `close_notify`.

Also include a control where the server keeps the TLS connection alive after
the full response and the client closes after consuming exactly
`Content-Length`.

This matrix decides whether the failure is response consumption or teardown.

### WP3 — HTTP/TLS and framing controls

For the same payload sizes:

- plain HTTP/1.1 fixed-length;
- HTTPS fixed-length;
- HTTPS chunked;
- HTTPS close-delimited where supported by the existing client contract.

If only HTTPS fails, keep the investigation in TLS/connector ownership.
If plain HTTP also fails, move the fault boundary upward toward Hyper/body
adaptation.

### WP4 — Consumer-mode controls

Exercise the same response through:

- `Response::bytes()`;
- `Response::text()` for ASCII payloads;
- `Response::bytes_stream()` collected by the test;
- raw/encoded stream path when compression features are enabled, with
  compression disabled in the baseline reproducer.

If streaming receives complete bytes but buffering fails, investigate body
collection/adaptation. If both fail at the same boundary, investigate below
that layer.

### WP5 — Connector controls

Exercise:

- normal direct client;
- caller-supplied plain TCP custom dialer;
- existing pinned/static destination path if it shares the same connector;
- proxy route only as a secondary regression after the direct defect is
  understood.

The direct and custom-dialer cases must agree on completeness.

### WP6 — Below-Hyper isolation

Build test-only probes; do not create production alternatives.

Probe A: tokio-rustls client reads the local TLS origin directly into a bounded
buffer until the declared body is complete.

Probe B: minimal Hyper/hyper-util H1 client over the same
`TokioIo<tokio_rustls::TlsStream<_>>` consumes the response body without
EggFetch `ResponseBody`, timeout, decompression, or lease wrappers.

Interpretation:

- Probe A fails: rustls/tokio-rustls/fixture boundary.
- Probe A passes, Probe B fails: Hyper/hyper-util or IO adapter boundary.
- Probe B passes, EggFetch fails: EggFetch body/deadline/lease layer.
- All upstream probes pass only with graceful close: teardown semantics are the
  controlling factor.

Keep these probes test-only and small.

### WP7 — Implement the narrow correction

After WP1–WP6, fix only the proven owner.

Add a red-before/green-after regression that would have caught the issue
without relying on EggReplay.

### WP8 — Cross-platform regression

Run the new suite on:

- native Windows x86_64 — mandatory;
- Linux x86_64;
- macOS on an available supported architecture.

Linux/macOS are regression controls; Windows is the defect closure platform.

Do not add a permanent routine CI matrix solely for this corrective without
explicit verification-policy approval.

Accepted Windows evidence sources:

- maintainer-run native Windows Tier-1 commands with captured toolchain/commit;
- an existing approved workflow surface that can execute the focused test
  without adding a new automatic workflow;
- downstream EggReplay hosted Windows confirmation as **supplemental** evidence,
  not the only upstream proof.

### WP9 — Renew qualification

Any executable EggFetch change invalidates the live Stage C binding.

After the fix/disposition:

- Tier 1;
- Tier 2;
- Tier 3/package if publication will resume;
- security preflight before publication;
- exact-SHA Stage C rebinding per ADR 0003;
- compatibility/profile checks;
- public API/semver oracle.

### WP10 — Downstream confirmation

After a fixed EggFetch artifact/version is available, rerun the EggReplay
M013F large-response Windows reproducer without lowering the payload envelope.

The downstream proof must show:

- proxy-less EggFetch direct download completes;
- EggReplay MITM path records and serves the complete body;
- no upstream body-error event;
- both plain-TCP and Eggress dialer paths agree.

Do not require EggReplay to carry a permanent special-case for this bug.

## 8. Failure, cancellation, timeout, and pool semantics

The corrective must preserve existing classifications:

- premature EOF before declared length -> body error;
- complete declared body -> success even if later transport teardown is abrupt,
  when the underlying HTTP/TLS contract has delivered all body bytes;
- read timeout -> read timeout classification;
- total deadline -> total timeout classification;
- early caller drop -> ordinary cancellation/drop;
- pool permit released once and not reused after a connection-level protocol
  error.

A fix must not convert a real truncation into success merely because the peer
closed.

## 9. Compatibility and migration

No public API change is expected.

If a dependency version changes:

- keep MSRV >= project floor compatibility;
- run all supported feature profiles;
- update lockfile and dependency documentation;
- run semver/public-API oracles;
- document whether the change affects wheels or linked native artifacts.

The Python/HTTPX facades must inherit the corrected engine behavior without
adapter-specific code.

The live Stage C binding must point to the new exact SHA before release
publication resumes.

## 10. Required tests

At minimum add:

1. fixed-length HTTPS body completes at 64 KiB;
2. fixed-length HTTPS body completes around 128 KiB total-response boundary;
3. fixed-length HTTPS body completes above 128 KiB;
4. fixed-length HTTPS body completes at/above 256 KiB;
5. graceful TLS close case;
6. abrupt TLS close after complete body;
7. deliberately truncated fixed-length body still errors;
8. keep-alive-after-response control;
9. plain HTTP control;
10. chunked HTTPS control;
11. `bytes()` control;
12. `text()` control;
13. `bytes_stream()` control;
14. direct connector control;
15. custom dialer control;
16. test-only raw tokio-rustls isolation;
17. test-only minimal Hyper isolation;
18. timeout regression;
19. pool/connection reuse regression after successful complete body;
20. no reuse after protocol/body truncation error.

Do not use public Internet.

## 11. Required verification commands

Start with Tier 1:

```bash
./scripts/check.sh
```

Focused Rust commands should include the new Windows completeness fixture and
relevant core transport tests.

Before closure after executable change:

```bash
./scripts/check.sh extended
./scripts/check.sh package
git diff --check
```

Run `./scripts/check_security.sh` at release time, not as routine CI.

On Windows, record at minimum:

```text
rustc --version
cargo --version
cargo test -p eggfetch-core <focused completeness target> -- --nocapture
```

plus the repository Tier-1 equivalent available in that environment.

## 12. Documentation updates

Update only current-authority docs:

- core transport architecture if ownership changes;
- TLS/protocol architecture if a dependency/EOF contract changes;
- residual differences if an external limitation remains;
- verification/qualification ledger for the renewed exact SHA;
- release notes if the fix ships in the next 0.2.x patch.

Do not rewrite historical closure records except factual errata.

## 13. Acceptance criteria

- [ ] EggFetch contains its own deterministic local reproducer.
- [ ] Graceful vs abrupt TLS shutdown is explicitly distinguished.
- [ ] Plain HTTP vs HTTPS isolates the TLS boundary.
- [ ] Raw tokio-rustls and minimal Hyper probes identify the first failing
      layer.
- [ ] No body-size threshold workaround is introduced.
- [ ] Fixed-length responses above the previously failing Windows boundary
      complete on native Windows or fail with a proven external blocker.
- [ ] Genuine premature EOF still errors.
- [ ] Buffered and streaming consumers agree on completeness.
- [ ] Direct and custom-dialer paths agree.
- [ ] Timeouts/pool lifecycle remain correct.
- [ ] No public `ResponseBody` shape changes.
- [ ] Linux/macOS controls remain green.
- [ ] Windows evidence is captured explicitly.
- [ ] Tier 1/2 and required package/API qualification are green after any
      executable change.
- [ ] Stage C exact-SHA binding is renewed.
- [ ] EggReplay hosted Windows reproduction passes at the original large-body
      envelope after the corrected artifact is consumed.
- [ ] Pending release publication/rehearsal gates are not reopened before this
      corrective closes.

## 14. Stop conditions

Stop and do not modify production EggFetch code if any of these is established:

1. The defect disappears solely when the test origin performs correct TLS
   `close_notify`, and raw rustls/Hyper behavior proves EggFetch conforms.
2. Raw tokio-rustls reproduces the defect independently and no compatible
   upstream fix is available.
3. Minimal Hyper reproduces the defect independently and the necessary fix
   belongs upstream.
4. The only proposed remedy is to special-case Windows, lower supported body
   sizes, ignore `Content-Length`, or suppress body errors.
5. A dependency bump would violate MSRV/public compatibility and requires a
   broader roadmap decision.

For stop conditions 2–3, retain the release block until an upstream fix or an
explicit support-policy decision resolves the defect.

## 15. Closure evidence required

Create:

`plans/closure/core-transport-policy/006-windows-tls-response-completeness.md`

Record:

- exact reproducer sizes and response-head lengths;
- graceful/abrupt shutdown matrix;
- HTTP vs HTTPS matrix;
- buffered/streaming matrix;
- raw rustls and minimal Hyper results;
- root-cause ownership verdict;
- implementation/dependency commits;
- Windows toolchain and focused test output;
- Linux/macOS controls;
- Tier 1/2/3 results as applicable;
- renewed Stage C binding SHA;
- public API/semver result;
- downstream EggReplay Windows run and commit;
- residual risks.

Closure status must remain **corrective pass required** or **blocked** while a
known supported-Windows response truncation remains.

## 16. Handoff notes

This plan is intentionally diagnostic-first. The current evidence points toward
the EggFetch/Hyper/rustls receive path, but the TLS-origin teardown contract has
not yet been independently eliminated.

Do not start by editing buffer capacities.

The most valuable first implementation artifact is the standalone EggFetch
reproducer with graceful/abrupt TLS shutdown modes and the raw-rustls/minimal-
Hyper split. Once that verdict exists, the actual production fix should be
small and obvious.
