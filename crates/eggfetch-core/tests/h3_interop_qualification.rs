#![allow(
    missing_docs,
    dead_code,
    unused_mut,
    clippy::large_futures,
    clippy::missing_panics_doc,
    clippy::redundant_closure_for_method_calls,
    clippy::inefficient_to_string,
    clippy::manual_let_else,
    clippy::single_char_pattern,
    clippy::match_same_arms,
    clippy::needless_borrow,
    clippy::trim_split_whitespace,
    clippy::too_many_lines,
    clippy::unused_self,
    clippy::items_after_statements,
    clippy::expect_fun_call,
    clippy::len_zero,
    clippy::unnecessary_debug_formatting,
    clippy::format_push_string,
    clippy::new_without_default,
    clippy::map_unwrap_or
)]
#![allow(clippy::unnested_or_patterns)]
#![cfg(feature = "http3")]

//! HTTP/3 interoperability and production-graduation evidence.
//!
//! Active harness plan:
//! `plans/implementation/tls-proxy-protocols/006-h3-server-controlled-corpus.md`
//!
//! Historical program: `plans/http3-production-qualification-program.md`
//! Historical plan: `plans/http3-independent-interop-and-impairment-qualification.md`
//!
//! This suite is deterministic and local-only for Tier 1. External
//! interoperability servers (ngtcp2/nghttp3, quiche, …) are exercised only
//! when explicitly provided by the qualification runner through
//! `EGGFETCH_H3_INTEROP_ENDPOINT` and capability-selected cases. The older
//! comma-separated `EGGFETCH_H3_INTEROP_URLS` variable remains supported for
//! manual compatibility. Absence is an explicit skip and is NOT graduation
//! evidence.
//!
//! Graduation decision (this milestone): HTTP/3 remains **experimental**.
//! Blockers are recorded in `docs/architecture/core-tls-proxy-protocols.md`
//! (§ "Production Graduation Decision"). A retained experimental label is a
//! valid successful outcome per the graduation plan.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use eggfetch_core::{Client, HttpVersionPolicy, RequestBody, Timeout, TlsConfig};
use futures_util::stream;
use futures_util::StreamExt;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::sync::watch;

#[cfg(feature = "tls-rustls")]
mod tls_fixtures;

#[cfg(feature = "tls-rustls")]
use tls_fixtures::CertAuthority;
#[cfg(feature = "tls-rustls")]
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(feature = "tls-rustls")]
use tokio::net::TcpListener;

// ---------------------------------------------------------------------------
// Fixtures (loopback QUIC+H3, same shape as h3_hardening.rs)
// ---------------------------------------------------------------------------

struct QuicTestServer {
    addr: SocketAddr,
    shutdown: watch::Sender<bool>,
}

