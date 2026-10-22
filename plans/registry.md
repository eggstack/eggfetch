# eggfetch Active Planning Registry

This file is the compact control surface for active planning. Detailed
requirements and completed history remain in source roadmaps, implementation
plans, `plans/closure/`, legacy plan records, and Git history.

Canonical direction remains in:

- `plans/000-long-term-specification.md`
- `plans/001-terminology-and-domain-model.md`
- `plans/002-long-term-roadmap.md`
- `plans/003-planning-process.md`

Live qualification binding (authoritative, never duplicated here — see the
ledger for SHAs):

- `plans/httpx-parity-correction-status.md` (+ `compat/*/profile.toml`)

Release train: M001/M004 (0.2.1) and M005 (0.2.2) are closed across
crates.io, signed tags, PyPI, and the published GitHub Releases. M005
published the closed M007 capability as coordinated 0.2.2. M001B remains
superseded history. Current public version is 0.2.2 (six crates + PyPI
`eggfetch`).

Validation tiers: `.skills/verification-qualification.md`. Normative policy:
`docs/verification-policy.md`.

## Status vocabulary

- **proposed** — roadmap or plan exists but is not approved for execution.
- **ready** — dependencies and interfaces are satisfied; plan may be handed off.
- **active** — implementation or closure work is in progress.
- **blocked** — a named dependency or evidence requirement prevents progress.
- **closing** — implementation landed and closure evidence is being gathered.
- **closed** — closure record accepted.
- **conditionally closed** — substantial work landed, but a named correctness
  or operational evidence condition remains.
- **superseded** — replaced by another document.
- **archived** — no longer active and retained for traceability.

## Active subsystem roadmaps

