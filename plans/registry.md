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
| Core transport and request policy | closed | `plans/subsystems/core-transport-policy-roadmap.md` | M007 closed and published as 0.2.2; subsystem at steady state | ADR-0006 implemented, qualified, and published. |
| Python bindings and HTTPX compatibility | closed | `plans/subsystems/python-httpx-compat-roadmap.md` | M001-M003 closed on live Stage C binding | Facade work reopens only via gated roadmap Phase 4 trigger. |
| TLS, proxy, and protocols | closed | `plans/subsystems/tls-proxy-protocols-roadmap.md` | M004 implemented and fully evidenced; Tier 2 compat suite (1934 passed) + both API-manifest gates green on `ubuntu-latest`/3.12.14, run `37511735210` | H3 remains experimental; graduation stays a separately gated future. |
| Release and verification | closed | `plans/subsystems/release-verification-roadmap.md` | M005 closed; subsystem at steady state | No active release task. M003 standing corrective intake remains proposed. |
| Performance and footprint | closed | `plans/subsystems/performance-footprint-roadmap.md` | Campaigns closed; fixtures repaired | New optimization needs its own milestone plan. |

## Dependency-ready implementation plans

| Subsystem | Milestone | Status | Implementation plan | Dependencies / handoff note |
|---|---|---|---|---|
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

**Core transport gate:** M007 is closed under accepted ADR-0006 and published
as 0.2.2; the subsystem is at steady state. No further core-transport
implementation is required.

**TLS/protocol execution gate:** none active. M004 closed on
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

## Blocked / operational work

| Subsystem | Milestone | Blocker |
|---|---|---|
| TLS, proxy, protocols | H3 graduation | Independent non-Quinn interop, GOAWAY/drain, public-origin, impairment, upstream-risk evidence. |
| Python compat | HTTPX 1.0 program | Upstream RC/stable trigger has not fired. |

## Closure work and current control points

No implementation handoff is ready. Core transport M007 is closed and
published; release-verification M005 is closed with all public identities
recorded; TLS/protocol M004 is implemented under ADR-0007 and conditionally
closed pending the Linux-host compatibility gates. HTTPX 1.0, H3 graduation, and
Node maturation remain gated/unregistered and were not unblocked by M004.
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