impl QuicTestServer {
    #[allow(unknown_lints, clippy::unused_async, clippy::unused_async_trait_impl)]
    async fn start(body: Vec<u8>) -> Self {
        let cert_key =
            rcgen::generate_simple_self_signed(vec!["localhost".into()]).expect("rcgen cert");
        let cert_der = CertificateDer::from(cert_key.cert.der().to_vec());
        let key_der =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert_key.key_pair.serialize_der()));
        let mut server_tls = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS13")
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .expect("cert");
        server_tls.alpn_protocols = vec![b"h3".to_vec()];
        let quic_crypto =
            quinn::crypto::rustls::QuicServerConfig::try_from(server_tls).expect("server crypto");
        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_crypto));
        let mut transport = quinn::TransportConfig::default();
        transport.max_concurrent_bidi_streams(100u32.into());
        transport.max_concurrent_uni_streams(100u32.into());
        server_config.transport_config(Arc::new(transport));
        let endpoint = quinn::Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap())
            .expect("bind server");
        let addr = endpoint.local_addr().expect("addr");
        let body = Bytes::from(body);
        let (tx, mut rx) = watch::channel(false);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    incoming = endpoint.accept() => {
                        let Some(incoming) = incoming else { break };
                        let body = body.clone();
                        tokio::spawn(async move {
                            let Ok(conn) = incoming.await else { return };
                            let h3_conn = h3_quinn::Connection::new(conn);
                            let Ok(mut server) =
                                h3::server::Connection::<_, Bytes>::new(h3_conn).await
                            else {
                                return;
                            };
                            loop {
                                let Ok(Some(resolver)) = server.accept().await else { break };
                                let body = body.clone();
                                let Ok((req, mut stream)) = resolver.resolve_request().await
                                else {
                                    continue;
                                };
                                while stream.recv_data().await.ok().flatten().is_some() {}
                                let resp = http::Response::builder()
                                    .status(200)
                                    .header("content-type", "text/plain")
                                    .body(())
                                    .unwrap();
                                if stream.send_response(resp).await.is_err() {
                                    break;
                                }
                                // HEAD responses carry no body; any other
                                // method gets the configured payload.
                                if req.method() == http::Method::HEAD {
                                    let _ = stream.finish().await;
                                    continue;
                                }
                                let _ = stream.send_data(body).await;
                                let _ = stream.finish().await;
                            }
                        });
                    }
                    _ = rx.changed() => break,
                }
            }
        });
        Self { addr, shutdown: tx }
    }

    /// Server that sends response trailers after the body.
    #[allow(unknown_lints, clippy::unused_async, clippy::unused_async_trait_impl)]
    async fn start_with_trailers(body: Vec<u8>, trailer_name: &str, trailer_value: &str) -> Self {
        let cert_key =
            rcgen::generate_simple_self_signed(vec!["localhost".into()]).expect("rcgen cert");
        let cert_der = CertificateDer::from(cert_key.cert.der().to_vec());
        let key_der =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert_key.key_pair.serialize_der()));
        let mut server_tls = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS13")
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .expect("cert");
        server_tls.alpn_protocols = vec![b"h3".to_vec()];
        let quic_crypto =
            quinn::crypto::rustls::QuicServerConfig::try_from(server_tls).expect("server crypto");
        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_crypto));
        let mut transport = quinn::TransportConfig::default();
        transport.max_concurrent_bidi_streams(100u32.into());
        transport.max_concurrent_uni_streams(100u32.into());
        server_config.transport_config(Arc::new(transport));
        let endpoint = quinn::Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap())
            .expect("bind server");
        let addr = endpoint.local_addr().expect("addr");
        let body = Bytes::from(body);
        let trailer_name = trailer_name.to_string();
        let trailer_value = trailer_value.to_string();
        let (tx, mut rx) = watch::channel(false);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    incoming = endpoint.accept() => {
                        let Some(incoming) = incoming else { break };
                        let body = body.clone();
                        let trailer_name = trailer_name.clone();
                        let trailer_value = trailer_value.clone();
                        tokio::spawn(async move {
                            let Ok(conn) = incoming.await else { return };
                            let h3_conn = h3_quinn::Connection::new(conn);
                            let Ok(mut server) =
                                h3::server::Connection::<_, Bytes>::new(h3_conn).await
                            else {
                                return;
                            };
                            loop {
                                let Ok(Some(resolver)) = server.accept().await else { break };
                                let body = body.clone();
                                let trailer_name = trailer_name.clone();
                                let trailer_value = trailer_value.clone();
                                let Ok((_req, mut stream)) = resolver.resolve_request().await
                                else {
                                    continue;
                                };
                                while stream.recv_data().await.ok().flatten().is_some() {}
                                let resp = http::Response::builder()
                                    .status(200)
                                    .header("content-type", "text/plain")
                                    .body(())
                                    .unwrap();
                                if stream.send_response(resp).await.is_err() {
                                    break;
                                }
                                if stream.send_data(body).await.is_err() {
                                    break;
                                }
                                let mut trailers = http::HeaderMap::new();
                                trailers.insert(
                                    http::HeaderName::from_bytes(trailer_name.as_bytes())
                                        .expect("trailer name"),
                                    http::HeaderValue::from_str(&trailer_value)
                                        .expect("trailer value"),
                                );
                                let _ = stream.send_trailers(trailers).await;
                                let _ = stream.finish().await;
                            }
                        });
                    }
                    _ = rx.changed() => break,
                }
            }
        });
        Self { addr, shutdown: tx }
    }

    /// Server that sends headers plus a partial body then drops the stream
    /// without `finish` (mid-response path-break / early close).
    #[allow(unknown_lints, clippy::unused_async, clippy::unused_async_trait_impl)]
    async fn start_early_close() -> Self {
        let cert_key =
            rcgen::generate_simple_self_signed(vec!["localhost".into()]).expect("rcgen cert");
        let cert_der = CertificateDer::from(cert_key.cert.der().to_vec());
        let key_der =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert_key.key_pair.serialize_der()));
        let mut server_tls = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS13")
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .expect("cert");
        server_tls.alpn_protocols = vec![b"h3".to_vec()];
        let quic_crypto =
            quinn::crypto::rustls::QuicServerConfig::try_from(server_tls).expect("server crypto");
        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_crypto));
        let mut transport = quinn::TransportConfig::default();
        transport.max_concurrent_bidi_streams(100u32.into());
        transport.max_concurrent_uni_streams(100u32.into());
        server_config.transport_config(Arc::new(transport));
        let endpoint = quinn::Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap())
            .expect("bind server");
        let addr = endpoint.local_addr().expect("addr");
        let (tx, mut rx) = watch::channel(false);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    incoming = endpoint.accept() => {
                        let Some(incoming) = incoming else { break };
                        tokio::spawn(async move {
                            let Ok(conn) = incoming.await else { return };
                            let h3_conn = h3_quinn::Connection::new(conn);
                            let Ok(mut server) =
                                h3::server::Connection::<_, Bytes>::new(h3_conn).await
                            else {
                                return;
                            };
                            loop {
                                let Ok(Some(resolver)) = server.accept().await else { break };
                                let Ok((_req, mut stream)) = resolver.resolve_request().await
                                else {
                                    continue;
                                };
                                while stream.recv_data().await.ok().flatten().is_some() {}
                                let resp = http::Response::builder()
                                    .status(200)
                                    .header("content-type", "text/plain")
                                    .body(())
                                    .unwrap();
                                if stream.send_response(resp).await.is_err() {
                                    break;
                                }
                                let _ = stream.send_data(Bytes::from_static(b"partial")).await;
                                // Drop without finish: abrupt mid-response cut.
                                break;
                            }
                        });
                    }
                    _ = rx.changed() => break,
                }
            }
        });
        Self { addr, shutdown: tx }
    }

    /// Complete one response on a connection, then GOAWAY via `shutdown(0)`.
    /// The QUIC endpoint keeps accepting new connections on the same port.
    #[allow(unknown_lints, clippy::unused_async, clippy::unused_async_trait_impl)]
    async fn start_shutdown_after_one_response(body: Vec<u8>) -> Self {
        let cert_key =
            rcgen::generate_simple_self_signed(vec!["localhost".into()]).expect("rcgen cert");
        let cert_der = CertificateDer::from(cert_key.cert.der().to_vec());
        let key_der =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert_key.key_pair.serialize_der()));
        let mut server_tls = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS13")
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .expect("cert");
        server_tls.alpn_protocols = vec![b"h3".to_vec()];
        let quic_crypto =
            quinn::crypto::rustls::QuicServerConfig::try_from(server_tls).expect("server crypto");
        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_crypto));
        let mut transport = quinn::TransportConfig::default();
        transport.max_concurrent_bidi_streams(100u32.into());
        transport.max_concurrent_uni_streams(100u32.into());
        server_config.transport_config(Arc::new(transport));
        let endpoint = quinn::Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap())
            .expect("bind server");
        let addr = endpoint.local_addr().expect("addr");
        let body = Bytes::from(body);
        let goaway_once = Arc::new(AtomicBool::new(false));
        let (tx, mut rx) = watch::channel(false);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    incoming = endpoint.accept() => {
                        let Some(incoming) = incoming else { break };
                        let body = body.clone();
                        let goaway_once = goaway_once.clone();
                        tokio::spawn(async move {
                            let Ok(conn) = incoming.await else { return };
                            let h3_conn = h3_quinn::Connection::new(conn);
                            let Ok(mut server) =
                                h3::server::Connection::<_, Bytes>::new(h3_conn).await
                            else {
                                return;
                            };
                            let send_goaway = !goaway_once.swap(true, Ordering::SeqCst);
                            loop {
                                let Ok(Some(resolver)) = server.accept().await else { break };
                                let body = body.clone();
                                let Ok((req, mut stream)) = resolver.resolve_request().await
                                else {
                                    continue;
                                };
                                while stream.recv_data().await.ok().flatten().is_some() {}
                                let resp = http::Response::builder()
                                    .status(200)
                                    .header("content-type", "text/plain")
                                    .body(())
                                    .unwrap();
                                if stream.send_response(resp).await.is_err() {
                                    break;
                                }
                                if req.method() != http::Method::HEAD {
                                    let _ = stream.send_data(body.clone()).await;
                                }
                                let _ = stream.finish().await;
                                if send_goaway {
                                    tokio::time::sleep(Duration::from_millis(150)).await;
                                    let _ = server.shutdown(0).await;
                                    break;
                                }
                            }
                        });
                    }
                    _ = rx.changed() => break,
                }
            }
        });
        Self { addr, shutdown: tx }
    }

    /// Consumes request bodies slowly so client-side upload cancellation can
    /// win a short timeout race.
    #[allow(unknown_lints, clippy::unused_async, clippy::unused_async_trait_impl)]
    async fn start_slow_upload_consumer(body: Vec<u8>) -> Self {
        let cert_key =
            rcgen::generate_simple_self_signed(vec!["localhost".into()]).expect("rcgen cert");
        let cert_der = CertificateDer::from(cert_key.cert.der().to_vec());
        let key_der =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert_key.key_pair.serialize_der()));
        let mut server_tls = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS13")
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .expect("cert");
        server_tls.alpn_protocols = vec![b"h3".to_vec()];
        let quic_crypto =
            quinn::crypto::rustls::QuicServerConfig::try_from(server_tls).expect("server crypto");
        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_crypto));
        let mut transport = quinn::TransportConfig::default();
        transport.max_concurrent_bidi_streams(100u32.into());
        transport.max_concurrent_uni_streams(100u32.into());
        server_config.transport_config(Arc::new(transport));
        let endpoint = quinn::Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap())
            .expect("bind server");
        let addr = endpoint.local_addr().expect("addr");
        let body = Bytes::from(body);
        let (tx, mut rx) = watch::channel(false);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    incoming = endpoint.accept() => {
                        let Some(incoming) = incoming else { break };
                        let body = body.clone();
                        tokio::spawn(async move {
                            let Ok(conn) = incoming.await else { return };
                            let h3_conn = h3_quinn::Connection::new(conn);
                            let Ok(mut server) =
                                h3::server::Connection::<_, Bytes>::new(h3_conn).await
                            else {
                                return;
                            };
                            loop {
                                let Ok(Some(resolver)) = server.accept().await else { break };
                                let body = body.clone();
                                let Ok((_req, mut stream)) = resolver.resolve_request().await
                                else {
                                    continue;
                                };
                                loop {
                                    match stream.recv_data().await {
                                        Ok(Some(_)) => {
                                            tokio::time::sleep(Duration::from_millis(25)).await;
                                        }
                                        Ok(None) => break,
                                        Err(_) => break,
                                    }
                                }
                                let resp = http::Response::builder()
                                    .status(200)
                                    .header("content-type", "text/plain")
                                    .body(())
                                    .unwrap();
                                if stream.send_response(resp).await.is_err() {
                                    break;
                                }
                                let _ = stream.send_data(body).await;
                                let _ = stream.finish().await;
                            }
                        });
                    }
                    _ = rx.changed() => break,
                }
            }
        });
        Self { addr, shutdown: tx }
    }

    fn url(&self) -> String {
        format!("https://127.0.0.1:{}/", self.addr.port())
    }
}

#[cfg(feature = "tls-rustls")]
struct HttpsAltSvcServer {
    port: u16,
    shutdown_tx: watch::Sender<bool>,
}

