#![allow(missing_docs)]

//! Lean high-level policy profile coverage.
//!
//! Exercises the supported lean embedding recipe
//! (`native-http1` + `high-level-url` + `tls-rustls`, without
//! `logical-retry`, `redirects`, or `basic-auth`) through stable APIs only:
//! Bearer auth, timeouts, body limits, pooling, TLS, and typed failure
//! reporting. Every test here must also pass under the full
//! `http1`/`http2`/default compatibility profiles, where the same calls
//! take the no-policy-configured fast path.

mod test_server;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use eggfetch_core::{AuthScheme, Client, Error, NetworkFailureKind, Timeout, TimeoutPhase};
use test_server::{TestServer, TestServerConfig};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// Counting loopback server returning a fixed status for every request.
struct CountingServer {
    port: u16,
    shutdown: tokio::sync::watch::Sender<bool>,
    request_count: Arc<AtomicUsize>,
    seen_auth: Arc<std::sync::Mutex<Vec<String>>>,
    status: u16,
}

impl CountingServer {
    async fn start(status: u16) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
        let request_count = Arc::new(AtomicUsize::new(0));
        let seen_auth = Arc::new(std::sync::Mutex::new(Vec::new()));
        let rc = request_count.clone();
        let auth = seen_auth.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    result = listener.accept() => {
                        match result {
                            Ok((mut stream, _)) => {
                                rc.fetch_add(1, Ordering::SeqCst);
                                let auth = auth.clone();
                                tokio::spawn(async move {
                                    let mut reader = BufReader::new(&mut stream);
                                    let mut request_line = String::new();
                                    reader.read_line(&mut request_line).await.ok();
                                    loop {
                                        let mut line = String::new();
                                        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                                            break;
                                        }
                                        if line.trim().is_empty() {
                                            break;
                                        }
                                        if let Some(value) = line.strip_prefix("authorization:")
                                            .or_else(|| line.strip_prefix("Authorization:")) {
                                            auth.lock().unwrap().push(value.trim().to_owned());
                                        }
                                    }
                                    let reason = match status {
                                        200 => "OK",
                                        302 => "Found",
                                        503 => "Service Unavailable",
                                        _ => "Status",
                                    };
                                    let (extra, body) = if status == 302 {
                                        (
                                            "Location: http://127.0.0.1:1/final\r\n".to_owned(),
                                            Vec::new(),
                                        )
                                    } else {
                                        (String::new(), b"lean-body".to_vec())
                                    };
                                    let response = format!(
                                        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",
                                        body.len()
                                    );
                                    stream.write_all(response.as_bytes()).await.ok();
                                    stream.write_all(&body).await.ok();
                                });
                            }
                            Err(_) => break,
                        }
                    }
                    _ = shutdown_rx.changed() => break,
                }
            }
        });

        Self {
            port,
            shutdown: shutdown_tx,
            request_count,
            seen_auth,
            status,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }
}

#[tokio::test]
async fn lean_returns_503_with_single_attempt() {
    let server = CountingServer::start(503).await;
    let client = Client::new();
    let mut response = client.get(&server.url("/")).unwrap().send().await.unwrap();
    assert_eq!(response.status().as_u16(), 503);
    let body = response.bytes().await.unwrap();
    assert_eq!(body.as_ref(), b"lean-body");
    // No retry orchestration: one logical attempt, one server hit.
    assert_eq!(server.request_count.load(Ordering::SeqCst), 1);
    let _ = server.shutdown.send(true);
    let _ = server.status;
}

#[tokio::test]
async fn lean_returns_3xx_without_following() {
    let server = CountingServer::start(302).await;
    let client = Client::new();
    let response = client.get(&server.url("/")).unwrap().send().await.unwrap();
    assert_eq!(response.status().as_u16(), 302);
    // The redirect target is a closed port; following it would produce a
    // transport error instead of this 302. A single hit proves no hop.
    assert_eq!(server.request_count.load(Ordering::SeqCst), 1);
    // No redirect history is constructed on the lean path.
    assert!(response.history().is_empty());
    let _ = server.shutdown.send(true);
}

#[tokio::test]
async fn lean_bearer_header_present_and_redacted() {
    let server = CountingServer::start(200).await;
    let client = Client::builder()
        .auth(AuthScheme::bearer("lean-secret-token").unwrap())
        .build();
    let mut response = client.get(&server.url("/")).unwrap().send().await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
    response.bytes().await.unwrap();
    let seen = server.seen_auth.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0], "Bearer lean-secret-token");

    // Diagnostics redact the token.
    let scheme = AuthScheme::bearer("lean-secret-token").unwrap();
    let debug = format!("{scheme:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("lean-secret-token"));

    let req = client.get(&server.url("/")).unwrap().build().unwrap();
    let req_debug = format!("{req:?}");
    assert!(!req_debug.contains("lean-secret-token"));
    let _ = server.shutdown.send(true);
}

#[tokio::test]
async fn lean_typed_connection_refused() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let failure = Client::new()
        .get(&format!("http://{addr}/"))
        .unwrap()
        .send_detailed()
        .await
        .unwrap_err();
    assert_eq!(
        failure.network_failure_kind(),
        Some(NetworkFailureKind::ConnectionRefused)
    );
    // The ordinary error contract is unchanged.
    assert_eq!(failure.into_error().kind(), "hyper_client");
}

#[tokio::test]
async fn lean_total_timeout_enforced() {
    let mut server = TestServer::start(&TestServerConfig {
        response_delay_ms: 500,
        ..Default::default()
    });
    let client = Client::builder()
        .timeout(Timeout {
            total: Some(Duration::from_millis(50)),
            ..Timeout::default()
        })
        .build();
    let err = client.get(&server.url()).unwrap().send().await.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Timeout {
                phase: TimeoutPhase::Total,
                ..
            }
        ),
        "expected total timeout, got {err:?}"
    );
    server.shutdown();
}

#[tokio::test]
async fn lean_body_cap_enforced() {
    let mut server = TestServer::start(&TestServerConfig {
        response_body: Some(vec![b'x'; 4096]),
        ..Default::default()
    });
    let client = Client::builder().max_decoded_body_size(16).build();
    let err = client
        .get(&server.url())
        .unwrap()
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap_err();
    assert_eq!(err.kind(), "decoded_body_too_large");
    server.shutdown();
}
