#![allow(
    missing_docs,
    dead_code,
    clippy::missing_panics_doc,
    clippy::redundant_closure_for_method_calls
)]
//! Strict HTTPS-downgrade redirect policy integration tests.
//!
//! Proves the transport policy end to end with local servers rather than
//! string-matching error text:
//! - allowed redirect chains complete under strict mode;
//! - an HTTPS -> HTTP downgrade is rejected before the downgraded
//!   destination receives any request bytes (its hit counter stays zero);
//! - compat (Allow) behavior is unchanged.

mod test_server;
mod tls_fixtures;

use std::sync::Arc;

use eggfetch_core::{Client, Error, RedirectDowngradePolicy, RedirectPolicy};
use test_server::{TestServer, TestServerConfig};
use tls_fixtures::CertAuthority;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::watch;

/// A minimal HTTPS server that always answers with a redirect to `location`.
struct HttpsRedirectServer {
    port: u16,
    shutdown_tx: watch::Sender<bool>,
}

impl HttpsRedirectServer {
    async fn start(ca: &CertAuthority, hostnames: &[&str], location: String) -> Self {
        let (cert_der, key_der) = ca.generate_server_cert(hostnames);
        let server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let Ok((tcp_stream, _)) = result else { break };
                        let acceptor = acceptor.clone();
                        let location = location.clone();
                        tokio::spawn(async move {
                            let Ok(tls_stream) = acceptor.accept(tcp_stream).await else {
                                return;
                            };
                            let mut reader = BufReader::new(tls_stream);
                            // Read request line + headers (discard).
                            let mut line = String::new();
                            if reader.read_line(&mut line).await.is_err() {
                                return;
                            }
                            loop {
                                let mut header = String::new();
                                match reader.read_line(&mut header).await {
                                    Ok(0) | Err(_) => return,
                                    Ok(_) if header.trim().is_empty() => break,
                                    Ok(_) => {}
                                }
                            }
                            let response = format!(
                                "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            );
                            let stream = reader.get_mut();
                            let _ = stream.write_all(response.as_bytes()).await;
                            let _ = stream.flush().await;
                        });
                    }
                    _ = shutdown_rx.changed() => break,
                }
            }
        });

        Self { port, shutdown_tx }
    }

    fn url(&self) -> String {
        format!("https://127.0.0.1:{}/", self.port)
    }
}

impl Drop for HttpsRedirectServer {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(true);
    }
}

fn strict_client(ca: &CertAuthority) -> Client {
    Client::builder()
        .tls_config(
            eggfetch_core::TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .build(),
        )
        .redirect_policy(RedirectPolicy::strict(5))
        .build()
}

/// Allowed HTTP -> HTTP chain completes under strict mode (Deny only
/// affects HTTPS -> HTTP).
#[tokio::test]
async fn strict_policy_allows_http_chain() {
    let final_server = TestServer::start(&TestServerConfig {
        response_body: Some(b"strict-ok".to_vec()),
        close_connection: true,
        ..Default::default()
    });
    let final_port = final_server.port();

    let redirect_server = TestServer::start(&TestServerConfig {
        redirect: Some((302, format!("http://127.0.0.1:{final_port}/"))),
        close_connection: true,
        ..Default::default()
    });

    let client = Client::builder()
        .redirect_policy(RedirectPolicy::strict(5))
        .build();

    let mut resp = client
        .get(&redirect_server.url())
        .unwrap()
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.as_ref(), b"strict-ok");
    assert_eq!(resp.history().len(), 1);
}

/// HTTPS -> HTTP downgrade is rejected and the downgrade target sees zero
/// requests, proving rejection happens before second-hop I/O.
#[tokio::test]
async fn strict_policy_rejects_downgrade_before_second_hop_io() {
    let downgrade_target = TestServer::start(&TestServerConfig {
        response_body: Some(b"must never be served".to_vec()),
        close_connection: true,
        ..Default::default()
    });
    let target_port = downgrade_target.port();
    let target_hits_before = downgrade_target.requests_served();

    let ca = CertAuthority::new();
    let origin = HttpsRedirectServer::start(
        &ca,
        &["127.0.0.1"],
        format!("http://127.0.0.1:{target_port}/stolen"),
    )
    .await;

    let client = strict_client(&ca);
    let err = client.get(&origin.url()).unwrap().send().await.unwrap_err();

    assert!(
        matches!(err, Error::InvalidRedirectLocation(_)),
        "expected downgrade rejection, got: {err:?}"
    );
    assert_eq!(err.kind(), "invalid_redirect_location");
    // No request bytes reached the downgraded destination.
    assert_eq!(
        downgrade_target.requests_served(),
        target_hits_before,
        "downgrade target must see zero requests"
    );
}

/// Same downgrade succeeds under the compat Allow policy, proving the
/// rejection above comes from the strict policy rather than a generic
/// redirect failure.
#[tokio::test]
async fn compat_policy_still_follows_downgrade() {
    let downgrade_target = TestServer::start(&TestServerConfig {
        response_body: Some(b"downgraded-body".to_vec()),
        close_connection: true,
        ..Default::default()
    });
    let target_port = downgrade_target.port();

    let ca = CertAuthority::new();
    let origin = HttpsRedirectServer::start(
        &ca,
        &["127.0.0.1"],
        format!("http://127.0.0.1:{target_port}/plain"),
    )
    .await;

    let client = Client::builder()
        .tls_config(
            eggfetch_core::TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .build(),
        )
        .redirect_policy(RedirectPolicy::new(true, 5))
        .build();

    let mut resp = client.get(&origin.url()).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.bytes().await.unwrap();
    assert_eq!(body.as_ref(), b"downgraded-body");
    assert!(
        downgrade_target.requests_served() >= 1,
        "compat policy must dispatch the second hop"
    );
}

/// The downgrade rule is visible at the builder level without rebuilding
/// the whole policy.
#[tokio::test]
async fn builder_downgrade_setter_selects_strict() {
    let downgrade_target = TestServer::start(&TestServerConfig {
        response_body: Some(b"unreached".to_vec()),
        close_connection: true,
        ..Default::default()
    });
    let target_port = downgrade_target.port();

    let ca = CertAuthority::new();
    let origin = HttpsRedirectServer::start(
        &ca,
        &["127.0.0.1"],
        format!("http://127.0.0.1:{target_port}/x"),
    )
    .await;
    // Select strict downgrade handling via the builder setter rather than
    // constructing a full RedirectPolicy.
    let probe = Client::builder()
        .tls_config(
            eggfetch_core::TlsConfig::builder()
                .ca_certificate_pem(&ca.cert_pem())
                .unwrap()
                .build(),
        )
        .follow_redirects(true)
        .max_redirects(5)
        .redirect_downgrade_policy(RedirectDowngradePolicy::Deny)
        .build();
    let err = probe.get(&origin.url()).unwrap().send().await.unwrap_err();
    assert!(matches!(err, Error::InvalidRedirectLocation(_)));
    assert_eq!(downgrade_target.requests_served(), 0);
}
