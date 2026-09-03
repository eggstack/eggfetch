//! HTTP/3 transport over QUIC (Quinn + h3).
//!
//! This module is only compiled when the `http3` feature is enabled.
//! HTTP/3 uses QUIC instead of TCP, so it has its own connection lifecycle,
//! TLS configuration, and stream multiplexing model.

use std::sync::Arc;

use bytes::Buf;
use dashmap::DashMap;

use crate::body::{RequestBody, ResponseBody};
use crate::error::{Error, Result};
use crate::response::Response;

/// Type alias for the h3 request sender, parameterised over the
/// h3-quinn connection and bytes body type.
type H3Sender = h3::client::SendRequest<h3_quinn::OpenStreams, bytes::Bytes>;

/// Cached h3 sender and its background driver for a single origin.
///
/// The `SendRequest` is clonable; clones share the same h3 connection.
/// When all clones (including the one stored here) are dropped the h3
/// connection is closed with `HTTP_NO_ERROR`.
struct CachedH3Sender {
    sender: H3Sender,
    _driver: tokio::task::JoinHandle<()>,
}

/// Per-origin cache cell. The [`tokio::sync::OnceCell`] guarantees only
/// one caller per origin establishes the QUIC connection; concurrent
/// requests to the same origin await the same initialization instead of
/// racing get-then-insert (which duplicated connections and orphaned the
/// loser's driver task).
type CachedH3SenderCell = Arc<tokio::sync::OnceCell<CachedH3Sender>>;

/// A shared QUIC endpoint for HTTP/3 connections.
///
/// The endpoint is cloneable and manages the underlying UDP socket.
/// h3 senders are cached per origin so that the same h3 connection
/// (and therefore the same QUIC connection) is reused for subsequent
/// requests to the same host:port.
#[derive(Clone)]
pub(crate) struct H3Connector {
    endpoint: quinn::Endpoint,
    tls_config: Option<crate::tls::TlsConfig>,
    sender_cache: Arc<DashMap<String, CachedH3SenderCell>>,
}

impl H3Connector {
    /// Create a new H3 connector.
    pub(crate) fn new(tls_config: Option<crate::tls::TlsConfig>) -> Result<Self> {
        let bind_addr = "0.0.0.0:0"
            .parse()
            .map_err(|e| Error::Connect(format!("invalid QUIC bind address: {e}")))?;
        let endpoint = quinn::Endpoint::client(bind_addr)
            .map_err(|e| Error::Connect(format!("failed to create QUIC endpoint: {e}")))?;

        Ok(Self {
            endpoint,
            tls_config,
            sender_cache: Arc::new(DashMap::new()),
        })
    }

    /// Issue an HTTP/3 request and return a `Response` with a streaming body.
    #[allow(clippy::too_many_lines)]
    pub(crate) async fn send_request(
        &self,
        request: http::Request<RequestBody>,
        url: url::Url,
    ) -> Result<Response> {
        let host = url
            .host_str()
            .ok_or_else(|| Error::InvalidUrl("missing host".into()))?;
        let port = url.port_or_known_default().unwrap_or(443);
        let cache_key = format!("{host}:{port}");

        // Resolve the address without blocking the tokio worker.
        let addr = tokio::net::lookup_host(format!("{host}:{port}"))
            .await
            .map_err(|e| Error::Connect(format!("DNS resolve: {e}")))?
            .next()
            .ok_or_else(|| Error::Connect("no addresses resolved".into()))?;

        // Get or create the cached h3 sender for this origin. The
        // OnceCell serializes connection establishment per origin: the
        // first caller connects, concurrent callers await the same cell
        // and reuse the resulting sender.
        let cell = self.sender_cache.entry(cache_key).or_default().clone();
        let cached = cell
            .get_or_try_init(|| async {
                // Establish a new QUIC connection.
                let quinn_conn = self.connect_new(addr, host).await?;

                // Build the h3 client – returns (driver, sender).
                let h3_conn = h3_quinn::Connection::new(quinn_conn);
                let (mut driver, sender) = h3::client::new(h3_conn)
                    .await
                    .map_err(|e| Error::H3Protocol(format!("h3 client init: {e}")))?;

                // Drive the h3 connection in the background.
                let driver_handle = tokio::spawn(async move {
                    use futures_util::future;
                    let _ = future::poll_fn(|cx| driver.poll_close(cx)).await;
                });

                Ok::<_, Error>(CachedH3Sender {
                    sender,
                    _driver: driver_handle,
                })
            })
            .await?;
        let sender = cached.sender.clone();

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
        let mut request_stream = {
            let mut sender = sender;
            sender
                .send_request(h3_request)
                .await
                .map_err(|e| Error::H3Protocol(format!("send request: {e}")))?
        };

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
        //
        // The body stream holds only the request_stream. The h3 sender and
        // driver are kept alive by the sender_cache, so we don't need to
        // pass them through here.
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

    /// Establish a new QUIC connection.
    async fn connect_new(
        &self,
        addr: std::net::SocketAddr,
        host: &str,
    ) -> Result<quinn::Connection> {
        let quic_config = build_quic_client_config(self.tls_config.as_ref())?;

        self.endpoint
            .connect_with(quic_config, addr, host)
            .map_err(|e| Error::Connect(format!("QUIC connect: {e}")))?
            .await
            .map_err(|e| Error::H3Connect(format!("QUIC handshake failed: {e}")))
    }
}

/// Build a QUIC client configuration.
///
/// QUIC requires TLS 1.3 only. When a `TlsConfig` is provided, its trust
/// store, client identity, and verification settings are honoured. Otherwise
/// a default config with webpki roots is used.
fn build_quic_client_config(
    tls_config: Option<&crate::tls::TlsConfig>,
) -> Result<quinn::ClientConfig> {
    let rc = if let Some(tc) = tls_config {
        tc.build_quic_rustls_config()?
    } else {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let provider = crate::tls::process_crypto_provider()?;
        let mut rc = rustls::ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .map_err(|e| Error::Tls(format!("TLS version config: {e}")))?
            .with_root_certificates(root_store)
            .with_no_client_auth();

        rc.alpn_protocols = vec![b"h3".to_vec()];
        rc
    };

    let quic_crypto = quinn::crypto::rustls::QuicClientConfig::try_from(rc)
        .map_err(|e| Error::Tls(format!("QUIC TLS config conversion: {e}")))?;

    let mut quic_config = quinn::ClientConfig::new(Arc::new(quic_crypto));

    // Configure transport parameters
    let mut transport = quinn::TransportConfig::default();
    transport.max_concurrent_bidi_streams(100u32.into());
    transport.max_concurrent_uni_streams(100u32.into());

    // 30-second idle timeout
    transport.max_idle_timeout(Some(
        quinn::IdleTimeout::try_from(std::time::Duration::from_secs(30))
            .map_err(|e| Error::Tls(format!("invalid idle timeout: {e}")))?,
    ));

    quic_config.transport_config(Arc::new(transport));

    Ok(quic_config)
}
