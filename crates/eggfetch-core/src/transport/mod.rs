//! Transport layer abstractions.
//!
//! Direct and proxy send paths live here. The redirect pipeline and
//! request normalization stay in `pipeline.rs` and `client.rs`.

use bytes::Bytes;

/// TLS-capable connector used by the hyper client.
pub(crate) type Connector =
    hyper_rustls::HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>;

/// HTTP request body type expected by the hyper client.
pub(crate) type HyperRequestBody =
    http_body_util::combinators::UnsyncBoxBody<Bytes, Box<dyn std::error::Error + Send + Sync>>;

/// Hyper legacy client type used for direct requests.
pub(crate) type HyperClient = hyper_util::client::legacy::Client<Connector, HyperRequestBody>;

pub(crate) mod direct;

#[cfg(feature = "proxy")]
pub(crate) mod connect;

#[cfg(feature = "proxy")]
pub(crate) mod proxy;
