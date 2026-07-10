# Milestone P Plan: Authentication Subsystem

## Objective

Implement a reusable authentication subsystem in `eggfetch-core` and expose familiar authentication ergonomics through the Python sync and async clients.

The initial milestone should support Basic and Bearer authentication well, while establishing an abstraction capable of supporting Digest, API-key schemes, challenge-response flows, OAuth token refresh, and custom authentication later.

Authentication must be implemented as core request transformation/state logic. Python should normalize user input and configure the core; it must not maintain a separate auth engine.

## Prerequisites

Milestone N must be complete.

Milestone O should ideally be complete because cookie- and auth-based session flows interact during redirects. Authentication can begin in parallel with late cookie work only if request-pipeline ownership and redirect behavior are already stable.

Required stable foundations:

- cross-origin redirect header stripping
- replayability model
- logical redirect-chain timeout
- sync/async API parity
- client state and locking model

## Scope

Milestone P includes:

- core authentication abstraction
- Basic authentication
- Bearer-token authentication
- client-level auth
- request-level auth override/disable
- URL credential handling policy
- redirect security rules
- Python auth normalization
- structured tests

Milestone P does not initially include:

- Digest authentication
- OAuth authorization-code flows
- automatic browser login
- Kerberos/Negotiate/NTLM
- AWS SigV4
- mTLS client certificates
- proxy authentication beyond preserving extension points

## Core abstraction

Authentication should be modeled as a request modifier with room for stateful/challenge-driven flows.

A simple initial trait may look like:

```rust
pub trait Auth: Send + Sync {
    fn apply(&self, request: &mut Request) -> Result<()>;
}
```

However, future Digest or token-refresh auth may require response inspection and async behavior. Avoid freezing an abstraction that cannot evolve.

Recommended direction:

```rust
pub trait Auth: Send + Sync {
    fn prepare(&self, request: &mut Request) -> Result<()>;

    fn on_response(
        &self,
        request: &RequestMetadata,
        response: &ResponseMetadata,
    ) -> AuthAction {
        AuthAction::Done
    }
}
```

Or keep Basic/Bearer represented as an internal enum now while documenting a future richer auth trait. The key requirement is that auth application occurs centrally in the core pipeline.

## Initial auth types

Suggested public types:

```rust
pub enum AuthScheme {
    Basic(BasicAuth),
    Bearer(BearerAuth),
}

pub struct BasicAuth {
    username: String,
    password: String,
}

pub struct BearerAuth {
    token: String,
}
```

Secrets must not appear in `Debug`, `Display`, logs, error messages, or Python reprs.

Use redacted debug implementations.

## Basic authentication

Generate:

```text
Authorization: Basic base64(username:password)
```

Required decisions:

- encode username/password as UTF-8 bytes initially
- reject usernames containing `:` or document the ambiguity
- allow empty password
- do not log the generated value

A small base64 dependency may be justified. Add it with minimal features and document the dependency.

Basic auth should be preemptive when configured explicitly.

## Bearer authentication

Generate:

```text
Authorization: Bearer <token>
```

Validate enough to prevent header injection:

- reject CR/LF
- reject invalid header-value bytes

Do not impose token-format assumptions beyond header safety.

## Client and request configuration

Rust target:

```rust
let client = Client::builder()
    .auth(BasicAuth::new("user", "pass"))
    .build()?;

client.get(url).auth(BearerAuth::new(token)).send().await?;
client.get(url).without_auth().send().await?;
```

Required precedence:

1. request-level explicit auth
2. request-level auth disabled
3. client-level auth
4. no auth

An explicit user-supplied `Authorization` header must have a documented policy.

Recommended:

- explicit request Authorization header overrides configured auth for that request
- alternatively reject simultaneous header plus auth to avoid ambiguity

Choose one policy and test it. Prefer rejecting conflicting sources if the compatibility cost is acceptable.

## URL credentials

URLs such as:

```text
https://user:pass@example.com/
```

must have deliberate behavior.

Recommended:

- parse URL userinfo
- convert it to Basic auth only when no explicit auth/Authorization is configured
- remove userinfo from request URL and logs/repr
- never expose password in errors

Alternatively reject URL credentials initially. If supported, test redaction thoroughly.

## Redirect security

Authentication must be recomputed for every redirect destination.

Required rules:

- never blindly carry Authorization across an origin change
- strip Authorization and Proxy-Authorization cross-origin
- client-level auth scoped to one origin should not apply to unrelated redirect destinations
- same-origin redirect may retain/reapply auth
- HTTPS-to-HTTP downgrade must not retain credentials without an explicit unsafe policy