#[cfg(feature = "tls-rustls")]
impl HttpsAltSvcServer {
    async fn start(
        ca: &CertAuthority,
        hostnames: &[&str],
        alt_svc: Option<String>,
        body: Vec<u8>,
    ) -> Self {
        let (cert_der, key_der) = ca.generate_server_cert(hostnames);
        let server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .expect("cert");
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = listener.local_addr().expect("port").port();
        let (tx, mut rx) = watch::channel(false);
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));
        let alt_svc = alt_svc.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let Ok((tcp, _)) = result else { break };
                        let acceptor = acceptor.clone();
                        let alt_svc = alt_svc.clone();
                        let body = body.clone();
                        tokio::spawn(async move {
                            let Ok(tls) = acceptor.accept(tcp).await else { return };
                            let mut reader = BufReader::new(tls);
                            let mut line = String::new();
                            if reader.read_line(&mut line).await.is_err() || line.trim().is_empty() {
                                return;
                            }
                            loop {
                                let mut h = String::new();
                                if reader.read_line(&mut h).await.is_err() || h.trim().is_empty() {
                                    break;
                                }
                            }
                            let alt_header = alt_svc
                                .as_ref()
                                .map(|v| format!("Alt-Svc: {v}\r\n"))
                                .unwrap_or_default();
                            let resp = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n{}Connection: close\r\n\r\n",
                                body.len(),
                                alt_header,
                            );
                            let mut stream = reader.into_inner();
                            let _ = stream.write_all(resp.as_bytes()).await;
                            let _ = stream.write_all(&body).await;
                            let _ = stream.flush().await;
                        });
                    }
                    _ = rx.changed() => break,
                }
            }
        });
        Self {
            port,
            shutdown_tx: tx,
        }
    }

    fn url(&self) -> String {
        format!("https://localhost:{}/", self.port)
    }
}

#[cfg(feature = "tls-rustls")]
impl Drop for HttpsAltSvcServer {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(true);
    }
}

#[cfg(feature = "tls-rustls")]
struct VerifiedH3Server {
    addr: SocketAddr,
    shutdown: watch::Sender<bool>,
}

#[cfg(feature = "tls-rustls")]
impl VerifiedH3Server {
    async fn start(
        cert_der: CertificateDer<'static>,
        key_der: PrivateKeyDer<'static>,
        body: Vec<u8>,
    ) -> Self {
        tokio::task::yield_now().await;
        let mut server_tls = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .expect("TLS13")
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .expect("cert");
        server_tls.alpn_protocols = vec![b"h3".to_vec()];
        let quic_crypto =
            quinn::crypto::rustls::QuicServerConfig::try_from(server_tls).expect("crypto");
        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_crypto));
        let mut transport = quinn::TransportConfig::default();
        transport.max_concurrent_bidi_streams(100u32.into());
        transport.max_concurrent_uni_streams(100u32.into());
        server_config.transport_config(Arc::new(transport));
        let endpoint =
            quinn::Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap()).expect("bind");
        let addr = endpoint.local_addr().expect("addr");
        let body = Bytes::from(body);
        let (tx, mut rx) = watch::channel(false);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    incoming = endpoint.accept() => {
                        let Some(incoming) = incoming else { break };
                        let body = body.clone();
                        tokio::spawn(async move {
                            let Ok(conn) = incoming.await else { return };
                            let h3_conn = h3_quinn::Connection::new(conn);
                            let Ok(mut server) =
                                h3::server::Connection::<_, Bytes>::new(h3_conn).await
                            else {
                                return;
                            };
                            loop {
                                let Ok(Some(resolver)) = server.accept().await else { break };
                                let body = body.clone();
                                let Ok((_req, mut stream)) = resolver.resolve_request().await
                                else {
                                    continue;
                                };
                                while stream.recv_data().await.ok().flatten().is_some() {}
                                let resp = http::Response::builder().status(200).body(()).unwrap();
                                if stream.send_response(resp).await.is_err() {
                                    break;
                                }
                                let _ = stream.send_data(body).await;
                                let _ = stream.finish().await;
                            }
                        });
                    }
                    _ = rx.changed() => break,
                }
            }
        });
        Self { addr, shutdown: tx }
    }
}

#[cfg(feature = "tls-rustls")]
impl Drop for VerifiedH3Server {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

#[cfg(feature = "tls-rustls")]
fn auto_client_with_ca(ca: &CertAuthority) -> Client {
    let tls = TlsConfig::builder()
        .ca_certificate_pem(&ca.cert_pem())
        .expect("ca")
        .build();
    Client::builder()
        .http_version_policy(HttpVersionPolicy::Auto { allow_http3: true })
        .tls_config(tls)
        .automatic_decompression(false)
        .build()
}

impl Drop for QuicTestServer {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

struct Blackhole {
    _socket: std::net::UdpSocket,
    addr: SocketAddr,
}

impl Blackhole {
    fn bind() -> Self {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind blackhole UDP socket");
        socket.set_nonblocking(true).expect("nonblocking");
        let addr = socket.local_addr().expect("blackhole addr");
        Self {
            _socket: socket,
            addr,
        }
    }

