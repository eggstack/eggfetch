#![allow(
    missing_docs,
    dead_code,
    unused_mut,
    clippy::large_futures,
    clippy::missing_panics_doc,
    clippy::redundant_closure_for_method_calls,
    clippy::inefficient_to_string,
    clippy::manual_let_else,
    clippy::single_char_pattern,
    clippy::match_same_arms,
    clippy::needless_borrow,
    clippy::trim_split_whitespace,
    clippy::too_many_lines,
    clippy::unused_self,
    clippy::items_after_statements,
    clippy::expect_fun_call,
    clippy::len_zero,
    clippy::unnecessary_debug_formatting,
    clippy::format_push_string,
    clippy::new_without_default,
    clippy::map_unwrap_or
)]
//! Integration tests for retry + redirect+auth subsystems.
//!
//! These tests use tokio's async TCP utilities.

use std::error::Error as StdError;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use eggfetch_core::{Client, Error, RequestBody, RetryPolicy, Timeout, TimeoutPhase};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, watch};

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
// Deterministic stale-idle connection fixture
// ---------------------------------------------------------------------------

/// A loopback server that closes its first keepalive connection only after the
/// client has completed the first request. This makes Hyper's reused-idle
/// connection retry path observable without relying on timing or an external
/// origin.
struct StaleIdleServer {
    port: u16,
    request_count: Arc<AtomicUsize>,
    connection_count: Arc<AtomicUsize>,
    close_idle: Option<oneshot::Sender<()>>,
    idle_closed: Option<oneshot::Receiver<()>>,
    shutdown: Option<oneshot::Sender<()>>,
    accept_handle: Option<tokio::task::JoinHandle<()>>,
}

impl StaleIdleServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let request_count = Arc::new(AtomicUsize::new(0));
        let connection_count = Arc::new(AtomicUsize::new(0));
        let (close_idle_tx, close_idle_rx) = oneshot::channel();
        let (idle_closed_tx, idle_closed_rx) = oneshot::channel();
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        let request_count_for_accept = request_count.clone();
        let connection_count_for_accept = connection_count.clone();
        let accept_handle = tokio::spawn(async move {
            let mut first_close = Some(close_idle_rx);
            let mut first_closed = Some(idle_closed_tx);

            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let Ok((stream, _)) = result else {
                            break;
                        };
                        connection_count_for_accept.fetch_add(1, Ordering::SeqCst);
                        let request_count = request_count_for_accept.clone();
                        let close_rx = first_close.take();
                        let closed_tx = first_closed.take();
                        tokio::spawn(async move {
                            serve_stale_idle_connection(
                                stream,
                                request_count,
                                close_rx,
                                closed_tx,
                            )
                            .await;
                        });
                    }
                    _ = &mut shutdown_rx => break,
                }
            }
        });

        Self {
            port,
            request_count,
            connection_count,
            close_idle: Some(close_idle_tx),
            idle_closed: Some(idle_closed_rx),
            shutdown: Some(shutdown_tx),
            accept_handle: Some(accept_handle),
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    async fn close_idle_connection(&mut self) {
        self.close_idle
            .take()
            .expect("stale connection close command already sent")
            .send(())
            .expect("stale connection handler exited before close command");
        self.idle_closed
            .take()
            .expect("stale connection close acknowledgement already received")
            .await
            .expect("stale connection handler dropped close acknowledgement");
    }

    async fn shutdown(&mut self) {
        let _ = self
            .shutdown
            .take()
            .expect("server already shut down")
            .send(());
        self.accept_handle
            .take()
            .expect("server accept task already joined")
            .await
            .expect("stale connection accept task panicked");
    }
}

async fn serve_stale_idle_connection(
    stream: tokio::net::TcpStream,
    request_count: Arc<AtomicUsize>,
    close_rx: Option<oneshot::Receiver<()>>,
    closed_tx: Option<oneshot::Sender<()>>,
) {
    let (read_half, mut write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    if read_http_request(&mut reader).await.is_none() {
        return;
    }

    request_count.fetch_add(1, Ordering::SeqCst);
    let keepalive = close_rx.is_some();
    let connection = if keepalive { "keep-alive" } else { "close" };
    let response =
        format!("HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: {connection}\r\n\r\nok");
    if write_half.write_all(response.as_bytes()).await.is_err() || write_half.flush().await.is_err()
    {
        return;
    }

    if let Some(close_rx) = close_rx {
        let _ = close_rx.await;
        let _ = write_half.shutdown().await;
        let _ = closed_tx
            .expect("first connection must carry close acknowledgement")
            .send(());
    } else {
        let _ = write_half.shutdown().await;
    }
}

async fn read_http_request(reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>) -> Option<()> {
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).await.ok()? == 0 {
        return None;
    }

    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await.ok()? == 0 {
            return None;
        }
        if line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().ok()?;
            }
        }
    }

    if content_length > 0 {
        let mut body = vec![0u8; content_length];
        tokio::io::AsyncReadExt::read_exact(reader, &mut body)
            .await
            .ok()?;
    }
    Some(())
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
async fn retry_total_deadline_caps_across_attempts() {
    // Every response is a delayed retryable 503. The original total
    // deadline of 250ms must cap the whole retry sequence; restarting
    // the full `total` per attempt would spend ~1s across 10 attempts.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
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
                tokio::time::sleep(Duration::from_millis(100)).await;
                let body = b"unavailable".as_slice();
                let response = format!(
                    "HTTP/1.1 503 Service Unavailable\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.ok();
                stream.write_all(body).await.ok();
            });
        }
    });

    let policy = RetryPolicy::builder()
        .max_attempts(10)
        .backoff_factor(0.0)
        .initial_delay(Duration::from_millis(1))
        .max_delay(Duration::from_millis(1))
        .build();
    let timeout = Timeout::builder().total(Duration::from_millis(250)).build();

    let client = Client::builder().retry(policy).build();
    let url = format!("http://127.0.0.1:{port}/");
    let start = std::time::Instant::now();
    let result = client.get(&url).unwrap().timeout(timeout).send().await;
    let elapsed = start.elapsed();

    assert!(
        elapsed < Duration::from_millis(500),
        "total deadline should cap retries across attempts, took {elapsed:?}"
    );
    match result {
        Err(Error::Timeout { phase, .. }) => assert_eq!(phase, TimeoutPhase::Total),
        other => panic!("expected total-deadline timeout error, got {other:?}"),
    }
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

