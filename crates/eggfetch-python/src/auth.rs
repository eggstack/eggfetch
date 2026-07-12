//! Python auth bindings.

use pyo3::prelude::*;
use pyo3::types::PyTuple;

use crate::errors::map_err;

/// HTTP Basic Authentication credentials.
///
/// Creates an Authorization header with Base64-encoded username:password.
///
/// Args:
///     username: The username.
///     password: The password (default: "").
#[pyclass(name = "BasicAuth")]
#[derive(Clone)]
pub struct PyBasicAuth {
    pub(crate) inner: eggfetch_core::BasicAuth,
}

#[pymethods]
impl PyBasicAuth {
    #[new]
    #[pyo3(signature = (username, password=""))]
    fn new(username: &str, password: &str) -> PyResult<Self> {
        let inner = eggfetch_core::BasicAuth::new(username, password).map_err(map_err)?;
        Ok(Self { inner })
    }

    #[getter]
    fn username(&self) -> &str {
        self.inner.username()
    }

    fn __repr__(&self) -> String {
        format!("BasicAuth(username={})", self.inner.username())
    }
}

/// HTTP Bearer Token Authentication credentials.
///
/// Creates an Authorization header with a bearer token.
///
/// Args:
///     token: The bearer token.
#[pyclass(name = "BearerAuth")]
#[derive(Clone)]
pub struct PyBearerAuth {
    pub(crate) inner: eggfetch_core::BearerAuth,
}

#[pymethods]
impl PyBearerAuth {
    #[new]
    fn new(token: &str) -> PyResult<Self> {
        let inner = eggfetch_core::BearerAuth::new(token).map_err(map_err)?;
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        let _ = self;
        "BearerAuth(<redacted>)".to_string()
    }
}

/// Sentinel value to disable auth on a single request.
///
/// Pass ``auth=eggfetch.NOAUTH`` to a request method to suppress
/// client-level auth for that request only.
#[pyclass(name = "NoAuth")]
pub struct PyNoAuth;

#[pymethods]
impl PyNoAuth {
    #[new]
    fn new() -> Self {
        Self
    }

    #[allow(clippy::unused_self)]
    fn __repr__(&self) -> String {
        "NOAUTH".to_string()
    }
}

/// How the `auth` parameter was specified by the caller.
pub(crate) enum AuthOverride {
    /// Argument was omitted or set to `None` — inherit client-level auth.
    Inherit,
    /// `eggfetch.NOAUTH` sentinel was passed — disable auth for this request.
    Disable,
    /// An auth object was provided — override client auth.
    Override(eggfetch_core::AuthScheme),
}

/// Parse a Python `auth` argument into an `AuthOverride`.
///
/// Accepts:
/// - Omitted or `None` → `AuthOverride::Inherit`
/// - `eggfetch.NOAUTH` → `AuthOverride::Disable`
/// - `("username", "password")` tuple → `AuthOverride::Override(...)`
/// - `BasicAuth` instance → `AuthOverride::Override(...)`
/// - `BearerAuth` instance → `AuthOverride::Override(...)`
/// - Anything else → `Err`
pub fn parse_auth(auth: Option<&Bound<'_, PyAny>>) -> PyResult<AuthOverride> {
    match auth {
        None => Ok(AuthOverride::Inherit),
        Some(val) => {
            if val.is_none() {
                return Ok(AuthOverride::Inherit);
            }
            // Check for the NOAUTH sentinel.
            if val.is_instance_of::<PyNoAuth>() {
                return Ok(AuthOverride::Disable);
            }
            // Check for BasicAuth
            if let Ok(basic) = val.extract::<PyBasicAuth>() {
                return Ok(AuthOverride::Override(eggfetch_core::AuthScheme::Basic(
                    basic.inner,
                )));
            }
            // Check for BearerAuth
            if let Ok(bearer) = val.extract::<PyBearerAuth>() {
                return Ok(AuthOverride::Override(eggfetch_core::AuthScheme::Bearer(
                    bearer.inner,
                )));
            }
            // Check for (username, password) tuple
            if let Ok(tuple) = val.downcast::<PyTuple>() {
                if tuple.len() == 2 {
                    let username: String = tuple.get_item(0)?.extract()?;
                    let password: String = tuple.get_item(1)?.extract()?;
                    let scheme =
                        eggfetch_core::AuthScheme::basic(username, password).map_err(map_err)?;
                    return Ok(AuthOverride::Override(scheme));
                }
            }
            Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "auth must be a (username, password) tuple, BasicAuth, BearerAuth, NOAUTH, or None",
            ))
        }
    }
}
