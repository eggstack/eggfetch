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

#[cfg(feature = "advanced-routing")]
pub(crate) type CustomConnector = custom_connector::CustomConnector;

#[cfg(feature = "advanced-routing")]
pub(crate) type TimeoutCustomClient = hyper_util::client::legacy::Client<
    lifecycle::LifecycleConnector<connect_timeout::ConnectTimeout<CustomConnector>>,
    HyperRequestBody,
>;

/// Hyper legacy client type with a connect-phase timeout wrapper.
///
/// Standard-route client: ordinary DNS -> TCP/TLS Hyper path. Available with
/// `transport-http1`/`transport-http2` (and therefore with `native-http1`,
/// `native-http2`, `http1`, `http2`, and the lean `standard-http1/2` recipes).
#[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
pub(crate) type TimeoutHyperClient = hyper_util::client::legacy::Client<
    lifecycle::LifecycleConnector<connect_timeout::ConnectTimeout<Connector>>,
    HyperRequestBody,
>;

/// Hyper legacy client type using the direct connector with socket options.
///
/// Advanced-routing only: requires `advanced-routing` (retained by
/// `native-http1`/`native-http2` and therefore by `http1`/`http2`/defaults).
#[cfg(feature = "advanced-routing")]
pub(crate) type TimeoutDirectClient = hyper_util::client::legacy::Client<
    lifecycle::LifecycleConnector<
        connect_timeout::ConnectTimeout<direct_connector::DirectConnector>,
    >,
    HyperRequestBody,
>;

#[cfg(unix)]
#[cfg(feature = "advanced-routing")]
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
#[cfg(any(
    feature = "transport-http1",
    feature = "transport-http2",
    feature = "advanced-routing",
    feature = "proxy"
))]
pub(crate) mod hyper_client;
#[cfg(feature = "advanced-routing")]
pub(crate) mod custom_connector {
    #[cfg(feature = "tls-rustls")]
    pub(crate) type CustomConnector = hyper_rustls::HttpsConnector<super::dialer::DialerConnector>;
    #[cfg(not(feature = "tls-rustls"))]
    pub(crate) type CustomConnector = super::dialer::DialerConnector;
}
#[cfg(feature = "http3")]
pub mod datagram;
pub mod dialer;
#[cfg(any(feature = "transport-http1", feature = "transport-http2"))]
pub(crate) mod direct;
pub mod direct_connector;
pub mod lifecycle;
pub mod metrics;
pub(crate) mod standard_resolver;
#[cfg(all(unix, feature = "advanced-routing"))]
pub(crate) mod uds;

#[cfg(feature = "proxy")]
pub(crate) mod connect;

#[cfg(feature = "proxy")]
pub(crate) mod proxy;

#[cfg(feature = "proxy")]
pub(crate) mod socks;

#[cfg(feature = "http3")]
pub(crate) mod http3;

/// The ALPN protocol identifier for HTTP/2.
pub(crate) const ALPN_HTTP2: &[u8] = b"h2";

/// Whether a negotiated ALPN protocol selects HTTP/2.
///
/// Every transport that reports a negotiated HTTP/2 connection to hyper-util
/// tests the same ALPN payload, so the test lives here once. The helper
/// deliberately takes the already-negotiated protocol and returns a plain
/// answer: each call site keeps its own transport-specific structure (which
/// stream variant it inspects, and whether the `tls-rustls` slice exists) and
/// its own behavior for the answer.
pub(crate) fn negotiated_http2(alpn: Option<&[u8]>) -> bool {
    alpn.is_some_and(|protocol| protocol == ALPN_HTTP2)
}

#[cfg(test)]
mod tests {
    use super::{negotiated_http2, ALPN_HTTP2};

    #[test]
    fn negotiated_http2_detects_only_the_h2_protocol() {
        assert!(negotiated_http2(Some(ALPN_HTTP2)));
        assert!(!negotiated_http2(Some(b"http/1.1")));
        assert!(!negotiated_http2(Some(b"http/1.0")));
        assert!(!negotiated_http2(Some(b"")));
        assert!(!negotiated_http2(None));
    }

    #[test]
    fn negotiated_http2_matches_the_previous_inline_expression() {
        // The four transports used to test the ALPN payload inline. The
        // shared helper must answer exactly as that expression did.
        for candidate in [
            Some(ALPN_HTTP2),
            Some(&b"http/1.1"[..]),
            Some(&b""[..]),
            None,
        ] {
            let expected = matches!(candidate, Some(protocol) if protocol == ALPN_HTTP2);
            assert_eq!(negotiated_http2(candidate), expected);
        }
    }
}
