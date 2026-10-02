# Core Transport Policy M007 — Native Transport Failure Classification

Status: closed — see `plans/closure/core-transport-policy/007-native-transport-failure-classification.md`

Repository baseline: 092dcc7a0cdc76451cbca833ef80175ecd7b82b9

Source roadmap:

- plans/subsystems/core-transport-policy-roadmap.md, Milestone 7

Accepted ADR:

- plans/adrs/ADR-0006-native-transport-failure-classification.md

Historical evidence:

- plans/native-request-failure-introspection.md
- plans/custom-dialer-transport-extension.md
- plans/physical-connection-admission-and-io-inactivity-guardrails.md

Primary class: capability / invariant

## 1. Objective

Finish Eggfetch's typed failure boundary for native embedders without changing
the existing Error enum or native frame API.

Add one generic, evidence-backed transport classifier to eggfetch_core::Error
so Client::execute_http_body() callers and NativeResponseBody consumers can
distinguish connection establishment, TLS, HTTP protocol/framing, and
cancellation failures without Hyper/hyper-util/rustls downcasts and without
Display parsing.

Preserve the existing timeout, physical-admission, TransportIoTimeout,
DialErrorKind, and RequestFailure / NetworkFailureKind contracts.

M007 closes implementation and qualification only. Publication is a later
release-verification intake, so exact-pin downstream consumers remain blocked
until a release containing this API exists.

## 2. Current implementation evidence

At the baseline:

- crates/eggfetch-core/src/error.rs owns Error, RequestFailure,
  NetworkFailureKind, timeout helpers, custom_transport_error(), and the
  physical-admission helper.
- Client::send_detailed() carries RequestFailureContext only through the
  high-level URL pipeline.
- Client::execute_http_body() returns Result<Response<NativeResponseBody>> and
  does not allocate RequestFailureContext.
- transport/direct.rs::send_raw_request() calls map_send_error() without a
  detailed context.
- body.rs::IncomingErrorBody maps hyper::Error directly to Error::Hyper.
- retry.rs has a private source walk for hyper::Error::is_canceled().
- transport/direct.rs::map_send_error_for_proxy() separately searches nested
  Hyper errors for parse failure.

The gap is therefore a general Error-level classification seam, not a new
request pipeline.

## 3. Required public contract

Introduce one non-exhaustive public enum in the existing error domain.
Representative shape:

    #[non_exhaustive]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum TransportFailureKind {
        Connect,
        Tls,
        Protocol,
        Cancelled,
    }

Add one additive method on Error:

    pub fn transport_failure_kind(&self) -> Option<TransportFailureKind>;

Naming may change only if the replacement remains generic, contained in the
error domain, and is recorded in closure.

Semantics:

- Connect: typed evidence proves connection establishment failed and no more
  specific TLS/cancellation/protocol category applies.
- Tls: TLS configuration/handshake/verification or nested rustls processing
  failed.
- Protocol: typed HTTP framing/protocol failure, including Hyper
  parse/incomplete-message termination where proven.
- Cancelled: typed evidence says the transport/request was cancelled before
  normal completion.
- None: supported typed evidence does not prove one of those categories.

The classifier is diagnostic only and does not imply retryability.

## 4. Invariants

- Error remains the same exhaustive enum; no new variants and no
  #[non_exhaustive] on Error.
- Error::kind() tokens and Display output stay unchanged.
- execute_http_body() and NativeResponseBody signatures/shapes stay unchanged.
- RequestFailure / NetworkFailureKind semantics stay unchanged.
- TimeoutPhase owns request-phase timeout classification.
- TransportIoDirection owns established-I/O inactivity classification.
- is_physical_connection_admission_timeout() remains authoritative for its
  compatibility boundary.
- custom_transport_error()/DialErrorKind remain authoritative for
  caller-owned dialer timeout/authentication/rejection/connection facts.
- No classifier parses Display/Debug text.
- Unknown evidence stays unknown.
- No retry, redirect, proxy fallback, health, pool, deadline, TLS trust, or
  route-selection policy changes.
- No adapter behavior change, dependency, or feature flag.

## 5. Classification evidence and precedence

Implement one crate-private classifier authority used by the public Error
method and, only where semantics are identical, existing internal callers.

Required order:

