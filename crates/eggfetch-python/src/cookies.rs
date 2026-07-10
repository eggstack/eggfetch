//! Python cookie types wrapping the eggfetch-core cookie subsystem.

use pyo3::prelude::*;
use pyo3::types::{PyFloat, PyList, PyString};

/// A Python-accessible HTTP cookie.
///
/// Wraps `eggfetch_core::cookie::Cookie` with read-only properties.
#[pyclass(name = "Cookie")]
#[derive(Debug, Clone)]
pub struct PyCookie {
    inner: eggfetch_core::cookie::Cookie,
}

impl PyCookie {
    /// Create a `PyCookie` from a core `Cookie`.
    pub fn from_core(cookie: eggfetch_core::cookie::Cookie) -> Self {
        Self { inner: cookie }
    }

    /// Get a reference to the inner core cookie.
    pub fn inner(&self) -> &eggfetch_core::cookie::Cookie {
        &self.inner
    }
}

#[pymethods]
impl PyCookie {
    /// Cookie name.
    #[getter]
    fn name(&self) -> &str {
        self.inner.name()
    }

    /// Cookie value.
    #[getter]
    fn value(&self) -> &str {
        self.inner.value()
    }

    /// Cookie domain.
    #[getter]
    fn domain(&self) -> &str {
        self.inner.domain()
    }

    /// Whether this is a host-only cookie (no subdomain matching).
    #[getter]
    fn is_host_only(&self) -> bool {
        self.inner.is_host_only()
    }

    /// Cookie path.
    #[getter]
    fn path(&self) -> &str {
        self.inner.path()
    }

    /// Whether this cookie requires HTTPS.
    #[getter]
    fn is_secure(&self) -> bool {
        self.inner.is_secure()
    }

    /// Whether this cookie is HTTP-only (not accessible via JavaScript).
    #[getter]
    fn is_http_only(&self) -> bool {
        self.inner.is_http_only()
    }

    /// The `SameSite` attribute, if set.
    #[getter]
    fn same_site<'py>(&self, py: Python<'py>) -> Bound<'py, PyAny> {
        match self.inner.same_site() {
            Some(eggfetch_core::cookie::SameSite::Strict) => PyString::new(py, "Strict").into_any(),
            Some(eggfetch_core::cookie::SameSite::Lax) => PyString::new(py, "Lax").into_any(),
            Some(eggfetch_core::cookie::SameSite::None) => PyString::new(py, "None").into_any(),
            None => py.None().into_bound(py).into_any(),
        }
    }

    /// Expiry time as seconds since epoch, or None if session cookie.
    #[getter]
    fn expires<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self.inner.expires() {
            Some(t) => {
                let secs = t
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?
                    .as_secs_f64();
                Ok(PyFloat::new(py, secs).into_any())
            }
            None => Ok(py.None().into_bound(py).into_any()),
        }
    }

    /// Whether this is a persistent cookie (has an explicit expiry).
    #[getter]
    fn is_persistent(&self) -> bool {
        self.inner.is_persistent()
    }

    /// The cookie name=value pair string.
    #[getter]
    fn name_value_pair(&self) -> String {
        self.inner.name_value_pair()
    }

    fn __repr__(&self) -> String {
        format!(
            "<Cookie name='{}' domain='{}' path='{}'>",
            self.inner.name(),
            self.inner.domain(),
            self.inner.path()
        )
    }

    fn __str__(&self) -> String {
        self.inner.name_value_pair()
    }
}

/// A mutable mapping of cookies, backed by a [`CookieJar`].
///
/// Supports the standard mapping protocol (`__getitem__`, `__setitem__`,
/// `__delitem__`, `__contains__`, `__len__`, `__iter__`).
///
/// Keys are cookie names (strings). Values are [`Cookie`] objects.
/// When multiple cookies share a name (different domains/paths),
/// the first match is returned.
#[pyclass(name = "Cookies")]
#[derive(Debug, Clone)]
pub struct PyCookies {
    jar: eggfetch_core::cookie::CookieJar,
}

impl PyCookies {
    /// Create a Cookies mapping from an existing jar.
    pub fn from_jar(jar: eggfetch_core::cookie::CookieJar) -> Self {
        Self { jar }
    }
}

#[pymethods]
impl PyCookies {
    /// Create an empty Cookies mapping.
    #[new]
    fn py_new() -> Self {
        Self {
            jar: eggfetch_core::cookie::CookieJar::new(),
        }
    }

