//! Timeout model.

/// Timeout configuration placeholder.
///
/// Phase-aware timeouts (connect, pool, write, read, total) land in
/// Milestone D.
#[derive(Debug, Default, Clone, Copy)]
pub struct Timeout {
    _private: (),
}
