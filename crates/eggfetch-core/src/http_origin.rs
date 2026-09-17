//! Native transport origin derived from `http::Uri`.
//!
//! The low-level native API receives a caller-owned `http::Request<B>` with
//! an `http::Uri`. It must not serialize that URI and reparse it as
//! `url::Url` merely to recover scheme, host and port. This module supplies
//! the small canonical transport facts the engine needs: scheme (HTTP/HTTPS
//! only), host, and effective port.
//!
//! The native caller owns construction of the `http::Uri`. If a Unicode
//! hostname cannot be represented by the `http` crate's URI type, the caller
//! must provide an appropriate ASCII/punycode URI. No IDNA transformation is
//! performed here.

use crate::error::{Error, Result};

/// Transport scheme for a native origin.
///
/// Only HTTP and HTTPS are accepted; anything else is rejected before I/O.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum HttpScheme {
    /// Cleartext HTTP (default port 80).
    Http,
    /// TLS HTTPS (default port 443).
    Https,
}

impl HttpScheme {
    /// Canonical lowercase scheme string.
    #[must_use]
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }

    /// Effective default port for the scheme.
    #[must_use]
    pub(crate) fn default_port(self) -> u16 {
        match self {
            Self::Http => 80,
            Self::Https => 443,
        }
    }

    /// Whether this scheme requires TLS.
    #[must_use]
    pub(crate) fn is_https(self) -> bool {
        matches!(self, Self::Https)
    }
}

/// Canonical native origin: scheme + host + effective port.
///
/// Path, query and fragment never participate in origin identity. The host
/// is stored ASCII-lowercased so `EXAMPLE.COM` and `example.com` share a
/// pool slot, matching the high-level `url::Url` normalization for ASCII
/// DNS names. No IDNA/punycode conversion is performed: the caller-supplied
/// `http::Uri` authority is authoritative.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct HttpOrigin {
    scheme: HttpScheme,
    host: String,
    port: u16,
}

impl HttpOrigin {
    /// Derive a native origin from an `http::Uri`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidUrl`] for a missing scheme or authority,
    /// [`Error::Unsupported`] for a non-HTTP(S) scheme or a missing
    /// authority, and [`Error::InvalidUrl`] for userinfo or an empty host.
    pub(crate) fn from_uri(uri: &http::Uri) -> Result<Self> {
        let scheme_str = uri.scheme_str().ok_or_else(|| {
            Error::InvalidUrl(
                "native request URI must be absolute with http or https scheme".into(),
            )
        })?;
        let scheme = if scheme_str.eq_ignore_ascii_case("http") {
            HttpScheme::Http
        } else if scheme_str.eq_ignore_ascii_case("https") {
            HttpScheme::Https
        } else {
            return Err(Error::Unsupported(
                "native request URI must use http or https and include an authority".into(),
            ));
        };

        let authority = uri.authority().ok_or_else(|| {
            Error::Unsupported(
                "native request URI must use http or https and include an authority".into(),
            )
        })?;
        let authority_str = authority.as_str();
        // `http::Uri` accepts userinfo (`user[:pass]@host`); the high-level
        // API rejects it and the native path must too.
        if authority_str.contains('@') {
            return Err(Error::InvalidUrl(
                "URL userinfo is not supported; configure authentication explicitly".into(),
            ));
        }

        let host = uri.host().ok_or_else(|| {
            Error::Unsupported(
                "native request URI must use http or https and include an authority".into(),
            )
        })?;
        if host.is_empty() {
            return Err(Error::Unsupported(
                "native request URI must use http or https and include an authority".into(),
            ));
        }
        // `Uri::host` for IPv6 authorities returns the bracketed form
        // (`[::1]`); strip one bracket pair for canonical storage so IPv6
        // pool keys and SNI identity match the high-level `url` form.
        // DNS names are ASCII-lowercased for canonical identity; no IDNA
        // transformation is performed.
        let canonical_host = if host.starts_with('[') && host.ends_with(']') && host.len() >= 2 {
            host[1..host.len() - 1].to_ascii_lowercase()
        } else {
            host.to_ascii_lowercase()
        };
        if canonical_host.is_empty() {
            return Err(Error::Unsupported(
                "native request URI must use http or https and include an authority".into(),
            ));
        }

        // Reject a malformed explicit port (non-numeric or out of range).
        // `http::Uri` silently drops an invalid port (`port()` and
        // `port_u16()` both return `None` for `:notaport`/`:99999`), which
        // would weaken validation relative to `url::Url::parse`. Detect an
        // explicit port via the raw authority so such URIs fail closed
        // instead of falling back to the scheme default.
        let has_explicit_port = if authority_str.starts_with('[') {
            match authority_str.find(']') {
                Some(end) => authority_str[end + 1..].starts_with(':'),
                None => {
                    return Err(Error::InvalidUrl(
                        "native request URI has an invalid authority".into(),
                    ));
                }
            }
        } else {
            authority_str.contains(':')
        };
        let port = if let Some(port) = uri.port_u16() {
            port
        } else {
            if has_explicit_port || uri.port().is_some() {
                return Err(Error::InvalidUrl(
                    "native request URI has an invalid port".into(),
                ));
            }
            scheme.default_port()
        };
        Ok(Self {
            scheme,
            host: canonical_host,
            port,
        })
    }

