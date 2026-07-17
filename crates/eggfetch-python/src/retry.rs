//! Python retry bindings.

use std::collections::HashSet;
use std::time::Duration;

use pyo3::prelude::*;

/// Python wrapper for an `eggfetch` retry policy.
///
/// Controls which requests are retried, how many times, and with what
/// backoff strategy.
///
/// Args:
///     `max_attempts`: Maximum total attempts including the first request (default 1, meaning no retries).
///     `backoff_factor`: Exponential backoff factor (default 0.5).
///     `max_delay`: Maximum delay between retries in seconds (default 30.0).
///     `initial_delay`: Initial delay before the first retry in seconds (default 0.5).
///     `statuses`: Set of HTTP status codes that trigger a retry (default: {408, 429, 502, 503, 504}).
///     `respect_retry_after`: Whether to respect Retry-After headers (default False).
///     `allow_post`: Whether POST requests may be retried (default False).
///     `allow_put`: Whether PUT requests may be retried (default False).
///     `allow_delete`: Whether DELETE requests may be retried (default False).
///     `allow_patch`: Whether PATCH requests may be retried (default False).
///     `max_elapsed`: Maximum total elapsed time across all attempts in seconds (default None, no limit).
#[pyclass(name = "Retry", frozen)]
#[derive(Clone)]
pub struct PyRetry {
    inner: eggfetch_core::RetryPolicy,
}

#[pymethods]
impl PyRetry {
    #[new]
    #[pyo3(signature = (
        max_attempts=1,
        backoff_factor=0.5,
        max_delay=30.0,
        initial_delay=0.5,
        statuses=None,
        respect_retry_after=false,
        allow_post=false,
        allow_put=false,
        allow_delete=false,
        allow_patch=false,
        max_elapsed=None,
    ))]
    #[allow(
        clippy::too_many_arguments,
        clippy::needless_pass_by_value,
        clippy::unnecessary_wraps,
        clippy::too_many_bool_params
    )]
    fn new(
        max_attempts: usize,
        backoff_factor: f64,
        max_delay: f64,
        initial_delay: f64,
        statuses: Option<HashSet<u16>>,
        respect_retry_after: bool,
        allow_post: bool,
        allow_put: bool,
        allow_delete: bool,
        allow_patch: bool,
        max_elapsed: Option<f64>,
    ) -> PyResult<Self> {
        let mut builder = eggfetch_core::RetryPolicy::builder()
            .max_attempts(max_attempts)
            .backoff_factor(backoff_factor)
            .max_delay(Duration::from_secs_f64(max_delay))
            .initial_delay(Duration::from_secs_f64(initial_delay))
            .respect_retry_after(respect_retry_after);

        if let Some(s) = &statuses {
            builder = builder.retry_statuses(s.iter().copied());
        }

        if allow_post {
            builder = builder.allow_post_retry();
        }
        if allow_put {
            builder = builder.allow_put_retry();
        }
        if allow_delete {
            builder = builder.allow_delete_retry();
        }
        if allow_patch {
            builder = builder.allow_patch_retry();
        }

        if let Some(elapsed) = max_elapsed {
            builder = builder.max_elapsed(Duration::from_secs_f64(elapsed));
        }

        let inner = builder.build();
        Ok(Self { inner })
    }

    fn __repr__(&self) -> String {
        format!(
            "Retry(max_attempts={}, backoff_factor={})",
            self.inner.max_attempts(),
            self.inner.backoff().factor(),
        )
    }

    /// Returns the maximum number of total attempts.
    #[getter]
    fn max_attempts(&self) -> usize {
        self.inner.max_attempts()
    }

    /// Returns the backoff factor.
    #[getter]
    fn backoff_factor(&self) -> f64 {
        self.inner.backoff().factor()
    }

    /// Returns the initial delay before the first retry in seconds.
    #[getter]
    fn initial_delay(&self) -> f64 {
        self.inner.backoff().initial_delay().as_secs_f64()
    }

    /// Returns the maximum delay between retries in seconds.
    #[getter]
    fn max_delay(&self) -> f64 {
        self.inner.backoff().max_delay().as_secs_f64()
    }

    /// Returns whether Retry-After headers are respected.
    #[getter]
    fn respect_retry_after(&self) -> bool {
        self.inner.respect_retry_after()
    }

    /// Returns the set of retryable HTTP status codes.
    #[getter]
    fn statuses(&self) -> Vec<u16> {
        self.inner.status_policy().statuses().to_vec()
    }

    /// Returns the maximum total elapsed time in seconds, or None if unlimited.
    #[getter]
    fn max_elapsed(&self) -> Option<f64> {
        self.inner.max_elapsed().map(|d| d.as_secs_f64())
    }

    /// Returns whether POST requests are retryable.
    #[getter]
    fn allow_post(&self) -> bool {
        self.inner.method_policy().is_retryable(&http::Method::POST)
    }

    /// Returns whether PUT requests are retryable.
    #[getter]
    fn allow_put(&self) -> bool {
        self.inner.method_policy().is_retryable(&http::Method::PUT)
    }

    /// Returns whether DELETE requests are retryable.
    #[getter]
    fn allow_delete(&self) -> bool {
        self.inner
            .method_policy()
            .is_retryable(&http::Method::DELETE)
    }

    /// Returns whether PATCH requests are retryable.
    #[getter]
    fn allow_patch(&self) -> bool {
        self.inner
            .method_policy()
            .is_retryable(&http::Method::PATCH)
    }
}

impl PyRetry {
    /// Returns the inner retry policy.
    pub(crate) fn policy(&self) -> eggfetch_core::RetryPolicy {
        self.inner.clone()
    }

    /// Build a default retry policy (3 attempts, default backoff).
    pub(crate) fn default_policy() -> eggfetch_core::RetryPolicy {
        eggfetch_core::RetryPolicy::builder()
            .max_attempts(3)
            .build()
    }
}

/// Parse a Python `retries` argument into an optional retry policy.
///
/// Accepts:
/// - Omitted or `None` → inherit client-level setting
/// - `True` → use default retry policy (3 attempts)
/// - `False` → disable retries
/// - A `Retry` instance → use that policy
pub(crate) fn parse_retry_option(
    val: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<eggfetch_core::RetryPolicy>> {
    match val {
        None => Ok(None),
        Some(v) => {
            if v.is_none() {
                return Ok(None);
            }
            if let Ok(flag) = v.extract::<bool>() {
                return if flag {
                    Ok(Some(PyRetry::default_policy()))
                } else {
                    Ok(Some(eggfetch_core::RetryPolicy::default()))
                };
            }
            if let Ok(retry) = v.extract::<PyRetry>() {
                return Ok(Some(retry.policy()));
            }
            Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "retries must be True, False, None, or a Retry instance",
            ))
        }
    }
}
