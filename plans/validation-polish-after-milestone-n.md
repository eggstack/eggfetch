# Validation and Polish Plan After Milestone N

## Objective

Validate and polish the newly hardened streaming, redirect, cookie, authentication, timeout, and packaging behavior before beginning multipart/file uploads.

This pass is deliberately narrow. It should not add major product features. Its purpose is to prove that the public API introduced through Milestone N behaves correctly across lifecycle edges, feature combinations, supported Python versions, and operating systems.

## Scope

This pass includes:

- lifecycle review of Python `StreamingResponse`
- sync/async cancellation and close behavior
- cookie/auth redirect validation
- TLS trust-store behavior documentation and tests
- Windows CI and wheel smoke tests
- feature-matrix builds
- public API/repr/error audit
- documentation cleanup

This pass does not include:

- multipart uploads
- compression
- proxies
- retries
- HTTP/2
- CLI expansion

## Track 1: StreamingResponse lifecycle review

Audit `crates/eggfetch-python/src/streaming.rs` with particular attention to ownership and state transitions.

Required states:

- unread/open
- actively iterating
- buffered/read
- consumed
- closed

Validate behavior for:

- explicit `close()`/`aclose()`
- context-manager exit
- iterator exhaustion
- early loop break
- object drop without explicit close
- client close while response remains live
- client drop while response remains live
- second iterator request
- `read()` after partial iteration
- `text()` after partial iteration
- exception during decoding
- read timeout during iteration
- Python interpreter shutdown where practical

Do not allow silent empty results after invalid state transitions. Use named exceptions consistently.

### Tests

Add sync and async tests for every state transition.

At minimum:

- partial iteration followed by `read()` follows documented policy
- iterator after `close()` raises `StreamClosed`
- second consumption raises `StreamConsumed`
- `.content`/`.text` before read raises `ResponseNotRead` where intended
- dropping an unread response releases the pool permit
- closing client with live response does not deadlock
- response can safely outlive the client only if ownership model explicitly supports it; otherwise raise/close deterministically

## Track 2: GIL and async bridge audit

Review every network/body operation in the sync wrapper.

Confirm:

- GIL is released while blocking on Tokio
- no borrowed Python references cross the GIL release boundary
- no new runtime is created per chunk
- exceptions are converted after reacquiring the GIL

For async wrappers, confirm:

- cancellation drops pending futures
- no Tokio runtime nesting panic is possible
- `__aenter__`/`__aexit__` and async iterators follow protocol correctly
- cancellation during incremental decoding leaves response closeable

Add stress tests with concurrent async streams and repeated cancellation.

## Track 3: Redirect, cookie, and auth integration audit

Validate the request pipeline order explicitly:

1. merge client/request headers
2. compute cookie header for destination
3. resolve auth for current hop
4. validate body/content length
5. send
6. process Set-Cookie
7. apply redirect policy

Required security tests:

- same-origin redirect retains permitted auth
- cross-origin redirect strips request- and client-level auth
- HTTPS-to-HTTP redirect strips auth
- raw Cookie header is not reintroduced after cross-origin stripping
- jar cookies are recomputed for each destination
- intermediate redirect Set-Cookie is available to the next matching hop
- unrelated destination never receives host-only cookie
- redirect history contains no secret-bearing repr/error text

Add combined cookie+auth tests rather than testing each subsystem only in isolation.

## Track 4: Auth input policy review

Review permissive edge behavior for bearer tokens and Basic credentials.

Decide and document:

- whether empty bearer tokens are allowed
- whether bearer tokens containing spaces are allowed
- whether non-ASCII bearer values are allowed
- UTF-8 Basic credential behavior
- URL userinfo rejection behavior

Do not impose arbitrary restrictions solely for aesthetics, but make the policy deliberate and compatible with valid HTTP header values.

Add tests matching the final policy.

## Track 5: Cookie compatibility review

Audit:

- default path derivation
- leading-dot domain normalization
- IP literal handling
- same-name cookies across paths/domains
- deletion through Max-Age and Expires
- request-level `cookies=` isolation
- client constructor cookie persistence
- response cookie extraction from duplicate Set-Cookie headers

Ensure ambiguous Python lookup does not silently choose an arbitrary cookie.

Test with sync and async clients.

## Track 6: TLS trust-store behavior

The client now falls back from native roots to `webpki-roots` when native roots are unavailable.

Document clearly:

- native roots are preferred
- fallback roots may not include enterprise/private CAs
- no fallback occurs after certificate verification failure; fallback is only for unavailable native roots

Add tests where practical for:

- native root store construction success path
- fallback construction path through an injectable/test hook
- invalid certificate failure
- hostname mismatch

Avoid environment-dependent external TLS tests in the main suite.

## Track 7: Feature-matrix validation

Run and fix builds for meaningful feature combinations.

Required:

```sh
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --all-features
cargo test -p eggfetch-core --all-features
cargo test --workspace --all-features
```

Specifically validate cookies disabled and Python/default builds with cookies enabled.

Ensure optional modules and docs do not reference unavailable types under disabled features.

## Track 8: CI and wheel matrix

Extend CI to include Windows.

Target matrix:

- Ubuntu: Python 3.10-3.13
- macOS: Python 3.10-3.13
- Windows: Python 3.10-3.13

Add wheel smoke validation rather than only `maturin develop`:

1. build wheel
2. create clean environment
3. install wheel
4. import `eggfetch`
5. run a local-server GET and one streaming request

If full matrix wheel builds are expensive, use one wheel smoke version per OS and source-build tests across all declared Python versions.

## Track 9: CI status visibility

Confirm Actions trigger on push and pull request.

Ensure separate visible checks for:

- Rust formatting/lints/tests/docs
- Python tests
- wheel smoke builds

Add badges only after workflow names stabilize.

Document branch-protection recommendations.

## Track 10: Public API polish

Audit Python exports and reprs:

- `Client`
- `AsyncClient`
- `Response`
- `StreamingResponse`
- iterator types
- `Cookies`
- `Cookie`
- `BasicAuth`
- `BearerAuth`
- `NOAUTH`
- exception classes

Remove accidental implementation-detail exports.

Ensure secret-bearing objects are redacted.

Ensure sync and async method signatures align where intentional.

## Track 11: Documentation truth pass

Update:

- README
- architecture overview
- dependency policy
- feature flags
- compatibility matrix
- Python examples

Clarify:

- buffered versus live streaming APIs
- response body state rules
- cookie/auth redirect policy
- TLS root fallback
- supported Python/platform matrix
- current remaining limitations

## Acceptance criteria

This pass is complete when:

- all StreamingResponse lifecycle transitions are deterministic and tested
- pool permits release on every close/drop/error/cancellation path
- cookie/auth redirect behavior is security-tested
- auth and cookie edge policies are deliberate and documented
- TLS root fallback is documented accurately
- all required feature combinations build
- Windows CI is included
- clean wheel smoke tests pass on Linux, macOS, and Windows
- public exports and reprs are polished
- documentation matches code

## Required validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps

cd crates/eggfetch-python
maturin develop
python -m pytest
maturin build
```

## Handoff note

Begin Milestone Q only after this pass is green. Multipart will add another complex live-body producer and should build on proven streaming and cancellation semantics.
