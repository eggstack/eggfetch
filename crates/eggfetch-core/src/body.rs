//! Body model for requests and responses.

/// Body placeholder.
///
/// Real body semantics (buffered vs streaming, ownership rules) land in
/// Milestone E.
#[derive(Debug, Default, Clone)]
pub struct Body {
    _private: (),
}
