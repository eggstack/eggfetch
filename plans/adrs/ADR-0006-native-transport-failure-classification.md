# ADR-0006: Native Transport Failure Classification on the Existing Error Surface

Status: accepted

Date: 2026-10-01

Decision owners: project maintainers

Related specification sections:

- plans/000-long-term-specification.md §4.1, §4.3, §5, §8
- plans/001-terminology-and-domain-model.md §2, §4, §9
- plans/002-long-term-roadmap.md Phase 3 and Phase 4 public-helper gate

Affected subsystem roadmaps:

- plans/subsystems/core-transport-policy-roadmap.md

## Context

Eggfetch already exposes the stable eggfetch_core::Error enum plus an opt-in
RequestFailure / NetworkFailureKind path for high-level detailed sends. The
completed native-request-failure-introspection work established the correct
rule: capture typed provenance before private transport errors collapse, keep
Error and Error::kind() compatible, and never classify by Display text.

That detailed path does not cover Client::execute_http_body(), which returns
Error directly, or NativeResponseBody, whose frame error is also Error. Native
embedders therefore still need Hyper/hyper-util/rustls source-chain knowledge
to distinguish cancellation, malformed or incomplete HTTP framing, nested TLS
failure, and generic connection failure.

Eggfetch itself already performs some of the same source-chain inspection
internally for Hyper cancellation and proxy parse handling. Leaving these
facts private makes the public Error abstraction porous and duplicates
transport-library coupling in every embedder.

ADR-0005 freezes body shapes and low-level public helper growth and requires an
ADR/containment proof for genuine new public capability. This decision supplies
that proof. A downstream consumer motivated the review, but no downstream
type, policy, label, or retry behavior is part of this decision.

## Decision drivers

- Preserve the exhaustive Error enum and every existing Error::kind() token.
- Preserve NativeResponseBody::Error = Error and execute_http_body()'s return
  type.
- Hide Hyper, hyper-util, rustls, and platform error types behind an
  Eggfetch-owned classification.
- Use typed evidence with bounded source inspection; never parse Display or
  Debug text.
- Avoid a parallel detailed-native response/body API.
- Keep NetworkFailureKind DNS/refusal provenance separate from broader
  transport classification.
- Add no dependency, feature flag, transport owner, or policy owner.

## Considered options

### Option A — One additive classifier on Error (selected)

Add one non-exhaustive public enum in the existing error domain, representative
name TransportFailureKind, and one additive Error method, representative name
transport_failure_kind().

Initial categories:

- Connect
- Tls
- Protocol
- Cancelled

Unknown or ambiguous cases return None.

Existing orthogonal facts remain authoritative: Timeout/TimeoutPhase,
TransportIoTimeout/TransportIoDirection,
is_physical_connection_admission_timeout(), custom_transport_error() /
DialErrorKind, and RequestFailure::network_failure_kind().

### Option B — execute_http_body_detailed() plus a detailed body

Rejected. A request wrapper does not solve errors that arrive after response
headers. A parallel body/error type would duplicate the native frame surface
and widen Tower/native maintenance.

### Option C — New Error variants

Rejected. Error is exhaustively matchable. New variants would create avoidable
compatibility breakage.

### Option D — Public helpers under transport modules

Rejected. ADR-0005 and rust-surface-containment intentionally prevent adjacent
helper growth around lifecycle, metrics, dialer, direct connector, and pool.

### Option E — Keep consumer downcasts

Rejected. This leaks implementation details and duplicates brittle coupling.

## Decision

Select Option A.

The public addition is contained to eggfetch_core's existing error domain:
one non-exhaustive generic enum and one classifier method on Error.

No existing Error variant/field/kind token/Display output, Result alias,
NativeResponseBody shape, execute_http_body signature, feature alias, timeout
semantic, route policy, or adapter API changes.

The classifier is a diagnostic fact surface only. It does not decide retry,
fallback, health, or application action.

Classification precedence must be typed and explicit. Prefer existing
Eggfetch variants, then bounded typed causes where Error necessarily wraps a
transport-library error. Nested TLS evidence must outrank generic connect;
Hyper cancellation and parse/incomplete-message evidence must not require
consumer downcasts.

UnexpectedEof is context-sensitive. It must not be globally classified as HTTP
protocol failure solely from I/O kind. Use Hyper's typed incomplete-message /
parse evidence, or classify UnexpectedEof only at an Eggfetch-owned HTTP/body
boundary where truncation semantics are proven by a focused fixture.

## Consequences

### Positive

- Native dispatch and NativeResponseBody consumers use one unchanged Error
  type and one Eggfetch-owned classification method.
- Eggfetch can centralize duplicated source inspection.
- The addition is small, additive, dependency-free, and evolvable.

### Negative

- The Rust public API oracle changes and exact-SHA compatibility
  requalification is required.
- Eggfetch owns the semantics of these broad categories going forward.

### Neutral or deferred

- NetworkFailureKind remains the high-level DNS/refusal provenance surface.
- Publication/version selection stays with release-verification.
- More detailed subcategories require fresh evidence and review.

## Compatibility and migration

Existing callers are unchanged and may ignore the new method. Native embedders
may remove Hyper/rustls source downcasts after a release containing the API is
published.

No Python, CLI, FFI, Node, HTTPX, config, data, wire, or persistence migration
is authorized.

## Security and reliability implications

Classification output is enum-only. It must not expose credential-bearing
URLs, bodies, TLS peer text, proxy text, OS messages, or arbitrary nested
Debug/Display output. Unknown evidence returns None.

Existing retry, fallback, pool, timeout, TLS, and route decisions remain owned
by their existing components.

## Verification

Required evidence includes typed-source unit tests, loopback native dispatch
fixtures, malformed/truncated NativeResponseBody fixtures, TLS and
cancellation proofs, custom DialErrorKind non-regression, public API oracle
updates, native/lean profile checks, Tier 1, Tier 2/MSRV, exact-SHA Stage C
rebinding, and a no-new-dependency check.

## Supersession

None. This is a narrow containment exception under ADR-0005 and does not
weaken ADR-0005 for unrelated public helper growth.
