# Corrective Pass Plan: Streaming, Cookies, and Authentication

## Objective

Close the remaining Milestone N gap and audit the newly landed cookie and authentication subsystems before beginning multipart/file uploads.

The repository now has substantial public Python behavior. This pass should verify that the semantics are safe under redirects, cancellation, streaming, concurrent use, feature-gated builds, and error handling. The priority is corrective validation, not feature expansion.

## Scope

This pass includes:

- true Python network streaming
- cookie scope and mutation semantics
- authentication scope and redirect behavior
- secret redaction audit
- feature-gated Rust builds
- sync/async parity
- wheel and CI validation
- documentation corrections

This pass does not include:

- multipart files
- proxies
- compression
- retries
- HTTP/2
- CLI expansion

# Track A: Complete true Python response streaming

## Required API

Implement live response streaming for both Python clients:

```python
with client.stream("GET", url) as response:
    for chunk in response.iter_bytes():
        ...

async with async_client.stream("GET", url) as response:
    async for chunk in response.aiter_bytes():
        ...
```

Buffered methods such as `get()` and `post()` should continue returning buffered `Response` objects.

## Core ownership

The Python streaming response must own the live Rust response body and its pool lease.

The lease must be released when:

- the body reaches EOF
- the response is explicitly closed
- iteration errors
- read timeout occurs
- Python cancellation drops the async read
- the Python object is garbage collected

## Sync implementation

The sync iterator should use the runtime already owned by `Client`.

Each `__next__` call must:

1. release the GIL
2. block on exactly one next-chunk future
3. return Python `bytes`
4. translate errors without consuming unrelated client state

Do not create a runtime per chunk.

## Async implementation

Use the existing `pyo3-async-runtimes` bridge.

Cancellation must drop the pending future and leave the client reusable.

## Body state machine

Define explicit states:

- unread
- streaming
- buffered
- consumed
- closed

Invalid transitions should raise stable exceptions, for example:

- `ResponseNotRead`
- `StreamConsumed`
- `StreamClosed`

Add `read()` and `aread()` for consuming the remaining live body into cached content.

## Text and line streaming

Implement incremental decoding.

Do not decode each chunk independently because multibyte code points may cross chunk boundaries.

`iter_lines()` and `aiter_lines()` must preserve line fragments between chunks.

## Required tests

- first chunk arrives before the complete body is sent
- large response is not eagerly buffered
- sync early break plus close releases the permit
- async cancellation releases the permit
- read timeout on a later chunk raises the correct exception
- client remains reusable after stream error/cancellation
- split UTF-8 character decodes correctly
- line delimiter split across chunks is handled correctly
- double consumption and use-after-close raise documented exceptions

# Track B: Cookie semantics audit

## Request-local versus persistent cookies

Define and test exact behavior for:

```python
Client(cookies=...)
client.cookies.set(...)
client.get(url, cookies=...)
eggfetch.get(url, cookies=...)
```

Recommended semantics:

- `Client(cookies=...)` initializes persistent client state
- `client.cookies.set()` mutates persistent state
- request-level `cookies=` affects only that request
- response `Set-Cookie` updates persistent state when a persistent client is used
- top-level helpers use a short-lived jar only for that request chain

Request-level cookies must not silently mutate the persistent jar before a response is received.

## Redirect behavior

For every redirect hop:

1. ingest `Set-Cookie` from the previous response
2. compute destination URL
3. recompute matching cookies from the jar
4. never carry a serialized Cookie header blindly

Required tests:

- intermediate redirect cookie is available on the next matching hop
- host-only cookie does not cross hosts
- Domain cookie follows only valid subdomains
- Secure cookie does not cross HTTPS to HTTP
- explicit raw Cookie header does not leak cross-origin
- request-level cookies do not become persistent unexpectedly

## Same-name ambiguity

Define Python lookup behavior when multiple cookies share a name across domains or paths.

Recommended:

- `cookies.get(name)` raises an ambiguity error if more than one match exists without domain/path qualifiers
- add domain/path parameters for precise lookup

Do not return an arbitrary cookie.

## Expiry and replacement

Add deterministic clock-driven tests for:

- Max-Age precedence
- deletion
- replacement by name/domain/path
- lazy expiry cleanup
- default path computation

## Feature-gated builds

Run and fix:

```sh
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls
cargo check -p eggfetch-core --all-features
```

Ensure cookie types are conditionally exported and no unconditional references break cookie-disabled builds.

# Track C: Authentication semantics audit

## Cross-origin redirect behavior

Verify that stripping `Authorization` is not followed by accidental reapplication of client-level auth on the redirected request.

Required policy:

- explicit new user request to another origin may use configured client auth
- automatic cross-origin redirect must not reapply client auth by default
- same-origin redirect may retain/reapply auth
- HTTPS-to-HTTP redirect must strip auth

