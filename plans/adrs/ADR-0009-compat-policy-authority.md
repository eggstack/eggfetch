# ADR-0009: Compat Policy Authority Between the Engine and the HTTPX Facades

Status: accepted

Date: 2026-10-22

Accepted: 2026-10-22, on maintainer instruction to execute
`plans/implementation/python-httpx-compat/005-compat-policy-authority-reconciliation.md`,
which depends on this decision.

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md` §4.2, §4.3, §8
- `plans/001-terminology-and-domain-model.md` §6–§7

Affected subsystem roadmaps:

- `plans/subsystems/python-httpx-compat-roadmap.md`
- `plans/subsystems/core-transport-policy-roadmap.md`

## Context

ADR-0001 established that all HTTP logic lives in `eggfetch-core` and that
adapters parse input, call the engine, and format output. The HTTPX
compatibility facades are a deliberate, documented exception in one respect:
they must reproduce HTTPX's own behavior, including behavior HTTPX implements
in Python rather than in Rust.

That exception was applied without a written rule about *which* policy each
side owns. Three concrete consequences are visible in the current tree.

### 1. Two cookie engines, opposite verdicts

`eggfetch-core` implements its own RFC 6265 cookie jar
(`crates/eggfetch-core/src/cookie.rs`). One of its rules is that a cookie
carrying an explicit `Domain` attribute naming an IP address is rejected:

```rust
// Reject IP addresses with explicit Domain attribute.
if is_ip_address(&normalized) {
    return None;
}
```

The compat facades instead use the Python standard library's
`http.cookiejar` through `eggfetch.compat.httpx._cookies.Cookies`. The
standard library accepts that same cookie. Verified against a local server
returning `Set-Cookie: q=1; Domain=127.0.0.1`:

- standard-library jar: stores `('q', '.127.0.0.1', domain_specified=True)`;
- engine jar: rejects it.

Both engines are live in the compat request path. The facade's jar is what
the user sees as `client.cookies`, and the engine's jar participates in
request preparation and in redirect hops. In the common case the facade sets
a `Cookie` header and the engine's injection is skipped, which is why the
divergence is invisible; it is not that the rules agree.

### 2. Two redirect-policy implementations

The engine owns redirect reconstruction: the method-rewrite table, the
`Authorization`/`Proxy-Authorization` stripping rule, and the typed rebuild
helpers that make a missed field a compile error. The facade also
implements a redirect loop in Python, with its own method-rewrite table and
its own credential-stripping rule. The two rules differ:

- the engine strips `Authorization` on **every** redirect hop;
- the facade strips it only **cross-origin**, and preserves it for
  same-origin redirects and for `https` to `http` downgrades.

Correction (2026-10-22, recorded during implementation of
`plans/implementation/python-httpx-compat/005-compat-policy-authority-reconciliation.md`):
the two bullets above misdescribe the measured behavior and are superseded by
the measurement below. The engine's rule is **origin-based**, not
always-strip — `crates/eggfetch-core/src/pipeline/redirect.rs`
(`advance_redirect_hop`) re-attaches a manually-set `Authorization` header when
`origin(previous) == origin(next)`, and `crates/eggfetch-core/src/redirect.rs`
(`strip_headers_for_redirect`) drops it on every **cross-origin** hop. The
facade's rule, measured against upstream HTTPX 0.28.1, preserves
`Authorization` only for the canonical `http:80` -> `https:443` same-host
upgrade; an `https` -> `http` downgrade is cross-origin and is stripped by
**both** implementations. The decisions in rules 1-6 are unaffected; the
difference the rule 4 documentation duty targets is real, but its exact
shape is the one recorded in `docs/residual-differences.md` and pinned by
`crates/eggfetch-python/tests/compat/test_redirect_policy_parity.py`.

The difference is not documented anywhere, and `docs/residual-differences.md`
has no redirect-policy section.

### 3. Compat-only semantics reach the native Python surface

The native `eggfetch` Python surface is documented as distinct from the compat
facades, and `AGENTS.md` records that the compat `NO_PROXY` parser differs
from the native `NoProxy::parse()`. Two compat-specific behaviors nevertheless
apply to the native surface:

- the native client's environment `NO_PROXY` handling calls the **compat**
  parser (`crates/eggfetch-python/src/request_preparation.rs` selects the
  compat parser for the shared client-configuration path), so the native
  surface does not use native `NO_PROXY` semantics, while the CLI does;
- the compat SOCKS normalization that rewrites `socks5://` to `socks5h://` is
  applied on the shared path, so it silently changes DNS-resolution semantics
  for native-API users who never touched a compat facade.

Neither is a correctness bug on its own. Both are undocumented surfaces where
"native Python" and "compat Python" are not distinguishable, which is the
opposite of the separation `AGENTS.md` asserts.

## Decision drivers

- The facades must remain HTTPX-compatible. HTTPX's own behavior is the
  specification for the facades, including where it differs from the engine.
- The engine must remain the single implementation of HTTP semantics for
  native Rust callers.
- Where two live implementations can observe the same state, they must not
  silently disagree.
- The native Python surface must not inherit compat-only semantics without
  saying so.
- Any divergence that is intentional must be recorded where a reader will
  find it, in `docs/residual-differences.md`.
- No second engine and no forked request path.

## Considered options

### Option A — The engine owns all policy; the facades delegate everything

Rejected: it cannot produce HTTPX compatibility. HTTPX's `Cookies` is a
`MutableMapping` backed by `http.cookiejar`, `follow_redirects` defaults to
`False`, and `trust_env` changes behavior. Those are facade obligations.

### Option B — The facades own all policy; the engine matches HTTPX

Rejected: the engine is also the native Rust surface, whose callers are not
bound by HTTPX's behavior. It would trade one divergence for a larger one.

