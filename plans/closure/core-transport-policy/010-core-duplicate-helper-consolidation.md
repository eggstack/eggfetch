# Core Transport Milestone 010 — Core Duplicate-Helper Consolidation

Status: closed

Source implementation plan:

- `plans/implementation/core-transport-policy/010-core-duplicate-helper-consolidation.md`

Source subsystem roadmap:

- `plans/subsystems/core-transport-policy-roadmap.md#milestone-10--core-duplicate-helper-consolidation`

Repository baseline reviewed: `873ea28541c82817317430c9523fca8525d99c81`.

Implementation commits:

- `d6c41adf4cf8f62ef990cccedf2f86512858fa0b` — feat: implement the eight
  registered milestones as one qualification freeze. This commit carries the
  milestone's implementation together with the other seven milestones
  executed in the same coordinated pass.
- `26fa619299fb8d447f25cbcb5eb713a1f537a30c` — fix(security): patch the four
  dependencies with live RustSec advisories. The bound Stage C SHA; it
  changes only `Cargo.lock` and `fuzz/Cargo.lock` relative to the freeze
  commit above, and no Rust source.

## 1. Executive finding

M010 is implemented, the milestone's own targeted gates are green, and the
milestone **is closed**. The milestone changes Rust sources under
`crates/eggfetch-core/src/` and adds a new test target; all of those are
executable inputs under the exact-SHA rule in
`.skills/verification-qualification.md`, so the live Stage C binding was
invalidated by it at the time of implementation. The binding has since been
re-established and the frozen-SHA tier gates were run at the bound SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c`:

- **Tier 1** (`./scripts/check.sh`) is green — 1767 Rust workspace tests
  passed (0 failed, 0 ignored), which includes the full `--all-features`
  `eggfetch-core` suite; 581 Python behaviour tests passed; 134 HTTPX
  compatibility smoke tests passed; the native Python API manifest (66
  exports), the Python typing surface and typing fixture, the
  lint-suppression guard self-test and policy scan, the H3
  qualification-runner self-test (runner logic only), the adapter feature
  ownership check and the release version/ref validation all pass; the stable
  public contract fixtures pass; and the feature-isolated profile
  whole-suite compile passes for all four standard/native profiles.
- **Tier 2** (`./scripts/check.sh extended`) is green across every phase —
  the pinned HTTPX compatibility suite reports 2073 passed / 0 failed, the API
  manifest comparison is clean for both facades (74 symbols for httpx 0.28.1,
  76 symbols for httpx2 2.12.0; 0 unexplained, 0 stale), the Rust public-API
  oracle passes in eight profiles and `cargo-semver-checks` 0.49.0 reports
  "no semver update required" against the planning baseline
  `03ecba973010e2858bf16a2b5f84d51ce70adae4`, and the feature matrix, MSRV
  matrix on the exact Rust 1.89.0 toolchain, documentation, FFI, resource,
  lifecycle, soak, downstream behavioral-fixture, lossless-merge and Criterion
  phases all pass. The only skip is the policy-defined optional one: the Node
  JS surface, whose native `eggfetch.node` artifact is absent.
- **Tier 3** (`./scripts/check.sh package`) is green, and the live security
  preflight (`./scripts/check_security.sh`) reports
  `advisories ok, bans ok, licenses ok, sources ok`.

The two items that the previous closure pass left outstanding are therefore
recorded as resolved in §10. The rebinding establishes a compatibility
binding only: it authorizes no publication, and HTTP/3 remains experimental.

What the milestone actually delivered:

- Seven duplication clusters are consolidated, one implementation per cluster.
  The plan's expected diff was "mostly deletions"; the measured diff adds
  ~380 lines of shared helpers and their tests and removes the duplicated
  comparison logic and message literals. No runtime behavior changed and no
  public API changed.
- Cluster D — the one cluster with a live behavioral consequence — is decided
  and proven: `HttpOrigin`'s stricter rule is adopted, the caller trace shows
  that both the permissive rule's extra acceptance and the strict rule's
  rejections are unreachable, and pool keying is proven unchanged for every
  supported URL shape.
- Cluster C's precedence is preserved exactly, including the fact that the
  built-in-proxy arm is evaluated at the dispatch boundary rather than in
  the preparation phase.
- A new test target, `crates/eggfetch-core/tests/consolidation_evidence_tests.rs`,
  carries ten tests, one or two per cluster plus a source-level public-surface
  assertion.

One thing is **not** claimed: the plan's own premises are wrong in eight
documented ways — its cluster list, its error kinds, its cluster-C precedence
description, its cluster-G file attribution and its cluster-D IPv6 claim are
each contradicted by the tree (§3, "The plan's premises, corrected"). This
record answers the acceptance criteria against the measured tree, not against
the plan's wording.

The plan's §13 criterion "the full workspace test suite is green" and its §15
evidence list were the release-verification subsystem's work rather than the
milestone's own; they were delivered afterwards, by the frozen-SHA
qualification on `26fa619299fb8d447f25cbcb5eb713a1f537a30c` (§10).

## 2. Requirement-to-evidence matrix

The requirements are the acceptance criteria of §13 of the implementation plan.

| Requirement | Evidence | Result | Notes |
|---|---|---|---|
| Each of the seven clusters has exactly one implementation of its rule (§13) | Per-cluster source measurement in §3; `crates/eggfetch-core/tests/consolidation_evidence_tests.rs` → 10 passed | Met | The per-cluster before/after table is in §3. One implementation each for A, B, C, D, E, F, G |
| The effective-port validation rejects the same inputs with the same error kind and message as before (§13) | `http_origin.rs:254-360`; `cluster_b_effective_port_rule_has_one_home` and `cluster_d_non_http_scheme_is_rejected_before_the_origin_normalization`; §3 "Cluster B" | Met | Seven call sites, one comparison, five message literals in one place. The error kinds measured at the baseline are preserved exactly — see §3, "The plan's premises, corrected", item 2 |
| The custom-dialing ladder preserves its precedence and its five incompatibilities (§13) | `pipeline/custom_dialing.rs`; three call sites at `pipeline/prepare.rs:451`, `pipeline/mod.rs:279`, `pipeline/mod.rs:540`; `cluster_c_custom_dialing_ladder_has_one_home`; §3 "Cluster C" | Met | Before and after are identical, including the two pre-ladder messages that precede the ladder and the absence of a built-in-proxy arm on the native path |
| The two origin normalizations agree on every URL shape in the test table, and the chosen authoritative rule is recorded (§13) | `pool.rs::origin_verdict_agreement_table`; §3 "Cluster D" | Met | The stricter `HttpOrigin` rule is adopted; the caller trace and the twelve-shape verdict table are in §3 |
| Pool keying is unchanged for every supported URL shape (§13) | `pool.rs::pool_key_is_stable_for_every_supported_shape` | Met | Compares the new key against a verbatim copy of the pre-consolidation permissive rule for every accepted shape — all identical. Independently confirmed at compiler level: the baseline's "associated items `from_url` and `origin_string` are never used" warning for `HttpOrigin::from_url` disappears on the current tree, because `OriginKey::from_url` now reduces through it (§4) |
| No public API changed in any profile (§13) | The Rust public-API oracle on the bound SHA `26fa6192…`: passed in all eight supported profiles, diffed against the frozen `compat/rust-public-api/*.txt` snapshots with `cargo-public-api 0.52.0` and the pinned `nightly-2026-05-07`, with `cargo-semver-checks` 0.49.0 reporting "no semver update required" against the planning baseline `03ecba97…` | Met | The oracle was re-run on the bound SHA, reproducing the implementer's eight IDENTICAL results; see §10, the oracle finding. Corroborated at source level: `git diff -U0 -- crates/eggfetch-core/src/ \| grep -E "^[+-]pub "` produces no output, and `crates/eggfetch-core/tests/consolidation_evidence_tests.rs::public_surface_is_unchanged` asserts the same property |
| Every error kind is unchanged, and the shared error messages are byte-identical to the current text (§13) | §3 "Cluster B" and "Cluster C"; the eight baseline UTF-8 literals are byte-identical to `TARGET_NOT_UTF8_MESSAGE`; the five ladder messages are byte-identical to the baseline literals | Met | `Error::InvalidResolvedTarget` for the five destination subjects and `Error::InvalidProxyUrl` for the proxy peer, both as at the baseline |
| The full workspace test suite is green (§13) | 1767 Rust workspace tests passed, 0 failed, 0 ignored, on `26fa6192…` (Tier 1) | Met | See §10, the tier-gate finding |
| Tier 1 and Tier 2 are green on the frozen executable SHA (§13) | `./scripts/check.sh` and `./scripts/check.sh extended` on `26fa6192…` — green | Met | Tier 3 (`./scripts/check.sh package`) and the live security preflight are green on the same SHA; see §10, the tier-gate finding |
| Both HTTPX profiles are rebound to that exact SHA (§13) | Both HTTPX profiles and the ledger's Stage C SHA rebound to `26fa6192…`, and the compat qualification was re-run against it | Met | The binding that this milestone invalidated was re-established at the bound SHA; see §7 and §10, the Stage C rebinding finding |
| Closure evidence: the before/after per-cluster copy count table (§15) | §3 | Met | — |
| Closure evidence: the cluster D caller trace and the chosen authoritative rule (§15) | §3, "Cluster D" | Met | — |
| Closure evidence: the origin-verdict agreement table (§15) | §3, "Cluster D" | Met | Twelve shapes |
| Closure evidence: the pool-key stability evidence (§15) | §3, "Cluster D" | Met | — |
| Closure evidence: the per-cluster equivalence test output (§15) | §4 | Met | Ten tests in the new target, plus the crate's unit tests |
| Closure evidence: the public API delta for every profile (§15) | §4, §7 | Met | No delta in any of the eight profiles |
| Closure evidence: known limitations, including the remaining M009 and python-httpx-compat M005 work (§15) | §10, §11 | Met | M009 is closed, not open; the redirect method-rewrite table's location remains tracked by python-httpx-compat M005 and is unaffected by this milestone |

## 3. Production implementation evidence

**The seven clusters, before and after.** The "before" column is the baseline
`873ea285…` as measured by source inspection; the "after" column is the
current tree.

| Cluster | Before | After | Shared helper | Discriminating parameter |
|---|---|---|---|---|
| **A** resolved-target / SNI client builders | 2 | 1 | `client::routes::build_direct_client(config, direct_connector_config, transport_metrics, pool, lifecycle, binding)` | `DirectRouteBinding::{Resolved{target, sni_hostname}, Sni{sni_hostname}}` |
| **B** effective-port validation | 6 comparison sites / 11 message literals | 1 / 5 | `http_origin::validate_effective_port(subject, addresses, expected_port)` and `validate_url_effective_port(subject, url, addresses)` | `EffectivePortSubject::{ResolvedDestination, ProxiedDestination, ResolvedProxyPeer}` |
| **C** custom-dialing incompatibility ladder | 9 literal copies across 2 ladders | 5 (one per arm) | `pipeline::custom_dialing::custom_dialing_conflict(state)` | `CustomDialingState.has_built_in_proxy: Option<bool>` |
| **D** origin normalization | 3 normalizations | 1 | `http_origin::HttpOrigin::from_url()` | none — the scheme is derived from the URL, not passed in |
| **E** post-handshake ALPN `h2` detection | 4 | 1 | `transport::negotiated_http2(Option<&[u8]>) -> bool` | none |
| **F** `HyperClientPolicy` constructors | 2 byte-identical bodies | 1 | `HyperClientPolicy::new(retry_canceled_requests, idle_timeout, max_idle_per_host, enabler)` | none — the **call site** decides when to resolve the idle values |
| **G** the `"target extension is not valid UTF-8"` literal | 8 literals across 3 files | 1 | `transport_hints::target_extension_str(&[u8]) -> Result<&str>` plus `TARGET_NOT_UTF8_MESSAGE` | none |

**Production changes, file by file.**

| File | Change |
|---|---|
| `crates/eggfetch-core/src/http_origin.rs` | New `EffectivePortSubject` enum (`:254`), `requires_http_url()` (`:280`), `port_mismatch()` (`:295`), `validate_effective_port()` (`:316`), `validate_url_effective_port()` (`:341`). Two factually-wrong IPv6 doc comments corrected (`:115`, `:185`). Module doc extended (`:13-18`) |
| `crates/eggfetch-core/src/pipeline/custom_dialing.rs` | **New file**, 225 lines. `CustomDialingConflict` (`:23`), `CustomDialingState` (`:69`), `custom_dialing_conflict()` (`:102`), plus 9 unit tests |
| `crates/eggfetch-core/src/pool.rs` | `OriginKey::from_url(url)` (`:242`) and `from_url_with_proxy_scheme(url, …)` (`:254`) now reduce through `HttpOrigin::from_url()`. `from_url` dropped its redundant `scheme` parameter. Four unit tests added: the verdict table (`:1793`), the pool-key stability check (`:1834`), the proxied-identity check (`:1857`) and the cluster-F idle-policy equality check (`:1876`), plus the `origin_shapes()` helper (`:1712`) |
| `crates/eggfetch-core/src/transport/mod.rs` | New `ALPN_HTTP2` const (`:134`) and `negotiated_http2()` (`:144`) plus 2 unit tests |
| `crates/eggfetch-core/src/transport_hints.rs` | New `TARGET_NOT_UTF8_MESSAGE` (`:83`) and `target_extension_str()` (`:94`) plus 2 unit tests |
| `crates/eggfetch-core/src/transport/hyper_client.rs` | `persistent()` and `cached_route()` collapsed into one `HyperClientPolicy::new()` (`:179`) |
| `crates/eggfetch-core/src/client/routes.rs` | New `DirectRouteBinding` (`:109`) and `build_direct_client()` (`:133`); `build_resolved_client()` deleted; `resolved_client()` / `sni_client()` now call the shared builder |
| `crates/eggfetch-core/src/client.rs` | `HyperClientPolicy::persistent(` → `::new(`; comment corrected |
| `crates/eggfetch-core/src/pipeline/prepare.rs` | Four sites moved to shared helpers: two effective-port checks (`:335`, `:343`), two target UTF-8 decodes (`:221`, `:257`), one ladder call (`:451`) |
| `crates/eggfetch-core/src/pipeline/mod.rs` | Native effective-port check → shared helper (`:509`); native ladder (`:540`) and high-level dispatch-boundary arm (`:279`) → shared ladder; `mod custom_dialing;` added |
| `crates/eggfetch-core/src/request.rs` | Two effective-port checks (`:842`, `:849`) → shared helpers. **No UTF-8 decode was moved from this file; there is none** (see §3, "Corrections to the implementer's file table") |
| `crates/eggfetch-core/src/proxy.rs` | Proxy-peer effective-port check (`:658`) → shared helper |
| `crates/eggfetch-core/src/transport/connect.rs` | One ALPN site (`:115`) and two UTF-8 sites (`:338`, `:551`) → shared helpers |
| `crates/eggfetch-core/src/transport/proxy.rs` | Four UTF-8 sites (`:590`, `:627`, `:713`, `:1133`) → shared helper. **No `validate_target` call was added to the two paths that deliberately lacked it** |
| `crates/eggfetch-core/src/transport/{uds,direct_connector,socks}.rs` | One ALPN site each (`uds.rs:100`, `direct_connector.rs:302`, `socks.rs:386`) → shared helper, with each site's own `cfg`/stream-variant structure preserved |
| `crates/eggfetch-core/src/pipeline/hyper_dispatch.rs` | One route-key error site (`:169`) → shared subject error. This file was **not** in the milestone's declared ownership, but it carried a third copy of cluster B's message text, and consolidating it was required by the "exactly one implementation per cluster" criterion. **M010's change to this file is one line**; the other hunks in this file's diff against the baseline (four `clippy::too_many_arguments` reason additions and the `missing_client_error` change) belong to other milestones' work and are not M010's |

**Cluster A — the shared builder.** `build_direct_client()` takes the six
parameters the two predecessors shared and one discriminating enum. The
differences were enumerated *before* the helper was designed: resolved-target
binding (`with_resolved_target`), SNI binding (optional versus mandatory), the
enabler computed twice versus once (same value), and nothing else — TLS
materialization, connect timeout, certificate verification, metrics, base
connector config, Hyper policy and lifecycle were already identical. The
"neither" combination (no resolved target, no SNI) is **not** part of this
cluster: it is the *persistent* direct singleton built in `client.rs` with a
different Hyper policy source and different TLS-failure handling, and folding
it in would change its policy.

**Cluster B — one comparison, five messages, seven sites.** The seven call
sites are `pipeline/prepare.rs:335` and `:343` (resolved and proxied
destinations), `pipeline/mod.rs:509` (native resolved destination),
`request.rs:842` and `:849` (resolved and proxied destinations), `proxy.rs:658`
(resolved proxy peer) and `pipeline/hyper_dispatch.rs:169` (the route-key
construction error). The first six sites' comparison logic was *identical*
(`addresses.iter().any(|a| a.port() != expected_port)`); only the
expected-port source, the error kind and the message differed. The subject
enum carries the last two; the caller supplies the expected port. The seventh
site does not compare ports — it needs a canonical HTTP(S) origin — and is
consolidated through `requires_http_url()` so that its message text is not a
third copy. The one comparison now lives at `http_origin.rs:323`.

**Cluster C — precedence, before and after, and reachability.**

*High-level path* (`send_single_request` → `prepare::prepare_single_request` →
back to `send_single_request`):

1. *(prepare, before the ladder, `prepare.rs:415`)* resolved target + built-in
   proxy → "caller-supplied resolved destinations require direct routing;
   disable the proxy explicitly" — **not** a ladder message; it precedes the
   ladder.
2. *(prepare, before the ladder, `prepare.rs:421`)* resolved target + UDS
   client → "caller-supplied resolved destinations are incompatible with
   Unix-domain routing" — also precedes the ladder.
3. *(prepare, ladder, arm 1)* resolved destinations → "custom dialing is
   incompatible with caller-supplied resolved destinations".
4. *(prepare, ladder, arm 2)* local-address or socket-option routing →
   "custom dialing is incompatible with local-address or socket-option
   routing".
5. *(prepare, ladder, arm 3)* Unix-domain routing → "custom dialing is
   incompatible with Unix-domain routing".
6. *(prepare, ladder, arm 4)* HTTP/3 → "custom dialing is incompatible with
   HTTP/3".
7. *(dispatch boundary, arm 5)* built-in proxy routing → "custom dialing is
   incompatible with built-in proxy routing".

*Native path* (`send_native_http_body`): the native proxy rejection ("native
frame bodies are not supported through the built-in proxy routes") precedes
the ladder, then arms 1–4 in the same order; **there is no arm 5**.

Before and after are identical. Arm 5 still fires at the dispatch boundary,
*after* the pool permit is acquired, so a request with a custom dialer plus a
proxy plus a pool timeout still reports the pool timeout first. That
precedence is preserved by construction because arm 5 is **not** moved into
the preparation phase.

Reachability: arm 1 (custom dialer + resolved target, no proxy, no UDS
client) — reachable; arm 2 (custom dialer + `local_address()` /
`socket_options()`, which the builder does not reject) — reachable; arm 3
(custom dialer + UDS configured) — reachable; arm 4 (custom dialer +
`Http3Only`) — reachable; arm 5 (high-level only; custom dialer + built-in
proxy and none of the above) — reachable; the native path has arms 1–4 only.

How the shared implementation preserves it: one `custom_dialing_conflict(state)`
evaluates arms 1→4→5. The high-level preparation phase calls it with
`has_built_in_proxy: None` (`prepare.rs:449`); the native path also uses
`None` (`mod.rs:538`); the high-level dispatch boundary calls it with
`Some(has_proxy)` (`mod.rs:277`). That `Option` is the single discriminating
parameter encoding "this path has no built-in-proxy arm" versus "this path
has it, and it is or is not active". The crate's own unit tests
(`custom_dialing.rs`) assert the order directly: resolved destinations win
over every other arm; local-address wins over UDS and HTTP/3; UDS wins over
HTTP/3 and the built-in proxy; HTTP/3 wins over the built-in proxy; the
built-in proxy is last; an absent built-in-proxy arm never conflicts; a
disabled built-in-proxy arm never conflicts; every arm keeps its own message.

**Cluster D — the decision and its evidence.**

*Decision: adopt `HttpOrigin`'s stricter rule* — only HTTP/HTTPS schemes,
non-empty host.

*Caller trace (why nothing observable changes):* the only production caller of
`OriginKey::from_url` is `pipeline/prepare.rs` (`:470` and `:475`), whose URL
comes from a `Request`. `Request`s are built only through `Client::parse_url()`
(`client.rs:1457`), which rejects any scheme other than http/https with
`Error::Unsupported("URL scheme '{other}' is not supported; use http or
https")` and rejects userinfo. Redirect hops are validated by
`redirect::validate_redirect_url()` (`src/redirect.rs:346`), which also
rejects non-HTTP(S) schemes. `Request::new` is `pub(crate)`
(`request.rs:282`, `request.rs:555`), so no external caller can inject a
foreign URL. `url::Url` itself rejects an empty host for the special
http/https schemes. ⇒ the permissive rule's only extra acceptance (a
non-HTTP(S) scheme) and the strict rule's rejections (empty host,
non-HTTP(S) scheme) are **both unreachable**.

*URL-shape verdict table* (implemented as `pool.rs::origin_verdict_agreement_table`):

| Shape | legacy permissive | consolidated strict | changed? | pool key |
|---|---|---|---|---|
| `http://example.com/path` | accept | accept | no | `http/example.com/80` |
| `https://example.com/path` | accept | accept | no | `https/example.com/443` |
| `ftp://example.com/path` | accept | reject | yes (unreachable) | — |
| empty host | not representable as a `url::Url` | reject | no | — |
| `http://EXAMPLE.COM/path` | accept | accept | no | `http/example.com/80` |
| `http://127.0.0.1:8080/path` | accept | accept | no | `http/127.0.0.1/8080` |
| `http://[::1]:8080/path` | accept | accept | no | `http/[::1]/8080` |
| `http://example.com:80/path` | accept | accept | no | `http/example.com/80` |
| `http://example.com:8080/path` | accept | accept | no | `http/example.com/8080` |
| `http://xn--bcher-kva.example/path` | accept | accept | no | `http/xn--bcher-kva.example/80` |
| `http://example.com./path` | accept | accept | no | `http/example.com./80` |
| `http://user:pass@example.com/path` | accept | accept | no | `http/example.com/80` |

*Pool-key stability:* proven by `pool_key_is_stable_for_every_supported_shape`,
which compares the new key against a verbatim copy of the pre-consolidation
permissive rule for every accepted shape — all identical.

*The two `None`s stay different:* `HttpOrigin::from_url()` returning `None` is
a hard error (the caller's `InvalidResolvedTarget`); `OriginKey::from_url()`
returning `None` is a soft degrade (no pool key → global permit only). They
were not merged. `OriginKey::from_url_with_proxy_scheme()` now goes through the
same constructor.

**Cluster F — build-time versus use-time idle policy.** `persistent` resolved
`pool_config.idle_timeout` and `pool_config.effective_max_idle_per_host()` at
**build** time; `cached_route` resolved `pool.idle_timeout()` and
`pool.max_idle_per_host()` at **use** time. `Pool::new(pool_config)` receives
the same mutated `pool_config`, so both resolve identical values — proven by
the test `pool_config_and_pool_resolve_the_same_idle_policy`
(`pool.rs:1876`), which asserts the equality for three pool configurations.
Callers still supply their arguments at their original moment, so the shared
constructor does not move a resolution across a lifecycle boundary.

**The IPv6 pool-key divergence — a residual, not fixed.**

*Found:* the high-level path builds the pool origin from the URL **host**
(`url::Url::host_str()` returns `[::1]` for `http://[::1]:8080`, verified
against the vendored `url-2.5.8` source), while the native path builds it
from the URI **authority** (`http::Uri::host()` returns `[::1]`, which
`HttpOrigin::from_uri()` strips to `::1`). The same IPv6 endpoint therefore
gets two different pool keys depending on the entry path, and
`HttpOrigin::from_url()` and `HttpOrigin::from_uri()` disagree with each
other.

*Changed:* **only the two factually-wrong doc comments** in
`http_origin.rs` — `from_uri` claimed its stripping "matches the high-level
`url` form", and `from_url` claimed `host_str` "already strips IPv6 brackets";
both are false and both now state the divergence and why it is not fixed here.

*Not changed:* the keying itself — changing it would alter pool keying, which
the plan forbids. Recorded as a residual in `docs/residual-differences.md`
("IPv6 pool-key divergence between the high-level and native origin paths",
`:278`) and in `docs/architecture/core-timeout-pool.md`.

**The plan's premises, corrected.** The plan's §1 table and §3 evidence are
wrong in eight ways. The tree, not the plan, is the authority for this
milestone.

1. **B6 was omitted from the plan.** `crates/eggfetch-core/src/proxy.rs` is
   the sixth effective-port site and the only one with a different error kind
   (`InvalidProxyUrl`). It was folded in.
2. **The plan's error kinds are wrong.** The plan says B1–B5 are
   `InvalidUrl`. Measured truth: B1–B5 are `Error::InvalidResolvedTarget`;
   only B6 is `Error::InvalidProxyUrl`. Both kinds are as at the baseline.
3. **Cluster B has seven sites, not five or six.**
   `pipeline/hyper_dispatch.rs:169` carried a *third* copy of "resolved
   destinations require an HTTP or HTTPS URL" — same kind, same text. The
   plan's list misses it; it was consolidated.
4. **The plan's cluster C precedence is wrong.** The plan lists
   `prepare.rs:456,461,466,475 ≡ mod.rs:512,517,522,531` as *identical*
   ladders. They are not: `mod.rs` has **two** separate ladders (native at
   `:540`, high-level dispatch boundary at `:279`), and the high-level
   dispatch-boundary arm is absent from the native path.
5. **The plan's cluster G third file is wrong.** It is
   `src/transport/proxy.rs`, not `src/proxy.rs`.
6. **The plan's claim that cluster G's error kinds are not all the same is
   wrong.** All 8 sites used `Error::InvalidUrl` with identical text.
7. **The plan's cluster A "five combinations" does not exist.** "Neither" (no
   resolved target, no SNI) is the *persistent* direct singleton built in
   `client.rs` with a **different** Hyper policy source and different
   TLS-failure handling; folding it in would change its policy. "Resolved +
   metrics" is not a separate combination — both builders always pass
   `transport_metrics`. The shared builder supports the three real
   combinations.
8. **The plan's cluster D IPv6 claim is wrong.** `HttpOrigin::from_uri`
   **strips** the IPv6 brackets (`::1`); `HttpOrigin::from_url` and
   `OriginKey::from_url` **keep** them (`[::1]`).

**Corrections to the implementer's file table.** Two rows of the implementer's
own file-by-file table are misattributed; the cluster totals are right.

- `crates/eggfetch-core/src/request.rs` is listed as moving "2 effective-port
  checks **and 2 target UTF-8 decodes**". The file has the two effective-port
  checks and **no** UTF-8 decode — not at the baseline and not now. `git show
  873ea285…:crates/eggfetch-core/src/request.rs` contains no `from_utf8`,
  `to_str()` or `String::from` UTF-8 decode.
- `crates/eggfetch-core/src/pipeline/prepare.rs` is listed as moving "1 target
  UTF-8 decode". It moves **two** (`prepare.rs:221` and `:257`).
- The eight UTF-8 sites are therefore `pipeline/prepare.rs` (2),
  `transport/connect.rs` (2) and `transport/proxy.rs` (4) — measured against
  the baseline, where the literal appears exactly eight times across those
  three files and nowhere else in `crates/eggfetch-core/src/`.

**Test target.** `crates/eggfetch-core/tests/consolidation_evidence_tests.rs`
is a new integration-test target, 392 lines, carrying ten tests:

| Test | Covers |
|---|---|
| `cluster_a_route_client_builder_has_one_home` | Cluster A |
| `cluster_b_effective_port_rule_has_one_home` | Cluster B |
| `cluster_c_custom_dialing_ladder_has_one_home` | Cluster C |
| `cluster_d_origin_normalization_has_one_home` | Cluster D |
| `cluster_e_alpn_detection_has_one_home` | Cluster E |
| `cluster_f_policy_has_one_constructor` | Cluster F |
| `cluster_g_target_utf8_diagnostic_has_one_home` | Cluster G |
| `public_surface_is_unchanged` | The no-public-API-change claim, at source level |
| `cluster_d_non_http_scheme_is_rejected_before_the_origin_normalization` | Cluster D's rejection path |
| `cluster_g_non_utf8_target_keeps_its_error_kind_and_message` | Cluster G's error kind and byte-identical message |

## 4. Verification executed

### Commands run

The working tree is shared with other agents and carries other milestones'
changes. Every measurement below is therefore pinned to a tree fingerprint.

```sh
# tree fingerprint used by the measurements
git rev-parse HEAD                              # 873ea28541c82817317430c9523fca8525d99c81
git diff | sha256sum                            # worktree fingerprint

# compile gates
cargo check --offline -p eggfetch-core --all-features --all-targets
RUSTFLAGS= cargo check --offline -p eggfetch-core --no-default-features \
  --features standard-http1,tls-rustls --all-targets
RUSTFLAGS= cargo check --offline -p eggfetch-core --no-default-features \
  --features standard-http2,tls-rustls --all-targets
RUSTFLAGS= cargo check --offline -p eggfetch-core --no-default-features \
  --features native-http1,tls-rustls --all-targets
RUSTFLAGS= cargo check --offline -p eggfetch-core --no-default-features \
  --features native-http2,tls-rustls --all-targets

# the crate's own test suites
cargo test --offline -p eggfetch-core --all-features --lib -- --test-threads=1
RUSTFLAGS= cargo test --offline -p eggfetch-core --no-default-features \
  --features standard-http1,tls-rustls --lib -- --test-threads=1
cargo test --offline -p eggfetch-core --all-features --test consolidation_evidence_tests
cargo test --offline -p eggfetch-core --all-features \
  --test pool_tests --test proxy_tests --test redirect_strict_tests \
  --test resolved_route_cache_tests --test custom_dialer_tests \
  --test request_failure_tests --test direct_transport_tests --test tls_integration

# lint and format
cargo clippy --offline -p eggfetch-core --all-targets --all-features -- -D warnings
rustfmt --edition 2021 --check <the 19 M010-edited Rust files>

# baseline "before" state, measured in a detached worktree at the baseline SHA
git worktree add --detach /tmp/eggfetch-m010-baseline 873ea285…
cd /tmp/eggfetch-m010-baseline
CARGO_TARGET_DIR=/tmp/eggfetch-m010-baseline-target RUSTFLAGS= cargo check --offline \
  -p eggfetch-core --no-default-features --features standard-http1,tls-rustls --lib
git worktree remove --force /tmp/eggfetch-m010-baseline

# per-cluster source measurements
grep -rn "validate_effective_port\|validate_url_effective_port" crates/eggfetch-core/src/
grep -rn "negotiated_http2" crates/eggfetch-core/src/
grep -rn "target_extension_str" crates/eggfetch-core/src/
grep -rn "custom_dialing_conflict" crates/eggfetch-core/src/
grep -rn "HyperClientPolicy::new" crates/eggfetch-core/src/
git grep -n "target extension is not valid UTF-8" 873ea285… -- crates/eggfetch-core/src/

# public-surface corroboration
git diff -U0 -- crates/eggfetch-core/src/ | grep -E "^[+-]pub "
git diff --stat -- crates/eggfetch-core/src/transport/alt_svc.rs

# oracle tooling availability
cargo public-api --version ; cargo semver-checks --version
rustup toolchain list | grep 2026-05-07
```

### Results (pass/fail/skip with reason; counts without concealment)

Snapshot — 2026-10-22 17:52Z–17:58Z, `HEAD=873ea285…`,
`git diff | sha256sum = c86574bef2220a4ab81a71fe461c7772b25b8b1b0ccfda9628f0bb557be69f63`:

| # | Invocation | Result |
|---|---|---|
| 1 | `cargo check --offline -p eggfetch-core --all-features --all-targets` | exit 0, clean |
| 2 | `RUSTFLAGS= cargo check … --features standard-http1,tls-rustls --all-targets` | exit 0; 24 warnings, all in the pre-existing profile-specific dead-code / unused-import category (§4, "Lean-profile warning delta") |
| 3 | the same for `standard-http2,tls-rustls` | exit 0, 0 errors |
| 4 | the same for `native-http1,tls-rustls` | exit 0, 0 errors |
| 5 | the same for `native-http2,tls-rustls` | exit 0, 0 errors |
| 6 | `cargo test --offline -p eggfetch-core --all-features --lib -- --test-threads=1` | exit 0, **849 passed, 0 failed** |
| 7 | `RUSTFLAGS= cargo test … --features standard-http1,tls-rustls --lib -- --test-threads=1` | exit 0, **393 passed, 0 failed** |
| 8 | `cargo test --offline -p eggfetch-core --all-features --test consolidation_evidence_tests` | exit 0, **10 passed, 0 failed** |
| 9 | the focused eight-target batch | exit 0, **145 passed, 0 failed** — 15 (`pool_tests`) + 17 (`proxy_tests`) + 22 (`redirect_strict_tests`) + 55 (`resolved_route_cache_tests`) + 4 (`custom_dialer_tests`) + 4 (`request_failure_tests`) + 14 (`direct_transport_tests`) + 14 (`tls_integration`) |
| 10 | `cargo clippy --offline -p eggfetch-core --all-targets --all-features -- -D warnings` | exit 0, clean |
| 11 | `rustfmt --edition 2021 --check` on the 19 M010-edited Rust files | exit 0, clean |
| 12 | lean-profile lib check at the baseline SHA, in a detached worktree | exit 0, **23 lib warnings** |
| 13 | the same on the current tree | exit 0, **22 lib warnings** |
| 14 | `git diff -U0 -- crates/eggfetch-core/src/ \| grep -E "^[+-]pub "` | no output |
| 15 | `git diff --stat -- crates/eggfetch-core/src/transport/alt_svc.rs` | no output — `AltSvcOrigin` untouched |
| 16 | `cargo public-api --version` / `cargo semver-checks --version` | `cargo-public-api 0.52.0` / `cargo-semver-checks 0.49.0` — the pinned oracle versions are installed |
| 17 | `rustup toolchain list \| grep 2026-05-07` | `nightly-2026-05-07-x86_64-unknown-linux-gnu` — the pinned nightly is installed |

**Lean-profile warning delta (rows 12–13).** The lean-profile lib warning set
was measured at the baseline SHA in a detached worktree and on the current
tree, and the two sets differ by exactly three lines:

```
< warning: associated function `cached_route` is never used
< warning: associated items `from_url` and `origin_string` are never used
> warning: method `origin_string` is never used
```

Net: **23 warnings at the baseline → 22 on the current tree.** No warning is
added. Two disappear, and they are exactly the two the consolidation should
remove: `HyperClientPolicy::cached_route` was merged into `new`, and
`HttpOrigin::from_url` — which the baseline reports as never used, because
`OriginKey::from_url` did its own normalization and never called it — is now
called by `OriginKey::from_url`. The compiler independently confirms cluster
D's consolidation. The baseline worktree was removed afterwards; `git
worktree list` shows only the main tree.

The baseline `--all-targets` lean check **fails to compile** (exit 101): the
baseline predates M009's test-target gating, so the whole suite cannot be
built under the lean profile there. That is M009's recorded finding, not
M010's, and it is why the warning delta above is measured lib-only.

**The public-API oracle.** The implementer reports that the Rust public-API
oracle, run over all eight supported profiles and diffed against the frozen
`compat/rust-public-api/*.txt` snapshots with `cargo-public-api 0.52.0` and
the pinned `nightly-2026-05-07`, produced **eight IDENTICAL** results:
`default`, `http1-tls-rustls`, `http2-tls-rustls`, `standard-http1-tls-rustls`,
`standard-http2-tls-rustls`, `native-http1-tls-rustls`, `native-http2-tls-rustls`
and `all-features`. No public item was added, removed or changed in any
profile.

The oracle was subsequently run on the bound SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c` as part of Tier 2 and passed in all
eight supported profiles, reproducing the implementer's result; the
`cargo-semver-checks` 0.49.0 run reports "no semver update required" against
the *planning* baseline `03ecba97…`, not the session's baseline `873ea28…`.
Because all eight snapshots match byte-for-byte, the cumulative public surface
of the whole session is unchanged, which is a strictly stronger statement than
"this milestone's change only". See §10.

Out of scope at the time of the milestone's own verification, and since run on
the bound SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c`:

- `./scripts/check.sh` (Tier 1), `./scripts/check.sh extended` (Tier 2) and
  `./scripts/check.sh package` (Tier 3) — the targeted equivalents in the
  table above were the substitute evidence at the time; the full gates are
  green on the bound SHA (§10).
- The full workspace test suite (`cargo test --workspace --exclude
  eggfetch-python --all-features`) — green on the bound SHA: 1767 passed,
  0 failed, 0 ignored.
- The Python HTTPX compat pytest suites (`crates/eggfetch-python/tests/compat/`)
  — green on the bound SHA: 2073 passed, 0 failed.
- The Rust public-API oracle — green on the bound SHA: passed in eight
  profiles, with `cargo-semver-checks` 0.49.0 reporting "no semver update
  required" against the planning baseline `03ecba97…` (see above).
- The MSRV gate — green on the bound SHA, on the exact Rust 1.89.0
  toolchain (five checks).

Every cargo invocation used the shell assignment prefix (`RUSTFLAGS= cargo …`).
The `env` binary on this machine is a shim that ignores its arguments; an
invocation of the form `env RUSTFLAGS= cargo --version` returns nothing and
does not run cargo at all, which is a false pass. This hazard is recorded in
the M008 closure record and was avoided here.

## 5. Invariant review

The plan's §4 invariants, checked against the tree:

- **No public API changes in any profile.** Met. No `pub` item was added,
  removed or changed in `crates/eggfetch-core/src/` (row 14), and the oracle
  reports no delta in any of the eight profiles.
- **The origin identity used for pooling, SNI, certificate verification,
  Host, cookies, auth, redirects and Alt-Svc trust remains the logical
  origin; a route peer address never becomes origin identity.** Met. Cluster D
  changed which *URLs* are accepted as origins, not which origin is used for
  any purpose; the caller trace shows the change is unreachable, and the
  pool-key stability test proves the key components are unchanged for every
  supported shape. `AltSvcOrigin` in `transport/alt_svc.rs` was not touched
  (row 15).
- **The effective-port validation keeps its current strictness.** Met. The
  comparison is the same expression, moved; the error kinds are the baseline
  kinds; the messages are byte-identical.
- **The custom-dialing incompatibility ladder keeps its current ordering and
  its current set of incompatible combinations.** Met. §3, "Cluster C",
  with the ordering asserted by the crate's own unit tests.
- **The `TransportFailureKind` domain established by ADR-0006 is unchanged.**
  Met. No transport failure classification was touched; cluster E changes only
  *how* the ALPN result is computed, not the failure attached to it.
- **Timeout ownership, pool admission, and body-lifecycle semantics are
  unchanged.** Met. No timeout, pool-permit or body-lifecycle code was
  modified. The effective-port validation still runs before any connection
  attempt and still consumes neither a deadline nor a pool permit.
- **The `ResponseBody` public variant shapes remain frozen.** Met. No
  `ResponseBody` variant, field or attribute was touched.
- **No new cargo dependency, and no new error variant.** Met. `Cargo.lock` is
  untouched by this milestone, and no `Error` variant was added.
- **Redaction rules are unchanged.** Met. No redaction-relevant code was
  touched; the shared helpers emit no credential-bearing text.
- **The `eggfetch-http-connect` boundary is unchanged.** Met. That crate was
  not modified by this milestone.

Additional invariants:

- **No new cargo feature was added, removed or renamed.** The feature
  footprint of the new helpers is expressed with the existing `proxy`,
  `http3`, `high-level-url` and `advanced-routing` features.
- **No CI job, matrix or workflow was added.** `.github/workflows/` is
  unchanged by this milestone.
- **No `unsafe` was introduced**; the workspace-wide `unsafe_code = "forbid"`
  holds.

## 6. Failure, timeout, pool, and recovery review

No new request path, transport, timeout, pool or recovery logic was added.
The milestone moves existing comparisons, existing message literals and two
existing client builders into shared helpers.

The four behavioral surfaces the milestone could have disturbed, and what
guards them:

- **The effective-port rejection.** It still runs at the same points in the
  request lifecycle, still before any connection attempt, still consumes
  neither a deadline nor a pool permit, and still produces the same error
  kind and the same byte-identical message. Cluster B's seven sites are
  enumerated in §3 and each maps to one of the two shared entry points.
- **The custom-dialing precedence.** §3, "Cluster C", enumerates the seven
  high-level messages and the five native ones, before and after. The
  dispatch-boundary arm is deliberately **not** moved into the preparation
  phase, so a request with a custom dialer plus a proxy plus a pool timeout
  still reports the pool timeout first.
- **Pool keying.** The pool-key stability test compares the new key against a
  verbatim copy of the pre-consolidation permissive rule for every accepted
  URL shape; all identical. The two `None` semantics — hard error versus
  soft degrade — were deliberately **not** merged.
- **The ALPN detection.** Cluster E's helper computes the same predicate as
  the four inline expressions it replaced, and the unit test
  `negotiated_http2_matches_the_previous_inline_expression` compares it
  against the previous expression over candidate inputs. Each of the four call
  sites keeps its own `cfg` and stream-variant structure and its own
  transport-specific context.

Cluster F is the one place where a resolution *could* have moved across a
lifecycle boundary: `persistent` resolved the idle policy at build time and
`cached_route` at use time. They were **not** merged in that respect — the
call sites still supply their arguments at their original moment. The
underlying values are proven equal by
`pool_config_and_pool_resolve_the_same_idle_policy`, because `Pool::new()`
receives the same mutated `pool_config`.

No new timeout owner and no second pool admission point was introduced.

## 7. Compatibility and feature-profile review (incl. exact-SHA binding)

Supported core profiles and their gates after this milestone:

| Profile | Contract fixtures | Behavioral suite | Whole-suite compile | Public-surface snapshot |
|---|---|---|---|---|
| default | Tier 1 | yes | — | `default.txt` |
| http1 + tls-rustls | Tier 1 | yes | — | `http1-tls-rustls.txt` |
| http2 + tls-rustls | Tier 1 | yes | — | `http2-tls-rustls.txt` |
| standard-http1 + tls-rustls | Tier 1 | yes (`lean_policy_tests`, `lean_route_tests`) | Tier 1 | `standard-http1-tls-rustls.txt` |
| standard-http2 + tls-rustls | Tier 1 | yes | Tier 1 | `standard-http2-tls-rustls.txt` |
| native-http1 + tls-rustls | Tier 1 | yes | Tier 1 | `native-http1-tls-rustls.txt` |
| native-http2 + tls-rustls | Tier 1 | yes | Tier 1 | `native-http2-tls-rustls.txt` |
| all features | Tier 1 | yes | — | `all-features.txt` |

All four feature-isolated profiles compile the whole core test suite on the
current tree (rows 2–5). The Tier 1 gate that enforces this is
`tier1_profile_whole_suite_compile` in `scripts/check.sh:160`; it is a
compile, not a run.

**Exact-SHA binding.** This milestone changed Rust sources under
`crates/eggfetch-core/src/` and added a new test target — all executable
inputs under `.skills/verification-qualification.md` — and therefore
invalidated the then-live Stage C binding
`42a9c96db5b25354843bd83bd458c71757e95651`
(`compat/httpx/0.28.1/profile.toml:15` and `compat/httpx2/2.12.0/profile.toml:13`,
and the ledger's Stage C entry in `plans/httpx-parity-correction-status.md`),
which is an ancestor of the baseline `873ea285…` but did not cover the
milestone's changes. The binding has since been re-established at the frozen
SHA `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, which carries this
milestone's implementation in the freeze commit
`d6c41adf4cf8f62ef990cccedf2f86512858fa0b`. Both HTTPX profiles and the
ledger's Stage C SHA point at that SHA, and the frozen-SHA qualification is
green (§10). The rebinding establishes a compatibility binding only: it
authorizes no publication, and HTTP/3 remains experimental.

**Feature-profile effects: none.** Every consolidation compiles in every
profile; the four feature-isolated profiles and the default and all-features
profiles were compiled in this pass, and no public-surface delta was observed
in any profile.

**Migration.** No migration is required: the changed items are crate-private
(`pub(crate)` / `pub(super)`), and the only removed parameters were on
`pub(crate)` functions.

**A note on the main agent's later change.** After the milestone's own
verification, the main agent extended the Tier 1 whole-suite compile gate
from the lean profile to all four feature-isolated profiles, which required
gating six wholly high-level-dependent test targets, three high-level tests
in `http_version_capability_tests`, and two inline unit tests in
`crates/eggfetch-core/src/auth.rs` with a `high-level-url` feature gate. That
extension is **outside M010's scope** and is recorded here only so that this
record does not claim credit for it. It changed only test-target gating and
the Tier 1 gate's command list — no runtime behavior, no public surface. It
is also already recorded as a resolved finding in the M009 closure record
(§10, the native-profile finding), so it must not be counted twice in the
registry or the ledger. See §10.

## 8. Security review

No security-relevant behavior changed. The milestone moves existing code
without changing it.

- No trust store, certificate handling, credential, redaction, proxy, cookie
  or TLS configuration was touched, and no new dependency was introduced.
- Cluster A's shared builder performs the *same* TLS materialization, the
  same connect timeout and the same certificate-verification setup as the two
  builders it replaces; only the binding differs, and the binding is a
  caller-supplied resolved-address snapshot or a caller-supplied SNI
  hostname.
- Cluster D's stricter origin rule does not weaken anything: it rejects a
  non-HTTP(S) scheme and an empty host where the permissive rule accepted
  them, and the trace shows both rejections are unreachable.
- No shared helper emits a credential-bearing string. The effective-port
  messages carry a port number; the ladder messages name a routing mode; the
  UTF-8 message is the literal it always was.
- `Authorization`, `Proxy-Authorization`, `Cookie` and `Set-Cookie` redaction
  is untouched — no redaction-relevant code appears in the milestone's diff.
- The `eggfetch-http-connect` crate is untouched.

## 9. Documentation and operations

| File | Change |
|---|---|
| `docs/architecture/core-engine.md` | New "Shared rule helpers (crate-private)" table (`:337-350`) listing the six single-sourced rules and their call sites; a `HyperClientPolicy` correction |
| `docs/architecture/core-tls-proxy-protocols.md` | New "Cached direct route client construction" section with the `DirectRouteBinding` table, and an ALPN table naming each transport's call site and the difference the call site keeps |
| `docs/architecture/core-timeout-pool.md` | New "One origin normalization" subsection (`:214`) recording the adopted rule and the pool-key consequence |
| `docs/residual-differences.md` | New section "IPv6 pool-key divergence between the high-level and native origin paths (Rust-only)" (`:278-300`), recording the divergence, the consequence and why it is not fixed here |

Operations: no new cargo invocation was added to any tier gate by this
milestone. The Tier 1 gate's whole-suite compile step was extended to more
profiles by the main agent after the milestone's verification (§7, "A note on
the main agent's later change"); that is a compile, not a run, and it adds
compile time only. No new external dependency, no network access and no
credential is required.

## 10. Unresolved findings

| Severity | Finding | Impact | Required action |
|---|---|---|---|
| Resolved | The live Stage C binding (`42a9c96d…`) was invalidated by this milestone's Rust-source and test-target changes, and the Stage C rebinding had not been performed at the time of implementation | The previous binding could not be cited by any publication | **Resolved 2026-10-22.** Both HTTPX profiles and the ledger's Stage C SHA were rebound to `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, and the frozen-SHA qualification was re-run |
| Resolved | Tier 1, Tier 2, and Tier 3 had not been run on the frozen SHA at the time of implementation, and neither had the full workspace test suite or the Python HTTPX compat suites | The repository-wide gates were unproven for this tree | **Resolved 2026-10-22.** Tier 1, Tier 2, and Tier 3 are green on `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, together with the live security preflight; the full workspace suite reported 1767 passed, 0 failed, 0 ignored, and the pinned compat suites reported 2073 passed, 0 failed |
| Resolved | The Rust public-API oracle result was the implementer's report and was not re-run during the milestone's own closure pass; the source-level corroboration (no `pub` delta in the diff, `public_surface_is_unchanged`) is weaker than the oracle | The plan's §13 "no public API changed in any profile" rested on an un-reproduced measurement | **Resolved 2026-10-22.** The oracle ran on the bound SHA and passed in all eight profiles, with `cargo-semver-checks` 0.49.0 reporting "no semver update required" against the planning baseline `03ecba97…` — the eight IDENTICAL results reproduced |
| Resolved | The plan's own §1 table and §3 evidence are wrong in the eight ways listed in §3, and its §13 acceptance criteria are worded against those wrong premises | The plan cannot serve as the acceptance reference for this milestone; a future reader who checks the plan against the tree will find nine mismatches | **Resolved 2026-10-23.** The plan's §1 table, §3 evidence and §13 acceptance criteria were corrected against the tree measured in this record: cluster B's copy count (six port-comparison sites over seven call sites; the plan's list omitted the baseline `request.rs:869` and `pipeline/hyper_dispatch.rs:161` sites) and its two error kinds (`Error::InvalidResolvedTarget` for the five destination sites, `Error::InvalidProxyUrl` for the proxy peer); cluster C's structure (two four-arm ladders plus the high-level dispatch-boundary arm, which the native path lacks, and the two pre-ladder messages); cluster G's file attribution (`src/transport/proxy.rs`, not `src/proxy.rs`) and its single `Error::InvalidUrl` kind; cluster A's difference set (three differences; the persistent direct singleton is out of the cluster); and cluster D's IPv6 host form (`HttpOrigin::from_uri` strips the brackets, `HttpOrigin::from_url` and `OriginKey::from_url` keep them), recorded as a residual rather than a fix. §13 now states the criterion each cluster actually met, and the plan and this record agree |
| Medium | The IPv6 pool-key divergence between the high-level and native origin paths remains open by design | The per-origin concurrency limit is not shared between the two entry paths for an IPv6-literal endpoint. Nothing else changes: scheme, port and lowercase-host normalization are identical | Closing it changes pool keying, which this milestone forbids. It needs its own milestone under the same subsystem. Recorded in `docs/residual-differences.md` |
| Medium | `EffectivePortSubject::requires_http_url()` has a third arm (`ResolvedProxyPeer` → "resolved proxy peers require the proxy URL's effective port") that is **unreachable**: the proxy-peer path resolves its port from the proxy URL, which reports its own diagnostic ("proxy scheme … has no known default port") first | The arm exists only to keep the enum exhaustive; the message is never emitted. A reader could mistake it for a live diagnostic | Acceptable as implemented; the doc comment at `http_origin.rs:271-279` states that the arm is kept for exhaustiveness and why it is unreachable |
| Low | `OriginKey::from_url` and `from_url_with_proxy_scheme` lost their `scheme` parameter | Both are `pub(crate)`, so no public API changes, and the change is the point of the consolidation: a future caller can no longer pass a scheme that disagrees with the URL | None. Recorded so the parameter removal is not mistaken for an accident |
| Low | `crates/eggfetch-core/src/pipeline/hyper_dispatch.rs` was edited outside the milestone's declared ownership. M010's change to the file is **one line** (`:169`, the route-key error site); the other hunks in that file's diff against the baseline belong to other milestones | The milestone's "declared file ownership" no longer matches the tree exactly | Recorded for honesty. The line was required by the "exactly one implementation per cluster" criterion and does not change behavior |
| Low | The main agent's post-verification extension of the whole-suite compile gate to all four feature-isolated profiles (six file-level `high-level-url` test gates, three per-test gates in `http_version_capability_tests`, two inline gates in `src/auth.rs`, and the `scripts/check.sh` gate rename to `tier1_profile_whole_suite_compile`) postdates this milestone's own verification and is **already recorded** as a resolved finding in the M009 closure record | If it is recorded again as M010 work, the registry and the ledger will double-count a single change | Record it once, against M009 or the release-verification subsystem, not against M010. It changed only test-target gating and the Tier 1 gate's command list — no runtime behavior, no public surface |
| Low | The plan's §10 "Required tests" asks for per-cluster equivalence coverage of the four ALPN transports and the three effective-port subject variants; the delivered coverage is the shared-helper unit tests plus the `consolidation_evidence_tests` target, not one test per transport per subject | The four ALPN call sites are structurally identical after consolidation and each keeps its own `cfg`/variant structure; the coverage is by construction rather than by four separate transport fixtures | Acceptable as implemented. If the stricter reading is wanted, four transport-level ALPN tests would have to be added under a later milestone |

**The condition, the risk, and the exact future evidence.** The two named
conditions of the previous closure pass are now met:

- **Condition 1 — Stage C rebinding.** Met. Both HTTPX profiles and the
  ledger's Stage C SHA were rebound to the frozen SHA
  `26fa619299fb8d447f25cbcb5eb713a1f537a30c`, which carries this milestone's
  implementation in the freeze commit
  `d6c41adf4cf8f62ef990cccedf2f86512858fa0b`, and the compat qualification
  was re-run against it.
- **Condition 2 — the frozen-SHA tier gate run.** Met. `./scripts/check.sh`,
  `./scripts/check.sh extended` and `./scripts/check.sh package` are green on
  that same SHA, together with the full workspace test suite (1767 passed, 0
  failed, 0 ignored), the pinned Python HTTPX compat suites (2073 passed, 0
  failed) and the eight-profile public-API oracle.

**Exact evidence now on record.** Against the frozen SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c`: a green `./scripts/check.sh`,
`./scripts/check.sh extended` and `./scripts/check.sh package`; a green
`cargo test --workspace --exclude eggfetch-python --all-features -- --test-threads=1`;
a green
`EGGFETCH_COMPAT_REQUIRED=1 python -m pytest crates/eggfetch-python/tests/compat/ -v --strict-markers`
against the rebound profiles; an eight-profile public-API oracle run
reporting eight IDENTICAL results and "no semver update required"; and a live
security preflight reporting
`advisories ok, bans ok, licenses ok, sources ok`.

**What the rebinding does not claim.** It establishes a compatibility binding
only. It authorizes no publication, HTTP/3 remains experimental, and the
findings that are still open in §10 — the plan's eight wrong premises and the
IPv6 pool-key divergence — are unchanged by it and remain pending maintainer
decisions.

## 11. Roadmap disposition

`plans/subsystems/core-transport-policy-roadmap.md` milestone 10 moves from
`conditionally closed` to `closed`, with §11 (completion definition) and §12
(milestone status) updated in the same pass.

The milestone's own scope — the seven consolidations, cluster D's decision
and its caller trace, the origin-verdict table, the pool-key stability proof,
the cluster-C precedence proof, the per-cluster evidence tests and the
documentation updates — is complete and measured, and the cross-cutting
release evidence (Tier 1 / Tier 2 / Tier 3 on the frozen SHA and the Stage C
rebinding) was delivered on the bound SHA
`26fa619299fb8d447f25cbcb5eb713a1f537a30c`. The findings in §10 that are
scoped to this milestone remain open and await maintainer decisions: the
plan's eight wrong premises and the IPv6 pool-key divergence.

M009 (lean path single-sourcing) is closed, not open. The redirect
method-rewrite table's location remains tracked by python-httpx-compat M005
and is untouched by this milestone.

## 12. Registry updates

`plans/registry.md`:

- The core-transport-policy row records milestone 10 as closed.
- The implementation-plan table for M010 points at this closure record.
- The blocked / operational work section no longer records the Stage C
  rebinding as pending for the milestones closed in this pass: it was
  delivered on the bound SHA `26fa6192…`, together with the Tier 1 / Tier 2 /
  Tier 3 run.
- The main agent's feature-isolated-profile whole-suite compile extension is
  recorded **once**, against M009 or the release-verification subsystem, and
  not again against M010 (§10).
