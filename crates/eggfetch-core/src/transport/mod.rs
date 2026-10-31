//! Transport layer abstractions.
//!
//! Direct and proxy send paths live here. The redirect pipeline and
//! request normalization stay in `pipeline/` and `client.rs`.
//! HTTP/3 transport over QUIC is available when the `http3` feature is enabled.

use bytes::Bytes;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::task::{Context, Poll};

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

/// HTTP request body type expected by Hyper.
///
/// The completion channel marks when Hyper has consumed the request body
/// producer. The high-level response-header read timeout starts at this
/// boundary, so connection setup and a stalled upload remain owned by their
/// connect/write budgets.
pub(crate) struct HyperRequestBody {
    inner: Pin<
        Box<
            http_body_util::combinators::UnsyncBoxBody<
                Bytes,
                Box<dyn std::error::Error + Send + Sync>,
            >,
        >,
    >,
    completion_tx: Mutex<Option<tokio::sync::oneshot::Sender<tokio::time::Instant>>>,
    completion_rx: Option<tokio::sync::oneshot::Receiver<tokio::time::Instant>>,
    complete: AtomicBool,
}

impl HyperRequestBody {
    pub(crate) fn new(
        inner: http_body_util::combinators::UnsyncBoxBody<
            Bytes,
            Box<dyn std::error::Error + Send + Sync>,
        >,
    ) -> Self {
        let (completion_tx, completion_rx) = tokio::sync::oneshot::channel();
        Self {
            inner: Box::pin(inner),
            completion_tx: Mutex::new(Some(completion_tx)),
            completion_rx: Some(completion_rx),
            complete: AtomicBool::new(false),
        }
    }

    pub(crate) fn take_completion_receiver(
        &mut self,
    ) -> Option<tokio::sync::oneshot::Receiver<tokio::time::Instant>> {
        self.completion_rx.take()
    }

    fn mark_complete(&self) {
        if !self.complete.swap(true, Ordering::AcqRel) {
            if let Ok(mut sender) = self.completion_tx.lock() {
                if let Some(sender) = sender.take() {
                    let _ = sender.send(tokio::time::Instant::now());
                }
            }
        }
    }
}

impl http_body::Body for HyperRequestBody {
    type Data = Bytes;
    type Error = Box<dyn std::error::Error + Send + Sync>;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let this = self.as_mut().get_mut();
        match this.inner.as_mut().poll_frame(cx) {
            Poll::Ready(Some(Ok(frame))) => {
                if this.inner.as_ref().get_ref().is_end_stream() {
                    this.mark_complete();
                }
                Poll::Ready(Some(Ok(frame)))
            }
            Poll::Ready(None) => {
                this.mark_complete();
                Poll::Ready(None)
            }
            Poll::Ready(Some(Err(error))) => {
                if let Ok(mut sender) = this.completion_tx.lock() {
                    sender.take();
                }
                Poll::Ready(Some(Err(error)))
            }
            other => other,
        }
    }

    fn is_end_stream(&self) -> bool {
        // Preserve the wrapped body's framing signal exactly. Hyper may
        // inspect an empty body without polling it, so publish completion
        // when that signal is observed as well.
        let is_end_stream = self.inner.as_ref().get_ref().is_end_stream();
        if is_end_stream {
            self.mark_complete();
        }
        is_end_stream
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.inner.as_ref().get_ref().size_hint()
    }
}

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
