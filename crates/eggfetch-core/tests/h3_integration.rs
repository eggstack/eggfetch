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
#![cfg(feature = "http3")]
#![allow(clippy::unnested_or_patterns)]

//! Real HTTP/3 integration tests.
//!
//! These tests start a local QUIC server and complete actual HTTP/3
//! request/response cycles, verifying the full Quinn + h3 stack.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use eggfetch_core::{Client, Error, HttpVersionPolicy, TlsConfig};
use futures_util::StreamExt;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::sync::watch;

// ---------------------------------------------------------------------------
// QUIC test server
// ---------------------------------------------------------------------------

/// A minimal QUIC test server backed by Quinn + h3.
///
/// The server generates a self-signed certificate, binds to a random
/// local port, and serves HTTP/3 requests in a background tokio task.
/// Dropping the server shuts down the accept loop.
struct QuicTestServer {
    addr: SocketAddr,
    shutdown: watch::Sender<bool>,
}

impl QuicTestServer {
    /// Start a server that responds to every request with `body`.
    #[allow(clippy::unused_async)]
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
        .expect("TLS version config")
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .expect("server TLS cert");
        server_tls.alpn_protocols = vec![b"h3".to_vec()];
        server_tls.max_early_data_size = u32::MAX;

        let quic_crypto = quinn::crypto::rustls::QuicServerConfig::try_from(server_tls)
            .expect("QUIC server config conversion");

        let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_crypto));

        let mut transport = quinn::TransportConfig::default();
        transport.max_concurrent_bidi_streams(100u32.into());
        transport.max_concurrent_uni_streams(100u32.into());
        server_config.transport_config(Arc::new(transport));

        let endpoint = quinn::Endpoint::server(server_config, "127.0.0.1:0".parse().unwrap())
            .expect("bind QUIC endpoint");
        let addr = endpoint.local_addr().expect("local addr");

        let body = Bytes::from(body);
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    incoming = endpoint.accept() => {
                        let Some(incoming) = incoming else { break };
                        let body = body.clone();
                        tokio::spawn(async move {
                            let conn = match incoming.await {
                                Ok(c) => c,
                                Err(_) => return,
                            };
                            let h3_conn = h3_quinn::Connection::new(conn);
                            let mut server_conn =
                                match h3::server::Connection::<_, Bytes>::new(h3_conn).await {
                                    Ok(c) => c,
                                    Err(_) => return,
                                };
                            // Accept requests in a loop to handle multiple
                            // h3 connections on the same QUIC connection.
                            loop {
                                let resolver = match server_conn.accept().await {
                                    Ok(Some(r)) => r,
                                    _ => break,
                                };
                                let body = body.clone();
                                let (_req, mut stream) =
                                    match resolver.resolve_request().await {
                                        Ok(rs) => rs,
                                        Err(_) => continue,
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
                                let _ = stream.send_data(body).await;
                                let _ = stream.finish().await;
                            }
                        });
                    }
                    _ = shutdown_rx.changed() => {
                        break;
                    }
                }
            }
        });

        Self {
            addr,
            shutdown: shutdown_tx,
        }
    }

    fn url(&self) -> String {
        format!("https://127.0.0.1:{}/", self.addr.port())
    }
}

impl Drop for QuicTestServer {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

fn build_h3_client() -> Client {
    let tls_config = TlsConfig::builder()
        .danger_accept_invalid_certs(true)
        .build();
    Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .tls_config(tls_config)
        .automatic_decompression(false)
        .build()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn h3_simple_get_request() {
    let server = QuicTestServer::start(b"hello h3".to_vec()).await;
    let client = build_h3_client();
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.expect("text");
    assert_eq!(body, "hello h3");
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_large_response_body() {
    let payload = vec![0xAB_u8; 1_048_576]; // 1 MB
    let server = QuicTestServer::start(payload.clone()).await;
    let client = build_h3_client();
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.bytes().await.expect("bytes");
    assert_eq!(body.len(), 1_048_576);
    assert_eq!(&body[..], &payload[..]);
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_request_headers() {
    let server = QuicTestServer::start(b"ok".to_vec()).await;
    let client = build_h3_client();
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .header("x-custom", "test-value")
        .header("user-agent", "eggfetch-test")
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.expect("text");
    assert_eq!(body, "ok");
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_post_with_body() {
    let server = QuicTestServer::start(b"received".to_vec()).await;
    let client = build_h3_client();
    let mut resp = client
        .post(&server.url())
        .unwrap()
        .body("request body data")
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.expect("text");
    assert_eq!(body, "received");
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_concurrent_streams() {
    let server = QuicTestServer::start(b"ok".to_vec()).await;

    let mut handles = Vec::new();
    for i in 0..5 {
        let url = server.url();
        handles.push(tokio::spawn(async move {
            let client = build_h3_client();
            let mut resp = client
                .get(&url)
                .unwrap()
                .header("x-stream-id", &i.to_string())
                .send()
                .await
                .expect("send");
            assert_eq!(resp.status().as_u16(), 200);
            let body = resp.text().await.expect("text");
            assert_eq!(body, "ok");
        }));
    }
    for h in handles {
        h.await.expect("task join");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_connection_reuse() {
    let server = QuicTestServer::start(b"reused".to_vec()).await;
    let client = build_h3_client();

    for _ in 0..3 {
        let mut resp = client
            .get(&server.url())
            .unwrap()
            .send()
            .await
            .expect("send");
        assert_eq!(resp.status().as_u16(), 200);
        let body = resp.text().await.expect("text");
        assert_eq!(body, "reused");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_streaming_response() {
    let payload = vec![0x42_u8; 256 * 1024]; // 256 KB
    let server = QuicTestServer::start(payload.clone()).await;
    let client = build_h3_client();
    let mut resp = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect("send");
    assert_eq!(resp.status().as_u16(), 200);

    let mut stream = resp.bytes_stream().expect("bytes_stream");
    let mut collected = Vec::new();
    while let Some(chunk) = stream.next().await {
        collected.extend_from_slice(&chunk.expect("chunk"));
    }
    assert_eq!(collected.len(), payload.len());
    assert_eq!(collected, payload);
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_error_on_no_server() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let client = build_h3_client();
    let url = format!("https://127.0.0.1:{port}/");
    let result = client.get(&url).unwrap().send().await;

    match result {
        Err(Error::H3Connect(_))
        | Err(Error::Connect(_))
        | Err(Error::H3Protocol(_))
        | Err(Error::H3ConnectionClosed(_))
        | Err(Error::H3Stream(_))
        | Err(Error::Unsupported(_)) => {}
        other => {
            panic!(
                "expected H3/Connect/Unsupported error for request to non-existent QUIC server, got: {other:?}"
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn h3_different_paths() {
    let server = QuicTestServer::start(b"path-response".to_vec()).await;
    let client = build_h3_client();

    let paths = ["/", "/foo", "/bar/baz", "/api/v1/test"];
    for path in paths {
        let url = format!("{}{path}", server.url());
        let mut resp = client.get(&url).unwrap().send().await.expect("send");
        assert_eq!(resp.status().as_u16(), 200);
        let body = resp.text().await.expect("text");
        assert_eq!(body, "path-response");
    }
}
