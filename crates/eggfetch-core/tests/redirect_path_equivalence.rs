//! The redirect path's outgoing request must match the shared expectation.
//!
//! The lean path asserts the same expectation in
//! `lean_path_equivalence.rs`; the two targets are mutually exclusive at
//! compile time, so the shared expectation in `hop_equivalence_support.rs`
//! is what ties the two builds together.

// The redirect path is the multi-hop path used when `redirects` is present.
#![cfg(all(feature = "high-level-url", feature = "redirects"))]

#[path = "hop_equivalence_support.rs"]
mod support;

use support::{expected_request, scenario_client, scenario_request, spawn_capture_server};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// The redirect path puts exactly the expected request on the wire.
#[tokio::test]
async fn redirect_path_sends_the_expected_request() {
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
        "the redirect path must put the expected request on the wire"
    );
}

/// The redirect path's first hop is what the lean path sends: a redirect
/// response is only followed when the policy asks for it, and the default
/// policy does not.
#[tokio::test]
async fn redirect_path_default_policy_does_not_follow() {
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
    server.await.expect("server task");
}