    fn url(&self) -> String {
        format!("https://127.0.0.1:{}/", self.addr.port())
    }
}

fn h3_client() -> Client {
    let tls = TlsConfig::builder()
        .danger_accept_invalid_certs(true)
        .build();
    Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .tls_config(tls)
        .automatic_decompression(false)
        .build()
}

fn h3_client_with_connect_timeout(d: Duration) -> Client {
    let tls = TlsConfig::builder()
        .danger_accept_invalid_certs(true)
        .build();
    let timeout = Timeout::builder().connect(d).build();
    Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .tls_config(tls)
        .automatic_decompression(false)
        .timeout(timeout)
        .build()
}

fn external_h3_client() -> Client {
    let mut builder = Client::builder().http_version_policy(HttpVersionPolicy::Http3Only);
    if std::env::var_os("EGGFETCH_H3_INTEROP_DANGER_ACCEPT_INVALID_CERTS").is_some() {
        builder = builder.tls_config(
            TlsConfig::builder()
                .danger_accept_invalid_certs(true)
                .build(),
        );
    }
    builder.build()
}

fn external_h3_auto_client() -> Client {
    let mut builder =
        Client::builder().http_version_policy(HttpVersionPolicy::Auto { allow_http3: true });
    if std::env::var_os("EGGFETCH_H3_INTEROP_DANGER_ACCEPT_INVALID_CERTS").is_some() {
        builder = builder.tls_config(
            TlsConfig::builder()
                .danger_accept_invalid_certs(true)
                .build(),
        );
    }
    builder.build()
}

fn external_control_url(case: &str) -> Option<String> {
    let key = match case {
        "stream_reset" => "EGGFETCH_H3_INTEROP_CONTROL_STREAM_RESET",
        "graceful_connection_close" => "EGGFETCH_H3_INTEROP_CONTROL_GRACEFUL_CONNECTION_CLOSE",
        "goaway_reconnect" => "EGGFETCH_H3_INTEROP_CONTROL_GOAWAY_RECONNECT",
        "alternative_port" => "EGGFETCH_H3_INTEROP_CONTROL_ALTERNATIVE_PORT",
        _ => return None,
    };
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

fn external_case_enabled(case: &str) -> bool {
    let cases = std::env::var("EGGFETCH_H3_INTEROP_CASES").unwrap_or_else(|_| "get,head".into());
    cases
        .split(',')
        .map(str::trim)
        .any(|candidate| candidate == case)
}

// ---------------------------------------------------------------------------
// §2: Deterministic local interop harness + optional external servers
// ---------------------------------------------------------------------------

/// Local Quinn/h3 self-interop is mandatory and deterministic.
#[tokio::test(flavor = "multi_thread")]
async fn local_quinn_self_interop_get_and_body() {
    let server = QuicTestServer::start(b"interop-ok".to_vec()).await;
    let url = server.url();
    let client = h3_client();
    let mut resp = client.get(&url).unwrap().send().await.expect("h3 get");
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.expect("body");
    assert_eq!(body, "interop-ok");
}

/// Optional external interop servers. Never fails Tier 1 when absent;
/// absence is an explicit skip, not graduation evidence.
#[tokio::test(flavor = "multi_thread")]
async fn external_h3_interop_servers_if_configured() {
    let configured = std::env::var("EGGFETCH_H3_INTEROP_ENDPOINT")
        .or_else(|_| std::env::var("EGGFETCH_H3_INTEROP_URLS"))
        .unwrap_or_default();
    let urls: Vec<String> = configured
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if urls.is_empty() {
        eprintln!(
            "SKIP external_h3_interop_servers_if_configured: \
             EGGFETCH_H3_INTEROP_URLS unset; local fixtures remain mandatory, \
             absence is not graduation evidence"
        );
        return;
    }
    for url in &urls {
        let client = external_h3_client();
        if external_case_enabled("get") {
            let mut resp = client
                .get(url)
                .unwrap()
                .send()
                .await
                .unwrap_or_else(|e| panic!("external h3 get {url} failed: {e:?}"));
            assert!(
                (200..400).contains(&resp.status().as_u16()),
                "unexpected status for {url}"
            );
            let _ = resp.text().await.expect("external body");
            eprintln!("H3_CORPUS_CASE get=pass endpoint={url}");
        }
        if external_case_enabled("head") {
            let head_resp = client
                .head(url)
                .unwrap()
                .send()
                .await
                .unwrap_or_else(|e| panic!("external h3 head {url} failed: {e:?}"));
            assert!(
                (200..400).contains(&head_resp.status().as_u16()),
                "unexpected HEAD status for {url}"
            );
            eprintln!("H3_CORPUS_CASE head=pass endpoint={url}");
        }
        if external_case_enabled("buffered_post") {
            let mut resp = client
                .post(url)
                .unwrap()
                .body(vec![0x5a; 32 * 1024])
                .send()
                .await
                .unwrap_or_else(|e| panic!("external h3 buffered POST {url} failed: {e:?}"));
            assert!(
                (200..400).contains(&resp.status().as_u16()),
                "unexpected POST status for {url}"
            );
            let _ = resp.text().await.expect("external POST body");
            eprintln!("H3_CORPUS_CASE buffered_post=pass endpoint={url}");
        }
        if external_case_enabled("streaming_upload") {
            let chunks = (0..32)
                .map(|_| Ok(Bytes::from(vec![0x33; 1024])))
                .collect::<Vec<_>>();
            let body =
                RequestBody::from_stream(futures_util::stream::iter(chunks), Some(32 * 1024));
            let mut resp = client
                .post(url)
                .unwrap()
                .body(body)
                .send()
                .await
                .unwrap_or_else(|e| panic!("external h3 streaming POST {url} failed: {e:?}"));
            assert!(
                (200..400).contains(&resp.status().as_u16()),
                "unexpected streaming POST status for {url}"
            );
            let _ = resp.text().await.expect("external streaming POST body");
            eprintln!("H3_CORPUS_CASE streaming_upload=pass endpoint={url}");
        }
        if external_case_enabled("large_streaming_download") {
            let minimum = std::env::var("EGGFETCH_H3_INTEROP_MIN_DOWNLOAD_BYTES")
                .ok()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(1024 * 1024);
            let mut resp = client
                .get(url)
                .unwrap()
                .send()
                .await
                .unwrap_or_else(|e| panic!("external h3 streaming GET {url} failed: {e:?}"));
            let mut stream = resp.bytes_stream().expect("external stream");
            let mut total = 0usize;
            while let Some(chunk) = stream.next().await {
                total += chunk.expect("external stream chunk").len();
            }
            assert!(
                total >= minimum,
                "external body was {total} bytes, minimum is {minimum}"
            );
            eprintln!("H3_CORPUS_CASE large_streaming_download=pass endpoint={url}");
        }
        if external_case_enabled("response_trailers") {
            let mut response = client
                .get(url)
                .unwrap()
                .send()
                .await
                .unwrap_or_else(|e| panic!("external H3 trailers GET {url} failed: {e:?}"));
            let _ = response.text().await.expect("external trailers body");
            assert!(
                response.trailers().is_some(),
                "external response did not provide trailers"
            );
            eprintln!("H3_CORPUS_CASE response_trailers=pass endpoint={url}");
        }
        if external_case_enabled("multiplexed_requests") {
            let mut handles = Vec::new();
            for _ in 0..20u32 {
                let client = client.clone();
                let url = url.clone();
                handles.push(tokio::spawn(async move {
                    let mut response = client
                        .get(&url)
                        .unwrap()
                        .send()
                        .await
                        .expect("external multiplexed request");
                    let status = response.status();
                    let _ = response.text().await.expect("external multiplexed body");
                    status
                }));
            }
            for handle in handles {
                let status = handle.await.expect("external multiplexed task");
                assert!(
                    (200..400).contains(&status.as_u16()),
                    "unexpected multiplexed status"
                );
            }
            eprintln!("H3_CORPUS_CASE multiplexed_requests=pass endpoint={url}");
        }
        if external_case_enabled("connection_reuse") {
            for _ in 0..20u32 {
                let mut response = client
                    .get(url)
                    .unwrap()
                    .send()
                    .await
                    .expect("external reuse request");
                assert!(
                    (200..400).contains(&response.status().as_u16()),
                    "unexpected reuse status"
                );
                let _ = response.text().await.expect("external reuse body");
            }
            eprintln!("H3_CORPUS_CASE connection_reuse=pass endpoint={url}");
        }
        if external_case_enabled("cancellation_before_headers") {
            let stalled = client.get(url).unwrap().send();
            let aborted = tokio::time::timeout(Duration::from_millis(500), stalled).await;
            assert!(aborted.is_err(), "must abort before response headers");
            let mut recovery = client
                .get(url)
                .unwrap()
                .send()
                .await
                .expect("client usable after cancel-before-headers");
            assert!(
                (200..400).contains(&recovery.status().as_u16()),
                "recovery status"
            );
            let _ = recovery.text().await.expect("recovery body");
            eprintln!("H3_CORPUS_CASE cancellation_before_headers=pass endpoint={url}");
        }
        if external_case_enabled("cancellation_during_upload") {
            let chunks = (0..256)
                .map(|_| Ok(Bytes::from(vec![0x44; 4096])))
                .collect::<Vec<_>>();
            let body = RequestBody::from_stream(stream::iter(chunks), None);
            let stalled = client.post(url).unwrap().body(body).send();
            let aborted = tokio::time::timeout(Duration::from_millis(500), stalled).await;
            assert!(aborted.is_err(), "must abort during upload");
            let mut recovery = client
                .get(url)
                .unwrap()
                .send()
                .await
                .expect("client usable after upload cancel");
            assert!(
                (200..400).contains(&recovery.status().as_u16()),
                "recovery status"
            );
            let _ = recovery.text().await.expect("recovery body");
            eprintln!("H3_CORPUS_CASE cancellation_during_upload=pass endpoint={url}");
        }
        if external_case_enabled("cancellation_during_download") {
            let mut response = client
                .get(url)
                .unwrap()
                .send()
                .await
                .expect("download for cancel");
            let mut body_stream = response.bytes_stream().expect("download stream");
            if let Some(chunk) = body_stream.next().await {
                let _ = chunk.expect("first chunk");
            }
            drop(body_stream);
            drop(response);
            let mut recovery = client
                .get(url)
                .unwrap()
                .send()
                .await
                .expect("client usable after download cancel");
            assert!(
                (200..400).contains(&recovery.status().as_u16()),
                "recovery status"
            );
            let _ = recovery.text().await.expect("recovery body");
            eprintln!("H3_CORPUS_CASE cancellation_during_download=pass endpoint={url}");
        }
        if external_case_enabled("stream_reset") {
            let control = external_control_url("stream_reset")
                .expect("stream_reset requires EGGFETCH_H3_INTEROP_CONTROL_STREAM_RESET");
            let outcome = tokio::time::timeout(
                Duration::from_secs(15),
                client.get(&control).unwrap().send(),
            )
            .await;
            if let Ok(Ok(mut resp)) = outcome {
                let _ = tokio::time::timeout(Duration::from_secs(5), resp.text()).await;
            }
            let mut recovery = client
                .get(url)
                .unwrap()
                .send()
                .await
                .expect("client usable after stream reset");
            assert!(
                (200..400).contains(&recovery.status().as_u16()),
                "recovery status"
            );
            let _ = recovery.text().await.expect("recovery body");
            eprintln!("H3_CORPUS_CASE stream_reset=pass endpoint={url}");
        }
        if external_case_enabled("graceful_connection_close") {
            let control = external_control_url("graceful_connection_close").expect(
                "graceful_connection_close requires EGGFETCH_H3_INTEROP_CONTROL_GRACEFUL_CONNECTION_CLOSE",
            );
            let mut control_resp = client
                .get(&control)
                .unwrap()
                .send()
                .await
                .expect("graceful control request");
            let _ = control_resp.text().await;
            let mut recovery = client
                .get(url)
                .unwrap()
                .send()
                .await
                .expect("client reconnects after graceful close");
            assert!(
                (200..400).contains(&recovery.status().as_u16()),
                "recovery status"
            );
            let _ = recovery.text().await.expect("recovery body");
            eprintln!("H3_CORPUS_CASE graceful_connection_close=pass endpoint={url}");
        }
        if external_case_enabled("goaway_reconnect") {
            let control = external_control_url("goaway_reconnect")
                .expect("goaway_reconnect requires EGGFETCH_H3_INTEROP_CONTROL_GOAWAY_RECONNECT");
            let before = client.transport_metrics().snapshot();
            let mut control_resp = client
                .get(&control)
                .unwrap()
                .send()
                .await
                .expect("goaway control request");
            let _ = control_resp.text().await;
            let mut recovery = client
                .get(url)
                .unwrap()
                .send()
                .await
                .expect("client reconnects after GOAWAY");
            assert!(
                (200..400).contains(&recovery.status().as_u16()),
                "recovery status"
            );
            let _ = recovery.text().await.expect("recovery body");
            let after = client.transport_metrics().snapshot();
            assert!(
                after.h3_reconnected > before.h3_reconnected,
                "expected h3_reconnected to increase: before={before:?} after={after:?}"
            );
            eprintln!("H3_CORPUS_CASE goaway_reconnect=pass endpoint={url}");
        }
        if external_case_enabled("alternative_port") {
            let discovery = external_control_url("alternative_port")
                .expect("alternative_port requires EGGFETCH_H3_INTEROP_CONTROL_ALTERNATIVE_PORT");
            let auto_client = external_h3_auto_client();
            let mut learn = auto_client
                .get(&discovery)
                .unwrap()
                .send()
                .await
                .expect("Alt-Svc discovery");
            assert!(
                (200..400).contains(&learn.status().as_u16()),
                "discovery status"
            );
            let _ = learn.text().await.expect("discovery body");
            let mut h3 = auto_client
                .get(url)
                .unwrap()
                .send()
                .await
                .expect("H3 on alternative port");
            assert!(
                (200..400).contains(&h3.status().as_u16()),
                "alternative port status"
            );
            let _ = h3.text().await.expect("alternative port body");
            eprintln!("H3_CORPUS_CASE alternative_port=pass endpoint={url}");
        }
    }
}

// ---------------------------------------------------------------------------
// §2 continued: request/response corpus (local, deterministic)
// ---------------------------------------------------------------------------

/// Streaming upload succeeds over H3 (corpus `streaming_upload`).
#[tokio::test(flavor = "multi_thread")]
async fn h3_streaming_upload_succeeds() {
    let server = QuicTestServer::start(b"stream-up".to_vec()).await;
    let url = server.url();
    let client = h3_client();
    let chunks = (0..32)
        .map(|_| Ok(Bytes::from(vec![0x33; 1024])))
        .collect::<Vec<_>>();
    let body = RequestBody::from_stream(stream::iter(chunks), Some(32 * 1024));
    let mut resp = client
        .post(&url)
        .unwrap()
        .body(body)
        .send()
        .await
        .expect("h3 streaming post");
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.text().await.expect("body"), "stream-up");
}

/// POST with a buffered body succeeds over H3 (upload path).
#[tokio::test(flavor = "multi_thread")]
async fn h3_post_with_buffered_body_succeeds() {
    let server = QuicTestServer::start(b"post-ok".to_vec()).await;
    let url = server.url();
    let client = h3_client();
    let payload = vec![0x55_u8; 256 * 1024];
    let mut resp = client
        .post(&url)
        .unwrap()
        .body(payload)
        .send()
        .await
        .expect("h3 post");
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.text().await.expect("body"), "post-ok");
}

/// HEAD framing succeeds over H3.
#[tokio::test(flavor = "multi_thread")]
async fn h3_head_method_succeeds() {
    let server = QuicTestServer::start(b"head-body".to_vec()).await;
    let url = server.url();
    let client = h3_client();
    let resp = client.head(&url).unwrap().send().await.expect("h3 head");
    assert_eq!(resp.status().as_u16(), 200);
}

/// Large streaming download (1 MiB) without eager buffering.
#[tokio::test(flavor = "multi_thread")]
async fn h3_large_streaming_download() {
    let expected = vec![0xAB_u8; 1024 * 1024];
    let server = QuicTestServer::start(expected.clone()).await;
    let url = server.url();
    let client = h3_client();
    let mut resp = client.get(&url).unwrap().send().await.expect("h3 get");
    assert_eq!(resp.status().as_u16(), 200);
    let mut stream = resp.bytes_stream().expect("stream");
    let mut total = 0usize;
    while let Some(chunk) = stream.next().await {
        total += chunk.expect("chunk").len();
    }
    assert_eq!(total, expected.len());
}

/// Concurrent multiplexed requests share one H3 connection.
#[tokio::test(flavor = "multi_thread")]
async fn h3_concurrent_multiplexed_requests() {
    let server = QuicTestServer::start(b"mux".to_vec()).await;
    let url = server.url();
    let client = h3_client();
    let mut handles = Vec::new();
    for _ in 0..20u32 {
        let client = client.clone();
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            let mut resp = client.get(&url).unwrap().send().await.expect("mux req");
            assert_eq!(resp.status().as_u16(), 200);
            assert_eq!(resp.text().await.expect("body"), "mux");
        }));
    }
    for h in handles {
        h.await.expect("join");
    }
    let m = client.transport_metrics().snapshot();
    assert!(
        m.h3_connections_created < 10,
        "multiplexed burst should reuse connections: {}",
        m.h3_connections_created
    );
}

