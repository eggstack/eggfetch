# Authentication

eggfetch supports HTTP Basic and Bearer token authentication with redacted secrets, precedence resolution, and cross-origin credential stripping.

## Auth Schemes

### Basic Authentication

`BasicAuth` encodes `username:password` as Base64 and sets the `Authorization: Basic <encoded>` header.

```rust
use eggfetch_core::{BasicAuth, AuthScheme};

let auth = AuthScheme::Basic(BasicAuth::new("user", "password")?);
```

```python
import eggfetch

# Explicit Basic auth
auth = eggfetch.BasicAuth("user", "password")
response = client.get(url, auth=auth)

# Tuple shorthand
response = client.get(url, auth=("user", "password"))
```

### Bearer Authentication

`BearerAuth` sets the `Authorization: Bearer <token>` header.

```rust
use eggfetch_core::{BearerAuth, AuthScheme};

let auth = AuthScheme::Bearer(BearerAuth::new("my-token")?);
```

```python
auth = eggfetch.BearerAuth("my-token")
response = client.get(url, auth=auth)
```

## Precedence Resolution

When both client-level and request-level auth are configured, the following precedence applies:

1. Request-level explicit auth (via `.auth()`)
2. Request-level auth disabled (via `.without_auth()`)
3. Client-level auth (via `ClientBuilder::auth()`)
4. No auth

If a raw `Authorization` header is set on the request and auth is also configured, an error is raised to prevent ambiguity.

## Disabling Auth Per-Request

Use `without_auth()` to prevent client-level auth from being applied to a specific request:

```python
# Client has auth configured, but this request must not carry credentials
response = client.get(url, auth=eggfetch.NOAUTH)
```

`NOAUTH` is a sentinel object, not `None`. Using `None` falls back to client-level auth.

## Cross-Origin Credential Stripping

On cross-origin redirects, `Authorization`, `Cookie`, and `Proxy-Authorization` headers are stripped. Client-level auth is not reapplied on cross-origin hops. Same-origin redirects do reapply client-level auth.

## URL Credentials

URL userinfo (e.g., `https://user:pass@host/`) is rejected. Configure `BasicAuth` or another explicit auth scheme instead. The password is not echoed in the resulting error.

## Input Validation

- Basic auth usernames must not contain `:` (RFC 7617 ambiguity)
- CR/LF characters in usernames, passwords, or bearer tokens are rejected to prevent header injection
- Bearer tokens may be empty, contain spaces, or contain UTF-8

## Secret Redaction

`AuthScheme`, `BasicAuth`, and `BearerAuth` implement custom `Debug` and `Display` traits that redact sensitive values. Credentials are never printed in logs, error messages, or repr diagnostics.

```rust
let auth = BasicAuth::new("admin", "secret123")?;
println!("{auth:?}"); // BasicAuth { username: "admin", password: "<redacted>" }
```

## Python API

```python
import eggfetch

# Client-level auth (applies to all requests)
client = eggfetch.Client(auth=eggfetch.BasicAuth("user", "pass"))

# Request-level auth (overrides client)
response = client.get(url, auth=eggfetch.BearerAuth("token"))

# Disable auth for one request
response = client.get(url, auth=eggfetch.NOAUTH)
```

## CLI

```bash
# Basic auth
eggfetch --auth user:password https://example.com

# Bearer auth
eggfetch --bearer my-token https://example.com

# Auth via environment variables
export EGGFETCH_AUTH="user:password"
export EGGFETCH_BEARER="my-token"
eggfetch https://example.com
```
