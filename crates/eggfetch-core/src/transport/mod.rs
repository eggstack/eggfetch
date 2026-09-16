//! Transport layer abstractions.
//!
//! Direct and proxy send paths live here. The redirect pipeline and
//! request normalization stay in `pipeline/` and `client.rs`.
//! HTTP/3 transport over QUIC is available when the `http3` feature is enabled.

use bytes::Bytes;

/// Private connection-establishment evidence shared by transport seams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectFailureKind {
    Dns,
    ConnectionRefused,
    Connect,
}

/// TLS-capable connector used by the hyper client.
#[cfg(feature = "tls-rustls")]
pub(crate) type Connector = hyper_rustls::HttpsConnector<
    hyper_util::client::legacy::connect::HttpConnector<standard_resolver::DefaultResolver>,
>;

/// Cleartext connector used when Rustls is not compiled in.
#[cfg(not(feature = "tls-rustls"))]
pub(crate) type Connector =
    hyper_util::client::legacy::connect::HttpConnector<standard_resolver::DefaultResolver>;

/// HTTP request body type expected by the hyper client.
pub(crate) type HyperRequestBody =
    http_body_util::combinators::UnsyncBoxBody<Bytes, Box<dyn std::error::Error + Send + Sync>>;

#[cfg(any(feature = "http1", feature = "http2"))]
pub(crate) type CustomConnector = custom_connector::CustomConnector;

#[cfg(any(feature = "http1", feature = "http2"))]
pub(crate) type TimeoutCustomClient = hyper_util::client::legacy::Client<
    lifecycle::LifecycleConnector<connect_timeout::ConnectTimeout<CustomConnector>>,
    HyperRequestBody,
>;

/// Hyper legacy client type with a connect-phase timeout wrapper.
#[cfg(any(feature = "http1", feature = "http2"))]
pub(crate) type TimeoutHyperClient = hyper_util::client::legacy::Client<
    lifecycle::LifecycleConnector<connect_timeout::ConnectTimeout<Connector>>,
    HyperRequestBody,
>;

/// Hyper legacy client type using the direct connector with socket options.
#[cfg(any(feature = "http1", feature = "http2"))]
pub(crate) type TimeoutDirectClient = hyper_util::client::legacy::Client<
    lifecycle::LifecycleConnector<
        connect_timeout::ConnectTimeout<direct_connector::DirectConnector>,
    >,
    HyperRequestBody,
>;

#[cfg(unix)]
#[cfg(any(feature = "http1", feature = "http2"))]
pub(crate) type TimeoutUdsClient = hyper_util::client::legacy::Client<
    lifecycle::LifecycleConnector<connect_timeout::ConnectTimeout<uds::UdsConnector>>,
    HyperRequestBody,
>;

#[cfg(feature = "proxy")]
pub(crate) type TimeoutSocksClient = hyper_util::client::legacy::Client<
    lifecycle::LifecycleConnector<connect_timeout::ConnectTimeout<socks::SocksConnector>>,
    HyperRequestBody,
>;

/// Hyper client for ordinary HTTP forward-proxy requests. The connector
/// establishes the proxy leg and marks it as proxied so Hyper owns absolute
/// form framing, response parsing, and connection reuse.
#[cfg(feature = "proxy")]
pub(crate) type TimeoutForwardClient = hyper_util::client::legacy::Client<
    lifecycle::LifecycleConnector<connect_timeout::ConnectTimeout<proxy::ForwardProxyConnector>>,
    HyperRequestBody,
>;

/// Hyper client for one HTTPS origin reached through an HTTP CONNECT tunnel.
#[cfg(feature = "proxy")]
pub(crate) type TimeoutConnectClient = hyper_util::client::legacy::Client<
    lifecycle::LifecycleConnector<connect_timeout::ConnectTimeout<connect::ConnectProxyConnector>>,
    HyperRequestBody,
>;

pub mod alt_svc;
pub(crate) mod connect_timeout;
#[cfg(any(feature = "http1", feature = "http2"))]
pub(crate) mod hyper_client;
pub(crate) mod custom_connector {
    #[cfg(feature = "tls-rustls")]
    pub(crate) type CustomConnector = hyper_rustls::HttpsConnector<super::dialer::DialerConnector>;
    #[cfg(not(feature = "tls-rustls"))]
    pub(crate) type CustomConnector = super::dialer::DialerConnector;
}
pub mod dialer;
#[cfg(any(feature = "http1", feature = "http2"))]
pub(crate) mod direct;
pub mod direct_connector;
pub mod lifecycle;
pub mod metrics;
pub(crate) mod standard_resolver;
#[cfg(all(unix, any(feature = "http1", feature = "http2")))]
pub(crate) mod uds;

#[cfg(feature = "proxy")]
pub(crate) mod connect;

#[cfg(feature = "proxy")]
pub(crate) mod proxy;

#[cfg(feature = "proxy")]
pub(crate) mod socks;

#[cfg(feature = "http3")]
pub(crate) mod http3;
