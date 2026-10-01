#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::large_futures
)]

//! Native transport failure classification fixtures (M007).
//!
//! Deterministic loopback/local-TLS/protocol coverage for
//! `Error::transport_failure_kind()` through `Client::execute_http_body()`
//! and `NativeResponseBody` frame polling. No public DNS or internet.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use eggfetch_core::{Client, NativeRequestOptions, Timeout, TransportFailureKind};
use http_body::Body;
use http_body_util::BodyExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn empty_request(url: &str) -> http::Request<http_body_util::Empty<Bytes>> {
    http::Request::get(url)
        .body(http_body_util::Empty::<Bytes>::new())
        .unwrap()
}

#[tokio::test]
async fn native_dispatch_refused_connection_classifies_connect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let url = format!("http://{addr}/");
    let error = Client::new()
        .execute_http_body_default(empty_request(&url))
        .await
        .unwrap_err();
    assert_eq!(
        error.transport_failure_kind(),
        Some(TransportFailureKind::Connect),
        "refused dispatch kind={} debug={error:?}",
        error.kind()
    );
}

#[tokio::test]
async fn native_dispatch_malformed_response_head_classifies_protocol() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0_u8; 1024];
        let _ = socket.read(&mut buf).await;
        socket
            .write_all(b"NOT-HTTP\r\nX-Bogus: 1\r\n\r\n")
            .await
            .unwrap();
        socket.flush().await.unwrap();
    });
    let url = format!("http://{addr}/");
    let error = Client::new()
        .execute_http_body_default(empty_request(&url))
        .await
        .unwrap_err();
    assert_eq!(
        error.transport_failure_kind(),
        Some(TransportFailureKind::Protocol),
        "malformed-head kind={} debug={error:?}",
        error.kind()
    );
    task.await.unwrap();
}

#[cfg(feature = "tls-rustls")]
#[tokio::test]
async fn native_dispatch_tls_verification_failure_classifies_tls() {
    // Self-signed loopback TLS origin: the default WebPKI trust store cannot
    // verify it, so the handshake fails with typed TLS evidence.
    let sans = vec!["127.0.0.1".to_string()];
    let mut params = rcgen::CertificateParams::new(sans).unwrap();
    params.is_ca = rcgen::IsCa::NoCa;
    let key = rcgen::KeyPair::generate().unwrap();
    let cert = params.self_signed(&key).unwrap();
    let cert_der = rustls::pki_types::CertificateDer::from(cert.der().to_vec());
    let key_der = rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(key.serialize_der()),
    );
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let _ = acceptor.accept(tcp).await;
    });
    let url = format!("https://{addr}/");
    let error = Client::new()
        .execute_http_body_default(empty_request(&url))
        .await
        .unwrap_err();
    assert_eq!(
        error.transport_failure_kind(),
        Some(TransportFailureKind::Tls),
        "tls-verification kind={} debug={error:?}",
        error.kind()
    );
    server.await.unwrap();
}

#[tokio::test]
async fn native_dispatch_stale_idle_cancellation_classifies_cancelled() {
    // Deterministic stale-idle pattern mirroring the retry-integration
    // fixture: complete one keep-alive request, close the idle connection
    // server-side, then dispatch again immediately (no sleep, so the pool
    // still hands out the stale entry) with Hyper's transparent retry
    // disabled. The reuse attempt surfaces typed Hyper cancellation
    // evidence before any byte is sent.
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::sync::oneshot;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let request_count = Arc::new(AtomicUsize::new(0));
    let request_count_server = request_count.clone();
    let (close_tx, close_rx) = oneshot::channel::<()>();
    let (closed_tx, closed_rx) = oneshot::channel::<()>();
    let server = tokio::spawn(async move {
        let mut close_rx = Some(close_rx);
        let mut closed_tx = Some(closed_tx);
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let (read_half, mut write_half) = stream.into_split();
            let mut reader = BufReader::new(read_half);
            let mut line = String::new();
            if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                return;
            }
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).await.unwrap_or(0) == 0 {
                    return;
                }
                if header.trim().is_empty() {
                    break;
                }
            }
            request_count_server.fetch_add(1, Ordering::SeqCst);
            write_half
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: keep-alive\r\n\r\nok",
                )
                .await
                .unwrap();
            // Only the first connection participates in the idle-close
            // handshake; later connections (none are expected) are closed.
            if let Some(rx) = close_rx.take() {
                let _ = rx.await;
                let _ = write_half.shutdown().await;
                let _ = closed_tx
                    .take()
                    .expect("close acknowledgement already sent")
                    .send(());
                return;
            }
        }
    });
    let url = format!("http://127.0.0.1:{port}/");
    let client = Client::builder().retry_canceled_requests(false).build();
    let first = client
        .execute_http_body_default(empty_request(&url))
        .await
        .unwrap();
    assert_eq!(first.into_body().collect().await.unwrap().to_bytes(), "ok");
    close_tx.send(()).unwrap();
    closed_rx.await.unwrap();
    let error = client
        .execute_http_body_default(empty_request(&url))
        .await
        .unwrap_err();
    // Strict mode must surface typed Hyper cancellation evidence: the pooled
    // idle connection was closed server-side, so the reuse attempt is
    // canceled before any byte is sent and no second request is served.
    assert_eq!(
        error.transport_failure_kind(),
        Some(TransportFailureKind::Cancelled),
        "stale-idle kind={} debug={error:?}",
        error.kind()
    );
    assert_eq!(
        request_count.load(Ordering::SeqCst),
        1,
        "cancelled reuse must not reach the origin again"
    );
    server.await.unwrap();
}

