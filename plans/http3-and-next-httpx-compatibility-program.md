# HTTP/3 and Next HTTPX Compatibility Program

Planning baseline: `4456680361dbfdc2d15b0d43a18cdcb20704f667` (`main`, 2026-09-10)
Current qualified HTTPX executable SHA: `d034a1005857a7f403222dda4bda5f2f204a44fe`
Current qualified reference: `httpx==0.28.1`

## Objective

Open the next scoped development program after post-audit maturation. The program has three independent goals:

1. move HTTP/3 from a functional experimental transport toward a production-graduation decision;
2. add a separately versioned compatibility target for the actively maintained `httpx2==2.12.0` line without mutating the existing HTTPX 0.28.1 contract;
3. track the original HTTPX 1.0 development line without committing EggFetch to unstable pre-release APIs.

This program is intentionally narrower than a generic feature-expansion roadmap. It reuses the current single-engine architecture, validation tiers, compatibility oracle, and exact-SHA qualification process.

## Research basis and current-code findings

### HTTP/3

The current core already has a real Quinn/h3 transport, bounded per-origin connection cache, connection coalescing, multi-address fallback, phase-aware timeout mapping, stream limits, trailer support, and transport metrics. The missing production-facing pieces are not another connection-cache rewrite. They are discovery/fallback/draining semantics and broader interoperability evidence.

Current repository searches show no Alt-Svc implementation and no explicit HTTP/3 GOAWAY/draining path. `Http3Only` and `Auto { allow_http3: true }` select the H3 route directly; the default does not discover H3 from HTTPS responses.

Upstream risk remains material: EggFetch currently depends on `h3 = 0.0.8`, `h3-quinn = 0.0.10`, and Quinn 0.11. Hyper's HTTP/3 integration remains unfinished and the h3 ecosystem still carries active correctness/interoperability work. Production graduation must therefore be evidence-driven rather than label-driven.

### Compatibility lineage

There is no stable original `httpx` release newer than 0.28.1 at this baseline. Original HTTPX has resumed a `1.0.dev*` redesign; `1.0.dev6` is a pre-release and is not an appropriate exact-SHA Stage C target.

Pydantic's `httpx2` is a maintained fork originating from HTTPX 0.28.1. `httpx2==2.12.0` is stable and has accumulated concrete public/API behavior changes including `FunctionAuth`, `Origin`/`URL.origin`, `QUERY`, header merge operators, SSE, optional WebSockets, trust-store changes, proxy/no-proxy fixes, decompression hardening, multipart validation, and WSGI fixes.

The existing API-manifest generator is already package-parameterized. The current `compat/httpx/0.28.1/` profile/ledger structure should be generalized and reused rather than duplicated as a second incompatible evidence system.

## Architectural invariants

- All network I/O remains in `eggfetch-core`.
- No alternate Python networking engine may be introduced for SSE, WebSockets, HTTPX2, or H3 fallback.
- Existing `eggfetch.compat.httpx` 0.28.1 semantics remain version-pinned and must not be silently changed to HTTPX2 semantics.
- New compatibility profiles are additive and independently qualified.
- Native Rust APIs remain idiomatic; facade-specific quirks belong in version-specific adapters unless they represent generally useful core behavior.
- Existing Tier 1/2/3 validation and qualification mechanisms are extended where necessary; do not add new CI matrices/workflows or evidence formats merely for this program.
- No compatibility claim is broader than direct oracle/differential evidence.

## Qualification rule

The current HTTPX 0.28.1 Stage C claim remains valid for its frozen executable SHA until qualification-sensitive implementation begins. Any source, test, dependency, compatibility-script, validation-script, manifest, or packaging change in this program invalidates the claim for the new executable tree.

Therefore all executable/test/validation work must complete before the final freeze. The final closure plan requalifies HTTPX 0.28.1 and, if its independent acceptance gates pass, qualifies HTTPX2 2.12.0 on the same frozen executable SHA. Documentation-only descendants may follow afterward.

## Ordered implementation plans

1. `http3-alt-svc-discovery-fallback-and-draining.md`
2. `http3-interoperability-and-production-graduation.md`
3. `httpx2-2.12-profile-and-delta-baseline.md`
4. `httpx2-2.12-core-facade-parity.md`
5. `httpx2-2.12-sse-and-websocket-parity.md`
6. `httpx-1.0-preview-tracking.md`
7. `post-next-scope-compatibility-requalification-and-closure.md`
8. `post-next-scope-documentation-and-plan-hygiene.md`

Plans 1–6 may change executable/test/validation files and must finish before the final freeze. Plan 7 is the exact-SHA evidence/closure gate. Plan 8 is documentation-only after qualification.

## Program-level acceptance criteria

- [ ] H3 Auto mode has explicit, tested discovery/fallback semantics rather than assuming QUIC availability.
- [ ] H3 connection shutdown/draining and failure suppression do not create hidden request replay.
- [ ] H3 interoperability evidence spans independent implementations and realistic failure conditions.
- [ ] HTTP/3 is promoted only if the graduation plan's objective bar passes; otherwise the experimental label remains with concrete blockers recorded.
- [ ] `compat/httpx/0.28.1/` remains an independent historical/current contract and is not rewritten to mimic HTTPX2.
- [ ] a versioned HTTPX2 2.12 profile, API oracle baseline, differential cases, and implementation exist for the declared target surface.
- [ ] SSE and optional WebSocket behavior reuse EggFetch streaming/upgraded-stream infrastructure rather than creating a second HTTP stack.
- [ ] HTTPX 1.0 pre-release tracking produces a delta/risk record but no Stage C/public parity claim.
- [ ] final exact-SHA qualification renews HTTPX 0.28.1 and records the independent HTTPX2 result.
- [ ] normal Tier 1 CI remains the only routine CI architecture unless separately authorized.

## Explicit non-goals

- HTTP/3 0-RTT in the first graduation milestone;
- WebTransport, H3 datagrams, MASQUE/CONNECT-UDP, or QUIC connection migration as graduation blockers;
- replacing Quinn/h3 solely to chase feature count;
- changing default protocol selection before discovery/fallback evidence exists;
- treating HTTPX 1.0.dev* as a stable compatibility contract;
- deleting or repurposing the HTTPX 0.28.1 profile;
- browser/Pyodide networking in EggFetch core;
- new CI/release automation unrelated to the scoped work.

## Exit condition

This program closes only after the final exact-SHA requalification/closure plan and documentation truth pass are complete. A retained HTTP/3 experimental designation is an acceptable successful outcome if the graduation evidence exposes upstream or interoperability blockers; hiding those blockers to claim stability is not.