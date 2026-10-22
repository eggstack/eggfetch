#![allow(missing_docs)]

//! Fail-closed classification for HTTP-version policies the build cannot honor.
//!
//! A strict `Http3Only` policy on a build without the `http3` feature must be
//! reported as a missing HTTP/3 capability, not as a missing Hyper client.
//! `Auto` keeps negotiating down to HTTP/1.1 and must never fail for this
//! reason.

mod test_server;

use eggfetch_core::{Client, HttpVersionPolicy, Timeout};
use std::time::Duration;
use test_server::{TestServer, TestServerConfig};

/// The capability the failure must name.
#[allow(
    dead_code,
    reason = "only asserted on builds without the http3 feature"
)]
const CAPABILITY_MARKER: &str = "HTTP/3";
/// The generic wording the failure must not reuse.
const GENERIC_MARKER: &str = "HTTP client not available";

fn strict_http3_client() -> Client {
    Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        // Keep the QUIC attempt short on builds that do have HTTP/3: the
        // request never reaches a QUIC peer in this fixture.
        .timeout(
            Timeout::builder()
                .connect(Duration::from_millis(500))
                .build(),
        )
        .build()
}

#[tokio::test]
#[cfg(not(feature = "http3"))]
#[cfg(feature = "high-level-url")]
async fn http_version_http3_only_without_http3_names_the_missing_capability() {
    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();

    let failure = strict_http3_client()
        .get(&url)
        .unwrap()
        .send_detailed()
        .await
        .unwrap_err();

    let message = failure.error().to_string();
    assert!(
        message.contains(CAPABILITY_MARKER),
        "the failure must name the missing HTTP/3 capability: {message}"
    );
    assert!(
        !message.contains(GENERIC_MARKER),
        "the failure must not reuse the generic missing-client wording: {message}"
    );
    assert_eq!(
        server.connections_accepted(),
        0,
        "an unbuildable protocol request must not open a connection"
    );
    server.shutdown();
}

#[tokio::test]
#[cfg(feature = "http3")]
#[cfg(feature = "high-level-url")]
async fn http_version_http3_only_with_http3_does_not_report_a_capability_gap() {
    // With HTTP/3 compiled in there is no capability gap to report: the
    // request is routed to the QUIC transport and fails there (if at all).
    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();

    let failure = strict_http3_client()
        .get(&url)
        .unwrap()
        .send_detailed()
        .await
        .unwrap_err();

    let message = failure.error().to_string();
    assert!(
        !message.contains(GENERIC_MARKER),
        "a build with HTTP/3 support has no missing-client gap to report: {message}"
    );
    server.shutdown();
}

#[tokio::test]
#[cfg(feature = "high-level-url")]
async fn http_version_auto_policy_negotiates_down_to_http1_without_failing() {
    let mut server = TestServer::start(&TestServerConfig {
        response_body: Some(b"auto negotiated".to_vec()),
        ..Default::default()
    });
    let url = server.url();

    for policy in [
        HttpVersionPolicy::Auto { allow_http3: false },
        HttpVersionPolicy::Auto { allow_http3: true },
    ] {
        let client = Client::builder().http_version_policy(policy).build();
        let mut response = client.get(&url).unwrap().send().await.unwrap();
        assert_eq!(response.status(), 200, "auto negotiation must not fail");
        assert_eq!(
            response.bytes().await.unwrap(),
            bytes::Bytes::from_static(b"auto negotiated")
        );
    }
    server.shutdown();
}

#[tokio::test]
#[cfg(not(feature = "http3"))]
async fn http_version_native_frame_path_reports_the_same_capability_gap() {
    // The native frame path has its own dispatch arm and must classify the
    // missing capability the same way as the high-level path.
    let mut server = TestServer::start(&TestServerConfig::default());
    let url = server.url();

    let request = http::Request::builder()
        .uri(&url)
        .body(http_body_util::Empty::<bytes::Bytes>::new())
        .unwrap();

    let error = strict_http3_client()
        .execute_http_body(request, eggfetch_core::NativeRequestOptions::default())
        .await
        .unwrap_err();

    let message = error.to_string();
    assert!(
        message.contains(CAPABILITY_MARKER),
        "the native frame failure must name the missing HTTP/3 capability: {message}"
    );
    assert!(
        !message.contains(GENERIC_MARKER),
        "the native frame failure must not reuse the generic missing-client wording: {message}"
    );
    assert_eq!(
        server.connections_accepted(),
        0,
        "an unbuildable protocol request must not open a connection"
    );
    server.shutdown();
}
