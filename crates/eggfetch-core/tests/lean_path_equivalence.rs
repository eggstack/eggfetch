//! The lean path's outgoing request must match the shared expectation.
//!
//! The redirect path asserts the same expectation in
//! `redirect_path_equivalence.rs`; the two targets are mutually exclusive
//! at compile time, so the shared expectation in
//! `hop_equivalence_support.rs` is what ties the two builds together.

// The lean path is the single-hop path used when `redirects` is absent.
#![cfg(all(feature = "high-level-url", not(feature = "redirects")))]

#[path = "hop_equivalence_support.rs"]
mod support;

use support::{expected_request, scenario_client, scenario_request, spawn_capture_server};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The lean path puts exactly the expected request on the wire.
#[tokio::test]
async fn lean_path_sends_the_expected_request() {
    let (url, server) = spawn_capture_server().await;
    let client = scenario_client();

    let response = scenario_request(client.post(&url).expect("valid URL"))
        .send()
        .await
        .expect("response");
    assert_eq!(response.status(), 200);

    let captured = server.await.expect("capture task");
    assert_eq!(
        captured,
        expected_request(),
        "the lean path must put the expected request on the wire"
    );
}

/// The lean path does not follow a redirect: a 3xx response is returned to
/// the caller untouched.
#[tokio::test]
async fn lean_path_returns_redirects_without_following() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback listener");
    let address = listener.local_addr().expect("local address");
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept connection");
        let mut buffer = [0u8; 4096];
        let _ = socket.read(&mut buffer).await;
        let _ = socket
            .write_all(b"HTTP/1.1 302 Found\r\nLocation: /elsewhere\r\nContent-Length: 0\r\n\r\n")
            .await;
    });

    let client = scenario_client();
    let response = client
        .get(&format!("http://{address}/"))
        .expect("valid URL")
        .send()
        .await
        .expect("response");

    assert_eq!(response.status(), 302);
    assert!(
        response.headers().get("location").is_some(),
        "the redirect target must be left to the caller"
    );
    server.await.expect("server task");
}