| Subsystem | Status | Roadmap | Current milestone | Dependencies or blockers |
|---|---|---|---|---|
| Core transport and request policy | active | `plans/subsystems/core-transport-policy-roadmap.md` | M008, M009 and M010 conditionally closed | All three carry the same remaining condition: the Stage C rebinding plus the frozen-SHA tier gates. The subsystem has no further registered milestone; a new one needs its own plan. |
| Python bindings and HTTPX compatibility | active | `plans/subsystems/python-httpx-compat-roadmap.md` | M004/M005 conditionally closed | ADR-0009 accepted. Both milestones renew the exact-SHA binding; their remaining condition is the Stage C rebinding plus the frozen-SHA tier gates. HTTPX 1.0 remains gated on the upstream trigger. |
| TLS, proxy, and protocols | active | `plans/subsystems/tls-proxy-protocols-roadmap.md` | M005 conditionally closed | ADR-0008 accepted. H3 remains experimental; graduation stays a separately gated future. |
| Release and verification | active | `plans/subsystems/release-verification-roadmap.md` | M006 and M007 conditionally closed | Both renew the exact-SHA binding; their remaining condition is the Stage C rebinding plus the frozen-SHA tier gates. M003 standing corrective intake remains proposed. |
| Performance and footprint | closed | `plans/subsystems/performance-footprint-roadmap.md` | Campaigns closed; fixtures repaired | New optimization needs its own milestone plan. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
| TLS, proxy, and protocols | M005 uncompiled-protocol fail-closed at adapter boundaries | conditionally closed | `plans/implementation/tls-proxy-protocols/005-uncompiled-protocol-fail-closed.md` | `plans/closure/tls-proxy-protocols/005-uncompiled-protocol-fail-closed.md`. ADR-0008 accepted. The CLI `--http2` silent downgrade and the CLI `--http3` misleading error are closed, and the FFI's unguarded strict policy now fails closed. Engine's native-caller downgrade unchanged. Remaining condition: Stage C rebinding plus the frozen-SHA tier gates. |
| Python bindings and HTTPX compatibility | M004 httpx2 URL origin reachability | conditionally closed | `plans/implementation/python-httpx-compat/004-httpx2-origin-reachability.md` | `plans/closure/python-httpx-compat/004-httpx2-origin-reachability.md`. `URL.origin` is reachable from request, response, client, and redirect observables. The 0.28.1 facade gains no `origin` and is byte-for-byte unchanged. Remaining condition: Stage C rebinding plus the frozen-SHA tier gates. |
| Python bindings and HTTPX compatibility | M005 compat policy authority reconciliation | conditionally closed | `plans/implementation/python-httpx-compat/005-compat-policy-authority-reconciliation.md` | `plans/closure/python-httpx-compat/005-compat-policy-authority-reconciliation.md`. ADR-0009 accepted. The two-cookie-engine disagreement is aligned (engine accepts an IP-literal `Domain` only for the exact response host), the redirect credential-rule divergence is measured and recorded, and compat `NO_PROXY` / `socks5h` no longer leak into the native Python surface. Remaining condition: Stage C rebinding plus the frozen-SHA tier gates. |
| Core transport and request policy | M008 profile matrix completion | conditionally closed | `plans/implementation/core-transport-policy/008-profile-matrix-completion.md` | `plans/closure/core-transport-policy/008-profile-matrix-completion.md`. Adds `standard-http2` and `native-http2` to the contract matrix and the public-API oracle; runs the lean behavioral suites under the lean profile. No runtime change. Remaining condition: Stage C rebinding plus the frozen-SHA tier gates. |
| Core transport and request policy | M009 lean path single-sourcing | conditionally closed | `plans/implementation/core-transport-policy/009-lean-path-single-sourcing.md` | `plans/closure/core-transport-policy/009-lean-path-single-sourcing.md`. The lean and redirect paths now share one ungated first-hop preparation builder (`pipeline/hop`); the whole core test suite compiles under the lean profile, gated by Tier 1. The plan's "exactly one exhaustive destructuring" criterion was amended to name the first-hop path; the transport-preparation and subsequent-hop sites remain exhaustive by the plan's own requirement. Remaining condition: Stage C rebinding plus the frozen-SHA tier gates. |
| Core transport and request policy | M010 core duplicate-helper consolidation | conditionally closed | `plans/implementation/core-transport-policy/010-core-duplicate-helper-consolidation.md` | `plans/closure/core-transport-policy/010-core-duplicate-helper-consolidation.md`. Seven duplication clusters consolidated with no runtime and no public-API change: the Rust public-API oracle is byte-identical to the frozen snapshots in all eight profiles. Cluster D adopts the stricter origin rule, which changes nothing observable; the IPv6 pool-key divergence it exposes is recorded, not fixed. Remaining condition: Stage C rebinding plus the frozen-SHA tier gates. |
| Release and verification | M006 verification guard hardening | conditionally closed | `plans/implementation/release-verification/006-verification-guard-hardening.md` | `plans/closure/release-verification/006-verification-guard-hardening.md`. The lint guard now anchors on the attribute's lint list and rejects blanket `allow`/`expect`/`deny` in any form, scans `fuzz/`, and self-tests in Tier 1; the Tier 2 downstream gate fails closed instead of skipping a release-blocking gate; the Tier 2 profile-isolated rows use the same bare `RUSTFLAGS=` treatment as Tier 1; the H3 runner self-test runs first in Tier 1. Measured: zero existing suppressions are newly matched. Remaining condition: Stage C rebinding plus the frozen-SHA tier gates. |
| Release and verification | M007 planning and documentation truthfulness hygiene | conditionally closed | `plans/implementation/release-verification/007-planning-and-documentation-hygiene.md` | `plans/closure/release-verification/007-planning-and-documentation-hygiene.md`. All 2,267 unticked acceptance-criterion boxes under `plans/` are classified (760 met, 250 superseded, 1,257 deferred, 0 never applicable) with no plan file retaining unticked boxes under a completion record without a dated open-items section; `fuzz/Cargo.lock` is refreshed and the fuzz tree builds again; the compat profiles' machine-readable content is byte-identical after their comment log is removed; the two httpx replacement packages now ship one canonical source; every adapter and unowned-core lint suppression carries a `reason =`. Remaining condition: Stage C rebinding plus the frozen-SHA tier gates. |
| TLS, proxy, and protocols | M004 caller-owned H3 datagram routing | closed | `plans/implementation/tls-proxy-protocols/004-caller-owned-h3-datagram-routing.md` | `plans/closure/tls-proxy-protocols/004-caller-owned-h3-datagram-routing.md`. Closed on `42a9c96db5b25354843bd83bd458c71757e95651` with unconditional exact-SHA Stage C. Rust-only, additive API; no Quinn-type exposure and no Eggress dependency. |
| Release and verification | M005 0.2.2 M007 publication | closed | `plans/implementation/release-verification/005-0-2-2-m007-publication.md` | `plans/closure/release-verification/005-0-2-2-m007-publication.md`. Published on candidate `015a56d7ec3edf186eec8ebccff01cbf5584274e`: six crates.io packages, signed `v0.2.2`, 19 PyPI files, GitHub Release `401746856`. |
| Core transport and request policy | M007 native transport failure classification | closed | `plans/implementation/core-transport-policy/007-native-transport-failure-classification.md` | `plans/closure/core-transport-policy/007-native-transport-failure-classification.md`. Published as eggfetch 0.2.2 by release-verification M005; exact-pin downstream consumers may now begin their own adoption milestone. |
| Release and verification | M001 coordinated 0.2.x publication umbrella | closed | `plans/archive/implementation/release-verification/001-release-publication.md` | `plans/closure/release-verification/001-coordinated-0-2-1-publication.md` |
| Release and verification | M001A 0.2.1 release-candidate preparation | closed | `plans/implementation/release-verification/001a-0-2-1-release-candidate-preparation.md` | Closed on original candidate `41757569123c0b8038550b956d8b244ab55094a6`. |
| Release and verification | M002 Python 3.15 wheel rehearsal | closed | `plans/implementation/release-verification/002-python-315-wheel-rehearsal.md` | Run `36223505399` green: 18 wheels + 1 sdist, `publish=false`. |
| Release and verification | M001B 0.2.1 coordinated publication | superseded | `plans/implementation/release-verification/001b-0-2-1-coordinated-publication.md` | Superseded before execution by M004. |
| Release and verification | M004 0.2.1 tagged release finalization | closed | `plans/archive/implementation/release-verification/004-0-2-1-tagged-release-finalization.md` | `plans/closure/release-verification/004-0-2-1-tagged-release-finalization.md` |

