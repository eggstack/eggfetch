//! Python proxy parameter parsing.

use pyo3::prelude::*;

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
pub(crate) fn normalize_compat_proxy_url(url: &str) -> String {
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
                return Ok(ProxyOverride::Override(normalize_compat_proxy_url(&url)));
            }
            // Handle Proxy objects: extract URL and embed auth if present.
            if let Ok(url_obj) = val.getattr("url") {
                if let Ok(url_string) = url_obj.str() {
                    let mut url = url_string.to_string_lossy().to_string();
                    // Embed raw_auth into the URL so all_compat can extract it.
                    if let Ok(raw_auth) = val.getattr("raw_auth") {
                        if !raw_auth.is_none() {
                            if let Ok(auth_tuple) = raw_auth.downcast::<pyo3::types::PyTuple>() {
                                if auth_tuple.len() == 2 {
                                    if let (Ok(username), Ok(password)) = (
                                        auth_tuple.get_item(0)?.extract::<String>(),
                                        auth_tuple.get_item(1)?.extract::<String>(),
                                    ) {
                                        if let Ok(mut parsed) = url::Url::parse(&url) {
                                            let _ = parsed.set_username(&username);
                                            let _ = parsed.set_password(Some(&password));
                                            url = parsed.to_string();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return Ok(ProxyOverride::Override(normalize_compat_proxy_url(&url)));
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
