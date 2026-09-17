//! CONNECT authority ownership.
//!
//! Centralizes `host:port` formatting so every consumer does not
//! reimplement IPv6 bracketing.

use crate::error::ConnectError;

/// CONNECT destination in authority form.
///
/// Holds a validated host and port. [`ConnectTarget::authority`] renders
/// the exact `host:port` string used for both the `CONNECT` request-target
/// and the `Host` header so they cannot diverge.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConnectTarget {
    host: String,
    port: u16,
}

impl ConnectTarget {
    /// Create a CONNECT target.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectError::InvalidTarget`] when the host is empty,
    /// contains request-line control bytes (`CR`, `LF`, other C0 controls,
    /// `DEL`, or space), or has malformed IPv6 brackets. Bracketed
    /// (`[::1]`) and unbracketed (`::1`) IPv6 literals are both accepted
    /// and normalize to the same bracketed authority. No path, query, or
    /// body semantics exist in this type.
    pub fn new(host: impl Into<String>, port: u16) -> Result<Self, ConnectError> {
        let host = host.into();
        validate_host(&host)?;
        Ok(Self { host, port })
    }

    /// Return the raw host as supplied (brackets preserved when given).
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Return the port.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Render the authority-form `host:port` target.
    ///
    /// Domains render as `example.com:443`, IPv4 as `127.0.0.1:443`,
    /// and IPv6 bracketed as `[::1]:443`. Pre-bracketed and unbracketed
    /// IPv6 normalize to the same bracketed form.
    #[must_use]
    pub fn authority(&self) -> String {
        authority_form(&self.host, self.port)
    }
}

/// Render authority without re-validating (already validated at construction).
fn authority_form(host: &str, port: u16) -> String {
    if let Some(inner) = strip_brackets(host) {
        format!("[{inner}]:{port}")
    } else if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// Return the inner literal when `host` is fully bracketed.
fn strip_brackets(host: &str) -> Option<&str> {
    if host.starts_with('[') && host.ends_with(']') && host.len() >= 2 {
        Some(&host[1..host.len() - 1])
    } else {
        None
    }
}

/// Validate a CONNECT host without echoing it in errors.
fn validate_host(host: &str) -> Result<(), ConnectError> {
    if host.is_empty() {
        return Err(ConnectError::InvalidTarget);
    }
    // Bracket shape: brackets may only wrap the whole host.
    if host.contains('[') || host.contains(']') {
        let Some(inner) = strip_brackets(host) else {
            return Err(ConnectError::InvalidTarget);
        };
        if inner.is_empty() || inner.contains('[') || inner.contains(']') {
            return Err(ConnectError::InvalidTarget);
        }
        validate_host_inner(inner)?;
        return Ok(());
    }
    validate_host_inner(host)
}

/// Validate the unbracketed host body.
fn validate_host_inner(host: &str) -> Result<(), ConnectError> {
    if host.is_empty() {
        return Err(ConnectError::InvalidTarget);
    }
    for c in host.chars() {
        // Reject request-line control bytes: C0, DEL, and space. This
        // blocks CR/LF injection and other control smuggling while
        // leaving ordinary domain/IPv4/IPv6 text untouched.
        if c == '\r' || c == '\n' || c.is_control() || c == ' ' || c == '\u{7f}' {
            return Err(ConnectError::InvalidTarget);
        }
    }
    Ok(())
}