## Current execution order and dependency gates

**Release execution gate:** none active. M005 is closed; 0.2.2 is the current
public version across all six crates, PyPI, the signed `v0.2.2` tag, and the
GitHub Release. Historical v0.2.0/v0.2.1 public identities remain immutable.
The next candidate is a fresh bounded corrective or feature plan through M003.

**Executed in the current pass (2026-10-22), in the recorded order:** TLS M005,
python-httpx-compat M004, python-httpx-compat M005, core M008, core M009,
core M010, release-verification M006, release-verification M007. All eight are
**conditionally closed**: the implementation is complete and each milestone's
own targeted gates are green, but each changes executable inputs, so the live
Stage C binding is invalidated and the Stage C rebinding plus a full Tier 1 /
Tier 2 / Tier 3 run on the frozen SHA were deliberately deferred at this stage.
No commit was made; the maintainer reviews the working tree.

**Remaining recommended order:** none of the registered milestones remain. The
four previously queued milestones — core M009, core M010, release-verification
M006, release-verification M007 — have all been executed and are conditionally
closed. What remains is the release-verification **Stage C rebinding and the
frozen-SHA tier gate run**, which are maintainer actions rather than
milestones, plus the deferred future programs in the blocked/operational table
below.

**Core transport gate:** M008, M009 and M010 are conditionally closed. None of
the three changes runtime behavior. M008 changes only validation scripts and
the oracle. M009 extracts the shared first-hop preparation builder into
`pipeline/hop` and gates the test targets per feature-isolated profile, so the
whole core test suite now compiles under the lean and native profiles. M010
consolidates seven duplication clusters with no public-API change — the Rust
public-API oracle is byte-identical to the frozen snapshots in all eight
profiles.

