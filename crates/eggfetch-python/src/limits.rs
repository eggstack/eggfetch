use pyo3::prelude::*;

/// HTTPX-compatible resource limits for the connection pool.
#[pyclass(name = "Limits")]
#[derive(Debug, Clone)]
pub struct PyLimits {
    pub inner: eggfetch_core::Limits,
}

#[pymethods]
impl PyLimits {
    /// Create new Limits.
    ///
    /// Args:
    ///     `max_connections`: Maximum concurrent connections (optional).
    ///     `max_keepalive_connections`: Maximum idle keep-alive connections (optional).
    ///         Applied on a per-host basis; total idle connections grow with
    ///         the number of distinct origins.
    ///     `keepalive_expiry`: Keep-alive timeout in seconds (optional).
    ///     `max_connections_per_host`: Maximum connections per host (optional).
    #[new]
    #[pyo3(signature = (*, max_connections=None, max_keepalive_connections=None, keepalive_expiry=None, max_connections_per_host=None))]
    fn new(
        max_connections: Option<usize>,
        max_keepalive_connections: Option<usize>,
        keepalive_expiry: Option<f64>,
        max_connections_per_host: Option<usize>,
    ) -> PyResult<Self> {
        let keepalive_expiry_duration = match keepalive_expiry {
            Some(secs) => {
                if !secs.is_finite() || secs < 0.0 {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                        "keepalive_expiry must be a finite, non-negative number",
                    ));
                }
                Some(std::time::Duration::from_secs_f64(secs))
            }
            None => None,
        };

        Ok(Self {
            inner: eggfetch_core::Limits {
                max_connections,
                max_connections_per_host,
                max_idle_connections: max_keepalive_connections,
                max_idle_connections_per_host: max_keepalive_connections,
                keepalive_expiry: keepalive_expiry_duration,
            },
        })
    }

    #[getter]
    fn max_connections(&self) -> Option<usize> {
        self.inner.max_connections
    }

    #[getter]
    fn max_keepalive_connections(&self) -> Option<usize> {
        self.inner.max_idle_connections
    }

    #[getter]
    fn keepalive_expiry(&self) -> Option<f64> {
        self.inner.keepalive_expiry.map(|d| d.as_secs_f64())
    }

    #[getter]
    fn max_connections_per_host(&self) -> Option<usize> {
        self.inner.max_connections_per_host
    }

    fn __repr__(&self) -> String {
        format!(
            "Limits(max_connections={:?}, max_keepalive_connections={:?}, keepalive_expiry={:?})",
            self.inner.max_connections,
            self.inner.max_idle_connections,
            self.inner.keepalive_expiry,
        )
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}
