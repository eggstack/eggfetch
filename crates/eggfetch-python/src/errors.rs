//! Python exception hierarchy mapped from Rust errors.

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;

create_exception!(
    eggfetch,
    EggfetchError,
    PyException,
    "Base exception for all eggfetch errors."
);
create_exception!(
    eggfetch,
    RequestError,
    EggfetchError,
    "Error related to request construction or execution."
);
create_exception!(
    eggfetch,
    InvalidUrl,
    RequestError,
    "The provided URL is invalid."
);
create_exception!(
    eggfetch,
    TimeoutException,
    RequestError,
    "A timeout was exceeded."
);
create_exception!(
    eggfetch,
    PoolTimeout,
    TimeoutException,
    "Pool acquisition timed out."
);
create_exception!(
    eggfetch,
    ConnectTimeout,
    TimeoutException,
    "Connection establishment timed out."
);
create_exception!(
    eggfetch,
    ReadTimeout,
    TimeoutException,
    "Reading response timed out."
);
create_exception!(
    eggfetch,
    WriteTimeout,
    TimeoutException,
    "Writing request body timed out."
);
create_exception!(
    eggfetch,
    NetworkError,
    RequestError,
    "A network-level error occurred."
);
create_exception!(
    eggfetch,
    ProtocolError,
    RequestError,
    "An HTTP protocol error occurred."
);
create_exception!(
    eggfetch,
    BodyError,
    RequestError,
    "An error occurred while processing the body."
);
create_exception!(
    eggfetch,
    HTTPStatusError,
    EggfetchError,
    "The server returned an error status code."
);
create_exception!(
    eggfetch,
    UnsupportedKwarg,
    EggfetchError,
    "An unsupported keyword argument was passed."
);
create_exception!(
    eggfetch,
    TooManyRedirects,
    RequestError,
    "Too many redirects were followed."
);

/// Map an eggfetch-core error to the appropriate Python exception.
#[allow(clippy::needless_pass_by_value)]
pub fn map_err(err: eggfetch_core::Error) -> PyErr {
    use eggfetch_core::timeout::TimeoutPhase;

    match err {
        eggfetch_core::Error::InvalidUrl(msg) => InvalidUrl::new_err(msg),
        eggfetch_core::Error::InvalidMethod(msg)
        | eggfetch_core::Error::InvalidHeaderName(msg)
        | eggfetch_core::Error::InvalidHeaderValue(msg)
        | eggfetch_core::Error::RequestBuild(msg)
        | eggfetch_core::Error::Unsupported(msg)
        | eggfetch_core::Error::Pool(msg) => RequestError::new_err(msg),
        eggfetch_core::Error::Connect(msg) | eggfetch_core::Error::Tls(msg) => {
            NetworkError::new_err(msg)
        }
        eggfetch_core::Error::Protocol(msg) => ProtocolError::new_err(msg),
        eggfetch_core::Error::Body(msg) => BodyError::new_err(msg),
        eggfetch_core::Error::Hyper(arc) => NetworkError::new_err(arc.to_string()),
        eggfetch_core::Error::HyperClient(arc) => NetworkError::new_err(arc.to_string()),
        eggfetch_core::Error::Io(arc) => NetworkError::new_err(arc.to_string()),
        eggfetch_core::Error::InvalidRedirectLocation(msg) => {
            RequestError::new_err(msg)
        }
        eggfetch_core::Error::BodyNotReplayableForRedirect => {
            RequestError::new_err("request body is not replayable for redirect".to_string())
        }
        eggfetch_core::Error::TooManyRedirects { followed, max } => {
            TooManyRedirects::new_err(format!(
                "too many redirects: followed {followed}, max is {max}"
            ))
        }
        eggfetch_core::Error::Timeout { phase, elapsed } => {
            let msg = format!("{phase} timeout after {elapsed:?}");
            match phase {
                TimeoutPhase::Pool => PoolTimeout::new_err(msg),
                TimeoutPhase::Connect => ConnectTimeout::new_err(msg),
                TimeoutPhase::Read => ReadTimeout::new_err(msg),
                TimeoutPhase::Write => WriteTimeout::new_err(msg),
                TimeoutPhase::Total => TimeoutException::new_err(msg),
            }
        }
    }
}