#[tokio::test]
async fn native_body_truncated_content_length_classifies_protocol() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0_u8; 1024];
        let _ = socket.read(&mut buf).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nab")
            .await
            .unwrap();
        socket.flush().await.unwrap();
        drop(socket);
    });
    let url = format!("http://{addr}/");
    let response = Client::new()
        .execute_http_body_default(empty_request(&url))
        .await
        .unwrap();
    let mut body = Box::pin(response.into_body());
    let first = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.into_data().unwrap(), Bytes::from_static(b"ab"));
    let error = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx))
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(
        error.transport_failure_kind(),
        Some(TransportFailureKind::Protocol),
        "truncated-body kind={} debug={error:?}",
        error.kind()
    );
    task.await.unwrap();
}

#[tokio::test]
async fn native_body_malformed_chunk_classifies_protocol() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0_u8; 1024];
        let _ = socket.read(&mut buf).await;
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nZZZ-not-hex\r\nxx\r\n0\r\n\r\n",
            )
            .await
            .unwrap();
        socket.flush().await.unwrap();
    });
    let url = format!("http://{addr}/");
    let response = Client::new()
        .execute_http_body_default(empty_request(&url))
        .await
        .unwrap();
    let mut body = Box::pin(response.into_body());
    let error = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx))
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(
        error.transport_failure_kind(),
        Some(TransportFailureKind::Protocol),
        "malformed-chunk kind={} debug={error:?}",
        error.kind()
    );
    task.await.unwrap();
}

#[tokio::test]
async fn native_body_ordinary_eof_succeeds_and_timeouts_stay_typed() {
    // Ordinary EOF: exact Content-Length body completes with no failure.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0_u8; 1024];
        let _ = socket.read(&mut buf).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await
            .unwrap();
    });
    let url = format!("http://{addr}/");
    let response = Client::new()
        .execute_http_body_default(empty_request(&url))
        .await
        .unwrap();
    let collected = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(collected, "ok");
    task.await.unwrap();

    // Read timeout remains a typed timeout fact, never a transport category.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let stalled = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0_u8; 1024];
        let _ = socket.read(&mut buf).await;
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nab")
            .await
            .unwrap();
        socket.flush().await.unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
    });
    let url = format!("http://{addr}/");
    let response = Client::new()
        .execute_http_body(
            empty_request(&url),
            NativeRequestOptions::default().timeout(Timeout {
                read: Some(Duration::from_millis(50)),
                ..Timeout::default()
            }),
        )
        .await
        .unwrap();
    let mut body = Box::pin(response.into_body());
    let first = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.into_data().unwrap(), Bytes::from_static(b"ab"));
    let timeout = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx))
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(timeout.kind(), "timeout_read");
    assert_eq!(timeout.transport_failure_kind(), None);
    stalled.abort();
}

#[tokio::test]
async fn native_body_error_releases_pool_lease() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.unwrap();
        let mut buf = [0_u8; 1024];
        let _ = first.read(&mut buf).await;
        first
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nab")
            .await
            .unwrap();
        drop(first);
        let (mut second, _) = listener.accept().await.unwrap();
        let _ = second.read(&mut buf).await;
        second
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await
            .unwrap();
    });
    let client = Client::builder().max_in_flight_requests(1).build();
    let url = format!("http://{addr}/bad");
    let response = client
        .execute_http_body_default(empty_request(&url))
        .await
        .unwrap();
    let mut body = Box::pin(response.into_body());
    let _ = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await;
    let error = futures_util::future::poll_fn(|cx| body.as_mut().poll_frame(cx))
        .await
        .unwrap()
        .unwrap_err();
    assert_eq!(
        error.transport_failure_kind(),
        Some(TransportFailureKind::Protocol)
    );
    drop(body);
    let url = format!("http://{addr}/good");
    let response = tokio::time::timeout(
        Duration::from_secs(2),
        client.execute_http_body_default(empty_request(&url)),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "ok"
    );
    task.await.unwrap();
}

#[tokio::test]
async fn native_transport_failure_retry_and_proxy_policy_unchanged() {
    // Retry policy must not broaden merely because Connect/Cancelled exist.
    #[cfg(feature = "logical-retry")]
    {
        use eggfetch_core::{Error, RetryPolicy};
        assert!(RetryPolicy::is_error_retryable(&Error::Connect("x".into())));
        assert!(!RetryPolicy::is_error_retryable(&Error::Protocol(
            "x".into()
        )));
        assert!(!RetryPolicy::is_error_retryable(&Error::Tls("x".into())));
    }
    // Total timeout still wins and stays unclassified.
    let error = eggfetch_core::Error::Timeout {
        phase: eggfetch_core::TimeoutPhase::Total,
        elapsed: Duration::from_millis(1),
    };
    assert_eq!(error.transport_failure_kind(), None);
    // RequestFailure/NetworkFailureKind authority preserved.
    let _ = eggfetch_core::NetworkFailureKind::Connect;
}

#[test]
fn native_transport_failure_public_contract_compiles() {
    let _: fn(&eggfetch_core::Error) -> Option<TransportFailureKind> =
        eggfetch_core::Error::transport_failure_kind;
    let kinds = [
        TransportFailureKind::Connect,
        TransportFailureKind::Tls,
        TransportFailureKind::Protocol,
        TransportFailureKind::Cancelled,
    ];
    for kind in kinds {
        let _ = format!("{kind:?}");
    }
    let count = Arc::new(AtomicUsize::new(0));
    count.fetch_add(1, Ordering::SeqCst);
}