Pass redirect context into auth resolution if needed.

## Request override semantics

Distinguish:

- auth argument omitted: inherit client auth
- `auth=None`: disable client auth for that request
- request auth object/tuple: override client auth

Ensure PyO3 signatures preserve this distinction with a sentinel if necessary.

## Explicit Authorization conflicts

Choose and test one policy:

- explicit Authorization header wins, or
- simultaneous `auth=` and Authorization raises a conflict error

Recommended: reject conflicting sources to avoid ambiguous security behavior.

## URL credentials

If URL userinfo is unsupported, reject it clearly and ensure passwords never appear in errors.

If supported, convert to Basic auth only when no explicit auth exists and redact the URL everywhere.

Do not leave behavior accidental.

## Secret redaction audit

Search and test all output paths:

- Rust Debug/Display
- Python repr
- exceptions
- request/response history
- assertion failures where practical
- tracing/logging helpers
- URL formatting

Add tests asserting that raw password/token values do not occur in formatted output.

## Feature-gated builds

If auth is feature-gated, test with and without it. If it is intentionally always enabled, document that decision.

# Track D: Cookie/auth interaction

Test realistic session flows:

- Basic auth request receives a session cookie
- redirect follows using cookie but does not unnecessarily resend stripped credentials cross-origin
- same-origin redirect preserves intended auth and cookie behavior
- request-level auth disable does not disable cookie handling
- raw Authorization and Cookie headers follow their documented precedence rules

Ensure the request pipeline order is explicit:

1. defaults
2. request headers
3. auth resolution
4. cookie injection
5. redirect safety transformations
6. final protocol header normalization

Adjust order if necessary, but document it.

# Track E: Sync/async parity

Extend `test_parity.py` or shared fixtures to cover:

- client cookies
- request-local cookies
- cookie updates from responses
- Basic auth
- Bearer auth
- auth override
- auth disable
- redirect auth stripping
- streaming response state/errors

Both clients should expose the same kwargs, defaults, exceptions, and observable behavior except where sync versus async protocol requires different method names (`close`/`aclose`, iterator types).

# Track F: Packaging and CI closure

## Python matrix

Validate Python 3.10-3.13.

Build wheels for at least:

- Linux x86_64
- macOS arm64
- Windows x86_64

Each wheel should be installed into a clean environment and pass:

- import smoke test
- basic request
- cookie persistence test
- auth request test
- streaming first-chunk test

## CI

Ensure visible push/PR checks for:

- fmt
- clippy
- Rust tests
- docs
- Python build/tests
- feature-matrix checks
- wheel smoke builds

The current absence of visible combined statuses must be resolved or documented with a concrete repository-setting limitation.

# Track G: Documentation truth pass

Update:

- README.md
- AGENTS.md
- CONTRIBUTING.md
- docs/architecture/overview.md
- docs/architecture/feature-flags.md
- docs/architecture/dependency-policy.md
- plans/ROADMAP.md

Required documentation:

- buffered versus live streaming APIs
- cookie persistence and request-local semantics
- ambiguous cookie lookup behavior
- auth inheritance/override/disable semantics
- cross-origin redirect credential policy
- feature availability
- supported Python/platform matrix

Do not mark Milestone N complete until live Python streaming and CI/package validation are genuinely complete.

# Suggested implementation order

1. Audit auth reapplication across redirects and fix it.
2. Audit request-local cookie mutation and redirect cookie recomputation.
3. Add feature-gated build checks.
4. Implement sync live streaming.
5. Implement async live streaming and cancellation.
6. Add incremental text/line decoding.
7. Complete parity tests.
8. Add wheel matrix and visible CI checks.
9. Complete documentation updates.

# Final acceptance criteria

This corrective pass is complete when:

- Python sync and async clients expose true live response streaming
- streaming cancellation/close reliably releases pool permits
- cross-origin redirects do not reapply client auth
- request-level cookies do not unexpectedly mutate persistent state
- cookies are recomputed safely for every redirect destination
- ambiguous cookie lookup is deterministic
- auth secrets are absent from all formatting/error paths
- cookie-disabled and relevant feature combinations compile
- sync/async parity tests cover cookies, auth, and streaming
- wheels pass clean-environment smoke tests on the declared matrix
- CI checks are visible and reliable
- documentation matches actual behavior

## Validation commands

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check -p eggfetch-core --no-default-features
cargo check -p eggfetch-core --no-default-features --features http1,tls-rustls

cd crates/eggfetch-python
maturin develop
python -m pytest
maturin build
```

## Handoff note

Do not start multipart/file uploads until this pass is complete. Multipart depends directly on correct live streaming, timeout, redirect replay, auth, cookie, and cancellation behavior.
