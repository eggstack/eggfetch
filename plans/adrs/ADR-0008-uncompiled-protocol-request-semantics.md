# ADR-0008: Uncompiled-Protocol Request Semantics at Adapter Boundaries

Status: accepted

Date: 2026-10-22

Accepted: 2026-10-22, on maintainer instruction to execute
`plans/implementation/tls-proxy-protocols/005-uncompiled-protocol-fail-closed.md`,
which depends on this decision.

Decision owners: project maintainers

Related specification sections:

- `plans/000-long-term-specification.md` §4.1, §5
- `plans/001-terminology-and-domain-model.md` §6–§7

Affected subsystem roadmaps:

- `plans/subsystems/tls-proxy-protocols-roadmap.md`
- `plans/subsystems/core-transport-policy-roadmap.md`
- `plans/subsystems/release-verification-roadmap.md`

## Context

`eggfetch-core` compiles protocol support behind cargo features: `transport-http1`,
`transport-http2`, and `http3`. The public `HttpVersionPolicy` type offers
`Http1Only`, `Http2Only`, `Http3Only`, and `Auto { allow_http3 }`.

The engine resolves a policy to a transport through
`HttpVersionPolicyEnabler::from_policy`
(`crates/eggfetch-core/src/http_version.rs`). When the requested capability is
not compiled in, that function downgrades:

- `Http2Only` and `Auto` become `Http1Only` without `transport-http2`;
- `Http3Only` becomes `Http1Only` without `http3`.

The downgrade is deliberate and documented in the engine, and `AGENTS.md`
restates it for native Rust callers. It is the right contract for a library
whose caller has not been compiled against a protocol: silently negotiating
H1 for a native caller is predictable and keeps the engine small.

Adapters are different. `eggfetch-cli` exposes `--http2` and `--http3` as
user-facing flags, and `eggfetch-ffi` accepts an HTTP-version policy value
through `eggfetch_client_builder_http_version`. In both, the user states a
protocol expectation in a *per-request* control surface. When the built binary
or library cannot honor it, the current behavior is:

- **CLI `--http2`**: the request is issued over HTTP/1.1 with no warning. The
  user asked for H2 and silently received H1.
- **CLI `--http3`**: the request fails with `Error::Unsupported("HTTP client not
  available for this protocol")`. The cause is not the missing H3 capability
  but the absence of a Hyper client in the `Http3Only` construction path
  (`crates/eggfetch-core/src/client.rs`), and the message does not say so.
- **FFI**: `eggfetch_client_builder_http_version` accepts the `Http2Only` and
  `Auto { allow_http3: true }` values without consulting the compiled feature
  set, so an FFI consumer can request a protocol the linked build cannot
  provide.

This state also contradicts the repository's own documentation.
`docs/architecture/cli.md` states that *"Requesting an uncompiled capability
fails instead of silently downgrading."* The engine downgrades; the CLI does
not fail. The documented contract and the shipped behavior disagree.

Two further properties make the current state undesirable:

1. **Build-invocation dependence.** Cargo unifies features across a workspace
   build, so `cargo build -p eggfetch-cli` and `cargo build --workspace`
   produce binaries with different protocol capability from the same source.
   Nothing in the CLI reports which capability it actually has.
2. **No capability introspection.** No adapter can currently answer "which
   protocols does this build support?" without duplicating the feature
   mapping by hand.

## Decision drivers

- A user who states a protocol expectation must not silently receive a
  different protocol.
- The engine's documented downgrade for native Rust callers must not change:
  it is a public contract, it is recorded in `AGENTS.md`, and it keeps the
  engine's surface small.
- Adapters must not need to duplicate engine knowledge to know their own
  compiled capability.
- The fix must not add a second protocol-negotiation implementation.
- The documented CLI contract must become true, or be corrected; the two must
  not continue to disagree.
- Feature-profile isolation must be preserved: the lean and native profiles
  must not acquire capability they did not have.

## Considered options

### Option A — Engine fails closed for strict policies

Change `HttpVersionPolicyEnabler::from_policy` so that `Http2Only` without
`transport-http2`, and `Http3Only` without `http3`, are errors rather than
downgrades.

Rejected for the native Rust surface: it removes a documented engine
contract, changes behavior for existing native callers who build without
`transport-http2`, and expands the error surface of a library whose callers
chose their feature set. It also conflates "the engine cannot do this" with
"the caller asked for something invalid".

### Option B — Adapters fail closed at their own boundary; engine unchanged

Keep the engine's downgrade for native Rust callers. Give each adapter a
compile-time-accurate statement of the protocol capability of the core it was
linked against, and have the adapter reject a strict protocol request that the
linked core cannot honor, with an error that names the missing capability.

The engine remains the single implementation of protocol negotiation; the
adapter only decides whether the *request the user stated* is executable.

### Option C — Adapters warn instead of failing

Emit a warning when a strict protocol request is downgraded.

Rejected: a warning is not observable to a library consumer (the FFI has no
warning channel), does not stop a user from depending on H2-specific behavior
that did not happen, and leaves the documented CLI contract false.

### Option D — Compile the missing capability into the adapters

Enable `http2` in the CLI and `http3` in the CLI by default so the request can
always be honored.