#[tokio::test]
async fn stale_idle_default_retries_transparently() {
    let mut server = StaleIdleServer::start().await;
    let client = Client::new();
    let url = server.url();

    let mut first = client.get(&url).unwrap().send().await.unwrap();
    assert_eq!(first.bytes().await.unwrap().as_ref(), b"ok");
    server.close_idle_connection().await;

    let mut second = client.get(&url).unwrap().send().await.unwrap();
    assert_eq!(second.status(), 200);
    assert_eq!(second.bytes().await.unwrap().as_ref(), b"ok");
    assert_eq!(server.request_count.load(Ordering::SeqCst), 2);
    assert_eq!(server.connection_count.load(Ordering::SeqCst), 2);
    server.shutdown().await;
}

#[tokio::test]
async fn stale_idle_strict_mode_surfaces_transport_failure() {
    let mut server = StaleIdleServer::start().await;
    let client = Client::builder().retry_canceled_requests(false).build();
    let url = server.url();

    let mut first = client.get(&url).unwrap().send().await.unwrap();
    assert_eq!(first.bytes().await.unwrap().as_ref(), b"ok");
    server.close_idle_connection().await;

    let result = client.get(&url).unwrap().send().await;
    let error = result.expect_err("strict mode must not transparently retry");
    assert_eq!(error.kind(), "hyper_client");
    assert!(StdError::source(&error).is_some());
    assert_eq!(server.request_count.load(Ordering::SeqCst), 1);
    assert_eq!(server.connection_count.load(Ordering::SeqCst), 1);
    server.shutdown().await;
}

#[tokio::test]
async fn stale_idle_strict_mode_allows_explicit_retry() {
    let mut server = StaleIdleServer::start().await;
    let policy = RetryPolicy::builder()
        .max_attempts(2)
        .backoff_factor(0.0)
        .build();
    let client = Client::builder()
        .retry_canceled_requests(false)
        .retry(policy)
        .build();
    let url = server.url();

    let mut first = client.get(&url).unwrap().send().await.unwrap();
    assert_eq!(first.bytes().await.unwrap().as_ref(), b"ok");
    server.close_idle_connection().await;

    let mut second = client.get(&url).unwrap().send().await.unwrap();
    assert_eq!(second.status(), 200);
    assert_eq!(second.bytes().await.unwrap().as_ref(), b"ok");
    assert_eq!(server.request_count.load(Ordering::SeqCst), 2);
    assert_eq!(server.connection_count.load(Ordering::SeqCst), 2);
    server.shutdown().await;
}

#[tokio::test]
async fn stale_idle_strict_mode_does_not_retry_one_shot_body() {
    let mut server = StaleIdleServer::start().await;
    let client = Client::builder().retry_canceled_requests(false).build();
    let url = server.url();
    let mut first = client.get(&url).unwrap().send().await.unwrap();
    assert_eq!(first.bytes().await.unwrap().as_ref(), b"ok");
    server.close_idle_connection().await;

    let body = RequestBody::from_stream(
        futures_util::stream::once(async {
            Ok::<bytes::Bytes, Error>(bytes::Bytes::from_static(b"body"))
        }),
        Some(4),
    );
    let result = client.post(&url).unwrap().body(body).send().await;
    assert!(
        result.is_err(),
        "strict mode must surface the stale-idle failure"
    );
    assert_eq!(server.request_count.load(Ordering::SeqCst), 1);
    assert_eq!(server.connection_count.load(Ordering::SeqCst), 1);
    server.shutdown().await;
}

#[tokio::test]
async fn retry_honors_retry_after_over_backoff() {
    // First response is a retryable 503 carrying `Retry-After: 1`.
    // With respect_retry_after enabled, the server-directed delay must
    // take precedence over the configured backoff (whose initial delay
    // alone would exceed the whole-test bound below).
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let request_count = Arc::new(AtomicUsize::new(0));
    let rc = request_count.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            let rc = rc.clone();
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
                let count = rc.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    stream
                        .write_all(
                            b"HTTP/1.1 503 Service Unavailable\r\nRetry-After: 1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        )
                        .await
                        .ok();
                } else {
                    let body = b"ok".as_slice();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    stream.write_all(response.as_bytes()).await.ok();
                    stream.write_all(body).await.ok();
                }
            });
        }
    });

    let policy = RetryPolicy::builder()
        .max_attempts(2)
        .respect_retry_after(true)
        .initial_delay(Duration::from_secs(30))
        .max_delay(Duration::from_secs(30))
        .build();

    let client = Client::builder().build();
    let url = format!("http://127.0.0.1:{port}/");
    let start = std::time::Instant::now();
    let resp = client
        .get(&url)
        .unwrap()
        .retry(policy)
        .send()
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert_eq!(resp.status(), 200);
    assert_eq!(request_count.load(Ordering::SeqCst), 2);
    assert!(
        elapsed < Duration::from_secs(15),
        "Retry-After (1s) should replace the 30s backoff delay, took {elapsed:?}"
    );
}