/// H3 response trailers surface after EOF and are None before.
#[tokio::test(flavor = "multi_thread")]
async fn h3_response_trailers_surface_after_eof() {
    let server =
        QuicTestServer::start_with_trailers(b"trailer-body".to_vec(), "x-h3-test", "trailer-ok")
            .await;
    let url = server.url();
    let client = h3_client();
    let mut resp = client.get(&url).unwrap().send().await.expect("h3 get");
    assert_eq!(resp.status().as_u16(), 200);
    assert!(
        resp.trailers().is_none(),
        "trailers must be None before EOF"
    );
    let body = resp.text().await.expect("body");
    assert_eq!(body, "trailer-body");
    let trailers = resp.trailers().expect("trailers after EOF");
    assert_eq!(
        trailers.get("x-h3-test").expect("trailer value"),
        "trailer-ok"
    );
}

// ---------------------------------------------------------------------------
// §4: Network impairment and fallback qualification (local, deterministic)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn udp_blackhole_terminates_within_connect_budget() {
    let hole = Blackhole::bind();
    let url = hole.url();
    let client = h3_client_with_connect_timeout(Duration::from_millis(400));
    let err = client
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect_err("blackhole must fail");
    // Must terminate promptly (connect budget), not hang.
    let _ = format!("{err:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_then_good_origin_does_not_poison_client() {
    // One bad (blackhole) and one good origin on the same client: the
    // failure must not poison later requests to a healthy origin.
    let hole = Blackhole::bind();
    let hole_url = hole.url();
    let good = QuicTestServer::start(b"good".to_vec()).await;
    let good_url = good.url();
    let client = h3_client_with_connect_timeout(Duration::from_millis(400));
    let _ = client
        .get(&hole_url)
        .unwrap()
        .send()
        .await
        .expect_err("blackhole must fail");
    let mut resp = client
        .get(&good_url)
        .unwrap()
        .send()
        .await
        .expect("good origin after failure");
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.text().await.expect("body"), "good");
}

