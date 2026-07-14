//! HTTP/3 transport over QUIC (Quinn + h3).
//!
//! This module is only compiled when the `http3` feature is enabled.
//! HTTP/3 uses QUIC instead of TCP, so it has its own connection lifecycle,
//! TLS configuration, and stream multiplexing model.

use std::net::ToSocketAddrs;
use std::sync::Arc;

use bytes::Buf;

use crate::body::{RequestBody, ResponseBody};
use crate::error::{Error, Result};
use crate::response::Response;

/// A shared QUIC endpoint for HTTP/3 connections.
///
/// The endpoint is cloneable and manages the underlying UDP socket.
/// A new QUIC connection is established per request (connection caching
/// may be added in a future milestone).
#[derive(Clone)]
pub(crate) struct H3Connector {
    endpoint: quinn::Endpoint,
    tls_config: Option<crate::tls::TlsConfig>,
}

impl H3Connector {
    /// Create a new H3 connector.
    pub(crate) fn new(tls_config: Option<crate::tls::TlsConfig>) -> Result<Self> {
        let endpoint = quinn::Endpoint::client("0.0.0.0:0".parse().unwrap())
            .map_err(|e| Error::Connect(format!("failed to create QUIC endpoint: {e}")))?;

        Ok(Self {
            endpoint,
            tls_config,
        })
    }

    /// Issue an HTTP/3 request and return a `Response` with a streaming body.
    pub(crate) async fn send_request(
        &self,
        request: http::Request<RequestBody>,
        url: url::Url,
    ) -> Result<Response> {
        let host = url
            .host_str()
            .ok_or_else(|| Error::InvalidUrl("missing host".into()))?;
        let port = url.port_or_known_default().unwrap_or(443);

        // Resolve the address
        let addr = format!("{host}:{port}")
            .to_socket_addrs()
            .map_err(|e| Error::Connect(format!("DNS resolve: {e}")))?
            .next()
            .ok_or_else(|| Error::Connect("no addresses resolved".into()))?;

        // Build QUIC client config
        let quic_config = build_quic_client_config(self.tls_config.as_ref())?;

        // Connect via QUIC
        let quinn_conn = self
            .endpoint
            .connect_with(quic_config, addr, host)
            .map_err(|e| Error::Connect(format!("QUIC connect: {e}")))?
            .await
            .map_err(|e| Error::H3Connect(format!("QUIC handshake failed: {e}")))?;

        // Wrap in h3-quinn connection
        let h3_conn = h3_quinn::Connection::new(quinn_conn);

        // Build h3 client driver and sender
        let (mut driver, mut sender) = h3::client::new(h3_conn)
            .await
            .map_err(|e| Error::H3Protocol(format!("h3 client init: {e}")))?;

        // Spawn the driver to handle connection-level events.
        // The driver must be polled continuously via poll_close().
        tokio::spawn(async move {
            use futures_util::future;
            let _ = future::poll_fn(|cx| driver.poll_close(cx)).await;
        });

        // Decompose the incoming request
        let (parts, body) = request.into_parts();

        // Build h3 request (body is always () for the h3 request frame)
        let mut h3_request = http::Request::builder()
            .method(parts.method)
            .uri(parts.uri)
            .version(parts.version);

        for (name, value) in &parts.headers {
            h3_request = h3_request.header(name, value);
        }

        let h3_request = h3_request
            .body(())
            .map_err(|e| Error::RequestBuild(e.to_string()))?;

        // Send the request headers
        let mut request_stream = sender
            .send_request(h3_request)
            .await
            .map_err(|e| Error::H3Protocol(format!("send request: {e}")))?;

        // Send request body
        match body {
            RequestBody::Empty => {}
            RequestBody::Bytes(bytes) => {
                request_stream
                    .send_data(bytes)
                    .await
                    .map_err(|e| Error::H3Protocol(format!("send data: {e}")))?;
            }
            RequestBody::Stream {
                stream: body_stream,
                ..
            } => {
                use futures_util::StreamExt;
                let mut body_stream = body_stream;
                while let Some(chunk) = body_stream.next().await {
                    let bytes = chunk?;
                    request_stream
                        .send_data(bytes)
                        .await
                        .map_err(|e| Error::H3Protocol(format!("send data: {e}")))?;
                }
            }
        }

        // Signal end of request body
        request_stream
            .finish()
            .await
            .map_err(|e| Error::H3Protocol(format!("finish stream: {e}")))?;

        // Receive response headers
        let response = request_stream
            .recv_response()
            .await
            .map_err(|e| Error::H3Protocol(format!("recv response: {e}")))?;

        let status = response.status();
        let resp_version = response.version();
        let mut resp_headers = http::HeaderMap::new();
        for (name, value) in response.headers() {
            resp_headers.insert(name.clone(), value.clone());
        }

        // Build a streaming response body from the h3 data frames.
        // recv_data() returns `impl Buf`; we convert to Bytes for compatibility
        // with our BoxBytesStream type.
        let body_stream = futures_util::stream::unfold(request_stream, |mut stream| async move {
            match stream.recv_data().await {
                Ok(Some(mut data)) => {
                    let bytes = data.copy_to_bytes(data.remaining());
                    Some((Ok::<_, Error>(bytes), stream))
                }
                Ok(None) => None,
                Err(e) => Some((Err(Error::H3Protocol(format!("recv data: {e}"))), stream)),
            }
        });

        let body = ResponseBody::streaming(Box::pin(body_stream));

        Ok(Response::new(status, resp_version, resp_headers, url, body))
    }
}

/// Build a QUIC client configuration.
///
/// QUIC requires TLS 1.3 only. We build a fresh rustls config with TLS 1.3
/// and default webpki roots. Custom TLS configuration for QUIC will be
/// supported in a future milestone.
fn build_quic_client_config(
    _tls_config: Option<&crate::tls::TlsConfig>,
) -> Result<quinn::ClientConfig> {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let mut rc = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13])
    .map_err(|e| Error::Tls(format!("TLS version config: {e}")))?
    .with_root_certificates(root_store)
    .with_no_client_auth();

    rc.alpn_protocols = vec![b"h3".to_vec()];

    let quic_crypto = quinn::crypto::rustls::QuicClientConfig::try_from(rc)
        .map_err(|e| Error::Tls(format!("QUIC TLS config conversion: {e}")))?;

    let mut quic_config = quinn::ClientConfig::new(Arc::new(quic_crypto));

    // Configure transport parameters
    let mut transport = quinn::TransportConfig::default();
    transport.max_concurrent_bidi_streams(100u32.into());
    transport.max_concurrent_uni_streams(100u32.into());
    quic_config.transport_config(Arc::new(transport));

    Ok(quic_config)
}
