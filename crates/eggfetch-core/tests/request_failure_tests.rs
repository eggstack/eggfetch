#![allow(missing_docs)]

//! Native detailed request-failure API coverage.

mod test_server;

use eggfetch_core::{Client, NetworkFailureKind};
use eggfetch_core::{Error, Timeout, TimeoutPhase};
use std::time::Duration;
use test_server::{TestServer, TestServerConfig};
use tokio::net::TcpListener;

#[tokio::test]
async fn detailed_success_matches_ordinary_response_semantics() {
    let mut server = TestServer::start(&TestServerConfig {
        response_body: Some(b"detailed success".to_vec()),
        ..Default::default()
    });
    let url = server.url();
    let client = Client::new();

    let mut ordinary = client.get(&url).unwrap().send().await.unwrap();
    let ordinary_status = ordinary.status();
    let ordinary_body = ordinary.bytes().await.unwrap();
    let mut detailed = client.get(&url).unwrap().send_detailed().await.unwrap();

    assert_eq!(detailed.status(), ordinary_status);
    assert_eq!(detailed.bytes().await.unwrap(), ordinary_body);
    server.shutdown();
}

#[tokio::test]
async fn detailed_loopback_refusal_is_structured_and_legacy_error_is_preserved() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let failure = Client::new()
        .get(&format!("http://{addr}/"))
        .unwrap()
        .send_detailed()
        .await
        .unwrap_err();
    // The existing standard Hyper route exposes this as `HyperClient`; the
    // detailed path adds refusal provenance without changing that contract.
    assert_eq!(failure.error().kind(), "hyper_client");
    assert_eq!(
        failure.network_failure_kind(),
        Some(NetworkFailureKind::ConnectionRefused)
    );
    assert_eq!(failure.into_error().kind(), "hyper_client");
}

#[tokio::test]
async fn detailed_resolved_connector_refusal_keeps_connect_error_shape() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let failure = Client::new()
        .get(&format!("http://example.invalid:{}/", addr.port()))
        .unwrap()
        .resolved_addresses([addr])
        .send_detailed()
        .await
        .unwrap_err();

    assert_eq!(failure.error().kind(), "connect");
    assert_eq!(
        failure.network_failure_kind(),
        Some(NetworkFailureKind::ConnectionRefused)
    );
}

#[tokio::test]
async fn detailed_timeout_preserves_ordinary_error_and_phase() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 200,
        ..Default::default()
    });
    let client = Client::builder()
        .timeout(Timeout {
            total: Some(Duration::from_millis(20)),
            ..Timeout::default()
        })
        .build();
    let url = server.url();

    let ordinary = client.get(&url).unwrap().send().await.unwrap_err();
    let detailed = client.get(&url).unwrap().send_detailed().await.unwrap_err();

    assert!(matches!(
        ordinary,
        Error::Timeout {
            phase: TimeoutPhase::Total,
            ..
        }
    ));
    assert_eq!(detailed.error().kind(), ordinary.kind());
    assert!(detailed.is_timeout());
    assert_eq!(detailed.timeout_phase(), Some(TimeoutPhase::Total));
    assert_eq!(detailed.network_failure_kind(), None);
    server.shutdown();
}
