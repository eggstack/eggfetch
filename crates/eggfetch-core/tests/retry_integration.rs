#![allow(warnings)]
//! Integration tests for retry + redirect+auth subsystems.
//!
//! These tests use tokio's async TCP utilities.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use eggfetch_core::{Client, Error, RequestBody, RetryPolicy};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::watch;

// ---------------------------------------------------------------------------
// Helper: start a mock HTTP server
// ---------------------------------------------------------------------------

struct MockServer {
    port: u16,
    shutdown: watch::Sender<bool>,
    request_count: Arc<AtomicUsize>,
}

impl MockServer {
    /// Start a server that returns 503 for the first `fail_count` requests,
    /// then 200 for subsequent requests.
    async fn start(fail_count: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let request_count = Arc::new(AtomicUsize::new(0));
        let rc = request_count.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((mut stream, _)) => {
                                let count = rc.fetch_add(1, Ordering::SeqCst);
                                tokio::spawn(async move {
                                    let mut buf_reader = BufReader::new(&mut stream);
                                    let mut request_line = String::new();
                                    buf_reader.read_line(&mut request_line).await.ok();

                                    loop {
                                        let mut line = String::new();
                                        buf_reader.read_line(&mut line).await.ok();
                                        if line.trim().is_empty() {
                                            break;
                                        }
                                    }

                                    let status = if count < fail_count {
                                        503u16
                                    } else {
                                        200
                                    };
                                    let reason = match status {
                                        200 => "OK",
                                        _ => "Service Unavailable",
                                    };
                                    let body = match status {
                                        200 => b"ok".as_slice(),
                                        _ => b"unavailable".as_slice(),
                                    };

                                    let response = format!(
                                        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                        body.len()
                                    );
                                    stream.write_all(response.as_bytes()).await.ok();
                                    stream.write_all(body).await.ok();
                                });
                            }
                            Err(_) => break,
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        break;
                    }
                }
            }
        });

        Self {
            port,
            shutdown: shutdown_tx,
            request_count,
        }
    }

    /// Start a server that redirects on `/redirect` to `/final`.
    /// `/final` returns 503 for the first `fail_count` requests to `/final`,
    /// then 200.
    async fn start_redirect_then_fail(fail_count: usize) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let final_count = Arc::new(AtomicUsize::new(0));
        let fc = final_count.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((mut stream, _)) => {
                                let fc = fc.clone();
                                tokio::spawn(async move {
                                    let mut buf_reader = BufReader::new(&mut stream);
                                    let mut request_line = String::new();
                                    buf_reader.read_line(&mut request_line).await.ok();

                                    loop {
                                        let mut line = String::new();
                                        buf_reader.read_line(&mut line).await.ok();
                                        if line.trim().is_empty() {
                                            break;
                                        }
                                    }

                                    let path: String = request_line
                                        .split_whitespace()
                                        .nth(1)
                                        .unwrap_or("/")
                                        .to_string();

                                    if path == "/redirect" {
                                        let response = format!(
                                            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                                        );
                                        stream.write_all(response.as_bytes()).await.ok();
                                    } else if path == "/final" {
                                        let count = fc.fetch_add(1, Ordering::SeqCst);
                                        let (status, reason, body): (u16, &str, &[u8]) =
                                            if count < fail_count {
                                                (503, "Service Unavailable", b"unavailable")
                                            } else {
                                                (200, "OK", b"ok")
                                            };

                                        let response = format!(
                                            "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                            body.len()
                                        );
                                        stream.write_all(response.as_bytes()).await.ok();
                                        stream.write_all(body).await.ok();
                                    } else {
                                        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                                        stream.write_all(response.as_bytes()).await.ok();
                                    }
                                });
                            }
                            Err(_) => break,
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        break;
                    }
                }
            }
        });

        Self {
            port,
            shutdown: shutdown_tx,
            request_count: final_count,
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn shutdown(&self) {
        let _ = self.shutdown.send(true);
    }
}

// ---------------------------------------------------------------------------
// Retry + redirect integration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn retry_across_redirect_chain() {
    let server = MockServer::start_redirect_then_fail(2).await;
    let policy = RetryPolicy::builder()
        .max_attempts(3)
        .backoff_factor(0.0)
        .build();

    let client = Client::builder()
        .retry(policy)
        .follow_redirects(true)
        .build();
    let url = format!("{}/redirect", server.url());
    let resp = client.get(&url).unwrap().send().await.unwrap();
    assert_eq!(resp.status(), 200);
    // 2 redirects failed (503) + 1 succeeded = 3 requests to /final
    assert_eq!(server.request_count.load(Ordering::SeqCst), 3);
    server.shutdown();
}

#[tokio::test]
async fn retry_gives_up_after_budget_exhausted() {
    let server = MockServer::start(100).await;
    let policy = RetryPolicy::builder()
        .max_attempts(3)
        .backoff_factor(0.0)
        .build();

    let client = Client::builder().retry(policy).build();
    let url = format!("{}/", server.url());
    let resp = client.get(&url).unwrap().send().await.unwrap();
    assert_eq!(resp.status(), 503);
    assert_eq!(server.request_count.load(Ordering::SeqCst), 3);
    server.shutdown();
}

#[tokio::test]
async fn retry_respects_total_timeout() {
    let server = MockServer::start(100).await;
    let policy = RetryPolicy::builder()
        .max_attempts(50)
        .backoff_factor(1.0)
        .initial_delay(Duration::from_millis(100))
        .max_delay(Duration::from_millis(100))
        .max_elapsed(Duration::from_millis(250))
        .build();

    let client = Client::builder().retry(policy).build();

    let start = std::time::Instant::now();
    let url = format!("{}/", server.url());
    let _ = client.get(&url).unwrap().send().await;
    let elapsed = start.elapsed();

    // Should have been cut short by the elapsed budget
    assert!(elapsed < Duration::from_secs(2));
    // Made some attempts but not all 50
    let count = server.request_count.load(Ordering::SeqCst);
    assert!(count >= 2, "expected at least 2 attempts, got {count}");
    assert!(count < 50, "expected fewer than 50 attempts, got {count}");
    server.shutdown();
}

#[tokio::test]
async fn retry_stream_body_sends_once() {
    let server = MockServer::start(10).await;
    let policy = RetryPolicy::builder().max_attempts(3).build();

    let body = RequestBody::from_stream(
        futures_util::stream::empty::<std::result::Result<bytes::Bytes, Error>>(),
        None,
    );

    let client = Client::builder().retry(policy).build();
    let url = format!("{}/", server.url());
    // Stream body with retry configured: sends once without retry
    let resp = client.post(&url).unwrap().body(body).send().await.unwrap();
    assert_eq!(resp.status(), 503);
    // Only 1 request — no retries for stream bodies
    assert_eq!(server.request_count.load(Ordering::SeqCst), 1);
    server.shutdown();
}
