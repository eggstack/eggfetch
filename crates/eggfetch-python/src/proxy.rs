//! Python proxy parameter parsing.

use pyo3::prelude::*;

/// Which rule set a client applies when it interprets proxy configuration.
///
/// The native `eggfetch` surface uses eggfetch's own rules.  The HTTPX
/// compatibility facades reproduce HTTPX 0.28.1's rules and therefore opt
/// into [`ProxyEnvSemantics::Httpx`] explicitly; the native default is
/// [`ProxyEnvSemantics::Native`] so that native users never inherit
/// compat-only semantics (ADR-0009 rule 5).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum ProxyEnvSemantics {
    /// eggfetch's native rules: `NoProxy::parse` for environment `NO_PROXY`,
    /// and an explicit `socks5://` proxy URL is passed through unchanged.
    #[default]
    Native,
    /// HTTPX 0.28.1's rules: `NoProxy::parse_httpx` for environment
    /// `NO_PROXY`, and `socks5://` is normalized to `socks5h://` so the
    /// destination hostname is resolved by the SOCKS proxy.
    Httpx,
}

/// Parse the `proxy_env` constructor option.
///
/// `None` selects the native semantics.  Only the two documented values are
/// accepted; an unknown value is an error rather than a silent fallback.
pub(crate) fn parse_proxy_env_semantics(value: Option<&str>) -> PyResult<ProxyEnvSemantics> {
    match value {
        None | Some("native") => Ok(ProxyEnvSemantics::Native),
        Some("httpx") => Ok(ProxyEnvSemantics::Httpx),
        Some(other) => Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
            "proxy_env must be 'native' or 'httpx', got {other:?}"
        ))),
    }
}

/// Parse an environment `NO_PROXY` value with the selected rule set.
pub(crate) fn parse_no_proxy(
    value: &str,
    semantics: ProxyEnvSemantics,
) -> eggfetch_core::Result<eggfetch_core::NoProxy> {
    match semantics {
        ProxyEnvSemantics::Native => eggfetch_core::NoProxy::parse(value),
        ProxyEnvSemantics::Httpx => eggfetch_core::NoProxy::parse_httpx(value),
    }
}

/// Apply the selected semantics to an explicit proxy URL.
///
/// Only the HTTPX semantics rewrite `socks5://` to `socks5h://`.  The native
/// semantics leave the scheme untouched so that `socks5://` keeps its
/// local-DNS meaning for native callers.
pub(crate) fn apply_proxy_semantics(url: &str, semantics: ProxyEnvSemantics) -> String {
    match semantics {
        ProxyEnvSemantics::Native => url.to_owned(),
        ProxyEnvSemantics::Httpx => normalize_httpx_proxy_url(url),
    }
}

/// How the `proxy` parameter was specified by the caller.
#[derive(PartialEq, Eq)]
pub(crate) enum ProxyOverride {
    /// Argument was omitted or set to `None` — inherit client-level proxy.
    Inherit,
    /// `False` was passed — disable proxy for this request.
    Disable,
    /// A URL string was provided — override proxy for this request.
    Override(String),
}

impl std::fmt::Debug for ProxyOverride {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inherit => write!(f, "Inherit"),
            Self::Disable => write!(f, "Disable"),
            Self::Override(url) => {
                let has_credentials = url::Url::parse(url).is_ok_and(|parsed| {
                    !parsed.username().is_empty() || parsed.password().is_some()
                });
                if has_credentials {
                    // Never render credential-bearing URLs: the value is
                    // transient (consumed by route resolution), so diagnostics
                    // must not leak the password.
                    write!(f, "Override(\"<redacted>\")")
                } else {
                    f.debug_tuple("Override").field(url).finish()
                }
            }
        }
    }
}

/// Return the environment proxies in HTTPX's scheme-aware order.
///
/// HTTPX delegates this lookup to `urllib.request.getproxies()`, which gives
/// lowercase variables precedence and applies the platform's standard proxy
/// environment rules. Reusing that stdlib helper keeps the facade aligned
/// with the pinned reference instead of maintaining a second precedence table.
pub(crate) fn env_proxy_urls(py: Python<'_>) -> PyResult<Vec<(&'static str, String)>> {
    let urllib = py.import("urllib.request")?;
    let values = urllib.call_method0("getproxies")?;
    let value = |name: &str| -> PyResult<Option<String>> {
        let value = values.call_method1("get", (name,))?;
        if value.is_none() {
            return Ok(None);
        }
        let value: String = value.extract()?;
        Ok((!value.is_empty()).then_some(value))
    };
    let mut proxies = Vec::new();
    if let Some(url) = value("http")? {
        proxies.push(("http", normalize_environment_proxy_url(&url)));
    }
    if let Some(url) = value("https")? {
        proxies.push(("https", normalize_environment_proxy_url(&url)));
    }
    if let Some(url) = value("all")? {
        proxies.push(("all", normalize_environment_proxy_url(&url)));
    }
    Ok(proxies)
}

pub(crate) fn env_no_proxy(py: Python<'_>) -> PyResult<Option<String>> {
    let urllib = py.import("urllib.request")?;
    let values = urllib.call_method0("getproxies")?;
    let value = values.call_method1("get", ("no",))?;
    if value.is_none() {
        return Ok(None);
    }
    let value: String = value.extract()?;
    Ok((!value.is_empty()).then_some(value))
}

/// Normalize the proxy URL forms accepted by HTTPX's environment helper.
pub(crate) fn normalize_environment_proxy_url(url: &str) -> String {
    if url.contains("://") {
        url.to_owned()
    } else {
        format!("http://{url}")
    }
}