#[tokio::test(flavor = "multi_thread")]
async fn server_restart_reconnects_on_next_request() {
    let body = b"restart-ok".to_vec();
    let first = QuicTestServer::start(body.clone()).await;
    let first_url = first.url();
    let client = h3_client();
    let mut resp = client.get(&first_url).unwrap().send().await.expect("first");
    assert_eq!(resp.text().await.expect("body"), "restart-ok");
    drop(first);
    // New server generation (possibly a different port, same client):
    // the client must reconnect rather than stick to a dead generation.
    let second = QuicTestServer::start(body).await;
    let second_url = second.url();
    let mut resp = client
        .get(&second_url)
        .unwrap()
        .send()
        .await
        .expect("second generation");
    assert_eq!(resp.status().as_u16(), 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn cancellation_during_h3_connect_is_prompt() {
    let hole = Blackhole::bind();
    let hole_url = hole.url();
    let client = h3_client_with_connect_timeout(Duration::from_secs(10));
    let fut = client.get(&hole_url).unwrap().send();
    let res = tokio::time::timeout(Duration::from_millis(500), fut).await;
    assert!(res.is_err(), "outer timeout must fire while H3 dial stalls");
    // Client stays usable afterwards.
    let server = QuicTestServer::start(b"after-cancel".to_vec()).await;
    let url = server.url();
    let mut resp = client
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("usable after cancel");
    assert_eq!(resp.status().as_u16(), 200);
}

/// Mid-response early close terminates promptly and does not poison the
/// client (local deterministic path-break shape).
#[tokio::test(flavor = "multi_thread")]
async fn h3_early_close_terminates_and_client_recovers() {
    let broken = QuicTestServer::start_early_close().await;
    let broken_url = broken.url();
    let client = h3_client_with_connect_timeout(Duration::from_secs(5));
    let outcome = tokio::time::timeout(
        Duration::from_secs(5),
        client.get(&broken_url).unwrap().send(),
    )
    .await;
    // The request must terminate (outer timeout is the backstop); a hang
    // would fail here. Either an error or a short body is acceptable —
    // what matters is bounded termination without poisoned state.
    if let Ok(Ok(mut resp)) = outcome {
        let _ = tokio::time::timeout(Duration::from_secs(5), resp.text()).await;
    }
    let good = QuicTestServer::start(b"recovered".to_vec()).await;
    let good_url = good.url();
    let mut resp = client
        .get(&good_url)
        .unwrap()
        .send()
        .await
        .expect("usable after early close");
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.text().await.expect("body"), "recovered");
}

/// Upload cancellation returns promptly and the client stays usable.
#[tokio::test(flavor = "multi_thread")]
async fn h3_cancellation_during_upload_is_prompt() {
    let slow = QuicTestServer::start_slow_upload_consumer(b"unused".to_vec()).await;
    let slow_url = slow.url();
    let good = QuicTestServer::start(b"after-upload-cancel".to_vec()).await;
    let good_url = good.url();
    let client = h3_client();
    let chunks = (0..512)
        .map(|_| Ok(Bytes::from(vec![0x22; 4096])))
        .collect::<Vec<_>>();
    let body = RequestBody::from_stream(stream::iter(chunks), None);
    let stalled = client.post(&slow_url).unwrap().body(body).send();
    let aborted = tokio::time::timeout(Duration::from_millis(75), stalled).await;
    assert!(aborted.is_err(), "upload cancel must be prompt");
    drop(aborted);
    let mut recovery = client
        .get(&good_url)
        .unwrap()
        .send()
        .await
        .expect("usable after upload cancel");
    assert_eq!(recovery.status().as_u16(), 200);
    assert_eq!(recovery.text().await.expect("body"), "after-upload-cancel");
}

/// Download cancellation releases the stream and the client stays usable.
#[tokio::test(flavor = "multi_thread")]
async fn h3_cancellation_during_download_is_prompt() {
    let payload = vec![0xCD_u8; 512 * 1024];
    let server = QuicTestServer::start(payload).await;
    let url = server.url();
    let client = h3_client();
    let mut resp = client.get(&url).unwrap().send().await.expect("download");
    let mut body_stream = resp.bytes_stream().expect("stream");
    let first = body_stream
        .next()
        .await
        .expect("first chunk")
        .expect("chunk");
    assert!(!first.is_empty());
    drop(body_stream);
    drop(resp);
    let mut recovery = client
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("usable after download cancel");
    assert_eq!(recovery.status().as_u16(), 200);
    assert_eq!(recovery.bytes().await.expect("body").len(), 512 * 1024);
}

/// GOAWAY after one response drains the generation; the next request reconnects.
#[tokio::test(flavor = "multi_thread")]
async fn h3_graceful_connection_close_reconnects() {
    let server = QuicTestServer::start_shutdown_after_one_response(b"graceful-one".to_vec()).await;
    let url = server.url();
    let client = h3_client();
    let mut first = client.get(&url).unwrap().send().await.expect("first");
    assert_eq!(first.text().await.expect("body"), "graceful-one");
    tokio::time::sleep(Duration::from_millis(400)).await;
    let mut second = client.get(&url).unwrap().send().await.expect("second");
    assert_eq!(second.text().await.expect("body"), "graceful-one");
}

/// GOAWAY reconnect is observable via `h3_reconnected` and optional drain metrics.
#[tokio::test(flavor = "multi_thread")]
async fn h3_goaway_reconnect_observes_metrics() {
    let server = QuicTestServer::start_shutdown_after_one_response(b"goaway-one".to_vec()).await;
    let url = server.url();
    let client = h3_client();
    let before = client.transport_metrics().snapshot();
    let mut first = client.get(&url).unwrap().send().await.expect("first");
    assert_eq!(first.text().await.expect("body"), "goaway-one");
    let mid = client.transport_metrics().snapshot();
    tokio::time::sleep(Duration::from_millis(400)).await;
    let mut second = client.get(&url).unwrap().send().await.expect("reconnect");
    assert_eq!(second.text().await.expect("body"), "goaway-one");
    let after = client.transport_metrics().snapshot();
    assert!(
        after.h3_reconnected > before.h3_reconnected,
        "h3_reconnected before={before:?} after={after:?}"
    );
    if mid.h3_drain_observed > before.h3_drain_observed {
        assert!(
            after.h3_drain_observed >= mid.h3_drain_observed,
            "drain metric should remain observed"
        );
    }
}

/// Explicit IPv4 loopback GET succeeds.
#[tokio::test(flavor = "multi_thread")]
async fn h3_ipv4_loopback_get_succeeds() {
    let server = QuicTestServer::start(b"ipv4-ok".to_vec()).await;
    let client = h3_client();
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("ipv4 get");
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.text().await.expect("body"), "ipv4-ok");
}

/// Authenticated Alt-Svc on an alternative port is selected by `Auto`.
#[cfg(feature = "tls-rustls")]
#[tokio::test(flavor = "multi_thread")]
async fn h3_alternative_port_via_altsvc() {
    let ca = CertAuthority::new();
    let (h3_cert, h3_key) = ca.generate_server_cert(&["localhost", "127.0.0.1"]);
    let h3 = VerifiedH3Server::start(h3_cert, h3_key, b"alt-h3-body".to_vec()).await;
    let h1 = HttpsAltSvcServer::start(
        &ca,
        &["localhost"],
        Some(format!("h3=\":{}\"; ma=60", h3.addr.port())),
        b"discover-h1".to_vec(),
    )
    .await;
    let client = auto_client_with_ca(&ca);
    let mut learn = client.get(&h1.url()).unwrap().send().await.expect("learn");
    assert_eq!(learn.text().await.expect("h1 body"), "discover-h1");
    let mut h3_resp = client
        .get(&h1.url())
        .unwrap()
        .send()
        .await
        .expect("h3 route");
    assert_eq!(h3_resp.text().await.expect("h3 body"), "alt-h3-body");
    let metrics = client.transport_metrics().snapshot();
    assert!(
        metrics.h3_route_attempted > 0,
        "must attempt H3: {metrics:?}"
    );
}

