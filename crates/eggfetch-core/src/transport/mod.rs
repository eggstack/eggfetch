//! Transport layer abstractions.
//!
//! Direct and proxy send paths live here. The redirect pipeline and
//! request normalization stay in `pipeline.rs` and `client.rs`.
//! HTTP/3 transport over QUIC is available when the `http3` feature is enabled.

use bytes::Bytes;

/// TLS-capable connector used by the hyper client.
pub(crate) type Connector =
    hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>;

/// HTTP request body type expected by the hyper client.
pub(crate) type HyperRequestBody =
    http_body_util::combinators::UnsyncBoxBody<Bytes, Box<dyn std::error::Error + Send + Sync>>;

/// Hyper legacy client type with a connect-phase timeout wrapper.
pub(crate) type TimeoutHyperClient = hyper_util::client::legacy::Client<
    connect_timeout::ConnectTimeout<Connector>,
    HyperRequestBody,
>;

/// Hyper legacy client type using the direct connector with socket options.
pub(crate) type TimeoutDirectClient = hyper_util::client::legacy::Client<
    connect_timeout::ConnectTimeout<direct_connector::DirectConnector>,
    HyperRequestBody,
>;

#[cfg(unix)]
pub(crate) type TimeoutUdsClient = hyper_util::client::legacy::Client<
    connect_timeout::ConnectTimeout<uds::UdsConnector>,
    HyperRequestBody,
>;

#[cfg(feature = "proxy")]
pub(crate) type TimeoutSocksClient = hyper_util::client::legacy::Client<
    connect_timeout::ConnectTimeout<socks::SocksConnector>,
    HyperRequestBody,
>;

pub(crate) mod connect_timeout;
pub(crate) mod direct;
pub mod direct_connector;
pub(crate) mod uds;

#[cfg(feature = "proxy")]
pub(crate) mod connect;

#[cfg(feature = "proxy")]
pub(crate) mod proxy;

#[cfg(feature = "proxy")]
pub(crate) mod socks;

#[cfg(feature = "http3")]
pub(crate) mod http3;
