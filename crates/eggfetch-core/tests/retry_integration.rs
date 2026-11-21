// This target exercises the retry policy and the redirect-following client builder, which the lean profile compiles out.
// It therefore requires `logical-retry` and `redirects` and compiles to an empty
// test binary under a profile that omits them.
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
#![cfg(all(feature = "logical-retry", feature = "redirects"))]
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
// Deterministic dropped-connection fixture
// ---------------------------------------------------------------------------

/// A loopback server that reads one request and then closes the connection
/// without writing a single response byte.
///
/// This makes a *mid-request* transport failure deterministic: the client has
/// already written the request when the socket goes away, so Hyper reports
/// `IncompleteMessage`.
///
/// It deliberately replaces the earlier stale-idle fixture, which closed a
/// pooled idle connection and asserted that the client had *not* yet observed
/// the FIN. Whether Hyper classifies that situation as `is_canceled`
/// (retryable) or `IncompleteMessage` (not retryable) depends on scheduling,
/// so those tests failed intermittently under load — on unmodified `main` as
/// well as here. A canceled error cannot be forced from the server side: when
/// the pool does observe the close it quietly opens a new connection, and when
/// it does not the resulting failure is a mid-request one. These tests
/// therefore pin the contract eggfetch owns — non-retryable transport failures
/// surface and are never retried, with or without a policy — instead of
/// Hyper's internal classification.
struct DroppedConnectionServer {
    port: u16,
    request_count: Arc<AtomicUsize>,
    shutdown: Option<oneshot::Sender<()>>,
    accept_handle: Option<tokio::task::JoinHandle<()>>,
}

impl DroppedConnectionServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let request_count = Arc::new(AtomicUsize::new(0));
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        let counter = request_count.clone();
        let accept_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        let Ok((stream, _)) = result else {
                            break;
                        };
                        let counter = counter.clone();
                        tokio::spawn(async move {
                            let (read_half, write_half) = stream.into_split();
                            let mut reader = BufReader::new(read_half);
                            if read_http_request(&mut reader).await.is_some() {
                                counter.fetch_add(1, Ordering::SeqCst);
                            }
                            // Close without writing a response: dropping the
                            // write half sends the FIN the client observes
                            // mid-response.
                            drop(write_half);
                        });
                    }
                    _ = &mut shutdown_rx => break,
                }
            }
        });

        Self {
            port,
            request_count,
            shutdown: Some(shutdown_tx),
            accept_handle: Some(accept_handle),
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/", self.port)
    }

    fn shutdown(&mut self) {
        let _ = self
            .shutdown
            .take()
            .expect("server already shut down")
            .send(());
        if let Some(handle) = self.accept_handle.take() {
            handle.abort();
        }
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
async fn dropped_connection_mid_request_surfaces_as_a_transport_error() {
    let mut server = DroppedConnectionServer::start().await;
    let client = Client::new();

    let error = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect_err("a dropped connection must surface as an error");
    assert_eq!(error.kind(), "hyper_client");
    assert!(StdError::source(&error).is_some());
    assert_eq!(server.request_count.load(Ordering::SeqCst), 1);
    server.shutdown();
}

#[tokio::test]
async fn dropped_connection_mid_request_is_not_retried_by_a_policy() {
    // A mid-request failure may already have reached the server, so it is
    // never replayed: is_error_retryable excludes it and a configured policy
    // must not mask it.
    let mut server = DroppedConnectionServer::start().await;
    let policy = RetryPolicy::builder()
        .max_attempts(3)
        .backoff_factor(0.0)
        .build();
    let client = Client::builder().retry(policy).build();

    let error = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .expect_err("a dropped connection must not be retried");
    assert_eq!(error.kind(), "hyper_client");
    assert_eq!(server.request_count.load(Ordering::SeqCst), 1);
    server.shutdown();
}

#[tokio::test]
async fn dropped_connection_mid_request_is_not_retried_for_a_one_shot_body() {
    let mut server = DroppedConnectionServer::start().await;
    let client = Client::builder().retry_canceled_requests(false).build();
    let body = RequestBody::from_stream(
        futures_util::stream::once(async {
            Ok::<bytes::Bytes, Error>(bytes::Bytes::from_static(b"body"))
        }),
        Some(4),
    );

    let result = client.post(&server.url()).unwrap().body(body).send().await;
    assert!(result.is_err(), "a dropped connection must surface");
    assert_eq!(server.request_count.load(Ordering::SeqCst), 1);
    server.shutdown();
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

#[tokio::test]
async fn retry_final_body_uses_remaining_original_budget() {
    // Explicit logical-retry -> successful headers -> slow final body must
    // observe the remaining original `Timeout.total`, not a fresh total.
    //
    // total = 1500ms, attempt 1 consumes ~800ms then 503, backoff ~50ms,
    // remaining ~= 650ms, discrimination = 1000ms, fresh = 1500ms.
    let total = Duration::from_millis(1500);
    let discrimination = Duration::from_millis(1000);

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
                    // Material attempt-1 cost before the retryable failure.
                    tokio::time::sleep(Duration::from_millis(800)).await;
                    stream
                        .write_all(
                            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        )
                        .await
                        .ok();
                } else {
                    // Successful final headers immediately, then stall the
                    // body well beyond both the remaining budget and a fresh
                    // total so only the deadline decides the outcome.
                    stream
                        .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
                        .await
                        .ok();
                    stream.flush().await.ok();
                    tokio::time::sleep(Duration::from_millis(3000)).await;
                    stream.write_all(b"0\r\n\r\n").await.ok();
                }
            });
        }
    });

    let policy = RetryPolicy::builder()
        .max_attempts(3)
        .backoff_factor(0.0)
        .initial_delay(Duration::from_millis(50))
        .max_delay(Duration::from_millis(50))
        .build();
    let client = Client::builder().retry(policy).build();
    let url = format!("http://127.0.0.1:{port}/");
    let mut resp = client
        .get(&url)
        .unwrap()
        .timeout(Timeout {
            total: Some(total),
            ..Timeout::default()
        })
        .send()
        .await
        .expect("retry must reach final headers before the original total");

    assert_eq!(
        request_count.load(Ordering::SeqCst),
        2,
        "explicit logical retry must have produced two attempts"
    );
    assert_eq!(resp.status(), 200);

    // Final body must fail with Total inside the discrimination window. A
    // fresh total after retry would still be pending here.
    let body_start = std::time::Instant::now();
    let outcome = tokio::time::timeout(discrimination, resp.bytes()).await;
    let body_elapsed = body_start.elapsed();
    let result = outcome.expect(
        "final body must resolve within the discrimination window; \
         a fresh total after retry would still be pending",
    );
    match result {
        Err(Error::Timeout {
            phase: TimeoutPhase::Total,
            ..
        }) => {}
        other => panic!("expected final body Total, got {other:?}"),
    }
    assert!(
        body_elapsed < discrimination,
        "final body must use the remaining original budget; body_elapsed={body_elapsed:?}"
    );
}