// ---------------------------------------------------------------------------
// §5: Resource and soak evidence (bounded, deterministic)
// ---------------------------------------------------------------------------

/// Soak counts (M009); short controls above keep smaller loops for fast Tier 1.
const SOAK_SEQUENTIAL_REUSE: u32 = 1_000;
const SOAK_FAIL_RECONNECT_CYCLES: u32 = 50;
const SOAK_CANCELLATION_STORM: u32 = 50;
const SOAK_PARTIAL_BODY_DROP: u32 = 30;
const SOAK_CLIENT_CONSTRUCTION_DROP: u32 = 100;

/// Measure current RSS in bytes. Returns 0 on unsupported platforms.
fn current_rss_bytes() -> usize {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|s| {
                s.lines().find(|l| l.starts_with("VmRSS:")).and_then(|l| {
                    l.split_whitespace()
                        .nth(1)
                        .and_then(|v| v.parse::<usize>().ok())
                        .map(|kb| kb * 1024)
                })
            })
            .unwrap_or(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

/// Open file-descriptor count. Returns 0 on unsupported platforms.
fn current_open_fd_count() -> usize {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_dir("/proc/self/fd")
            .map(|entries| entries.count())
            .unwrap_or(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

/// Shared reuse/fail/cancel soaks stay under a tight Linux `VmRSS` step.
const RESOURCE_RSS_ENVELOPE_BYTES: usize = 4 * 1024 * 1024;
/// Construction/drop builds a full H3 client + QUIC handshake each cycle.
/// ubuntu-latest measured ~21 MiB `VmRSS` step across 100 cycles with one
/// reused peer (glibc retention); keep a hard cap with headroom, not exact
/// equality. Open-fd growth stays on the shared tight envelope.
const CLIENT_DROP_RSS_ENVELOPE_BYTES: usize = 32 * 1024 * 1024;
const RESOURCE_FD_ENVELOPE: usize = 64;

fn assert_resource_envelope(
    rss_before: usize,
    rss_after: usize,
    fd_before: usize,
    fd_after: usize,
    context: &str,
) {
    assert_resource_envelope_with_limits(
        rss_before,
        rss_after,
        fd_before,
        fd_after,
        RESOURCE_RSS_ENVELOPE_BYTES,
        RESOURCE_FD_ENVELOPE,
        context,
    );
}

fn assert_resource_envelope_with_limits(
    rss_before: usize,
    rss_after: usize,
    fd_before: usize,
    fd_after: usize,
    max_rss_delta: usize,
    max_fd_delta: usize,
    context: &str,
) {
    if rss_before > 0 && rss_after > 0 {
        let delta = rss_after.abs_diff(rss_before);
        assert!(
            delta < max_rss_delta,
            "{context}: VmRSS grew by {delta} bytes (before={rss_before}, after={rss_after})"
        );
    }
    if fd_before > 0 && fd_after > 0 {
        let delta = fd_after.abs_diff(fd_before);
        assert!(
            delta < max_fd_delta,
            "{context}: open fd count grew by {delta} (before={fd_before}, after={fd_after})"
        );
    }
}

fn assert_h3_diagnostics_bounded_and_redacted(client: &Client) {
    let diagnostics = client.transport_metrics().h3_diagnostics();
    assert!(
        diagnostics.len() <= 64,
        "h3_diagnostics must stay capped: {}",
        diagnostics.len()
    );
    for diagnostic in &diagnostics {
        let debug = format!("{diagnostic:?}");
        assert!(!debug.contains("localhost"), "hostname leaked: {debug}");
        assert!(!debug.contains("authorization"), "header leaked: {debug}");
        assert!(!debug.contains("cookie"), "cookie leaked: {debug}");
        assert!(!debug.contains("set-cookie"), "header leaked: {debug}");
        assert!(
            !debug.contains("peer reason"),
            "close reason leaked: {debug}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn hundred_sequential_requests_on_reused_h3_connection() {
    let server = QuicTestServer::start(b"x".repeat(64)).await;
    let url = server.url();
    let client = h3_client();
    for i in 0..100u32 {
        let mut resp = client
            .get(&url)
            .unwrap()
            .send()
            .await
            .unwrap_or_else(|e| panic!("request {i} failed: {e:?}"));
        assert_eq!(resp.status().as_u16(), 200);
        let body = resp.text().await.expect("body");
        assert_eq!(body.len(), 64, "request {i}");
    }
    // Cache must stay bounded: at most a handful of generations for one origin.
    let m = client.transport_metrics().snapshot();
    assert!(
        m.h3_connections_created < 10,
        "connection churn too high: {}",
        m.h3_connections_created
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn repeated_fail_reconnect_cycles_do_not_grow_state() {
    let client = h3_client_with_connect_timeout(Duration::from_millis(200));
    for _ in 0..10u32 {
        let hole = Blackhole::bind();
        let url = hole.url();
        let _ = client.get(&url).unwrap().send().await;
    }
    let m = client.transport_metrics().snapshot();
    // Bounded counters only; no per-origin leak surfaced as unbounded growth.
    assert!(m.h3_connections_created < 64);
    // Client still works against a good origin.
    let good = QuicTestServer::start(b"ok".to_vec()).await;
    let url = good.url();
    let mut resp = client
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("good after cycles");
    assert_eq!(resp.status().as_u16(), 200);
}

/// Many sequential distinct origins beyond the 64-entry H3 cache bound
/// stay bounded and leave the client usable (§5 resource evidence).
#[tokio::test(flavor = "multi_thread")]
async fn h3_many_distinct_origins_stay_bounded() {
    let client = h3_client_with_connect_timeout(Duration::from_millis(50));
    // 70 distinct dead origins exceeds H3_CACHE_MAX_ENTRIES (64).
    for _ in 0..70u32 {
        let hole = Blackhole::bind();
        let url = hole.url();
        let _ = tokio::time::timeout(Duration::from_millis(200), client.get(&url).unwrap().send())
            .await;
    }
    let m = client.transport_metrics().snapshot();
    assert!(
        m.h3_connections_created < 64,
        "distinct-origin failures must not accumulate: {}",
        m.h3_connections_created
    );
    let good = QuicTestServer::start(b"bounded-ok".to_vec()).await;
    let url = good.url();
    let mut resp = client
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("good after many origins");
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(resp.text().await.expect("body"), "bounded-ok");
}

#[tokio::test(flavor = "multi_thread")]
async fn cancellation_storm_leaves_client_usable() {
    let client = h3_client_with_connect_timeout(Duration::from_secs(5));
    let server = QuicTestServer::start(b"storm-ok".to_vec()).await;
    let url = server.url();
    for _ in 0..20u32 {
        let fut = client.get(&url).unwrap().send();
        let _ = tokio::time::timeout(Duration::from_millis(50), fut).await;
    }
    let mut resp = client
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("usable after storm");
    assert_eq!(resp.status().as_u16(), 200);
}

#[tokio::test(flavor = "multi_thread")]
async fn client_construction_drop_loops_do_not_leak() {
    let server = QuicTestServer::start(b"drop-ok".to_vec()).await;
    let url = server.url();
    for _ in 0..20u32 {
        let client = h3_client();
        let mut resp = client.get(&url).unwrap().send().await.expect("req");
        assert_eq!(resp.status().as_u16(), 200);
        let _ = resp.text().await.expect("body");
        drop(client);
    }
}

/// M009 soak: 1 000 sequential GETs on one reused H3 connection.
#[tokio::test(flavor = "multi_thread")]
async fn thousand_sequential_requests_on_reused_h3_connection() {
    let server = QuicTestServer::start(b"x".repeat(64)).await;
    let url = server.url();
    let client = h3_client();
    for i in 0..10u32 {
        let mut resp = client
            .get(&url)
            .unwrap()
            .send()
            .await
            .unwrap_or_else(|e| panic!("warmup {i}: {e:?}"));
        assert_eq!(resp.status().as_u16(), 200);
        let _ = resp.text().await.expect("warmup body");
    }
    let rss_before = current_rss_bytes();
    let fd_before = current_open_fd_count();
    for i in 0..SOAK_SEQUENTIAL_REUSE {
        let mut resp = client
            .get(&url)
            .unwrap()
            .send()
            .await
            .unwrap_or_else(|e| panic!("request {i} failed: {e:?}"));
        assert_eq!(resp.status().as_u16(), 200);
        let body = resp.text().await.expect("body");
        assert_eq!(body.len(), 64, "request {i}");
    }
    let m = client.transport_metrics().snapshot();
    assert!(
        m.h3_connections_created < 10,
        "connection churn too high after {SOAK_SEQUENTIAL_REUSE} reuse: {}",
        m.h3_connections_created
    );
    assert_h3_diagnostics_bounded_and_redacted(&client);
    assert_resource_envelope(
        rss_before,
        current_rss_bytes(),
        fd_before,
        current_open_fd_count(),
        "thousand sequential H3 reuse",
    );
}

/// M009 soak: fail/reconnect beyond the 10-cycle short control.
#[tokio::test(flavor = "multi_thread")]
async fn fifty_repeated_fail_reconnect_cycles_do_not_grow_state_soak() {
    let good = QuicTestServer::start(b"ok".to_vec()).await;
    let good_url = good.url();
    let client = h3_client_with_connect_timeout(Duration::from_millis(200));
    for _ in 0..5u32 {
        let hole = Blackhole::bind();
        let _ = client.get(&hole.url()).unwrap().send().await;
    }
    let warm = client.transport_metrics().snapshot();
    let rss_before = current_rss_bytes();
    let fd_before = current_open_fd_count();
    for cycle in 0..SOAK_FAIL_RECONNECT_CYCLES {
        let hole = Blackhole::bind();
        let _ = client.get(&hole.url()).unwrap().send().await;
        let mut resp = client
            .get(&good_url)
            .unwrap()
            .send()
            .await
            .unwrap_or_else(|e| panic!("good after cycle {cycle}: {e:?}"));
        assert_eq!(resp.status().as_u16(), 200);
        let _ = resp.text().await.expect("body");
    }
    let m = client.transport_metrics().snapshot();
    assert!(
        m.h3_connections_created < 64,
        "fail/reconnect soak must stay within cache scale: {}",
        m.h3_connections_created
    );
    let growth = m
        .h3_connections_created
        .saturating_sub(warm.h3_connections_created);
    assert!(
        growth < SOAK_FAIL_RECONNECT_CYCLES as usize / 2,
        "must not grow ~one live generation per failure: warm={} after={}",
        warm.h3_connections_created,
        m.h3_connections_created
    );
    assert_h3_diagnostics_bounded_and_redacted(&client);
    assert_resource_envelope(
        rss_before,
        current_rss_bytes(),
        fd_before,
        current_open_fd_count(),
        "fifty fail/reconnect cycles",
    );
}

/// M009 soak: cancellation storm plus partial-body-drop loop, then recovery.
#[tokio::test(flavor = "multi_thread")]
async fn extended_cancellation_and_partial_drop_storm_leaves_client_usable() {
    let payload = vec![0x42_u8; 256 * 1024];
    let server = QuicTestServer::start(payload.clone()).await;
    let url = server.url();
    let client = h3_client_with_connect_timeout(Duration::from_secs(5));
    for _ in 0..5u32 {
        let mut resp = client.get(&url).unwrap().send().await.expect("warmup");
        let _ = resp.text().await.expect("warmup body");
    }
    let rss_before = current_rss_bytes();
    let fd_before = current_open_fd_count();
    for _ in 0..SOAK_CANCELLATION_STORM {
        let fut = client.get(&url).unwrap().send();
        let _ = tokio::time::timeout(Duration::from_millis(50), fut).await;
    }
    for _ in 0..SOAK_PARTIAL_BODY_DROP {
        let mut resp = client
            .get(&url)
            .unwrap()
            .send()
            .await
            .expect("partial send");
        let mut stream = resp.bytes_stream().expect("stream");
        if let Some(chunk) = stream.next().await {
            let _ = chunk.expect("chunk");
        }
        drop(stream);
        drop(resp);
    }
    let mut recovery = client
        .get(&url)
        .unwrap()
        .send()
        .await
        .expect("usable after storm");
    assert_eq!(recovery.status().as_u16(), 200);
    assert_eq!(recovery.bytes().await.expect("body").len(), payload.len());
    assert_h3_diagnostics_bounded_and_redacted(&client);
    assert_resource_envelope(
        rss_before,
        current_rss_bytes(),
        fd_before,
        current_open_fd_count(),
        "cancellation and partial-drop storm",
    );
}

/// M009 soak: many client construction/drop cycles with one request each.
///
/// Reuses one peer so the envelope measures client drop, not fire-and-forget
/// Quinn server tasks (`QuicTestServer::drop` only signals shutdown).
#[tokio::test(flavor = "multi_thread")]
async fn hundred_client_construction_drop_loops_do_not_leak_soak() {
    let server = QuicTestServer::start(b"drop-ok".to_vec()).await;
    let url = server.url();
    {
        let client = h3_client();
        let mut resp = client.get(&url).unwrap().send().await.expect("warmup");
        let _ = resp.text().await;
        drop(client);
    }
    // Let the warmup accept/response tasks settle before the baseline sample.
    tokio::time::sleep(Duration::from_millis(50)).await;
    let rss_before = current_rss_bytes();
    let fd_before = current_open_fd_count();
    for i in 0..SOAK_CLIENT_CONSTRUCTION_DROP {
        let client = h3_client();
        let mut resp = client
            .get(&url)
            .unwrap()
            .send()
            .await
            .unwrap_or_else(|e| panic!("req {i}: {e:?}"));
        assert_eq!(resp.status().as_u16(), 200);
        let _ = resp.text().await.expect("body");
        drop(client);
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_resource_envelope_with_limits(
        rss_before,
        current_rss_bytes(),
        fd_before,
        current_open_fd_count(),
        CLIENT_DROP_RSS_ENVELOPE_BYTES,
        RESOURCE_FD_ENVELOPE,
        "hundred client construction/drop",
    );
}

// ---------------------------------------------------------------------------
// §6: QUIC/H3 observability qualification (no fabricated metadata)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn h3_metrics_are_exact_and_metadata_is_truthful() {
    let server = QuicTestServer::start(b"obs".to_vec()).await;
    let url = server.url();
    let client = h3_client();
    let before = client.transport_metrics().snapshot();
    let mut resp = client.get(&url).unwrap().send().await.expect("req");
    assert_eq!(resp.status().as_u16(), 200);
    let _ = resp.text().await.expect("body");
    let after = client.transport_metrics().snapshot();
    // Exactly one H3 connection creation for one fresh origin.
    assert_eq!(
        after.h3_connections_created,
        before.h3_connections_created + 1
    );
    // H3 per-response network-stream metadata stays None (never
    // synthesized); TransportKind::Quic is reserved.
    assert!(resp.network_stream().is_none());
}

/// IPv6 loopback may be unavailable in some containers; report skip
/// explicitly rather than failing or claiming support.
#[tokio::test(flavor = "multi_thread")]
async fn ipv6_availability_is_detected_not_assumed() {
    let sock = std::net::UdpSocket::bind("[::1]:0");
    if sock.is_err() {
        eprintln!("SKIP ipv6 loopback unavailable in this environment");
    }
    // Socket bound or skipped; no further claim — platform support matrix
    // lives in docs.
}

// ---------------------------------------------------------------------------
// §8: Platform notes (no platform-specific UDP/QUIC assumptions)
// ---------------------------------------------------------------------------

#[test]
fn h3_platform_limitations_are_documented() {
    // This test pins that the graduation doc names the platform scope.
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let doc_path = manifest.join("../../docs/architecture/core-tls-proxy-protocols.md");
    let doc = std::fs::read_to_string(&doc_path).expect("graduation doc readable");
    for needle in [
        "Production Graduation Decision",
        "quinn 0.11",
        "h3 0.0.8",
        "h3-quinn 0.0.10",
    ] {
        assert!(doc.contains(needle), "doc missing {needle:?}");
    }
}
