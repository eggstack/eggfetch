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
pub(crate) fn env_proxy_urls() -> Vec<(&'static str, String)> {
    let value = |upper: &str, lower: &str| {
        std::env::var(upper)
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| std::env::var(lower).ok().filter(|v| !v.is_empty()))
    };
    let mut proxies = Vec::new();
    if let Some(url) = value("HTTP_PROXY", "http_proxy") {
        proxies.push(("http", url));
    }
    if let Some(url) = value("HTTPS_PROXY", "https_proxy") {
        proxies.push(("https", url));
    }
    if let Some(url) = value("ALL_PROXY", "all_proxy") {
        proxies.push(("all", url));
    }
    proxies
}

pub(crate) fn env_no_proxy() -> Option<String> {
    std::env::var("NO_PROXY")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("no_proxy").ok().filter(|v| !v.is_empty()))
}

/// Parse a Python `proxy` argument into a `ProxyOverride`.
///
/// Accepts:
/// - Omitted or `None` → `ProxyOverride::Inherit`
/// - `False` → `ProxyOverride::Disable`
/// - A string URL → `ProxyOverride::Override(url_string)`
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
                return Ok(ProxyOverride::Override(url));
            }
            Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "proxy must be a URL string, False, or None",
            ))
        }
    }
}