    /// Number of cookies in the jar.
    fn __len__(&self) -> usize {
        self.jar.len()
    }

    /// Iterate over cookie names.
    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let names: Vec<PyObject> = self
            .jar
            .all_cookies()
            .iter()
            .map(|c| PyString::new(py, c.name()).into())
            .collect();
        let list = PyList::new(py, names)?;
        py.import("builtins")?.getattr("iter")?.call1((list,))
    }

    /// Check if a cookie name exists.
    fn __contains__(&self, name: &str) -> bool {
        self.jar.get(name, None, None).is_some()
    }

    /// Get a cookie by name.
    fn __getitem__(&self, name: &str) -> PyResult<PyCookie> {
        self.jar
            .get(name, None, None)
            .map(PyCookie::from_core)
            .ok_or_else(|| {
                PyErr::new::<pyo3::exceptions::PyKeyError, _>(format!("cookie '{name}' not found"))
            })
    }

    /// Set or replace a cookie by name.
    ///
    /// The value must be a [`Cookie`] object.
    fn __setitem__(&self, _name: &str, cookie: &Bound<'_, PyAny>) -> PyResult<()> {
        let py_cookie: PyCookie = cookie.extract()?;
        self.jar.set(py_cookie.inner.clone());
        Ok(())
    }

    /// Delete a cookie by name.
    fn __delitem__(&self, name: &str) -> PyResult<()> {
        let cookies = self.jar.all_cookies();
        let cookie = cookies.iter().find(|c| c.name() == name).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyKeyError, _>(format!("cookie '{name}' not found"))
        })?;
        self.jar.delete(name, cookie.domain(), cookie.path());
        Ok(())
    }

    /// Get a cookie by name, returning None if not found.
    #[pyo3(signature = (name, default=None))]
    fn get<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        default: Option<&Bound<'py, PyAny>>,
    ) -> Bound<'py, PyAny> {
        match self.jar.get(name, None, None) {
            Some(cookie) => Py::new(py, PyCookie::from_core(cookie))
                .expect("failed to create PyCookie")
                .into_bound(py)
                .into_any(),
            None => match default {
                Some(d) => d.clone().into_any(),
                None => py.None().into_bound(py).into_any(),
            },
        }
    }

    /// Set a cookie with explicit domain and path.
    #[pyo3(signature = (name, value, *, domain=None, path="/"))]
    fn set(
        &self,
        name: &str,
        value: &str,
        domain: Option<&str>,
        path: Option<&str>,
    ) -> PyResult<()> {
        let url_str = format!(
            "http://{}{}",
            domain.unwrap_or("localhost"),
            path.unwrap_or("/")
        );
        let url = url::Url::parse(&url_str)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyValueError, _>(e.to_string()))?;
        let temp_jar = eggfetch_core::cookie::CookieJar::new();
        let set_cookie = if let Some(d) = domain {
            format!("{name}={value}; Domain={d}; Path={}", path.unwrap_or("/"))
        } else {
            format!("{name}={value}; Path={}", path.unwrap_or("/"))
        };
        temp_jar.update_from_response(&url, &[set_cookie]);
        if let Some(cookie) = temp_jar.all_cookies().into_iter().next() {
            self.jar.set(cookie);
        }
        Ok(())
    }

    /// Clear all cookies.
    fn clear(&self) {
        self.jar.clear();
    }

    /// Return a list of all Cookie objects.
    fn values<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let cookies: Vec<PyCookie> = self
            .jar
            .all_cookies()
            .into_iter()
            .map(PyCookie::from_core)
            .collect();
        PyList::new(py, cookies)
    }

    /// Return a list of (name, Cookie) tuples.
    fn items<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let py_tuple = py.import("builtins")?.getattr("tuple")?;
        let items: Vec<PyObject> = self
            .jar
            .all_cookies()
            .into_iter()
            .map(|c| {
                let name: PyObject = PyString::new(py, c.name()).into();
                let cookie: PyObject = Py::new(py, PyCookie::from_core(c))?.into_any();
                let tup = py_tuple.call1((PyList::new(py, [name, cookie])?,))?;
                Ok(tup.into())
            })
            .collect::<PyResult<Vec<_>>>()?;
        PyList::new(py, items)
    }

    fn __repr__(&self) -> String {
        format!("Cookies({})", self.jar.len())
    }
}
