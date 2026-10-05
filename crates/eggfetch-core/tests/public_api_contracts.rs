//! Stable compile contracts for the supported `eggfetch-core` profiles.
//!
//! These are deliberately small use-site checks.  The exact public API
//! snapshots live under `compat/rust-public-api/`; this fixture keeps the
//! highest-value import paths covered by ordinary stable Cargo checks.

use eggfetch_core::{
    Client, ClientBuilder, HttpVersionPolicy, Limits, NativeHttpService, NativeRequestOptions,
    Timeout, TransportHints,
};

#[test]
fn core_root_contracts_compile() {
    let _: fn() -> Client = Client::new;
    let _: fn() -> ClientBuilder = Client::builder;
    let client = Client::new();
    let _service = NativeHttpService::new(client);
    let _ = HttpVersionPolicy::default();
    let _ = Limits::default();
    let _ = Timeout::disabled();
    let _ = Timeout::builder();
    let _ = NativeRequestOptions::default();
    let _ = TransportHints::default();
}

#[cfg(feature = "high-level-url")]
#[test]
fn high_level_request_contracts_compile() {
    let client = Client::new();
    let request = client.get("https://example.com").expect("valid URL");
    let _ = request.header("user-agent", "eggfetch");
    let _ = client.request(http::Method::GET, "https://example.com");
}

#[cfg(feature = "advanced-routing")]
#[test]
fn advanced_routing_contracts_compile() {
    use std::net::SocketAddr;

    let target = eggfetch_core::ResolvedTarget::new(["127.0.0.1:443"
        .parse::<SocketAddr>()
        .expect("socket address")])
    .expect("valid resolved target");
    let hints = eggfetch_core::TransportHints {
        resolved_target: Some(target),
        sni_hostname: Some("example.com".to_owned()),
        ..Default::default()
    };
    let _ = hints;
}

#[cfg(feature = "proxy")]
#[test]
fn proxy_contracts_compile() {
    let _ = eggfetch_core::Proxy::all("http://proxy.example:8080");
    let _ = eggfetch_core::NoProxy::parse("localhost,127.0.0.1");
}

#[cfg(feature = "tls-rustls")]
#[test]
fn tls_contracts_compile() {
    let _ = eggfetch_core::TlsConfig::default();
    let _ = eggfetch_core::TlsConfigBuilder::new();
    let _ = eggfetch_core::TrustStore::WebPkiOnly;
}

#[cfg(feature = "logical-retry")]
#[test]
fn retry_contracts_compile() {
    let _ = eggfetch_core::RetryPolicy::default();
    let _ = eggfetch_core::RetryPolicyBuilder::new();
}

#[cfg(feature = "redirects")]
#[test]
fn redirect_contracts_compile() {
    let _ = eggfetch_core::RedirectPolicy::default();
}

#[test]
fn transport_failure_kind_contract_compiles() {
    let _: fn(&eggfetch_core::Error) -> Option<eggfetch_core::TransportFailureKind> =
        eggfetch_core::Error::transport_failure_kind;
    let _ = [
        eggfetch_core::TransportFailureKind::Connect,
        eggfetch_core::TransportFailureKind::Tls,
        eggfetch_core::TransportFailureKind::Protocol,
        eggfetch_core::TransportFailureKind::Cancelled,
    ];
}

/// Caller-owned H3 datagram routing seam.
///
/// The fixture is deliberately a *use-site* compile check: it proves a caller
/// can implement both halves of the contract and install the dialer, without
/// pinning any particular type. The behavioral contract is covered by the
/// dedicated H3 routing tests.
#[cfg(feature = "http3")]
#[test]
fn caller_owned_h3_datagram_route_contracts_compile() {
    use std::future::Future;
    use std::net::SocketAddr;
    use std::pin::Pin;
    use std::sync::Arc;

    use eggfetch_core::{
        DatagramDialFuture, DatagramDialer, DatagramRecvFuture, DatagramRoute, DatagramSendFuture,
        DialError, DialErrorKind, DialTarget,
    };

    // The boxed future aliases are usable as `Pin<Box<dyn Future + Send>>`,
    // which is what makes both traits usable as trait objects.
    fn _future_shapes() {
        fn _send_is_a_future(_: DatagramSendFuture<'static>) {}
        fn _recv_is_a_future(_: DatagramRecvFuture<'static>) {}
        fn _dial_is_a_future(_: DatagramDialFuture<'static>) {}
        fn _is_a_pinned_send_future(_: Pin<Box<dyn Future<Output = ()> + Send>>) {}
        let _ = _is_a_pinned_send_future;
    }

    fn assert_send<T: Send>(_: T) {}

    /// A compile-only route that reports a fixed peer.
    struct FixtureRoute {
        peer: SocketAddr,
    }

    impl DatagramRoute for FixtureRoute {
        fn send<'a>(&'a self, _payload: &'a [u8]) -> DatagramSendFuture<'a> {
            Box::pin(async { Ok(()) })
        }

        fn recv<'a>(&'a self, _buffer: &'a mut [u8]) -> DatagramRecvFuture<'a> {
            Box::pin(async { Ok(0) })
        }

        fn local_addr(&self) -> SocketAddr {
            self.peer
        }

        fn peer_addr(&self) -> SocketAddr {
            self.peer
        }
    }

    /// A compile-only dialer that always yields the fixture route.
    struct FixtureDialer {
        peer: SocketAddr,
    }

    impl DatagramDialer for FixtureDialer {
        fn connect(&self, target: DialTarget) -> DatagramDialFuture<'_> {
            let peer = self.peer;
            Box::pin(async move {
                // The caller-visible target carries logical origin identity,
                // which is what a real provider resolves into a carrier.
                let _ = target;
                Ok(Arc::new(FixtureRoute { peer }) as Arc<dyn DatagramRoute>)
            })
        }
    }

    // Object safety: both traits are used as trait objects by the bridge.
    fn assert_object_safe(_dialer: &dyn DatagramDialer, _route: &dyn DatagramRoute) {}

    let peer: SocketAddr = "127.0.0.1:4433".parse().expect("socket address");
    let dialer = FixtureDialer { peer };
    let route: Arc<dyn DatagramRoute> = Arc::new(FixtureRoute { peer });
    assert_object_safe(&dialer, route.as_ref());

    // Installing the dialer is a plain additive builder hook.
    let client = eggfetch_core::Client::builder()
        .datagram_dialer(dialer)
        .build();
    let _ = client;

    // The boxed futures are nameable, `Send`, and pinned.
    assert_send(DialError::new(DialErrorKind::Connection, "fixture"));
}
