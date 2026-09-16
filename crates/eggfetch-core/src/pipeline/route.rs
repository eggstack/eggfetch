//! Transport route selection: declarative precedence for dispatch.
//!
//! Precedence: configured UDS, caller-owned custom dialer, specialized direct
//! connector when applicable (no proxy), effective proxy/SOCKS path, SNI
//! override direct path, H3 where selected, standard Hyper direct path. H3
//! never bypasses proxy rules because proxy routes are selected first.

/// Declarative transport route selected after preparation.
///
/// Precedence: configured UDS, caller-owned custom dialer, specialized direct
/// connector when applicable (no proxy), effective proxy/SOCKS path, SNI
/// override direct path, H3 where selected, standard Hyper direct path. H3
/// never bypasses proxy rules because proxy routes are selected first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TransportRoute {
    /// Configured Unix-domain-socket client.
    Uds,
    /// Caller-supplied raw-stream dialer.
    Custom,
    /// Specialized direct connector (socket options / local address).
    Direct,
    /// Effective proxy or SOCKS path.
    Proxy,
    /// Cached SNI-override direct client.
    SniDirect,
    /// HTTP/3 over QUIC.
    H3,
    /// Standard Hyper direct path.
    Standard,
}

/// Select the transport route from prepared state.
///
/// Pure function over availability flags so precedence is directly tested
/// without constructing clients.
#[allow(
    clippy::fn_params_excessive_bools,
    reason = "route selection is a pure precedence predicate over six availability flags; bundling into a struct adds indirection for tests"
)]
pub(super) fn select_route(
    has_uds: bool,
    has_custom: bool,
    has_direct_no_proxy: bool,
    has_proxy: bool,
    has_sni: bool,
    use_h3: bool,
) -> TransportRoute {
    if has_uds {
        TransportRoute::Uds
    } else if has_custom {
        TransportRoute::Custom
    } else if has_direct_no_proxy {
        TransportRoute::Direct
    } else if has_proxy {
        TransportRoute::Proxy
    } else if has_sni {
        TransportRoute::SniDirect
    } else if use_h3 {
        TransportRoute::H3
    } else {
        TransportRoute::Standard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_route_precedence_matches_pipeline_order() {
        // UDS wins over everything.
        assert_eq!(
            select_route(true, false, true, true, true, true),
            TransportRoute::Uds
        );
        // Custom dialer wins over everything except UDS; it is never
        // silently bypassed (custom+proxy fails closed before selection).
        assert_eq!(
            select_route(true, true, true, true, true, true),
            TransportRoute::Uds
        );
        assert_eq!(
            select_route(false, true, true, true, true, true),
            TransportRoute::Custom
        );
        assert_eq!(
            select_route(false, true, false, false, false, false),
            TransportRoute::Custom
        );
        // Specialized direct wins when no proxy (even with SNI/H3 present).
        assert_eq!(
            select_route(false, false, true, false, true, true),
            TransportRoute::Direct
        );
        // Proxy wins over SNI/H3; H3 never bypasses proxy rules.
        assert_eq!(
            select_route(false, false, false, true, true, true),
            TransportRoute::Proxy
        );
        assert_eq!(
            select_route(false, false, false, true, false, true),
            TransportRoute::Proxy
        );
        // SNI wins over H3/standard when no proxy/direct.
        assert_eq!(
            select_route(false, false, false, false, true, true),
            TransportRoute::SniDirect
        );
        // H3 only when selected and no earlier route applies.
        assert_eq!(
            select_route(false, false, false, false, false, true),
            TransportRoute::H3
        );
        assert_eq!(
            select_route(false, false, false, false, false, false),
            TransportRoute::Standard
        );
        // `has_direct_no_proxy` already encodes "no proxy": callers compute
        // it as `direct.is_some() && !has_proxy`, so a proxy-present call
        // always passes `false` here and selects Proxy (H3 never bypasses
        // proxy rules).
    }
}
