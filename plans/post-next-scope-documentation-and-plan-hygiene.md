# Post-Next-Scope Documentation and Plan Hygiene

Planning baseline: `4456680361dbfdc2d15b0d43a18cdcb20704f667` (`main`, 2026-09-10)
Parent program: `plans/http3-and-next-httpx-compatibility-program.md`
Depends on: successful completion of `post-next-scope-compatibility-requalification-and-closure.md`
Change class: documentation/profile navigation only after the final executable freeze

## Objective

Perform the documentation truth pass only after exact-SHA qualification is complete. Reconcile HTTP/3's final support status, the two versioned compatibility contracts, HTTPX 1.0 preview status, and plan/roadmap navigation without changing executable/test/build/validation/package files.

If this pass discovers an executable correctness defect, stop; reopen executable work and repeat final requalification rather than hiding the defect in prose.

## 1. Bind the docs pass to the final evidence

Record:

- `FROZEN_EXECUTABLE_SHA`;
- HTTPX 0.28.1 qualification stage/SHA;
- HTTPX2 2.12 qualification stage/SHA;
- HTTP/3 graduation outcome;
- HTTPX 1.0 preview version/status.

Acceptance:
- [ ] every compatibility/support claim names or links to the authoritative profile/ledger rather than copying volatile test counts broadly.

## 2. HTTP/3 documentation truth pass

Reconcile at minimum:

- `README.md`;
- `AGENTS.md`;
- `docs/architecture/core-tls-proxy-protocols.md`;
- `docs/architecture/overview.md`;
- `docs/architecture/core-timeout-pool.md` where routing/limits changed;
- `docs/reference/feature-matrix.md`;
- Rust/Python guides and troubleshooting where H3 configuration is user-visible.

Document exactly:

- Alt-Svc discovery and cache rules;
- Auto vs `Http3Only` behavior;
- safe fallback and request replay boundaries;
- suppression/backoff semantics;
- proxy precedence;
- GOAWAY/draining behavior;
- observable QUIC metrics;
- supported platforms;
- retained advanced-feature exclusions.

If graduation failed, retain experimental labels consistently and name the blocking evidence. If it succeeded, remove experimental wording only for the qualified ordinary HTTP/3 surface.

## 3. Compatibility documentation truth pass

Clearly separate:

### HTTPX 0.28.1
- `eggfetch.compat.httpx`;
- exact pinned version;
- qualified Python/runtime scope;
- retained differences;
- current exact-SHA evidence.

### HTTPX2 2.12.0
- `eggfetch.compat.httpx2` (or final chosen import path);
- independent profile/stage;
- new surface such as FunctionAuth/Origin/QUERY/SSE/optional WS as actually implemented;
- Python-version/package limits;
- retained intentional differences.

### HTTPX 1.0
- preview/reconnaissance only;
- exact observed dev/RC version;
- no parity/qualification claim;
- trigger for opening a stable implementation program.

Acceptance:
- [ ] no table collapses HTTPX and HTTPX2 into a single ambiguous “HTTPX parity” claim;
- [ ] HTTPX 1.0 preview cannot be mistaken for supported compatibility.

## 4. Architecture documentation

Explain the implementation boundary so future work does not create duplicate stacks:

- core owns all HTTP/H3 network I/O;
- Alt-Svc/fallback belongs to transport/routing policy;
- SSE is application framing over streamed HTTP responses;
- WebSocket framing owns an upgraded stream returned by core;
- shared compatibility helpers may serve multiple profiles, but profile-specific semantics remain explicit.

Update module maps if files moved/factored during HTTPX2 work.

## 5. Security and lifecycle documentation

Update security/troubleshooting material for:

- Alt-Svc origin authentication and alternative authority handling;
- fallback/replay safety;
- WebSocket proxy/SOCKS/TLS behavior;
- SSE/WS buffer limits;
- decompression/multipart hardening differences;
- credential redaction across new diagnostics.

## 6. Roadmap and plan index

Update `plans/README.md` so this program becomes completed and no child remains accidentally active. Update `plans/ROADMAP.md` current-product position with the final H3/compatibility status.

Keep completed plan files in place as historical records. Do not create a second archive/navigation system.

## 7. Documentation validation and descendant audit

Run existing documentation/Tier 1 validation appropriate for docs-only descendants. Then compare the final frozen executable SHA to HEAD and verify every post-freeze change is documentation/profile/ledger-only.

Do not modify `scripts/check.sh` or tests to make docs pass; that would invalidate the qualification freeze.

## Non-goals

- implementation fixes;
- qualification evidence collection;
- release publication;
- new docs site tooling;
- archival churn of historical plan files;
- changing compatibility stages during prose cleanup.

## Exit criteria

- [ ] README/guides/architecture accurately describe final H3 behavior;
- [ ] HTTPX 0.28.1 and HTTPX2 2.12 contracts are unmistakably separate;
- [ ] HTTPX 1.0 remains clearly preview-only;
- [ ] plan index/roadmap reflect closure;
- [ ] docs validation passes;
- [ ] descendant audit proves no post-freeze executable drift.