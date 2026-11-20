# Core Transport and Request Policy Milestone 011 — Profile-Independent Request Policy and Native `http::Uri` Builder

Status: closing

Repository baseline: `db3a553b27f3c3a8a29b77446d7d46449c4be8b8`

Source roadmap:

- `plans/subsystems/core-transport-policy-roadmap.md` (core transport and request policy)

Long-term requirements:

- `plans/000-long-term-specification.md` §4.1, §5
- `plans/001-terminology-and-domain-model.md` §6–§7
- `plans/002-long-term-roadmap.md` Phase 4 items 2 and 4

Applicable ADRs:

- `plans/adrs/ADR-0001-single-engine-thin-adapters.md`
- `plans/adrs/ADR-0002-pipeline-route-finalize.md`
- `plans/adrs/ADR-0005-frozen-body-feature-containment.md`

Primary class: capability

Source issues:

- eggstack/eggfetch#31 — `proxy` feature pulls the whole `http1` alias
- eggstack/eggfetch#32 — add `http::Uri` request construction so a
  `native-http1` client needs no `high-level-url`
- eggstack/eggfetch#33 — make `without_retry()` and `redirect_policy()`
  available without the `redirects`/`logical-retry` features

## 1. Objective

Give a consumer on a minimal eggfetch profile three things it could not
express before, without changing any existing profile's behavior:

1. Proxy support without the logical-retry/redirect/Basic-auth policy bundle.
2. A request builder that takes an `http::Uri` and needs no `url`/`idna`/ICU
   closure.
3. The two request-policy disable switches (`without_retry()`,
   `redirect_policy()`) callable in every profile.

## 2. Design

### 2.1 `proxy` selects `native-http1`, not `http1`

```toml
proxy = ["native-http1", "tls-rustls", "tokio/io-util", "high-level-url", "dep:eggfetch-http-connect", "dep:base64"]
```

The proxy implementation lives in `proxy.rs`, `transport/proxy.rs`, and
`pipeline/proxy_dispatch.rs`; none of them references `logical-retry`,
`redirects`, or `basic-auth`. Proxy auth keeps its own `dep:base64` edge,
so `Proxy-Authorization` encoding is unaffected by dropping `basic-auth`.
The default feature set and the `http1`/`http2` aliases are unchanged, so
the CLI and Python adapters keep the full policy bundle.

This is a Cargo feature-implication change and therefore a breaking change
for a downstream crate that relied on `proxy` implying `http1`. See §5.

### 2.2 A separate native request builder

`NativeRequestBuilder` (`crates/eggfetch-core/src/native_request.rs`) is
built from an `http::Uri` and dispatches through
`Client::execute_http_body`, the existing frame-preserving native path.

It is deliberately a *separate type* from the high-level `RequestBuilder`,
not a second constructor on it:

- the high-level builder owns URL semantics (query pairs, relative redirect
  resolution, cookie and proxy matching by URL) and returns the high-level
  `Response`;
- the native builder owns only the wire-level request and returns a plain
  `http::Response`.

Unifying them behind one type would have forced either a union return type
on `send()` or a breaking change to `Request::url()`/`Response::url()`,
which issue #32 forbids ("the existing `high-level-url` entry points keep
their current behavior and signatures"). Keeping them separate satisfies the
issue's intent — a URL-free builder API in every profile — with no
signature change to the high-level surface.

`RequestBody` implements `http_body::Body` so the native builder can hand
a request body to `execute_http_body` without exposing `http-body-util`
types in the public API.

### 2.3 The disable switches become profile-independent

- `redirect.rs` is no longer gated on `redirects`, so `RedirectPolicy` and
  `RedirectDowngradePolicy` are available in every profile. The
  request-level `Option<RedirectPolicy>` follows.
- `RequestBuilder::without_retry()` is no longer gated on `logical-retry`.
  Its backing state is an always-present `Request::retry_disabled` flag
  that the gated retry loop consults, alongside the unchanged
  `Option<RetryPolicy>` override.

With the corresponding feature enabled, both setters are honored exactly as
before. Without it, the capability is already absent, so the setter is a
semantic no-op that keeps the assertion expressible. The client-level
`ClientBuilder::redirect_policy` stays gated on `redirects`, because the
client-level policy is the redirect loop's own default.

## 3. Tests

- `crates/eggfetch-core/tests/native_request_builder_tests.rs` — native
  builder dispatch, body, URI validation, and the profile-independent
  assertions. Compiles and passes in every profile.
- `crates/eggfetch-core/tests/request_policy_assertion_tests.rs` — the
  setters compile in every profile; `without_retry()` suppresses a
  client-level retry policy (one logical attempt); a later `retry()` call
  re-enables retries; a request-level redirect policy is honored; without
  `redirects` the stored policy is inert.
- `crates/eggfetch-core/tests/public_api_contracts.rs` — compile contracts
  for the new surface, run across all eight contract-matrix profiles.
- `crates/eggfetch-core/tests/cross_feature_tests.rs` — the
  redirect/Basic-auth-dependent proxy tests gained per-test feature gates so
  the file still compiles in the policy-bundle-free proxy profile.

## 4. Validation

- Tier 1 (`./scripts/check.sh`) green, including the eight-profile contract
  matrix and the lean behavioral suites.
- Focused profile builds green for `--no-default-features`, `http1`,
  `http1,tls-rustls`, `http1,tls-rustls,tls-native-roots`,
  `standard-http1/2`, `native-http1/2`, the proxy-only profile, and
  `--all-features`.
- Rust public-surface oracle regenerated for all eight profiles and passing
  (`python scripts/check_rust_public_api.py --skip-semver`). The diff is
  additive only: zero removed lines.

## 5. Release consequence (open)

`cargo-semver-checks` classifies `feature_no_longer_enables_feature` as a
**major** lint, so the `proxy` implication change in §2.1 makes the
semver cross-check of `scripts/check_rust_public_api.py` report
"semver requires new major version" against the current oracle baseline
`03ecba973010e2858bf16a2b5f84d51ce70adae4`. The lint fires under both
`--release-type patch` and `--release-type minor`.

This is an intended consequence of issue #31, not an accidental API break:
the Rust public API itself gains items and removes none. Closing it needs a
maintainer decision on the next version number and on whether the oracle
baseline advances to a SHA that includes this change (the baseline has
never been advanced since the oracle was introduced in `ba4b7d57`, and
the previous feature-graph change — the `standard-http1`/`native-http1`
split — already predates it).

## 6. Non-goals

- no change to the default feature set or the `http1`/`http2` aliases;
- no change to Python/CLI/HTTPX defaults;
- no new mandatory dependency;
- no change to the high-level `RequestBuilder`/`Request`/`Response`
  signatures;
- no second redirect/retry engine, and no change to the gated loops'
  behavior when their feature is enabled;
- no removal of `url`/`idna`/ICU from the high-level profile.
