//! Authentication subsystem for eggfetch.
//!
//! Provides Basic and Bearer authentication with redacted debug output,
//! CR/LF injection prevention, and a central apply pipeline.
//!
//! Authentication is applied centrally in the client pipeline. Secrets
//! are never exposed in `Debug`, `Display`, logs, or error messages.

use crate::error::{Error, Result};

/// An authentication scheme applied to outgoing requests.
///
/// Authentication is configured at the client level and can be overridden
/// or disabled per-request. The client applies auth centrally before
/// sending, after default headers but before the hyper request is built.
///
/// # Precedence
///
/// 1. Request-level explicit auth (via [`RequestBuilder::auth`])
/// 2. Request-level auth disabled (via [`RequestBuilder::without_auth`])
/// 3. Client-level auth (via [`ClientBuilder::auth`])
/// 4. No auth
///
/// [`RequestBuilder::auth`]: crate::request::RequestBuilder::auth
/// [`RequestBuilder::without_auth`]: crate::request::RequestBuilder::without_auth
/// [`ClientBuilder::auth`]: crate::client::ClientBuilder::auth
#[derive(Clone)]
pub enum AuthScheme {
    /// HTTP Basic Authentication (`Authorization: Basic <credentials>`).
    Basic(BasicAuth),
    /// HTTP Bearer Token Authentication (`Authorization: Bearer <token>`).
    Bearer(BearerAuth),
}

impl AuthScheme {
    /// Create a Basic auth scheme from username and password.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidAuthHeader`] if the username contains `:`
    /// or if credentials contain invalid header-value bytes.
    pub fn basic(username: impl Into<String>, password: impl Into<String>) -> Result<Self> {
        Ok(Self::Basic(BasicAuth::new(username, password)?))
    }

    /// Create a Bearer auth scheme from a token.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidAuthHeader`] if the token contains CR/LF
    /// or invalid header-value bytes.
    pub fn bearer(token: impl Into<String>) -> Result<Self> {
        Ok(Self::Bearer(BearerAuth::new(token)?))
    }

    /// Apply this auth scheme to the request headers.
    ///
    /// Sets the `Authorization` header to the appropriate value.
    /// Returns an error if the header value is invalid.
    pub(crate) fn apply(&self, headers: &mut crate::headers::Headers) -> Result<()> {
        match self {
            Self::Basic(b) => {
                let value = b.header_value();
                headers.insert("authorization", &value)?;
            }
            Self::Bearer(b) => {
                let value = b.header_value();
                headers.insert("authorization", &value)?;
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for AuthScheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Basic(b) => f.debug_tuple("Basic").field(b).finish(),
            Self::Bearer(b) => f.debug_tuple("Bearer").field(b).finish(),
        }
    }
}

/// HTTP Basic Authentication credentials.
///
/// Generates `Authorization: Basic base64(username:password)` headers.
///
/// # Security
///
/// The `Debug` and `Display` implementations redact the password.
/// The raw password is never exposed in error messages or logs.
///
/// # Policy
///
/// - Usernames containing `:` are rejected (RFC 7617 ambiguity).
/// - Passwords may be empty.
/// - Credentials are encoded as UTF-8 bytes.
/// - CR/LF characters in username or password are rejected.
#[derive(Clone)]
pub struct BasicAuth {
    username: String,
    password: String,
}

impl BasicAuth {
    /// Create new Basic auth credentials.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidAuthHeader`] if:
    /// - The username contains `:`
    /// - The username or password contains CR (`\r`) or LF (`\n`)
    /// - The credentials produce an invalid header value
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Result<Self> {
        let username = username.into();
        let password = password.into();

        if username.contains(':') {
            return Err(Error::InvalidAuthHeader(
                "basic auth username must not contain ':'".into(),
            ));
        }

        validate_auth_bytes(username.as_bytes(), "username")?;
        validate_auth_bytes(password.as_bytes(), "password")?;

        Ok(Self { username, password })
    }

    /// Returns the Base64-encoded header value (without the scheme prefix).
    fn encoded_credentials(&self) -> String {
        use base64::Engine;
        let credential = format!("{}:{}", self.username, self.password);
        base64::engine::general_purpose::STANDARD.encode(credential.as_bytes())
    }

    /// Returns the full `Authorization` header value.
    fn header_value(&self) -> String {
        format!("Basic {}", self.encoded_credentials())
    }

    /// Returns a reference to the username.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }
}

impl std::fmt::Debug for BasicAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BasicAuth")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

impl std::fmt::Display for BasicAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BasicAuth(username={})", self.username)
    }
}

