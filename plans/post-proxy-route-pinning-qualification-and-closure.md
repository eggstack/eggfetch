# Post-Proxy Route Pinning Qualification and Closure

Status: Complete

Date: 2026-09-14

Parent: [`proxy-route-pinning-and-egress-interoperability-program.md`](proxy-route-pinning-and-egress-interoperability-program.md)

Depends on:

- [`pinned-proxy-peer-routing.md`](pinned-proxy-peer-routing.md)
- [`pinned-proxy-destination-routing.md`](pinned-proxy-destination-routing.md)

Baseline: `d681da848e36199846604b71f2162a1054d24aa6`

## Purpose

Close the proxy route-pinning program with evidence that the new native controls are bounded, fail closed, dependency-neutral, compatible with existing direct/proxy behavior, and suitable for unrelated embedders without turning Eggfetch into a routing-policy framework.

This phase should introduce no new architecture except corrections exposed by qualification.

## 1. Freeze the executable surface

After both implementation child plans satisfy their focused tests, identify one executable/test/fixture SHA for qualification. Record:

```text
baseline SHA
implementation/freeze SHA
files changed
public API additions
production dependency delta
feature delta
```

Do not mix unrelated HTTP/3, compatibility-facade, binding, or dependency work into the freeze. If unrelated executable changes land first, rebase/reconcile and establish a new freeze rather than claiming evidence from the earlier tree.

Documentation-only descendants may be used for closure records after the executable freeze, consistent with current repository practice.

## 2. Audit the public API boundary

Verify that the final API preserves the intended model:

```text
Proxy object
  logical proxy URI
  optional physical proxy-peer snapshot

Request
  logical origin URL
  optional physical proxied-target snapshot

existing Request::resolved_target
  direct route only
```

Required findings:

- proxy peer and proxied target cannot be confused by the API or debug output;
- direct `resolved_addresses()` still rejects an effective proxy;
- no new public field is required on `TransportHints` merely to add proxy pinning;
- ordinary callers need no changes;
- Python/CLI/HTTPX facades do not expose unsupported invented reference behavior;
- no downstream-specific policy concept appears in the public API;
- caller responsibility for validating supplied addresses is documented explicitly.

If implementation broadened `ResolvedTarget` documentation or extracted a private shared static-destination representation, verify that source compatibility and existing semantics remain intact.

## 3. End-to-end route-binding matrix

Run deterministic local fixtures that prove actual wire destinations, not merely builder state.

Minimum matrix:

| Proxy route | Proxy peer | Ultimate target | Expected result |
|---|---|---|---|
| HTTP proxy -> HTTP origin | pinned | unpinned | succeeds; proxy TCP peer is pinned |
| HTTP proxy -> HTTP origin | pinned | pinned | rejected before I/O in v1 |
| HTTP proxy -> HTTPS origin | pinned | pinned | CONNECT uses supplied target; destination TLS/Host stay logical |
| HTTPS proxy -> HTTPS origin | pinned | pinned | proxy TLS authenticates proxy host; destination TLS authenticates origin host |
| SOCKS5 -> HTTP origin | pinned | pinned | SOCKS server peer and target IP both pinned |
| SOCKS5 -> HTTPS origin | pinned | pinned | destination TLS identity remains logical |
| SOCKS5H | pinned | unpinned | existing remote-DNS behavior succeeds |
| SOCKS5H | pinned | pinned | rejected before I/O |
| no proxy | n/a | direct `resolved_addresses()` | existing direct static routing unchanged |

For successful pinned cases, assert the peer/destination seen by the local fixture. For rejected cases, assert the listener observes no connection when the incompatibility is detectable before proxy I/O.

## 4. DNS non-fallback proof

Add/retain tests that make logical proxy/origin hostnames intentionally unresolvable while supplied local `SocketAddr` values work.

Prove separately:

- pinned proxy peer performs no proxy-host DNS;
- pinned CONNECT target performs no origin DNS before CONNECT;
- pinned SOCKS5 target performs no origin DNS;
- retries do not introduce DNS;
- same-origin redirects do not introduce DNS;
- unsupported pinned SOCKS5H never silently becomes local DNS or SOCKS5;
- exhausted supplied candidates never fall back to system DNS.

Use counters/test seams where available. Success alone is weaker evidence than an explicit no-resolution assertion.

## 5. Identity and TLS proof

Use local certificates/fixtures to prove the four-way separation:

```text
physical proxy peer != logical proxy host
physical origin target != logical origin host
```

