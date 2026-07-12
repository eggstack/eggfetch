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

/// Parse a Python `auth` argument into an optional Rust `AuthScheme`.
///
/// Accepts:
/// - `None` → `Ok(None)`
/// - `("username", "password")` tuple → `Ok(Some(AuthScheme::Basic(...)))`
/// - `BasicAuth` instance → `Ok(Some(AuthScheme::Basic(...)))`
/// - `BearerAuth` instance → `Ok(Some(AuthScheme::Bearer(...)))`
/// - Anything else → `Err`
pub fn parse_auth(auth: Option<&Bound<'_, PyAny>>) -> PyResult<Option<eggfetch_core::AuthScheme>> {
    match auth {
        None => Ok(None),
        Some(val) => {
            if val.is_none() {
                return Ok(None);
            }
            // Check for BasicAuth
            if let Ok(basic) = val.extract::<PyBasicAuth>() {
                return Ok(Some(eggfetch_core::AuthScheme::Basic(basic.inner)));
            }
            // Check for BearerAuth
            if let Ok(bearer) = val.extract::<PyBearerAuth>() {
                return Ok(Some(eggfetch_core::AuthScheme::Bearer(bearer.inner)));
            }
            // Check for (username, password) tuple
            if let Ok(tuple) = val.downcast::<PyTuple>() {
                if tuple.len() == 2 {
                    let username: String = tuple.get_item(0)?.extract()?;
                    let password: String = tuple.get_item(1)?.extract()?;
                    let scheme =
                        eggfetch_core::AuthScheme::basic(username, password).map_err(map_err)?;
                    return Ok(Some(scheme));
                }
            }
            Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "auth must be a (username, password) tuple, BasicAuth, or BearerAuth",
            ))
        }
    }
}