**Python compat gate:** M004 and M005 are conditionally closed. ADR-0009 was
accepted on 2026-10-22. Both renew the exact-SHA binding because they change
executable Python inputs. The HTTPX 1.0 program stays gated on the upstream
trigger.

**TLS/protocol execution gate:** M005 is conditionally closed; ADR-0008 was
accepted on 2026-10-22. M004 closed on
`42a9c96db5b25354843bd83bd458c71757e95651` with an **unconditional** exact-SHA
Stage C binding. It preserved direct H3, exposed no Quinn types, and took no
Eggress dependency. Tier 2's full pinned HTTPX compatibility suite and the
API-manifest comparison are green on `ubuntu-latest` / Python 3.12.14, run
[`37511735210`](https://github.com/eggstack/eggfetch/actions/runs/37511735210)
at `e675b2e7` (Rust tree identical to the freeze): 1934 passed / 0 failed, both
facades 0 unexplained and 0 stale. The earlier conditional closure blamed
`[platform.macos]` vs `platform.system()` plus "a Python 3.11-vs-3.12
`codes.is_integer` artifact"; the latter was wrong — `int.is_integer` exists in
every stock CPython, and the local venv's 3.11.9 build is simply damaged. A
concrete carrier (MASQUE CONNECT-UDP, inter-process) is the natural next
milestone but is unregistered and not implied. Closure did not authorize H3
graduation or publication.

**Gated futures:** HTTPX 1.0 (RC/stable + frozen API + fresh delta), H3
graduation (independent interop/drain/impairment/upstream evidence), and Node
maturation (explicit scope decision) remain unregistered. M004 was an
experimental H3 routing extension, not the H3 graduation program, and its
closure unblocks none of them.

## Pending maintainer actions

| # | Action | Owner | Reference |
|---|---|---|---|
| 1 | Review and commit the four implemented milestones (TLS M005, python-httpx-compat M004/M005, core M008) from the working tree. Nothing is committed; the maintainer owns the commit. | project maintainers | the four closure records under `plans/closure/` |
| 2 | Establish the Stage C binding for the new executable freeze and rebind both HTTPX profiles, then run Tier 1 / Tier 2 / Tier 3 on that frozen SHA. This is the single outstanding condition on all four conditionally closed milestones. | project maintainers | `plans/closure/python-httpx-compat/005-compat-policy-authority-reconciliation.md` §10 |
| 3 | Decide whether the FFI publishes a strict HTTP/3 policy value. The published values are `0`=`Http1Only`, `1`=`Http2Only`, `2`/`3`=`Auto`, so no strict HTTP/3 value exists to reject. The guard path is written and unit-tested; only the ABI value and `docs/ffi/README.md` are missing. | project maintainers | `plans/closure/tls-proxy-protocols/005-uncompiled-protocol-fail-closed.md` §10 |
| 4 | Decide whether the CLI adapter gets an owning subsystem roadmap, or whether CLI work is admitted under an existing subsystem. The CLI's 581-line `run()` decomposition is deferred until this is decided. | project maintainers | `plans/implementation/release-verification/007-planning-and-documentation-hygiene.md` §5 |
| 5 | Decide whether the `tracing` feature is enabled in an adapter, given a test, or explicitly retired. | project maintainers | this registry |
| 6 | Decide whether to authorise a dev-dependency on an HTTP/2 server fixture in `eggfetch-cli`, so the CLI's `--http2` negotiation-success path can be asserted end to end. The rejection path is fully covered today. | project maintainers | `plans/closure/tls-proxy-protocols/005-uncompiled-protocol-fail-closed.md` §10 |

Resolved in the current pass: ADR-0008 and ADR-0009 were accepted on
2026-10-22 on maintainer instruction to execute their dependent milestones.

## Blocked / operational work

| Subsystem | Milestone | Blocker |
|---|---|---|
| TLS, proxy, protocols | H3 graduation | Independent non-Quinn interop, GOAWAY/drain, public-origin, impairment, upstream-risk evidence. |
| Python compat | HTTPX 1.0 program | Upstream RC/stable trigger has not fired. |
| Release and verification | Stage C rebinding for the 2026-10-22 executable freeze | Needs a commit of the eight implemented milestones, then a frozen-SHA qualification run. Blocks publication of anything built on the current tree. |
| Release and verification | Downstream behavioral-fixture artifact | Tier 2's downstream gate now fails closed by design. `target/downstream-qualification/artifact-manifest.json` has no in-repository producer; the operator must supply it. |
| Core transport | IPv6 pool-key divergence | The high-level and native origin paths key the same IPv6 endpoint differently. Closing it changes pool keying, so it needs its own milestone. |

## Deferred work (explicitly not planned)

These were identified in the same repository audit and are recorded here so
they are not silently dropped. Each is deferred for a stated reason, not
because it was overlooked.

| Item | Reason for deferral |
|---|---|
| Moving the 249 flat legacy plan files into `plans/archive/` | Bulk reorganization with no correctness value; the flat set is a documented grandfathered legacy archive. |
| Decomposing the CLI's 581-line `run()` | Polish, and no subsystem roadmap currently owns the CLI adapter. Needs a maintainer ownership decision first. |
| `tracing` feature has no test and no adapter consumer | A capability decision, not hygiene. See pending maintainer action 5. |
| Node adapter has zero automated tests | Registered gated future requiring an explicit scope decision. |
| FFI has no proxy, multipart, custom-CA, UDS, dialer, SNI, trailers, metrics or retry-policy surface | Registered gated future; out of scope for the protocol-capability milestone. |
| FFI `multipart` cargo feature has no reachable C ABI | Would need an ABI surface decision; belongs with the FFI surface milestone. |

## Closure work and current control points

Four milestones were implemented and conditionally closed in the 2026-10-22
pass: TLS/protocol M005, python-httpx-compat M004 and M005, and core
transport M008. Each closure record names its own outstanding condition; all
four share the same one — the Stage C rebinding and the frozen-SHA tier gates,
because each changes executable inputs. Core transport M007 is closed and
published; release-verification M005 is closed with all public identities
recorded; TLS/protocol M004 is implemented under ADR-0007 and conditionally
closed pending the Linux-host compatibility gates. HTTPX 1.0, H3 graduation,
and Node maturation remain gated/unregistered and were not unblocked by M004.
The low-severity compatibility-fixture finding from M005 remains routed to the
standing Phase 3 corrective intake and is independent of M004.

M001A and M002 are closed historical release evidence. M002 run
`36223505399` proved the original candidate's 18-wheel + 1-sdist matrix with
`publish=false`; M004 run `36253723990` renewed the rehearsal on the final
candidate before publication. M004 publish run `36255517733` completed
successfully. All M006-family work is also closed. M005 closed on candidate
`015a56d7ec3edf186eec8ebccff01cbf5584274e` with build-only rehearsal
`36991057953` and PyPI publish run `36994291293`. Stage C is bound to that
candidate unless executable or qualification inputs change.

The four milestones closed in the 2026-10-22 pass all change executable
inputs — engine sources, Python sources, or validation scripts — so each
requires renewed exact-SHA Stage C qualification at its own freeze, and each
invalidates the current binding when it lands. Because none of them is
committed yet, the binding cannot be re-established until the maintainer
commits the tree and a qualification run is executed at that frozen SHA.