/// HTTP Bearer Token Authentication credentials.
///
/// Generates `Authorization: Bearer <token>` headers.
///
/// # Security
///
/// The `Debug` and `Display` implementations redact the token.
/// The raw token is never exposed in error messages or logs.
///
/// # Validation
///
/// - CR (`\r`) and LF (`\n`) characters are rejected to prevent header injection.
/// - No token format assumptions beyond header safety.
#[derive(Clone)]
pub struct BearerAuth {
    token: String,
}

impl BearerAuth {
    /// Create new Bearer auth credentials.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidAuthHeader`] if the token contains CR/LF
    /// or produces an invalid header value.
    pub fn new(token: impl Into<String>) -> Result<Self> {
        let token = token.into();
        validate_auth_bytes(token.as_bytes(), "token")?;
        Ok(Self { token })
    }

    /// Returns the full `Authorization` header value.
    fn header_value(&self) -> String {
        format!("Bearer {}", self.token)
    }
}

impl std::fmt::Debug for BearerAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BearerAuth")
            .field("token", &"<redacted>")
            .finish()
    }
}

impl std::fmt::Display for BearerAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BearerAuth(<redacted>)")
    }
}

/// Validate that auth credential bytes are safe for use in header values.
///
/// Rejects CR and LF to prevent header injection.
fn validate_auth_bytes(bytes: &[u8], field_name: &str) -> Result<()> {
    if bytes.contains(&b'\r') {
        return Err(Error::InvalidAuthHeader(format!(
            "auth {field_name} must not contain CR"
        )));
    }
    if bytes.contains(&b'\n') {
        return Err(Error::InvalidAuthHeader(format!(
            "auth {field_name} must not contain LF"
        )));
    }
    Ok(())
}