1. Explicit Eggfetch error categories first.
   - TLS-family variants -> Tls.
   - unambiguous HTTP protocol/framing variants -> Protocol.
   - explicit Connect -> Connect.
   - CustomTransport preserves DialErrorKind; only Connection may map broadly
     to Connect if richer custom_transport_error() remains unchanged.
2. Wrapped typed causes next.
   - nested rustls::Error / Eggfetch-owned TLS cause -> Tls before generic
     connect;
   - nested hyper::Error::is_canceled() -> Cancelled;
   - nested hyper::Error::is_parse() or is_incomplete_message() -> Protocol;
   - hyper-util is_connect() / proven connection I/O -> Connect after more
     specific evidence is ruled out.
3. Bound every source-chain walk; no unbounded recursion.
4. Do not globally classify std::io::ErrorKind::UnexpectedEof as Protocol.
   Only classify it where an Eggfetch-owned HTTP/body boundary proves
   premature message termination.
5. Never expose transport-library/private connector types publicly.

## 6. Native dispatch requirements

Deterministic native HTTP/1 proof must cover:

- refused/failed connection -> Connect;
- nested TLS handshake or verification failure -> Tls;
- Hyper cancellation before a normal response -> Cancelled;
- malformed response-head/protocol parse failure -> Protocol.

Do not add execute_http_body_detailed(). The existing Error is the carrier.

## 7. NativeResponseBody requirements

NativeResponseBody continues to expose Error unchanged. Errors returned while
polling frames must classify through the same method.

Required cases:

- truncated Content-Length / incomplete HTTP message -> Protocol;
- malformed chunk framing or equivalent typed Hyper parse failure -> Protocol;
- body cancellation where Hyper supplies typed evidence -> Cancelled;
- ordinary EOF succeeds;
- read/total timeouts remain their existing typed timeout facts;
- error/drop still releases the PoolGuard as today.

If a required body case loses typed evidence before reaching Error, repair the
private mapping boundary rather than changing the public body error type.

## 8. Internal deduplication

After correctness is established, inspect private walkers in retry.rs and
transport/direct.rs.

Reuse the centralized private classifier only where policy semantics are
identical. Do not broaden RetryPolicy::is_error_retryable(), proxy fallback,
or other policy merely because TransportFailureKind contains Connect or
Cancelled.

Retaining a specialized private walk is acceptable when it requires richer
context; closure must document why.

## 9. Ordered work packages

### A — Public contract and unit evidence

Add TransportFailureKind, transport_failure_kind(), public compile/API
contract coverage, and precedence tests.

### B — Central private classifier

Implement bounded typed source inspection, unknown fallback, TLS-before-connect
precedence, cancellation, protocol, and connection classification.

### C — Native dispatch fixtures

Use loopback/local TLS/protocol fixtures; no public DNS or internet dependency.

### D — Native response-body fixtures

Prove incomplete/malformed framing, normal EOF, timeout preservation, and
lease release.

### E — Internal deduplication audit

Consolidate only exact-semantic duplicate inspection and pin existing
retry/proxy behavior with regression tests.

### F — Documentation and closure

Update docs/reference/errors.md, docs/architecture/core-engine.md,
docs/architecture/rust-surface-containment.md, .skills/rust-development.md,
and generated/public API evidence through repository tooling.

Write:
plans/closure/core-transport-policy/007-native-transport-failure-classification.md

After closure, route publication/version selection through
release-verification M003 standing intake.

## 10. Focused verification

Expected focused commands include:

    cargo test -p eggfetch-core --all-features transport_failure -- --test-threads=1
    cargo test -p eggfetch-core --all-features --test native_transport_failure_classification -- --test-threads=1

Native/lean profiles:

    cargo check -p eggfetch-core --no-default-features --features native-http1,tls-rustls
    cargo test -p eggfetch-core --no-default-features --features native-http1,tls-rustls --test native_transport_failure_classification -- --test-threads=1
    cargo check -p eggfetch-core --no-default-features --features standard-http1,tls-rustls

If one fixture intentionally needs advanced-routing, split the fixture rather
than weakening supported-profile evidence.

Also rerun existing focused lifecycle/custom-dialer/native-body tests covering
physical admission, I/O inactivity, cancellation recovery, and DialError
source preservation.

## 11. Broad qualification