    /// Derive a native origin from high-level `url::Url` components.
    ///
    /// This adapter lives behind the high-level URL feature and reduces
    /// immediately to the same canonical component representation so the
    /// high-level and native paths share pool and route identity.
    #[cfg(feature = "high-level-url")]
    pub(crate) fn from_url(url: &url::Url) -> Option<Self> {
        let scheme = if url.scheme().eq_ignore_ascii_case("http") {
            HttpScheme::Http
        } else if url.scheme().eq_ignore_ascii_case("https") {
            HttpScheme::Https
        } else {
            return None;
        };
        let host = url.host_str()?;
        if host.is_empty() {
            return None;
        }
        let port = url.port_or_known_default()?;
        // `url::Url::host_str` already strips IPv6 brackets; lowercase for
        // canonical identity to match `from_uri`.
        Some(Self {
            scheme,
            host: host.to_ascii_lowercase(),
            port,
        })
    }

    /// Canonical scheme string (`http` or `https`).
    #[must_use]
    pub(crate) fn scheme_str(&self) -> &'static str {
        self.scheme.as_str()
    }

    /// Canonical (lowercased, bracket-stripped) host.
    #[must_use]
    pub(crate) fn host(&self) -> &str {
        &self.host
    }

    /// Effective port (explicit or default-for-scheme).
    #[must_use]
    pub(crate) fn port_ref(&self) -> u16 {
        self.port
    }

    /// Whether this origin requires TLS.
    #[must_use]
    pub(crate) fn is_https(&self) -> bool {
        self.scheme.is_https()
    }

    /// Canonical origin string (`scheme://host:port`) for route-cache
    /// identity. The port is always explicit so default and explicit-default
    /// forms share one key.
    #[must_use]
    pub(crate) fn origin_string(&self) -> String {
        format!("{}://{}:{}", self.scheme.as_str(), self.host, self.port)
    }
}