Rejected: it contradicts `docs/architecture/feature-flags.md` and the
deliberate CLI feature ladder (the CLI is a cleartext-oriented, HTTP/1.1 tool
that must stay small), and it does not solve the FFI case, where the consumer
chooses the feature set.

## Decision

**Option B.**

1. **The engine's documented downgrade is unchanged.** `Http2Only`/`Auto`
   continue to resolve to `Http1Only` when `transport-http2` is absent, and
   `Http3Only` continues to resolve to `Http1Only` when `http3` is absent, for
   native Rust callers. `AGENTS.md` and the `http_version.rs` documentation
   remain accurate. ADR-0001's single-engine boundary is preserved: the
   engine still performs all protocol negotiation.

2. **An adapter that exposes a user-facing protocol control must fail closed
   when the linked engine cannot honor a strict request.** The request is
   rejected before any connection is attempted, with an error that names the
   specific missing capability and states that the built artifact does not
   support it. A silent downgrade is never acceptable at an adapter boundary
   that offers the user a protocol choice.

3. **`Auto` is not a strict request.** `Auto { allow_http3: false }` and
   `Auto { allow_http3: true }` continue to negotiate down to H1 when H3 is
   unavailable. Only `Http2Only` and `Http3Only` are strict.

4. **Capability knowledge must be derived, not duplicated.** Each adapter
   expresses its core feature forwarding in its own `Cargo.toml` and reads it
   with `cfg!`, so an adapter's compiled capability and the core's compiled
   capability cannot drift. The CLI gains `http2`/`http3` features that
   forward to the corresponding core features and are off by default;
   `eggfetch-ffi` already has equivalent forwarding features.

5. **The engine's `Http3Only` error text must not mislead.** Where an
   uncompiled `Http3Only` reaches the engine (a native Rust caller, or an
   adapter that did not apply rule 2), the failure must not be reported as a
   missing Hyper client. The engine must distinguish "this build has no H3
   support" from "this request had no usable client", because those are
   different operator problems. This is a message-classification correction
   inside the existing error domain, not a new error type.

6. **`docs/architecture/cli.md` must state the implemented behavior.** The
   sentence claiming that an uncompiled capability "fails instead of silently
   downgrading" becomes true under rule 2, and the document must additionally
   state that `Auto` still downgrades.

## Consequences

### Positive

- A user who requests H2 or H3 through the CLI or FFI either gets that
  protocol or a clear error naming the missing capability.
- The engine keeps one protocol-negotiation implementation and its documented
  native-caller contract.
- The documented CLI contract and the shipped behavior stop disagreeing.
- Build-invocation differences become observable: each built artifact reports
  its own capability rather than behaving differently silently.
- The correction in rule 5 removes a misleading error class that affected
  native callers too.

### Negative

- Adapters acquire a small amount of capability-awareness logic that the
  engine does not own. This is a boundary check, not a second implementation
  of negotiation.
- The CLI's `--http2`/`--http3` behavior depends on the CLI's compiled
  features, so a workspace build and a standalone build of the same source
  differ in supported flags. This is intended: the artifact reports what it
  can do.
- Existing native callers that relied on the `Http3Only`-without-H3 error text
  will see a different message.

### Neutral or deferred

- H3 graduation is unaffected; HTTP/3 remains experimental.
- The FFI's missing capabilities beyond protocol selection (proxy, multipart,
  custom CA, UDS, dialer, SNI, trailers, metrics, retry policy) are a separate
  surface-coverage question and are explicitly out of scope here.
- Adding a public engine API that reports compiled capability is not required
  by this decision. If a later milestone finds that adapters need it, it must
  be proposed as its own bounded change under ADR-0005 containment.

## Compatibility and migration

- Native Rust callers: no behavioral change except the corrected `Http3Only`
  error message.
- CLI: `--http2` and `--http3` on a build without the corresponding feature
  change from "silently downgrade" or "misleading error" to an explicit
  failure. Builds with the feature enabled are unaffected.
- FFI: `eggfetch_client_builder_http_version` with a strict value that the
  linked build cannot honor now fails at request time with a named cause
  instead of downgrading.
- HTTPX 0.28.1 and httpx2 2.12.0 Stage C bindings are unaffected in
  principle: the facades do not expose a strict protocol control. The
  engine-side message correction in rule 5 is an executable change and must
  therefore be requalified at an exact SHA under
  `.skills/verification-qualification.md`.
- No crate version bump is implied by this ADR.

## Security and reliability implications

- Silent protocol downgrade is a reliability hazard: a caller may depend on
  H2-only behavior (multiplexing, header compression, stream priority) and
  receive H1 without knowing. Failing closed removes that class of silent
  capability loss.
- Failing before any connection attempt avoids opening a connection that
  would then be abandoned.
- The corrected `Http3Only` classification prevents an operator from
  misdiagnosing a build-configuration problem as a transport failure.

## Verification

- CLI integration tests asserting that `--http2`/`--http3` fail with a
  capability-naming error on a build without the feature, and succeed against
  a suitable server on a build with it.
- FFI tests asserting the same for `eggfetch_client_builder_http_version`.
- A test proving `Auto` still downgrades to H1 without failing.
- A test proving the engine's strict-policy-without-feature error names the
  missing capability rather than a missing client.
- Tier 1 and Tier 2 green, with the public API oracle unchanged unless the
  engine-side correction changes public surface (it must not).

## Supersession

None.
