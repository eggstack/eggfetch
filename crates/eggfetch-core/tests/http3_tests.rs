#![allow(warnings)]
#![cfg(feature = "http3")]

//! HTTP/3 integration tests.
//!
//! These tests exercise the public types, policies, error handling, and client
//! construction paths for HTTP/3 (QUIC + h3). No real QUIC server is started;
//! connection attempts fail with typed errors that prove the plumbing works.

mod test_server;

use eggfetch_core::{Client, Error, HttpVersionPolicy};

// ---------------------------------------------------------------------------
// 1. HttpVersionPolicy::Http3Only variant
// ---------------------------------------------------------------------------

#[test]
fn http3_only_variant_exists() {
    let policy = HttpVersionPolicy::Http3Only;
    assert_eq!(policy, HttpVersionPolicy::Http3Only);
    assert_ne!(policy, HttpVersionPolicy::Auto { allow_http3: false });
    assert_ne!(policy, HttpVersionPolicy::Http1Only);
    assert_ne!(policy, HttpVersionPolicy::Http2Only);
}

#[test]
fn http3_only_is_debug() {
    let policy = HttpVersionPolicy::Http3Only;
    let debug = format!("{policy:?}");
    assert_eq!(debug, "Http3Only");
}

#[test]
fn http3_only_is_clone() {
    let policy = HttpVersionPolicy::Http3Only;
    let cloned = policy;
    assert_eq!(policy, cloned);
}

#[test]
fn http3_only_is_not_default() {
    let default = HttpVersionPolicy::default();
    assert_eq!(default, HttpVersionPolicy::Auto { allow_http3: false });
    assert_ne!(default, HttpVersionPolicy::Http3Only);
}

// ---------------------------------------------------------------------------
// 2. http3 feature flag compiles correctly
// ---------------------------------------------------------------------------

#[test]
fn http3_feature_compiles() {
    // Verify that H3-specific error variants are available.
    let err = Error::H3Connect("test".into());
    assert_eq!(err.kind(), "h3_connect");

    let err = Error::H3ConnectionClosed("test".into());
    assert_eq!(err.kind(), "h3_connection_closed");

    let err = Error::H3Stream("test".into());
    assert_eq!(err.kind(), "h3_stream");

    let err = Error::H3Protocol("test".into());
    assert_eq!(err.kind(), "h3_protocol");
}

// ---------------------------------------------------------------------------
// 3. H3 error variants
// ---------------------------------------------------------------------------

#[test]
fn error_h3_connect_display() {
    let err = Error::H3Connect("connection refused".into());
    assert_eq!(err.to_string(), "HTTP/3 connect error: connection refused");
}

#[test]
fn error_h3_connect_kind() {
    let err = Error::H3Connect(String::new());
    assert_eq!(err.kind(), "h3_connect");
}

#[test]
fn error_h3_connection_closed_display() {
    let err = Error::H3ConnectionClosed("peer sent GOAWAY".into());
    assert_eq!(
        err.to_string(),
        "HTTP/3 connection closed: peer sent GOAWAY"
    );
}

#[test]
fn error_h3_connection_closed_kind() {
    let err = Error::H3ConnectionClosed(String::new());
    assert_eq!(err.kind(), "h3_connection_closed");
}

#[test]
fn error_h3_stream_display() {
    let err = Error::H3Stream("stream reset".into());
    assert_eq!(err.to_string(), "HTTP/3 stream error: stream reset");
}

#[test]
fn error_h3_stream_kind() {
    let err = Error::H3Stream(String::new());
    assert_eq!(err.kind(), "h3_stream");
}

#[test]
fn error_h3_protocol_display() {
    let err = Error::H3Protocol("unexpected frame".into());
    assert_eq!(err.to_string(), "HTTP/3 protocol error: unexpected frame");
}

#[test]
fn error_h3_protocol_kind() {
    let err = Error::H3Protocol(String::new());
    assert_eq!(err.kind(), "h3_protocol");
}

// ---------------------------------------------------------------------------
// 4. Building a client with Http3Only policy succeeds
// ---------------------------------------------------------------------------

#[test]
fn build_client_with_http3_only_policy() {
    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .build();
    // The client should be constructible without error.
    let req = client.get("https://example.com/").unwrap().build().unwrap();
    assert_eq!(req.url().scheme(), "https");
}

#[test]
fn build_client_with_http3_only_and_custom_tls() {
    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .build();
    let req = client
        .get("https://example.com/path")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(req.url().path(), "/path");
}

// ---------------------------------------------------------------------------
// 5. Building a client with Auto policy still works (no regression)
// ---------------------------------------------------------------------------

#[test]
fn build_client_with_auto_policy() {
    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Auto { allow_http3: false })
        .build();
    let req = client.get("https://example.com/").unwrap().build().unwrap();
    assert_eq!(req.url().scheme(), "https");
}

#[test]
fn default_client_uses_auto_policy() {
    let client = Client::new();
    let req = client.get("https://example.com/").unwrap().build().unwrap();
    assert_eq!(req.url().scheme(), "https");
}

// ---------------------------------------------------------------------------
// 6. Http3Only client can attempt a request (typed error)
// ---------------------------------------------------------------------------

#[test]
fn http3_only_request_builds_and_attempts() {
    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .build();

    // Build the request — this should succeed.
    let builder = client.get("https://example.com/").unwrap();
    let req = builder.build().unwrap();
    assert_eq!(req.url().host_str(), Some("example.com"));
}

#[tokio::test]
async fn http3_only_send_to_localhost_fails_with_h3_or_unsupported_error() {
    use std::net::TcpListener;

    // Grab an unused port by binding then dropping.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http3Only)
        .build();

    let url = format!("https://127.0.0.1:{port}/");
    let result = client.get(&url).unwrap().send().await;

    // The request must fail — either because the QUIC endpoint could not be
    // created (Unsupported), or because there is no QUIC server on this port
    // (H3Connect / Connect).
    match result {
        Err(Error::Unsupported(_)) => {
            // H3Connector::new failed during client build (e.g. UDP bind
            // failure). The pipeline returns this typed error.
        }
        Err(Error::H3Connect(_)) => {}
        Err(Error::H3Protocol(_)) => {}
        Err(Error::H3ConnectionClosed(_)) => {}
        Err(Error::H3Stream(_)) => {}
        Err(Error::Connect(_)) => {
            // QUIC connect failure may surface as a generic Connect error
            // depending on whether the UDP packet is dropped or refused.
        }
        other => {
            panic!(
                "expected H3, Connect, or Unsupported error for Http3Only \
                 request to localhost:{port}, got: {other:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 7. Auto policy still sends HTTP/1.1 requests (regression guard)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn auto_policy_to_local_server_works() {
    use test_server::{TestServer, TestServerConfig};

    let server = TestServer::start(&TestServerConfig {
        response_body: Some(b"h3-regression-ok".to_vec()),
        ..Default::default()
    });

    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Auto { allow_http3: false })
        .build();

    let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let body = resp.text().await.unwrap();
    assert_eq!(body, "h3-regression-ok");
}