impl std::fmt::Display for HttpOrigin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}://{}:{}", self.scheme.as_str(), self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin(uri: &str) -> HttpOrigin {
        let parsed: http::Uri = uri.parse().expect("valid test URI");
        HttpOrigin::from_uri(&parsed).expect("origin parses")
    }

    #[test]
    fn http_default_port() {
        let o = origin("http://example.com/");
        assert_eq!(o.scheme_str(), "http");
        assert_eq!(o.host(), "example.com");
        assert_eq!(o.port_ref(), 80);
    }

    #[test]
    fn https_default_port() {
        let o = origin("https://example.com/");
        assert_eq!(o.scheme_str(), "https");
        assert_eq!(o.host(), "example.com");
        assert_eq!(o.port_ref(), 443);
    }

    #[test]
    fn explicit_non_default_port_preserved() {
        let o = origin("http://example.com:8080/path?q=1");
        assert_eq!(o.port_ref(), 8080);
        assert_eq!(o.host(), "example.com");
    }

    #[test]
    fn explicit_default_port_preserved() {
        let o = origin("http://example.com:80/");
        assert_eq!(o.port_ref(), 80);
        let implicit = origin("http://example.com/");
        assert_eq!(o, implicit);
    }

    #[test]
    fn ipv4_authority() {
        let o = origin("http://127.0.0.1:8080/");
        assert_eq!(o.host(), "127.0.0.1");
        assert_eq!(o.port_ref(), 8080);
    }

    #[test]
    fn ipv6_without_port() {
        let o = origin("http://[::1]/");
        assert_eq!(o.host(), "::1");
        assert_eq!(o.port_ref(), 80);
    }

    #[test]
    fn ipv6_with_port() {
        let o = origin("https://[2001:db8::1]:8443/path");
        assert_eq!(o.host(), "2001:db8::1");
        assert_eq!(o.port_ref(), 8443);
    }

    #[test]
    fn missing_scheme_rejected() {
        let uri: http::Uri = "/just/a/path".parse().expect("path-only URI");
        let err = HttpOrigin::from_uri(&uri).unwrap_err();
        assert!(matches!(err, Error::InvalidUrl(_)), "got {err:?}");
    }

    #[test]
    fn missing_authority_rejected() {
        // `http:/only-path` either fails URI parsing or carries no
        // authority; both are rejections.
        match "http:/only-path".parse::<http::Uri>() {
            Err(_) => {}
            Ok(uri) => {
                assert!(uri.authority().is_none());
                let err = HttpOrigin::from_uri(&uri).unwrap_err();
                assert!(
                    matches!(err, Error::Unsupported(_)),
                    "expected unsupported, got {err:?}"
                );
            }
        }
    }

    #[test]
    fn unsupported_scheme_rejected() {
        let uri: http::Uri = "ftp://example.com/file".parse().expect("ftp URI");
        let err = HttpOrigin::from_uri(&uri).unwrap_err();
        assert!(matches!(err, Error::Unsupported(_)), "got {err:?}");
    }

    #[test]
    fn userinfo_rejected() {
        let uri: http::Uri = "http://user:pass@example.com/"
            .parse()
            .expect("userinfo URI parses in http crate");
        let err = HttpOrigin::from_uri(&uri).unwrap_err();
        assert!(
            matches!(err, Error::InvalidUrl(_)),
            "expected userinfo rejection, got {err:?}"
        );
    }

    #[test]
    fn user_without_password_rejected() {
        let uri: http::Uri = "http://user@example.com/"
            .parse()
            .expect("userinfo URI parses in http crate");
        let err = HttpOrigin::from_uri(&uri).unwrap_err();
        assert!(matches!(err, Error::InvalidUrl(_)), "got {err:?}");
    }

    #[test]
    fn empty_host_rejected() {
        // `http:///path` either fails URI parsing or yields no usable host;
        // both are rejections and must never become a pool origin.
        match "http:///path".parse::<http::Uri>() {
            Err(_) => {}
            Ok(uri) => {
                let err = HttpOrigin::from_uri(&uri).unwrap_err();
                assert!(
                    matches!(err, Error::Unsupported(_) | Error::InvalidUrl(_)),
                    "got {err:?}"
                );
            }
        }
    }

    #[test]
    fn malformed_port_rejected() {
        // Malformed ports either fail URI parsing or parse but are rejected
        // by `from_uri` (no silent fallback to the scheme default).
        for candidate in ["http://example.com:notaport/", "http://example.com:99999/"] {
            match candidate.parse::<http::Uri>() {
                Err(_) => {}
                Ok(uri) => {
                    let err = HttpOrigin::from_uri(&uri).unwrap_err();
                    assert!(
                        matches!(err, Error::InvalidUrl(_)),
                        "expected invalid port for {candidate}, got {err:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn path_query_ignored_for_identity() {
        let a = origin("https://example.com/a?b=1");
        let b = origin("https://example.com/other");
        assert_eq!(a, b);
    }

    #[test]
    fn host_lowercased_for_identity() {
        let lower = origin("http://example.com/");
        let upper = origin("http://EXAMPLE.COM/");
        assert_eq!(lower, upper);
    }
}