Before closure:

    ./scripts/check.sh
    ./scripts/check.sh extended

Tier 2 is mandatory because M007 changes public Rust API and executable core
behavior. It must include the six-profile public API oracle, semver checks,
feature matrix, Rust 1.89.0 MSRV matrix, docs, and compatibility qualification.

Also inspect:

    cargo tree -p eggfetch-core
    cargo tree -p eggfetch-core -e features
    cargo tree -p eggfetch-core -d

Expected dependency delta: none.

Tier 3/package is deferred to the later release/publication milestone.

Because executable inputs change, renew exact-SHA Stage C qualification via
the existing ledger/profile process. Do not hardcode the live SHA in ordinary
architecture prose.

## 12. Security and privacy

The new output is enum-only. Never expose URL/query/userinfo, proxy
credentials, auth/cookies, TLS certificate text, request/response bodies,
OS messages, or arbitrary nested Display/Debug strings.

Add representative secret-bearing nested-error tests proving classification
does not surface those values or weaken existing redaction.

## 13. Compatibility

No config, wire, persistence, Python, CLI, FFI, Node, or HTTPX migration.

The Rust addition is additive. Existing exhaustive matching on Error remains
valid because Error itself is unchanged. TransportFailureKind is
non-exhaustive from birth.

NetworkFailureKind remains the high-level DNS/refusal provenance surface and
is not replaced.

## 14. Acceptance criteria

1. A public non-exhaustive generic transport-failure enum exists in the
   eggfetch_core error domain.
2. Error exposes one general classifier method for that enum.
3. Existing Error variants, kind tokens, Display, exhaustiveness, and Result
   alias are unchanged.
4. execute_http_body and NativeResponseBody signatures/shapes are unchanged.
5. Native dispatch nested TLS, cancellation, protocol, and connect failures
   classify without consumer Hyper/rustls downcasts.
6. Native body incomplete/malformed HTTP framing classifies Protocol without
   changing timeout/EOF semantics.
7. Admission, TimeoutPhase, TransportIoTimeout, DialErrorKind, and
   NetworkFailureKind retain authority.
8. Unknown/ambiguous errors return None; no message parsing exists.
9. Existing retry/proxy behavior is unchanged.
10. No dependency or feature flag is added.
11. native-http1,tls-rustls and standard-http1,tls-rustls profiles compile and
    focused classification tests pass.
12. Tier 1 and Tier 2 pass, including MSRV and API/semver oracle evidence.
13. Exact-SHA Stage C qualification is renewed.
14. Documentation clearly distinguishes Error::kind(),
    TransportFailureKind, NetworkFailureKind, timeout facts, and DialErrorKind.
15. Closure explicitly hands publication to release-verification and does not
    claim a release.

## 15. Stop conditions

Stop rather than widen scope if:

- classification requires changing Error variants or NativeResponseBody's
  public error type/shape;
- only Display/Debug parsing can distinguish a required category;
- a transport-library type would have to become public;
- a new dependency/feature flag becomes necessary without separate review;
- centralization changes retry, proxy fallback, timeout, route, pool, or TLS
  trust policy;
- UnexpectedEof cannot be classified contextually without false positives;
- public API/semver tooling finds an unanticipated break;
- exact-SHA qualification finds a facade/runtime regression.

## 16. Closure evidence required

The closure record must contain:

- implementation/final qualification SHAs;
- final public type/method names;
- category precedence and route/phase coverage table;
- dispatch and NativeResponseBody fixture results;
- TLS/cancellation/protocol/connect evidence;
- DialErrorKind, timeout, admission, and NetworkFailureKind non-regression;
- any retained internal source inspection and rationale;
- Tier 1/Tier 2/MSRV/API-oracle results;
- exact-SHA Stage C rebinding evidence;
- dependency-tree delta;
- security/redaction review;
- documentation updates;
- unresolved findings by severity;
- disposition;
- release-verification handoff and explicit note that exact-pin downstream
  consumers remain blocked until publication.

## 17. Non-goals

- No downstream-specific facade or policy.
- No Error redesign.
- No detailed native body wrapper.
- No DNS/refusal redesign.
- No retry/fallback/health changes.
- No timeout semantic changes.
- No proxy/H3 expansion.
- No adapter error-surface expansion.
- No CI workflow/matrix addition.
- No package publication or version selection.
