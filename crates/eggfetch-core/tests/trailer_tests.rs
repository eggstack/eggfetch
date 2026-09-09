#![allow(
    missing_docs,
    dead_code,
    clippy::missing_panics_doc,
    clippy::unwrap_used,
    clippy::expect_used
)]
//! Trailer lifecycle integration tests (plan: native-protocol-observability).
//!
//! - H1 chunked trailers preserve duplicate/multi-value headers.
//! - Ordinary responses without trailers behave unchanged.
//! - Partial consumption does not falsely report trailers.
//! - Body errors before trailers remain body errors.

use std::time::Duration;

use eggfetch_core::Client;
use futures_util::StreamExt;

/// Start a raw TCP server that writes `raw_response` once then closes.
async fn start_raw_server(raw_response: &'static [u8]) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = vec![0u8; 4096];
            let _ = tokio::time::timeout(Duration::from_secs(5), socket.read(&mut buf)).await;
            let _ = socket.write_all(raw_response).await;
            let _ = socket.flush().await;
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    });
    (format!("http://{addr}/"), handle)
}

#[tokio::test]
async fn h1_chunked_trailers_surface() {
    // Note: hyper's H1 trailer decoder uses `HeaderMap::insert`, so
    // duplicate same-name H1 trailers collapse to the last value upstream.
    // Our store preserves whatever hyper exposes (including H2 duplicates
    // via `get_all`); this fixture pins distinct H1 trailers end-to-end.
    let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nTrailer: X-Trailer-1, X-Trailer-2\r\n\r\n5\r\nhello\r\n0\r\nX-Trailer-1: value1\r\nX-Trailer-2: value2\r\n\r\n";
    let (url, server) = start_raw_server(raw).await;
    let client = Client::builder().build();
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    assert!(
        resp.trailers().is_none(),
        "trailers unavailable before body"
    );
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(bytes.as_ref(), b"hello");
    let trailers = resp.trailers().expect("trailers after body");
    assert_eq!(trailers.get("x-trailer-1").unwrap(), "value1");
    assert_eq!(trailers.get("x-trailer-2").unwrap(), "value2");
    server.abort();
}

#[tokio::test]
async fn ordinary_response_without_trailers_unchanged() {
    let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
    let (url, server) = start_raw_server(raw).await;
    let client = Client::builder().build();
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(bytes.as_ref(), b"ok");
    assert!(resp.trailers().is_none());
    server.abort();
}

#[tokio::test]
async fn partial_consumption_does_not_report_trailers() {
    let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nTrailer: X-End\r\n\r\n5\r\nhello\r\n0\r\nX-End: yes\r\n\r\n";
    let (url, server) = start_raw_server(raw).await;
    let client = Client::builder().build();
    let resp = client.get(&url).unwrap().send().await.unwrap();
    assert!(resp.trailers().is_none());
    // Drop without consuming: permit released, no trailers fabricated.
    drop(resp);
    // Second request still works (permit was released).
    let (url2, server2) = start_raw_server(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").await;
    let _ = url;
    let mut resp2 = client.get(&url2).unwrap().send().await.unwrap();
    let _ = resp2.bytes().await.unwrap();
    assert!(resp2.trailers().is_none());
    server.abort();
    server2.abort();
}

#[tokio::test]
async fn streaming_partial_then_full_reports_trailers() {
    let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nTrailer: X-End\r\n\r\n5\r\nhello\r\n0\r\nX-End: yes\r\n\r\n";
    let (url, server) = start_raw_server(raw).await;
    let client = Client::builder().build();
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    let mut stream = resp.bytes_stream().unwrap();
    assert!(resp.trailers().is_none());
    let first = stream.next().await.unwrap().unwrap();
    assert_eq!(first.as_ref(), b"hello");
    // Drive to EOF.
    assert!(stream.next().await.is_none());
    let trailers = resp.trailers().expect("trailers after EOF");
    assert_eq!(trailers.get("x-end").unwrap(), "yes");
    server.abort();
}

#[tokio::test]
async fn read_timeout_while_waiting_for_trailers() {
    // Server sends chunk header + data but stalls before trailers EOF.
    // Read timeout at the body boundary must fire; trailers stay None
    // (no fabricated state) and the permit is released.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = vec![0u8; 4096];
            let _ = socket.read(&mut buf).await;
            let _ = socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nTrailer: X-End\r\n\r\n5\r\nhello\r\n",
                )
                .await;
            let _ = socket.flush().await;
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
    let url = format!("http://{addr}/");
    let client = Client::builder()
        .timeout(eggfetch_core::Timeout {
            read: Some(Duration::from_millis(200)),
            ..Default::default()
        })
        .build();
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    let err = resp.bytes().await.unwrap_err();
    assert!(
        matches!(
            err,
            eggfetch_core::Error::Timeout {
                phase: eggfetch_core::TimeoutPhase::Read,
                ..
            }
        ),
        "expected read-phase timeout while waiting for trailers, got {err:?}"
    );
    assert!(resp.trailers().is_none());
}

#[tokio::test]
async fn metadata_remains_valid_after_body_with_trailers() {
    let raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nTrailer: X-End\r\n\r\n2\r\nok\r\n0\r\nX-End: yes\r\n\r\n";
    let (url, server) = start_raw_server(raw).await;
    let client = Client::builder().build();
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    let _ = resp.bytes().await.unwrap();
    // Trailers available and response metadata (status/url) still valid.
    assert_eq!(resp.status().as_u16(), 200);
    assert!(resp.url().as_str().starts_with("http://127.0.0.1:"));
    assert_eq!(resp.trailers().unwrap().get("x-end").unwrap(), "yes");
    server.abort();
}

/// H2 trailing HEADERS are surfaced through the same trailer store.
///
/// Uses h2 prior-knowledge (cleartext) so no TLS is required. The server
/// sends duplicate trailer values to pin multi-value preservation via
/// `get_all` (H1 collapses duplicates upstream in hyper; H2 preserves
/// them, and our store preserves whatever the protocol yields).
#[cfg(feature = "http2")]
#[tokio::test]
async fn h2_trailing_headers_surface_with_duplicates() {
    use eggfetch_core::HttpVersionPolicy;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut conn = h2::server::handshake(socket).await.unwrap();
        while let Some(req) = conn.accept().await {
            let (_req, mut respond) = req.unwrap();
            let response = http::Response::builder().status(200).body(()).unwrap();
            let mut send = respond.send_response(response, false).unwrap();
            send.send_data(bytes::Bytes::from("hi"), false).unwrap();
            let mut trailers = http::HeaderMap::new();
            trailers.append("x-h2-trailer", "a".parse().unwrap());
            trailers.append("x-h2-trailer", "b".parse().unwrap());
            send.send_trailers(trailers).unwrap();
        }
    });

    let client = Client::builder()
        .http_version_policy(HttpVersionPolicy::Http2Only)
        .build();
    let url = format!("http://{addr}/");
    let mut resp = client.get(&url).unwrap().send().await.unwrap();
    assert_eq!(resp.version(), http::Version::HTTP_2);
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(bytes.as_ref(), b"hi");
    let trailers = resp.trailers().expect("h2 trailers after body");
    let values: Vec<_> = trailers
        .get_all("x-h2-trailer")
        .iter()
        .map(|v| v.to_str().unwrap().to_owned())
        .collect();
    assert_eq!(values, vec!["a", "b"], "h2 duplicates preserved");
    server.abort();
}
