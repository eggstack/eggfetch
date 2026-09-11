# HTTP/3 Production Qualification Evidence

Date: 2026-09-11
Frozen executable SHA: `639bf186a71c054e11278d1b160ffe7a6f172c02`
Decision: **experimental retained**

Routine remote CI: GitHub Actions runs `34578376515` and `34579165931` passed
on documentation/profile/ledger-only descendants `c053fe0` and `a5237f8` of
the frozen executable SHA. No direct workflow event was emitted for the
intermediate freeze commit because the commits were pushed in fast-forwards;
the descendant audit proves executable identity.

This is the final-plan evidence record for the frozen executable tree. It
separates evidence that was executed from evidence that was unavailable. A
passing local Quinn fixture or an unavailable external implementation does
not satisfy the independent HTTP/3 graduation gate.

## Prerequisite closure audit

The two child plans were audited without silently waiving their unchecked
items:

- `http3-independent-interop-and-impairment-qualification.md`: deterministic
  local controls pass; independent servers, independent drain/reset, public
  origins, and the full impairment matrix remain blockers.
- `http3-upstream-risk-resource-and-observability-hardening.md`: the resolved
  dependency set, deterministic lifecycle/resource/diagnostic coverage, and
  EggFetch-owned Alt-Svc fuzz target are recorded; open upstream correctness
  risk, long external soak, and platform breadth remain blockers.

## Executed evidence on the frozen tree

| Evidence | Result |
| --- | --- |
| H3 hardening | `12/12` passed |
| Alt-Svc discovery/fallback/draining | `16/16` passed |
| H3 interop qualification controls | `20/20` passed; Quinn/h3 loopback only |
| Tier 1 | passed |
| Extended tier | passed; Node native artifact and Rust 1.80 MSRV were explicit optional skips |
| Package tier | passed; crate package, wheel smoke, and content checks passed |
| Full compatibility | three consecutive runs, each `1870 passed`, `26 warnings`, zero skips/xfails |
| API oracles | HTTPX 0.28.1: `71` allowed, zero stale/unexplained; httpx2 2.12.0: `79` allowed, zero stale/unexplained |
| Downstream portfolio | `4/4` required packages passed |
| Local resource/lifecycle | deterministic H3 resource and lifecycle controls passed |
| Diagnostics/metadata | exact transport counters and truthful unavailable H3 response metadata passed |

## Missing or blocked evidence

- No pinned, authenticated adapters or immutable server identities were
  available for two independent non-Quinn implementations. The machine
  runner therefore recorded `0/2`, not a pass.
- No controllable independent server was available for GOAWAY/drain/reset or
  restart qualification.
- The public-origin ledger remains empty. The local `curl` build has no HTTP/3
  protocol support, and no volatile origin was promoted to routine evidence.
- The impairment coordinator was run without a namespace/netem runner: all 14
  required matrix scenarios were explicitly `unsupported`. Local blackhole,
  restart, early-close, cancellation, and address-capability controls passed.
- `cargo-fuzz` was not installed; the existing Alt-Svc fuzz target and
  deterministic adversarial tests were inspected, while no fuzz smoke result
  is claimed.
- Runtime qualification was performed on Linux aarch64 only. macOS,
  Windows, and other architectures remain unexecuted for H3 and are not
  included in a support claim.

## Upstream dependency and risk ledger

Resolved versions from `Cargo.lock`:

| Component | Version / feature | Disposition |
| --- | --- | --- |
| `h3` | `0.0.8`, unstable third-party-backend feature | graduation blocker remains; re-audit on any bump |
| `h3-quinn` | `0.0.10` | retained; no upgrade benefit demonstrated before freeze |
| `quinn` | `0.11.11` | retained; no upgrade benefit demonstrated before freeze |
| `quinn-proto` / `quinn-udp` | `0.11.16` / `0.5.15` | retained; the known pre-0.11.14 Quinn DoS issue is not present |
| `rustls` / `tokio-rustls` / `tokio` | `0.23.41` / `0.26.4` / `1.52.3` | retained and covered by repository gates |

The open [h3 buffered-data-on-connection-close issue](https://github.com/hyperium/h3/issues/338)
is capable of affecting ordinary response completion in the pinned h3 frame
layer. No EggFetch workaround or reachability proof was established, so it
blocks promotion. Related open h3 cancellation/reset work remains a risk
review item; no local workaround converts protocol failure into success.

The [Quinn advisory](https://github.com/quinn-rs/quinn/security/advisories/GHSA-6xvm-j4wr-6v98)
was checked against the resolved graph; `quinn-proto 0.11.16` is beyond the
affected versions.

## Decision

The parent gate is not satisfied. HTTP/3 remains experimental for ordinary
request/response operation. This is not a compatibility failure: both HTTPX
profiles were renewed independently on the frozen executable tree, while H3
graduation remains blocked by the missing external evidence and open upstream
correctness risk above.

Any future promotion requires a new exact executable freeze and fresh focused
and compatibility qualification after the independent-server, impairment,
public-origin, and upstream-risk blockers are closed.