This implies auth needs scope information.

## Auth scope

Client auth should default to the origin of the request on which it is first applied or be configured as generally applicable only when safe.

For simple Python/client auth, expected behavior is usually to send auth to all requests made explicitly through that client, but not to leak it across cross-origin redirects.

Therefore distinguish:

- explicit new user request to another origin: configured client auth may apply
- automatic redirect to another origin: auth must be stripped and not automatically reapplied unless an explicit policy permits it

Pass redirect context into auth application or suppress auth on cross-origin redirect hops.

## Replay and challenge preparation

Basic/Bearer auth do not require body replay.

Keep future challenge-response needs in mind:

- Digest may require a second request after 401
- request body must be replayable
- timeout and retry budget must include auth challenge round trips

Do not implement this now, but avoid an API that treats all auth as one immutable header permanently attached before the first send.

## Python API

Target behavior:

```python
client = eggfetch.Client(auth=("user", "pass"))
response = client.get(url)

response = eggfetch.get(url, auth=("user", "pass"))
response = eggfetch.get(url, auth=eggfetch.BasicAuth("user", "pass"))
response = eggfetch.get(url, auth=eggfetch.BearerAuth(token))
```

Also support:

```python
client.get(url, auth=None)  # explicitly disable client auth for this request
```

Decide how omission differs from `None`. PyO3 signatures may need a sentinel to distinguish:

- not supplied: inherit client auth
- supplied `None`: disable auth

Do not collapse those two cases accidentally.

## Python auth classes

Expose:

- `BasicAuth`
- `BearerAuth`

Their reprs must redact secrets:

```python
<BasicAuth username='user' password=<redacted>>
<BearerAuth token=<redacted>>
```

Do not make token/password readable properties unless there is a compelling reason.

## Error handling

Invalid user auth configuration should raise Python `ValueError` or `TypeError` before networking.

Core errors may include:

- invalid auth header value
- conflicting Authorization sources
- unsupported auth scheme
- URL credential parse error

Do not include secret values in error messages.

## Logging and tracing

If tracing is enabled later, Authorization and Proxy-Authorization must always be redacted.

Add a central sensitive-header redaction helper now if logging/repr code handles headers.

Audit:

- request debug formatting
- response history formatting
- Python reprs
- exception messages
- test failure output

## Tests

### Core Basic tests

- correct Basic header generation
- empty password
- UTF-8 credentials under documented policy
- colon-in-username policy
- redacted debug output
- CR/LF rejection

### Core Bearer tests

- correct Bearer header
- opaque token preserved
- CR/LF rejection
- redacted debug output

### Precedence tests

- client auth applies
- request auth overrides client auth
- request auth disable works
- explicit Authorization conflict policy works
- URL credentials precedence works if supported

### Redirect tests

- same-origin redirect retains/reapplies auth
- cross-origin redirect strips auth
- cross-origin redirect does not reapply client auth automatically
- HTTPS-to-HTTP redirect strips auth
- redirect history contains no leaked secret representation

### Python tests

- auth tuple for sync helper
- auth tuple for async client
- BasicAuth object
- BearerAuth object
- request override
- request disable
- invalid tuple/type errors
- repr redaction
- exception redaction
- sync/async parity

## Feature gating

Authentication primitives are small enough to remain part of the default core if only Basic/Bearer are included, but the base64 dependency and future schemes may justify an `auth` feature.

Choose one policy deliberately and document it.

Recommendation:

- Basic/Bearer enabled by default for Python
- Rust core may use a default-enabled `auth` feature to preserve opt-out capability

## Documentation

Document:

- supported auth schemes
- precedence and override rules
- redirect credential stripping
- URL credential behavior
- secret redaction guarantees
- gaps versus requests/httpx

Security docs should explicitly warn against Basic auth over plaintext HTTP.

## Acceptance criteria

Milestone P is complete when:

- Basic and Bearer auth are implemented in the core
- secrets are redacted from all formatting/error paths
- client/request auth precedence is deterministic
- request-level disabling works
- cross-origin redirects never leak auth
- Python sync/async auth behavior matches
- tests cover redaction and redirect safety
- docs state security behavior clearly

## Validation

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

cd crates/eggfetch-python
maturin develop
python -m pytest
```

## Handoff note

Milestone Q should implement multipart/file uploads next. Keep authentication request transformation independent of body construction so auth can apply uniformly to raw, form, JSON, and multipart requests.
