//! Client configuration.
//!
//! Pool configuration lives in [`crate::pool::PoolConfig`] (Milestone C).
//! Timeout configuration lives in [`crate::timeout::Timeout`] (Milestone D).
//! Redirect configuration will live here in a future milestone.

/// Client configuration placeholder.
///
/// Concrete fields (TLS config, redirect policy, etc.) are
/// filled in by later milestones. Pool configuration is in
/// [`crate::pool::PoolConfig`].
#[derive(Debug, Default, Clone)]
pub struct Config {
    _private: (),
}