### Option C — Explicit split with a reconciliation rule

Keep the current division of labor but write it down, add a rule for the case
where both sides can observe the same state, and separate the native surface
from compat-only semantics.

### Option D — Record the divergences and change nothing else

Rejected: it leaves the two cookie engines free to keep disagreeing, and
leaves the native surface's environment semantics undocumented.

## Decision

**Option C.** The following rules are established.

1. **The engine owns HTTP semantics.** Request and response construction,
   the redirect method-rewrite table, credential handling on redirect hops,
   the wire framing, and the cookie rules that apply to any caller are owned
   by `eggfetch-core`. Native Rust callers see exactly these rules.

2. **A facade owns HTTPX-shaped behavior.** The facade owns the objects and
   defaults HTTPX defines: the `Cookies` mapping and its backing jar, the
   `follow_redirects` default, `trust_env`, `mounts`, the auth flow objects,
   and the exception types. The facade's own rules are the specification for
   the facade.

3. **Where one state is observable from both sides, the two must agree, or
   the divergence must be recorded.** The engine's cookie jar and the
   facade's cookie jar both observe the server's `Set-Cookie` headers and
   both influence what is sent. They must not apply contradictory rules to
   the same header. Two acceptable resolutions:
   - the engine's rule is relaxed to match the standard library's verdict for
     the affected case, so both engines accept or both reject; or
   - the engine's jar is made subordinate to the facade's jar in the facade
     request path, so only one jar ever decides.
   The implementation must measure the observable difference before choosing,
   and record the outcome in `docs/residual-differences.md` either way.

4. **The engine's redirect policy is authoritative for the wire; a facade
   that performs its own redirect loop must match the engine's rules or
   document the difference.** The engine's origin-scoped rule exists for
   credential safety. A facade that keeps credentials on a same-origin
   redirect, across a canonical `http:80` -> `https:443` upgrade, or across
   an `https` to `http` downgrade, must either adopt the engine rule or
   justify the difference in `docs/residual-differences.md`
   as an HTTPX-compatibility requirement. Silent difference is not
   acceptable.

5. **Compat-only environment semantics must not reach the native surface.**
   The native `eggfetch` Python client uses the native `NO_PROXY` parser and
   does not rewrite `socks5://` to `socks5h://`. The compat facades keep both
   behaviors. If the native surface needs an equivalent knob, it is added as
   a native-surface option, not by inheriting the compat default.

6. **The engine's typed rebuild helpers remain the only place a hop request
   is constructed.** A facade that builds a follow-up request must route its
   field construction through the engine, not through an independent Python
   table that can drift.

## Consequences

### Positive

- The facades stay HTTPX-compatible while the engine keeps its own contract.
- Two live cookie engines can no longer disagree silently.
- A reader of `docs/residual-differences.md` can learn the full set of
  intentional facade/engine differences instead of discovering them by
  reading two codebases.
- The native Python surface behaves like the native Rust surface for
  environment-driven proxy configuration.

### Negative

- Rule 3 requires a measurable decision on cookie-rule parity, which may
  change engine cookie behavior for native callers.
- Rule 5 changes native Python environment semantics; existing native users
  relying on the compat `NO_PROXY` parser or on `socks5` DNS behavior change
  behavior.
- The engine's cookie jar and the facade's jar remain two implementations.
  Rule 3 constrains their disagreement; it does not merge them.

### Neutral or deferred

- Merging the two cookie engines into one is explicitly deferred. It is a
  larger change than this decision authorizes and would require the facade's
  `Cookies` type to become an engine type.
- The facade's duplicated method-rewrite table may remain if it is proven
  equivalent to the engine's by a shared conformance fixture.
- HTTPX 1.0 support is unaffected.

## Compatibility and migration

- Native Rust callers: cookie behavior may change only if the engine's
  IP-address `Domain` rule is relaxed under rule 3. That change must be
  justified in the closure record as a deliberate alignment, not a
  side effect.
- Native Python callers: environment `NO_PROXY` parsing and SOCKS scheme
  normalization change per rule 5. This is a user-visible behavior change
  and must be documented in the native surface's documentation and in
  `docs/residual-differences.md`.
- Compat facades: HTTPX compatibility must be preserved exactly. Any change
  to a facade's observable behavior requires an HTTPX conformance argument
  and a fresh exact-SHA qualification.
- The engine and the facades are executable inputs. All changes require
  renewed exact-SHA Stage C qualification of both the 0.28.1 and 2.12.0
  profiles under `.skills/verification-qualification.md`.

## Security and reliability implications

- Rule 4 matters for credential safety. Stripping `Authorization` on every
  redirect hop is the conservative engine behavior; a facade that preserves
  credentials across a downgrade hop must justify it, because a downgraded
  hop sends credentials in clear text.
- Rule 3 matters for request correctness. A cookie accepted by one engine and
  rejected by the other produces a request whose `Cookie` header depends on
  which code path assembled it.
- Rule 5 keeps environment-driven proxy configuration predictable for native
  callers.

## Verification

- A conformance fixture that feeds one identical `Set-Cookie` header stream
  through the engine jar and the facade jar and asserts an agreed verdict per
  case, with any remaining disagreement listed in
  `docs/residual-differences.md`.
- A redirect conformance fixture that drives the same redirect chain through
  the engine and through the facade and asserts an agreed credential outcome
  and method sequence, with any remaining difference recorded.
- Native-surface tests proving the native `NO_PROXY` parser and unmodified
  SOCKS scheme are used on the native client.
- Existing HTTPX 0.28.1 and httpx2 2.12.0 conformance suites green.
- Tier 1 and Tier 2 green, with the public API oracle unchanged unless a rule
  change requires it (it must not).

## Supersession

None.
