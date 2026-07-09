//! Python headers wrapper around `http::HeaderMap`.

use pyo3::prelude::*;
use pyo3::types::{PyList, PyString, PyTuple};

/// A case-insensitive HTTP headers container exposed to Python.
#[pyclass(name = "Headers")]
#[derive(Debug, Clone)]
pub struct PyHeaders {
    inner: http::HeaderMap,
}

impl PyHeaders {
    /// Create a `PyHeaders` from an `http::HeaderMap`.
    pub fn from_header_map(inner: http::HeaderMap) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyHeaders {
    /// Get a header value by name (case-insensitive).
    ///
    /// Returns the value as a string, or `default` if not present.
    #[pyo3(signature = (name, default=None))]
    fn get<'py>(
        &self,
        py: Python<'py>,
        name: &str,
        default: Option<&Bound<'py, PyAny>>,
    ) -> Bound<'py, PyAny> {
        match self.inner.get(name).and_then(|v| v.to_str().ok()) {
            Some(s) => PyString::new(py, s).into_any(),
            None => match default {
                Some(d) => d.clone(),
                None => py.None().into_bound(py),
            },
        }
    }

    /// Check if a header name is present (case-insensitive).
    fn __contains__(&self, name: &str) -> bool {
        self.inner.contains_key(name)
    }

    /// Get a header value by name (case-insensitive), raising `KeyError` if absent.
    fn __getitem__(&self, name: &str) -> PyResult<String> {
        self.inner
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
            .ok_or_else(|| {
                PyErr::new::<pyo3::exceptions::PyKeyError, _>(format!("header '{name}' not found"))
            })
    }

    /// Returns the number of headers.
    fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// Iterate over header names.
    fn __iter__(&self) -> PyResult<Py<PyList>> {
        Python::with_gil(|py| {
            let names: Vec<String> = self.inner.keys().map(|k| k.as_str().to_owned()).collect();
            Ok(PyList::new(py, names)?.into())
        })
    }

    /// Returns a list of header names.
    fn keys(&self) -> PyResult<Py<PyList>> {
        Python::with_gil(|py| {
            let names: Vec<String> = self.inner.keys().map(|k| k.as_str().to_owned()).collect();
            Ok(PyList::new(py, names)?.into())
        })
    }

    /// Returns a list of header values.
    fn values(&self) -> PyResult<Py<PyList>> {
        Python::with_gil(|py| {
            let vals: Vec<String> = self
                .inner
                .values()
                .filter_map(|v| v.to_str().ok())
                .map(str::to_string)
                .collect();
            Ok(PyList::new(py, vals)?.into())
        })
    }

    /// Returns a list of (name, value) tuples.
    fn items(&self) -> PyResult<Py<PyList>> {
        Python::with_gil(|py| {
            let mut result: Vec<Bound<'_, PyTuple>> = Vec::new();
            for (k, v) in &self.inner {
                let name = k.as_str();
                if let Ok(val) = v.to_str() {
                    result.push(PyTuple::new(py, [name, val])?);
                }
            }
            Ok(PyList::new(py, result)?.into())
        })
    }

    /// Get all values for a header name (case-insensitive).
    ///
    /// Returns a list of strings, one per occurrence of the header.
    /// Raises `ValueError` if the header name is invalid.
    fn get_list(&self, name: &str) -> PyResult<Vec<String>> {
        let header_name = http::header::HeaderName::from_bytes(name.as_bytes()).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Invalid header name: {e}"))
        })?;
        let values: Vec<String> = self
            .inner
            .get_all(&header_name)
            .iter()
            .map(|v| String::from_utf8_lossy(v.as_bytes()).to_string())
            .collect();
        Ok(values)
    }

    /// String representation for debugging.
    fn __repr__(&self) -> String {
        let pairs: Vec<String> = self
            .inner
            .iter()
            .filter_map(|(k, v)| {
                let name = k.as_str();
                let val = v.to_str().ok()?;
                Some(format!("{name}: {val}"))
            })
            .collect();
        format!("Headers({{{}}})", pairs.join(", "))
    }
}
