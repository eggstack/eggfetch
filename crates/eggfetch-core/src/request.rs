//! Request types and builder.

/// HTTP method placeholder.
///
/// Will be replaced by a richer enum in Milestone B.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// `GET` request method.
    Get,
}

/// Outgoing request placeholder.
#[derive(Debug, Clone)]
pub struct Request {
    _private: (),
}

/// Fluent request builder placeholder.
#[derive(Debug)]
pub struct RequestBuilder {
    _private: (),
}
