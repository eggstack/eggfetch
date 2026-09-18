//! Stream adapters for request and response body timeouts.
//!
//! [`body_timeout_stream`] is the single authoritative high-level response
//! timeout owner (per-chunk read inactivity plus the absolute total
//! deadline). The former focused `ReadTimeoutStream` was removed once its
//! read-only coverage moved here so no second production timeout path can
//! silently diverge.

mod body_timeout;
mod write_timeout;

pub(crate) use body_timeout::body_timeout_stream;
pub(crate) use write_timeout::write_timeout_stream;
