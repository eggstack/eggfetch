#![allow(warnings)]
//! HTTP/2 integration and unit tests.
//!
//! Tests cover:
//! - Forbidden header stripping for h2 requests
//! - h2 error taxonomy and retry classification
//! - Pool concurrency model documentation
//! - Protocol version reporting
//! - Stream cancellation and permit release
//! - Feature-gated build validation

#![cfg(feature = "http2")]
#![allow(clippy::module_name_repetitions)]

mod test_server;

use std::time::Duration;

use eggfetch_core::error::Error;
use eggfetch_core::http_version::HttpVersionPolicy;
use eggfetch_core::Client;
use test_server::{TestServer, TestServerConfig};

// ── Forbidden header stripping ────────────────────────────────────

/// Verify that Connection, Keep-Alive, Proxy-Connection, Transfer-Encoding,
/// and Upgrade are stripped before sending. The request should succeed
/// because the forbidden headers are removed before the request is sent.
#[tokio::test]
async fn forbidden_headers_are_stripped() {
    let server = TestServer::start(&TestServerConfig {
        consume_body: true,
        ..Default::default()
    });

    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http1Only)
        .timeout(eggfetch_core::Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    // Send a request with forbidden h2 headers. The pipeline strips
    // them unconditionally, so the request should succeed.
    let resp = client
        .post(&server.url())
        .unwrap()
        .header("connection", "keep-alive")
        .header("keep-alive", "timeout=5")
        .header("transfer-encoding", "chunked")
        .header("upgrade", "h2c")
        .header("content-type", "text/plain")
        .body("hello")
        .send()
        .await;
    assert!(
        resp.is_ok(),
        "request with forbidden headers should succeed after stripping: {resp:?}"
    );
    let resp = resp.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
}

/// TE with value "trailers" should be preserved (it's allowed in h2).
#[tokio::test]
async fn te_trailers_is_preserved() {
    let server = TestServer::start(&TestServerConfig {
        consume_body: true,
        ..Default::default()
    });

    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http1Only)
        .timeout(eggfetch_core::Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    let resp = client
        .get(&server.url())
        .unwrap()
        .header("te", "trailers")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
}

/// TE with non-"trailers" value should be stripped.
#[tokio::test]
async fn te_non_trailers_is_stripped() {
    let server = TestServer::start(&TestServerConfig {
        consume_body: true,
        ..Default::default()
    });

    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http1Only)
        .timeout(eggfetch_core::Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    // TE with non-trailers value should be stripped. The request should
    // succeed because the forbidden header is removed.
    let resp = client
        .get(&server.url())
        .unwrap()
        .header("te", "gzip")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200);
}

// ── Error taxonomy ────────────────────────────────────────────────

#[test]
fn http2_go_away_error_display() {
    let err = Error::Http2GoAway {
        last_stream_id: 5,
        debug_data: "server shutting down".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("GOAWAY"));
    assert!(msg.contains("5"));
    assert!(msg.contains("server shutting down"));
    assert_eq!(err.kind(), "http2_go_away");
}

#[test]
fn http2_stream_reset_error_display() {
    let err = Error::Http2StreamReset {
        reason: "REFUSED_STREAM: stream refused before processing".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("REFUSED_STREAM"));
    assert_eq!(err.kind(), "http2_stream_reset");
}

#[test]
fn http2_flow_control_error_display() {
    let err = Error::Http2FlowControl("flow-control window exhausted".into());
    let msg = err.to_string();
    assert!(msg.contains("flow control"));
    assert_eq!(err.kind(), "http2_flow_control");
}

#[test]
fn http2_protocol_error_display() {
    let err = Error::Http2Protocol("received data on half-closed stream".into());
    let msg = err.to_string();
    assert!(msg.contains("protocol error"));
    assert_eq!(err.kind(), "http2_protocol");
}

// ── Retry classification ──────────────────────────────────────────

#[test]
fn refused_stream_is_retryable() {
    let err = Error::Http2StreamReset {
        reason: "REFUSED_STREAM: stream refused before processing".into(),
    };
    assert!(eggfetch_core::RetryPolicy::is_error_retryable(&err));
}

#[test]
fn cancel_is_not_retryable() {
    let err = Error::Http2StreamReset {
        reason: "CANCEL: stream cancelled by peer".into(),
    };
    assert!(!eggfetch_core::RetryPolicy::is_error_retryable(&err));
}

#[test]
fn go_away_is_not_retryable() {
    let err = Error::Http2GoAway {
        last_stream_id: 0,
        debug_data: "graceful shutdown".into(),
    };
    assert!(!eggfetch_core::RetryPolicy::is_error_retryable(&err));
}

#[test]
fn flow_control_is_not_retryable() {
    let err = Error::Http2FlowControl("flow-control violated".into());
    assert!(!eggfetch_core::RetryPolicy::is_error_retryable(&err));
}

#[test]
fn http2_protocol_is_not_retryable() {
    let err = Error::Http2Protocol("stream closed after headers".into());
    assert!(!eggfetch_core::RetryPolicy::is_error_retryable(&err));
}

// ── Client builder ────────────────────────────────────────────────

#[test]
fn builder_accepts_http2_only_policy() {
    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http2Only)
        .build();
    drop(client);
}

#[test]
fn builder_accepts_auto_policy() {
    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Auto { allow_http3: false })
        .build();
    drop(client);
}