/// Normalize the SOCKS scheme used by the HTTPX compatibility facade.
///
/// httpcore 1.0.9 sends hostname destinations as SOCKS domain names for both
/// `socks5` and `socks5h`; the native core keeps its lower-level distinction
/// for callers outside the facade.
pub(crate) fn normalize_httpx_proxy_url(url: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(url) else {
        return url.to_owned();
    };
    if parsed.scheme() == "socks5" {
        let _ = parsed.set_scheme("socks5h");
        parsed.to_string()
    } else {
        url.to_owned()
    }
}

/// Parse a Python `proxy` argument into a `ProxyOverride`.
///
/// Accepts:
/// - Omitted or `None` → `ProxyOverride::Inherit`
/// - `False` → `ProxyOverride::Disable`
/// - A string URL → `ProxyOverride::Override(url_string)`
/// - A Proxy object → extracts URL (with embedded auth) and returns `ProxyOverride::Override(url_string)`
/// - Anything else → `Err`
///
/// The returned URL is unmodified.  Scheme-specific normalization (the
/// HTTPX `socks5` → `socks5h` rewrite) is applied later by
/// [`apply_proxy_semantics`], once the client's selected semantics are known.
pub fn parse_proxy(proxy: Option<&Bound<'_, PyAny>>) -> PyResult<ProxyOverride> {
    match proxy {
        None => Ok(ProxyOverride::Inherit),
        Some(val) => {
            if val.is_none() {
                return Ok(ProxyOverride::Inherit);
            }
            if val.is_instance_of::<pyo3::types::PyBool>() {
                let b: bool = val.extract()?;
                if b {
                    return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                        "proxy must be a URL string, False, or None; True is not valid",
                    ));
                }
                return Ok(ProxyOverride::Disable);
            }
            if let Ok(url) = val.extract::<String>() {
                if let Ok(parsed) = url::Url::parse(&url) {
                    if !parsed.username().is_empty() || parsed.password().is_some() {
                        return Err(crate::errors::ProxyError::new_err(
                            "proxy URL credentials must be supplied with Proxy(auth=...), not embedded in the URL",
                        ));
                    }
                }
                return Ok(ProxyOverride::Override(url));
            }
            // Handle Proxy objects: extract URL and embed auth if present.
            if let Ok(url_obj) = val.getattr("url") {
                if let Ok(url_string) = url_obj.str() {
                    let mut url = url_string.to_string_lossy().to_string();
                    // Embed raw_auth into the URL so all_compat can extract it.
                    if let Ok(raw_auth) = val.getattr("raw_auth") {
                        if !raw_auth.is_none() {
                            if let Ok(auth_tuple) = raw_auth.cast::<pyo3::types::PyTuple>() {
                                if auth_tuple.len() == 2 {
                                    if let (Ok(username), Ok(password)) = (
                                        auth_tuple.get_item(0)?.extract::<String>(),
                                        auth_tuple.get_item(1)?.extract::<String>(),
                                    ) {
                                        if let Ok(mut parsed) = url::Url::parse(&url) {
                                            parsed.set_username(&username).map_err(|()| {
                                                PyErr::new::<pyo3::exceptions::PyValueError, _>(
                                                    "proxy username contains invalid characters",
                                                )
                                            })?;
                                            parsed.set_password(Some(&password)).map_err(|()| {
                                                PyErr::new::<pyo3::exceptions::PyValueError, _>(
                                                    "proxy password contains invalid characters",
                                                )
                                            })?;
                                            url = parsed.to_string();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return Ok(ProxyOverride::Override(url));
                }
            }
            Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "proxy must be a URL string, False, or None",
            ))
        }
    }
}

/// Extract proxy-leg headers and TLS config from a Python `Proxy` object.
///
/// Returns `(headers, tls_config)` when `proxy` is an instance of
/// `eggfetch.compat.httpx.Proxy`; otherwise `(None, None)`. Import failures
/// are treated as absent metadata so callers do not fail when the compat
/// facade is unavailable.
pub(crate) fn extract_proxy_extras(
    py: Python<'_>,
    proxy: Option<&Bound<'_, PyAny>>,
) -> PyResult<(
    Option<eggfetch_core::Headers>,
    Option<eggfetch_core::TlsConfig>,
)> {
    let Some(proxy_obj) = proxy else {
        return Ok((None, None));
    };
    let Ok(proxy_module) = py.import("eggfetch.compat.httpx._proxy") else {
        return Ok((None, None));
    };
    let Ok(proxy_class) = proxy_module.getattr("Proxy") else {
        return Ok((None, None));
    };
    if !proxy_obj.is_instance(&proxy_class).unwrap_or(false) {
        return Ok((None, None));
    }
    let headers = if proxy_obj.hasattr("headers")? {
        let h = proxy_obj.getattr("headers")?;
        Some(crate::conversion::python_headers_to_rust(py, &h)?)
    } else {
        None
    };
    let ssl_ctx = proxy_obj.getattr("ssl_context").ok();
    let tls = crate::tls::ssl_context_to_tls_config(py, ssl_ctx.as_ref())?;
    Ok((headers, tls))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_debug_redacts_credential_url() {
        let credentialed =
            ProxyOverride::Override("http://user:s3cret@example.com:8080".to_owned());
        let rendered = format!("{credentialed:?}");
        assert!(
            rendered.contains("<redacted>"),
            "credential-bearing override must redact: {rendered}"
        );
        assert!(
            !rendered.contains("s3cret"),
            "password must not appear in Debug: {rendered}"
        );
        let plain = ProxyOverride::Override("http://example.com:8080".to_owned());
        let rendered = format!("{plain:?}");
        assert!(
            rendered.contains("http://example.com:8080"),
            "credential-free URL stays visible: {rendered}"
        );
    }
}