Assertions:

- HTTPS proxy SNI and certificate verification use logical proxy host;
- origin TLS inside CONNECT/SOCKS uses logical origin host;
- HTTP Host uses logical origin authority;
- response URL/history remain logical;
- proxy authentication is sent only on proxy leg;
- cross-origin auth/cookie stripping continues to use logical origins;
- explicit existing SNI/proxy-TLS overrides keep their current precedence.

A test that only disables certificate validation is insufficient evidence for identity preservation.

## 6. Retry, redirect, timeout, and fallback proof

Verify that all physical route snapshots participate correctly in request reconstruction.

### Retry

- same proxy-peer and target snapshots are reused;
- total deadline shrinks across attempts;
- non-replayable bodies retain existing retry rejection;
- terminal `RequestFailure` reflects the final typed failure rather than a stale transient candidate.

### Redirect

- same-origin + same effective proxy may preserve destination state;
- cross-origin pin reuse fails closed;
- redirect into `NO_PROXY` cannot apply proxy peer state to direct transport;
- redirect selecting a different proxy cannot inherit old proxy peers;
- credentials retain current stripping/redaction behavior.

### Candidate fallback

- proxy peer candidates share one proxy-connect budget;
- ultimate CONNECT/SOCKS candidates share the appropriate remaining request/connect budget;
- candidate cycling occurs only for failures classified as safe/address-specific by the implementation;
- no fresh full timeout is granted per candidate.

## 7. Pool/reuse audit

Inspect the exact cache/client/pool keys used by all affected routes and record a disposition for:

```text
HTTP forward proxy
HTTPS CONNECT
HTTPS proxy
SOCKS5
SOCKS5H
```

For every reusable connection/tunnel, either:

1. prove the physical route snapshot participates in compatibility/keying; or
2. prove the route does not reuse such physical connections and therefore cannot cross constraints.

Required adversarial test: issue two requests for the same logical proxy/origin with different pinned address sets and prove the second cannot ride an incompatible connection established by the first.

Do not introduce a new connection cache only to satisfy this audit.

## 8. Error and observability audit

Exercise ordinary `send()` and native `send_detailed()` where applicable.

Check that:

- pre-I/O invalid pinned configuration has a stable typed/error-kind disposition;
- proxy-host DNS classification still occurs only when system resolution was actually attempted;
- proxy-peer connection refusal/connect failure retains typed provenance where supported;
- CONNECT/SOCKS target failure is not mislabeled as proxy DNS;
- proxy TLS and origin TLS failures remain distinguishable through the existing error/source model;
- transient candidate failures do not leak as terminal detailed classification after success/fallback;
- no implementation matches error `Display`/`Debug` strings.

Do not expand the public error enum merely for prettier diagnostics unless a concrete unrepresentable failure requires it and compatibility impact is separately reviewed.

## 9. Egress dependency/reuse closure

Re-run the Egress decision against the implementation rather than copying the planning conclusion blindly.

Expected disposition: **no Eggfetch -> Egress dependency**.

Record the exact Egress SHA reviewed and confirm:

- `eggress-routing` remains a policy/scheduler layer above the HTTP engine rather than a neutral dependency;
- `eggress-uri` would broaden protocol grammar without deleting enough Eggfetch code to justify coupling;
- `eggress-protocol-socks` remains orthogonal to route pinning and would not remove Eggfetch's timeout/metrics/TLS/pool integration burden;
- Eggfetch `Dialer` remains the generic external composition seam for applications that want Egress to own the entire route.

If the implementation unexpectedly duplicates a small neutral primitive that exists in Egress, measure the actual dependency/code delta before revising this disposition. Do not add an umbrella Egress dependency as a convenience.

## 10. Dependency and feature audit

Expected production dependency delta: zero.

Record before/after output for:

```sh
cargo tree -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo tree -p eggfetch-core --no-default-features --features http1,tls-rustls,proxy
cargo tree -p eggfetch-core -e features
cargo tree -p eggfetch-core -d
```

Acceptance:

- minimal non-proxy profile is unchanged by proxy-only implementation;
- no `eggress-*` package appears;
- no second resolver, proxy client, routing framework, or TLS stack appears;
- feature ownership remains truthful;
- proxy pinning stays behind the existing `proxy` feature except for any zero-cost shared address type already required by direct routing.

Do not claim binary slimming. If artifact size is measured, report it as supporting evidence only and compare equivalent feature profiles.

## 11. Repository validation