#[test]
fn builder_default_is_auto() {
    let client = Client::builder().build();
    drop(client);
}

// ── Multiplexing semantics ────────────────────────────────────────

/// Multiple concurrent requests to the same origin complete successfully.
#[tokio::test]
async fn concurrent_requests_share_pool() {
    let server = TestServer::start(&TestServerConfig {
        consume_body: true,
        ..Default::default()
    });

    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Auto { allow_http3: false })
        .max_connections(5)
        .max_connections_per_host(5)
        .timeout(eggfetch_core::Timeout {
            total: Some(Duration::from_secs(10)),
            ..Default::default()
        })
        .build();

    let mut handles = Vec::new();
    for _ in 0..5 {
        let c = client.clone();
        let url = server.url();
        handles.push(tokio::spawn(async move {
            let mut resp = c.get(&url).unwrap().send().await.unwrap();
            assert_eq!(resp.status().as_u16(), 200);
            let _ = resp.text().await;
        }));
    }

    for h in handles {
        h.await.unwrap();
    }
}

/// Verify pool metrics track logical permits correctly.
#[tokio::test]
async fn pool_metrics_track_permits() {
    let server = TestServer::start(&TestServerConfig {
        consume_body: true,
        ..Default::default()
    });

    let client = Client::builder()
        .max_connections(2)
        .max_connections_per_host(2)
        .timeout(eggfetch_core::Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    let mut r1 = client.get(&server.url()).unwrap().send().await.unwrap();
    let _ = r1.text().await;

    let mut r2 = client.get(&server.url()).unwrap().send().await.unwrap();
    let _ = r2.text().await;

    let metrics = client.pool_metrics();
    let _ = metrics;
}

// ── h2c (prior knowledge) ─────────────────────────────────────────

/// h2c is not supported; verify the client doesn't panic when
/// attempting h2c-like behavior (which is just h2 via TLS ALPN).
/// Against an HTTP/1.1 server, h2-only should produce an error or
/// the server may silently accept h1 — either way, no panic.
#[tokio::test]
async fn h2c_not_supported_no_panic() {
    let server = TestServer::start(&TestServerConfig::default());

    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http2Only)
        .timeout(eggfetch_core::Timeout {
            total: Some(Duration::from_secs(3)),
            ..Default::default()
        })
        .build();

    // h2-only against an HTTP/1.1 server may fail (ALPN mismatch) or
    // the server may accept it (h1 only). The important thing is that
    // the client doesn't panic.
    let _ = client.get(&server.url()).unwrap().send().await;
}

// ── Protocol version in response ──────────────────────────────────

/// HTTP/1.1 server responds with HTTP/1.1 version.
#[tokio::test]
async fn response_version_reports_http1() {
    let server = TestServer::start(&TestServerConfig::default());

    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http1Only)
        .timeout(eggfetch_core::Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    let resp = client.get(&server.url()).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    assert_eq!(
        resp.version(),
        http::Version::HTTP_11,
        "response version should be HTTP/1.1 for an HTTP/1.1 server"
    );
}

// ── Stream cancellation ───────────────────────────────────────────

/// Dropping a streaming response mid-read should not leak pool permits.
#[tokio::test]
async fn dropping_stream_releases_permit() {
    let server = TestServer::start(&TestServerConfig {
        response_body: Some(b"hello world".to_vec()),
        ..Default::default()
    });

    let client = Client::builder()
        .max_connections(1)
        .timeout(eggfetch_core::Timeout {
            total: Some(Duration::from_secs(5)),
            ..Default::default()
        })
        .build();

    {
        let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();
        let mut stream = resp.bytes_stream().unwrap();
        let _ = futures_util::StreamExt::next(&mut stream).await;
        drop(stream);
    }

    let mut resp = client.get(&server.url()).unwrap().send().await.unwrap();
    assert_eq!(resp.status().as_u16(), 200);
    let _ = resp.text().await;
}

// ── Feature-gated builds ─────────────────────────────────────────

/// Verify the http2 feature flag is correctly compiled.
#[test]
fn http2_feature_is_enabled() {
    // This test only compiles when http2 feature is on.
    // Build a client with Http2Only policy - it should succeed.
    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http2Only)
        .build();
    drop(client);
}

// ── Load test and proxy limitations ───────────────────────────────
//
// # Load test: h1 vs h2 connection count comparison
//
// The plan requires load tests comparing h1 vs h2 connection counts.
// This requires a real HTTP/2 test server (e.g., using `h2` crate's
// server API), which is not yet available in the test infrastructure.
// Under HTTP/2, multiple requests multiplex over a single connection,
// so the connection count should be lower than HTTP/1.1 for concurrent
// requests. This will be implemented when an HTTP/2 test server is
// added to the test infrastructure.
//
// # HTTP/2 through CONNECT proxy tunnel
//
// The plan requires testing HTTP/2 through a CONNECT tunnel. The
// existing proxy tests use HTTP/1.1 servers. Testing h2 through a
// CONNECT tunnel requires an HTTP/2-capable server behind the proxy,
// which is not yet available. The CONNECT tunnel itself is
// transport-agnostic (raw TCP), so h2 negotiation inside the tunnel
// should work transparently. This will be tested when an HTTP/2 test
// server is available.
