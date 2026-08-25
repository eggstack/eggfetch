//! Python timeout wrapper.

use std::time::Duration;

use pyo3::prelude::*;

/// A timeout configuration exposed to Python.
///
/// Accepts either a float (seconds) for all phases, or keyword arguments
/// for per-phase control. Matches HTTPX's Timeout API shape.
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
    ///     seconds: Timeout duration in seconds (scalar). Applied to pool,
    ///              connect, write, and read phases. Total is not set.
    ///     pool: Pool phase timeout in seconds (optional).
    ///     connect: Connect phase timeout in seconds (optional).
    ///     write: Write phase timeout in seconds (optional).
    ///     read: Read phase timeout in seconds (optional).
    ///     total: Total timeout in seconds (optional).
    #[new]
    #[pyo3(signature = (seconds=None, *, pool=None, connect=None, write=None, read=None, total=None))]
    fn new(
        seconds: Option<f64>,
        pool: Option<f64>,
        connect: Option<f64>,
        write: Option<f64>,
        read: Option<f64>,
        total: Option<f64>,
    ) -> PyResult<Self> {
        // Validate all provided values are finite and non-negative.
        // Zero is valid (an immediately-expiring deadline), matching
        // `parse_timeout` and the HTTPX-compatible Timeout class.
        for (name, val) in [
            ("pool", pool),
            ("connect", connect),
            ("write", write),
            ("read", read),
            ("total", total),
        ] {
            if let Some(v) = val {
                if !v.is_finite() || v < 0.0 {
                    return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                        "{name} timeout must be a finite, non-negative number"
                    )));
                }
            }
        }

        if let Some(s) = seconds {
            if !s.is_finite() || s < 0.0 {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "timeout must be a finite, non-negative number",
                ));
            }
            // Scalar mode: apply to pool, connect, write, read; respect any explicit per-phase
            let duration = Duration::from_secs_f64(s);
            Ok(Self {
                inner: eggfetch_core::Timeout {
                    pool: pool.map(Duration::from_secs_f64).or(Some(duration)),
                    connect: connect.map(Duration::from_secs_f64).or(Some(duration)),
                    write: write.map(Duration::from_secs_f64).or(Some(duration)),
                    read: read.map(Duration::from_secs_f64).or(Some(duration)),
                    total: total.map(Duration::from_secs_f64),
                },
            })
        } else {
            // Per-phase mode
            Ok(Self {
                inner: eggfetch_core::Timeout {
                    pool: pool.map(Duration::from_secs_f64),
                    connect: connect.map(Duration::from_secs_f64),
                    write: write.map(Duration::from_secs_f64),
                    read: read.map(Duration::from_secs_f64),
                    total: total.map(Duration::from_secs_f64),
                },
            })
        }
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