The normative policy is `docs/verification-policy.md`.

Run Tier 1 on every implementation increment:

```sh
./scripts/check.sh
```

Before program closure, run:

```sh
./scripts/check.sh extended
./scripts/check.sh package
```

Optional prerequisite skips in extended validation must be recorded as skips, not converted into passes.

Because this program changes executable transport behavior, also run the current full compatibility qualification required by the live HTTPX/HTTPX2 profile status. If the repository's exact-SHA Stage C ledger is still active at implementation time, renew it on the frozen executable SHA using the current profile procedure and record the results. Do not cargo-cult historical run counts if the normative compatibility procedure has changed; follow the live profile/status files.

Hosted CI should run through the repository's single normal workflow. Do not add a proxy-route matrix, external proxy service, automatic evidence workflow, or extra CI job; deterministic local fixtures belong in existing tests.

## 12. Documentation closure

Update, as applicable:

```text
docs/architecture/core-tls-proxy-protocols.md
docs/architecture/core-engine.md
docs/rust/guide.md
docs/reference/errors.md
README.md / feature matrix only if public capability summaries need it
plans/README.md
plans/ROADMAP.md only if the live roadmap needs a current-position note
```

Documentation must state clearly:

- physical proxy peer versus logical proxy URI;
- physical proxied target versus logical origin URL;
- direct `resolved_addresses()` remains a separate direct-only API;
- supported CONNECT/SOCKS5 combinations;
- SOCKS5H remote-resolution limitation;
- plaintext HTTP forward-proxy limitation;
- retry/redirect snapshot semantics;
- no-DNS-fallback guarantees;
- pool/reuse constraints;
- caller responsibility for validating supplied addresses;
- Egress relationship: external routing/dialer composition, not a core dependency.

Mark the parent/child plans complete only after the final executable evidence is bound to a specific SHA. Preserve them as historical implementation records afterward.

## Final acceptance criteria

- [x] Both child plans' acceptance criteria are satisfied or any unsupported row is explicitly narrowed/fail-closed.
- [x] Wire fixtures prove proxy-peer and supported ultimate-target physical destinations.
- [x] DNS non-fallback is directly tested for pinned proxy peers and supported pinned targets.
- [x] Proxy and origin logical TLS/HTTP identities remain correct under physical pinning.
- [x] Retry/redirect/candidate timeout behavior is bounded and deterministic.
- [x] Connection reuse cannot cross incompatible physical-route constraints.
- [x] Existing unpinned proxy, direct static routing, dialer, Python/CLI/HTTPX, and HTTP/3 behavior remains compatible.
- [x] Tier 1, extended, package, affected focused suites, and current compatibility qualification are green or truthfully record optional skips.
- [x] No new production dependency appears; specifically no `eggress-*` dependency is present.
- [x] Documentation and plan status match the implementation actually shipped.

## Closure record

Executable freeze: `13c4ab4`.
Documentation-only closure is recorded in the descendant commit that updates
these plan statuses and evidence.

The API audit confirms separate proxy-peer and proxied-target state, no new
`TransportHints` field, unchanged direct-only `resolved_addresses()` behavior,
caller-owned address validation, and no Python/CLI/HTTPX exposure. Local wire
fixtures prove pinned proxy peer/target behavior and logical TLS/HTTP identity;
unsupported SOCKS5H and plaintext-forward combinations fail before proxy I/O.
Retry/redirect reconstruction, timeout-budgeted candidate loops, and route-aware
SOCKS cache keys preserve the snapshots without cross-route reuse. HTTPS CONNECT
candidate fallback is limited to typed 502/504 proxy rejection, while local
SOCKS5 candidate fallback is limited to destination-specific 0x03/0x04/0x05
replies; proxy-wide failures stop. Dependency
and feature review found zero production dependency delta and no `eggress-*`
package. Egress remains an external policy/dialer composition layer.

The dependency audit covered the minimal direct recipe, the proxy-enabled
recipe, the full feature graph, and duplicate-package output from `cargo tree`.
The production graph remains on the existing Hyper/Rustls/Tokio stack, with no
`eggress-*` package and no new production dependency or feature.

Validation on the freeze passed Tier 1, extended, and package gates, including
documentation examples/links, FFI, resource/lifecycle/soak suites, and the
existing compatibility suites. Extended validation explicitly skipped only the
unbuilt Node artifact, the Rust-1.80/MSRV parser incompatibility, and the
missing downstream artifact manifest.
