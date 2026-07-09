//! Stream adapters for request and response body timeouts.

mod read_timeout;
mod write_timeout;

pub(crate) use read_timeout::read_timeout_stream;
pub(crate) use write_timeout::write_timeout_stream;
