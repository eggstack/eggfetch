//! Error type and result alias for the eggfetch engine.

use std::fmt;

/// Result alias using [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Error taxonomy placeholder.
///
/// Concrete variants land in later milestones alongside the networking
/// engine. Keep this type the single error entry point for callers.
#[derive(Debug)]
pub enum Error {
    /// Placeholder variant used during skeleton development.
    Unimplemented(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unimplemented(what) => write!(f, "not yet implemented: {what}"),
        }
    }
}

impl std::error::Error for Error {}