/// Determine the effective auth for a request.
///
/// Precedence:
/// 1. Request-level explicit auth
/// 2. Request-level auth disabled
/// 3. Client-level auth
/// 4. No auth
///
/// If the request has an explicit `Authorization` header AND auth is
/// configured, returns `Err` to avoid ambiguity (the user must choose
/// one or the other).
pub(crate) fn resolve_request_auth(
    request_auth: Option<&AuthScheme>,
    request_auth_disabled: bool,
    client_auth: Option<&AuthScheme>,
    request_headers: &crate::headers::Headers,
) -> Result<Option<AuthScheme>> {
    // Check for conflicting Authorization header + auth config.
    if request_headers.contains("authorization") {
        if request_auth.is_some() || client_auth.is_some() {
            return Err(Error::ConflictingAuth(
                "explicit Authorization header conflicts with configured auth; \
                 use .auth() or remove the header, but not both"
                    .into(),
            ));
        }
        // User set a raw Authorization header with no auth config — that's fine.
        return Ok(None);
    }

    // Request-level explicit auth overrides everything.
    if let Some(auth) = request_auth {
        return Ok(Some(auth.clone()));
    }

    // Request-level auth disabled.
    if request_auth_disabled {
        return Ok(None);
    }

    // Client-level auth.
    if let Some(auth) = client_auth {
        return Ok(Some(auth.clone()));
    } // No auth.
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- BasicAuth tests ---

    #[test]
    fn basic_auth_header_value() {
        let auth = BasicAuth::new("user", "pass").unwrap();
        // user:pass = "dXNlcjpwYXNz" in base64
        assert_eq!(auth.header_value(), "Basic dXNlcjpwYXNz");
    }

    #[test]
    fn basic_auth_empty_password() {
        let auth = BasicAuth::new("user", "").unwrap();
        // user: = "dXNlcjo=" in base64
        assert_eq!(auth.header_value(), "Basic dXNlcjo=");
    }

    #[test]
    fn basic_auth_utf8_credentials() {
        use base64::Engine;
        let auth = BasicAuth::new("usér", "päss").unwrap();
        assert!(auth.header_value().starts_with("Basic "));
        // Verify it can be decoded
        let header_val = auth.header_value();
        let encoded = header_val.strip_prefix("Basic ").unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "usér:päss");
    }

    #[test]
    fn basic_auth_rejects_colon_in_username() {
        let err = BasicAuth::new("user:name", "pass").unwrap_err();
        assert_eq!(err.kind(), "invalid_auth_header");
        assert!(!err.to_string().contains("pass"));
    }

    #[test]
    fn basic_auth_rejects_cr_in_username() {
        let err = BasicAuth::new("user\r", "pass").unwrap_err();
        assert_eq!(err.kind(), "invalid_auth_header");
    }

    #[test]
    fn basic_auth_rejects_lf_in_password() {
        let err = BasicAuth::new("user", "pass\n").unwrap_err();
        assert_eq!(err.kind(), "invalid_auth_header");
    }

    #[test]
    fn basic_auth_redacted_debug() {
        let auth = BasicAuth::new("user", "secret123").unwrap();
        let debug = format!("{auth:?}");
        assert!(debug.contains("user"));
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("secret123"));
    }

    #[test]
    fn basic_auth_redacted_display() {
        let auth = BasicAuth::new("user", "secret123").unwrap();
        let display = format!("{auth}");
        assert!(display.contains("user"));
        assert!(!display.contains("secret123"));
    }

    #[test]
    fn basic_auth_username_accessor() {
        let auth = BasicAuth::new("admin", "pw").unwrap();
        assert_eq!(auth.username(), "admin");
    }

    // --- BearerAuth tests ---

    #[test]
    fn bearer_auth_header_value() {
        let auth = BearerAuth::new("my-token").unwrap();
        assert_eq!(auth.header_value(), "Bearer my-token");
    }

    #[test]
    fn bearer_auth_rejects_cr() {
        let err = BearerAuth::new("tok\r").unwrap_err();
        assert_eq!(err.kind(), "invalid_auth_header");
    }

    #[test]
    fn bearer_auth_rejects_lf() {
        let err = BearerAuth::new("tok\n").unwrap_err();
        assert_eq!(err.kind(), "invalid_auth_header");
    }

    #[test]
    fn bearer_auth_redacted_debug() {
        let auth = BearerAuth::new("secret-token").unwrap();
        let debug = format!("{auth:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("secret-token"));
    }

    #[test]
    fn bearer_auth_redacted_display() {
        let auth = BearerAuth::new("secret-token").unwrap();
        let display = format!("{auth}");
        assert!(display.contains("<redacted>"));
        assert!(!display.contains("secret-token"));
    }

    // --- AuthScheme tests ---

    #[test]
    fn auth_scheme_debug() {
        let scheme = AuthScheme::basic("user", "pass").unwrap();
        let debug = format!("{scheme:?}");
        assert!(debug.contains("Basic"));

        let scheme = AuthScheme::bearer("token").unwrap();
        let debug = format!("{scheme:?}");
        assert!(debug.contains("Bearer"));
    }

    // --- resolve_request_auth tests ---

    #[test]
    fn resolve_no_auth() {
        let headers = crate::headers::Headers::new();
        let result = resolve_request_auth(None, false, None, &headers).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn resolve_client_auth() {
        let headers = crate::headers::Headers::new();
        let client_auth = Some(AuthScheme::bearer("tok").unwrap());
        let result = resolve_request_auth(None, false, client_auth.as_ref(), &headers).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn resolve_request_auth_overrides_client() {
        let headers = crate::headers::Headers::new();
        let request_auth = Some(AuthScheme::bearer("req-tok").unwrap());
        let client_auth = Some(AuthScheme::bearer("client-tok").unwrap());
        let result =
            resolve_request_auth(request_auth.as_ref(), false, client_auth.as_ref(), &headers)
                .unwrap();
        let auth = result.unwrap();
        match auth {
            AuthScheme::Bearer(b) => {
                assert_eq!(b.header_value(), "Bearer req-tok");
            }
            AuthScheme::Basic(_) => panic!("expected Bearer"),
        }
    }

    #[test]
    fn resolve_auth_disabled() {
        let headers = crate::headers::Headers::new();
        let client_auth = Some(AuthScheme::bearer("tok").unwrap());
        let result = resolve_request_auth(None, true, client_auth.as_ref(), &headers).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn resolve_explicit_header_conflicts_with_auth() {
        let mut headers = crate::headers::Headers::new();
        headers.insert("authorization", "Bearer manual").unwrap();
        let client_auth = Some(AuthScheme::bearer("tok").unwrap());
        let err = resolve_request_auth(None, false, client_auth.as_ref(), &headers).unwrap_err();
        assert_eq!(err.kind(), "conflicting_auth");
    }

    #[test]
    fn resolve_explicit_header_no_auth_config() {
        let mut headers = crate::headers::Headers::new();
        headers.insert("authorization", "Bearer manual").unwrap();
        let result = resolve_request_auth(None, false, None, &headers).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn resolve_request_auth_disabled_overrides_client() {
        let headers = crate::headers::Headers::new();
        let client_auth = Some(AuthScheme::bearer("tok").unwrap());
        let result = resolve_request_auth(None, true, client_auth.as_ref(), &headers).unwrap();
        assert!(result.is_none());
    }
}
