#![allow(
    missing_docs,
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::large_futures
)]
//! Tests for the native `http::Uri` request builder.
//!
//! The native builder is available in every profile, including the minimal
//! native slice that compiles without `url`. These tests use stable APIs
//! only and must therefore pass under every supported profile.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use eggfetch_core::{Client, Error, NativeRequestBuilder, RedirectPolicy};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// One recorded request: the request line and the body bytes it carried.
type RecordedRequest = (String, Vec<u8>);

/// Loopback server that answers every request with a fixed body and records
/// what it received.
struct RecordingServer {
    port: u16,
    shutdown: tokio::sync::watch::Sender<bool>,
    request_count: Arc<AtomicUsize>,
    seen: Arc<std::sync::Mutex<Vec<RecordedRequest>>>,
}

impl RecordingServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
        let request_count = Arc::new(AtomicUsize::new(0));
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let counter = request_count.clone();
        let recorded = seen.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let Ok((mut stream, _)) = result else { break };
                        counter.fetch_add(1, Ordering::SeqCst);
                        let recorded = recorded.clone();
                        tokio::spawn(async move {
                            let mut reader = BufReader::new(&mut stream);
                            let mut request_line = String::new();
                            reader.read_line(&mut request_line).await.ok();
                            let mut content_length = 0usize;
                            loop {
                                let mut line = String::new();
                                if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                                    break;
                                }
                                if line.trim().is_empty() {
                                    break;
                                }
                                if let Some(value) = line
                                    .split_once(':')
                                    .filter(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
                                {
                                    content_length = value.1.trim().parse().unwrap_or(0);
                                }
                            }
                            let mut body = vec![0u8; content_length];
                            if content_length > 0 {
                                use tokio::io::AsyncReadExt as _;
                                reader.read_exact(&mut body).await.ok();
                            }
                            recorded
                                .lock()
                                .unwrap()
                                .push((request_line.trim().to_owned(), body));

                            let response_body = b"native-body";
                            let response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                response_body.len()
                            );
                            stream.write_all(response.as_bytes()).await.ok();
                            stream.write_all(response_body).await.ok();
                        });
                    }
                    _ = shutdown_rx.changed() => break,
                }
            }
        });

        Self {
            port,
            shutdown: shutdown_tx,
            request_count,
            seen,
        }
    }

    fn uri(&self, path: &str) -> http::Uri {
        format!("http://127.0.0.1:{}{path}", self.port)
            .parse()
            .unwrap()
    }
}

#[tokio::test]
async fn native_builder_dispatches_an_http_uri() {
    let server = RecordingServer::start().await;
    let client = Client::new();

    let response = client
        .get_uri(&server.uri("/native"))
        .unwrap()
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(server.request_count.load(Ordering::SeqCst), 1);
    let recorded = server.seen.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert!(
        recorded[0].0.starts_with("GET /native "),
        "unexpected request line: {}",
        recorded[0].0
    );
    server.shutdown.send(true).unwrap();
}

#[tokio::test]
async fn native_builder_sends_the_requested_body() {
    let server = RecordingServer::start().await;
    let client = Client::new();

    let response = client
        .post_uri(&server.uri("/submit"))
        .unwrap()
        .header("content-type", "text/plain")
        .bytes("payload")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);
    let recorded = server.seen.lock().unwrap();
    assert_eq!(recorded[0].0, "POST /submit HTTP/1.1");
    assert_eq!(recorded[0].1, b"payload");
    server.shutdown.send(true).unwrap();
}

#[tokio::test]
async fn native_builder_rejects_a_relative_uri() {
    let client = Client::new();
    let relative: http::Uri = "/only/a/path".parse().unwrap();
    let error = client
        .get_uri(&relative)
        .expect_err("relative URI must fail");
    assert!(
        matches!(error, Error::InvalidUrl(_) | Error::Unsupported(_)),
        "unexpected error: {error:?}"
    );
}

#[tokio::test]
async fn native_builder_rejects_a_non_http_scheme() {
    let client = Client::new();
    let ftp: http::Uri = "ftp://example.com/file".parse().unwrap();
    let error = client.get_uri(&ftp).expect_err("non-HTTP(S) URI must fail");
    assert!(
        matches!(error, Error::InvalidUrl(_) | Error::Unsupported(_)),
        "unexpected error: {error:?}"
    );
}

#[tokio::test]
async fn native_builder_builds_the_requested_target() {
    let client = Client::new();
    let uri: http::Uri = "https://example.com/path?q=1".parse().unwrap();

    let request = client
        .request_uri(http::Method::PUT, &uri)
        .unwrap()
        .header("x-test", "value")
        .bytes("body")
        .build()
        .unwrap();

    assert_eq!(request.method(), http::Method::PUT);
    assert_eq!(request.uri(), &uri);
    assert_eq!(request.headers().get("x-test").unwrap(), "value");
}

#[tokio::test]
async fn native_builder_policy_assertions_are_callable() {
    let server = RecordingServer::start().await;
    let client = Client::new();

    // The native transport dispatches exactly once and never follows
    // redirects, so both assertions hold by construction and must not
    // change the wire behavior.
    let response = client
        .get_uri(&server.uri("/assert"))
        .unwrap()
        .without_retry()
        .redirect_policy(RedirectPolicy::strict(0))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(server.request_count.load(Ordering::SeqCst), 1);
    server.shutdown.send(true).unwrap();
}

#[tokio::test]
async fn native_builder_is_exported_from_the_crate_root() {
    // Compile-time anchor for the re-export path.
    let _: fn(&Client, &http::Uri) -> Result<NativeRequestBuilder, Error> = Client::get_uri;
}
