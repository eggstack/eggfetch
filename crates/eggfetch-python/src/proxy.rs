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

/// Read the effective proxy URL from environment variables.
///
/// Checks `HTTP_PROXY`, `http_proxy`, `HTTPS_PROXY`, `https_proxy`,
/// `ALL_PROXY`, `all_proxy` in priority order. Returns `None` if no
/// variable is set or all are empty.
pub(crate) fn env_proxy_url() -> Option<String> {
    std::env::var("HTTP_PROXY")
        .or_else(|_| std::env::var("http_proxy"))
        .or_else(|_| std::env::var("HTTPS_PROXY"))
        .or_else(|_| std::env::var("https_proxy"))
        .or_else(|_| std::env::var("ALL_PROXY"))
        .or_else(|_| std::env::var("all_proxy"))
        .ok()
        .filter(|v| !v.is_empty())
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
