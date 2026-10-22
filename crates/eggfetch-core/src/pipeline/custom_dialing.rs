//! The custom-dialing incompatibility ladder.
//!
//! A client that installs a custom [`Dialer`](crate::transport::dialer::Dialer)
//! owns connection establishment itself, so several other routing choices
//! cannot be honored at the same time. The engine rejects those combinations
//! with one shared rule instead of letting each dispatch path spell the
//! combinations out again.
//!
//! The rule is evaluated by two different paths:
//!
//! * the high-level preparation path, before the pool is acquired, and
//! * the native dispatch path, before the route is selected.
//!
//! Both paths evaluate the same arms in the same order, so a request that
//! violates more than one combination always produces the same first error.
//! Each arm keeps its own diagnostic: the messages are what operators match
//! on, and the ladder deliberately does not homogenize them.

use crate::error::Error;

/// One arm of the custom-dialing incompatibility ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CustomDialingConflict {
    /// Caller-supplied resolved destinations.
    ResolvedDestinations,
    /// Local-address or socket-option routing.
    LocalAddressOrSocketOptions,
    /// Unix-domain routing.
    UnixDomainRouting,
    /// HTTP/3.
    Http3,
    /// Built-in proxy routing.
    BuiltInProxyRouting,
}

impl CustomDialingConflict {
    /// The diagnostic for this incompatibility.
    pub(super) fn message(self) -> &'static str {
        match self {
            Self::ResolvedDestinations => {
                "custom dialing is incompatible with caller-supplied resolved destinations"
            }
            Self::LocalAddressOrSocketOptions => {
                "custom dialing is incompatible with local-address or socket-option routing"
            }
            Self::UnixDomainRouting => "custom dialing is incompatible with Unix-domain routing",
            Self::Http3 => "custom dialing is incompatible with HTTP/3",
            Self::BuiltInProxyRouting => {
                "custom dialing is incompatible with built-in proxy routing"
            }
        }
    }

    /// The error for this incompatibility.
    pub(super) fn error(self) -> Error {
        Error::Unsupported(self.message().into())
    }
}

/// The conditions the custom-dialing ladder tests.
///
/// The arms are independent routing facts, so they stay named here rather
/// than becoming positional flags at the call site.
#[allow(
    clippy::struct_excessive_bools,
    reason = "the ladder arms are independent routing facts; named fields keep the call sites readable"
)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct CustomDialingState {
    /// A caller-supplied resolved destination is present.
    pub(super) has_resolved_target: bool,
    /// Local-address or socket-option routing is configured.
    pub(super) has_local_address_or_socket_options: bool,
    /// Unix-domain routing is configured.
    pub(super) has_uds: bool,
    /// The client policy is HTTP/3-only.
    pub(super) is_http3_only: bool,
    /// Whether the built-in proxy route applies to this path.
    ///
    /// `None` marks a path that has no built-in-proxy arm at all: the native
    /// dispatch path rejects native proxy frames with its own diagnostic
    /// before the ladder is reached. `Some(false)` marks a path that has the
    /// arm but does not currently use it.
    pub(super) has_built_in_proxy: Option<bool>,
}

/// Evaluate the custom-dialing incompatibility ladder.
///
/// The arms are evaluated in one fixed order, so the first incompatible
/// combination is the one the caller sees:
///
/// 1. caller-supplied resolved destinations
/// 2. local-address or socket-option routing
/// 3. Unix-domain routing
/// 4. HTTP/3
/// 5. built-in proxy routing
///
/// The built-in-proxy arm comes last because the high-level preparation phase
/// evaluates arms 1-4 before the pool is acquired, while the built-in-proxy
/// arm is evaluated at the dispatch boundary, after preparation. The native
/// dispatch path has no built-in-proxy arm.
pub(super) fn custom_dialing_conflict(state: CustomDialingState) -> Option<CustomDialingConflict> {
    if state.has_resolved_target {
        return Some(CustomDialingConflict::ResolvedDestinations);
    }
    if state.has_local_address_or_socket_options {
        return Some(CustomDialingConflict::LocalAddressOrSocketOptions);
    }
    if state.has_uds {
        return Some(CustomDialingConflict::UnixDomainRouting);
    }
    if state.is_http3_only {
        return Some(CustomDialingConflict::Http3);
    }
    if state.has_built_in_proxy == Some(true) {
        return Some(CustomDialingConflict::BuiltInProxyRouting);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{custom_dialing_conflict, CustomDialingConflict, CustomDialingState};

    fn state() -> CustomDialingState {
        CustomDialingState::default()
    }

    #[test]
    fn no_conflict_without_any_arm() {
        assert_eq!(custom_dialing_conflict(state()), None);
    }

    #[test]
    fn resolved_destinations_win_over_every_other_arm() {
        let mut arms = state();
        arms.has_resolved_target = true;
        arms.has_local_address_or_socket_options = true;
        arms.has_uds = true;
        arms.is_http3_only = true;
        arms.has_built_in_proxy = Some(true);
        assert_eq!(
            custom_dialing_conflict(arms),
            Some(CustomDialingConflict::ResolvedDestinations)
        );
    }

    #[test]
    fn local_address_wins_over_uds_and_http3() {
        let mut arms = state();
        arms.has_local_address_or_socket_options = true;
        arms.has_uds = true;
        arms.is_http3_only = true;
        arms.has_built_in_proxy = Some(true);
        assert_eq!(
            custom_dialing_conflict(arms),
            Some(CustomDialingConflict::LocalAddressOrSocketOptions)
        );
    }

    #[test]
    fn uds_wins_over_http3_and_built_in_proxy() {
        let mut arms = state();
        arms.has_uds = true;
        arms.is_http3_only = true;
        arms.has_built_in_proxy = Some(true);
        assert_eq!(
            custom_dialing_conflict(arms),
            Some(CustomDialingConflict::UnixDomainRouting)
        );
    }

    #[test]
    fn http3_wins_over_built_in_proxy() {
        let mut arms = state();
        arms.is_http3_only = true;
        arms.has_built_in_proxy = Some(true);
        assert_eq!(
            custom_dialing_conflict(arms),
            Some(CustomDialingConflict::Http3)
        );
    }

    #[test]
    fn built_in_proxy_is_last() {
        let mut arms = state();
        arms.has_built_in_proxy = Some(true);
        assert_eq!(
            custom_dialing_conflict(arms),
            Some(CustomDialingConflict::BuiltInProxyRouting)
        );
    }

    #[test]
    fn absent_built_in_proxy_arm_never_conflicts() {
        let mut arms = state();
        arms.has_built_in_proxy = None;
        assert_eq!(custom_dialing_conflict(arms), None);
    }

    #[test]
    fn disabled_built_in_proxy_arm_never_conflicts() {
        let mut arms = state();
        arms.has_built_in_proxy = Some(false);
        assert_eq!(custom_dialing_conflict(arms), None);
    }

    #[test]
    fn every_arm_keeps_its_own_message() {
        // The exact texts are frozen by the closure-evidence fixture; here
        // the property under test is that no two arms share a diagnostic.
        let messages = [
            CustomDialingConflict::ResolvedDestinations.message(),
            CustomDialingConflict::LocalAddressOrSocketOptions.message(),
            CustomDialingConflict::UnixDomainRouting.message(),
            CustomDialingConflict::Http3.message(),
            CustomDialingConflict::BuiltInProxyRouting.message(),
        ];
        for (index, message) in messages.iter().enumerate() {
            for other in &messages[index + 1..] {
                assert_ne!(message, other, "each arm keeps its own diagnostic");
            }
        }
    }
}
