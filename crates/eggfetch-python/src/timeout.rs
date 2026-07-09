//! Python timeout wrapper.

use std::time::Duration;

use pyo3::prelude::*;

/// A timeout configuration exposed to Python.
///
/// Accepts a float (seconds) and applies it to pool, connect, write, and read phases.
#[pyclass(name = "Timeout")]
#[derive(Debug, Clone)]
pub struct PyTimeout {
    /// The inner timeout configuration.
    pub inner: eggfetch_core::Timeout,
}

#[pymethods]
impl PyTimeout {
    /// Create a new Timeout.
    ///
    /// Args:
    ///     seconds: Timeout duration in seconds. Applied to pool, connect,
    ///              write, and read phases. Total is not set.
    #[new]
    #[pyo3(signature = (seconds,))]
    fn new(seconds: f64) -> PyResult<Self> {
        if seconds < 0.0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "timeout must be a non-negative number",
            ));
        }
        let duration = Duration::from_secs_f64(seconds);
        Ok(Self {
            inner: eggfetch_core::Timeout {
                pool: Some(duration),
                connect: Some(duration),
                write: Some(duration),
                read: Some(duration),
                total: None,
            },
        })
    }

    /// Pool phase timeout in seconds, if set.
    #[getter]
    fn pool(&self) -> Option<f64> {
        self.inner.pool.map(|d| d.as_secs_f64())
    }

    /// Connect phase timeout in seconds, if set.
    #[getter]
    fn connect(&self) -> Option<f64> {
        self.inner.connect.map(|d| d.as_secs_f64())
    }

    /// Write phase timeout in seconds, if set.
    #[getter]
    fn write(&self) -> Option<f64> {
        self.inner.write.map(|d| d.as_secs_f64())
    }

    /// Read phase timeout in seconds, if set.
    #[getter]
    fn read(&self) -> Option<f64> {
        self.inner.read.map(|d| d.as_secs_f64())
    }

    /// Total timeout in seconds, if set.
    #[getter]
    fn total(&self) -> Option<f64> {
        self.inner.total.map(|d| d.as_secs_f64())
    }

    /// Returns True if any phase has a timeout set.
    fn has_any(&self) -> bool {
        self.inner.has_any()
    }

    fn __repr__(&self) -> String {
        format!(
            "Timeout(pool={:?}, connect={:?}, write={:?}, read={:?}, total={:?})",
            self.inner.pool,
            self.inner.connect,
            self.inner.write,
            self.inner.read,
            self.inner.total
        )
    }
}
